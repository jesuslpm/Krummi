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

/// The write of the 5.2 sweep (In-order scan PRD: AC-R12, AC-R13): it splits pages, scans the tree from both
/// ends, removes intervals that merge pages and free subtrees over several levels, with a prefix and without, and
/// scans again. A scan that fails stops at both ends (R8).
fn range_removals(write: &mut WriteTransaction<'_>) -> Vec<Result<()>> {
    let tree = match write.open_tree("t", TreeKind::Normal) {
        Ok(tree) => tree,
        Err(error) => return vec![Err(error)],
    };
    let mut results = Vec::new();
    for i in 12..48 {
        results.push(write.set(&tree, &long_key(i), b"split"));
    }
    let ok = |results: &[Result<()>]| results.iter().all(Result::is_ok);
    results.push(scan_whole(write, &tree, ok(&results).then_some(48)));
    let (lower, upper) = (long_key(5), long_key(30));
    results.push(write.remove_range(&tree, &lower[..]..&upper[..]).map(drop));
    // The keys from 40 to 47.
    results.push(write.remove_range_prefix(&tree, b"0004", ..).map(drop));
    results.push(scan_whole(write, &tree, ok(&results).then_some(15)));
    results
}

/// Scans `tree` whole, alternating ends, and checks that its keys are in order and, when given, that it holds
/// `count` entries (after an earlier failure what the write sees is left to the implementation); after a failure
/// the scan returns nothing more from either end.
fn scan_whole(
    write: &WriteTransaction<'_>,
    tree: &WriteTree<'_>,
    count: Option<usize>,
) -> Result<()> {
    let mut scan = write.range(tree, ..)?;
    let (mut front, mut back) = (Vec::new(), Vec::new());
    loop {
        let step = if (front.len() + back.len()) % 2 == 0 {
            scan.next().map(|e| e.map(|(k, _)| front.push(k)))
        } else {
            scan.next_back().map(|e| e.map(|(k, _)| back.push(k)))
        };
        match step {
            None => break,
            Some(Ok(())) => {}
            Some(Err(error)) => {
                assert!(
                    scan.next().is_none() && scan.next_back().is_none(),
                    "the scan stopped"
                );
                return Err(error);
            }
        }
    }
    back.reverse();
    front.extend(back);
    if let Some(count) = count {
        assert_eq!(front.len(), count);
    }
    assert!(
        front.windows(2).all(|pair| pair[0] < pair[1]),
        "in key order"
    );
    Ok(())
}

/// AC-B18, second part, with its 2026-10-25 annotation (T11): a failure of disk full or maximum size at each
/// point where the write of [`splits_and_merges`] reads, changes, reserves or frees a page, also after it changed
/// many and in the middle of splits and merges over several levels, or where its commit writes the catalog, makes
/// that operation fail with it; the commit fails with that class and applies nothing. After the abort, or after
/// that failed commit, the tree, the catalog and the count are as before, and a new write reads and modifies them.
/// On disk and in a temporary store, at one point in `step`; once no point is left, the write commits.
fn t11_sweep(step: u64) {
    t11_sweep_of(
        step,
        splits_and_merges,
        &[Class::DiskFull, Class::MaxSize],
        700,
        |store| {
            let read = store.read().unwrap();
            let tree = read.open_tree("t", TreeKind::Normal).unwrap();
            assert_eq!(read.len(&tree).unwrap(), 7, "0, 8, 16, 24, 32, 40 and 100");
            assert_eq!(read.get(&tree, &long_key(40)).unwrap(), Some(&b"split"[..]));
        },
    );
}

/// AC-R12 with AC-B18's second part, and AC-R13's I/O failure in the middle of a scan: a failure of disk full,
/// maximum size or input and output at each point where the write of [`range_removals`] reads, changes, reserves
/// or frees a page, in its range removals and in its scans, fails that operation, and its commit fails with that
/// class and applies nothing (the rest as in [`t11_sweep`]).
fn t11_range_sweep(step: u64) {
    let classes = [Class::DiskFull, Class::MaxSize, Class::Io];
    t11_sweep_of(step, range_removals, &classes, 500, |store| {
        let read = store.read().unwrap();
        let tree = read.open_tree("t", TreeKind::Normal).unwrap();
        assert_eq!(read.len(&tree).unwrap(), 15, "0 to 4 and 30 to 39");
        assert_eq!(read.get(&tree, &long_key(30)).unwrap(), Some(&b"split"[..]));
        assert_eq!(read.get(&tree, &long_key(29)).unwrap(), None);
    });
}

/// The failures a T11 sweep injects, in turns.
#[derive(Clone, Copy)]
enum Class {
    DiskFull,
    MaxSize,
    Io,
}

impl Class {
    fn error(self, folder: &std::path::Path) -> Error {
        match self {
            Class::DiskFull => Error::DiskFull {
                folder: folder.to_path_buf(),
            },
            Class::MaxSize => Error::MaxSize {
                max_size: 1 << 20,
                needed: 2 << 20,
            },
            Class::Io => Error::Io {
                folder: folder.to_path_buf(),
                source: io::Error::other("injected"),
            },
        }
    }
}

/// The T11 sweep of the write `change` on a store with the trees `other` and `t` of 12 entries each, with the
/// failures of `classes` in turns; the write must have more than `min_points` points, and `finished` checks the
/// store once it commits.
fn t11_sweep_of(
    step: u64,
    change: fn(&mut WriteTransaction<'_>) -> Vec<Result<()>>,
    classes: &[Class],
    min_points: u64,
    finished: impl Fn(&Store),
) {
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
            let failure = classes[(points / step) as usize % classes.len()].error(tmp.path());
            let class = failure.kind();
            let mut write = store.write().unwrap();
            store.engine.set_page_fault(Some((points, failure)));
            let results = change(&mut write);
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
        assert!(points > min_points, "{points} points");
        finished(store);
    }
    on_disk.close().unwrap();
    testing::reset();
}

/// AC-B18 at every 13th point: an odd step, so that both classes, aborts and failed commits take turns.
#[test]
fn ac_b18_a_failure_at_a_sample_of_page_operations_of_splits_and_merges_applies_nothing() {
    t11_sweep(13);
}

/// AC-R12 and AC-R13 at every 13th point of the write with range removals and scans.
#[test]
fn ac_r12_a_failure_at_a_sample_of_page_operations_of_range_removals_and_scans_applies_nothing() {
    t11_range_sweep(13);
}

/// `cargo test -p krummi --lib -- --ignored ac_r12_a_failure_at_every`.
#[test]
#[ignore = "acceptance: a failure at every page operation of the write and its commit"]
fn ac_r12_a_failure_at_every_page_operation_of_range_removals_and_scans_applies_nothing() {
    t11_range_sweep(1);
}

/// `cargo test -p krummi --lib -- --ignored ac_b18_a_failure_at_every`.
#[test]
#[ignore = "acceptance: a failure at every page operation of the write and its commit"]
fn ac_b18_a_failure_at_every_page_operation_of_splits_and_merges_applies_nothing() {
    t11_sweep(1);
}
