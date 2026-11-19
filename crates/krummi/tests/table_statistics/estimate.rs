//! The estimate (AC-ST1, the parts of AC-ST2 without the background work, and the estimate's errors in AC-ST9).

use std::ops::Bound::{self, Excluded, Included, Unbounded};

use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Condition, ErrorKind, Schema, Store, TableMode, ValueRule};

use super::build::{commit_distribution, distribution, distribution_row};
use super::support::*;
use super::{create, row};

fn equal<'a>(column: &'a str, value: Value<'a>) -> Condition<'a> {
    Condition::Equal { column, value }
}

fn range<'a>(column: &'a str, lower: Bound<Value<'a>>, upper: Bound<Value<'a>>) -> Condition<'a> {
    Condition::Range {
        column,
        lower,
        upper,
    }
}

/// The estimate of `condition` on the table `table` in a new read of `store`.
fn estimate(store: &Store, table: &str, condition: &Condition<'_>) -> krummi::Estimate {
    let read = store.read().unwrap();
    let handle = read.open_table(table).unwrap();
    read.estimate(&handle, condition).unwrap()
}

fn rows_of(store: &Store, table: &str, condition: &Condition<'_>) -> f64 {
    estimate(store, table, condition).rows
}

/// `a` and `b` differ by at most 10⁻⁹ of `b`.
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

/// The rows of the table of AC-ST1 whose `n` is within `lower` and `upper`, counted from the rule that made them.
fn real_n(lower: &Bound<i64>, upper: &Bound<i64>) -> f64 {
    let n = |id: i64| match distribution_row(id)[1].1 {
        Value::I64(n) => Some(n),
        _ => None,
    };
    (0..10_000)
        .filter_map(n)
        .filter(|n| (*lower, *upper).contains(n))
        .count() as f64
}

trait Contains {
    fn contains(&self, n: &i64) -> bool;
}

impl Contains for (Bound<i64>, Bound<i64>) {
    fn contains(&self, n: &i64) -> bool {
        std::ops::RangeBounds::contains(self, n)
    }
}

/// AC-ST1 (ST2): with the table read whole, the frequent value and the nulls estimate their rows exactly, a missing
/// value and each interval their rows within 5 % of the table, and the exact rules hold.
#[test]
fn ac_st1_the_estimates_follow_the_rows() {
    stores(|store, _| {
        commit_distribution(store);
        store.analyze("d", None).unwrap();
        let frequent = estimate(store, "d", &equal("n", Value::I64(7)));
        assert!(same(frequent.rows, 4_000.0), "{}", frequent.rows);
        assert!(!frequent.stale && frequent.joint);
        assert!(same(rows_of(store, "d", &equal("n", Value::Null)), 1_000.0));
        assert!(same(
            rows_of(store, "d", &equal("b", Value::Bool(true))),
            4_000.0
        ));
        assert!(same(
            rows_of(store, "d", &equal("b", Value::Bool(false))),
            5_000.0
        ));
        assert!(
            rows_of(store, "d", &equal("n", Value::I64(5))) <= 500.0,
            "a missing value"
        );
        let text = Value::String("frequent".into());
        assert!(same(rows_of(store, "d", &equal("s", text)), 4_000.0));
        let bounds = [
            Unbounded,
            Included(1_020),
            Excluded(1_020),
            Included(1_060),
            Excluded(1_060),
            Included(7),
            Excluded(7),
        ];
        for lower in &bounds {
            for upper in &bounds {
                let value = |b: &Bound<i64>| b.map(Value::I64);
                let condition = range("n", value(lower), value(upper));
                let estimated = rows_of(store, "d", &condition);
                let real = if matches!((lower, upper), (Unbounded, Unbounded)) {
                    10_000.0
                } else {
                    real_n(lower, upper)
                };
                assert!(
                    (estimated - real).abs() <= 500.0,
                    "{lower:?}..{upper:?}: {estimated} for {real}"
                );
            }
        }
        // The exact rules: an inverted interval is empty, the whole interval is every row, the primary key alone
        // gives at most one row and a column without nulls has no null rows.
        assert_eq!(
            rows_of(
                store,
                "d",
                &range(
                    "n",
                    Included(Value::I64(1_060)),
                    Included(Value::I64(1_020))
                )
            ),
            0.0
        );
        assert_eq!(
            rows_of(store, "d", &range("n", Unbounded, Unbounded)),
            10_000.0
        );
        assert!(rows_of(store, "d", &equal("id", Value::I64(3))) <= 1.0);
        assert_eq!(rows_of(store, "d", &equal("id", Value::Null)), 0.0);
        // A large value estimates their fraction, a short one does not count them.
        let large = Value::Bytes(vec![1; 4_096].into());
        assert!(same(rows_of(store, "d", &equal("big", large)), 2_000.0));
        let short = Value::Bytes(vec![1; 2_048].into());
        assert!(rows_of(store, "d", &equal("big", short)) < 2_000.0);
    });
}

/// AC-ST2 (ST2, ST3): without statistics the first estimate samples and gives the values of AC-ST1; the others use
/// what it left in memory; an empty table estimates zero without sampling; small statistics of a table that more than
/// doubled are sampled again.
#[test]
fn ac_st2_the_first_estimate_samples_and_the_others_use_memory() {
    stores(|store, _| {
        commit_distribution(store);
        let samples = store.statistics_samples();
        assert!(same(
            rows_of(store, "d", &equal("n", Value::I64(7))),
            4_000.0
        ));
        assert_eq!(store.statistics_samples(), samples + 1, "it sampled");
        assert!(same(rows_of(store, "d", &equal("n", Value::Null)), 1_000.0));
        let mut write = store.write().unwrap();
        let table = write.open_table("d").unwrap();
        for id in 10_000..11_000 {
            write.insert_row(&table, &distribution_row(id)).unwrap();
        }
        let whole = write
            .estimate(&table, &range("n", Unbounded, Unbounded))
            .unwrap();
        assert_eq!(whole.rows, 11_000.0, "the write's own rows");
        let frequent = write.estimate(&table, &equal("n", Value::I64(7))).unwrap();
        assert!(same(frequent.rows, 4_400.0), "{}", frequent.rows);
        drop(write);
        assert_eq!(
            store.statistics_samples(),
            samples + 1,
            "memory served the others"
        );

        // An empty table estimates zero without sampling.
        let mut write = store.write().unwrap();
        create(&mut write);
        write.commit().unwrap();
        let samples = store.statistics_samples();
        assert_eq!(rows_of(store, "t", &equal("n", Value::I64(1))), 0.0);
        assert_eq!(store.statistics_samples(), samples);

        // Statistics of 100 rows are sampled again with 201 rows, not with 200.
        let grow = |from: i64, to: i64| {
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            for id in from..to {
                write.insert_row(&table, &row(id, 1)).unwrap();
            }
            write.commit().unwrap();
        };
        grow(0, 100);
        store.analyze("t", None).unwrap();
        grow(100, 200);
        let samples = store.statistics_samples();
        rows_of(store, "t", &equal("n", Value::I64(1)));
        assert_eq!(store.statistics_samples(), samples, "200 rows");
        grow(200, 201);
        rows_of(store, "t", &equal("n", Value::I64(1)));
        assert_eq!(store.statistics_samples(), samples + 1, "201 rows");
    });
}

/// AC-ST14 (ST3): a write that estimates on a table it created samples what it sees; that statistic serves only it.
#[test]
fn ac_st14_a_write_samples_its_own_table_for_itself() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("d", TableMode::CreateNew(&distribution()))
            .unwrap();
        for id in 0..1_000 {
            write.insert_row(&table, &distribution_row(id)).unwrap();
        }
        let samples = store.statistics_samples();
        let found = write.estimate(&table, &equal("n", Value::I64(7))).unwrap();
        assert!(same(found.rows, 400.0), "{}", found.rows);
        write.estimate(&table, &equal("n", Value::Null)).unwrap();
        assert_eq!(
            store.statistics_samples(),
            samples + 1,
            "once for the write"
        );
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("d").unwrap();
        assert!(
            read.column_statistics(&table, "n").unwrap().is_none(),
            "not published"
        );
        read.estimate(&table, &equal("n", Value::I64(7))).unwrap();
        assert_eq!(store.statistics_samples(), samples + 2);
    });
}

/// AC-ST9 (ST12): the errors of an estimate.
#[test]
fn ac_st9_the_errors_of_an_estimate() {
    stores(|store, _| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("n", ColumnType::I64),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let fail = |condition: Condition<'_>| read.estimate(&table, &condition).unwrap_err();
        assert_eq!(
            fail(equal("x", Value::I64(1))).kind(),
            ErrorKind::DifferentSchema
        );
        let rule = |error: krummi::Error| match error {
            krummi::Error::InvalidValue { rule, .. } => rule,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            rule(fail(equal("n", Value::Bool(true)))),
            ValueRule::WrongType
        );
        assert_eq!(
            rule(fail(range("n", Included(Value::Null), Unbounded))),
            ValueRule::InvalidRange
        );
        assert_eq!(
            rule(fail(range(
                "n",
                Included(Value::Bool(true)),
                Included(Value::Null)
            ))),
            ValueRule::WrongType,
            "the lower bound first"
        );
        // A long value is no size error, and a null on a column without nulls estimates zero.
        let estimated = read.estimate(&table, &equal("id", Value::Null)).unwrap();
        assert_eq!(estimated.rows, 0.0);
    });
}
