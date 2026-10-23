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

use common::{disk, next_writer, put, queue_on_thread, read_text, text, wait_until};
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

fn commit_on(engine: &Engine, page: u64, value: &str) -> krummi_tx::Result<()> {
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, page, value);
    tx.commit()
}

/// AC-E1 (E2, E3): a durable commit held after its point of no return has released the turn; the next writer sees
/// its changes, a read does not, and once it returns a read on its thread does.
#[test]
fn ac_e1_the_turn_is_released_at_the_point_of_no_return() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let pages = reserve(&engine, 1, "a");
    engine.set_journal_paused(true);
    std::thread::scope(|s| {
        let engine = &engine;
        let pages = &pages;
        let held = queue_on_thread(s, engine, 1, move || {
            commit_on(engine, pages[0], "b").unwrap();
            read_text(engine, pages[0])
        });
        let next = next_writer(engine);
        assert_eq!(
            text(next.page(pages[0]).unwrap()),
            "b",
            "the next writer sees it"
        );
        assert_eq!(read_text(engine, pages[0]), "a", "a read does not");
        next.abort();
        engine.set_journal_paused(false);
        assert_eq!(
            held.join().unwrap(),
            "b",
            "after it returns, its thread sees it"
        );
    });
}

/// AC-E2 (E4, G4), first part: durable commits from 8 threads held in the queue are written in one write and
/// become visible together.
#[test]
fn ac_e2_commits_waiting_together_share_one_write() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let pages = reserve(&engine, 8, "-");
    engine.set_journal_paused(true);
    let writes = engine.journal_writes();
    std::thread::scope(|s| {
        let engine = &engine;
        let mut threads = Vec::new();
        for (i, &page) in pages.iter().enumerate() {
            threads.push(queue_on_thread(s, engine, i + 1, move || {
                commit_on(engine, page, &format!("t{i}"))
            }));
        }
        let read = engine.read().unwrap();
        assert!(pages.iter().all(|&p| text(read.page(p).unwrap()) == "-"));
        engine.set_journal_paused(false);
        for thread in threads {
            thread.join().unwrap().unwrap();
        }
    });
    assert_eq!(engine.journal_writes(), writes + 1, "one write for the 8");
    let read = engine.read().unwrap();
    for (i, &page) in pages.iter().enumerate() {
        assert_eq!(text(read.page(page).unwrap()), format!("t{i}"));
    }
}

/// AC-E5 (E8, T12): when the write of a group fails, its durable commits fail with the I/O class, so do the open
/// writer's commit and every request, the one already waiting included, and reads see none of them.
#[test]
fn ac_e5_a_failed_group_fails_its_commits_and_the_store_refuses_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let pages = reserve(&engine, 5, "-");
    engine.set_journal_paused(true);
    commit_deferred(&engine, pages[0], "deferred");
    std::thread::scope(|s| {
        let engine = &engine;
        let pages = &pages;
        let held: Vec<_> = (1..=3)
            .map(|i| {
                queue_on_thread(s, engine, i + 1, move || {
                    commit_on(engine, pages[i], "held")
                })
            })
            .collect();
        let mut open = engine.write(None).unwrap();
        assert_eq!(text(open.page(pages[1]).unwrap()), "held", "it sees them");
        put(&mut open, pages[4], "open");
        let waiting = s.spawn(move || engine.write(None).err().map(|e| e.kind()));
        wait_until(|| engine.queued() == 1);
        testing::reset();
        testing::inject(Op::Write, 0, Action::Fail(io::ErrorKind::Other));
        engine.set_journal_paused(false);
        for commit in held {
            assert_eq!(commit.join().unwrap().unwrap_err().kind(), ErrorKind::Io);
        }
        assert_eq!(
            waiting.join().unwrap(),
            Some(ErrorKind::Io),
            "the waiting request fails while the writer is open"
        );
        assert_eq!(open.commit().unwrap_err().kind(), ErrorKind::Io);
    });
    testing::reset();
    assert_eq!(engine.write(None).err().unwrap().kind(), ErrorKind::Io);
    let read = engine.read().unwrap();
    assert!(
        pages.iter().all(|&p| text(read.page(p).unwrap()) == "-"),
        "reads see none"
    );
    drop(read);
    drop(engine);
    let engine = disk(tmp.path());
    let present: Vec<bool> = pages[..4]
        .iter()
        .map(|&p| read_text(&engine, p) != "-")
        .collect();
    let prefix = present.iter().take_while(|&&p| p).count();
    assert!(present[prefix..].iter().all(|&p| !p), "{present:?}");
    assert_eq!(read_text(&engine, pages[4]), "-");
}

/// AC-E6 (E2): a panic after the point of no return does not undo the commit.
#[test]
fn ac_e6_a_panic_after_the_point_of_no_return_keeps_the_commit() {
    use krummi_tx::Point;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::Arc;
    for target in [Point::CommitQueued, Point::CommitWait] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = disk(tmp.path());
        let pages = reserve(&engine, 1, "a");
        let txid = engine.journal_durable() + 1;
        engine.set_hook(Some(Arc::new(move |point| {
            if point == target {
                panic!("interrupted at {point:?}");
            }
        })));
        let committed = catch_unwind(AssertUnwindSafe(|| commit_on(&engine, pages[0], "b")));
        engine.set_hook(None);
        assert!(committed.is_err(), "{target:?}");
        wait_until(|| engine.journal_durable() >= txid);
        assert_eq!(read_text(&engine, pages[0]), "b", "{target:?}");
        engine.close().unwrap();
        assert_eq!(read_text(&disk(tmp.path()), pages[0]), "b", "{target:?}");
    }
}

/// AC-E7 (E3, E5): a deferred commit behind a held durable one returns at once; the next writer sees both, a read
/// on its thread neither, until their group is durable.
#[test]
fn ac_e7_a_deferred_commit_returns_without_waiting() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let pages = reserve(&engine, 2, "-");
    engine.set_journal_paused(true);
    std::thread::scope(|s| {
        let engine = &engine;
        let pages = &pages;
        let held = queue_on_thread(s, engine, 1, move || commit_on(engine, pages[0], "durable"));
        commit_deferred(engine, pages[1], "deferred");
        let next = next_writer(engine);
        assert_eq!(text(next.page(pages[0]).unwrap()), "durable");
        assert_eq!(text(next.page(pages[1]).unwrap()), "deferred");
        next.abort();
        let read = engine.read().unwrap();
        assert_eq!(text(read.page(pages[0]).unwrap()), "-");
        assert_eq!(text(read.page(pages[1]).unwrap()), "-");
        drop(read);
        engine.set_journal_paused(false);
        held.join().unwrap().unwrap();
    });
    wait_until(|| engine.journal_queued() == 0 && read_text(&engine, pages[1]) == "deferred");
    assert_eq!(read_text(&engine, pages[0]), "durable");
}

/// AC-E11 (E11): a durable commit without changes waits for the commits its transaction saw, and fails if their
/// group fails; a deferred one, or one with nothing pending, ends at once.
#[test]
fn ac_e11_a_commit_without_changes_waits_for_what_it_saw() {
    for fails in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = disk(tmp.path());
        let pages = reserve(&engine, 1, "-");
        engine.set_journal_paused(true);
        std::thread::scope(|s| {
            let engine = &engine;
            let pages = &pages;
            let held = queue_on_thread(s, engine, 1, move || commit_on(engine, pages[0], "held"));
            // Aborting, and a deferred commit without changes, end at once with commits pending.
            next_writer(engine).abort();
            next_writer(engine).commit_deferred().unwrap();
            let (done, finished) = mpsc::channel();
            let empty = s.spawn(move || {
                let tx = next_writer(engine);
                let result = tx.commit();
                done.send(()).unwrap();
                result
            });
            assert!(
                finished.recv_timeout(Duration::from_millis(200)).is_err(),
                "it waits"
            );
            if fails {
                testing::reset();
                testing::inject(Op::Write, 0, Action::Fail(io::ErrorKind::Other));
            }
            engine.set_journal_paused(false);
            let result = empty.join().unwrap();
            let held = held.join().unwrap();
            if fails {
                assert_eq!(result.unwrap_err().kind(), ErrorKind::Io);
                assert_eq!(held.unwrap_err().kind(), ErrorKind::Io);
            } else {
                result.unwrap();
                held.unwrap();
            }
        });
        testing::reset();
        if !fails {
            engine
                .write(Some(Duration::ZERO))
                .unwrap()
                .commit()
                .unwrap();
        }
    }
}

/// AC-E18 (E14): in a temporary store a deferred commit is visible as soon as it returns.
#[test]
fn ac_e18_a_temporary_store_shows_a_deferred_commit_at_once() {
    let engine = common::temporary();
    let pages = reserve(&engine, 1, "-");
    commit_deferred(&engine, pages[0], "deferred");
    assert_eq!(read_text(&engine, pages[0]), "deferred");
}
