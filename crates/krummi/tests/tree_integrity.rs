//! Crashes, power cuts and altered bytes in stores with trees, on disk (Trees and key operations PRD: AC-A16; Trees
//! PRD: AC-B18, its first part; In-order scan PRD: AC-R12, its first part; Large values PRD: AC-V8, its first part,
//! and AC-V11; Multi-value trees PRD: AC-M14, its first part, and AC-M15; Fixed-size trees PRD: AC-X12, its first
//! part, and AC-X13; root PRD: AC10 repeated with the trees, their scans, a value off its leaf, a multi-value tree and
//! a fixed-size tree). The failures of T11 in the middle of a split, a merge or a range removal, at the page
//! operations of values off their leaf, of a multi-value tree or of a fixed-size tree, the second part of AC-B18,
//! AC-R12, AC-V8, AC-M14 and AC-X12, need the engine's hooks: they are in `src/tests.rs`.
//!
//! The crash sweeps record the file operations of a run and rebuild, for every point of the tested commit, what a
//! power cut (the models of `POWER_CUTS`) or a crash of the process (`Model::Written`: the system keeps every write
//! it was given) leaves, with the machinery of the transaction tests.

#[path = "../../krummi-tx/tests/common/mod.rs"]
mod common;
mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use common::{POWER_CUTS, Run};
use krummi::{ErrorKind, OpenMode, Store, StoreOptions, TreeKind, WriteTransaction};
use krummi_pager::DATA_FILE;
use krummi_pager::format::{PAGE_HEADER_SIZE, PAGE_SIZE};
use krummi_pager::testing::{self, Model};
use krummi_tree::multi::volume;
use support::{
    MULTI, NORMAL, fixed_height, fixed_leaf_page, fixed_model, fixed_value, leaf_page, long_key,
    open, subtree_root, value16,
};

/// What the tests see of a store's trees: each tree of the list with its count and what Leer finds for every key
/// the test used, in order.
type Seen = Vec<(String, u64, Vec<Option<Vec<u8>>>)>;

/// What a new read sees, checking that each tree's count is the number of keys Leer finds (A5).
fn seen(store: &Store, keys: &[Vec<u8>]) -> Seen {
    let read = store.read().unwrap();
    let list = read.list_trees().unwrap();
    list.into_iter()
        .map(|(name, kind)| {
            assert_eq!(kind, TreeKind::Normal);
            let tree = read.open_tree(&name).unwrap();
            let values: Vec<Option<Vec<u8>>> = keys
                .iter()
                .map(|key| read.get(&tree, key).unwrap().map(<[u8]>::to_vec))
                .collect();
            let count = read.len(&tree).unwrap();
            let found = values.iter().flatten().count() as u64;
            assert_eq!(count, found, "{name}: the count is the keys found");
            (name, count, values)
        })
        .collect()
}

/// Commits `setup`, then the write `change`, recording the file operations, and with `through_close` also those of
/// the close, which moves the commit into `krummi.data`; then, for every point from the start of that write on and
/// every model of a power cut or a crash of the process, reopens what survives and checks that the trees, every
/// Leer of `keys` and the counts are all as before the write or all as after it, after it once the commit had
/// returned, and that a new write reads and modifies every tree.
fn crash_sweep(
    keys: &[Vec<u8>],
    through_close: bool,
    setup: impl Fn(&Store),
    change: impl Fn(&mut WriteTransaction<'_>),
) {
    crash_sweep_with(
        through_close,
        setup,
        change,
        |store| seen(store, keys),
        |store, now| {
            let mut write = store.write().unwrap();
            for (name, _, _) in now {
                let tree = write.open_tree(name).unwrap();
                for key in keys.iter().take(3) {
                    write.get(&tree, key).unwrap();
                }
                write.set(&tree, &keys[0], b"modified").unwrap();
                write.remove(&tree, &keys[1]).unwrap();
            }
            write.commit().unwrap();
        },
    );
}

/// The sweep of [`crash_sweep`], with what the test sees of the store taken by `look` and the new write after each
/// reopening made by `new_write`, which gets what `look` saw.
fn crash_sweep_with<T: PartialEq>(
    through_close: bool,
    setup: impl Fn(&Store),
    change: impl Fn(&mut WriteTransaction<'_>),
    look: impl Fn(&Store) -> T,
    new_write: impl Fn(&Store, &T),
) {
    let source = Run::begin();
    let store = open(source.path());
    setup(&store);
    let start = testing::recorded_len();
    let before = look(&store);
    let mut write = store.write().unwrap();
    change(&mut write);
    write.commit().unwrap();
    let returned = testing::recorded_len();
    let after = look(&store);
    assert!(before != after, "the write changes what the test sees");
    // Without `through_close`, the close comes after the recording.
    let open_store = if through_close {
        store.close().unwrap();
        None
    } else {
        Some(store)
    };
    let run = Run::finish(source, vec![start, returned]);
    drop(open_store);
    // The checks below are about what survived, not about durability: their own flushes are skipped.
    testing::skip_syncs(true);
    let mut outcomes = [false; 2];
    for cut in start..=run.ops.len() {
        for model in POWER_CUTS.into_iter().chain([Model::Written]) {
            let folder = run.crash(cut, model);
            let store = open(folder.path());
            let now = look(&store);
            let label = format!("cut {cut} of {}, {model:?}", run.ops.len());
            if cut >= returned {
                assert!(now == after, "{label}: the commit had returned");
            } else {
                assert!(now == before || now == after, "{label}: half a commit");
            }
            outcomes[usize::from(now == after)] = true;
            new_write(&store, &now);
            store.close().unwrap();
        }
    }
    assert_eq!(
        outcomes, [true; 2],
        "some points keep the state before, others the one after"
    );
    testing::reset();
}

/// The crash sweep of AC-A16: a write that creates, renames, empties and deletes trees and stores and removes
/// entries with splits and merges of pages.
fn catalog_and_key_operations(through_close: bool) {
    let key = |i: usize| long_key(i, 1500);
    let keys: Vec<Vec<u8>> = (0..90).map(key).collect();
    let fill = |write: &mut WriteTransaction<'_>, name: &str, range: std::ops::Range<usize>| {
        let tree = write.open_tree_with(name, OpenMode::OpenOrCreate).unwrap();
        for i in range {
            write.set(&tree, &key(i), name.as_bytes()).unwrap();
        }
    };
    crash_sweep(
        &keys,
        through_close,
        |store| {
            let mut write = store.write().unwrap();
            fill(&mut write, "a", 0..40);
            fill(&mut write, "b", 0..30);
            fill(&mut write, "c", 30..60);
            fill(&mut write, "d", 0..5);
            write.commit().unwrap();
        },
        |write| {
            let created = write.open_tree_with("e", OpenMode::CreateNew).unwrap();
            for i in 40..80 {
                write.set(&created, &key(i), b"created").unwrap();
            }
            write.rename_tree("a", "f", false).unwrap();
            let renamed = write.open_tree("f").unwrap();
            for i in 5..35 {
                assert!(write.remove(&renamed, &key(i)).unwrap());
            }
            for i in 60..90 {
                write.set(&renamed, &key(i), b"renamed").unwrap();
            }
            let emptied = write.open_tree_with("b", OpenMode::Empty).unwrap();
            write.set(&emptied, &key(1), b"emptied").unwrap();
            write.delete_tree("c").unwrap();
            write.rename_tree("d", "c", false).unwrap();
        },
    );
}

/// The crash sweep of AC-B18: a write that splits and merges pages of a tree until it gains and loses levels.
fn splits_and_merges(through_close: bool) {
    let key = |i: usize| long_key(i, 2048);
    let keys: Vec<Vec<u8>> = (0..100).map(key).collect();
    crash_sweep(
        &keys,
        through_close,
        |store| {
            let mut write = store.write().unwrap();
            let tree = write.open_tree_with("t", OpenMode::CreateNew).unwrap();
            for i in 0..40 {
                write.set(&tree, &key(i), b"before").unwrap();
            }
            write.commit().unwrap();
        },
        |write| {
            let tree = write.open_tree("t").unwrap();
            for i in 40..100 {
                write.set(&tree, &key(i), b"split").unwrap();
            }
            for i in (0..100).filter(|i| i % 10 != 0) {
                assert!(write.remove(&tree, &key(i)).unwrap());
            }
        },
    );
}

/// AC-A16 (G1, G5, A5): interrupted at every point where its commit writes or flushes (the journal), the write of
/// [`catalog_and_key_operations`] leaves the list of trees, every Leer and every count as before or as after, never
/// half of it.
#[test]
fn ac_a16_crashes_during_a_commit_of_catalog_and_key_operations() {
    catalog_and_key_operations(false);
}

/// AC-A16 also at every point of the close that moves that commit into `krummi.data`:
/// `cargo test -p krummi --test tree_integrity -- --ignored through_the_close`.
#[test]
#[ignore = "acceptance: every point of the commit and of the close that follows it"]
fn ac_a16_crashes_through_the_close() {
    catalog_and_key_operations(true);
}

/// AC-B18, first part (G1, G5): interrupted at every point where its commit writes or flushes, the write of
/// [`splits_and_merges`] leaves the tree with all its entries of before or all those of after, and it can be read
/// and modified.
#[test]
fn ac_b18_crashes_during_a_commit_with_splits_and_merges() {
    splits_and_merges(false);
}

/// AC-B18 also at every point of the close that moves that commit into `krummi.data` (see above).
#[test]
#[ignore = "acceptance: every point of the commit and of the close that follows it"]
fn ac_b18_crashes_through_the_close() {
    splits_and_merges(true);
}

/// The crash sweep of AC-R12: a write that removes intervals of a tree of several levels, freeing whole subtrees
/// and merging the children at the bounds until the tree loses levels, and a prefix.
fn range_removals(through_close: bool) {
    let key = |i: usize| long_key(i, 2048);
    let keys: Vec<Vec<u8>> = (0..100).map(key).collect();
    crash_sweep(
        &keys,
        through_close,
        |store| {
            let mut write = store.write().unwrap();
            let tree = write.open_tree_with("t", OpenMode::CreateNew).unwrap();
            for key in &keys {
                write.set(&tree, key, b"before").unwrap();
            }
            write.commit().unwrap();
        },
        |write| {
            let tree = write.open_tree("t").unwrap();
            let (lower, upper) = (key(10), key(90));
            assert_eq!(
                write.remove_range(&tree, &lower[..]..&upper[..]).unwrap(),
                80
            );
            // The keys from 90 to 99.
            assert_eq!(write.remove_range_prefix(&tree, b"0009", ..).unwrap(), 10);
            assert_eq!(write.range(&tree, ..).unwrap().count(), 10);
        },
    );
}

/// AC-R12, first part (G1, G5, AC-B18): interrupted at every point where its commit writes or flushes, the write of
/// [`range_removals`] leaves the tree with all its entries of before or all those of after, and it can be read and
/// modified.
#[test]
fn ac_r12_crashes_during_a_commit_with_range_removals() {
    range_removals(false);
}

/// AC-R12 also at every point of the close that moves that commit into `krummi.data` (see above).
#[test]
#[ignore = "acceptance: every point of the commit and of the close that follows it"]
fn ac_r12_crashes_through_the_close() {
    range_removals(true);
}

/// A value of `len` bytes, different for each `seed`, whose pages never hold the same bytes.
fn pattern(len: usize, seed: u8) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8 ^ seed).collect()
}

/// The crash sweep of AC-V8: over a tree whose leaves hold values in them and off them (keys of 100 bytes keep a
/// value of up to 3,974 bytes in its leaf), a write that stores values off their leaf, overwrites them with larger
/// and smaller ones and with values in their leaf, overwrites values in their leaf with values off it, and removes
/// values off their leaf.
fn values_off_their_leaf(through_close: bool) {
    let key = |i: usize| long_key(i, 100);
    let keys: Vec<Vec<u8>> = (0..16).map(key).collect();
    // Off its leaf for odd `i`, in a run of one, two or three pages.
    let before = |i: usize| match i % 2 {
        1 => pattern(5_000 + i / 2 % 3 * PAGE_SIZE, i as u8),
        _ => pattern(100, i as u8),
    };
    crash_sweep(
        &keys,
        through_close,
        |store| {
            let mut write = store.write().unwrap();
            let tree = write.open_tree_with("t", OpenMode::CreateNew).unwrap();
            for i in 0..12 {
                write.set(&tree, &key(i), &before(i)).unwrap();
            }
            write.commit().unwrap();
        },
        |write| {
            let tree = write.open_tree("t").unwrap();
            write.set(&tree, &key(1), &pattern(20_000, 101)).unwrap();
            write.set(&tree, &key(5), &pattern(4_500, 105)).unwrap();
            write.set(&tree, &key(3), b"in its leaf").unwrap();
            write.set(&tree, &key(2), &pattern(9_000, 102)).unwrap();
            assert!(write.remove(&tree, &key(7)).unwrap());
            assert!(write.remove(&tree, &key(9)).unwrap());
            for i in 12..16 {
                write
                    .set(&tree, &key(i), &pattern(6_000 * (i - 11), i as u8))
                    .unwrap();
            }
        },
    );
}

/// AC-V8, first part (V3, G1, G5, AC-B18): interrupted at every point where its commit writes or flushes, the write
/// of [`values_off_their_leaf`] leaves every key with its value of before or that of after, all at once, and the
/// count matching the keys found; the tree can be read and modified, values off their leaf included.
#[test]
fn ac_v8_crashes_during_a_commit_with_values_off_their_leaf() {
    values_off_their_leaf(false);
}

/// AC-V8 also at every point of the close that moves that commit, its value runs included, into `krummi.data` (see
/// above).
#[test]
#[ignore = "acceptance: every point of the commit and of the close that follows it"]
fn ac_v8_crashes_through_the_close() {
    values_off_their_leaf(true);
}

/// The entries a scan of the whole tree returns from one end until it ends or fails, as [`check_items`] checks
/// them.
fn check_scan(
    read: &krummi::ReadTransaction<'_>,
    tree: &krummi::ReadTree<'_>,
    entries: &[(Vec<u8>, Vec<u8>)],
    front: bool,
    offset: usize,
) {
    let scan = read.range(tree, ..).unwrap();
    let own = |(key, value): (&[u8], &[u8])| (key.to_vec(), value.to_vec());
    check_items(scan, entries, own, front, offset);
}

/// The items a scan returns from one end, owned with `own`, until it ends or fails: a failure must be of the damage
/// class, after which the scan returns nothing more from either end, and every item before it must be the next one
/// of `expected` from that end.
fn check_items<I, T: PartialEq + std::fmt::Debug>(
    mut scan: impl DoubleEndedIterator<Item = krummi::Result<I>>,
    expected: &[T],
    own: impl Fn(I) -> T,
    front: bool,
    offset: usize,
) {
    let mut expected: Box<dyn Iterator<Item = &T>> = if front {
        Box::new(expected.iter())
    } else {
        Box::new(expected.iter().rev())
    };
    loop {
        let step = if front { scan.next() } else { scan.next_back() };
        match step {
            None => {
                assert!(
                    expected.next().is_none(),
                    "byte {offset}: the scan ended early"
                );
                return;
            }
            Some(Ok(item)) => {
                let want = expected.next().expect("no item beyond the tree's");
                assert_eq!(own(item), *want, "byte {offset}");
            }
            Some(Err(error)) => {
                assert_eq!(error.kind(), ErrorKind::Damaged, "byte {offset}: {error}");
                assert!(
                    scan.next().is_none() && scan.next_back().is_none(),
                    "byte {offset}"
                );
                return;
            }
        }
    }
}

/// AC10 of the root PRD with trees (G6, D10), and with a value off its leaf (Large values PRD: AC-V11, V7): every
/// altered byte of a small store with trees, read whole through the public API, gives the damage class, "not a store"
/// (the mark) or no observable difference: the same trees, counts and values, never different ones, nor a value off
/// its leaf with a different byte. The suite alters one byte in `step`, plus the header of the value's run and the
/// first and last byte of each of its pages; the acceptance run, every byte.
fn ac10_trees_sweep(step: usize) {
    // The sweep checks damage detection, not durability: its flushes to disk are skipped.
    testing::skip_syncs(true);
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let store = open(&source);
    // A tree of two levels or more, a small one with a value off its leaf and an empty one, after a second write that
    // frees pages: of the first tree, and the value's run of one page, which it overwrites with a run of two pages.
    // The store keeps the size it had without the value, so that the sample costs what it did.
    type Entries = Vec<(Vec<u8>, Vec<u8>)>;
    let mut trees: Vec<(String, Entries)> = vec![
        (
            "big".into(),
            (0..24)
                .map(|i| (long_key(i, 900), vec![i as u8; 50]))
                .collect(),
        ),
        ("empty".into(), Vec::new()),
        (
            "small".into(),
            vec![
                (b"k".to_vec(), b"v".to_vec()),
                (b"large".to_vec(), pattern(5_000, 1)),
                (b"none".to_vec(), Vec::new()),
            ],
        ),
    ];
    let mut write = store.write().unwrap();
    for (name, entries) in &trees {
        let tree = write.open_tree_with(name, OpenMode::CreateNew).unwrap();
        for (key, value) in entries {
            write.set(&tree, key, value).unwrap();
        }
    }
    write.commit().unwrap();
    let mut write = store.write().unwrap();
    let big = write.open_tree("big").unwrap();
    for (key, _) in trees[0].1.drain(..12) {
        assert!(write.remove(&big, &key).unwrap());
    }
    let small = write.open_tree("small").unwrap();
    let large = pattern(PAGE_SIZE + 100, 2);
    write.set(&small, b"large", &large).unwrap();
    trees[2].1[1].1 = large;
    write.commit().unwrap();
    store.close().unwrap();
    let (first, count) = support::value_run(&source, "small", b"large").unwrap();
    assert_eq!(count, 2, "the value is off its leaf, in a run of two pages");

    let pristine = fs::read(source.join(DATA_FILE)).unwrap();
    let copy = tmp.path().join("copy");
    fs::create_dir(&copy).unwrap();
    let names: Vec<(String, TreeKind)> = trees.iter().map(|(n, _)| (n.clone(), NORMAL)).collect();
    let damaged = |error: krummi::Error, offset: usize| {
        assert_eq!(error.kind(), ErrorKind::Damaged, "byte {offset}: {error}");
    };
    let run = first as usize * PAGE_SIZE..(first + count) as usize * PAGE_SIZE;
    let edges = run
        .clone()
        .step_by(PAGE_SIZE)
        .flat_map(|page| [page, page + PAGE_SIZE - 1]);
    let offsets: BTreeSet<usize> = (0..pristine.len())
        .step_by(step)
        .chain(run.start..run.start + PAGE_HEADER_SIZE)
        .chain(edges)
        .collect();
    each_altered_byte(&pristine, &copy, offsets, |store, offset| {
        let read = store.read().unwrap();
        match read.list_trees() {
            Ok(list) => assert_eq!(list, names, "byte {offset}"),
            Err(error) => damaged(error, offset),
        }
        for (name, entries) in &trees {
            let tree = match read.open_tree(name) {
                Ok(tree) => tree,
                Err(error) => {
                    damaged(error, offset);
                    continue;
                }
            };
            assert_eq!(
                read.len(&tree).unwrap(),
                entries.len() as u64,
                "byte {offset}"
            );
            for (key, value) in entries {
                match read.get(&tree, key) {
                    Ok(found) => assert_eq!(found, Some(&value[..]), "byte {offset}"),
                    Err(error) => damaged(error, offset),
                }
            }
            match read.get(&tree, b"absent") {
                Ok(found) => assert_eq!(found, None, "byte {offset}"),
                Err(error) => damaged(error, offset),
            }
            // The whole tree scanned from each end (In-order scan PRD: root AC10 with scans).
            check_scan(&read, &tree, entries, true, offset);
            check_scan(&read, &tree, entries, false, offset);
        }
    });
    testing::reset();
}

/// Writes `pristine`, the data file of a closed store, into the folder `copy` with the byte at each of `offsets`
/// altered, one at a time, and opens it: a store that does not open fails with the damage class or as not a store
/// (the mark); one that opens goes to `read_all` with the offset.
fn each_altered_byte(
    pristine: &[u8],
    copy: &Path,
    offsets: impl IntoIterator<Item = usize>,
    read_all: impl Fn(&Store, usize),
) {
    for offset in offsets {
        let mut bytes = pristine.to_vec();
        bytes[offset] ^= 0xFF;
        fs::write(copy.join(DATA_FILE), &bytes).unwrap();
        match Store::open(copy, &StoreOptions::default()) {
            Ok(store) => read_all(&store, offset),
            Err(error) => assert!(
                matches!(error.kind(), ErrorKind::Damaged | ErrorKind::NotAStore),
                "byte {offset}: {error}"
            ),
        }
    }
}

#[test]
fn ac10_sample_of_altered_bytes_with_trees() {
    ac10_trees_sweep(397);
}

/// `cargo test -p krummi --test tree_integrity -- --ignored ac10`.
#[test]
#[ignore = "acceptance: alters every byte of a store with trees"]
fn ac10_every_altered_byte_with_trees() {
    ac10_trees_sweep(1);
}

/// What a multi-value tree holds: each key with its values (Multi-value trees PRD, M1).
type MultiModel = BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>>;

/// The number of entries of a multi-value tree, and each key with its number of values and its values.
type MultiSeen = (u64, Vec<(Vec<u8>, u64, Vec<Vec<u8>>)>);

/// What the tests see of the multi-value tree `m`, the store's only tree: its number of entries and each key with its
/// number of values and its values, checking that each key's number of values is the number its value scan finds.
fn multi_seen(store: &Store) -> MultiSeen {
    let read = store.read().unwrap();
    assert_eq!(read.list_trees().unwrap(), [("m".to_owned(), MULTI)]);
    let tree = read.open_multi_tree("m").unwrap();
    let keys = read
        .keys(&tree, ..)
        .unwrap()
        .map(|item| {
            let (key, count) = item.unwrap();
            let values: Vec<Vec<u8>> = read
                .values(&tree, key, ..)
                .unwrap()
                .map(|value| value.unwrap().to_vec())
                .collect();
            assert_eq!(values.len() as u64, count, "the number of values");
            assert_eq!(read.value_count(&tree, key).unwrap(), count);
            (key.to_vec(), count, values)
        })
        .collect();
    (read.len(&tree).unwrap(), keys)
}

/// The crash sweep of AC-M14: over a multi-value tree whose keys of 2,000 bytes spread over several leaves, with a
/// key `a` with its values in its entry, near the promotion threshold, and keys `b` and `c` with theirs in subtrees, a
/// write that moves `a` to a subtree and `b` back to its entry, adds values of 2,048 bytes to `c` and removes most of
/// them, so that its subtree splits and merges pages, and adds keys and removes most of them, so that the tree does.
fn multi_values(through_close: bool) {
    let filler = |i: usize| long_key(i, 2000);
    let long = |j: usize| long_key(j, krummi_tree::MAX_KEY);
    crash_sweep_with(
        through_close,
        |store| {
            let mut write = store.write().unwrap();
            let tree = write
                .open_multi_tree_with("m", OpenMode::CreateNew)
                .unwrap();
            for i in 0..300 {
                if i < 200 {
                    write.add(&tree, b"a", &value16(i)).unwrap();
                }
                write.add(&tree, b"b", &value16(i)).unwrap();
            }
            for j in 0..10 {
                write.add(&tree, b"c", &long(j)).unwrap();
            }
            for i in 0..10 {
                write.add(&tree, &filler(i), b"x").unwrap();
            }
            assert!(write.subtree_of(&tree, b"a").unwrap().is_none());
            assert!(write.subtree_of(&tree, b"b").unwrap().is_some());
            write.commit().unwrap();
        },
        |write| {
            let tree = write.open_multi_tree("m").unwrap();
            for i in 200..260 {
                assert!(write.add(&tree, b"a", &value16(i)).unwrap());
            }
            for i in 100..300 {
                assert!(write.remove_value(&tree, b"b", &value16(i)).unwrap());
            }
            assert!(write.subtree_of(&tree, b"a").unwrap().is_some(), "promoted");
            assert!(write.subtree_of(&tree, b"b").unwrap().is_none(), "demoted");
            for j in 10..40 {
                assert!(write.add(&tree, b"c", &long(j)).unwrap());
            }
            for j in (0..40).filter(|j| j % 4 != 0) {
                assert!(write.remove_value(&tree, b"c", &long(j)).unwrap());
            }
            for i in 10..40 {
                assert!(write.add(&tree, &filler(i), b"x").unwrap());
                assert!(write.add(&tree, &filler(i), b"y").unwrap());
            }
            for i in (0..40).filter(|i| i % 4 != 0) {
                assert!(write.remove_key(&tree, &filler(i)).unwrap() > 0);
            }
        },
        multi_seen,
        |store, _| {
            let mut write = store.write().unwrap();
            let tree = write.open_multi_tree("m").unwrap();
            write.add(&tree, b"a", b"new").unwrap();
            write.remove_key(&tree, b"b").unwrap();
            write.add(&tree, b"c", &long(99)).unwrap();
            assert!(write.contains(&tree, b"c", &long(0)).unwrap());
            write.commit().unwrap();
        },
    );
}

/// AC-M14, first part (G1, G5, AC-B18): interrupted at every point where its commit writes or flushes, the write of
/// [`multi_values`] leaves the multi-value tree with every key, its values and its number of values, and the number
/// of entries, all as before or all as after; and a new write reads and modifies it.
#[test]
fn ac_m14_crashes_during_a_commit_that_moves_values_and_splits_and_merges() {
    multi_values(false);
}

/// AC-M14 also at every point of the close that moves that commit into `krummi.data` (see above).
#[test]
#[ignore = "acceptance: every point of the commit and of the close that follows it"]
fn ac_m14_crashes_through_the_close() {
    multi_values(true);
}

/// AC-M15 (M11, G6, AC10): AC10 with a multi-value tree whose keys have their values in their entries and in a
/// subtree, read whole through the public API: every altered byte gives the damage class, "not a store" (the mark) or
/// no observable difference: the same tree, number of entries, keys with their numbers of values, and values, from
/// either end and with Contains (the first, the last and an absent value), never a value with a different byte. The
/// suite alters one byte in `step`, plus the header and the last byte of the subtree's root and every byte of the
/// entry of its key that points to it; the acceptance run, every byte.
fn ac10_multi_sweep(step: usize) {
    // The sweep checks damage detection, not durability: its flushes to disk are skipped.
    testing::skip_syncs(true);
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let store = open(&source);
    let mut model = MultiModel::from([
        (
            b"i".to_vec(),
            [&b""[..], b"a", b"bb", b"\xFF\xFF"]
                .map(<[u8]>::to_vec)
                .into(),
        ),
        (
            b"s".to_vec(),
            (0..300).map(value16).chain([Vec::new()]).collect(),
        ),
        (b"t".to_vec(), [b"x".to_vec()].into()),
    ]);
    let mut write = store.write().unwrap();
    let tree = write
        .open_multi_tree_with("m", OpenMode::CreateNew)
        .unwrap();
    for (key, values) in &model {
        for value in values {
            write.add(&tree, key, value).unwrap();
        }
    }
    write.commit().unwrap();
    // A second write that frees pages of the subtree and adds a key.
    let mut write = store.write().unwrap();
    let tree = write.open_multi_tree("m").unwrap();
    for i in 200..300 {
        assert!(write.remove_value(&tree, b"s", &value16(i)).unwrap());
        model.get_mut(&b"s"[..]).unwrap().remove(&value16(i));
    }
    write.add(&tree, b"u", b"u").unwrap();
    model.insert(b"u".to_vec(), [b"u".to_vec()].into());
    assert!(write.subtree_of(&tree, b"s").unwrap().is_some());
    write.commit().unwrap();
    store.close().unwrap();

    let pristine = fs::read(source.join(DATA_FILE)).unwrap();
    let (subtree, _) = subtree_root(&source, "m", b"s");
    let leaf = leaf_page(&source, Some("m"), b"s") as usize * PAGE_SIZE;
    let values = &model[&b"s"[..]];
    // The entry of `s` as D1 lays it out: tag, flags (the empty value), number of values, inline bytes, root.
    let mut blob = vec![1, 1];
    for field in [
        values.len() as u64,
        volume(0, values.iter().map(Vec::len)) as u64,
        subtree,
    ] {
        blob.extend(field.to_le_bytes());
    }
    let entry = pristine[leaf..leaf + PAGE_SIZE]
        .windows(blob.len())
        .position(|window| window == blob)
        .expect("the entry of the key with a subtree")
        + leaf;
    let root = subtree as usize * PAGE_SIZE;
    let offsets: BTreeSet<usize> = (0..pristine.len())
        .step_by(step)
        .chain(root..root + PAGE_HEADER_SIZE)
        .chain([root + PAGE_SIZE - 1])
        .chain(entry..entry + blob.len())
        .collect();

    let copy = tmp.path().join("copy");
    fs::create_dir(&copy).unwrap();
    let damaged = |error: krummi::Error, offset: usize| {
        assert_eq!(error.kind(), ErrorKind::Damaged, "byte {offset}: {error}");
    };
    let counts: Vec<(Vec<u8>, u64)> = model
        .iter()
        .map(|(key, values)| (key.clone(), values.len() as u64))
        .collect();
    let own_key = |(key, count): (&[u8], u64)| (key.to_vec(), count);
    each_altered_byte(&pristine, &copy, offsets, |store, offset| {
        let read = store.read().unwrap();
        match read.list_trees() {
            Ok(list) => assert_eq!(list, [("m".to_owned(), MULTI)], "byte {offset}"),
            Err(error) => damaged(error, offset),
        }
        let tree = match read.open_multi_tree("m") {
            Ok(tree) => tree,
            Err(error) => return damaged(error, offset),
        };
        let entries = read.len(&tree).unwrap();
        assert_eq!(entries, model.len() as u64, "byte {offset}");
        for front in [true, false] {
            let scan = read.keys(&tree, ..).unwrap();
            check_items(scan, &counts, own_key, front, offset);
        }
        for (key, values) in &model {
            match read.value_count(&tree, key) {
                Ok(count) => assert_eq!(count, values.len() as u64, "byte {offset}"),
                Err(error) => damaged(error, offset),
            }
            let values: Vec<Vec<u8>> = values.iter().cloned().collect();
            for front in [true, false] {
                let scan = read.values(&tree, key, ..).unwrap();
                check_items(scan, &values, <[u8]>::to_vec, front, offset);
            }
            // Contains with the first and the last value and an absent one: the scans already read every value.
            let ends = [values.first(), values.last()].map(|value| (&value.unwrap()[..], true));
            for (value, there) in ends.into_iter().chain([(&b"absent"[..], false)]) {
                match read.contains(&tree, key, value) {
                    Ok(found) => assert_eq!(found, there, "byte {offset}: {key:?} {value:?}"),
                    Err(error) => damaged(error, offset),
                }
            }
        }
    });
    testing::reset();
}

#[test]
fn ac10_sample_of_altered_bytes_with_a_multi_value_tree() {
    ac10_multi_sweep(397);
}

/// `cargo test -p krummi --test tree_integrity -- --ignored ac10`.
#[test]
#[ignore = "acceptance: alters every byte of a store with a multi-value tree"]
fn ac10_every_altered_byte_with_a_multi_value_tree() {
    ac10_multi_sweep(1);
}

/// The length of the values of the fixed-size tree of AC-X12: eight entries fill a leaf, and a leaf of one or two is
/// under a quarter full, so that removals merge leaves.
const CRASH_LEN: usize = 1000;

/// The number of entries of a fixed-size tree, and its entries.
type FixedSeen = (u64, Vec<(u64, Vec<u8>)>);

/// What the tests see of the fixed-size tree `f` of values of `len` bytes, the store's only tree: its number of
/// entries and its entries from a whole scan, checking that their number is the number of entries.
fn fixed_seen(store: &Store, len: usize) -> FixedSeen {
    let read = store.read().unwrap();
    let kind = TreeKind::FixedSize(len as u16);
    assert_eq!(read.list_trees().unwrap(), [("f".to_owned(), kind)]);
    let tree = read.open_fixed_tree("f", len).unwrap();
    let entries: Vec<(u64, Vec<u8>)> = read
        .range_fixed(&tree, ..)
        .unwrap()
        .map(|item| {
            let (key, value) = item.unwrap();
            (key, value.to_vec())
        })
        .collect();
    let count = read.len(&tree).unwrap();
    assert_eq!(count, entries.len() as u64, "the number of entries");
    (count, entries)
}

/// The crash sweep of AC-X12: over a fixed-size tree of 40 entries in five leaves, a write that adds keys past the
/// last one and between the others, so that leaves split by both paths, removes most keys one by one and an interval,
/// so that leaves merge and whole leaves go away, overwrites a value and adds one with Añadir.
fn fixed_values(through_close: bool) {
    let set = |write: &mut WriteTransaction<'_>, tree: &krummi::WriteFixedTree<'_>, key: u64| {
        let value = fixed_value(key, CRASH_LEN);
        write.set_fixed(tree, key, &value).unwrap();
    };
    crash_sweep_with(
        through_close,
        |store| {
            let mut write = store.write().unwrap();
            let tree = write
                .open_fixed_tree_with("f", CRASH_LEN, OpenMode::CreateNew)
                .unwrap();
            for key in (0..400).step_by(10) {
                set(&mut write, &tree, key);
            }
            write.commit().unwrap();
        },
        |write| {
            let tree = write.open_fixed_tree("f", CRASH_LEN).unwrap();
            for key in (400..800).step_by(10).chain((5..400).step_by(10)) {
                set(write, &tree, key);
            }
            let added = |key: &u64| key.is_multiple_of(10) || *key < 400;
            for key in (0..800)
                .step_by(5)
                .filter(|key| key % 30 != 0 && added(key))
            {
                assert!(write.remove_fixed(&tree, key).unwrap(), "{key}");
            }
            assert_eq!(write.remove_range_fixed(&tree, 300..=600).unwrap(), 11);
            write.set_fixed(&tree, 0, &[0xEE; CRASH_LEN]).unwrap();
            assert!(write.try_add_fixed(&tree, 1, &[0xAA; CRASH_LEN]).unwrap());
        },
        |store| fixed_seen(store, CRASH_LEN),
        |store, _| {
            let mut write = store.write().unwrap();
            let tree = write.open_fixed_tree("f", CRASH_LEN).unwrap();
            write.set_fixed(&tree, 2, &[2; CRASH_LEN]).unwrap();
            write.remove_fixed(&tree, 0).unwrap();
            assert!(write.get_fixed(&tree, 30).unwrap().is_some());
            write.commit().unwrap();
        },
    );
}

/// AC-X12, first part (G1, G5, AC-B18): interrupted at every point where its commit writes or flushes, the write of
/// [`fixed_values`] leaves the fixed-size tree with all its entries and their number as before or all as after; and a
/// new write reads and modifies it.
#[test]
fn ac_x12_crashes_during_a_commit_with_splits_and_merges_of_a_fixed_size_tree() {
    fixed_values(false);
}

/// AC-X12 also at every point of the close that moves that commit into `krummi.data`:
/// `cargo test -p krummi --test tree_integrity -- --ignored ac_x12`.
#[test]
#[ignore = "acceptance: every point of the commit and of the close that follows it"]
fn ac_x12_crashes_through_the_close() {
    fixed_values(true);
}

/// The length of the values of the fixed-size tree of AC-X13: 75 entries fill a leaf.
const AC10_LEN: usize = 100;

/// AC-X13 (X10, G6, AC10): AC10 with a fixed-size tree of two levels, read whole through the public API: every altered
/// byte gives the damage class, "not a store" (the mark) or no observable difference: the same tree and kind, number
/// of entries, entries from either end and Leer of every key and of absent ones, never a key or a value with a
/// different byte. The suite alters one byte in `step`, plus the headers of the root and of a leaf (the page header
/// and the node header of D1); the acceptance run, every byte.
fn ac10_fixed_sweep(step: usize) {
    // The sweep checks damage detection, not durability: its flushes to disk are skipped.
    testing::skip_syncs(true);
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let store = open(&source);
    let mut model = fixed_model((0..200).map(|i| i * 7), AC10_LEN);
    let mut write = store.write().unwrap();
    let tree = write
        .open_fixed_tree_with("f", AC10_LEN, OpenMode::CreateNew)
        .unwrap();
    for (key, value) in &model {
        write.set_fixed(&tree, *key, value).unwrap();
    }
    write.commit().unwrap();
    // A second write that frees pages: an interval that empties a leaf and merges the others, and a new key.
    let mut write = store.write().unwrap();
    let tree = write.open_fixed_tree("f", AC10_LEN).unwrap();
    let removed = write.remove_range_fixed(&tree, 300..1200).unwrap();
    model.retain(|key, _| !(300..1200).contains(key));
    assert_eq!(removed, 129);
    write.set_fixed(&tree, 1, &[0xFF; AC10_LEN]).unwrap();
    model.insert(1, vec![0xFF; AC10_LEN]);
    write.commit().unwrap();
    store.close().unwrap();
    assert_eq!(fixed_height(&source, "f", AC10_LEN), 2, "two levels");

    let pristine = fs::read(source.join(DATA_FILE)).unwrap();
    let root = support::root_page(&source, "f") as usize * PAGE_SIZE;
    let leaf = fixed_leaf_page(&source, "f", AC10_LEN, 0) as usize * PAGE_SIZE;
    // The page header and the node header (D1: kind, level, count, length and leftmost child).
    let headers = PAGE_HEADER_SIZE + 16;
    let offsets: BTreeSet<usize> = (0..pristine.len())
        .step_by(step)
        .chain(root..root + headers)
        .chain(leaf..leaf + headers)
        .collect();

    let copy = tmp.path().join("copy");
    fs::create_dir(&copy).unwrap();
    let damaged = |error: krummi::Error, offset: usize| {
        assert_eq!(error.kind(), ErrorKind::Damaged, "byte {offset}: {error}");
    };
    let kind = TreeKind::FixedSize(AC10_LEN as u16);
    let entries: Vec<(u64, Vec<u8>)> = model.clone().into_iter().collect();
    let own = |(key, value): (u64, &[u8])| (key, value.to_vec());
    each_altered_byte(&pristine, &copy, offsets, |store, offset| {
        let read = store.read().unwrap();
        match read.list_trees() {
            Ok(list) => assert_eq!(list, [("f".to_owned(), kind)], "byte {offset}"),
            Err(error) => damaged(error, offset),
        }
        let tree = match read.open_fixed_tree("f", AC10_LEN) {
            Ok(tree) => tree,
            Err(error) => return damaged(error, offset),
        };
        let count = read.len(&tree).unwrap();
        assert_eq!(count, model.len() as u64, "byte {offset}");
        for front in [true, false] {
            let scan = read.range_fixed(&tree, ..).unwrap();
            check_items(scan, &entries, own, front, offset);
        }
        for (key, value) in &model {
            match read.get_fixed(&tree, *key) {
                Ok(found) => assert_eq!(found, Some(&value[..]), "byte {offset}: {key}"),
                Err(error) => damaged(error, offset),
            }
        }
        for key in [2, 300, 1394, u64::MAX] {
            match read.get_fixed(&tree, key) {
                Ok(found) => assert_eq!(found, None, "byte {offset}: {key}"),
                Err(error) => damaged(error, offset),
            }
        }
    });
    testing::reset();
}

#[test]
fn ac_x13_sample_of_altered_bytes_with_a_fixed_size_tree() {
    ac10_fixed_sweep(397);
}

/// `cargo test -p krummi --test tree_integrity -- --ignored ac_x13`.
#[test]
#[ignore = "acceptance: alters every byte of a store with a fixed-size tree"]
fn ac_x13_every_altered_byte_with_a_fixed_size_tree() {
    ac10_fixed_sweep(1);
}
