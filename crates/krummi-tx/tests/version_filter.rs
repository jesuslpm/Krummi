//! The filter of pages with versions in the scratch area (W12 of the margin diagnosis, performance plan): one writer
//! commits new versions of a set of pages and of two value runs, freed and reserved again on every commit, while
//! readers check that every snapshot sees one whole commit, also across a pause and over a long snapshot. A reader
//! that skipped the lock for a page with a version it sees would read another commit's bytes. Each mode runs for
//! `KRUMMI_FILTER_SECS` seconds (2 by default); the independent verification of the run ran it for 40.
mod common;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use common::{disk, put, temporary, text};
use krummi_tx::{Engine, PAYLOAD_SIZE, run_pages};

/// Pages every commit changes.
const HOT: usize = 40;
/// Pages no commit changes after the first.
const STATIC: usize = 100;
/// A value run of three pages.
const LEN_A: usize = 2 * PAYLOAD_SIZE + 100;
/// A value run of nine pages.
const LEN_B: usize = 8 * PAYLOAD_SIZE + 500;

/// The bytes of a value of commit `g`.
fn pattern(g: u64, seed: u64, len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| ((i as u64 * 31 + g * 7 + seed) % 251) as u8)
        .collect()
}

/// Commit `g`: the hot pages, both runs again and the commit's number in slot 0.
fn commit_gen(engine: &Engine, g: u64, hot: &[u64], first: bool) {
    let mut tx = engine.write(None).unwrap();
    for (i, &p) in hot.iter().enumerate() {
        put(&mut tx, p, &format!("{g}:{i}"));
    }
    if !first {
        for (slot, len) in [(1usize, LEN_A), (2usize, LEN_B)] {
            let old = tx.slot(slot).unwrap();
            tx.free(old, run_pages(len as u64)).unwrap();
        }
    }
    for (slot, len, seed) in [(1usize, LEN_A, 1u64), (2usize, LEN_B, 2u64)] {
        let count = run_pages(len as u64);
        let (firstp, body) = tx.allocate_value_run(count).unwrap();
        body[..len].copy_from_slice(&pattern(g, seed, len));
        tx.set_slot(slot, firstp).unwrap();
    }
    // One more than the commit, so that 0 means never set.
    tx.set_slot(0, g + 1).unwrap();
    tx.commit().unwrap();
}

/// Opens a snapshot and checks that it sees one whole commit, at least `floor` and never older than the last one
/// this reader saw, twice.
fn check(engine: &Engine, hot: &[u64], stat: &[u64], floor: u64, last: &mut u64) -> u64 {
    let read = engine.read().unwrap();
    let g1 = read.slot(0).unwrap();
    assert!(g1 >= 1);
    let g = g1 - 1;
    assert!(
        g + 1 >= floor,
        "snapshot older than committed before open: {g} < {floor}"
    );
    assert!(g >= *last, "went back");
    *last = g;
    let verify = |read: &krummi_tx::ReadTransaction<'_>| {
        for (i, &p) in hot.iter().enumerate() {
            assert_eq!(
                text(read.page(p).unwrap()),
                format!("{g}:{i}"),
                "hot page {p} g={g}"
            );
        }
        for &p in stat {
            assert_eq!(text(read.page(p).unwrap()), "static", "static page {p}");
        }
        for (slot, len, seed) in [(1usize, LEN_A, 1u64), (2usize, LEN_B, 2u64)] {
            let first = read.slot(slot).unwrap();
            let run = read.run(first, run_pages(len as u64)).unwrap();
            assert_eq!(
                &run[..len],
                &pattern(g, seed, len)[..],
                "run slot {slot} g={g}"
            );
        }
    };
    verify(&read);
    std::thread::yield_now();
    if g.is_multiple_of(3) {
        std::thread::sleep(Duration::from_micros(300));
    }
    verify(&read);
    g
}

/// Runs the writer and seven readers for `secs` seconds.
fn stress(engine: &Engine, secs: u64) {
    let mut tx = engine.write(None).unwrap();
    let hot: Vec<u64> = (0..HOT).map(|_| tx.allocate().unwrap()).collect();
    let stat: Vec<u64> = (0..STATIC).map(|_| tx.allocate().unwrap()).collect();
    for &p in &stat {
        put(&mut tx, p, "static");
    }
    for &p in &hot {
        put(&mut tx, p, "x");
    }
    tx.commit().unwrap();
    commit_gen(engine, 0, &hot, true);
    // One more than the last commit made.
    let committed = AtomicU64::new(1);
    let stop = AtomicBool::new(false);
    let reads = AtomicU64::new(0);
    std::thread::scope(|s| {
        for _ in 0..6 {
            s.spawn(|| {
                let mut last = 0;
                while !stop.load(Ordering::Relaxed) {
                    let floor = committed.load(Ordering::SeqCst);
                    check(engine, &hot, &stat, floor, &mut last);
                    reads.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
        // A long snapshot, held while many commits go by.
        s.spawn(|| {
            while !stop.load(Ordering::Relaxed) {
                let read = engine.read().unwrap();
                let g = read.slot(0).unwrap() - 1;
                std::thread::sleep(Duration::from_millis(15));
                for (i, &p) in hot.iter().enumerate() {
                    assert_eq!(text(read.page(p).unwrap()), format!("{g}:{i}"), "long hot");
                }
                let first = read.slot(2).unwrap();
                let run = read.run(first, run_pages(LEN_B as u64)).unwrap();
                assert_eq!(&run[..LEN_B], &pattern(g, 2, LEN_B)[..], "long run");
            }
        });
        let start = Instant::now();
        let mut g = 1u64;
        while start.elapsed() < Duration::from_secs(secs) {
            commit_gen(engine, g, &hot, false);
            committed.store(g + 1, Ordering::SeqCst);
            g += 1;
        }
        stop.store(true, Ordering::Relaxed);
        assert!(reads.load(Ordering::Relaxed) > 0, "{g} commits");
    });
}

/// Seconds of each mode.
fn secs() -> u64 {
    std::env::var("KRUMMI_FILTER_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2)
}

#[test]
fn snapshots_see_whole_commits_while_the_filter_changes_on_disk() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    stress(&engine, secs());
    engine.close().unwrap();
}

#[test]
fn snapshots_see_whole_commits_while_the_filter_changes_in_temporary_mode() {
    let engine = temporary();
    stress(&engine, secs());
}
