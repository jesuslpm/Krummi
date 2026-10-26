//! R1 of the code review of the trees branch (`docs/reviews/trees-keys/2026-10-25-code.md`): the memory the tree
//! operations reserve comes from fallible reservations, so running out of it fails the operation with the
//! out-of-memory class instead of aborting the process (P13, B11). In a read transaction the read stays usable; in
//! a write the operation fails, the write can no longer commit, its commit fails with that class and applies
//! nothing, and the store keeps accepting writes (T11, the transactions PRD's validation row "Falta memoria al
//! abrir una transacción o durante ella").
//!
//! The engine's `Op::Alloc` fault point makes the nth reservation fail. Each test sweeps every reservation of the
//! operation, from the first until the operation succeeds. The tests that build their trees in the same write
//! that runs the operation have every page of it already copied, so the only reservations left are those of the
//! tree code: before R1's fix those operations reserved through the infallible allocator and the sweep found no
//! reservation to fail.

use std::io;

use krummi::{ErrorKind, OpenMode, Result, Store, StoreOptions, TreeKind, WriteTransaction};
use krummi_pager::testing::{self, Action, Op};

const NORMAL: TreeKind = TreeKind::Normal;
/// Entries of this many bytes fill a node page with a handful of them, so a few dozen make a tree of several
/// levels.
const BIG: usize = 1024;
/// Enough entries of [`BIG`] bytes for a branch root over several leaves.
const ENTRIES: u64 = 20;

/// Makes the `nth` reservation from now on fail for lack of memory.
fn fail_alloc(nth: usize) {
    testing::reset();
    testing::inject(Op::Alloc, nth, Action::Fail(io::ErrorKind::OutOfMemory));
}

fn temporary() -> Store {
    Store::open_temporary(&StoreOptions::default()).unwrap()
}

fn key(i: u64) -> [u8; 8] {
    i.to_be_bytes()
}

/// Stores [`ENTRIES`] entries of [`BIG`] bytes in the tree `name`, creating it.
fn fill(write: &mut WriteTransaction<'_>, name: &str) -> Result<()> {
    let tree = write.open_tree_with(name, NORMAL, OpenMode::OpenOrCreate)?;
    for i in 0..ENTRIES {
        write.set(&tree, &key(i), &[i as u8; BIG])?;
    }
    Ok(())
}

/// Commits the tree `name` with the entry `k` → `v`.
fn commit_tree(store: &Store, name: &str) {
    let mut write = store.write().unwrap();
    let tree = write
        .open_tree_with(name, NORMAL, OpenMode::CreateNew)
        .unwrap();
    write.set(&tree, b"k", b"v").unwrap();
    write.commit().unwrap();
}

/// A new write on `store` commits, and a read sees it.
fn assert_accepts_writes(store: &Store, nth: usize) {
    let mut write = store.write().unwrap();
    let tree = write
        .open_tree_with("after", NORMAL, OpenMode::OpenOrCreate)
        .unwrap();
    write.set(&tree, b"k", b"after").unwrap();
    write.commit().unwrap();
    let read = store.read().unwrap();
    let tree = read.open_tree("after", NORMAL).unwrap();
    assert_eq!(
        read.get(&tree, b"k").unwrap(),
        Some(&b"after"[..]),
        "a write after reservation {nth} failed"
    );
}

/// Runs `op` in a write on a new temporary store prepared by `setup`, with reservation `nth` failing from the
/// moment `op` calls its second argument, for nth = 0, 1, ... until `op` succeeds. Each failure has the
/// out-of-memory class and follows T11: the commit fails with that class, `unchanged` holds and a new write
/// commits. Returns the number of reservations that failed.
fn sweep(
    setup: impl Fn(&Store),
    op: impl Fn(&mut WriteTransaction<'_>, &dyn Fn()) -> Result<()>,
    unchanged: impl Fn(&Store),
) -> usize {
    for nth in 0.. {
        let store = temporary();
        setup(&store);
        let mut write = store.write().unwrap();
        let result = op(&mut write, &|| fail_alloc(nth));
        testing::reset();
        match result {
            Ok(()) => {
                write.abort();
                store.close().unwrap();
                return nth;
            }
            Err(error) => {
                assert_eq!(
                    error.kind(),
                    ErrorKind::OutOfMemory,
                    "reservation {nth}: {error}"
                );
                let error = write.commit().unwrap_err();
                assert_eq!(
                    error.kind(),
                    ErrorKind::OutOfMemory,
                    "reservation {nth}: {error}"
                );
                unchanged(&store);
                assert_accepts_writes(&store, nth);
                store.close().unwrap();
            }
        }
    }
    unreachable!("the sweep ends when the operation succeeds")
}

/// What the read of `name`'s entry `k` sees on `store`.
fn committed_value(store: &Store, name: &str, k: &[u8]) -> Option<Vec<u8>> {
    let read = store.read().unwrap();
    let tree = read.open_tree(name, NORMAL).unwrap();
    read.get(&tree, k).unwrap().map(<[u8]>::to_vec)
}

fn no_trees(store: &Store) {
    let read = store.read().unwrap();
    assert_eq!(read.list_trees().unwrap(), Vec::new(), "nothing applied");
}

/// R1, the review's scenario (P13, B11, T11): listing the trees in a read that already opened one fails with the
/// out-of-memory class at every reservation, and the same read then lists them and reads the open tree.
#[test]
fn r1_list_trees_in_a_read_fails_with_out_of_memory_and_the_read_stays_usable() {
    let store = temporary();
    for name in ["a", "b", "c"] {
        commit_tree(&store, name);
    }
    let expected: Vec<(String, TreeKind)> = ["a", "b", "c"]
        .iter()
        .map(|name| (name.to_string(), NORMAL))
        .collect();
    let read = store.read().unwrap();
    let tree = read.open_tree("a", NORMAL).unwrap();
    let mut failures = 0;
    loop {
        fail_alloc(failures);
        let result = read.list_trees();
        testing::reset();
        match result {
            Ok(list) => {
                assert_eq!(list, expected);
                break;
            }
            Err(error) => {
                assert_eq!(error.kind(), ErrorKind::OutOfMemory, "{error}");
                assert_eq!(
                    read.list_trees().unwrap(),
                    expected,
                    "the read stays usable"
                );
                assert_eq!(read.get(&tree, b"k").unwrap(), Some(&b"v"[..]));
                failures += 1;
            }
        }
    }
    // A reservation for each name and for each entry of the list.
    assert!(failures >= 6, "{failures} reservations failed");
    drop(read);
    store.close().unwrap();
}

/// R1 (P13, T11): listing the trees in a write fails with the out-of-memory class, the write can no longer commit
/// and applies nothing, and the store keeps accepting writes.
#[test]
fn r1_list_trees_in_a_write_follows_t11() {
    let failures = sweep(
        |store| commit_tree(store, "a"),
        |write, arm| {
            let tree = write.open_tree("a", NORMAL)?;
            write.set(&tree, b"k", b"changed")?;
            arm();
            write.list_trees()?;
            Ok(())
        },
        |store| {
            assert_eq!(
                committed_value(store, "a", b"k").as_deref(),
                Some(&b"v"[..])
            )
        },
    );
    assert!(failures >= 2, "{failures} reservations failed");
}

/// R1 (P13, T11): opening an existing tree in a write reserves its place in the write's table of trees and its
/// name; failing follows T11.
#[test]
fn r1_opening_a_tree_in_a_write_follows_t11() {
    let failures = sweep(
        |store| commit_tree(store, "t"),
        |write, arm| {
            write.open_tree_with("other", NORMAL, OpenMode::CreateNew)?;
            arm();
            let tree = write.open_tree("t", NORMAL)?;
            write.set(&tree, b"k", b"changed")?;
            Ok(())
        },
        |store| {
            assert_eq!(
                committed_value(store, "t", b"k").as_deref(),
                Some(&b"v"[..])
            );
            assert!(!store.read().unwrap().tree_exists("other").unwrap());
        },
    );
    assert!(failures >= 2, "{failures} reservations failed");
}

/// R1 (P13, T11): creating a tree fails with the out-of-memory class at every reservation, including its place in
/// the write's table and its name; failing follows T11.
#[test]
fn r1_creating_a_tree_follows_t11() {
    let failures = sweep(
        |_| {},
        |write, arm| {
            write.open_tree_with("first", NORMAL, OpenMode::CreateNew)?;
            arm();
            write.open_tree_with("second", NORMAL, OpenMode::CreateNew)?;
            Ok(())
        },
        no_trees,
    );
    assert!(failures >= 3, "{failures} reservations failed");
}

/// R1 (P13, T11): sets that split nodes, up to a new root, fail with the out-of-memory class at every reservation
/// of the splits (the copy of the node, its cells and the separator key); failing follows T11.
#[test]
fn r1_sets_that_split_follow_t11() {
    let failures = sweep(
        |store| commit_tree(store, "t"),
        |write, arm| {
            arm();
            fill(write, "t")
        },
        |store| {
            let read = store.read().unwrap();
            let tree = read.open_tree("t", NORMAL).unwrap();
            assert_eq!(read.len(&tree).unwrap(), 1, "nothing applied");
        },
    );
    assert!(failures >= 4, "{failures} reservations failed");
}

/// R1 (P13, T11): removals that merge nodes fail with the out-of-memory class at the copy of the merged node;
/// failing follows T11. The tree is built in the same write, so the merges copy no page of the transaction.
#[test]
fn r1_removes_that_merge_follow_t11() {
    let failures = sweep(
        |_| {},
        |write, arm| {
            fill(write, "t")?;
            let tree = write.open_tree("t", NORMAL)?;
            arm();
            for i in 0..ENTRIES {
                write.remove(&tree, &key(i))?;
            }
            Ok(())
        },
        no_trees,
    );
    assert!(failures >= 1, "{failures} reservations failed");
}

/// R1 (P13, T11): deleting a tree of several levels fails with the out-of-memory class at the list of children of
/// a branch; failing follows T11. The tree is built in the same write, so the frees copy no page.
#[test]
fn r1_deleting_a_multi_level_tree_follows_t11() {
    let failures = sweep(
        |_| {},
        |write, arm| {
            fill(write, "t")?;
            arm();
            write.delete_tree("t")
        },
        no_trees,
    );
    assert!(failures >= 1, "{failures} reservations failed");
}

/// R1 (P13, T11): renaming an open tree reserves the new name in the write's table; failing follows T11. Another
/// tree created first in the same write has already copied the catalog's pages.
#[test]
fn r1_renaming_an_open_tree_follows_t11() {
    let failures = sweep(
        |store| commit_tree(store, "t"),
        |write, arm| {
            write.open_tree("t", NORMAL)?;
            write.open_tree_with("other", NORMAL, OpenMode::CreateNew)?;
            arm();
            write.rename_tree("t", "u", false)
        },
        |store| {
            let read = store.read().unwrap();
            assert!(read.tree_exists("t").unwrap());
            assert!(!read.tree_exists("u").unwrap());
            assert!(!read.tree_exists("other").unwrap());
        },
    );
    assert!(failures >= 1, "{failures} reservations failed");
}

/// R1 (P13, T11): the commit that writes a changed tree's entry into the catalog fails with the out-of-memory class
/// at every reservation, applies nothing, and the store keeps accepting writes.
#[test]
fn r1_committing_changed_tree_entries_follows_t11() {
    let mut failures = 0;
    loop {
        let store = temporary();
        commit_tree(&store, "t");
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        write.set(&tree, b"k2", b"v2").unwrap();
        fail_alloc(failures);
        let result = write.commit();
        testing::reset();
        match result {
            Ok(()) => {
                assert_eq!(
                    committed_value(&store, "t", b"k2").as_deref(),
                    Some(&b"v2"[..])
                );
                store.close().unwrap();
                break;
            }
            Err(error) => {
                assert_eq!(error.kind(), ErrorKind::OutOfMemory, "{error}");
                assert_eq!(committed_value(&store, "t", b"k2"), None, "nothing applied");
                assert_accepts_writes(&store, failures);
                store.close().unwrap();
                failures += 1;
            }
        }
    }
    assert!(failures >= 1, "{failures} reservations failed");
}

/// R3 of the follow-up code review (`docs/reviews/trees-keys/2026-10-26-code.md`; A3, A7, B7): renaming an open
/// tree to an existing name without overwriting fails with the already-exists class even when the next
/// reservations would fail for lack of memory, because the rename reserves nothing before rejecting it; the write
/// keeps its pending change and commits.
#[test]
fn r3_renaming_onto_an_existing_tree_is_rejected_before_any_reservation() {
    for nth in 0..4 {
        let store = temporary();
        commit_tree(&store, "t");
        commit_tree(&store, "u");
        let mut write = store.write().unwrap();
        let t = write.open_tree("t", NORMAL).unwrap();
        write.set(&t, b"k", b"changed").unwrap();
        fail_alloc(nth);
        let error = write.rename_tree("t", "u", false).unwrap_err();
        testing::reset();
        assert_eq!(
            error.kind(),
            ErrorKind::TreeAlreadyExists,
            "nth {nth}: {error}"
        );
        write.commit().unwrap();
        assert_eq!(
            committed_value(&store, "t", b"k").as_deref(),
            Some(&b"changed"[..]),
            "nth {nth}"
        );
    }
}

/// Commits the tree `t` with [`ENTRIES`] entries of [`BIG`] bytes: a branch root over several leaves.
fn commit_big_tree(store: &Store) {
    let mut write = store.write().unwrap();
    fill(&mut write, "t").unwrap();
    write.commit().unwrap();
}

/// The number of entries of the committed tree `t`.
fn committed_len(store: &Store) -> u64 {
    let read = store.read().unwrap();
    let tree = read.open_tree("t", NORMAL).unwrap();
    read.len(&tree).unwrap()
}

/// In-order scan PRD, AC-R12 and R8 (T11): running out of memory while a range removal frees inner subtrees and
/// fixes the children at its bounds fails it with the out-of-memory class, and the write applies nothing.
#[test]
fn ac_r12_a_range_removal_out_of_memory_follows_t11() {
    let failed = sweep(
        commit_big_tree,
        |write, arm| {
            let tree = write.open_tree("t", NORMAL)?;
            arm();
            let (lower, upper) = (key(3), key(17));
            let removed = write.remove_range(&tree, &lower[..]..&upper[..])?;
            assert_eq!(removed, 14);
            Ok(())
        },
        |store| assert_eq!(committed_len(store), ENTRIES),
    );
    assert!(failed >= 1, "the removal reserves memory");
}

/// In-order scan PRD, AC-R13 (R8): running out of memory in the middle of a write's scan, when its back end
/// starts, fails that step with the out-of-memory class; the scan then returns nothing from either end, and the
/// write follows T11.
#[test]
fn ac_r13_a_write_scan_out_of_memory_stops_and_follows_t11() {
    let failed = sweep(
        commit_big_tree,
        |write, arm| {
            let tree = write.open_tree("t", NORMAL)?;
            let mut scan = write.range(&tree, ..)?;
            assert!(scan.next().unwrap().is_ok());
            arm();
            match scan.next_back().unwrap() {
                Ok(_) => Ok(()),
                Err(error) => {
                    assert!(scan.next().is_none() && scan.next_back().is_none());
                    Err(error)
                }
            }
        },
        |store| assert_eq!(committed_len(store), ENTRIES),
    );
    assert_eq!(failed, 1, "the back end reserves its path once");
}

/// In-order scan PRD, AC-R13 (R8): in a read transaction, running out of memory in the middle of a scan fails that
/// step with the out-of-memory class and the scan then returns nothing from either end; asking for a scan, which
/// copies its bounds, fails the same way; the read stays usable and a new scan returns every entry.
#[test]
fn ac_r13_a_read_scan_out_of_memory_stops_and_the_read_stays_usable() {
    let store = temporary();
    commit_big_tree(&store);
    let read = store.read().unwrap();
    let tree = read.open_tree("t", NORMAL).unwrap();
    let mut scan = read.range(&tree, ..).unwrap();
    assert!(scan.next().unwrap().is_ok());
    fail_alloc(0);
    let error = scan.next_back().unwrap().unwrap_err();
    testing::reset();
    assert_eq!(error.kind(), ErrorKind::OutOfMemory);
    assert!(scan.next().is_none() && scan.next_back().is_none());
    let lower = key(3);
    fail_alloc(0);
    let error = read.range(&tree, &lower[..]..).map(|_| ()).unwrap_err();
    testing::reset();
    assert_eq!(error.kind(), ErrorKind::OutOfMemory);
    assert_eq!(read.range(&tree, ..).unwrap().count() as u64, ENTRIES);
    assert!(read.get(&tree, &key(0)).unwrap().is_some());
}

/// Codex review R1 (`docs/reviews/trees-scan/2026-10-26-code-02.md#R1`), AC-R1: once one end of a scan returns
/// nothing more, the other end returns nothing either, without reading or reserving anything, so it cannot fail
/// and the write still commits; from either end first.
#[test]
fn r1_an_exhausted_scan_ends_at_both_ends_without_reserving() {
    for front_first in [true, false] {
        let store = temporary();
        let mut write = store.write().unwrap();
        let tree = write
            .open_tree_with("pending", NORMAL, OpenMode::CreateNew)
            .unwrap();
        write.set(&tree, b"a", b"pending").unwrap();
        let mut scan = write.range(&tree, ..).unwrap();
        let (first, then) = if front_first {
            (scan.next(), scan.next())
        } else {
            (scan.next_back(), scan.next_back())
        };
        assert!(first.unwrap().is_ok() && then.is_none());
        fail_alloc(0);
        let other = if front_first {
            scan.next_back()
        } else {
            scan.next()
        };
        // The failure armed above would hit the first reservation: there must be none.
        let reserved = testing::count(Op::Alloc);
        testing::reset();
        assert!(other.is_none(), "front first: {front_first}");
        assert_eq!(reserved, 0, "front first: {front_first}");
        drop(scan);
        write.commit().unwrap();
        assert!(store.read().unwrap().tree_exists("pending").unwrap());
    }
}
