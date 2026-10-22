//! What transactions see, commit and leave behind (Transactions PRD: T2, T7, T9–T12, T14–T17; AC6, AC10, AC11,
//! AC12, AC14, AC20, AC-T1–AC-T5, AC-T12, AC-T13, AC-T15–AC-T17, AC-T19–AC-T21).

mod common;

use std::fs;
use std::io;
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;

use common::*;
use muninndb_pager::DATA_FILE;
use muninndb_pager::format::PAGE_SIZE;
use muninndb_pager::testing::{self, Action, Op};
use muninndb_tx::{Engine, ErrorKind, FIRST_PAGE, OpenOptions, Point, WriteTransaction};

fn with_max(max_size: u64) -> OpenOptions {
    OpenOptions {
        max_size: Some(max_size),
    }
}

/// Fails the next `count` calls of `op` on this thread.
fn fail_many(op: Op, count: usize, kind: io::ErrorKind) {
    testing::reset();
    for nth in 0..count {
        testing::inject(op, nth, Action::Fail(kind));
    }
}

#[test]
fn ac6_a_read_keeps_its_snapshot_while_a_write_commits_without_waiting_for_it() {
    both(|engine| {
        let page = new_page(engine, "a");
        let read = engine.read().unwrap();
        set(engine, page, "b");
        assert_eq!(text(read.page(page).unwrap()), "a");
        assert_eq!(read_text(engine, page), "b");
    });
}

#[test]
fn ac_t1_committed_changes_survive_close_and_drop() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "a");
    set(&engine, page, "b");
    engine.close().unwrap();
    let engine = disk(tmp.path());
    assert_eq!(read_text(&engine, page), "b");
    set(&engine, page, "c");
    drop(engine);
    assert_eq!(read_text(&disk(tmp.path()), page), "c");
    let engine = temporary();
    let page = new_page(&engine, "a");
    assert_eq!(read_text(&engine, page), "a");
}

#[test]
fn ac_t2_abort_drop_and_panic_apply_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let on_disk = disk(tmp.path());
    let page = new_page(&on_disk, "a");
    for engine in [&on_disk, &temporary()] {
        let page = if std::ptr::eq(engine, &on_disk) {
            page
        } else {
            new_page(engine, "a")
        };
        for how in ["abort", "drop", "panic"] {
            let size = engine.info().size_bytes;
            match how {
                "abort" => change(engine, page).abort(),
                "drop" => drop(change(engine, page)),
                _ => thread::scope(|s| {
                    let result = s.spawn(|| {
                        let _tx = change(engine, page);
                        panic!("the thread with the write panics");
                    });
                    assert!(result.join().is_err());
                }),
            }
            assert_eq!(read_text(engine, page), "a", "{how}");
            let grown = engine.info().size_bytes;
            assert!(grown >= size, "{how}: the size never goes down");
            if how == "abort" {
                assert!(grown > size, "the aborted write made the store grow");
            }
            engine
                .write(Some(Duration::ZERO))
                .unwrap()
                .commit()
                .unwrap();
        }
    }
    on_disk.close().unwrap();
    assert_eq!(read_text(&disk(tmp.path()), page), "a");
}

/// A write that changes `page` to "b" and makes the store grow.
fn change(engine: &Engine, page: u64) -> WriteTransaction<'_> {
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, page, "b");
    for _ in 0..20 {
        tx.allocate().unwrap();
    }
    tx
}

#[test]
fn ac_t3_a_read_never_sees_an_open_write() {
    both(|engine| {
        let page = new_page(engine, "a");
        let mut tx = engine.write(None).unwrap();
        put(&mut tx, page, "b");
        assert_eq!(read_text(engine, page), "a", "same thread");
        thread::scope(|s| {
            let other = s.spawn(|| read_text(engine, page)).join().unwrap();
            assert_eq!(other, "a", "another thread");
        });
        assert_eq!(
            text(tx.page(page).unwrap()),
            "b",
            "the write sees its own change"
        );
        tx.commit().unwrap();
        assert_eq!(read_text(engine, page), "b");
    });
}

#[test]
fn ac_t4_a_write_sees_what_was_committed_before_its_turn() {
    both(|engine| {
        let page = new_page(engine, "a");
        let (held, is_held) = mpsc::channel();
        let (release, released) = mpsc::channel::<()>();
        thread::scope(|s| {
            s.spawn(move || {
                let mut first = engine.write(None).unwrap();
                put(&mut first, page, "b");
                held.send(()).unwrap();
                released.recv().unwrap();
                first.commit().unwrap();
            });
            is_held.recv().unwrap();
            let second = s.spawn(move || {
                let read = engine.read().unwrap();
                let tx = engine.write(None).unwrap();
                (text(tx.page(page).unwrap()), text(read.page(page).unwrap()))
            });
            wait_until(|| engine.queued() == 1);
            release.send(()).unwrap();
            assert_eq!(second.join().unwrap(), ("b".into(), "a".into()));
        });
    });
}

#[test]
fn ac_t5_a_read_during_a_commit_sees_all_of_it_or_nothing() {
    both(|engine| {
        let (first, second) = (new_page(engine, "a"), new_page(engine, "a"));
        let (at, reached) = mpsc::channel::<Point>();
        let (resume, resumed) = mpsc::channel::<()>();
        let (at, resumed) = (Mutex::new(at), Mutex::new(resumed));
        engine.set_hook(Some(Arc::new(move |point| {
            if point != Point::Handoff {
                at.lock().unwrap().send(point).unwrap();
                resumed.lock().unwrap().recv().unwrap();
            }
        })));
        thread::scope(|s| {
            s.spawn(move || {
                let mut tx = engine.write(None).unwrap();
                put(&mut tx, first, "b");
                put(&mut tx, second, "b");
                tx.commit().unwrap();
            });
            for expected in [Point::CommitFlush, Point::CommitSeal, Point::CommitPublish] {
                assert_eq!(reached.recv().unwrap(), expected);
                let read = engine.read().unwrap();
                let seen = (
                    text(read.page(first).unwrap()),
                    text(read.page(second).unwrap()),
                );
                assert_eq!(seen, ("a".into(), "a".into()), "at {expected:?}");
                resume.send(()).unwrap();
            }
        });
        engine.set_hook(None);
        assert_eq!(
            (read_text(engine, first), read_text(engine, second)),
            ("b".into(), "b".into())
        );
    });
}

#[test]
fn ac_t19_a_panic_anywhere_in_a_commit_applies_nothing() {
    both(|engine| {
        let page = new_page(engine, "a");
        for target in [Point::CommitFlush, Point::CommitSeal, Point::CommitPublish] {
            engine.set_hook(Some(Arc::new(move |point| {
                if point == target {
                    panic!("interrupted at {point:?}");
                }
            })));
            thread::scope(|s| {
                let commit = s.spawn(|| {
                    let mut tx = engine.write(None).unwrap();
                    put(&mut tx, page, "b");
                    tx.commit()
                });
                assert!(commit.join().is_err());
            });
            engine.set_hook(None);
            assert_eq!(read_text(engine, page), "a", "{target:?}");
            let mut tx = engine.write(Some(Duration::ZERO)).unwrap();
            put(&mut tx, page, "c");
            tx.commit().unwrap();
            set(engine, page, "a");
        }
    });
}

#[test]
fn ac_t12_ten_thousand_reads_at_once_and_out_of_memory() {
    both(|engine| {
        let page = new_page(engine, "a");
        let all_open = Barrier::new(8);
        thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    let reads: Vec<_> = (0..1250).map(|_| engine.read().unwrap()).collect();
                    for read in &reads {
                        assert_eq!(text(read.page(page).unwrap()), "a");
                    }
                    all_open.wait();
                });
            }
        });
        testing::reset();
        testing::inject(Op::Snapshot, 0, Action::Fail(io::ErrorKind::OutOfMemory));
        assert_eq!(engine.read().err().unwrap().kind(), ErrorKind::OutOfMemory);
        testing::reset();
        assert_eq!(read_text(engine, page), "a");
    });
}

#[test]
fn ac_t13_a_failed_operation_makes_the_commit_fail_with_its_class() {
    // Maximum size.
    let tmp = tempfile::tempdir().unwrap();
    let on_disk = Engine::open(tmp.path(), &with_max(1 << 20)).unwrap();
    for engine in [
        &on_disk,
        &Engine::open_temporary(&with_max(1 << 20)).unwrap(),
    ] {
        let page = new_page(engine, "a");
        let mut tx = engine.write(None).unwrap();
        let error = loop {
            if let Err(error) = tx.allocate() {
                break error;
            }
        };
        assert_eq!(error.kind(), ErrorKind::MaxSize, "{error}");
        assert_eq!(
            text(tx.page(page).unwrap()),
            "a",
            "a later operation can be tried"
        );
        assert_eq!(tx.commit().unwrap_err().kind(), ErrorKind::MaxSize);
        assert_eq!(read_text(engine, page), "a");
        set(engine, page, "c");
        assert_eq!(read_text(engine, page), "c");
    }
    // Out of memory.
    both(|engine| {
        let page = new_page(engine, "a");
        let mut tx = engine.write(Some(Duration::ZERO)).unwrap();
        fail_many(Op::Alloc, 1, io::ErrorKind::OutOfMemory);
        let error = tx.page_mut(page).err().unwrap();
        testing::reset();
        assert_eq!(error.kind(), ErrorKind::OutOfMemory, "{error}");
        assert_eq!(text(tx.page(page).unwrap()), "a");
        assert_eq!(tx.commit().unwrap_err().kind(), ErrorKind::OutOfMemory);
        set(engine, page, "b");
    });
    // A damaged page.
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "a");
    let other = new_page(&engine, "a");
    engine.close().unwrap();
    damage(tmp.path(), page as usize * PAGE_SIZE + 100);
    let engine = disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    assert_eq!(tx.page(page).err().unwrap().kind(), ErrorKind::Damaged);
    put(&mut tx, other, "b");
    assert_eq!(tx.commit().unwrap_err().kind(), ErrorKind::Damaged);
    assert_eq!(read_text(&engine, other), "a");
    set(&engine, other, "c");
    assert_eq!(read_text(&engine, other), "c");
}

fn damage(folder: &Path, offset: usize) {
    let path = folder.join(DATA_FILE);
    let mut bytes = fs::read(&path).unwrap();
    bytes[offset] ^= 0xFF;
    fs::write(&path, bytes).unwrap();
}

#[test]
fn ac20_ac_t15_reads_return_the_engine_s_memory_and_keep_it() {
    both(|engine| {
        let page = new_page(engine, "a");
        let oldest = engine.read().unwrap(); // keeps the next version out of the data file
        set(engine, page, "b");
        let first = engine.read().unwrap();
        let reference = first.page(page).unwrap();
        let second = engine.read().unwrap();
        assert_eq!(
            reference.as_ptr(),
            second.page(page).unwrap().as_ptr(),
            "the same version, not a copy"
        );
        assert_eq!(reference.as_ptr(), first.page(page).unwrap().as_ptr());
        set(engine, page, "c");
        set(engine, page, "d");
        assert_eq!(text(reference), "b");
        assert_eq!(reference.as_ptr(), first.page(page).unwrap().as_ptr());
        assert_eq!(text(oldest.page(page).unwrap()), "a");
    });
}

#[test]
fn ac_t16_a_long_read_can_fill_the_maximum_and_the_space_comes_back_when_it_ends() {
    let tmp = tempfile::tempdir().unwrap();
    let max = 1 << 20;
    let on_disk = Engine::open(tmp.path(), &with_max(max)).unwrap();
    for engine in [&on_disk, &Engine::open_temporary(&with_max(max)).unwrap()] {
        // Reserve pages until the store reaches its maximum.
        loop {
            let mut tx = engine.write(None).unwrap();
            match tx.allocate() {
                Ok(page) => {
                    put(&mut tx, page, "a");
                    tx.commit().unwrap();
                }
                Err(error) => {
                    assert_eq!(error.kind(), ErrorKind::MaxSize, "{error}");
                    break;
                }
            }
        }
        let slack = max - engine.info().size_bytes;
        let count = slack / PAGE_SIZE as u64;
        assert!(count >= 1, "{slack} bytes left");
        let pages: Vec<u64> = (FIRST_PAGE..FIRST_PAGE + count).collect();
        let long_read = engine.read().unwrap();
        let mut tx = engine.write(None).unwrap();
        for &page in &pages {
            put(&mut tx, page, "b");
        }
        tx.commit().unwrap();
        // The read keeps the versions before "b" in the data file, so "b" stays in scratch: no room is left.
        let mut tx = engine.write(None).unwrap();
        let error = tx.page_mut(pages[0]).err().unwrap();
        assert_eq!(error.kind(), ErrorKind::MaxSize, "{error}");
        assert_eq!(tx.commit().unwrap_err().kind(), ErrorKind::MaxSize);
        assert_eq!(read_text(engine, pages[0]), "b");
        assert_eq!(text(long_read.page(pages[0]).unwrap()), "a");
        drop(long_read);
        let mut tx = engine.write(None).unwrap();
        for &page in &pages {
            put(&mut tx, page, "c");
        }
        tx.commit().unwrap();
        assert_eq!(read_text(engine, pages[0]), "c");
    }
}

#[test]
fn ac_t17_an_empty_commit_always_succeeds() {
    let tmp = tempfile::tempdir().unwrap();
    let on_disk = disk(tmp.path());
    for engine in [&on_disk, &temporary()] {
        let page = new_page(engine, "a");
        set(engine, page, "b"); // leaves a version a later commit would write in place
        let info = engine.info();
        fail_many(Op::Write, 100, io::ErrorKind::Other);
        engine.write(None).unwrap().commit().unwrap();
        testing::reset();
        assert_eq!(engine.info(), info);
        assert_eq!(read_text(engine, page), "b");
    }
}

#[test]
fn ac11_a_full_disk_fails_the_commit_and_the_next_write_works_without_reopening() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "a");
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, page, "b");
    fail_many(Op::Reserve, 1, io::ErrorKind::StorageFull);
    let error = loop {
        if let Err(error) = tx.allocate() {
            break error;
        }
    };
    testing::reset();
    assert_eq!(error.kind(), ErrorKind::DiskFull, "{error}");
    assert_eq!(tx.commit().unwrap_err().kind(), ErrorKind::DiskFull);
    assert_eq!(read_text(&engine, page), "a");
    let mut tx = engine.write(None).unwrap();
    for _ in 0..20 {
        tx.allocate().unwrap();
    }
    put(&mut tx, page, "c");
    tx.commit().unwrap();
    assert_eq!(read_text(&engine, page), "c");
}

#[test]
fn ac12_ac_t20_a_failed_flush_refuses_writes_and_close_retries_it() {
    for fault_persists in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = disk(tmp.path());
        let page = new_page(&engine, "a");
        set(&engine, page, "b"); // stays in scratch until the next commit writes it in place
        let mut tx = engine.write(None).unwrap();
        put(&mut tx, page, "c");
        fail_many(Op::Write, 1, io::ErrorKind::Other);
        let error = tx.commit().unwrap_err();
        testing::reset();
        assert_eq!(error.kind(), ErrorKind::Io, "{error}");
        assert_eq!(engine.write(None).err().unwrap().kind(), ErrorKind::Io);
        assert_eq!(
            read_text(&engine, page),
            "b",
            "reads see everything committed"
        );
        if fault_persists {
            fail_many(Op::Write, 100, io::ErrorKind::Other);
            assert_eq!(engine.close().unwrap_err().kind(), ErrorKind::Io);
            testing::reset();
            disk(tmp.path());
        } else {
            engine.close().unwrap();
            assert_eq!(read_text(&disk(tmp.path()), page), "b");
        }
    }
}

/// J6 replaces AC-T21: the close records in both header copies the last commit, which `muninn.data` holds.
#[test]
fn j6_the_close_records_the_last_commit_in_both_header_copies() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "a");
    for text in ["b", "c", "d"] {
        set(&engine, page, text);
    }
    engine.close().unwrap();
    let bytes = fs::read(tmp.path().join(DATA_FILE)).unwrap();
    let copies = muninndb_pager::format::header_copies(&bytes[..2 * PAGE_SIZE]);
    assert_eq!(copies.map(|c| c.unwrap().applied_txid), [4, 4]);
    let engine = disk(tmp.path());
    assert_eq!(read_text(&engine, page), "d");
    engine.close().unwrap();
}

#[test]
fn ac14_a_temporary_store_with_transactions_touches_no_file() {
    testing::reset();
    for op in [
        Op::CreateFile,
        Op::Write,
        Op::Reserve,
        Op::Sync,
        Op::Rename,
        Op::Remove,
        Op::SyncDir,
        Op::Map,
        Op::Lock,
        Op::Read,
        Op::Temp,
    ] {
        testing::inject(op, 0, Action::Fail(io::ErrorKind::Other));
    }
    let engine = temporary();
    let page = new_page(&engine, "a");
    for text in ["b", "c"] {
        set(&engine, page, text);
    }
    let mut tx = engine.write(None).unwrap();
    for _ in 0..20 {
        tx.allocate().unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(read_text(&engine, page), "c");
    engine.close().unwrap();
    testing::reset();
}

/// AC10 on a store with transactions: every altered byte gives the damage class, "not a store" (the mark) or no
/// visible difference, never different data. The suite runs a sample; the full sweep is an acceptance step.
fn ac10_sweep(step: usize) {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let engine = disk(&source);
    let mut expected = Vec::new();
    for i in 0..4 {
        let page = new_page(&engine, &format!("page {i}"));
        expected.push((page, format!("page {i}")));
    }
    set(&engine, expected[0].0, "changed");
    expected[0].1 = "changed".into();
    engine.close().unwrap();
    let pristine = fs::read(source.join(DATA_FILE)).unwrap();
    let copy = tmp.path().join("copy");
    fs::create_dir(&copy).unwrap();
    for offset in (0..pristine.len()).step_by(step) {
        let mut bytes = pristine.clone();
        bytes[offset] ^= 0xFF;
        fs::write(copy.join(DATA_FILE), &bytes).unwrap();
        let engine = match Engine::open(&copy, &options()) {
            Ok(engine) => engine,
            Err(error) => {
                assert!(
                    matches!(error.kind(), ErrorKind::Damaged | ErrorKind::NotAStore),
                    "byte {offset}: {error}"
                );
                continue;
            }
        };
        let read = engine.read().unwrap();
        for (page, text_written) in &expected {
            match read.page(*page) {
                Ok(payload) => assert_eq!(&text(payload), text_written, "byte {offset}"),
                Err(error) => {
                    assert_eq!(error.kind(), ErrorKind::Damaged, "byte {offset}: {error}")
                }
            }
        }
        drop(read);
        drop(engine);
    }
}

#[test]
fn ac10_sample_of_altered_bytes() {
    ac10_sweep(97);
}

#[test]
#[ignore = "acceptance: alters every byte of the store"]
fn ac10_every_altered_byte() {
    ac10_sweep(1);
}
