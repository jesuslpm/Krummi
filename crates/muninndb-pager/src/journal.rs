//! The write-ahead journal (J2, J4, J10, J11, J12 and J16 of the Journal PRD).
//!
//! The journal is a sequence of files `muninn.journal.<19 digits>` in the store's folder, numbered one by one. Each
//! commit is one record, aligned to [`ALIGN`] bytes so that recovery can find the records after a damaged one:
//!
//! - a 64-byte header: magic, commit number (`txid`), `durable_upto` (the last commit that was durable when the
//!   record was written, J9), page count, flags, stored and raw payload lengths, payload checksum and header
//!   checksum;
//! - the payload: the commit's pages as (page number, sealed page) pairs, compressed with LZ4 when that makes it
//!   smaller (J16).
//!
//! Records are appended to the current file. When it reaches [`DEFAULT_FILE_LIMIT`] the journal flushes it and goes
//! on in a new file, whose name is made durable before any commit in it returns. A write that fails leaves the
//! write position where it was, so the next record overwrites what it left (J12).

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use xxhash_rust::xxh3::xxh3_64;

use crate::error::{Error, Result};
use crate::format::PAGE_SIZE;
use crate::pager::Pager;
use crate::sys;

/// Prefix of the journal files' names (J2).
pub const JOURNAL_PREFIX: &str = "muninn.journal.";
/// Records start at multiples of this many bytes of their file.
pub(crate) const ALIGN: u64 = 4096;
/// Size of a record header.
pub(crate) const RECORD_HEADER: usize = 64;
/// Size at which the journal goes on in a new file.
pub(crate) const DEFAULT_FILE_LIMIT: u64 = 16 << 20;
const MAGIC: u64 = u64::from_le_bytes(*b"MuninnJ1");
const FLAG_COMPRESSED: u32 = 1;
/// Bytes of one page in a raw payload: its number and the sealed page.
pub(crate) const PAGE_ENTRY: usize = 8 + PAGE_SIZE;

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

/// Encodes a whole record, padded to [`ALIGN`].
fn encode_record(
    txid: u64,
    durable_upto: u64,
    pages: &[(u64, &[u8; PAGE_SIZE])],
) -> Result<Vec<u8>> {
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
    let mut record = Vec::new();
    record
        .try_reserve_exact(len)
        .map_err(|_| out_of_memory(len))?;
    record.extend_from_slice(&header.encode());
    record.extend_from_slice(payload);
    record.resize(len, 0);
    Ok(record)
}

/// The journal of a store on disk.
pub(crate) struct Journal {
    folder: PathBuf,
    state: Mutex<State>,
    file_limit: AtomicU64,
    /// Called by a durable append between writing its record and flushing it (tests: AC-J13).
    sync_hook: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

struct State {
    /// The file records are appended to.
    current: Option<Current>,
    /// Number of the next file to create.
    next_number: u64,
    /// Last commit written to the journal.
    written_upto: u64,
    /// Last commit known to be durable (J4, J10).
    synced_upto: u64,
    /// Files that exist, oldest first, with the last commit each holds (the current one included).
    files: VecDeque<(u64, u64)>,
}

struct Current {
    number: u64,
    file: File,
    path: PathBuf,
    /// Where the next record goes.
    len: u64,
}

impl Journal {
    /// The journal of the store in `folder`, empty, after recovery: commits from `last_txid + 1` on, in files
    /// numbered from `next_number`.
    pub(crate) fn new(folder: &Path, last_txid: u64, next_number: u64) -> Journal {
        Journal {
            folder: folder.to_path_buf(),
            state: Mutex::new(State {
                current: None,
                next_number,
                written_upto: last_txid,
                synced_upto: last_txid,
                files: VecDeque::new(),
            }),
            file_limit: AtomicU64::new(DEFAULT_FILE_LIMIT),
            sync_hook: Mutex::new(None),
        }
    }

    /// Sets the function a durable append calls between writing and flushing its record, or none (tests).
    pub(crate) fn set_sync_hook(&self, hook: Option<Arc<dyn Fn() + Send + Sync>>) {
        *self
            .sync_hook
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = hook;
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sets the size at which the journal goes on in a new file (tests).
    pub(crate) fn set_file_limit(&self, bytes: u64) {
        self.file_limit.store(bytes, Ordering::Relaxed);
    }

    /// Last commit known to be durable.
    pub(crate) fn synced_upto(&self) -> u64 {
        self.lock().synced_upto
    }

    /// Appends the record of commit `txid` and, when `durable`, flushes it to the disk (J4); a deferred commit is
    /// only written (J10). On failure nothing of it counts: the next record goes where this one would have gone,
    /// and a record that was completely written is wiped when possible, so that it never appears after a reopen
    /// (J12) — except after a failed flush, which leaves it entire or absent.
    pub(crate) fn append(
        &self,
        pager: &Pager,
        txid: u64,
        pages: &[(u64, &[u8; PAGE_SIZE])],
        durable: bool,
    ) -> Result<()> {
        pager.check_writable()?;
        let mut state = self.lock();
        let record = encode_record(txid, state.synced_upto, pages)?;
        let rolls = state.current.as_ref().is_none_or(|c| {
            c.len > 0 && c.len + record.len() as u64 > self.file_limit.load(Ordering::Relaxed)
        });
        if rolls {
            self.roll(&mut state, pager)?;
        }
        let current = state
            .current
            .as_mut()
            .expect("the journal has a current file");
        let offset = current.len;
        let mut guard = WipeOnFailure {
            current,
            offset,
            armed: true,
        };
        sys::write_at(&guard.current.file, &guard.current.path, offset, &record)
            .map_err(|e| pager.fail_write(e, record.len() as u64))
            .inspect_err(|_| guard.armed = false)?;
        if durable {
            let hook = self
                .sync_hook
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            if let Some(hook) = hook {
                hook();
            }
            if let Err(e) = sys::sync(&guard.current.file, &guard.current.path) {
                // Entire or absent after a reopen (J12): the record stays as it is.
                guard.armed = false;
                guard.current.len = offset + record.len() as u64;
                return Err(pager.fail_write(e, 0));
            }
        }
        guard.armed = false;
        guard.current.len = offset + record.len() as u64;
        drop(guard);
        state.written_upto = txid;
        if durable {
            state.synced_upto = txid;
        }
        if let Some(last) = state.files.back_mut() {
            last.1 = txid;
        }
        Ok(())
    }

    /// Flushes the current file and goes on in a new one, whose name is made durable first.
    fn roll(&self, state: &mut State, pager: &Pager) -> Result<()> {
        if let Some(current) = &state.current {
            sys::sync(&current.file, &current.path).map_err(|e| pager.fail_write(e, 0))?;
            state.synced_upto = state.written_upto;
        }
        let number = state.next_number;
        let path = self.folder.join(file_name(number));
        let file = sys::create_new(&path).map_err(|e| journal_error(pager, e))?;
        if let Err(e) = sys::sync_dir(&self.folder) {
            drop(file);
            let _ = sys::remove(&path);
            return Err(pager.fail_write(e, 0));
        }
        state.next_number += 1;
        state.files.push_back((number, state.written_upto));
        state.current = Some(Current {
            number,
            file,
            path,
            len: 0,
        });
        Ok(())
    }

    /// Goes on in a new file when the current one holds records, so that the old ones can all be removed once
    /// both header copies record them as applied (tests: AC-J18).
    pub(crate) fn start_new_file(&self, pager: &Pager) -> Result<()> {
        pager.check_writable()?;
        let mut state = self.lock();
        if state.current.as_ref().is_some_and(|c| c.len > 0) {
            self.roll(&mut state, pager)?;
        }
        Ok(())
    }

    /// Makes every commit written so far durable (J11, J5).
    pub(crate) fn make_durable(&self, pager: &Pager) -> Result<()> {
        pager.check_writable()?;
        let mut state = self.lock();
        if state.synced_upto == state.written_upto {
            return Ok(());
        }
        let current = state
            .current
            .as_ref()
            .expect("unsynced commits are in the current file");
        sys::sync(&current.file, &current.path).map_err(|e| pager.fail_write(e, 0))?;
        state.synced_upto = state.written_upto;
        Ok(())
    }

    /// Number of the first file that may hold commits after `applied`: what a header with `applied` records.
    pub(crate) fn first_needed(&self, applied: u64) -> u64 {
        let state = self.lock();
        let current = state.current.as_ref().map(|c| c.number);
        state
            .files
            .iter()
            .find(|(number, last)| *last > applied || Some(*number) == current)
            .map_or(state.next_number, |(number, _)| *number)
    }

    /// Whether the journal has gone on in more than one file since its files were last removed.
    pub(crate) fn has_old_files(&self) -> bool {
        self.lock().files.len() > 1
    }

    /// Removes the files whose commits are all at most `applied` (the smaller of what the two header copies
    /// record), except the current one; or every file, the current one included, when `all`.
    pub(crate) fn remove_files(&self, pager: &Pager, applied: u64, all: bool) -> Result<()> {
        let mut state = self.lock();
        while let Some(&(number, last)) = state.files.front() {
            let is_current = state.current.as_ref().is_some_and(|c| c.number == number);
            if last > applied || (is_current && !all) {
                break;
            }
            if is_current {
                state.current = None;
            }
            match sys::remove(&self.folder.join(file_name(number))) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(journal_error(pager, e)),
            }
            state.files.pop_front();
        }
        Ok(())
    }
}

/// Wipes the header of a record whose append did not complete, so it is never read as a commit (J12).
struct WipeOnFailure<'a> {
    current: &'a mut Current,
    offset: u64,
    armed: bool,
}

impl Drop for WipeOnFailure<'_> {
    fn drop(&mut self) {
        if self.armed {
            let zeros = [0u8; RECORD_HEADER];
            let _ = sys::write_at(&self.current.file, &self.current.path, self.offset, &zeros);
        }
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
        assert_eq!(file_name(1), "muninn.journal.0000000000000000001");
        assert_eq!(parse_file_name(OsStr::new(&file_name(42))), Some(42));
        for other in [
            "muninn.journal.1",
            "muninn.journal.00000000000000000001",
            "muninn.journal.000000000000000000a",
            "muninn.journal.0000000000000000001.bak",
            "muninn.data",
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
            let mut damaged = record.clone();
            damaged[byte] ^= 0x10;
            assert!(RecordHeader::decode(&damaged).is_none(), "byte {byte}");
        }
    }
}
