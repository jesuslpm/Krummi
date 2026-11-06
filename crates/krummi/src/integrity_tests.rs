//! Remembered page integrity (the remembered page integrity PRD, AC-I1 to AC-I11), through the public API and the
//! engine's test hooks: how many integrity checks the engine makes, which pages it remembers, where pages live, damage
//! that appears while the store is open, and memory that the system does not provide.

use krummi_tree::catalog;

use super::*;
use crate::check_tests::{
    FIXED_KEYS, FIXED_LEN, commit_fixed, commit_tree, concurrent_reads_while_a_write_moves_nodes,
    damaged_page, fixed_leaf, leaf, long_key, on_both, read_fixed, root_of, transfer,
};
use crate::value_tests::pattern;

/// Integrity checks the store has made since it opened.
fn checks(store: &Store) -> u64 {
    store.engine.integrity_checks()
}

/// Integrity checks that `f` makes.
fn checks_of<T>(store: &Store, f: impl FnOnce() -> T) -> (T, u64) {
    let before = checks(store);
    let result = f();
    (result, checks(store) - before)
}

/// Whether the integrity of page `page`, or of the value run that starts there, is remembered.
fn is_remembered(store: &Store, page: u64) -> bool {
    store.engine.is_remembered(page)
}

/// A value that the tree keeps off its leaf, in a run of three pages.
fn big_value(seed: u8) -> Vec<u8> {
    pattern(
        3 * krummi_pager::format::PAGE_SIZE - krummi_pager::format::PAGE_HEADER_SIZE,
        seed,
    )
}

/// The first page and the page count of the value run of `key` in tree `name`.
fn run_of(store: &Store, name: &str, key: &[u8]) -> (u64, u64) {
    let read = store.read().unwrap();
    let root = catalog::lookup(&read.inner, name).unwrap().unwrap().root;
    krummi_tree::run_of(&read.inner, root, key)
        .unwrap()
        .unwrap()
}

/// Commits tree `name` with 200 keys of 64 bytes and the value off its leaf of key `big`.
fn commit_tree_with_big_value(store: &Store, name: &str) {
    commit_tree(store, name, 200, 64);
    let mut write = store.write().unwrap();
    let tree = write.open_tree(name).unwrap();
    write.set(&tree, b"big", &big_value(1)).unwrap();
    write.commit().unwrap();
}

/// Reads the keys `0`, `100` and `199` of tree `name` and its value off its leaf, checking what they return.
fn read_tree_with_big_value(read: &ReadTransaction<'_>, name: &str) {
    let tree = read.open_tree(name).unwrap();
    for i in [0, 100, 199] {
        assert_eq!(
            read.get(&tree, &long_key(i, 64)).unwrap(),
            Some(i.to_string().as_bytes())
        );
    }
    assert!(read.get(&tree, b"big").unwrap() == Some(&big_value(1)[..]));
}

/// AC-I1 (I1, I2, I5): the pages of a tree and a value off its leaf in the store's pages are checked on their first
/// read only, by every read transaction of the opening, and checked again after reopening.
#[test]
fn ac_i1_each_page_and_value_is_checked_on_its_first_read() {
    let tmp = tempfile::tempdir().unwrap();
    let disk = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    commit_tree_with_big_value(&disk, "t");
    transfer(&disk);
    disk.close().unwrap();

    let disk = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let temporary = Store::open_temporary(&StoreOptions::default()).unwrap();
    commit_tree_with_big_value(&temporary, "t");
    transfer(&temporary);
    let mut first_reads = 0;
    for store in [&disk, &temporary] {
        let read = store.read().unwrap();
        let ((), first) = checks_of(store, || read_tree_with_big_value(&read, "t"));
        assert!(first > 0, "the first reads check");
        let (big, _) = run_of(store, "t", b"big");
        assert!(is_remembered(store, big), "the value is remembered whole");
        let ((), again) = checks_of(store, || read_tree_with_big_value(&read, "t"));
        assert_eq!(again, 0, "the same transaction does not check again");
        drop(read);
        let read = store.read().unwrap();
        let ((), other) = checks_of(store, || read_tree_with_big_value(&read, "t"));
        assert_eq!(other, 0, "another transaction does not check again");
        if std::ptr::eq(store, &disk) {
            first_reads = first;
        }
    }
    temporary.close().unwrap();
    disk.close().unwrap();

    let disk = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let read = disk.read().unwrap();
    let ((), reopened) = checks_of(&disk, || read_tree_with_big_value(&read, "t"));
    assert_eq!(
        reopened, first_reads,
        "reopening checks the same pages again"
    );
    drop(read);
    disk.close().unwrap();
}

/// AC-I2 (I2, I3): a write transaction checks every page it reads from the store's pages, also the remembered ones:
/// the nodes of a key it reads twice, the leaf it copies and the free-space map page it copies to reserve pages. A
/// later read transaction does not check again the nodes the write did not change.
#[test]
fn ac_i2_a_write_checks_every_page_it_reads() {
    on_both(|store| {
        commit_tree(store, "t", 200, 64);
        commit_tree(store, "d", 200, 64);
        let mut write = store.write().unwrap();
        write.delete_tree("d").unwrap();
        write.commit().unwrap();
        transfer(store);

        // The nodes of key 0 and key 150, remembered by a read; the map page, by a write that copies it to reserve
        // pages and rolls back.
        let (k0, k150) = (long_key(0, 64), long_key(150, 64));
        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        read.get(&tree, &k0).unwrap().unwrap();
        read.get(&tree, &k150).unwrap().unwrap();
        drop(read);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        for i in 1000..1100 {
            write.set(&tree, &long_key(i, 64), b"x").unwrap();
        }
        drop(write);
        let root = root_of(store, "t");
        let (leaf0, leaf150) = (leaf(store, root, &k0), leaf(store, root, &k150));
        assert_ne!(leaf0, leaf150);
        for page in [root, leaf0, leaf150, krummi_tx::MAP_PAGE] {
            assert!(is_remembered(store, page), "page {page} is remembered");
        }

        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        let (_, first) = checks_of(store, || write.get(&tree, &k0).unwrap().unwrap().to_vec());
        let (_, second) = checks_of(store, || write.get(&tree, &k0).unwrap().unwrap().to_vec());
        assert!(first >= 2, "the root and the leaf at least");
        assert_eq!(second, first, "the same pages are checked again");
        // A new value for key 0 changes its leaf; new keys of 1,500 bytes take pages from the map.
        let (_, copies) = checks_of(store, || {
            write.set(&tree, &k0, b"changed").unwrap();
            for i in 2000..2010 {
                write.set(&tree, &long_key(i, 1500), b"y").unwrap();
            }
        });
        assert!(copies >= 2, "the leaf and the map page it copies");
        write.commit().unwrap();

        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        let (_, unchanged) = checks_of(store, || read.get(&tree, &k150).unwrap().unwrap().to_vec());
        assert!(is_remembered(store, leaf150));
        assert_eq!(
            unchanged, 0,
            "the nodes the write did not change are not checked again"
        );
    });
}

/// AC-I3 (I3): committed versions still in the scratch area and the copies a write changes are not checked.
#[test]
fn ac_i3_versions_and_copies_are_not_checked() {
    on_both(|store| {
        store.engine.set_transfer_paused(true);
        commit_tree_with_big_value(store, "t");
        let read = store.read().unwrap();
        let ((), versions) = checks_of(store, || read_tree_with_big_value(&read, "t"));
        assert_eq!(versions, 0, "the scratch area's versions");
        drop(read);

        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        write.set(&tree, &long_key(5, 64), b"new").unwrap();
        let (_, copies) = checks_of(store, || {
            write
                .get(&tree, &long_key(5, 64))
                .unwrap()
                .unwrap()
                .to_vec()
        });
        assert_eq!(copies, 0, "the write's own copies");
        write.commit().unwrap();
        store.engine.set_transfer_paused(false);
    });
}

/// AC-I4 (I4, C8, G3): a remembered page that a write changes: a read opened before keeps seeing the earlier content;
/// once that read closes and the new version moves to the store's pages, a new read sees the new content, checked
/// once.
#[test]
fn ac_i4_a_moved_page_is_checked_again_once() {
    on_both(|store| {
        commit_tree(store, "t", 3, 64);
        transfer(store);
        let key = long_key(1, 64);
        let page = root_of(store, "t");
        let before = store.read().unwrap();
        let tree = before.open_tree("t").unwrap();
        assert_eq!(before.get(&tree, &key).unwrap(), Some(&b"1"[..]));
        assert!(is_remembered(store, page));

        let mut write = store.write().unwrap();
        let tree_w = write.open_tree("t").unwrap();
        write.set(&tree_w, &key, b"new").unwrap();
        write.commit().unwrap();
        assert_eq!(
            before.get(&tree, &key).unwrap(),
            Some(&b"1"[..]),
            "its snapshot"
        );
        drop(before);
        transfer(store);
        assert!(!is_remembered(store, page), "the transfer forgot it");

        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        let (value, first) = checks_of(store, || read.get(&tree, &key).unwrap().unwrap().to_vec());
        assert_eq!(value, b"new");
        assert!(
            first > 0 && is_remembered(store, page),
            "checked and remembered"
        );
        let (_, again) = checks_of(store, || read.get(&tree, &key).unwrap().unwrap().to_vec());
        assert_eq!(again, 0, "checked once");
    });
}

/// AC-I5 (I1, I4): the pages of a remembered value off its leaf, freed and reused by another value that reaches the
/// store's pages: reading the new value checks it once and returns what was written.
#[test]
fn ac_i5_a_reused_value_run_is_checked_for_its_new_content() {
    on_both(|store| {
        commit_tree_with_big_value(store, "t");
        transfer(store);
        read_tree_with_big_value(&store.read().unwrap(), "t");
        let (first, count) = run_of(store, "t", b"big");
        assert!(is_remembered(store, first));

        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        write.remove(&tree, b"big").unwrap();
        write.commit().unwrap();
        transfer(store);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        write.set(&tree, b"other", &big_value(9)).unwrap();
        write.commit().unwrap();
        transfer(store);
        assert_eq!(
            run_of(store, "t", b"other"),
            (first, count),
            "the same pages"
        );
        assert!(!is_remembered(store, first));

        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        let (value, checked) = checks_of(store, || {
            read.get(&tree, b"other").unwrap().unwrap().to_vec()
        });
        assert!(value == big_value(9), "what was written");
        assert!(checked > 0 && is_remembered(store, first));
        let (_, again) = checks_of(store, || {
            read.get(&tree, b"other").unwrap().unwrap().to_vec()
        });
        assert_eq!(again, 0);
    });
}

/// Flips byte `offset` of the payload of page `page` in the data file of the closed store in `folder`.
fn damage_file(folder: &std::path::Path, page: u64, offset: usize) {
    use std::io::{Read, Seek, SeekFrom, Write};
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(folder.join(krummi_pager::DATA_FILE))
        .unwrap();
    let at = page * krummi_pager::format::PAGE_SIZE as u64
        + (krummi_pager::format::PAGE_HEADER_SIZE + offset) as u64;
    let mut byte = [0];
    file.seek(SeekFrom::Start(at)).unwrap();
    file.read_exact(&mut byte).unwrap();
    file.seek(SeekFrom::Start(at)).unwrap();
    file.write_all(&[byte[0] ^ 0xFF]).unwrap();
}

/// AC-I6 (I1, I2, G6): a page of the data file altered with the store closed fails with the damage class and its
/// page on every read: twice in one read transaction and once in another.
#[test]
fn ac_i6_a_page_damaged_while_closed_fails_on_every_read() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    commit_tree(&store, "t", 3, 64);
    transfer(&store);
    let page = root_of(&store, "t");
    store.close().unwrap();
    damage_file(tmp.path(), page, 100);

    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let key = long_key(1, 64);
    let read = store.read().unwrap();
    let tree = read.open_tree("t").unwrap();
    assert_eq!(damaged_page(read.get(&tree, &key)), page);
    assert_eq!(damaged_page(read.get(&tree, &key)), page);
    drop(read);
    let read = store.read().unwrap();
    let tree = read.open_tree("t").unwrap();
    assert_eq!(damaged_page(read.get(&tree, &key)), page);
    assert!(!is_remembered(&store, page));
    drop(read);
    store.close().unwrap();
}

/// AC-I7 (I2, I8): a page not read yet in this opening, altered with the store open, fails when it is read.
#[test]
fn ac_i7_a_page_not_read_yet_and_damaged_while_open_fails() {
    on_both(|store| {
        commit_tree(store, "t", 3, 64);
        transfer(store);
        let page = root_of(store, "t");
        assert!(
            !is_remembered(store, page),
            "not read since the transfer wrote it"
        );
        store.engine.flip_byte_in_place(page, 100).unwrap();
        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        assert_eq!(damaged_page(read.get(&tree, &long_key(1, 64))), page);
    });
}

/// AC-I8 (I3, I5, I8), normal tree: a remembered leaf with no version left to transfer, altered with the store open,
/// makes a write transaction that reads it fail with the damage class and its page in that operation, and the write
/// cannot commit; after reopening, the first read of the page fails the same way. It replaces AC-C7.
#[test]
fn ac_i8_a_write_detects_a_remembered_page_damaged_while_open() {
    let tmp = tempfile::tempdir().unwrap();
    let mut page_on_disk = None;
    for (index, mut store) in [
        Store::open(tmp.path(), &StoreOptions::default()).unwrap(),
        Store::open_temporary(&StoreOptions::default()).unwrap(),
    ]
    .into_iter()
    .enumerate()
    {
        commit_tree(&store, "n", 200, 64);
        transfer(&store);
        let key = long_key(3, 64);
        let read = store.read().unwrap();
        let tree = read.open_tree("n").unwrap();
        assert!(read.get(&tree, &key).unwrap().is_some());
        drop(read);
        let page = leaf(&store, root_of(&store, "n"), &key);
        assert!(is_remembered(&store, page));

        // A byte of a key, which keeps the layout valid: only the integrity check can catch it.
        let read = store.read().unwrap();
        let at = read
            .inner
            .page(page)
            .unwrap()
            .windows(key.len())
            .position(|w| w == key)
            .expect("the key in its leaf");
        drop(read);
        store.engine.flip_byte_in_place(page, at + 10).unwrap();
        assert!(
            is_remembered(&store, page),
            "the flip is no write of the engine"
        );

        let write = store.write().unwrap();
        let tree = write.open_tree("n").unwrap();
        assert_eq!(damaged_page(write.get(&tree, &key)), page);
        assert!(write.commit().is_err(), "the write cannot commit (T11)");
        if index == 0 {
            page_on_disk = Some((page, key));
        }
        store.close().unwrap();
    }

    let (page, key) = page_on_disk.unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let tree = read.open_tree("n").unwrap();
    assert_eq!(damaged_page(read.get(&tree, &key)), page);
    drop(read);
    store.close().unwrap();
}

/// AC-I8 (I3, I8, X14), fixed-size tree: the same with a byte of a value of a remembered fixed-size leaf. It replaces
/// the fourth part of AC-X19.
#[test]
fn ac_i8_a_write_detects_a_remembered_fixed_size_leaf_damaged_while_open() {
    on_both(|store| {
        commit_fixed(store, "f", FIXED_KEYS);
        transfer(store);
        read_fixed(&store.read().unwrap(), "f", 1400);
        let page = fixed_leaf(store, root_of(store, "f"), 1400);
        assert!(is_remembered(store, page));
        let read = store.read().unwrap();
        let mut entry = 1400u64.to_le_bytes().to_vec();
        entry.extend(crate::check_tests::fixed_value(1400));
        let at = read
            .inner
            .page(page)
            .unwrap()
            .windows(entry.len())
            .position(|w| w == entry)
            .expect("the entry in its leaf");
        drop(read);
        store.engine.flip_byte_in_place(page, at + 8).unwrap();

        let write = store.write().unwrap();
        let tree = write.open_fixed_tree("f", FIXED_LEN).unwrap();
        assert_eq!(damaged_page(write.get_fixed(&tree, 1400)), page);
        assert!(write.commit().is_err(), "the write cannot commit (T11)");
    });
}

/// AC-I9 (I4, I7): four threads read correct pages and a damaged one at once while a write changes the correct ones
/// and they move to the store's pages: every read returns the data of its snapshot, and the damaged page fails on
/// every read.
#[test]
fn ac_i9_concurrent_reads_while_pages_move() {
    let tmp = tempfile::tempdir().unwrap();
    let stores = [
        Store::open(tmp.path(), &StoreOptions::default()).unwrap(),
        Store::open_temporary(&StoreOptions::default()).unwrap(),
    ];
    concurrent_reads_while_a_write_moves_nodes(stores, |store, bad_page| {
        transfer(store);
        // Not read since the transfer wrote it, as a page damaged with the store closed (AC-I6).
        assert!(!store.engine.is_remembered(bad_page));
        store.engine.flip_byte_in_place(bad_page, 0).unwrap();
    });
}

/// AC-I10 (I4, I5, I6): a store with remembered pages grows while a read reads them: the remembered pages the
/// transfer did not write stay unchecked, and the new pages are checked on their first read.
#[test]
fn ac_i10_growing_keeps_what_is_remembered() {
    on_both(|store| {
        commit_tree(store, "a", 50, 64);
        transfer(store);
        let read = store.read().unwrap();
        let tree = read.open_tree("a").unwrap();
        let keys: Vec<Vec<u8>> = (0..50).map(|i| long_key(i, 64)).collect();
        for key in &keys {
            read.get(&tree, key).unwrap().unwrap();
        }
        let leaves: Vec<u64> = keys
            .iter()
            .map(|key| leaf(store, root_of(store, "a"), key))
            .collect();

        let size = store.info().size_bytes;
        commit_tree(store, "b", 400, 1500);
        assert!(store.info().size_bytes > size, "the store grew");
        let ((), during) = checks_of(store, || {
            for key in &keys {
                read.get(&tree, key).unwrap().unwrap();
            }
        });
        assert_eq!(during, 0, "the read's remembered pages after the growth");
        drop(read);
        transfer(store);
        for &page in &leaves {
            assert!(
                is_remembered(store, page),
                "page {page} was not written, so it stays remembered"
            );
        }

        // Nothing has read the tree since the transfer wrote it; finding its leaf reads it, so that comes after.
        let read = store.read().unwrap();
        let tree = read.open_tree("b").unwrap();
        let key = long_key(399, 1500);
        let (_, first) = checks_of(store, || read.get(&tree, &key).unwrap().unwrap().to_vec());
        let (_, again) = checks_of(store, || read.get(&tree, &key).unwrap().unwrap().to_vec());
        let new_leaf = leaf(store, root_of(store, "b"), &key);
        assert!(
            new_leaf * krummi_pager::format::PAGE_SIZE as u64 >= size,
            "a page of the growth"
        );
        assert!(
            first > 0 && is_remembered(store, new_leaf),
            "checked on its first read"
        );
        assert_eq!(again, 0);
    });
}

/// AC-I11 (I6, P13): without memory for the remembered pages, opening fails with the out-of-memory class, and so does
/// a growth, which leaves the store's size as it was and the store usable. A growth beyond the maximum size fails
/// with its own class first.
#[test]
fn ac_i11_no_memory_for_the_remembered_pages() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    commit_tree(&store, "t", 20, 64);
    store.close().unwrap();
    let openings = krummi_pager::testing::with_remembered_commits_failing(|| {
        [
            Store::open(tmp.path(), &StoreOptions::default()),
            Store::open_temporary(&StoreOptions::default()),
        ]
    });
    for opening in openings {
        match opening {
            Err(error) => assert_eq!(error.kind(), ErrorKind::OutOfMemory, "{error}"),
            Ok(_) => panic!("the opening got memory the test denied"),
        }
    }

    on_both(|store| {
        commit_tree(store, "t", 20, 64);
        let size = store.info().size_bytes;
        let grow = |store: &Store| {
            let mut write = store.write().unwrap();
            let tree = write.open_tree("t").unwrap();
            let result = (100..400).try_for_each(|i| write.set(&tree, &long_key(i, 1500), b"g"));
            result.and_then(|()| write.commit())
        };
        let error =
            krummi_pager::testing::with_remembered_commits_failing(|| grow(store)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::OutOfMemory, "{error}");
        assert_eq!(store.info().size_bytes, size, "the size it had");
        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        assert_eq!(read.get(&tree, &long_key(3, 64)).unwrap(), Some(&b"3"[..]));
        drop(read);
        grow(store).unwrap();
        assert!(store.info().size_bytes > size);
    });

    let small = StoreOptions {
        max_size: Some(krummi_pager::MIN_MAX_SIZE),
    };
    let store = Store::open_temporary(&small).unwrap();
    let fill = |from: usize| {
        let mut write = store.write().unwrap();
        let tree = write.open_tree_with("t", OpenMode::OpenOrCreate).unwrap();
        (from..from + 2000).try_for_each(|i| write.set(&tree, &long_key(i, 1500), b"g"))
    };
    assert_eq!(
        fill(0).unwrap_err().kind(),
        ErrorKind::MaxSize,
        "the store reached its maximum size"
    );
    let error = krummi_pager::testing::with_remembered_commits_failing(|| fill(5000)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MaxSize, "{error}");
    store.close().unwrap();
}
