//! The write-ahead journal (J2, J4, J10, J11, J12 and J16 of the Journal PRD).
//!
//! The journal is a sequence of files `krummi.journal.<19 digits>` in the store's folder, numbered one by one. Each
//! commit is one record, aligned to [`ALIGN`] bytes so that recovery can find the records after a damaged one:
//!
//! - a 64-byte header: magic, commit number (`txid`), `durable_upto` (the last commit that was durable when the
//!   record was written, J9), page count, flags, stored and raw payload lengths, payload checksum and header
//!   checksum;
//! - the payload: the commit's pages as (page number, sealed page) pairs, compressed with LZ4 when that makes it
//!   smaller (J16). The pages of a value run are not sealed one by one: their number carries [`RAW_ENTRY`] (D5 of
//!   the large values execution).
//!
//! Records are appended to the current file. A file is created when a record needs it, with its final size
//! ([`DEFAULT_FILE_LIMIT`], or more for a record that does not fit in it) reserved on disk, and its name and size
//! are made durable before any record is written into it, so writing never changes its size (E7 of the grouped
//! journal PRD). Files are written directly to the disk when the system allows it, and through the system's cache
//! otherwise (E6). A commit reserves its place and enters a queue under the write turn; the journal's own thread
//! writes everything queued together and makes it durable (E2, E4). After a group fails, nothing more is written
//! until the close retries it (E8, E12).

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use xxhash_rust::xxh3::xxh3_64;

use crate::error::{Error, Result};
use crate::format::PAGE_SIZE;
use crate::pager::Pager;
use crate::sys::{self, AlignedBuf};

/// Prefix of the journal files' names (J2).
pub const JOURNAL_PREFIX: &str = "krummi.journal.";
/// Records start at multiples of this many bytes of their file.
pub(crate) const ALIGN: u64 = 4096;
/// Size of a record header.
pub(crate) const RECORD_HEADER: usize = 64;
/// Size of a journal file (E7). Builds with test hooks use a smaller one, because tests copy and alter whole files.
#[cfg(not(any(test, feature = "test-hooks")))]
pub(crate) const DEFAULT_FILE_LIMIT: u64 = RELEASE_FILE_LIMIT;
#[cfg(any(test, feature = "test-hooks"))]
pub(crate) const DEFAULT_FILE_LIMIT: u64 = 256 << 10;
/// Size of a journal file outside builds with test hooks.
pub const RELEASE_FILE_LIMIT: u64 = 16 << 20;
const MAGIC: u64 = u64::from_le_bytes(*b"KrummiJ1");
const FLAG_COMPRESSED: u32 = 1;
/// Bytes of one page in a raw payload: its number and the sealed page.
pub(crate) const PAGE_ENTRY: usize = 8 + PAGE_SIZE;
/// Flag of the page number of an entry whose page is part of a value run (D5 of the large values execution): it is
/// not sealed on its own, so recovery writes it without checking it, and the record's checksum covers it. Its run
/// is checked as a whole when it is read (D1).
pub(crate) const RAW_ENTRY: u64 = 1 << 63;

/// The name of journal file number `number`.
pub(crate) fn file_name(number: u64) -> String {
    format!("{JOURNAL_PREFIX}{number:019}")
}

/// The number of a journal file from its name, if it is one (case-insensitively on Windows, like the other
/// reserved names).
pub(crate) fn parse_file_name(name: &OsStr) -> Option<u64> {
    let name = name.to_str()?;
    let (prefix, digits) = name.split_at_checked(JOURNAL_PREFIX.len())?;
    let prefix_matches = if cfg!(windows) {
        prefix.eq_ignore_ascii_case(JOURNAL_PREFIX)
    } else {
        prefix == JOURNAL_PREFIX
    };
    if !prefix_matches || digits.len() != 19 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The fixed part of a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordHeader {
    pub txid: u64,
    pub durable_upto: u64,
    pub page_count: u32,
    pub compressed: bool,
    pub stored_len: u64,
    pub raw_len: u64,
    pub payload_checksum: u64,
}

impl RecordHeader {
    fn encode(&self) -> [u8; RECORD_HEADER] {
        let mut out = [0u8; RECORD_HEADER];
        out[0..8].copy_from_slice(&MAGIC.to_le_bytes());
        out[8..16].copy_from_slice(&self.txid.to_le_bytes());
        out[16..24].copy_from_slice(&self.durable_upto.to_le_bytes());
        out[24..28].copy_from_slice(&self.page_count.to_le_bytes());
        let flags = if self.compressed { FLAG_COMPRESSED } else { 0 };
        out[28..32].copy_from_slice(&flags.to_le_bytes());
        out[32..40].copy_from_slice(&self.stored_len.to_le_bytes());
        out[40..48].copy_from_slice(&self.raw_len.to_le_bytes());
        out[48..56].copy_from_slice(&self.payload_checksum.to_le_bytes());
        let checksum = xxh3_64(&out[..56]);
        out[56..64].copy_from_slice(&checksum.to_le_bytes());
        out
    }

    /// Decodes a record header, or `None` when it is not an intact one.
    pub(crate) fn decode(bytes: &[u8]) -> Option<RecordHeader> {
        let bytes = bytes.get(..RECORD_HEADER)?;
        let word = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"));
        let half = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
        if word(0) != MAGIC || word(56) != xxh3_64(&bytes[..56]) {
            return None;
        }
        let header = RecordHeader {
            txid: word(8),
            durable_upto: word(16),
            page_count: half(24),
            compressed: half(28) & FLAG_COMPRESSED != 0,
            stored_len: word(32),
            raw_len: word(40),
            payload_checksum: word(48),
        };
        let consistent = header.raw_len == u64::from(header.page_count) * PAGE_ENTRY as u64
            && (header.compressed || header.stored_len == header.raw_len)
            && header.txid > 0;
        consistent.then_some(header)
    }

    /// Bytes the whole record takes in its file, padding included.
    pub(crate) fn record_len(&self) -> u64 {
        (RECORD_HEADER as u64 + self.stored_len).next_multiple_of(ALIGN)
    }
}

/// The raw payload of a record from its stored bytes, after checking them; `None` when they are damaged.
pub(crate) fn decode_payload(header: &RecordHeader, stored: &[u8]) -> Option<Vec<u8>> {
    if xxh3_64(stored) != header.payload_checksum {
        return None;
    }
    if !header.compressed {
        return Some(stored.to_vec());
    }
    let mut raw = Vec::new();
    raw.try_reserve_exact(header.raw_len as usize).ok()?;
    raw.resize(header.raw_len as usize, 0);
    match lz4_flex::block::decompress_into(stored, &mut raw) {
        Ok(n) if n == raw.len() => Some(raw),
        _ => None,
    }
}

/// Encodes a whole record, padded to [`ALIGN`], in memory aligned for direct writes.
fn encode_record(
    txid: u64,
    durable_upto: u64,
    pages: &[(u64, &[u8; PAGE_SIZE])],
) -> Result<AlignedBuf> {
    let raw_len = pages.len() * PAGE_ENTRY;
    let out_of_memory = |requested: usize| Error::OutOfMemory {
        requested: requested as u64,
    };
    let mut raw = Vec::new();
    raw.try_reserve_exact(raw_len)
        .map_err(|_| out_of_memory(raw_len))?;
    for (number, page) in pages {
        raw.extend_from_slice(&number.to_le_bytes());
        raw.extend_from_slice(&page[..]);
    }
    let bound = lz4_flex::block::get_maximum_output_size(raw_len);
    let mut compressed = Vec::new();
    compressed
        .try_reserve_exact(bound)
        .map_err(|_| out_of_memory(bound))?;
    compressed.resize(bound, 0);
    let stored_len = match lz4_flex::block::compress_into(&raw, &mut compressed) {
        Ok(n) if n < raw_len => Some(n),
        _ => None,
    };
    let payload: &[u8] = match stored_len {
        Some(n) => &compressed[..n],
        None => &raw,
    };
    let header = RecordHeader {
        txid,
        durable_upto,
        page_count: pages.len() as u32,
        compressed: stored_len.is_some(),
        stored_len: payload.len() as u64,
        raw_len: raw_len as u64,
        payload_checksum: xxh3_64(payload),
    };
    let len = header.record_len() as usize;
    let mut record = AlignedBuf::zeroed(len).map_err(|_| out_of_memory(len))?;
    record[..RECORD_HEADER].copy_from_slice(&header.encode());
    record[RECORD_HEADER..RECORD_HEADER + payload.len()].copy_from_slice(payload);
    Ok(record)
}

/// Bytes of records the journal queue holds before a commit waits for room (E2 of the grouped journal PRD).
pub(crate) const DEFAULT_QUEUE_LIMIT: u64 = 8 << 20;

const _: () = assert!(
    ALIGN.is_multiple_of(sys::DIRECT_ALIGN as u64),
    "records must start where direct writes can"
);

/// A function the journal calls; tests use them to stop it at a known point.
pub(crate) type Hook = Arc<dyn Fn() + Send + Sync>;

/// The journal of a store on disk.
///
/// A commit reserves its place in a file and enters the queue under the write turn (its point of no return, E2);
/// the journal's own thread takes everything in the queue, writes it in one write per file, makes it durable, and
/// only then counts the group as durable (E4). After a group fails, nothing more is written until the close
/// retries it (E8, E12).
pub(crate) struct Journal {
    shared: Arc<Shared>,
    writer: Mutex<Option<JoinHandle<()>>>,
}

struct Shared {
    folder: PathBuf,
    state: Mutex<State>,
    /// Wakes the writer: records queued, writing resumed, or the close.
    work: Condvar,
    /// Wakes whoever waits for the writer: a group durable, room in the queue, or a failure.
    progress: Condvar,
    /// Last durable commit, which snapshots read without the lock (E3).
    durable: AtomicU64,
    file_limit: AtomicU64,
    queue_limit: AtomicU64,
    /// Whether new files are opened for direct writes (E6).
    direct: bool,
    /// The store's flag that makes it refuse writes (G12, E8).
    writes_disabled: Arc<AtomicBool>,
    /// Called by the writer between writing a group and making it durable (tests: AC-J13).
    sync_hook: Mutex<Option<Hook>>,
    /// Called by the writer when a group fails, so the transaction layer can wake its waiting requests (T12).
    failure_hook: Mutex<Option<Hook>>,
    /// Called by the writer when a group is durable, so its versions can go to `krummi.data` (J5).
    durable_hook: Mutex<Option<Hook>>,
    /// Writes of groups so far, one per file a group touches (tests: AC-E2, AC-E8).
    writes: AtomicU64,
}

struct State {
    /// The file the next record goes into.
    current: Option<Arc<OpenFile>>,
    /// Where the next record goes in `current`.
    current_len: u64,
    /// The size of `current`, reserved when it was created (E7).
    current_capacity: u64,
    /// Number of the next file to create.
    next_number: u64,
    /// Last commit that passed its point of no return.
    queued_upto: u64,
    /// Last commit known to be durable (J4, J10).
    synced_upto: u64,
    /// Files that exist, oldest first, with the last commit each holds (the current one included).
    files: VecDeque<(u64, u64)>,
    /// Records waiting to be written, in commit order.
    queue: VecDeque<Queued>,
    queued_bytes: u64,
    /// Why the last group failed; until the close retries it nothing more is written (E8, E12).
    failed: Option<(io::ErrorKind, String)>,
    /// The writer takes nothing while set (tests).
    paused: bool,
    /// The writer ends once nothing is left that it may write.
    stop: bool,
}

/// A journal file, shared by the records reserved in it.
struct OpenFile {
    file: File,
    path: PathBuf,
    /// Whether it takes direct writes, which are durable once written (E6).
    direct: bool,
}

impl OpenFile {
    /// Makes what was written to the file durable: a direct write already is, so it only counts as the flush (E6).
    fn flush(&self) -> io::Result<()> {
        if self.direct {
            sys::written_through(&self.path)
        } else {
            sys::sync(&self.file, &self.path)
        }
    }
}

/// A record that passed its point of no return and waits to be written.
struct Queued {
    txid: u64,
    file: Arc<OpenFile>,
    offset: u64,
    record: AlignedBuf,
}

impl Journal {
    /// Starts the journal of the store in `folder` and its writer thread, before recovery (E13): failing to start
    /// the thread is the out-of-memory class. It takes nothing until [`Journal::begin`].
    pub(crate) fn start(folder: &Path, pager: &Pager) -> Result<Journal> {
        let shared = Arc::new(Shared {
            folder: folder.to_path_buf(),
            state: Mutex::new(State {
                current: None,
                current_len: 0,
                current_capacity: 0,
                next_number: 0,
                queued_upto: 0,
                synced_upto: 0,
                files: VecDeque::new(),
                queue: VecDeque::new(),
                queued_bytes: 0,
                failed: None,
                paused: false,
                stop: false,
            }),
            work: Condvar::new(),
            progress: Condvar::new(),
            durable: AtomicU64::new(0),
            file_limit: AtomicU64::new(DEFAULT_FILE_LIMIT),
            queue_limit: AtomicU64::new(DEFAULT_QUEUE_LIMIT),
            direct: direct_writes(),
            writes_disabled: pager.failure_flag(),
            sync_hook: Mutex::new(None),
            failure_hook: Mutex::new(None),
            durable_hook: Mutex::new(None),
            writes: AtomicU64::new(0),
        });
        let out_of_memory = || Error::OutOfMemory { requested: 0 };
        sys::thread_point().map_err(|_| out_of_memory())?;
        #[cfg(any(test, feature = "test-hooks"))]
        let context = sys::faults::context();
        let writer = Arc::clone(&shared);
        let handle = std::thread::Builder::new()
            .name("krummi-journal".into())
            .spawn(move || {
                #[cfg(any(test, feature = "test-hooks"))]
                sys::faults::adopt(&context);
                writer.run();
            })
            .map_err(|_| out_of_memory())?;
        Ok(Journal {
            shared,
            writer: Mutex::new(Some(handle)),
        })
    }

    /// Sets where the journal goes on after recovery: commits from `last_txid + 1` on, in files numbered from
    /// `next_number`.
    pub(crate) fn begin(&self, last_txid: u64, next_number: u64) {
        let mut state = self.shared.lock();
        state.next_number = next_number;
        state.queued_upto = last_txid;
        state.synced_upto = last_txid;
        self.shared.durable.store(last_txid, Ordering::Release);
    }

    /// Sets the function the writer calls between writing a group and making it durable, or none (tests).
    pub(crate) fn set_sync_hook(&self, hook: Option<Hook>) {
        *self
            .shared
            .sync_hook
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = hook;
    }

    /// Sets the function the writer calls when a group fails, or none.
    pub(crate) fn set_failure_hook(&self, hook: Option<Hook>) {
        *self
            .shared
            .failure_hook
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = hook;
    }

    /// Sets the function the writer calls when a group is durable, or none (J5).
    pub(crate) fn set_durable_hook(&self, hook: Option<Hook>) {
        *self
            .shared
            .durable_hook
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = hook;
    }

    /// Sets the size of the files created from now on (tests).
    pub(crate) fn set_file_limit(&self, bytes: u64) {
        self.shared.file_limit.store(bytes, Ordering::Relaxed);
    }

    /// Sets the bytes the queue holds before a commit waits for room (tests: AC-E17).
    pub(crate) fn set_queue_limit(&self, bytes: u64) {
        self.shared.queue_limit.store(bytes, Ordering::Relaxed);
    }

    /// Stops the writer from taking records, or lets it go on (tests).
    pub(crate) fn set_paused(&self, paused: bool) {
        self.shared.lock().paused = paused;
        self.shared.work.notify_all();
    }

    /// Number of records waiting in the queue (tests).
    pub(crate) fn queued(&self) -> usize {
        self.shared.lock().queue.len()
    }

    /// Writes of groups so far, one per file a group touches (tests).
    pub(crate) fn writes(&self) -> u64 {
        self.shared.writes.load(Ordering::Acquire)
    }

    /// Wakes the commits waiting for room in the queue, after the store stopped accepting writes (T12).
    pub(crate) fn wake(&self) {
        drop(self.shared.lock());
        self.shared.progress.notify_all();
    }

    /// Last commit known to be durable (E3).
    pub(crate) fn durable(&self) -> u64 {
        self.shared.durable.load(Ordering::Acquire)
    }

    /// Reserves the place of commit `txid` in a journal file and puts its record in the queue: everything that can
    /// fail without the store refusing writes happens before, so after this the commit cannot be undone (E2, E7).
    /// Waits, first, while the queue is full; fails with the I/O class if the store stops accepting writes.
    ///
    /// Each entry is a page number and a whole page: sealed for that number, or a page of a value run, whose number
    /// carries [`RAW_ENTRY`].
    pub(crate) fn enqueue(
        &self,
        pager: &Pager,
        txid: u64,
        pages: &[(u64, &[u8; PAGE_SIZE])],
    ) -> Result<()> {
        pager.check_writable()?;
        let record = encode_record(txid, 0, pages)?;
        let len = record.len() as u64;
        let shared = &*self.shared;
        let mut state = shared.lock();
        while !state.queue.is_empty()
            && state.queued_bytes + len > shared.queue_limit.load(Ordering::Relaxed)
            && state.failed.is_none()
            && pager.accepts_writes()
        {
            state = shared
                .progress
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        if let Some(failure) = &state.failed {
            return Err(shared.failure_error(failure));
        }
        pager.check_writable()?;
        let fits = state
            .current
            .as_ref()
            .is_some_and(|_| state.current_len + len <= state.current_capacity);
        if !fits {
            self.roll(&mut state, pager, len)?;
        }
        let file = Arc::clone(state.current.as_ref().expect("a file was just created"));
        let offset = state.current_len;
        state.current_len += len;
        if let Some(last) = state.files.back_mut() {
            last.1 = txid;
        }
        state.queue.push_back(Queued {
            txid,
            file,
            offset,
            record,
        });
        state.queued_bytes += len;
        state.queued_upto = txid;
        drop(state);
        shared.work.notify_all();
        Ok(())
    }

    /// Goes on in a new file for a record of `record_len` bytes, created with its final size, which is made durable
    /// with its name before anything is written into it (E7). The writer makes durable what it writes into the old
    /// one.
    fn roll(&self, state: &mut State, pager: &Pager, record_len: u64) -> Result<()> {
        let shared = &*self.shared;
        let number = state.next_number;
        let path = shared.folder.join(file_name(number));
        let capacity = shared.file_limit.load(Ordering::Relaxed).max(record_len);
        let (file, direct) = sys::create_journal_file(&path, capacity, shared.direct)
            .map_err(|e| journal_error(pager, e))?;
        let made_durable = sys::sync(&file, &path).and_then(|()| sys::sync_dir(&shared.folder));
        if let Err(e) = made_durable {
            drop(file);
            let _ = sys::remove(&path);
            return Err(pager.fail_flush(e));
        }
        state.next_number += 1;
        state.files.push_back((number, state.queued_upto));
        state.current = Some(Arc::new(OpenFile { file, path, direct }));
        state.current_len = 0;
        state.current_capacity = capacity;
        Ok(())
    }

    /// Waits until commit `txid` is durable, or fails with the I/O class if its group failed (J4, E8).
    pub(crate) fn wait_durable(&self, txid: u64) -> Result<()> {
        let shared = &*self.shared;
        let mut state = shared.lock();
        loop {
            if state.synced_upto >= txid {
                return Ok(());
            }
            if let Some(failure) = &state.failed {
                return Err(shared.failure_error(failure));
            }
            state = shared
                .progress
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Waits until every commit that passed its point of no return is durable (J11, E10).
    pub(crate) fn make_durable(&self, pager: &Pager) -> Result<()> {
        pager.check_writable()?;
        let target = self.shared.lock().queued_upto;
        self.wait_durable(target)
    }

    /// Makes the next record go into a new file when the current one holds records, so that the old ones can all
    /// be removed once both header copies record them as applied (tests: AC-J18).
    pub(crate) fn start_new_file(&self, pager: &Pager) -> Result<()> {
        pager.check_writable()?;
        let mut state = self.shared.lock();
        if state.current.is_some() && state.current_len > 0 {
            state.current = None;
        }
        Ok(())
    }

    /// Writes and makes durable everything in the queue, retrying a group that failed, and stops the writer (E12).
    /// The writer is stopped even when this fails.
    pub(crate) fn close(&self, pager: &Pager) -> Result<()> {
        let shared = &*self.shared;
        shared.lock().stop = true;
        shared.work.notify_all();
        let handle = self
            .writer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(handle) = handle {
            // A writer that panicked left its group in the queue, which is written below.
            let _ = handle.join();
        }
        let mut state = shared.lock();
        state.failed = None;
        state.paused = false;
        let group: Vec<Queued> = state.queue.drain(..).collect();
        if group.is_empty() {
            return Ok(());
        }
        let durable_upto = state.synced_upto;
        drop(state);
        let written = shared.write_group(&group, durable_upto);
        shared.finish_group(group, written.map_err(|e| (e.kind(), e.to_string())));
        match &shared.lock().failed {
            Some(failure) => Err(pager.fail_flush(io::Error::new(failure.0, failure.1.clone()))),
            None => Ok(()),
        }
    }

    /// Number of the first file that may hold commits after `applied`: what a header with `applied` records.
    pub(crate) fn first_needed(&self, applied: u64) -> u64 {
        let state = self.shared.lock();
        state
            .files
            .iter()
            .find(|(_, last)| *last > applied)
            .map_or(state.next_number, |(number, _)| *number)
            .min(state.current_number().unwrap_or(u64::MAX))
    }

    /// Whether no journal file exists: nothing was committed since the store opened or the files were removed.
    pub(crate) fn has_no_files(&self) -> bool {
        self.shared.lock().files.is_empty()
    }

    /// Whether [`Journal::remove_files`] with `applied` would remove a file, the current one aside: whether
    /// recording `applied` in both header copies releases anything.
    pub(crate) fn can_remove_files(&self, applied: u64) -> bool {
        let state = self.shared.lock();
        state
            .files
            .front()
            .is_some_and(|&(number, last)| state.removable(number, last, applied, false))
    }

    /// Removes the files whose commits are all at most `applied` (the smaller of what the two header copies
    /// record), except the current one; or every file, the current one included, when `all`.
    pub(crate) fn remove_files(&self, pager: &Pager, applied: u64, all: bool) -> Result<()> {
        let mut state = self.shared.lock();
        while let Some(&(number, last)) = state.files.front() {
            if !state.removable(number, last, applied, all) {
                break;
            }
            if state.current_number() == Some(number) {
                state.current = None;
            }
            match sys::remove(&self.shared.folder.join(file_name(number))) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(journal_error(pager, e)),
            }
            state.files.pop_front();
        }
        Ok(())
    }
}

impl Drop for Journal {
    /// A journal dropped without its close (an opening that failed) stops its writer.
    fn drop(&mut self) {
        self.shared.lock().stop = true;
        self.shared.work.notify_all();
        let handle = self
            .writer
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }
}

impl State {
    /// Whether file `number`, whose last commit is `last`, may go with `applied`: the current file only when
    /// `all`. Every release decision goes through here.
    fn removable(&self, number: u64, last: u64, applied: u64, all: bool) -> bool {
        last <= applied && (all || self.current_number() != Some(number))
    }

    fn current_number(&self) -> Option<u64> {
        self.current
            .as_ref()
            .and(self.files.back().map(|(n, _)| *n))
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn failure_error(&self, failure: &(io::ErrorKind, String)) -> Error {
        Error::Io {
            folder: self.folder.clone(),
            source: io::Error::new(failure.0, failure.1.clone()),
        }
    }

    /// The writer thread: takes everything in the queue, writes it as one group and makes it durable, until the
    /// close. A panic counts as a failed group (E13).
    fn run(&self) {
        loop {
            let (group, durable_upto) = {
                let mut state = self.lock();
                loop {
                    let can_write =
                        !state.queue.is_empty() && !state.paused && state.failed.is_none();
                    if can_write {
                        break;
                    }
                    if state.stop {
                        return;
                    }
                    state = self
                        .work
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                let group: Vec<Queued> = state.queue.drain(..).collect();
                (group, state.synced_upto)
            };
            let written =
                panic::catch_unwind(AssertUnwindSafe(|| self.write_group(&group, durable_upto)));
            let stopped = written.is_err();
            let outcome = match written {
                Ok(result) => result.map_err(|e| (e.kind(), e.to_string())),
                Err(_) => Err((
                    io::ErrorKind::Other,
                    "the journal's writer stopped".to_owned(),
                )),
            };
            self.finish_group(group, outcome);
            if stopped {
                return;
            }
        }
    }

    /// Writes a group: each record says which commits were durable when it was written (J9), the records of each
    /// file go in one write, and then each file is made durable.
    fn write_group(&self, group: &[Queued], durable_upto: u64) -> io::Result<()> {
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for (i, queued) in group.iter().enumerate() {
            match runs.last_mut() {
                Some((start, end))
                    if Arc::ptr_eq(&group[*start].file, &queued.file)
                        && group[*end - 1].offset + group[*end - 1].record.len() as u64
                            == queued.offset =>
                {
                    *end = i + 1;
                }
                _ => runs.push((i, i + 1)),
            }
        }
        for &(start, end) in &runs {
            let records = &group[start..end];
            let file = &records[0].file;
            let total: usize = records.iter().map(|q| q.record.len()).sum();
            match AlignedBuf::zeroed(total) {
                Ok(mut buf) => {
                    let mut at = 0;
                    for queued in records {
                        let len = queued.record.len();
                        buf[at..at + len].copy_from_slice(&queued.record);
                        set_durable_upto(&mut buf[at..at + RECORD_HEADER], durable_upto);
                        at += len;
                    }
                    sys::write_at(&file.file, &file.path, records[0].offset, &buf[..total])?;
                }
                // No memory for one buffer: one write per record, still one flush.
                Err(_) => {
                    for queued in records {
                        let mut header = [0u8; RECORD_HEADER];
                        header.copy_from_slice(&queued.record[..RECORD_HEADER]);
                        set_durable_upto(&mut header, durable_upto);
                        let mut record = AlignedBuf::zeroed(queued.record.len())?;
                        record.copy_from_slice(&queued.record);
                        record[..RECORD_HEADER].copy_from_slice(&header);
                        sys::write_at(&file.file, &file.path, queued.offset, &record)?;
                    }
                }
            }
            self.writes.fetch_add(1, Ordering::AcqRel);
        }
        let hook = self
            .sync_hook
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(hook) = hook {
            hook();
        }
        let mut flushed: Vec<&Arc<OpenFile>> = Vec::new();
        for &(start, _) in &runs {
            let file = &group[start].file;
            if !flushed.iter().any(|f| Arc::ptr_eq(f, file)) {
                file.flush()?;
                flushed.push(file);
            }
        }
        Ok(())
    }

    /// Records the outcome of a group: durable, or failed, and then put back at the front of the queue for the
    /// close to retry, with the store refusing writes (E8).
    fn finish_group(
        &self,
        group: Vec<Queued>,
        outcome: std::result::Result<(), (io::ErrorKind, String)>,
    ) {
        let mut state = self.lock();
        match outcome {
            Ok(()) => {
                let last = group.last().map_or(state.synced_upto, |q| q.txid);
                state.synced_upto = state.synced_upto.max(last);
                state.queued_bytes -= group.iter().map(|q| q.record.len() as u64).sum::<u64>();
                self.durable.store(state.synced_upto, Ordering::Release);
                drop(state);
                let hook = self
                    .durable_hook
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone();
                if let Some(hook) = hook {
                    hook();
                }
            }
            Err(failure) => {
                for queued in group.into_iter().rev() {
                    state.queue.push_front(queued);
                }
                state.failed = Some(failure);
                self.writes_disabled.store(true, Ordering::Release);
                drop(state);
                let hook = self
                    .failure_hook
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone();
                if let Some(hook) = hook {
                    hook();
                }
            }
        }
        self.progress.notify_all();
    }
}

/// Rewrites, in an encoded record header, which commits were durable when it was written (J9).
fn set_durable_upto(header: &mut [u8], durable_upto: u64) {
    let mut decoded = RecordHeader::decode(header).expect("an encoded record header");
    decoded.durable_upto = durable_upto;
    header[..RECORD_HEADER].copy_from_slice(&decoded.encode());
}

/// Whether new journal files are opened for direct writes (E6). Builds with test hooks write through the system's
/// cache when `KRUMMI_JOURNAL_BUFFERED` is set, to run the journal's tests with the fallback too.
fn direct_writes() -> bool {
    #[cfg(any(test, feature = "test-hooks"))]
    {
        std::env::var_os("KRUMMI_JOURNAL_BUFFERED").is_none()
    }
    #[cfg(not(any(test, feature = "test-hooks")))]
    {
        true
    }
}

/// Classifies a failure to create or remove a journal file: a missing permission is the access class; the rest
/// like a failed write of the store.
fn journal_error(pager: &Pager, source: io::Error) -> Error {
    match source.kind() {
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => Error::Access {
            folder: pager.folder().map(Path::to_path_buf).unwrap_or_default(),
            source,
        },
        _ => pager.fail_write(source, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_round_trip_and_others_are_not_journal_files() {
        assert_eq!(file_name(1), "krummi.journal.0000000000000000001");
        assert_eq!(parse_file_name(OsStr::new(&file_name(42))), Some(42));
        for other in [
            "krummi.journal.1",
            "krummi.journal.00000000000000000001",
            "krummi.journal.000000000000000000a",
            "krummi.journal.0000000000000000001.bak",
            "krummi.data",
        ] {
            assert_eq!(parse_file_name(OsStr::new(other)), None, "{other}");
        }
    }

    #[test]
    fn records_round_trip_compressed_or_raw() {
        let mut same = [7u8; PAGE_SIZE];
        same[0] = 1;
        let mut noise = [0u8; PAGE_SIZE];
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        for b in noise.iter_mut() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *b = x as u8;
        }
        for (page, compressed) in [(&same, true), (&noise, false)] {
            let record = encode_record(5, 4, &[(9, page)]).unwrap();
            assert_eq!(record.len() as u64 % ALIGN, 0);
            let header = RecordHeader::decode(&record).unwrap();
            assert_eq!(
                (header.txid, header.durable_upto, header.compressed),
                (5, 4, compressed)
            );
            let stored = &record[RECORD_HEADER..RECORD_HEADER + header.stored_len as usize];
            let raw = decode_payload(&header, stored).unwrap();
            assert_eq!(raw[..8], 9u64.to_le_bytes());
            assert_eq!(&raw[8..], &page[..]);
            let mut damaged = stored.to_vec();
            damaged[3] ^= 1;
            assert!(decode_payload(&header, &damaged).is_none());
        }
    }

    #[test]
    fn every_byte_of_a_record_header_is_checked() {
        let record = encode_record(5, 4, &[(9, &[1u8; PAGE_SIZE])]).unwrap();
        for byte in 0..RECORD_HEADER {
            let mut damaged = record.to_vec();
            damaged[byte] ^= 0x10;
            assert!(RecordHeader::decode(&damaged).is_none(), "byte {byte}");
        }
    }
}
