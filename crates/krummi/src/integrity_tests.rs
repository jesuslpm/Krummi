//! Remembered page integrity (the remembered page integrity PRD, AC-I1 to AC-I11, with AC-I2 and AC-I8 replaced by
//! AC-S4 and AC-S6 of the writer checks PRD, which also has AC-S5 here), through the public API and the engine's test
//! hooks: how many integrity checks the engine makes, which pages it remembers, where pages live, damage that appears
//! while the store is open, and memory that the system does not provide.

use krummi_tree::catalog;

use super::*;
use crate::check_tests::{
    FIXED_KEYS, FIXED_LEN, commit_fixed, commit_tree, concurrent_reads_while_a_write_moves_nodes,
    damaged_page, fixed_leaf, leaf, long_key, on_both, root_of, transfer,
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

/// Whether the integrity of page `page` is remembered.
fn is_remembered(store: &Store, page: u64) -> bool {
    store.engine.is_remembered(page)
}

/// Whether the value run `(first, count)` is remembered as one run of that length.
fn is_run_remembered(store: &Store, (first, count): (u64, u64)) -> bool {
    store.engine.is_run_remembered(first, count)
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
        let big = run_of(store, "t", b"big");
        assert!(
            is_run_remembered(store, big),
            "the value is remembered whole"
        );
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

/// AC-I1 (I1, C5): a value off its leaf stays remembered when the table of checked contents, which holds the mark of
/// its length, gives the mark's slot to a node: with one slot, every read of the value also checks the layout of its
/// leaf, which takes the slot, and the value is still not checked again.
#[test]
fn ac_i1_a_value_stays_remembered_when_its_mark_is_replaced() {
    let tmp = tempfile::tempdir().unwrap();
    let stores = krummi_pager::testing::with_checked_slots(1, || {
        [
            Store::open(tmp.path(), &StoreOptions::default()).unwrap(),
            Store::open_temporary(&StoreOptions::default()).unwrap(),
        ]
    });
    for store in stores {
        commit_tree_with_big_value(&store, "t");
        transfer(&store);
        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        let get = || read.get(&tree, b"big").unwrap().unwrap().to_vec();
        let first = checks_of(&store, get);
        assert!(first.1 > 0);
        for _ in 0..3 {
            let (value, again) = checks_of(&store, get);
            assert!(value == big_value(1));
            assert_eq!(again, 0, "the value and its path stay remembered");
        }
        drop(read);
        store.close().unwrap();
    }
}

/// AC-S4 (S2, I1, I2), first part: the nodes of a tree remembered by an earlier read, the free-space map page remembered
/// by an earlier write and a remembered value off its leaf, all in the store's pages: a write that reads a key twice,
/// stores keys that split nodes and move the reference of the value to another leaf, removes keys that merge nodes,
/// reserves pages, reads the value and commits computes the integrity of none of them. It replaces AC-I2.
#[test]
fn ac_s4_a_write_uses_the_remembered_pages() {
    on_both(|store| {
        commit_tree(store, "t", 400, 64);
        commit_tree(store, "d", 200, 64);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        write.set(&tree, b"big", &big_value(1)).unwrap();
        write.delete_tree("d").unwrap();
        write.commit().unwrap();
        transfer(store);

        // Every node of the tree and the value, remembered by a read; the map page, by a write that copies it to
        // reserve pages and rolls back.
        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        for i in 0..400 {
            assert!(read.get(&tree, &long_key(i, 64)).unwrap().is_some());
        }
        assert!(read.get(&tree, b"big").unwrap() == Some(&big_value(1)[..]));
        drop(read);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        for i in 1000..1100 {
            write.set(&tree, &long_key(i, 64), b"x").unwrap();
        }
        drop(write);
        let root = root_of(store, "t");
        let (k0, k399) = (long_key(0, 64), long_key(399, 64));
        let (leaf0, big_leaf) = (leaf(store, root, &k0), leaf(store, root, b"big"));
        assert_eq!(leaf(store, root, &k399), big_leaf, "the last leaf");
        let big = run_of(store, "t", b"big");
        for page in [root, leaf0, big_leaf, krummi_tx::MAP_PAGE] {
            assert!(is_remembered(store, page), "page {page} is remembered");
        }
        assert!(is_run_remembered(store, big), "the value is remembered");
        let free = store.info().free_pages;

        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        let ((), checked) = checks_of(store, || {
            for _ in 0..2 {
                assert_eq!(write.get(&tree, &k0).unwrap(), Some(&b"0"[..]));
            }
            // Keys after the last ones split the last leaf, which holds the value's reference, and take pages from
            // the map.
            for i in 1000..1100 {
                write.set(&tree, &long_key(i, 64), b"x").unwrap();
            }
            // Removing most of the first keys leaves their leaves under a quarter full, which merges them.
            for i in 0..380 {
                assert!(write.remove(&tree, &long_key(i, 64)).unwrap());
            }
            assert!(write.get(&tree, b"big").unwrap() == Some(&big_value(1)[..]));
            write.commit().unwrap();
        });
        assert_eq!(checked, 0, "no integrity check of a remembered page");
        let root_after = root_of(store, "t");
        assert_ne!(
            leaf(store, root_after, b"big"),
            big_leaf,
            "the value's reference moved to another leaf"
        );
        assert!(store.info().free_pages > free, "merges freed pages");
        assert_eq!(
            run_of(store, "t", b"big"),
            big,
            "the value stays in its run"
        );
    });
}

/// AC-S4 (S2, I1, I5), second part: a page of the store's pages that nobody read in the opening is checked on its
/// first read by a write, and not on the next ones, of that write, of another write or of a read.
#[test]
fn ac_s4_a_write_remembers_a_page_on_its_first_read() {
    let tmp = tempfile::tempdir().unwrap();
    let disk = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    commit_tree(&disk, "t", 200, 64);
    transfer(&disk);
    disk.close().unwrap();
    let disk = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let temporary = Store::open_temporary(&StoreOptions::default()).unwrap();
    commit_tree(&temporary, "t", 200, 64);
    transfer(&temporary);

    for store in [disk, temporary] {
        let key = long_key(150, 64);
        let root = root_of(&store, "t");
        assert!(!is_remembered(&store, root), "not read in this opening");
        let get = |write: &WriteTransaction<'_>| {
            let tree = write.open_tree("t").unwrap();
            checks_of(&store, || write.get(&tree, &key).unwrap().unwrap().to_vec())
        };
        let write = store.write().unwrap();
        let (value, first) = get(&write);
        assert_eq!(value, b"150");
        assert!(first >= 2, "the root and the leaf at least");
        let page = leaf(&store, root, &key);
        assert!(is_remembered(&store, root) && is_remembered(&store, page));
        assert_eq!(get(&write).1, 0, "the same write does not check again");
        drop(write);
        let write = store.write().unwrap();
        assert_eq!(get(&write).1, 0, "another write does not check again");
        drop(write);
        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        let (_, again) = checks_of(&store, || read.get(&tree, &key).unwrap().unwrap().to_vec());
        assert_eq!(again, 0, "a read does not check again");
        drop(read);
        store.close().unwrap();
    }
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
        assert!(is_run_remembered(store, (first, count)));

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
        assert!(!is_run_remembered(store, (first, count)));

        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        let (value, checked) = checks_of(store, || {
            read.get(&tree, b"other").unwrap().unwrap().to_vec()
        });
        assert!(value == big_value(9), "what was written");
        assert!(checked > 0 && is_run_remembered(store, (first, count)));
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

/// AC-S5 (S2, T11, G6): a page of the data file altered with the store closed makes every read of a write that reads
/// it fail with the damage class and its page, twice, and the write cannot commit. A temporary store, which cannot be
/// closed, does the same with a page not read since the transfer wrote it, altered while it is open.
#[test]
fn ac_s5_a_write_fails_on_every_read_of_a_damaged_page() {
    let tmp = tempfile::tempdir().unwrap();
    let disk = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    commit_tree(&disk, "t", 3, 64);
    transfer(&disk);
    let on_disk = root_of(&disk, "t");
    disk.close().unwrap();
    damage_file(tmp.path(), on_disk, 100);
    let disk = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let mut temporary = Store::open_temporary(&StoreOptions::default()).unwrap();
    commit_tree(&temporary, "t", 3, 64);
    transfer(&temporary);
    let in_memory = root_of(&temporary, "t");
    assert!(!is_remembered(&temporary, in_memory));
    temporary.engine.flip_byte_in_place(in_memory, 100).unwrap();

    for (store, page) in [(disk, on_disk), (temporary, in_memory)] {
        let key = long_key(1, 64);
        let write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        assert_eq!(damaged_page(write.get(&tree, &key)), page);
        assert_eq!(damaged_page(write.get(&tree, &key)), page, "again");
        assert!(!is_remembered(&store, page), "a failure is not remembered");
        assert_eq!(
            damaged_page(write.commit()),
            page,
            "the write cannot commit"
        );
        store.close().unwrap();
    }
}

/// The trees of AC-S6: a normal tree and a fixed-size one.
#[derive(Clone, Copy, Debug)]
enum Kind {
    Normal,
    Fixed,
}

/// The index of the key AC-S6 alters, and of another key of its leaf.
const AC_S6_KEYS: [usize; 2] = [3, 4];
/// The key of the fixed-size tree of AC-S6 for index `i` is this plus `i`.
const FIXED_BASE: u64 = 1397;

/// A transaction that reads a tree of AC-S6.
#[derive(Clone, Copy)]
enum Source<'a, 's> {
    Read(&'a ReadTransaction<'s>),
    Write(&'a WriteTransaction<'s>),
}

impl Kind {
    /// The value of key `i` of the normal tree: the tree keeps it in its leaf, and it appears once in the leaf.
    fn value(i: usize) -> Vec<u8> {
        format!("value of {i:04}").into_bytes()
    }

    /// Commits the tree and moves it to the store's pages.
    fn commit(self, store: &Store) {
        match self {
            Kind::Normal => {
                let mut write = store.write().unwrap();
                let tree = write.open_tree_with("t", OpenMode::CreateNew).unwrap();
                for i in 0..200 {
                    write.set(&tree, &long_key(i, 64), &Kind::value(i)).unwrap();
                }
                write.commit().unwrap();
            }
            Kind::Fixed => commit_fixed(store, "t", FIXED_KEYS),
        }
        transfer(store);
    }

    /// The leaf of key `i`.
    fn leaf(self, store: &Store, i: usize) -> u64 {
        let root = root_of(store, "t");
        match self {
            Kind::Normal => leaf(store, root, &long_key(i, 64)),
            Kind::Fixed => fixed_leaf(store, root, FIXED_BASE + i as u64),
        }
    }

    /// The value of key `i` as `source` reads it.
    fn get(self, source: Source<'_, '_>, i: usize) -> Result<Vec<u8>> {
        let key = long_key(i, 64);
        let fixed_key = FIXED_BASE + i as u64;
        let value = match (self, source) {
            (Kind::Normal, Source::Read(read)) => read.get(&read.open_tree("t")?, &key)?,
            (Kind::Normal, Source::Write(write)) => write.get(&write.open_tree("t")?, &key)?,
            (Kind::Fixed, Source::Read(read)) => {
                read.get_fixed(&read.open_fixed_tree("t", FIXED_LEN)?, fixed_key)?
            }
            (Kind::Fixed, Source::Write(write)) => {
                write.get_fixed(&write.open_fixed_tree("t", FIXED_LEN)?, fixed_key)?
            }
        };
        Ok(value.expect("the key is there").to_vec())
    }

    /// Changes the value of key `i` in `write`.
    fn change(self, write: &mut WriteTransaction<'_>, i: usize) {
        match self {
            Kind::Normal => {
                let tree = write.open_tree("t").unwrap();
                write.set(&tree, &long_key(i, 64), b"changed").unwrap();
            }
            Kind::Fixed => {
                let tree = write.open_fixed_tree("t", FIXED_LEN).unwrap();
                let key = FIXED_BASE + i as u64;
                write.set_fixed(&tree, key, &[0xAB; FIXED_LEN]).unwrap();
            }
        }
    }

    /// Flips, with the store open, the first byte of the value of key `i` in its leaf `page`, a byte that keeps the
    /// leaf's layout coherent, so that only the integrity check can find it. Returns the value with the byte flipped.
    fn flip_value(self, store: &mut Store, page: u64, i: usize) -> Vec<u8> {
        let (entry, offset) = match self {
            Kind::Normal => (Kind::value(i), 0),
            Kind::Fixed => {
                let key = FIXED_BASE + i as u64;
                let mut entry = key.to_le_bytes().to_vec();
                entry.extend(crate::check_tests::fixed_value(key));
                (entry, 8)
            }
        };
        let read = store.read().unwrap();
        let payload = read.inner.page(page).unwrap();
        let mut found = payload
            .windows(entry.len())
            .enumerate()
            .filter(|(_, w)| *w == entry);
        let (at, _) = found.next().expect("the entry in its leaf");
        assert!(found.next().is_none(), "the entry appears once");
        drop(read);
        store.engine.flip_byte_in_place(page, at + offset).unwrap();
        let mut value = entry[offset..].to_vec();
        value[0] ^= 0xFF;
        value
    }
}

/// AC-S6 (S2, S3, I5) for a tree of `kind`: a remembered leaf with no version left to transfer, with a byte of the
/// value of one of its entries altered while the store is open, keeping its layout coherent. A write that reads it
/// without changing it returns the altered byte without failing. After reopening, the first read of the leaf fails
/// with the damage class and its page; when another write changes another entry of the leaf and commits before, the
/// leaf is read after the transfer and a new opening without a damage error and returns the altered byte. A temporary
/// store, which cannot be reopened, reads it after the transfer the same way.
fn ac_s6(kind: Kind) {
    let [altered, other] = AC_S6_KEYS;
    for change_before in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let disk = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
        let temporary = Store::open_temporary(&StoreOptions::default()).unwrap();
        for (on_disk, mut store) in [(true, disk), (false, temporary)] {
            let what = format!("{kind:?}, change before {change_before}, on disk {on_disk}");
            kind.commit(&store);
            let read = store.read().unwrap();
            kind.get(Source::Read(&read), altered).unwrap();
            drop(read);
            let page = kind.leaf(&store, altered);
            assert_eq!(kind.leaf(&store, other), page, "{what}: the same leaf");
            assert!(is_remembered(&store, page), "{what}");
            let flipped = kind.flip_value(&mut store, page, altered);

            let write = store.write().unwrap();
            let read_by_write = kind.get(Source::Write(&write), altered);
            assert_eq!(
                read_by_write.unwrap(),
                flipped,
                "{what}: the write reads it"
            );
            drop(write);
            if change_before {
                let mut write = store.write().unwrap();
                kind.change(&mut write, other);
                write.commit().unwrap();
                transfer(&store);
                assert_eq!(kind.leaf(&store, altered), page, "{what}: the same page");
            }
            if !on_disk {
                if change_before {
                    let read = store.read().unwrap();
                    let value = kind.get(Source::Read(&read), altered);
                    assert_eq!(value.unwrap(), flipped, "{what}: sealed with the change");
                }
                store.close().unwrap();
                continue;
            }
            store.close().unwrap();
            let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
            let read = store.read().unwrap();
            let value = kind.get(Source::Read(&read), altered);
            if change_before {
                assert_eq!(value.unwrap(), flipped, "{what}: sealed with the change");
            } else {
                assert_eq!(damaged_page(value), page, "{what}: found after reopening");
            }
            drop(read);
            store.close().unwrap();
        }
    }
}

/// AC-S6 in a normal tree. It replaces AC-I8, which replaced AC-C7.
#[test]
fn ac_s6_a_write_uses_a_remembered_leaf_altered_while_open() {
    ac_s6(Kind::Normal);
}

/// AC-S6 in a fixed-size tree. It replaces AC-I8's part for these trees, and so the fourth part of AC-X19.
#[test]
fn ac_s6_a_write_uses_a_remembered_fixed_size_leaf_altered_while_open() {
    ac_s6(Kind::Fixed);
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
