//! Tests of the scratch area: snapshots, publishing, flushing in place, retiring versions and the failures of a
//! flush (Transactions PRD: T2, T9, T12, T14, T16; AC-T15, AC20).

use std::io;

use super::*;
use crate::error::ErrorKind;
use crate::format::seal_page;
use crate::pager::OpenOptions;
use crate::sys::Op;
use crate::sys::faults::{self, Action};

const PAGE: u64 = 3;

fn sealed(page_number: u64, text: &str) -> SealedPage {
    let mut page = alloc_sealed().unwrap();
    seal_page(page_number, text.as_bytes(), &mut page);
    page
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
    writer.publish(vec![(PAGE, sealed(PAGE, text))])
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
