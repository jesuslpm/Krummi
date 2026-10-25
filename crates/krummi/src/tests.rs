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
    // The next eight writes fail, a margin over the first write of the transfer pass, which already makes it fail,
    // whichever thread sharing these faults writes first (the journal's writer adopts them); the transaction tests
    // use the same margin.
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

/// A key of 2,048 bytes that starts with `i`: three fit in a leaf and three in a branch, so a few dozen of them
/// build a tree of several levels.
fn long_key(i: usize) -> Vec<u8> {
    let mut key = format!("{i:05}").into_bytes();
    key.resize(krummi_tree::MAX_KEY, b'.');
    key
}

/// Each tree a new read sees, with its count and what it holds for every key of the test, in order.
type Seen = Vec<(String, u64, Vec<Option<Vec<u8>>>)>;

/// What a new read sees of the trees, for the keys `keys` (see [`Seen`]).
fn trees_seen(store: &Store, keys: &[Vec<u8>]) -> Seen {
    let read = store.read().unwrap();
    let list = read.list_trees().unwrap();
    list.into_iter()
        .map(|(name, kind)| {
            let tree = read.open_tree(&name, kind).unwrap();
            let values = keys
                .iter()
                .map(|key| read.get(&tree, key).unwrap().map(<[u8]>::to_vec))
                .collect();
            (name, read.len(&tree).unwrap(), values)
        })
        .collect()
}

/// The write of the T11 sweep: through `tree`, splits that reach the root, so the tree gains levels, and removals
/// whose merges bring it back down, with its operations' results. It goes on after a failure, as an application
/// may (T11).
fn splits_and_merges(write: &mut WriteTransaction<'_>) -> Vec<Result<()>> {
    let tree = match write.open_tree("t", TreeKind::Normal) {
        Ok(tree) => tree,
        Err(error) => return vec![Err(error)],
    };
    let mut results = Vec::new();
    for i in 12..48 {
        results.push(write.set(&tree, &long_key(i), b"split"));
    }
    for i in (0..48).filter(|i| i % 8 != 0) {
        results.push(write.remove(&tree, &long_key(i)).map(drop));
    }
    results.push(write.try_add(&tree, &long_key(100), b"added").map(drop));
    results
}

/// AC-B18, second part, with its 2026-10-25 annotation (T11): a failure of disk full or maximum size at each
/// point where the write of [`splits_and_merges`] reads, changes, reserves or frees a page, also after it changed
/// many and in the middle of splits and merges over several levels, or where its commit writes the catalog, makes
/// that operation fail with it; the commit fails with that class and applies nothing. After the abort, or after
/// that failed commit, the tree, the catalog and the count are as before, and a new write reads and modifies them.
/// On disk and in a temporary store, at one point in `step`; once no point is left, the write commits.
fn t11_sweep(step: u64) {
    // The checks are about T11, not about durability: the store's flushes to disk are skipped.
    testing::skip_syncs(true);
    let tmp = tempfile::tempdir().unwrap();
    let on_disk = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let in_memory = Store::open_temporary(&StoreOptions::default()).unwrap();
    let keys: Vec<Vec<u8>> = (0..48).chain([100]).map(long_key).collect();
    for store in [&on_disk, &in_memory] {
        let mut write = store.write().unwrap();
        for name in ["other", "t"] {
            let tree = write
                .open_tree_with(name, TreeKind::Normal, OpenMode::CreateNew)
                .unwrap();
            for i in 0..12 {
                write.set(&tree, &long_key(i), name.as_bytes()).unwrap();
            }
        }
        write.commit().unwrap();
        let before = trees_seen(store, &keys);
        let mut points = 0;
        loop {
            let failure = if points % 2 == 0 {
                Error::DiskFull {
                    folder: tmp.path().to_path_buf(),
                }
            } else {
                Error::MaxSize {
                    max_size: 1 << 20,
                    needed: 2 << 20,
                }
            };
            let class = failure.kind();
            let mut write = store.write().unwrap();
            store.engine.set_page_fault(Some((points, failure)));
            let results = splits_and_merges(&mut write);
            let label = format!("point {points}");
            if store.engine.page_fault_pending() {
                assert!(results.iter().all(Result::is_ok), "{label}");
                let committed = write.commit();
                if store.engine.page_fault_pending() {
                    store.engine.set_page_fault(None);
                    committed.unwrap();
                    break;
                }
                assert_eq!(
                    committed.unwrap_err().kind(),
                    class,
                    "{label}: in the commit"
                );
            } else {
                let first = results.iter().find_map(|r| r.as_ref().err());
                assert_eq!(
                    first.map(Error::kind),
                    Some(class),
                    "{label}: the failing operation"
                );
                if points % 4 < 2 {
                    write.abort();
                } else {
                    assert_eq!(write.commit().unwrap_err().kind(), class, "{label}: commit");
                }
            }
            assert!(
                trees_seen(store, &keys) == before,
                "{label}: nothing applied"
            );
            let mut write = store.write().unwrap();
            let tree = write.open_tree("t", TreeKind::Normal).unwrap();
            assert_eq!(write.len(&tree).unwrap(), 12, "{label}");
            write.set(&tree, b"probe", b"x").unwrap();
            assert_eq!(write.get(&tree, &long_key(11)).unwrap(), Some(&b"t"[..]));
            assert!(write.remove(&tree, b"probe").unwrap());
            write.commit().unwrap();
            points += step;
        }
        assert!(points > 700, "{points} points");
        let read = store.read().unwrap();
        let tree = read.open_tree("t", TreeKind::Normal).unwrap();
        assert_eq!(read.len(&tree).unwrap(), 7, "0, 8, 16, 24, 32, 40 and 100");
        assert_eq!(read.get(&tree, &long_key(40)).unwrap(), Some(&b"split"[..]));
    }
    on_disk.close().unwrap();
    testing::reset();
}

/// AC-B18 at every 13th point: an odd step, so that both classes, aborts and failed commits take turns.
#[test]
fn ac_b18_a_failure_at_a_sample_of_page_operations_of_splits_and_merges_applies_nothing() {
    t11_sweep(13);
}

/// `cargo test -p krummi --lib -- --ignored ac_b18_a_failure_at_every`.
#[test]
#[ignore = "acceptance: a failure at every page operation of the write and its commit"]
fn ac_b18_a_failure_at_every_page_operation_of_splits_and_merges_applies_nothing() {
    t11_sweep(1);
}
