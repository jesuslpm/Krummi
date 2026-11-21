//! AC-JS7 (JS5, D11, PY2, PY3, IX8): the document and its strings without escapes point into the row read, by
//! position, by name, with the typed read and with a projection, and into the index entry for an included column,
//! without reading the row; strings and names with escapes are copies, and so is a copied row's document; and a JSON
//! column is prepared only as JSON.

use std::borrow::Cow;

use krummi::table::projection::{Bytes, JsonDocument, Text, TextBytes};
use krummi::table::value::{ColumnType, Value};
use krummi::{ErrorKind, IndexDefinition, IndexKey, IndexMode};

use super::*;

/// The document of the row: a string without escapes, one with escapes and a name with escapes.
const DOCUMENT: &str = r#"{ "t" : "plain", "e" : "a\nb", "n\tm" : 1 }"#;
const NORMALIZED: &str = r#"{"t":"plain","e":"a\nb","n\tm":1}"#;

/// A table `t` of an `i64` key `id`, an `i64` `k`, a `string` `s` and a JSON column `doc`, whose index `ix` on `k`
/// includes `s` and `doc`.
fn schema() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("k", ColumnType::I64),
            Column::new("s", ColumnType::String),
            Column::new("doc", ColumnType::Json),
        ],
        &["id"],
    )
}

/// Asserts that `json`, the document read from memory where the text of `s` starts at `s_at` with 3 bytes, is the
/// normalized document stored right after it, past its header byte; and that its strings and names point into it
/// without escapes and are copies with them.
#[track_caller]
fn assert_in_place(json: &Json<'_>, s_at: usize) {
    let text = json.text().unwrap();
    assert_eq!(text, NORMALIZED);
    assert_eq!(
        at(text),
        s_at + 3 + 1,
        "the document follows `s` and its header"
    );
    let document = json.document().unwrap();
    let plain = document.get("t").unwrap().as_text().unwrap();
    assert!(matches!(plain, Cow::Borrowed(_)));
    assert_eq!(at(&plain), at(text) + NORMALIZED.find("plain").unwrap());
    assert!(matches!(document.get("e").unwrap().as_text(), Some(Cow::Owned(e)) if e == "a\nb"));
    let names: Vec<Cow<'_, str>> = document.members().unwrap().map(|(name, _)| name).collect();
    assert!(matches!(&names[0], Cow::Borrowed("t")));
    assert_eq!(at(&names[0]), at(text) + 2);
    assert!(matches!(&names[2], Cow::Owned(name) if name == "n\tm"));
}

#[test]
fn ac_js7_reads_point_into_the_row_and_the_index_entry() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &schema());
        let index =
            IndexDefinition::new("ix", vec![IndexKey::new("k")]).with_included(&["s", "doc"]);
        write
            .create_index(&table, &index, IndexMode::CreateNew)
            .unwrap();
        write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(1)),
                    ("k", Value::I64(7)),
                    ("s", Value::String("abc".into())),
                    ("doc", doc(DOCUMENT)),
                ],
            )
            .unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let projection = read.projection(&table, &["doc", "s"]).unwrap();
        let prepared = projection.column::<JsonDocument>(0).unwrap();
        let row = read.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
        let s_at = string_at(&row, "s");
        let Some(Value::Json(by_position)) = row.value(3) else {
            panic!("a document")
        };
        assert_in_place(&by_position, s_at);
        let Some(Value::Json(by_name)) = row.get("doc") else {
            panic!("a document")
        };
        assert_in_place(&by_name, s_at);
        assert_in_place(&row.json(3).unwrap(), s_at);
        assert_in_place(
            &projection.bind(&row).unwrap().get(&prepared).unwrap(),
            s_at,
        );
        let row_at = at(row.json(3).unwrap().text().unwrap());
        // A scan's rows too.
        for scanned in read.range_rows(&table, &[], ..).unwrap() {
            let scanned = scanned.unwrap();
            assert_in_place(&scanned.json(3).unwrap(), string_at(&scanned, "s"));
        }
        // An entry of the index, which holds `s` and `doc` in that order: in the entry, not in the row.
        for entry in read.index_entries(&table, "ix", &[], ..).unwrap() {
            let entry = entry.unwrap();
            let Some(Value::String(Cow::Borrowed(s))) = entry.get("s") else {
                panic!("an included text borrows the entry")
            };
            let Some(Value::Json(json)) = entry.get("doc") else {
                panic!("a document")
            };
            assert_in_place(&json, at(s));
            assert_ne!(
                at(json.text().unwrap()),
                row_at,
                "the entry's, not the row's"
            );
            let Some(Value::Json(by_position)) = entry.value(2) else {
                panic!("a document")
            };
            assert_eq!(at(by_position.text().unwrap()), at(json.text().unwrap()));
        }
        // A copied row has a copy of its own, which outlives the transaction and navigates as one read.
        let copied = row.into_owned();
        let bound = projection.bind(&copied).unwrap().get(&prepared).unwrap();
        drop(read);
        for json in [copied.json(3).unwrap(), bound] {
            let text = json.text().unwrap();
            assert_eq!(text, NORMALIZED);
            assert_ne!(at(text), row_at, "a copy");
            let document = json.document().unwrap();
            assert_eq!(
                document
                    .pointer("/t")
                    .unwrap()
                    .unwrap()
                    .as_text()
                    .as_deref(),
                Some("plain")
            );
            assert_eq!(document.get("n\tm").unwrap().as_i64(), Some(1));
        }
    });
}

/// AC-JS7 (PY2): a JSON column prepared as `string` or `bytes`, and a `string` column prepared as JSON, fail with a
/// different schema, with the type asked for and the column's.
#[test]
fn ac_js7_a_json_column_is_prepared_only_as_json() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        create(&mut write, "t", &schema());
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let projection = read.projection(&table, &["doc", "s"]).unwrap();
        let different = |error: Error| match error {
            Error::DifferentSchema {
                table,
                column,
                expected,
                found,
                ..
            } => (
                table,
                column.unwrap(),
                String::from(expected),
                String::from(found),
            ),
            other => panic!("expected a different schema, got {other:?}"),
        };
        let doc_as = |expected: &str| {
            (
                "t".to_owned(),
                "doc".to_owned(),
                expected.to_owned(),
                "json".to_owned(),
            )
        };
        assert_eq!(
            different(projection.column::<Text>(0).unwrap_err()),
            doc_as("string")
        );
        assert_eq!(
            different(projection.column::<TextBytes>(0).unwrap_err()),
            doc_as("string")
        );
        assert_eq!(
            different(projection.column::<Bytes>(0).unwrap_err()),
            doc_as("bytes")
        );
        assert_eq!(
            different(projection.column::<JsonDocument>(1).unwrap_err()),
            (
                "t".to_owned(),
                "s".to_owned(),
                "json".to_owned(),
                "string".to_owned()
            )
        );
        assert!(projection.column::<JsonDocument>(0).is_ok());
        assert_eq!(
            projection.column::<Text>(1).map(drop).map_err(|e| e.kind()),
            Ok(())
        );
        assert_eq!(
            projection.column::<i64>(0).unwrap_err().kind(),
            ErrorKind::DifferentSchema
        );
    });
}
