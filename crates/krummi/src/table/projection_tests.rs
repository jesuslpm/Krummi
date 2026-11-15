//! Internal tests of the column projections (`docs/prd/krummi/tables/tables-projection.prd.md`) that need a malformed
//! row or a history without a version: AC-PY2's damage and AC-PY9, with the malformed rows of the row reads tests.

use super::malformed_tests::{BAD, ROWS, bad_text_store, damaged_page, panics, text_row, texts};
use super::projection::{Text, TextBytes};
use crate::table::schema::version_key;
use crate::table::value::{ColumnType, Value};
use crate::{Column, Store, StoreOptions, TableMode, TableRow};

/// AC-PY9 on the row `BAD` of [`bad_text_store`], bound to a projection of its three columns: the others give their
/// values, the text prepared as text panics and prepared as bytes gives them as they are (L2).
fn bad_text_projected(projection: &crate::Projection<'_>, row: &TableRow<'_>) {
    let bound = projection.bind(row).unwrap();
    assert_eq!(bound.get(&projection.column::<i64>(0).unwrap()), Some(BAD));
    assert_eq!(bound.get(&projection.column::<i32>(1).unwrap()), Some(50));
    let text = projection.column::<Text>(2).unwrap();
    assert!(panics(|| bound.get(&text)), "as text");
    assert_eq!(
        bound
            .get(&projection.column::<TextBytes>(2).unwrap())
            .as_deref(),
        Some(&b"row\xFF"[..])
    );
}

/// AC-PY9: a row whose text is not UTF-8, in a page whose integrity holds, read by its key and scanned, in a read and in
/// a write transaction, reads with a projection as L2 says.
#[test]
fn ac_py9_a_text_that_is_not_utf8_panics_only_when_read_as_text() {
    let store = bad_text_store();
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let projection = read.projection(&table, &["id", "n", "s"]).unwrap();
    let row = read.get_row(&table, &[Value::I64(BAD)]).unwrap().unwrap();
    bad_text_projected(&projection, &row);
    let scanned = read
        .range_rows(&table, &[], Value::I64(BAD)..)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    bad_text_projected(&projection, &scanned);
    drop(row);
    drop(scanned);
    drop(projection);
    drop(read);

    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write.insert_row(&table, &text_row(ROWS)).unwrap();
    let projection = write.projection(&table, &["id", "n", "s"]).unwrap();
    let row = write.get_row(&table, &[Value::I64(BAD)]).unwrap().unwrap();
    bad_text_projected(&projection, &row);
    drop(row);
    write.commit().unwrap();
}

/// AC-PY2, its damage: a row of a version missing from the history fails with damage in the read and in the scan step
/// that returns it, before binding it to a projection, as in AC-L3; the projection reads the other rows.
#[test]
fn ac_py2_a_row_of_a_version_missing_from_the_history_fails_before_binding() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&texts()))
        .unwrap();
    write.insert_row(&table, &text_row(BAD)).unwrap();
    write
        .add_column(&table, &Column::new("extra", ColumnType::I32))
        .unwrap();
    for id in (0..ROWS).filter(|&id| id != BAD) {
        write.insert_row(&table, &text_row(id)).unwrap();
    }
    write.commit().unwrap();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let history = write.table_trees(&table).unwrap().history;
    let update = krummi_tree::remove(&mut write.inner, history, &version_key(1)).unwrap();
    let state = write.tables.get_mut().get_mut(table.id).unwrap();
    state.history = update.root;
    state.dirty = true;
    write.commit().unwrap();

    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let projection = read.projection(&table, &["s", "extra", "id"]).unwrap();
    let history = read.table_trees(&table).unwrap().history;
    let page = krummi_tree::leaf_of(&read.inner, history, &version_key(1)).unwrap();
    assert_eq!(damaged_page(read.get_row(&table, &[Value::I64(BAD)])), page);
    let mut scan = read.range_rows(&table, &[], ..).unwrap();
    for id in 0..BAD {
        let row = scan.next().unwrap().unwrap();
        let bound = projection.bind(&row).unwrap();
        assert_eq!(bound.get(&projection.column::<i64>(2).unwrap()), Some(id));
        assert_eq!(
            bound.get(&projection.column::<Text>(0).unwrap()).as_deref(),
            Some(format!("row{id}").as_str())
        );
    }
    assert_eq!(damaged_page(scan.next().unwrap().map(drop)), page);
}
