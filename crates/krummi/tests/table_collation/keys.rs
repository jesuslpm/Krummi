//! The primary key of a `ci` or `ci_ai` column (AC-IC3) and the size of its folded keys and bounds (AC-IC7).

use std::borrow::Cow;
use std::ops::Bound::{Excluded, Included, Unbounded};

use krummi::table::projection::Text;
use krummi::table::value::{ColumnType, Value};
use krummi::{BulkLoadError, Collation, Column, Error, Schema, SizeSubject, Store, Upserted};

use super::{create, text};
use crate::support::*;

fn s(text: &str) -> Value<'_> {
    Value::String(Cow::Borrowed(text))
}

/// A table whose key `id` is a text of `collation`, with an `n` integer and a `ci` text `note` outside the key.
fn keyed(collation: Collation) -> Schema {
    Schema::new(
        vec![
            text("id").not_null().with_collation(collation),
            Column::new("n", ColumnType::I64),
            text("note").with_collation(Collation::Ci),
        ],
        &["id"],
    )
}

/// The key text of a duplicate key.
#[track_caller]
fn duplicate<T>(result: Result<T, Error>) -> String {
    match result.err() {
        Some(Error::DuplicateKey {
            key, index: None, ..
        }) => key,
        other => panic!("expected a duplicate primary key, got {other:?}"),
    }
}

/// The size and subject of an invalid size.
#[track_caller]
fn too_large<T>(result: Result<T, Error>) -> (SizeSubject, u64) {
    match result.err() {
        Some(Error::InvalidSize { what, received, .. }) => (what, received),
        other => panic!("expected an invalid size, got {other:?}"),
    }
}

/// The text of the key and `n` of the row of key `key` in `t`.
fn read(store: &Store, key: &str) -> Option<(String, Option<i64>)> {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let row = read.get_row(&table, &[s(key)]).unwrap()?;
    Some((row.text(0).unwrap().into_owned(), row.i64(1)))
}

#[test]
fn ac_ic3_equivalent_keys_are_the_same_row() {
    for collation in [Collation::Ci, Collation::CiAi] {
        stores(|store, _| {
            let mut write = store.write().unwrap();
            let table = create(&mut write, "t", &keyed(collation));
            write
                .insert_row(&table, &[("id", s("Ana")), ("n", Value::I64(1))])
                .unwrap();
            assert_eq!(
                duplicate(write.insert_row(&table, &[("id", s("ANA"))])),
                "(\"ANA\")"
            );
            if collation == Collation::CiAi {
                assert_eq!(
                    duplicate(write.insert_row(&table, &[("id", s("Àna"))])),
                    "(\"Àna\")"
                );
            } else {
                write.insert_row(&table, &[("id", s("Àna"))]).unwrap();
                assert!(write.delete_row(&table, &[s("àna")]).unwrap());
            }
            write.commit().unwrap();
            assert_eq!(read(store, "ana"), Some(("Ana".to_owned(), Some(1))));
            assert_eq!(read(store, "aNa"), Some(("Ana".to_owned(), Some(1))));

            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            // An update through an equivalent key changes the other columns and keeps the key (IC4).
            assert!(
                write
                    .update_row(&table, &[("id", s("ANA")), ("n", Value::I64(2))])
                    .unwrap()
            );
            assert_eq!(
                write
                    .upsert_row(&table, &[("id", s("anA")), ("n", Value::I64(3))])
                    .unwrap(),
                Upserted::Updated
            );
            // An update that gives only an equivalent key is identical: nothing changes.
            assert!(write.update_row(&table, &[("id", s("ANA"))]).unwrap());
            write.commit().unwrap();
            assert_eq!(read(store, "ANA"), Some(("Ana".to_owned(), Some(3))));

            // Deleting and inserting in the same write changes the key's spelling.
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            assert!(write.delete_row(&table, &[s("ANA")]).unwrap());
            write
                .insert_row(&table, &[("id", s("ANA")), ("n", Value::I64(4))])
                .unwrap();
            write.commit().unwrap();
            assert_eq!(read(store, "ana"), Some(("ANA".to_owned(), Some(4))));

            // Deleting a range of equivalent bounds deletes the row.
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            assert_eq!(
                write
                    .delete_range(&table, &[], (Included(s("ANA")), Excluded(s("ana"))))
                    .unwrap(),
                0
            );
            assert_eq!(
                write
                    .delete_range(&table, &[], s("Ana")..=s("aNA"))
                    .unwrap(),
                1
            );
            assert_eq!(write.delete_range(&table, &[], ..).unwrap(), 0);
            write.commit().unwrap();
            assert_eq!(read(store, "ana"), None);
        });
    }
}

#[test]
fn ac_ic3_a_bulk_load_with_equivalent_keys_fails_as_with_repeated_ones() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        create(&mut write, "t", &keyed(Collation::Ci));
        write.commit().unwrap();
        let rows = [vec![("id", s("Bea"))], vec![("id", s("BEA"))]];
        match store.bulk_load::<_, ()>("t", rows.iter().map(Ok)) {
            Err(BulkLoadError::Store(Error::DuplicateKey {
                key, index: None, ..
            })) => {
                assert_eq!(key, "(\"BEA\")")
            }
            other => panic!("expected a duplicate key, got {other:?}"),
        }
        assert_eq!(read(store, "bea"), None, "the load failed whole");
    });
}

#[test]
fn ac_ic3_a_text_outside_the_key_keeps_its_spelling() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &keyed(Collation::Ci));
        write
            .insert_row(&table, &[("id", s("k")), ("note", s("Eva"))])
            .unwrap();
        // Not identical: the row changes.
        assert!(
            write
                .update_row(&table, &[("id", s("k")), ("note", s("EVA"))])
                .unwrap()
        );
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let row = read.get_row(&table, &[s("K")]).unwrap().unwrap();
        assert_eq!(row.get("note"), Some(s("EVA")));
        assert!(
            matches!(row.text(2), Some(Cow::Borrowed("EVA"))),
            "without a copy"
        );
        let projection = read.projection(&table, &["note"]).unwrap();
        let note = projection.column::<Text>(0).unwrap();
        assert!(matches!(
            projection.bind(&row).unwrap().get(&note),
            Some(Cow::Borrowed("EVA"))
        ));
    });
}

/// `ŉ` takes 2 bytes and folds to `ʼn`, 3: 700 of them make a key element of 1,402 bytes, but of 2,102 folded, over
/// the 2,048 of a key (IC7).
fn growing() -> String {
    "ŉ".repeat(700)
}

/// `a` with a combining acute accent takes 3 bytes and folds in `ci_ai` to `a`, 1: 800 of them make an element of
/// 2,402 bytes, but of 802 folded.
fn shrinking() -> String {
    "a\u{301}".repeat(800)
}

#[test]
fn ac_ic7_keys_and_bounds_are_measured_folded() {
    stores(|store, _| {
        let long = growing();
        let mut write = store.write().unwrap();
        let ci = create(&mut write, "t", &keyed(Collation::Ci));
        assert_eq!(
            too_large(write.insert_row(&ci, &[("id", s(&long))])),
            (SizeSubject::PrimaryKey, 2_102)
        );
        assert_eq!(
            too_large(write.delete_range(&ci, &[], (Included(s(&long)), Unbounded))),
            (SizeSubject::PrimaryKey, 2_102),
            "even when the range is empty"
        );
        assert_eq!(
            too_large(write.get_row(&ci, &[s(&long)])),
            (SizeSubject::PrimaryKey, 2_102)
        );
        // A prefix value of a key of two columns.
        let pair = Schema::new(
            vec![
                text("a").not_null().with_collation(Collation::Ci),
                text("b").not_null(),
            ],
            &["a", "b"],
        );
        let two = create(&mut write, "u", &pair);
        assert_eq!(
            too_large(write.delete_range(&two, &[s(&long)], ..)),
            (SizeSubject::PrimaryKey, 2_102)
        );
        // The same text fits in a binary key.
        let binary = create(&mut write, "b", &keyed(Collation::Binary));
        write.insert_row(&binary, &[("id", s(&long))]).unwrap();

        // A text over the maximum whose folded form fits is a key of a `ci_ai` column, read whole.
        let short = shrinking();
        let ciai = create(&mut write, "v", &keyed(Collation::CiAi));
        write.insert_row(&ciai, &[("id", s(&short))]).unwrap();
        let row = write
            .get_row(&ciai, &[s(&"a".repeat(800))])
            .unwrap()
            .unwrap();
        assert_eq!(row.text(0).unwrap(), short.as_str());
        assert_eq!(
            too_large(write.insert_row(&binary, &[("id", s(&short))])),
            (SizeSubject::PrimaryKey, 2_402)
        );
        write.commit().unwrap();
    });
}
