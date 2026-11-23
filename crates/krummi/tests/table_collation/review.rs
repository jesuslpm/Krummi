//! Regressions of the Codex code review of the collation run (docs/reviews/tables-collation/2026-11-23-code.md): R1,
//! an estimate decides a large value by its folded form (IC7); R2, analyze never reads a large original through an
//! index prefix (IC8, ST1); R4, an index entry returns a collated key text in a copy of its own (IC5, IX8).

use std::borrow::Cow;

use krummi::table::value::{ColumnType, Value};
use krummi::{
    Collation, Column, Condition, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions,
};

use super::{create, text};

fn s(text: &str) -> Value<'_> {
    Value::String(Cow::Borrowed(text))
}

/// A temporary store with `t`: an integer key `id`, a `ci_ai` text `t` and an integer `n`, with the index `ix` on
/// `(t, n)`.
fn store_with_prefix_index() -> Store {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            text("t").with_collation(Collation::CiAi),
            Column::new("n", ColumnType::I64),
        ],
        &["id"],
    );
    let mut write = store.write().unwrap();
    let table = create(&mut write, "t", &schema);
    let index = IndexDefinition::new("ix", vec![IndexKey::new("t"), IndexKey::new("n")]);
    (write.create_index(&table, &index, IndexMode::CreateNew)).unwrap();
    write.commit().unwrap();
    store
}

#[test]
fn r1_a_long_spelling_of_a_short_folded_form_estimates_as_it() {
    let store = store_with_prefix_index();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .insert_row(&table, &[("id", Value::I64(1)), ("t", s("Ana"))])
        .unwrap();
    write.analyze(&table, Some("t")).unwrap();
    let estimate = |value| {
        let condition = Condition::Equal { column: "t", value };
        write.estimate(&table, &condition).unwrap().rows
    };
    let long = format!("a{}na", "\u{301}".repeat(1_024));
    assert!(long.len() > 2_048);
    assert_eq!((estimate(s("ana")), estimate(s(&long))), (1.0, 1.0));
}

#[test]
fn r2_analyze_never_reads_a_large_original_through_an_index_prefix() {
    let store = store_with_prefix_index();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let huge = format!("a{}", "\u{301}".repeat(100_000));
    let row = [("id", Value::I64(1)), ("t", s(&huge)), ("n", Value::I64(1))];
    write.insert_row(&table, &row).unwrap();
    write.commit().unwrap();
    let pages = |column: Option<&str>| {
        let before = store.statistics_pages_read();
        store.analyze("t", column).unwrap();
        store.statistics_pages_read() - before
    };
    let column_only = pages(Some("t"));
    assert_eq!(
        pages(None),
        column_only,
        "the prefix reads no more than the column"
    );
}

#[test]
fn r4_an_entry_returns_a_collated_key_text_in_its_own_copy() {
    let store = store_with_prefix_index();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let row = [("id", Value::I64(1)), ("t", s("Ana")), ("n", Value::I64(1))];
    write.insert_row(&table, &row).unwrap();
    write.commit().unwrap();
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let entry = read
        .index_entries(&table, "ix", &[], ..)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert!(matches!(entry.get("t"), Some(Value::String(Cow::Owned(text))) if text == "Ana"));
}

#[test]
fn r2_analyze_never_folds_a_large_default_through_an_index_prefix() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64),
        ],
        &["id"],
    );
    let mut write = store.write().unwrap();
    let table = create(&mut write, "t", &schema);
    write
        .insert_row(&table, &[("id", Value::I64(1)), ("n", Value::I64(1))])
        .unwrap();
    // A column added after the row, whose default is a `ci_ai` text over the maximum that folds to `a`.
    let huge = format!("a{}", "\u{301}".repeat(100_000));
    let added = text("t")
        .not_null()
        .with_collation(Collation::CiAi)
        .with_default(Value::String(Cow::Owned(huge)));
    write.add_column(&table, &added).unwrap();
    let index = IndexDefinition::new("ix", vec![IndexKey::new("t"), IndexKey::new("n")]);
    (write.create_index(&table, &index, IndexMode::CreateNew)).unwrap();
    write.analyze(&table, None).unwrap();
    let column = write.column_statistics(&table, "t").unwrap().unwrap();
    assert_eq!(column.large_fraction, 1.0);
    let prefixes = write.index_statistics(&table, "ix").unwrap().unwrap();
    let pair = prefixes.iter().find(|p| p.columns == 2).unwrap();
    assert!(
        pair.frequent.is_empty(),
        "the default was not read: {pair:?}"
    );
}
