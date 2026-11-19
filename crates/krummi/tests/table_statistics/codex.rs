//! Regression tests of the findings of the first Codex code review (docs/reviews/tables-statistics/2026-11-19-code.md).

use std::borrow::Cow;

use krummi::table::StatisticsPoint;
use krummi::table::value::{ColumnType, Value};
use krummi::{
    Column, Condition, Direction, ErrorKind, IndexDefinition, IndexKey, IndexMode, OpenMode,
    Schema, Store, TableMode,
};

use super::support::*;

fn pair_schema() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64),
        ],
        &["id"],
    )
}

fn equal(column: &str, n: i64) -> Condition<'_> {
    Condition::Equal {
        column,
        value: Value::I64(n),
    }
}

/// R1 (T11): damage met while an estimate samples in a write makes the write unable to commit.
#[test]
fn r1_damage_while_sampling_in_a_write_fails_the_write() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&pair_schema()))
        .unwrap();
    write
        .insert_row(&table, &[("id", Value::I64(1)), ("n", Value::I64(1))])
        .unwrap();
    write.commit().unwrap();
    store.close().unwrap();
    damage(tmp.path(), root_page(tmp.path(), "t"));
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    write
        .open_tree_with("previous", OpenMode::CreateNew)
        .unwrap();
    let table = write.open_table("t").unwrap();
    let error = write.estimate(&table, &equal("n", 1)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Damaged);
    assert!(write.commit().is_err(), "the write cannot commit");
}

/// R3 (ST8): a write's own statistics end when it empties the table.
#[test]
fn r3_a_writes_own_statistics_end_when_it_empties_the_table() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&pair_schema()))
            .unwrap();
        for id in 0..1_000 {
            write
                .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(1))])
                .unwrap();
        }
        write.estimate(&table, &equal("n", 1)).unwrap();
        let table = write.open_table_with("t", TableMode::Empty).unwrap();
        assert!(write.column_statistics(&table, "n").unwrap().is_none());
        for id in 0..1_000 {
            write
                .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(2))])
                .unwrap();
        }
        let found = write.estimate(&table, &equal("n", 2)).unwrap();
        assert!((found.rows - 1_000.0).abs() < 1e-6, "{}", found.rows);
    });
}

/// R4 (AC-ST1): an interval over values whose histogram bounds were shortened keeps within 5 % of the table.
#[test]
fn r4_shortened_bounds_keep_the_interval_estimate() {
    stores(|store, _| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("s", ColumnType::String),
            ],
            &["id"],
        );
        let prefix = "B".repeat(64);
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .unwrap();
        for id in 0..10_000i64 {
            let s = match id % 10 {
                0..=3 => Value::String(Cow::Borrowed("A")),
                4 => Value::Null,
                _ => Value::String(Cow::Owned(format!("{prefix}{:03}", id % 100))),
            };
            write
                .insert_row(&table, &[("id", Value::I64(id)), ("s", s)])
                .unwrap();
        }
        write.commit().unwrap();
        store.analyze("t", None).unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let bound = |n: u32| Value::String(Cow::Owned(format!("{prefix}{n:03}")));
        let condition = Condition::Range {
            column: "s",
            lower: std::ops::Bound::Included(bound(50)),
            upper: std::ops::Bound::Included(bound(99)),
        };
        let rows = read.estimate(&table, &condition).unwrap().rows;
        let real = (0..10_000i64)
            .filter(|id| id % 10 >= 5 && id % 100 >= 50)
            .count() as f64;
        assert!((rows - real).abs() <= 500.0, "{rows} for {real}");
    });
}

/// R7 (ST6, ST7): analyze on the store saves its statistics for a table renamed while it sampled.
#[test]
fn r7_analyze_saves_for_a_table_renamed_while_it_sampled() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&pair_schema()))
        .unwrap();
    write
        .insert_row(&table, &[("id", Value::I64(1)), ("n", Value::I64(1))])
        .unwrap();
    write.commit().unwrap();
    store.hold_statistics(StatisticsPoint::WorkSampled, true);
    std::thread::scope(|scope| {
        let analyze = scope.spawn(|| store.analyze("t", None));
        std::thread::sleep(std::time::Duration::from_millis(100));
        let mut write = store.write().unwrap();
        write.rename_table("t", "u", false).unwrap();
        write.commit().unwrap();
        store.hold_statistics(StatisticsPoint::WorkSampled, false);
        analyze.join().unwrap().unwrap();
    });
    store.close().unwrap();
    let store = open(tmp.path());
    let read = store.read().unwrap();
    let table = read.open_table("u").unwrap();
    assert!(
        read.column_statistics(&table, "n").unwrap().is_some(),
        "saved"
    );
}

/// R8 (ST1): analyze in a write samples a table of more than 30,000 rows.
#[test]
fn r8_analyze_in_a_write_samples_a_large_table() {
    let store = Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&pair_schema()))
        .unwrap();
    for id in 0..60_000 {
        write
            .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(id % 7))])
            .unwrap();
    }
    write.analyze(&table, None).unwrap();
    let stats = write.column_statistics(&table, "n").unwrap().unwrap();
    assert_eq!(stats.origin_rows, 60_000);
    assert!(stats.rows_read < 60_000, "{}", stats.rows_read);
}

/// R9 (ST3, ST17): a write's own index-prefix statistics serve its next estimate.
#[test]
fn r9_a_writes_own_prefix_statistics_serve_it_again() {
    stores(|store, _| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("a", ColumnType::I64),
                Column::new("b", ColumnType::I64),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .unwrap();
        write
            .create_index(
                &table,
                &IndexDefinition::new("ix", vec![IndexKey::new("a"), IndexKey::new("b")]),
                IndexMode::CreateNew,
            )
            .unwrap();
        write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(1)),
                    ("a", Value::I64(1)),
                    ("b", Value::I64(1)),
                ],
            )
            .unwrap();
        let samples = store.statistics_samples();
        let both = Condition::EqualColumns(vec![("a", Value::I64(1)), ("b", Value::I64(1))]);
        write.estimate(&table, &both).unwrap();
        write.estimate(&table, &both).unwrap();
        assert_eq!(store.statistics_samples(), samples + 1);
    });
}

/// R10 (ST3): estimating a table that replaced a tree of the same name in this write reads the write's view.
#[test]
fn r10_a_table_that_replaced_a_tree_estimates() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        write.open_tree_with("t", OpenMode::CreateNew).unwrap();
        write.commit().unwrap();
        let mut write = store.write().unwrap();
        write.delete_tree("t").unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&pair_schema()))
            .unwrap();
        write
            .insert_row(&table, &[("id", Value::I64(1)), ("n", Value::I64(1))])
            .unwrap();
        assert!(write.estimate(&table, &equal("n", 1)).unwrap().rows <= 1.0);
    });
}

/// R11 (ST8): the statistics of a table end with an emptying that committed, also of a table without rows.
#[test]
fn r11_statistics_end_with_a_committed_emptying() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        write
            .open_table_with("t", TableMode::CreateNew(&pair_schema()))
            .unwrap();
        write.commit().unwrap();
        store.analyze("t", None).unwrap();
        let mut write = store.write().unwrap();
        write.open_table_with("t", TableMode::Empty).unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert!(read.column_statistics(&table, "n").unwrap().is_none());
    });
}

/// R14 (ST16): equal frequent combinations come in the index's order, with its direction.
#[test]
fn r14_ties_follow_the_index_order() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&pair_schema()))
            .unwrap();
        for id in [1, 2] {
            write
                .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(id))])
                .unwrap();
        }
        let mut n = IndexKey::new("n");
        n.direction = Direction::Descending;
        write
            .create_index(
                &table,
                &IndexDefinition::new("ix", vec![n, IndexKey::new("id")]),
                IndexMode::CreateNew,
            )
            .unwrap();
        write.analyze(&table, None).unwrap();
        let prefixes = write.index_statistics(&table, "ix").unwrap().unwrap();
        let first: Vec<_> = prefixes[0]
            .frequent
            .iter()
            .map(|(v, _)| v[0].clone())
            .collect();
        assert_eq!(first, [Value::I64(2), Value::I64(1)]);
    });
}

/// R15 (ST12): every name's size is checked before the columns.
#[test]
fn r15_name_sizes_come_before_the_columns() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        write
            .open_table_with("t", TableMode::CreateNew(&pair_schema()))
            .unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let pairs = vec![("missing", Value::I64(1)), ("", Value::I64(1))];
        let error = read
            .estimate(&table, &Condition::EqualColumns(pairs))
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidSize);
        drop(read);
        assert_eq!(
            store.analyze("missing", Some("")).unwrap_err().kind(),
            ErrorKind::InvalidSize
        );
    });
}
