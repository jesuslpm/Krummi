//! Damage of a table's and a sequence's own pages through the public API (tables with schema PRD 7.1, AC19, E19):
//! the operations that only read the catalog never fail with it, and those that need a damaged page fail with the
//! damage class and its page number (G6, A9); in a write, the damage is the transaction's failure (T11). A scan that
//! meets the damage partway stops there and returns nothing more from either end.
//!
//! The pages to damage are taken from the store as it is when they are damaged, after its last commit, because a
//! commit may move a page that it rewrites (copy on write). The stores are on disk, as damage changes their files.

mod support;

use std::borrow::Cow;
use std::convert::Infallible;
use std::path::Path;

use krummi::table::BulkLoadError;
use krummi::table::value::{ColumnType, Value};
use krummi::{
    Column, ErrorKind, Result, Schema, SequenceMode, Store, TableMode, TableRow, TreeKind,
    WriteTransaction,
};
use krummi_tree::catalog;
use support::*;

// ---------------------------------------------------------------------------------------------------------------
// Helpers

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

/// The row of `id`, with a name long enough that a few hundred rows take several leaves.
fn row(id: i64) -> Vec<(&'static str, Value<'static>)> {
    vec![
        ("id", Value::I64(id)),
        ("name", Value::String(Cow::Owned(format!("{id:0>200}")))),
    ]
}

/// The id of a read row.
fn id_of(row: TableRow<'_>) -> i64 {
    match row.get("id") {
        Some(Value::I64(id)) => *id,
        other => panic!("expected an id, got {other:?}"),
    }
}

/// The ids of the rows of the table: even, so that an odd id goes between two of them.
const ROWS: i64 = 600;

fn ids() -> Vec<i64> {
    (0..ROWS).map(|i| 2 * i).collect()
}

/// The page of the leaf of each row of the table `t`, in key order, in the closed store at `folder`.
fn leaves_of_rows(folder: &Path) -> Vec<u64> {
    inspect(folder, |read| {
        let root = catalog::lookup(read, "t").unwrap().unwrap().root;
        assert!(
            krummi_tree::height(read, root).unwrap() > 1,
            "several leaves"
        );
        let mut keys = Vec::new();
        krummi_tree::for_each(read, root, &mut |key, _| keys.push(key.to_vec())).unwrap();
        keys.iter()
            .map(|key| krummi_tree::leaf_of(read, root, key).unwrap())
            .collect()
    })
}

/// Asserts that `outcome` failed with damage of `page`.
#[track_caller]
fn fails_at<T: std::fmt::Debug>(outcome: Result<T>, page: u64) {
    assert_eq!(damaged_page(outcome.unwrap_err()), page);
}

/// Runs `op` in a new write, which must fail with damage of `page`; then committing fails with damage too (T11).
#[track_caller]
fn write_fails_at<T: std::fmt::Debug>(
    store: &Store,
    page: u64,
    op: impl FnOnce(&mut WriteTransaction<'_>) -> Result<T>,
) {
    let mut write = store.write().unwrap();
    fails_at(op(&mut write), page);
    assert_eq!(
        write.commit().unwrap_err().kind(),
        ErrorKind::Damaged,
        "T11"
    );
}

/// A store at `folder` with the table `t` of the rows of [`ids`] and the sequence `s` that handed out 7, committed.
fn table_and_sequence(folder: &Path) {
    let store = open(folder);
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema()))
        .unwrap();
    for id in ids() {
        write.insert_row(&table, &row(id)).unwrap();
    }
    let sequence = write
        .open_sequence_with("s", SequenceMode::CreateNew(Some(7)))
        .unwrap();
    assert_eq!(write.next_value(&sequence).unwrap(), 7);
    write.commit().unwrap();
    store.close().unwrap();
}

// ---------------------------------------------------------------------------------------------------------------
// AC19: a table and a sequence with a page of their own damaged

/// AC19 (E19, A9): with a leaf of a table's rows in the middle of its tree and the page of a sequence damaged, Open,
/// Create new, Open or create, Exists, Describe, List, Rename without overwrite or to its own name and the row count
/// do not fail with damage, in a read and in a write that then commits.
#[test]
fn ac19_catalog_operations_never_fail_with_the_damage() {
    let tmp = tempfile::tempdir().unwrap();
    table_and_sequence(tmp.path());
    let leaves = leaves_of_rows(tmp.path());
    let leaf = leaves[leaves.len() / 2];
    let sequence = root_page(tmp.path(), "s");
    damage(tmp.path(), leaf);
    damage(tmp.path(), sequence);

    let store = open(tmp.path());
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    assert_eq!(read.row_count(&table).unwrap(), ROWS as u64);
    assert_eq!(read.describe_table("t").unwrap().version(), 1);
    read.open_sequence("s").unwrap();
    assert!(read.tree_exists("t").unwrap() && read.tree_exists("s").unwrap());
    assert_eq!(read.tree_kind("t").unwrap(), Some(TreeKind::Table));
    assert_eq!(read.tree_kind("s").unwrap(), Some(TreeKind::Sequence));
    let listed = [
        ("s".to_owned(), TreeKind::Sequence),
        ("t".to_owned(), TreeKind::Table),
    ];
    assert_eq!(read.list_trees().unwrap(), listed);
    drop(read);

    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    assert_eq!(write.row_count(&table).unwrap(), ROWS as u64);
    let error = write
        .open_table_with("t", TableMode::CreateNew(&schema()))
        .err()
        .unwrap();
    assert_eq!(error.kind(), ErrorKind::TreeAlreadyExists);
    let table = write
        .open_table_with("t", TableMode::OpenOrCreate(&schema()))
        .unwrap();
    assert_eq!(write.row_count(&table).unwrap(), ROWS as u64);
    write.open_sequence("s").unwrap();
    let error = write
        .open_sequence_with("s", SequenceMode::CreateNew(None))
        .err()
        .unwrap();
    assert_eq!(error.kind(), ErrorKind::TreeAlreadyExists);
    write
        .open_sequence_with("s", SequenceMode::OpenOrCreate(Some(1)))
        .unwrap();
    write.rename_table("t", "t", false).unwrap();
    write.rename_table("t", "u", false).unwrap();
    write.rename_sequence("s", "s", false).unwrap();
    write.rename_sequence("s", "v", false).unwrap();
    assert_eq!(write.describe_table("u").unwrap().version(), 1);
    assert_eq!(
        write.row_count(&table).unwrap(),
        ROWS as u64,
        "the handle follows"
    );
    assert!(write.tree_exists("u").unwrap() && write.tree_exists("v").unwrap());
    write.commit().unwrap();

    let read = store.read().unwrap();
    assert_eq!(
        read.list_trees().unwrap(),
        [
            ("u".to_owned(), TreeKind::Table),
            ("v".to_owned(), TreeKind::Sequence)
        ]
    );
    let table = read.open_table("u").unwrap();
    assert_eq!(read.row_count(&table).unwrap(), ROWS as u64);
    drop(read);
    store.close().unwrap();
}

/// AC19 (E19, G6, T11): with a leaf of a table's rows in the middle of its tree and the page of a sequence damaged,
/// reading, saving, deleting and loading a row of that leaf, and asking the sequence for a value, fail with the damage
/// and its page, and the write cannot commit; the rows of other leaves still read. A scan forward or backward, in a
/// read and in a write, returns the rows before the leaf from its end, then fails with the leaf's page, and then
/// returns nothing more from either end.
#[test]
fn ac19_what_needs_a_damaged_page_fails_with_it() {
    let tmp = tempfile::tempdir().unwrap();
    table_and_sequence(tmp.path());
    let leaves = leaves_of_rows(tmp.path());
    let leaf = leaves[leaves.len() / 2];
    let sequence = root_page(tmp.path(), "s");
    assert!(
        leaves[0] != leaf && leaves[leaves.len() - 1] != leaf,
        "a leaf in the middle"
    );
    damage(tmp.path(), leaf);
    damage(tmp.path(), sequence);
    let ids = ids();
    // Two rows of the damaged leaf, the second right after the first, and an id that goes between them.
    let at = leaves.iter().position(|&page| page == leaf).unwrap();
    assert_eq!(leaves[at + 1], leaf);
    let (inside, between) = (ids[at], ids[at] + 1);
    let (first, last) = (ids[0], ids[ids.len() - 1]);

    let store = open(tmp.path());
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    fails_at(
        read.get_row(&table, &[Value::I64(inside)]).map(|_| ()),
        leaf,
    );
    fails_at(
        read.get_row(&table, &[Value::I64(between)]).map(|_| ()),
        leaf,
    );
    assert!(
        read.get_row(&table, &[Value::I64(first)])
            .unwrap()
            .is_some()
    );
    assert!(read.get_row(&table, &[Value::I64(last)]).unwrap().is_some());
    let around_first = collect(
        read.range_rows(&table, &[], Value::I64(first)..=Value::I64(ids[1]))
            .unwrap(),
        id_of,
    );
    assert_eq!(
        around_first,
        [first, ids[1]],
        "a range that does not need the leaf"
    );
    for front in [true, false] {
        let scan = read.range_rows(&table, &[], ..).unwrap();
        scan_fails_at(scan, &ids, id_of, front, leaf);
    }
    drop(read);

    // Reading, scanning, saving and deleting in a write, each in its own write, which then cannot commit.
    write_fails_at(&store, leaf, |w| {
        let table = w.open_table("t").unwrap();
        w.get_row(&table, &[Value::I64(inside)]).map(|_| ())
    });
    for front in [true, false] {
        let write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let scan = write.range_rows(&table, &[], ..).unwrap();
        scan_fails_at(scan, &ids, id_of, front, leaf);
        assert_eq!(
            write.commit().unwrap_err().kind(),
            ErrorKind::Damaged,
            "T11"
        );
    }
    type Op = fn(&mut WriteTransaction<'_>, i64) -> Result<()>;
    let ops: [(&str, i64, Op); 5] = [
        ("insert", between, |w, id| {
            let table = w.open_table("t").unwrap();
            w.insert_row(&table, &row(id))
        }),
        ("update", inside, |w, id| {
            let table = w.open_table("t").unwrap();
            w.update_row(&table, &row(id)).map(|_| ())
        }),
        ("upsert", between, |w, id| {
            let table = w.open_table("t").unwrap();
            w.upsert_row(&table, &row(id)).map(|_| ())
        }),
        ("delete", inside, |w, id| {
            let table = w.open_table("t").unwrap();
            w.delete_row(&table, &[Value::I64(id)]).map(|_| ())
        }),
        ("delete range", inside, |w, id| {
            let table = w.open_table("t").unwrap();
            w.delete_range(&table, &[], Value::I64(id)..=Value::I64(id + 1))
                .map(|_| ())
        }),
    ];
    for (label, id, op) in ops {
        let mut write = store.write().unwrap();
        let error = op(&mut write, id).unwrap_err();
        assert_eq!(damaged_page(error), leaf, "{label}");
        assert_eq!(
            write.commit().unwrap_err().kind(),
            ErrorKind::Damaged,
            "{label}: T11"
        );
    }
    // A load that needs the leaf fails whole with its damage.
    let source = [Ok::<_, Infallible>(row(last + 2)), Ok(row(between))];
    match store.bulk_load("t", source).unwrap_err() {
        BulkLoadError::Store(error) => assert_eq!(damaged_page(error), leaf),
        other => panic!("expected the damage, got {other:?}"),
    }
    // Asking the sequence for a value.
    write_fails_at(&store, sequence, |w| {
        let s = w.open_sequence("s").unwrap();
        w.next_value(&s)
    });

    // Nothing changed: the row count, the rows outside the leaf, and the damage still there.
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    assert_eq!(read.row_count(&table).unwrap(), ROWS as u64);
    assert!(
        read.get_row(&table, &[Value::I64(last + 2)])
            .unwrap()
            .is_none()
    );
    fails_at(
        read.get_row(&table, &[Value::I64(inside)]).map(|_| ()),
        leaf,
    );
    drop(read);
    store.close().unwrap();
}

/// AC19 (E19, G6, D3): with the page of a table's schema history damaged, the catalog operations still work and a row
/// of the current version still reads, but a row of an older version, which needs that page to be read, fails with
/// the damage and its page, by its key and in a scan, which then returns nothing more from either end.
#[test]
fn ac19_an_older_row_needs_the_schema_history() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema()))
        .unwrap();
    // The rows 100 and 101 at version 1, then the rows 0 to 9 at version 2.
    for id in [100, 101] {
        write.insert_row(&table, &row(id)).unwrap();
    }
    write
        .add_column(&table, &Column::new("extra", ColumnType::I32))
        .unwrap();
    for id in 0..10 {
        write.insert_row(&table, &row(id)).unwrap();
    }
    write.commit().unwrap();
    // The history's page as the store has it after its last commit.
    let read = store.read().unwrap();
    let history = read
        .table_trees(&read.open_table("t").unwrap())
        .unwrap()
        .history;
    drop(read);
    store.close().unwrap();
    damage(tmp.path(), history);

    let store = open(tmp.path());
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    assert_eq!(read.describe_table("t").unwrap().version(), 2);
    assert_eq!(read.row_count(&table).unwrap(), 12);
    assert_eq!(
        read.list_trees().unwrap(),
        [("t".to_owned(), TreeKind::Table)]
    );
    assert!(read.get_row(&table, &[Value::I64(3)]).unwrap().is_some());
    fails_at(
        read.get_row(&table, &[Value::I64(100)]).map(|_| ()),
        history,
    );
    let ids: Vec<i64> = (0..10).chain([100, 101]).collect();
    for front in [true, false] {
        let scan = read.range_rows(&table, &[], ..).unwrap();
        scan_fails_at(scan, &ids, id_of, front, history);
    }
    drop(read);
    write_fails_at(&store, history, |w| {
        let table = w.open_table("t").unwrap();
        w.get_row(&table, &[Value::I64(101)]).map(|_| ())
    });
    store.close().unwrap();
}
