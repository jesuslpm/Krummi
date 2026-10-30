//! Multi-value trees through the public API (Multi-value trees PRD, M1–M14): AC-M1–AC-M8, AC-M10–AC-M13, AC-M17 and
//! AC-M18. What must not compile (AC-M9) is in `trees_compile.rs`; crashes and altered bytes (AC-M14, its first part,
//! and AC-M15) are in `tree_integrity.rs`; where the references of the scans point (AC-M16), a store that refuses
//! writes (the T12 part of AC-M10) and failures in the middle of an operation (AC-M14, its second part) need the
//! engine's test hooks and are in the library's own tests.
//!
//! Each criterion runs on a store on disk and on a temporary one, except the parts that reopen or damage the
//! store's files, which run on disk only. The tests that depend on the thresholds or on where the values of a key
//! are take them from the implementation (`krummi_tree::multi` and the hidden `subtree_of`).

mod support;

use std::collections::BTreeSet;
use std::ops::Bound::{self, Excluded, Included, Unbounded};
use std::ops::RangeBounds;
use std::path::{Path, PathBuf};

use krummi::{Error, OpenMode, Result, SizeSubject, Store, StoreOptions, TreeKind};
use krummi_pager::DATA_FILE;
use krummi_pager::format::PAGE_SIZE;
use krummi_tree::catalog;
use krummi_tree::multi::{multi_thresholds, volume};
use support::*;

/// Collects a scan, owning each item with `own`.
fn collect<T, U>(scan: impl Iterator<Item = Result<T>>, own: impl Fn(T) -> U) -> Vec<U> {
    scan.map(|item| item.map(&own))
        .collect::<Result<_>>()
        .unwrap()
}

/// Collects a scan taking one item from the front and one from the back in turn.
fn alternate<T, U>(
    mut scan: impl DoubleEndedIterator<Item = Result<T>>,
    own: impl Fn(T) -> U,
) -> Vec<U> {
    let (mut front, mut back) = (Vec::new(), Vec::new());
    loop {
        match scan.next() {
            Some(item) => front.push(own(item.unwrap())),
            None => break,
        }
        match scan.next_back() {
            Some(item) => back.push(own(item.unwrap())),
            None => break,
        }
    }
    front.extend(back.into_iter().rev());
    front
}

fn own_key((key, count): (&[u8], u64)) -> (Vec<u8>, u64) {
    (key.to_vec(), count)
}

fn own_value(value: &[u8]) -> Vec<u8> {
    value.to_vec()
}

fn reversed<T>(mut items: Vec<T>) -> Vec<T> {
    items.reverse();
    items
}

/// Each key of `model` with its number of values.
fn counts(model: &Model) -> Vec<(Vec<u8>, u64)> {
    model
        .iter()
        .map(|(key, values)| (key.clone(), values.len() as u64))
        .collect()
}

/// Checks that the multi-value tree `tree` of the transaction `tx`, read or write, holds exactly `model`: its number
/// of entries, its keys with their numbers of values from both ends, and the values of each key from both ends and
/// with Contains.
macro_rules! check {
    ($tx:expr, $tree:expr, $model:expr) => {{
        let (tx, tree, model): (_, _, &Model) = (&$tx, &$tree, &$model);
        assert_eq!(tx.len(tree).unwrap(), model.len() as u64, "entries");
        let want = counts(model);
        assert_eq!(collect(tx.keys(tree, ..).unwrap(), own_key), want, "keys");
        let back = collect(tx.keys(tree, ..).unwrap().rev(), own_key);
        assert_eq!(reversed(back), want, "keys backwards");
        for (key, values) in model {
            let values: Vec<Vec<u8>> = values.iter().cloned().collect();
            let count = tx.value_count(tree, key).unwrap();
            assert_eq!(count, values.len() as u64, "values of {key:?}");
            let scan = tx.values(tree, key, ..).unwrap();
            assert_eq!(collect(scan, own_value), values, "values of {key:?}");
            let back = collect(tx.values(tree, key, ..).unwrap().rev(), own_value);
            assert_eq!(reversed(back), values, "values of {key:?} backwards");
            for value in &values {
                assert!(
                    tx.contains(tree, key, value).unwrap(),
                    "{key:?} has {value:?}"
                );
            }
        }
    }};
}

/// Checks in a new read that the multi-value tree `name` holds exactly `model`.
fn check_read(store: &Store, name: &str, model: &Model) {
    let read = store.read().unwrap();
    let tree = read.open_multi_tree(name).unwrap();
    check!(read, tree, model);
}

/// Runs `scenario` on a store on disk, which then closes and opens again, and checks that the multi-value tree `m`
/// holds the model that `scenario` returns.
fn on_disk_reopened(scenario: impl Fn(&Store) -> Model) {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let model = scenario(&store);
    store.close().unwrap();
    let store = open(tmp.path());
    check_read(&store, "m", &model);
    store.close().unwrap();
}

/// A linear congruential generator, so that the random orders are the same on every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            items.swap(i, j);
        }
    }
}

/// AC-M1 (M1, M5, M6, AC-B7): values of 0, 1 and 2,048 bytes, with every byte and some prefixes of others, added in a
/// random order to keys of 1 and 2,048 bytes, are added once and then not, and come back as one set per key in
/// unsigned byte order, the empty value first and each prefix before the values it starts; in the write, in a later
/// read and, on disk, after reopening.
#[test]
fn ac_m1_the_values_of_a_key_are_a_set_in_byte_order() {
    let scenario = |store: &Store| {
        let keys = [vec![b'k'], long_key(1, MAX_KEY)];
        let mut values = vec![
            vec![],
            vec![0x7F; 2],
            vec![0x7F; 100],
            vec![0x00; MAX_KEY],
            vec![0x7F; MAX_KEY],
            vec![0xFF; MAX_KEY],
        ];
        values.extend((0..=255).map(|byte| vec![byte]));
        let mut adds: Vec<(&[u8], &[u8])> = keys
            .iter()
            .flat_map(|key| values.iter().map(move |value| (&key[..], &value[..])))
            .collect();
        Lcg(1).shuffle(&mut adds);

        let mut model = Model::new();
        let mut write = store.write().unwrap();
        let tree = create_multi(&mut write, "m");
        for (key, value) in adds {
            assert!(
                write.add(&tree, key, value).unwrap(),
                "added the first time"
            );
            model
                .entry(key.to_vec())
                .or_default()
                .insert(value.to_vec());
            assert!(!write.add(&tree, key, value).unwrap(), "not the second");
            let count = write.value_count(&tree, key).unwrap();
            assert_eq!(count, model[key].len() as u64, "the second changes nothing");
        }
        check!(write, tree, model);
        for key in &keys {
            let mut first = write.values(&tree, key, ..).unwrap();
            assert_eq!(first.next().unwrap().unwrap(), b"", "the empty value first");
            assert_eq!(
                first.next().unwrap().unwrap(),
                [0x00],
                "then the least byte"
            );
            for absent in [
                vec![0x7F; 3],
                vec![0x00; MAX_KEY - 1],
                vec![1, 2],
                vec![0xFF; 2],
            ] {
                assert!(!write.contains(&tree, key, &absent).unwrap());
            }
        }
        assert_eq!(write.value_count(&tree, b"absent").unwrap(), 0);
        assert!(!write.contains(&tree, b"absent", b"").unwrap());
        assert_eq!(write.len(&tree).unwrap(), 2, "the number of keys");
        write.commit().unwrap();
        check_read(store, "m", &model);
        model
    };
    stores(|store, _| {
        scenario(store);
    });
    on_disk_reopened(scenario);
}

/// What an invalid-size error reports: what is out of size, the size received, the minimum and the maximum.
type Size = (SizeSubject, u64, u64, u64);

/// The invalid-size error of AC-M2 for a key of `len` bytes.
fn key_error(len: usize) -> Size {
    (SizeSubject::Key, len as u64, 1, MAX_KEY as u64)
}

/// The invalid-size error of AC-M2 for a value of 2,049 bytes.
const VALUE_ERROR: Size = (SizeSubject::Value, MAX_KEY as u64 + 1, 0, MAX_KEY as u64);

/// The invalid-size error of AC-M2 for a prefix of 2,049 bytes.
const PREFIX_ERROR: Size = (SizeSubject::Prefix, MAX_KEY as u64 + 1, 0, MAX_KEY as u64);

/// The size failures of AC-M2 of the operations that both transactions have.
macro_rules! check_read_sizes {
    ($tx:expr, $tree:expr, $folder:expr) => {{
        let (tx, tree, folder) = (&$tx, &$tree, $folder);
        let size = |error: Error| invalid_size(error, folder);
        let long = vec![0; MAX_KEY + 1];
        let long = &long[..];
        let (value, prefix) = (VALUE_ERROR, PREFIX_ERROR);
        for bad in [&[][..], long] {
            let key = key_error(bad.len());
            assert_eq!(size(tx.contains(tree, bad, b"v").unwrap_err()), key);
            let both = tx.contains(tree, bad, long).unwrap_err();
            assert_eq!(size(both), key, "the key before the value");
            assert_eq!(size(tx.value_count(tree, bad).unwrap_err()), key);
            assert_eq!(size(tx.values(tree, bad, ..).err().unwrap()), key);
            let all = tx.values_prefix(tree, bad, long, long..).err().unwrap();
            assert_eq!(size(all), key, "the key before the bounds and the prefix");
            assert_eq!(
                size(tx.keys(tree, bad..).err().unwrap()),
                key,
                "a key bound"
            );
            assert_eq!(size(tx.keys(tree, ..=bad).err().unwrap()), key);
            let excluded = tx.keys(tree, (Excluded(bad), Unbounded)).err().unwrap();
            assert_eq!(size(excluded), key);
        }
        assert_eq!(size(tx.contains(tree, b"k", long).unwrap_err()), value);
        assert_eq!(size(tx.values(tree, b"k", long..).err().unwrap()), value);
        assert_eq!(size(tx.values(tree, b"k", ..long).err().unwrap()), value);
        let excluded = tx.values(tree, b"k", (Excluded(long), Unbounded));
        assert_eq!(size(excluded.err().unwrap()), value);
        let both = tx.values_prefix(tree, b"k", long, long..).err().unwrap();
        assert_eq!(size(both), value, "the lower bound before the prefix");
        let upper = tx.values_prefix(tree, b"k", b"", ..=long).err().unwrap();
        assert_eq!(size(upper), value);
        let only = tx.values_prefix(tree, b"k", long, ..).err().unwrap();
        assert_eq!(size(only), prefix);
        let both = tx.keys_prefix(tree, long, long..).err().unwrap();
        assert_eq!(size(both), key_error(long.len()));
        assert_eq!(size(tx.keys_prefix(tree, long, ..).err().unwrap()), prefix);
        let empty: &[u8] = b"";
        assert!(
            tx.values(tree, b"k", empty..).is_ok(),
            "an empty value bound is valid"
        );
        assert!(tx.values_prefix(tree, b"k", long, ..empty).is_err());
    }};
}

/// AC-M2 (M1, M6, M7, M12, AC-B15): a value of 2,049 bytes fails as a value of 0 to 2,048 bytes, a key that is empty or
/// of 2,049 bytes as a key of 1 to 2,048, and a prefix of 2,049 bytes as a prefix of 0 to 2,048, without effect, in the
/// order key, value or lower bound, upper bound, prefix; the transaction stays usable.
#[test]
fn ac_m2_sizes_are_checked_in_order_without_effect() {
    stores(|store, folder| {
        let mut model = Model::from([(b"k".to_vec(), set_of(&[b"v"]))]);
        commit_multi(store, "m", &model);
        let size = |error: Error| invalid_size(error, folder);
        let long = vec![0; MAX_KEY + 1];
        let long = &long[..];
        let (value, prefix) = (VALUE_ERROR, PREFIX_ERROR);

        let mut write = store.write().unwrap();
        let tree = write.open_multi_tree("m").unwrap();
        check_read_sizes!(write, tree, folder);
        assert_eq!(size(write.add(&tree, b"k", long).unwrap_err()), value);
        assert_eq!(
            size(write.remove_value(&tree, b"k", long).unwrap_err()),
            value
        );
        let ranged = write.remove_value_range(&tree, b"k", long..).unwrap_err();
        assert_eq!(size(ranged), value);
        let ranged = write.remove_value_range(&tree, b"k", ..=long).unwrap_err();
        assert_eq!(size(ranged), value);
        let both = write.remove_value_range_prefix(&tree, b"k", long, long..);
        assert_eq!(
            size(both.unwrap_err()),
            value,
            "the lower bound before the prefix"
        );
        let only = write.remove_value_range_prefix(&tree, b"k", long, ..);
        assert_eq!(size(only.unwrap_err()), prefix);
        for bad in [&[][..], long] {
            let key = key_error(bad.len());
            assert_eq!(size(write.add(&tree, bad, b"v").unwrap_err()), key);
            assert_eq!(
                size(write.add(&tree, bad, long).unwrap_err()),
                key,
                "the key first"
            );
            assert_eq!(size(write.remove_value(&tree, bad, long).unwrap_err()), key);
            assert_eq!(size(write.remove_key(&tree, bad).unwrap_err()), key);
            let ranged = write.remove_value_range(&tree, bad, long..).unwrap_err();
            assert_eq!(size(ranged), key);
            let all = write.remove_value_range_prefix(&tree, bad, long, ..long);
            assert_eq!(size(all.unwrap_err()), key);
            assert_eq!(size(write.remove_key_range(&tree, bad..).unwrap_err()), key);
            assert_eq!(size(write.remove_key_range(&tree, ..bad).unwrap_err()), key);
            let both = write.remove_key_range_prefix(&tree, long, bad..);
            assert_eq!(
                size(both.unwrap_err()),
                key,
                "the lower bound before the prefix"
            );
        }
        let only = write.remove_key_range_prefix(&tree, long, ..);
        assert_eq!(size(only.unwrap_err()), prefix);
        check!(write, tree, model);
        assert!(
            write.add(&tree, b"k", b"w").unwrap(),
            "the transaction stays usable"
        );
        model.get_mut(&b"k"[..]).unwrap().insert(b"w".to_vec());
        write.commit().unwrap();

        let read = store.read().unwrap();
        let tree = read.open_multi_tree("m").unwrap();
        check_read_sizes!(read, tree, folder);
        check!(read, tree, model);
    });
}

/// AC-M3 (M5, M6): removing the values of a key one by one reports each as there once and then not; the key goes
/// away with its last value, for Contains, its number of values, the key scan and the number of entries; and adding a
/// value brings it back with that one value. The key's values are in its entry or in a subtree.
#[test]
fn ac_m3_a_key_goes_away_with_its_last_value() {
    stores(|store, _| {
        let mut model = Model::from([
            (b"few".to_vec(), set_of(&[b"", b"a", b"b"])),
            (b"many".to_vec(), (0..300).map(value16).collect()),
            (b"other".to_vec(), set_of(&[b"x"])),
        ]);
        commit_multi(store, "m", &model);
        let mut write = store.write().unwrap();
        let tree = write.open_multi_tree("m").unwrap();
        assert!(write.subtree_of(&tree, b"few").unwrap().is_none());
        assert!(write.subtree_of(&tree, b"many").unwrap().is_some());
        for key in [&b"few"[..], b"many"] {
            let values: Vec<Vec<u8>> = model[key].iter().cloned().collect();
            for value in &values {
                assert!(write.remove_value(&tree, key, value).unwrap(), "there");
                assert!(!write.remove_value(&tree, key, value).unwrap(), "no longer");
                let left = model.get_mut(key).unwrap();
                left.remove(value);
                let left = left.len() as u64;
                assert_eq!(write.value_count(&tree, key).unwrap(), left);
            }
            model.remove(key);
            assert!(!write.contains(&tree, key, &values[0]).unwrap());
            assert_eq!(write.value_count(&tree, key).unwrap(), 0);
            check!(write, tree, model);
            assert!(write.add(&tree, key, b"again").unwrap());
            model.insert(key.to_vec(), set_of(&[b"again"]));
            assert_eq!(write.value_count(&tree, key).unwrap(), 1);
            check!(write, tree, model);
        }
        write.commit().unwrap();
        check_read(store, "m", &model);
    });
}

/// AC-M4 (M6, M2): Remove key returns the number of values of a key with one value, one with several in its entry
/// and one with a subtree, and the key goes away; a key that is not there returns `0` without failing.
#[test]
fn ac_m4_remove_key_returns_its_number_of_values() {
    stores(|store, _| {
        let mut model = Model::from([
            (b"one".to_vec(), set_of(&[b"1"])),
            (b"several".to_vec(), (0..5).map(value16).collect()),
            (b"subtree".to_vec(), (0..300).map(value16).collect()),
            (b"z".to_vec(), set_of(&[b""])),
        ]);
        commit_multi(store, "m", &model);
        let mut write = store.write().unwrap();
        let tree = write.open_multi_tree("m").unwrap();
        assert!(write.subtree_of(&tree, b"several").unwrap().is_none());
        assert!(write.subtree_of(&tree, b"subtree").unwrap().is_some());
        for (key, count) in [(&b"one"[..], 1), (b"several", 5), (b"subtree", 300)] {
            assert_eq!(write.remove_key(&tree, key).unwrap(), count);
            model.remove(key);
            check!(write, tree, model);
            assert_eq!(write.remove_key(&tree, key).unwrap(), 0, "no longer there");
        }
        assert_eq!(write.remove_key(&tree, b"absent").unwrap(), 0);
        write.commit().unwrap();
        check_read(store, "m", &model);
    });
}

/// A store that a test can close and open again when it is on disk.
struct Reopenable {
    store: Option<Store>,
    folder: Option<PathBuf>,
}

impl Reopenable {
    fn new(folder: Option<&Path>) -> Reopenable {
        let store = match folder {
            Some(folder) => open(folder),
            None => Store::open_temporary(&StoreOptions::default()).unwrap(),
        };
        Reopenable {
            store: Some(store),
            folder: folder.map(Path::to_path_buf),
        }
    }

    fn store(&self) -> &Store {
        self.store.as_ref().unwrap()
    }

    /// Closes the store and opens it again, on disk; a temporary store stays as it is.
    fn reopen(&mut self) {
        if let Some(folder) = &self.folder {
            self.store.take().unwrap().close().unwrap();
            self.store = Some(open(folder));
        }
    }
}

/// An operation of AC-M5 on the key `k`.
#[derive(Debug, Clone)]
enum Step {
    Add(Vec<u8>),
    Remove(Vec<u8>),
    /// Remove value range from this value on.
    RemoveFrom(Vec<u8>),
}

/// The steps of AC-M5 with what the key holds after each and whether its values are then in a subtree: adding one
/// by one past the promotion threshold, removing one by one below the demotion threshold, adding again past the
/// promotion threshold and removing a range that goes at once below the demotion threshold. With `with_empty`, the
/// empty value is the fourth one added, and it stays.
fn threshold_steps(with_empty: bool) -> Vec<(Step, BTreeSet<Vec<u8>>, bool)> {
    let (promotion, demotion) = multi_thresholds();
    // The volume of the key `k`, whose length is the first argument.
    let volume_of = |values: &BTreeSet<Vec<u8>>| volume(b"k".len(), values.iter().map(Vec::len));
    let mut values = BTreeSet::new();
    let mut in_subtree = false;
    let mut steps = Vec::new();
    let mut fresh = (0..).map(value16);
    let mut added = 0;
    // The value added after `added_before` others.
    let mut next_value = |added_before: usize| {
        if with_empty && added_before == 3 {
            Vec::new()
        } else {
            fresh.next().unwrap()
        }
    };
    for phase in 0..2 {
        // Adding, until two steps after the promotion.
        let mut past = 0;
        while past < 3 {
            let value = next_value(added);
            added += 1;
            values.insert(value.clone());
            in_subtree |= volume_of(&values) > promotion;
            steps.push((Step::Add(value), values.clone(), in_subtree));
            past += usize::from(in_subtree);
        }
        if phase == 0 {
            // Removing the greatest values, until two steps after the demotion.
            let mut past = 0;
            while past < 3 {
                let value = values.last().unwrap().clone();
                values.remove(&value);
                in_subtree &= volume_of(&values) >= demotion;
                steps.push((Step::Remove(value), values.clone(), in_subtree));
                past += usize::from(!in_subtree);
            }
        }
    }
    // A range from above the promotion threshold to below the demotion threshold at once.
    assert!(volume_of(&values) > promotion);
    let mut kept = BTreeSet::new();
    let from = values
        .iter()
        .find(|value| {
            kept.insert((*value).clone());
            volume_of(&kept) >= demotion
        })
        .unwrap()
        .clone();
    values.retain(|value| *value < from);
    assert!(volume_of(&values) < demotion);
    steps.push((Step::RemoveFrom(from), values, false));
    steps
}

/// Applies `step` to the key `k`, checks what it returns, and then the whole tree and where the key's values are.
fn apply_step(
    write: &mut krummi::WriteTransaction<'_>,
    tree: &krummi::WriteMultiTree<'_>,
    (step, values, in_subtree): &(Step, BTreeSet<Vec<u8>>, bool),
    before: usize,
) {
    match step {
        Step::Add(value) => assert!(write.add(tree, b"k", value).unwrap()),
        Step::Remove(value) => {
            assert!(write.remove_value(tree, b"k", value).unwrap());
            assert!(!write.contains(tree, b"k", value).unwrap());
        }
        Step::RemoveFrom(from) => {
            let removed = write.remove_value_range(tree, b"k", &from[..]..).unwrap();
            assert_eq!(removed, (before - values.len()) as u64);
        }
    }
    let model = Model::from([(b"k".to_vec(), values.clone())]);
    check!(write, *tree, model);
    let found = write.subtree_of(tree, b"k").unwrap().is_some();
    assert_eq!(found, *in_subtree, "{step:?} with {} values", values.len());
}

/// Checks in a new read the key `k` after a step of AC-M5, and where its values are.
fn check_step(store: &Store, (step, values, in_subtree): &(Step, BTreeSet<Vec<u8>>, bool)) {
    let model = Model::from([(b"k".to_vec(), values.clone())]);
    check_read(store, "m", &model);
    let read = store.read().unwrap();
    let tree = read.open_multi_tree("m").unwrap();
    let found = read.subtree_of(&tree, b"k").unwrap().is_some();
    assert_eq!(found, *in_subtree, "{step:?} in a read");
}

/// AC-M5 (M2, M8, M1): with the thresholds of the implementation, the values of a key move to a subtree in the
/// addition that passes the promotion threshold, stay there between the two thresholds, and move back to its entry in
/// the removal that goes below the demotion threshold, also a value range removal that goes from above the promotion
/// threshold to below the demotion one at once; with and without the empty value, which keeps its place; in separate
/// writes and in one. After each step Contains, the number of values and the value scan from both ends match, in the
/// write, in a later read and, on disk, after reopening (here after each change of representation and at the end).
#[test]
fn ac_m5_values_move_to_a_subtree_and_back_at_the_thresholds() {
    let (promotion, demotion) = multi_thresholds();
    assert!(demotion < promotion);
    for with_empty in [false, true] {
        let steps = threshold_steps(with_empty);
        let changes = steps.windows(2).filter(|w| w[0].2 != w[1].2).count();
        assert_eq!(changes, 4, "promoted, demoted, promoted and demoted");
        for separate in [false, true] {
            let tmp = tempfile::tempdir().unwrap();
            for folder in [Some(tmp.path()), None] {
                let mut store = Reopenable::new(folder);
                let mut write = store.store().write().unwrap();
                create_multi(&mut write, "m");
                write.commit().unwrap();
                if separate {
                    let mut before = 0;
                    let mut was_in_subtree = false;
                    for step in &steps {
                        let mut write = store.store().write().unwrap();
                        let tree = write.open_multi_tree("m").unwrap();
                        apply_step(&mut write, &tree, step, before);
                        write.commit().unwrap();
                        check_step(store.store(), step);
                        if step.2 != was_in_subtree {
                            store.reopen();
                            check_step(store.store(), step);
                        }
                        (before, was_in_subtree) = (step.1.len(), step.2);
                    }
                } else {
                    let mut write = store.store().write().unwrap();
                    let tree = write.open_multi_tree("m").unwrap();
                    let mut before = 0;
                    for step in &steps {
                        apply_step(&mut write, &tree, step, before);
                        before = step.1.len();
                    }
                    write.commit().unwrap();
                }
                let last = steps.last().unwrap();
                check_step(store.store(), last);
                store.reopen();
                check_step(store.store(), last);
                store.store.take().unwrap().close().unwrap();
            }
        }
    }
}

/// The keys of the scans of AC-M6: values in the entry and in subtrees, with and without the empty value, and keys
/// and values made of `0xFF` bytes.
fn scan_model() -> Model {
    Model::from([
        (
            b"a".to_vec(),
            set_of(&[b"", b"a", b"ab", b"b", b"\xFF", b"\xFF\xFF"]),
        ),
        (b"aa".to_vec(), set_of(&[b"0000000000000050"])),
        (
            b"ab".to_vec(),
            (0..300)
                .map(value16)
                .chain([
                    vec![],
                    b"a".to_vec(),
                    vec![0xFF],
                    vec![0xFF, 0x00],
                    vec![0xFF, 0xFF],
                ])
                .collect(),
        ),
        (b"b".to_vec(), (0..300).map(value16).collect()),
        (vec![0xFF], set_of(&[b"", b"z"])),
        (vec![0xFF, 0xFF], set_of(&[b"\xFF"])),
    ])
}

/// A lower and an upper bound.
type Bounds = (Bound<&'static [u8]>, Bound<&'static [u8]>);

/// Every pair of bounds made of unbounded and of each of `items` included and excluded.
fn bound_pairs(items: &[&'static [u8]]) -> Vec<Bounds> {
    let bounds: Vec<Bound<&[u8]>> = std::iter::once(Unbounded)
        .chain(
            items
                .iter()
                .flat_map(|item| [Included(*item), Excluded(*item)]),
        )
        .collect();
    bounds
        .iter()
        .flat_map(|lower| bounds.iter().map(move |upper| (*lower, *upper)))
        .collect()
}

/// Whether `item` is within the bounds and starts with the prefix (R2–R4).
fn within(item: &[u8], prefix: &[u8], bounds: (Bound<&[u8]>, Bound<&[u8]>)) -> bool {
    item.starts_with(prefix) && bounds.contains(&item)
}

/// Checks every key scan and every value scan of AC-M6 of the tree `tree` of `tx`, which holds `model`, from the
/// front, from the back and alternating the ends.
macro_rules! check_scans {
    ($tx:expr, $tree:expr, $model:expr) => {{
        let (tx, tree, model): (_, _, &Model) = (&$tx, &$tree, &$model);
        let keys = counts(model);
        let key_bounds: [&[u8]; 6] = [b"a", b"aa", b"ab", b"b", b"\xFF", b"\xFF\xFF"];
        for bounds in bound_pairs(&key_bounds) {
            for prefix in [&b""[..], b"a", b"\xFF", b"\xFF\xFF"] {
                let want: Vec<(Vec<u8>, u64)> = keys
                    .iter()
                    .filter(|(key, _)| within(key, prefix, bounds))
                    .cloned()
                    .collect();
                let label = format!("keys {prefix:?} {bounds:?}");
                let scan = || tx.keys_prefix(tree, prefix, bounds).unwrap();
                assert_eq!(collect(scan(), own_key), want, "{label}");
                assert_eq!(reversed(collect(scan().rev(), own_key)), want, "{label}");
                assert_eq!(alternate(scan(), own_key), want, "{label}");
            }
        }
        let value_bounds: [&[u8]; 6] = [
            b"",
            b"a",
            b"0000000000000050",
            b"0000000000000149",
            b"\xFF",
            b"\xFF\x00",
        ];
        let absent = (b"absent".to_vec(), BTreeSet::new());
        for (key, values) in model.iter().chain([(&absent.0, &absent.1)]) {
            for bounds in bound_pairs(&value_bounds) {
                for prefix in [&b""[..], b"a", b"000000000000001", b"\xFF", b"\xFF\xFF"] {
                    let want: Vec<Vec<u8>> = values
                        .iter()
                        .filter(|value| within(value, prefix, bounds))
                        .cloned()
                        .collect();
                    let label = format!("values of {key:?} {prefix:?} {bounds:?}");
                    let scan = || tx.values_prefix(tree, key, prefix, bounds).unwrap();
                    assert_eq!(collect(scan(), own_value), want, "{label}");
                    assert_eq!(reversed(collect(scan().rev(), own_value)), want, "{label}");
                    assert_eq!(alternate(scan(), own_value), want, "{label}");
                }
            }
        }
    }};
}

/// AC-M6 (M7, R1–R4, R6): key scans and value scans, with bounds included, excluded and open, the empty value as a
/// bound, prefixes (also empty and ending in `0xFF`) and both ends, return exactly what R1 to R4 say, each key with its
/// number of values; the values of a key that is not there, and the keys of an empty tree or of one left without keys,
/// are nothing; inverted bounds are an empty interval, also with the page of the key's entry damaged (on disk).
#[test]
fn ac_m6_scans_of_keys_and_of_the_values_of_a_key() {
    let model = scan_model();
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let tree = create_multi(&mut write, "m");
        assert!(
            collect(write.keys(&tree, ..).unwrap(), own_key).is_empty(),
            "an empty tree"
        );
        assert!(collect(write.values(&tree, b"a", ..).unwrap(), own_value).is_empty());
        for (key, values) in &model {
            for value in values {
                write.add(&tree, key, value).unwrap();
            }
        }
        assert!(write.subtree_of(&tree, b"a").unwrap().is_none());
        assert!(write.subtree_of(&tree, b"ab").unwrap().is_some());
        assert!(write.subtree_of(&tree, b"b").unwrap().is_some());
        check_scans!(write, tree, model);
        write.commit().unwrap();
        let read = store.read().unwrap();
        let tree = read.open_multi_tree("m").unwrap();
        check_scans!(read, tree, model);
        drop(read);

        let mut write = store.write().unwrap();
        let tree = write.open_multi_tree("m").unwrap();
        assert_eq!(
            write.remove_key_range(&tree, ..).unwrap(),
            model.len() as u64
        );
        check_scans!(write, tree, Model::new());
        write.commit().unwrap();
        let read = store.read().unwrap();
        let tree = read.open_multi_tree("m").unwrap();
        assert!(
            collect(read.keys(&tree, ..).unwrap(), own_key).is_empty(),
            "left without keys"
        );
    });

    let tmp = tempfile::tempdir().unwrap();
    commit_multi(&open(tmp.path()), "m", &model);
    damage(tmp.path(), leaf_page(tmp.path(), Some("m"), b"a"));
    let store = open(tmp.path());
    let read = store.read().unwrap();
    let tree = read.open_multi_tree("m").unwrap();
    assert!(
        read.value_count(&tree, b"a").is_err(),
        "the entry's page is damaged"
    );
    let (a, b): (&[u8], &[u8]) = (b"a", b"b");
    for key in [a, b"ab"] {
        for bounds in [
            (Included(b), Included(a)),
            (Excluded(a), Included(a)),
            (Included(a), Excluded(a)),
        ] {
            let scan = read.values(&tree, key, bounds).unwrap();
            assert!(collect(scan, own_value).is_empty(), "{bounds:?}");
        }
        let scan = read.values_prefix(&tree, key, b"c", a..b).unwrap();
        assert!(
            collect(scan, own_value).is_empty(),
            "a prefix outside the bounds"
        );
    }
    drop(read);
    let mut write = store.write().unwrap();
    let tree = write.open_multi_tree("m").unwrap();
    assert_eq!(write.remove_value_range(&tree, b"a", b..a).unwrap(), 0);
    write.commit().unwrap();
    store.close().unwrap();
}

/// AC-M7 (M8, M6): Remove value range returns how many values it removed from a key in its entry and in a subtree,
/// also all of them, and the key that loses its last value goes away; Remove key range returns how many keys it
/// removed, also keys with a subtree, the number of entries goes down by as many, and a key scan returns the rest
/// with their numbers of values.
#[test]
fn ac_m7_removing_ranges_of_values_and_of_keys() {
    stores(|store, _| {
        let mut model = Model::from([
            (b"i".to_vec(), set_of(&[b"", b"a", b"b", b"c", b"d"])),
            (
                b"s".to_vec(),
                (0..300).map(value16).chain([vec![]]).collect(),
            ),
            (b"t".to_vec(), (0..300).map(value16).collect()),
            (b"u".to_vec(), set_of(&[b"1", b"2"])),
            (b"v".to_vec(), (0..300).map(value16).collect()),
            (b"w".to_vec(), set_of(&[b"x"])),
            (b"wx".to_vec(), (0..300).map(value16).collect()),
            (b"z".to_vec(), set_of(&[b"z"])),
        ]);
        commit_multi(store, "m", &model);
        let mut write = store.write().unwrap();
        let tree = write.open_multi_tree("m").unwrap();
        let (a, c): (&[u8], &[u8]) = (b"a", b"c");
        assert_eq!(write.remove_value_range(&tree, b"i", a..c).unwrap(), 2);
        model
            .get_mut(&b"i"[..])
            .unwrap()
            .retain(|v| !(a..c).contains(&&v[..]));
        check!(write, tree, model);
        let removed = write.remove_value_range_prefix(&tree, b"s", b"000000000000001", ..);
        assert_eq!(removed.unwrap(), 10);
        model
            .get_mut(&b"s"[..])
            .unwrap()
            .retain(|v| !v.starts_with(b"000000000000001"));
        let empty: &[u8] = b"";
        assert_eq!(
            write.remove_value_range(&tree, b"s", ..=empty).unwrap(),
            1,
            "the empty value"
        );
        model.get_mut(&b"s"[..]).unwrap().remove(&Vec::new());
        assert!(write.subtree_of(&tree, b"s").unwrap().is_some());
        check!(write, tree, model);
        assert_eq!(write.remove_value_range(&tree, b"i", ..).unwrap(), 3);
        assert_eq!(write.remove_value_range(&tree, b"t", ..).unwrap(), 300);
        model.remove(&b"i"[..]);
        model.remove(&b"t"[..]);
        check!(write, tree, model);
        assert_eq!(write.remove_value_range(&tree, b"absent", ..).unwrap(), 0);

        let (u, v): (&[u8], &[u8]) = (b"u", b"v");
        assert_eq!(write.remove_key_range(&tree, u..=v).unwrap(), 2);
        model.remove(u);
        model.remove(v);
        check!(write, tree, model);
        assert_eq!(write.remove_key_range_prefix(&tree, b"w", ..).unwrap(), 2);
        model.remove(&b"w"[..]);
        model.remove(&b"wx"[..]);
        check!(write, tree, model);
        assert_eq!(write.len(&tree).unwrap(), 2);
        write.commit().unwrap();
        check_read(store, "m", &model);
    });
}

/// The pages in use: the pages of the store minus its free pages, as its information gives them.
fn in_use(store: &Store) -> u64 {
    let info = store.info();
    info.page_count - info.free_pages
}

/// The keys of AC-M8: ten long ones with their values in their entries, over several pages of the tree, and three
/// with subtrees, one of several levels.
fn space_model() -> Model {
    let mut model: Model = (0..10)
        .map(|i| (long_key(i, 600), set_of(&[b"", b"x", &[i as u8; 50]])))
        .collect();
    model.insert(
        long_key(20, 16),
        (0..40).map(|i| long_key(i, MAX_KEY)).collect(),
    );
    model.insert(long_key(21, 16), (0..300).map(value16).collect());
    model.insert(
        long_key(22, 16),
        (0..300).map(value16).chain([vec![]]).collect(),
    );
    model
}

/// A store of the same kind as the one of `folder`: on disk in a new folder, or temporary.
fn store_like(folder: Option<&Path>) -> (Store, Option<tempfile::TempDir>) {
    match folder {
        Some(_) => {
            let tmp = tempfile::tempdir().unwrap();
            (open(tmp.path()), Some(tmp))
        }
        None => (
            Store::open_temporary(&StoreOptions::default()).unwrap(),
            None,
        ),
    }
}

/// A tree `a` and the multi-value tree `m` created empty, each committed.
fn space_base(store: &Store) {
    commit_tree(store, "a", &[(b"k", b"v")]);
    commit_multi(store, "m", &Model::new());
}

/// AC-M8 (M10, A2, A3): within one region of free space, a multi-value tree with keys in their entries and in
/// subtrees that loses all its keys, by Remove value, Remove key, Remove value range, Remove key range or emptying,
/// takes the pages it took created empty; a key moved back from its subtree to its entry takes the pages of the same
/// values never moved; and deleting the tree or overwriting it with Rename gives back the pages it took.
#[test]
fn ac_m8_the_pages_of_the_subtrees_go_back_to_free_space() {
    let model = space_model();
    for form in 0..5 {
        stores(|store, _| {
            space_base(store);
            let empty = in_use(store);
            commit_multi(store, "m", &model);
            assert!(
                in_use(store) > empty + 20,
                "pages of the tree and its subtrees"
            );
            let mut write = store.write().unwrap();
            let tree = write.open_multi_tree("m").unwrap();
            assert!(
                write
                    .subtree_of(&tree, &long_key(20, 16))
                    .unwrap()
                    .is_some()
            );
            for (key, values) in &model {
                match form {
                    0 => {
                        for value in values {
                            assert!(write.remove_value(&tree, key, value).unwrap());
                        }
                    }
                    1 => assert_eq!(write.remove_key(&tree, key).unwrap(), values.len() as u64),
                    2 => {
                        let removed = write.remove_value_range(&tree, key, ..).unwrap();
                        assert_eq!(removed, values.len() as u64);
                    }
                    _ => {}
                }
            }
            match form {
                3 => assert_eq!(
                    write.remove_key_range(&tree, ..).unwrap(),
                    model.len() as u64
                ),
                4 => drop(write.open_multi_tree_with("m", OpenMode::Empty).unwrap()),
                _ => {}
            }
            assert_eq!(write.len(&tree).unwrap(), 0);
            write.commit().unwrap();
            assert_eq!(in_use(store), empty, "form {form}");
        });
    }

    stores(|store, folder| {
        let key = b"k";
        space_base(store);
        commit_multi(
            store,
            "m",
            &Model::from([(key.to_vec(), (0..300).map(value16).collect())]),
        );
        let mut write = store.write().unwrap();
        let tree = write.open_multi_tree("m").unwrap();
        let mut i = 300;
        while write.subtree_of(&tree, key).unwrap().is_some() {
            i -= 1;
            assert!(write.remove_value(&tree, key, &value16(i)).unwrap());
        }
        write.commit().unwrap();
        let (never, _tmp) = store_like(folder);
        space_base(&never);
        commit_multi(
            &never,
            "m",
            &Model::from([(key.to_vec(), (0..i).map(value16).collect())]),
        );
        let read = never.read().unwrap();
        let tree = read.open_multi_tree("m").unwrap();
        assert!(
            read.subtree_of(&tree, key).unwrap().is_none(),
            "never in a subtree"
        );
        drop(read);
        assert_eq!(in_use(store), in_use(&never));
        never.close().unwrap();
    });

    stores(|store, _| {
        commit_tree(store, "a", &[(b"k", b"v")]);
        commit_tree(store, "src", &[(b"k", b"v")]);
        let before = in_use(store);
        for overwrite in [false, true] {
            commit_multi(store, "m", &model);
            assert!(in_use(store) > before + 20);
            let mut write = store.write().unwrap();
            if overwrite {
                write.rename_tree("src", "m", true).unwrap();
            } else {
                write.delete_tree("m").unwrap();
            }
            write.commit().unwrap();
            assert_eq!(in_use(store), before, "overwrite {overwrite}");
        }
    });
}

/// The fields of a different-tree-type error, after checking its folder.
#[track_caller]
fn different(error: Error, folder: Option<&Path>) -> (String, TreeKind, TreeKind) {
    match error {
        Error::DifferentTreeType {
            folder: f,
            name,
            found,
            requested,
        } => {
            assert_eq!(f, folder_of(folder));
            (name, found, requested)
        }
        other => panic!("expected a different tree type, got {other}"),
    }
}

const MODES: [OpenMode; 4] = [
    OpenMode::CreateNew,
    OpenMode::OpenOrCreate,
    OpenMode::CreateOrEmpty,
    OpenMode::Empty,
];

/// Checks that the write `$write` cannot open the normal tree `n` as multi-value nor the multi-value tree `m` as
/// normal, in any way.
macro_rules! check_types_in_write {
    ($write:expr, $folder:expr) => {{
        let folder = $folder;
        let normal = ("n".to_owned(), NORMAL, MULTI);
        let multi = ("m".to_owned(), MULTI, NORMAL);
        assert_eq!(
            different($write.open_multi_tree("n").err().unwrap(), folder),
            normal
        );
        assert_eq!(
            different($write.open_tree("m").err().unwrap(), folder),
            multi
        );
        for mode in MODES {
            let error = $write.open_multi_tree_with("n", mode).err().unwrap();
            assert_eq!(different(error, folder), normal, "{mode:?}");
            let error = $write.open_tree_with("m", mode).err().unwrap();
            assert_eq!(different(error, folder), multi, "{mode:?}");
        }
    }};
}

/// AC-M10 (M4, M12, AC-B4, AC-B6, AC-B8, A3): opening or creating a tree as the other kind, in every mode, fails with
/// the different-tree-type class, its name, its kind and the kind requested, and leaves the tree intact, also with a
/// page of the tree damaged; the write commits the changes made around the failure and a read goes on; deleting a
/// tree takes no kind; a deleted normal tree gives its name to a multi-value one; Listar reports each kind; and
/// Rename with overwrite deletes the target whatever its kind, also a multi-value one with subtrees. The part of a
/// store that refuses writes is in the library's tests.
#[test]
fn ac_m10_each_kind_opens_only_as_itself() {
    let model = Model::from([
        (b"k".to_vec(), set_of(&[b"", b"v"])),
        (b"s".to_vec(), (0..300).map(value16).collect()),
    ]);
    stores(|store, folder| {
        commit_tree(store, "n", &[(b"k", b"v")]);
        commit_multi(store, "m", &model);
        let mut write = store.write().unwrap();
        let normal = write.open_tree("n").unwrap();
        let multi = write.open_multi_tree("m").unwrap();
        write.set(&normal, b"before", b"1").unwrap();
        write.add(&multi, b"before", b"1").unwrap();
        check_types_in_write!(write, folder);
        write.set(&normal, b"after", b"2").unwrap();
        write.add(&multi, b"after", b"2").unwrap();
        write.commit().unwrap();

        let mut model = model.clone();
        model.insert(b"before".to_vec(), set_of(&[b"1"]));
        model.insert(b"after".to_vec(), set_of(&[b"2"]));
        let read = store.read().unwrap();
        let normal = ("n".to_owned(), NORMAL, MULTI);
        let multi = ("m".to_owned(), MULTI, NORMAL);
        assert_eq!(
            different(read.open_multi_tree("n").err().unwrap(), folder),
            normal
        );
        assert_eq!(different(read.open_tree("m").err().unwrap(), folder), multi);
        let tree = read.open_tree("n").unwrap();
        assert_eq!(
            read.get(&tree, b"before").unwrap(),
            Some(&b"1"[..]),
            "the read goes on"
        );
        assert_eq!(read.get(&tree, b"after").unwrap(), Some(&b"2"[..]));
        check_read(store, "m", &model);
        let kinds = vec![("m".to_owned(), MULTI), ("n".to_owned(), NORMAL)];
        assert_eq!(read.list_trees().unwrap(), kinds);
        drop(read);

        let mut write = store.write().unwrap();
        write.delete_tree("n").unwrap();
        let reborn = write
            .open_multi_tree_with("n", OpenMode::CreateNew)
            .unwrap();
        write.add(&reborn, b"k", b"v").unwrap();
        create_in(&mut write, "x", b"from x");
        let kinds = [("m", MULTI), ("n", MULTI), ("x", NORMAL)].map(|(n, k)| (n.to_owned(), k));
        assert_eq!(write.list_trees().unwrap(), kinds);
        write.rename_tree("x", "m", true).unwrap();
        let renamed = write.open_tree("m").unwrap();
        assert_eq!(write.get(&renamed, b"k").unwrap(), Some(&b"from x"[..]));
        write.rename_tree("n", "m", true).unwrap();
        let renamed = write.open_multi_tree("m").unwrap();
        assert_eq!(write.value_count(&renamed, b"k").unwrap(), 1);
        assert_eq!(write.list_trees().unwrap(), [("m".to_owned(), MULTI)]);
        write.delete_tree("m").unwrap();
        assert!(write.list_trees().unwrap().is_empty());
        write.commit().unwrap();
    });

    // A damaged page of each tree, with the catalog intact.
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_tree(&store, "n", &[(b"k", b"v")]);
    commit_multi(&store, "m", &model);
    store.close().unwrap();
    for name in ["n", "m"] {
        damage(tmp.path(), root_page(tmp.path(), name));
    }
    let store = open(tmp.path());
    let folder = Some(tmp.path());
    let mut write = store.write().unwrap();
    check_types_in_write!(write, folder);
    write.commit().unwrap();
    let read = store.read().unwrap();
    assert_eq!(
        different(read.open_multi_tree("n").err().unwrap(), folder).1,
        NORMAL
    );
    assert_eq!(
        different(read.open_tree("m").err().unwrap(), folder).1,
        MULTI
    );
    drop(read);
    store.close().unwrap();
}

/// Creates the normal tree `name` in `write` with the key `k` and `value`.
fn create_in(write: &mut krummi::WriteTransaction<'_>, name: &str, value: &[u8]) {
    let tree = create(write, name);
    write.set(&tree, b"k", value).unwrap();
}

/// Checks that every operation of a multi-value tree that both transactions have fails on `$tree` with `$expect`.
macro_rules! read_operations_fail {
    ($tx:expr, $tree:expr, $expect:expr) => {{
        let (tx, tree, expect) = (&$tx, &$tree, $expect);
        expect(tx.contains(tree, b"k", b"v").unwrap_err());
        expect(tx.value_count(tree, b"k").unwrap_err());
        expect(tx.len(tree).unwrap_err());
        expect(tx.keys(tree, ..).err().unwrap());
        expect(tx.keys_prefix(tree, b"k", ..).err().unwrap());
        expect(tx.values(tree, b"k", ..).err().unwrap());
        expect(tx.values_prefix(tree, b"k", b"v", ..).err().unwrap());
        let long = vec![0; MAX_KEY + 1];
        let size = |error: Error| error.kind() == krummi::ErrorKind::InvalidSize;
        assert!(
            size(tx.contains(tree, &long, b"v").unwrap_err()),
            "the size wins"
        );
        assert!(size(tx.contains(tree, b"k", &long).unwrap_err()));
        assert!(size(tx.value_count(tree, b"").unwrap_err()));
        assert!(size(tx.keys(tree, &long[..]..).err().unwrap()));
        assert!(size(tx.keys_prefix(tree, &long, ..).err().unwrap()));
        assert!(size(tx.values(tree, b"k", ..=&long[..]).err().unwrap()));
        assert!(size(tx.values_prefix(tree, b"k", &long, ..).err().unwrap()));
    }};
}

/// Checks that every operation of a multi-value tree fails on `$tree` with `$expect`, in the write `$write`.
macro_rules! write_operations_fail {
    ($write:expr, $tree:expr, $expect:expr) => {{
        let expect = $expect;
        read_operations_fail!($write, $tree, &expect);
        let tree = &$tree;
        expect($write.add(tree, b"k", b"w").unwrap_err());
        expect($write.remove_value(tree, b"k", b"v").unwrap_err());
        expect($write.remove_key(tree, b"k").unwrap_err());
        expect($write.remove_key_range(tree, ..).unwrap_err());
        expect($write.remove_key_range_prefix(tree, b"k", ..).unwrap_err());
        expect($write.remove_value_range(tree, b"k", ..).unwrap_err());
        expect(
            $write
                .remove_value_range_prefix(tree, b"k", b"v", ..)
                .unwrap_err(),
        );
        let long = vec![0; MAX_KEY + 1];
        let size = |error: Error| error.kind() == krummi::ErrorKind::InvalidSize;
        assert!(
            size($write.add(tree, &long, b"v").unwrap_err()),
            "the size wins"
        );
        assert!(size($write.add(tree, b"k", &long).unwrap_err()));
        assert!(size($write.remove_value(tree, b"k", &long).unwrap_err()));
        assert!(size($write.remove_key(tree, b"").unwrap_err()));
        assert!(size(
            $write.remove_key_range(tree, ..=&long[..]).unwrap_err()
        ));
        assert!(size(
            $write.remove_key_range_prefix(tree, &long, ..).unwrap_err()
        ));
        assert!(size(
            $write
                .remove_value_range(tree, b"k", &long[..]..)
                .unwrap_err()
        ));
        assert!(size(
            $write
                .remove_value_range_prefix(tree, b"k", &long, ..)
                .unwrap_err()
        ));
    }};
}

/// AC-M11 (M4, M12, A6, A7, AC-B14): in a write with a multi-value tree opened twice, deleting it
/// through one handle makes every operation through the other fail without effect with tree not found and the tree's
/// last name, also after another tree is created with that name; a handle of another transaction fails without effect
/// with misuse; and an argument out of its size wins over both.
#[test]
fn ac_m11_handles_of_a_deleted_tree_and_of_another_transaction() {
    stores(|store, folder| {
        let model = Model::from([(b"k".to_vec(), set_of(&[b"v"]))]);
        commit_multi(store, "m", &model);
        let mut write = store.write().unwrap();
        let first = write.open_multi_tree("m").unwrap();
        let second = write.open_multi_tree("m").unwrap();
        write.delete_tree("m").unwrap();
        let gone = |error: Error| assert_eq!(not_found(error, folder), "m");
        write_operations_fail!(write, second, &gone);
        let current = create_multi(&mut write, "m");
        write_operations_fail!(write, second, &gone);
        write_operations_fail!(write, first, &gone);
        assert_eq!(
            write.len(&current).unwrap(),
            0,
            "the new tree does not change"
        );
        assert!(write.add(&current, b"k", b"new").unwrap());
        write.commit().unwrap();
        let model = Model::from([(b"k".to_vec(), set_of(&[b"new"]))]);
        check_read(store, "m", &model);

        let old = store.write().unwrap();
        let stale = old.open_multi_tree("m").unwrap();
        old.commit().unwrap();
        let mut write = store.write().unwrap();
        let wrong = |error: Error| misuse(error, folder);
        write_operations_fail!(write, stale, &wrong);
        let tree = write.open_multi_tree("m").unwrap();
        check!(write, tree, model);
        write.commit().unwrap();

        let first = store.read().unwrap();
        let stale = first.open_multi_tree("m").unwrap();
        drop(first);
        let read = store.read().unwrap();
        read_operations_fail!(read, stale, &wrong);
        let tree = read.open_multi_tree("m").unwrap();
        check!(read, tree, model);
    });
}

/// A key of AC-M12: 1,000 bytes, so that [`KEYS`] of them spread over several leaves of the tree.
fn damage_key(i: usize) -> Vec<u8> {
    long_key(i, 1000)
}

/// The number of keys of AC-M12.
const KEYS: usize = 24;
/// The key of AC-M12 whose values are in a subtree of several levels, the empty value with them.
const DEEP: usize = 3;
/// A key of AC-M12 with its values in a subtree, in another leaf.
const OTHER_SUBTREE: usize = 9;
/// The key of AC-M12 whose leaf is damaged.
const HIT: usize = 13;
/// The keys of AC-M12 that are read and modified while another one needs a damaged page.
const OTHERS: [usize; 3] = [0, OTHER_SUBTREE, KEYS - 1];

/// The tree of AC-M12: [`KEYS`] keys with three values in their entries, except [`DEEP`], with the empty value and
/// twenty values of 2,048 bytes in a subtree of several levels, and [`OTHER_SUBTREE`], with 300 values in a subtree.
fn damage_model() -> Model {
    (0..KEYS)
        .map(|i| {
            let values = match i {
                DEEP => (0..20)
                    .map(|j| long_key(j, MAX_KEY))
                    .chain([vec![]])
                    .collect(),
                OTHER_SUBTREE => (0..300).map(value16).collect(),
                _ => set_of(&[b"", b"a", b"b"]),
            };
            (damage_key(i), values)
        })
        .collect()
}

/// A closed store on disk with the normal tree `other` and the multi-value tree `m` of [`damage_model`], with the
/// page that `target` gives damaged; and that page and what else `target` found before the damage.
fn damaged_multi_store<T>(target: impl Fn(&Path) -> (u64, T)) -> (tempfile::TempDir, u64, T) {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_tree(&store, "other", &[(b"k", b"v")]);
    commit_multi(&store, "m", &damage_model());
    store.close().unwrap();
    let (page, found) = target(tmp.path());
    damage(tmp.path(), page);
    (tmp, page, found)
}

/// Checks a scan that must fail with damage of `page`: the items it returns from one end until then, owned with
/// `own`, are the next ones of `expected` from that end, and after the failure nothing more comes out of either end
/// (R8).
fn scan_fails_at<I, T: PartialEq + std::fmt::Debug>(
    mut scan: impl DoubleEndedIterator<Item = Result<I>>,
    expected: &[T],
    own: impl Fn(I) -> T,
    front: bool,
    page: u64,
) {
    let mut expected: Box<dyn Iterator<Item = &T>> = if front {
        Box::new(expected.iter())
    } else {
        Box::new(expected.iter().rev())
    };
    loop {
        let step = if front { scan.next() } else { scan.next_back() };
        match step.expect("the scan fails before it ends") {
            Ok(item) => assert_eq!(own(item), *expected.next().unwrap(), "front {front}"),
            Err(error) => {
                assert_eq!(damaged_page(error), page, "front {front}");
                assert!(
                    scan.next().is_none() && scan.next_back().is_none(),
                    "the scan ends"
                );
                return;
            }
        }
    }
}

/// Checks that the operations of the transaction `$tx` on the values of `$key` that need the damaged `$page` fail with
/// it, twice: Contains with the value `$present`, which is there, and with one that is not; and value scans, which
/// return the values `$values` from either end before failing with it.
macro_rules! value_reads_fail {
    ($tx:expr, $tree:expr, $key:expr, $values:expr, $present:expr, $page:expr) => {{
        let (tx, tree, key, page): (_, _, &[u8], u64) = (&$tx, &$tree, &$key, $page);
        let values: Vec<Vec<u8>> = $values.iter().cloned().collect();
        for _ in 0..2 {
            for value in [&$present[..], b"absent"] {
                let error = tx.contains(tree, key, value).unwrap_err();
                assert_eq!(damaged_page(error), page, "contains {value:?}");
            }
            for front in [true, false] {
                let scan = tx.values(tree, key, ..).unwrap();
                scan_fails_at(scan, &values, own_value, front, page);
            }
        }
    }};
}

/// Checks that a new read sees the trees of [`damaged_multi_store`] as they were: their list, the number of entries
/// and, for each key but those of `skip`, its number of values and its values.
fn unchanged(store: &Store, model: &Model, skip: &[usize]) {
    let read = store.read().unwrap();
    let kinds = vec![("m".to_owned(), MULTI), ("other".to_owned(), NORMAL)];
    assert_eq!(read.list_trees().unwrap(), kinds);
    let tree = read.open_multi_tree("m").unwrap();
    assert_eq!(read.len(&tree).unwrap(), model.len() as u64);
    let skipped: Vec<Vec<u8>> = skip.iter().map(|&i| damage_key(i)).collect();
    for (key, values) in model.iter().filter(|(key, _)| !skipped.contains(key)) {
        assert_eq!(read.value_count(&tree, key).unwrap(), values.len() as u64);
        let found = collect(read.values(&tree, key, ..).unwrap(), own_value);
        assert_eq!(found, values.iter().cloned().collect::<Vec<_>>());
    }
}

/// Checks that a write on the store of [`damaged_multi_store`] reads and modifies the keys `keys`, which need no
/// damaged page: Contains, adding a value, removing the first one, the number of values and the value scan; that it
/// commits; and that a new read sees it, with [`unchanged`] for the keys but those of `skip`. Returns the model
/// afterwards.
fn other_keys_work(store: &Store, mut model: Model, keys: &[usize], skip: &[usize]) -> Model {
    let mut write = store.write().unwrap();
    let tree = write.open_multi_tree("m").unwrap();
    for &i in keys {
        let key = damage_key(i);
        let first = model[&key].first().unwrap().clone();
        assert!(write.contains(&tree, &key, &first).unwrap());
        assert!(write.add(&tree, &key, b"new").unwrap());
        assert!(write.remove_value(&tree, &key, &first).unwrap());
        let values = model.get_mut(&key).unwrap();
        values.insert(b"new".to_vec());
        values.remove(&first);
        let count = write.value_count(&tree, &key).unwrap();
        assert_eq!(count, values.len() as u64);
        let found = collect(write.values(&tree, &key, ..).unwrap(), own_value);
        assert_eq!(found, values.iter().cloned().collect::<Vec<_>>());
    }
    write.commit().unwrap();
    unchanged(store, &model, skip);
    model
}

/// AC-M12 (M11, A9, R8, G6, F9), on disk, with the root page of the subtree of a key damaged, a subtree of several
/// levels: adding, removing and checking one of its values that is there or not, and scanning its values, fail with
/// damage and that page, again when repeated; a value scan from the front returns the empty value, which is in the
/// key's entry, then fails and ends, and a new scan in a read fails the same; the number of values of the key, the
/// key scan and the other keys work; and a write that removes the key, removes values until it goes back to its entry
/// or away, removes a range of keys that holds it, empties the tree, deletes it or overwrites it with Rename fails
/// with damage and the subtree's page, and its commit fails with it and applies nothing (T11).
#[test]
fn ac_m12_a_damaged_subtree_of_a_key() {
    let model = damage_model();
    let deep = damage_key(DEEP);
    let (tmp, page, ()) = damaged_multi_store(|folder| {
        let (root, levels) = subtree_root(folder, "m", &damage_key(DEEP));
        assert!(levels >= 2, "a subtree of {levels} levels");
        (root, ())
    });
    let store = open(tmp.path());
    unchanged(&store, &model, &[DEEP]);
    let present = long_key(5, MAX_KEY);

    let read = store.read().unwrap();
    let tree = read.open_multi_tree("m").unwrap();
    value_reads_fail!(read, tree, deep, [Vec::new()], present, page);
    assert!(
        read.contains(&tree, &deep, b"").unwrap(),
        "the empty value is in the key's entry"
    );
    assert_eq!(read.value_count(&tree, &deep).unwrap(), 21);
    let keys = collect(read.keys(&tree, ..).unwrap(), own_key);
    assert_eq!(keys, counts(&model));
    drop(read);

    // The operations on its values fail, the write goes on, and its commit fails with the first failure.
    let mut write = store.write().unwrap();
    let tree = write.open_multi_tree("m").unwrap();
    for _ in 0..2 {
        for value in [&present[..], b"absent"] {
            let error = write.add(&tree, &deep, value).unwrap_err();
            assert_eq!(damaged_page(error), page, "add {value:?}");
            let error = write.remove_value(&tree, &deep, value).unwrap_err();
            assert_eq!(damaged_page(error), page, "remove {value:?}");
        }
    }
    value_reads_fail!(write, tree, deep, [Vec::new()], present, page);
    assert_eq!(write.value_count(&tree, &deep).unwrap(), 21);
    let keys = collect(write.keys(&tree, ..).unwrap(), own_key);
    assert_eq!(keys, counts(&model));
    assert_eq!(damaged_page(write.commit().unwrap_err()), page, "T11");
    unchanged(&store, &model, &[DEEP]);
    let model = other_keys_work(&store, model, &OTHERS, &[DEEP]);

    // Removing every value of 2,048 bytes leaves the empty value, below the demotion threshold.
    let longest: &[u8] = &long_key(0, MAX_KEY);
    let (before, after) = (damage_key(DEEP - 1), damage_key(DEEP + 1));
    for operation in [
        "remove key",
        "remove values until its entry",
        "remove all its values",
        "remove a range of keys",
        "create or empty",
        "empty",
        "delete",
        "overwrite with rename",
    ] {
        let mut write = store.write().unwrap();
        let tree = write.open_multi_tree("m").unwrap();
        let error = match operation {
            "remove key" => write.remove_key(&tree, &deep).unwrap_err(),
            "remove values until its entry" => write
                .remove_value_range(&tree, &deep, longest..)
                .unwrap_err(),
            "remove all its values" => write.remove_value_range(&tree, &deep, ..).unwrap_err(),
            "remove a range of keys" => write
                .remove_key_range(&tree, &before[..]..=&after[..])
                .unwrap_err(),
            "create or empty" => write
                .open_multi_tree_with("m", OpenMode::CreateOrEmpty)
                .err()
                .unwrap(),
            "empty" => write
                .open_multi_tree_with("m", OpenMode::Empty)
                .err()
                .unwrap(),
            "delete" => write.delete_tree("m").unwrap_err(),
            _ => write.rename_tree("other", "m", true).unwrap_err(),
        };
        assert_eq!(damaged_page(error), page, "{operation}");
        let error = write.commit().unwrap_err();
        assert_eq!(damaged_page(error), page, "{operation}: T11");
        unchanged(&store, &model, &[DEEP]);
    }
    store.close().unwrap();
}

/// AC-M12 (M11, A9, R8, G6, F9), on disk, with a damaged leaf of the tree on the way to a key: adding, removing and
/// checking one of its values that is there or not, scanning its values, counting them and removing the key or its
/// values fail with damage and that page, again when repeated, and a new scan in a read fails the same; the key scan
/// returns the keys before that leaf from each end, then fails and ends; the keys of the other leaves are read and
/// modified; and a write that failed cannot be committed (T11).
#[test]
fn ac_m12_a_damaged_leaf_on_the_way_to_a_key() {
    let model = damage_model();
    let hit = damage_key(HIT);
    let (tmp, page, in_the_leaf) = damaged_multi_store(|folder| {
        let page = leaf_page(folder, Some("m"), &damage_key(HIT));
        assert_ne!(page, root_page(folder, "m"), "a leaf below the root");
        let in_the_leaf: Vec<usize> = (0..KEYS)
            .filter(|&i| leaf_page(folder, Some("m"), &damage_key(i)) == page)
            .collect();
        for i in OTHERS.into_iter().chain([DEEP]) {
            assert!(
                !in_the_leaf.contains(&i),
                "{i} in another leaf: {in_the_leaf:?}"
            );
        }
        (page, in_the_leaf)
    });
    let store = open(tmp.path());
    unchanged(&store, &model, &in_the_leaf);
    let keys = counts(&model);

    let read = store.read().unwrap();
    let tree = read.open_multi_tree("m").unwrap();
    value_reads_fail!(read, tree, hit, Vec::<Vec<u8>>::new(), b"a", page);
    for _ in 0..2 {
        let error = read.value_count(&tree, &hit).unwrap_err();
        assert_eq!(damaged_page(error), page);
        for front in [true, false] {
            scan_fails_at(read.keys(&tree, ..).unwrap(), &keys, own_key, front, page);
        }
    }
    drop(read);

    let mut write = store.write().unwrap();
    let tree = write.open_multi_tree("m").unwrap();
    for _ in 0..2 {
        for value in [&b"a"[..], b"absent"] {
            let error = write.add(&tree, &hit, value).unwrap_err();
            assert_eq!(damaged_page(error), page, "add {value:?}");
            let error = write.remove_value(&tree, &hit, value).unwrap_err();
            assert_eq!(damaged_page(error), page, "remove {value:?}");
        }
        let error = write.remove_value_range(&tree, &hit, ..).unwrap_err();
        assert_eq!(damaged_page(error), page);
        let error = write.remove_key(&tree, &hit).unwrap_err();
        assert_eq!(damaged_page(error), page);
    }
    value_reads_fail!(write, tree, hit, Vec::<Vec<u8>>::new(), b"a", page);
    scan_fails_at(write.keys(&tree, ..).unwrap(), &keys, own_key, true, page);
    assert_eq!(damaged_page(write.commit().unwrap_err()), page, "T11");
    unchanged(&store, &model, &in_the_leaf);
    let others = [&OTHERS[..], &[DEEP]].concat();
    other_keys_work(&store, model, &others, &in_the_leaf);
    store.close().unwrap();
}

/// The tree of AC-M13 before its write: `p` with its values in its entry, near the promotion threshold; `d` and `s`
/// with theirs in subtrees, `s` with values of 2,048 bytes, in one leaf; and four keys of 2,048 bytes, one to a leaf.
fn snapshot_model() -> Model {
    let mut model = Model::from([
        (b"p".to_vec(), (0..200).map(value16).collect()),
        (b"d".to_vec(), (0..300).map(value16).collect()),
        (
            b"s".to_vec(),
            (0..2).map(|j| long_key(j, MAX_KEY)).collect(),
        ),
    ]);
    for i in 0..4 {
        model.insert(long_key(i, MAX_KEY), set_of(&[b"x"]));
    }
    model
}

/// The write of AC-M13 through `tree`, also applied to `model`: `p` moves to a subtree and back to its entry, `d` to
/// its entry and back to a subtree, the subtree of `s` gains levels with values of 2,048 bytes and loses some, and the
/// tree gains keys of 2,048 bytes and loses some, so that it gains levels too. It calls `check_snapshot` when the trees
/// have grown and when they have shrunk.
fn snapshot_write(
    write: &mut krummi::WriteTransaction<'_>,
    tree: &krummi::WriteMultiTree<'_>,
    model: &mut Model,
    check_snapshot: &dyn Fn(),
) {
    let mut add = |write: &mut krummi::WriteTransaction<'_>, key: &[u8], value: Vec<u8>| {
        assert!(write.add(tree, key, &value).unwrap());
        model.entry(key.to_vec()).or_default().insert(value);
    };
    for i in 200..240 {
        add(write, b"p", value16(i));
    }
    for j in 2..40 {
        add(write, b"s", long_key(j, MAX_KEY));
    }
    for i in 4..40 {
        add(write, &long_key(i, MAX_KEY), b"x".to_vec());
        add(write, &long_key(i, MAX_KEY), b"y".to_vec());
    }
    assert!(write.subtree_of(tree, b"p").unwrap().is_some(), "promoted");
    check_snapshot();
    let mut remove = |write: &mut krummi::WriteTransaction<'_>, key: &[u8], value: Vec<u8>| {
        assert!(write.remove_value(tree, key, &value).unwrap());
        let values = model.get_mut(key).unwrap();
        values.remove(&value);
        if values.is_empty() {
            model.remove(key);
        }
    };
    for i in 100..240 {
        remove(write, b"p", value16(i));
    }
    for i in 100..300 {
        remove(write, b"d", value16(i));
    }
    for j in (0..40).filter(|j| j % 3 != 0) {
        remove(write, b"s", long_key(j, MAX_KEY));
    }
    for i in (0..40).filter(|i| i % 3 != 0) {
        remove(write, &long_key(i, MAX_KEY), b"x".to_vec());
        if i >= 4 {
            remove(write, &long_key(i, MAX_KEY), b"y".to_vec());
        }
    }
    assert!(write.subtree_of(tree, b"p").unwrap().is_none(), "demoted");
    assert!(write.subtree_of(tree, b"d").unwrap().is_none(), "demoted");
    check_snapshot();
    for i in 100..300 {
        assert!(write.add(tree, b"d", &value16(i)).unwrap());
        model.get_mut(&b"d"[..]).unwrap().insert(value16(i));
    }
    assert!(write.subtree_of(tree, b"d").unwrap().is_some(), "promoted");
}

/// AC-M13 (G3, T14, AC-B17): a read opened before a write whose additions and removals move keys from their entries
/// to subtrees and back, and split and merge pages of the tree and of the subtrees until their heights change, sees
/// exactly its snapshot, its keys with their values and numbers of values and the number of entries, through a handle
/// opened before the write and one opened after, before and after the commit; and a new read sees the write's. On
/// disk, the heights of the tree and of a subtree are checked before and after.
#[test]
fn ac_m13_a_read_keeps_its_snapshot_while_a_write_moves_values_and_changes_heights() {
    let scenario = |store: &Store| {
        let snapshot = snapshot_model();
        commit_multi(store, "m", &snapshot);
        let read = store.read().unwrap();
        let before = read.open_multi_tree("m").unwrap();
        check!(read, before, snapshot);
        let check_snapshot = || {
            let tree = read.open_multi_tree("m").unwrap();
            check!(read, tree, snapshot);
            check!(read, before, snapshot);
        };
        let mut model = snapshot.clone();
        let mut write = store.write().unwrap();
        let tree = write.open_multi_tree("m").unwrap();
        snapshot_write(&mut write, &tree, &mut model, &check_snapshot);
        check!(write, tree, model);
        check_snapshot();
        write.commit().unwrap();
        check_snapshot();
        check_read(store, "m", &model);
    };
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_multi(&store, "m", &snapshot_model());
    store.close().unwrap();
    let heights = |folder: &Path| (height(folder, "m"), subtree_root(folder, "m", b"s").1);
    let before = heights(tmp.path());
    let store = open(tmp.path());
    scenario(&store);
    store.close().unwrap();
    let after = heights(tmp.path());
    assert!(
        after.0 > before.0 && after.1 > before.1,
        "{before:?} to {after:?}"
    );

    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    scenario(&store);
    store.close().unwrap();
}

/// AC-M17 (M9, T10): a write on disk that only adds values that are there, removes values and keys that are not,
/// removes ranges of keys and of values that remove nothing, checks, counts and scans, and fails for invalid sizes
/// and different tree types, commits without error and leaves the store's files byte-identical.
#[test]
fn ac_m17_a_write_that_changes_nothing_leaves_the_files_identical() {
    let model = Model::from([
        (b"i".to_vec(), set_of(&[b"", b"a", b"b"])),
        (b"s".to_vec(), (0..300).map(value16).collect()),
        (
            b"t".to_vec(),
            (0..300).map(value16).chain([vec![]]).collect(),
        ),
    ]);
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_tree(&store, "n", &[(b"k", b"v")]);
    commit_multi(&store, "m", &model);
    store.close().unwrap();
    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        let tree = write.open_multi_tree("m").unwrap();
        assert!(write.subtree_of(&tree, b"s").unwrap().is_some());
        for (key, values) in &model {
            for value in values {
                assert!(!write.add(&tree, key, value).unwrap());
                assert!(write.contains(&tree, key, value).unwrap());
            }
            assert_eq!(write.value_count(&tree, key).unwrap(), values.len() as u64);
            assert!(!write.remove_value(&tree, key, b"zz").unwrap());
            assert!(!write.remove_value(&tree, key, &value16(999)).unwrap());
            let (x, y): (&[u8], &[u8]) = (b"x", b"y");
            assert_eq!(write.remove_value_range(&tree, key, x..y).unwrap(), 0);
            assert_eq!(write.remove_value_range(&tree, key, y..x).unwrap(), 0);
            let empty: &[u8] = b"";
            assert_eq!(write.remove_value_range(&tree, key, ..empty).unwrap(), 0);
            assert_eq!(
                write
                    .remove_value_range_prefix(&tree, key, b"9", ..)
                    .unwrap(),
                0
            );
        }
        assert!(
            !write.remove_value(&tree, b"s", b"").unwrap(),
            "no empty value"
        );
        assert!(!write.remove_value(&tree, b"absent", b"").unwrap());
        assert_eq!(write.remove_key(&tree, b"absent").unwrap(), 0);
        assert_eq!(write.remove_value_range(&tree, b"absent", ..).unwrap(), 0);
        let (j, r, z): (&[u8], &[u8], &[u8]) = (b"j", b"r", b"z");
        assert_eq!(write.remove_key_range(&tree, j..r).unwrap(), 0);
        assert_eq!(write.remove_key_range(&tree, z..j).unwrap(), 0);
        assert_eq!(write.remove_key_range_prefix(&tree, b"q", ..).unwrap(), 0);
        check!(write, tree, model);
        assert!(write.add(&tree, b"", b"v").is_err());
        assert!(write.add(&tree, b"k", &[0; MAX_KEY + 1]).is_err());
        assert!(write.remove_key(&tree, &[0; MAX_KEY + 1]).is_err());
        assert!(
            write
                .remove_key_range_prefix(&tree, &[0; MAX_KEY + 1], ..)
                .is_err()
        );
        assert!(write.open_tree("m").is_err());
        assert!(
            write
                .open_multi_tree_with("n", OpenMode::OpenOrCreate)
                .is_err()
        );
        assert!(write.open_tree_with("m", OpenMode::Empty).is_err());
        write.commit().unwrap();
    });
    assert!(!changed, "a write that changes nothing writes nothing");
}

/// AC-M18 (M14, AC-B12): the store of format `6` that 5.1 created (`tests/data/store-5.1`) and the one of format `7`
/// that 5.3 created (`tests/data/store-5.3`, a normal tree `t` with twenty entries and one off its leaf, and an empty
/// tree `e`, made by the 5.3 code of commit `cd50b90` compiled apart) are refused as previous formats and left
/// byte-identical, with nothing added to their folders; a new store reports format `8`; and the entry of a key with a
/// subtree holds its number of values and its subtree's page in little endian, as the implementation lays it out
/// (D1: tag `1`, the flag of the empty value, the number of values, the bytes of its inline encoding, the root).
#[test]
fn ac_m18_formats_6_and_7_are_previous_and_a_new_store_has_format_8() {
    for (fixture, format) in [("store-5.1", 6), ("store-5.3", 7)] {
        let tmp = tempfile::tempdir().unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(fixture)
            .join(DATA_FILE);
        let data = tmp.path().join(DATA_FILE);
        std::fs::copy(source, &data).unwrap();
        let before = std::fs::read(&data).unwrap();
        match Store::open(tmp.path(), &StoreOptions::default()).err() {
            Some(Error::DifferentFormat {
                found, expected, ..
            }) => assert_eq!((found, expected), (format, 8), "{fixture}"),
            other => panic!("{fixture}: expected a previous format, got {other:?}"),
        }
        assert_eq!(std::fs::read(&data).unwrap(), before, "{fixture}");
        let entries: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries, vec![DATA_FILE], "{fixture}");
    }

    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    assert_eq!(store.info().format_version, 8);
    let values: BTreeSet<Vec<u8>> = (0..300).map(value16).chain([vec![]]).collect();
    commit_multi(
        &store,
        "m",
        &Model::from([(b"key".to_vec(), values.clone())]),
    );
    store.close().unwrap();
    let (leaf, subtree) = inspect(tmp.path(), |read| {
        let root = catalog::lookup(read, "m").unwrap().unwrap().root;
        let leaf = krummi_tree::leaf_of(read, root, b"key").unwrap();
        (
            leaf,
            krummi_tree::multi::subtree_of(read, root, b"key")
                .unwrap()
                .unwrap(),
        )
    });
    let mut blob = vec![1, 1];
    blob.extend((values.len() as u64).to_le_bytes());
    // The bytes of the inline encoding are the volume without the key, whose length is the first argument.
    let inline_len = volume(0, values.iter().map(Vec::len));
    blob.extend((inline_len as u64).to_le_bytes());
    blob.extend(subtree.to_le_bytes());
    let bytes = std::fs::read(tmp.path().join(DATA_FILE)).unwrap();
    let page = &bytes[leaf as usize * PAGE_SIZE..][..PAGE_SIZE];
    assert!(
        page.windows(blob.len()).any(|window| window == blob),
        "the entry of the key holds {blob:?}"
    );
}
