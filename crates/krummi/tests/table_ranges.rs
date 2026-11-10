//! Ranges of the primary key of a table through the public API (tables with schema PRD 7.1, T7): scans of every prefix
//! and every combination of included, excluded and absent bounds, forward and backward, malformed ranges and the sizes
//! of their values, range delete and the rows that remain (AC9, E12, AC-N3); the order of N5 for each orderable type,
//! alone and before another key column, with SQL Server's `uuid` order checked on `SqlGuid` vectors (AC-N1, AC17);
//! and a property test of random tables against a reference model of N5 that compares values by their type (AC-N1).
//!
//! Deterministic: the random sequences come from fixed seeds. The criteria run on a store on disk and on a temporary
//! one, except the property test, which runs on a temporary store.

mod support;

use std::borrow::Cow;
use std::cmp::Ordering;
use std::ops::Bound::{self, Excluded, Included, Unbounded};
use std::path::Path;

use krummi::ids::{Ulid, Uuid};
use krummi::table::key::key_len;
use krummi::table::value::{
    ColumnType, DATE_MAX, DATE_MIN, DURATION_MAX, Decimal, TIME_MAX, TIMESTAMP_MAX, TIMESTAMP_MIN,
    UuidOrder, Value, normalize,
};
use krummi::{
    Column, Error, ErrorKind, ReadTable, ReadTransaction, Schema, SizeSubject, Store, StoreOptions,
    TableMode, TableRow, ValueRule, WriteTable, WriteTransaction,
};
use support::*;

// ---------------------------------------------------------------------------------------------------------------
// Helpers

fn s(text: &str) -> Value<'_> {
    Value::String(Cow::Borrowed(text))
}

fn b(bytes: &[u8]) -> Value<'_> {
    Value::Bytes(Cow::Borrowed(bytes))
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

/// Values compared by their debug form, so that the canonical NaN equals itself and `-0` differs from `+0`.
fn same(a: &[Value<'_>], b: &[Value<'_>]) -> bool {
    format!("{a:?}") == format!("{b:?}")
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
            sequence,
            rule,
        } => {
            assert_eq!(f, folder_of(folder));
            assert_eq!(sequence, None);
            (table, column, rule)
        }
        other => panic!("expected an invalid value, got {other}"),
    }
}

/// The expected data of an invalid value in the column `column` of the table `t`.
fn in_t(column: Option<&str>, rule: ValueRule) -> (Option<String>, Option<String>, ValueRule) {
    (Some("t".to_owned()), column.map(str::to_owned), rule)
}

type Bounds<'v> = (Bound<Value<'v>>, Bound<Value<'v>>);

/// Every bound on the values `values`: absent, and each value included and excluded.
fn every_bound(values: &[Value<'static>]) -> Vec<Bound<Value<'static>>> {
    let mut bounds = vec![Unbounded];
    for value in values {
        bounds.push(Included(value.clone()));
        bounds.push(Excluded(value.clone()));
    }
    bounds
}

/// Whether `cmp`, the order of a value against a bound's value, puts the value at or after `lower`.
fn after(lower: &Bound<Value<'_>>, cmp: impl Fn(&Value<'_>) -> Ordering) -> bool {
    match lower {
        Included(v) => cmp(v).is_ge(),
        Excluded(v) => cmp(v).is_gt(),
        Unbounded => true,
    }
}

/// Whether `cmp` puts the value at or before `upper`.
fn before(upper: &Bound<Value<'_>>, cmp: impl Fn(&Value<'_>) -> Ordering) -> bool {
    match upper {
        Included(v) => cmp(v).is_le(),
        Excluded(v) => cmp(v).is_lt(),
        Unbounded => true,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// AC9: a table with a primary key of two columns

/// `a i32 not null, b i64 not null, v string`, primary key `(a, b)`.
fn pairs() -> Schema {
    Schema::new(
        vec![
            Column::new("a", ColumnType::I32).not_null(),
            Column::new("b", ColumnType::I64).not_null(),
            Column::new("v", ColumnType::String),
        ],
        &["a", "b"],
    )
}

const AS: [i32; 3] = [1, 2, 3];
const BS: [i64; 3] = [10, 20, 30];

/// Every key of the table of [`fill_pairs`], in key order.
fn all_pairs() -> Vec<(i32, i64)> {
    AS.iter()
        .flat_map(|&a| BS.iter().map(move |&b| (a, b)))
        .collect()
}

/// Inserts the rows of [`all_pairs`] in reverse, each with `v` the text `a-b`.
fn fill_pairs(write: &mut WriteTransaction<'_>, table: &WriteTable<'_>) {
    for (a, b) in reversed(all_pairs()) {
        let v = format!("{a}-{b}");
        write
            .insert_row(
                table,
                &[("a", Value::I32(a)), ("b", Value::I64(b)), ("v", s(&v))],
            )
            .unwrap();
    }
}

/// A store with the committed table `t` of [`fill_pairs`].
fn pairs_store(store: &Store) {
    let mut write = store.write().unwrap();
    let table = create_table(&mut write, "t", &pairs());
    fill_pairs(&mut write, &table);
    write.commit().unwrap();
}

/// The key of a row of [`pairs`], after checking that its columns are all there and its `v` is its key's.
fn pair_of(row: &TableRow<'_>) -> (i32, i64) {
    assert_eq!(row.len(), 3);
    match (row.get("a"), row.get("b"), row.get("v")) {
        (Some(&Value::I32(a)), Some(&Value::I64(b)), Some(Value::String(v))) => {
            assert_eq!(v.as_ref(), format!("{a}-{b}"));
            (a, b)
        }
        other => panic!("not a row of the table: {other:?}"),
    }
}

fn pairs_of<'t>(scan: impl Iterator<Item = krummi::Result<TableRow<'t>>>) -> Vec<(i32, i64)> {
    collect(scan, |row| pair_of(&row))
}

/// The keys of [`all_pairs`] that the model puts in the range of `prefix` and `bounds` (E12).
fn model_pairs(prefix: &[Value<'_>], bounds: &Bounds<'_>) -> Vec<(i32, i64)> {
    all_pairs()
        .into_iter()
        .filter(|&(a, b)| {
            let key = [Value::I32(a), Value::I64(b)];
            let k = prefix.len();
            if !same(&key[..k], prefix) {
                return false;
            }
            if k == 2 {
                return true;
            }
            let cmp = |bound: &Value<'_>| match (&key[k], bound) {
                (Value::I32(x), Value::I32(y)) => x.cmp(y),
                (Value::I64(x), Value::I64(y)) => x.cmp(y),
                _ => unreachable!(),
            };
            after(&bounds.0, cmp) && before(&bounds.1, cmp)
        })
        .collect()
}

/// The ranges of AC9: every prefix of [`pairs`], present or not, and on the next column every combination of absent
/// bounds and bounds included and excluded on values that are rows and values between and around them.
fn pair_ranges() -> Vec<(Vec<Value<'static>>, Bounds<'static>)> {
    let a_values: Vec<Value<'static>> = [0, 1, 2, 3, 4].map(Value::I32).to_vec();
    let b_values: Vec<Value<'static>> = [5, 10, 15, 20, 30, 35].map(Value::I64).to_vec();
    let mut ranges = Vec::new();
    for lower in every_bound(&a_values) {
        for upper in every_bound(&a_values) {
            ranges.push((vec![], (lower.clone(), upper)));
        }
    }
    for a in [0, 2, 3] {
        for lower in every_bound(&b_values) {
            for upper in every_bound(&b_values) {
                ranges.push((vec![Value::I32(a)], (lower.clone(), upper)));
            }
        }
    }
    for (a, b) in [(2, 20), (2, 25), (4, 10)] {
        ranges.push((vec![Value::I32(a), Value::I64(b)], (Unbounded, Unbounded)));
    }
    ranges
}

/// AC9 (E12, AC-N1): every prefix and every combination of included, excluded and absent bounds returns, forward,
/// backward and from both ends at once, the rows of the range in key order, in a read and in a write; an inverted
/// range returns none and is not an error.
#[test]
fn ac9_every_prefix_and_bound_scans_forward_and_backward() {
    stores(|store, _| {
        pairs_store(store);
        let read = store.read().unwrap();
        let read_table = read.open_table("t").unwrap();
        let write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let ranges = pair_ranges();
        let mut empty = 0;
        for (prefix, bounds) in &ranges {
            let expected = model_pairs(prefix, bounds);
            empty += usize::from(expected.is_empty());
            let scan = || {
                read.range_rows(&read_table, prefix, bounds.clone())
                    .unwrap()
            };
            assert_eq!(pairs_of(scan()), expected, "{prefix:?} {bounds:?}");
            assert_eq!(pairs_of(scan().rev()), reversed(expected.clone()));
            let scan = || write.range_rows(&table, prefix, bounds.clone()).unwrap();
            assert_eq!(pairs_of(scan()), expected, "{prefix:?} {bounds:?}");
            assert_eq!(pairs_of(scan().rev()), reversed(expected.clone()));
            assert_eq!(alternate(scan(), |row| pair_of(&row)), expected);
        }
        assert!(
            ranges.len() > 400 && empty > 100,
            "{} ranges, {empty} empty",
            ranges.len()
        );

        // Inverted ranges, and a range with a bound excluded at both ends.
        for bounds in [
            (Included(Value::I32(3)), Included(Value::I32(1))),
            (Excluded(Value::I32(2)), Excluded(Value::I32(2))),
            (Included(Value::I32(2)), Excluded(Value::I32(2))),
        ] {
            assert_eq!(pairs_of(write.range_rows(&table, &[], bounds).unwrap()), []);
        }
        let inverted = (Included(Value::I64(30)), Excluded(Value::I64(10)));
        assert_eq!(
            pairs_of(
                read.range_rows(&read_table, &[Value::I32(2)], inverted)
                    .unwrap()
            ),
            []
        );
        // The rows of a scan are those of a read by their key.
        let row = write
            .range_rows(&table, &[Value::I32(2), Value::I64(20)], ..)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        let got = write
            .get_row(&table, &[Value::I32(2), Value::I64(20)])
            .unwrap()
            .unwrap();
        assert!(same(row.values(), got.values()));
    });
}

/// AC9 (E12, E17): each malformed range fails with an invalid value, with the table, and the column when a value
/// breaks its rules: a value of another type, out of its domain or null, in the prefix or in a bound; a prefix longer
/// than the key and a bound when the prefix gives every column, with an invalid range; a value's rule before the
/// range's; and a handle of a deleted table fails with not found before any of them.
#[test]
fn ac9_malformed_ranges_fail_with_invalid_value() {
    stores(|store, folder| {
        pairs_store(store);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let dates = create_table(
            &mut write,
            "d",
            &Schema::new(vec![Column::new("d", ColumnType::Date).not_null()], &["d"]),
        );
        let one = Value::I32(1);
        let cases: Vec<(Vec<Value<'static>>, Bounds<'static>, _)> = vec![
            (
                vec![Value::I64(1)],
                (Unbounded, Unbounded),
                in_t(Some("a"), ValueRule::WrongType),
            ),
            (
                vec![Value::Null],
                (Unbounded, Unbounded),
                in_t(Some("a"), ValueRule::NullNotAllowed),
            ),
            (
                vec![],
                (Included(Value::Null), Unbounded),
                in_t(Some("a"), ValueRule::NullNotAllowed),
            ),
            (
                vec![],
                (Unbounded, Excluded(s("1").into_owned())),
                in_t(Some("a"), ValueRule::WrongType),
            ),
            (
                vec![one.clone()],
                (Excluded(Value::I32(10)), Unbounded),
                in_t(Some("b"), ValueRule::WrongType),
            ),
            (
                vec![one.clone()],
                (Unbounded, Included(Value::Null)),
                in_t(Some("b"), ValueRule::NullNotAllowed),
            ),
            (
                vec![one.clone(), Value::I64(10), Value::I64(10)],
                (Unbounded, Unbounded),
                in_t(None, ValueRule::InvalidRange),
            ),
            (
                vec![one.clone(), Value::I64(10)],
                (Included(Value::I64(10)), Unbounded),
                in_t(None, ValueRule::InvalidRange),
            ),
            (
                vec![one.clone(), Value::I64(10)],
                (Unbounded, Excluded(Value::I64(20))),
                in_t(None, ValueRule::InvalidRange),
            ),
            // A value's rule comes before the range's.
            (
                vec![Value::Bool(true), Value::I64(10), Value::I64(10)],
                (Unbounded, Unbounded),
                in_t(Some("a"), ValueRule::WrongType),
            ),
        ];
        for (prefix, bounds, expected) in cases {
            let error = write
                .range_rows(&table, &prefix, bounds.clone())
                .err()
                .unwrap();
            assert_eq!(
                invalid_value(error, folder),
                expected,
                "{prefix:?} {bounds:?}"
            );
            let error = write
                .delete_range(&table, &prefix, bounds.clone())
                .unwrap_err();
            assert_eq!(
                invalid_value(error, folder),
                expected,
                "{prefix:?} {bounds:?}"
            );
        }
        // Out of the domain of a date, in the prefix and in a bound.
        let out = Value::Date(DATE_MAX + 1);
        for (prefix, bounds) in [
            (vec![out.clone()], (Unbounded, Unbounded)),
            (vec![], (Unbounded, Included(out.clone()))),
            (vec![], (Excluded(Value::Date(DATE_MIN - 1)), Unbounded)),
        ] {
            let error = write.range_rows(&dates, &prefix, bounds).err().unwrap();
            assert_eq!(
                invalid_value(error, folder),
                (
                    Some("d".to_owned()),
                    Some("d".to_owned()),
                    ValueRule::OutOfDomain
                )
            );
        }
        assert_eq!(write.row_count(&table).unwrap(), 9);
        write.commit().unwrap();

        // In a read, the same failures.
        let read = store.read().unwrap();
        let read_table = read.open_table("t").unwrap();
        let error = read
            .range_rows(&read_table, &[Value::Null], ..)
            .err()
            .unwrap();
        assert_eq!(
            invalid_value(error, folder),
            in_t(Some("a"), ValueRule::NullNotAllowed)
        );
        let three = [Value::I32(1), Value::I64(1), Value::I64(1)];
        let error = read.range_rows(&read_table, &three, ..).err().unwrap();
        assert_eq!(
            invalid_value(error, folder),
            in_t(None, ValueRule::InvalidRange)
        );
        drop(read);

        // A handle of a deleted table: not found, before the invalid value.
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.delete_table("t").unwrap();
        let error = write.range_rows(&table, &[Value::Null], ..).err().unwrap();
        assert_eq!(not_found(error, folder), "t");
        let error = write.delete_range(&table, &[Value::Null], ..).unwrap_err();
        assert_eq!(not_found(error, folder), "t");
    });
}

/// AC9 (E12, E17, E18): a prefix value or a bound that does not fit in the 2,048 bytes of a key fails with invalid
/// size, with its stored size, even on an empty table, and after the invalid values; a bound of exactly 2,048 bytes
/// works on either side, included and excluded (document 3, the `B ‖ FF` rule).
#[test]
fn ac9_bounds_that_do_not_fit_in_a_key_fail_with_their_size() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let schema = Schema::new(
            vec![
                Column::new("k", ColumnType::Bytes).not_null(),
                Column::new("n", ColumnType::I32).not_null(),
            ],
            &["k", "n"],
        );
        let table = create_table(&mut write, "t", &schema);
        // A `bytes` element of n bytes without `00` takes n + 2.
        let long = vec![0x41; MAX_KEY - 1];
        let size = |error| invalid_size(error, folder);
        let expected = (
            SizeSubject::PrimaryKey,
            MAX_KEY as u64 + 1,
            1,
            MAX_KEY as u64,
        );
        let cases: Vec<(Vec<Value<'_>>, Bounds<'_>)> = vec![
            (vec![b(&long)], (Unbounded, Unbounded)),
            (vec![], (Included(b(&long)), Unbounded)),
            (vec![], (Unbounded, Excluded(b(&long)))),
            (vec![], (Included(b(b"a")), Included(b(&long)))),
            // Inverted, and still too large.
            (vec![], (Included(b(&long)), Excluded(b(b"a")))),
        ];
        for (prefix, bounds) in &cases {
            let error = write
                .range_rows(&table, prefix, bounds.clone())
                .err()
                .unwrap();
            assert_eq!(size(error), expected, "{bounds:?}");
            let error = write
                .delete_range(&table, prefix, bounds.clone())
                .unwrap_err();
            assert_eq!(size(error), expected);
        }
        // A prefix of 2,047 bytes and a bound on the next column that takes it over.
        let fits = vec![0x41; MAX_KEY - 3];
        let error = write
            .range_rows(&table, &[b(&fits)], Value::I32(1)..)
            .err()
            .unwrap();
        assert_eq!(
            size(error),
            (
                SizeSubject::PrimaryKey,
                MAX_KEY as u64 + 1,
                1,
                MAX_KEY as u64
            )
        );
        // An invalid value comes before the size (E17).
        let error = write
            .range_rows(&table, &[b(&long)], (Included(Value::I64(1)), Unbounded))
            .err()
            .unwrap();
        assert_eq!(
            invalid_value(error, folder),
            in_t(Some("n"), ValueRule::WrongType)
        );

        // Keys of exactly 2,048 bytes: as bounds, `B ‖ FF` would take 2,049.
        let one = Schema::new(vec![Column::new("k", ColumnType::Bytes).not_null()], &["k"]);
        let keys = create_table(&mut write, "k", &one);
        let (x, y, shorter) = (
            vec![0x41; MAX_KEY - 2],
            vec![0x42; MAX_KEY - 2],
            vec![0x41; MAX_KEY - 3],
        );
        for key in [&x, &y, &shorter] {
            write.insert_row(&keys, &[("k", b(key))]).unwrap();
        }
        let scan_of = |write: &WriteTransaction<'_>, bounds: Bounds<'_>| -> Vec<Vec<u8>> {
            collect(
                write.range_rows(&keys, &[], bounds).unwrap(),
                |row| match row.get("k") {
                    Some(Value::Bytes(k)) => k.to_vec(),
                    other => panic!("{other:?}"),
                },
            )
        };
        let scan = |bounds| scan_of(&write, bounds);
        assert_eq!(
            scan((Included(b(&x)), Included(b(&x)))),
            std::slice::from_ref(&x)
        );
        assert_eq!(scan((Excluded(b(&x)), Unbounded)), std::slice::from_ref(&y));
        assert_eq!(
            scan((Unbounded, Included(b(&x)))),
            [shorter.clone(), x.clone()]
        );
        assert_eq!(
            scan((Unbounded, Excluded(b(&x)))),
            std::slice::from_ref(&shorter)
        );
        assert_eq!(
            scan((Excluded(b(&x)), Excluded(b(&y)))),
            Vec::<Vec<u8>>::new()
        );
        assert_eq!(
            scan((Excluded(b(&shorter)), Included(b(&y)))),
            [x.clone(), y.clone()]
        );
        assert_eq!(
            write
                .delete_range(&keys, &[], (Included(b(&x)), Included(b(&x))))
                .unwrap(),
            1
        );
        assert_eq!(scan_of(&write, (Unbounded, Unbounded)), [shorter, y]);
        assert_eq!(write.row_count(&keys).unwrap(), 2);
    });
}

/// AC9 and AC-N3 (E12): deleting a range returns how many rows it deleted, and exactly the rows outside it remain,
/// in the write, in the row count and, after the commit, in a read.
#[test]
fn ac9_acn3_range_delete_leaves_the_rows_outside_the_range() {
    stores(|store, _| {
        pairs_store(store);
        let ranges = pair_ranges();
        let mut deleted_some = 0;
        for (prefix, bounds) in ranges.iter().step_by(7) {
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            let gone = model_pairs(prefix, bounds);
            let deleted = write.delete_range(&table, prefix, bounds.clone()).unwrap();
            assert_eq!(deleted, gone.len() as u64, "{prefix:?} {bounds:?}");
            let left: Vec<(i32, i64)> = all_pairs()
                .into_iter()
                .filter(|p| !gone.contains(p))
                .collect();
            assert_eq!(pairs_of(write.range_rows(&table, &[], ..).unwrap()), left);
            assert_eq!(write.row_count(&table).unwrap(), left.len() as u64);
            deleted_some += usize::from(deleted > 0);
            // Aborted: the next range starts from the nine rows.
        }
        assert!(deleted_some > 20, "{deleted_some}");

        // Committed: a read sees the rows that remain and their count.
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let bounds = (Excluded(Value::I64(10)), Unbounded);
        assert_eq!(
            write
                .delete_range(&table, &[Value::I32(2)], bounds)
                .unwrap(),
            2
        );
        assert_eq!(write.delete_range(&table, &[], Value::I32(3)..).unwrap(), 3);
        // Nothing left in those ranges.
        assert_eq!(write.delete_range(&table, &[], Value::I32(3)..).unwrap(), 0);
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let expected = [(1, 10), (1, 20), (1, 30), (2, 10)];
        assert_eq!(
            pairs_of(read.range_rows(&table, &[], ..).unwrap()),
            expected
        );
        assert_eq!(read.row_count(&table).unwrap(), 4);
        // Reading a row that is not there says so, and deleting it returns false.
        let gone = [Value::I32(2), Value::I64(20)];
        assert!(read.get_row(&table, &gone).unwrap().is_none());
        drop(read);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        assert!(write.get_row(&table, &gone).unwrap().is_none());
        assert!(!write.delete_row(&table, &gone).unwrap());
        assert_eq!(write.delete_range(&table, &gone, ..).unwrap(), 0);
        assert_eq!(write.row_count(&table).unwrap(), 4);
    });
}

/// E16 (AC13): a range delete that finds no rows, and scans, are no change, and the write writes nothing; a range
/// delete with rows is.
#[test]
fn e16_a_range_delete_without_rows_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    pairs_store(&open(tmp.path()));
    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        assert_eq!(write.range_rows(&table, &[], ..).unwrap().count(), 9);
        assert_eq!(write.delete_range(&table, &[], Value::I32(4)..).unwrap(), 0);
        assert_eq!(
            write
                .delete_range(&table, &[Value::I32(2)], Value::I64(21)..Value::I64(29))
                .unwrap(),
            0
        );
        let inverted = (Included(Value::I32(3)), Included(Value::I32(1)));
        assert_eq!(write.delete_range(&table, &[], inverted).unwrap(), 0);
        assert!(write.delete_range(&table, &[Value::Null], ..).is_err());
        write.commit().unwrap();
    });
    assert!(!changed, "a range delete without rows writes nothing");
    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        assert_eq!(write.delete_range(&table, &[Value::I32(2)], ..).unwrap(), 3);
        write.commit().unwrap();
    });
    assert!(changed);
}

/// E11 and E12 (document 1, *Cost of mixed versions*): a scan over rows written in several schema versions, mixed in
/// key order, reads each with every live column of the current schema, with the default or null in the columns added
/// after it and without the dropped ones, forward and backward, in a write and after the commit in a read.
#[test]
fn e12_a_scan_reads_rows_of_every_schema_version() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("a", ColumnType::I32),
                Column::new("x", ColumnType::String),
            ],
            &["id"],
        );
        let table = create_table(&mut write, "t", &schema);
        // Version 1: ids 0, 3, 6.
        for id in [0, 3, 6] {
            let row = [
                ("id", Value::I64(id)),
                ("a", Value::I32(id as i32)),
                ("x", s("gone")),
            ];
            write.insert_row(&table, &row).unwrap();
        }
        // Version 2 adds `b` with a default: ids 1, 4, 7.
        let with_b = Column::new("b", ColumnType::I64)
            .not_null()
            .with_default(Value::I64(-7));
        write.add_column(&table, &with_b).unwrap();
        for id in [1, 4, 7] {
            let row = [
                ("id", Value::I64(id)),
                ("a", Value::I32(id as i32)),
                ("b", Value::I64(id * 10)),
            ];
            write.insert_row(&table, &row).unwrap();
        }
        // Version 3 drops `x`, version 4 adds `c`: ids 2, 5, 8.
        write.drop_column(&table, "x").unwrap();
        write
            .add_column(&table, &Column::new("c", ColumnType::Bool))
            .unwrap();
        for id in [2, 5, 8] {
            let row = [
                ("id", Value::I64(id)),
                ("b", Value::I64(id * 10)),
                ("c", Value::Bool(true)),
            ];
            write.insert_row(&table, &row).unwrap();
        }
        let expected: Vec<String> = (0..9)
            .map(|id: i64| {
                let (a, b, c) = match id % 3 {
                    0 => (format!("I32({id})"), "I64(-7)".to_owned(), "Null"),
                    1 => (format!("I32({id})"), format!("I64({})", id * 10), "Null"),
                    _ => ("Null".to_owned(), format!("I64({})", id * 10), "Bool(true)"),
                };
                format!("{{\"id\": I64({id}), \"a\": {a}, \"b\": {b}, \"c\": {c}}}")
            })
            .collect();
        let text = |row: TableRow<'_>| format!("{row:?}");
        assert_eq!(
            collect(write.range_rows(&table, &[], ..).unwrap(), text),
            expected
        );
        let backward = collect(write.range_rows(&table, &[], ..).unwrap().rev(), text);
        assert_eq!(backward, reversed(expected.clone()));
        let middle = (Excluded(Value::I64(1)), Included(Value::I64(7)));
        assert_eq!(
            collect(write.range_rows(&table, &[], middle).unwrap(), text),
            expected[2..8]
        );
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert_eq!(
            collect(read.range_rows(&table, &[], ..).unwrap(), text),
            expected
        );
        assert_eq!(
            alternate(read.range_rows(&table, &[], ..).unwrap(), text),
            expected
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC-N1 and AC17: the order of N5 for each orderable type

/// SQL Server's order of a `uniqueidentifier` given in RFC 9562 bytes, written as `SqlGuid` compares: the bytes of
/// `Guid.ToByteArray()` (the first three fields little-endian) in the order 10-15, 8-9, 6-7, 4-5, 0-3.
fn sql_server_cmp(a: &[u8; 16], b: &[u8; 16]) -> Ordering {
    let dotnet = |r: &[u8; 16]| -> [u8; 16] {
        let mut d = *r;
        d[..4].reverse();
        d[4..6].reverse();
        d[6..8].reverse();
        d
    };
    let (a, b) = (dotnet(a), dotnet(b));
    const ORDER: [usize; 16] = [10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3];
    ORDER
        .iter()
        .map(|&i| a[i].cmp(&b[i]))
        .find(|o| o.is_ne())
        .unwrap_or(Ordering::Equal)
}

/// N5 on two normalized values of a column of type `ty`, by their type and never by their encoding.
fn model_cmp(ty: &ColumnType, a: &Value<'_>, b: &Value<'_>) -> Ordering {
    fn float(a: f64, b: f64) -> Ordering {
        match (a.is_nan(), b.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => a.partial_cmp(&b).unwrap(),
        }
    }
    match (a, b) {
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::I8(a), Value::I8(b)) => a.cmp(b),
        (Value::I16(a), Value::I16(b)) => a.cmp(b),
        (Value::I32(a), Value::I32(b)) => a.cmp(b),
        (Value::I64(a), Value::I64(b)) => a.cmp(b),
        (Value::U8(a), Value::U8(b)) => a.cmp(b),
        (Value::U16(a), Value::U16(b)) => a.cmp(b),
        (Value::U32(a), Value::U32(b)) => a.cmp(b),
        (Value::U64(a), Value::U64(b)) => a.cmp(b),
        (Value::F32(a), Value::F32(b)) => float(f64::from(*a), f64::from(*b)),
        (Value::F64(a), Value::F64(b)) => float(*a, *b),
        (Value::Decimal(a), Value::Decimal(b)) => a.mantissa.cmp(&b.mantissa),
        (Value::String(a), Value::String(b)) => a.as_bytes().cmp(b.as_bytes()),
        (Value::Bytes(a), Value::Bytes(b)) => a.cmp(b),
        (Value::Date(a), Value::Date(b)) => a.cmp(b),
        (Value::Time(a), Value::Time(b)) => a.cmp(b),
        (Value::Timestamp(a), Value::Timestamp(b)) => a.cmp(b),
        (Value::Duration(a), Value::Duration(b)) => a.cmp(b),
        (Value::Ulid(a), Value::Ulid(b)) => a.0.cmp(&b.0),
        (Value::Uuid(a), Value::Uuid(b)) => match ty {
            ColumnType::Uuid(UuidOrder::SqlServer) => sql_server_cmp(&a.0, &b.0),
            _ => a.0.cmp(&b.0),
        },
        _ => panic!("values of different types"),
    }
}

fn model_cmp_key(types: &[ColumnType], a: &[Value<'_>], b: &[Value<'_>]) -> Ordering {
    types
        .iter()
        .zip(a.iter().zip(b))
        .map(|(ty, (a, b))| model_cmp(ty, a, b))
        .find(|o| o.is_ne())
        .unwrap_or(Ordering::Equal)
}

const ORDERABLE: [ColumnType; 21] = [
    ColumnType::Bool,
    ColumnType::I8,
    ColumnType::I16,
    ColumnType::I32,
    ColumnType::I64,
    ColumnType::U8,
    ColumnType::U16,
    ColumnType::U32,
    ColumnType::U64,
    ColumnType::F32,
    ColumnType::F64,
    ColumnType::Decimal {
        precision: 38,
        scale: 3,
    },
    ColumnType::String,
    ColumnType::Bytes,
    ColumnType::Date,
    ColumnType::Time,
    ColumnType::Timestamp,
    ColumnType::Duration,
    ColumnType::Uuid(UuidOrder::Rfc9562),
    ColumnType::Uuid(UuidOrder::SqlServer),
    ColumnType::Ulid,
];

/// SplitMix64: a small, deterministic generator.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn pick<T: Clone>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize].clone()
    }

    fn chance(&mut self, one_in: u64) -> bool {
        self.below(one_in) == 0
    }
}

const NANOS: i128 = 1000;

/// The extreme and special values of `ty` before normalization: the ends of each domain, negative integers, `-0` and
/// `+0`, NaNs with different bits and infinities, empty `bytes` and `string`, `00` and U+0000 at the start, in the
/// middle and at the end, and `uuid`s and `ulid`s whose groups SQL Server compares separately.
fn special_values(ty: &ColumnType) -> Vec<Value<'static>> {
    let owned_s = |t: &str| Value::String(Cow::Owned(t.to_owned()));
    let owned_b = |t: &[u8]| Value::Bytes(Cow::Owned(t.to_vec()));
    match ty {
        ColumnType::Bool => vec![Value::Bool(true), Value::Bool(false)],
        ColumnType::I8 => [i8::MIN, -2, -1, 0, 1, i8::MAX].map(Value::I8).to_vec(),
        ColumnType::I16 => [i16::MIN, -256, -255, -1, 0, 255, 256, i16::MAX]
            .map(Value::I16)
            .to_vec(),
        ColumnType::I32 => [i32::MIN, -65_536, -1, 0, 1, 65_536, i32::MAX]
            .map(Value::I32)
            .to_vec(),
        ColumnType::I64 => [
            i64::MIN,
            i64::MIN + 1,
            -1 << 40,
            -1,
            0,
            1,
            1 << 40,
            i64::MAX,
        ]
        .map(Value::I64)
        .to_vec(),
        ColumnType::U8 => [0, 1, 255].map(Value::U8).to_vec(),
        ColumnType::U16 => [0, 255, 256, u16::MAX].map(Value::U16).to_vec(),
        ColumnType::U32 => [0, 1, u32::MAX].map(Value::U32).to_vec(),
        ColumnType::U64 => [0, 1, i64::MAX as u64, i64::MAX as u64 + 1, u64::MAX]
            .map(Value::U64)
            .to_vec(),
        ColumnType::F32 => [
            0x8000_0000, // -0
            0x0000_0000, // +0
            0x7FC0_0000, // NaN
            0xFFC0_0001, // NaNs with other bits
            0x7F80_0001,
            0x7F80_0000, // +inf
            0xFF80_0000, // -inf
            0x0000_0001, // subnormals
            0x8000_0001,
            0x3F80_0000, // 1
            0xBF80_0000, // -1
            0x7F7F_FFFF, // MAX
            0xFF7F_FFFF, // MIN
        ]
        .map(|bits| Value::F32(f32::from_bits(bits)))
        .to_vec(),
        ColumnType::F64 => [
            0x8000_0000_0000_0000,
            0,
            0x7FF8_0000_0000_0000,
            0xFFF8_0000_0000_0001,
            0x7FF0_0000_0000_0001,
            0x7FF0_0000_0000_0000,
            0xFFF0_0000_0000_0000,
            1,
            0x8000_0000_0000_0001,
            0x3FF0_0000_0000_0000,
            0xBFF0_0000_0000_0000,
        ]
        .map(|bits| Value::F64(f64::from_bits(bits)))
        .to_vec(),
        ColumnType::Decimal { scale, .. } => {
            let max = 10i128.pow(38) - 1;
            [-max, -1001, -1, 0, 1, 999, max]
                .map(|mantissa| {
                    Value::Decimal(Decimal {
                        mantissa,
                        scale: *scale,
                    })
                })
                .to_vec()
        }
        ColumnType::String => [
            "",
            "\0",
            "\0\0",
            "\0a",
            "a",
            "a\0",
            "a\0b",
            "ab",
            "b",
            "é",
            "\u{10FFFF}",
        ]
        .map(owned_s)
        .to_vec(),
        ColumnType::Bytes => [
            &[][..],
            &[0],
            &[0, 0],
            &[0, 1],
            &[0, 0xFF],
            &[1],
            &[1, 0],
            &[1, 0, 1],
            &[0xFF],
            &[0xFF, 0],
        ]
        .map(owned_b)
        .to_vec(),
        ColumnType::Date => [DATE_MIN, -1, 0, 1, DATE_MAX].map(Value::Date).to_vec(),
        ColumnType::Time => [0, 1_000, 1_499, 60_000_000_000, TIME_MAX * 1000]
            .map(Value::Time)
            .to_vec(),
        ColumnType::Timestamp => [
            i128::from(TIMESTAMP_MIN) * NANOS,
            -NANOS,
            -500,
            0,
            NANOS,
            i128::from(TIMESTAMP_MAX) * NANOS,
        ]
        .map(Value::Timestamp)
        .to_vec(),
        ColumnType::Duration => {
            let max = i128::from(DURATION_MAX) * NANOS;
            [-max, -NANOS, 0, NANOS, max].map(Value::Duration).to_vec()
        }
        ColumnType::Uuid(_) | ColumnType::Ulid => {
            let mut values = Vec::new();
            for group in [0..4, 4..6, 6..8, 8..10, 10..16] {
                for fill in [0x01, 0xFF] {
                    let mut bytes = [0u8; 16];
                    bytes[group.clone()].fill(fill);
                    values.push(bytes);
                }
            }
            values.push([0; 16]);
            values.push([0xFF; 16]);
            values
                .into_iter()
                .map(|bytes| match ty {
                    ColumnType::Ulid => Value::Ulid(Ulid(bytes)),
                    _ => Value::Uuid(Uuid(bytes)),
                })
                .collect()
        }
        ColumnType::Vector(_) => unreachable!("vectors are not orderable"),
    }
}

/// A random value of `ty` before normalization, often one of its [`special_values`].
fn random_value(rng: &mut Rng, ty: &ColumnType) -> Value<'static> {
    if rng.chance(2) {
        return rng.pick(&special_values(ty));
    }
    let (r1, r2) = (rng.next(), rng.next());
    match ty {
        ColumnType::Bool => Value::Bool(r1 & 1 == 0),
        ColumnType::I8 => Value::I8(r1 as i8),
        ColumnType::I16 => Value::I16(r1 as i16),
        ColumnType::I32 => Value::I32(r1 as i32),
        ColumnType::I64 => Value::I64(r1 as i64 >> rng.below(64)),
        ColumnType::U8 => Value::U8(r1 as u8),
        ColumnType::U16 => Value::U16(r1 as u16),
        ColumnType::U32 => Value::U32(r1 as u32),
        ColumnType::U64 => Value::U64(r1 >> rng.below(64)),
        ColumnType::F32 => Value::F32(f32::from_bits(r1 as u32)),
        ColumnType::F64 => Value::F64(f64::from_bits(r1)),
        ColumnType::Decimal { scale, .. } => Value::Decimal(Decimal {
            mantissa: i128::from(r1 as i64) * i128::from(r2 >> 1) / 7,
            scale: *scale,
        }),
        ColumnType::String => {
            let parts = ["", "\0", "a", "b", "é", "\u{FFFF}", "\u{10FFFF}", "\0\0"];
            let n = rng.below(4);
            Value::String(Cow::Owned((0..n).map(|_| rng.pick(&parts)).collect()))
        }
        ColumnType::Bytes => {
            let n = rng.below(5);
            Value::Bytes(Cow::Owned(
                (0..n)
                    .map(|_| rng.pick(&[0x00, 0x01, 0xFE, 0xFF, 0x41]))
                    .collect(),
            ))
        }
        ColumnType::Date => Value::Date(DATE_MIN + (r1 % (DATE_MAX - DATE_MIN) as u64) as i32),
        ColumnType::Time => Value::Time((r1 % (TIME_MAX * 1000) as u64) as i64),
        ColumnType::Timestamp => {
            let (min, max) = (
                i128::from(TIMESTAMP_MIN) * NANOS,
                i128::from(TIMESTAMP_MAX) * NANOS,
            );
            Value::Timestamp(min + (u128::from(r1) * u128::from(r2) % (max - min) as u128) as i128)
        }
        ColumnType::Duration => Value::Duration(i128::from(r1 as i64)),
        ColumnType::Uuid(_) | ColumnType::Ulid => {
            // Each group of bytes that SQL Server compares as a unit is all 00, all FF, all 01 or random, so that
            // many values tie on the groups compared first and the later groups decide.
            let mut bytes = [0u8; 16];
            for group in [0..4, 4..6, 6..8, 8..10, 10..16] {
                let fill = rng.below(4);
                for byte in &mut bytes[group] {
                    *byte = match fill {
                        0 => 0x00,
                        1 => 0xFF,
                        2 => 0x01,
                        _ => rng.next() as u8,
                    };
                }
            }
            if *ty == ColumnType::Ulid {
                Value::Ulid(Ulid(bytes))
            } else {
                Value::Uuid(Uuid(bytes))
            }
        }
        ColumnType::Vector(_) => unreachable!("vectors are not orderable"),
    }
}

fn normalized(ty: &ColumnType, value: Value<'static>) -> Value<'static> {
    normalize(ty, false, value).unwrap()
}

/// Checks that a value read back is stored as N4 says: `-0` as `+0` and every NaN as the one NaN.
#[track_caller]
fn check_float(value: &Value<'_>) {
    match *value {
        Value::F32(v) if v.is_nan() => assert_eq!(v.to_bits(), 0x7FC0_0000),
        Value::F32(v) if v == 0.0 => assert_eq!(v.to_bits(), 0),
        Value::F64(v) if v.is_nan() => assert_eq!(v.to_bits(), 0x7FF8_0000_0000_0000),
        Value::F64(v) if v == 0.0 => assert_eq!(v.to_bits(), 0),
        _ => {}
    }
}

/// The values of the key columns `names` of `row`, owned, after checking each float.
fn key_values(row: &TableRow<'_>, names: &[String]) -> Vec<Value<'static>> {
    names
        .iter()
        .map(|name| {
            let value = row.get(name).expect("a key column").clone().into_owned();
            check_float(&value);
            value
        })
        .collect()
}

/// The SQL Server order vectors, generated once with `SqlGuid` (AC17), in their order.
fn sql_server_vectors() -> Vec<Uuid> {
    include_str!("data/sqlserver-order/sqlguid-order.txt")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let hex = line.trim();
            Uuid(std::array::from_fn(|i| {
                u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()
            }))
        })
        .collect()
}

/// Inserts `values` as the key column `k` of `table`, alone or with `n` (`with_n`) after it in the key, and returns
/// the keys the model expects in the table, in its order: duplicates by N5 (`-0` and `+0`, two NaNs) fail with a
/// duplicate key and leave the first.
fn insert_keys(
    write: &mut WriteTransaction<'_>,
    table: &WriteTable<'_>,
    ty: &ColumnType,
    values: &[Value<'static>],
    with_n: bool,
) -> Vec<Vec<Value<'static>>> {
    let types = if with_n {
        vec![*ty, ColumnType::I32]
    } else {
        vec![*ty]
    };
    let mut expected: Vec<Vec<Value<'static>>> = Vec::new();
    for value in values {
        for n in if with_n { &[1, -1][..] } else { &[0][..] } {
            let mut key = vec![normalized(ty, value.clone())];
            let mut row = vec![("k", value.clone())];
            if with_n {
                key.push(Value::I32(*n));
                row.push(("n", Value::I32(*n)));
            }
            let duplicate = expected
                .iter()
                .any(|e| model_cmp_key(&types, e, &key).is_eq());
            match write.insert_row(table, &row) {
                Ok(()) => {
                    assert!(!duplicate, "{key:?} of {ty:?}");
                    expected.push(key);
                }
                Err(error) => {
                    assert!(duplicate, "{key:?} of {ty:?}: {error}");
                    assert_eq!(error.kind(), ErrorKind::DuplicateKey);
                }
            }
        }
    }
    expected.sort_by(|a, b| model_cmp_key(&types, a, b));
    expected
}

/// AC-N1 (N5, N4): a table per orderable type, with that type as a one-column primary key and before an `i32` in a
/// two-column one, returns its rows forward and backward in the order of N5, with negative integers, `-0` and NaN,
/// `ulid`, `uuid` in both orders, empty `bytes` and `string` and `00` and U+0000 in them; duplicates by N5 fail;
/// each key reads back with `-0` as `+0` and every NaN as the one NaN.
#[test]
fn acn1_each_orderable_type_scans_in_the_order_of_n5() {
    stores(|store, _| {
        let mut rng = Rng(0x0005_CA45);
        let mut write = store.write().unwrap();
        let mut expected_tables = Vec::new();
        for (t, ty) in ORDERABLE.iter().enumerate() {
            let mut values = special_values(ty);
            values.extend((0..30).map(|_| random_value(&mut rng, ty)));
            // Shuffled, so that the order of the scan is not the order of the inserts.
            for i in (1..values.len()).rev() {
                values.swap(i, rng.below(i as u64 + 1) as usize);
            }
            for with_n in [false, true] {
                let mut columns = vec![Column::new("k", *ty).not_null()];
                let mut key = vec!["k"];
                if with_n {
                    columns.push(Column::new("n", ColumnType::I32).not_null());
                    key.push("n");
                }
                columns.push(Column::new("v", ColumnType::Bool));
                let name = format!("t{t}-{with_n}");
                let table = create_table(&mut write, &name, &Schema::new(columns, &key));
                let expected = insert_keys(&mut write, &table, ty, &values, with_n);
                let names: Vec<String> = key.iter().map(|k| (*k).to_owned()).collect();
                let forward = collect(write.range_rows(&table, &[], ..).unwrap(), |row| {
                    key_values(&row, &names)
                });
                assert!(
                    same_keys(&forward, &expected),
                    "{ty:?}: {forward:?}\n{expected:?}"
                );
                let backward = collect(write.range_rows(&table, &[], ..).unwrap().rev(), |row| {
                    key_values(&row, &names)
                });
                assert!(same_keys(&backward, &reversed(expected.clone())), "{ty:?}");
                assert_eq!(write.row_count(&table).unwrap(), expected.len() as u64);
                expected_tables.push((name, names, expected));
            }
        }
        write.commit().unwrap();
        let read = store.read().unwrap();
        for (name, names, expected) in expected_tables {
            let table = read.open_table(&name).unwrap();
            let forward = collect(read.range_rows(&table, &[], ..).unwrap(), |row| {
                key_values(&row, &names)
            });
            assert!(same_keys(&forward, &expected), "{name}");
        }
    });
}

fn same_keys(a: &[Vec<Value<'_>>], b: &[Vec<Value<'_>>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
}

/// AC17 and AC-N1: rows with a `uuid` key in SQL Server's order come out in the order that `SqlGuid` gives the
/// vectors kept in the repository, forward and backward, also before another key column; in RFC 9562's order, in
/// the order of their 16 bytes, which differs on these vectors.
#[test]
fn ac17_sql_server_uuid_rows_scan_in_the_order_of_sqlguid() {
    stores(|store, _| {
        let vectors = sql_server_vectors();
        assert!(vectors.len() >= 1000, "{} vectors", vectors.len());
        let mut shuffled = vectors.clone();
        Lcg(17).shuffle(&mut shuffled);
        let mut write = store.write().unwrap();
        let tables = [
            (UuidOrder::SqlServer, false),
            (UuidOrder::SqlServer, true),
            (UuidOrder::Rfc9562, false),
        ];
        for (i, (order, with_n)) in tables.into_iter().enumerate() {
            let mut columns = vec![Column::new("g", ColumnType::Uuid(order)).not_null()];
            let mut key = vec!["g"];
            if with_n {
                columns.push(Column::new("n", ColumnType::I32).not_null());
                key.push("n");
            }
            let table = create_table(&mut write, &format!("u{i}"), &Schema::new(columns, &key));
            for uuid in &shuffled {
                for n in [1, 0] {
                    let mut row = vec![("g", Value::Uuid(*uuid))];
                    if with_n {
                        row.push(("n", Value::I32(n)));
                    } else if n == 0 {
                        continue;
                    }
                    write.insert_row(&table, &row).unwrap();
                }
            }
            let uuid_of = |row: TableRow<'_>| match row.get("g") {
                Some(Value::Uuid(u)) => *u,
                other => panic!("{other:?}"),
            };
            let mut expected: Vec<Uuid> = match order {
                UuidOrder::SqlServer => vectors.clone(),
                UuidOrder::Rfc9562 => {
                    let mut sorted = vectors.clone();
                    sorted.sort_by_key(|u| u.0);
                    assert_ne!(sorted, vectors, "the two orders differ on the vectors");
                    sorted
                }
            };
            if with_n {
                expected = expected.into_iter().flat_map(|u| [u, u]).collect();
            }
            let forward = collect(write.range_rows(&table, &[], ..).unwrap(), uuid_of);
            assert!(forward == expected, "{order:?} {with_n}");
            let backward = collect(write.range_rows(&table, &[], ..).unwrap().rev(), uuid_of);
            assert!(backward == reversed(expected), "{order:?} {with_n}");
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC-N1: property test against a reference model of N5

/// Pads the last `string` or `bytes` column of `values` so that the key has `target` bytes, when it has one.
fn pad_to(types: &[ColumnType], values: &mut [Value<'static>], target: usize) {
    let Some(i) = types
        .iter()
        .rposition(|t| matches!(t, ColumnType::String | ColumnType::Bytes))
    else {
        return;
    };
    let len = key_len(types, values);
    if len >= target {
        return;
    }
    let extra = target - len;
    match &mut values[i] {
        Value::String(text) => text.to_mut().push_str(&"a".repeat(extra)),
        Value::Bytes(bytes) => bytes.to_mut().extend(std::iter::repeat_n(0x41, extra)),
        _ => unreachable!(),
    }
}

/// Whether the model puts the key `row` inside the range of `prefix` and the bounds on the next column.
fn model_in_range(
    types: &[ColumnType],
    row: &[Value<'_>],
    prefix: &[Value<'_>],
    bounds: &Bounds<'_>,
) -> bool {
    let k = prefix.len();
    if model_cmp_key(&types[..k], &row[..k], prefix).is_ne() {
        return false;
    }
    if k == types.len() {
        return true;
    }
    let cmp = |bound: &Value<'_>| model_cmp(&types[k], &row[k], bound);
    after(&bounds.0, cmp) && before(&bounds.1, cmp)
}

/// A table of the property test: its key types and column names, and the model's rows (key values and `v`).
struct ModelTable {
    name: String,
    types: Vec<ColumnType>,
    names: Vec<String>,
    rows: Vec<(Vec<Value<'static>>, i64)>,
}

impl ModelTable {
    /// The model's rows in the order of N5.
    fn sorted(&self) -> Vec<(Vec<Value<'static>>, i64)> {
        let mut rows = self.rows.clone();
        rows.sort_by(|a, b| model_cmp_key(&self.types, &a.0, &b.0));
        rows
    }

    /// A row of a scan as the model holds it, after checking its floats.
    fn row_of(&self, row: &TableRow<'_>) -> (Vec<Value<'static>>, i64) {
        let v = match row.get("v") {
            Some(&Value::I64(v)) => v,
            other => panic!("{other:?}"),
        };
        (key_values(row, &self.names), v)
    }

    fn check_rows(
        &self,
        got: &[(Vec<Value<'static>>, i64)],
        expected: &[(Vec<Value<'static>>, i64)],
    ) {
        assert_eq!(got.len(), expected.len(), "{:?}", self.types);
        for (g, e) in got.iter().zip(expected) {
            assert!(
                same(&g.0, &e.0) && g.1 == e.1,
                "{g:?} {e:?} of {:?}",
                self.types
            );
        }
    }
}

/// AC-N1 (N5, E12, AC-N3): random tables with primary keys of one to four columns of every orderable type, with
/// extreme values and keys at the size limit: the duplicate keys, the order of full scans forward and backward, the
/// rows of random ranges and of random range deletes are those of a model of N5 that compares values by their type;
/// every row reads back with its values, `-0` as `+0` and every NaN as the one NaN.
#[test]
fn acn1_random_tables_follow_the_model_of_n5() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut rng = Rng(0x007A_B1E5_0007);
    let mut write = store.write().unwrap();
    let mut tables: Vec<ModelTable> = Vec::new();
    let (mut ranges, mut duplicates, mut too_long, mut at_limit, mut deleted) = (0, 0, 0, 0, 0);
    for t in 0..120 {
        let columns = 1 + rng.below(4) as usize;
        let types: Vec<ColumnType> = (0..columns).map(|_| rng.pick(&ORDERABLE)).collect();
        let names: Vec<String> = (0..columns).map(|i| format!("k{i}")).collect();
        let mut schema_columns: Vec<Column> = types
            .iter()
            .zip(&names)
            .map(|(ty, name)| Column::new(name, *ty).not_null())
            .collect();
        schema_columns.push(Column::new("v", ColumnType::I64).not_null());
        let key: Vec<&str> = names.iter().map(String::as_str).collect();
        let name = format!("p{t}");
        let table = create_table(&mut write, &name, &Schema::new(schema_columns, &key));
        let mut model = ModelTable {
            name,
            types: types.clone(),
            names,
            rows: Vec::new(),
        };
        for v in 0..40 {
            let raw: Vec<Value<'static>> =
                types.iter().map(|ty| random_value(&mut rng, ty)).collect();
            let mut keyv: Vec<Value<'static>> = types
                .iter()
                .zip(&raw)
                .map(|(ty, value)| normalized(ty, value.clone()))
                .collect();
            // Raw values, unless padded: padding works on the normalized ones.
            let mut given = raw;
            if rng.chance(4) {
                pad_to(
                    &types,
                    &mut keyv,
                    rng.pick(&[MAX_KEY - 1, MAX_KEY, MAX_KEY + 1]),
                );
                given = keyv.clone();
            }
            let mut row: Vec<(&str, Value<'_>)> = model
                .names
                .iter()
                .map(String::as_str)
                .zip(given.iter().map(Value::reborrow))
                .collect();
            row.push(("v", Value::I64(v)));
            let len = key_len(&types, &keyv);
            let duplicate = model
                .rows
                .iter()
                .any(|(k, _)| model_cmp_key(&types, k, &keyv).is_eq());
            match write.insert_row(&table, &row) {
                Ok(()) => {
                    assert!(len <= MAX_KEY && !duplicate, "{keyv:?} of {types:?}");
                    at_limit += usize::from(len == MAX_KEY);
                    model.rows.push((keyv, v));
                }
                Err(error) if len > MAX_KEY => {
                    assert_eq!(
                        invalid_size(error, None),
                        (SizeSubject::PrimaryKey, len as u64, 1, MAX_KEY as u64)
                    );
                    too_long += 1;
                }
                Err(error) => {
                    assert!(duplicate, "{keyv:?} of {types:?}: {error}");
                    assert_eq!(error.kind(), ErrorKind::DuplicateKey);
                    duplicates += 1;
                }
            }
        }
        // Full scans, forward and backward, and each row read by its key.
        let sorted = model.sorted();
        let forward = collect(write.range_rows(&table, &[], ..).unwrap(), |row| {
            model.row_of(&row)
        });
        model.check_rows(&forward, &sorted);
        let backward = collect(write.range_rows(&table, &[], ..).unwrap().rev(), |row| {
            model.row_of(&row)
        });
        model.check_rows(&backward, &reversed(sorted.clone()));
        for (key, v) in &model.rows {
            let row = write
                .get_row(&table, key)
                .unwrap()
                .expect("a row of the model");
            model.check_rows(&[model.row_of(&row)], &[(key.clone(), *v)]);
        }
        // Random ranges: a prefix of k columns taken from a row (or random), bounds on column k included, excluded or
        // absent, from rows or random values; one range in five is deleted.
        for _ in 0..20 {
            let k = rng.below(columns as u64 + 1) as usize;
            let source: Vec<Value<'static>> = if model.rows.is_empty() || rng.chance(5) {
                types
                    .iter()
                    .map(|ty| normalized(ty, random_value(&mut rng, ty)))
                    .collect()
            } else {
                rng.pick(&model.rows).0
            };
            let prefix = &source[..k];
            let bound = |rng: &mut Rng| -> Bound<Value<'static>> {
                if k == columns {
                    return Unbounded;
                }
                let value = if !model.rows.is_empty() && rng.chance(2) {
                    rng.pick(&model.rows).0[k].clone()
                } else {
                    normalized(&types[k], random_value(rng, &types[k]))
                };
                match rng.below(3) {
                    0 => Included(value),
                    1 => Excluded(value),
                    _ => Unbounded,
                }
            };
            let bounds = (bound(&mut rng), bound(&mut rng));
            let prefix_len = key_len(&types[..k], prefix);
            let bound_len = |b: &Bound<Value<'_>>| match b {
                Included(v) | Excluded(v) => {
                    prefix_len + key_len(&types[k..=k], std::slice::from_ref(v))
                }
                Unbounded => prefix_len,
            };
            let longest = prefix_len
                .max(bound_len(&bounds.0))
                .max(bound_len(&bounds.1));
            let delete = rng.chance(5);
            if longest > MAX_KEY {
                let error = write
                    .range_rows(&table, prefix, bounds.clone())
                    .err()
                    .unwrap();
                let (what, received, _, _) = invalid_size(error, None);
                assert!(what == SizeSubject::PrimaryKey && received > MAX_KEY as u64);
                continue;
            }
            let expected: Vec<_> = sorted_in(&model, prefix, &bounds);
            if delete {
                let removed = write.delete_range(&table, prefix, bounds.clone()).unwrap();
                assert_eq!(removed, expected.len() as u64);
                model
                    .rows
                    .retain(|(key, _)| !model_in_range(&types, key, prefix, &bounds));
                assert_eq!(write.row_count(&table).unwrap(), model.rows.len() as u64);
                deleted += removed;
            } else {
                let got = collect(
                    write.range_rows(&table, prefix, bounds.clone()).unwrap(),
                    |row| model.row_of(&row),
                );
                model.check_rows(&got, &expected);
                let got = collect(
                    write
                        .range_rows(&table, prefix, bounds.clone())
                        .unwrap()
                        .rev(),
                    |row| model.row_of(&row),
                );
                model.check_rows(&got, &reversed(expected));
            }
            ranges += 1;
        }
        tables.push(model);
    }
    write.commit().unwrap();
    // After the commit, a read sees every table as the model has it.
    let read = store.read().unwrap();
    for model in &tables {
        let table: ReadTable<'_> = read.open_table(&model.name).unwrap();
        let rows = collect(read.range_rows(&table, &[], ..).unwrap(), |row| {
            model.row_of(&row)
        });
        model.check_rows(&rows, &model.sorted());
        assert_eq!(read.row_count(&table).unwrap(), model.rows.len() as u64);
    }
    check_read_scan_is_sendable(&read);
    assert!(ranges > 1500, "{ranges} ranges");
    assert!(
        duplicates > 50 && too_long > 20 && at_limit > 20,
        "{duplicates} {too_long} {at_limit}"
    );
    assert!(deleted > 100, "{deleted} rows deleted");
}

/// The rows of the model in the range of `prefix` and `bounds`, in the order of N5.
fn sorted_in(
    model: &ModelTable,
    prefix: &[Value<'_>],
    bounds: &Bounds<'_>,
) -> Vec<(Vec<Value<'static>>, i64)> {
    model
        .sorted()
        .into_iter()
        .filter(|(key, _)| model_in_range(&model.types, key, prefix, bounds))
        .collect()
}

/// A scan of a read can be sent to another thread, like its transaction.
fn check_read_scan_is_sendable(read: &ReadTransaction<'_>) {
    fn sendable<T: Send>(_: &T) {}
    let table = read.open_table("p0").unwrap();
    let scan = read.range_rows(&table, &[], ..).unwrap();
    sendable(&scan);
}
