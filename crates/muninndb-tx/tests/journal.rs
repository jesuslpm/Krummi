//! Tests of the journal and recovery (Journal PRD, module 3) at the transaction level.

mod common;

use std::fs;
use std::path::Path;

use common::{disk, new_page, read_text, set};
use muninndb_pager::TEMP_DIR;

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// AC-J11: the versions a long read retains live in files of `muninn.temp`, which the close removes.
#[test]
fn ac_j11_retained_versions_live_in_muninn_temp_until_close() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "v0");
    let read = engine.read().unwrap();
    for i in 1..=20 {
        set(&engine, page, &format!("v{i}"));
    }
    let temp = tmp.path().join(TEMP_DIR);
    assert!(temp.is_dir());
    assert!(!names(&temp).is_empty(), "the area has files");
    assert_eq!(common::text(read.page(page).unwrap()), "v0");
    assert_eq!(read_text(&engine, page), "v20");
    drop(read);
    engine.close().unwrap();
    assert!(!temp.exists());
    let engine = disk(tmp.path());
    assert_eq!(read_text(&engine, page), "v20");
    engine.close().unwrap();
}

/// J7: a failure of the temporary area makes the commit fail with its class and apply nothing.
#[test]
fn a_failure_of_the_temporary_area_fails_the_commit_without_applying_it() {
    use muninndb_pager::testing::{Action, Op, inject, reset};
    use muninndb_tx::ErrorKind;
    use std::io;
    for (kind, class) in [
        (io::ErrorKind::PermissionDenied, ErrorKind::Access),
        (io::ErrorKind::StorageFull, ErrorKind::DiskFull),
        (io::ErrorKind::OutOfMemory, ErrorKind::OutOfMemory),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = disk(tmp.path());
        reset();
        // The first operation on the area removes a leftover folder; the second creates it.
        inject(Op::Temp, 1, Action::Fail(kind));
        let mut tx = engine.write(None).unwrap();
        let page = tx.allocate().unwrap();
        common::put(&mut tx, page, "lost");
        assert_eq!(tx.commit().unwrap_err().kind(), class, "{kind:?}");
        reset();
        let page = new_page(&engine, "kept");
        assert_eq!(read_text(&engine, page), "kept");
        engine.close().unwrap();
    }
}
