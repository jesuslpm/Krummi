//! Rows of a table through the public API (tables with schema PRD 7.1, T6): insert, update, insert or update, read
//! and delete by the primary key (AC5, E10, E11), values at the limits of their types and domains stored and read
//! through a table (AC6, AC7, AC-N2), the precedence and the data of the errors (AC14), the sizes of a primary key, a
//! row and a column name (AC15), what changes and what does not (AC13, E16), the row count (AC-N3), and the row
//! parts of the catalog criteria (AC1, AC2, AC3, AC-N6). The page-counting hooks of AC4, AC9 and AC-N3 are internal
//! tests of the crate (`src/table/row_tests.rs`); the references that the writes end are compile-time tests in the
//! documentation of the rows.
//!
//! Each criterion runs on a store on disk and on a temporary one, except the parts that reopen or damage the store's
//! files, which run on disk only.

mod support;

use std::borrow::Cow;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use krummi::table::value::{
    ColumnType, DATE_MAX, DATE_MIN, DURATION_MAX, Decimal, TIME_MAX, TIMESTAMP_MAX, TIMESTAMP_MIN,
    Value, VectorElement, VectorType,
};
use krummi::{
    Column, Error, ErrorKind, ReadTransaction, Schema, SchemaDifference, SchemaRule, SizeSubject,
    TableMode, TableRow, Upserted, ValueRule, WriteTable, WriteTransaction,
};
use krummi_tree::catalog::MAX_EXTRA;
use support::*;

// ---------------------------------------------------------------------------------------------------------------
// Helpers

/// `id i64 not null, name string, n i32 not null default 7, score f64`, primary key `id`.
fn people() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String),
            Column::new("n", ColumnType::I32)
                .not_null()
                .with_default(Value::I32(7)),
            Column::new("score", ColumnType::F64),
        ],
        &["id"],
    )
}

/// `id i64 not null, b bytes`, primary key `id`.
fn blobs() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("b", ColumnType::Bytes),
        ],
        &["id"],
    )
}

/// `k string not null, v i32`, primary key `k`.
fn texts() -> Schema {
    Schema::new(
        vec![
            Column::new("k", ColumnType::String).not_null(),
            Column::new("v", ColumnType::I32),
        ],
        &["k"],
    )
}

fn create<'s>(write: &mut WriteTransaction<'s>, name: &str, schema: &Schema) -> WriteTable<'s> {
    write
        .open_table_with(name, TableMode::CreateNew(schema))
        .unwrap()
}

fn s(text: &str) -> Value<'_> {
    Value::String(Cow::Borrowed(text))
}

fn b(bytes: &[u8]) -> Value<'_> {
    Value::Bytes(Cow::Borrowed(bytes))
}

fn vector(element: VectorElement, dimension: u16, cosine: bool) -> ColumnType {
    ColumnType::Vector(VectorType {
        element,
        dimension,
        cosine,
    })
}

/// A person row: `id`, `name`, `n` and `score`.
fn person(id: i64, name: &str, n: i32, score: f64) -> Vec<(&str, Value<'_>)> {
    vec![
        ("id", Value::I64(id)),
        ("name", s(name)),
        ("n", Value::I32(n)),
        ("score", Value::F64(score)),
    ]
}

/// A row as owned pairs of column name and value, in the order the row gives them.
fn owned(row: &TableRow<'_>) -> Vec<(String, Value<'static>)> {
    (0..row.len())
        .map(|i| {
            (
                row.name(i).unwrap().to_owned(),
                row.value(i).unwrap().clone().into_owned(),
            )
        })
        .collect()
}

/// The row of `key` that the write sees, owned, or `None`.
fn read_w(
    write: &WriteTransaction<'_>,
    table: &WriteTable<'_>,
    key: &[Value<'_>],
) -> Option<Vec<(String, Value<'static>)>> {
    write.get_row(table, key).unwrap().map(|row| owned(&row))
}

/// The row of `key` in the table `name` that the read sees, owned, or `None`.
fn read_r(
    read: &ReadTransaction<'_>,
    name: &str,
    key: &[Value<'_>],
) -> Option<Vec<(String, Value<'static>)>> {
    let table = read.open_table(name).unwrap();
    read.get_row(&table, key).unwrap().map(|row| owned(&row))
}

/// Owned pairs from borrowed ones.
fn pairs(row: &[(&str, Value<'_>)]) -> Vec<(String, Value<'static>)> {
    row.iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone().into_owned()))
        .collect()
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

/// The table and key text of a duplicate-key error, after checking its folder.
#[track_caller]
fn duplicate(error: Error, folder: Option<&Path>) -> (String, String) {
    match error {
        Error::DuplicateKey {
            folder: f,
            table,
            key,
            index,
            index_key,
        } => {
            assert_eq!(f, folder_of(folder));
            assert_eq!((index, index_key), (None, None));
            (table, key)
        }
        other => panic!("expected a duplicate key, got {other}"),
    }
}

/// The table and column of a different-schema error for a row's column, after checking its folder and difference.
#[track_caller]
fn unknown_column(error: Error, folder: Option<&Path>) -> (String, Option<String>) {
    match error {
        Error::DifferentSchema {
            folder: f,
            table,
            column,
            difference,
            ..
        } => {
            assert_eq!(f, folder_of(folder));
            assert_eq!(difference, SchemaDifference::MissingColumn);
            (table, column)
        }
        other => panic!("expected a different schema, got {other}"),
    }
}

fn some(name: &str) -> Option<String> {
    Some(name.to_owned())
}

fn named(row: Vec<(&str, Value<'static>)>) -> Vec<(String, Value<'static>)> {
    row.into_iter().map(|(n, v)| (n.to_owned(), v)).collect()
}

/// Taken by each test that builds a value of 256 MiB, so that those tests run one at a time and the test threads do
/// not hold several such buffers, and the stores' copies of them, at once. A test that failed while holding it does
/// not fail the others.
fn one_large_test_at_a_time() -> MutexGuard<'static, ()> {
    static LARGE: Mutex<()> = Mutex::new(());
    LARGE.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---------------------------------------------------------------------------------------------------------------
// AC5 and E10: storing rows

/// AC5 (E10, E11): a row without the columns that have a default reads their default, and null in the others; an
/// explicit null stores null, never the default; a column without nulls left without a value or given null fails
/// with an invalid value. A read row has every live column in schema order, by name and by position.
#[test]
fn ac5_insert_takes_defaults_and_nulls() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &people());
        write
            .add_column(
                &table,
                &Column::new("d", ColumnType::I32).with_default(Value::I32(5)),
            )
            .unwrap();
        write.insert_row(&table, &[("id", Value::I64(1))]).unwrap();
        write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(2)),
                    ("d", Value::Null),
                    ("name", Value::Null),
                ],
            )
            .unwrap();
        assert_eq!(
            read_w(&write, &table, &[Value::I64(1)]).unwrap(),
            named(vec![
                ("id", Value::I64(1)),
                ("name", Value::Null),
                ("n", Value::I32(7)),
                ("score", Value::Null),
                ("d", Value::I32(5)),
            ])
        );
        let row = write.get_row(&table, &[Value::I64(2)]).unwrap().unwrap();
        assert_eq!(row.get("d"), Some(Value::Null), "an explicit null");
        assert_eq!(row.get("n"), Some(Value::I32(7)));
        assert_eq!(row.value(4), Some(Value::Null), "by position");
        assert_eq!(
            (row.len(), row.get("missing"), row.value(5)),
            (5, None, None)
        );
        drop(row);
        let error = write
            .insert_row(&table, &[("id", Value::I64(3)), ("n", Value::Null)])
            .unwrap_err();
        assert_eq!(
            invalid_value(error, folder),
            (some("t"), some("n"), ValueRule::NullNotAllowed)
        );
        write
            .add_column(
                &table,
                &Column::new("m", ColumnType::I8)
                    .not_null()
                    .with_default(Value::I8(1)),
            )
            .unwrap();
        write.commit().unwrap();

        // A column without nulls and without a default, left without a value.
        let mut write = store.write().unwrap();
        let strict = create(
            &mut write,
            "strict",
            &Schema::new(
                vec![
                    Column::new("id", ColumnType::I64).not_null(),
                    Column::new("x", ColumnType::I32).not_null(),
                ],
                &["id"],
            ),
        );
        let error = write
            .insert_row(&strict, &[("id", Value::I64(1))])
            .unwrap_err();
        assert_eq!(
            invalid_value(error, folder),
            (some("strict"), some("x"), ValueRule::NullNotAllowed)
        );
        assert_eq!(write.row_count(&strict).unwrap(), 0, "no effect");
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(
            read_r(&read, "t", &[Value::I64(1)]).unwrap(),
            named(vec![
                ("id", Value::I64(1)),
                ("name", Value::Null),
                ("n", Value::I32(7)),
                ("score", Value::Null),
                ("d", Value::I32(5)),
                ("m", Value::I8(1)),
            ])
        );
    });
}

/// AC5 (E10): an update changes only the columns it gives, also when a column was added after the application
/// defined its schema; the others keep their value. An explicit null stores null.
#[test]
fn ac5_update_changes_only_the_given_columns() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &people());
        write.insert_row(&table, &person(1, "ana", 3, 1.5)).unwrap();
        write.commit().unwrap();
        // A later version of the application adds a column and fills it.
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write
            .add_column(&table, &Column::new("extra", ColumnType::I32))
            .unwrap();
        write
            .update_row(&table, &[("id", Value::I64(1)), ("extra", Value::I32(9))])
            .unwrap();
        write.commit().unwrap();
        // The earlier version, which knows only its own schema, updates one column.
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::OpenOrCreate(&people()))
            .unwrap();
        let found = write
            .update_row(&table, &[("id", Value::I64(1)), ("name", s("eva"))])
            .unwrap();
        assert!(found);
        assert!(
            write
                .update_row(&table, &[("id", Value::I64(1)), ("score", Value::Null)])
                .unwrap()
        );
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(
            read_r(&read, "t", &[Value::I64(1)]).unwrap(),
            named(vec![
                ("id", Value::I64(1)),
                ("name", s("eva").into_owned()),
                ("n", Value::I32(3)),
                ("score", Value::Null),
                ("extra", Value::I32(9)),
            ])
        );
    });
}

/// AC5 (E10): an insert or update of a row that does not exist inserts it, so a column without nulls and without a
/// default left without a value fails with an invalid value naming it and changes nothing, and the write can commit
/// its other changes; of a row that exists, it updates, and the column keeps its value.
#[test]
fn ac5_upsert_that_inserts_needs_the_columns_without_nulls() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let strict = create(
            &mut write,
            "strict",
            &Schema::new(
                vec![
                    Column::new("id", ColumnType::I64).not_null(),
                    Column::new("x", ColumnType::I32).not_null(),
                    Column::new("y", ColumnType::I32),
                ],
                &["id"],
            ),
        );
        write
            .insert_row(&strict, &[("id", Value::I64(1)), ("x", Value::I32(10))])
            .unwrap();
        let error = write
            .upsert_row(&strict, &[("id", Value::I64(2)), ("y", Value::I32(5))])
            .unwrap_err();
        assert_eq!(
            invalid_value(error, folder),
            (some("strict"), some("x"), ValueRule::NullNotAllowed)
        );
        assert_eq!(read_w(&write, &strict, &[Value::I64(2)]), None, "no effect");
        assert_eq!(write.row_count(&strict).unwrap(), 1, "no effect");
        assert_eq!(
            write
                .upsert_row(&strict, &[("id", Value::I64(1)), ("y", Value::I32(5))])
                .unwrap(),
            Upserted::Updated
        );
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(
            read_r(&read, "strict", &[Value::I64(1)]).unwrap(),
            named(vec![
                ("id", Value::I64(1)),
                ("x", Value::I32(10)),
                ("y", Value::I32(5)),
            ])
        );
        assert_eq!(read_r(&read, "strict", &[Value::I64(2)]), None);
    });
}

/// AC5 (E6, E10, E17): a row with a column that the table does not have fails with a different schema naming the
/// column; a row without a primary key column, or with a column given twice, fails with an invalid value. In insert,
/// update and insert or update, without effect.
#[test]
fn ac5_unknown_missing_and_repeated_columns() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &people());
        write.insert_row(&table, &person(1, "ana", 3, 1.5)).unwrap();
        let before = read_w(&write, &table, &[Value::I64(1)]);
        type Op = fn(
            &mut WriteTransaction<'_>,
            &WriteTable<'_>,
            &[(&str, Value<'_>)],
        ) -> krummi::Result<()>;
        let ops: [(&str, Op); 3] = [
            ("insert", |w, t, r| w.insert_row(t, r)),
            ("update", |w, t, r| w.update_row(t, r).map(|_| ())),
            ("upsert", |w, t, r| w.upsert_row(t, r).map(|_| ())),
        ];
        for (label, op) in ops {
            let error = op(
                &mut write,
                &table,
                &[("id", Value::I64(1)), ("colour", s("red"))],
            )
            .unwrap_err();
            assert_eq!(
                unknown_column(error, folder),
                ("t".to_owned(), some("colour")),
                "{label}"
            );
            let error = op(&mut write, &table, &[("name", s("x"))]).unwrap_err();
            assert_eq!(
                invalid_value(error, folder),
                (some("t"), some("id"), ValueRule::MissingKeyColumn),
                "{label}"
            );
            let error = op(
                &mut write,
                &table,
                &[
                    ("id", Value::I64(1)),
                    ("n", Value::I32(1)),
                    ("n", Value::I32(2)),
                ],
            )
            .unwrap_err();
            assert_eq!(
                invalid_value(error, folder),
                (some("t"), some("n"), ValueRule::RepeatedColumn),
                "{label}"
            );
        }
        assert_eq!(
            read_w(&write, &table, &[Value::I64(1)]),
            before,
            "no effect"
        );
        assert_eq!(write.row_count(&table).unwrap(), 1);
        write.commit().unwrap();
    });
}

/// E10, E11: a read row borrows its text and bytes from the store (D11), and the primary key of a row never changes:
/// to change it, the row is deleted and inserted with the new key in the same write.
#[test]
fn e11_read_rows_borrow_and_keys_move_by_delete_and_insert() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &texts());
        write
            .insert_row(&table, &[("k", s("a")), ("v", Value::I32(1))])
            .unwrap();
        {
            let row = write.get_row(&table, &[s("a")]).unwrap().unwrap();
            assert!(matches!(
                row.get("k"),
                Some(Value::String(Cow::Borrowed("a")))
            ));
            assert_eq!(row.name(0), Some("k"));
        }
        let old = write.get_row(&table, &[s("a")]).unwrap().unwrap();
        let moved: Vec<(String, Value<'static>)> = owned(&old)
            .into_iter()
            .map(|(n, v)| {
                if n == "k" {
                    (n, s("b").into_owned())
                } else {
                    (n, v)
                }
            })
            .collect();
        drop(old);
        assert!(write.delete_row(&table, &[s("a")]).unwrap());
        let moved: Vec<(&str, Value<'_>)> = moved
            .iter()
            .map(|(n, v)| (n.as_str(), v.reborrow()))
            .collect();
        write.insert_row(&table, &moved).unwrap();
        assert_eq!(read_w(&write, &table, &[s("a")]), None);
        assert_eq!(
            read_w(&write, &table, &[s("b")]).unwrap(),
            named(vec![("k", s("b").into_owned()), ("v", Value::I32(1))])
        );
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let row = read.get_row(&table, &[s("b")]).unwrap().unwrap();
        assert!(matches!(
            row.get("k"),
            Some(Value::String(Cow::Borrowed("b")))
        ));
    });
}

/// E11, E17: a key given to read or delete must be complete and valid for the key's columns: a missing column, a
/// value of another type, a null or more values than the key fail with an invalid value, and a key over 2,048 bytes
/// with the invalid size; a handle of another transaction is a misuse.
#[test]
fn e11_keys_to_read_and_delete() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let pairs = create(
            &mut write,
            "pairs",
            &Schema::new(
                vec![
                    Column::new("a", ColumnType::I32).not_null(),
                    Column::new("b", ColumnType::String).not_null(),
                ],
                &["a", "b"],
            ),
        );
        write
            .insert_row(&pairs, &[("b", s("x")), ("a", Value::I32(1))])
            .unwrap();
        assert!(read_w(&write, &pairs, &[Value::I32(1), s("x")]).is_some());
        let cases: [(&[Value<'_>], Option<String>, ValueRule); 5] = [
            (&[Value::I32(1)], some("b"), ValueRule::MissingKeyColumn),
            (&[], some("a"), ValueRule::MissingKeyColumn),
            (&[Value::I64(1), s("x")], some("a"), ValueRule::WrongType),
            (
                &[Value::I32(1), Value::Null],
                some("b"),
                ValueRule::NullNotAllowed,
            ),
            (
                &[Value::I32(1), s("x"), s("y")],
                None,
                ValueRule::InvalidRange,
            ),
        ];
        for (key, column, rule) in cases {
            let error = write.get_row(&pairs, key).err().unwrap();
            assert_eq!(
                invalid_value(error, folder),
                (some("pairs"), column.clone(), rule)
            );
            let error = write.delete_row(&pairs, key).unwrap_err();
            assert_eq!(invalid_value(error, folder), (some("pairs"), column, rule));
        }
        let long = "x".repeat(MAX_KEY);
        let error = write
            .get_row(&pairs, &[Value::I32(1), s(&long)])
            .err()
            .unwrap();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::PrimaryKey, 2 + 2 + 2048, 1, 2048)
        );
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("pairs").unwrap();
        let error = read.get_row(&table, &[Value::I32(1)]).err().unwrap();
        assert_eq!(
            invalid_value(error, folder),
            (some("pairs"), some("b"), ValueRule::MissingKeyColumn)
        );
        let other = store.read().unwrap();
        misuse(
            other
                .get_row(&table, &[Value::I32(1), s("x")])
                .err()
                .unwrap(),
            folder,
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC-N3: update, insert or update, read, delete and the row count

/// AC-N3 (E10, E11, N6): updating a missing row changes nothing and says it was not found; inserting or updating a
/// missing row inserts it, and again with other values updates it; reading a missing row gives none and deleting it
/// gives false. After each operation the count of the write is its number of rows, and a read opened before it still
/// sees the count it saw.
#[test]
fn ac_n3_update_upsert_read_delete_and_the_row_count() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &people());
        write.insert_row(&table, &person(1, "ana", 1, 1.0)).unwrap();
        write.commit().unwrap();
        let before = store.read().unwrap();
        let old = before.open_table("t").unwrap();

        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let other = write.open_table("t").unwrap();
        let counts = |write: &WriteTransaction<'_>, expected: u64| {
            assert_eq!(write.row_count(&table).unwrap(), expected);
            assert_eq!(write.row_count(&other).unwrap(), expected, "every handle");
            assert_eq!(before.row_count(&old).unwrap(), 1, "the earlier read");
        };
        assert!(!write.update_row(&table, &person(2, "bob", 2, 2.0)).unwrap());
        assert_eq!(
            read_w(&write, &table, &[Value::I64(2)]),
            None,
            "not inserted"
        );
        counts(&write, 1);
        assert_eq!(
            write.upsert_row(&table, &person(2, "bob", 2, 2.0)).unwrap(),
            Upserted::Inserted
        );
        counts(&write, 2);
        assert_eq!(
            write
                .upsert_row(&table, &[("id", Value::I64(2)), ("name", s("rob"))])
                .unwrap(),
            Upserted::Updated
        );
        counts(&write, 2);
        assert_eq!(
            read_w(&write, &table, &[Value::I64(2)]).unwrap(),
            pairs(&person(2, "rob", 2, 2.0)),
            "only the given columns"
        );
        assert!(write.update_row(&table, &person(1, "ann", 5, 5.0)).unwrap());
        assert_eq!(
            read_w(&write, &table, &[Value::I64(1)]).unwrap(),
            pairs(&person(1, "ann", 5, 5.0))
        );
        counts(&write, 2);
        assert_eq!(read_w(&write, &table, &[Value::I64(3)]), None);
        assert!(!write.delete_row(&table, &[Value::I64(3)]).unwrap());
        counts(&write, 2);
        write.insert_row(&table, &person(3, "cy", 3, 3.0)).unwrap();
        counts(&write, 3);
        assert!(write.delete_row(&table, &[Value::I64(1)]).unwrap());
        assert_eq!(read_w(&write, &table, &[Value::I64(1)]), None);
        counts(&write, 2);
        write.commit().unwrap();
        assert_eq!(before.row_count(&old).unwrap(), 1);
        assert_eq!(
            read_r(&before, "t", &[Value::I64(1)]).unwrap(),
            pairs(&person(1, "ana", 1, 1.0)),
            "the snapshot"
        );
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert_eq!(read.row_count(&table).unwrap(), 2);
        assert_eq!(read_r(&read, "t", &[Value::I64(1)]), None);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC14: precedence and data of the errors

/// `id i64, a i32, b i32, n i32`, all without nulls, primary key `id`.
fn abn() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("a", ColumnType::I32).not_null(),
            Column::new("b", ColumnType::I32).not_null(),
            Column::new("n", ColumnType::I32).not_null(),
        ],
        &["id"],
    )
}

/// AC14 (E17): the examples of the criterion verbatim, and the three steps: a repeated column first, in the order
/// given; else a missing key column, in key order; else the first column in schema order that breaks another rule,
/// with its first rule in E17's order.
#[test]
fn ac14_a_row_that_breaks_several_rules() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &abn());
        let rule_of = |write: &mut WriteTransaction<'_>, row: &[(&str, Value<'_>)]| {
            invalid_value(write.insert_row(&table, row).unwrap_err(), folder)
        };
        // `[(b, 2), (b, 3), (id, 1), (a, nulo)]` gives a repeated column in `b`.
        assert_eq!(
            rule_of(
                &mut write,
                &[
                    ("b", Value::I32(2)),
                    ("b", Value::I32(3)),
                    ("id", Value::I64(1)),
                    ("a", Value::Null)
                ]
            ),
            (some("t"), some("b"), ValueRule::RepeatedColumn)
        );
        // `[(n, nulo), (n, 5)]` with `n` without nulls gives a repeated column in `n`.
        assert_eq!(
            rule_of(&mut write, &[("n", Value::Null), ("n", Value::I32(5))]),
            (some("t"), some("n"), ValueRule::RepeatedColumn)
        );
        // A row without `id` and with `a` null gives a missing key column in `id`.
        assert_eq!(
            rule_of(&mut write, &[("a", Value::Null), ("b", Value::I32(1))]),
            (some("t"), some("id"), ValueRule::MissingKeyColumn)
        );
        // The first repeat in the order given, not in schema order.
        assert_eq!(
            rule_of(
                &mut write,
                &[
                    ("n", Value::I32(1)),
                    ("a", Value::I32(1)),
                    ("a", Value::I32(1)),
                    ("n", Value::I32(1))
                ]
            ),
            (some("t"), some("a"), ValueRule::RepeatedColumn)
        );
        // Two rules in two columns: the first column in schema order, whatever the order given.
        assert_eq!(
            rule_of(
                &mut write,
                &[
                    ("id", Value::I64(1)),
                    ("n", Value::I32(1)),
                    ("b", Value::I64(2)),
                    ("a", Value::Null)
                ]
            ),
            (some("t"), some("a"), ValueRule::NullNotAllowed)
        );
        // A column left without a value counts in schema order too.
        assert_eq!(
            rule_of(&mut write, &[("id", Value::I64(1)), ("b", Value::Null)]),
            (some("t"), some("a"), ValueRule::NullNotAllowed)
        );
        assert_eq!(write.row_count(&table).unwrap(), 0);
        write.commit().unwrap();

        // Two rules in one column: its first in E17's order (a vector of another dimension, with a NaN).
        let mut write = store.write().unwrap();
        let vectors = create(
            &mut write,
            "v",
            &Schema::new(
                vec![
                    Column::new("id", ColumnType::I64).not_null(),
                    Column::new("v", vector(VectorElement::F32, 2, true)),
                ],
                &["id"],
            ),
        );
        let nan3: Vec<u8> = [f32::NAN, 0.0, 0.0]
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();
        let error = write
            .insert_row(
                &vectors,
                &[
                    ("id", Value::I64(1)),
                    ("v", Value::Vector(Cow::Borrowed(&nan3))),
                ],
            )
            .unwrap_err();
        assert_eq!(
            invalid_value(error, folder),
            (some("v"), some("v"), ValueRule::VectorDimension)
        );
        write.commit().unwrap();
    });
}

/// AC14 (E17): for row operations, each pair of consecutive levels of the precedence that a row operation can meet:
/// the sizes of the column names, a handle of a deleted table, a different schema, an invalid value, the size of the
/// primary key and then of the row; the page failures and the duplicate key are in
/// [`ac14_precedence_with_damaged_pages`]. The levels of the catalog and of the invalid schema are the catalog's
/// (`tests/tables.rs`).
#[test]
fn ac14_precedence_of_row_failures() {
    let _large = one_large_test_at_a_time();
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &texts());
        let gone = write.open_table("t").unwrap();
        let blob = create(&mut write, "blob", &blobs());
        let empty_name = [("", Value::I32(1)), ("k", s("a"))];
        let error = write.insert_row(&table, &empty_name).unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::ColumnName, 0, 1, 1024)
        );
        write.delete_table("t").unwrap();
        // Names' sizes before the handle of a deleted table.
        let error = write.insert_row(&gone, &empty_name).unwrap_err();
        assert_eq!(invalid_size(error, folder).0, SizeSubject::ColumnName);
        // The handle of a deleted table before a different schema.
        let error = write
            .update_row(&gone, &[("k", s("a")), ("colour", Value::Null)])
            .unwrap_err();
        assert_eq!(not_found(error, folder), "t");
        let error = write.get_row(&gone, &[s("a")]).err().unwrap();
        assert_eq!(not_found(error, folder), "t");

        let table = create(&mut write, "t", &texts());
        // A different schema before an invalid value (a repeated column given first).
        let error = write
            .insert_row(
                &table,
                &[("k", s("a")), ("k", s("b")), ("colour", Value::Null)],
            )
            .unwrap_err();
        assert_eq!(
            unknown_column(error, folder),
            ("t".to_owned(), some("colour"))
        );
        // An invalid value before the size of the primary key.
        let long = "x".repeat(MAX_KEY);
        let error = write
            .insert_row(&table, &[("k", s(&long)), ("v", Value::I64(1))])
            .unwrap_err();
        assert_eq!(
            invalid_value(error, folder),
            (some("t"), some("v"), ValueRule::WrongType)
        );
        let error = write.insert_row(&table, &[("k", s(&long))]).unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::PrimaryKey, 2050, 1, 2048)
        );
        // The size of the primary key before the size of the row.
        let big = vec![0u8; MAX_VALUE + 1];
        let wide = create(
            &mut write,
            "wide",
            &Schema::new(
                vec![
                    Column::new("k", ColumnType::Bytes).not_null(),
                    Column::new("b", ColumnType::Bytes),
                ],
                &["k"],
            ),
        );
        let error = write
            .insert_row(&wide, &[("k", b(long.as_bytes())), ("b", b(&big))])
            .unwrap_err();
        assert_eq!(invalid_size(error, folder).0, SizeSubject::PrimaryKey);
        let error = write
            .insert_row(&blob, &[("id", Value::I64(1)), ("b", b(&big))])
            .unwrap_err();
        assert_eq!(invalid_size(error, folder).0, SizeSubject::Row);
        drop(big);
        assert_eq!(write.row_count(&table).unwrap(), 0);
        assert_eq!(write.row_count(&blob).unwrap(), 0);
        write.insert_row(&table, &[("k", s("ok"))]).unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(
            read_r(&read, "t", &[s("ok")]).unwrap(),
            named(vec![("k", s("ok").into_owned()), ("v", Value::Null)])
        );
    });
}

/// AC14 (E17): with the page of a table's rows damaged, the sizes of what needs the schema still win, as they are
/// checked without reading it; the damage wins over the duplicate key; and the two exceptions: an update, or an
/// insert or update of a row that exists, checks the size of the row it would give after reading the row, so there
/// the damage wins. In a write, damage is the transaction's failure (T11, E19).
#[test]
fn ac14_precedence_with_damaged_pages() {
    let _large = one_large_test_at_a_time();
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = create(&mut write, "blob", &blobs());
    write
        .insert_row(&table, &[("id", Value::I64(1)), ("b", b(b"x"))])
        .unwrap();
    let rows = write.table_trees(&table).unwrap().rows;
    write.commit().unwrap();
    store.close().unwrap();
    damage(tmp.path(), rows);
    let folder = Some(tmp.path());
    let big = vec![0u8; MAX_VALUE + 1];
    let over = [("id", Value::I64(1)), ("b", b(&big))];

    // The size of the row before the page failures, for an insert.
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = write.open_table("blob").unwrap();
    let error = write.insert_row(&table, &over).unwrap_err();
    assert_eq!(invalid_size(error, folder).0, SizeSubject::Row);
    // The page failure before the duplicate key.
    let error = write
        .insert_row(&table, &[("id", Value::I64(1)), ("b", b(b"x"))])
        .unwrap_err();
    assert_eq!(damaged_page(error), rows);
    assert_eq!(
        write.commit().unwrap_err().kind(),
        ErrorKind::Damaged,
        "T11"
    );

    // The exceptions: an update and an insert or update of a row that exists read it first.
    type Op =
        fn(&mut WriteTransaction<'_>, &WriteTable<'_>, &[(&str, Value<'_>)]) -> krummi::Result<()>;
    let ops: [(&str, Op); 2] = [
        ("update", |w, t, r| w.update_row(t, r).map(|_| ())),
        ("upsert", |w, t, r| w.upsert_row(t, r).map(|_| ())),
    ];
    for (label, op) in ops {
        let mut write = store.write().unwrap();
        let table = write.open_table("blob").unwrap();
        let error = op(&mut write, &table, &over).unwrap_err();
        assert_eq!(damaged_page(error), rows, "{label}");
        drop(write);
    }
    drop(big);
    store.close().unwrap();
}

/// AC14 (E17, E18): the size of the row that an update or an insert or update of an existing row would give is
/// checked after reading it, with the invalid size of a row and no effect; the write stays usable (B7).
#[test]
fn ac14_the_size_of_an_updated_row() {
    let _large = one_large_test_at_a_time();
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "blob", &blobs());
        write
            .insert_row(&table, &[("id", Value::I64(1)), ("b", b(b"x"))])
            .unwrap();
        let big = vec![0u8; MAX_VALUE + 1];
        let over = [("id", Value::I64(1)), ("b", b(&big))];
        let error = write.update_row(&table, &over).unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (
                SizeSubject::Row,
                4 + MAX_VALUE as u64 + 1,
                0,
                MAX_VALUE as u64
            )
        );
        let error = write.upsert_row(&table, &over).unwrap_err();
        assert_eq!(invalid_size(error, folder).0, SizeSubject::Row);
        drop(big);
        assert_eq!(
            read_w(&write, &table, &[Value::I64(1)]).unwrap(),
            named(vec![("id", Value::I64(1)), ("b", b(b"x").into_owned())])
        );
        write.insert_row(&table, &[("id", Value::I64(2))]).unwrap();
        write.commit().unwrap();
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC15: sizes

/// AC15 (E18): a primary key of exactly 2,048 bytes in its stored form works and one of 2,049 fails without effect
/// with the invalid size of a primary key; a `string` key is stored as its code, its bytes with each `00` escaped
/// as `00 FF`, and a terminator (document 3). The same for reading and deleting by key.
#[test]
fn ac15_a_primary_key_of_2048_bytes() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &texts());
        let fits = "k".repeat(MAX_KEY - 2);
        let zeros = "\0".repeat((MAX_KEY - 2) / 2);
        let over = "k".repeat(MAX_KEY - 1);
        write.insert_row(&table, &[("k", s(&fits))]).unwrap();
        write.insert_row(&table, &[("k", s(&zeros))]).unwrap();
        let error = write.insert_row(&table, &[("k", s(&over))]).unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::PrimaryKey, 2049, 1, 2048)
        );
        let zeros_over = "\0".repeat((MAX_KEY - 2) / 2 + 1);
        let error = write
            .upsert_row(&table, &[("k", s(&zeros_over))])
            .unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::PrimaryKey, 2050, 1, 2048)
        );
        let error = write.delete_row(&table, &[s(&over)]).unwrap_err();
        assert_eq!(invalid_size(error, folder).0, SizeSubject::PrimaryKey);
        assert_eq!(write.row_count(&table).unwrap(), 2, "no effect");
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert!(read_r(&read, "t", &[s(&fits)]).is_some());
        assert!(read_r(&read, "t", &[s(&zeros)]).is_some());
    });
}

/// AC15 (E18): a row of exactly 268,435,456 bytes in its stored form works and one of a byte more fails without
/// effect with the invalid size of a row. The row of `blobs` is the header (3 bytes), the null bitmap of its one
/// nullable column (1 byte) and the `bytes` value; the `i64` key is only in the tree's key (document 1). The value
/// that fits is dropped before the one that does not is built, and read back against its pattern, so that the test
/// holds one buffer of 256 MiB at a time besides the store's copy.
#[test]
fn ac15_a_row_of_256_mib() {
    let _large = one_large_test_at_a_time();
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "blob", &blobs());
        let header = 3 + 1;
        {
            let fits = pattern(MAX_VALUE - header);
            let row = [("id", Value::I64(1)), ("b", b(&fits))];
            let layout = write.table_schema(&table).unwrap().layout();
            let values: Vec<Value<'_>> = row.iter().map(|(_, v)| v.reborrow()).collect();
            assert_eq!(layout.encoded_len(&values), MAX_VALUE, "the stored form");
            write.insert_row(&table, &row).unwrap();
        }
        let over = vec![0u8; MAX_VALUE - header + 1];
        let error = write
            .insert_row(&table, &[("id", Value::I64(2)), ("b", b(&over))])
            .unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::Row, MAX_VALUE as u64 + 1, 0, MAX_VALUE as u64)
        );
        drop(over);
        assert_eq!(write.row_count(&table).unwrap(), 1);
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("blob").unwrap();
        let row = read.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
        assert!(
            matches!(row.get("b"), Some(Value::Bytes(Cow::Borrowed(bytes))) if is_pattern(bytes, MAX_VALUE - header))
        );
    });
}

/// The table, column and rule of an invalid-schema error, after checking its folder.
#[track_caller]
fn invalid_schema(error: Error, folder: Option<&Path>) -> (String, Option<String>, SchemaRule) {
    match error {
        Error::InvalidSchema {
            folder: f,
            table,
            column,
            rule,
            index,
        } => {
            assert_eq!(f, folder_of(folder));
            assert_eq!(index, None);
            (table, column, rule)
        }
        other => panic!("expected an invalid schema, got {other}"),
    }
}

/// `id i64 not null`, primary key `id`.
fn only_id() -> Schema {
    Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"])
}

/// A column `name bytes not null` whose default has `len` bytes.
fn bytes_default(name: &str, len: usize) -> Column {
    Column::new(name, ColumnType::Bytes)
        .not_null()
        .with_default(Value::Bytes(Cow::Owned(vec![42; len])))
}

/// E5, E17, E18 (E5's schema too large, decided on 2026-11-11 after R1 of the code review): a default that makes a
/// valid row of 256 MiB, but whose schema does not fit in the table's catalog entry, fails with a schema too large
/// naming its column, after the catalog and in every mode that may create the table, and when a column with it is
/// added: without effect, and the write commits its other changes.
#[test]
fn ac15_a_default_too_large_for_the_catalog_fails() {
    let _large = one_large_test_at_a_time();
    stores(|store, folder| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                bytes_default("b", MAX_VALUE - 3),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        for mode in [
            TableMode::CreateNew(&schema),
            TableMode::OpenOrCreate(&schema),
            TableMode::CreateOrEmpty(&schema),
        ] {
            let error = write.open_table_with("t", mode).err().unwrap();
            assert_eq!(
                invalid_schema(error, folder),
                ("t".to_owned(), some("b"), SchemaRule::SchemaTooLarge),
                "{mode:?}"
            );
            assert!(!write.tree_exists("t").unwrap(), "{mode:?}: no table");
        }
        drop(schema);
        let table = create(&mut write, "u", &only_id());
        let before = write.table_schema(&table).unwrap();
        let error = write
            .add_column(&table, &bytes_default("b", MAX_VALUE - 3))
            .unwrap_err();
        assert_eq!(
            invalid_schema(error, folder),
            ("u".to_owned(), some("b"), SchemaRule::SchemaTooLarge)
        );
        assert_eq!(write.table_schema(&table).unwrap(), before, "no effect");
        write.insert_row(&table, &[("id", Value::I64(1))]).unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert!(!read.tree_exists("t").unwrap());
        assert_eq!(read.describe_table("u").unwrap(), before);
        assert_eq!(
            read_r(&read, "u", &[Value::I64(1)]).unwrap(),
            named(vec![("id", Value::I64(1))])
        );
    });
}

/// E4, E5 (E5's schema too large, decided on 2026-11-11): a schema whose catalog entry has exactly the most bytes of
/// a value is created, and renaming its column to a longer name, which would pass them, fails with a schema too large
/// naming the renamed column, without effect.
#[test]
fn ac15_a_schema_that_fills_the_catalog_entry() {
    let _large = one_large_test_at_a_time();
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        // The size of the entry grows by one byte with each byte of the default.
        let probe = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                bytes_default("c", 1),
            ],
            &["id"],
        );
        let probe = create(&mut write, "probe", &probe);
        let small = write.table_schema(&probe).unwrap().stored_len();
        write.delete_table("probe").unwrap();
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                bytes_default("c", 1 + MAX_EXTRA - small),
            ],
            &["id"],
        );
        let table = create(&mut write, "t", &schema);
        drop(schema);
        let before = write.table_schema(&table).unwrap();
        assert_eq!(before.stored_len(), MAX_EXTRA, "the entry is full");
        let error = write.rename_column(&table, "c", "cc").unwrap_err();
        assert_eq!(
            invalid_schema(error, folder),
            ("t".to_owned(), some("cc"), SchemaRule::SchemaTooLarge)
        );
        assert_eq!(write.table_schema(&table).unwrap(), before, "no effect");
        write.commit().unwrap();
        assert_eq!(store.read().unwrap().describe_table("t").unwrap(), before);
    });
}

fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(pattern_byte).collect()
}

fn pattern_byte(i: usize) -> u8 {
    (i % 251) as u8
}

/// Whether `bytes` are the [`pattern`] of `len` bytes, without building it.
fn is_pattern(bytes: &[u8], len: usize) -> bool {
    bytes.len() == len
        && bytes
            .iter()
            .enumerate()
            .all(|(i, &byte)| byte == pattern_byte(i))
}

/// AC15 (E4, E18): column names of 1 and 1,024 bytes work in a row; of 0 and 1,025 bytes fail without effect with
/// the invalid size of a column name, before anything else.
#[test]
fn ac15_column_names_in_a_row() {
    stores(|store, folder| {
        let long = "c".repeat(1024);
        let mut write = store.write().unwrap();
        let table = create(
            &mut write,
            "t",
            &Schema::new(
                vec![
                    Column::new("a", ColumnType::I64).not_null(),
                    Column::new(long.clone(), ColumnType::I32),
                ],
                &["a"],
            ),
        );
        write
            .insert_row(&table, &[("a", Value::I64(1)), (&long, Value::I32(2))])
            .unwrap();
        let too_long = "c".repeat(1025);
        for (name, len) in [("", 0), (too_long.as_str(), 1025)] {
            let error = write
                .insert_row(&table, &[("a", Value::I64(2)), (name, Value::I32(2))])
                .unwrap_err();
            assert_eq!(
                invalid_size(error, folder),
                (SizeSubject::ColumnName, len, 1, 1024)
            );
            let error = write
                .update_row(&table, &[("a", Value::I64(1)), (name, Value::I32(3))])
                .unwrap_err();
            assert_eq!(invalid_size(error, folder).1, len);
        }
        assert_eq!(write.row_count(&table).unwrap(), 1);
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let row = read.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
        assert_eq!(row.get(&long), Some(Value::I32(2)));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC-N2, AC6, AC7: values through a table

/// AC-N2 (N4, N5, N10, E17): a duplicate primary key, also of an identical row, and a value that breaks each rule of
/// N4 fail with their class and data; the table and the transaction stay as before and the transaction commits its
/// other changes. `+0` over a stored `-0` key, and a NaN over another NaN, are the same key (N5).
#[test]
fn ac_n2_failures_leave_the_table_and_the_write_usable() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let table = create(
            &mut write,
            "t",
            &Schema::new(
                vec![
                    Column::new("id", ColumnType::I64).not_null(),
                    Column::new("x", ColumnType::I32)
                        .not_null()
                        .with_default(Value::I32(0)),
                    Column::new("d", ColumnType::Date),
                    Column::new(
                        "m",
                        ColumnType::Decimal {
                            precision: 4,
                            scale: 2,
                        },
                    ),
                    Column::new("v", vector(VectorElement::F32, 2, true)),
                ],
                &["id"],
            ),
        );
        write
            .insert_row(&table, &[("id", Value::I64(1)), ("x", Value::I32(1))])
            .unwrap();
        let before = read_w(&write, &table, &[Value::I64(1)]);
        let error = write
            .insert_row(&table, &[("id", Value::I64(1)), ("x", Value::I32(1))])
            .unwrap_err();
        assert_eq!(duplicate(error, folder), ("t".to_owned(), "(1)".to_owned()));
        let f32s = |xs: &[f32]| -> Vec<u8> { xs.iter().flat_map(|x| x.to_le_bytes()).collect() };
        let (one, wrong, nan, zero) = (
            f32s(&[1.0, 0.0]),
            f32s(&[1.0]),
            f32s(&[1.0, f32::INFINITY]),
            f32s(&[0.0, -0.0]),
        );
        let cases: [(&str, Value<'_>, ValueRule); 8] = [
            ("x", Value::Null, ValueRule::NullNotAllowed),
            ("x", Value::I64(1), ValueRule::WrongType),
            ("d", Value::Date(DATE_MAX + 1), ValueRule::OutOfDomain),
            (
                "m",
                Value::Decimal(Decimal {
                    mantissa: 10_000,
                    scale: 2,
                }),
                ValueRule::DecimalPrecision,
            ),
            (
                "v",
                Value::Vector(Cow::Borrowed(&wrong)),
                ValueRule::VectorDimension,
            ),
            (
                "v",
                Value::Vector(Cow::Borrowed(&nan)),
                ValueRule::VectorNotFinite,
            ),
            (
                "v",
                Value::Vector(Cow::Borrowed(&zero)),
                ValueRule::ZeroVectorCosine,
            ),
            ("d", s("2026-01-01"), ValueRule::WrongType),
        ];
        for (column, value, rule) in cases {
            let error = write
                .insert_row(&table, &[("id", Value::I64(2)), (column, value)])
                .unwrap_err();
            assert_eq!(
                invalid_value(error, folder),
                (some("t"), some(column), rule)
            );
        }
        assert_eq!(read_w(&write, &table, &[Value::I64(1)]), before);
        assert_eq!(read_w(&write, &table, &[Value::I64(2)]), None);
        assert_eq!(write.row_count(&table).unwrap(), 1);
        write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(3)),
                    ("v", Value::Vector(Cow::Borrowed(&one))),
                ],
            )
            .unwrap();

        // N5: the same key after normalization.
        let floats = create(
            &mut write,
            "floats",
            &Schema::new(vec![Column::new("f", ColumnType::F64).not_null()], &["f"]),
        );
        write
            .insert_row(&floats, &[("f", Value::F64(-0.0))])
            .unwrap();
        let error = write
            .insert_row(&floats, &[("f", Value::F64(0.0))])
            .unwrap_err();
        assert_eq!(
            duplicate(error, folder),
            ("floats".to_owned(), "(0.0)".to_owned())
        );
        write
            .insert_row(
                &floats,
                &[("f", Value::F64(f64::from_bits(0x7FF8_0000_0000_0001)))],
            )
            .unwrap();
        let error = write
            .insert_row(
                &floats,
                &[("f", Value::F64(f64::from_bits(0xFFF0_0000_0000_0042)))],
            )
            .unwrap_err();
        assert_eq!(
            duplicate(error, folder),
            ("floats".to_owned(), "(NaN)".to_owned())
        );
        assert_eq!(write.row_count(&floats).unwrap(), 2);
        write.commit().unwrap();

        let read = store.read().unwrap();
        assert_eq!(read.row_count(&read.open_table("t").unwrap()).unwrap(), 2);
        let floats = read.open_table("floats").unwrap();
        let zero = read.get_row(&floats, &[Value::F64(-0.0)]).unwrap().unwrap();
        assert_eq!(
            zero.get("f")
                .map(|v| matches!(v, Value::F64(z) if z.to_bits() == 0)),
            Some(true),
            "+0"
        );
        assert!(
            read.get_row(&floats, &[Value::F64(f64::NAN)])
                .unwrap()
                .is_some()
        );
    });
}

/// AC-N2 (N4): values at the limits of their types are read equal: vectors of dimension 1 (8 for `bit`) and 4,096 of
/// each element type, and a `decimal` of 38 digits of both signs.
#[test]
fn ac_n2_values_at_the_limits_of_their_types() {
    stores(|store, _| {
        let elements = [
            ("f32", VectorElement::F32),
            ("f16", VectorElement::F16),
            ("i8", VectorElement::I8),
            ("bit", VectorElement::Bit),
        ];
        let mut columns = vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new(
                "dec",
                ColumnType::Decimal {
                    precision: 38,
                    scale: 5,
                },
            ),
        ];
        for (name, element) in elements {
            let small = if element == VectorElement::Bit { 8 } else { 1 };
            columns.push(Column::new(
                format!("{name}-small"),
                vector(element, small, false),
            ));
            columns.push(Column::new(
                format!("{name}-4096"),
                vector(element, 4096, false),
            ));
        }
        let schema = Schema::new(columns, &["id"]);
        let vector_bytes = |element: VectorElement, dimension: usize| -> Vec<u8> {
            match element {
                VectorElement::F32 => (0..dimension)
                    .flat_map(|i| (i as f32 + 0.5).to_le_bytes())
                    .collect(),
                // 1.0 in binary16, then 2.0, alternating.
                VectorElement::F16 => (0..dimension)
                    .flat_map(|i| {
                        if i % 2 == 0 {
                            [0x00, 0x3C]
                        } else {
                            [0x00, 0x40]
                        }
                    })
                    .collect(),
                VectorElement::I8 => (0..dimension)
                    .map(|i| (i % 255) as u8)
                    .chain(0.25f32.to_le_bytes())
                    .collect(),
                VectorElement::Bit => (0..dimension / 8).map(|i| (i * 37) as u8).collect(),
            }
        };
        let max = 10i128.pow(38) - 1;
        for (id, mantissa) in [(1, max), (2, -max)] {
            let mut owned_values: Vec<(String, Value<'static>)> = vec![
                ("id".to_owned(), Value::I64(id)),
                (
                    "dec".to_owned(),
                    Value::Decimal(Decimal { mantissa, scale: 5 }),
                ),
            ];
            for (name, element) in elements {
                let small = if element == VectorElement::Bit { 8 } else { 1 };
                for (suffix, dim) in [("small", small), ("4096", 4096)] {
                    owned_values.push((
                        format!("{name}-{suffix}"),
                        Value::Vector(Cow::Owned(vector_bytes(element, dim))),
                    ));
                }
            }
            let mut write = store.write().unwrap();
            let table = write
                .open_table_with("t", TableMode::OpenOrCreate(&schema))
                .unwrap();
            let row: Vec<(&str, Value<'_>)> = owned_values
                .iter()
                .map(|(n, v)| (n.as_str(), v.reborrow()))
                .collect();
            write.insert_row(&table, &row).unwrap();
            assert_eq!(
                read_w(&write, &table, &[Value::I64(id)]).unwrap(),
                owned_values
            );
            write.commit().unwrap();
            let read = store.read().unwrap();
            assert_eq!(read_r(&read, "t", &[Value::I64(id)]).unwrap(), owned_values);
        }
    });
}

/// AC6 (E8): values at the ends of each domain are read equal and one beyond each end fails with an invalid value;
/// a `decimal` with more decimals than its scale rounds ties away from zero on both signs; an instant with a fraction
/// of 100 ns rounds to the microsecond, `0001-01-01T00:00:00.0000005Z` reads `0001-01-01T00:00:00.000001Z`; 99.995
/// in a `decimal(4, 2)` and `9999-12-31T23:59:59.9999995Z` fail with an invalid value.
#[test]
fn ac6_domains_and_rounding_through_a_table() {
    stores(|store, folder| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("date", ColumnType::Date),
                Column::new("time", ColumnType::Time),
                Column::new("ts", ColumnType::Timestamp),
                Column::new("dur", ColumnType::Duration),
                Column::new(
                    "dec",
                    ColumnType::Decimal {
                        precision: 4,
                        scale: 2,
                    },
                ),
            ],
            &["id"],
        );
        let micros = |m: i64| i128::from(m) * 1000;
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &schema);
        let inside: [(&str, Value<'_>); 8] = [
            ("date", Value::Date(DATE_MIN)),
            ("date", Value::Date(DATE_MAX)),
            ("time", Value::Time(0)),
            ("time", Value::Time(TIME_MAX * 1000)),
            ("ts", Value::Timestamp(micros(TIMESTAMP_MIN))),
            ("ts", Value::Timestamp(micros(TIMESTAMP_MAX))),
            ("dur", Value::Duration(-micros(DURATION_MAX))),
            ("dur", Value::Duration(micros(DURATION_MAX))),
        ];
        for (i, (column, value)) in inside.iter().enumerate() {
            let id = Value::I64(i as i64);
            write
                .insert_row(&table, &[("id", id.clone()), (column, value.clone())])
                .unwrap();
            assert_eq!(
                read_w(&write, &table, &[id]).unwrap()[..]
                    .iter()
                    .find(|(n, _)| n == column)
                    .unwrap()
                    .1,
                value.clone().into_owned()
            );
        }
        let outside: [(&str, Value<'_>); 9] = [
            ("date", Value::Date(DATE_MIN - 1)),
            ("date", Value::Date(DATE_MAX + 1)),
            ("time", Value::Time(-1000)),
            ("time", Value::Time((TIME_MAX + 1) * 1000)),
            ("ts", Value::Timestamp(micros(TIMESTAMP_MIN) - 1000)),
            ("ts", Value::Timestamp(micros(TIMESTAMP_MAX) + 1000)),
            ("dur", Value::Duration(-micros(DURATION_MAX) - 1000)),
            ("dur", Value::Duration(micros(DURATION_MAX) + 1000)),
            // 9999-12-31T23:59:59.9999995Z rounds past the last instant.
            ("ts", Value::Timestamp(micros(TIMESTAMP_MAX) + 500)),
        ];
        for (column, value) in outside {
            let error = write
                .insert_row(&table, &[("id", Value::I64(100)), (column, value)])
                .unwrap_err();
            assert_eq!(
                invalid_value(error, folder),
                (some("t"), some(column), ValueRule::OutOfDomain)
            );
        }
        // 99.995 in a decimal(4, 2) rounds to 100.00, past its precision.
        let error = write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(100)),
                    (
                        "dec",
                        Value::Decimal(Decimal {
                            mantissa: 99_995,
                            scale: 3,
                        }),
                    ),
                ],
            )
            .unwrap_err();
        assert_eq!(
            invalid_value(error, folder),
            (some("t"), some("dec"), ValueRule::DecimalPrecision)
        );
        // Rounding: decimals with ties away from zero, instants to the nearest microsecond with ties up.
        let rounded: [(&str, Value<'_>, Value<'_>); 6] = [
            (
                "dec",
                Value::Decimal(Decimal {
                    mantissa: 1_235,
                    scale: 3,
                }),
                Value::Decimal(Decimal {
                    mantissa: 124,
                    scale: 2,
                }),
            ),
            (
                "dec",
                Value::Decimal(Decimal {
                    mantissa: -1_235,
                    scale: 3,
                }),
                Value::Decimal(Decimal {
                    mantissa: -124,
                    scale: 2,
                }),
            ),
            (
                "dec",
                Value::Decimal(Decimal {
                    mantissa: 1_234,
                    scale: 3,
                }),
                Value::Decimal(Decimal {
                    mantissa: 123,
                    scale: 2,
                }),
            ),
            (
                "dec",
                Value::Decimal(Decimal {
                    mantissa: -1_234,
                    scale: 3,
                }),
                Value::Decimal(Decimal {
                    mantissa: -123,
                    scale: 2,
                }),
            ),
            (
                "ts",
                Value::Timestamp(micros(1_000) + 100),
                Value::Timestamp(micros(1_000)),
            ),
            // 0001-01-01T00:00:00.0000005Z reads 0001-01-01T00:00:00.000001Z.
            (
                "ts",
                Value::Timestamp(micros(TIMESTAMP_MIN) + 500),
                Value::Timestamp(micros(TIMESTAMP_MIN) + 1000),
            ),
        ];
        for (i, (column, given, read)) in rounded.iter().enumerate() {
            let id = Value::I64(200 + i as i64);
            write
                .insert_row(&table, &[("id", id.clone()), (column, given.clone())])
                .unwrap();
            let row = read_w(&write, &table, &[id]).unwrap();
            assert_eq!(
                row.iter().find(|(n, _)| n == column).unwrap().1,
                read.clone().into_owned(),
                "{i}"
            );
        }
        assert_eq!(
            Value::Timestamp(micros(TIMESTAMP_MIN) + 1000).to_string(),
            "0001-01-01T00:00:00.000001Z"
        );
        write.commit().unwrap();
    });
}

/// AC7 (E7): `bit` vectors with one bit set at each position of a byte, a known `f16`, a vector with `-0` and text
/// with U+0000 or empty are stored and read as E7 says: the bits where they were given, `-0` as `+0`, the texts
/// equal; and the empty text and the empty bytes serve as primary keys.
#[test]
fn ac7_bits_f16_zero_and_text_through_a_table() {
    stores(|store, _| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("bits", vector(VectorElement::Bit, 8, false)),
                Column::new("half", vector(VectorElement::F16, 1, false)),
                Column::new("floats", vector(VectorElement::F32, 2, false)),
                Column::new("text", ColumnType::String),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &schema);
        for bit in 0..8u8 {
            let bits = [1u8 << bit];
            write
                .insert_row(
                    &table,
                    &[
                        ("id", Value::I64(bit.into())),
                        ("bits", Value::Vector(Cow::Borrowed(&bits))),
                    ],
                )
                .unwrap();
        }
        let floats: Vec<u8> = [-0.0f32, 2.0]
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();
        write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(10)),
                    // 1.5 in binary16 is 0x3E00.
                    ("half", Value::Vector(Cow::Borrowed(&[0x00, 0x3E]))),
                    ("floats", Value::Vector(Cow::Borrowed(&floats))),
                    ("text", s("a\0b")),
                ],
            )
            .unwrap();
        write
            .insert_row(&table, &[("id", Value::I64(11)), ("text", s(""))])
            .unwrap();
        let strings = create(&mut write, "strings", &texts());
        write
            .insert_row(&strings, &[("k", s("")), ("v", Value::I32(1))])
            .unwrap();
        let bytes = create(
            &mut write,
            "bytes",
            &Schema::new(vec![Column::new("k", ColumnType::Bytes).not_null()], &["k"]),
        );
        write.insert_row(&bytes, &[("k", b(b""))]).unwrap();
        write.insert_row(&bytes, &[("k", b(b"\0"))]).unwrap();
        write.commit().unwrap();

        let read = store.read().unwrap();
        let t = read.open_table("t").unwrap();
        for bit in 0..8u8 {
            let row = read
                .get_row(&t, &[Value::I64(bit.into())])
                .unwrap()
                .unwrap();
            assert_eq!(
                row.get("bits"),
                Some(Value::Vector(Cow::Borrowed(&[1u8 << bit])))
            );
        }
        let row = read.get_row(&t, &[Value::I64(10)]).unwrap().unwrap();
        assert_eq!(
            row.get("half"),
            Some(Value::Vector(Cow::Borrowed(&[0x00, 0x3E])))
        );
        let plus: Vec<u8> = [0.0f32, 2.0].iter().flat_map(|x| x.to_le_bytes()).collect();
        assert_eq!(
            row.get("floats"),
            Some(Value::Vector(Cow::Borrowed(&plus))),
            "-0 as +0"
        );
        assert_eq!(row.get("text"), Some(s("a\0b")));
        let row = read.get_row(&t, &[Value::I64(11)]).unwrap().unwrap();
        assert_eq!(row.get("text"), Some(s("")));
        assert_eq!(
            read_r(&read, "strings", &[s("")]).unwrap(),
            named(vec![("k", s("").into_owned()), ("v", Value::I32(1))])
        );
        assert!(read_r(&read, "bytes", &[b(b"")]).is_some());
        assert_eq!(
            read.row_count(&read.open_table("bytes").unwrap()).unwrap(),
            2
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Row parts of AC1, AC2, AC3 and AC-N6

/// AC1 (E1, E6): Open or create with a compatible schema that lacks columns the table has opens it, and its rows read
/// with all their columns.
#[test]
fn ac1_a_compatible_subset_schema_reads_every_column() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &people());
        write
            .add_column(&table, &Column::new("extra", ColumnType::Bool))
            .unwrap();
        write
            .insert_row(
                &table,
                &[("id", Value::I64(1)), ("extra", Value::Bool(true))],
            )
            .unwrap();
        write.commit().unwrap();
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::OpenOrCreate(&people()))
            .unwrap();
        let expected = named(vec![
            ("id", Value::I64(1)),
            ("name", Value::Null),
            ("n", Value::I32(7)),
            ("score", Value::Null),
            ("extra", Value::Bool(true)),
        ]);
        assert_eq!(read_w(&write, &table, &[Value::I64(1)]).unwrap(), expected);
        write.commit().unwrap();
    });
}

/// AC2 (E2): renaming a table with rows onto another without overwrite fails with the tree already exists; with
/// overwrite the target goes with its rows, and the renamed table keeps its rows, its identity and its handles.
#[test]
fn ac2_rename_tables_with_rows() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let a = create(&mut write, "a", &people());
        let b = create(&mut write, "b", &people());
        write.insert_row(&a, &person(1, "a1", 1, 1.0)).unwrap();
        write.insert_row(&a, &person(2, "a2", 2, 2.0)).unwrap();
        write.insert_row(&b, &person(1, "b1", 1, 1.0)).unwrap();
        write.commit().unwrap();
        let mut write = store.write().unwrap();
        let a = write.open_table("a").unwrap();
        let id = write.table_schema(&a).unwrap().table_id();
        let error = write.rename_table("a", "b", false).unwrap_err();
        assert_eq!(already_exists(error, folder), "b");
        let b = write.open_table("b").unwrap();
        assert_eq!(
            read_w(&write, &b, &[Value::I64(1)]).unwrap(),
            pairs(&person(1, "b1", 1, 1.0))
        );
        write.rename_table("a", "b", true).unwrap();
        assert_eq!(not_found(write.row_count(&b).err().unwrap(), folder), "b");
        assert_eq!(write.row_count(&a).unwrap(), 2, "the handle keeps working");
        assert_eq!(
            read_w(&write, &a, &[Value::I64(1)]).unwrap(),
            pairs(&person(1, "a1", 1, 1.0))
        );
        let renamed = write.open_table("b").unwrap();
        assert_eq!(write.table_schema(&renamed).unwrap().table_id(), id);
        write.insert_row(&a, &person(3, "a3", 3, 3.0)).unwrap();
        assert_eq!(write.row_count(&renamed).unwrap(), 3);
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert!(!read.tree_exists("a").unwrap());
        assert_eq!(
            read_r(&read, "b", &[Value::I64(2)]).unwrap(),
            pairs(&person(2, "a2", 2, 2.0))
        );
        assert_eq!(read.row_count(&read.open_table("b").unwrap()).unwrap(), 3);
    });
}

/// AC3 (E3): emptying a table with rows leaves it without rows, with its schema and its identity, and another handle
/// of it sees it empty, before and after the commit.
#[test]
fn ac3_an_emptied_table_has_no_rows() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &people());
        for id in 0..50 {
            write.insert_row(&table, &person(id, "x", 1, 1.0)).unwrap();
        }
        write.commit().unwrap();
        let read = store.read().unwrap();
        let schema = read.describe_table("t").unwrap();
        drop(read);
        let mut write = store.write().unwrap();
        let other = write.open_table("t").unwrap();
        write.open_table_with("t", TableMode::Empty).unwrap();
        assert_eq!(write.row_count(&other).unwrap(), 0);
        assert_eq!(read_w(&write, &other, &[Value::I64(1)]), None);
        write.insert_row(&other, &person(1, "new", 1, 1.0)).unwrap();
        write
            .open_table_with("t", TableMode::CreateOrEmpty(&people()))
            .unwrap();
        assert_eq!(write.row_count(&other).unwrap(), 0);
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(read.describe_table("t").unwrap(), schema);
        assert_eq!(read.row_count(&read.open_table("t").unwrap()).unwrap(), 0);
        assert_eq!(read_r(&read, "t", &[Value::I64(1)]), None);
    });
}

/// AC-N6 (E4, E11, N3): rows stored before each change of schema read with the new schema: an added column that
/// allows nulls reads null, one with a default its default, a dropped column never appears, a renamed one reads under
/// its new name, a column that now allows nulls keeps its value, and one added with a dropped column's name reads
/// null. An update upgrades the row to the current version, keeping its other values.
#[test]
fn ac_n6_old_rows_read_with_the_new_schema() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &people());
        write.insert_row(&table, &person(1, "ana", 3, 1.5)).unwrap();
        write.commit().unwrap();
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write
            .add_column(&table, &Column::new("nullable", ColumnType::U8))
            .unwrap();
        write
            .add_column(
                &table,
                &Column::new("defaulted", ColumnType::I16)
                    .not_null()
                    .with_default(Value::I16(-4)),
            )
            .unwrap();
        write.drop_column(&table, "score").unwrap();
        write.rename_column(&table, "name", "label").unwrap();
        write
            .modify_column(&table, "n", ColumnType::I32, true)
            .unwrap();
        write
            .add_column(&table, &Column::new("score", ColumnType::F64))
            .unwrap();
        let expected = named(vec![
            ("id", Value::I64(1)),
            ("label", s("ana").into_owned()),
            ("n", Value::I32(3)),
            ("nullable", Value::Null),
            ("defaulted", Value::I16(-4)),
            ("score", Value::Null),
        ]);
        assert_eq!(read_w(&write, &table, &[Value::I64(1)]).unwrap(), expected);
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(read_r(&read, "t", &[Value::I64(1)]).unwrap(), expected);
        drop(read);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write
            .update_row(&table, &[("id", Value::I64(1)), ("n", Value::Null)])
            .unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(
            read_r(&read, "t", &[Value::I64(1)]).unwrap(),
            named(vec![
                ("id", Value::I64(1)),
                ("label", s("ana").into_owned()),
                ("n", Value::Null),
                ("nullable", Value::Null),
                ("defaulted", Value::I16(-4)),
                ("score", Value::Null),
            ])
        );
    });
}

/// The address of the text, bytes or vector of the column `name` of `row`, which must borrow it.
#[track_caller]
fn borrowed_at(row: &TableRow<'_>, name: &str) -> *const u8 {
    match row.get(name) {
        Some(Value::String(Cow::Borrowed(text))) => text.as_ptr(),
        Some(Value::Bytes(Cow::Borrowed(bytes)) | Value::Vector(Cow::Borrowed(bytes))) => {
            bytes.as_ptr()
        }
        Some(_) => panic!("the column {name} does not borrow its value"),
        None => panic!("the row has no column {name}"),
    }
}

/// D11, AC20 (E11, AC-N6): rows written before a column with a default was added read that default as a reference
/// into the store's memory, not as a copy made for each read: two old rows, read by their key and in a scan, in the
/// write that added the columns and in a read, borrow the same bytes of a text, a 1 MiB bytes and a vector default.
///
/// A read may see the same bytes at two addresses: the store's background transfer can move the catalog page from
/// its version in the scratch area to the data file between two reads of one read transaction. A copy made for each
/// read never gives one address, so the read is repeated in a new transaction until no transfer cuts it.
#[test]
fn e11_old_rows_borrow_the_defaults_of_added_columns() {
    stores(|store, _| {
        let big = vec![42u8; 1 << 20];
        let elements: Vec<u8> = [1f32, 2.0].iter().flat_map(|v| v.to_le_bytes()).collect();
        // Whether every row borrows each default at one address, after checking its values.
        let same_bytes = |rows: &[TableRow<'_>]| {
            for row in rows {
                assert_eq!(row.get("b"), Some(b(&big)));
                assert_eq!(row.get("s"), Some(s("text")));
                assert_eq!(row.get("v"), Some(Value::Vector(Cow::Borrowed(&elements))));
            }
            ["b", "s", "v"].iter().all(|name| {
                let first = borrowed_at(&rows[0], name);
                rows.iter().all(|row| borrowed_at(row, name) == first)
            })
        };
        let keys = [[Value::I64(1)], [Value::I64(2)]];
        let mut write = store.write().unwrap();
        let only_id = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
        let table = create(&mut write, "t", &only_id);
        for key in &keys {
            write.insert_row(&table, &[("id", key[0].clone())]).unwrap();
        }
        let added = [
            Column::new("b", ColumnType::Bytes).with_default(b(&big).into_owned()),
            Column::new("s", ColumnType::String).with_default(s("text").into_owned()),
            Column::new("v", vector(VectorElement::F32, 2, false))
                .with_default(Value::Vector(Cow::Owned(elements.clone()))),
        ];
        for column in &added {
            write.add_column(&table, column).unwrap();
        }
        {
            let mut rows: Vec<TableRow<'_>> = keys
                .iter()
                .map(|key| write.get_row(&table, key).unwrap().unwrap())
                .collect();
            rows.extend(
                write
                    .range_rows(&table, &[], ..)
                    .unwrap()
                    .map(Result::unwrap),
            );
            assert_eq!(rows.len(), 4);
            // The write changed every page it reads here, and a write's own pages do not move while it is open.
            assert!(
                same_bytes(&rows),
                "the write borrows each default at one address"
            );
        }
        write.commit().unwrap();
        // Each page moves once, so a few reads suffice; ten is a margin.
        let borrowed_once = (0..10).any(|_| {
            let read = store.read().unwrap();
            let table = read.open_table("t").unwrap();
            let mut rows: Vec<TableRow<'_>> = keys
                .iter()
                .map(|key| read.get_row(&table, key).unwrap().unwrap())
                .collect();
            rows.extend(
                read.range_rows(&table, &[], ..)
                    .unwrap()
                    .map(Result::unwrap),
            );
            assert_eq!(rows.len(), 4);
            same_bytes(&rows)
        });
        assert!(borrowed_once, "a read borrows each default at one address");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC13 and E16: what changes

/// AC13 (E16, T10): in a write that does nothing else, inserting, updating or inserting or updating a row when it is
/// not identical, and deleting a row that was there, are changes; updating a missing row, an identical update (also
/// one that gives only the key, of a row written in an older schema version), deleting a missing row, reading, counting and a failure of public validation are
/// not, and the write writes nothing.
#[test]
fn ac13_e16_what_row_operations_change() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = create(&mut write, "t", &people());
    write.insert_row(&table, &person(1, "ana", 3, 1.5)).unwrap();
    write
        .insert_row(
            &table,
            &[("id", Value::I64(2)), ("score", Value::F64(f64::NAN))],
        )
        .unwrap();
    // Rows of an older version: an identical update leaves them as they are, without upgrading them.
    write
        .add_column(&table, &Column::new("extra", ColumnType::Bool))
        .unwrap();
    write.commit().unwrap();
    store.close().unwrap();

    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        assert!(!write.update_row(&table, &person(9, "x", 1, 1.0)).unwrap());
        assert!(write.update_row(&table, &[("id", Value::I64(1))]).unwrap());
        assert!(write.update_row(&table, &person(1, "ana", 3, 1.5)).unwrap());
        assert!(
            write
                .update_row(
                    &table,
                    &[("id", Value::I64(2)), ("score", Value::F64(-f64::NAN))]
                )
                .unwrap()
        );
        assert_eq!(
            write.upsert_row(&table, &person(1, "ana", 3, 1.5)).unwrap(),
            Upserted::Updated
        );
        assert!(!write.delete_row(&table, &[Value::I64(9)]).unwrap());
        assert!(write.get_row(&table, &[Value::I64(1)]).unwrap().is_some());
        assert_eq!(write.row_count(&table).unwrap(), 2);
        let fails = [
            write.insert_row(&table, &person(1, "ana", 3, 1.5)).err(),
            write
                .insert_row(&table, &[("id", Value::I64(5)), ("n", Value::Null)])
                .err(),
            write
                .update_row(&table, &[("id", Value::I64(1)), ("zzz", Value::Null)])
                .err(),
            write.upsert_row(&table, &[("name", s("x"))]).err(),
            write.delete_row(&table, &[Value::I32(1)]).err(),
        ];
        let kinds = fails.map(|e| e.unwrap().kind());
        use ErrorKind::*;
        assert_eq!(
            kinds,
            [
                DuplicateKey,
                InvalidValue,
                DifferentSchema,
                InvalidValue,
                InvalidValue
            ]
        );
        write.commit().unwrap();
    });
    assert!(!changed, "a write that changes nothing writes nothing");

    type Change = dyn Fn(&mut WriteTransaction<'_>, &WriteTable<'_>);
    let changes: [&Change; 5] = [
        &|w, t| w.insert_row(t, &person(3, "c", 1, 1.0)).unwrap(),
        &|w, t| {
            assert!(
                w.update_row(t, &[("id", Value::I64(1)), ("n", Value::I32(4))])
                    .unwrap()
            )
        },
        &|w, t| {
            assert_eq!(
                w.upsert_row(t, &person(4, "d", 1, 1.0)).unwrap(),
                Upserted::Inserted
            )
        },
        &|w, t| {
            assert_eq!(
                w.upsert_row(t, &[("id", Value::I64(1)), ("name", Value::Null)])
                    .unwrap(),
                Upserted::Updated
            )
        },
        &|w, t| assert!(w.delete_row(t, &[Value::I64(2)]).unwrap()),
    ];
    for (i, change) in changes.iter().enumerate() {
        let copy = tempfile::tempdir().unwrap();
        for (path, bytes) in files(tmp.path()) {
            std::fs::write(copy.path().join(path.file_name().unwrap()), bytes).unwrap();
        }
        let changed = files_change(copy.path(), |store| {
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            change(&mut write, &table);
            write.commit().unwrap();
        });
        assert!(changed, "change {i}");
    }
}

/// D11 (S4 of the row reads run): a text or bytes value read from a row borrows the transaction's page, not the row,
/// so it stays valid after the row is dropped, while the transaction is open, in a read and in a write.
#[test]
fn d11_values_outlive_their_row() {
    stores(|store, _| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("s", ColumnType::String),
                Column::new("b", ColumnType::Bytes),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &schema);
        let row = [
            ("id", Value::I64(1)),
            ("s", s("text")),
            ("b", b(&[0, 1, 2])),
        ];
        write.insert_row(&table, &row).unwrap();
        let (text, bytes) = {
            let row = write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
            (row.get("s").unwrap(), row.get("b").unwrap())
        };
        assert!(
            matches!(&text, Value::String(Cow::Borrowed("text"))),
            "{text:?}"
        );
        assert!(
            matches!(&bytes, Value::Bytes(Cow::Borrowed([0, 1, 2]))),
            "{bytes:?}"
        );
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let scanned: Vec<Value<'_>> = read
            .range_rows(&table, &[], ..)
            .unwrap()
            .map(|row| row.unwrap().get("s").unwrap())
            .collect();
        assert!(
            matches!(scanned.as_slice(), [Value::String(Cow::Borrowed("text"))]),
            "{scanned:?}"
        );
    });
}

/// A read transaction keeps one reader per table, under the root of its history tree, which each table has its own of:
/// two tables that never changed their schema, read in turns in one read transaction, each read with its own schema
/// (R1-001 of the RDD review of the row reads run).
#[test]
fn tables_read_in_one_read_transaction_keep_their_own_schemas() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let a = create(&mut write, "a", &people());
        let b = create(&mut write, "b", &texts());
        for id in 0..3 {
            write.insert_row(&a, &person(id, "ann", 1, 0.5)).unwrap();
            let k = format!("k{id}");
            write
                .insert_row(&b, &[("k", s(&k)), ("v", Value::I32(10))])
                .unwrap();
        }
        write.commit().unwrap();
        let read = store.read().unwrap();
        let a = read.open_table("a").unwrap();
        let b = read.open_table("b").unwrap();
        for _ in 0..2 {
            let row = read.get_row(&a, &[Value::I64(1)]).unwrap().unwrap();
            assert_eq!(owned(&row), pairs(&person(1, "ann", 1, 0.5)));
            let row = read.get_row(&b, &[s("k1")]).unwrap().unwrap();
            assert_eq!(owned(&row), pairs(&[("k", s("k1")), ("v", Value::I32(10))]));
            assert_eq!(read.range_rows(&a, &[], ..).unwrap().count(), 3);
            let ks: Vec<String> = read
                .range_rows(&b, &[], ..)
                .unwrap()
                .map(|row| row.unwrap().text(0).unwrap().into_owned())
                .collect();
            assert_eq!(ks, ["k0", "k1", "k2"]);
        }
    });
}

/// A transaction finds the reader of each of many tables, read in turns, each with its own schema, in a read and in a
/// write transaction (R3 of the third Codex code review: the readers a transaction keeps).
#[test]
fn many_tables_read_in_turns_keep_their_own_schemas() {
    let store = krummi::Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
    let tables = 300;
    // Table `t{i}` has `i + 1` columns `c0`.. of `i32`, keyed by `c0`, and the row `c_j = j`.
    let schema = |i: usize| {
        let columns = (0..=i)
            .map(|j| Column::new(format!("c{j}"), ColumnType::I32).not_null())
            .collect();
        Schema::new(columns, &["c0"])
    };
    let mut write = store.write().unwrap();
    for i in 0..tables {
        let table = create(&mut write, &format!("t{i}"), &schema(i));
        let names: Vec<String> = (0..=i).map(|j| format!("c{j}")).collect();
        let row: Vec<(&str, Value<'_>)> = names
            .iter()
            .enumerate()
            .map(|(j, name)| (name.as_str(), Value::I32(j as i32)))
            .collect();
        write.insert_row(&table, &row).unwrap();
    }
    write.commit().unwrap();
    let expected = |i: usize| (0..=i as i32).map(Value::I32).collect::<Vec<_>>();
    let read = store.read().unwrap();
    let handles: Vec<_> = (0..tables)
        .map(|i| read.open_table(&format!("t{i}")).unwrap())
        .collect();
    for _ in 0..2 {
        for (i, table) in handles.iter().enumerate().rev() {
            let row = read.get_row(table, &[Value::I32(0)]).unwrap().unwrap();
            assert_eq!(row.values().collect::<Vec<_>>(), expected(i), "t{i}");
        }
    }
    drop(read);
    let write = store.write().unwrap();
    let handles: Vec<_> = (0..tables)
        .map(|i| write.open_table(&format!("t{i}")).unwrap())
        .collect();
    for (i, table) in handles.iter().enumerate() {
        let mut scan = write.range_rows(table, &[], ..).unwrap();
        let row = scan.next_row().unwrap().unwrap();
        assert_eq!(row.values().collect::<Vec<_>>(), expected(i), "t{i}");
    }
}

/// Threads of one read transaction open and read many tables at once, in different orders, so that they race to keep
/// their readers; each table reads with its own schema (R3-trie-concurrent-publication-uncommitted of the RDD review of
/// the row reads run).
#[test]
fn threads_of_one_read_transaction_read_each_table_with_its_own_schema() {
    let store = krummi::Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
    let tables = 64;
    // Table `t{i}` has `i % WIDTHS + 1` columns `c0`.. of `i32`, keyed by `c0`, and the row `c_j = j`.
    const WIDTHS: usize = 8;
    let width = |i: usize| i % WIDTHS + 1;
    let names = |i: usize| (0..width(i)).map(|j| format!("c{j}")).collect::<Vec<_>>();
    let mut write = store.write().unwrap();
    for i in 0..tables {
        let columns = names(i)
            .into_iter()
            .map(|name| Column::new(name, ColumnType::I32).not_null())
            .collect();
        let table = create(&mut write, &format!("t{i}"), &Schema::new(columns, &["c0"]));
        let names = names(i);
        let row: Vec<(&str, Value<'_>)> = names
            .iter()
            .enumerate()
            .map(|(j, name)| (name.as_str(), Value::I32(j as i32)))
            .collect();
        write.insert_row(&table, &row).unwrap();
    }
    write.commit().unwrap();
    let read = store.read().unwrap();
    std::thread::scope(|scope| {
        for thread in 0..8 {
            let read = &read;
            scope.spawn(move || {
                // 7 is prime to the number of tables, so each thread visits them all, from its own start.
                for step in 0..tables {
                    let i = (step * 7 + thread * 13) % tables;
                    let table = read.open_table(&format!("t{i}")).unwrap();
                    let row = read.get_row(&table, &[Value::I32(0)]).unwrap().unwrap();
                    let expected: Vec<Value<'_>> = (0..width(i) as i32).map(Value::I32).collect();
                    assert_eq!(row.values().collect::<Vec<_>>(), expected, "t{i}");
                }
            });
        }
    });
}

/// S3 of the row reads run: every column that the read of a row checked can be read, a null and a variable-length
/// column of an older version included, by key and in a scan, after a column is added and another dropped.
#[test]
fn e19_every_column_of_a_checked_older_row_reads() {
    stores(|store, _| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("s", ColumnType::String),
                Column::new("b", ColumnType::Bytes),
                Column::new("gone", ColumnType::I32),
                Column::new("flag", ColumnType::Bool),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &schema);
        write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(1)),
                    ("s", s("one")),
                    ("b", b(&[1])),
                    ("flag", Value::Bool(true)),
                ],
            )
            .unwrap();
        write.insert_row(&table, &[("id", Value::I64(2))]).unwrap();
        write
            .add_column(&table, &Column::new("added", ColumnType::String))
            .unwrap();
        write.drop_column(&table, "gone").unwrap();
        let expected = [
            vec![
                Value::I64(1),
                s("one"),
                b(&[1]),
                Value::Bool(true),
                Value::Null,
            ],
            vec![
                Value::I64(2),
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
            ],
        ];
        for (id, want) in [1, 2].into_iter().zip(&expected) {
            let row = write.get_row(&table, &[Value::I64(id)]).unwrap().unwrap();
            assert_eq!(row.values().collect::<Vec<_>>(), *want);
        }
        let scanned: Vec<Vec<Value<'_>>> = write
            .range_rows(&table, &[], ..)
            .unwrap()
            .map(|row| row.unwrap().values().collect())
            .collect();
        assert_eq!(scanned, expected);
    });
}

/// The typed reads of a row (T4 of the row reads run) give what `value` gives, for a column of every type they read,
/// in a row read, a row made owned and an older row read through the history; `None` for a null; and a key column.
#[test]
fn typed_reads_give_the_values_of_their_columns() {
    use krummi::ids::{Ulid, Uuid};
    use krummi::table::value::UuidOrder;
    stores(|store, _| {
        let types = [
            ("bool", ColumnType::Bool, Value::Bool(true)),
            ("i8", ColumnType::I8, Value::I8(-8)),
            ("i16", ColumnType::I16, Value::I16(-16)),
            ("i32", ColumnType::I32, Value::I32(-32)),
            ("i64", ColumnType::I64, Value::I64(-64)),
            ("u8", ColumnType::U8, Value::U8(8)),
            ("u16", ColumnType::U16, Value::U16(16)),
            ("u32", ColumnType::U32, Value::U32(32)),
            ("u64", ColumnType::U64, Value::U64(64)),
            ("f32", ColumnType::F32, Value::F32(3.5)),
            ("f64", ColumnType::F64, Value::F64(-6.25)),
            ("date", ColumnType::Date, Value::Date(20_000)),
            ("time", ColumnType::Time, Value::Time(3_600_000_000_000)),
            (
                "timestamp",
                ColumnType::Timestamp,
                Value::Timestamp(1_700_000_000_000_000_000),
            ),
            ("duration", ColumnType::Duration, Value::Duration(-5_000)),
            (
                "uuid",
                ColumnType::Uuid(UuidOrder::SqlServer),
                Value::Uuid(Uuid([7; 16])),
            ),
            ("ulid", ColumnType::Ulid, Value::Ulid(Ulid([9; 16]))),
            ("text", ColumnType::String, s("text")),
            ("bytes", ColumnType::Bytes, b(&[1, 0, 2])),
        ];
        let mut columns = vec![Column::new("id", ColumnType::I64).not_null()];
        columns.extend(types.iter().map(|(name, ty, _)| Column::new(*name, *ty)));
        let schema = Schema::new(columns, &["id"]);
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &schema);
        let mut full: Vec<(&str, Value<'_>)> = vec![("id", Value::I64(1))];
        full.extend(types.iter().map(|(name, _, value)| (*name, value.clone())));
        write.insert_row(&table, &full).unwrap();
        write.insert_row(&table, &[("id", Value::I64(2))]).unwrap();
        let typed = |row: &TableRow<'_>| -> Vec<Option<Value<'static>>> {
            vec![
                row.bool(1).map(Value::Bool),
                row.i8(2).map(Value::I8),
                row.i16(3).map(Value::I16),
                row.i32(4).map(Value::I32),
                row.i64(5).map(Value::I64),
                row.u8(6).map(Value::U8),
                row.u16(7).map(Value::U16),
                row.u32(8).map(Value::U32),
                row.u64(9).map(Value::U64),
                row.f32(10).map(Value::F32),
                row.f64(11).map(Value::F64),
                row.date(12).map(Value::Date),
                row.time(13).map(Value::Time),
                row.timestamp(14).map(Value::Timestamp),
                row.duration(15).map(Value::Duration),
                row.uuid(16).map(Value::Uuid),
                row.ulid(17).map(Value::Ulid),
                row.text(18).map(|t| Value::String(t.into_owned().into())),
                row.bytes(19).map(|b| Value::Bytes(b.into_owned().into())),
            ]
        };
        let check = |row: &TableRow<'_>, null: bool| {
            let want: Vec<Option<Value<'static>>> = types
                .iter()
                .map(|(_, _, value)| (!null).then(|| value.clone().into_owned()))
                .collect();
            assert_eq!(typed(row), want);
            for i in 1..=types.len() {
                let value = row.value(i).unwrap();
                assert_eq!(typed(row)[i - 1].clone().unwrap_or(Value::Null), value);
            }
            assert_eq!(row.i64(0), Some(if null { 2 } else { 1 }), "the key column");
            assert_eq!(row.i64(row.len()), None, "past the live columns");
        };
        let row = write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
        check(&row, false);
        check(&row.into_owned(), false);
        check(
            &write.get_row(&table, &[Value::I64(2)]).unwrap().unwrap(),
            true,
        );
        write
            .add_column(&table, &Column::new("added", ColumnType::I32))
            .unwrap();
        let old = write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
        check(&old, false);
        assert_eq!(old.i32(20), None, "an added column without a default");
    });
}

/// A typed read of a column of another type is a misuse of the row, which panics naming the column.
#[test]
#[should_panic(expected = "the column n is a i64, not a string")]
fn a_typed_read_of_another_type_panics() {
    let store = krummi::Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
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
        .insert_row(&table, &[("id", Value::I64(1)), ("n", Value::I64(2))])
        .unwrap();
    let row = write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
    let _ = row.text(1);
}

/// E17 and E10 with a row that gives every column in schema order (the write's fast path for such rows): it fails
/// with the same first rule, in schema order, as the same row given in another order, and it stores the same row.
#[test]
fn a_row_in_schema_order_fails_and_is_stored_as_in_any_other_order() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "p", &people());
        let bad = [
            ("id", Value::I64(1)),
            ("name", Value::I64(2)),
            ("n", Value::Null),
            ("score", s("x")),
        ];
        let reordered = [
            bad[3].clone(),
            bad[2].clone(),
            bad[1].clone(),
            bad[0].clone(),
        ];
        for row in [&bad[..], &reordered[..]] {
            let error = write.insert_row(&table, row).unwrap_err();
            assert_eq!(
                invalid_value(error, folder),
                (some("p"), some("name"), ValueRule::WrongType)
            );
        }
        // The first broken rule of the first column in schema order wins, whatever the rules of the later ones.
        let null_first = [
            ("id", Value::I64(1)),
            ("name", s("a")),
            ("n", Value::Null),
            ("score", s("x")),
        ];
        let error = write.insert_row(&table, &null_first).unwrap_err();
        assert_eq!(
            invalid_value(error, folder),
            (some("p"), some("n"), ValueRule::NullNotAllowed)
        );
        // A canonical float is stored normalized either way.
        let in_order = person(1, "a", 3, -0.0);
        let mut other = person(2, "a", 3, -0.0);
        other.reverse();
        write.insert_row(&table, &in_order).unwrap();
        write.insert_row(&table, &other).unwrap();
        let first = read_w(&write, &table, &[Value::I64(1)]).unwrap();
        let second = read_w(&write, &table, &[Value::I64(2)]).unwrap();
        assert_eq!(first[1..], second[1..]);
        assert_eq!(first[3].1, Value::F64(0.0));
        // A duplicate key in schema order is the duplicate it always was.
        let error = write.insert_row(&table, &in_order).unwrap_err();
        assert!(matches!(error, Error::DuplicateKey { .. }), "{error:?}");
    });
}

/// The rows of `table` in key order, as `(id, v)` pairs.
fn id_rows(write: &WriteTransaction<'_>, table: &WriteTable<'_>) -> Vec<(i64, Vec<u8>)> {
    write
        .range_rows(table, &[], ..)
        .unwrap()
        .map(|row| {
            let row = row.unwrap();
            match (row.get("id"), row.get("v")) {
                (Some(Value::I64(id)), Some(Value::Bytes(v))) => (id, v.into_owned()),
                other => panic!("not a row of the table: {other:?}"),
            }
        })
        .collect()
}

/// E10, E11 and E12 with the inserts of increasing keys that add at the end of the rows tree's last leaf: mixed with
/// inserts of smaller keys, deletes of rows and ranges, updates in place and of another size, upserts and emptying
/// the table, the table always holds the rows of a model, in key order, with its count, and keeps them after the
/// commit.
#[test]
fn ordered_inserts_mixed_with_other_changes_keep_the_rows_in_order() {
    stores(|store, _folder| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("v", ColumnType::Bytes),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        let mut table = create(&mut write, "t", &schema);
        let mut model = std::collections::BTreeMap::<i64, Vec<u8>>::new();
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rand = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        let mut next = 0i64;
        for step in 0..12_000 {
            let roll = rand(100);
            let len = if rand(50) == 0 {
                3_000
            } else {
                rand(120) as usize
            };
            let bytes = vec![step as u8; len];
            let existing = |r: u64, model: &std::collections::BTreeMap<i64, Vec<u8>>| {
                model.keys().nth(r as usize % model.len().max(1)).copied()
            };
            match roll {
                0..60 => {
                    next += 1 + rand(3) as i64;
                    write
                        .insert_row(&table, &[("id", Value::I64(next)), ("v", b(&bytes))])
                        .unwrap();
                    model.insert(next, bytes);
                }
                60..70 => {
                    let id = rand(next.max(1) as u64) as i64;
                    let inserted =
                        write.insert_row(&table, &[("id", Value::I64(id)), ("v", b(&bytes))]);
                    match model.entry(id) {
                        std::collections::btree_map::Entry::Vacant(e) => {
                            inserted.unwrap();
                            e.insert(bytes);
                        }
                        std::collections::btree_map::Entry::Occupied(_) => {
                            assert!(matches!(inserted, Err(Error::DuplicateKey { .. })));
                        }
                    }
                }
                70..80 => {
                    if let Some(id) = existing(rand(1 << 20), &model) {
                        assert!(write.delete_row(&table, &[Value::I64(id)]).unwrap());
                        model.remove(&id);
                    }
                }
                80..92 => {
                    if let Some(id) = existing(rand(1 << 20), &model) {
                        // Half of the updates keep the length, which rewrites the row where it is.
                        let len = if rand(2) == 0 { model[&id].len() } else { len };
                        let bytes = vec![step as u8; len];
                        assert!(
                            write
                                .update_row(&table, &[("id", Value::I64(id)), ("v", b(&bytes))])
                                .unwrap()
                        );
                        model.insert(id, bytes);
                    }
                }
                92..97 => {
                    let id = next + 1 + rand(2) as i64;
                    write
                        .upsert_row(&table, &[("id", Value::I64(id)), ("v", b(&bytes))])
                        .unwrap();
                    next = next.max(id);
                    model.insert(id, bytes);
                }
                97..99 => {
                    let from = rand(next.max(1) as u64) as i64;
                    let to = from + rand(40) as i64;
                    let removed = write
                        .delete_range(&table, &[], Value::I64(from)..Value::I64(to))
                        .unwrap();
                    let before = model.len();
                    model.retain(|&id, _| !(from..to).contains(&id));
                    assert_eq!(removed, (before - model.len()) as u64);
                }
                _ => {
                    if rand(20) == 0 {
                        table = write
                            .open_table_with("t", TableMode::CreateOrEmpty(&schema))
                            .unwrap();
                        model.clear();
                    }
                }
            }
            if step % 997 == 0 {
                let expected: Vec<_> = model.iter().map(|(k, v)| (*k, v.clone())).collect();
                assert_eq!(id_rows(&write, &table), expected, "step {step}");
                assert_eq!(write.row_count(&table).unwrap(), model.len() as u64);
            }
        }
        let expected: Vec<_> = model.iter().map(|(k, v)| (*k, v.clone())).collect();
        assert_eq!(id_rows(&write, &table), expected);
        write.commit().unwrap();
        let write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        assert_eq!(id_rows(&write, &table), expected);
        assert_eq!(write.row_count(&table).unwrap(), model.len() as u64);
    });
}
