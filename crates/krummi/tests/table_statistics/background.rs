//! The background work (AC-ST2's save, AC-ST4, AC-ST5, AC-ST7 and the temporary stores of AC-ST10, through
//! `stores`): saving what an estimate sampled, renewing stale statistics, overlapping builds, the write turn and close.

use std::time::Duration;

use krummi::table::StatisticsPoint;
use krummi::table::value::Value;
use krummi::{Condition, ErrorKind, Store};

use super::build::{commit_distribution, distribution_row, read_statistics};
use super::support::*;

fn frequent() -> Condition<'static> {
    Condition::Equal {
        column: "n",
        value: Value::I64(7),
    }
}

fn estimate(store: &Store) -> krummi::Estimate {
    let read = store.read().unwrap();
    let table = read.open_table("d").unwrap();
    read.estimate(&table, &frequent()).unwrap()
}

fn insert(store: &Store, ids: std::ops::Range<i64>) {
    let mut write = store.write().unwrap();
    let table = write.open_table("d").unwrap();
    for id in ids {
        write.insert_row(&table, &distribution_row(id)).unwrap();
    }
    write.commit().unwrap();
}

/// Waits until a save of the statistics thread holds the write turn: a write with a zero limit then times out.
fn wait_for_the_save_turn(store: &Store) {
    for _ in 0..1_000 {
        match store.write_with_timeout(Duration::ZERO) {
            Ok(write) => drop(write),
            Err(error) => {
                assert_eq!(error.kind(), ErrorKind::Timeout);
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the save never got the turn");
}

/// AC-ST2 (ST3, ST6): what the first estimate sampled is saved in the background and survives reopening.
#[test]
fn ac_st2_what_an_estimate_samples_is_saved_in_the_background() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_distribution(&store);
    estimate(&store);
    store.wait_statistics_idle();
    let saved = read_statistics(&store, "n").unwrap();
    assert_eq!(saved.origin_rows, 10_000);
    store.close().unwrap();
    let store = open(tmp.path());
    let samples = store.statistics_samples();
    estimate(&store);
    assert_eq!(store.statistics_samples(), samples, "the saved ones serve");
}

/// AC-ST4 (ST2, ST5, ST6): stale statistics are used, flagged and renewed in the background, also when part of the
/// modifications came before a reopening.
#[test]
fn ac_st4_stale_statistics_are_used_and_renewed() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_distribution(&store);
    store.analyze("d", None).unwrap();
    insert(&store, 10_000..11_000);
    store.close().unwrap();
    let store = open(tmp.path());
    insert(&store, 11_000..12_600);
    let samples = store.statistics_samples();
    let stale = estimate(&store);
    assert!(
        stale.stale,
        "2,600 modifications pass the threshold of 10,000 rows"
    );
    assert_eq!(
        store.statistics_samples(),
        samples,
        "the estimate did not sample"
    );
    store.wait_statistics_idle();
    let renewed = estimate(&store);
    assert!(!renewed.stale);
    assert_eq!(read_statistics(&store, "n").unwrap().origin_rows, 12_600);
}

/// AC-ST5 (ST6, N8): a renewal held after sampling and an analyze of a later snapshot, finishing in either order,
/// leave the later statistics, and neither waits for the other.
#[test]
fn ac_st5_the_newer_build_wins_in_either_order() {
    stores(|store, _| {
        commit_distribution(store);
        store.analyze("d", None).unwrap();
        insert(store, 10_000..12_600);
        store.hold_statistics(StatisticsPoint::WorkSampled, true);
        assert!(estimate(store).stale, "asks for a renewal");
        insert(store, 12_600..12_601);
        // The analyze passes the held point on its own thread only after the hold is released, so it runs in a
        // thread of its own while this one releases the renewal first.
        std::thread::scope(|scope| {
            let analyze = scope.spawn(|| store.analyze("d", None));
            std::thread::sleep(Duration::from_millis(50));
            store.hold_statistics(StatisticsPoint::WorkSampled, false);
            analyze.join().unwrap().unwrap();
        });
        store.wait_statistics_idle();
        let kept = read_statistics(store, "n").unwrap();
        assert_eq!(kept.origin_rows, 12_601, "the later snapshot");
    });
}

/// AC-ST7 (ST6, T4): a save holding the write turn makes an application write with a limit time out; analyze on the
/// store with a zero limit while another write is open times out after sampling, its statistics left in memory.
#[test]
fn ac_st7_a_save_holds_the_turn_like_any_write() {
    stores(|store, _| {
        commit_distribution(store);
        store.hold_statistics(StatisticsPoint::SaveTurn, true);
        estimate(store);
        wait_for_the_save_turn(store);
        let limited = store
            .write_with_timeout(Duration::from_millis(30))
            .err()
            .map(|e| e.kind());
        assert_eq!(limited, Some(ErrorKind::Timeout));
        store.hold_statistics(StatisticsPoint::SaveTurn, false);
        store.wait_statistics_idle();
        assert!(read_statistics(store, "n").is_some(), "saved once released");

        let samples = store.statistics_samples();
        std::thread::scope(|scope| {
            let write = store.write().unwrap();
            let analyzed = scope
                .spawn(|| store.analyze_with_timeout("d", None, Duration::ZERO))
                .join()
                .unwrap();
            assert_eq!(analyzed.unwrap_err().kind(), ErrorKind::Timeout);
            drop(write);
        });
        assert_eq!(store.statistics_samples(), samples + 1, "it sampled first");
    });
}

/// ST10: closing with a renewal held after sampling drops it; closing with a save holding the turn waits for its
/// commit, so its statistics are there after reopening.
#[test]
fn st10_close_drops_a_sample_and_waits_for_a_save_with_the_turn() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_distribution(&store);
    store.analyze("d", None).unwrap();
    insert(&store, 10_000..12_600);
    store.hold_statistics(StatisticsPoint::WorkSampled, true);
    estimate(&store);
    store.close().unwrap();
    let store = open(tmp.path());
    assert_eq!(
        read_statistics(&store, "n").unwrap().origin_rows,
        10_000,
        "dropped"
    );
    store.hold_statistics(StatisticsPoint::SaveTurn, true);
    estimate(&store);
    wait_for_the_save_turn(&store);
    store.close().unwrap();
    let store = open(tmp.path());
    assert_eq!(
        read_statistics(&store, "n").unwrap().origin_rows,
        12_600,
        "committed"
    );
    let (alive, saves, renewals) = store.statistics_work();
    assert_eq!(
        (alive, saves, renewals),
        (false, 0, 0),
        "no thread until a request"
    );
}

/// ST1, ST16 (verifier B1): reading the statistics of a column sees those an estimate left in memory before their save
/// commits, and asks for no work.
#[test]
fn st16_reading_sees_the_statistics_in_memory() {
    stores(|store, _| {
        commit_distribution(store);
        store.hold_statistics(StatisticsPoint::SaveTurn, true);
        estimate(store);
        let seen = read_statistics(store, "n").expect("in memory");
        assert_eq!(seen.origin_rows, 10_000);
        store.hold_statistics(StatisticsPoint::SaveTurn, false);
        store.wait_statistics_idle();
        let write = store.write().unwrap();
        let table = write.open_table("d").unwrap();
        let write_seen = write.column_statistics(&table, "n").unwrap();
        drop(write);
        assert_eq!(write_seen.map(|s| s.origin_rows), Some(10_000));
    });
}
