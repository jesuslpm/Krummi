//! Recovery when a store opens (J8 and J9 of the Journal PRD, step 8 of the parent's order of checks).
//!
//! Pass 1 reads every journal file the header needs, one record at a time, and finds the intact records, also
//! after a damaged one: they start at multiples of [`ALIGN`]. Recovery never holds more than one record in memory. It decides, before anything changes, which commits apply:
//!
//! - commits up to the one the header records as applied are already in `krummi.data` and are skipped;
//! - the next ones apply in order while they follow one another;
//! - when the next expected commit is missing (incomplete, damaged, or in a missing file), it is a commit that did
//!   not finish if no later intact record was written while it was already durable, and it is discarded with all
//!   later ones; otherwise the journal is damaged and the opening fails without touching the store.
//!
//! Pass 2 writes the pages of the commits that apply, flushes `krummi.data`, records the last one in both header
//! copies and removes the journal and the temporary area. Each sealed page is checked before anything is written;
//! the pages of a value run travel as raw entries, which the record's checksum covers. An interruption at any point
//! leaves what the next opening needs to do the same again.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::format::{HEADER_PAGES, Header, PAGE_SIZE, verify_page};
use crate::journal::{
    ALIGN, PageEntry, RECORD_HEADER, RecordHeader, decode_entries, decode_payload, parse_file_name,
};
use crate::pager::Pager;
use crate::sys;
use crate::temp::TEMP_DIR;

/// What recovery leaves for the journal: the last commit and the number of the next journal file.
pub(crate) struct Recovered {
    pub last_txid: u64,
    pub next_number: u64,
}

/// An intact record found in pass 1: its file and offset, and its header.
struct Found {
    file: usize,
    offset: u64,
    header: RecordHeader,
}

/// Recovers the store `pager` opened from `folder` (J8).
pub(crate) fn recover(pager: &mut Pager, folder: &Path) -> Result<Recovered> {
    let header = pager.header();
    let (journal_files, temp_present) = list(folder)?;
    let needed: Vec<&(u64, PathBuf)> = journal_files
        .iter()
        .filter(|(number, _)| *number >= header.journal_first)
        .collect();
    let next_number = journal_files.last().map_or(header.journal_first, |(n, _)| {
        (n + 1).max(header.journal_first)
    });

    // Pass 1: read and decide; nothing changes yet.
    let mut files = Vec::with_capacity(needed.len());
    for (_, path) in &needed {
        files.push(JournalFile::open(path, folder)?);
    }
    let mut found = Vec::new();
    for (index, file) in files.iter().enumerate() {
        scan(file, index, folder, &mut found)?;
    }
    let apply = decide(&found, header.applied_txid, folder)?;

    // Pass 2: apply, check, record, clean up.
    let mut last_txid = header.applied_txid;
    let mut applied = BTreeMap::new();
    for record in &apply {
        let raw = files[record.file]
            .payload(record.offset, &record.header, folder)?
            .ok_or_else(|| damaged(folder))?;
        apply_pages(pager, &raw, record.header.page_count, folder, &mut applied)?;
        last_txid = record.header.txid;
    }
    drop(files);
    // Every applied page is now its last committed version, whatever mix of versions a cut transfer left in it (Y2 of
    // the journal page diffs PRD). One that does not verify was damaged in `krummi.data` by something else: the
    // opening fails with the lowest such page, before the header records anything, so the next one repeats (Y3).
    check_applied(pager, &applied)?;
    if !journal_files.is_empty() || !apply.is_empty() {
        pager.flush_data()?;
        for _ in 0..2 {
            pager.write_header(Header {
                applied_txid: last_txid,
                journal_first: next_number,
                ..pager.header()
            })?;
        }
        for (_, path) in &journal_files {
            match sys::remove(path) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(access(folder, e)),
            }
        }
    }
    if temp_present {
        sys::remove_temp_dir(&folder.join(TEMP_DIR)).map_err(|e| access(folder, e))?;
    }
    Ok(Recovered {
        last_txid,
        next_number,
    })
}

/// The journal files in `folder`, sorted by number, and whether `krummi.temp` is there. An entry with a reserved
/// name of the wrong kind is damage (J9).
fn list(folder: &Path) -> Result<(Vec<(u64, PathBuf)>, bool)> {
    let mut files = Vec::new();
    let mut temp = false;
    for entry in fs::read_dir(folder).map_err(|e| access(folder, e))? {
        let entry = entry.map_err(|e| access(folder, e))?;
        let name = entry.file_name();
        let kind = entry.file_type().map_err(|e| access(folder, e))?;
        if let Some(number) = parse_file_name(&name) {
            if !kind.is_file() {
                return Err(damaged(folder));
            }
            files.push((number, entry.path()));
        } else if is_temp_dir(&name) {
            if !kind.is_dir() {
                return Err(damaged(folder));
            }
            temp = true;
        }
    }
    files.sort_unstable_by_key(|(number, _)| *number);
    Ok((files, temp))
}

fn is_temp_dir(name: &std::ffi::OsStr) -> bool {
    match name.to_str() {
        Some(name) if cfg!(windows) => name.eq_ignore_ascii_case(TEMP_DIR),
        Some(name) => name == TEMP_DIR,
        None => false,
    }
}

/// A journal file opened for reading.
struct JournalFile {
    path: PathBuf,
    file: File,
    len: u64,
}

impl JournalFile {
    /// Opens a journal file. A missing permission is the access class; another failure, the I/O class.
    fn open(path: &Path, folder: &Path) -> Result<JournalFile> {
        let file = sys::open_read(path).map_err(|e| read_error(folder, e))?;
        let len = file.metadata().map_err(|e| read_error(folder, e))?.len();
        Ok(JournalFile {
            path: path.to_path_buf(),
            file,
            len,
        })
    }

    /// The `len` bytes at `offset`, fewer at the end of the file.
    fn read(&self, offset: u64, len: usize, folder: &Path) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(len)
            .map_err(|_| Error::OutOfMemory {
                requested: len as u64,
            })?;
        bytes.resize(len, 0);
        let read = sys::read_at(&self.file, &self.path, offset, &mut bytes)
            .map_err(|e| read_error(folder, e))?;
        bytes.truncate(read);
        Ok(bytes)
    }

    /// The header of the record at `offset`, if one is intact there.
    fn header(&self, offset: u64, folder: &Path) -> Result<Option<RecordHeader>> {
        Ok(RecordHeader::decode(&self.read(
            offset,
            RECORD_HEADER,
            folder,
        )?))
    }

    /// The raw payload of the record with `header` at `offset`, if it is intact and whole.
    fn payload(
        &self,
        offset: u64,
        header: &RecordHeader,
        folder: &Path,
    ) -> Result<Option<Vec<u8>>> {
        let start = offset + RECORD_HEADER as u64;
        if start + header.stored_len > self.len {
            return Ok(None);
        }
        let stored = self.read(start, header.stored_len as usize, folder)?;
        if stored.len() as u64 != header.stored_len {
            return Ok(None);
        }
        Ok(decode_payload(header, &stored))
    }
}

/// Adds to `found` every intact record of a file, in order: after a damaged region the next intact record is
/// searched at every multiple of [`ALIGN`].
fn scan(file: &JournalFile, index: usize, folder: &Path, found: &mut Vec<Found>) -> Result<()> {
    let mut offset = 0;
    while offset + RECORD_HEADER as u64 <= file.len {
        let intact = match file.header(offset, folder)? {
            Some(header) => file.payload(offset, &header, folder)?.map(|_| header),
            None => None,
        };
        match intact {
            Some(header) => {
                found.push(Found {
                    file: index,
                    offset,
                    header,
                });
                offset += header.record_len();
            }
            None => offset += ALIGN,
        }
    }
    Ok(())
}

/// The records to apply, in order, or the damage class (J9).
fn decide<'a>(found: &'a [Found], applied: u64, folder: &Path) -> Result<Vec<&'a Found>> {
    let mut expected = applied + 1;
    let mut apply = Vec::new();
    for (i, record) in found.iter().enumerate() {
        let txid = record.header.txid;
        if txid < expected {
            continue;
        }
        if txid == expected {
            apply.push(record);
            expected += 1;
            continue;
        }
        // Commit `expected` is missing: damage only if a later record says it was durable.
        if found[i..]
            .iter()
            .any(|later| later.header.durable_upto >= expected)
        {
            return Err(damaged(folder));
        }
        break;
    }
    Ok(apply)
}

/// Checks every page in `applied` whose last entry was not a page of a value run: each must be its last committed
/// version now, sealed for its number, or the opening fails with the damage class and the lowest page that is not
/// (Y2, Y3 of the journal page diffs PRD).
fn check_applied(pager: &Pager, applied: &BTreeMap<u64, bool>) -> Result<()> {
    for (&number, &raw) in applied {
        if !raw
            && !pager
                .raw_pages(number, 1)
                .is_some_and(|page| verify_page(number, page))
        {
            return Err(pager.damaged(number));
        }
    }
    Ok(())
}

/// Applies the `count` page entries of a commit's raw payload to `krummi.data` and records in `applied`, for each page,
/// whether its last entry was a page of a value run (Y2 of the journal page diffs PRD). Every entry is decoded, and
/// each page the commit created is rebuilt and checked, before any write: a record that fails is damage of the journal
/// (J9). A created page is written as its runs over zeros; any other page, as its runs over what `krummi.data` holds,
/// zeros past its end.
fn apply_pages(
    pager: &mut Pager,
    raw: &[u8],
    count: u32,
    folder: &Path,
    applied: &mut BTreeMap<u64, bool>,
) -> Result<()> {
    let entries = decode_entries(raw, count).ok_or_else(|| damaged(folder))?;
    let rebuilt = |entry: &PageEntry<'_>, base: Option<&[u8]>| {
        let mut page = [0u8; PAGE_SIZE];
        if let Some(base) = base {
            page.copy_from_slice(base);
        }
        for &(at, bytes) in &entry.runs {
            page[at..at + bytes.len()].copy_from_slice(bytes);
        }
        page
    };
    let mut highest = 0;
    for entry in &entries {
        let unchecked =
            entry.raw || !entry.created || verify_page(entry.number, &rebuilt(entry, None));
        if entry.number < HEADER_PAGES || !unchecked {
            return Err(damaged(folder));
        }
        highest = highest.max(entry.number);
    }
    pager.ensure_pages_unbounded(highest + 1)?;
    for entry in &entries {
        let base = if entry.created {
            None
        } else {
            pager.raw_pages(entry.number, 1)
        };
        let page = rebuilt(entry, base);
        pager.write_sealed(entry.number, &page)?;
        applied.insert(entry.number, entry.raw);
    }
    Ok(())
}

fn damaged(folder: &Path) -> Error {
    Error::Damaged {
        folder: Some(folder.to_path_buf()),
        page: None,
    }
}

fn access(folder: &Path, source: io::Error) -> Error {
    Error::Access {
        folder: folder.to_path_buf(),
        source,
    }
}

fn read_error(folder: &Path, source: io::Error) -> Error {
    match source.kind() {
        io::ErrorKind::PermissionDenied => access(folder, source),
        io::ErrorKind::OutOfMemory => Error::OutOfMemory { requested: 0 },
        _ => Error::Io {
            folder: folder.to_path_buf(),
            source,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(txid: u64, durable_upto: u64) -> Found {
        Found {
            file: 0,
            offset: 0,
            header: RecordHeader {
                txid,
                durable_upto,
                page_count: 0,
                compressed: false,
                stored_len: 0,
                raw_len: 0,
                payload_checksum: 0,
            },
        }
    }

    fn txids(found: &[Found], applied: u64) -> Result<Vec<u64>> {
        decide(found, applied, Path::new("store"))
            .map(|a| a.iter().map(|r| r.header.txid).collect())
    }

    #[test]
    fn applied_commits_are_skipped_and_the_rest_apply_in_order() {
        let found = [record(4, 3), record(5, 4), record(6, 5), record(7, 6)];
        assert_eq!(txids(&found, 5).unwrap(), vec![6, 7]);
    }

    #[test]
    fn a_missing_commit_nobody_saw_durable_ends_the_journal() {
        // 6 is missing; 7 was written before 6 was durable (deferred commits after a power cut).
        let found = [record(5, 4), record(7, 5), record(8, 5)];
        assert_eq!(txids(&found, 4).unwrap(), vec![5]);
    }

    /// D5 of the large values execution (AC-V8, Pager part): a commit whose record holds a sealed page and the pages
    /// of a run as raw entries, recovered after a process crash or a power cut at every point of its journal
    /// write, applies whole or not at all: the run reads back intact in one piece, or the commit is not there.
    #[test]
    fn raw_entries_of_a_run_recover_whole_or_not_at_all_after_a_crash_or_a_power_cut() {
        use crate::format::{PAGE_HEADER_SIZE, seal_page, seal_run};
        use crate::journal::{Journal, RAW_ENTRY};
        use crate::pager::OpenOptions;
        use crate::sys::faults::{self, Model};

        let base = tempfile::tempdir().unwrap();
        Pager::open(base.path(), &OpenOptions::default())
            .unwrap()
            .close()
            .unwrap();
        let data = base.path().join(crate::pager::DATA_FILE);
        let mut page = [0u8; PAGE_SIZE];
        seal_page(2, b"sealed", &mut page);
        let mut run = vec![0u8; 3 * PAGE_SIZE];
        for (i, b) in run[PAGE_HEADER_SIZE..].iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        seal_run(3, &mut run);
        let mut entries = vec![(2, &page, None)];
        for (i, run_page) in run.as_chunks::<PAGE_SIZE>().0.iter().enumerate() {
            entries.push(((3 + i as u64) | RAW_ENTRY, run_page, None));
        }

        let source = tempfile::tempdir().unwrap();
        fs::copy(&data, source.path().join(crate::pager::DATA_FILE)).unwrap();
        let mut pager = Pager::open(source.path(), &OpenOptions::default()).unwrap();
        let journal = Journal::start(source.path(), &pager).unwrap();
        let recovered = recover(&mut pager, source.path()).unwrap();
        journal.begin(recovered.last_txid, recovered.next_number);
        faults::reset();
        faults::start_recording();
        journal.enqueue(&pager, 1, &entries).unwrap();
        journal.make_durable(&pager).unwrap();
        let ops = faults::take_recording();
        // The crash: nothing reaches `krummi.data`, the journal holds the commit.
        drop(journal);
        drop(pager);

        let mut applied = 0;
        for cut in 0..=ops.len() {
            for model in [
                Model::Flushed,
                Model::TornFirst,
                Model::SkipFirst,
                Model::Written,
            ] {
                let target = tempfile::tempdir().unwrap();
                fs::copy(&data, target.path().join(crate::pager::DATA_FILE)).unwrap();
                faults::materialize(&ops, cut, source.path(), target.path(), model);
                let mut pager = Pager::open(target.path(), &OpenOptions::default()).unwrap();
                let recovered = recover(&mut pager, target.path()).unwrap();
                let at = format!("cut {cut} of {}, {model:?}", ops.len());
                if recovered.last_txid == 1 {
                    applied += 1;
                    assert_eq!(&pager.page(2).unwrap()[..6], b"sealed", "{at}");
                    assert_eq!(pager.run(3, 3).unwrap(), &run[..], "{at}");
                } else {
                    assert_eq!(recovered.last_txid, 0, "{at}");
                    assert!((2..6).all(|n| pager.page_is_blank(n)), "{at}");
                }
            }
        }
        assert!(applied > 0, "the durable commit was recovered");
    }

    /// D5: only raw entries skip the check of their page; a created sealed entry that does not verify is damage, and
    /// a raw entry for a header page too.
    #[test]
    fn only_raw_entries_skip_the_check_of_their_page() {
        use crate::journal::{RAW_ENTRY, encode_page};
        use crate::pager::OpenOptions;

        let tmp = tempfile::tempdir().unwrap();
        let mut pager = Pager::open(tmp.path(), &OpenOptions::default()).unwrap();
        let page = [7u8; PAGE_SIZE];
        let entry = |number: u64| {
            let mut entry = Vec::new();
            encode_page(&mut entry, (number, &page, None));
            entry
        };
        let mut applied = BTreeMap::new();
        apply_pages(
            &mut pager,
            &entry(3 | RAW_ENTRY),
            1,
            tmp.path(),
            &mut applied,
        )
        .unwrap();
        assert!(pager.raw_pages(3, 1).unwrap().iter().all(|&b| b == 7));
        assert_eq!(applied.get(&3), Some(&true));
        for damaged in [entry(4), entry(1 | RAW_ENTRY)] {
            let error = apply_pages(&mut pager, &damaged, 1, tmp.path(), &mut applied).unwrap_err();
            assert_eq!(error.kind(), crate::ErrorKind::Damaged);
        }
    }

    #[test]
    fn a_missing_commit_a_later_record_saw_durable_is_damage() {
        let found = [record(5, 4), record(7, 6)];
        assert_eq!(
            txids(&found, 4).unwrap_err().kind(),
            crate::ErrorKind::Damaged
        );
    }

    /// Sealed versions of page `number` for the tests of Y2: a first one, then each with a few runs of bytes changed,
    /// from a fixed seed.
    fn versions_of(number: u64, count: usize, seed: u64) -> Vec<[u8; PAGE_SIZE]> {
        use crate::format::{PAGE_HEADER_SIZE, seal_page};
        let mut x = seed;
        let mut next = move |bound: usize| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % bound as u64) as usize
        };
        let mut payload = vec![0u8; PAGE_SIZE - PAGE_HEADER_SIZE];
        let mut out = Vec::new();
        for _ in 0..count {
            for _ in 0..1 + next(6) {
                let at = next(payload.len() - 64);
                for b in &mut payload[at..at + 1 + next(60)] {
                    *b = next(256) as u8;
                }
            }
            let mut page = [0u8; PAGE_SIZE];
            seal_page(number, &payload, &mut page);
            out.push(page);
        }
        out
    }

    /// The raw payload of one commit holding the entry of `page`, copied from `base` or created.
    fn payload_of(number: u64, page: &[u8; PAGE_SIZE], base: Option<&[u8; PAGE_SIZE]>) -> Vec<u8> {
        let mut raw = Vec::new();
        crate::journal::encode_page(&mut raw, (number, page, base));
        raw
    }

    /// AC-Y3 (Y2): whatever bytes of the transferred version and of any later one, skipped versions included, a cut
    /// transfer leaves in a page, applying the entries of the commits after the transferred one gives the last
    /// version; so does a second recovery over the first one's result, and a chain that created the page again. A
    /// page whose bytes outside every entry were altered fails the check with its number (AC-Y4, Y3).
    #[test]
    fn entries_applied_over_any_mix_of_later_versions_give_the_last_one() {
        use crate::pager::OpenOptions;

        let tmp = tempfile::tempdir().unwrap();
        let mut pager = Pager::open(tmp.path(), &OpenOptions::default()).unwrap();
        const PAGE: u64 = 5;
        pager.ensure_pages_unbounded(PAGE + 1).unwrap();
        let mut x = 0x6B72_756D_6D69_0003u64;
        let mut next = move |bound: usize| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % bound as u64) as usize
        };
        for round in 0..200 {
            let versions = versions_of(PAGE, 6, 0x1000 + round);
            // Version 3 created again when `round` is odd: its entry goes over zeros.
            let created = round % 2 == 1;
            let transferred = next(3);
            let payloads: Vec<Vec<u8>> = (transferred + 1..versions.len())
                .map(|v| {
                    let base = (!(created && v == 3)).then(|| &versions[v - 1]);
                    payload_of(PAGE, &versions[v], base)
                })
                .collect();
            // Every byte from the transferred version or a later one; with the page created again, its bytes
            // before that may be anything, so they come from any version or are noise.
            let mut mixed = [0u8; PAGE_SIZE];
            for (i, b) in mixed.iter_mut().enumerate() {
                let from = transferred + next(versions.len() - transferred);
                *b = if created && from < 3 && next(2) == 0 {
                    next(256) as u8
                } else {
                    versions[from][i]
                };
            }
            pager.write_sealed(PAGE, &mixed).unwrap();
            for attempt in 0..2 {
                let mut applied = BTreeMap::new();
                for raw in &payloads {
                    apply_pages(&mut pager, raw, 1, tmp.path(), &mut applied).unwrap();
                }
                check_applied(&pager, &applied).unwrap();
                assert_eq!(
                    pager.raw_pages(PAGE, 1).unwrap(),
                    &versions[versions.len() - 1][..],
                    "round {round}, attempt {attempt}"
                );
            }
        }

        // AC-Y4: a byte that no entry holds, altered in the transferred version, is damage of that page.
        let versions = versions_of(PAGE, 3, 7);
        let raw = payload_of(PAGE, &versions[1], Some(&versions[0]));
        let held = crate::journal::decode_entries(&raw, 1)
            .unwrap()
            .remove(0)
            .runs;
        let outside = (crate::format::PAGE_HEADER_SIZE..PAGE_SIZE)
            .find(|&at| {
                !held
                    .iter()
                    .any(|&(start, bytes)| (start..start + bytes.len()).contains(&at))
            })
            .unwrap();
        let mut damaged = versions[0];
        damaged[outside] ^= 0x01;
        pager.write_sealed(PAGE, &damaged).unwrap();
        let mut applied = BTreeMap::new();
        apply_pages(&mut pager, &raw, 1, tmp.path(), &mut applied).unwrap();
        match check_applied(&pager, &applied) {
            Err(Error::Damaged { page, .. }) => assert_eq!(page, Some(PAGE)),
            other => panic!("expected damage, got {other:?}"),
        }
    }
}
