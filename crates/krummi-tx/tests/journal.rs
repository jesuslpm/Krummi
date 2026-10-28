//! Tests of the journal and recovery (Journal PRD, module 3) at the transaction level.

mod common;

use std::fs;
use std::path::Path;

use common::{disk, new_page, read_text, set};
use krummi_pager::TEMP_DIR;

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// AC-J11: the versions a long read retains live in files of `krummi.temp`, which the close removes.
#[test]
fn ac_j11_retained_versions_live_in_krummi_temp_until_close() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "v0");
    let read = engine.read().unwrap();
    for i in 1..=20 {
        set(&engine, page, &format!("v{i}"));
    }
    let temp = tmp.path().join(TEMP_DIR);
    assert!(temp.is_dir());
    assert!(!names(&temp).is_empty(), "the area has files");
    assert_eq!(common::text(read.page(page).unwrap()), "v0");
    assert_eq!(read_text(&engine, page), "v20");
    drop(read);
    engine.close().unwrap();
    assert!(!temp.exists());
    let engine = disk(tmp.path());
    assert_eq!(read_text(&engine, page), "v20");
    engine.close().unwrap();
}

/// J7: a failure of the temporary area makes the commit fail with its class and apply nothing.
#[test]
fn a_failure_of_the_temporary_area_fails_the_commit_without_applying_it() {
    use krummi_pager::testing::{Action, Op, inject, reset};
    use krummi_tx::ErrorKind;
    use std::io;
    for (kind, class) in [
        (io::ErrorKind::PermissionDenied, ErrorKind::Access),
        (io::ErrorKind::StorageFull, ErrorKind::DiskFull),
        (io::ErrorKind::OutOfMemory, ErrorKind::OutOfMemory),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = disk(tmp.path());
        reset();
        // The first operation on the area removes a leftover folder; the second creates it.
        inject(Op::Temp, 1, Action::Fail(kind));
        let mut tx = engine.write(None).unwrap();
        let page = tx.allocate().unwrap();
        common::put(&mut tx, page, "lost");
        assert_eq!(tx.commit().unwrap_err().kind(), class, "{kind:?}");
        reset();
        let page = new_page(&engine, "kept");
        assert_eq!(read_text(&engine, page), "kept");
        engine.close().unwrap();
    }
}

fn journal_files(folder: &Path) -> Vec<String> {
    names(folder)
        .into_iter()
        .filter(|n| n.starts_with(krummi_pager::JOURNAL_PREFIX))
        .collect()
}

fn journal_bytes(folder: &Path) -> u64 {
    journal_files(folder)
        .iter()
        .map(|n| fs::metadata(folder.join(n)).unwrap().len())
        .sum()
}

/// AC-J1: opening leaves only `krummi.data`; a commit adds journal files while the transfer is held back; a clean
/// close leaves only `krummi.data` again, with every change.
#[test]
fn ac_j1_the_journal_appears_with_commits_and_goes_with_a_clean_close() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("new").join("store");
    let engine = disk(&folder);
    assert_eq!(names(&folder), vec!["krummi.data"]);
    let page = new_page(&engine, "a");
    let hold = engine.read().unwrap();
    set(&engine, page, "b");
    assert!(!journal_files(&folder).is_empty());
    drop(hold);
    engine.close().unwrap();
    assert_eq!(names(&folder), vec!["krummi.data"]);
    let engine = disk(&folder);
    assert_eq!(read_text(&engine, page), "b");
    engine.close().unwrap();
}

/// AC-J8 and J17: making what is committed durable.
#[test]
fn ac_j8_making_what_is_committed_durable() {
    use krummi_pager::testing::{Action, Op, inject, reset};
    use krummi_tx::ErrorKind;
    use std::io;
    use std::sync::mpsc;

    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    engine.make_durable().unwrap();
    let page = new_page(&engine, "a");

    // It does not wait for the open write transaction.
    std::thread::scope(|s| {
        let (opened, wait_opened) = mpsc::channel();
        let (finish, wait_finish) = mpsc::channel::<()>();
        let engine = &engine;
        let holder = s.spawn(move || {
            let tx = engine.write(None).unwrap();
            opened.send(()).unwrap();
            wait_finish.recv().unwrap();
            tx.abort();
        });
        wait_opened.recv().unwrap();
        engine.make_durable().unwrap();
        finish.send(()).unwrap();
        holder.join().unwrap();
    });

    // A failed flush is an I/O failure, and the store refuses writes. The journal's writer is paused so that the
    // deferred commit is still pending when the fault is injected (otherwise it may already be durable).
    engine.set_journal_paused(true);
    let mut tx = engine.write(None).unwrap();
    common::put(&mut tx, page, "deferred");
    tx.commit_deferred().unwrap();
    reset();
    inject(Op::Sync, 0, Action::Fail(io::ErrorKind::Other));
    engine.set_journal_paused(false);
    assert_eq!(engine.make_durable().unwrap_err().kind(), ErrorKind::Io);
    reset();
    assert_eq!(engine.write(None).err().unwrap().kind(), ErrorKind::Io);
    // With nothing pending, a store that refuses writes still fails at once.
    assert_eq!(engine.make_durable().unwrap_err().kind(), ErrorKind::Io);
    drop(engine);

    // A temporary store accepts both and does nothing more.
    let engine = common::temporary();
    let page = new_page(&engine, "a");
    let mut tx = engine.write(None).unwrap();
    common::put(&mut tx, page, "b");
    tx.commit_deferred().unwrap();
    engine.make_durable().unwrap();
    assert_eq!(read_text(&engine, page), "b");
}

/// AC-J15 (first part): a commit of compressible pages takes less space in the journal than its pages.
#[test]
fn ac_j15_compressible_commits_take_less_space_in_the_journal() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let hold = engine.read().unwrap();
    let mut tx = engine.write(None).unwrap();
    for _ in 0..100 {
        let page = tx.allocate().unwrap();
        tx.page_mut(page).unwrap().fill(0x5A);
    }
    tx.commit().unwrap();
    let journal = journal_bytes(tmp.path());
    assert!(journal > 0 && journal < 100 * 8192, "{journal} bytes");
    drop(hold);
    engine.close().unwrap();
}

/// AC-J17: a transfer held back by the test blocks neither another commit nor a read.
#[test]
fn ac_j17_a_running_transfer_blocks_neither_writers_nor_readers() {
    use std::sync::{Arc, Barrier};
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "a");
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let (e, r) = (Arc::clone(&entered), Arc::clone(&release));
    let armed = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let first = Arc::clone(&armed);
    engine.set_transfer_hook(Some(Arc::new(move || {
        if first.swap(false, std::sync::atomic::Ordering::SeqCst) {
            e.wait();
            r.wait();
        }
    })));
    std::thread::scope(|s| {
        let stuck = s.spawn(|| set(&engine, page, "b"));
        entered.wait();
        set(&engine, page, "c");
        assert_eq!(read_text(&engine, page), "c");
        release.wait();
        stuck.join().unwrap();
    });
    engine.set_transfer_hook(None);
    engine.close().unwrap();
}

/// AC-J18: without reads, a maximum size with room only for one write's pages is enough for any number of
/// writes, and the journal does not grow.
#[test]
fn ac_j18_the_journal_stays_bounded_and_outside_the_maximum_size() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let pages: Vec<u64> = (0..4)
        .map(|i| new_page(&engine, &format!("p{i}")))
        .collect();
    let mut tx = engine.write(None).unwrap();
    while tx.allocate().unwrap() < 200 {}
    tx.commit().unwrap();
    let size = engine.info().size_bytes;
    assert!(size >= 1 << 20);
    engine.close().unwrap();
    let options = krummi_tx::OpenOptions {
        max_size: Some(size + 5 * 8192),
    };
    let engine = krummi_tx::Engine::open(tmp.path(), &options).unwrap();
    engine.set_journal_file_limit(64 * 1024);
    let mut measures = Vec::new();
    for round in 0..2 {
        for i in 0..1000 {
            let mut tx = engine.write(None).unwrap();
            for &page in &pages {
                common::put(&mut tx, page, &format!("{round}-{i}"));
            }
            tx.commit().unwrap();
        }
        engine.checkpoint().unwrap();
        measures.push(journal_bytes(tmp.path()));
    }
    assert!(measures[1] <= measures[0], "{measures:?}");
    assert_eq!(read_text(&engine, pages[3]), "1-999");
    engine.close().unwrap();
}

/// S1 of the journal follow-ups: closing a store nobody changed writes nothing.
#[test]
fn closing_a_store_nobody_changed_leaves_it_byte_identical() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "a");
    engine.close().unwrap();
    let data = tmp.path().join(krummi_pager::DATA_FILE);
    let before = fs::read(&data).unwrap();
    let engine = disk(tmp.path());
    assert_eq!(read_text(&engine, page), "a");
    engine.close().unwrap();
    assert_eq!(fs::read(&data).unwrap(), before);
    assert_eq!(names(tmp.path()), vec!["krummi.data"]);
}

/// S3 of the journal follow-ups (J5): deferred commits alone, without any durable one, still reach `krummi.data`
/// once enough of them are pending, so the versions waiting in `krummi.temp` stay bounded. The bound follows the
/// growth of the temporary area from which a transfer pass starts at once, lowered here from 32 MiB to 256 KiB.
/// The transfer moves only durable versions, and the ones still in the journal's queue are bounded by the queue,
/// lowered the same way from 8 MiB. The journal runs slow, as on a slow disk: with the default queue every commit
/// of the loop could wait in it, and the bound would depend on the machine.
#[test]
fn deferred_commits_alone_do_not_pile_up_in_the_temporary_area() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    engine.set_transfer_pacing(std::time::Duration::from_millis(200), 256 * 1024);
    engine.set_journal_queue_limit(256 * 1024);
    engine.set_journal_sync_hook(Some(std::sync::Arc::new(|| {
        std::thread::sleep(std::time::Duration::from_millis(50));
    })));
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    let mut most = 0;
    for _ in 0..400 {
        let mut tx = engine.write(None).unwrap();
        let page = tx.allocate().unwrap();
        for b in tx.page_mut(page).unwrap().iter_mut() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *b = x as u8;
        }
        tx.commit_deferred().unwrap();
        most = most.max(engine.scratch_bytes());
    }
    assert!(most < 2 << 20, "{most} bytes waited in the temporary area");
    engine.set_journal_sync_hook(None);
    engine.close().unwrap();
}

/// S4 of the journal follow-ups: the temporary area grows in files of at most 8 MiB.
#[test]
fn the_temporary_area_grows_in_files_of_at_most_8_mib() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let hold = engine.read().unwrap();
    for _ in 0..21 {
        let mut tx = engine.write(None).unwrap();
        for _ in 0..100 {
            tx.allocate().unwrap();
        }
        tx.commit().unwrap();
    }
    let temp = tmp.path().join(TEMP_DIR);
    let sizes: Vec<u64> = fs::read_dir(&temp)
        .unwrap()
        .map(|e| e.unwrap().metadata().unwrap().len())
        .collect();
    assert!(sizes.iter().sum::<u64>() >= 2100 * 8192, "{sizes:?}");
    assert!(sizes.iter().all(|&s| s <= 8 << 20), "{sizes:?}");
    drop(hold);
    engine.close().unwrap();
}

/// Verifier advisory on S1 (P6): a close without changes still rewrites the header when one of its copies was
/// damaged at the opening, so the other copy can be damaged later without losing the store.
#[test]
fn a_close_without_changes_still_repairs_a_damaged_header_copy() {
    use krummi_pager::format::HEADER_COPY_OFFSETS;
    for damaged in 0..2 {
        let tmp = tempfile::tempdir().unwrap();
        let engine = disk(tmp.path());
        let page = new_page(&engine, "kept");
        engine.close().unwrap();
        let data = tmp.path().join(krummi_pager::DATA_FILE);
        let flip = |copy: usize| {
            let mut bytes = fs::read(&data).unwrap();
            bytes[HEADER_COPY_OFFSETS[copy] as usize + 20] ^= 0x01;
            fs::write(&data, bytes).unwrap();
        };
        flip(damaged);
        disk(tmp.path()).close().unwrap();
        flip(1 - damaged);
        let engine = disk(tmp.path());
        assert_eq!(read_text(&engine, page), "kept", "copy {damaged}");
        engine.close().unwrap();
    }
}
