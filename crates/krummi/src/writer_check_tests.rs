//! Layout checks of the nodes a write transaction changes (the writer checks PRD, S1: AC-S1 to AC-S3 and the layout
//! part of AC-S7), through the public API and the write transaction's record of the pages it changes, the reads of
//! those pages and the layout checks the tree code reports.

use std::collections::BTreeMap;

use krummi_tree::{fixed, height};
use krummi_tx::{LayoutEvent, Memory};

use super::*;
use crate::check_tests::{commit_tree, damaged_page, leaf, long_key, on_both, root_of, transfer};

/// Bytes of the keys, and of the multi-value tree's values, of AC-S1: a few fill a leaf and a branch, so a few hundred
/// split leaves and branches.
const LONG: usize = 1000;
/// Keys of the normal tree and values of the multi-value key of AC-S1.
const COUNT: usize = 200;
/// Length of the values of the fixed-size tree of AC-S1: three fill a leaf.
const FIXED_LONG: usize = fixed::MAX_LEN;
/// Keys of the fixed-size tree of AC-S1: enough leaves to split a branch.
const FIXED_COUNT: u64 = 2000;

/// What S1 asks of a record of a write transaction: every read of a page it changed checks the layout when the page
/// changed since its last check, and only then. Returns the reads that checked and the reads that did not.
fn assert_checked_once_per_change(log: &[LayoutEvent]) -> (usize, usize) {
    // For each page changed so far, whether it changed since its last check.
    let mut pending = BTreeMap::new();
    let (mut checks, mut skips) = (0, 0);
    let mut events = log.iter().enumerate();
    while let Some((at, &event)) = events.next() {
        match event {
            LayoutEvent::Change(page) => {
                pending.insert(page, true);
            }
            LayoutEvent::Read(page) => {
                let changed = pending.get_mut(&page).unwrap_or_else(|| {
                    panic!("event {at}: a read of page {page} before its change")
                });
                let checked = log.get(at + 1) == Some(&LayoutEvent::Check(page));
                assert_eq!(checked, *changed, "event {at}: page {page}");
                if checked {
                    events.next();
                    *changed = false;
                    checks += 1;
                } else {
                    skips += 1;
                }
            }
            LayoutEvent::Check(page) => assert!(
                !pending.contains_key(&page),
                "event {at}: a check of changed page {page} without its read"
            ),
        }
    }
    (checks, skips)
}

/// The layout checks of `page` in `log` after its first change.
fn checks_after_change(log: &[LayoutEvent], page: u64) -> usize {
    log.iter()
        .skip_while(|&&event| event != LayoutEvent::Change(page))
        .filter(|&&event| event == LayoutEvent::Check(page))
        .count()
}

/// What a write transaction recorded since it started recording, and the reads that checked and did not so far.
#[derive(Default)]
struct Record {
    log: Vec<LayoutEvent>,
    reads: (usize, usize),
}

impl Record {
    /// Adds what `write` recorded since the last step and asserts S1 on the whole record; for a load in order of
    /// `stored` entries since the last step, some reads checked and some did not.
    fn load(&mut self, write: &WriteTransaction<'_>, stored: usize, what: &str) {
        self.log.extend(write.inner.take_layout_log());
        let (checks, skips) = assert_checked_once_per_change(&self.log);
        assert!(
            checks - self.reads.0 >= stored,
            "{what}: the leaf of each entry after it changed"
        );
        assert!(
            skips > self.reads.1,
            "{what}: reads of unchanged nodes skip the check"
        );
        self.reads = (checks, skips);
    }
}

/// AC-S1 (S1): a write transaction that stores keys in order, in a normal tree, in a fixed-size tree and in the
/// subtree of a multi-value key, with leaf and branch splits, checks the layout of each changed node once after
/// each change, the nodes a split creates included, and never on a read without a change before it; and it reads and
/// commits what it stored.
#[test]
fn ac_s1_each_changed_node_is_checked_once_after_each_change() {
    on_both(|store| {
        let mut write = store.write().unwrap();
        write.inner.record_layout();
        let mut record = Record::default();
        let tree = write.open_tree_with("n", OpenMode::CreateNew).unwrap();
        for i in 0..COUNT {
            let value = i.to_string();
            write
                .set(&tree, &long_key(i, LONG), value.as_bytes())
                .unwrap();
        }
        record.load(&write, COUNT, "normal tree");

        let fixed_tree = write
            .open_fixed_tree_with("f", FIXED_LONG, OpenMode::CreateNew)
            .unwrap();
        for key in 0..FIXED_COUNT {
            write.set_fixed(&fixed_tree, key, &fixed_long(key)).unwrap();
        }
        record.load(&write, FIXED_COUNT as usize, "fixed-size tree");

        let multi = write
            .open_multi_tree_with("m", OpenMode::CreateNew)
            .unwrap();
        for i in 0..COUNT {
            assert!(write.add(&multi, b"key", &long_key(i, LONG)).unwrap());
        }
        assert!(write.subtree_of(&multi, b"key").unwrap().is_some());
        record.load(&write, COUNT, "subtree of a multi-value key");

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
        record.load(&write, 0, "reads");
        write.commit().unwrap();

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
    });
}

/// The value of `key` in the fixed-size tree of AC-S1.
fn fixed_long(key: u64) -> Vec<u8> {
    let mut value = key.to_le_bytes().to_vec();
    value.resize(FIXED_LONG, b'.');
    value
}

/// AC-S2 (S1, T11): a node a write just changed, whose content before the change is remembered as checked by the
/// store, with its layout damaged before its next read, also after a read that checked it: that read fails with the
/// damage class and its page, a second read fails the same, and the write cannot commit.
#[test]
fn ac_s2_a_changed_node_with_a_damaged_layout_fails_every_read() {
    for read_between in [false, true] {
        on_both(|store| {
            commit_tree(store, "n", 20, 64);
            transfer(store);
            let key = long_key(3, 64);
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

            let mut write = store.write().unwrap();
            let tree = write.open_tree("n").unwrap();
            write.set(&tree, &key, b"changed").unwrap();
            if read_between {
                assert_eq!(write.get(&tree, &key).unwrap(), Some(&b"changed"[..]));
            }
            // More entries than the node holds, with its kind and level right, so that only the layout check finds it:
            // damage the test writes through the page API before the next read.
            write.inner.page_mut(page).unwrap()[2..4].copy_from_slice(&u16::MAX.to_le_bytes());
            assert_eq!(damaged_page(write.get(&tree, &key)), page);
            assert_eq!(damaged_page(write.get(&tree, &key)), page);
            assert_eq!(damaged_page(write.commit()), page);
        });
    }
}

/// AC-S3 (S1): a node a write changed and checked, when the write commits, and in another case aborts, is checked
/// again by the next write that changes it and reads it.
#[test]
fn ac_s3_the_next_write_checks_a_changed_node_again() {
    for commit in [true, false] {
        on_both(|store| {
            commit_tree(store, "n", 20, 64);
            let key = long_key(3, 64);
            let page = leaf(store, root_of(store, "n"), &key);

            let mut write = store.write().unwrap();
            let tree = write.open_tree("n").unwrap();
            write.inner.record_layout();
            write.set(&tree, &key, b"first").unwrap();
            for _ in 0..2 {
                assert_eq!(write.get(&tree, &key).unwrap(), Some(&b"first"[..]));
            }
            let log = write.inner.take_layout_log();
            assert_checked_once_per_change(&log);
            assert_eq!(checks_after_change(&log, page), 1, "checked once");
            if commit {
                write.commit().unwrap();
            } else {
                write.abort();
            }

            let mut write = store.write().unwrap();
            let tree = write.open_tree("n").unwrap();
            write.inner.record_layout();
            write.set(&tree, &key, b"second").unwrap();
            assert_eq!(write.get(&tree, &key).unwrap(), Some(&b"second"[..]));
            let log = write.inner.take_layout_log();
            assert_checked_once_per_change(&log);
            assert_eq!(
                checks_after_change(&log, page),
                1,
                "commit {commit}: checked again"
            );
            write.commit().unwrap();
            assert_eq!(
                leaf(store, root_of(store, "n"), &key),
                page,
                "the same page"
            );
        });
    }
}

/// AC-S7, the part of the layout (S1, C3): a committed version of a node still in the scratch area, read twice by a
/// write that does not change it, has its layout checked on each read, and not its integrity.
#[test]
fn ac_s7_a_version_in_the_scratch_area_is_checked_on_every_read_of_a_write() {
    on_both(|store| {
        commit_tree(store, "n", 20, 64);
        transfer(store);
        let key = long_key(3, 64);
        let page = leaf(store, root_of(store, "n"), &key);
        assert_eq!(root_of(store, "n"), page, "a tree of one leaf");
        let keeper = store.read().unwrap();
        let mut write = store.write().unwrap();
        let tree = write.open_tree("n").unwrap();
        write.set(&tree, &key, b"changed").unwrap();
        write.commit().unwrap();
        transfer(store);

        let write = store.write().unwrap();
        let tree = write.open_tree("n").unwrap();
        assert_eq!(
            store.engine.memory_of(write.inner.page(page).unwrap()),
            Some(Memory::Versions),
            "kept in the scratch area by the read opened before it"
        );
        let integrity = store.engine.integrity_checks();
        write.inner.record_layout();
        for _ in 0..2 {
            assert_eq!(write.get(&tree, &key).unwrap(), Some(&b"changed"[..]));
        }
        let log = write.inner.take_layout_log();
        assert_eq!(
            log,
            [LayoutEvent::Check(page), LayoutEvent::Check(page)],
            "its layout on each read"
        );
        assert_eq!(
            store.engine.integrity_checks(),
            integrity,
            "not its integrity"
        );
        drop((write, keeper));
    });
}
