//! The write turn and its queue (Transactions PRD: T3, T4, T5, T12, T13, T18; AC7, AC-T6, AC-T7, AC-T8, AC-T9,
//! AC-T14, AC-T18, AC-T22).

mod common;

use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use common::*;
use krummi_pager::testing::{self, Action, Op};
use krummi_tx::{Error, ErrorKind, Point};

const LONG: Duration = Duration::from_secs(1);

#[test]
fn ac7_concurrent_writers_never_overlap_and_all_commit() {
    both(|engine| {
        let page = new_page(engine, "0");
        let open = AtomicUsize::new(0);
        thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    let mut tx = engine.write(None).unwrap();
                    assert_eq!(open.fetch_add(1, Ordering::SeqCst), 0, "two writes open");
                    let count: u32 = text(tx.page(page).unwrap()).parse().unwrap();
                    put(&mut tx, page, &(count + 1).to_string());
                    open.fetch_sub(1, Ordering::SeqCst);
                    tx.commit().unwrap();
                });
            }
        });
        assert_eq!(read_text(engine, page), "8");
    });
}

#[test]
fn ac7_ac_t7_queued_requests_get_the_write_in_arrival_order_and_expired_ones_leave() {
    both(|engine| {
        let order = Mutex::new(Vec::new());
        let holder = engine.write(None).unwrap();
        let (expired, has_expired) = mpsc::channel();
        thread::scope(|s| {
            let order = &order;
            s.spawn(move || {
                let tx = engine.write(None).unwrap();
                order.lock().unwrap().push("A");
                tx.commit().unwrap();
            });
            wait_until(|| engine.queued() == 1);
            // B's limit is long enough for C to queue behind it even on a loaded machine.
            s.spawn(move || {
                let error = engine.write(Some(LONG)).err().unwrap();
                assert_eq!(error.kind(), ErrorKind::Timeout);
                order.lock().unwrap().push("B expired");
                expired.send(()).unwrap();
            });
            wait_until(|| engine.queued() == 2);
            s.spawn(move || {
                let tx = engine.write(None).unwrap();
                order.lock().unwrap().push("C");
                tx.commit().unwrap();
            });
            wait_until(|| engine.queued() == 3);
            has_expired.recv().unwrap();
            assert_eq!(engine.queued(), 2, "B left the queue");
            drop(holder);
        });
        assert_eq!(*order.lock().unwrap(), ["B expired", "A", "C"]);
    });
}

#[test]
fn ac_t6_a_wait_limit_expires_without_effects() {
    both(|engine| {
        let (ready, is_ready) = mpsc::channel();
        let (release, released) = mpsc::channel::<()>();
        thread::scope(|s| {
            s.spawn(move || {
                let tx = engine.write(None).unwrap();
                ready.send(()).unwrap();
                released.recv().unwrap();
                drop(tx);
            });
            is_ready.recv().unwrap();
            let start = Instant::now();
            let error = engine
                .write(Some(Duration::from_millis(100)))
                .err()
                .unwrap();
            assert_eq!(error.kind(), ErrorKind::Timeout, "{error}");
            assert!(start.elapsed() >= Duration::from_millis(100));
            assert_eq!(engine.queued(), 0, "an expired request leaves the queue");
            // A zero limit with a write open fails while that write is still open.
            let error = engine.write(Some(Duration::ZERO)).err().unwrap();
            assert_eq!(error.kind(), ErrorKind::Timeout, "{error}");
            assert_eq!(engine.queued(), 0, "a zero limit never queues");
            let later = s.spawn(move || engine.write(None).map(|tx| tx.commit()).is_ok());
            wait_until(|| engine.queued() == 1);
            release.send(()).unwrap();
            assert!(later.join().unwrap());
        });
        // A zero limit with no write open and nobody queued gets the write.
        engine
            .write(Some(Duration::ZERO))
            .unwrap()
            .commit()
            .unwrap();
    });
}

#[test]
fn ac_t8_a_second_write_on_the_same_thread_is_misuse() {
    both(|engine| {
        let page = new_page(engine, "a");
        let mut tx = engine.write(None).unwrap();
        for limit in [None, Some(LONG), Some(Duration::ZERO)] {
            let start = Instant::now();
            let error = engine.write(limit).err().unwrap();
            assert_eq!(error.kind(), ErrorKind::Misuse, "{error}");
            assert!(start.elapsed() < LONG, "misuse does not wait");
        }
        let other = temporary();
        other.write(None).unwrap().commit().unwrap();
        put(&mut tx, page, "b");
        tx.commit().unwrap();
        assert_eq!(read_text(engine, page), "b");
    });
}

#[test]
fn ac_t9_a_limit_that_cannot_be_counted_is_misuse_and_24_hours_is_accepted() {
    both(|engine| {
        let error = engine.write(Some(Duration::MAX)).err().unwrap();
        assert_eq!(error.kind(), ErrorKind::Misuse, "{error}");
        assert_eq!(engine.queued(), 0);
        let day = Duration::from_secs(24 * 60 * 60);
        engine.write(Some(day)).unwrap().commit().unwrap();
    });
}

#[test]
fn ac_t14_a_failed_write_makes_waiting_and_new_requests_fail_at_once() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = &disk(tmp.path());
    let page = new_page(engine, "a");
    let (ready, is_ready) = mpsc::channel();
    let (go, can_go) = mpsc::channel::<()>();
    let (failed, has_failed) = mpsc::channel();
    let (end, ended) = mpsc::channel::<()>();
    thread::scope(|s| {
        let writer = s.spawn(move || {
            testing::reset();
            testing::inject(Op::Reserve, 0, Action::Fail(io::ErrorKind::Other));
            let mut tx = engine.write(None).unwrap();
            ready.send(()).unwrap();
            can_go.recv().unwrap();
            // Reserving pages until the store must grow: the growth fails with an I/O error.
            let error = loop {
                if let Err(error) = tx.allocate() {
                    break error;
                }
            };
            testing::reset();
            assert_eq!(error.kind(), ErrorKind::Io, "{error}");
            failed.send(()).unwrap();
            let again = engine.write(None).err().unwrap();
            assert_eq!(
                again.kind(),
                ErrorKind::Misuse,
                "the thread that has the write"
            );
            ended.recv().unwrap();
            assert_eq!(tx.commit().unwrap_err().kind(), ErrorKind::Io);
        });
        is_ready.recv().unwrap();
        let waiting = s.spawn(move || engine.write(None).err().unwrap().kind());
        wait_until(|| engine.queued() == 1);
        go.send(()).unwrap();
        // The waiting request fails as soon as the store refuses writes, with the failed write still open.
        has_failed.recv().unwrap();
        assert_eq!(waiting.join().unwrap(), ErrorKind::Io);
        for limit in [None, Some(Duration::ZERO)] {
            let error = engine.write(limit).err().unwrap();
            assert_eq!(error.kind(), ErrorKind::Io, "{error}");
        }
        let error = engine.write(Some(Duration::MAX)).err().unwrap();
        assert_eq!(error.kind(), ErrorKind::Misuse, "misuse wins over I/O");
        assert_eq!(read_text(engine, page), "a");
        end.send(()).unwrap();
        writer.join().unwrap();
    });
}

#[test]
fn ac_t18_nobody_jumps_the_queue_at_the_hand_off() {
    both(|engine| {
        let (at_handoff, reached) = mpsc::channel::<()>();
        let (resume, resumed) = mpsc::channel::<()>();
        let (at_handoff, resumed) = (Mutex::new(at_handoff), Mutex::new(resumed));
        let armed = AtomicBool::new(true);
        engine.set_hook(Some(Arc::new(move |point| {
            if point == Point::Handoff && armed.swap(false, Ordering::SeqCst) {
                at_handoff.lock().unwrap().send(()).unwrap();
                resumed.lock().unwrap().recv().unwrap();
            }
        })));
        let order = Mutex::new(Vec::new());
        let holder_done = AtomicBool::new(false);
        thread::scope(|s| {
            let order = &order;
            let (release, released) = mpsc::channel::<()>();
            let (held, is_held) = mpsc::channel();
            let holder_done = &holder_done;
            s.spawn(move || {
                let tx = engine.write(None).unwrap();
                held.send(()).unwrap();
                released.recv().unwrap();
                drop(tx); // stops at the hand-off: A is waiting
                holder_done.store(true, Ordering::SeqCst);
            });
            is_held.recv().unwrap();
            s.spawn(move || {
                let tx = engine.write(None).unwrap();
                order.lock().unwrap().push("A");
                tx.commit().unwrap();
            });
            wait_until(|| engine.queued() == 1);
            release.send(()).unwrap();
            reached.recv().unwrap();
            // The write just ended and A is waiting: it is not free.
            let error = engine.write(Some(Duration::ZERO)).err().unwrap();
            assert_eq!(error.kind(), ErrorKind::Timeout, "{error}");
            s.spawn(move || {
                let tx = engine.write(None).unwrap();
                order.lock().unwrap().push("C");
                tx.commit().unwrap();
            });
            wait_until(|| engine.queued() == 2); // C queued behind A during the hand-off
            resume.send(()).unwrap();
        });
        engine.set_hook(None);
        assert!(holder_done.load(Ordering::SeqCst));
        assert_eq!(*order.lock().unwrap(), ["A", "C"]);
    });
}

#[test]
fn ac_t22_the_new_errors_carry_the_store_and_the_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let on_disk = disk(tmp.path());
    let in_memory = temporary();
    for (engine, folder) in [
        (&on_disk, Some(tmp.path().to_path_buf())),
        (&in_memory, None),
    ] {
        let tx = engine.write(None).unwrap();
        thread::scope(|s| {
            let limit = Duration::from_millis(10);
            match s
                .spawn(move || engine.write(Some(limit)).err().unwrap())
                .join()
                .unwrap()
            {
                Error::Timeout {
                    folder: f,
                    limit: l,
                } => assert_eq!((f, l), (folder.clone(), limit)),
                other => panic!("expected a timeout, got {other:?}"),
            }
        });
        match engine.write(None).err().unwrap() {
            Error::Misuse { folder: f, .. } => assert_eq!(f, folder),
            other => panic!("expected misuse, got {other:?}"),
        }
        drop(tx);
    }
}

/// Turn handoff S1: releasing the turn wakes only the request that gets it, not every request in the queue.
#[test]
fn releasing_the_turn_wakes_only_the_request_that_gets_it() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let held = engine.write(None).unwrap();
    let waiting = 8;
    let before = thread::scope(|s| {
        for _ in 0..waiting {
            s.spawn(|| drop(engine.write(None).unwrap()));
        }
        wait_until(|| engine.queued() == waiting);
        let before = engine.turn_wakeups();
        drop(held);
        before
    });
    let wakeups = engine.turn_wakeups() - before;
    // One per handoff, plus room for spurious wakeups; waking every request gives 8 + 7 + ... + 1 = 36.
    assert!(wakeups <= 12, "{wakeups} wakeups for {waiting} handoffs");
}
