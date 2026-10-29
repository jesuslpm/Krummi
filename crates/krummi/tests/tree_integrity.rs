//! Crashes, power cuts and altered bytes in stores with trees, on disk (Trees and key operations PRD: AC-A16; Trees
//! PRD: AC-B18, its first part; In-order scan PRD: AC-R12, its first part; Large values PRD: AC-V8, its first part,
//! and AC-V11; root PRD: AC10 repeated with the trees, their scans and a value off its leaf). The failures of T11 in
//! the middle of a split, a merge or a range removal, or at the page operations of values off their leaf, the second
//! part of AC-B18, AC-R12 and AC-V8, need the engine's hooks: they are in `src/tests.rs`.
//!
//! The crash sweeps record the file operations of a run and rebuild, for every point of the tested commit, what a
//! power cut (the models of `POWER_CUTS`) or a crash of the process (`Model::Written`: the system keeps every write
//! it was given) leaves, with the machinery of the transaction tests.

#[path = "../../krummi-tx/tests/common/mod.rs"]
mod common;
mod support;

use std::collections::BTreeSet;
use std::fs;

use common::{POWER_CUTS, Run};
use krummi::{ErrorKind, OpenMode, Store, StoreOptions, TreeKind, WriteTransaction};
use krummi_pager::DATA_FILE;
use krummi_pager::format::{PAGE_HEADER_SIZE, PAGE_SIZE};
use krummi_pager::testing::{self, Model};
use support::{NORMAL, long_key, open};

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
    let source = Run::begin();
    let store = open(source.path());
    setup(&store);
    let start = testing::recorded_len();
    let before = seen(&store, keys);
    let mut write = store.write().unwrap();
    change(&mut write);
    write.commit().unwrap();
    let returned = testing::recorded_len();
    let after = seen(&store, keys);
    assert_ne!(before, after, "the write changes what the test sees");
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
            let now = seen(&store, keys);
            let label = format!("cut {cut} of {}, {model:?}", run.ops.len());
            if cut >= returned {
                assert!(now == after, "{label}: the commit had returned");
            } else {
                assert!(now == before || now == after, "{label}: half a commit");
            }
            outcomes[usize::from(now == after)] = true;
            let mut write = store.write().unwrap();
            for (name, _, _) in &now {
                let tree = write.open_tree(name).unwrap();
                for key in keys.iter().take(3) {
                    write.get(&tree, key).unwrap();
                }
                write.set(&tree, &keys[0], b"modified").unwrap();
                write.remove(&tree, &keys[1]).unwrap();
            }
            write.commit().unwrap();
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

/// The entries a scan of the whole tree returns from one end until it ends or fails: a failure must be of the
/// damage class, after which the scan returns nothing more from either end, and every entry before it must be
/// the next one expected from that end.
fn check_scan(
    read: &krummi::ReadTransaction<'_>,
    tree: &krummi::ReadTree<'_>,
    entries: &[(Vec<u8>, Vec<u8>)],
    front: bool,
    offset: usize,
) {
    let mut scan = read.range(tree, ..).unwrap();
    let mut expected: Box<dyn Iterator<Item = &(Vec<u8>, Vec<u8>)>> = if front {
        Box::new(entries.iter())
    } else {
        Box::new(entries.iter().rev())
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
            Some(Ok((key, value))) => {
                let (k, v) = expected.next().expect("no entry beyond the tree's");
                assert_eq!((key, value), (&k[..], &v[..]), "byte {offset}");
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
    for offset in offsets {
        let mut bytes = pristine.clone();
        bytes[offset] ^= 0xFF;
        fs::write(copy.join(DATA_FILE), &bytes).unwrap();
        let store = match Store::open(&copy, &StoreOptions::default()) {
            Ok(store) => store,
            Err(error) => {
                assert!(
                    matches!(error.kind(), ErrorKind::Damaged | ErrorKind::NotAStore),
                    "byte {offset}: {error}"
                );
                continue;
            }
        };
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
    }
    testing::reset();
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
