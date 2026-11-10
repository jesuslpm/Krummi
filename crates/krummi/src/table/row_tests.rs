//! Internal tests of the tables with the engine's test hooks: the row count reads the same pages whatever the number
//! of rows (AC9, AC-N3), and each schema change writes the same pages whatever the number of rows (AC4, AC-N6).

use std::collections::BTreeSet;

use krummi_tx::LayoutEvent;

use crate::table::value::{ColumnType, Value};
use crate::{Column, Schema, Store, StoreOptions, TableMode};

/// `id i64 not null, name string`, primary key `id`.
fn schema() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String),
        ],
        &["id"],
    )
}

/// A temporary store with the table `t` of `rows` rows, committed.
fn store_with(rows: i64) -> Store {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema()))
        .unwrap();
    let name = "a name of some length to fill the pages".to_owned();
    for id in 0..rows {
        write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(id)),
                    ("name", Value::String((&name).into())),
                ],
            )
            .unwrap();
    }
    write.commit().unwrap();
    store
}

/// The pages that opening the table and asking for its row count read: in a read opened before a write that adds a
/// row, in a read after it, and in the write before and after it adds the row. Each count is checked.
fn count_reads(rows: i64) -> [u64; 4] {
    let store = store_with(rows);
    let before = store.read().unwrap();
    let mut write = store.write().unwrap();
    let start = write.inner.pages_read();
    let table = write.open_table("t").unwrap();
    assert_eq!(write.row_count(&table).unwrap(), rows as u64);
    let in_write = write.inner.pages_read() - start;
    write.insert_row(&table, &[("id", Value::I64(-1))]).unwrap();
    let start = write.inner.pages_read();
    assert_eq!(write.row_count(&table).unwrap(), rows as u64 + 1);
    let after_insert = write.inner.pages_read() - start;
    write.commit().unwrap();
    let start = before.inner.pages_read();
    let old = before.open_table("t").unwrap();
    assert_eq!(
        before.row_count(&old).unwrap(),
        rows as u64,
        "it does not see the write"
    );
    let concurrent = before.inner.pages_read() - start;
    let read = store.read().unwrap();
    let start = read.inner.pages_read();
    let table = read.open_table("t").unwrap();
    assert_eq!(read.row_count(&table).unwrap(), rows as u64 + 1);
    let after = read.inner.pages_read() - start;
    [concurrent, after, in_write, after_insert]
}

/// AC9, AC-N3 (N6, E19): asking for the row count, with the opening of the table, reads the same pages with 10 rows
/// as with 100,000, in a read that does not see a write, in a read after it and in the write; the count itself reads
/// none once the table is open.
#[test]
fn ac9_the_row_count_reads_the_same_pages_with_10_and_100000_rows() {
    let small = count_reads(10);
    let large = count_reads(100_000);
    assert_eq!(small, large);
    assert_eq!(small[3], 0, "the count of an open table reads nothing");
}

/// A change of a table's schema through its handle.
type Change = dyn Fn(&mut crate::WriteTransaction<'_>, &crate::WriteTable<'_>);

/// The pages that each schema change writes on the table `t` of `rows` rows, each in its own write: adding a column
/// that allows nulls, adding one with a default, dropping one, renaming one and allowing nulls in one. No page of the
/// rows tree is among them.
fn change_writes(rows: i64) -> Vec<usize> {
    let store = store_with(rows);
    // A column without nulls, to allow them in it.
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let strict = Column::new("z", ColumnType::I8)
        .not_null()
        .with_default(Value::I8(1));
    write.add_column(&table, &strict).unwrap();
    write.commit().unwrap();
    let changes: [&Change; 5] = [
        &|w, t| w.add_column(t, &Column::new("x", ColumnType::I32)).unwrap(),
        &|w, t| {
            w.add_column(
                t,
                &Column::new("d", ColumnType::I32)
                    .not_null()
                    .with_default(Value::I32(5)),
            )
            .unwrap()
        },
        &|w, t| w.drop_column(t, "name").unwrap(),
        &|w, t| w.rename_column(t, "name", "label").unwrap(),
        &|w, t| w.modify_column(t, "z", ColumnType::I8, true).unwrap(),
    ];
    let mut written = Vec::new();
    for change in changes {
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let rows_root = write.table_trees(&table).unwrap().rows;
        let leaves: BTreeSet<u64> = (0..rows)
            .step_by(97)
            .map(|id| {
                let key =
                    crate::table::key::encode_key(&[ColumnType::I64], &[Value::I64(id)]).unwrap();
                krummi_tree::leaf_of(&write.inner, rows_root, &key).unwrap()
            })
            .collect();
        write.inner.record_layout();
        change(&mut write, &table);
        let pages: BTreeSet<u64> = write
            .inner
            .take_layout_log()
            .into_iter()
            .filter_map(|event| match event {
                LayoutEvent::Change(page) => Some(page),
                _ => None,
            })
            .collect();
        assert!(
            !pages.contains(&rows_root) && pages.is_disjoint(&leaves),
            "no page of the rows"
        );
        assert_eq!(write.table_trees(&table).unwrap().rows, rows_root);
        written.push(pages.len());
        write.abort();
    }
    written
}

/// AC4, AC-N6 (E4, N3): each schema change writes the same pages with 10 rows as with 100,000: those of the catalog
/// and of the schema history, none of the rows.
#[test]
fn ac4_each_schema_change_writes_the_same_pages_with_10_and_100000_rows() {
    let small = change_writes(10);
    let large = change_writes(100_000);
    assert_eq!(small, large);
    assert!(small.iter().all(|&pages| pages > 0), "each change writes");
}
