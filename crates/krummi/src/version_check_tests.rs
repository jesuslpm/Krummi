//! Remembered layout checks of the versions of the scratch area (the version checks PRD, AC-U1 to AC-U7), through
//! the public API and the engine's test hooks: how many versions the tree code checks, and where pages live.

use krummi_tx::{LayoutEvent, Memory};

use super::check_tests::{
    FIXED_LEN, commit_tree, concurrent_reads_while_a_write_moves_nodes, damage_layout,
    damaged_page, fixed_value, leaf, long_key, on_both, root_of, transfer, value16,
};
use super::*;

/// Layout checks of versions of the scratch area so far.
fn checks(store: &Store) -> u64 {
    store.engine.version_checks()
}

/// Reads key `i` of every tree of [`commit_trees`] in `read`, a read or a write transaction, and checks it.
macro_rules! read_all {
    ($read:expr, $i:expr) => {{
        let (read, i) = (&$read, $i);
        let normal = read.open_tree("n").unwrap();
        assert_eq!(
            read.get(&normal, &long_key(i, 64)).unwrap(),
            Some(i.to_string().as_bytes())
        );
        let fixed = read.open_fixed_tree("f", FIXED_LEN).unwrap();
        assert_eq!(
            read.get_fixed(&fixed, i as u64).unwrap(),
            Some(&fixed_value(i as u64)[..])
        );
        let multi = read.open_multi_tree("m").unwrap();
        assert!(read.contains(&multi, b"key", &value16(i)).unwrap());
    }};
}

/// Commits a normal tree `n` of 200 keys, a fixed-size tree `f` of 2,000 keys and a multi-value tree `m` whose key
/// `key` holds 300 values in its subtree, each of several nodes.
fn commit_trees(store: &Store) {
    commit_tree(store, "n", 200, 64);
    let mut write = store.write().unwrap();
    let fixed = write
        .open_fixed_tree_with("f", FIXED_LEN, OpenMode::CreateNew)
        .unwrap();
    for key in 0..2_000u64 {
        write.set_fixed(&fixed, key, &fixed_value(key)).unwrap();
    }
    let multi = write
        .open_multi_tree_with("m", OpenMode::CreateNew)
        .unwrap();
    for i in 0..300 {
        write.add(&multi, b"key", &value16(i)).unwrap();
    }
    assert!(write.subtree_of(&multi, b"key").unwrap().is_some());
    write.commit().unwrap();
}

/// AC-U1 (U1): versions of the nodes of a normal tree, a fixed-size tree and a multi-value subtree, kept in the
/// scratch area by a read opened before them, are checked on their first read only: neither the same read
/// transaction, nor another one, nor a write transaction checks them again, and none checks their integrity. It
/// replaces AC-S7.
#[test]
fn ac_u1_a_version_in_the_scratch_area_is_checked_on_its_first_read_only() {
    on_both(|store| {
        let keeper = store.read().unwrap();
        commit_trees(store);
        transfer(store);
        let page = leaf(store, root_of(store, "n"), &long_key(7, 64));
        let integrity = store.engine.integrity_checks();
        let before = checks(store);
        let read = store.read().unwrap();
        assert_eq!(
            store.engine.memory_of(read.inner.page(page).unwrap()),
            Some(Memory::Versions),
            "kept in the scratch area by the keeper"
        );
        read_all!(read, 7);
        let first = checks(store);
        assert!(
            first > before,
            "the first read checks the versions it reads"
        );
        read_all!(read, 7);
        drop(read);
        let read = store.read().unwrap();
        read_all!(read, 7);
        drop(read);
        let write = store.write().unwrap();
        write.inner.record_layout();
        read_all!(write, 7);
        read_all!(write, 7);
        assert!(
            !write
                .inner
                .take_layout_log()
                .iter()
                .any(|event| matches!(event, LayoutEvent::Check(_))),
            "the write checks no remembered version"
        );
        drop(write);
        assert_eq!(checks(store), first, "no later read checks them again");
        assert_eq!(
            store.engine.integrity_checks(),
            integrity,
            "nor their integrity"
        );
        drop(keeper);
    });
}

/// AC-U2 (U1, T11): a version with a damaged layout fails every read, by read transactions and by a write, which
/// cannot commit.
#[test]
fn ac_u2_a_damaged_version_fails_every_read_and_the_write_cannot_commit() {
    on_both(|store| {
        commit_tree(store, "n", 20, 64);
        transfer(store);
        let key = long_key(3, 64);
        let page = leaf(store, root_of(store, "n"), &key);
        let keeper = store.read().unwrap();
        damage_layout(store, page);
        transfer(store);
        let before = checks(store);
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert_eq!(damaged_page(read.get(&tree, &key)), page);
        assert_eq!(damaged_page(read.get(&tree, &key)), page);
        drop(read);
        let write = store.write().unwrap();
        let tree = write.open_tree("n").unwrap();
        assert_eq!(damaged_page(write.get(&tree, &key)), page);
        assert_eq!(damaged_page(write.commit()), page);
        assert_eq!(checks(store), before + 3, "each read checked it");
        drop(keeper);
    });
}

/// AC-U3 (U1, G3): a read opened before a newer version keeps reading the remembered one without checking it again,
/// and the first read of the newer version checks it once.
#[test]
fn ac_u3_a_newer_version_is_checked_and_the_older_one_stays_remembered() {
    on_both(|store| {
        let keeper = store.read().unwrap();
        commit_tree(store, "n", 1, 64);
        let key = long_key(0, 64);
        let old = store.read().unwrap();
        let tree = old.open_tree("n").unwrap();
        assert_eq!(old.get(&tree, &key).unwrap(), Some(&b"0"[..]));
        let mut write = store.write().unwrap();
        let changed = write.open_tree("n").unwrap();
        write.set(&changed, &key, b"new").unwrap();
        write.commit().unwrap();
        let before = checks(store);
        assert_eq!(old.get(&tree, &key).unwrap(), Some(&b"0"[..]));
        assert_eq!(checks(store), before, "the older version stays remembered");
        let new = store.read().unwrap();
        let tree = new.open_tree("n").unwrap();
        assert_eq!(new.get(&tree, &key).unwrap(), Some(&b"new"[..]));
        assert_eq!(
            checks(store),
            before + 1,
            "the newer version of the one-leaf tree is checked once"
        );
        assert_eq!(new.get(&tree, &key).unwrap(), Some(&b"new"[..]));
        assert_eq!(checks(store), before + 1);
        drop((new, old, keeper));
    });
}

/// AC-U4 (U1): a remembered version whose page is freed and reused for another node does not lend its pass to it.
#[test]
fn ac_u4_a_reused_page_is_checked_again() {
    on_both(|store| {
        let keeper = store.read().unwrap();
        commit_tree(store, "a", 1, 64);
        let page = root_of(store, "a");
        let read = store.read().unwrap();
        let tree = read.open_tree("a").unwrap();
        assert!(read.get(&tree, &long_key(0, 64)).unwrap().is_some());
        drop(read);
        let mut write = store.write().unwrap();
        write.delete_tree("a").unwrap();
        write.commit().unwrap();
        // A later write that reserves a page reuses the freed one.
        let reused = (0..20)
            .map(|round| format!("b{round}"))
            .find(|name| {
                commit_tree(store, name, 1, 64);
                root_of(store, name) == page
            })
            .expect("the freed page is reused");
        let before = checks(store);
        let read = store.read().unwrap();
        let tree = read.open_tree(&reused).unwrap();
        assert!(read.get(&tree, &long_key(0, 64)).unwrap().is_some());
        assert_eq!(
            checks(store),
            before + 1,
            "the new node at that page is checked"
        );
        drop((read, keeper));
    });
}

/// AC-U5 (U1, S1): a write that copies a remembered version to change it checks the copy on its next read.
#[test]
fn ac_u5_a_copy_of_a_remembered_version_is_checked() {
    on_both(|store| {
        let keeper = store.read().unwrap();
        commit_tree(store, "n", 1, 64);
        let page = root_of(store, "n");
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert!(read.get(&tree, &long_key(0, 64)).unwrap().is_some());
        drop(read);
        let mut write = store.write().unwrap();
        write.inner.page_mut(page).unwrap();
        write.inner.record_layout();
        let tree = write.open_tree("n").unwrap();
        assert!(write.get(&tree, &long_key(0, 64)).unwrap().is_some());
        assert_eq!(
            write.inner.take_layout_log(),
            [LayoutEvent::Read(page), LayoutEvent::Check(page)]
        );
        drop((write, keeper));
    });
}

/// AC-U6 (U1, C2): once the version leaves the scratch area, a new read of the node from the store's pages checks
/// it on its first read only, in the shared memory.
#[test]
fn ac_u6_after_the_version_leaves_the_node_is_checked_from_the_store_once() {
    on_both(|store| {
        let keeper = store.read().unwrap();
        commit_tree(store, "n", 1, 64);
        let page = root_of(store, "n");
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert!(read.get(&tree, &long_key(0, 64)).unwrap().is_some());
        drop((read, keeper));
        transfer(store);
        let read = store.read().unwrap();
        assert_eq!(
            store.engine.memory_of(read.inner.page(page).unwrap()),
            Some(Memory::Data)
        );
        let (versions, remembered) = (checks(store), store.engine.checked_remembered());
        let tree = read.open_tree("n").unwrap();
        assert!(read.get(&tree, &long_key(0, 64)).unwrap().is_some());
        let after_first = store.engine.checked_remembered();
        // The catalog's root, read to open the tree, and the tree's root, each checked once.
        assert_eq!(after_first, remembered + 2, "checked from the store");
        assert!(read.get(&tree, &long_key(0, 64)).unwrap().is_some());
        assert_eq!(checks(store), versions, "no version read");
        assert_eq!(
            store.engine.checked_remembered(),
            after_first,
            "and not again"
        );
        drop(read);
    });
}

/// AC-U7 (U1, U3): four threads read nodes whose versions are in the scratch area, one of them damaged, while a
/// write commits newer versions and the transfer writes them: every read returns its snapshot's data and the damaged
/// version fails every read. The number of checks is not counted.
#[test]
fn ac_u7_concurrent_reads_of_versions_while_a_write_moves_them() {
    let tmp = tempfile::tempdir().unwrap();
    concurrent_reads_while_a_write_moves_nodes(
        [
            Store::open(tmp.path(), &StoreOptions::default()).unwrap(),
            Store::open_temporary(&StoreOptions::default()).unwrap(),
        ],
        |store, page| damage_layout(store, page),
    );
}
