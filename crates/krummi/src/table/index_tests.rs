//! Internal tests of the secondary indexes over rows whose content is malformed and of what their writes change
//! (AC-IX2, AC-IX13 and IX12 of the secondary indexes PRD): the rows are read as the reads read them, without checking
//! their content, so a row altered only in a column outside the index does not fail its creation, its delete or a
//! range delete; the damage that the reads keep, a row of a version missing from the history, fails them, and the
//! write then cannot commit (T11); an entry that a delete must remove and is not there is damage of the index page
//! where it should be; a panic of a range delete after it removed rows leaves the write unable to commit; and an
//! identical update writes no page. The malformed rows are written through the tree, as the row reads tests write them.

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use krummi_tx::LayoutEvent;

use super::index_key::encode_entry;
use super::key::KeyColumn;
use super::malformed_tests::{
    BAD, ROWS, alter, bad_text_store, damaged_page, id_key, panics, row_page, store_with, stored,
    text_row, texts,
};
use super::schema::version_key;
use crate::table::value::{ColumnType, Value};
use crate::{
    Column, Direction, ErrorKind, IndexDefinition, IndexEntry, IndexKey, IndexMode, Nulls, Store,
    StoreOptions, TableMode, WriteTransaction,
};

/// The index on `n` of [`texts`].
fn on_n() -> IndexDefinition {
    IndexDefinition::new("ix_n", vec![IndexKey::new("n")])
}

/// The index on `s` of [`texts`], the column that [`bad_text_store`] alters.
fn on_s() -> IndexDefinition {
    IndexDefinition::new("ix_s", vec![IndexKey::new("s")])
}

/// A store of [`texts`] with its rows and the index of `definition`, committed.
fn indexed_store(definition: &IndexDefinition) -> Store {
    let store = store_with(&texts(), (0..ROWS).map(text_row));
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .create_index(&table, definition, IndexMode::CreateNew)
        .unwrap();
    write.commit().unwrap();
    store
}

/// The primary keys of the entries of the index `name` of the table `t` in `write`, in the index's order.
fn entry_ids(write: &WriteTransaction<'_>, name: &str) -> Vec<Value<'static>> {
    let table = write.open_table("t").unwrap();
    (write
        .stored_index_entries(&table, name)
        .unwrap()
        .unwrap()
        .into_iter())
    .map(|entry| entry.primary_key[0].clone())
    .collect()
}

/// The root of the tree of the index `name` of the table `t` in `write`.
fn index_root(write: &WriteTransaction<'_>, name: &str) -> u64 {
    let table = write.open_table("t").unwrap();
    let trees = write.index_trees(&table).unwrap();
    trees.into_iter().find(|(n, _)| n == name).unwrap().1
}

/// The rows of `ids`, as their primary keys.
fn ids(ids: impl IntoIterator<Item = i64>) -> Vec<Value<'static>> {
    ids.into_iter().map(Value::I64).collect()
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
    let entries = write.stored_index_entries(&table, "ix_n").unwrap().unwrap();
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

/// AC-IX13 (IX12): with the row `BAD` altered only in its text, which is not in the index, deleting it and deleting
/// a range that holds it do not fail, and the index keeps the entries of the other rows.
#[test]
fn ac_ix13_a_row_malformed_outside_the_index_deletes_without_failing() {
    let store = bad_text_store();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .create_index(&table, &on_n(), IndexMode::CreateNew)
        .unwrap();
    assert!(write.delete_row(&table, &[Value::I64(BAD)]).unwrap());
    let others = (0..ROWS).filter(|&id| id != BAD);
    assert_eq!(entry_ids(&write, "ix_n"), ids(others));
    write.abort();

    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .create_index(&table, &on_n(), IndexMode::CreateNew)
        .unwrap();
    let range = Value::I64(BAD - 2)..Value::I64(BAD + 2);
    assert_eq!(write.delete_range(&table, &[], range).unwrap(), 4);
    let others = (0..ROWS).filter(|id| !(BAD - 2..BAD + 2).contains(id));
    assert_eq!(entry_ids(&write, "ix_n"), ids(others));
    write.commit().unwrap();
}

/// Rewrites the tree of the index `name` of the table `t` without the entry of the row `id`, in a write that
/// commits, and returns the entry's key.
fn remove_entry(store: &Store, name: &str, id: i64) -> Vec<u8> {
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let entries = write.stored_index_entries(&table, name).unwrap().unwrap();
    let entry = entries
        .into_iter()
        .find(|entry| entry.primary_key == [Value::I64(id)])
        .unwrap()
        .bytes;
    let at = write
        .table_schema(&table)
        .unwrap()
        .indexes()
        .iter()
        .position(|index| index.name == name)
        .unwrap();
    let root = index_root(&write, name);
    let update = krummi_tree::remove(&mut write.inner, root, &entry).unwrap();
    assert!(update.count_changed);
    let state = write.tables.get_mut().get_mut(table.id).unwrap();
    state.index_roots[at] = update.root;
    state.dirty = true;
    write.commit().unwrap();
    entry
}

/// AC-IX13 (IX12, T11): a row whose entry is missing from an index fails its delete, and a range delete that holds
/// it, with damage of the index page where the entry should be, and the write cannot commit.
#[test]
fn ac_ix13_a_missing_entry_fails_a_delete_with_damage_of_the_index_page() {
    let store = indexed_store(&on_n());
    let entry = remove_entry(&store, "ix_n", BAD);
    type Delete = fn(&mut WriteTransaction<'_>) -> crate::Result<()>;
    let deletes: [(&str, Delete); 2] = [
        ("delete", |write| {
            let table = write.open_table("t").unwrap();
            write.delete_row(&table, &[Value::I64(BAD)]).map(drop)
        }),
        ("range delete", |write| {
            let table = write.open_table("t").unwrap();
            write.delete_range(&table, &[], ..).map(drop)
        }),
    ];
    for (what, delete) in deletes {
        let mut write = store.write().unwrap();
        let root = index_root(&write, "ix_n");
        let page = krummi_tree::leaf_of(&write.inner, root, &entry).unwrap();
        assert_eq!(damaged_page(delete(&mut write)), page, "{what}");
        assert_eq!(
            write.commit().unwrap_err().kind(),
            ErrorKind::Damaged,
            "{what}: T11"
        );
    }
    let read = store.read().unwrap();
    assert_eq!(
        read.row_count(&read.open_table("t").unwrap()).unwrap(),
        ROWS as u64
    );
}

/// AC-IX13 (IX12): with the row `BAD` altered so that its key column of the index has a value with no entry, deleting
/// it fails with damage of the index page where that entry would be, and the write cannot commit.
#[test]
fn ac_ix13_an_index_column_altered_to_a_value_without_entry_fails_its_delete() {
    let store = indexed_store(&on_n());
    // The bytes of the row `BAD` with `n` 777, from a store with only that row: a row of the same version.
    let mut row = text_row(BAD);
    row[1].1 = Value::I32(777);
    let twin = store_with(&texts(), [row]);
    let altered = stored(&twin, &id_key(BAD));
    alter(&store, &id_key(BAD), |bytes| *bytes = altered);

    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    assert_eq!(
        write
            .get_row(&table, &[Value::I64(BAD)])
            .unwrap()
            .unwrap()
            .get("n"),
        Some(Value::I32(777))
    );
    // The index has 10 small entries: its root is its only leaf.
    let root = index_root(&write, "ix_n");
    let outcome = write.delete_row(&table, &[Value::I64(BAD)]);
    assert_eq!(damaged_page(outcome), root);
    assert_eq!(
        write.commit().unwrap_err().kind(),
        ErrorKind::Damaged,
        "T11"
    );
}

/// AC-IX13 (IX12, L3): updating a malformed row of a table with indexes fails with damage of its page, and the write
/// cannot commit.
#[test]
fn ac_ix13_updating_a_malformed_row_of_a_table_with_indexes_is_damage() {
    let store = bad_text_store();
    let page = row_page(&store, &id_key(BAD));
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .create_index(&table, &on_n(), IndexMode::CreateNew)
        .unwrap();
    let outcome = write.update_row(&table, &[("id", Value::I64(BAD)), ("n", Value::I32(1))]);
    assert_eq!(damaged_page(outcome), page);
    assert_eq!(
        write.commit().unwrap_err().kind(),
        ErrorKind::Damaged,
        "T11"
    );
}

/// A temporary store of [`texts`] whose row `BAD` is of version 1 and the others of version 2, with the index of
/// `index` when given, created before the version 1 leaves the table's history, committed; and the page of the
/// history's leaf that should hold version 1.
fn missing_version_store(index: Option<IndexDefinition>) -> (Store, u64) {
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
    if let Some(index) = index {
        write
            .create_index(&table, &index, IndexMode::CreateNew)
            .unwrap();
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
    let history = read
        .table_trees(&read.open_table("t").unwrap())
        .unwrap()
        .history;
    let page = krummi_tree::leaf_of(&read.inner, history, &version_key(1)).unwrap();
    drop(read);
    (store, page)
}

/// AC-IX13 (AC-L3): with the row `BAD` of a version missing from the table's history, creating an index fails with
/// damage of the history page that should have it, and the write cannot commit (T11).
#[test]
fn ac_ix13_a_version_missing_from_the_history_fails_a_creation() {
    let (store, page) = missing_version_store(None);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let outcome = write.create_index(&table, &on_n(), IndexMode::CreateNew);
    assert_eq!(damaged_page(outcome), page);
    assert!(write.table_schema(&table).unwrap().indexes().is_empty());
    assert_eq!(
        write.commit().unwrap_err().kind(),
        ErrorKind::Damaged,
        "T11"
    );
}

/// AC-IX13 (AC-L3, IX12): in a table with an index, deleting the row `BAD`, of a version missing from the table's
/// history, fails with damage of the history page that should have it, and the write cannot commit (T11).
#[test]
fn ac_ix13_a_version_missing_from_the_history_fails_a_delete() {
    let (store, page) = missing_version_store(Some(on_n()));
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let outcome = write.delete_row(&table, &[Value::I64(BAD)]);
    assert_eq!(damaged_page(outcome), page);
    assert_eq!(write.row_count(&table).unwrap(), ROWS as u64);
    assert_eq!(
        write.commit().unwrap_err().kind(),
        ErrorKind::Damaged,
        "T11"
    );
}

/// A store of [`texts`] with the index on `s`, whose row `BAD` then has a text that is not UTF-8, which reading as a
/// value panics (L2).
fn bad_text_in_index() -> Store {
    let store = indexed_store(&on_s());
    alter(&store, &id_key(BAD), |bytes| {
        *bytes.last_mut().unwrap() = 0xFF
    });
    store
}

/// AC-IX13 (IX12; L2): a panic while deleting a single row of a table with indexes, here reading the malformed text of
/// its index column, comes before any change, and the write commits its other changes.
#[test]
fn ac_ix13_a_panic_while_deleting_a_row_comes_before_any_change() {
    let store = bad_text_in_index();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let delete = catch_unwind(AssertUnwindSafe(|| {
        write.delete_row(&table, &[Value::I64(BAD)])
    }));
    assert!(delete.is_err(), "the read of the text panics");
    assert!(!write.inner.has_failed());
    assert_eq!(write.row_count(&table).unwrap(), ROWS as u64);
    assert_eq!(entry_ids(&write, "ix_s"), ids(0..ROWS));
    assert!(write.delete_row(&table, &[Value::I64(0)]).unwrap());
    write.commit().unwrap();
    let read = store.read().unwrap();
    assert_eq!(
        read.row_count(&read.open_table("t").unwrap()).unwrap(),
        ROWS as u64 - 1
    );
}

/// AC-IX13 (IX12, T11; the user's exception to L2): a panic captured while deleting a range of a table with indexes,
/// after the range delete removed its first rows, leaves the write unable to commit, and nothing of it is applied; a
/// panic at the range's first rows comes before any change, and the write commits.
#[test]
fn ac_ix13_a_panic_while_deleting_a_range_leaves_the_write_unable_to_commit() {
    let store = bad_text_in_index();
    // The range delete reads 4 rows at a time in these tests: the first rows go before it meets `BAD`.
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let delete = catch_unwind(AssertUnwindSafe(|| write.delete_range(&table, &[], ..)));
    assert!(delete.is_err(), "the read of the text panics");
    assert!(write.inner.has_failed());
    assert_eq!(write.commit().unwrap_err().kind(), ErrorKind::Damaged);
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    assert_eq!(
        read.row_count(&table).unwrap(),
        ROWS as u64,
        "nothing applied"
    );
    assert_eq!(
        read.stored_index_entries(&table, "ix_s")
            .unwrap()
            .unwrap()
            .len(),
        ROWS as usize
    );
    drop(read);

    // From `BAD` on, the panic comes in the first batch, before any change.
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let from_bad = catch_unwind(AssertUnwindSafe(|| {
        write.delete_range(&table, &[], Value::I64(BAD)..)
    }));
    assert!(from_bad.is_err());
    assert!(!write.inner.has_failed());
    assert_eq!(write.row_count(&table).unwrap(), ROWS as u64);
    assert_eq!(
        write.delete_range(&table, &[], ..Value::I64(BAD)).unwrap(),
        5
    );
    write.commit().unwrap();
    let read = store.read().unwrap();
    assert_eq!(
        read.row_count(&read.open_table("t").unwrap()).unwrap(),
        ROWS as u64 - 5
    );
}

/// The pages that `change` changes in a write of `store`, which then aborts, through the test hook of the
/// transaction's page changes.
fn changed_pages(store: &Store, change: impl FnOnce(&mut WriteTransaction<'_>)) -> BTreeSet<u64> {
    let mut write = store.write().unwrap();
    write.inner.record_layout();
    change(&mut write);
    let pages = (write.inner.take_layout_log().into_iter())
        .filter_map(|event| match event {
            LayoutEvent::Change(page) => Some(page),
            _ => None,
        })
        .collect();
    write.abort();
    pages
}

/// AC-IX2 (E10, E16): an identical update writes no page, of the rows or of the indexes; an update of a column in no
/// index writes no page of the index; and one of a key column of the index writes its page.
#[test]
fn ac_ix2_an_identical_update_writes_no_page_of_the_indexes() {
    let store = indexed_store(&on_n());
    let root = {
        let write = store.write().unwrap();
        index_root(&write, "ix_n")
    };
    let update = |row: Vec<(&'static str, Value<'static>)>| {
        move |write: &mut WriteTransaction<'_>| {
            let table = write.open_table("t").unwrap();
            assert!(write.update_row(&table, &row).unwrap());
        }
    };
    // `n` of row 3 is 30 already, and the whole row repeats its values.
    let identical = changed_pages(
        &store,
        update(vec![("id", Value::I64(3)), ("n", Value::I32(30))]),
    );
    assert_eq!(identical, BTreeSet::new(), "an identical update");
    let identical = changed_pages(&store, update(text_row(3)));
    assert_eq!(
        identical,
        BTreeSet::new(),
        "an identical update of every column"
    );
    // The index has 10 small entries: its root is its only leaf.
    let text = changed_pages(
        &store,
        update(vec![
            ("id", Value::I64(3)),
            ("s", Value::String("other".into())),
        ]),
    );
    assert!(
        !text.is_empty() && !text.contains(&root),
        "a column in no index"
    );
    let key = changed_pages(
        &store,
        update(vec![("id", Value::I64(3)), ("n", Value::I32(31))]),
    );
    assert!(key.contains(&root), "a key column of the index");
}

/// Adds the entry `key`, `value` to the tree of the index `name` of the table `t`, in a write that commits: an entry
/// that no row write made, written through the tree, so its page's integrity holds.
fn add_entry(store: &Store, name: &str, key: &[u8], value: &[u8]) {
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let at = (write.table_schema(&table).unwrap().indexes().iter())
        .position(|index| index.name == name)
        .unwrap();
    let root = index_root(&write, name);
    let update = krummi_tree::try_add(&mut write.inner, root, key, value).unwrap();
    assert!(update.count_changed);
    let state = write.tables.get_mut().get_mut(table.id).unwrap();
    state.index_roots[at] = update.root;
    state.dirty = true;
    write.commit().unwrap();
}

/// The key of the entry of `ix_n` for `n` and the primary key `id`.
fn n_entry(n: i32, id: i64) -> Vec<u8> {
    let column = KeyColumn {
        ty: ColumnType::I32,
        direction: Direction::Ascending,
        nulls: Nulls::First,
        nullable: true,
        collation: crate::table::collation::Collation::Binary,
    };
    encode_entry(&[column], &[Value::I32(n)], &id_key(id)).unwrap()
}

/// AC-IX13 (IX12, L2): malformed entries in a page whose integrity holds come from a scan without error, the other
/// entries with their values; reading a value of a malformed entry panics: a key that is not an index element, and
/// included columns that are not a row.
#[test]
fn ac_ix13_a_malformed_entry_is_returned_and_reading_it_panics() {
    let with_s = on_n().with_included(&["s"]);
    let store = indexed_store(&with_s);
    // A key that no element of `n` starts with sorts after every entry; then a well-formed key with a value that
    // is not a row of the included columns, after the entry of `n` 50.
    add_entry(&store, "ix_n", &[0xFF, 0xFF, 0xFF], b"");
    add_entry(&store, "ix_n", &n_entry(50, 900), &[0xEE; 3]);
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let entries: Vec<IndexEntry<'_>> = (read.index_entries(&table, "ix_n", &[], ..).unwrap())
        .map(|entry| entry.unwrap())
        .collect();
    assert_eq!(entries.len(), ROWS as usize + 2);
    let (bad_key, rest) = entries.split_last().unwrap();
    assert!(panics(|| bad_key.value(0)));
    assert!(panics(|| bad_key.get("id")));
    let bad_value = &rest[6];
    assert_eq!(bad_value.get("id"), Some(Value::I64(900)));
    assert_eq!(bad_value.get("n"), Some(Value::I32(50)));
    assert!(panics(|| bad_value.get("s")));
    let good: Vec<(Value<'_>, Value<'_>, Value<'_>)> = (rest.iter())
        .filter(|entry| entry.get("id") != Some(Value::I64(900)))
        .map(|entry| {
            (
                entry.value(0).unwrap(),
                entry.value(1).unwrap(),
                entry.value(2).unwrap(),
            )
        })
        .collect();
    let expected: Vec<(Value<'_>, Value<'_>, Value<'_>)> = (0..ROWS)
        .map(|id| {
            let row = text_row(id);
            (row[1].1.clone(), row[2].1.clone(), Value::I64(id))
        })
        .collect();
    assert_eq!(good, expected);
    // Backward, the malformed key comes first.
    let mut back = read.index_entries(&table, "ix_n", &[], ..).unwrap().rev();
    assert!(panics(|| back.next().unwrap().unwrap().value(0)));
    assert_eq!(back.count(), ROWS as usize + 1);
}

/// AC-IX13 (IX12, T11): with rows requested, an entry whose row is not in the table fails with damage of the index
/// page that holds it, a key that does not split as such an entry too, from either end, and then the scan gives
/// nothing more; an interval without them scans; in a write, the failure is the transaction's.
#[test]
fn ac_ix13_an_entry_whose_row_is_missing_fails_a_row_scan_with_damage() {
    for (what, entry) in [
        ("missing row", n_entry(50, 900)),
        ("malformed key", vec![0xFF, 0xFF]),
    ] {
        let store = indexed_store(&on_n());
        add_entry(&store, "ix_n", &entry, b"");
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let root = read.index_trees(&table).unwrap()[0].1;
        let page = krummi_tree::leaf_of(&read.inner, root, &entry).unwrap();
        let mut rows = read.index_rows(&table, "ix_n", &[], ..).unwrap();
        let before: Vec<_> = rows.by_ref().take_while(Result::is_ok).collect();
        assert!(before.len() < ROWS as usize + 1, "{what}");
        assert!(rows.next().is_none(), "{what}: nothing after a failure");
        let mut rows = read.index_rows(&table, "ix_n", &[], ..).unwrap();
        let failed = rows.find(Result::is_err).unwrap();
        assert_eq!(damaged_page(failed), page, "{what}");
        let back = read
            .index_rows(&table, "ix_n", &[], ..)
            .unwrap()
            .rev()
            .find(Result::is_err);
        assert_eq!(damaged_page(back.unwrap()), page, "{what}");
        let mut lent = read.index_rows(&table, "ix_n", &[], ..).unwrap();
        let failed = loop {
            match lent.next_row() {
                Ok(Some(_)) => {}
                outcome => break outcome.map(|_| ()),
            }
        };
        assert_eq!(damaged_page(failed), page, "{what}: lent");
        assert_eq!(lent.next_row().unwrap().map(drop), None);
        // The entries scan, and an interval of the other rows scans their rows.
        assert_eq!(
            read.index_entries(&table, "ix_n", &[], ..).unwrap().count(),
            ROWS as usize + 1
        );
        let below = read
            .index_rows(&table, "ix_n", &[], ..Value::I32(50))
            .unwrap();
        assert_eq!(below.map(Result::unwrap).count(), 5, "{what}");
        drop(read);

        let write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let failed = write
            .index_rows(&table, "ix_n", &[], ..)
            .unwrap()
            .find(Result::is_err);
        assert_eq!(damaged_page(failed.unwrap()), page, "{what}");
        assert_eq!(
            write.commit().unwrap_err().kind(),
            ErrorKind::Damaged,
            "{what}: T11"
        );
    }
}

/// R3 of the code review of the collation run: a `ci` primary key column that is also the key of an index keeps its
/// original text once in the entry, though it is two elements of the entry's key (IC5, IC7 of the collation PRD).
#[test]
fn a_collated_column_keeps_its_original_text_once() {
    use super::collation::Collation;
    use super::index::EntryLayout;
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let schema = crate::Schema::new(
        vec![
            Column::new("id", ColumnType::String)
                .not_null()
                .with_collation(Collation::CiAi),
            Column::new("n", ColumnType::I64),
        ],
        &["id"],
    );
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema))
        .unwrap();
    let down = IndexDefinition::new("down", vec![IndexKey::new("id").descending()]);
    write
        .create_index(&table, &down, IndexMode::CreateNew)
        .unwrap();
    let described = write.describe_table("t").unwrap();
    let layout = EntryLayout::new(&described, &described.indexes()[0]);
    assert_eq!(layout.original_places, [0], "one slot for the column");
    write.abort();
}
