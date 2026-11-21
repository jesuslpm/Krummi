//! The JSON columns of the tables (7.5, `docs/prd/krummi/tables/tables-json.prd.md`) through the public API: the
//! round trip of the corpus (AC-JS1), the faults of a write with their table and column (AC-JS3 at table level), the
//! column null against the document `null` (AC-JS4), reads without a copy (AC-JS7), the schema rules and defaults
//! (AC-JS8), the identical update (AC-JS9), the largest document (AC-JS10), the format (AC-JS12) and the statistics
//! (AC-JS13). Each criterion runs on a store on disk and on a temporary one, except the parts that reopen or rewrite
//! the store's files, which run on disk only.

// The reader tests' generator and reference model (AC-JS11 (2)), of which these tests use a part.
#[allow(dead_code)]
#[path = "json_reader/generator.rs"]
mod generator;
mod support;

#[path = "table_json/reads.rs"]
mod reads;
#[path = "table_json/round_trip.rs"]
mod round_trip;
#[path = "table_json/schema.rs"]
mod schema;
#[path = "table_json/writes.rs"]
mod writes;

use std::path::Path;

use krummi::table::json::{Json, JsonKind, JsonValue};
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, JsonFault, Schema, Store, TableMode, ValueRule, WriteTable};
use krummi::{TableRow, WriteTransaction};
use krummi_pager::format::{Header, PAGE_SIZE, header_copies, write_header_pages};
use support::*;

use generator::Node;

/// A document given as text, which a write validates (JS3).
fn doc(text: &str) -> Value<'_> {
    Value::Json(Json::new(text))
}

/// A document given as bytes, which may not be UTF-8 (JS3).
fn doc_bytes(bytes: &[u8]) -> Value<'_> {
    Value::Json(Json::from_bytes(bytes))
}

/// A table `t` of an `i64` key `id` and a JSON column `doc` that allows nulls.
fn docs() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("doc", ColumnType::Json),
        ],
        &["id"],
    )
}

fn create<'s>(write: &mut WriteTransaction<'s>, name: &str, schema: &Schema) -> WriteTable<'s> {
    write
        .open_table_with(name, TableMode::CreateNew(schema))
        .unwrap()
}

/// The normalized text of the document a value read back holds; panics on another value.
#[track_caller]
fn text_of<'v>(value: &'v Value<'_>) -> &'v str {
    match value {
        Value::Json(json) => json.text().expect("a document read back"),
        other => panic!("not a document: {other:?}"),
    }
}

/// The normalized text of the column `doc` of the row of key `id` in `t`, as a read of `store` sees it, or `None` for
/// a null.
fn read_text(store: &Store, id: i64) -> Option<String> {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let row = read.get_row(&table, &[Value::I64(id)]).unwrap().unwrap();
    row.json(1).map(|json| json.text().unwrap().to_owned())
}

/// The table, column, rule and JSON fault of an invalid value, after checking its folder.
#[track_caller]
fn invalid_value(
    error: Error,
    folder: Option<&Path>,
) -> (Option<String>, Option<String>, ValueRule, Option<JsonFault>) {
    match error {
        Error::InvalidValue {
            folder: f,
            table,
            column,
            sequence,
            rule,
            json,
        } => {
            assert_eq!(f, folder_of(folder));
            assert_eq!(sequence, None);
            (table, column, rule, json.map(|fault| *fault))
        }
        other => panic!("expected an invalid value, got {other:?}"),
    }
}

/// Whether `value`, a value inside a document, is the value of the reference model `node` of AC-JS11 (2): its class,
/// its text for a number (the text it was written with), its decoded text for a string, and the names and values of
/// its properties, in order, and its elements.
#[track_caller]
fn assert_model(node: &Node, value: JsonValue<'_>) {
    match node {
        Node::Null => assert_eq!(value.kind(), JsonKind::Null),
        Node::Bool(b) => assert_eq!(value.as_bool(), Some(*b)),
        Node::Number(text) => {
            assert_eq!(value.kind(), JsonKind::Number);
            assert_eq!(value.text(), text);
        }
        Node::Str(text) => assert_eq!(value.as_text().as_deref(), Some(text.as_str())),
        Node::Array(items) => {
            assert_eq!(value.len(), Some(items.len()));
            for (item, element) in items.iter().zip(value.elements().unwrap()) {
                assert_model(item, element);
            }
        }
        Node::Object(members) => {
            assert_eq!(value.len(), Some(members.len()));
            for ((name, item), (read, element)) in members.iter().zip(value.members().unwrap()) {
                assert_eq!(&*read, name);
                assert_model(item, element);
            }
        }
    }
}

/// The address of a borrowed text, to compare where reads point (D11).
fn at(text: &str) -> usize {
    text.as_ptr() as usize
}

/// The address of the text of the `string` column `s` of `row`, which borrows it (D11).
#[track_caller]
fn string_at(row: &TableRow<'_>, name: &str) -> usize {
    match row.get(name) {
        Some(Value::String(std::borrow::Cow::Borrowed(text))) => at(text),
        other => panic!("{name} does not borrow its text: {other:?}"),
    }
}

/// AC-JS12 (JS12): a store of format 13, written by 7.3, is of a previous format and this engine does not open it
/// (F11), leaving it as it was; a new store has format 14.
#[test]
fn ac_js12_a_store_of_format_13_is_of_a_previous_format() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    create(&mut write, "t", &docs());
    write.commit().unwrap();
    store.close().unwrap();
    let data = tmp.path().join(krummi_pager::DATA_FILE);
    let mut bytes = std::fs::read(&data).unwrap();
    let [Some(a), Some(b)] = header_copies(&bytes[..2 * PAGE_SIZE]) else {
        panic!("both header copies")
    };
    assert_eq!((a.format_version, b.format_version), (14, 14));
    let current = if a.revision >= b.revision { a } else { b };
    write_header_pages(
        &mut bytes[..2 * PAGE_SIZE],
        &Header {
            format_version: 13,
            ..current
        },
    );
    std::fs::write(&data, &bytes).unwrap();
    match Store::open(tmp.path(), &krummi::StoreOptions::default()).err() {
        Some(Error::DifferentFormat {
            found, expected, ..
        }) => assert_eq!((found, expected), (13, 14)),
        other => panic!("expected a different format, got {other:?}"),
    }
    assert_eq!(std::fs::read(&data).unwrap(), bytes, "left as it was");
    let new = Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
    assert_eq!(new.info().format_version, 14);
}
