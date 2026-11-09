//! Layout checks of the nodes a write transaction changes (the commit checks PRD, H1 and H2: AC-H1 to AC-H6), through
//! the public API, the write transaction's record of the pages it changes and reads, and the record of the nodes its
//! commit checks. AC-S4 to AC-S6 of the writer checks PRD, about the integrity of what a write reads (S2), are with the
//! remembered page integrity tests.

use std::collections::BTreeSet;

use krummi_tree::{catalog, fixed, height};
use krummi_tx::{LayoutEvent, MAP_PAGE, META_PAGE, commit_checks};

use super::*;
use crate::check_tests::{commit_tree, damaged_page, leaf, long_key, on_both, root_of};

/// Bytes of the keys, and of the multi-value tree's values, of AC-H1: a few fill a leaf and a branch, so a few hundred
/// split leaves and branches.
const LONG: usize = 1000;
/// Keys of the normal tree and values of the multi-value key of AC-H1.
const COUNT: usize = 200;
/// Length of the values of the fixed-size tree of AC-H1: three fill a leaf.
const FIXED_LONG: usize = fixed::MAX_LEN;
/// Keys of the fixed-size tree of AC-H1: enough leaves to split a branch.
const FIXED_COUNT: u64 = 2000;

/// H1 on a record of a write transaction: no read of a page it changed checks the layout. Returns the pages changed
/// and the reads of them.
fn assert_no_read_checks_a_change(log: &[LayoutEvent]) -> (BTreeSet<u64>, usize) {
    let mut changed = BTreeSet::new();
    let mut reads = 0;
    for (at, &event) in log.iter().enumerate() {
        match event {
            LayoutEvent::Change(page) => {
                changed.insert(page);
            }
            LayoutEvent::Read(page) => {
                assert!(
                    changed.contains(&page),
                    "event {at}: a read of page {page} before its change"
                );
                reads += 1;
            }
            LayoutEvent::Check(page) => assert!(
                !changed.contains(&page),
                "event {at}: a read checked page {page}, which the transaction changed"
            ),
        }
    }
    (changed, reads)
}

/// The pages of `changed` that hold a node of either format after the commit, by their committed bytes: every one but
/// the store's own pages whose kind is a node's.
fn committed_nodes(store: &Store, changed: &BTreeSet<u64>) -> BTreeSet<u64> {
    let read = store.read().unwrap();
    changed
        .iter()
        .copied()
        .filter(|&page| page != META_PAGE && page != MAP_PAGE)
        .filter(|&page| matches!(read.inner.page(page).unwrap()[0], 1..=4))
        .collect()
}

/// Commits `write` and returns the nodes its commit checked, asserting each was checked once.
fn commit_checked(write: WriteTransaction<'_>) -> BTreeSet<u64> {
    commit_checks::start();
    write.commit().unwrap();
    let checked = commit_checks::take();
    let unique: BTreeSet<u64> = checked.iter().copied().collect();
    assert_eq!(unique.len(), checked.len(), "each node once: {checked:?}");
    unique
}

/// More entries than the node holds, with its kind and level right: only the layout check finds it.
fn damage_layout(write: &mut WriteTransaction<'_>, page: u64) {
    write.inner.page_mut(page).unwrap()[2..4].copy_from_slice(&u16::MAX.to_le_bytes());
}

/// AC-H1 (H1, H2): a write that creates a normal, a fixed-size and a multi-value tree and stores keys in order in
/// each, and in the subtree of a multi-value key, with leaf and branch splits, checks no changed node on a read, and
/// its commit checks once each page of a node it writes, of those trees and of the catalog, the nodes a split creates
/// and nobody reads again included; and what it stored reads back after the commit.
#[test]
fn ac_h1_each_changed_node_is_checked_once_at_commit() {
    on_both(|store| {
        let mut write = store.write().unwrap();
        write.inner.record_layout();
        let tree = write.open_tree_with("n", OpenMode::CreateNew).unwrap();
        for i in 0..COUNT {
            let value = i.to_string();
            write
                .set(&tree, &long_key(i, LONG), value.as_bytes())
                .unwrap();
        }
        let fixed_tree = write
            .open_fixed_tree_with("f", FIXED_LONG, OpenMode::CreateNew)
            .unwrap();
        for key in 0..FIXED_COUNT {
            write.set_fixed(&fixed_tree, key, &fixed_long(key)).unwrap();
        }
        let multi = write
            .open_multi_tree_with("m", OpenMode::CreateNew)
            .unwrap();
        for i in 0..COUNT {
            assert!(write.add(&multi, b"key", &long_key(i, LONG)).unwrap());
        }
        assert!(write.subtree_of(&multi, b"key").unwrap().is_some());
        for i in 0..COUNT {
            let value = i.to_string();
            assert_eq!(
                write.get(&tree, &long_key(i, LONG)).unwrap(),
                Some(value.as_bytes())
            );
            assert!(write.contains(&multi, b"key", &long_key(i, LONG)).unwrap());
        }
        for key in 0..FIXED_COUNT {
            assert_eq!(
                write.get_fixed(&fixed_tree, key).unwrap(),
                Some(&fixed_long(key)[..])
            );
        }
        let (changed, reads) = assert_no_read_checks_a_change(&write.inner.take_layout_log());
        assert!(
            reads > COUNT + FIXED_COUNT as usize,
            "{reads} reads of changed nodes"
        );
        let catalog = write.inner.slot(catalog::ROOT_SLOT).unwrap();
        let checked = commit_checked(write);
        assert_eq!(checked, committed_nodes(store, &changed));

        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        let fixed_tree = read.open_fixed_tree("f", FIXED_LONG).unwrap();
        let multi = read.open_multi_tree("m").unwrap();
        for i in 0..COUNT {
            let value = i.to_string();
            assert_eq!(
                read.get(&tree, &long_key(i, LONG)).unwrap(),
                Some(value.as_bytes())
            );
            assert!(read.contains(&multi, b"key", &long_key(i, LONG)).unwrap());
        }
        for key in 0..FIXED_COUNT {
            assert_eq!(
                read.get_fixed(&fixed_tree, key).unwrap(),
                Some(&fixed_long(key)[..])
            );
        }
        let subtree = read.subtree_of(&multi, b"key").unwrap().unwrap();
        assert!(checked.contains(&catalog), "the catalog's root");
        for (what, root, levels) in [
            (
                "normal tree",
                root_of(store, "n"),
                height(&read.inner, root_of(store, "n")).unwrap(),
            ),
            (
                "fixed-size tree",
                root_of(store, "f"),
                fixed::height(&read.inner, root_of(store, "f"), FIXED_LONG).unwrap(),
            ),
            ("subtree", subtree, height(&read.inner, subtree).unwrap()),
        ] {
            assert!(levels >= 3, "{what}: branches split, {levels} levels");
            assert!(checked.contains(&root), "{what}: its root");
        }
    });
}

/// AC-H1 (H2), with merges: a write that removes most keys of a tree, which merges its nodes and frees their pages,
/// checks at commit only the nodes it keeps.
#[test]
fn ac_h1_merged_nodes_are_checked_only_while_kept() {
    on_both(|store| {
        commit_tree(store, "n", COUNT, LONG);
        let mut write = store.write().unwrap();
        write.inner.record_layout();
        let tree = write.open_tree("n").unwrap();
        for i in 10..COUNT {
            assert!(write.remove(&tree, &long_key(i, LONG)).unwrap());
        }
        let (changed, _) = assert_no_read_checks_a_change(&write.inner.take_layout_log());
        // A page the transaction freed is no longer one it may read.
        let kept: BTreeSet<u64> = changed
            .iter()
            .copied()
            .filter(|&page| write.inner.may_hold_data(page))
            .collect();
        let checked = commit_checked(write);
        let free = store.info().free_pages;
        assert!(free > 10, "the merges freed {free} pages");
        // The commit also records the tree's new root in the catalog, after the operations.
        let mut expected = committed_nodes(store, &kept);
        let write = store.write().unwrap();
        expected.insert(write.inner.slot(catalog::ROOT_SLOT).unwrap());
        write.abort();
        assert_eq!(checked, expected);
        assert!(changed.len() > kept.len(), "freed pages are not checked");
    });
}

/// The value of `key` in the fixed-size tree of AC-H1.
fn fixed_long(key: u64) -> Vec<u8> {
    let mut value = key.to_le_bytes().to_vec();
    value.resize(FIXED_LONG, b'.');
    value
}

/// AC-H2 (H2): changed nodes with a damaged layout, which no later operation reads, fail the commit, durable or
/// deferred, with the damage class and the lowest of their pages; another transaction reads what was there before,
/// and the next write commits.
#[test]
fn ac_h2_a_changed_node_with_a_damaged_layout_fails_the_commit() {
    for deferred in [false, true] {
        on_both(|store| {
            commit_tree(store, "n", COUNT, LONG);
            let (first, last) = (long_key(0, LONG), long_key(COUNT - 1, LONG));
            let root = root_of(store, "n");
            let pages = [leaf(store, root, &first), leaf(store, root, &last)];
            assert_ne!(pages[0], pages[1]);

            let mut write = store.write().unwrap();
            let tree = write.open_tree("n").unwrap();
            write.set(&tree, &first, b"changed").unwrap();
            write.set(&tree, &last, b"changed").unwrap();
            for page in pages {
                damage_layout(&mut write, page);
            }
            let result = if deferred {
                write.commit_deferred()
            } else {
                write.commit()
            };
            assert_eq!(damaged_page(result), *pages.iter().min().unwrap());

            let read = store.read().unwrap();
            let tree = read.open_tree("n").unwrap();
            assert_eq!(read.get(&tree, &first).unwrap(), Some(&b"0"[..]));
            drop(read);
            let mut write = store.write().unwrap();
            let tree = write.open_tree("n").unwrap();
            write.set(&tree, &first, b"again").unwrap();
            write.commit().unwrap();
        });
    }
}

/// AC-H3 (H2, F8): a node changed in a page no tree refers to, damaged and then freed, is not checked, and the commit
/// goes through; kept, the same node fails the commit.
#[test]
fn ac_h3_a_freed_node_is_not_checked() {
    for free in [true, false] {
        on_both(|store| {
            let mut write = store.write().unwrap();
            let page = write.inner.allocate().unwrap();
            // A node whose layout check always fails: a damaged node.
            write.inner.node_mut(page, |_| false).unwrap();
            if free {
                write.inner.free(page, 1).unwrap();
                commit_checks::start();
                write.commit().unwrap();
                assert!(commit_checks::take().is_empty());
            } else {
                assert_eq!(damaged_page(write.commit()), page);
            }
        });
    }
}

/// AC-H4 (T11, H2): a write with a damaged changed node and a later operation that fails with another class fails its
/// commit with that class, without checking a node.
#[test]
fn ac_h4_an_earlier_failure_wins_over_the_check() {
    on_both(|store| {
        commit_tree(store, "n", 20, 64);
        let key = long_key(3, 64);
        let page = leaf(store, root_of(store, "n"), &key);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("n").unwrap();
        write.set(&tree, &key, b"changed").unwrap();
        damage_layout(&mut write, page);
        let misuse = write.inner.page_mut(META_PAGE).map(|_| ());
        assert_eq!(misuse.unwrap_err().kind(), ErrorKind::Misuse);
        commit_checks::start();
        assert_eq!(write.commit().unwrap_err().kind(), ErrorKind::Misuse);
        assert!(commit_checks::take().is_empty());
    });
}

/// AC-H5 (H2): an aborted write that changed nodes, and a commit that only reserves and frees pages, check no node.
#[test]
fn ac_h5_aborts_and_commits_without_nodes_check_nothing() {
    on_both(|store| {
        commit_tree(store, "n", 20, 64);
        commit_checks::start();
        let mut write = store.write().unwrap();
        let tree = write.open_tree("n").unwrap();
        write.set(&tree, &long_key(3, 64), b"changed").unwrap();
        write.abort();
        let mut write = store.write().unwrap();
        let page = write.inner.allocate().unwrap();
        write.inner.free(page, 1).unwrap();
        write.commit().unwrap();
        assert!(commit_checks::take().is_empty());
    });
}

/// AC-H6 (H1, C1): a changed node whose level, or kind, changes without breaking its layout fails its next read with
/// the damage class and its page.
#[test]
fn ac_h6_reads_of_changed_nodes_still_check_their_kind_and_level() {
    for (at, byte) in [(1, 1), (0, 3)] {
        on_both(|store| {
            commit_tree(store, "n", COUNT, LONG);
            let key = long_key(5, LONG);
            let root = root_of(store, "n");
            let page = leaf(store, root, &key);
            assert_ne!(page, root, "a leaf below a branch");
            let mut write = store.write().unwrap();
            let tree = write.open_tree("n").unwrap();
            write.set(&tree, &key, b"changed").unwrap();
            write.inner.page_mut(page).unwrap()[at] = byte;
            assert_eq!(damaged_page(write.get(&tree, &key)), page);
        });
    }
}
