//! Secondary indexes through the public API (secondary indexes PRD 7.2, T2–T4 of its execution): their definitions in
//! the catalog, creating them over the rows a table has and dropping them (AC-IX1, AC-IX7 in memory), scanning every
//! index for rows and for entries (AC-IX3), their rules and errors (AC-IX9), the schema changes and the table
//! operations that keep or drop them (AC-IX10), Describe with their identities and the index state (AC-IX11), what is
//! a change (AC-IX12), damage (the parts of AC-IX13 that create, drop, Describe and scan), the format (AC-IX15) and the
//! row writes that keep them (AC-IX2, AC-IX6, AC-IX8). The entries are also read through a test hook that gives their
//! stored bytes; the intervals of a scan and the property test are in `table_index_scans.rs`.
//!
//! Each criterion runs on a store on disk and on a temporary one, except the parts that reopen or damage the store's
//! files, which run on disk only.

#[path = "../../krummi-tx/tests/common/mod.rs"]
mod common;
mod support;

use std::borrow::Cow;
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::time::Duration;

use common::Run;
use krummi::ids::{Ulid, Uuid};
use krummi::table::key::key_len;
use krummi::table::testing::{StoredIndexEntry, with_identity_source};
use krummi::table::value::{
    ColumnType, DATE_MAX, DATE_MIN, DURATION_MAX, Decimal, TIME_MAX, TIMESTAMP_MAX, TIMESTAMP_MIN,
    UuidOrder, Value, VectorElement, VectorType,
};
use krummi::{
    BulkLoadError, Column, Direction, Error, ErrorKind, IndexDefinition, IndexKey, IndexMode,
    Nulls, OpenMode, Schema, SchemaDifference, SchemaIndex, SchemaRule, SequenceMode, SizeSubject,
    Store, TableMode, TableSchema, Upserted, WriteTable, WriteTransaction,
};
use support::*;

// ---------------------------------------------------------------------------------------------------------------
// Helpers

fn key(column: &str) -> IndexKey {
    IndexKey::new(column)
}

fn index(name: &str, key: &[IndexKey]) -> IndexDefinition {
    IndexDefinition::new(name, key.to_vec())
}

/// `id i64 not null, n i32, m i32, s string, a i64, b bool, ve vector(f32, 2)`, primary key `id`.
fn people() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I32),
            Column::new("m", ColumnType::I32),
            Column::new("s", ColumnType::String),
            Column::new("a", ColumnType::I64),
            Column::new("b", ColumnType::Bool),
            Column::new("ve", vector(2)),
        ],
        &["id"],
    )
}

fn vector(dimension: u16) -> ColumnType {
    ColumnType::Vector(VectorType {
        element: VectorElement::F32,
        dimension,
        cosine: false,
    })
}

fn text(value: &str) -> Value<'static> {
    Value::String(Cow::Owned(value.to_owned()))
}

/// The row `id` of [`people`] with `n` and `s`.
fn person(id: i64, n: Option<i32>, s: &str) -> Vec<(&'static str, Value<'static>)> {
    vec![
        ("id", Value::I64(id)),
        ("n", n.map_or(Value::Null, Value::I32)),
        ("s", text(s)),
    ]
}

fn create_table<'s>(
    write: &mut WriteTransaction<'s>,
    name: &str,
    schema: &Schema,
) -> WriteTable<'s> {
    write
        .open_table_with(name, TableMode::CreateNew(schema))
        .unwrap()
}

/// Commits the table `t` of [`people`] with the rows `rows`, and returns its schema.
fn commit_people(store: &Store, rows: &[Vec<(&'static str, Value<'static>)>]) -> TableSchema {
    let mut write = store.write().unwrap();
    let table = create_table(&mut write, "t", &people());
    for row in rows {
        write.insert_row(&table, row).unwrap();
    }
    write.commit().unwrap();
    store.read().unwrap().describe_table("t").unwrap()
}

/// The entries of the index `name` of `table`, through the test hook.
fn entries(
    write: &WriteTransaction<'_>,
    table: &WriteTable<'_>,
    name: &str,
) -> Vec<StoredIndexEntry> {
    write
        .stored_index_entries(table, name)
        .unwrap()
        .expect("the index exists")
}

/// The entries of the index `name` of the table `t` that a new read sees.
fn read_entries(store: &Store, name: &str) -> Vec<StoredIndexEntry> {
    let read = store.read().unwrap();
    read.stored_index_entries(&read.open_table("t").unwrap(), name)
        .unwrap()
        .expect("the index exists")
}

/// The names of the indexes of `schema`, in its order.
fn index_names(schema: &TableSchema) -> Vec<&str> {
    schema.indexes().iter().map(|ix| ix.name.as_str()).collect()
}

/// The table, index, column and rule of an invalid-schema error, after checking its folder.
#[track_caller]
fn invalid_schema(
    error: Error,
    folder: Option<&Path>,
) -> (String, Option<String>, Option<String>, SchemaRule) {
    match error {
        Error::InvalidSchema {
            folder: f,
            table,
            column,
            rule,
            index,
        } => {
            assert_eq!(f, folder_of(folder));
            (table, index.map(String::from), column, rule)
        }
        other => panic!("expected an invalid schema, got {other}"),
    }
}

/// The table, index, difference, expected and found text of a different-schema error of an index, after checking
/// its class and its folder.
#[track_caller]
fn different_schema(
    error: Error,
    folder: Option<&Path>,
) -> (String, Option<String>, SchemaDifference, String, String) {
    assert_eq!(error.kind(), ErrorKind::DifferentSchema);
    match error {
        Error::DifferentIndex {
            folder: f,
            table,
            index,
            difference,
            expected,
            found,
        } => {
            assert_eq!(f, folder_of(folder));
            (
                table,
                Some(index),
                difference,
                expected.into(),
                found.into(),
            )
        }
        other => panic!("expected a different index, got {other}"),
    }
}

/// The table, primary key, index and index key of a duplicate-key error, after checking its folder.
#[track_caller]
fn duplicate(
    error: Error,
    folder: Option<&Path>,
) -> (String, String, Option<String>, Option<String>) {
    match error {
        Error::DuplicateKey {
            folder: f,
            table,
            key,
            index,
            index_key,
        } => {
            assert_eq!(f, folder_of(folder));
            (
                table,
                key,
                index.map(String::from),
                index_key.map(String::from),
            )
        }
        other => panic!("expected a duplicate key, got {other}"),
    }
}

/// The table, index and received size of an invalid size of an index entry, after checking its folder and limits.
#[track_caller]
fn entry_too_large(error: Error, folder: Option<&Path>) -> (String, String, u64) {
    match error {
        Error::InvalidSize {
            folder: f,
            what: SizeSubject::IndexEntry,
            received,
            min: 1,
            max: 2048,
            table: Some(table),
            index: Some(index),
        } => {
            assert_eq!(f, folder_of(folder));
            (table.into(), index.into(), received)
        }
        other => panic!("expected an index entry too large, got {other}"),
    }
}

/// The table and index of a tree-already-exists error for an index, after checking its folder.
#[track_caller]
fn index_exists(error: Error, folder: Option<&Path>) -> (String, String) {
    match error {
        Error::TreeAlreadyExists {
            folder: f,
            name,
            index: Some(index),
        } => {
            assert_eq!(f, folder_of(folder));
            (name, index.into())
        }
        other => panic!("expected an index that exists, got {other}"),
    }
}

fn some(text: &str) -> Option<String> {
    Some(text.to_owned())
}

/// The entries that `index` of a table with `schema` must have for `rows` (each a map of its columns' values), in the
/// order of N5: its key columns' values, the primary key `id` and its included columns' values; equal keys in the
/// order of the primary key (IX8).
#[allow(clippy::type_complexity)]
fn model_entries(
    schema: &TableSchema,
    index: &SchemaIndex,
    rows: &[Vec<(String, Value<'static>)>],
) -> Vec<(
    Vec<Value<'static>>,
    Vec<Value<'static>>,
    Vec<Value<'static>>,
)> {
    let get = |row: &[(String, Value<'static>)], name: &str| {
        row.iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
            .unwrap()
    };
    let mut model: Vec<_> = rows
        .iter()
        .map(|row| {
            (
                index.key.iter().map(|k| get(row, &k.name)).collect(),
                vec![get(row, "id")],
                index.included.iter().map(|c| get(row, &c.name)).collect(),
            )
        })
        .collect();
    model.sort_by(
        |(a, pa, _): &(
            Vec<Value<'static>>,
            Vec<Value<'static>>,
            Vec<Value<'static>>,
        ),
         (b, pb, _)| {
            index
                .key
                .iter()
                .zip(a.iter().zip(b))
                .map(|(k, (a, b))| {
                    let ty = schema.column(&k.name).unwrap().ty;
                    column_cmp(&ty, k.direction, k.nulls, a, b)
                })
                .find(|o| o.is_ne())
                .unwrap_or_else(|| model_cmp(&ColumnType::I64, &pa[0], &pb[0]))
        },
    );
    model
}

/// Asserts that `entries` are those of [`model_entries`], in its order.
#[track_caller]
fn assert_entries(
    entries: &[StoredIndexEntry],
    schema: &TableSchema,
    index: &SchemaIndex,
    rows: &[Vec<(String, Value<'static>)>],
) {
    let model = model_entries(schema, index, rows);
    assert_eq!(
        entries.len(),
        model.len(),
        "{}: one entry per row",
        index.name
    );
    for (at, (entry, (key, primary, included))) in entries.iter().zip(&model).enumerate() {
        assert!(
            same_all(&entry.key, key)
                && same_all(&entry.primary_key, primary)
                && same_all(&entry.included, included),
            "{}: entry {at} is {entry:?}, expected {key:?} {primary:?} {included:?}",
            index.name
        );
    }
    assert!(
        entries.windows(2).all(|w| w[0].bytes < w[1].bytes),
        "{}: keys in byte order",
        index.name
    );
}

/// Every row of the table `t`, each as its columns' names and owned values, as the write sees them.
fn rows_of(
    write: &WriteTransaction<'_>,
    table: &WriteTable<'_>,
) -> Vec<Vec<(String, Value<'static>)>> {
    write
        .range_rows(table, &[], ..)
        .unwrap()
        .map(|row| {
            let row = row.unwrap();
            row.columns()
                .map(|(name, value)| (name.to_owned(), value.into_owned()))
                .collect()
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------
// AC-IX1: creating indexes over the rows of a table

/// The sortable columns of [`all_types`] besides the primary key and `big`, with their types.
fn sortable() -> Vec<(&'static str, ColumnType)> {
    vec![
        ("bo", ColumnType::Bool),
        ("i8", ColumnType::I8),
        ("i16", ColumnType::I16),
        ("i32", ColumnType::I32),
        ("i64", ColumnType::I64),
        ("u8", ColumnType::U8),
        ("u16", ColumnType::U16),
        ("u32", ColumnType::U32),
        ("u64", ColumnType::U64),
        ("f32", ColumnType::F32),
        ("f64", ColumnType::F64),
        (
            "de",
            ColumnType::Decimal {
                precision: 10,
                scale: 2,
            },
        ),
        ("s", ColumnType::String),
        ("by", ColumnType::Bytes),
        ("da", ColumnType::Date),
        ("ti", ColumnType::Time),
        ("ts", ColumnType::Timestamp),
        ("du", ColumnType::Duration),
        ("u1", ColumnType::Uuid(UuidOrder::Rfc9562)),
        ("u2", ColumnType::Uuid(UuidOrder::SqlServer)),
        ("ul", ColumnType::Ulid),
    ]
}

/// `id i64 not null`, a column of each sortable type of N4 (all allowing nulls), a vector `ve` and a text `big`;
/// primary key `id`.
fn all_types() -> Schema {
    let mut columns = vec![Column::new("id", ColumnType::I64).not_null()];
    columns.extend(
        sortable()
            .into_iter()
            .map(|(name, ty)| Column::new(name, ty)),
    );
    columns.push(Column::new("ve", vector(2)));
    columns.push(Column::new("big", ColumnType::String));
    Schema::new(columns, &["id"])
}

/// The values a row of [`all_types`] may have in the column `name`: nulls, repeated values and the extremes of each
/// domain of E8, `-0` and NaNs of other bits than the stored one.
fn pool(name: &str) -> Vec<Value<'static>> {
    const NANOS: i128 = 1000;
    let dec = |mantissa| Value::Decimal(Decimal { mantissa, scale: 2 });
    let bytes = |b: &[u8]| Value::Bytes(Cow::Owned(b.to_vec()));
    let mut values = match name {
        "bo" => vec![Value::Bool(false), Value::Bool(true)],
        "i8" => [i8::MIN, -1, 0, 1, i8::MAX].map(Value::I8).to_vec(),
        "i16" => [i16::MIN, -1, 0, 1, i16::MAX].map(Value::I16).to_vec(),
        "i32" => [i32::MIN, -1, 0, 1, i32::MAX].map(Value::I32).to_vec(),
        "i64" => [i64::MIN, -1, 0, 1, i64::MAX].map(Value::I64).to_vec(),
        "u8" => [0, 1, u8::MAX].map(Value::U8).to_vec(),
        "u16" => [0, 1, u16::MAX].map(Value::U16).to_vec(),
        "u32" => [0, 1, u32::MAX].map(Value::U32).to_vec(),
        "u64" => [0, 1, u64::MAX].map(Value::U64).to_vec(),
        "f32" => [
            f32::NEG_INFINITY,
            f32::MIN,
            -1.5,
            -0.0,
            0.0,
            1e-45,
            1.5,
            f32::MAX,
            f32::INFINITY,
            f32::from_bits(0x7FC0_0001),
            f32::from_bits(0xFFC0_0000),
        ]
        .map(Value::F32)
        .to_vec(),
        "f64" => [
            f64::NEG_INFINITY,
            f64::MIN,
            -1.5,
            -0.0,
            0.0,
            5e-324,
            1.5,
            f64::MAX,
            f64::INFINITY,
            f64::from_bits(0x7FF0_0000_0000_0001),
            f64::from_bits(0xFFF8_0000_0000_0000),
        ]
        .map(Value::F64)
        .to_vec(),
        "de" => vec![
            dec(-9_999_999_999),
            dec(-1),
            dec(0),
            dec(1),
            dec(9_999_999_999),
        ],
        "s" => ["", "a", "a\0", "a\0b", "ab", "b", "\u{10FFFF}"]
            .map(text)
            .to_vec(),
        "by" => vec![
            bytes(b""),
            bytes(&[0]),
            bytes(&[0, 0]),
            bytes(&[0, 0xFF]),
            bytes(&[0x61]),
            bytes(&[0xFF]),
        ],
        "da" => [DATE_MIN, -1, 0, DATE_MAX].map(Value::Date).to_vec(),
        "ti" => vec![
            Value::Time(0),
            Value::Time(1000),
            Value::Time(i64::try_from(i128::from(TIME_MAX) * NANOS).unwrap()),
        ],
        "ts" => [
            i128::from(TIMESTAMP_MIN) * NANOS,
            -NANOS,
            0,
            i128::from(TIMESTAMP_MAX) * NANOS,
        ]
        .map(Value::Timestamp)
        .to_vec(),
        "du" => [
            -i128::from(DURATION_MAX) * NANOS,
            -NANOS,
            0,
            i128::from(DURATION_MAX) * NANOS,
        ]
        .map(Value::Duration)
        .to_vec(),
        other => panic!("no pool for {other}"),
    };
    values.push(Value::Null);
    values
}

/// The row `id` of [`all_types`], its values drawn from [`pool`] by `rng`; the identifiers are distinct per row,
/// with some nulls, and `big` is null.
fn typed_row(id: i64, rng: &mut Lcg) -> Vec<(&'static str, Value<'static>)> {
    let mut row = vec![("id", Value::I64(id))];
    for (name, _) in sortable() {
        let value = match name {
            "u1" | "u2" | "ul" if id % 7 == 3 => Value::Null,
            "u1" | "u2" => Value::Uuid(Uuid(random_16(rng))),
            "ul" => Value::Ulid(Ulid(random_16(rng))),
            name => {
                let pool = pool(name);
                pool[rng.next() as usize % pool.len()].clone()
            }
        };
        row.push((name, value));
    }
    let ve = [id as f32, -1.0]
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    row.push(("ve", Value::Vector(Cow::Owned(ve))));
    row
}

fn random_16(rng: &mut Lcg) -> [u8; 16] {
    let mut bytes = [0; 16];
    for byte in &mut bytes {
        *byte = rng.next() as u8;
    }
    bytes
}

/// Every combination of a direction and a null order.
const ORDERS: [(Direction, Nulls); 4] = [
    (Direction::Ascending, Nulls::First),
    (Direction::Descending, Nulls::Last),
    (Direction::Ascending, Nulls::Last),
    (Direction::Descending, Nulls::First),
];

fn ordered(column: &str, (direction, nulls): (Direction, Nulls)) -> IndexKey {
    IndexKey {
        column: column.to_owned(),
        direction,
        nulls,
    }
}

/// The indexes of AC-IX1: one per sortable column in each order by turns, unique on the identifiers; every
/// combination of orders of two consecutive columns behind a `bytes` column and behind a `string` one; an entry at
/// the most bytes of a key; included columns, a vector and a primary key column among them; and the columns added
/// after the rows.
fn typed_indexes() -> Vec<IndexDefinition> {
    let mut indexes: Vec<IndexDefinition> = sortable()
        .into_iter()
        .enumerate()
        .map(|(i, (name, _))| {
            let definition = index(&format!("one_{name}"), &[ordered(name, ORDERS[i % 4])]);
            match name {
                "u1" | "u2" | "ul" => definition.unique(),
                _ => definition,
            }
        })
        .collect();
    for (first, second) in [("by", "s"), ("s", "i32")] {
        for (i, a) in ORDERS.iter().enumerate() {
            for (j, b) in ORDERS.iter().enumerate() {
                indexes.push(index(
                    &format!("two_{first}_{second}_{i}{j}"),
                    &[ordered(first, *a), ordered(second, *b)],
                ));
            }
        }
    }
    indexes.push(index("big", &[key("big")]));
    indexes.push(
        index(
            "covering",
            &[key("i32").descending(), key("i64").nulls_last()],
        )
        .with_included(&["ve", "by", "id"]),
    );
    indexes.push(index("late", &[key("late_d"), key("late_n")]));
    indexes.push(index("late_unique", &[key("late_n").descending().nulls_last()]).unique());
    indexes
}

/// The table `t` of [`all_types`] with 64 rows of [`typed_row`], an entry at the most bytes of a key, columns added
/// after 60 of the rows (`late_d`, not null with the default 5, and `late_n`) and the indexes of [`typed_indexes`],
/// created over the rows in a write that has not committed them.
fn typed_write(store: &Store) -> (WriteTransaction<'_>, WriteTable<'_>) {
    let mut rng = Lcg(7);
    let mut write = store.write().unwrap();
    let table = create_table(&mut write, "t", &all_types());
    for id in 0..60 {
        write.insert_row(&table, &typed_row(id, &mut rng)).unwrap();
    }
    // An entry of the most bytes of a key: the text's element and the primary key's take 2,048 bytes.
    let pk = key_len(&[ColumnType::I64], &[Value::I64(0)]);
    let big = "x".repeat(2048 - pk - 2);
    assert_eq!(key_len(&[ColumnType::String], &[text(&big)]) + pk, 2048);
    write
        .update_row(&table, &[("id", Value::I64(0)), ("big", text(&big))])
        .unwrap();
    write.commit().unwrap();

    // Columns added after the rows, with and without a default; rows after them give them values.
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .add_column(
            &table,
            &Column::new("late_d", ColumnType::I32)
                .not_null()
                .with_default(Value::I32(5)),
        )
        .unwrap();
    write
        .add_column(&table, &Column::new("late_n", ColumnType::I64))
        .unwrap();
    for id in 60..64 {
        let mut row = typed_row(id, &mut rng);
        row.push(("late_d", Value::I32(i32::try_from(id).unwrap() % 2)));
        row.push(("late_n", Value::I64(-id)));
        write.insert_row(&table, &row).unwrap();
    }
    let definitions = typed_indexes();
    for definition in &definitions {
        write
            .create_index(&table, definition, IndexMode::CreateNew)
            .unwrap();
    }
    (write, table)
}

/// AC-IX1 (IX1, IX2, N5, AC-N7): over a table with rows of every sortable type, with the extremes of their domains,
/// `-0`, NaNs of several bits, both orders of `uuid`, nulls and repeated values, and with columns added after the
/// rows with and without a default, unique and non-unique indexes in every combination of direction and null order,
/// behind `bytes` with a zero byte and `string` with U+0000, with an entry at the most bytes of a key and with
/// included columns, have each one entry per row, in the order of N5 with equal keys by the primary key, the
/// values a read of the row gives (the default or null in the added columns) and the included ones; as the write
/// sees them and, after the commit, as a read does.
#[test]
fn ac_ix1_indexes_created_over_rows_have_their_entries_in_n5_order() {
    stores(|store, _| {
        let (write, table) = typed_write(store);
        let definitions = typed_indexes();
        let schema = write.table_schema(&table).unwrap();
        assert_eq!(schema.indexes().len(), definitions.len());
        let rows = rows_of(&write, &table);
        assert_eq!(rows.len(), 64);
        for ix in schema.indexes() {
            assert_entries(&entries(&write, &table, &ix.name), &schema, ix, &rows);
        }
        // The columns added after the rows read their default or null in the entries of the rows before them.
        let late = entries(&write, &table, "late");
        let before: Vec<_> = late
            .iter()
            .filter(|e| matches!(e.primary_key[0], Value::I64(id) if id < 60))
            .collect();
        assert_eq!(before.len(), 60);
        assert!(
            before
                .iter()
                .all(|e| same_all(&e.key, &[Value::I32(5), Value::Null]))
        );
        // The entry at the limit, and the primary key column that `covering` was given among its included columns.
        let big_entry = entries(&write, &table, "big")
            .into_iter()
            .find(|e| e.key != [Value::Null])
            .unwrap();
        assert_eq!(big_entry.bytes.len(), 2048);
        let covering = schema.index("covering").unwrap();
        let included: Vec<&str> = covering.included.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(included, ["ve", "by"]);
        let written: Vec<(String, Vec<StoredIndexEntry>)> = schema
            .indexes()
            .iter()
            .map(|ix| (ix.name.clone(), entries(&write, &table, &ix.name)))
            .collect();
        write.commit().unwrap();
        for (name, written) in written {
            let read = read_entries(store, &name);
            let same_entry = |(a, b): (&StoredIndexEntry, &StoredIndexEntry)| {
                a.bytes == b.bytes && same_all(&a.key, &b.key) && same_all(&a.included, &b.included)
            };
            assert!(
                read.len() == written.len() && read.iter().zip(&written).all(same_entry),
                "{name}"
            );
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC-IX3: scanning every index for rows and for entries

/// Whether `value` is text, bytes or a vector that borrows the page (D11).
fn borrows(value: &Value<'_>) -> bool {
    matches!(
        value,
        Value::String(Cow::Borrowed(_))
            | Value::Bytes(Cow::Borrowed(_))
            | Value::Vector(Cow::Borrowed(_))
    )
}

/// Asserts that `entry`, an entry of `index`, has the values of the model's entry `(key, primary, included)` (IX8):
/// its key columns, its included ones and the primary key column `id`, in that order, by position and by name; the
/// key and primary key values in a copy, the included text, bytes and vectors borrowing the page.
#[track_caller]
#[allow(clippy::type_complexity)]
fn assert_entry(
    entry: &krummi::IndexEntry<'_>,
    index: &SchemaIndex,
    (key, primary, included): &(
        Vec<Value<'static>>,
        Vec<Value<'static>>,
        Vec<Value<'static>>,
    ),
) {
    let names: Vec<&str> = (index.key.iter().map(|k| k.name.as_str()))
        .chain(index.included.iter().map(|c| c.name.as_str()))
        .chain(["id"])
        .collect();
    let expected: Vec<&Value<'static>> = key.iter().chain(included).chain(primary).collect();
    assert_eq!(entry.len(), names.len(), "{}", index.name);
    assert!(!entry.is_empty());
    assert_eq!(entry.value(names.len()), None);
    assert_eq!(entry.name(names.len()), None);
    for (at, (name, value)) in names.iter().zip(&expected).enumerate() {
        assert_eq!(entry.name(at), Some(*name), "{}", index.name);
        let found = entry.value(at).unwrap();
        assert!(
            same(&found, value),
            "{}: {name} is {found:?}, expected {value:?}",
            index.name
        );
        assert!(
            same(&entry.get(name).unwrap(), value),
            "{}: {name} by name",
            index.name
        );
        let in_copy = at < key.len() || at >= key.len() + included.len();
        let shared = matches!(found, Value::String(_) | Value::Bytes(_) | Value::Vector(_));
        assert!(
            !shared || borrows(&found) != in_copy,
            "{}: {name} is {found:?}, which should be in a copy: {in_copy}",
            index.name
        );
    }
    let columns: Vec<(&str, Value<'_>)> = entry.columns().collect();
    assert_eq!(entry.values().len(), names.len());
    assert!(columns.iter().map(|(n, _)| *n).eq(names.iter().copied()));
    assert!(entry.get("not a column").is_none());
}

/// Asserts that the scans of the whole of every index of `schema` give, for entries and for rows, forward and
/// backward, those of [`model_entries`] over `rows` in IX8's order, each row whole; in a read or in a write.
macro_rules! assert_index_scans {
    ($tx:expr, $table:expr, $schema:expr, $rows:expr) => {{
        let (schema, rows): (&TableSchema, &[Vec<(String, Value<'static>)>]) = ($schema, $rows);
        let row_of = |id: &Value<'_>| {
            rows.iter()
                .find(|row| row.iter().any(|(n, v)| n == "id" && same(v, id)))
                .unwrap()
        };
        for ix in schema.indexes() {
            let model = model_entries(schema, ix, rows);
            let forward: Vec<krummi::IndexEntry<'_>> =
                ($tx.index_entries($table, &ix.name, &[], ..))
                    .unwrap()
                    .map(Result::unwrap)
                    .collect();
            assert_eq!(forward.len(), model.len(), "{}", ix.name);
            for (entry, model) in forward.iter().zip(&model) {
                assert_entry(entry, ix, model);
            }
            let backward: Vec<krummi::IndexEntry<'_>> =
                ($tx.index_entries($table, &ix.name, &[], ..))
                    .unwrap()
                    .rev()
                    .map(Result::unwrap)
                    .collect();
            assert_eq!(backward.len(), model.len(), "{}", ix.name);
            for (entry, model) in backward.iter().zip(model.iter().rev()) {
                assert_entry(entry, ix, model);
            }
            for back in [false, true] {
                let scan = $tx.index_rows($table, &ix.name, &[], ..).unwrap();
                let found: Vec<_> = if back {
                    scan.rev().collect()
                } else {
                    scan.collect()
                };
                let expected: Vec<_> = if back {
                    model.iter().rev().collect()
                } else {
                    model.iter().collect()
                };
                assert_eq!(found.len(), expected.len(), "{}", ix.name);
                for (row, (_, primary, _)) in found.into_iter().zip(expected) {
                    let row = row.unwrap();
                    assert!(same(&row.get("id").unwrap(), &primary[0]), "{}", ix.name);
                    let written = row_of(&primary[0]);
                    assert!(
                        (row.columns().zip(written))
                            .all(|((n, v), (wn, wv))| n == wn && same(&v, wv)),
                        "{}",
                        ix.name
                    );
                }
            }
        }
    }};
}

/// AC-IX3 (IX8, AC-N7): over the indexes of AC-IX1, whose rows repeat their keys, nulls included, scanning each index
/// whole forward and backward gives its entries and its rows in IX8's order (equal keys by the primary key, and the
/// exact reverse backward): the entries with their key columns, their included columns and the primary key, by
/// position and by name, the key's values in a copy and the included text, bytes and vectors borrowing the page, the
/// `uuid` of SQL Server's order in RFC 9562's bytes; the rows whole, also lent one at a time and read through a
/// projection; in the write that created the indexes and in a read after it commits.
#[test]
fn ac_ix3_scans_of_every_index_give_its_rows_and_entries_in_order() {
    stores(|store, _| {
        let (write, table) = typed_write(store);
        let schema = write.table_schema(&table).unwrap();
        let rows = rows_of(&write, &table);
        assert_index_scans!(write, &table, &schema, &rows);
        write.commit().unwrap();

        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert_index_scans!(read, &table, &schema, &rows);
        // Repeated keys, nulls among them, in the order of the primary key.
        let ids = |name: &str, prefix: &[Value<'_>]| -> Vec<Value<'static>> {
            (read.index_entries(&table, name, prefix, ..).unwrap())
                .map(|entry| entry.unwrap().get("id").unwrap().into_owned())
                .collect()
        };
        for prefix in [vec![Value::Null], vec![Value::Bool(true)]] {
            let found = ids("one_bo", &prefix);
            assert!(found.len() > 1, "{prefix:?}");
            assert!(
                found
                    .windows(2)
                    .all(|w| model_cmp(&ColumnType::I64, &w[0], &w[1]).is_lt())
            );
        }
        // The `uuid`s of SQL Server's order come in RFC 9562's bytes, in the order of SQL Server.
        let u2: Vec<Value<'static>> = (read.index_entries(&table, "one_u2", &[], ..).unwrap())
            .map(|entry| entry.unwrap().value(0).unwrap().into_owned())
            .filter(|value| !matches!(value, Value::Null))
            .collect();
        assert!(u2.len() > 10);
        let sql = ColumnType::Uuid(UuidOrder::SqlServer);
        let at = sortable().iter().position(|(n, _)| *n == "u2").unwrap();
        let (direction, _) = ORDERS[at % 4];
        assert!(u2.windows(2).all(|w| {
            let order = model_cmp(&sql, &w[0], &w[1]);
            match direction {
                Direction::Ascending => order.is_lt(),
                Direction::Descending => order.is_gt(),
            }
        }));
        // Lent one at a time, and through a projection.
        let projection = read.projection(&table, &["s", "id"]).unwrap();
        let id = projection.column::<i64>(1).unwrap();
        let mut scan = read.index_rows(&table, "covering", &[], ..).unwrap();
        let mut lent = Vec::new();
        while let Some(row) = scan.next_row().unwrap() {
            lent.push(projection.bind(row).unwrap().get(&id).unwrap());
        }
        let projected: Vec<i64> = (read.index_rows(&table, "covering", &[], ..).unwrap())
            .map(|row| projection.bind(&row.unwrap()).unwrap().get(&id).unwrap())
            .collect();
        let expected: Vec<i64> = model_entries(&schema, schema.index("covering").unwrap(), &rows)
            .into_iter()
            .map(|(_, primary, _)| match primary[0] {
                Value::I64(id) => id,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(lent, expected);
        assert_eq!(projected, expected);
    });
}

/// `name string not null, id i64 not null, n i32, t string, by bytes, ve vector(f32, 2)`, primary key `(name, id)`.
fn named_pairs() -> Schema {
    Schema::new(
        vec![
            Column::new("name", ColumnType::String).not_null(),
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I32),
            Column::new("t", ColumnType::String),
            Column::new("by", ColumnType::Bytes),
            Column::new("ve", vector(2)),
        ],
        &["name", "id"],
    )
}

/// AC-IX3 (IX1, IX8): with a primary key column in the index's key and the other, a text, among its included columns,
/// an entry has each column once: the key columns, the included ones without the primary key's, and then the text of
/// the primary key, in a copy; the included text, bytes and vector borrow the page and read as the row's; and equal
/// keys, a null among them, come in the order of the primary key.
#[test]
fn ac_ix3_a_primary_key_column_among_the_included_comes_once_in_a_copy() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create_table(&mut write, "c", &named_pairs());
        let rows = [
            ("b", 1, Some(2)),
            ("a", 1, Some(2)),
            ("a", 2, None),
            ("c", 1, Some(1)),
            ("a", 3, Some(2)),
        ];
        for (i, (name, id, n)) in rows.into_iter().enumerate() {
            let row = [
                ("name", text(name)),
                ("id", Value::I64(id)),
                ("n", n.map_or(Value::Null, Value::I32)),
                ("t", text(&format!("t{i}"))),
                (
                    "by",
                    Value::Bytes(Cow::Owned(vec![0, u8::try_from(i).unwrap()])),
                ),
                ("ve", floats(&[i as f32, 1.0])),
            ];
            write.insert_row(&table, &row).unwrap();
        }
        let definition = index("ix", &[key("n"), key("id").descending()])
            .with_included(&["t", "name", "by", "ve"]);
        write
            .create_index(&table, &definition, IndexMode::CreateNew)
            .unwrap();
        write.commit().unwrap();

        let read = store.read().unwrap();
        let table = read.open_table("c").unwrap();
        let described = read.describe_table("c").unwrap();
        let included: Vec<&str> = (described.index("ix").unwrap().included.iter())
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(included, ["t", "by", "ve"]);
        let entries: Vec<krummi::IndexEntry<'_>> = (read.index_entries(&table, "ix", &[], ..))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let order: Vec<(String, i64)> = entries
            .iter()
            .map(
                |entry| match (entry.get("name").unwrap(), entry.get("id").unwrap()) {
                    (Value::String(name), Value::I64(id)) => (name.into_owned(), id),
                    other => panic!("{other:?}"),
                },
            )
            .collect();
        let expected =
            [("a", 2), ("c", 1), ("a", 3), ("a", 1), ("b", 1)].map(|(n, id)| (n.to_owned(), id));
        assert_eq!(order, expected);
        for entry in &entries {
            let names: Vec<&str> = (0..entry.len()).map(|i| entry.name(i).unwrap()).collect();
            assert_eq!(names, ["n", "id", "t", "by", "ve", "name"]);
            assert!(matches!(entry.value(5), Some(Value::String(Cow::Owned(_)))));
            assert!(matches!(
                entry.get("t"),
                Some(Value::String(Cow::Borrowed(_)))
            ));
            assert!(matches!(
                entry.get("by"),
                Some(Value::Bytes(Cow::Borrowed(_)))
            ));
            assert!(matches!(
                entry.get("ve"),
                Some(Value::Vector(Cow::Borrowed(_)))
            ));
            let key = [entry.get("name").unwrap(), entry.get("id").unwrap()];
            let row = read.get_row(&table, &key).unwrap().unwrap();
            for name in ["n", "t", "by", "ve"] {
                assert!(
                    same(&entry.get(name).unwrap(), &row.get(name).unwrap()),
                    "{name}"
                );
            }
        }
        assert_eq!(format!("{:?}", entries[0]).matches("\"name\"").count(), 1);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC-IX7 and IX11: a creation that fails changes nothing

/// The rows of AC-IX7: `n` repeats 1 (rows 5 and 2) and 0 (rows 9 and 7), and is null in rows 3 and 4; `b` is true
/// in the rows of odd keys.
fn repeating_rows() -> Vec<Vec<(&'static str, Value<'static>)>> {
    [
        person(5, Some(1), "e"),
        person(2, Some(1), "b"),
        person(9, Some(0), "i"),
        person(7, Some(0), "g"),
        person(3, None, "c"),
        person(4, None, "d"),
        person(1, Some(8), "a"),
    ]
    .into_iter()
    .map(|mut row| {
        let Value::I64(id) = row[0].1 else {
            unreachable!()
        };
        row.push(("b", Value::Bool(id % 2 == 1)));
        row
    })
    .collect()
}

type Row = Vec<(&'static str, Value<'static>)>;

/// The index that every store of AC-IX7 has besides the one it tries: `n` descending with its nulls last.
fn other_index() -> IndexDefinition {
    index("other", &[key("n").descending().nulls_last()])
}

/// Commits the table `t` of [`repeating_rows`] and `extra` with [`other_index`]; then runs `attempt` on it in a write
/// that also creates the table `u` with a row and commits, after which `t` has the schema, the indexes and the rows it
/// had before. Returns the store's information and the entries of [`other_index`], which do not depend on the random
/// identities.
fn after_a_write(
    store: &Store,
    extra: &[Row],
    attempt: impl FnOnce(&mut WriteTransaction<'_>, &WriteTable<'_>),
) -> (krummi::StoreInfo, Vec<StoredIndexEntry>) {
    let rows: Vec<Row> = repeating_rows()
        .into_iter()
        .chain(extra.iter().cloned())
        .collect();
    commit_people(store, &rows);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .create_index(&table, &other_index(), IndexMode::CreateNew)
        .unwrap();
    write.commit().unwrap();
    let before = store.read().unwrap().describe_table("t").unwrap();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let rows_before = rows_of(&write, &table);
    attempt(&mut write, &table);
    assert_eq!(rows_of(&write, &table), rows_before);
    let other = create_table(&mut write, "u", &people());
    write.insert_row(&other, &person(1, Some(1), "x")).unwrap();
    write.commit().unwrap();
    assert_eq!(store.read().unwrap().describe_table("t").unwrap(), before);
    (store.info(), read_entries(store, "other"))
}

/// AC-IX7 and IX11 (IX10, H15), in memory: creating a unique index over rows that repeat its key fails with a
/// duplicate key, with the table, the index, the repeated values and the primary key of the second of the first two
/// rows that repeat them in the index's order; an entry too large fails with its size, the first row's in the order
/// of the primary key, also when rows repeat the key of the unique index (AC-IX9); and the write commits its other
/// changes with the catalog, the table, its other index, the pages in use and free and the store's information as
/// they would be without the attempt.
#[test]
fn ac_ix7_a_creation_that_fails_leaves_the_store_as_without_it() {
    stores(|_, folder| {
        let (twin, _twin_dir) = store_like(folder);
        let without = after_a_write(&twin, &[], |_, _| {});
        let (store, dir) = store_like(folder);
        let folder = dir.as_ref().map(tempfile::TempDir::path);
        let with = after_a_write(&store, &[], |write, table| {
            let unique = index("ix_n", &[key("n")]).unique();
            let error = write
                .create_index(table, &unique, IndexMode::CreateNew)
                .unwrap_err();
            assert_eq!(
                duplicate(error, folder),
                ("t".to_owned(), "(9)".to_owned(), some("ix_n"), some("(0)"))
            );
            // Descending, the first two rows that repeat a key are those of 1.
            let descending = index("ix_n", &[key("n").descending()]).unique();
            let error = write
                .create_index(table, &descending, IndexMode::CreateIfNotExists)
                .unwrap_err();
            assert_eq!(
                duplicate(error, folder),
                ("t".to_owned(), "(5)".to_owned(), some("ix_n"), some("(1)"))
            );
            // The values of every key column that repeat.
            let two = index("ix_nb", &[key("n"), key("b")]).unique();
            let error = write
                .create_index(table, &two, IndexMode::CreateNew)
                .unwrap_err();
            assert_eq!(
                duplicate(error, folder),
                (
                    "t".to_owned(),
                    "(9)".to_owned(),
                    some("ix_nb"),
                    some("(0, true)")
                )
            );
        });
        assert_eq!(with, without);
        drop(store);

        // Rows 6 and 8 have a text whose entry in an index on `s` has 2,051 and 2,050 bytes; row 10 repeats the text
        // of row 5.
        let long = |id, len| person(id, Some(3), &"y".repeat(len));
        let extra = [long(8, 2046), long(6, 2047), person(10, Some(4), "e")];
        let (twin, _twin_dir) = store_like(folder);
        let without = after_a_write(&twin, &extra, |_, _| {});
        let (store, dir) = store_like(folder);
        let folder = dir.as_ref().map(tempfile::TempDir::path);
        let with = after_a_write(&store, &extra, |write, table| {
            for definition in [
                index("ix_s", &[key("s")]),
                index("ix_s", &[key("s")]).unique(),
            ] {
                let error = write
                    .create_index(table, &definition, IndexMode::CreateNew)
                    .unwrap_err();
                assert_eq!(
                    entry_too_large(error, folder),
                    ("t".to_owned(), "ix_s".to_owned(), 2051)
                );
            }
        });
        assert_eq!(with, without);
    });
}

/// IX7 and N7 at creation: rows with a null in a column of a unique index do not collide, and the index has their
/// entries.
#[test]
fn nulls_never_collide_in_a_unique_index() {
    stores(|store, _| {
        let rows = repeating_rows();
        commit_people(store, &rows[4..]);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let definition = index("ix_n", &[key("n"), key("m")]).unique();
        write
            .create_index(&table, &definition, IndexMode::CreateNew)
            .unwrap();
        let keys: Vec<Vec<Value<'static>>> = entries(&write, &table, "ix_n")
            .into_iter()
            .map(|e| e.key)
            .collect();
        assert_eq!(
            keys,
            [
                vec![Value::Null, Value::Null],
                vec![Value::Null, Value::Null],
                vec![Value::I32(8), Value::Null],
            ]
        );
        write.commit().unwrap();
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC-IX9: definitions, names and their errors

/// The definitions of IX3 that fail on [`people`] with `by_n` on `n`, with the column and the rule of each, in IX3's
/// order; when one breaks two rules, the first.
fn invalid_definitions() -> Vec<(IndexDefinition, Option<&'static str>, SchemaRule)> {
    use SchemaRule::*;
    let many: Vec<IndexKey> = ["n", "m", "s", "a", "b"]
        .iter()
        .cycle()
        .take(35)
        .map(|c| key(c))
        .collect();
    vec![
        (index("x", &[]), None, NoIndexKey),
        (index("x", &[]).with_included(&["zz"]), None, NoIndexKey),
        (index("x", &[key("zz")]), Some("zz"), ColumnNotFound),
        (
            index("x", &[key("n"), key("zz")]).with_included(&["yy"]),
            Some("zz"),
            ColumnNotFound,
        ),
        (
            index("x", &[key("n"), key("n")]).with_included(&["yy"]),
            Some("yy"),
            ColumnNotFound,
        ),
        (
            index("x", &[key("ve"), key("zz")]),
            Some("zz"),
            ColumnNotFound,
        ),
        (
            index("x", &[key("m"), key("ve")]),
            Some("ve"),
            VectorIndexKey,
        ),
        (
            index("x", &[key("ve"), key("ve")]),
            Some("ve"),
            VectorIndexKey,
        ),
        (
            index("x", &[key("m"), key("s"), key("m")]),
            Some("m"),
            RepeatedIndexColumn,
        ),
        (
            index("x", &[key("m")]).with_included(&["s", "s"]),
            Some("s"),
            RepeatedIndexColumn,
        ),
        (
            index("x", &[key("m")]).with_included(&["s", "m"]),
            Some("m"),
            RepeatedIndexColumn,
        ),
        (
            index("x", &[key("m"), key("id")]).with_included(&["id"]),
            Some("id"),
            RepeatedIndexColumn,
        ),
        (
            index("x", &[key("m")]).with_included(&["ve", "ve"]),
            Some("ve"),
            RepeatedIndexColumn,
        ),
        (index("x", &many), Some("n"), RepeatedIndexColumn),
        (index("x", &[key("id")]), None, RedundantIndexKey),
        (
            index("x", &[key("id").nulls_last()]).unique(),
            None,
            RedundantIndexKey,
        ),
        (
            index("x", &[key("id")]).with_included(&["s"]),
            None,
            RedundantIndexKey,
        ),
        (index("x", &[key("n")]), None, SameIndexKey),
        (
            index("x", &[key("n")]).unique().with_included(&["s"]),
            None,
            SameIndexKey,
        ),
    ]
}

/// AC-IX9 (IX3): each definition that breaks a rule of IX3 fails without effect with an invalid schema, the table,
/// the index, the column if there is one and the first rule it breaks, in Create new and in Create if not exists; the
/// definitions next to them work.
#[test]
fn ac_ix9_definitions_that_break_ix3_fail_with_their_rule() {
    stores(|store, folder| {
        commit_people(store, &[person(1, Some(1), "a")]);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write
            .create_index(&table, &index("by_n", &[key("n")]), IndexMode::CreateNew)
            .unwrap();
        let before = write.table_schema(&table).unwrap();
        for (definition, column, rule) in invalid_definitions() {
            for mode in [IndexMode::CreateNew, IndexMode::CreateIfNotExists] {
                let error = write.create_index(&table, &definition, mode).unwrap_err();
                assert_eq!(
                    invalid_schema(error, folder),
                    ("t".to_owned(), some("x"), column.map(str::to_owned), rule),
                    "{definition:?} {mode:?}"
                );
            }
        }
        assert_eq!(write.table_schema(&table).unwrap(), before, "no effect");
        // Next to them: other directions and null orders make other keys, and a primary key column may be in a key
        // that is not along the primary key.
        let valid = [
            index("v1", &[key("n").descending()]),
            index("v2", &[key("n").nulls_last()]),
            index("v3", &[key("id").descending()]),
            index("v4", &[key("id"), key("n")]),
            index("v5", &[key("n"), key("id")]).with_included(&["ve", "s"]),
        ];
        for definition in &valid {
            write
                .create_index(&table, definition, IndexMode::CreateNew)
                .unwrap();
        }
        write.commit().unwrap();
        let after = store.read().unwrap().describe_table("t").unwrap();
        assert_eq!(index_names(&after), ["by_n", "v1", "v2", "v3", "v4", "v5"]);
    });
}

/// AC-IX9 (IX3): on a primary key of two columns, a key along it (its columns in its order, all ascending) is refused
/// unless it is a strict prefix in a unique index, whatever its nulls and included columns; the whole primary key is
/// refused also unique; other orders and other columns are accepted.
#[test]
fn ac_ix9_a_key_along_the_primary_key() {
    stores(|store, folder| {
        let schema = Schema::new(
            vec![
                Column::new("a", ColumnType::I32).not_null(),
                Column::new("b", ColumnType::I32).not_null(),
                Column::new("c", ColumnType::I32),
            ],
            &["a", "b"],
        );
        let mut write = store.write().unwrap();
        let table = create_table(&mut write, "t", &schema);
        let refused = [
            index("x", &[key("a")]),
            index("x", &[key("a").nulls_last()]),
            index("x", &[key("a")]).with_included(&["c"]),
            index("x", &[key("a"), key("b")]),
            index("x", &[key("a"), key("b")]).unique(),
        ];
        for definition in &refused {
            let error = write
                .create_index(&table, definition, IndexMode::CreateNew)
                .unwrap_err();
            assert_eq!(
                invalid_schema(error, folder),
                (
                    "t".to_owned(),
                    some("x"),
                    None,
                    SchemaRule::RedundantIndexKey
                ),
                "{definition:?}"
            );
        }
        let accepted = [
            index("p1", &[key("a")]).unique(),
            index("p2", &[key("a").descending()]),
            index("p3", &[key("b")]),
            index("p4", &[key("a"), key("c")]),
            index("p5", &[key("a"), key("b"), key("c")]),
            index("p6", &[key("b"), key("a")]),
        ];
        for definition in &accepted {
            write
                .create_index(&table, definition, IndexMode::CreateNew)
                .unwrap();
        }
        assert_eq!(write.table_schema(&table).unwrap().indexes().len(), 6);
        write.commit().unwrap();
    });
}

/// AC-IX9 (IX1, IX3, IX10): an index name of 0 or 1,025 bytes fails with an invalid size of an index name, also from
/// the handle of a deleted table; names of 1 and 1,024 bytes work; a table takes 64 indexes and refuses the 65th; and
/// an index takes 32 key columns and refuses 33.
#[test]
fn ac_ix9_names_and_counts_at_their_limits() {
    stores(|store, folder| {
        let columns: Vec<Column> = std::iter::once(Column::new("id", ColumnType::I64).not_null())
            .chain((0..70).map(|i| Column::new(format!("c{i}"), ColumnType::I32)))
            .collect();
        let mut write = store.write().unwrap();
        let table = create_table(&mut write, "t", &Schema::new(columns, &["id"]));
        let create = |write: &mut WriteTransaction<'_>, name: &str, key: Vec<IndexKey>| {
            write.create_index(
                &table,
                &IndexDefinition::new(name, key),
                IndexMode::CreateNew,
            )
        };
        for len in [0, 1025] {
            let name = "x".repeat(len);
            let error = create(&mut write, &name, vec![key("c0")]).unwrap_err();
            assert_eq!(
                invalid_size(error, folder),
                (SizeSubject::IndexName, len as u64, 1, 1024)
            );
            let error = write.drop_index(&table, &name).unwrap_err();
            assert_eq!(
                invalid_size(error, folder),
                (SizeSubject::IndexName, len as u64, 1, 1024)
            );
        }
        let keys = |n: usize| -> Vec<IndexKey> { (0..n).map(|i| key(&format!("c{i}"))).collect() };
        let error = create(&mut write, "wide", keys(33)).unwrap_err();
        assert_eq!(
            invalid_schema(error, folder),
            (
                "t".to_owned(),
                some("wide"),
                None,
                SchemaRule::TooManyIndexKeyColumns
            )
        );
        create(&mut write, "wide", keys(32)).unwrap();
        create(&mut write, "x", vec![key("c1")]).unwrap();
        create(&mut write, &"y".repeat(1024), vec![key("c2")]).unwrap();
        for i in 3..64 {
            create(&mut write, &format!("i{i:02}"), vec![key(&format!("c{i}"))]).unwrap();
        }
        assert_eq!(write.table_schema(&table).unwrap().indexes().len(), 64);
        let error = create(&mut write, "i64", vec![key("c64")]).unwrap_err();
        assert_eq!(
            invalid_schema(error, folder),
            (
                "t".to_owned(),
                some("i64"),
                None,
                SchemaRule::TooManyIndexes
            )
        );
        write.commit().unwrap();

        // From the handle of a table this write deleted: the size of the name, then the deleted table.
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.delete_table("t").unwrap();
        let long = index(&"x".repeat(1025), &[key("c0")]);
        let error = write
            .create_index(&table, &long, IndexMode::CreateNew)
            .unwrap_err();
        assert_eq!(invalid_size(error, folder).0, SizeSubject::IndexName);
        for mode in [IndexMode::CreateNew, IndexMode::CreateIfNotExists] {
            let error = write
                .create_index(&table, &index("x", &[key("c0")]), mode)
                .unwrap_err();
            assert_eq!(not_found(error, folder), "t");
        }
        assert_eq!(
            not_found(write.drop_index(&table, "x").unwrap_err(), folder),
            "t"
        );
        write.commit().unwrap();
    });
}

/// AC-IX9 (IX2, IX10): Create new over a name that exists fails with tree already exists, before the definition is
/// checked; Create if not exists over the same definition does nothing, also when either gives primary key columns
/// among its included ones, which Describe does not show; over one that differs, in each part of IX2 alone and in two
/// at once, it fails with a different schema and the first difference, also when the definition given is not valid;
/// and over the same definition once its primary key columns are left out, but invalid as given, with an invalid
/// schema.
#[test]
fn ac_ix9_create_new_and_create_if_not_exists_over_an_existing_name() {
    stores(|store, folder| {
        commit_people(store, &[person(1, Some(1), "a")]);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let existing = index("ix", &[key("n"), key("s").descending().nulls_last()])
            .unique()
            .with_included(&["id", "a", "b"]);
        write
            .create_index(&table, &existing, IndexMode::CreateNew)
            .unwrap();
        write
            .create_index(&table, &index("plain", &[key("m")]), IndexMode::CreateNew)
            .unwrap();
        let before = write.table_schema(&table).unwrap();
        let ix = before.index("ix").unwrap();
        let included: Vec<&str> = ix.included.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(included, ["a", "b"], "the primary key column is left out");

        for definition in [&existing, &index("ix", &[key("zz")])] {
            let error = write
                .create_index(&table, definition, IndexMode::CreateNew)
                .unwrap_err();
            assert_eq!(
                index_exists(error, folder),
                ("t".to_owned(), "ix".to_owned())
            );
        }
        for same in [
            existing.clone(),
            existing.clone().with_included(&["a", "b"]),
            existing.clone().with_included(&["a", "id", "b"]),
        ] {
            write
                .create_index(&table, &same, IndexMode::CreateIfNotExists)
                .unwrap();
        }
        assert_eq!(write.table_schema(&table).unwrap(), before, "nothing done");

        let at = |name: &str, direction: &str, nulls: &str, position: usize| {
            format!("{name:?} {direction} {nulls} at position {position}")
        };
        let with_key = |key: &[IndexKey]| IndexDefinition {
            key: key.to_vec(),
            ..existing.clone()
        };
        let not_unique = |definition: IndexDefinition| IndexDefinition {
            unique: false,
            ..definition
        };
        use SchemaDifference::{IndexIncluded, IndexKey as Key, IndexUniqueness};
        let cases: Vec<(IndexDefinition, SchemaDifference, String, String)> = vec![
            (
                not_unique(existing.clone()),
                IndexUniqueness,
                "not unique".into(),
                "unique".into(),
            ),
            (
                with_key(&[key("m"), key("s").descending().nulls_last()]),
                Key,
                at("m", "ascending", "nulls first", 1),
                at("n", "ascending", "nulls first", 1),
            ),
            (
                with_key(&[key("n"), key("s").nulls_last()]),
                Key,
                at("s", "ascending", "nulls last", 2),
                at("s", "descending", "nulls last", 2),
            ),
            (
                with_key(&[key("n"), key("s").descending()]),
                Key,
                at("s", "descending", "nulls first", 2),
                at("s", "descending", "nulls last", 2),
            ),
            (
                with_key(&[key("n")]),
                Key,
                "1 column".into(),
                "2 columns".into(),
            ),
            (
                with_key(&[key("n"), key("s").descending().nulls_last(), key("m")]),
                Key,
                "3 columns".into(),
                "2 columns".into(),
            ),
            (
                existing.clone().with_included(&["b", "a"]),
                IndexIncluded,
                "\"b\" at position 1".into(),
                "\"a\" at position 1".into(),
            ),
            (
                existing.clone().with_included(&["id", "a"]),
                IndexIncluded,
                "1 column".into(),
                "2 columns".into(),
            ),
            // Two differences at once: the first in IX2's order.
            (
                not_unique(with_key(&[key("m")])),
                IndexUniqueness,
                "not unique".into(),
                "unique".into(),
            ),
            (
                with_key(&[key("n"), key("s"), key("m")]),
                Key,
                at("s", "ascending", "nulls first", 2),
                at("s", "descending", "nulls last", 2),
            ),
            (
                with_key(&[key("n")]).with_included(&["b"]),
                Key,
                "1 column".into(),
                "2 columns".into(),
            ),
            // A definition that differs and is not valid: the difference.
            (
                with_key(&[key("zz")]),
                Key,
                at("zz", "ascending", "nulls first", 1),
                at("n", "ascending", "nulls first", 1),
            ),
        ];
        for (definition, difference, expected, found) in cases {
            let error = write
                .create_index(&table, &definition, IndexMode::CreateIfNotExists)
                .unwrap_err();
            assert_eq!(
                different_schema(error, folder),
                ("t".to_owned(), some("ix"), difference, expected, found),
                "{definition:?}"
            );
        }
        // The same definition once the primary key columns are left out, invalid as given.
        let repeated = index("plain", &[key("m")]).with_included(&["id", "id"]);
        let error = write
            .create_index(&table, &repeated, IndexMode::CreateIfNotExists)
            .unwrap_err();
        assert_eq!(
            invalid_schema(error, folder),
            (
                "t".to_owned(),
                some("plain"),
                some("id"),
                SchemaRule::RepeatedIndexColumn
            )
        );
        assert_eq!(write.table_schema(&table).unwrap(), before, "no effect");
        write.commit().unwrap();
    });
}

/// A column of bytes with a default of `len` bytes.
fn bytes_default(name: &str, len: usize) -> Column {
    Column::new(name, ColumnType::Bytes).with_default(Value::Bytes(Cow::Owned(vec![42; len])))
}

/// AC-IX9 (IX3, E5's schema too large): with the schema of a table a few bytes under the most of its catalog entry,
/// as the test hook measures it, an index whose stored definition takes them is created and fills the entry, one
/// whose name is a byte longer fails with a schema too large naming the index and no column, and then adding a column
/// fails with it too, both without effect. A temporary store, since the size of the entry does not depend on where
/// the store is.
#[test]
fn ac_ix9_a_schema_near_the_most_of_its_catalog_entry() {
    use krummi_tree::catalog::MAX_EXTRA;
    let store = Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let probe = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I32),
            bytes_default("c", 1),
        ],
        &["id"],
    );
    let probe = create_table(&mut write, "probe", &probe);
    let small = write.table_schema(&probe).unwrap().stored_len();
    write
        .create_index(&probe, &index("ix", &[key("n")]), IndexMode::CreateNew)
        .unwrap();
    let ix_len = write.table_schema(&probe).unwrap().stored_len() - small;
    write.delete_table("probe").unwrap();
    // The schema leaves exactly the bytes of `ix`.
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I32),
            bytes_default("c", 1 + MAX_EXTRA - small - ix_len),
        ],
        &["id"],
    );
    let table = create_table(&mut write, "t", &schema);
    drop(schema);
    let before = write.table_schema(&table).unwrap();
    let error = write
        .create_index(&table, &index("ixx", &[key("n")]), IndexMode::CreateNew)
        .unwrap_err();
    assert_eq!(
        invalid_schema(error, None),
        (
            "t".to_owned(),
            some("ixx"),
            None,
            SchemaRule::SchemaTooLarge
        )
    );
    assert_eq!(write.table_schema(&table).unwrap(), before, "no effect");
    drop(before);
    write
        .create_index(&table, &index("ix", &[key("n")]), IndexMode::CreateNew)
        .unwrap();
    let full = write.table_schema(&table).unwrap();
    assert_eq!(full.stored_len(), MAX_EXTRA, "the entry is full");
    let error = write
        .add_column(&table, &Column::new("d", ColumnType::Bool))
        .unwrap_err();
    assert_eq!(
        invalid_schema(error, None),
        ("t".to_owned(), None, some("d"), SchemaRule::SchemaTooLarge)
    );
    assert_eq!(write.table_schema(&table).unwrap(), full, "no effect");
    write.commit().unwrap();
}

// ---------------------------------------------------------------------------------------------------------------
// AC-IX10: schema changes and table operations

/// The table `t` of [`people`] with 40 rows and the indexes `ix_n` (`n`, including `s`) and `ix_ms` (`m`, `s`),
/// committed.
fn indexed(store: &Store) {
    commit_people(store, &indexed_rows(40));
    add_indexes(store);
}

/// The first `count` rows of the kind of [`indexed`].
fn indexed_rows(count: i64) -> Vec<Row> {
    (0..count)
        .map(|id| {
            let n = i32::try_from(id % 7).unwrap();
            let mut row = person(id, Some(n), &format!("s{id}"));
            row.push(("m", Value::I32(n % 3)));
            row
        })
        .collect()
}

/// Creates the indexes of [`indexed`] on the table `t` and commits.
fn add_indexes(store: &Store) {
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let ix_n = index("ix_n", &[key("n")]).with_included(&["s"]);
    write
        .create_index(&table, &ix_n, IndexMode::CreateNew)
        .unwrap();
    let ix_ms = index("ix_ms", &[key("m"), key("s")]);
    write
        .create_index(&table, &ix_ms, IndexMode::CreateNew)
        .unwrap();
    write.commit().unwrap();
}

/// The entries of every index of `table`, in the order of the indexes.
fn all_entries(write: &WriteTransaction<'_>, table: &WriteTable<'_>) -> Vec<Vec<StoredIndexEntry>> {
    let schema = write.table_schema(table).unwrap();
    schema
        .indexes()
        .iter()
        .map(|ix| entries(write, table, &ix.name))
        .collect()
}

/// AC-IX10 (IX5): renaming a column of an index and allowing nulls in it keep the index's entries and their order,
/// and Describe names it with its new name and the same identity; dropping a key column of an index, an included one,
/// or a primary key column that is in an index fails without effect with the rule of dropping a key column, naming
/// the first index in the order of their names that has it unless the column is of the primary key.
#[test]
fn ac_ix10_schema_changes_and_indexes() {
    stores(|store, folder| {
        indexed(store);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let ix_id = index("ix_id", &[key("m"), key("id")]);
        write
            .create_index(&table, &ix_id, IndexMode::CreateNew)
            .unwrap();
        let before = write.table_schema(&table).unwrap();
        let entries_before = all_entries(&write, &table);
        // `ix_id` and `ix_ms` have `m`, `ix_ms` and `ix_n` have `s`, `ix_n` has `n`.
        for (column, first) in [("m", "ix_id"), ("s", "ix_ms"), ("n", "ix_n")] {
            let error = write.drop_column(&table, column).unwrap_err();
            assert_eq!(
                invalid_schema(error, folder),
                (
                    "t".to_owned(),
                    some(first),
                    some(column),
                    SchemaRule::DropKeyColumn
                )
            );
        }
        let error = write.drop_column(&table, "id").unwrap_err();
        assert_eq!(
            invalid_schema(error, folder),
            ("t".to_owned(), None, some("id"), SchemaRule::DropKeyColumn)
        );
        assert_eq!(write.table_schema(&table).unwrap(), before, "no effect");
        // `a` is in no index.
        write.drop_column(&table, "a").unwrap();

        write.rename_column(&table, "s", "text").unwrap();
        write
            .modify_column(&table, "n", ColumnType::I32, true)
            .unwrap();
        let after = write.table_schema(&table).unwrap();
        let ix_ms = after.index("ix_ms").unwrap();
        assert_eq!(
            (ix_ms.key[1].id, ix_ms.key[1].name.as_str()),
            (before.column("s").unwrap().id, "text")
        );
        assert_eq!(after.index("ix_n").unwrap().included[0].name, "text");
        let ids = |schema: &TableSchema| -> Vec<u64> {
            schema.indexes().iter().map(|ix| ix.id).collect()
        };
        assert_eq!(ids(&after), ids(&before));
        assert_eq!(all_entries(&write, &table), entries_before);
        write.commit().unwrap();
        assert_eq!(read_entries(store, "ix_ms"), entries_before[1]);
    });
}

/// AC-IX10 (IX4, B9): dropping an index removes it and keeps the table, and another index can take its name with
/// another definition in the same write; another handle of the table in the write sees an index created and stops
/// seeing one dropped; dropping one that does not exist does nothing; and dropping indexes frees exactly their pages
/// (P12): the store then has the pages in use it had before they were created.
#[test]
fn ac_ix10_dropping_an_index() {
    stores(|store, _| {
        // Enough rows for each index to take several pages.
        commit_people(store, &indexed_rows(2000));
        let without_indexes = in_use(store);
        add_indexes(store);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let other = write.open_table("t").unwrap();
        let rows_before = rows_of(&write, &table);
        write.drop_index(&table, "ix_n").unwrap();
        assert_eq!(index_names(&write.table_schema(&other).unwrap()), ["ix_ms"]);
        assert_eq!(write.stored_index_entries(&other, "ix_n").unwrap(), None);
        write.drop_index(&table, "ix_n").unwrap();
        write.drop_index(&table, "missing").unwrap();
        let definition = index("ix_n", &[key("s").descending()]);
        write
            .create_index(&other, &definition, IndexMode::CreateNew)
            .unwrap();
        let schema = write.table_schema(&table).unwrap();
        assert_eq!(index_names(&schema), ["ix_ms", "ix_n"]);
        let ix_n = schema.index("ix_n").unwrap();
        assert_eq!(
            (ix_n.key[0].name.as_str(), ix_n.key[0].direction),
            ("s", Direction::Descending)
        );
        assert_entries(
            &entries(&write, &table, "ix_n"),
            &schema,
            ix_n,
            &rows_before,
        );
        assert_eq!(
            rows_of(&write, &table),
            rows_before,
            "the table does not change"
        );
        write.commit().unwrap();

        let with_indexes = in_use(store);
        assert!(
            with_indexes > without_indexes + 4,
            "the indexes have several pages"
        );
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.drop_index(&table, "ix_n").unwrap();
        write.drop_index(&table, "ix_ms").unwrap();
        assert_eq!(write.index_trees(&table).unwrap(), []);
        write.commit().unwrap();
        assert_eq!(
            in_use(store),
            without_indexes,
            "the indexes' pages are free"
        );
    });
}

/// AC-IX10 (IX4, E2, E3): emptying a table, with Empty and with Create or empty, empties its indexes, freeing every
/// page of their trees but the root, and keeps their definitions, their identities and the index state; renaming it keeps its indexes; deleting it, and overwriting it
/// with a rename, take its indexes with it and free their pages.
#[test]
fn ac_ix10_table_operations_and_indexes() {
    stores(|store, _| {
        // The pages of a store whose table was created and deleted, which keeps its catalog's root.
        commit_people(store, &[]);
        let mut write = store.write().unwrap();
        write.delete_table("t").unwrap();
        write.commit().unwrap();
        let empty_store = in_use(store);
        // The pages of the table with its indexes and no rows: the root of each tree.
        commit_people(store, &[]);
        add_indexes(store);
        let empty_indexed = in_use(store);
        let mut write = store.write().unwrap();
        write.delete_table("t").unwrap();
        write.commit().unwrap();
        let people = people();
        for mode in [TableMode::Empty, TableMode::CreateOrEmpty(&people)] {
            // Enough rows for each tree to take several pages.
            commit_people(store, &indexed_rows(2000));
            add_indexes(store);
            assert!(in_use(store) > empty_indexed + 6, "{mode:?}");
            let before = store.read().unwrap().describe_table("t").unwrap();
            let mut write = store.write().unwrap();
            let table = write.open_table_with("t", mode).unwrap();
            assert_eq!(write.table_schema(&table).unwrap(), before, "{mode:?}");
            assert_eq!(all_entries(&write, &table), [vec![], vec![]], "{mode:?}");
            write.commit().unwrap();
            for name in ["ix_n", "ix_ms"] {
                assert_eq!(read_entries(store, name), [], "{mode:?} {name}");
            }
            assert_eq!(store.read().unwrap().describe_table("t").unwrap(), before);
            assert_eq!(
                in_use(store),
                empty_indexed,
                "{mode:?}: the trees' pages are free"
            );
            let mut write = store.write().unwrap();
            write.delete_table("t").unwrap();
            write.commit().unwrap();
            assert_eq!(
                in_use(store),
                empty_store,
                "{mode:?}: deleted with its indexes"
            );
        }

        indexed(store);
        let before = store.read().unwrap().describe_table("t").unwrap();
        let ix_ms = read_entries(store, "ix_ms");
        let mut write = store.write().unwrap();
        write.rename_table("t", "renamed", false).unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        let renamed = read.open_table("renamed").unwrap();
        assert_eq!(read.table_schema(&renamed).unwrap(), before);
        assert_eq!(
            read.stored_index_entries(&renamed, "ix_ms").unwrap(),
            Some(ix_ms)
        );
        drop(read);

        // Overwritten by a rename, the table goes with its indexes.
        let mut write = store.write().unwrap();
        create_table(&mut write, "u", &people);
        write.rename_table("u", "renamed", true).unwrap();
        write.delete_table("renamed").unwrap();
        write.commit().unwrap();
        assert_eq!(in_use(store), empty_store, "overwritten with its indexes");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC-IX11: Describe, identities and the index state

/// A source that gives the 8-byte little-endian values of `values` in turn, then fails with input and output.
fn repeating(values: Vec<u64>) -> impl FnMut(&mut [u8]) -> krummi::Result<()> {
    let mut next = values.into_iter();
    move |bytes| match next.next() {
        Some(value) => {
            bytes.copy_from_slice(&value.to_le_bytes());
            Ok(())
        }
        None => Err(Error::Io {
            folder: Default::default(),
            source: std::io::Error::other("the test source ended"),
        }),
    }
}

/// AC-IX11 (IX6, P12): Describe lists a table's indexes as each transaction sees them, empty without any, in a read
/// and in a write, with identities and names, uniqueness, key columns with their directions and null orders and the
/// included columns; the index state changes with each creation and each drop, also of an index created and dropped
/// in one write, and not with an attempt that fails or does nothing, a rename of the table or of a column, or
/// emptying; an index keeps its identity through renames and one created again has another; an aborted write leaves
/// nothing; the schema state and version never change; and a projection prepared before keeps reading the rows.
#[test]
fn ac_ix11_describe_identities_and_the_index_state() {
    stores(|store, _| {
        let created = commit_people(store, &[person(1, Some(1), "a")]);
        assert_eq!(created.indexes(), []);
        let state0 = created.index_state();
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let projection = write.projection(&table, &["n"]).unwrap();
        let n = projection.column::<i32>(0).unwrap();
        let ix = index("ix", &[key("n").descending(), key("s").nulls_last()])
            .unique()
            .with_included(&["b", "id"]);
        write
            .create_index(&table, &ix, IndexMode::CreateNew)
            .unwrap();
        let schema = write.describe_table("t").unwrap();
        let state1 = schema.index_state();
        assert_ne!(state1, state0);
        let described = schema.index("ix").unwrap().clone();
        let column = |name: &str| schema.column(name).unwrap().id;
        assert_eq!(
            described,
            SchemaIndex {
                id: described.id,
                name: "ix".into(),
                unique: true,
                key: vec![
                    krummi::SchemaIndexKey {
                        id: column("n"),
                        name: "n".into(),
                        direction: Direction::Descending,
                        nulls: Nulls::First,
                    },
                    krummi::SchemaIndexKey {
                        id: column("s"),
                        name: "s".into(),
                        direction: Direction::Ascending,
                        nulls: Nulls::Last,
                    },
                ],
                included: vec![krummi::SchemaIndexColumn {
                    id: column("b"),
                    name: "b".into(),
                }],
            }
        );
        assert_eq!(
            (schema.version(), schema.schema_state()),
            (created.version(), created.schema_state())
        );
        // The projection prepared before reads the rows: the schema did not change (IX5, PY4).
        let row = write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
        assert_eq!(projection.bind(&row).unwrap().get(&n), Some(1));
        drop(row);
        // A read opened before the commit does not see it.
        let read = store.read().unwrap();
        assert_eq!(read.describe_table("t").unwrap(), created);
        drop(read);
        // Attempts that fail or do nothing, renames and emptying keep the state.
        write
            .create_index(&table, &ix, IndexMode::CreateIfNotExists)
            .unwrap();
        assert!(
            write
                .create_index(&table, &ix, IndexMode::CreateNew)
                .is_err()
        );
        let bad = index("bad", &[key("zz")]);
        assert!(
            write
                .create_index(&table, &bad, IndexMode::CreateNew)
                .is_err()
        );
        write.drop_index(&table, "missing").unwrap();
        write.rename_column(&table, "s", "text").unwrap();
        write.rename_table("t", "t2", false).unwrap();
        write.rename_table("t2", "t", false).unwrap();
        let table = write.open_table_with("t", TableMode::Empty).unwrap();
        let schema = write.describe_table("t").unwrap();
        assert_eq!(schema.index_state(), state1);
        assert_eq!(schema.index("ix").unwrap().id, described.id);
        assert_eq!(schema.index("ix").unwrap().key[1].name, "text");
        // An index created and dropped in the same write changes the state each time.
        write
            .create_index(&table, &index("dup", &[key("b")]), IndexMode::CreateNew)
            .unwrap();
        let state2 = write.describe_table("t").unwrap().index_state();
        write.drop_index(&table, "dup").unwrap();
        let state3 = write.describe_table("t").unwrap().index_state();
        assert!(state2 != state1 && state3 != state2 && state3 != state1);
        write.commit().unwrap();
        let read = store.read().unwrap();
        let committed = read.describe_table("t").unwrap();
        assert_eq!(
            (index_names(&committed), committed.index_state()),
            (vec!["ix"], state3)
        );
        drop(read);

        // An aborted write leaves nothing of what it drew.
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.drop_index(&table, "ix").unwrap();
        write
            .create_index(&table, &index("other", &[key("b")]), IndexMode::CreateNew)
            .unwrap();
        write.abort();
        assert_eq!(
            store.read().unwrap().describe_table("t").unwrap(),
            committed
        );

        // Dropped and created again, the index has another identity.
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.drop_index(&table, "ix").unwrap();
        let again = index("ix", &[key("n").descending(), key("text").nulls_last()]).unique();
        write
            .create_index(&table, &again, IndexMode::CreateNew)
            .unwrap();
        let again = write.describe_table("t").unwrap();
        assert_ne!(again.index("ix").unwrap().id, described.id);
        write.commit().unwrap();
    });
}

/// AC-IX11 (IX6, P12): with a generator that repeats values, an index identity and an index state equal to an
/// identity the table has used (its columns', its states', its indexes', also a dropped index's) are drawn again.
#[test]
fn ac_ix11_identities_equal_to_known_ones_are_drawn_again() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("n", ColumnType::I32),
            ],
            &["id"],
        );
        // Table 10, columns 20 and 21, schema state 30, index state 40.
        let table = with_identity_source(repeating(vec![10, 20, 21, 30, 40]), || {
            create_table(&mut write, "t", &schema)
        });
        let ix = index("ix", &[key("n")]);
        // The identity: 20, 30 and 40 are known, 50 is not; the state: 50 is the identity, 40 is known, 60 is not.
        with_identity_source(repeating(vec![20, 30, 40, 50, 50, 40, 60]), || {
            write.create_index(&table, &ix, IndexMode::CreateNew)
        })
        .unwrap();
        let described = write.table_schema(&table).unwrap();
        assert_eq!(
            (described.index("ix").unwrap().id, described.index_state()),
            (50, 60)
        );
        // Dropping it: its identity and the states are known.
        with_identity_source(repeating(vec![50, 60, 40, 70]), || {
            write.drop_index(&table, "ix")
        })
        .unwrap();
        assert_eq!(write.table_schema(&table).unwrap().index_state(), 70);
        // Created again: the dropped index's identity is still known.
        with_identity_source(repeating(vec![50, 70, 80, 90]), || {
            write.create_index(&table, &ix, IndexMode::CreateNew)
        })
        .unwrap();
        let described = write.table_schema(&table).unwrap();
        assert_eq!(
            (described.index("ix").unwrap().id, described.index_state()),
            (80, 90)
        );
        // A failing generator fails the creation, which then cannot commit (T11).
        let error = with_identity_source(repeating(vec![]), || {
            write.create_index(
                &table,
                &index("other", &[key("n").descending()]),
                IndexMode::CreateNew,
            )
        })
        .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Io);
        assert_eq!(write.table_schema(&table).unwrap(), described, "no effect");
        assert_eq!(write.commit().unwrap_err().kind(), ErrorKind::Io);
    });
}

/// AC-IX11 (IX6, design document 11): an index identity and an index state equal to a value the store knows outside
/// the table's history, the table's own identity and another table's identities, are drawn again.
#[test]
fn ac_ix11_identities_known_to_the_store_outside_the_table_are_drawn_again() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("n", ColumnType::I32),
            ],
            &["id"],
        );
        // `t`: table 10, columns 20 and 21, states 30 and 40; `u`: table 100, columns 110 and 111, states 120 and 130.
        let table = with_identity_source(repeating(vec![10, 20, 21, 30, 40]), || {
            create_table(&mut write, "t", &schema)
        });
        with_identity_source(repeating(vec![100, 110, 111, 120, 130]), || {
            create_table(&mut write, "u", &schema)
        });
        // The identity: 10 is `t`'s, 100 is `u`'s, 50 is new; the state: 60 is new.
        with_identity_source(repeating(vec![10, 100, 50, 60]), || {
            write.create_index(&table, &index("ix", &[key("n")]), IndexMode::CreateNew)
        })
        .unwrap();
        let described = write.table_schema(&table).unwrap();
        assert_eq!(
            (described.index("ix").unwrap().id, described.index_state()),
            (50, 60)
        );
        // Dropping it: 110 is a column of `u`, 70 is new.
        with_identity_source(repeating(vec![110, 70]), || write.drop_index(&table, "ix")).unwrap();
        let dropped = write.table_schema(&table).unwrap();
        assert_eq!(dropped.index_state(), 70);
        write.commit().unwrap();
        assert_eq!(store.read().unwrap().describe_table("t").unwrap(), dropped);
    });
}

/// AC-IX11 (IX6, design document 11): an index identity and an index state equal to the identity of a catalog object
/// that is not a table, a normal, a multi-value or a fixed-size tree or a sequence, are drawn again.
#[test]
fn ac_ix11_identities_of_other_catalog_objects_are_drawn_again() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("n", ColumnType::I32),
            ],
            &["id"],
        );
        let table = with_identity_source(repeating(vec![10, 20, 21, 30, 40]), || {
            create_table(&mut write, "t", &schema)
        });
        with_identity_source(repeating(vec![100, 110, 111, 120, 130]), || {
            create_table(&mut write, "u", &schema)
        });
        // The catalog's counter gives them 0, 1, 2 and 3.
        write.open_tree_with("normal", OpenMode::CreateNew).unwrap();
        write
            .open_multi_tree_with("multi", OpenMode::CreateNew)
            .unwrap();
        write
            .open_fixed_tree_with("fixed", 4, OpenMode::CreateNew)
            .unwrap();
        write
            .open_sequence_with("seq", SequenceMode::CreateNew(None))
            .unwrap();
        // The identity: 0 and 1 are trees', 50 is new; the state: 3 is the sequence's, 60 is new.
        with_identity_source(repeating(vec![0, 1, 50, 3, 60]), || {
            write.create_index(&table, &index("ix", &[key("n")]), IndexMode::CreateNew)
        })
        .unwrap();
        let described = write.table_schema(&table).unwrap();
        assert_eq!(
            (described.index("ix").unwrap().id, described.index_state()),
            (50, 60)
        );
        // Dropping it: 2 is the fixed-size tree's, 70 is new.
        with_identity_source(repeating(vec![2, 70]), || write.drop_index(&table, "ix")).unwrap();
        let dropped = write.table_schema(&table).unwrap();
        assert_eq!(dropped.index_state(), 70);
        write.commit().unwrap();
        assert_eq!(store.read().unwrap().describe_table("t").unwrap(), dropped);
    });
}

/// IX2, IX6 (P13, T11): the checks of a drawn identity against the other tables of the store copy none of their
/// defaults: with a 2 MiB default in another table, creating and dropping an index of an empty table under every
/// reservation failing in turn either succeeds or fails with out of memory and T11, and no reservation has the
/// default's size.
#[test]
fn checking_identities_copies_no_default_of_another_table() {
    use krummi_pager::testing::{self, Action, Op};
    const DEFAULT: usize = 2 << 20;
    type IndexChange = fn(&mut WriteTransaction<'_>, &WriteTable<'_>) -> krummi::Result<()>;
    let changes: [(&str, IndexChange); 2] = [
        ("create", |w, t| {
            w.create_index(t, &index("ix", &[key("n")]), IndexMode::CreateNew)
        }),
        ("drop", |w, t| w.drop_index(t, "ix")),
    ];
    stores(|store, _| {
        let small = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("n", ColumnType::I32),
            ],
            &["id"],
        );
        let large = Value::String(Cow::Owned("p".repeat(DEFAULT)));
        let other = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("p", ColumnType::String).with_default(large),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        create_table(&mut write, "t", &small);
        create_table(&mut write, "u", &other);
        write.commit().unwrap();
        for (name, change) in changes {
            let before = store.read().unwrap().describe_table("t").unwrap();
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            testing::reset();
            change(&mut write, &table).unwrap();
            let reservations = testing::count(Op::Alloc);
            testing::reset();
            drop(write);
            for nth in 0..reservations {
                let mut write = store.write().unwrap();
                let table = write.open_table("t").unwrap();
                testing::reset();
                testing::inject(
                    Op::Alloc,
                    nth,
                    Action::Fail(std::io::ErrorKind::OutOfMemory),
                );
                let changed = change(&mut write, &table);
                testing::reset();
                let Err(error) = changed else {
                    continue;
                };
                let Error::OutOfMemory { requested } = error else {
                    panic!("{name} {nth}: {error}");
                };
                assert!(requested < DEFAULT as u64, "{name} {nth}: {requested}");
                assert_eq!(
                    write.commit().unwrap_err().kind(),
                    ErrorKind::OutOfMemory,
                    "{name} {nth}: T11"
                );
                assert_eq!(
                    store.read().unwrap().describe_table("t").unwrap(),
                    before,
                    "{name} {nth}"
                );
            }
            // The change itself, for the next one.
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            change(&mut write, &table).unwrap();
            write.commit().unwrap();
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC-IX12: changes for T10

/// AC-IX12 (IX13, T10): in a write that does nothing else, creating an index, dropping one that exists, and creating
/// and dropping one in the same write are changes; Create if not exists over the same index, dropping one that does
/// not exist, every creation that fails public validation, a duplicate key and an entry too large included, and
/// scanning an index, also a scan that fails, are not. (That creating and dropping end the references read before, and
/// that a write ends what a scan lends, is checked when compiling: see the documentation of the modules that create
/// and scan indexes.)
#[test]
fn ac_ix12_what_changes_and_what_does_not() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_people(
        &store,
        &[
            person(1, Some(1), "a"),
            person(2, Some(1), &"y".repeat(2100)),
        ],
    );
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .create_index(&table, &index("ix", &[key("n")]), IndexMode::CreateNew)
        .unwrap();
    write.commit().unwrap();
    store.close().unwrap();

    let unchanged = !files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write
            .create_index(
                &table,
                &index("ix", &[key("n")]),
                IndexMode::CreateIfNotExists,
            )
            .unwrap();
        write.drop_index(&table, "missing").unwrap();
        let failures = [
            (index("ix", &[key("n")]), IndexMode::CreateNew),
            (index("ix", &[key("m")]), IndexMode::CreateIfNotExists),
            (index("bad", &[key("zz")]), IndexMode::CreateNew),
            (index(&"x".repeat(1025), &[key("n")]), IndexMode::CreateNew),
            (
                index("dup", &[key("n").descending()]).unique(),
                IndexMode::CreateNew,
            ),
            (index("big", &[key("s")]), IndexMode::CreateNew),
        ];
        let kinds: Vec<ErrorKind> = failures
            .iter()
            .map(|(definition, mode)| {
                write
                    .create_index(&table, definition, *mode)
                    .unwrap_err()
                    .kind()
            })
            .collect();
        assert_eq!(
            kinds,
            [
                ErrorKind::TreeAlreadyExists,
                ErrorKind::DifferentSchema,
                ErrorKind::InvalidSchema,
                ErrorKind::InvalidSize,
                ErrorKind::DuplicateKey,
                ErrorKind::InvalidSize,
            ]
        );
        // Scanning an index, for rows and for entries, and a scan that fails (IX13).
        let rows = write.index_rows(&table, "ix", &[], ..).unwrap().count();
        let entries = write.index_entries(&table, "ix", &[], ..).unwrap().count();
        assert_eq!((rows, entries), (2, 2));
        let missing = write.index_entries(&table, "missing", &[], ..).err();
        assert_eq!(missing.unwrap().kind(), ErrorKind::DifferentSchema);
        write.commit().unwrap();
    });
    assert!(unchanged, "nothing was a change");

    // Each in its own write, one after the other.
    type Op = fn(&mut WriteTransaction<'_>, &WriteTable<'_>);
    let changes: [(&str, Op); 3] = [
        ("create", |write, table| {
            let definition = index("new", &[key("m")]);
            write
                .create_index(table, &definition, IndexMode::CreateNew)
                .unwrap();
        }),
        ("drop", |write, table| {
            write.drop_index(table, "ix").unwrap()
        }),
        ("create and drop", |write, table| {
            let definition = index("brief", &[key("a")]);
            write
                .create_index(table, &definition, IndexMode::CreateIfNotExists)
                .unwrap();
            write.drop_index(table, "brief").unwrap();
        }),
    ];
    for (label, op) in changes {
        let changed = files_change(tmp.path(), |store| {
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            op(&mut write, &table);
            write.commit().unwrap();
        });
        assert!(changed, "{label} is a change");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// AC-IX13: damage

/// AC-IX13 and IX12, the parts of creating, dropping and Describe: with a page of an index damaged, Describe, opening
/// the table, dropping an index that does not exist and creating another index do not fail, in a read and in a write
/// that commits; dropping the damaged index fails with its damage, and the write cannot commit (T11); and with a page
/// of the table's rows damaged, creating an index fails with it.
#[test]
fn ac_ix13_damage_and_the_operations_on_indexes() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    indexed(&store);
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let ix_n = read.index_trees(&table).unwrap()[1].clone();
    assert_eq!(ix_n.0, "ix_n");
    let rows = read.table_trees(&table).unwrap().rows;
    let described = read.describe_table("t").unwrap();
    drop(read);
    store.close().unwrap();
    damage(tmp.path(), ix_n.1);

    let store = open(tmp.path());
    let read = store.read().unwrap();
    assert_eq!(read.describe_table("t").unwrap(), described);
    let table = read.open_table("t").unwrap();
    assert_eq!(read.table_schema(&table).unwrap(), described);
    assert_eq!(read.index_trees(&table).unwrap()[1], ix_n);
    drop(read);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    assert_eq!(write.describe_table("t").unwrap(), described);
    write.drop_index(&table, "missing").unwrap();
    write
        .create_index(&table, &index("ix_b", &[key("b")]), IndexMode::CreateNew)
        .unwrap();
    write.commit().unwrap();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    assert_eq!(
        damaged_page(write.drop_index(&table, "ix_n").unwrap_err()),
        ix_n.1
    );
    assert_eq!(
        write.commit().unwrap_err().kind(),
        ErrorKind::Damaged,
        "T11"
    );
    store.close().unwrap();

    // A damaged leaf of the rows: the rows are few, so their tree is its root.
    damage(tmp.path(), rows);
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let error = write
        .create_index(&table, &index("ix_a", &[key("a")]), IndexMode::CreateNew)
        .unwrap_err();
    assert_eq!(damaged_page(error), rows);
    assert_eq!(
        write.commit().unwrap_err().kind(),
        ErrorKind::Damaged,
        "T11"
    );
    store.close().unwrap();
}

/// The damage of a scan that fails: when it starts or at a step.
#[track_caller]
fn scan_damage<T>(scan: krummi::Result<impl Iterator<Item = krummi::Result<T>>>) -> u64 {
    let error = match scan {
        Err(error) => error,
        Ok(mut scan) => scan.find_map(Result::err).expect("a scan that fails"),
    };
    damaged_page(error)
}

/// AC-IX13 (IX12, T11): with the page of an index damaged, scanning it fails with the damage of that page, for entries
/// and for rows, forward and backward, in a read and in a write, which then cannot commit; another index of the table
/// scans.
#[test]
fn ac_ix13_scanning_a_damaged_index_page_fails_with_its_damage() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    indexed(&store);
    let read = store.read().unwrap();
    let ix_n = read.index_trees(&read.open_table("t").unwrap()).unwrap()[1].clone();
    assert_eq!(ix_n.0, "ix_n");
    drop(read);
    store.close().unwrap();
    damage(tmp.path(), ix_n.1);

    let store = open(tmp.path());
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    assert_eq!(
        scan_damage(read.index_entries(&table, "ix_n", &[], ..)),
        ix_n.1
    );
    assert_eq!(
        scan_damage(read.index_rows(&table, "ix_n", &[], ..)),
        ix_n.1
    );
    let back = read.index_rows(&table, "ix_n", &[], ..).map(Iterator::rev);
    assert_eq!(scan_damage(back), ix_n.1);
    let other = read.index_rows(&table, "ix_ms", &[], ..).unwrap();
    assert_eq!(other.map(Result::unwrap).count(), 40);
    drop(read);
    let write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let interval = [Value::I32(3)];
    assert_eq!(
        scan_damage(write.index_entries(&table, "ix_n", &interval, ..)),
        ix_n.1
    );
    assert_eq!(
        write.commit().unwrap_err().kind(),
        ErrorKind::Damaged,
        "T11"
    );
    store.close().unwrap();
}

// ---------------------------------------------------------------------------------------------------------------
// AC-IX15: the format

/// AC-IX15 (IX14, B12, F11): a new store has format 12; a store of format 11, the one before the indexes, is of a
/// previous format, even without indexes, and opening it leaves it as it was; and the store's information keeps its
/// shape, with the indexes' pages in its totals.
#[test]
fn ac_ix15_a_store_of_format_11_is_of_a_previous_format() {
    use krummi_pager::DATA_FILE;
    use krummi_pager::format::{Header, PAGE_SIZE, header_copies, write_header_pages};
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_people(&store, &[person(1, Some(1), "a")]);
    let without = in_use(&store);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .create_index(&table, &index("ix", &[key("n")]), IndexMode::CreateNew)
        .unwrap();
    write.commit().unwrap();
    let krummi::StoreInfo {
        format_version,
        page_size,
        size_bytes: _,
        page_count,
        max_size: _,
        temporary,
        free_pages: _,
    } = store.info();
    assert_eq!((format_version, page_size, temporary), (12, 8192, false));
    assert!(in_use(&store) > without, "the index's pages count");
    assert!(page_count >= in_use(&store));
    store.close().unwrap();
    // A store without indexes, with a table and without one.
    let without_indexes = tempfile::tempdir().unwrap();
    let store = open(without_indexes.path());
    commit_people(&store, &[person(1, Some(1), "a")]);
    store.close().unwrap();
    let empty = tempfile::tempdir().unwrap();
    open(empty.path()).close().unwrap();

    for folder in [tmp.path(), without_indexes.path(), empty.path()] {
        let data = folder.join(DATA_FILE);
        let mut bytes = std::fs::read(&data).unwrap();
        let [Some(a), Some(b)] = header_copies(&bytes[..2 * PAGE_SIZE]) else {
            panic!("both header copies")
        };
        assert_eq!((a.format_version, b.format_version), (12, 12));
        let current = if a.revision >= b.revision { a } else { b };
        write_header_pages(
            &mut bytes[..2 * PAGE_SIZE],
            &Header {
                format_version: 11,
                ..current
            },
        );
        std::fs::write(&data, &bytes).unwrap();
        match Store::open(folder, &krummi::StoreOptions::default()).err() {
            Some(Error::DifferentFormat {
                found, expected, ..
            }) => assert_eq!((found, expected), (11, 12)),
            other => panic!("expected a different format, got {other:?}"),
        }
        assert_eq!(std::fs::read(&data).unwrap(), bytes, "left as it was");
    }
    let new = Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
    assert_eq!(new.info().format_version, 12);
}

// ---------------------------------------------------------------------------------------------------------------
// AC-IX2, AC-IX6, AC-IX8 and AC-IX9: the indexes in every row write

/// The row `id` of [`people`] for the writes: `n` is `id % 5`, `m` is `id % 3`, `s` is `s{id}`, `a` is `id`, `b`
/// whether `id` is even and `ve` is `[id, 1]`.
fn member(id: i64) -> Row {
    let small = |k: i64| Value::I32(i32::try_from(id % k).unwrap());
    vec![
        ("id", Value::I64(id)),
        ("n", small(5)),
        ("m", small(3)),
        ("s", text(&format!("s{id}"))),
        ("a", Value::I64(id)),
        ("b", Value::Bool(id % 2 == 0)),
        ("ve", floats(&[id as f32, 1.0])),
    ]
}

/// A vector of `f32` elements.
fn floats(elements: &[f32]) -> Value<'static> {
    Value::Vector(Cow::Owned(
        elements.iter().flat_map(|x| x.to_le_bytes()).collect(),
    ))
}

/// The indexes of [`kept`]: `ix_n` on `n` including `s`, `ix_ms` on `m` descending with its nulls last and `s`, and
/// `ux_a`, unique on `a` and including `b`.
fn kept_indexes() -> Vec<IndexDefinition> {
    vec![
        index("ix_n", &[key("n")]).with_included(&["s"]),
        index("ix_ms", &[key("m").descending().nulls_last(), key("s")]),
        index("ux_a", &[key("a")]).unique().with_included(&["b"]),
    ]
}

/// Commits the table `t` of [`people`] with the rows 0 to 29 of [`member`], then the column `extra i32` with the
/// default 7 and the rows 30 to 39, so that the first 30 rows are of an older schema version, and the indexes of
/// [`kept_indexes`].
fn kept(store: &Store) {
    commit_people(store, &(0..30).map(member).collect::<Vec<_>>());
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let extra = Column::new("extra", ColumnType::I32).with_default(Value::I32(7));
    write.add_column(&table, &extra).unwrap();
    for id in 30..40 {
        write.insert_row(&table, &member(id)).unwrap();
    }
    for definition in kept_indexes() {
        write
            .create_index(&table, &definition, IndexMode::CreateNew)
            .unwrap();
    }
    write.commit().unwrap();
}

/// Each row of a scan, as its columns' names and owned values.
fn owned_rows<'t>(
    rows: impl Iterator<Item = krummi::Result<krummi::TableRow<'t>>>,
) -> Vec<Vec<(String, Value<'static>)>> {
    rows.map(|row| {
        let row = row.unwrap();
        row.columns()
            .map(|(name, value)| (name.to_owned(), value.into_owned()))
            .collect()
    })
    .collect()
}

/// Asserts that each index of `table` has exactly the entries of the table's rows, in the order of N5, as the write
/// sees them.
#[track_caller]
fn assert_in_step(write: &WriteTransaction<'_>, table: &WriteTable<'_>, what: &str) {
    let schema = write.table_schema(table).unwrap();
    let rows = rows_of(write, table);
    assert!(!schema.indexes().is_empty(), "{what}: indexes");
    for ix in schema.indexes() {
        assert_entries(&entries(write, table, &ix.name), &schema, ix, &rows);
    }
}

/// The rows and the entries of every index of the table `t` that a new read sees, with their keys' bytes.
#[allow(clippy::type_complexity)]
fn snapshot(
    store: &Store,
) -> (
    Vec<Vec<(String, Value<'static>)>>,
    Vec<(String, Vec<StoredIndexEntry>)>,
) {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let rows = owned_rows(read.range_rows(&table, &[], ..).unwrap());
    let indexes = (read.table_schema(&table).unwrap().indexes().iter())
        .map(|ix| {
            let entries = read
                .stored_index_entries(&table, &ix.name)
                .unwrap()
                .unwrap();
            (ix.name.clone(), entries)
        })
        .collect();
    (rows, indexes)
}

/// Asserts that each index of the table `t` has exactly the entries of its rows, as a new read sees them.
#[track_caller]
fn assert_read_in_step(store: &Store) {
    let schema = store.read().unwrap().describe_table("t").unwrap();
    let (rows, indexes) = snapshot(store);
    for (ix, (name, entries)) in schema.indexes().iter().zip(&indexes) {
        assert_eq!(&ix.name, name);
        assert_entries(entries, &schema, ix, &rows);
    }
}

/// Runs every kind of row write on the table `t` of [`kept`] in `write`, checking after each that the indexes are in
/// step with the rows; with `create`, first creates `ix_new` (`b`, then `n` descending, including `ve`), so that the
/// writes also keep an index created in the same write. Rounds 0 and 1 write different rows, so that one can follow
/// the other.
fn every_row_write(
    write: &mut WriteTransaction<'_>,
    table: &WriteTable<'_>,
    create: bool,
    round: i64,
) {
    if create {
        let ix_new = index("ix_new", &[key("b"), key("n").descending()]).with_included(&["ve"]);
        write
            .create_index(table, &ix_new, IndexMode::CreateNew)
            .unwrap();
    }
    assert_in_step(write, table, "before");
    // Old rows, of the older schema version, and new ones.
    let old = |id: i64| Value::I64(id + 20 * round);
    let new = |id: i64| id + 100 * round;
    // Rows with a null in `a`, the key of a unique index, do not collide (N7).
    for id in [new(40), new(41), new(42)] {
        let mut row = member(id);
        if id != new(40) {
            row[4].1 = Value::Null;
        }
        write.insert_row(table, &row).unwrap();
    }
    assert_in_step(write, table, "insert");
    let current = |id: i64| id + round;
    let updates: [(&str, Row); 6] = [
        // Only a column of no index, in a row of an older schema version.
        ("no index", vec![("id", old(3)), ("extra", Value::I32(1))]),
        // Only an included column: `ve` of `ix_new`, when it is there; and `b`, included in `ux_a` (and a key column
        // of `ix_new`).
        (
            "included",
            vec![("id", old(4)), ("ve", floats(&[9.0, 9.0]))],
        ),
        (
            "included b",
            vec![
                ("id", Value::I64(current(33))),
                ("b", Value::Bool(current(33) % 2 == 1)),
            ],
        ),
        // A key column of `ix_n`, in an older row and in a row of the current version (a fixed-width patch).
        ("key", vec![("id", old(5)), ("n", Value::I32(4))]),
        (
            "key, current version",
            vec![
                ("id", Value::I64(current(35))),
                ("n", Value::I32(2)),
                ("a", Value::I64(100 + current(35))),
            ],
        ),
        // A text that is a key column of `ix_ms` and included in `ix_n`.
        ("text", vec![("id", old(6)), ("s", text("changed"))]),
    ];
    for (what, row) in updates {
        assert!(write.update_row(table, &row).unwrap(), "{what}");
        assert_in_step(write, table, what);
    }
    // An identical update, and an update of a row that is not there.
    let identical = [("id", old(7)), ("n", Value::I32(2))];
    assert!(write.update_row(table, &identical).unwrap());
    let missing = [("id", Value::I64(99)), ("n", Value::I32(1))];
    assert!(!write.update_row(table, &missing).unwrap());
    assert_in_step(write, table, "identical and missing");
    let mut upserted = member(8 + 20 * round);
    upserted[3].1 = text("upserted");
    upserted[2].1 = Value::Null;
    assert_eq!(
        write.upsert_row(table, &upserted).unwrap(),
        Upserted::Updated
    );
    let inserted = write.upsert_row(table, &member(new(50))).unwrap();
    assert_eq!(inserted, Upserted::Inserted);
    assert_in_step(write, table, "upsert");
    assert!(write.delete_row(table, &[old(9)]).unwrap());
    assert!(!write.delete_row(table, &[Value::I64(99)]).unwrap());
    assert_in_step(write, table, "delete");
    let range = old(10)..old(20);
    assert_eq!(write.delete_range(table, &[], range).unwrap(), 10);
    assert_eq!(
        write.delete_range(table, &[], Value::I64(300)..).unwrap(),
        0
    );
    assert_in_step(write, table, "delete range");
}

/// AC-IX2 (IX7, N7, G1): with several indexes, one of them created in the same write, insert, update (giving only
/// columns of no index, only an included one, and a key column, also on rows of an older schema version), insert or
/// update, delete, range delete and bulk load each leave every index with exactly the entries of the table's rows,
/// in the write and after it commits; rows with nulls in a unique index's key do not collide.
#[test]
fn ac_ix2_every_row_write_keeps_the_indexes_in_step() {
    stores(|store, _| {
        kept(store);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        every_row_write(&mut write, &table, true, 0);
        write.commit().unwrap();
        assert_read_in_step(store);
        // A second write, on the indexes as committed.
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        every_row_write(&mut write, &table, false, 1);
        write.commit().unwrap();
        assert_read_in_step(store);

        // A bulk load, out of key order, with nulls in the unique index's key.
        let rows: Vec<Row> = [70, 64, 68, 61, 66]
            .into_iter()
            .map(|id| {
                let mut row = member(id);
                if id % 2 == 0 {
                    row[4].1 = Value::Null;
                }
                row
            })
            .collect();
        let loaded = store
            .bulk_load("t", rows.into_iter().map(Ok::<_, Infallible>))
            .unwrap();
        assert_eq!(loaded, 5);
        assert_read_in_step(store);
        let nulls = read_entries(store, "ux_a")
            .into_iter()
            .filter(|entry| entry.key == [Value::Null])
            .count();
        assert_eq!(nulls, 2 + 2 + 4, "two of each write, four of the load");
    });
}

/// AC-IX2 (G1): a write that aborts and one that a crash of the process interrupts leave the indexes with the
/// entries of the last committed rows, in step with them after reopening.
#[test]
fn ac_ix2_an_abort_and_a_crash_keep_the_indexes_of_the_last_commit() {
    let source = Run::begin();
    let store = open(source.path());
    kept(&store);
    let committed = snapshot(&store);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    every_row_write(&mut write, &table, true, 0);
    write.abort();
    assert_eq!(snapshot(&store), committed, "aborted");

    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    every_row_write(&mut write, &table, true, 0);
    write.commit().unwrap();
    let committed = snapshot(&store);
    // A write that the crash interrupts before its commit.
    let mut pending = store.write().unwrap();
    let table = pending.open_table("t").unwrap();
    every_row_write(&mut pending, &table, false, 1);
    let run = Run::finish(source, Vec::new());
    drop(pending);
    drop(store);
    let killed = run.killed();
    let store = open(killed.path());
    assert_eq!(snapshot(&store), committed, "after the crash");
    assert_read_in_step(&store);
}

/// The row `id` of [`uniques`]: `n` and `m` are `id`, `s` is `s{id}`, `a` is 10 · `id` and `b` is true.
fn unique_row(id: i64) -> Row {
    let small = Value::I32(i32::try_from(id).unwrap());
    vec![
        ("id", Value::I64(id)),
        ("n", small.clone()),
        ("m", small),
        ("s", text(&format!("s{id}"))),
        ("a", Value::I64(10 * id)),
        ("b", Value::Bool(true)),
    ]
}

/// The table `t` of [`people`] with the rows 1 to 5 of [`unique_row`] and the indexes `u_a` (unique on `a`), `u_m`
/// (unique on `m`), `u_nb` (unique on `n` and `b`), `u_s` (unique on `s`) and `x_s` (`s` descending), committed; and
/// the sequence `seq` with 3 values handed out.
fn uniques(store: &Store) {
    commit_people(store, &(1..=5).map(unique_row).collect::<Vec<_>>());
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    for definition in [
        index("u_a", &[key("a")]).unique(),
        index("u_m", &[key("m")]).unique(),
        index("u_nb", &[key("n"), key("b")]).unique(),
        index("u_s", &[key("s")]).unique(),
        index("x_s", &[key("s").descending()]),
    ] {
        write
            .create_index(&table, &definition, IndexMode::CreateNew)
            .unwrap();
    }
    let sequence = write
        .open_sequence_with("seq", SequenceMode::CreateNew(None))
        .unwrap();
    for _ in 0..3 {
        write.next_value(&sequence).unwrap();
    }
    write.commit().unwrap();
}

/// A new row `id` of [`uniques`] that repeats no unique index, with the columns `changes` replaced.
fn new_row(id: i64, changes: &[(&'static str, Value<'static>)]) -> Row {
    let mut row = unique_row(id);
    for (name, value) in changes {
        row.iter_mut().find(|(n, _)| n == name).unwrap().1 = value.clone();
    }
    row
}

/// A text whose entry in an index on `s` of [`people`], whose primary key is `id i64`, has 2,048 bytes ascending and
/// 2,049 descending, whose element ends a text with `00 01` (document 3).
fn at_the_limit() -> Value<'static> {
    text(&"y".repeat(2044))
}

/// Everything a failed row write must leave as it was in `write`: the rows, the entries of each index and the row
/// count of `t`.
#[allow(clippy::type_complexity)]
fn table_state(
    write: &WriteTransaction<'_>,
    table: &WriteTable<'_>,
) -> (
    Vec<Vec<(String, Value<'static>)>>,
    Vec<Vec<StoredIndexEntry>>,
    u64,
) {
    (
        rows_of(write, table),
        all_entries(write, table),
        write.row_count(table).unwrap(),
    )
}

/// The data of the duplicate key of the unique index `index` of `t` repeating `values`, for the row `key`.
fn index_duplicate(
    index: &str,
    key: &str,
    values: &str,
) -> (String, String, Option<String>, Option<String>) {
    ("t".to_owned(), key.to_owned(), some(index), some(values))
}

/// The data of an entry of `x_s` of `t` that is too large, with a text at the limit.
fn x_s_too_large() -> (String, String, u64) {
    ("t".to_owned(), "x_s".to_owned(), 2049)
}

/// AC-IX6 (IX7, IX10): inserting, updating and inserting or updating a row that repeats, without nulls, the values
/// of each unique index in turn fails without effect with a duplicate key, with the table, the primary key of the row
/// it saves, the index and the repeated values; a row whose entry is too large, by a text at the limit that fits in
/// an ascending index and not in a descending one, fails with its size and data; in every case the rows, the indexes
/// and the row count stay as they were, and the write commits its other changes.
#[test]
fn ac_ix6_a_row_that_repeats_a_unique_index_fails_without_effect() {
    stores(|store, folder| {
        uniques(store);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let before = table_state(&write, &table);
        let others = [
            ("n", Value::I32(90)),
            ("m", Value::I32(90)),
            ("a", Value::I64(900)),
            ("s", text("s90")),
        ];
        let repeating = |column: &'static str, value: Value<'static>| -> Row {
            let mut changes = others.to_vec();
            changes.retain(|(name, _)| *name != column);
            changes.push((column, value));
            new_row(9, &changes)
        };
        let inserts = [
            (
                repeating("a", Value::I64(10)),
                index_duplicate("u_a", "(9)", "(10)"),
            ),
            (
                repeating("m", Value::I32(2)),
                index_duplicate("u_m", "(9)", "(2)"),
            ),
            (
                repeating("n", Value::I32(3)),
                index_duplicate("u_nb", "(9)", "(3, true)"),
            ),
            (
                repeating("s", text("s1")),
                index_duplicate("u_s", "(9)", "(\"s1\")"),
            ),
        ];
        for (row, expected) in inserts {
            let error = write.insert_row(&table, &row).unwrap_err();
            assert_eq!(duplicate(error, folder), expected, "insert");
            assert_eq!(
                write.upsert_row(&table, &row).map(drop).unwrap_err().kind(),
                ErrorKind::DuplicateKey
            );
            assert_eq!(
                table_state(&write, &table),
                before,
                "insert into {:?}",
                expected.2
            );
        }
        let updates: [(Row, _); 4] = [
            (
                vec![("id", Value::I64(5)), ("a", Value::I64(10))],
                index_duplicate("u_a", "(5)", "(10)"),
            ),
            (
                vec![("id", Value::I64(5)), ("m", Value::I32(1))],
                index_duplicate("u_m", "(5)", "(1)"),
            ),
            (
                vec![("id", Value::I64(5)), ("n", Value::I32(2))],
                index_duplicate("u_nb", "(5)", "(2, true)"),
            ),
            (
                vec![("id", Value::I64(5)), ("s", text("s4"))],
                index_duplicate("u_s", "(5)", "(\"s4\")"),
            ),
        ];
        for (row, expected) in updates {
            let error = write.update_row(&table, &row).unwrap_err();
            assert_eq!(duplicate(error, folder), expected, "update");
            let error = write.upsert_row(&table, &row).unwrap_err();
            assert_eq!(duplicate(error, folder), expected, "upsert");
            assert_eq!(
                table_state(&write, &table),
                before,
                "update of {:?}",
                expected.2
            );
        }
        // A text at the limit fits in `u_s`, ascending, and not in `x_s`, descending.
        let long = repeating("s", at_the_limit());
        let error = write.insert_row(&table, &long).unwrap_err();
        assert_eq!(entry_too_large(error, folder), x_s_too_large(), "insert");
        let long = [("id", Value::I64(5)), ("s", at_the_limit())];
        let error = write.update_row(&table, &long).unwrap_err();
        assert_eq!(entry_too_large(error, folder), x_s_too_large(), "update");
        let error = write.upsert_row(&table, &long).unwrap_err();
        assert_eq!(entry_too_large(error, folder), x_s_too_large(), "upsert");
        assert_eq!(table_state(&write, &table), before, "too large");
        // A row that moves the unique values of its own entries to free ones, and the write commits its changes.
        write.insert_row(&table, &new_row(6, &[])).unwrap();
        let moved = [
            ("id", Value::I64(5)),
            ("a", Value::I64(500)),
            ("m", Value::I32(50)),
        ];
        assert!(write.update_row(&table, &moved).unwrap());
        write
            .insert_row(&table, &new_row(7, &[("a", Value::I64(50))]))
            .unwrap();
        write.commit().unwrap();
        assert_read_in_step(store);
        let read = store.read().unwrap();
        assert_eq!(read.row_count(&read.open_table("t").unwrap()).unwrap(), 7);
    });
}

/// AC-IX9 (IX10), the pairs that happen when saving a row: an entry too large and a duplicate (the size); the primary
/// key and an index repeated (the primary key, with no index); and two indexes repeated (the first by its name).
#[test]
fn ac_ix9_the_precedence_of_the_failures_when_saving_a_row() {
    stores(|store, folder| {
        uniques(store);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let before = table_state(&write, &table);
        // Too large in `x_s`, and repeating `u_a` and `u_m`.
        let long = new_row(
            9,
            &[
                ("n", Value::I32(90)),
                ("a", Value::I64(10)),
                ("m", Value::I32(2)),
                ("s", at_the_limit()),
            ],
        );
        let error = write.insert_row(&table, &long).unwrap_err();
        assert_eq!(entry_too_large(error, folder), x_s_too_large(), "insert");
        let long = [
            ("id", Value::I64(5)),
            ("a", Value::I64(10)),
            ("s", at_the_limit()),
        ];
        let error = write.update_row(&table, &long).unwrap_err();
        assert_eq!(entry_too_large(error, folder), x_s_too_large(), "update");
        // The primary key and `u_m` repeated.
        let row = new_row(
            1,
            &[
                ("m", Value::I32(2)),
                ("a", Value::I64(900)),
                ("s", text("s90")),
            ],
        );
        let error = write.insert_row(&table, &row).unwrap_err();
        assert_eq!(
            duplicate(error, folder),
            ("t".to_owned(), "(1)".to_owned(), None, None)
        );
        // `u_m` and `u_nb` repeated, with `u_s` too: the first by its name.
        let two = new_row(
            9,
            &[
                ("a", Value::I64(900)),
                ("m", Value::I32(2)),
                ("n", Value::I32(3)),
                ("s", text("s4")),
            ],
        );
        let error = write.insert_row(&table, &two).unwrap_err();
        assert_eq!(
            duplicate(error, folder),
            index_duplicate("u_m", "(9)", "(2)")
        );
        let two = [
            ("id", Value::I64(5)),
            ("n", Value::I32(3)),
            ("s", text("s4")),
        ];
        let error = write.update_row(&table, &two).unwrap_err();
        assert_eq!(
            duplicate(error, folder),
            index_duplicate("u_nb", "(5)", "(3, true)")
        );
        assert_eq!(table_state(&write, &table), before);
        write.commit().unwrap();
    });
}

/// What a bulk load that fails must leave as it was: the rows and the index entries of `t`, and the next value of
/// `seq`, taken in a write that aborts.
#[allow(clippy::type_complexity)]
fn load_state(
    store: &Store,
) -> (
    (
        Vec<Vec<(String, Value<'static>)>>,
        Vec<(String, Vec<StoredIndexEntry>)>,
    ),
    i64,
) {
    let mut write = store.write_with_timeout(Duration::ZERO).unwrap();
    let sequence = write.open_sequence("seq").unwrap();
    let next = write.next_value(&sequence).unwrap();
    write.abort();
    (snapshot(store), next)
}

/// The engine's error of a bulk load that fails.
#[track_caller]
fn load_error<E: std::fmt::Debug>(outcome: Result<u64, BulkLoadError<E>>) -> Error {
    match outcome {
        Err(BulkLoadError::Store(error)) => error,
        other => panic!("expected an error of the engine, got {other:?}"),
    }
}

/// AC-IX8 (E13, IX7): a bulk load into a table with indexes that has, before its end, a row that collides in a
/// unique index with a row of the table or with an earlier row of the same load, a row whose entry is too large, a
/// failing last row, a failing source, a cancellation or a panic, leaves the table, its indexes, its row count and
/// the sequences as they were, and the next write gets its turn at once.
#[test]
fn ac_ix8_a_bulk_load_that_fails_leaves_the_table_and_its_indexes_as_before() {
    stores(|store, folder| {
        uniques(store);
        let before = load_state(store);
        let load = |rows: Vec<Row>| store.bulk_load("t", rows.into_iter().map(Ok::<_, Infallible>));
        let failures = [
            // `u_a` with the row 1 of the table.
            (
                vec![
                    new_row(10, &[]),
                    new_row(11, &[("a", Value::I64(10))]),
                    new_row(12, &[]),
                ],
                index_duplicate("u_a", "(11)", "(10)"),
            ),
            // `u_m` with the row 12 of the same load, the first row of the load that collides.
            (
                vec![
                    new_row(12, &[]),
                    new_row(10, &[]),
                    new_row(11, &[("m", Value::I32(12))]),
                    new_row(13, &[("m", Value::I32(10))]),
                ],
                index_duplicate("u_m", "(11)", "(12)"),
            ),
            // The last row, with the row 12 of the load.
            (
                vec![
                    new_row(12, &[]),
                    new_row(10, &[]),
                    new_row(11, &[("a", Value::I64(120))]),
                ],
                index_duplicate("u_a", "(11)", "(120)"),
            ),
        ];
        for (rows, expected) in failures {
            assert_eq!(duplicate(load_error(load(rows)), folder), expected);
            assert_eq!(load_state(store), before, "{expected:?}");
        }
        let rows = vec![
            new_row(10, &[]),
            new_row(11, &[("s", at_the_limit())]),
            new_row(12, &[]),
        ];
        assert_eq!(
            entry_too_large(load_error(load(rows)), folder),
            x_s_too_large()
        );
        assert_eq!(load_state(store), before, "too large");
        // A failing source, which is also how the application cancels a load.
        let rows = [Ok(new_row(10, &[])), Ok(new_row(11, &[])), Err("cancelled")];
        match store.bulk_load("t", rows) {
            Err(BulkLoadError::Source(stop)) => assert_eq!(stop, "cancelled"),
            other => panic!("expected the source's error, got {other:?}"),
        }
        assert_eq!(load_state(store), before, "source");
        let source = [new_row(10, &[]), new_row(11, &[])]
            .into_iter()
            .map(Ok::<_, Infallible>)
            .chain(std::iter::from_fn(|| panic!("the source panicked")));
        let outcome = catch_unwind(AssertUnwindSafe(|| store.bulk_load("t", source)));
        assert!(outcome.is_err());
        assert_eq!(load_state(store), before, "panic");
    });
}

/// AC-IX8 (AC10 of 7.1, AC-N3): 100,000 rows loaded out of key order into a table with indexes give the table those
/// rows and each index exactly their entries, in the order of N5: what inserting them one by one gives, since each
/// insert keeps the indexes in step with the rows (AC-IX2). The indexes are not unique, which keeps the test short in
/// debug builds: the probes of unique indexes in loads are in the other tests of AC-IX8 and in AC-IX2.
#[test]
fn ac_ix8_a_bulk_load_out_of_order_gives_the_entries_of_inserts_one_by_one() {
    let mut ids: Vec<i64> = (0..100_000).collect();
    Lcg(11).shuffle(&mut ids);
    let row = |id: i64| -> Row {
        vec![
            ("id", Value::I64(id)),
            ("n", Value::I32(i32::try_from(id % 1000).unwrap())),
            ("s", text(&format!("{}", id * 7919 % 100_003))),
            ("a", Value::I64(-id)),
        ]
    };
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_people(&store, &[]);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    for definition in [
        index("ix_n", &[key("n").descending()]).with_included(&["s"]),
        index("ix_a", &[key("a")]),
    ] {
        write
            .create_index(&table, &definition, IndexMode::CreateNew)
            .unwrap();
    }
    write.commit().unwrap();
    let rows = ids.iter().map(|&id| Ok::<_, Infallible>(row(id)));
    assert_eq!(store.bulk_load("t", rows).unwrap(), 100_000);
    let (rows, _) = snapshot(&store);
    let found: Vec<&Value<'static>> = rows.iter().map(|row| &row[0].1).collect();
    let expected: Vec<Value<'static>> = (0..100_000).map(Value::I64).collect();
    assert!(
        found.iter().copied().eq(expected.iter()),
        "every row, in key order"
    );
    assert_read_in_step(&store);
}

/// IX5 and N7: allowing nulls in a column of a unique index, then storing rows with a null there, keeps the nulls
/// order of the index's column, and the nulls never collide.
#[test]
fn allowing_nulls_in_an_index_column_then_storing_nulls() {
    stores(|store, _| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("k", ColumnType::I32).not_null(),
            ],
            &["id"],
        );
        let row = |id: i64, k: Option<i32>| {
            [
                ("id", Value::I64(id)),
                ("k", k.map_or(Value::Null, Value::I32)),
            ]
        };
        let mut write = store.write().unwrap();
        let table = create_table(&mut write, "t", &schema);
        for id in 1..=3 {
            write
                .insert_row(&table, &row(id, Some(i32::try_from(id).unwrap())))
                .unwrap();
        }
        let definition = index("u_k", &[key("k").nulls_last()]).unique();
        write
            .create_index(&table, &definition, IndexMode::CreateNew)
            .unwrap();
        write.commit().unwrap();
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write
            .modify_column(&table, "k", ColumnType::I32, true)
            .unwrap();
        for id in [5, 4] {
            write.insert_row(&table, &row(id, None)).unwrap();
        }
        assert!(write.update_row(&table, &row(2, None)).unwrap());
        let found: Vec<(Vec<Value<'static>>, Vec<Value<'static>>)> = entries(&write, &table, "u_k")
            .into_iter()
            .map(|entry| (entry.key, entry.primary_key))
            .collect();
        let null = |id| (vec![Value::Null], vec![Value::I64(id)]);
        let value = |k, id| (vec![Value::I32(k)], vec![Value::I64(id)]);
        assert_eq!(found, [value(1, 1), value(3, 3), null(2), null(4), null(5)]);
        write.commit().unwrap();
    });
}

/// IX5 (E5): dropping a column that is only among the included columns of an index fails without effect with the
/// rule of dropping a key column, naming the index.
#[test]
fn dropping_an_included_column_fails_naming_its_index() {
    stores(|store, folder| {
        kept(store);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let before = write.table_schema(&table).unwrap();
        // `b` is only included, in `ux_a`.
        let error = write.drop_column(&table, "b").unwrap_err();
        assert_eq!(
            invalid_schema(error, folder),
            (
                "t".to_owned(),
                some("ux_a"),
                some("b"),
                SchemaRule::DropKeyColumn
            )
        );
        assert_eq!(write.table_schema(&table).unwrap(), before, "no effect");
        write.commit().unwrap();
    });
}

/// IX2, IX7 (P13, T11): a row write that the system does not give the memory of an index entry's content, its key or
/// the payload of its included columns, fails with out of memory, with T11 and no change, instead of ending the
/// process: an insert, an update of an included column and a delete, in a store on disk and in a temporary one.
#[test]
fn no_memory_for_an_entry_fails_the_row_write_with_out_of_memory() {
    use krummi_pager::testing::{self, Action, Op};
    const PAYLOAD: usize = 2 << 20;
    type RowWrite = fn(&mut WriteTransaction<'_>, &WriteTable<'_>) -> krummi::Result<()>;
    fn row(id: i64, p: &str) -> Vec<(&'static str, Value<'static>)> {
        vec![
            ("id", Value::I64(id)),
            ("k", Value::String(Cow::Owned("k".repeat(1000)))),
            ("p", Value::String(Cow::Owned(p.repeat(PAYLOAD)))),
        ]
    }
    // Each write, and whether it reserves a payload; every one reserves an entry's key.
    let writes: [(&str, RowWrite, bool); 3] = [
        ("insert", |w, t| w.insert_row(t, &row(2, "b")), true),
        (
            "update",
            |w, t| w.update_row(t, &row(1, "c")).map(|_| ()),
            true,
        ),
        (
            "delete",
            |w, t| w.delete_row(t, &[Value::I64(1)]).map(|_| ()),
            false,
        ),
    ];
    stores(|store, _| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("k", ColumnType::String),
                Column::new("p", ColumnType::String),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        let table = create_table(&mut write, "t", &schema);
        let ix = index("ix_k", &[key("k")]).with_included(&["p"]);
        write
            .create_index(&table, &ix, IndexMode::CreateNew)
            .unwrap();
        write.insert_row(&table, &row(1, "a")).unwrap();
        write.commit().unwrap();
        let entries = |store: &Store| {
            let read = store.read().unwrap();
            read.stored_index_entries(&read.open_table("t").unwrap(), "ix_k")
                .unwrap()
                .unwrap()
        };
        let before = entries(store);
        let key_len = before[0].bytes.len() as u64;
        for (name, row_write, payload) in writes {
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            testing::reset();
            row_write(&mut write, &table).unwrap();
            let reservations = testing::count(Op::Alloc);
            testing::reset();
            drop(write);
            let mut requested = Vec::new();
            for nth in 0..reservations {
                let mut write = store.write().unwrap();
                let table = write.open_table("t").unwrap();
                testing::reset();
                testing::inject(
                    Op::Alloc,
                    nth,
                    Action::Fail(std::io::ErrorKind::OutOfMemory),
                );
                let written = row_write(&mut write, &table);
                testing::reset();
                let Err(error) = written else {
                    continue;
                };
                let Error::OutOfMemory { requested: bytes } = error else {
                    panic!("{name} {nth}: {error}");
                };
                requested.push(bytes);
                assert_eq!(
                    write.commit().unwrap_err().kind(),
                    ErrorKind::OutOfMemory,
                    "{name} {nth}: T11"
                );
                assert_eq!(entries(store), before, "{name} {nth}");
            }
            let payload_failed = (requested.iter())
                .any(|&bytes| (PAYLOAD as u64..PAYLOAD as u64 + 64).contains(&bytes));
            assert_eq!(
                payload_failed, payload,
                "{name}: the payload, {requested:?}"
            );
            assert!(
                requested.contains(&key_len),
                "{name}: the key, {requested:?}"
            );
        }
    });
}
