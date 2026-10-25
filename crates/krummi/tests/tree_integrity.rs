//! Crashes, power cuts and altered bytes in stores with trees, on disk (Trees and key operations PRD: AC-A16; Trees
//! PRD: AC-B18, its first part; root PRD: AC10 repeated with the trees). The failures of T11 in the middle of a
//! split or a merge, the second part of AC-B18, need the engine's hooks: they are in `src/tests.rs`.
//!
//! The crash sweeps record the file operations of a run and rebuild, for every point of the tested commit, what a
//! power cut (the models of `POWER_CUTS`) or a crash of the process (`Model::Written`: the system keeps every write
//! it was given) leaves, with the machinery of the transaction tests.

#[path = "../../krummi-tx/tests/common/mod.rs"]
mod common;

use std::fs;

use common::{POWER_CUTS, Run};
use krummi::{ErrorKind, OpenMode, Store, StoreOptions, TreeKind, WriteTransaction};
use krummi_pager::DATA_FILE;
use krummi_pager::testing::{self, Model};

const NORMAL: TreeKind = TreeKind::Normal;

fn open(folder: &std::path::Path) -> Store {
    Store::open(folder, &StoreOptions::default()).unwrap()
}

/// A key of `len` bytes that starts with `i`, so that keys sort by `i`.
fn long_key(i: usize, len: usize) -> Vec<u8> {
    let mut key = format!("{i:05}").into_bytes();
    key.resize(len, b'.');
    key
}

/// What the tests see of a store's trees: each tree of the list with its count and what Leer finds for every key
/// the test used, in order.
type Seen = Vec<(String, u64, Vec<Option<Vec<u8>>>)>;

/// What a new read sees, checking that each tree's count is the number of keys Leer finds (A5).
fn seen(store: &Store, keys: &[Vec<u8>]) -> Seen {
    let read = store.read().unwrap();
    let list = read.list_trees().unwrap();
    list.into_iter()
        .map(|(name, kind)| {
            let tree = read.open_tree(&name, kind).unwrap();
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
                let tree = write.open_tree(name, NORMAL).unwrap();
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
        let tree = write
            .open_tree_with(name, NORMAL, OpenMode::OpenOrCreate)
            .unwrap();
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
            let created = write
                .open_tree_with("e", NORMAL, OpenMode::CreateNew)
                .unwrap();
            for i in 40..80 {
                write.set(&created, &key(i), b"created").unwrap();
            }
            write.rename_tree("a", "f", false).unwrap();
            let renamed = write.open_tree("f", NORMAL).unwrap();
            for i in 5..35 {
                assert!(write.remove(&renamed, &key(i)).unwrap());
            }
            for i in 60..90 {
                write.set(&renamed, &key(i), b"renamed").unwrap();
            }
            let emptied = write.open_tree_with("b", NORMAL, OpenMode::Empty).unwrap();
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
            let tree = write
                .open_tree_with("t", NORMAL, OpenMode::CreateNew)
                .unwrap();
            for i in 0..40 {
                write.set(&tree, &key(i), b"before").unwrap();
            }
            write.commit().unwrap();
        },
        |write| {
            let tree = write.open_tree("t", NORMAL).unwrap();
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

/// AC10 of the root PRD with trees (G6, D10): every altered byte of a small store with trees, read whole through
/// the public API, gives the damage class, "not a store" (the mark) or no observable difference: the same trees,
/// counts and values, never different ones. The suite alters one byte in `step`; the acceptance run, every byte.
fn ac10_trees_sweep(step: usize) {
    // The sweep checks damage detection, not durability: its flushes to disk are skipped.
    testing::skip_syncs(true);
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let store = open(&source);
    // A tree of two levels or more, a small one and an empty one, after a second write that frees pages.
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
                (b"none".to_vec(), Vec::new()),
            ],
        ),
    ];
    let mut write = store.write().unwrap();
    for (name, entries) in &trees {
        let tree = write
            .open_tree_with(name, NORMAL, OpenMode::CreateNew)
            .unwrap();
        for (key, value) in entries {
            write.set(&tree, key, value).unwrap();
        }
    }
    write.commit().unwrap();
    let mut write = store.write().unwrap();
    let big = write.open_tree("big", NORMAL).unwrap();
    for (key, _) in trees[0].1.drain(..12) {
        assert!(write.remove(&big, &key).unwrap());
    }
    write.commit().unwrap();
    store.close().unwrap();

    let pristine = fs::read(source.join(DATA_FILE)).unwrap();
    let copy = tmp.path().join("copy");
    fs::create_dir(&copy).unwrap();
    let names: Vec<(String, TreeKind)> = trees.iter().map(|(n, _)| (n.clone(), NORMAL)).collect();
    let damaged = |error: krummi::Error, offset: usize| {
        assert_eq!(error.kind(), ErrorKind::Damaged, "byte {offset}: {error}");
    };
    for offset in (0..pristine.len()).step_by(step) {
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
            let tree = match read.open_tree(name, NORMAL) {
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
