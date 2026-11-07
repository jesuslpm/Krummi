//! Tests of the scratch area: snapshots, publishing, flushing in place, retiring versions and the failures of a
//! flush (Transactions PRD: T2, T9, T12, T14, T16; AC-T15, AC20).

use std::io;

use super::*;
use crate::error::ErrorKind;
use crate::format::seal_run;
use crate::pager::OpenOptions;
use crate::sys::Op;
use crate::sys::faults::{self, Action};

const PAGE: u64 = 3;

/// Compiles only when `T` is not `Send`: otherwise the call below is ambiguous between the two impls.
trait NotSend<A> {
    fn check() {}
}
impl<T: ?Sized> NotSend<()> for T {}
impl<T: ?Sized + Send> NotSend<u8> for T {}

/// Compiles only when `T` is not `Sync`.
trait NotSync<A> {
    fn check() {}
}
impl<T: ?Sized> NotSync<()> for T {}
impl<T: ?Sized + Sync> NotSync<u8> for T {}

#[test]
fn the_writer_stays_on_its_thread_and_is_never_shared() {
    <ScratchWriter<'static> as NotSend<_>>::check();
    <ScratchWriter<'static> as NotSync<_>>::check();
}

fn payload(text: &str) -> PayloadBuf {
    let mut payload = alloc_payload().unwrap();
    payload[..text.len()].copy_from_slice(text.as_bytes());
    payload
}

fn text(payload: &[u8]) -> &str {
    std::str::from_utf8(
        &payload[..payload
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(payload.len())],
    )
    .unwrap()
}

/// A store, on disk or temporary, whose page `PAGE` holds "a" in the data file.
fn store(folder: Option<&std::path::Path>) -> Scratch {
    let mut pager = match folder {
        Some(folder) => Pager::open(folder, &OpenOptions::default()).unwrap(),
        None => Pager::open_temporary(&OpenOptions::default()).unwrap(),
    };
    pager.ensure_pages(PAGE + 1).unwrap();
    pager.write_page(PAGE, b"a").unwrap();
    Scratch::new(pager)
}

fn commit(scratch: &Scratch, text: &str) -> u64 {
    let mut writer = scratch.begin_write().unwrap();
    writer.flush().unwrap();
    let staged = writer.stage(&[(PAGE, payload(text))], &[]).unwrap();
    writer.publish(staged)
}

fn both_modes(test: impl Fn(Scratch)) {
    let tmp = tempfile::tempdir().unwrap();
    test(store(Some(tmp.path())));
    test(store(None));
}

#[test]
fn a_snapshot_sees_what_was_committed_when_it_opened() {
    both_modes(|scratch| {
        let before = scratch.snapshot().unwrap();
        assert_eq!(commit(&scratch, "b"), 1);
        let after = scratch.snapshot().unwrap();
        assert_eq!(text(before.page(PAGE).unwrap()), "a");
        assert_eq!(text(after.page(PAGE).unwrap()), "b");
        assert_eq!((before.txid(), after.txid()), (0, 1));
    });
}

#[test]
fn a_version_waits_in_scratch_while_an_older_snapshot_is_open() {
    both_modes(|scratch| {
        let old = scratch.snapshot().unwrap();
        commit(&scratch, "b");
        scratch.begin_write().unwrap().flush().unwrap();
        assert_eq!(text(scratch.pager().page(PAGE).unwrap()), "a");
        assert_eq!(scratch.scratch_bytes(), PAGE_SIZE_U64);
        drop(old);
        scratch.begin_write().unwrap().flush().unwrap();
        assert_eq!(text(scratch.pager().page(PAGE).unwrap()), "b");
        assert_eq!(scratch.scratch_bytes(), 0);
    });
}

#[test]
fn references_outlive_the_flush_of_their_version() {
    both_modes(|scratch| {
        commit(&scratch, "b");
        let reader = scratch.snapshot().unwrap();
        let first = reader.page(PAGE).unwrap();
        let again = reader.page(PAGE).unwrap();
        assert_eq!(first.as_ptr(), again.as_ptr(), "same version, same address");
        // The reader sees the newest version, so the flush writes it in place and retires the scratch copy,
        // which stays allocated while the reader is open.
        scratch.begin_write().unwrap().flush().unwrap();
        assert_eq!(scratch.scratch_bytes(), 0);
        assert_eq!(text(first), "b");
        assert_eq!(text(reader.page(PAGE).unwrap()), "b");
    });
}

#[test]
fn versions_nobody_can_read_are_retired() {
    both_modes(|scratch| {
        let old = scratch.snapshot().unwrap();
        for text in ["b", "c", "d"] {
            commit(&scratch, text);
        }
        scratch.begin_write().unwrap().flush().unwrap();
        // "b" and "c" are newer than the old snapshot and older than "d": nobody reads them.
        assert_eq!(scratch.scratch_bytes(), PAGE_SIZE_U64);
        assert_eq!(text(old.page(PAGE).unwrap()), "a");
        assert_eq!(text(scratch.snapshot().unwrap().page(PAGE).unwrap()), "d");
    });
}

#[test]
fn a_version_an_open_snapshot_reads_is_kept() {
    both_modes(|scratch| {
        commit(&scratch, "b");
        let middle = scratch.snapshot().unwrap();
        let old_ref = middle.page(PAGE).unwrap();
        commit(&scratch, "c");
        scratch.begin_write().unwrap().flush().unwrap();
        assert_eq!(text(middle.page(PAGE).unwrap()), "b");
        assert_eq!(text(old_ref), "b");
        assert_eq!(text(scratch.snapshot().unwrap().page(PAGE).unwrap()), "c");
    });
}

/// Sets its flag when dropped, so a reader loop ends even when the thread that owns it panics.
struct SetOnDrop<'a>(&'a std::sync::atomic::AtomicBool);

impl Drop for SetOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// O2 of the performance plan: a snapshot that opened with no version in the scratch area reads the store without
/// the scratch lock, so newer versions of its page, published and written in place meanwhile, never reach it.
#[test]
fn a_snapshot_opened_without_versions_keeps_its_page_while_newer_ones_are_written() {
    both_modes(|scratch| {
        let reader = scratch.snapshot().unwrap();
        let done = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|s| {
            s.spawn(|| {
                let _done = SetOnDrop(&done);
                for i in 0..300 {
                    commit(&scratch, &format!("v{i}"));
                    scratch.begin_write().unwrap().flush().unwrap();
                    scratch.transfer().unwrap();
                }
            });
            while !done.load(std::sync::atomic::Ordering::Relaxed) {
                assert_eq!(text(reader.page(PAGE).unwrap()), "a");
            }
        });
        assert_eq!(text(reader.page(PAGE).unwrap()), "a");
        drop(reader);
        assert_eq!(
            text(scratch.snapshot().unwrap().page(PAGE).unwrap()),
            "v299"
        );
    });
}

/// The writer's snapshot sees what was committed when it began, not what it publishes: its own versions are newer
/// than it, so a writer that began on an empty scratch area may skip the scratch lock too (O2).
#[test]
fn the_writers_snapshot_does_not_see_what_it_publishes() {
    both_modes(|scratch| {
        let mut writer = scratch.begin_write().unwrap();
        let staged = writer.stage(&[(PAGE, payload("b"))], &[]).unwrap();
        assert_eq!(writer.publish(staged), 1);
        assert_eq!(writer.snapshot().txid(), 0);
        assert_eq!(text(writer.snapshot().page(PAGE).unwrap()), "a");
        drop(writer);
        assert_eq!(text(scratch.snapshot().unwrap().page(PAGE).unwrap()), "b");
    });
}

#[test]
fn close_and_drop_write_what_was_committed() {
    let tmp = tempfile::tempdir().unwrap();
    let scratch = store(Some(tmp.path()));
    let old = scratch.snapshot().unwrap();
    commit(&scratch, "b");
    drop(old);
    scratch.close().unwrap();
    let pager = Pager::open(tmp.path(), &OpenOptions::default()).unwrap();
    assert_eq!(text(pager.page(PAGE).unwrap()), "b");
    let scratch = Scratch::new(pager);
    commit(&scratch, "c");
    drop(scratch);
    let pager = Pager::open(tmp.path(), &OpenOptions::default()).unwrap();
    assert_eq!(text(pager.page(PAGE).unwrap()), "c");
}

#[test]
fn a_failed_flush_refuses_writes_and_close_retries_it() {
    let tmp = tempfile::tempdir().unwrap();
    let scratch = store(Some(tmp.path()));
    commit(&scratch, "b");
    faults::reset();
    faults::inject(Op::Write, 0, Action::Fail(io::ErrorKind::Other));
    let error = scratch.begin_write().unwrap().flush().unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Io, "{error}");
    assert!(!scratch.pager().accepts_writes());
    assert_eq!(text(scratch.snapshot().unwrap().page(PAGE).unwrap()), "b");
    scratch.close().unwrap();
    let pager = Pager::open(tmp.path(), &OpenOptions::default()).unwrap();
    assert_eq!(text(pager.page(PAGE).unwrap()), "b");
}

#[test]
fn close_reports_a_flush_that_keeps_failing() {
    let tmp = tempfile::tempdir().unwrap();
    let scratch = store(Some(tmp.path()));
    commit(&scratch, "b");
    faults::reset();
    faults::inject(Op::Write, 0, Action::Fail(io::ErrorKind::Other));
    assert_eq!(scratch.close().unwrap_err().kind(), ErrorKind::Io);
    faults::reset();
    Pager::open(tmp.path(), &OpenOptions::default()).unwrap();
}

#[test]
fn a_snapshot_can_run_out_of_memory() {
    both_modes(|scratch| {
        faults::reset();
        faults::inject(Op::Snapshot, 0, Action::Fail(io::ErrorKind::OutOfMemory));
        assert_eq!(
            scratch.snapshot().err().unwrap().kind(),
            ErrorKind::OutOfMemory
        );
        faults::reset();
        assert_eq!(text(scratch.snapshot().unwrap().page(PAGE).unwrap()), "a");
    });
}

/// A run of `count` pages from page `first`, sealed (D1 of the large values execution), whose content is `fill`.
fn sealed_run(first: u64, count: u64, fill: u8) -> RunBuf {
    let mut run = alloc_run(count).unwrap();
    run[PAGE_HEADER_SIZE..].fill(fill);
    seal_run(first, &mut run);
    run
}

/// V7 of the large values PRD, D5 of its execution: a snapshot reads a run only whole and from one place. Asking for
/// a published run with fewer or more pages than it has, from one of its middle pages, or over a range that starts
/// in the store and ends in the run's versions of the temporary area, is the damage class with the first page asked
/// for; the run itself reads back whole.
#[test]
fn a_run_is_read_only_whole_and_from_one_place() {
    both_modes(|scratch| {
        let first = PAGE + 2;
        scratch.pager().ensure_pages(first + 4).unwrap();
        let mut writer = scratch.begin_write().unwrap();
        let staged = writer
            .stage(&[], &[(first, sealed_run(first, 3, 7))])
            .unwrap();
        writer.publish(staged);
        drop(writer);
        let snapshot = scratch.snapshot().unwrap();
        let run = snapshot.run(first, 3).unwrap();
        assert_eq!(run.len(), 3 * PAGE_SIZE);
        assert!(run[PAGE_HEADER_SIZE..].iter().all(|&b| b == 7));
        let damaged = |at: u64, count: u64| match snapshot.run(at, count) {
            Err(Error::Damaged { page, .. }) => page,
            other => panic!("expected damage, got {other:?}"),
        };
        assert_eq!(damaged(first, 2), Some(first), "fewer pages than the run");
        assert_eq!(damaged(first, 4), Some(first), "more pages than the run");
        assert_eq!(damaged(first + 1, 2), Some(first + 1), "a middle page");
        assert_eq!(
            damaged(first - 1, 4),
            Some(first - 1),
            "partly in the store and partly in the temporary area"
        );
    });
}

/// The run that [`ScratchWriter::stage`] gets must be whole pages, as its documentation says: part of a page is a
/// misuse that panics before anything is staged.
#[test]
#[should_panic(expected = "cannot be published")]
fn staging_a_run_that_is_not_whole_pages_panics() {
    let scratch = store(None);
    scratch.pager().ensure_pages(PAGE + 3).unwrap();
    let writer = scratch.begin_write().unwrap();
    let run: RunBuf = vec![0; PAGE_SIZE + 1].into_boxed_slice();
    let _ = writer.stage(&[], &[(PAGE + 1, run)]);
}

#[test]
fn rewriting_a_page_under_a_long_snapshot_keeps_memory_bounded() {
    both_modes(|scratch| {
        let long = scratch.snapshot().unwrap();
        for i in 0..3000 {
            commit(&scratch, &i.to_string());
        }
        // The long snapshot needs the data file's version; of the rest only the newest one is needed, and the
        // versions nobody can read are freed at once instead of waiting for the long snapshot to end.
        assert!(scratch.scratch_bytes() <= 2 * PAGE_SIZE_U64);
        assert!(scratch.lock().retired.is_empty());
        assert_eq!(text(long.page(PAGE).unwrap()), "a");
        assert_eq!(
            text(scratch.snapshot().unwrap().page(PAGE).unwrap()),
            "2999"
        );
    });
}

/// Candidate 3 of the writes diagnosis (performance plan): a commit that goes on in a new journal file does not close
/// the old one itself; the journal's thread does, off the commit.
#[test]
fn the_journal_thread_closes_the_file_a_commit_rolls_away_from() {
    let tmp = tempfile::tempdir().unwrap();
    let pager = Pager::open(tmp.path(), &OpenOptions::default()).unwrap();
    pager.ensure_pages(PAGE + 1).unwrap();
    let scratch = Scratch::open(pager).unwrap();
    // Every record gets a file of its own.
    scratch.set_journal_file_limit(crate::journal::ALIGN);
    for text in ["b", "c", "d"] {
        let mut writer = scratch.begin_write().unwrap();
        let txid = writer.commit(&[(PAGE, payload(text))], &[]).unwrap();
        drop(writer);
        scratch.wait_durable(txid).unwrap();
    }
    assert_eq!(crate::journal::files_closed_on_this_thread(), 0);
    // Each commit went on in a new file, and the writer took the old ones before writing the next record.
    let (next_number, retired) = scratch.shared.journal.as_ref().unwrap().rolls_and_retired();
    assert!(next_number >= 3, "{next_number} files");
    assert_eq!(retired, 0);
    assert_eq!(text(scratch.snapshot().unwrap().page(PAGE).unwrap()), "d");
    scratch.close().unwrap();
}
