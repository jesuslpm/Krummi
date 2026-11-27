//! Statistics of the tables through the public API (statistics PRD 7.3): the modification counter (AC-ST3), building
//! and reading them (AC-ST12, parts of AC-ST1 and AC-ST6), analyze and its errors (part of AC-ST9) and the format
//! (AC-ST11). Each criterion runs on a store on disk and on a temporary one, except the parts that reopen or rewrite
//! the store's files, which run on disk only.

#[path = "../../krummi-tx/tests/common/mod.rs"]
mod common;
mod support;

#[path = "table_statistics/background.rs"]
mod background;
#[path = "table_statistics/build.rs"]
mod build;
#[path = "table_statistics/codex.rs"]
mod codex;
#[path = "table_statistics/columns.rs"]
mod columns;
#[path = "table_statistics/estimate.rs"]
mod estimate;

use std::convert::Infallible;

use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, Schema, Store, TableMode, WriteTable, WriteTransaction};
use krummi_pager::format::{Header, PAGE_SIZE, header_copies, write_header_pages};
use support::*;

/// A table `t` of an `i64` key `id` and an `i64` column `n`.
fn schema() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64),
        ],
        &["id"],
    )
}

fn row(id: i64, n: i64) -> Vec<(&'static str, Value<'static>)> {
    vec![("id", Value::I64(id)), ("n", Value::I64(n))]
}

fn create(write: &mut WriteTransaction<'_>) {
    write
        .open_table_with("t", TableMode::CreateNew(&schema()))
        .unwrap();
}

/// The counter that a read of `store` sees.
fn counter(store: &Store) -> u64 {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    read.modification_counter(&table).unwrap()
}

/// Runs `change` on the table `t` in a write of `store`, commits it and returns what it added to the counter.
fn added(store: &Store, change: impl FnOnce(&mut WriteTransaction<'_>, &WriteTable<'_>)) -> u64 {
    let before = counter(store);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    change(&mut write, &table);
    write.commit().unwrap();
    counter(store) - before
}

/// AC-ST3 (ST4): each row operation adds what ST4 says to the counter, committed with its rows; an aborted write adds
/// nothing; the operations that ST4 does not count add nothing.
#[test]
fn ac_st3_each_row_operation_adds_what_st4_says() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        create(&mut write);
        write.commit().unwrap();
        assert_eq!(counter(store), 0, "a new table starts at zero");

        // Inserts with increasing keys take the fast path; random keys walk the tree.
        let ordered = added(store, |write, table| {
            for id in 0..100 {
                write.insert_row(table, &row(id, id)).unwrap();
            }
        });
        assert_eq!(ordered, 100);
        let random = added(store, |write, table| {
            for id in [500, 150, 320, 110, 999] {
                write.insert_row(table, &row(id, 0)).unwrap();
            }
        });
        assert_eq!(random, 5);
        // An update of the same length overwrites in place; one that changes the length rewrites the row.
        let updates = added(store, |write, table| {
            assert!(write.update_row(table, &row(1, 7)).unwrap());
            write
                .update_row(table, &[("id", Value::I64(2)), ("n", Value::Null)])
                .unwrap();
            write.upsert_row(table, &row(3, 8)).unwrap();
            write.upsert_row(table, &row(4_000, 8)).unwrap();
        });
        assert_eq!(updates, 4);
        let deletes = added(store, |write, table| {
            assert!(write.delete_row(table, &[Value::I64(10)]).unwrap());
            let removed = write
                .delete_range(table, &[], Value::I64(20)..Value::I64(30))
                .unwrap();
            assert_eq!(removed, 10);
        });
        assert_eq!(deletes, 11);
        // What ST4 does not count.
        let nothing = added(store, |write, table| {
            assert!(
                !write.update_row(table, &row(77_777, 1)).unwrap(),
                "not found"
            );
            write.update_row(table, &row(1, 7)).unwrap();
            write.update_row(table, &[("id", Value::I64(5))]).unwrap();
            assert!(!write.delete_row(table, &[Value::I64(77_777)]).unwrap());
            assert!(
                write.insert_row(table, &row(1, 0)).is_err(),
                "a duplicate key"
            );
            write.rename_column(table, "n", "m").unwrap();
            write.rename_column(table, "m", "n").unwrap();
        });
        assert_eq!(nothing, 0);
        // An aborted write leaves the counter as it was.
        let before = counter(store);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.insert_row(&table, &row(123_456, 1)).unwrap();
        assert_eq!(write.modification_counter(&table).unwrap(), before + 1);
        drop(write);
        assert_eq!(counter(store), before);

        // Emptying adds the rows it removes; a bulk load adds the rows it inserts, in order and out of order.
        let rows = {
            let read = store.read().unwrap();
            let table = read.open_table("t").unwrap();
            read.row_count(&table).unwrap()
        };
        let before = counter(store);
        let mut write = store.write().unwrap();
        write.open_table_with("t", TableMode::Empty).unwrap();
        write.commit().unwrap();
        assert_eq!(counter(store) - before, rows);
        let before = counter(store);
        let loaded = store
            .bulk_load("t", (0..50).map(|id| Ok::<_, Infallible>(row(id, id))))
            .unwrap();
        assert_eq!(loaded, 50);
        let loaded = store
            .bulk_load(
                "t",
                [90, 60, 80, 70].map(|id| Ok::<_, Infallible>(row(id, id))),
            )
            .unwrap();
        assert_eq!(loaded, 4);
        assert_eq!(counter(store) - before, 54);
        // A bulk load that fails whole adds nothing.
        let before = counter(store);
        let failed = store.bulk_load("t", [row(1_000, 1), row(1, 1)].map(Ok::<_, Infallible>));
        assert!(failed.is_err());
        assert_eq!(counter(store), before);
    });
}

/// AC-ST4 (ST4): the counter survives closing and reopening the store, and a renamed table keeps it.
#[test]
fn the_counter_survives_reopening_and_renaming() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    create(&mut write);
    let table = write.open_table("t").unwrap();
    for id in 0..30 {
        write.insert_row(&table, &row(id, id)).unwrap();
    }
    write.commit().unwrap();
    store.close().unwrap();
    let store = open(tmp.path());
    assert_eq!(counter(&store), 30);
    let mut write = store.write().unwrap();
    write.rename_table("t", "u", false).unwrap();
    write.rename_table("u", "t", false).unwrap();
    write.commit().unwrap();
    assert_eq!(counter(&store), 30);
}

/// AC-ST11 (ST13): a store of format 12, written by 7.2, is of a previous format; a new store has the current format, 16 since the vector indexes (VI14).
#[test]
fn ac_st11_a_store_of_format_12_is_of_a_previous_format() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    create(&mut write);
    write.commit().unwrap();
    store.close().unwrap();
    let data = tmp.path().join(krummi_pager::DATA_FILE);
    let mut bytes = std::fs::read(&data).unwrap();
    let [Some(a), Some(b)] = header_copies(&bytes[..2 * PAGE_SIZE]) else {
        panic!("both header copies")
    };
    assert_eq!((a.format_version, b.format_version), (16, 16));
    let current = if a.revision >= b.revision { a } else { b };
    write_header_pages(
        &mut bytes[..2 * PAGE_SIZE],
        &Header {
            format_version: 12,
            ..current
        },
    );
    std::fs::write(&data, &bytes).unwrap();
    match Store::open(tmp.path(), &krummi::StoreOptions::default()).err() {
        Some(Error::DifferentFormat {
            found, expected, ..
        }) => assert_eq!((found, expected), (12, 16)),
        other => panic!("expected a different format, got {other:?}"),
    }
    assert_eq!(std::fs::read(&data).unwrap(), bytes, "left as it was");
    let new = Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
    assert_eq!(new.info().format_version, 16);
}
