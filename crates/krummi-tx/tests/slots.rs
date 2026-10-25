//! The meta page's slots for the layers above (D2 of the trees and key operations execution): `u64` values read
//! in any transaction and set in write transactions, which travel with the commit like the frontier.

mod common;

use common::{POWER_CUTS, Run, both, disk};
use krummi_pager::DATA_FILE;
use krummi_pager::testing::{self, Model};
use krummi_tx::{Engine, META_SLOTS};

fn slots_of_read(engine: &Engine) -> Vec<u64> {
    let read = engine.read().unwrap();
    (0..META_SLOTS).map(|s| read.slot(s).unwrap()).collect()
}

#[test]
fn slots_start_at_zero_and_a_commit_makes_them_visible_to_new_reads() {
    both(|engine| {
        assert_eq!(slots_of_read(engine), [0; META_SLOTS]);
        let before = engine.read().unwrap();
        let mut tx = engine.write(None).unwrap();
        assert_eq!(tx.slot(1).unwrap(), 0, "a slot never set");
        tx.set_slot(1, 42).unwrap();
        tx.set_slot(META_SLOTS - 1, u64::MAX).unwrap();
        assert_eq!(tx.slot(1).unwrap(), 42, "the write sees its own change");
        tx.commit().unwrap();
        let mut expected = [0; META_SLOTS];
        expected[1] = 42;
        expected[META_SLOTS - 1] = u64::MAX;
        assert_eq!(slots_of_read(engine), expected);
        assert_eq!(
            before.slot(1).unwrap(),
            0,
            "an older read keeps its snapshot"
        );
        drop(before);

        let older = engine.read().unwrap();
        let mut tx = engine.write(None).unwrap();
        assert_eq!(tx.slot(1).unwrap(), 42, "the next write sees the commit");
        tx.set_slot(1, 7).unwrap();
        let page = tx.allocate().unwrap();
        tx.commit().unwrap();
        assert_eq!(
            older.slot(1).unwrap(),
            42,
            "an older read keeps its snapshot"
        );
        assert_eq!(engine.read().unwrap().slot(1).unwrap(), 7);
        assert_ne!(engine.read().unwrap().page(page).unwrap().len(), 0);
    });
}

#[test]
fn an_aborted_write_loses_its_slots() {
    both(|engine| {
        let mut tx = engine.write(None).unwrap();
        tx.set_slot(0, 5).unwrap();
        tx.commit().unwrap();
        let mut tx = engine.write(None).unwrap();
        tx.set_slot(0, 6).unwrap();
        tx.set_slot(2, 9).unwrap();
        tx.abort();
        let mut tx = engine.write(None).unwrap();
        tx.set_slot(3, 1).unwrap();
        drop(tx);
        assert_eq!(slots_of_read(engine), [5, 0, 0, 0]);
        assert_eq!(engine.write(None).unwrap().slot(0).unwrap(), 5);
    });
}

#[test]
fn setting_a_slot_to_its_value_is_not_a_change() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    tx.set_slot(0, 0).unwrap();
    tx.commit().unwrap();
    assert_eq!(engine.journal_writes(), 0, "nothing was written");
    let mut tx = engine.write(None).unwrap();
    tx.set_slot(0, 3).unwrap();
    tx.commit().unwrap();
    let writes = engine.journal_writes();
    let mut tx = engine.write(None).unwrap();
    tx.set_slot(0, 3).unwrap();
    tx.commit().unwrap();
    assert_eq!(engine.journal_writes(), writes);
}

#[test]
fn slots_survive_reopening() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    tx.set_slot(0, 11).unwrap();
    tx.set_slot(1, 12).unwrap();
    tx.commit().unwrap();
    engine.close().unwrap();
    let engine = disk(tmp.path());
    assert_eq!(slots_of_read(&engine), [11, 12, 0, 0]);
    assert_eq!(engine.write(None).unwrap().slot(1).unwrap(), 12);
}

/// After a crash or a power cut at every point of a run of durable commits that set the slots, every commit that
/// returned is there and the one in progress is entire or absent.
#[test]
fn slots_of_durable_commits_survive_any_crash() {
    let (source, engine) = Run::start();
    let mut returned = Vec::new();
    for i in 1..=3u64 {
        let mut tx = engine.write(None).unwrap();
        tx.set_slot(0, i).unwrap();
        tx.set_slot(1, 100 + i).unwrap();
        tx.commit().unwrap();
        returned.push(testing::recorded_len());
    }
    let run = Run::finish(source, returned);
    drop(engine);
    for cut in 0..=run.ops.len() {
        for model in POWER_CUTS.into_iter().chain([Model::Written]) {
            let folder = run.crash(cut, model);
            if !folder.path().join(DATA_FILE).exists() {
                continue;
            }
            let engine = disk(folder.path());
            let seen = slots_of_read(&engine);
            let done = run.returned_at(cut) as u64;
            let allowed = [done, done + 1].map(|n| {
                if n == 0 {
                    [0; META_SLOTS]
                } else {
                    [n, 100 + n, 0, 0]
                }
            });
            assert!(
                allowed.iter().any(|a| a[..] == seen[..]) && seen[0] <= 3,
                "cut {cut} {model:?}: {seen:?}, {done} returned"
            );
        }
    }
}
