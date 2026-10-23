//! The transfer to `krummi.data` without further commits, and the data flushes it pays for (J5, J6 of the Journal
//! PRD): odd/tasks/transfer-background-thread.md, S1–S4.

mod common;

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};

use common::{disk, new_page, read_text, set, wait_until};
use krummi_pager::JOURNAL_PREFIX;
use krummi_pager::testing::{self, Action, Op};
use krummi_tx::{Engine, ErrorKind};

fn journal_files(folder: &Path) -> usize {
    fs::read_dir(folder)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(JOURNAL_PREFIX)
        })
        .count()
}

fn commit_deferred(engine: &Engine, page: u64, text: &str) {
    let mut tx = engine.write(None).unwrap();
    common::put(&mut tx, page, text);
    tx.commit_deferred().unwrap();
}

/// A transfer hook that holds the first transfer after it is set until the test releases it.
fn hold_next_transfer(engine: &Engine) -> (Arc<Barrier>, Arc<Barrier>) {
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let (e, r) = (Arc::clone(&entered), Arc::clone(&release));
    let armed = AtomicBool::new(true);
    engine.set_transfer_hook(Some(Arc::new(move || {
        if armed.swap(false, Ordering::SeqCst) {
            e.wait();
            r.wait();
        }
    })));
    (entered, release)
}

/// S1, test 1: a read held a version back; once it ends, the version reaches `krummi.data` with no further commit.
#[test]
fn a_version_a_read_held_back_leaves_once_the_read_ends() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "a");
    wait_until(|| engine.scratch_bytes() == 0);
    let read = engine.read().unwrap();
    set(&engine, page, "b");
    assert!(
        engine.scratch_bytes() > 0,
        "the read holds the version back"
    );
    drop(read);
    wait_until(|| engine.scratch_bytes() == 0);
    assert_eq!(read_text(&engine, page), "b");
    engine.close().unwrap();
}

/// S1, test 2: the last commit was deferred and nothing follows it; once its group is durable, it reaches
/// `krummi.data`.
#[test]
fn a_last_deferred_commit_leaves_once_its_group_is_durable() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "a");
    wait_until(|| engine.scratch_bytes() == 0);
    engine.set_journal_paused(true);
    commit_deferred(&engine, page, "b");
    engine.set_journal_paused(false);
    wait_until(|| engine.scratch_bytes() == 0);
    assert_eq!(read_text(&engine, page), "b");
    engine.close().unwrap();
}

/// S1, test 3: a commit published while a running transfer had already built its list is transferred too.
#[test]
fn a_commit_published_during_a_transfer_is_transferred_too() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let first = new_page(&engine, "a");
    let second = new_page(&engine, "b");
    wait_until(|| engine.scratch_bytes() == 0);
    let (entered, release) = hold_next_transfer(&engine);
    std::thread::scope(|s| {
        let committer = s.spawn(|| set(&engine, first, "a2"));
        entered.wait();
        set(&engine, second, "b2");
        release.wait();
        committer.join().unwrap();
    });
    engine.set_transfer_hook(None);
    wait_until(|| engine.scratch_bytes() == 0);
    assert_eq!(read_text(&engine, second), "b2");
    engine.close().unwrap();
}

/// S3, test 4: while a long read holds `applied` back, commits that roll the journal over do not flush
/// `krummi.data`, since no journal file could be released; once the read ends, the files are released.
#[test]
fn data_flushes_do_not_grow_with_commits_while_no_file_can_be_released() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    engine.set_journal_file_limit(64 * 1024);
    let held = new_page(&engine, "a");
    let page = new_page(&engine, "a");
    let read = engine.read().unwrap();
    // The read needs the version of `held` in `krummi.data`, so the newer one stays in the scratch area and
    // `applied` stays before it.
    set(&engine, held, "b");
    for i in 0..50 {
        set(&engine, page, &format!("{i}"));
    }
    let after_50 = engine.data_syncs();
    for i in 50..200 {
        set(&engine, page, &format!("{i}"));
    }
    let after_200 = engine.data_syncs();
    assert!(journal_files(tmp.path()) > 2, "the journal rolled over");
    assert_eq!(after_200, after_50, "data flushes grew with commits");
    drop(read);
    wait_until(|| journal_files(tmp.path()) == 1);
    assert!(engine.data_syncs() > after_200);
    assert_eq!(read_text(&engine, page), "199");
    engine.close().unwrap();
}

/// S2, test 5: many commits per second take fewer transfer passes than commits.
#[test]
fn transfer_passes_coalesce_many_commits() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "a");
    let before = engine.transfer_passes();
    let commits = 300;
    for i in 0..commits {
        set(&engine, page, &format!("{i}"));
    }
    let passes = engine.transfer_passes() - before;
    assert!(
        passes < commits / 2,
        "{passes} passes for {commits} commits"
    );
    engine.close().unwrap();
}

/// S4, test 6: closing a store and dropping it without a close both end the transfer thread, on disk and in
/// memory, and the drop still writes what was committed.
#[test]
fn close_and_drop_end_the_transfer_thread() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "a");
    let alive = engine.transfer_thread_alive();
    assert!(alive.load(Ordering::SeqCst));
    engine.close().unwrap();
    assert!(!alive.load(Ordering::SeqCst), "the close ends the thread");
    let engine = disk(tmp.path());
    set(&engine, page, "b");
    let alive = engine.transfer_thread_alive();
    drop(engine);
    assert!(!alive.load(Ordering::SeqCst), "the drop ends the thread");
    assert_eq!(read_text(&disk(tmp.path()), page), "b");
    let engine = common::temporary();
    new_page(&engine, "a");
    let alive = engine.transfer_thread_alive();
    drop(engine);
    assert!(
        !alive.load(Ordering::SeqCst),
        "a temporary store ends it too"
    );
}

/// S4, test 6 (J13): a transfer that fails on the transfer thread makes the store refuse writes, which the next
/// operation reports; reads go on, and the close writes everything.
#[test]
fn a_transfer_failing_on_the_thread_fails_the_next_operation() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "a");
    wait_until(|| engine.scratch_bytes() == 0);
    engine.set_transfer_hook(Some(Arc::new(|| {
        // Faults are shared with the threads the store started: these fail the writes of this transfer.
        for nth in 0..64 {
            testing::inject(
                Op::Write,
                testing::count(Op::Write) + nth,
                Action::Fail(std::io::ErrorKind::Other),
            );
        }
    })));
    set(&engine, page, "b");
    wait_until(|| {
        engine
            .write(None)
            .is_err_and(|error| error.kind() == ErrorKind::Io)
    });
    engine.set_transfer_hook(None);
    testing::reset();
    assert_eq!(read_text(&engine, page), "b");
    engine.close().unwrap();
    assert_eq!(read_text(&disk(tmp.path()), page), "b");
}

/// S2: while the store is open only the transfer thread transfers: its own passes, the flush of a writer that
/// needs room under a maximum size (T14), and the transfers and checkpoints tests ask for.
#[test]
fn only_the_transfer_thread_transfers() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let pages: Vec<u64> = (0..4)
        .map(|i| new_page(&engine, &format!("p{i}")))
        .collect();
    let mut tx = engine.write(None).unwrap();
    while tx.allocate().unwrap() < 200 {}
    tx.commit().unwrap();
    let size = engine.info().size_bytes;
    engine.close().unwrap();
    let options = krummi_tx::OpenOptions {
        max_size: Some(size + 5 * 8192),
    };
    let engine = Engine::open(tmp.path(), &options).unwrap();
    let threads = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = Arc::clone(&threads);
    engine.set_transfer_hook(Some(Arc::new(move || {
        let name = std::thread::current().name().map(str::to_owned);
        seen.lock().unwrap().push(name);
    })));
    for i in 0..200 {
        let mut tx = engine.write(None).unwrap();
        for &page in &pages {
            common::put(&mut tx, page, &format!("{i}"));
        }
        tx.commit().unwrap();
    }
    assert!(engine.transfer_now());
    engine.checkpoint().unwrap();
    engine.set_transfer_hook(None);
    let threads = threads.lock().unwrap();
    assert!(threads.len() >= 3, "{} transfers", threads.len());
    assert!(
        threads
            .iter()
            .all(|name| name.as_deref() == Some("krummi-transfer")),
        "{threads:?}"
    );
    assert_eq!(read_text(&engine, pages[3]), "199");
    engine.close().unwrap();
}

/// S4 (J13, T12): a transfer that fails with a class that does not stop the store accepting writes (a full disk)
/// refuses nothing, and a later pass transfers what it left.
#[test]
fn a_transfer_failing_without_refusing_writes_is_retried() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let page = new_page(&engine, "a");
    wait_until(|| engine.scratch_bytes() == 0);
    let armed = AtomicBool::new(true);
    engine.set_transfer_hook(Some(Arc::new(move || {
        if armed.swap(false, Ordering::SeqCst) {
            testing::inject(
                Op::Write,
                testing::count(Op::Write),
                Action::Fail(std::io::ErrorKind::StorageFull),
            );
        }
    })));
    let before = engine.transfer_passes();
    set(&engine, page, "b");
    wait_until(|| engine.transfer_passes() > before);
    assert!(engine.scratch_bytes() > 0, "the full disk kept the version");
    set(&engine, page, "c");
    wait_until(|| engine.scratch_bytes() == 0);
    engine.set_transfer_hook(None);
    assert_eq!(read_text(&engine, page), "c");
    engine.close().unwrap();
    assert_eq!(read_text(&disk(tmp.path()), page), "c");
}

/// S1, S2 (RDD R3/R4): a pass that keeps failing without refusing writes (a full disk) is tried again at most once
/// per pass interval, and once the disk has room the versions it kept reach `krummi.data` with no further commit.
#[test]
fn a_failing_pass_is_retried_once_per_pass_interval() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    engine.set_transfer_pacing(std::time::Duration::from_millis(50), u64::MAX);
    engine.set_journal_file_limit(4096);
    let page = new_page(&engine, "a");
    set(&engine, page, "b");
    wait_until(|| engine.scratch_bytes() == 0);
    engine.set_transfer_hook(Some(Arc::new(|| {
        testing::inject(
            Op::Write,
            testing::count(Op::Write),
            Action::Fail(std::io::ErrorKind::StorageFull),
        );
    })));
    set(&engine, page, "d");
    let before = engine.transfer_passes();
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let passes = engine.transfer_passes() - before;
    engine.set_transfer_hook(None);
    // One pass per 50 ms interval at most, and some, since the failing pass is tried again.
    assert!(
        (2..=40).contains(&passes),
        "{passes} failing passes in 1.5 s"
    );
    wait_until(|| engine.scratch_bytes() == 0);
    assert_eq!(read_text(&engine, page), "d");
    engine.close().unwrap();
}

/// Transfer pacing S1: once a long read holds more than the threshold in the scratch area, commits that add less
/// than the threshold do not start a pass each; a pass starts at once only when the area grew by the threshold
/// since the last one.
#[test]
fn a_pass_starts_at_once_only_when_the_scratch_area_grew_by_the_threshold() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    engine.set_transfer_pacing(std::time::Duration::from_secs(5), 1 << 20);
    let hot = new_page(&engine, "a");
    let read = engine.read().unwrap();
    // Over the threshold at once: one immediate pass, which can move nothing past the read.
    let mut tx = engine.write(None).unwrap();
    for _ in 0..150 {
        tx.allocate().unwrap();
    }
    tx.commit().unwrap();
    assert!(engine.scratch_bytes() >= 1 << 20);
    let before = engine.transfer_passes();
    for i in 0..50 {
        set(&engine, hot, &format!("{i}"));
    }
    let passes = engine.transfer_passes() - before;
    assert!(
        passes <= 2,
        "{passes} passes for 50 commits that added 400 KiB"
    );
    // Growing by the threshold again starts a pass at once, long before the 5 s interval.
    let before = engine.transfer_passes();
    let start = std::time::Instant::now();
    let mut tx = engine.write(None).unwrap();
    for _ in 0..150 {
        tx.allocate().unwrap();
    }
    tx.commit().unwrap();
    wait_until(|| engine.transfer_passes() > before);
    assert!(
        start.elapsed() < std::time::Duration::from_secs(2),
        "the pass started after {:?}",
        start.elapsed()
    );
    drop(read);
    engine.close().unwrap();
}
