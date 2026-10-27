//! Large values where the public tests cannot see without the engine's test hooks: where the reference to a value
//! off its leaf points (AC-V3), the tree's height and the value runs while a read keeps its values and writes free
//! and reuse their pages (AC-V6), and a value that goes to the frontier because the free pages are scattered (AC-V9).

use krummi_pager::format::{PAGE_HEADER_SIZE, PAGE_SIZE};
use krummi_tx::Memory;

use super::*;
use crate::tests::long_key;

const NORMAL: TreeKind = TreeKind::Normal;

/// A value of `len` bytes, different for each `seed`, whose pages never hold the same bytes.
pub(crate) fn pattern(len: usize, seed: u8) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8 ^ seed).collect()
}

/// The largest value of a run of `pages` pages (D1).
fn run_capacity(pages: usize) -> usize {
    pages * PAGE_SIZE - PAGE_HEADER_SIZE
}

/// The value run of `key` in the tree `t` as `tx` sees it: its first page and its number of pages.
fn run_of<S: PageSource>(tx: &S, key: &[u8]) -> (u64, u64) {
    let root = catalog::lookup(tx, "t").unwrap().unwrap().root;
    krummi_tree::run_of(tx, root, key).unwrap().unwrap()
}

fn get<'t>(read: &'t ReadTransaction<'_>, key: &[u8]) -> &'t [u8] {
    let tree = read.open_tree("t", NORMAL).unwrap();
    read.get(&tree, key).unwrap().unwrap()
}

/// AC-V3 (V2, AC20, J7, T14, T16): a value off its leaf is read as one contiguous sequence of the bytes stored, never
/// a copy: in the write that stored it, its own copy; committed and not yet in `krummi.data`, and the older version a
/// read keeps after a later write overwrote it, the versions of the temporary area; once transferred, `krummi.data`
/// or the memory of a temporary store. The same memory comes back on each read.
#[test]
fn ac_v3_a_value_off_its_leaf_is_read_where_the_engine_keeps_it() {
    let tmp = tempfile::tempdir().unwrap();
    for store in [
        Store::open(tmp.path(), &StoreOptions::default()).unwrap(),
        Store::open_temporary(&StoreOptions::default()).unwrap(),
    ] {
        let engine = &store.engine;
        engine.set_transfer_paused(true);
        let (old, new) = (pattern(run_capacity(3), 1), pattern(run_capacity(3) + 1, 2));

        let mut write = store.write().unwrap();
        let tree = write
            .open_tree_with("t", NORMAL, OpenMode::CreateNew)
            .unwrap();
        write.set(&tree, b"k", &old).unwrap();
        let own = write.get(&tree, b"k").unwrap().unwrap();
        assert!(own == &old[..], "the write reads its own value");
        let (first, count) = run_of(&write.inner, b"k");
        assert_eq!(count, 3);
        assert_eq!(
            own.as_ptr(),
            write.inner.run(first, count).unwrap().as_ptr()
        );
        assert_eq!(engine.memory_of(own), None, "the write's own copy");
        write.commit().unwrap();

        let read = store.read().unwrap();
        let committed = get(&read, b"k");
        assert!(committed == &old[..]);
        assert_eq!(engine.memory_of(committed), Some(Memory::Versions));
        assert_eq!(committed.as_ptr(), get(&read, b"k").as_ptr(), "no copy");

        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        write.set(&tree, b"k", &new).unwrap();
        write.commit().unwrap();
        let kept = get(&read, b"k");
        assert!(kept == &old[..], "the older version");
        assert_eq!(engine.memory_of(kept), Some(Memory::Versions));
        assert_eq!(kept.as_ptr(), committed.as_ptr());
        let newer = store.read().unwrap();
        assert!(get(&newer, b"k") == &new[..]);
        assert_eq!(engine.memory_of(get(&newer, b"k")), Some(Memory::Versions));
        drop((read, newer));

        engine.set_transfer_paused(false);
        assert!(engine.transfer_now());
        let read = store.read().unwrap();
        let transferred = get(&read, b"k");
        assert!(transferred == &new[..]);
        assert_eq!(engine.memory_of(transferred), Some(Memory::Data));
        assert_eq!(transferred.as_ptr(), get(&read, b"k").as_ptr(), "no copy");
        drop(read);
        store.close().unwrap();
    }

    // In temporary mode, a value whose run starts in the memory the store had and ends in what it adds to grow.
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let pages = store.info().page_count;
    let value = pattern(store.info().size_bytes as usize, 3);
    let mut write = store.write().unwrap();
    let tree = write
        .open_tree_with("t", NORMAL, OpenMode::CreateNew)
        .unwrap();
    write.set(&tree, b"k", &value).unwrap();
    write.commit().unwrap();
    assert!(store.info().page_count > pages, "the store grew");
    assert!(store.engine.transfer_now());
    let read = store.read().unwrap();
    let (first, count) = run_of(&read.inner, b"k");
    assert!(
        first < pages && first + count > pages,
        "{first} {count} {pages}"
    );
    let bytes = get(&read, b"k");
    assert!(bytes == &value[..]);
    assert_eq!(store.engine.memory_of(bytes), Some(Memory::Data));
}

/// The value of entry `i` of generation `seed`: off its leaf for odd `i`, in a run of one, two or three pages, and in
/// its leaf for even `i`. With [`long_key`], three entries fit in a leaf and three in a branch whatever their values
/// (an entry off its leaf keeps its key in the leaf), so a few dozen make a tree of several levels.
pub(crate) fn mixed_value(i: usize, seed: u8) -> Vec<u8> {
    let len = if i % 2 == 1 {
        3_000 + (i / 2 % 3) * PAGE_SIZE
    } else {
        100 + i
    };
    pattern(len, seed ^ i as u8)
}

/// The root of the tree `t` in `write`, with the write's changes: the catalog has it only once the write commits.
fn root_in(write: &WriteTransaction<'_>) -> u64 {
    let trees = write.trees.borrow();
    trees.0.iter().find(|tree| tree.name == "t").unwrap().root
}

/// The height of the tree `t` committed as `read` sees it.
fn height_in_read(read: &ReadTransaction<'_>) -> u8 {
    let root = catalog::lookup(&read.inner, "t").unwrap().unwrap().root;
    krummi_tree::height(&read.inner, root).unwrap()
}

/// The height of the tree `t` in `write`, with its changes.
fn height_in_write(write: &WriteTransaction<'_>) -> u8 {
    krummi_tree::height(&write.inner, root_in(write)).unwrap()
}

type Model = std::collections::BTreeMap<Vec<u8>, Vec<u8>>;

/// Checks that `read` sees exactly `model` in the tree `t`: through Leer of every key of the model and of `absent`,
/// its count, and a scan of the whole tree.
#[track_caller]
fn check_snapshot(read: &ReadTransaction<'_>, model: &Model, absent: &[Vec<u8>], label: &str) {
    let tree = read.open_tree("t", NORMAL).unwrap();
    for (key, value) in model {
        let found = read.get(&tree, key).unwrap();
        assert!(found == Some(&value[..]), "{label}: {} bytes", value.len());
    }
    for key in absent.iter().filter(|key| !model.contains_key(*key)) {
        assert_eq!(read.get(&tree, key).unwrap(), None, "{label}");
    }
    assert_eq!(read.len(&tree).unwrap(), model.len() as u64, "{label}");
    let scanned: Vec<(Vec<u8>, Vec<u8>)> = read
        .range(&tree, ..)
        .unwrap()
        .map(|entry| entry.map(|(k, v)| (k.to_vec(), v.to_vec())).unwrap())
        .collect();
    assert!(
        scanned == model.clone().into_iter().collect::<Vec<_>>(),
        "{label}: the scan"
    );
}

/// Whether two runs, as (first page, pages), share a page.
fn overlap((a, m): (u64, u64), (b, n): (u64, u64)) -> bool {
    a < b + n && b < a + m
}

/// AC-V6 (V2, G3, T14, T16, AC-B17): a read holds references to values off their leaf, and a scan half consumed,
/// while a write overwrites and removes those values and splits and merges leaves holding values in and off them
/// until the tree's height changes, and commits; another write then reuses the pages the first one freed (F6). The
/// read's references keep their bytes, and it keeps reading and scanning exactly its snapshot, also once the writes
/// are transferred; a new read sees the writes' values. With keys of 2,048 bytes, three to a leaf and to a branch, 6
/// entries make two levels, 90 at least four, and the 14 left at the end fewer.
#[test]
fn ac_v6_a_read_keeps_its_values_off_their_leaf_while_writes_free_and_reuse_their_pages() {
    let tmp = tempfile::tempdir().unwrap();
    for store in [
        Store::open(tmp.path(), &StoreOptions::default()).unwrap(),
        Store::open_temporary(&StoreOptions::default()).unwrap(),
    ] {
        let engine = &store.engine;
        let keys: Vec<Vec<u8>> = (0..90).chain(200..230).map(long_key).collect();
        let snapshot: Model = (0..6).map(|i| (long_key(i), mixed_value(i, 1))).collect();
        let mut write = store.write().unwrap();
        let tree = write
            .open_tree_with("t", NORMAL, OpenMode::CreateNew)
            .unwrap();
        for (key, value) in &snapshot {
            write.set(&tree, key, value).unwrap();
        }
        write.commit().unwrap();
        // In `krummi.data` or the temporary store's memory, where reusing their pages writes over them.
        assert!(engine.transfer_now());

        let read = store.read().unwrap();
        let rtree = read.open_tree("t", NORMAL).unwrap();
        let references: Vec<&[u8]> = snapshot
            .keys()
            .map(|key| read.get(&rtree, key).unwrap().unwrap())
            .collect();
        let old_runs: Vec<(u64, u64)> = (1..6)
            .step_by(2)
            .map(|i| run_of(&read.inner, &long_key(i)))
            .collect();
        let start = height_in_read(&read);
        assert_eq!(start, 2);
        let mut half = read.range(&rtree, ..).unwrap();
        let mut scanned: Vec<(Vec<u8>, Vec<u8>)> = half
            .by_ref()
            .take(2)
            .map(|entry| entry.map(|(k, v)| (k.to_vec(), v.to_vec())).unwrap())
            .collect();

        let mut model = snapshot.clone();
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        let changes: [(usize, Option<Vec<u8>>); 4] = [
            (0, Some(mixed_value(1, 2))),
            (1, Some(pattern(3 * PAGE_SIZE, 2))),
            (3, Some(b"in its leaf now".to_vec())),
            (5, None),
        ];
        for (i, value) in changes {
            match value {
                Some(value) => {
                    write.set(&tree, &long_key(i), &value).unwrap();
                    model.insert(long_key(i), value);
                }
                None => {
                    assert!(write.remove(&tree, &long_key(i)).unwrap());
                    model.remove(&long_key(i));
                }
            }
        }
        for i in 6..90 {
            write.set(&tree, &long_key(i), &mixed_value(i, 2)).unwrap();
        }
        let tallest = height_in_write(&write);
        assert!(tallest >= 4, "{tallest} levels");
        check_snapshot(&read, &snapshot, &keys, "while the tree grows");
        for i in 6..90 {
            if i % 9 == 0 {
                model.insert(long_key(i), mixed_value(i, 2));
            } else {
                assert!(write.remove(&tree, &long_key(i)).unwrap());
            }
        }
        let end = height_in_write(&write);
        assert!(end < tallest && end != start, "{start}, {tallest}, {end}");
        write.commit().unwrap();

        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        let mut new_runs = Vec::new();
        for i in 200..230 {
            let value = mixed_value(2 * i + 1, 3);
            write.set(&tree, &long_key(i), &value).unwrap();
            let root = root_in(&write);
            new_runs.push(
                krummi_tree::run_of(&write.inner, root, &long_key(i))
                    .unwrap()
                    .unwrap(),
            );
            model.insert(long_key(i), value);
        }
        assert!(
            old_runs
                .iter()
                .any(|&old| new_runs.iter().any(|&new| overlap(old, new))),
            "the pages of the read's values are reused: {old_runs:?} {new_runs:?}"
        );
        write.commit().unwrap();

        for transfer in [false, true] {
            if transfer {
                assert!(engine.transfer_now());
            }
            for (reference, value) in references.iter().zip(snapshot.values()) {
                assert!(*reference == &value[..], "a reference changed");
            }
            check_snapshot(&read, &snapshot, &keys, "the old read");
            check_snapshot(&store.read().unwrap(), &model, &keys, "a new read");
        }
        scanned.extend(half.map(|entry| entry.map(|(k, v)| (k.to_vec(), v.to_vec())).unwrap()));
        assert!(scanned == snapshot.clone().into_iter().collect::<Vec<_>>());
        drop(read);
        assert!(engine.transfer_now());
        check_snapshot(&store.read().unwrap(), &model, &keys, "after the read");
        store.close().unwrap();
    }
}

/// The pages of the value AC-V9 stores: more than any hole the scattered free pages leave.
const VALUE_PAGES: u64 = 32;

/// Stores values of one page in the tree `t`, in writes of a few of them, until there are twice [`VALUE_PAGES`] and
/// the store ends at the frontier (it grows by doubling, P11), then removes every other one: the free pages are
/// scattered, in holes of one page, and the store has no room after the frontier.
fn scatter(store: &Store) {
    let key = |i: u64| format!("{i:08}").into_bytes();
    let mut write = store.write().unwrap();
    write
        .open_tree_with("t", NORMAL, OpenMode::CreateNew)
        .unwrap();
    write.commit().unwrap();
    let mut stored = 0;
    loop {
        let gap = store.info().page_count - store.engine.frontier();
        if gap == 0 && stored >= 2 * VALUE_PAGES {
            break;
        }
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        for _ in 0..gap.clamp(1, 16) {
            write.set(&tree, &key(stored), &[1; 5_000]).unwrap();
            stored += 1;
        }
        write.commit().unwrap();
    }
    let mut write = store.write().unwrap();
    let tree = write.open_tree("t", NORMAL).unwrap();
    for i in (0..stored).step_by(2) {
        assert!(write.remove(&tree, &key(i)).unwrap());
    }
    write.commit().unwrap();
    assert!(
        stored / 2 >= VALUE_PAGES,
        "enough scattered free pages: {stored} stored, {:?}",
        store.info()
    );
    assert_eq!(store.info().page_count, store.engine.frontier());
}

/// AC-V9 (V6, F5, T11): with enough scattered free pages for a value but no hole of its size, and the store ending at
/// the frontier, the value goes to the frontier: the store grows by at least its pages and the scattered pages stay
/// free; with a maximum size that does not let the store grow enough, the operation or the commit fails with the
/// maximum-size class, applies nothing, and a new write works.
#[test]
fn ac_v9_a_value_without_a_contiguous_hole_goes_to_the_frontier() {
    let value = pattern(run_capacity(VALUE_PAGES as usize), 4);
    let tmp = tempfile::tempdir().unwrap();
    for store in [
        Store::open(tmp.path(), &StoreOptions::default()).unwrap(),
        Store::open_temporary(&StoreOptions::default()).unwrap(),
    ] {
        scatter(&store);
        let before = store.info();
        let frontier = store.engine.frontier();
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        write.set(&tree, b"big", &value).unwrap();
        write.commit().unwrap();
        let after = store.info();
        let grown = after.page_count - before.page_count;
        assert!(grown >= VALUE_PAGES, "{grown}");
        let read = store.read().unwrap();
        assert_eq!(run_of(&read.inner, b"big"), (frontier, VALUE_PAGES));
        assert!(get(&read, b"big") == &value[..]);
        assert_eq!(
            after.free_pages,
            before.free_pages + grown - VALUE_PAGES,
            "the scattered free pages stay free"
        );
        drop(read);
        store.close().unwrap();
    }

    // A maximum size with room for a few pages more than the store has, fewer than the value's.
    let slack = 2 * VALUE_PAGES / 3 * PAGE_SIZE as u64;
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    scatter(&store);
    let size = store.info().size_bytes;
    store.close().unwrap();
    let options = |size: u64| StoreOptions {
        max_size: Some(size + slack),
    };
    let on_disk = Store::open(tmp.path(), &options(size)).unwrap();
    // A temporary store cannot reopen: the same preparation, which is deterministic, reaches the same size.
    let unlimited = Store::open_temporary(&StoreOptions::default()).unwrap();
    scatter(&unlimited);
    let size = unlimited.info().size_bytes;
    unlimited.close().unwrap();
    let in_memory = Store::open_temporary(&options(size)).unwrap();
    scatter(&in_memory);
    assert_eq!(in_memory.info().size_bytes, size, "the same preparation");
    for store in [on_disk, in_memory] {
        let read = store.read().unwrap();
        let count = read.len(&read.open_tree("t", NORMAL).unwrap()).unwrap();
        drop(read);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        let class = match write.set(&tree, b"big", &value) {
            Err(error) => {
                assert_eq!(write.commit().unwrap_err().kind(), error.kind());
                error.kind()
            }
            Ok(()) => write.commit().unwrap_err().kind(),
        };
        assert_eq!(class, ErrorKind::MaxSize);
        let read = store.read().unwrap();
        let tree = read.open_tree("t", NORMAL).unwrap();
        assert_eq!(read.get(&tree, b"big").unwrap(), None, "nothing applied");
        assert_eq!(read.len(&tree).unwrap(), count);
        drop(read);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        write.set(&tree, b"probe", b"works").unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(get(&read, b"probe"), b"works");
        drop(read);
        store.close().unwrap();
    }
}
