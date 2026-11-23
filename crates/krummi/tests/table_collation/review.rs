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

