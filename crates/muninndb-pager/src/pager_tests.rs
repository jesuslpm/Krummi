//! Fault tests of the Pager: injected disk-full, I/O and memory failures, process kills and power cuts
//! (Pager PRD: P8, P10, P11, P13, P14; AC-P9, AC-P14, AC-P15, AC-P16; parent AC11, AC12).

use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use super::*;
use crate::error::ErrorKind;
use crate::sys::Op;
use crate::sys::faults::{self, Action};

fn options() -> OpenOptions {
    OpenOptions::default()
}

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn assert_only_a_complete_store(folder: &Path) {
    let pager = Pager::open(folder, &options()).unwrap();
    assert_eq!(pager.info().size_bytes, INITIAL_SIZE);
    pager.close().unwrap();
    assert_eq!(names(folder), vec![DATA_FILE.to_owned()]);
}

fn fail(op: Op, nth: usize, kind: io::ErrorKind) {
    faults::reset();
    faults::inject(op, nth, Action::Fail(kind));
}

#[test]
fn ac_p15_disk_full_while_creating_leaves_nothing_behind() {
    let tmp = tempfile::tempdir().unwrap();
    fail(Op::Reserve, 0, io::ErrorKind::StorageFull);
    let error = Pager::open(tmp.path(), &options()).err().unwrap();
    faults::reset();
    assert_eq!(error.kind(), ErrorKind::DiskFull, "{error}");
    assert!(names(tmp.path()).is_empty(), "{:?}", names(tmp.path()));
    assert_only_a_complete_store(tmp.path());
}

#[test]
fn an_io_failure_while_creating_is_an_io_error_and_leaves_nothing_behind() {
    let tmp = tempfile::tempdir().unwrap();
    fail(Op::Write, 0, io::ErrorKind::Other);
    let error = Pager::open(tmp.path(), &options()).err().unwrap();
    faults::reset();
    assert_eq!(error.kind(), ErrorKind::Io, "{error}");
    assert!(names(tmp.path()).is_empty());
    assert_only_a_complete_store(tmp.path());
}

#[test]
fn ac_p15_and_ac11_disk_full_while_growing_keeps_the_store_usable() {
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = Pager::open(tmp.path(), &options()).unwrap();
    pager.write_page(2, b"before").unwrap();
    let size = pager.info().size_bytes;
    fail(Op::Reserve, 0, io::ErrorKind::StorageFull);
    let error = pager.ensure_pages(pager.page_count() + 1).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::DiskFull, "{error}");
    assert_eq!(pager.info().size_bytes, size);
    assert_eq!(
        fs::metadata(tmp.path().join(DATA_FILE)).unwrap().len(),
        size
    );
    assert_eq!(&pager.page(2).unwrap()[..6], b"before");
    pager.write_page(3, b"still writable").unwrap();
    // Space is freed: the next growth works without reopening.
    pager.ensure_pages(pager.page_count() + 1).unwrap();
    faults::reset();
    assert!(pager.info().size_bytes > size);
}

#[test]
fn ac12_a_failed_write_refuses_later_writes_until_reopened() {
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = Pager::open(tmp.path(), &options()).unwrap();
    pager.write_page(2, b"written before").unwrap();
    pager.sync().unwrap();
    fail(Op::Write, 0, io::ErrorKind::Other);
    assert_eq!(
        pager.write_page(3, b"x").err().unwrap().kind(),
        ErrorKind::Io
    );
    faults::reset();
    assert_eq!(
        pager.write_page(4, b"y").err().unwrap().kind(),
        ErrorKind::Io,
        "writes stay refused"
    );
    assert_eq!(
        pager.ensure_pages(1000).err().unwrap().kind(),
        ErrorKind::Io,
        "growth too"
    );
    assert_eq!(
        &pager.page(2).unwrap()[..14],
        b"written before",
        "reads still work"
    );
    assert_eq!(
        pager.info().page_count,
        INITIAL_SIZE / PAGE_SIZE_U64,
        "info still works"
    );
    pager.close().unwrap();
    let mut pager = Pager::open(tmp.path(), &options()).unwrap();
    assert_eq!(&pager.page(2).unwrap()[..14], b"written before");
    pager.write_page(3, b"writable again").unwrap();
}

#[test]
fn ac12_a_failed_flush_also_refuses_later_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = Pager::open(tmp.path(), &options()).unwrap();
    pager.write_page(2, b"a").unwrap();
    fail(Op::Sync, 0, io::ErrorKind::Other);
    assert_eq!(pager.sync().err().unwrap().kind(), ErrorKind::Io);
    faults::reset();
    assert_eq!(
        pager.write_page(2, b"b").err().unwrap().kind(),
        ErrorKind::Io
    );
}

#[test]
fn a_disk_full_write_does_not_refuse_later_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = Pager::open(tmp.path(), &options()).unwrap();
    fail(Op::Write, 0, io::ErrorKind::StorageFull);
    assert_eq!(
        pager.write_page(2, b"a").err().unwrap().kind(),
        ErrorKind::DiskFull
    );
    faults::reset();
    pager.write_page(2, b"b").unwrap();
}

#[test]
fn ac_p16_a_failed_flush_on_close_is_reported_and_releases_the_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = Pager::open(tmp.path(), &options()).unwrap();
    pager.write_page(2, b"a").unwrap();
    fail(Op::Sync, 0, io::ErrorKind::Other);
    let error = pager.close().err().unwrap();
    faults::reset();
    assert_eq!(error.kind(), ErrorKind::Io);
    Pager::open(tmp.path(), &options())
        .unwrap()
        .close()
        .unwrap();
}

#[test]
fn ac_p14_out_of_memory_in_a_temporary_store_keeps_it_usable() {
    let mut pager = Pager::open_temporary(&options()).unwrap();
    pager.write_page(2, b"kept").unwrap();
    fail(Op::Alloc, 0, io::ErrorKind::OutOfMemory);
    let error = pager.ensure_pages(pager.page_count() + 1).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::OutOfMemory);
    assert_eq!(pager.info().size_bytes, INITIAL_SIZE);
    assert_eq!(&pager.page(2).unwrap()[..4], b"kept");
    pager.ensure_pages(pager.page_count() + 1).unwrap();
    faults::reset();
    assert!(pager.info().temporary);
}

#[test]
fn ac_p14_out_of_memory_when_opening_a_temporary_store() {
    fail(Op::Alloc, 0, io::ErrorKind::OutOfMemory);
    let error = Pager::open_temporary(&options()).err().unwrap();
    faults::reset();
    assert_eq!(error.kind(), ErrorKind::OutOfMemory);
}

#[test]
fn ac_p14_out_of_memory_while_growing_a_store_on_disk_keeps_it_usable() {
    let tmp = tempfile::tempdir().unwrap();
    let mut pager = Pager::open(tmp.path(), &options()).unwrap();
    pager.write_page(2, b"kept").unwrap();
    // The first map call of this thread was made when opening; the next one is the growth.
    fail(Op::Map, 0, io::ErrorKind::OutOfMemory);
    let error = pager.ensure_pages(pager.page_count() + 1).err().unwrap();
    faults::reset();
    assert_eq!(error.kind(), ErrorKind::OutOfMemory);
    assert_eq!(pager.info().size_bytes, INITIAL_SIZE);
    // Windows cannot shrink a file while it is mapped, and the earlier mappings must stay alive (P15): there the
    // reserved space stays in the file (recorded as a deviation from P11 in odd/tasks/pager.md).
    if cfg!(not(windows)) {
        assert_eq!(
            fs::metadata(tmp.path().join(DATA_FILE)).unwrap().len(),
            INITIAL_SIZE
        );
    }
    assert_eq!(&pager.page(2).unwrap()[..4], b"kept");
    pager.write_page(3, b"still writable").unwrap();
}

/// Number of fault points (operations of `sys`) a creation goes through.
fn creation_points() -> usize {
    let tmp = tempfile::tempdir().unwrap();
    faults::reset();
    Pager::open(tmp.path(), &options()).unwrap();
    let points = faults::points();
    faults::reset();
    points
}

const ABORT_FOLDER_ENV: &str = "MUNINN_TEST_ABORT_FOLDER";
const ABORT_POINT_ENV: &str = "MUNINN_TEST_ABORT_POINT";

#[test]
fn ac_p9_a_creation_killed_at_any_point_leaves_a_complete_store_or_none() {
    let points = creation_points();
    assert!(
        points >= 6,
        "a creation creates, writes, reserves, flushes, renames and maps: {points}"
    );
    for point in 0..points {
        let tmp = tempfile::tempdir().unwrap();
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "pager::tests::child_helper_create_and_abort",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(ABORT_FOLDER_ENV, tmp.path())
            .env(ABORT_POINT_ENV, point.to_string())
            .status()
            .unwrap();
        assert!(
            !status.success(),
            "point {point}: the child should have been killed"
        );
        assert_only_a_complete_store(tmp.path());
    }
}

/// Helper run in a child process by the test above: creates a store and kills itself at the chosen point.
#[test]
fn child_helper_create_and_abort() {
    let (Some(folder), Some(point)) = (
        std::env::var_os(ABORT_FOLDER_ENV),
        std::env::var(ABORT_POINT_ENV).ok(),
    ) else {
        return;
    };
    faults::reset();
    faults::abort_at(point.parse().unwrap());
    let _ = Pager::open(Path::new(&folder), &options());
    panic!("the process should have been killed at point {point}");
}

#[test]
fn ac_p9_a_power_cut_during_or_after_creation_leaves_a_complete_store_or_none() {
    let source = tempfile::tempdir().unwrap();
    faults::reset();
    faults::start_recording();
    Pager::open(source.path(), &options())
        .unwrap()
        .close()
        .unwrap();
    let ops = faults::take_recording();
    faults::reset();
    for cut in 0..=ops.len() {
        let target = tempfile::tempdir().unwrap();
        faults::materialize_power_cut(&ops, cut, source.path(), target.path());
        assert_only_a_complete_store(target.path());
        if cut == ops.len() {
            // The creation had finished: the store must have survived as created.
            assert_eq!(
                fs::read(target.path().join(DATA_FILE)).unwrap(),
                fs::read(source.path().join(DATA_FILE)).unwrap()
            );
        }
    }
}

#[test]
fn ac_p14_out_of_memory_while_opening_a_store_on_disk_releases_it() {
    let tmp = tempfile::tempdir().unwrap();
    Pager::open(tmp.path(), &options())
        .unwrap()
        .close()
        .unwrap();
    fail(Op::Map, 0, io::ErrorKind::OutOfMemory);
    let error = Pager::open(tmp.path(), &options()).err().unwrap();
    faults::reset();
    assert_eq!(error.kind(), ErrorKind::OutOfMemory);
    Pager::open(tmp.path(), &options())
        .unwrap()
        .close()
        .unwrap();
}

#[test]
fn folders_created_by_open_are_kept_when_the_creation_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("a").join("b").join("store");
    fail(Op::Reserve, 0, io::ErrorKind::StorageFull);
    let error = Pager::open(&folder, &options()).err().unwrap();
    faults::reset();
    assert_eq!(error.kind(), ErrorKind::DiskFull);
    assert!(folder.is_dir());
    assert!(names(&folder).is_empty());
}

#[test]
fn folders_created_by_open_are_made_durable() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("a").join("b").join("store");
    faults::reset();
    faults::start_recording();
    Pager::open(&folder, &options()).unwrap().close().unwrap();
    let synced: Vec<_> = faults::take_recording()
        .into_iter()
        .filter_map(|op| match op {
            faults::Recorded::SyncDir(dir) => Some(dir),
            _ => None,
        })
        .collect();
    faults::reset();
    // The parent of every folder level that open created, then the store folder after the rename.
    for parent in [
        tmp.path().to_path_buf(),
        tmp.path().join("a"),
        tmp.path().join("a").join("b"),
        folder.clone(),
    ] {
        assert!(
            synced.contains(&parent),
            "{} not synced: {synced:?}",
            parent.display()
        );
    }
}

#[test]
fn an_impossible_growth_is_a_maximum_size_error() {
    let pager = Pager::open_temporary(&options()).unwrap();
    assert_eq!(
        pager.ensure_pages(u64::MAX).err().unwrap().kind(),
        ErrorKind::MaxSize
    );
    // A size that fits in a u64 but not in memory is an out-of-memory error in temporary mode.
    assert_eq!(
        pager
            .ensure_pages(u64::MAX / PAGE_SIZE_U64)
            .err()
            .unwrap()
            .kind(),
        ErrorKind::OutOfMemory
    );
    assert_eq!(pager.info().size_bytes, INITIAL_SIZE);
}

#[cfg(windows)]
#[test]
fn store_file_names_are_case_insensitive_on_windows() {
    let tmp = tempfile::tempdir().unwrap();
    Pager::open(tmp.path(), &options())
        .unwrap()
        .close()
        .unwrap();
    fs::rename(tmp.path().join(DATA_FILE), tmp.path().join("MUNINN.DATA")).unwrap();
    Pager::open(tmp.path(), &options())
        .unwrap()
        .close()
        .unwrap();
    let leftover = tempfile::tempdir().unwrap();
    fs::write(leftover.path().join("Muninn.Data.New"), b"half").unwrap();
    Pager::open(leftover.path(), &options())
        .unwrap()
        .close()
        .unwrap();
}
