//! Internal tests of creating an index over rows whose content is malformed (AC-IX13 and IX12 of the secondary indexes
//! PRD, the part of creating an index): the rows are read as the reads read them, without checking their content, so a
//! row altered only in a column outside the index does not fail; and the damage that the reads keep, a row of a
//! version missing from the history, fails the creation, which the write then cannot commit (T11). The malformed rows
//! are written through the tree, as the row reads tests write them.

use super::malformed_tests::{BAD, ROWS, bad_text_store, damaged_page, text_row, texts};
use super::schema::version_key;
use crate::table::value::{ColumnType, Value};
use crate::{
    Column, ErrorKind, IndexDefinition, IndexKey, IndexMode, Store, StoreOptions, TableMode,
};

/// The index on `n` of [`texts`].
fn on_n() -> IndexDefinition {
    IndexDefinition::new("ix_n", vec![IndexKey::new("n")])
}

/// AC-IX13: with the row `BAD` altered only in its text, which is not in the index, creating an index on `n` does not
/// fail, and the index has the entries of every row.
#[test]
fn ac_ix13_a_row_malformed_outside_the_index_does_not_fail_its_creation() {
    let store = bad_text_store();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .create_index(&table, &on_n(), IndexMode::CreateNew)
        .unwrap();
    let entries = write.index_entries(&table, "ix_n").unwrap().unwrap();
    let found: Vec<(Value<'static>, Value<'static>)> = entries
        .into_iter()
        .map(|entry| (entry.key[0].clone(), entry.primary_key[0].clone()))
        .collect();
    let expected: Vec<(Value<'static>, Value<'static>)> = (0..ROWS)
        .map(|id| (Value::I32(i32::try_from(id).unwrap() * 10), Value::I64(id)))
        .collect();
    assert_eq!(found, expected);
    write.commit().unwrap();
}

/// AC-IX13 (AC-L3): with the row `BAD` of a version missing from the table's history, creating an index fails with
/// damage of the history page that should have it, and the write cannot commit (T11).
#[test]
fn ac_ix13_a_version_missing_from_the_history_fails_a_creation() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&texts()))
        .unwrap();
    // `BAD` at version 1, the others at version 2.
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

    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let history = write.table_trees(&table).unwrap().history;
    let page = krummi_tree::leaf_of(&write.inner, history, &version_key(1)).unwrap();
    let outcome = write.create_index(&table, &on_n(), IndexMode::CreateNew);
    assert_eq!(damaged_page(outcome), page);
    assert!(write.table_schema(&table).unwrap().indexes().is_empty());
    assert_eq!(
        write.commit().unwrap_err().kind(),
        ErrorKind::Damaged,
        "T11"
    );
}

/// Until the indexes follow the row writes (T3 of the secondary indexes execution), a row write on a table with an
/// index panics before it changes anything, instead of leaving the index out of step with the rows.
#[test]
fn a_row_write_on_a_table_with_indexes_panics_before_changing_anything() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let store = super::malformed_tests::store_with(&texts(), (0..ROWS).map(text_row));
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .create_index(&table, &on_n(), IndexMode::CreateNew)
        .unwrap();
    let writes: [&dyn Fn(&mut crate::WriteTransaction<'_>); 3] = [
        &|write| drop(write.insert_row(&table, &text_row(ROWS))),
        &|write| drop(write.delete_row(&table, &[Value::I64(0)])),
        &|write| drop(write.delete_range(&table, &[], ..)),
    ];
    for row_write in writes {
        assert!(catch_unwind(AssertUnwindSafe(|| row_write(&mut write))).is_err());
    }
    assert_eq!(write.row_count(&table).unwrap(), ROWS as u64);
    assert_eq!(
        write.index_entries(&table, "ix_n").unwrap().unwrap().len(),
        ROWS as usize
    );
    write.commit().unwrap();
}
