//! Scans of the secondary indexes of a table through the public API (secondary indexes PRD 7.2, T4 of its execution):
//! the intervals of IX9 over ascending and descending columns with their nulls first and last, forward and backward,
//! for entries and for rows, their malformed forms and the size of their prefix (AC-IX4); an index that does not exist
//! and the precedence of the failures of a scan (IX10, AC-IX9); and a property test of random indexes and random row
//! writes against a reference model of N5 (AC-IX5).
//!
//! Deterministic: the random sequences come from fixed seeds. The criteria run on a store on disk and on a temporary
//! one.

mod support;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::ops::Bound::{self, Excluded, Included, Unbounded};
use std::path::Path;

use krummi::ids::Uuid;
use krummi::table::key::key_len;
use krummi::table::value::{ColumnType, DATE_MAX, UuidOrder, Value};
use krummi::{
    Column, Direction, Error, ErrorKind, IndexDefinition, IndexKey, IndexMode, Nulls, ReadTable,
    ReadTransaction, Schema, SchemaDifference, SizeSubject, Store, TableMode, TableSchema,
    ValueRule, WriteTable, WriteTransaction,
};
use support::*;

// ---------------------------------------------------------------------------------------------------------------
// Helpers

type Bounds = (Bound<Value<'static>>, Bound<Value<'static>>);

/// An interval of an index: the prefix and the bounds on the next column.
type Interval = (Vec<Value<'static>>, Bounds);

fn text(value: &str) -> Value<'static> {
    Value::String(Cow::Owned(value.to_owned()))
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

/// Every bound on the values `values`: absent, and each value included and excluded.
fn every_bound(values: &[Value<'static>]) -> Vec<Bound<Value<'static>>> {
    let mut bounds = vec![Unbounded];
    for value in values {
        bounds.push(Included(value.clone()));
        bounds.push(Excluded(value.clone()));
    }
    bounds
}

/// A row of the model: its primary key `id` and the values of the other columns by name, as the table reads them.
type ModelRow = (i64, BTreeMap<String, Value<'static>>);

/// A key column of an index of the model: its name, type, direction and null order.
#[derive(Clone)]
struct ModelKey {
    name: String,
    ty: ColumnType,
    direction: Direction,
    nulls: Nulls,
}

fn model_keys(schema: &TableSchema, index: &str) -> Vec<ModelKey> {
    (schema.index(index).unwrap().key.iter())
        .map(|key| ModelKey {
            name: key.name.clone(),
            ty: schema.column(&key.name).unwrap().ty,
            direction: key.direction,
            nulls: key.nulls,
        })
        .collect()
}

/// Whether the values `values` of the key columns `keys` are in the interval of IX9: equal to the prefix (a null
/// prefix value asks for a null), and, with a bound, the next column not null and within the bounds in the order of
/// N5 of its type, whatever its direction.
fn in_interval(
    keys: &[ModelKey],
    values: &[Value<'_>],
    (prefix, (lower, upper)): &Interval,
) -> bool {
    let equal = |key: &ModelKey, a: &Value<'_>, b: &Value<'_>| match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Null, _) | (_, Value::Null) => false,
        (a, b) => model_cmp(&key.ty, a, b).is_eq(),
    };
    if !(keys.iter().zip(values).zip(prefix)).all(|((key, a), b)| equal(key, a, b)) {
        return false;
    }
    if matches!((lower, upper), (Unbounded, Unbounded)) {
        return true;
    }
    let (key, value) = (&keys[prefix.len()], &values[prefix.len()]);
    if matches!(value, Value::Null) {
        return false;
    }
    let cmp = |bound: &Value<'_>| model_cmp(&key.ty, value, bound);
    let after = match lower {
        Included(v) => cmp(v).is_ge(),
        Excluded(v) => cmp(v).is_gt(),
        Unbounded => true,
    };
    let before = match upper {
        Included(v) => cmp(v).is_le(),
        Excluded(v) => cmp(v).is_lt(),
        Unbounded => true,
    };
    after && before
}

/// The primary keys of the rows whose entries in the index of key `keys` are in `interval`, forward in the index's
/// order (IX8): its key columns in the order of N5 with their direction and nulls, equal keys by the primary key.
fn model_scan(keys: &[ModelKey], rows: &[ModelRow], interval: &Interval) -> Vec<i64> {
    let values = |row: &ModelRow| -> Vec<Value<'static>> {
        keys.iter().map(|key| row.1[&key.name].clone()).collect()
    };
    let mut found: Vec<(Vec<Value<'static>>, i64)> = rows
        .iter()
        .map(|row| (values(row), row.0))
        .filter(|(values, _)| in_interval(keys, values, interval))
        .collect();
    found.sort_by(|(a, ia), (b, ib)| {
        (keys.iter().zip(a.iter().zip(b)))
            .map(|(key, (a, b))| column_cmp(&key.ty, key.direction, key.nulls, a, b))
            .find(|o| o.is_ne())
            .unwrap_or_else(|| ia.cmp(ib))
    });
    found.into_iter().map(|(_, id)| id).collect()
}

/// A row as the table reads it, for the model.
fn model_row(row: &krummi::TableRow<'_>) -> ModelRow {
    let Some(Value::I64(id)) = row.get("id") else {
        panic!("an id");
    };
    let values = row
        .columns()
        .filter(|(name, _)| *name != "id")
        .map(|(name, value)| (name.to_owned(), value.into_owned()))
        .collect();
    (id, values)
}

/// The rows of the table `table` as `rows` reads them, for the model.
fn model_rows<'r>(
    rows: impl Iterator<Item = krummi::Result<krummi::TableRow<'r>>>,
) -> Vec<ModelRow> {
    rows.map(|row| model_row(&row.unwrap())).collect()
}

/// Asserts that the row `found` has the columns and values, compared as [`same`] compares them, of the row of `rows`
/// with its primary key; returns that key.
#[track_caller]
fn assert_row(rows: &[ModelRow], found: &ModelRow) -> i64 {
    let (id, values) = found;
    let expected = &(rows.iter().find(|row| row.0 == *id))
        .unwrap_or_else(|| panic!("row {id} is not in the model"))
        .1;
    let equal = values.len() == expected.len()
        && (values.iter().zip(expected)).all(|((a, x), (b, y))| a == b && same(x, y));
    assert!(equal, "row {id}: {values:?}, expected {expected:?}");
    *id
}

/// The primary key of an entry or a row.
fn id_of(value: Option<Value<'_>>) -> i64 {
    match value {
        Some(Value::I64(id)) => id,
        other => panic!("expected an id, got {other:?}"),
    }
}

/// The scans of an interval of an index in a read and in a write: entries and rows, forward and backward, as the
/// primary keys of their rows, after checking that each entry's key columns are those of its row and that each row
/// read has every column and value of its row in `rows`.
macro_rules! scans {
    ($tx:expr, $table:expr, $index:expr, $keys:expr, $rows:expr, $interval:expr) => {{
        let (prefix, bounds) = $interval;
        let check = |entry: &krummi::IndexEntry<'_>| {
            let id = id_of(entry.get("id"));
            let row = &$rows.iter().find(|row| row.0 == id).unwrap().1;
            for (at, key) in $keys.iter().enumerate() {
                assert_eq!(entry.name(at), Some(key.name.as_str()));
                assert!(same(&entry.value(at).unwrap(), &row[&key.name]), "{id}");
            }
            id
        };
        let entries: Vec<i64> = ($tx.index_entries($table, $index, prefix, bounds.clone()))
            .unwrap()
            .map(|entry| check(&entry.unwrap()))
            .collect();
        let back: Vec<i64> = ($tx.index_entries($table, $index, prefix, bounds.clone()))
            .unwrap()
            .rev()
            .map(|entry| check(&entry.unwrap()))
            .collect();
        let rows: Vec<i64> = ($tx.index_rows($table, $index, prefix, bounds.clone()))
            .unwrap()
            .map(|row| assert_row(&$rows, &model_row(&row.unwrap())))
            .collect();
        let rows_back: Vec<i64> = ($tx.index_rows($table, $index, prefix, bounds.clone()))
            .unwrap()
            .rev()
            .map(|row| assert_row(&$rows, &model_row(&row.unwrap())))
            .collect();
        (entries, back, rows, rows_back)
    }};
}

/// Asserts that the scans of `interval` give the rows of the model, forward and in exact reverse.
#[track_caller]
fn assert_scans(found: (Vec<i64>, Vec<i64>, Vec<i64>, Vec<i64>), expected: &[i64], what: &str) {
    let (entries, back, rows, rows_back) = found;
    let reversed: Vec<i64> = expected.iter().rev().copied().collect();
    assert_eq!(entries, expected, "{what}: entries forward");
    assert_eq!(back, reversed, "{what}: entries backward");
    assert_eq!(rows, expected, "{what}: rows forward");
    assert_eq!(rows_back, reversed, "{what}: rows backward");
}

fn read_rows(read: &ReadTransaction<'_>, table: &ReadTable<'_>) -> Vec<ModelRow> {
    model_rows(read.range_rows(table, &[], ..).unwrap())
}

fn write_rows(write: &WriteTransaction<'_>, table: &WriteTable<'_>) -> Vec<ModelRow> {
    model_rows(write.range_rows(table, &[], ..).unwrap())
}

/// The table, column and rule of an invalid-value error, after checking its folder.
#[track_caller]
fn invalid_value(
    error: Error,
    folder: Option<&Path>,
) -> (Option<String>, Option<String>, ValueRule) {
    match error {
        Error::InvalidValue {
            folder: f,
            table,
            column,
            sequence: None,
            rule,
        } => {
            assert_eq!(f, folder_of(folder));
            (table, column, rule)
        }
        other => panic!("expected an invalid value, got {other}"),
    }
}

fn in_t(column: Option<&str>, rule: ValueRule) -> (Option<String>, Option<String>, ValueRule) {
    (Some("t".to_owned()), column.map(str::to_owned), rule)
}

/// The table, index and received size of an invalid size of a prefix of an index, after checking its folder and its
/// limits (B8).
#[track_caller]
fn prefix_too_large(error: Error, folder: Option<&Path>) -> (String, String, u64) {
    match error {
        Error::InvalidSize {
            folder: f,
            what: SizeSubject::Prefix,
            received,
            min: 1,
            max: 2048,
            table: Some(table),
            index: Some(index),
        } => {
            assert_eq!(f, folder_of(folder));
            (table.into(), index.into(), received)
        }
        other => panic!("expected a prefix too large, got {other}"),
    }
}

/// The table and index of a scan of an index that does not exist, after checking its class, data and folder.
#[track_caller]
fn missing_index(error: Error, folder: Option<&Path>) -> (String, String) {
    assert_eq!(error.kind(), ErrorKind::DifferentSchema);
    match error {
        Error::DifferentIndex {
            folder: f,
            table,
            index,
            difference: SchemaDifference::MissingIndex,
            expected,
            found,
        } => {
            assert_eq!(f, folder_of(folder));
            assert_eq!((&*expected, &*found), ("an index", "no index"));
            (table, index)
        }
        other => panic!("expected an index that does not exist, got {other}"),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// AC-IX4: intervals

/// `id i64 not null, a i32, f f64, g i32 not null, d date, s string`, primary key `id`.
fn intervals_schema() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("a", ColumnType::I32),
            Column::new("f", ColumnType::F64),
            Column::new("g", ColumnType::I32).not_null(),
            Column::new("d", ColumnType::Date),
            Column::new("s", ColumnType::String),
        ],
        &["id"],
    )
}

const A_VALUES: [Option<i32>; 5] = [None, Some(-1), Some(0), Some(2), Some(5)];

/// The values of `f`: null, `-0` (stored as `+0`), NaNs of two bits (stored as one) and the infinities.
fn f_values() -> Vec<Value<'static>> {
    vec![
        Value::Null,
        Value::F64(-1.5),
        Value::F64(-0.0),
        Value::F64(0.0),
        Value::F64(1.5),
        Value::F64(f64::NAN),
        Value::F64(f64::from_bits(0xFFF8_0000_0000_0001)),
        Value::F64(f64::INFINITY),
    ]
}

/// The indexes of AC-IX4: `ix0` to `ix3` on `a` and `f`, each column in every order across them; `ix_g` on a column
/// without nulls, `ix_d` on a date and `ix_s` on a text before `a`.
fn interval_indexes() -> Vec<IndexDefinition> {
    let mut indexes: Vec<IndexDefinition> = (0..4)
        .map(|i| {
            IndexDefinition::new(
                format!("ix{i}"),
                vec![ordered("a", ORDERS[i]), ordered("f", ORDERS[(i + 1) % 4])],
            )
        })
        .collect();
    indexes.push(IndexDefinition::new("ix_g", vec![IndexKey::new("g")]));
    indexes.push(IndexDefinition::new("ix_d", vec![IndexKey::new("d")]));
    indexes.push(IndexDefinition::new(
        "ix_s",
        vec![IndexKey::new("s"), IndexKey::new("a").descending()],
    ));
    indexes
}

/// Commits the table `t` of [`intervals_schema`] with 80 rows that repeat each value of `a` and `f`, and its indexes.
fn intervals_store(store: &Store) {
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&intervals_schema()))
        .unwrap();
    let f = f_values();
    for id in 0..80i64 {
        let i = usize::try_from(id).unwrap();
        let row = [
            ("id", Value::I64(id)),
            ("a", A_VALUES[i % 5].map_or(Value::Null, Value::I32)),
            ("f", f[(i / 5) % f.len()].clone()),
            ("g", Value::I32(i32::try_from(id % 3).unwrap())),
            ("d", Value::Date(i32::try_from(id).unwrap())),
            ("s", text(&format!("s{}", id % 4))),
        ];
        write.insert_row(&table, &row).unwrap();
    }
    for definition in interval_indexes() {
        write
            .create_index(&table, &definition, IndexMode::CreateNew)
            .unwrap();
    }
    write.commit().unwrap();
}

/// The intervals of AC-IX4 on `ix0` to `ix3`: no prefix with every pair of bounds on `a`; each prefix on `a`, a null
/// among them, with every pair of bounds on `f`, `-0` and NaN of other bits among them; and prefixes of both columns
/// with `-0` and NaN.
fn intervals() -> Vec<Interval> {
    let a_bounds = every_bound(&[-1, 0, 2, 3].map(Value::I32));
    let f_bounds = every_bound(&[
        Value::F64(-0.0),
        Value::F64(-1.5),
        Value::F64(f64::from_bits(0x7FF8_0000_0000_0042)),
        Value::F64(f64::INFINITY),
        Value::F64(f64::NEG_INFINITY),
    ]);
    let mut intervals = Vec::new();
    for lower in &a_bounds {
        for upper in &a_bounds {
            intervals.push((vec![], (lower.clone(), upper.clone())));
        }
    }
    for a in [Value::Null, Value::I32(-1), Value::I32(2), Value::I32(7)] {
        for lower in &f_bounds {
            for upper in &f_bounds {
                intervals.push((vec![a.clone()], (lower.clone(), upper.clone())));
            }
        }
    }
    for a in [Value::Null, Value::I32(0), Value::I32(5)] {
        for f in [
            Value::Null,
            Value::F64(-0.0),
            Value::F64(f64::from_bits(0xFFF8_0000_0000_0007)),
            Value::F64(1.5),
        ] {
            intervals.push((vec![a.clone(), f], (Unbounded, Unbounded)));
        }
    }
    intervals
}

/// AC-IX4 (IX8, IX9): on indexes whose columns are ascending and descending with their nulls first and last, every
/// interval of [`intervals`] gives, forward and backward, for entries and for rows, exactly the rows of IX9 in the
/// order of IX8: a null prefix value asks for the nulls, `-0` finds `+0` and any NaN the stored one, and with a bound
/// the nulls of its column are outside; an inverted interval gives nothing; in a read and in a write.
#[test]
fn ac_ix4_every_interval_gives_the_entries_of_ix9() {
    stores(|store, _| {
        intervals_store(store);
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let rows = read_rows(&read, &table);
        let schema = read.describe_table("t").unwrap();
        let mut inverted = 0;
        for index in ["ix0", "ix1", "ix2", "ix3"] {
            let keys = model_keys(&schema, index);
            for interval in intervals() {
                let expected = model_scan(&keys, &rows, &interval);
                let found = scans!(read, &table, index, keys, rows, &interval);
                assert_scans(found, &expected, &format!("{index} {interval:?}"));
                if let (_, (Included(lo) | Excluded(lo), Included(hi) | Excluded(hi))) = &interval
                    && bound_cmp(lo, hi).is_gt()
                {
                    assert!(expected.is_empty());
                    inverted += 1;
                }
            }
        }
        assert!(inverted > 0, "some intervals are inverted");
        // The nulls: a null prefix asks for them, and a bound leaves them out.
        let keys = model_keys(&schema, "ix1");
        let nulls = (vec![Value::Null], (Unbounded, Unbounded));
        assert_eq!(model_scan(&keys, &rows, &nulls).len(), 16);
        drop(read);

        // The same in a write, with a row it inserted.
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let row = [
            ("id", Value::I64(100)),
            ("a", Value::I32(2)),
            ("f", Value::F64(-0.0)),
            ("g", Value::I32(0)),
        ];
        write.insert_row(&table, &row).unwrap();
        let rows = write_rows(&write, &table);
        for index in ["ix0", "ix3"] {
            let keys = model_keys(&schema, index);
            for interval in intervals().into_iter().step_by(7) {
                let expected = model_scan(&keys, &rows, &interval);
                let found = scans!(write, &table, index, keys, rows, &interval);
                assert_scans(found, &expected, &format!("{index} {interval:?}"));
            }
        }
        write.commit().unwrap();
    });
}

/// The order of N5 of two bound values of [`intervals`], of `a` or of `f`.
fn bound_cmp(a: &Value<'_>, b: &Value<'_>) -> std::cmp::Ordering {
    let ty = match a {
        Value::I32(_) => ColumnType::I32,
        _ => ColumnType::F64,
    };
    model_cmp(&ty, a, b)
}

/// AC-IX4 (IX9): each malformed interval fails with an invalid value, with the table, its rule and the column when a
/// value breaks it: a value of another type, out of its domain, a null in the prefix of a column without nulls, a null
/// bound, a prefix longer than the key and a bound when the prefix gives every column; a malformed prefix length
/// before the values, and then the first value that fails, the prefix in key order, then the lower and the upper
/// bound. The same for entries and rows, in a read and in a write.
#[test]
fn ac_ix4_malformed_intervals_fail_with_their_rule() {
    stores(|store, folder| {
        intervals_store(store);
        let big = || Value::Date(DATE_MAX + 1);
        let cases: Vec<(&str, Interval, _)> = vec![
            (
                "ix0",
                (vec![Value::I64(1)], (Unbounded, Unbounded)),
                in_t(Some("a"), ValueRule::WrongType),
            ),
            (
                "ix0",
                (vec![], (Unbounded, Included(Value::I64(1)))),
                in_t(Some("a"), ValueRule::WrongType),
            ),
            (
                "ix0",
                (vec![Value::I32(1)], (Included(text("x")), Unbounded)),
                in_t(Some("f"), ValueRule::WrongType),
            ),
            (
                "ix_d",
                (vec![big()], (Unbounded, Unbounded)),
                in_t(Some("d"), ValueRule::OutOfDomain),
            ),
            (
                "ix_d",
                (vec![], (Unbounded, Excluded(big()))),
                in_t(Some("d"), ValueRule::OutOfDomain),
            ),
            (
                "ix_g",
                (vec![Value::Null], (Unbounded, Unbounded)),
                in_t(Some("g"), ValueRule::NullNotAllowed),
            ),
            (
                "ix0",
                (vec![], (Included(Value::Null), Unbounded)),
                in_t(Some("a"), ValueRule::InvalidRange),
            ),
            (
                "ix0",
                (vec![Value::Null], (Unbounded, Excluded(Value::Null))),
                in_t(Some("f"), ValueRule::InvalidRange),
            ),
            (
                "ix0",
                (
                    vec![Value::I32(1), Value::F64(1.0), Value::I32(1)],
                    (Unbounded, Unbounded),
                ),
                in_t(None, ValueRule::InvalidRange),
            ),
            (
                "ix0",
                (
                    vec![Value::I32(1), Value::F64(1.0)],
                    (Included(Value::F64(1.0)), Unbounded),
                ),
                in_t(None, ValueRule::InvalidRange),
            ),
            // A malformed prefix length before a value's rule.
            (
                "ix0",
                (
                    vec![Value::Bool(true), Value::F64(1.0), Value::I32(1)],
                    (Unbounded, Unbounded),
                ),
                in_t(None, ValueRule::InvalidRange),
            ),
            (
                "ix0",
                (
                    vec![Value::Bool(true), Value::Bool(true)],
                    (Included(Value::Null), Unbounded),
                ),
                in_t(None, ValueRule::InvalidRange),
            ),
            // The prefix in key order, then the lower and the upper bound.
            (
                "ix0",
                (
                    vec![Value::Bool(true), Value::Bool(true)],
                    (Unbounded, Unbounded),
                ),
                in_t(Some("a"), ValueRule::WrongType),
            ),
            (
                "ix0",
                (
                    vec![Value::I32(1)],
                    (Included(Value::Null), Included(Value::Bool(true))),
                ),
                in_t(Some("f"), ValueRule::InvalidRange),
            ),
            (
                "ix0",
                (
                    vec![Value::I32(1)],
                    (Included(Value::Bool(true)), Included(Value::Null)),
                ),
                in_t(Some("f"), ValueRule::WrongType),
            ),
        ];
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let mut write = store.write().unwrap();
        let write_table = write.open_table("t").unwrap();
        for (index, (prefix, bounds), expected) in &cases {
            let what = format!("{index} {prefix:?} {bounds:?}");
            let errors = [
                read.index_entries(&table, index, prefix, bounds.clone())
                    .err(),
                read.index_rows(&table, index, prefix, bounds.clone()).err(),
                write
                    .index_entries(&write_table, index, prefix, bounds.clone())
                    .err(),
                write
                    .index_rows(&write_table, index, prefix, bounds.clone())
                    .err(),
            ];
            for error in errors {
                assert_eq!(invalid_value(error.unwrap(), folder), *expected, "{what}");
            }
        }
        // A public validation failure leaves the write able to commit (B7).
        write
            .insert_row(
                &write_table,
                &[("id", Value::I64(500)), ("g", Value::I32(1))],
            )
            .unwrap();
        write.commit().unwrap();
        drop(read);
        let read = store.read().unwrap();
        assert_eq!(read.row_count(&read.open_table("t").unwrap()).unwrap(), 81);
    });
}

/// AC-IX4 (IX9, B8): a prefix, or a prefix and a bound, that encoded as in an entry passes 2,048 bytes fails with an
/// invalid size of a prefix with its size, the table and the index, also when the interval is empty or inverted;
/// one that fits scans.
#[test]
fn ac_ix4_a_prefix_that_does_not_fit_fails_with_its_size() {
    stores(|store, folder| {
        intervals_store(store);
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let long = |len: usize| text(&"x".repeat(len));
        let text_len =
            |value: &Value<'_>| key_len(&[ColumnType::String], std::slice::from_ref(value));
        let i32_len = |n: i32| key_len(&[ColumnType::I32], &[Value::I32(n)]);
        // The longest text whose element fits alone, one too long alone, and one too long with a bound.
        let fits = (1..2100)
            .rev()
            .map(long)
            .find(|v| text_len(v) <= 2048)
            .unwrap();
        assert_eq!(text_len(&fits), 2048);
        let over = long(2100);
        let with_bound = (1..2100)
            .map(long)
            .find(|v| text_len(v) + i32_len(0) > 2048)
            .unwrap();
        let cases: Vec<(Interval, usize)> = vec![
            (
                (vec![over.clone()], (Unbounded, Unbounded)),
                text_len(&over),
            ),
            (
                (
                    vec![with_bound.clone()],
                    (Included(Value::I32(0)), Unbounded),
                ),
                text_len(&with_bound) + i32_len(0),
            ),
            (
                (
                    vec![with_bound.clone()],
                    (Unbounded, Excluded(Value::I32(0))),
                ),
                text_len(&with_bound) + i32_len(0),
            ),
            // Inverted, so empty: the size of the prefix with the lower bound, which comes first.
            (
                (
                    vec![with_bound.clone()],
                    (Included(Value::I32(500)), Included(Value::I32(1))),
                ),
                text_len(&with_bound) + i32_len(500),
            ),
        ];
        for ((prefix, bounds), received) in &cases {
            let errors = [
                read.index_entries(&table, "ix_s", prefix, bounds.clone())
                    .err(),
                read.index_rows(&table, "ix_s", prefix, bounds.clone())
                    .err(),
            ];
            for error in errors {
                assert_eq!(
                    prefix_too_large(error.unwrap(), folder),
                    ("t".to_owned(), "ix_s".to_owned(), *received as u64),
                    "{bounds:?}"
                );
            }
        }
        // A prefix that fits alone scans, and finds nothing.
        let found = read
            .index_entries(&table, "ix_s", &[fits], ..)
            .unwrap()
            .count();
        assert_eq!(found, 0);
        // A prefix with a bound of exactly 2,048 bytes, the lower one when it has two, scans too, also inverted.
        let bounds = [
            (Included(Value::I32(0)), Unbounded, 0),
            (Unbounded, Excluded(Value::I32(0)), 0),
            (Included(Value::I32(500)), Included(Value::I32(1)), 500),
        ];
        for (lower, upper, counted) in bounds {
            let bounds = (lower, upper);
            let prefix = [(1..2100)
                .rev()
                .map(long)
                .find(|v| text_len(v) + i32_len(counted) <= 2048)
                .unwrap()];
            assert_eq!(text_len(&prefix[0]) + i32_len(counted), 2048);
            let entries = read.index_entries(&table, "ix_s", &prefix, bounds.clone());
            assert_eq!(entries.unwrap().count(), 0, "{bounds:?}");
            let rows = read.index_rows(&table, "ix_s", &prefix, bounds.clone());
            assert_eq!(rows.unwrap().count(), 0, "{bounds:?}");
        }
    });
}

/// AC-IX4 and AC-IX9 (IX9, IX10): a scan of an index that the table does not have fails with a different schema, with
/// the table and the index, also with a malformed interval (esquema distinto before valor no válido); a malformed
/// interval with a prefix that does not fit fails with the invalid value; an index name of 0 or 1,025 bytes fails
/// with its size before anything else, a handle of a deleted table then with not found, before the index.
#[test]
fn ac_ix9_the_precedence_of_the_failures_of_a_scan() {
    stores(|store, folder| {
        intervals_store(store);
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let missing = [
            (vec![], (Unbounded, Unbounded)),
            (vec![Value::Bool(true)], (Unbounded, Unbounded)),
            (vec![], (Included(Value::Null), Unbounded)),
        ];
        let name_1024 = "m".repeat(1024);
        for (prefix, bounds) in &missing {
            for name in ["missing", name_1024.as_str()] {
                let errors = [
                    read.index_entries(&table, name, prefix, bounds.clone())
                        .err(),
                    read.index_rows(&table, name, prefix, bounds.clone()).err(),
                ];
                for error in errors {
                    assert_eq!(
                        missing_index(error.unwrap(), folder),
                        ("t".to_owned(), name.to_owned())
                    );
                }
            }
        }
        // A malformed interval and a prefix that does not fit: the invalid value.
        let over = text(&"x".repeat(2100));
        let malformed: [(Interval, _); 2] = [
            (
                (vec![over.clone(), Value::I64(1)], (Unbounded, Unbounded)),
                in_t(Some("a"), ValueRule::WrongType),
            ),
            (
                (vec![over.clone()], (Included(Value::Null), Unbounded)),
                in_t(Some("a"), ValueRule::InvalidRange),
            ),
        ];
        for ((prefix, bounds), expected) in &malformed {
            let error = read
                .index_entries(&table, "ix_s", prefix, bounds.clone())
                .err();
            assert_eq!(invalid_value(error.unwrap(), folder), *expected);
        }
        // The name's size first.
        for len in [0, 1025] {
            let name = "n".repeat(len);
            let error = read
                .index_rows(&table, &name, &[Value::Bool(true)], ..)
                .err();
            assert_eq!(
                invalid_size(error.unwrap(), folder),
                (SizeSubject::IndexName, len as u64, 1, 1024)
            );
        }
        drop(read);

        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let error = write.index_rows(&table, "missing", &[], ..).err();
        assert_eq!(
            missing_index(error.unwrap(), folder),
            ("t".to_owned(), "missing".to_owned())
        );
        write.delete_table("t").unwrap();
        let long = "n".repeat(1025);
        let error = write.index_entries(&table, &long, &[], ..).err();
        assert_eq!(
            invalid_size(error.unwrap(), folder),
            (SizeSubject::IndexName, 1025, 1, 1024)
        );
        for index in ["missing", "ix0"] {
            let error = write
                .index_entries(&table, index, &[Value::Bool(true)], ..)
                .err();
            assert_eq!(not_found(error.unwrap(), folder), "t");
            let error = write.index_rows(&table, index, &[], ..).err();
            assert_eq!(not_found(error.unwrap(), folder), "t");
        }
        write.abort();
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC-IX5: a property test against the model of N5

/// The columns of the property test besides `id`, with their types; each allows nulls.
fn random_columns() -> Vec<(&'static str, ColumnType)> {
    vec![
        ("c0", ColumnType::I32),
        ("c1", ColumnType::F64),
        ("c2", ColumnType::String),
        ("c3", ColumnType::Bool),
        ("c4", ColumnType::Uuid(UuidOrder::SqlServer)),
    ]
}

/// The values the property test draws for the column `name`, null among them: few, so that keys repeat, with `-0` and
/// `+0`, NaNs of two bits, texts that are prefixes of each other with U+0000, and `uuid`s that sort differently in
/// SQL Server's order and in their bytes'.
fn random_pool(name: &str) -> Vec<Value<'static>> {
    let uuid = |first: u8, last: u8| {
        let mut bytes = [0u8; 16];
        bytes[0] = first;
        bytes[15] = last;
        Value::Uuid(Uuid(bytes))
    };
    let mut values = match name {
        "c0" => [-1, 0, 1, 2].map(Value::I32).to_vec(),
        "c1" => vec![
            Value::F64(-0.0),
            Value::F64(0.0),
            Value::F64(1.5),
            Value::F64(f64::NAN),
            Value::F64(f64::from_bits(0x7FF0_0000_0000_0009)),
            Value::F64(f64::NEG_INFINITY),
        ],
        "c2" => ["", "a", "a\0", "b"].map(text).to_vec(),
        "c3" => vec![Value::Bool(false), Value::Bool(true)],
        "c4" => vec![uuid(1, 2), uuid(2, 1), uuid(3, 3)],
        other => panic!("no pool for {other}"),
    };
    values.push(Value::Null);
    values
}

fn pick<T: Clone>(rng: &mut Lcg, items: &[T]) -> T {
    items[rng.next() as usize % items.len()].clone()
}

/// A random index of one to four distinct columns, each in a random order, unique one time in three.
fn random_index(rng: &mut Lcg, name: &str) -> IndexDefinition {
    let mut columns: Vec<&str> = random_columns().iter().map(|(name, _)| *name).collect();
    rng.shuffle(&mut columns);
    let len = 1 + rng.next() as usize % 4;
    let key = (columns[..len].iter())
        .map(|column| ordered(column, pick(rng, &ORDERS)))
        .collect();
    let index = IndexDefinition::new(name, key);
    if rng.next().is_multiple_of(3) {
        index.unique()
    } else {
        index
    }
}

/// A random row of the property test, or the columns of an update: each column given one time in two when `partial`.
fn random_values(rng: &mut Lcg, partial: bool) -> Vec<(&'static str, Value<'static>)> {
    let mut values = Vec::new();
    for (name, _) in random_columns() {
        if !partial || rng.next().is_multiple_of(2) {
            values.push((name, pick(rng, &random_pool(name))));
        }
    }
    values
}

/// The unique index of `indexes`, in the order of their names, whose key `row` repeats with another row of `rows`
/// than `id`, none of its values null (N7), as the model of N5 compares them.
fn model_duplicate(
    schema: &TableSchema,
    rows: &[ModelRow],
    id: i64,
    row: &BTreeMap<String, Value<'static>>,
) -> Option<String> {
    let mut names: Vec<&str> = schema.indexes().iter().map(|ix| ix.name.as_str()).collect();
    names.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    names.into_iter().find_map(|name| {
        let index = schema.index(name).unwrap();
        if !index.unique {
            return None;
        }
        let keys = model_keys(schema, name);
        let values: Vec<&Value<'static>> = keys.iter().map(|key| &row[&key.name]).collect();
        if values.iter().any(|v| matches!(v, Value::Null)) {
            return None;
        }
        let repeats = rows.iter().filter(|other| other.0 != id).any(|other| {
            (keys.iter().zip(&values)).all(|(key, v)| match &other.1[&key.name] {
                Value::Null => false,
                other => model_cmp(&key.ty, other, v).is_eq(),
            })
        });
        repeats.then(|| name.to_owned())
    })
}

/// The row of the model after normalizing `values` as a row stores them (N4): `-0` as `+0`, any NaN as one.
fn normalized(
    values: impl IntoIterator<Item = (String, Value<'static>)>,
) -> BTreeMap<String, Value<'static>> {
    values
        .into_iter()
        .map(|(name, value)| {
            let value = match value {
                Value::F64(x) if x.is_nan() => Value::F64(f64::NAN),
                // `-0` as `+0`.
                Value::F64(x) => Value::F64(x + 0.0),
                other => other,
            };
            (name, value)
        })
        .collect()
}

/// The primary key of a duplicate key from a unique index, and its index; `None` for a primary key.
#[track_caller]
fn duplicate_index(error: Error) -> Option<String> {
    match error {
        Error::DuplicateKey { index, .. } => index.map(String::from),
        other => panic!("expected a duplicate key, got {other}"),
    }
}

/// A bound absent, or on a value of `pool` included or excluded.
fn random_bound(rng: &mut Lcg, pool: &[Value<'static>]) -> Bound<Value<'static>> {
    match rng.next() % 3 {
        0 => Unbounded,
        1 => Included(pick(rng, pool)),
        _ => Excluded(pick(rng, pool)),
    }
}

/// Random intervals of the index of key `keys` over `rows`: prefixes of the values the rows have, or of the pool, and
/// random bounds on the next column.
fn random_intervals(
    rng: &mut Lcg,
    keys: &[ModelKey],
    rows: &[ModelRow],
    n: usize,
) -> Vec<Interval> {
    (0..n)
        .map(|_| {
            let k = rng.next() as usize % (keys.len() + 1);
            let from_row = !rows.is_empty() && rng.next().is_multiple_of(2);
            let row = (!rows.is_empty()).then(|| pick(rng, rows));
            let prefix = keys[..k]
                .iter()
                .map(|key| match &row {
                    Some(row) if from_row => row.1[&key.name].clone(),
                    _ => pick(rng, &random_pool(&key.name)),
                })
                .collect();
            let bounds = if k < keys.len() && !rng.next().is_multiple_of(3) {
                let mut pool = random_pool(&keys[k].name);
                pool.pop();
                (random_bound(rng, &pool), random_bound(rng, &pool))
            } else {
                (Unbounded, Unbounded)
            };
            (prefix, bounds)
        })
        .collect()
}

/// AC-IX5 (IX7, IX8, IX9, N5, N7): over tables with random indexes of one to four columns, in every direction and
/// null order, unique and not unique, random inserts, updates, inserts or updates and deletes give the duplicate keys
/// of the model, and then each index scans, forward and backward, for entries and for rows, exactly the rows of the
/// model in its order: the whole index and random intervals of it, in the write and after the commit. The model's rows
/// come from the writes and their expected outcomes, never from the table under test.
#[test]
fn ac_ix5_random_indexes_follow_the_model_of_n5() {
    stores(|store, _| {
        let mut rng = Lcg(5);
        // The duplicates of a unique index, and the intervals that give some rows but not all.
        let (mut index_duplicates, mut partial) = (0, 0);
        for round in 0..4 {
            let name = format!("t{round}");
            let mut columns = vec![Column::new("id", ColumnType::I64).not_null()];
            columns.extend(
                random_columns()
                    .into_iter()
                    .map(|(name, ty)| Column::new(name, ty)),
            );
            let mut write = store.write().unwrap();
            let table = write
                .open_table_with(&name, TableMode::CreateNew(&Schema::new(columns, &["id"])))
                .unwrap();
            for i in 0..4 {
                let definition = random_index(&mut rng, &format!("ix{i}"));
                if let Err(error) = write.create_index(&table, &definition, IndexMode::CreateNew) {
                    // A random key equal to another's.
                    assert_eq!(error.kind(), ErrorKind::InvalidSchema);
                }
            }
            let schema = write.table_schema(&table).unwrap();
            assert!(!schema.indexes().is_empty(), "round {round}");
            let mut duplicates = 0;
            // The rows the writes leave, kept from the writes and their expected outcomes, never read from the table.
            let mut model: BTreeMap<i64, BTreeMap<String, Value<'static>>> = BTreeMap::new();
            for _ in 0..150 {
                let rows: Vec<ModelRow> = model.clone().into_iter().collect();
                let id = (rng.next() % 24) as i64;
                let existing = model.get(&id).cloned().map(|values| (id, values));
                match rng.next() % 5 {
                    // Insert, update and insert or update.
                    op @ 0..=2 => {
                        let given = random_values(&mut rng, op != 0);
                        let mut row = [("id", Value::I64(id))].to_vec();
                        row.extend(given.iter().cloned());
                        let merged = match &existing {
                            Some((_, old)) => {
                                let mut merged = old.clone();
                                merged.extend(normalized(
                                    given.iter().map(|(n, v)| ((*n).to_owned(), v.clone())),
                                ));
                                merged
                            }
                            None => {
                                let mut merged: BTreeMap<String, Value<'static>> = random_columns()
                                    .iter()
                                    .map(|(n, _)| ((*n).to_owned(), Value::Null))
                                    .collect();
                                merged.extend(normalized(
                                    given.iter().map(|(n, v)| ((*n).to_owned(), v.clone())),
                                ));
                                merged
                            }
                        };
                        let outcome = match op {
                            0 => write.insert_row(&table, &row).map(|()| true),
                            1 => write.update_row(&table, &row),
                            _ => write.upsert_row(&table, &row).map(|_| true),
                        };
                        let expected = match (op, &existing) {
                            (0, Some(_)) => Err(None),
                            (1, None) => Ok(false),
                            _ => match model_duplicate(&schema, &rows, id, &merged) {
                                Some(index) => Err(Some(index)),
                                None => Ok(true),
                            },
                        };
                        match (outcome, expected) {
                            (Ok(done), Ok(expected)) => {
                                assert_eq!(done, expected);
                                if done {
                                    model.insert(id, merged);
                                }
                            }
                            (Err(error), Err(index)) => {
                                index_duplicates += usize::from(index.is_some());
                                assert_eq!(duplicate_index(error), index, "round {round}");
                                duplicates += 1;
                            }
                            (outcome, expected) => {
                                panic!("round {round}: {outcome:?}, expected {expected:?}")
                            }
                        }
                    }
                    3 => {
                        let deleted = write.delete_row(&table, &[Value::I64(id)]).unwrap();
                        assert_eq!(deleted, existing.is_some());
                        model.remove(&id);
                    }
                    _ => {
                        let end = id + (rng.next() % 4) as i64;
                        write
                            .delete_range(&table, &[], Value::I64(id)..Value::I64(end))
                            .unwrap();
                        model.retain(|key, _| !(id..end).contains(key));
                    }
                }
            }
            assert!(duplicates > 0, "round {round}: some writes repeat a key");
            let rows: Vec<ModelRow> = model.into_iter().collect();
            let table_rows = write_rows(&write, &table);
            assert_eq!(table_rows.len(), rows.len(), "round {round}");
            for row in &table_rows {
                assert_row(&rows, row);
            }
            for index in schema.indexes() {
                let keys = model_keys(&schema, &index.name);
                let mut intervals = vec![(vec![], (Unbounded, Unbounded))];
                intervals.extend(random_intervals(&mut rng, &keys, &rows, 30));
                for interval in &intervals {
                    let expected = model_scan(&keys, &rows, interval);
                    partial += usize::from(!expected.is_empty() && expected.len() < rows.len());
                    let found = scans!(write, &table, &index.name, keys, rows, interval);
                    assert_scans(found, &expected, &format!("{} {interval:?}", index.name));
                }
            }
            write.commit().unwrap();
            let read = store.read().unwrap();
            let table = read.open_table(&name).unwrap();
            for index in schema.indexes() {
                let keys = model_keys(&schema, &index.name);
                let mut intervals = vec![(vec![], (Unbounded, Unbounded))];
                intervals.extend(random_intervals(&mut rng, &keys, &rows, 30));
                for interval in &intervals {
                    let expected = model_scan(&keys, &rows, interval);
                    let found = scans!(read, &table, &index.name, keys, rows, interval);
                    assert_scans(found, &expected, &format!("{} {interval:?}", index.name));
                }
            }
        }
        assert!(index_duplicates > 0, "some writes repeat a unique index");
        assert!(
            partial > 20,
            "some intervals select part of an index: {partial}"
        );
    });
}
