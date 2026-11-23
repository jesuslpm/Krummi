//! Unique indexes over `ci` and `ci_ai` columns and the texts of their duplicate keys (AC-IC4), and the original texts
//! that index entries return (AC-IC6).

use std::borrow::Cow;

use krummi::table::value::{ColumnType, Value};
use krummi::{
    BulkLoadError, Collation, Column, Error, IndexDefinition, IndexKey, IndexMode, Schema, Store,
};

use super::{create, text};
use crate::support::*;

fn s(text: &str) -> Value<'_> {
    Value::String(Cow::Borrowed(text))
}

/// The primary key text, index and index key text of a duplicate key of an index.
#[track_caller]
fn duplicate<T>(result: Result<T, Error>) -> (String, String, String) {
    match result.err() {
        Some(Error::DuplicateKey {
            key,
            index: Some(index),
            index_key: Some(index_key),
            ..
        }) => (key, index.into(), index_key.into()),
        other => panic!("expected a duplicate key of an index, got {other:?}"),
    }
}

fn owned(a: &str, b: &str, c: &str) -> (String, String, String) {
    (a.to_owned(), b.to_owned(), c.to_owned())
}

/// `t`: a `ci` key `id`, a `ci` text `a` and a `ci_ai` text `b`, each with its unique index `ix_a` and `ix_b`.
fn uniques(write: &mut krummi::WriteTransaction<'_>) {
    let schema = Schema::new(
        vec![
            text("id").not_null().with_collation(Collation::Ci),
            text("a").with_collation(Collation::Ci),
            text("b").with_collation(Collation::CiAi),
        ],
        &["id"],
    );
    let table = create(write, "t", &schema);
    for (name, column) in [("ix_a", "a"), ("ix_b", "b")] {
        let definition = IndexDefinition::new(name, vec![IndexKey::new(column)]).unique();
        (write.create_index(&table, &definition, IndexMode::CreateNew)).unwrap();
    }
}

fn rows(store: &Store) -> usize {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    read.range_rows(&table, &[], ..).unwrap().count()
}

#[test]
fn ac_ic4_unique_indexes_compare_equivalent_values() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        uniques(&mut write);
        let table = write.open_table("t").unwrap();
        for row in [
            [("id", s("Ana")), ("a", s("x1")), ("b", s("y1"))],
            [("id", s("Bob")), ("a", s("x2")), ("b", s("y2"))],
        ] {
            write.insert_row(&table, &row).unwrap();
        }
        write.commit().unwrap();

        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        assert_eq!(
            duplicate(write.insert_row(&table, &[("id", s("c")), ("a", s("X1"))])),
            owned("(\"c\")", "ix_a", "(\"X1\")")
        );
        assert_eq!(
            duplicate(write.insert_row(&table, &[("id", s("c")), ("b", s("Ý1"))])),
            owned("(\"c\")", "ix_b", "(\"Ý1\")")
        );
        // An update through an equivalent key reports the key the row keeps (IC4, IC6).
        assert_eq!(
            duplicate(write.update_row(&table, &[("id", s("BOB")), ("a", s("X1"))])),
            owned("(\"Bob\")", "ix_a", "(\"X1\")")
        );
        // Nulls never collide; a row does not collide with itself.
        write.insert_row(&table, &[("id", s("n1"))]).unwrap();
        write.insert_row(&table, &[("id", s("n2"))]).unwrap();
        assert!(
            write
                .update_row(&table, &[("id", s("ana")), ("a", s("X1"))])
                .unwrap()
        );
        write.commit().unwrap();
        assert_eq!(rows(store), 4, "the failed writes changed nothing");
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let entry = read
            .index_entries(&table, "ix_a", &[s("x1")], ..)
            .unwrap()
            .next();
        assert_eq!(entry.unwrap().unwrap().get("a"), Some(s("X1")));
    });
}

#[test]
fn ac_ic4_loads_and_builds_report_equivalent_values() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        uniques(&mut write);
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                text("n").with_collation(Collation::Ci),
            ],
            &["id"],
        );
        let table = create(&mut write, "u", &schema);
        write
            .insert_row(&table, &[("id", Value::I64(1)), ("n", s("Ana"))])
            .unwrap();
        write
            .insert_row(&table, &[("id", Value::I64(2)), ("n", s("ANA"))])
            .unwrap();
        // The second entry in the index's order (IX10, IX11), with its stored text.
        let unique = IndexDefinition::new("ix_n", vec![IndexKey::new("n")]).unique();
        assert_eq!(
            duplicate(write.create_index(&table, &unique, IndexMode::CreateNew)),
            owned("(2)", "ix_n", "(\"ANA\")")
        );
        write.commit().unwrap();
        let load = [
            vec![("id", s("1")), ("a", s("Bea"))],
            vec![("id", s("2")), ("a", s("BEA"))],
        ];
        match store.bulk_load::<_, ()>("t", load.iter().map(Ok)) {
            Err(BulkLoadError::Store(error)) => assert_eq!(
                duplicate::<()>(Err(error)),
                owned("(\"2\")", "ix_a", "(\"BEA\")")
            ),
            other => panic!("expected a duplicate key, got {other:?}"),
        }
        assert_eq!(rows(store), 0);
    });
}

#[test]
fn ac_ic6_entries_return_the_stored_texts_in_folded_order() {
    stores(|store, _| {
        let schema = Schema::new(
            vec![
                text("id").not_null().with_collation(Collation::Ci),
                text("k1").with_collation(Collation::Ci),
                text("k2").with_collation(Collation::CiAi),
                text("inc").with_collation(Collation::Ci),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &schema);
        let index = IndexDefinition::new("ix", vec![IndexKey::new("k1"), IndexKey::new("k2")])
            .with_included(&["inc"]);
        (write.create_index(&table, &index, IndexMode::CreateNew)).unwrap();
        for (id, k1, k2) in [("Ana", "b", "Ñandú"), ("Bea", "A", "Éa"), ("Cid", "C", "x")] {
            let row = [
                ("id", s(id)),
                ("k1", s(k1)),
                ("k2", s(k2)),
                ("inc", s("Inc")),
            ];
            write.insert_row(&table, &row).unwrap();
        }
        assert!(
            write
                .update_row(&table, &[("id", s("ANA")), ("k1", s("B"))])
                .unwrap()
        );
        write.commit().unwrap();

        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let entries: Vec<Vec<Value<'static>>> =
            (read.index_entries(&table, "ix", &[], ..).unwrap())
                .map(|entry| {
                    let entry = entry.unwrap();
                    ["k1", "k2", "inc", "id"]
                        .map(|c| entry.get(c).unwrap().into_owned())
                        .to_vec()
                })
                .collect();
        // Folded order a < b < c; binary order would be A < C < b.
        assert_eq!(
            entries,
            [
                [s("A"), s("Éa"), s("Inc"), s("Bea")],
                [s("B"), s("Ñandú"), s("Inc"), s("Ana")],
                [s("C"), s("x"), s("Inc"), s("Cid")],
            ]
            .map(|row| row.to_vec())
        );
    });
}
