//! Remembered node layout checks (the remembered node layout check PRD, AC-C1 to AC-C9), through the public API and
//! the engine's test hooks: how many contents the engine remembers, where pages live, and damage that appears while
//! the store is open.

use std::thread;

use krummi_tree::{catalog, leaf_of};

use super::*;

/// A value of 16 bytes.
fn value16(i: usize) -> Vec<u8> {
    format!("{i:016}").into_bytes()
}

/// A key of `len` bytes that starts with `i`, so that a few of them fill a leaf.
fn long_key(i: usize, len: usize) -> Vec<u8> {
    let mut key = format!("{i:04}").into_bytes();
    key.resize(len, b'.');
    key
}

/// Runs `check` with a store on disk and with a temporary one.
fn on_both(check: impl Fn(&mut Store)) {
    let tmp = tempfile::tempdir().unwrap();
    for mut store in [
        Store::open(tmp.path(), &StoreOptions::default()).unwrap(),
        Store::open_temporary(&StoreOptions::default()).unwrap(),
    ] {
        check(&mut store);
        store.close().unwrap();
    }
}

/// Contents the store remembers as checked so far.
fn remembered(store: &Store) -> u64 {
    store.engine.checked_remembered()
}

/// Writes every committed version into the store's pages, out of the scratch area.
fn transfer(store: &Store) {
    assert!(store.engine.transfer_now());
}

/// The root page of tree `name`.
fn root_of(store: &Store, name: &str) -> u64 {
    let read = store.read().unwrap();
    catalog::lookup(&read.inner, name).unwrap().unwrap().root
}

/// The leaf of `key` in the tree with root `root`.
fn leaf(store: &Store, root: u64, key: &[u8]) -> u64 {
    let read = store.read().unwrap();
    leaf_of(&read.inner, root, key).unwrap()
}

/// The page a damage error names.
fn damaged_page<T: std::fmt::Debug>(result: Result<T>) -> u64 {
    match result {
        Err(Error::Damaged {
            page: Some(page), ..
        }) => page,
        other => panic!("expected damage, got {other:?}"),
    }
}

/// Commits the normal tree `name` with `count` keys of `len` bytes, the value of each key its number in text.
fn commit_tree(store: &Store, name: &str, count: usize, len: usize) {
    let mut write = store.write().unwrap();
    let tree = write.open_tree_with(name, OpenMode::CreateNew).unwrap();
    for i in 0..count {
        write
            .set(&tree, &long_key(i, len), i.to_string().as_bytes())
            .unwrap();
    }
    write.commit().unwrap();
}

/// Commits a write that makes the node at `page` an unknown kind, keeping its integrity code right.
fn damage_layout(store: &Store, page: u64) {
    let mut write = store.write().unwrap();
    write.inner.page_mut(page).unwrap()[0] = 0xEE;
    write.commit().unwrap();
}

/// AC-C1 (C1, C2, C4): a normal tree, the catalog and the subtree of a multi-value key, in the store's pages, are
/// checked once per node: the first read of each node remembers it, and later reads of the same content, in the
/// same read transaction or another one, return the same and remember nothing more.
#[test]
fn ac_c1_each_node_is_checked_once_while_the_store_is_open() {
    on_both(|store| {
        commit_tree(store, "n", 200, 64);
        let mut write = store.write().unwrap();
        let tree = write
            .open_multi_tree_with("m", OpenMode::CreateNew)
            .unwrap();
        for i in 0..300 {
            write.add(&tree, b"key", &value16(i)).unwrap();
        }
        assert!(write.subtree_of(&tree, b"key").unwrap().is_some());
        write.commit().unwrap();
        transfer(store);

        let before = remembered(store);
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert_eq!(
            read.get(&tree, &long_key(150, 64)).unwrap(),
            Some(&b"150"[..])
        );
        let after_tree = remembered(store);
        assert!(after_tree > before, "the catalog and the tree's path");
        let multi = read.open_multi_tree("m").unwrap();
        assert!(read.contains(&multi, b"key", &value16(250)).unwrap());
        let after_multi = remembered(store);
        assert!(
            after_multi > after_tree,
            "the multi-value tree and its subtree"
        );
        for _ in 0..3 {
            assert_eq!(
                read.get(&tree, &long_key(150, 64)).unwrap(),
                Some(&b"150"[..])
            );
            assert!(read.contains(&multi, b"key", &value16(250)).unwrap());
        }
        drop(read);
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        let multi = read.open_multi_tree("m").unwrap();
        assert_eq!(
            read.get(&tree, &long_key(150, 64)).unwrap(),
            Some(&b"150"[..])
        );
        assert!(read.contains(&multi, b"key", &value16(250)).unwrap());
        assert_eq!(remembered(store), after_multi, "nothing checked again");
    });
}

/// AC-C2 (C1, C2, C3, C8, G3): a remembered node that a write changes: a read opened before still sees the content it
/// remembered; the new version, while in the scratch area, is checked on every read and never remembered; once
/// transferred to the same page, a new read sees it and checks it once.
#[test]
fn ac_c2_a_changed_node_is_checked_again_once_in_the_store() {
    on_both(|store| {
        commit_tree(store, "n", 20, 64);
        transfer(store);
        let key = long_key(7, 64);
        let old = store.read().unwrap();
        let tree = old.open_tree("n").unwrap();
        assert_eq!(old.get(&tree, &key).unwrap(), Some(&b"7"[..]));
        let leaf_page = leaf(store, root_of(store, "n"), &key);

        let mut write = store.write().unwrap();
        let wtree = write.open_tree("n").unwrap();
        write.set(&wtree, &key, b"changed").unwrap();
        write.commit().unwrap();
        transfer(store);
        assert_eq!(
            old.get(&tree, &key).unwrap(),
            Some(&b"7"[..]),
            "its snapshot"
        );
        let before = remembered(store);
        let new = store.read().unwrap();
        let new_tree = new.open_tree("n").unwrap();
        for _ in 0..2 {
            assert_eq!(new.get(&new_tree, &key).unwrap(), Some(&b"changed"[..]));
        }
        assert_eq!(
            remembered(store),
            before,
            "the scratch version is not remembered"
        );
        drop((old, new));

        transfer(store);
        let before = remembered(store);
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert_eq!(read.get(&tree, &key).unwrap(), Some(&b"changed"[..]));
        let checked = remembered(store);
        assert!(checked > before, "the new content is checked");
        assert_eq!(read.get(&tree, &key).unwrap(), Some(&b"changed"[..]));
        assert_eq!(remembered(store), checked, "and only once");
        drop(read);
        assert_eq!(
            leaf(store, root_of(store, "n"), &key),
            leaf_page,
            "the same page"
        );
    });
}

/// AC-C3 (C1, C2): the page of a remembered node, freed and reused for another node with another content, reads that
/// content and checks it once.
#[test]
fn ac_c3_a_reused_page_is_checked_for_its_new_content() {
    on_both(|store| {
        commit_tree(store, "a", 1, 16);
        transfer(store);
        let page = root_of(store, "a");
        let read = store.read().unwrap();
        let tree = read.open_tree("a").unwrap();
        assert_eq!(read.get(&tree, &long_key(0, 16)).unwrap(), Some(&b"0"[..]));
        drop(read);

        let mut write = store.write().unwrap();
        write.delete_tree("a").unwrap();
        write.commit().unwrap();
        transfer(store);
        let mut write = store.write().unwrap();
        let tree = write.open_tree_with("b", OpenMode::CreateNew).unwrap();
        write.set(&tree, b"other", b"content").unwrap();
        write.commit().unwrap();
        transfer(store);
        assert_eq!(root_of(store, "b"), page, "the freed page is reused");

        let before = remembered(store);
        let read = store.read().unwrap();
        let tree = read.open_tree("b").unwrap();
        assert_eq!(read.get(&tree, b"other").unwrap(), Some(&b"content"[..]));
        let checked = remembered(store);
        assert!(checked > before);
        assert_eq!(read.get(&tree, b"other").unwrap(), Some(&b"content"[..]));
        assert_eq!(remembered(store), checked);
    });
}

/// AC-C4 (C1, C2, A9): a node in the store's pages with a damaged layout and a right integrity code fails with the
/// damage class and its page on every read, in the same read transaction and in another one, and is never
/// remembered; also when a right version of that page was remembered before.
#[test]
fn ac_c4_a_damaged_layout_in_the_store_fails_every_time() {
    on_both(|store| {
        commit_tree(store, "n", 20, 64);
        transfer(store);
        let key = long_key(3, 64);
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert!(
            read.get(&tree, &key).unwrap().is_some(),
            "the right version is remembered"
        );
        drop(read);
        let page = leaf(store, root_of(store, "n"), &key);

        damage_layout(store, page);
        transfer(store);
        let before = remembered(store);
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert_eq!(damaged_page(read.get(&tree, &key)), page);
        assert_eq!(damaged_page(read.get(&tree, &key)), page);
        drop(read);
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert_eq!(damaged_page(read.get(&tree, &key)), page);
        assert_eq!(
            remembered(store),
            before,
            "a damaged layout is never remembered"
        );
    });
}

/// AC-C5 (C3): the same damage in a version still in the scratch area, kept there by a read opened before it, fails
/// on both reads.
#[test]
fn ac_c5_a_damaged_layout_in_the_scratch_area_fails_every_time() {
    on_both(|store| {
        commit_tree(store, "n", 20, 64);
        transfer(store);
        let key = long_key(3, 64);
        let page = leaf(store, root_of(store, "n"), &key);
        let keeper = store.read().unwrap();
        damage_layout(store, page);
        transfer(store);
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert_eq!(damaged_page(read.get(&tree, &key)), page);
        assert_eq!(damaged_page(read.get(&tree, &key)), page);
        drop((read, keeper));
    });
}

/// AC-C6 (C1, M11): a multi-value leaf in the store's pages whose layout is right and remembered but whose entry is
/// inconsistent fails with the damage class and its page each time an operation uses it.
#[test]
fn ac_c6_an_inconsistent_entry_fails_although_its_layout_is_remembered() {
    on_both(|store| {
        let key = b"a multi-value key";
        let mut write = store.write().unwrap();
        let tree = write
            .open_multi_tree_with("m", OpenMode::CreateNew)
            .unwrap();
        for value in [&b"a"[..], b"b", b"c"] {
            write.add(&tree, key, value).unwrap();
        }
        write.commit().unwrap();
        let page = leaf(store, root_of(store, "m"), key);
        let mut write = store.write().unwrap();
        let node = write.inner.page_mut(page).unwrap();
        let at = node
            .windows(key.len())
            .position(|w| w == key)
            .expect("the key in its leaf");
        // The first byte of the entry's blob, after the key: an unknown tag.
        node[at + key.len()] = 7;
        write.commit().unwrap();
        transfer(store);

        let before = remembered(store);
        let read = store.read().unwrap();
        let tree = read.open_multi_tree("m").unwrap();
        assert_eq!(damaged_page(read.value_count(&tree, key)), page);
        let checked = remembered(store);
        assert!(checked > before, "its layout is right and remembered");
        assert_eq!(damaged_page(read.value_count(&tree, key)), page);
        assert_eq!(remembered(store), checked);
    });
}

/// AC-C7 (C2, C7): a byte of a remembered node changed in the store's pages without its integrity code, while the
/// store is open, makes the next read fail with the damage class and its page.
#[test]
fn ac_c7_a_byte_changed_in_a_remembered_node_is_detected() {
    on_both(|store| {
        commit_tree(store, "n", 200, 64);
        transfer(store);
        let key = long_key(3, 64);
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert!(read.get(&tree, &key).unwrap().is_some());
        drop(read);
        let page = leaf(store, root_of(store, "n"), &key);

        store.engine.flip_byte_in_place(page, 100).unwrap();
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert_eq!(damaged_page(read.get(&tree, &key)), page);
        drop(read);

        // A byte of a key, which keeps the layout valid: only the integrity code can catch it.
        let other = long_key(190, 64);
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        let other_page = leaf(store, root_of(store, "n"), &other);
        assert_ne!(other_page, page, "another leaf, still intact");
        assert!(read.get(&tree, &other).unwrap().is_some(), "remembered");
        let at = read
            .inner
            .page(other_page)
            .unwrap()
            .windows(other.len())
            .position(|w| w == other)
            .expect("the key in its leaf");
        drop(read);
        store
            .engine
            .flip_byte_in_place(other_page, at + 10)
            .unwrap();
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert_eq!(damaged_page(read.get(&tree, &other)), other_page);
    });
}

/// AC-C8 (C5, C6): with a table of four slots, the nodes of a tree of several leaves and a damaged node compete for
/// the same slots while four threads read them at once, at least a thousand times each, and a write changes them and
/// transfers them: every read returns the data of its snapshot, and the damaged node fails on every read.
#[test]
fn ac_c8_concurrent_reads_with_competing_slots() {
    const KEYS: usize = 24;
    const GENERATIONS: usize = 20;
    let tmp = tempfile::tempdir().unwrap();
    let stores = krummi_pager::testing::with_checked_slots(4, || {
        [
            Store::open(tmp.path(), &StoreOptions::default()).unwrap(),
            Store::open_temporary(&StoreOptions::default()).unwrap(),
        ]
    });
    for store in stores {
        assert_eq!(
            store.engine.checked_slots(),
            4,
            "the stores got the test's table"
        );
        let key = |i: usize| long_key(i, 1500);
        let mut write = store.write().unwrap();
        let good = write.open_tree_with("good", OpenMode::CreateNew).unwrap();
        for i in 0..KEYS {
            write.set(&good, &key(i), b"g0").unwrap();
        }
        let bad = write.open_tree_with("bad", OpenMode::CreateNew).unwrap();
        write.set(&bad, b"bad", b"bad").unwrap();
        write.commit().unwrap();
        let bad_page = root_of(&store, "bad");
        damage_layout(&store, bad_page);
        transfer(&store);

        thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    let mut node_reads = 0;
                    while node_reads < 1000 {
                        let read = store.read().unwrap();
                        let good = read.open_tree("good").unwrap();
                        let bad = read.open_tree("bad").unwrap();
                        let first = read.get(&good, &key(0)).unwrap().unwrap().to_vec();
                        for i in 1..KEYS {
                            assert_eq!(
                                read.get(&good, &key(i)).unwrap(),
                                Some(&first[..]),
                                "one snapshot"
                            );
                        }
                        assert_eq!(damaged_page(read.get(&bad, b"bad")), bad_page);
                        // Opening each tree reads the catalog's node; each Get of `good` reads its root
                        // branch and a leaf, and the Get of `bad` its one damaged leaf.
                        node_reads += 2 + 2 * KEYS + 1;
                    }
                });
            }
            for generation in 1..=GENERATIONS {
                let mut write = store.write().unwrap();
                let good = write.open_tree("good").unwrap();
                let value = format!("g{generation}");
                for i in 0..KEYS {
                    write.set(&good, &key(i), value.as_bytes()).unwrap();
                }
                write.commit().unwrap();
                transfer(&store);
            }
        });
        store.close().unwrap();
    }
}

/// AC-C9 (C4): reopening a store on disk forgets what it remembered: the first read of each node checks it again.
#[test]
fn ac_c9_reopening_forgets_the_remembered_nodes() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    commit_tree(&store, "n", 200, 64);
    transfer(&store);
    let read = |store: &Store| {
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert_eq!(
            read.get(&tree, &long_key(150, 64)).unwrap(),
            Some(&b"150"[..])
        );
    };
    let before = remembered(&store);
    read(&store);
    let first = remembered(&store) - before;
    assert!(first > 0);
    store.close().unwrap();

    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    assert_eq!(remembered(&store), 0, "a new opening remembers nothing");
    read(&store);
    assert_eq!(
        remembered(&store),
        first,
        "the same nodes are checked again"
    );
    store.close().unwrap();
}

/// C5 and P13 of the Pager PRD: when the system does not provide the memory of the table, opening fails with the
/// out-of-memory class instead of aborting the process, in both modes, and the store on disk opens afterwards.
#[test]
fn opening_without_memory_for_the_table_fails_with_out_of_memory() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    commit_tree(&store, "n", 20, 64);
    store.close().unwrap();

    // More slots than the address space holds: the system can never provide them.
    let openings = krummi_pager::testing::with_checked_slots(usize::MAX / 8, || {
        [
            Store::open(tmp.path(), &StoreOptions::default()),
            Store::open_temporary(&StoreOptions::default()),
        ]
    });
    for opening in openings {
        match opening {
            Err(error) => assert_eq!(error.kind(), ErrorKind::OutOfMemory, "{error}"),
            Ok(_) => panic!("the opening got a table the system cannot provide"),
        }
    }

    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let tree = read.open_tree("n").unwrap();
    assert_eq!(read.len(&tree).unwrap(), 20);
    drop(read);
    store.close().unwrap();
}
