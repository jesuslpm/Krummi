//! What the public tests cannot reach without the engine's test hooks: a store that refuses writes (T12) while a
//! write transaction is open.

use std::io;
use std::thread;

use krummi_pager::testing::{self, Action, Op};

use super::*;

/// AC-B4 (B7, T12): once the store refuses writes, an invalid size in the open write still fails with its own
/// class, not with the I/O class, and committing that write without changes ends without error.
#[test]
fn ac_b4_an_invalid_size_in_a_store_that_refuses_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    // The transfer that fails is the one the test runs on this thread, whose faults it injects.
    store.engine.set_transfer_paused(true);
    let mut write = store.write().unwrap();
    let tree = write
        .open_tree_with("t", TreeKind::Normal, OpenMode::CreateNew)
        .unwrap();
    write.set(&tree, b"k", b"v").unwrap();
    write.commit().unwrap();

    let mut write = store.write().unwrap();
    let tree = write.open_tree("t", TreeKind::Normal).unwrap();
    for nth in 0..8 {
        testing::inject(Op::Write, nth, Action::Fail(io::ErrorKind::Other));
    }
    assert!(!store.engine.transfer_now(), "the transfer failed");
    testing::reset();
    let refused = thread::scope(|s| {
        s.spawn(|| {
            store
                .write_with_timeout(Duration::ZERO)
                .err()
                .map(|e| e.kind())
        })
        .join()
        .unwrap()
    });
    assert_eq!(refused, Some(ErrorKind::Io), "the store refuses writes");

    let error = write
        .set(&tree, &[0; krummi_tree::MAX_KEY + 1], b"v")
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidSize);
    let error = write
        .open_tree_with("", TreeKind::Normal, OpenMode::OpenOrCreate)
        .err()
        .unwrap();
    assert_eq!(error.kind(), ErrorKind::InvalidSize);
    write.commit().unwrap();

    store.close().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let tree = read.open_tree("t", TreeKind::Normal).unwrap();
    assert_eq!(read.get(&tree, b"k").unwrap(), Some(&b"v"[..]));
}
