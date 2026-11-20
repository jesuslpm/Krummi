//! Failures of the statistics work (AC-ST8 of the statistics PRD, R13 of its first Codex code review): a save that
//! fails keeps the statistics in memory and a later estimate saves them again; analyze on the store reports its save's
//! failure and leaves what it published in memory.

use std::io;

use crate::table::value::{ColumnType, Value};
use crate::{Column, Condition, Error, ErrorKind, Schema, Store, StoreOptions, TableMode};

fn store_with_rows(folder: &std::path::Path) -> Store {
    let store = Store::open(folder, &StoreOptions::default()).unwrap();
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64),
        ],
        &["id"],
    );
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema))
        .unwrap();
    for id in 0..1_000 {
        write
            .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(id % 3))])
            .unwrap();
    }
    write.commit().unwrap();
    store
}

fn estimate(store: &Store) {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let condition = Condition::Equal {
        column: "n",
        value: Value::I64(1),
    };
    read.estimate(&table, &condition).unwrap();
}

fn seen(store: &Store) -> bool {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    read.column_statistics(&table, "n").unwrap().is_some()
}

fn disk_full(folder: &std::path::Path) -> Error {
    Error::DiskFull {
        folder: folder.to_path_buf(),
    }
}

/// ST9: a background save that fails leaves the store as it was and the statistics in memory; the next estimate that
/// uses them asks again, and that save lasts after reopening.
#[test]
fn a_failed_background_save_keeps_memory_and_is_asked_again() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_with_rows(tmp.path());
    store
        .shared()
        .engine
        .set_page_fault(Some((0, disk_full(tmp.path()))));
    estimate(&store);
    store.wait_statistics_idle();
    assert!(
        !store.shared().engine.page_fault_pending(),
        "the save failed"
    );
    assert!(seen(&store), "still in memory");
    // Not saved: a reopening would lose them; the next estimate asks to save them again.
    estimate(&store);
    store.wait_statistics_idle();
    store.close().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    assert!(seen(&store), "saved by the second request");
}

/// ST7: analyze on the store reports its save's failure with its class (a full disk, memory running out or
/// input/output) and leaves what it published in memory; a failure there is the write's (T11) and the store goes on
/// accepting writes.
#[test]
fn analyze_reports_its_save_failure_and_keeps_memory() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_with_rows(tmp.path());
    store
        .shared()
        .engine
        .set_page_fault(Some((0, disk_full(tmp.path()))));
    let error = store.analyze("t", None).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::DiskFull);
    assert!(seen(&store));
    store.shared().engine.set_page_fault(Some((
        0,
        Error::Io {
            folder: tmp.path().to_path_buf(),
            source: io::Error::other("injected"),
        },
    )));
    assert_eq!(store.analyze("t", None).unwrap_err().kind(), ErrorKind::Io);
    // R13 of the code review: memory running out while the save writes its pages.
    store
        .shared()
        .engine
        .set_page_fault(Some((0, Error::OutOfMemory { requested: 8_192 })));
    assert_eq!(
        store.analyze("t", None).unwrap_err().kind(),
        ErrorKind::OutOfMemory
    );
    assert!(seen(&store));
    store.write().unwrap().commit().unwrap();
}

/// ST9: a background renewal whose save meets a full disk leaves the store as it was: writes go on, the renewed
/// statistics serve from memory, and the next estimate that uses them asks to save them again, which lasts.
#[test]
fn a_renewal_with_a_full_disk_is_dropped_and_asked_again() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_with_rows(tmp.path());
    store.analyze("t", None).unwrap();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    for id in 1_000..2_000 {
        write
            .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(id % 3))])
            .unwrap();
    }
    write.commit().unwrap();
    let stale = |store: &Store| {
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let condition = Condition::Equal {
            column: "n",
            value: Value::I64(1),
        };
        read.estimate(&table, &condition).unwrap().stale
    };
    store
        .shared()
        .engine
        .set_page_fault(Some((0, disk_full(tmp.path()))));
    assert!(stale(&store));
    store.wait_statistics_idle();
    assert!(
        !store.shared().engine.page_fault_pending(),
        "the renewal's save failed"
    );
    let mut write = store.write().unwrap();
    write
        .open_tree_with("after", crate::OpenMode::CreateNew)
        .unwrap();
    write.commit().unwrap();
    // The renewal published them before its save failed: they serve from memory, and this estimate asks to save them.
    assert!(!stale(&store), "renewed in memory");
    store.wait_statistics_idle();
    store.close().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let saved = read.column_statistics(&table, "n").unwrap().unwrap();
    assert_eq!(
        saved.origin_rows, 2_000,
        "saved by the request that followed"
    );
}

/// Fails every write to the files from now on, once everything committed is in `krummi.data`: only a later commit meets
/// the faults (R13 of the code review: the earlier commit's transfer must not).
fn fail_writes_after_the_transfer(store: &Store) {
    use krummi_pager::testing::{self, Action, Op};
    store.shared().engine.checkpoint().unwrap();
    testing::reset();
    for nth in 0..1_000 {
        testing::inject(Op::Write, nth, Action::Fail(io::ErrorKind::Other));
    }
}

/// ST9, T12: a background save whose commit fails to write makes the store refuse writes: the next write request fails
/// with the input/output class, estimates go on from memory, another table's estimate samples without asking for a
/// save, and closing then succeeds and the store reopens with its rows.
#[test]
fn a_failed_background_commit_makes_the_store_refuse_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_with_rows(tmp.path());
    let mut write = store.write().unwrap();
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64),
        ],
        &["id"],
    );
    let other = write
        .open_table_with("u", TableMode::CreateNew(&schema))
        .unwrap();
    write
        .insert_row(&other, &[("id", Value::I64(1)), ("n", Value::I64(1))])
        .unwrap();
    write.commit().unwrap();
    fail_writes_after_the_transfer(&store);
    estimate(&store);
    store.wait_statistics_idle();
    krummi_pager::testing::reset();
    assert_eq!(store.write().err().map(|e| e.kind()), Some(ErrorKind::Io));
    assert!(seen(&store), "memory still serves");
    let samples = store.statistics_samples();
    let read = store.read().unwrap();
    let table = read.open_table("u").unwrap();
    let condition = Condition::Equal {
        column: "n",
        value: Value::I64(1),
    };
    read.estimate(&table, &condition).unwrap();
    drop(read);
    assert_eq!(store.statistics_samples(), samples + 1);
    assert_eq!(store.statistics_work().1, 0, "no save asked for");
    store.close().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    assert_eq!(read.row_count(&table).unwrap(), 1_000);
}

/// ST7, T12: analyze on the store whose save commit fails to write reports the input/output class, keeps what it
/// published in memory, and the store refuses writes after it.
#[test]
fn analyze_with_a_failed_commit_refuses_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store_with_rows(tmp.path());
    fail_writes_after_the_transfer(&store);
    let error = store.analyze("t", None).unwrap_err();
    krummi_pager::testing::reset();
    assert_eq!(error.kind(), ErrorKind::Io);
    assert_eq!(store.write().err().map(|e| e.kind()), Some(ErrorKind::Io));
    assert!(seen(&store), "kept in memory");
}
