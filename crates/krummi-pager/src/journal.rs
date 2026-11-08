//! The write-ahead journal (J2, J4, J10, J11, J12 and J16 of the Journal PRD).
//!
//! The journal is a sequence of files `krummi.journal.<19 digits>` in the store's folder, numbered one by one. Each
//! commit is one record, aligned to [`ALIGN`] bytes so that recovery can find the records after a damaged one:
//!
//! - a 64-byte header: magic, commit number (`txid`), `durable_upto` (the last commit that was durable when the
//!   record was written, J9), page count, flags, stored and raw payload lengths, payload checksum and header
//!   checksum;
//! - the payload: one entry per page of the commit, compressed with LZ4 when that makes it smaller (J16). An entry
//!   is the page's number with its flags, then the runs of 8-byte words where the sealed page differs from the
//!   version the commit copied it from, each as its offset, its length and its new bytes; a page the commit created
//!   ([`CREATED_ENTRY`]) is compared with a page of zeros instead, and recovery writes it over zeros (Y1 and Y2 of the
//!   journal page diffs PRD). The pages of a value run are not sealed one by one: their number carries
//!   [`RAW_ENTRY`] (D5 of the large values execution), and they are always created.
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

use crate::error::{Error, Result};
use crate::format::{PAGE_SIZE, xxh3_64};
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
/// The journal of format 10 (Y5 of the journal page diffs PRD), whose entries hold the bytes that changed; format 9's
/// records, with whole pages, had `KrummiJ1`.
const MAGIC: u64 = u64::from_le_bytes(*b"KrummiJ2");
const FLAG_COMPRESSED: u32 = 1;
/// The fewest bytes of a page entry in a raw payload: its number and a count of no runs.
pub(crate) const MIN_ENTRY: usize = 8 + 2;
/// The most bytes of a page entry: its number, its count of runs and one run that covers the whole page, which takes
/// more than any set of shorter runs (each run of `k` words takes `4 + 8 k` bytes, and runs are apart).
pub(crate) const MAX_ENTRY: usize = MIN_ENTRY + 4 + PAGE_SIZE;
/// Words of 8 bytes in a page, the unit an entry compares.
const WORDS: usize = PAGE_SIZE / 8;
/// Flag of the page number of an entry whose page is part of a value run (D5 of the large values execution): it is
/// not sealed on its own, so recovery writes it without checking it, and the record's checksum covers it. Its run
/// is checked as a whole when it is read (D1).
pub(crate) const RAW_ENTRY: u64 = 1 << 63;
/// Flag of the page number of an entry whose page the commit created: recovery writes its runs over a page of zeros,
/// not over what `krummi.data` holds (Y1, Y2 of the journal page diffs PRD).
pub(crate) const CREATED_ENTRY: u64 = 1 << 62;

/// A page of a commit for the journal: its number, with [`RAW_ENTRY`] for a page of a value run, the sealed page,
/// and the version the commit copied it from, `None` for a page the commit created (Y1).
pub(crate) type JournalPage<'a> = (u64, &'a [u8; PAGE_SIZE], Option<&'a [u8; PAGE_SIZE]>);

/// Appends the entry of `page`: its number with its flags, then each run of words that differs from `base`, or from
/// zeros when there is none, as offset, length and the new bytes (Y1). The entry holds the new value of every byte
/// it names, never a change of it, so applying it over any mix of the versions it follows gives `page` (Y2).
pub(crate) fn encode_page(out: &mut Vec<u8>, (entry, page, base): JournalPage<'_>) {
    let flags = if base.is_none() { CREATED_ENTRY } else { 0 };
    out.extend_from_slice(&(entry | flags).to_le_bytes());
    let count_at = out.len();
    out.extend_from_slice(&[0, 0]);
    let words = page.as_chunks::<8>().0;
    let differs = |word: usize| match base {
        Some(base) => words[word] != base.as_chunks::<8>().0[word],
        None => words[word] != [0; 8],
    };
    let (mut runs, mut word) = (0u16, 0);
    while word < WORDS {
        if !differs(word) {
            word += 1;
            continue;
        }
        let start = word;
        while word < WORDS && differs(word) {
            word += 1;
        }
        let (at, len) = (start * 8, (word - start) * 8);
        // A page has 8,192 bytes: an offset and a length fit in 16 bits.
        out.extend_from_slice(&(at as u16).to_le_bytes());
        out.extend_from_slice(&(len as u16).to_le_bytes());
        out.extend_from_slice(&page[at..at + len]);
        runs += 1;
    }
    out[count_at..count_at + 2].copy_from_slice(&runs.to_le_bytes());
}

/// A page entry of a decoded payload.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PageEntry<'a> {
    /// The page number, without flags.
    pub number: u64,
    /// The page is part of a value run (D5 of the large values execution).
    pub raw: bool,
    /// The commit created the page: its runs go over zeros (Y2).
    pub created: bool,
    /// Offset and new bytes of each run.
    pub runs: Vec<(usize, &'a [u8])>,
}

/// The `count` entries of a raw payload, or `None` when it is not exactly `count` entries whose runs lie within a
/// page.
pub(crate) fn decode_entries(raw: &[u8], count: u32) -> Option<Vec<PageEntry<'_>>> {
    let mut at: usize = 0;
    let mut take = |len: usize| {
        let bytes = raw.get(at..at.checked_add(len)?)?;
        at += len;
        Some(bytes)
    };
    let u16_of = |bytes: &[u8]| usize::from(u16::from_le_bytes([bytes[0], bytes[1]]));
    let mut entries = Vec::new();
    entries.try_reserve_exact(count as usize).ok()?;
    for _ in 0..count {
        let word = u64::from_le_bytes(take(8)?.try_into().expect("8 bytes"));
        let runs_count = u16_of(take(2)?);
        let mut runs = Vec::new();
        runs.try_reserve_exact(runs_count).ok()?;
        for _ in 0..runs_count {
            let (offset, len) = (u16_of(take(2)?), u16_of(take(2)?));
            if len == 0 || offset + len > PAGE_SIZE {
                return None;
            }
            runs.push((offset, take(len)?));
        }
        entries.push(PageEntry {
            number: word & !(RAW_ENTRY | CREATED_ENTRY),
            raw: word & RAW_ENTRY != 0,
            created: word & CREATED_ENTRY != 0,
            runs,
        });
    }
    (at == raw.len()).then_some(entries)
}

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
        let pages = u64::from(header.page_count);
        let consistent = (pages * MIN_ENTRY as u64..=pages * MAX_ENTRY as u64)
            .contains(&header.raw_len)
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

/// The memory a record's payload is built in, kept from one commit to the next so that a commit does not allocate
/// it again (O4 of the performance plan).
#[derive(Default)]
struct EncodeBuffers {
    /// The payload before compression.
    raw: Vec<u8>,
}

/// The most the commits keep for building records: the payload buffer up to this size, and record buffers the
/// writer gave back up to this many bytes in all; a larger commit gives its memory back. It covers commits of
/// 16 MiB of values, which allocated and freed their memory every time with the 8 MiB that O4 kept (candidate 4 of
/// the writes diagnosis, performance plan).
const ENCODE_RETAIN: usize = 32 << 20;

/// Record buffers the writer gave back once their group was durable, for the next records (candidate 4 of the
/// writes diagnosis).
type SpareRecords = Mutex<Vec<AlignedBuf>>;

/// Encodes a whole record, padded to [`ALIGN`], in memory aligned for direct writes: the smallest buffer of `spare`
/// that is large enough, or new memory. It builds the payload in `buffers` and compresses it straight into the
/// record.
fn encode_record(
    buffers: &mut EncodeBuffers,
    spare: &SpareRecords,
    txid: u64,
    durable_upto: u64,
    pages: &[JournalPage<'_>],
) -> Result<AlignedBuf> {
    let record = encode_record_in(buffers, spare, txid, durable_upto, pages);
    if buffers.raw.capacity() > ENCODE_RETAIN {
        buffers.raw = Vec::new();
    }
    record
}

fn encode_record_in(
    buffers: &mut EncodeBuffers,
    spare: &SpareRecords,
    txid: u64,
    durable_upto: u64,
    pages: &[JournalPage<'_>],
) -> Result<AlignedBuf> {
    let most_raw = pages.len() * MAX_ENTRY;
    let out_of_memory = |requested: usize| Error::OutOfMemory {
        requested: requested as u64,
    };
    let raw = &mut buffers.raw;
    raw.clear();
    raw.try_reserve_exact(most_raw)
        .map_err(|_| out_of_memory(most_raw))?;
    for &page in pages {
        encode_page(raw, page);
    }
    let raw_len = raw.len();
    let bound = lz4_flex::block::get_maximum_output_size(raw_len);
    // Room for the header and the payload, compressed (at most `bound` bytes) or not.
    let most = RECORD_HEADER + bound.max(raw_len);
    let mut record = match take_spare(spare, most) {
        Some(record) => record,
        None => AlignedBuf::zeroed(most).map_err(|_| out_of_memory(most))?,
    };
    record.set_len(most);
    let payload = &mut record[RECORD_HEADER..];
    let compressed = match lz4_flex::block::compress_into(raw, &mut payload[..bound]) {
        Ok(n) if n < raw_len => Some(n),
        _ => None,
    };
    let stored_len = compressed.unwrap_or_else(|| {
        payload[..raw_len].copy_from_slice(raw);
        raw_len
    });
    let header = RecordHeader {
        txid,
        durable_upto,
        page_count: pages.len() as u32,
        compressed: compressed.is_some(),
        stored_len: stored_len as u64,
        raw_len: raw_len as u64,
        payload_checksum: xxh3_64(&payload[..stored_len]),
    };
    let len = header.record_len() as usize;
    record[..RECORD_HEADER].copy_from_slice(&header.encode());
    // The padding is zeros, whatever an earlier record left there.
    record[RECORD_HEADER + stored_len..len].fill(0);
    record.set_len(len);
    Ok(record)
}

/// The smallest buffer of `spare` that holds `len` bytes, taken out of it.
fn take_spare(spare: &SpareRecords, len: usize) -> Option<AlignedBuf> {
    let len = len.max(1).next_multiple_of(sys::DIRECT_ALIGN);
    let mut spare = spare.lock().unwrap_or_else(PoisonError::into_inner);
    let (at, _) = spare
        .iter()
        .enumerate()
        .filter(|(_, buf)| buf.capacity() >= len)
        .min_by_key(|(_, buf)| buf.capacity())?;
    Some(spare.swap_remove(at))
}

/// Keeps `record`'s buffer in `spare` for the next records, dropping the smallest ones while they hold more than
/// [`ENCODE_RETAIN`] bytes in all.
fn give_back(spare: &SpareRecords, record: AlignedBuf) {
    if record.capacity() > ENCODE_RETAIN {
        return;
    }
    let mut spare = spare.lock().unwrap_or_else(PoisonError::into_inner);
    spare.push(record);
    while spare.iter().map(AlignedBuf::capacity).sum::<usize>() > ENCODE_RETAIN {
        let (smallest, _) = spare
            .iter()
            .enumerate()
            .min_by_key(|(_, buf)| buf.capacity())
            .expect("not empty while over the limit");
        drop(spare.swap_remove(smallest));
    }
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
    /// The memory records are built in; only the commit that enqueues uses it.
    encode: Mutex<EncodeBuffers>,
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
    /// Record buffers of durable groups, for the next commits.
    spare: SpareRecords,
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
    /// Files a commit went on from, for the writer to close: closing a journal file can take long, on Windows
    /// above all, so the commit leaves it to the writer (candidate 3 of the writes diagnosis, performance plan).
    retired: Vec<Arc<OpenFile>>,
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

// Journal files closed on each thread (tests: candidate 3 of the writes diagnosis).
#[cfg(test)]
thread_local! {
    static CLOSED_HERE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Journal files closed on the calling thread so far (tests).
#[cfg(test)]
pub(crate) fn files_closed_on_this_thread() -> usize {
    CLOSED_HERE.with(std::cell::Cell::get)
}

#[cfg(test)]
impl Drop for OpenFile {
    fn drop(&mut self) {
        CLOSED_HERE.with(|closed| closed.set(closed.get() + 1));
    }
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
                retired: Vec::new(),
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
            spare: Mutex::new(Vec::new()),
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
            encode: Mutex::new(EncodeBuffers::default()),
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
        pages: &[JournalPage<'_>],
    ) -> Result<()> {
        pager.check_writable()?;
        let record = {
            let mut buffers = self.encode.lock().unwrap_or_else(PoisonError::into_inner);
            encode_record(&mut buffers, &self.shared.spare, txid, 0, pages)?
        };
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
        let old = state
            .current
            .replace(Arc::new(OpenFile { file, path, direct }));
        if let Some(old) = old {
            state.retired.push(old);
            shared.work.notify_all();
        }
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
        // A writer that stopped may have left files to close.
        let retired = std::mem::take(&mut state.retired);
        let group: Vec<Queued> = state.queue.drain(..).collect();
        if group.is_empty() {
            drop(state);
            drop(retired);
            return Ok(());
        }
        let durable_upto = state.synced_upto;
        drop(state);
        drop(retired);
        let mut group = group;
        let written = shared.write_group(&mut group, durable_upto);
        shared.finish_group(group, written.map_err(|e| (e.kind(), e.to_string())));
        match &shared.lock().failed {
            Some(failure) => Err(pager.fail_flush(io::Error::new(failure.0, failure.1.clone()))),
            None => Ok(()),
        }
    }

    /// Number of the next file to create and number of files left for the writer to close (tests).
    #[cfg(test)]
    pub(crate) fn rolls_and_retired(&self) -> (u64, usize) {
        let state = self.shared.lock();
        (state.next_number, state.retired.len())
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

    /// The writer thread: closes the files the commits went on from, and takes everything in the queue, writes it as
    /// one group and makes it durable, until the close. A panic counts as a failed group (E13).
    fn run(&self) {
        loop {
            let (group, durable_upto, retired) = {
                let mut state = self.lock();
                let can_write = loop {
                    let can_write =
                        !state.queue.is_empty() && !state.paused && state.failed.is_none();
                    if can_write || !state.retired.is_empty() {
                        break can_write;
                    }
                    if state.stop {
                        return;
                    }
                    state = self
                        .work
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                };
                let group: Vec<Queued> = if can_write {
                    state.queue.drain(..).collect()
                } else {
                    Vec::new()
                };
                (group, state.synced_upto, std::mem::take(&mut state.retired))
            };
            // Outside the lock, so that neither the commits nor the writer wait for it.
            drop(retired);
            if group.is_empty() {
                continue;
            }
            let mut group = group;
            let written = panic::catch_unwind(AssertUnwindSafe(|| {
                self.write_group(&mut group, durable_upto)
            }));
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
    /// file go in one write, and then each file is made durable. A record alone in its file's write goes from its
    /// own buffer, without a copy (O5 of the performance plan); its header is rewritten in place, as a retry does
    /// again.
    fn write_group(&self, group: &mut [Queued], durable_upto: u64) -> io::Result<()> {
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
            let records = &mut group[start..end];
            let grouped = if records.len() > 1 {
                let total: usize = records.iter().map(|q| q.record.len()).sum();
                // The list of parts is allocated fallibly too, so that no memory for it also takes the fallback.
                let mut parts: Vec<&[u8]> = Vec::new();
                parts.try_reserve_exact(records.len()).ok().and_then(|()| {
                    parts.extend(records.iter().map(|q| &q.record[..]));
                    AlignedBuf::from_parts(&parts, total).ok()
                })
            } else {
                None
            };
            match grouped {
                Some(mut buf) => {
                    let mut at = 0;
                    for queued in records.iter() {
                        set_durable_upto(&mut buf[at..at + RECORD_HEADER], durable_upto);
                        at += queued.record.len();
                    }
                    let first = &records[0];
                    sys::write_at(&first.file.file, &first.file.path, first.offset, &buf[..at])?;
                }
                // A record alone, or no memory for one buffer: one write per record from its own buffer, without a
                // copy, still one flush.
                None => {
                    for queued in records.iter_mut() {
                        set_durable_upto(&mut queued.record[..RECORD_HEADER], durable_upto);
                        let file = &queued.file;
                        sys::write_at(&file.file, &file.path, queued.offset, &queued.record)?;
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
                for queued in group {
                    give_back(&self.spare, queued.record);
                }
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
            let record = encode(&mut EncodeBuffers::default(), 5, 4, &[(9, page, None)]);
            assert_eq!(record.len() as u64 % ALIGN, 0);
            let header = RecordHeader::decode(&record).unwrap();
            assert_eq!(
                (header.txid, header.durable_upto, header.compressed),
                (5, 4, compressed)
            );
            let stored = &record[RECORD_HEADER..RECORD_HEADER + header.stored_len as usize];
            let raw = decode_payload(&header, stored).unwrap();
            let entries = decode_entries(&raw, 1).unwrap();
            assert_eq!(
                (entries[0].number, entries[0].raw, entries[0].created),
                (9, false, true)
            );
            let mut rebuilt = [0u8; PAGE_SIZE];
            for &(at, bytes) in &entries[0].runs {
                rebuilt[at..at + bytes.len()].copy_from_slice(bytes);
            }
            assert_eq!(&rebuilt[..], &page[..]);
            let mut damaged = stored.to_vec();
            damaged[3] ^= 1;
            assert!(decode_payload(&header, &damaged).is_none());
        }
    }

    /// Pages for the record tests: compressible, half compressible and incompressible, the same on every target.
    fn sample_pages() -> Vec<[u8; PAGE_SIZE]> {
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        let mut noise = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        };
        let mut pages = vec![[7u8; PAGE_SIZE], [0u8; PAGE_SIZE], [0u8; PAGE_SIZE]];
        for (i, b) in pages[1].iter_mut().enumerate() {
            *b = if i % 2 == 0 { noise() } else { 0 };
        }
        for b in pages[2].iter_mut() {
            *b = noise();
        }
        pages
    }

    /// The bytes of encoded records stay those of the stored format of version 10 (O4 of the performance plan reworked
    /// how they are built; Y1 of the journal page diffs PRD changed what an entry holds): xxh3 of whole records,
    /// padding included. Pages created, compared with zeros, and pages with the version they were copied from.
    #[test]
    fn records_keep_the_bytes_of_the_stored_format() {
        let pages = sample_pages();
        let mut changed = pages[1];
        changed[100..108].copy_from_slice(b"changed!");
        changed[4000] ^= 0xFF;
        // Each page of a record by its sample, with the sample it was copied from or none if created.
        type Record<'a> = &'a [(usize, Option<usize>)];
        let cases: [(Record<'_>, u64); 7] = [
            (&[(0, None)], 0x8e72_efa3_a63d_8d50),
            (&[(1, None)], 0x681c_1020_4896_9288),
            // Incompressible: stored raw.
            (&[(2, None)], 0x1279_2d88_f798_6960),
            (&[(0, None), (1, None), (2, None)], 0x3695_e9fa_5d6b_42ef),
            (
                &[(2, None), (2, None), (0, None), (1, None)],
                0x2c56_ce06_eca4_aba5,
            ),
            // Copied pages: the runs that differ from their version, and nothing for an unchanged one.
            (&[(3, Some(1)), (2, Some(2))], 0x7a14_51b3_2830_e05d),
            (
                &[(3, Some(1)), (0, Some(2)), (1, None)],
                0x2476_9c15_723a_2c4c,
            ),
        ];
        let pages = [pages[0], pages[1], pages[2], changed];
        // One set of buffers for every case, and each record's buffer given back as the writer does, as the commits
        // of a store share theirs: what one record leaves in them never reaches the next.
        let mut buffers = EncodeBuffers::default();
        let spare = SpareRecords::default();
        let mut got = Vec::new();
        for (i, (which, expected)) in cases.into_iter().enumerate() {
            let entries: Vec<JournalPage<'_>> = which
                .iter()
                .enumerate()
                .map(|(n, &(p, base))| (100 + n as u64 * 3, &pages[p], base.map(|b| &pages[b])))
                .collect();
            let record = encode_record(&mut buffers, &spare, 40 + i as u64, 7, &entries).unwrap();
            got.push((xxh3_64(&record), expected));
            give_back(&spare, record);
        }
        for (i, (got, expected)) in got.into_iter().enumerate() {
            assert_eq!(got, expected, "case {i}");
        }
    }

    /// A record built with no spare record buffers.
    fn encode(
        buffers: &mut EncodeBuffers,
        txid: u64,
        durable_upto: u64,
        pages: &[JournalPage<'_>],
    ) -> AlignedBuf {
        encode_record(buffers, &SpareRecords::default(), txid, durable_upto, pages).unwrap()
    }

    /// Buffers kept from earlier, larger records build the same bytes as new ones: a small record after large ones,
    /// compressed or raw, in the record buffer the writer gave back, and a commit larger than what the buffers keep,
    /// after which they give their memory back.
    #[test]
    fn reused_buffers_build_the_same_records_as_new_ones() {
        let pages = sample_pages();
        let entries = |count: usize| -> Vec<JournalPage<'_>> {
            (0..count)
                .map(|n| (10 + n as u64, &pages[n % 3], None))
                .collect()
        };
        let mut buffers = EncodeBuffers::default();
        let spare = SpareRecords::default();
        for entries in [
            vec![
                (9, &pages[2], None),
                (12, &pages[1], None),
                (15, &pages[0], None),
            ],
            vec![(9, &pages[0], None)],
            vec![(9, &pages[2], None)],
            // More than the 8 MiB that O4 kept, less than what is kept now.
            entries(DEFAULT_QUEUE_LIMIT as usize / PAGE_SIZE + 2),
            vec![(9, &pages[2], None)],
            entries(ENCODE_RETAIN / PAGE_SIZE + 2),
            vec![(9, &pages[1], None)],
        ] {
            let reused = encode_record(&mut buffers, &spare, 3, 2, &entries).unwrap();
            let fresh = encode(&mut EncodeBuffers::default(), 3, 2, &entries);
            assert!(reused[..] == fresh[..], "{} pages", entries.len());
            give_back(&spare, reused);
            let kept: usize = spare.lock().unwrap().iter().map(AlignedBuf::capacity).sum();
            assert!(buffers.raw.capacity() <= ENCODE_RETAIN && kept <= ENCODE_RETAIN);
        }
    }

    /// Candidate 4 of the writes diagnosis: a commit builds its record in the buffer the writer gave back, and keeps
    /// a payload buffer larger than the 8 MiB that O4 kept.
    #[test]
    fn a_record_is_built_in_the_buffer_the_writer_gave_back() {
        let pages = sample_pages();
        let mut buffers = EncodeBuffers::default();
        let spare = SpareRecords::default();
        let first = encode_record(&mut buffers, &spare, 3, 2, &[(9, &pages[2], None)]).unwrap();
        let at = first.as_ptr();
        give_back(&spare, first);
        let second = encode_record(&mut buffers, &spare, 4, 2, &[(9, &pages[0], None)]).unwrap();
        assert_eq!(second.as_ptr(), at);
        assert!(spare.lock().unwrap().is_empty());
        let large: Vec<JournalPage<'_>> = (0..DEFAULT_QUEUE_LIMIT as usize / PAGE_SIZE + 2)
            .map(|n| (n as u64, &pages[n % 3], None))
            .collect();
        encode_record(&mut buffers, &spare, 5, 2, &large).unwrap();
        assert!(buffers.raw.capacity() > DEFAULT_QUEUE_LIMIT as usize);
    }

    #[test]
    fn every_byte_of_a_record_header_is_checked() {
        let record = encode(
            &mut EncodeBuffers::default(),
            5,
            4,
            &[(9, &[1u8; PAGE_SIZE], None)],
        );
        for byte in 0..RECORD_HEADER {
            let mut damaged = record.to_vec();
            damaged[byte] ^= 0x10;
            assert!(RecordHeader::decode(&damaged).is_none(), "byte {byte}");
        }
    }
}
