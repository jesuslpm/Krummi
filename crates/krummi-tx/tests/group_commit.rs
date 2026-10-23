//! Grouped journal writes (grouped journal PRD: E2, E10, E12, E13; AC-E10, AC-E12, AC-E13, AC-E17).
//!
//! Commits are held between their point of no return and their durability by pausing the journal's writer, which
//! shares the fault state of the thread that opened the store.

mod common;

use std::fs;
use std::io;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use common::{disk, put, read_text, text, wait_until};
use krummi_pager::DATA_FILE;
use krummi_pager::testing::{self, Action, Op};
use krummi_tx::{Engine, ErrorKind};

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn snapshot(folder: &Path) -> Vec<(String, Vec<u8>)> {
    names(folder)
        .into_iter()
        .map(|n| {
            let bytes = fs::read(folder.join(&n)).unwrap();
            (n, bytes)
        })
        .collect()
}

/// Reserves `count` pages holding `value` in one durable commit.
fn reserve(engine: &Engine, count: usize, value: &str) -> Vec<u64> {
    let mut tx = engine.write(None).unwrap();
    let pages: Vec<u64> = (0..count).map(|_| tx.allocate().unwrap()).collect();
    for &page in &pages {
        put(&mut tx, page, value);
    }
    tx.commit().unwrap();
    pages
}

fn commit_deferred(engine: &Engine, page: u64, value: &str) {
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, page, value);
    tx.commit_deferred().unwrap();
}

/// AC-E10 (E12): the close, and dropping the store without closing it, write the commits waiting in the journal's
/// queue; after a failed group the close retries it, and with the failure still there the close fails.
#[test]
fn ac_e10_the_close_writes_what_is_queued() {
    for dropped in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = disk(tmp.path());
        let pages = reserve(&engine, 3, "-");
        engine.set_journal_paused(true);
        for (i, &page) in pages.iter().enumerate() {
            commit_deferred(&engine, page, &format!("d{i}"));
        }
        assert_eq!(engine.journal_queued(), 3);
        if dropped {
            drop(engine);
        } else {
            engine.close().unwrap();
        }
        assert_eq!(names(tmp.path()), vec![DATA_FILE], "dropped {dropped}");
        let engine = disk(tmp.path());
        for (i, &page) in pages.iter().enumerate() {
            assert_eq!(
                read_text(&engine, page),
                format!("d{i}"),
                "dropped {dropped}"
            );
        }
    }

    for persists in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = disk(tmp.path());
        let pages = reserve(&engine, 2, "-");
        engine.set_journal_paused(true);
        for (i, &page) in pages.iter().enumerate() {
            commit_deferred(&engine, page, &format!("d{i}"));
        }
        testing::reset();
        let failures = if persists { 100 } else { 1 };
        for nth in 0..failures {
            testing::inject(Op::Write, nth, Action::Fail(io::ErrorKind::Other));
        }
        engine.set_journal_paused(false);
        wait_until(|| {
            engine
                .write(Some(Duration::ZERO))
                .is_err_and(|e| e.kind() == ErrorKind::Io)
        });
        if persists {
            assert_eq!(engine.close().unwrap_err().kind(), ErrorKind::Io);
            testing::reset();
            let engine = disk(tmp.path());
            let present: Vec<bool> = pages
                .iter()
                .enumerate()
                .map(|(i, &p)| read_text(&engine, p) == format!("d{i}"))
                .collect();
            let prefix = present.iter().take_while(|&&p| p).count();
            assert!(present[prefix..].iter().all(|&p| !p), "{present:?}");
        } else {
            engine.close().unwrap();
            testing::reset();
            let engine = disk(tmp.path());
            for (i, &page) in pages.iter().enumerate() {
                assert_eq!(read_text(&engine, page), format!("d{i}"));
            }
        }
    }
}

/// AC-E12 (E10): making what is committed durable waits for a commit held in the journal's queue that has not
/// returned yet.
#[test]
fn ac_e12_making_durable_waits_for_queued_commits() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let pages = reserve(&engine, 1, "-");
    engine.set_journal_paused(true);
    std::thread::scope(|s| {
        let engine = &engine;
        let pages = &pages;
        let committer = s.spawn(move || {
            let mut tx = engine.write(None).unwrap();
            put(&mut tx, pages[0], "held");
            tx.commit().unwrap();
        });
        wait_until(|| engine.journal_queued() == 1);
        let (done, finished) = mpsc::channel();
        let durable = s.spawn(move || {
            engine.make_durable().unwrap();
            done.send(engine.journal_durable()).unwrap();
        });
        assert!(
            finished.recv_timeout(Duration::from_millis(200)).is_err(),
            "it waits"
        );
        engine.set_journal_paused(false);
        let durable_upto = finished.recv().unwrap();
        durable.join().unwrap();
        committer.join().unwrap();
        assert!(durable_upto >= 2, "the held commit is durable");
    });
    assert_eq!(read_text(&engine, pages[0]), "held");
}

/// AC-E13 (E13): failing to start the journal's writer fails the opening with the out-of-memory class without
/// touching the store; the writer stopping with commits waiting fails them with the I/O class.
#[test]
fn ac_e13_failures_of_the_journals_writer() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let pages = reserve(&engine, 2, "a");
    engine.close().unwrap();
    let before = snapshot(tmp.path());
    testing::reset();
    testing::inject(Op::Thread, 0, Action::Fail(io::ErrorKind::OutOfMemory));
    let error = Engine::open(tmp.path(), &common::options()).err().unwrap();
    testing::reset();
    assert_eq!(error.kind(), ErrorKind::OutOfMemory);
    assert_eq!(snapshot(tmp.path()), before);

    let engine = disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, pages[0], "returned");
    tx.commit().unwrap();
    engine.set_journal_paused(true);
    testing::reset();
    testing::inject(Op::Write, 0, Action::Panic);
    std::thread::scope(|s| {
        let engine = &engine;
        let pages = &pages;
        let committer = s.spawn(move || {
            let mut tx = engine.write(None).unwrap();
            put(&mut tx, pages[1], "lost");
            tx.commit()
        });
        wait_until(|| engine.journal_queued() == 1);
        engine.set_journal_paused(false);
        assert_eq!(committer.join().unwrap().unwrap_err().kind(), ErrorKind::Io);
    });
    testing::reset();
    assert_eq!(engine.write(None).err().unwrap().kind(), ErrorKind::Io);
    drop(engine);
    let engine = disk(tmp.path());
    assert_eq!(read_text(&engine, pages[0]), "returned");
}

/// AC-E17 (E2): a commit that waits for room in a full journal queue keeps the write turn; when the store stops
/// accepting writes meanwhile, it fails with the I/O class and applies nothing, and the queued commits are still
/// written and seen.
#[test]
fn ac_e17_a_commit_waiting_for_room_in_the_queue() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    // One record per journal file, so the transfer records itself in the header and flushes `krummi.data`.
    engine.set_journal_file_limit(4096);
    let pages = reserve(&engine, 3, "-");
    let hold = engine.read().unwrap();
    {
        let mut tx = engine.write(None).unwrap();
        put(&mut tx, pages[0], "held back");
        tx.commit().unwrap();
    }
    engine.set_journal_queue_limit(1);
    engine.set_journal_paused(true);
    commit_deferred(&engine, pages[1], "queued");
    std::thread::scope(|s| {
        let engine = &engine;
        let pages = &pages;
        let waiting = s.spawn(move || {
            let mut tx = engine.write(None).unwrap();
            put(&mut tx, pages[2], "never");
            tx.commit_deferred()
        });
        wait_until(|| {
            engine
                .write(Some(Duration::ZERO))
                .is_err_and(|e| e.kind() == ErrorKind::Timeout)
        });
        std::thread::sleep(Duration::from_millis(50));
        assert!(!waiting.is_finished(), "it waits for room");
        assert_eq!(
            engine.write(Some(Duration::ZERO)).err().unwrap().kind(),
            ErrorKind::Timeout,
            "it keeps the turn"
        );
        drop(hold);
        testing::reset();
        for nth in 0..8 {
            testing::inject(Op::Sync, nth, Action::Fail(io::ErrorKind::Other));
        }
        assert!(!engine.transfer_now(), "the transfer failed");
        testing::reset();
        assert_eq!(waiting.join().unwrap().unwrap_err().kind(), ErrorKind::Io);
    });
    engine.set_journal_paused(false);
    wait_until(|| engine.journal_queued() == 0);
    wait_until(|| text(engine.read().unwrap().page(pages[1]).unwrap()) == "queued");
    assert_eq!(read_text(&engine, pages[2]), "-");
}
