//! Building and reading statistics (AC-ST12, parts of AC-ST1, AC-ST6 and AC-ST9), analyze in a write and on the store.

use std::borrow::Cow;

use krummi::table::value::{ColumnType, Value, VectorElement, VectorType};
use krummi::{
    Column, ColumnStatistics, Error, ErrorKind, IndexDefinition, IndexKey, IndexMode, Schema,
    SchemaRule, Store, TableMode,
};

use super::support::*;
use super::{added, create, row};

const ROWS: i64 = 10_000;

/// A table `d` of 10,000 rows from a fixed rule: in `n` and `s`, 40 % of the rows hold one value, 10 % are null and
/// the rest take 100 values evenly; `b` is 40 % true, 50 % false and 10 % null; `big` is 20 % large values of 2,049
/// bytes and the rest of 2,048; `long` holds texts of 1,000 bytes.
pub fn distribution() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64),
            Column::new("s", ColumnType::String),
            Column::new("b", ColumnType::Bool),
            Column::new("big", ColumnType::Bytes),
            Column::new("long", ColumnType::String),
        ],
        &["id"],
    )
}

pub fn distribution_row(id: i64) -> Vec<(&'static str, Value<'static>)> {
    let slot = id % 10;
    let other = 1_000 + (id / 10) % 100;
    let n = match slot {
        0..=3 => Value::I64(7),
        4 => Value::Null,
        _ => Value::I64(other),
    };
    let s = match slot {
        0..=3 => Value::String(Cow::Borrowed("frequent")),
        4 => Value::Null,
        _ => Value::String(Cow::Owned(format!("value {other}"))),
    };
    let b = match slot {
        0..=3 => Value::Bool(true),
        4 => Value::Null,
        _ => Value::Bool(false),
    };
    let big = vec![(id % 251) as u8; if id % 5 == 0 { 2_049 } else { 2_048 }];
    let long = format!("{:0>1000}", id % 300);
    vec![
        ("id", Value::I64(id)),
        ("n", n),
        ("s", s),
        ("b", b),
        ("big", Value::Bytes(Cow::Owned(big))),
        ("long", Value::String(Cow::Owned(long))),
    ]
}

pub fn commit_distribution(store: &Store) {
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("d", TableMode::CreateNew(&distribution()))
        .unwrap();
    for id in 0..ROWS {
        write.insert_row(&table, &distribution_row(id)).unwrap();
    }
    write.commit().unwrap();
}

pub fn read_statistics(store: &Store, column: &str) -> Option<ColumnStatistics> {
    let read = store.read().unwrap();
    let table = read.open_table("d").unwrap();
    read.column_statistics(&table, column).unwrap()
}

pub fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9
}

/// AC-ST12 (ST16), with the table of AC-ST1: before analyze there are none; after it, each column's origin and
/// fractions are those of its rows.
#[test]
fn ac_st12_the_statistics_read_are_those_of_the_rows() {
    stores(|store, _| {
        commit_distribution(store);
        assert_eq!(read_statistics(store, "n"), None, "none before analyze");
        store.analyze("d", None).unwrap();
        for column in ["n", "s", "b"] {
            let stats = read_statistics(store, column).unwrap();
            assert_eq!(
                (stats.origin_rows, stats.rows_read, stats.stale),
                (10_000, 10_000, false),
                "{column}"
            );
            assert_eq!(stats.origin_counter, 10_000, "{column}");
            assert!(close(stats.null_fraction, 0.1), "{column}");
            let total = stats.null_fraction
                + stats.large_fraction
                + stats.frequent.iter().map(|(_, f)| f).sum::<f64>()
                + stats.histogram.iter().map(|b| b.fraction).sum::<f64>();
            assert!(close(total, 1.0), "{column}: {total}");
        }
        let n = read_statistics(store, "n").unwrap();
        assert_eq!(n.frequent[0].0, Value::I64(7));
        assert!(close(n.frequent[0].1, 0.4));
        assert!(close(n.distinct, 101.0), "{}", n.distinct);
        let s = read_statistics(store, "s").unwrap();
        assert_eq!(s.frequent[0].0, Value::String(Cow::Borrowed("frequent")));
        let b = read_statistics(store, "b").unwrap();
        assert_eq!(b.frequent[0].0, Value::Bool(false));
        assert!(close(b.frequent[0].1, 0.5));
        assert_eq!(b.frequent[1].0, Value::Bool(true));
        // Large values: their fraction, out of the frequent values and the histogram.
        let big = read_statistics(store, "big").unwrap();
        assert!(close(big.large_fraction, 0.2), "{}", big.large_fraction);
        assert_eq!(read_statistics(store, "n").unwrap().large_fraction, 0.0);
        // Texts of 1,000 bytes give bounds shortened to a prefix of a value of the table.
        let long = read_statistics(store, "long").unwrap();
        assert!(!long.histogram.is_empty());
        for bucket in &long.histogram {
            let Value::String(upper) = &bucket.upper else {
                panic!("a text bound")
            };
            assert!(bucket.shortened && upper.len() < 1_000);
            assert!(
                upper.chars().all(|c| c.is_ascii_digit()),
                "a prefix of a value"
            );
        }
        // Buckets go in the column's order.
        let uppers: Vec<i64> = (n.histogram.iter())
            .map(|b| match b.upper {
                Value::I64(n) => n,
                _ => panic!("an i64 bound"),
            })
            .collect();
        assert!(uppers.windows(2).all(|w| w[0] < w[1]), "{uppers:?}");
    });
}

/// AC-ST12 (ST5, ST16): statistics become stale for transactions that see more modifications than the threshold; a
/// table analyzed with no rows gives all zero.
#[test]
fn ac_st12_staleness_and_an_empty_table() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        create(&mut write);
        let table = write.open_table("t").unwrap();
        write.analyze(&table, None).unwrap();
        write.commit().unwrap();
        let stats = |store: &Store| {
            let read = store.read().unwrap();
            let table = read.open_table("t").unwrap();
            read.column_statistics(&table, "n").unwrap().unwrap()
        };
        let empty = stats(store);
        assert_eq!((empty.origin_rows, empty.rows_read), (0, 0));
        assert!(empty.frequent.is_empty() && empty.histogram.is_empty());
        assert_eq!((empty.null_fraction, empty.distinct), (0.0, 0.0));
        // 500 modifications keep them fresh; one more makes them stale.
        added(store, |write, table| {
            for id in 0..500 {
                write.insert_row(table, &row(id, id)).unwrap();
            }
        });
        assert!(!stats(store).stale);
        added(store, |write, table| {
            write.insert_row(table, &row(500, 0)).unwrap();
        });
        assert!(stats(store).stale);
    });
}

/// AC-ST6 and ST8: renaming a column or the table, and creating or dropping an index, keep them; an added column has
/// none, also one with the name of a dropped column; emptying the table and dropping it take them away.
#[test]
fn ac_st6_table_changes_keep_or_drop_the_statistics() {
    stores(|store, _| {
        commit_distribution(store);
        store.analyze("d", None).unwrap();
        let mut write = store.write().unwrap();
        let table = write.open_table("d").unwrap();
        write.rename_column(&table, "n", "m").unwrap();
        write
            .create_index(
                &table,
                &IndexDefinition::new("ix", vec![IndexKey::new("m")]),
                IndexMode::CreateNew,
            )
            .unwrap();
        write.drop_index(&table, "ix").unwrap();
        assert!(write.column_statistics(&table, "m").unwrap().is_some());
        write.drop_column(&table, "s").unwrap();
        write
            .add_column(&table, &Column::new("s", ColumnType::String))
            .unwrap();
        assert!(
            write.column_statistics(&table, "s").unwrap().is_none(),
            "a new column"
        );
        write.commit().unwrap();
        let mut write = store.write().unwrap();
        write.rename_table("d", "e", false).unwrap();
        write.rename_table("e", "d", false).unwrap();
        write.commit().unwrap();
        assert!(
            read_statistics(store, "m").is_some(),
            "renaming the table keeps them"
        );
        let mut write = store.write().unwrap();
        let table = write.open_table_with("d", TableMode::Empty).unwrap();
        assert!(
            write.column_statistics(&table, "m").unwrap().is_none(),
            "emptying"
        );
        write.commit().unwrap();
        assert!(read_statistics(store, "m").is_none());
        store.analyze("d", None).unwrap();
        let mut write = store.write().unwrap();
        write.delete_table("d").unwrap();
        write
            .open_table_with("d", TableMode::CreateNew(&distribution()))
            .unwrap();
        write.commit().unwrap();
        assert!(read_statistics(store, "n").is_none(), "a new table");
    });
}

/// ST7: analyze in a write sees its own rows, and an abort leaves the statistics as they were.
#[test]
fn analyze_in_a_write_sees_its_rows_and_an_abort_leaves_them() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        create(&mut write);
        write.commit().unwrap();
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        for id in 0..10 {
            write.insert_row(&table, &row(id, 1)).unwrap();
        }
        write.analyze(&table, Some("n")).unwrap();
        let stats = write.column_statistics(&table, "n").unwrap().unwrap();
        assert_eq!((stats.origin_rows, stats.origin_counter), (10, 10));
        assert!(
            write.column_statistics(&table, "id").unwrap().is_none(),
            "one column"
        );
        drop(write);
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert!(
            read.column_statistics(&table, "n").unwrap().is_none(),
            "aborted"
        );
    });
}

/// AC-ST9 (ST12): the errors of reading statistics and of analyze.
#[test]
fn ac_st9_the_errors_of_reading_and_analyze() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        create(&mut write);
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let kind = |column: &str| read.column_statistics(&table, column).unwrap_err().kind();
        assert_eq!(kind("x"), ErrorKind::DifferentSchema);
        assert_eq!(kind(&"x".repeat(1_025)), ErrorKind::InvalidSize);
        assert_eq!(kind(""), ErrorKind::InvalidSize);
        drop(read);
        let analyze = |table: &str, column: Option<&str>| store.analyze(table, column).unwrap_err();
        assert_eq!(analyze("nope", None).kind(), ErrorKind::TreeNotFound);
        assert_eq!(analyze("t", Some("x")).kind(), ErrorKind::DifferentSchema);
        let write = store.write().unwrap();
        assert_eq!(analyze("t", None).kind(), ErrorKind::Misuse);
        drop(write);
        // A vector column has no statistics.
        let vector = ColumnType::Vector(VectorType {
            element: VectorElement::F32,
            dimension: 2,
            cosine: false,
        });
        let vectors = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("v", vector),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        write
            .open_table_with("v", TableMode::CreateNew(&vectors))
            .unwrap();
        write.commit().unwrap();
        match analyze("v", Some("v")) {
            Error::InvalidSchema { rule, .. } => assert_eq!(rule, SchemaRule::VectorColumn),
            other => panic!("{other:?}"),
        }
        store.analyze("v", None).unwrap();
    });
}

/// AC-ST1 and AC-ST12 (ST1): a table of 200,000 rows is sampled with the seed of the test hook: fewer rows are read
/// than it has, and the frequent value and the nulls estimate their rows within 5 % of the table.
#[test]
fn ac_st1_a_large_table_is_sampled() {
    stores(|store, _| {
        // The rule of `n` in the table of AC-ST1, in rows of two columns.
        let small = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("n", ColumnType::I64),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        write
            .open_table_with("d", TableMode::CreateNew(&small))
            .unwrap();
        write.commit().unwrap();
        let rows = (0..200_000).map(|id| {
            let row = distribution_row(id);
            Ok::<_, std::convert::Infallible>(vec![row[0].clone(), row[1].clone()])
        });
        store.bulk_load("d", rows).unwrap();
        store.set_statistics_seed(7);
        store.analyze("d", None).unwrap();
        let n = read_statistics(store, "n").unwrap();
        assert_eq!(n.origin_rows, 200_000);
        assert!(
            n.rows_read < 200_000 && n.rows_read >= 20_000,
            "{}",
            n.rows_read
        );
        assert!((n.null_fraction - 0.1).abs() <= 0.05, "{}", n.null_fraction);
        assert_eq!(n.frequent[0].0, Value::I64(7));
        assert!((n.frequent[0].1 - 0.4).abs() <= 0.05, "{}", n.frequent[0].1);
    });
}
