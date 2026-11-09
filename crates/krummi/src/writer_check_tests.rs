//! Layout checks of the nodes a write transaction changes (the commit checks PRD, H1 and H2: AC-H1 to AC-H3), through
//! the public API, the write transaction's record of the pages it changes and reads, and the count of the layout
//! checks of the scratch area's versions. AC-S4 to AC-S6 of the writer checks PRD, about the integrity of what a write
//! reads (S2), are with the remembered page integrity tests.

use std::collections::BTreeSet;

use krummi_tree::{fixed, height};
use krummi_tx::LayoutEvent;

use super::*;
use crate::check_tests::{commit_tree, damaged_page, leaf, long_key, on_both, root_of, transfer};

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

/// More entries than the node holds, with its kind and level right: only the layout check finds it.
fn damage_layout(write: &mut WriteTransaction<'_>, page: u64) {
    write.inner.page_mut(page).unwrap()[2..4].copy_from_slice(&u16::MAX.to_le_bytes());
}

/// The value of `key` in the fixed-size tree of AC-H1.
fn fixed_long(key: u64) -> Vec<u8> {
    let mut value = key.to_le_bytes().to_vec();
    value.resize(FIXED_LONG, b'.');
    value
}

/// AC-H1 (H1): a write that creates a normal, a fixed-size and a multi-value tree and stores keys in order in each,
/// and in the subtree of a multi-value key, with leaf and branch splits, checks no changed node on a read; what it
/// stored reads back after the commit, and that read checks the nodes the write committed.
#[test]
fn ac_h1_a_write_never_checks_the_nodes_it_changed() {
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
        let (_, reads) = assert_no_read_checks_a_change(&write.inner.take_layout_log());
        assert!(
            reads > COUNT + FIXED_COUNT as usize,
            "{reads} reads of changed nodes"
        );
        write.commit().unwrap();
        let before = store.engine.version_checks();

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
        for (what, levels) in [
            (
                "normal tree",
                height(&read.inner, root_of(store, "n")).unwrap(),
            ),
            (
                "fixed-size tree",
                fixed::height(&read.inner, root_of(store, "f"), FIXED_LONG).unwrap(),
            ),
            ("subtree", height(&read.inner, subtree).unwrap()),
        ] {
            assert!(levels >= 3, "{what}: branches split, {levels} levels");
        }
        // The later transaction checks what the write committed, on its first read of each version (U1).
        assert!(
            store.engine.version_checks() > before,
            "the committed nodes are checked when read"
        );
    });
}

/// AC-H1 (H1), with merges: a write that removes most keys of a tree, which merges its nodes and frees their pages,
/// checks none of the nodes it changed, and commits.
#[test]
fn ac_h1_merges_check_no_changed_node() {
    on_both(|store| {
        commit_tree(store, "n", COUNT, LONG);
        let mut write = store.write().unwrap();
        write.inner.record_layout();
        let tree = write.open_tree("n").unwrap();
        for i in 10..COUNT {
            assert!(write.remove(&tree, &long_key(i, LONG)).unwrap());
        }
        let (changed, reads) = assert_no_read_checks_a_change(&write.inner.take_layout_log());
        assert!(
            changed.len() > 10 && reads > 0,
            "{} changed, {reads} reads",
            changed.len()
        );
        write.commit().unwrap();
        assert!(store.info().free_pages > 10, "the merges freed pages");
    });
}

/// AC-H2 (H1, H2, H3): a node whose content before the change is remembered as checked by the store, changed by a
/// write and damaged in its layout without a later operation of the write reading it, is committed unchecked, durable
/// or deferred; two reads of it by later transactions fail with the damage class and its page, before and after the
/// transfer to the store. The node is a leaf below a branch, and the root of a tree of one leaf. Where a snapshot keeps
/// the version, the reads before the transfer come from the scratch area; where nothing does, the transfer may already
/// have moved it, so those reads come from either place.
#[test]
fn ac_h2_a_damaged_changed_node_fails_its_first_read_after_the_commit() {
    for (deferred, entries, held) in [
        (false, COUNT, true),
        (true, COUNT, true),
        (false, 1, true),
        (true, 1, true),
        (false, COUNT, false),
        (true, 1, false),
    ] {
        on_both(|store| {
            commit_tree(store, "n", entries, LONG);
            transfer(store);
            let key = long_key(0, LONG);
            let before = store.engine.checked_remembered();
            let read = store.read().unwrap();
            let tree = read.open_tree("n").unwrap();
            assert!(read.get(&tree, &key).unwrap().is_some());
            drop(read);
            assert!(
                store.engine.checked_remembered() > before,
                "its content before the change is remembered"
            );
            let page = leaf(store, root_of(store, "n"), &key);
            // A snapshot that keeps the damaged version in the scratch area until the transfer below; without it,
            // the transfer may move it to the store at any time.
            let mut keeper = held.then(|| store.read().unwrap());
            let mut write = store.write().unwrap();
            let tree = write.open_tree("n").unwrap();
            write.set(&tree, &key, b"changed").unwrap();
            damage_layout(&mut write, page);
            if deferred {
                write.commit_deferred().unwrap();
                store.write().unwrap().commit().unwrap();
            } else {
                write.commit().unwrap();
            }
            for transferred in [false, true] {
                if transferred {
                    drop(keeper.take());
                    transfer(store);
                }
                for _ in 0..2 {
                    let read = store.read().unwrap();
                    let tree = read.open_tree("n").unwrap();
                    assert_eq!(
                        damaged_page(read.get(&tree, &key)),
                        page,
                        "transferred {transferred}"
                    );
                }
            }
        });
    }
}

/// AC-H3 (H1, C1, T11): a changed node whose level, or kind, changes without breaking its layout fails its next read
/// with the damage class and its page, and the write cannot commit.
#[test]
fn ac_h3_reads_of_changed_nodes_still_check_their_kind_and_level() {
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
            assert_eq!(damaged_page(write.commit()), page);
        });
    }
}

/// Debug builds also check the layout of the nodes a write changed, on every read, like Voron's `VALIDATE` (annotation
/// of 2026-11-09 to H1): a changed node whose layout breaks fails the write's next read of it with the damage class
/// and its page, without a check event in the write's record (AC-H1 counts the checks release builds make).
#[cfg(debug_assertions)]
#[test]
fn debug_builds_check_the_layout_of_the_nodes_a_write_changed() {
    on_both(|store| {
        commit_tree(store, "n", COUNT, LONG);
        let key = long_key(5, LONG);
        let page = leaf(store, root_of(store, "n"), &key);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("n").unwrap();
        write.set(&tree, &key, b"changed").unwrap();
        write.inner.record_layout();
        damage_layout(&mut write, page);
        assert_eq!(damaged_page(write.get(&tree, &key)), page);
        assert_no_read_checks_a_change(&write.inner.take_layout_log());
    });
}
