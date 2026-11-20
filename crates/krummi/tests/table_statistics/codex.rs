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

/// R4 (AC-ST1): intervals over values whose histogram bounds were shortened keep within 5 % of the table: 4,000
/// rows `A`, 1,000 nulls and 50 rows of each of 100 suffixes after 64 `B`s (Codex's distribution).
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
        let suffix = |id: i64| (id / 10) % 100;
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .unwrap();
        for id in 0..10_000i64 {
            let s = match id % 10 {
                0..=3 => Value::String(Cow::Borrowed("A")),
                4 => Value::Null,
                _ => Value::String(Cow::Owned(format!("{prefix}{:03}", suffix(id)))),
            };
            write
                .insert_row(&table, &[("id", Value::I64(id)), ("s", s)])
                .unwrap();
        }
        write.commit().unwrap();
        store.analyze("t", None).unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let bound = |n: i64| Value::String(Cow::Owned(format!("{prefix}{n:03}")));
        for (from, to) in [(50, 99), (90, 99), (0, 99), (100, 199)] {
            let condition = Condition::Range {
                column: "s",
                lower: std::ops::Bound::Included(bound(from)),
                upper: std::ops::Bound::Included(bound(to)),
            };
            let rows = read.estimate(&table, &condition).unwrap().rows;
            let real = (0..10_000i64)
                .filter(|id| id % 10 >= 5 && (from..=to).contains(&suffix(*id)))
                .count() as f64;
            assert!(
                (rows - real).abs() <= 500.0,
                "{from}..{to}: {rows} for {real}"
            );
        }
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
        let samples = store.statistics_samples();
        let analyze = scope.spawn(|| store.analyze("t", None));
        wait_sampling(&store, samples);
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

/// R1 (T11): an auxiliary snapshot that cannot open fails the requesting write too.
#[test]
fn r1_a_failed_auxiliary_snapshot_fails_the_write() {
    let store = Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&pair_schema()))
        .unwrap();
    write
        .insert_row(&table, &[("id", Value::I64(1)), ("n", Value::I64(1))])
        .unwrap();
    write.commit().unwrap();
    let mut write = store.write().unwrap();
    write
        .open_tree_with("previous", OpenMode::CreateNew)
        .unwrap();
    let table = write.open_table("t").unwrap();
    krummi_pager::testing::reset();
    krummi_pager::testing::inject(
        krummi_pager::testing::Op::Snapshot,
        0,
        krummi_pager::testing::Action::Fail(std::io::ErrorKind::OutOfMemory),
    );
    let error = write.estimate(&table, &equal("n", 1)).unwrap_err();
    krummi_pager::testing::reset();
    assert_eq!(error.kind(), ErrorKind::OutOfMemory);
    assert!(write.commit().is_err(), "the write cannot commit");
}

/// R9 (ST16): a write reads the index-prefix statistics it built itself.
#[test]
fn r9_a_write_reads_its_own_prefix_statistics() {
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
        let both = Condition::EqualColumns(vec![("a", Value::I64(1)), ("b", Value::I64(1))]);
        write.estimate(&table, &both).unwrap();
        assert!(write.index_statistics(&table, "ix").unwrap().is_some());
    });
}

/// R11 (ST8): a build sampled before a committed emptying is neither published nor saved.
#[test]
fn r11_a_build_from_before_an_emptying_is_dropped() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        write
            .open_table_with("t", TableMode::CreateNew(&pair_schema()))
            .unwrap();
        write.commit().unwrap();
        store.hold_statistics(StatisticsPoint::WorkSampled, true);
        std::thread::scope(|scope| {
            let samples = store.statistics_samples();
            let analyze = scope.spawn(|| store.analyze("t", None));
            wait_sampling(store, samples);
            let mut write = store.write().unwrap();
            write.open_table_with("t", TableMode::Empty).unwrap();
            write.commit().unwrap();
            store.hold_statistics(StatisticsPoint::WorkSampled, false);
            analyze.join().unwrap().unwrap();
        });
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert!(read.column_statistics(&table, "n").unwrap().is_none());
    });
}

/// R15 (ST12): with a handle of a table this write deleted, an invalid name still comes first.
#[test]
fn r15_a_deleted_handle_comes_after_the_names() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&pair_schema()))
            .unwrap();
        write.delete_table("t").unwrap();
        let empty = Condition::Equal {
            column: "",
            value: Value::I64(1),
        };
        assert_eq!(
            write.estimate(&table, &empty).unwrap_err().kind(),
            ErrorKind::InvalidSize
        );
        assert_eq!(
            write.column_statistics(&table, "").unwrap_err().kind(),
            ErrorKind::InvalidSize
        );
        assert_eq!(
            write.index_statistics(&table, "").unwrap_err().kind(),
            ErrorKind::InvalidSize
        );
        assert_eq!(
            write.analyze(&table, Some("")).unwrap_err().kind(),
            ErrorKind::InvalidSize
        );
    });
}

/// R16 (ST8): a committed column rename while analyze samples keeps that build: it is published and saved.
#[test]
fn r16_a_rename_keeps_a_pending_build() {
    stores(|store, _| {
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
            let samples = store.statistics_samples();
            let analyze = scope.spawn(|| store.analyze("t", None));
            wait_sampling(store, samples);
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            write.rename_column(&table, "n", "m").unwrap();
            write.commit().unwrap();
            store.hold_statistics(StatisticsPoint::WorkSampled, false);
            analyze.join().unwrap().unwrap();
        });
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert!(read.column_statistics(&table, "m").unwrap().is_some());
    });
}

/// R11 (ST8): once a write that removed a column commits, an older read that samples it does not publish its
/// statistics again for the others.
#[test]
fn r11_a_removed_column_is_not_republished() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&pair_schema()))
            .unwrap();
        write
            .insert_row(&table, &[("id", Value::I64(1)), ("n", Value::I64(1))])
            .unwrap();
        write.commit().unwrap();
        let old = store.read().unwrap();
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.drop_column(&table, "n").unwrap();
        write.commit().unwrap();
        let old_table = old.open_table("t").unwrap();
        old.estimate(&old_table, &equal("n", 1)).unwrap();
        // The old read's own sample served it, but memory keeps nothing for the removed column.
        assert!(old.column_statistics(&old_table, "n").unwrap().is_none());
    });
}

/// R13 (ST9): damage met by a background renewal drops it without a visible error: the stale statistics are still
/// used and the store still accepts writes.
#[test]
fn r13_damage_during_a_renewal_is_dropped() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&pair_schema()))
        .unwrap();
    for id in 0..600 {
        write
            .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(id % 2))])
            .unwrap();
    }
    write.commit().unwrap();
    store.analyze("t", None).unwrap();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    for id in 600..1_300 {
        write
            .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(id % 2))])
            .unwrap();
    }
    write.commit().unwrap();
    store.close().unwrap();
    damage(tmp.path(), root_page(tmp.path(), "t"));
    let store = open(tmp.path());
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let stale = read.estimate(&table, &equal("n", 1)).unwrap();
    assert!(stale.stale, "700 modifications on 600 rows (threshold 620)");
    drop(read);
    store.wait_statistics_idle();
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let again = read.estimate(&table, &equal("n", 1)).unwrap();
    assert!(
        again.stale,
        "the renewal was dropped; the stale ones still serve"
    );
    drop(read);
    let mut write = store.write().unwrap();
    write.open_tree_with("after", OpenMode::CreateNew).unwrap();
    write.commit().unwrap();
}

/// Analyzes `t` of 600 rows and adds 700 more, so that its statistics are stale (threshold 620).
fn stale_table(store: &Store) {
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&pair_schema()))
        .unwrap();
    for id in 0..600 {
        write
            .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(id % 2))])
            .unwrap();
    }
    write.commit().unwrap();
    store.analyze("t", None).unwrap();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    for id in 600..1_300 {
        write
            .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(id % 2))])
            .unwrap();
    }
    write.commit().unwrap();
}

/// R13 (ST9): a renewal whose sample runs out of memory is dropped without a visible error; the stale statistics still
/// serve, writes go on, and the next estimate asks for a renewal again, which then renews them.
#[test]
fn r13_a_renewal_out_of_memory_is_dropped_and_asked_again() {
    use krummi_pager::testing::{self, Action, Op};
    stores(|store, _| {
        stale_table(store);
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        testing::reset();
        testing::inject(
            Op::Snapshot,
            0,
            Action::Fail(std::io::ErrorKind::OutOfMemory),
        );
        assert!(read.estimate(&table, &equal("n", 1)).unwrap().stale);
        store.wait_statistics_idle();
        testing::reset();
        drop(read);
        let mut write = store.write().unwrap();
        write.open_tree_with("after", OpenMode::CreateNew).unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert!(
            read.estimate(&table, &equal("n", 1)).unwrap().stale,
            "dropped: still stale"
        );
        drop(read);
        store.wait_statistics_idle();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert!(
            !read.estimate(&table, &equal("n", 1)).unwrap().stale,
            "renewed when asked again"
        );
    });
}

/// R13 (ST7, ST12): analyze on the store that cannot open its snapshot fails with its class and changes nothing.
#[test]
fn r13_analyze_without_memory_for_its_snapshot_changes_nothing() {
    use krummi_pager::testing::{self, Action, Op};
    stores(|store, _| {
        let mut write = store.write().unwrap();
        write
            .open_table_with("t", TableMode::CreateNew(&pair_schema()))
            .unwrap();
        write.commit().unwrap();
        testing::reset();
        testing::inject(
            Op::Snapshot,
            0,
            Action::Fail(std::io::ErrorKind::OutOfMemory),
        );
        let error = store.analyze("t", None).unwrap_err();
        testing::reset();
        assert_eq!(error.kind(), ErrorKind::OutOfMemory);
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert!(read.column_statistics(&table, "n").unwrap().is_none());
    });
}

/// R2 (ST1): rows stored in runs are sampled by the pages they use, without the run's checksum and without reading
/// their large values: damage in the middle of a run of a large value does not fail analyze, and the statistics of the
/// other columns and the large values' fraction are the rows'.
#[test]
fn r2_a_run_is_sampled_by_the_pages_used() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64),
            Column::new("blob", ColumnType::Bytes),
        ],
        &["id"],
    );
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema))
        .unwrap();
    for id in 0..20 {
        write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(id)),
                    ("n", Value::I64(id % 2)),
                    ("blob", Value::Bytes(Cow::Owned(vec![id as u8; 200_000]))),
                ],
            )
            .unwrap();
    }
    write.commit().unwrap();
    store.close().unwrap();
    let key = krummi::table::key::encode_key(&[ColumnType::I64], &[Value::I64(7)]).unwrap();
    let (first, count) = value_run(tmp.path(), "t", &key).expect("a row in a run");
    assert!(count > 4);
    damage(tmp.path(), first + count / 2);
    let store = open(tmp.path());
    store.analyze("t", None).unwrap();
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let n = read.column_statistics(&table, "n").unwrap().unwrap();
    assert_eq!(n.rows_read, 20);
    assert!((n.frequent[0].1 - 0.5).abs() < 1e-9);
    let blob = read.column_statistics(&table, "blob").unwrap().unwrap();
    assert!((blob.large_fraction - 1.0).abs() < 1e-9);
    // Reading the damaged row itself still fails: only sampling skips the run's check.
    let error = read.get_row(&table, &[Value::I64(7)]).err();
    assert_eq!(error.map(|e| e.kind()), Some(ErrorKind::Damaged));
}

/// R2 (ST1): a run whose row header and offsets are corrupt never makes the sample panic: analyze ends, with the row
/// left out or a damage error.
#[test]
fn r2_a_corrupt_run_never_panics_the_sample() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("s", ColumnType::String),
            Column::new("blob", ColumnType::Bytes),
        ],
        &["id"],
    );
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema))
        .unwrap();
    for id in 0..5 {
        write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(id)),
                    ("s", Value::String(Cow::Borrowed("x"))),
                    ("blob", Value::Bytes(Cow::Owned(vec![1; 50_000]))),
                ],
            )
            .unwrap();
    }
    write.commit().unwrap();
    store.close().unwrap();
    let key = krummi::table::key::encode_key(&[ColumnType::I64], &[Value::I64(2)]).unwrap();
    let (first, _) = value_run(tmp.path(), "t", &key).expect("a row in a run");
    let path = tmp.path().join(krummi_pager::DATA_FILE);
    let mut bytes = std::fs::read(&path).unwrap();
    let start = first as usize * krummi_pager::format::PAGE_SIZE;
    for byte in &mut bytes[start + 8..start + 80] {
        *byte = 0xFF;
    }
    std::fs::write(&path, bytes).unwrap();
    let store = open(tmp.path());
    let outcome =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| store.analyze("t", None)));
    let result = outcome.expect("no panic");
    assert!(
        result.is_ok()
            || result
                .as_ref()
                .is_err_and(|e| e.kind() == ErrorKind::Damaged),
        "{result:?}"
    );
}

/// The rows of a table `t` of `id`, `n` (`id % 2`) and `blob`, of 50,000 bytes in one row of every `every` and null
/// in the rest, loaded into a new store at `folder`, which is closed; the middle page of each value run is damaged.
/// Returns the keys of the rows in runs.
fn damaged_runs(folder: &std::path::Path, rows: i64, every: i64) -> Vec<i64> {
    let store = open(folder);
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64),
            Column::new("blob", ColumnType::Bytes),
        ],
        &["id"],
    );
    let mut write = store.write().unwrap();
    write
        .open_table_with("t", TableMode::CreateNew(&schema))
        .unwrap();
    write.commit().unwrap();
    let values = (0..rows).map(|id| {
        let blob = match id % every {
            0 => Value::Bytes(Cow::Owned(vec![id as u8; 50_000])),
            _ => Value::Null,
        };
        Ok::<_, std::convert::Infallible>(vec![
            ("id", Value::I64(id)),
            ("n", Value::I64(id % 2)),
            ("blob", blob),
        ])
    });
    store.bulk_load("t", values).unwrap();
    store.close().unwrap();
    let large: Vec<i64> = (0..rows).filter(|id| id % every == 0).collect();
    for id in &large {
        let key = krummi::table::key::encode_key(&[ColumnType::I64], &[Value::I64(*id)]).unwrap();
        let (first, count) = value_run(folder, "t", &key).expect("a row in a run");
        assert!(count > 2);
        damage(folder, first + count / 2);
    }
    large
}

/// RDD follow-up (ST1): a table of more than 30,000 rows is sampled by leaves reached at random, also without the
/// run's checksum: damage in the middle of the runs of its large values does not fail analyze, and the large values
/// read count in their fraction.
#[test]
fn a_random_leaf_sample_skips_the_runs_checksum() {
    let tmp = tempfile::tempdir().unwrap();
    let large = damaged_runs(tmp.path(), 31_000, 500);
    let store = open(tmp.path());
    store.set_statistics_seed(7);
    store.analyze("t", None).unwrap();
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let n = read.column_statistics(&table, "n").unwrap().unwrap();
    assert!(n.rows_read < 31_000, "sampled: {}", n.rows_read);
    let blob = read.column_statistics(&table, "blob").unwrap().unwrap();
    assert!(blob.large_fraction > 0.0, "damaged runs sampled");
    let error = read.get_row(&table, &[Value::I64(large[0])]).err();
    assert_eq!(error.map(|e| e.kind()), Some(ErrorKind::Damaged));
}

/// RDD follow-up (ST1, ST3, T11): an estimate in a write that changed the table's columns samples the write's own view,
/// reading the runs without their checksum: damage in the middle of a run of a large value neither fails the estimate
/// nor keeps the write from committing (R1 of docs/reviews/tables-statistics-followups/2026-11-20-code.md).
#[test]
fn sampling_in_a_write_skips_the_runs_checksum() {
    let tmp = tempfile::tempdir().unwrap();
    damaged_runs(tmp.path(), 20, 4);
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    // A column the committed table does not have: the estimate samples through the write (ST3).
    write
        .add_column(&table, &Column::new("extra", ColumnType::I64))
        .unwrap();
    let table = write.open_table("t").unwrap();
    let estimate = write.estimate(&table, &equal("n", 1)).unwrap();
    assert!(estimate.rows > 0.0, "{estimate:?}");
    write.commit().unwrap();
}

/// R13 (ST7, ST12): analyze on the store of a table with a damaged page fails with damage and publishes nothing.
#[test]
fn r13_analyze_of_a_damaged_table_fails_with_damage() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&pair_schema()))
        .unwrap();
    for id in 0..10 {
        write
            .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(id))])
            .unwrap();
    }
    write.commit().unwrap();
    store.close().unwrap();
    damage(tmp.path(), root_page(tmp.path(), "t"));
    let store = open(tmp.path());
    assert_eq!(
        store.analyze("t", None).unwrap_err().kind(),
        ErrorKind::Damaged
    );
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    assert!(read.column_statistics(&table, "n").unwrap().is_none());
}

/// R13 (ST7, ST9): analyze on the store whose save cannot open its snapshot (the second one, after the sample's)
/// reports out of memory, keeps its statistics in memory, other writes go on, and a later estimate saves them without
/// sampling again; they last after reopening.
#[test]
fn r13_analyze_out_of_memory_after_sampling_keeps_and_later_saves() {
    use krummi_pager::testing::{self, Action, Op};
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&pair_schema()))
        .unwrap();
    for id in 0..100 {
        write
            .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(id % 2))])
            .unwrap();
    }
    write.commit().unwrap();
    testing::reset();
    testing::inject(
        Op::Snapshot,
        1,
        Action::Fail(std::io::ErrorKind::OutOfMemory),
    );
    let error = store.analyze("t", None).unwrap_err();
    testing::reset();
    assert_eq!(error.kind(), ErrorKind::OutOfMemory);
    let mut write = store.write().unwrap();
    write.open_tree_with("after", OpenMode::CreateNew).unwrap();
    write.commit().unwrap();
    let samples = store.statistics_samples();
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    read.estimate(&table, &equal("n", 1)).unwrap();
    drop(read);
    assert_eq!(store.statistics_samples(), samples, "memory served");
    store.wait_statistics_idle();
    store.close().unwrap();
    let store = open(tmp.path());
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    assert!(
        read.column_statistics(&table, "n").unwrap().is_some(),
        "saved"
    );
}

/// R13 (ST7, T11): analyze in a write over a damaged table fails with damage, the write cannot commit, and the next
/// write works.
#[test]
fn r13_analyze_in_a_write_of_a_damaged_table_fails_the_write() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&pair_schema()))
        .unwrap();
    for id in 0..10 {
        write
            .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(id))])
            .unwrap();
    }
    write.commit().unwrap();
    store.close().unwrap();
    damage(tmp.path(), root_page(tmp.path(), "t"));
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    write
        .open_tree_with("previous", OpenMode::CreateNew)
        .unwrap();
    let table = write.open_table("t").unwrap();
    assert_eq!(
        write.analyze(&table, None).unwrap_err().kind(),
        ErrorKind::Damaged
    );
    assert_eq!(write.commit().unwrap_err().kind(), ErrorKind::Damaged);
    let mut write = store.write().unwrap();
    write.open_tree_with("next", OpenMode::CreateNew).unwrap();
    write.commit().unwrap();
}

/// Waits until a sample started after the store had read `samples`: its transaction already sees the table, so a change
/// committed now is one that sample does not see (instead of a fixed sleep; RDD review of the fix commits).
/// Analyze counts its sample after `read` and `open_table` (`Store::analyze_with_timeout`); a sample that never starts
/// fails the test after a minute instead of hanging it.
fn wait_sampling(store: &Store, samples: u64) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while store.statistics_samples() == samples {
        assert!(
            std::time::Instant::now() < deadline,
            "analyze never started its sample"
        );
        std::thread::yield_now();
    }
}
