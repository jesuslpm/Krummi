//! Internal tests of the multi-value trees: a randomized model test against a map of sets that crosses both
//! thresholds many times, the exact points of promotion and demotion, the empty value across them, reads that never
//! touch a subtree, empty value intervals decided before any read, and freeing every page. Deterministic: the random
//! sequences come from fixed seeds.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound::{self, Excluded, Included, Unbounded};

use krummi_tx::{Engine, OpenOptions, ReadTransaction, Result};

use crate::multi::{
    self, INLINE, Keys, MultiUpdate, SUBTREE, Values, add, contains, keys, multi_thresholds,
    remove_key, remove_key_range, remove_value, remove_value_range, subtree_of, value_count,
    values, volume,
};
use crate::{Interval, MAX_KEY, PageSource, create, node};

type Model = BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>>;

/// SplitMix64: a small, deterministic generator.
struct Rng(u64);

/// Bytes that include the extremes, so that values and keys are prefixes of others and sort at both ends.
const BYTES: [u8; 5] = [0x00, 0x01, 0x7f, 0x80, 0xff];

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// One of a few keys of 1 and of [`MAX_KEY`] bytes, so that keys gather many values.
    fn key(&mut self) -> Vec<u8> {
        match self.below(7) {
            0 => vec![0x00],
            1 => vec![0x01],
            2 => vec![0x7f],
            3 => vec![0xff],
            4 => vec![0x00; MAX_KEY],
            5 => vec![0xff; MAX_KEY],
            _ => vec![0x7f, 0x00],
        }
    }

    /// A value of 0, 1, 16 or 2,048 bytes; the 1-byte values are prefixes of the 16-byte ones.
    fn value(&mut self) -> Vec<u8> {
        match self.below(40) {
            0 => Vec::new(),
            1..=8 => vec![BYTES[self.below(5) as usize]],
            9..=37 => {
                let n = self.below(200) as u8;
                let mut value = vec![BYTES[usize::from(n % 5)]];
                value.extend((1..16u8).map(|i| (n / 5).wrapping_mul(7).wrapping_add(i)));
                value
            }
            _ => vec![BYTES[self.below(5) as usize]; MAX_KEY],
        }
    }

    /// A bound: open, or included or excluded at `pick`.
    fn bound(&mut self, pick: Vec<u8>) -> Bound<Vec<u8>> {
        match self.below(3) {
            0 => Unbounded,
            1 => Included(pick),
            _ => Excluded(pick),
        }
    }

    /// A prefix: none most of the time, else one or two extreme bytes or the start of `sample`.
    fn prefix(&mut self, sample: &[u8]) -> Vec<u8> {
        match self.below(6) {
            0 => vec![BYTES[self.below(5) as usize]],
            1 => vec![0xff, 0xff],
            2 => sample[..sample.len().min(1 + self.below(3) as usize)].to_vec(),
            _ => Vec::new(),
        }
    }
}

/// An interval with owned bounds, borrowed as an [`Interval`] for a call.
struct Owned {
    prefix: Vec<u8>,
    lower: Bound<Vec<u8>>,
    upper: Bound<Vec<u8>>,
}

impl Owned {
    fn all() -> Self {
        Owned {
            prefix: Vec::new(),
            lower: Unbounded,
            upper: Unbounded,
        }
    }

    fn borrow(&self) -> Interval<'_> {
        Interval {
            prefix: &self.prefix,
            lower: self.lower.as_ref().map(Vec::as_slice),
            upper: self.upper.as_ref().map(Vec::as_slice),
        }
    }

    /// Whether `item` is in the interval, computed directly from R1 and R2.
    fn holds(&self, item: &[u8]) -> bool {
        let lower = match &self.lower {
            Included(lower) => item >= &lower[..],
            Excluded(lower) => item > &lower[..],
            Unbounded => true,
        };
        let upper = match &self.upper {
            Included(upper) => item <= &upper[..],
            Excluded(upper) => item < &upper[..],
            Unbounded => true,
        };
        lower && upper && item.starts_with(&self.prefix)
    }
}

/// A random value interval over the values of `set` (the empty value is a valid bound).
fn value_interval(rng: &mut Rng, set: Option<&BTreeSet<Vec<u8>>>) -> Owned {
    let pick = |rng: &mut Rng| -> Vec<u8> {
        let existing: Vec<&Vec<u8>> = set.map(|s| s.iter().collect()).unwrap_or_default();
        match rng.below(4) {
            0 => Vec::new(),
            1 if !existing.is_empty() => {
                existing[rng.below(existing.len() as u64) as usize].clone()
            }
            _ => rng.value(),
        }
    };
    let (a, b) = (pick(rng), pick(rng));
    // Mostly ordered bounds, sometimes inverted ones (R3).
    let (a, b) = if rng.below(8) == 0 || a <= b {
        (a, b)
    } else {
        (b, a)
    };
    let lower = rng.bound(a.clone());
    let upper = rng.bound(b);
    let prefix = rng.prefix(&a);
    Owned {
        prefix,
        lower,
        upper,
    }
}

/// A random key interval: bounds of 1 or more bytes.
fn key_interval(rng: &mut Rng) -> Owned {
    let (a, b) = (rng.key(), rng.key());
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    let lower = rng.bound(a.clone());
    let upper = rng.bound(b);
    let prefix = if rng.below(4) == 0 {
        a[..1].to_vec()
    } else {
        Vec::new()
    };
    Owned {
        prefix,
        lower,
        upper,
    }
}

/// The items of a scan: forward, backward, or alternating ends starting with the front.
fn drain<T, I: DoubleEndedIterator<Item = Result<T>>>(mut scan: I, how: u8) -> Vec<T> {
    let mut front = Vec::new();
    let mut back = Vec::new();
    let mut turn = 0u64;
    loop {
        let from_front = match how {
            0 => true,
            1 => false,
            _ => turn.is_multiple_of(2),
        };
        turn += 1;
        let item = if from_front {
            scan.next()
        } else {
            scan.next_back()
        };
        match item {
            Some(item) => {
                let item = item.unwrap();
                if from_front {
                    front.push(item);
                } else {
                    back.push(item);
                }
            }
            None => break,
        }
    }
    assert!(scan.next().is_none() && scan.next_back().is_none(), "fused");
    back.reverse();
    front.extend(back);
    front
}

fn scan_values<S: PageSource>(
    tx: &S,
    root: u64,
    key: &[u8],
    interval: &Owned,
    how: u8,
) -> Vec<Vec<u8>> {
    let scan: Values<'_, S> = values(tx, root, key, interval.borrow()).unwrap();
    drain(scan, how).into_iter().map(<[u8]>::to_vec).collect()
}

fn scan_keys<S: PageSource>(tx: &S, root: u64, interval: &Owned, how: u8) -> Vec<(Vec<u8>, u64)> {
    let scan: Keys<'_, S> = keys(tx, root, interval.borrow()).unwrap();
    drain(scan, how)
        .into_iter()
        .map(|(k, c)| (k.to_vec(), c))
        .collect()
}

/// The volume of a key with the model's values (D2).
fn model_volume(key: &[u8], set: &BTreeSet<Vec<u8>>) -> usize {
    volume(key.len(), set.iter().map(Vec::len))
}

/// Checks everything the model says: the keys with their counts, each key's values in both directions, each
/// value's presence, the representation each key may have, absent keys, and random intervals of keys and values.
fn check<S: PageSource>(tx: &S, root: u64, model: &Model, rng: &mut Rng, label: &str) {
    let (promotion, demotion) = multi_thresholds();
    let expected: Vec<(Vec<u8>, u64)> = model
        .iter()
        .map(|(k, s)| (k.clone(), s.len() as u64))
        .collect();
    for how in 0..3 {
        assert!(
            scan_keys(tx, root, &Owned::all(), how) == expected,
            "{label}: keys {how}"
        );
    }
    for (key, set) in model {
        assert_eq!(
            value_count(tx, root, key).unwrap(),
            set.len() as u64,
            "{label}"
        );
        let all: Vec<Vec<u8>> = set.iter().cloned().collect();
        for how in 0..3 {
            assert!(
                scan_values(tx, root, key, &Owned::all(), how) == all,
                "{label}: values {how}"
            );
        }
        for value in set {
            assert!(contains(tx, root, key, value).unwrap(), "{label}");
        }
        for _ in 0..5 {
            let value = rng.value();
            assert_eq!(
                contains(tx, root, key, &value).unwrap(),
                set.contains(&value),
                "{label}"
            );
        }
        let volume = model_volume(key, set);
        match subtree_of(tx, root, key).unwrap() {
            Some(_) => assert!(
                volume >= demotion,
                "{label}: a subtree below demotion: {volume}"
            ),
            None => assert!(
                volume <= promotion,
                "{label}: inline above promotion: {volume}"
            ),
        }
        for _ in 0..4 {
            let interval = value_interval(rng, Some(set));
            let expected: Vec<Vec<u8>> =
                set.iter().filter(|v| interval.holds(v)).cloned().collect();
            let how = rng.below(3) as u8;
            assert!(
                scan_values(tx, root, key, &interval, how) == expected,
                "{label}: value interval {how}"
            );
        }
    }
    for _ in 0..3 {
        let key = rng.key();
        if !model.contains_key(&key) {
            assert_eq!(value_count(tx, root, &key).unwrap(), 0);
            assert!(scan_values(tx, root, &key, &Owned::all(), 0).is_empty());
            assert!(!contains(tx, root, &key, &rng.value()).unwrap());
        }
        let interval = key_interval(rng);
        let expected: Vec<(Vec<u8>, u64)> = model
            .iter()
            .filter(|(k, _)| interval.holds(k))
            .map(|(k, s)| (k.clone(), s.len() as u64))
            .collect();
        let how = rng.below(3) as u8;
        assert!(
            scan_keys(tx, root, &interval, how) == expected,
            "{label}: key interval"
        );
    }
}

/// Transitions seen by the model test.
#[derive(Default)]
struct Transitions {
    promotions: u32,
    demotions: u32,
}

/// Applies a random operation to the tree and the model, checking what it reports; `grow` makes additions frequent.
fn random_op(
    tx: &mut krummi_tx::WriteTransaction<'_>,
    root: &mut u64,
    model: &mut Model,
    rng: &mut Rng,
    grow: bool,
    seen: &mut Transitions,
) {
    let key = rng.key();
    let before = subtree_of(&*tx, *root, &key).unwrap().is_some();
    let roll = rng.below(100);
    let adds = if grow { 75 } else { 30 };
    if roll < adds {
        let value = rng.value();
        let MultiUpdate {
            root: new_root,
            values,
            key_changed,
        } = add(tx, *root, &key, &value).unwrap();
        let created = !model.contains_key(&key);
        let added = model.entry(key.clone()).or_default().insert(value);
        assert_eq!((values, key_changed), (u64::from(added), created));
        *root = new_root;
    } else if roll < 93 {
        // Mostly a value the key has.
        let value = match model.get(&key) {
            Some(set) if rng.below(4) != 0 => set
                .iter()
                .nth(rng.below(set.len() as u64) as usize)
                .unwrap()
                .clone(),
            _ => rng.value(),
        };
        let update = remove_value(tx, *root, &key, &value).unwrap();
        let removed = model.get_mut(&key).is_some_and(|s| s.remove(&value));
        let gone = model.get(&key).is_some_and(BTreeSet::is_empty);
        if gone {
            model.remove(&key);
        }
        assert_eq!(
            (update.values, update.key_changed),
            (u64::from(removed), gone)
        );
        *root = update.root;
    } else if roll < 97 {
        let interval = value_interval(rng, model.get(&key));
        let update = remove_value_range(tx, *root, &key, interval.borrow()).unwrap();
        let mut removed = 0;
        if let Some(set) = model.get_mut(&key) {
            let before = set.len();
            set.retain(|v| !interval.holds(v));
            removed = (before - set.len()) as u64;
        }
        let gone = model.get(&key).is_some_and(BTreeSet::is_empty);
        if gone {
            model.remove(&key);
        }
        assert_eq!((update.values, update.key_changed), (removed, gone));
        *root = update.root;
    } else if roll < 99 {
        let update = remove_key(tx, *root, &key).unwrap();
        let removed = model.remove(&key).map_or(0, |s| s.len() as u64);
        assert_eq!((update.values, update.key_changed), (removed, removed > 0));
        *root = update.root;
    } else {
        let interval = key_interval(rng);
        let update = remove_key_range(tx, *root, interval.borrow()).unwrap();
        let before = model.len();
        model.retain(|k, _| !interval.holds(k));
        assert_eq!(update.removed, (before - model.len()) as u64);
        *root = update.root;
    }
    let after = subtree_of(&*tx, *root, &key).unwrap().is_some();
    match (before, after) {
        (false, true) => seen.promotions += 1,
        (true, false) if model.contains_key(&key) => seen.demotions += 1,
        _ => {}
    }
}

/// AC-M1, AC-M3–AC-M7 at the engine: thousands of random operations on keys of 1 and 2,048 bytes and values of 0,
/// 1, 16 and 2,048 bytes match the model after every batch, in the write, in a later read and after reopening the
/// store, while keys cross both thresholds many times.
#[test]
fn random_operations_match_a_map_of_sets_across_both_thresholds() {
    let tmp = tempfile::tempdir().unwrap();
    let mut engine = Engine::open(tmp.path(), &OpenOptions::default()).unwrap();
    let mut rng = Rng(0x5eed);
    let mut model = Model::new();
    let mut seen = Transitions::default();
    let mut root = {
        let mut tx = engine.write(None).unwrap();
        let root = create(&mut tx).unwrap();
        tx.commit().unwrap();
        root
    };
    for batch in 0..48 {
        let grow = (batch / 4) % 2 == 0;
        let mut tx = engine.write(None).unwrap();
        for _ in 0..90 {
            random_op(&mut tx, &mut root, &mut model, &mut rng, grow, &mut seen);
        }
        check(
            &tx,
            root,
            &model,
            &mut rng,
            &format!("batch {batch}: write"),
        );
        tx.commit().unwrap();
        if batch % 8 == 7 {
            engine.close().unwrap();
            engine = Engine::open(tmp.path(), &OpenOptions::default()).unwrap();
        }
        let read = engine.read().unwrap();
        check(
            &read,
            root,
            &model,
            &mut rng,
            &format!("batch {batch}: read"),
        );
    }
    assert!(
        seen.promotions >= 10 && seen.demotions >= 10,
        "promotions {} demotions {}",
        seen.promotions,
        seen.demotions
    );
}

fn temporary() -> Engine {
    Engine::open_temporary(&OpenOptions::default()).unwrap()
}

/// A distinct 16-byte value for each `i`.
fn sixteen(i: u32) -> Vec<u8> {
    let mut value = vec![0x10; 16];
    value[..4].copy_from_slice(&i.to_be_bytes());
    value
}

/// The values of `key` in both directions, its count and its presence checks agree with `set`.
fn agree<S: PageSource>(tx: &S, root: u64, key: &[u8], set: &BTreeSet<Vec<u8>>, label: &str) {
    let all: Vec<Vec<u8>> = set.iter().cloned().collect();
    assert!(
        scan_values(tx, root, key, &Owned::all(), 0) == all,
        "{label}: forward"
    );
    let mut reversed = all.clone();
    reversed.reverse();
    let backward: Vec<Vec<u8>> = values(tx, root, key, Owned::all().borrow())
        .unwrap()
        .rev()
        .map(|v| v.unwrap().to_vec())
        .collect();
    assert!(backward == reversed, "{label}: backward");
    assert_eq!(
        value_count(tx, root, key).unwrap(),
        set.len() as u64,
        "{label}"
    );
    for value in set {
        assert!(contains(tx, root, key, value).unwrap(), "{label}");
    }
}

/// AC-M5 at the engine: adding values one by one, with and without the empty value, moves them to a subtree exactly
/// when the volume exceeds the promotion threshold; removing them keeps the subtree between the thresholds and moves
/// them back exactly when the volume falls below the demotion threshold; the values agree after every step, in the
/// same write and in a later read. The key's length makes the volume, which grows by 18 bytes per value, land exactly
/// on the threshold under test.
#[test]
fn promotion_and_demotion_happen_exactly_at_the_thresholds() {
    let (promotion, demotion) = multi_thresholds();
    for with_empty in [false, true] {
        for (target, up) in [(promotion, true), (demotion, false)] {
            let engine = temporary();
            let base = volume(0, if with_empty { vec![0] } else { vec![] });
            let key = vec![b'k'; 1 + (target - base - 1) % 18];
            let key = &key[..];
            let mut landed = false;
            let mut set = BTreeSet::new();
            let mut tx = engine.write(None).unwrap();
            let mut root = create(&mut tx).unwrap();
            if with_empty {
                root = add(&mut tx, root, key, b"").unwrap().root;
                set.insert(Vec::new());
            }
            let mut i = 0;
            // Up: inline while the volume is at most the promotion threshold, a subtree as soon as it exceeds it.
            loop {
                let value = sixteen(i);
                i += 1;
                set.insert(value.clone());
                let update = add(&mut tx, root, key, &value).unwrap();
                assert_eq!(update.values, 1);
                root = update.root;
                let volume = model_volume(key, &set);
                landed |= up && volume == target;
                let promoted = subtree_of(&tx, root, key).unwrap().is_some();
                assert_eq!(promoted, volume > promotion, "volume {volume}");
                agree(&tx, root, key, &set, &format!("up {volume}"));
                if promoted {
                    break;
                }
            }
            tx.commit().unwrap();
            let read = engine.read().unwrap();
            agree(&read, root, key, &set, "after promotion");
            drop(read);
            // Down: a subtree while the volume is at least the demotion threshold, inline as soon as it is below it.
            let mut tx = engine.write(None).unwrap();
            let mut removable: Vec<Vec<u8>> =
                set.iter().filter(|v| !v.is_empty()).cloned().collect();
            loop {
                let value = removable.pop().unwrap();
                set.remove(&value);
                let update = remove_value(&mut tx, root, key, &value).unwrap();
                assert_eq!((update.values, update.key_changed), (1, false));
                root = update.root;
                let volume = model_volume(key, &set);
                landed |= !up && volume == target;
                let inline = subtree_of(&tx, root, key).unwrap().is_none();
                assert_eq!(inline, volume < demotion, "volume {volume}");
                agree(&tx, root, key, &set, &format!("down {volume}"));
                if inline {
                    break;
                }
            }
            assert!(landed, "the volume reached {target}");
            // The empty value kept its place across both moves.
            assert_eq!(set.contains(&Vec::new()), with_empty);
            assert_eq!(
                scan_values(&tx, root, key, &Owned::all(), 0)
                    .first()
                    .is_some_and(Vec::is_empty),
                with_empty
            );
            tx.commit().unwrap();
            let read = engine.read().unwrap();
            agree(&read, root, key, &set, "after demotion");
        }
    }
}

/// AC-M5 at the engine: removing a value interval that takes the volume from above the promotion threshold to
/// below the demotion threshold moves the values back to the entry in that operation, the empty value included.
#[test]
fn a_value_range_removal_demotes_in_one_operation() {
    let (promotion, demotion) = multi_thresholds();
    let engine = temporary();
    let key = b"k";
    let mut tx = engine.write(None).unwrap();
    let mut root = create(&mut tx).unwrap();
    let mut set = BTreeSet::from([Vec::new()]);
    root = add(&mut tx, root, key, b"").unwrap().root;
    let mut i = 0;
    while model_volume(key, &set) <= promotion + 200 {
        set.insert(sixteen(i));
        root = add(&mut tx, root, key, &sixteen(i)).unwrap().root;
        i += 1;
    }
    assert!(subtree_of(&tx, root, key).unwrap().is_some());
    // Keep the empty value and the first three: the rest goes in one removal.
    let from = sixteen(3);
    let interval = Interval {
        prefix: b"",
        lower: Included(&from),
        upper: Unbounded,
    };
    let update = remove_value_range(&mut tx, root, key, interval).unwrap();
    assert_eq!(
        (update.values, update.key_changed),
        (u64::from(i) - 3, false)
    );
    root = update.root;
    set.retain(|v| v < &from);
    assert!(model_volume(key, &set) < demotion);
    assert!(subtree_of(&tx, root, key).unwrap().is_none());
    agree(&tx, root, key, &set, "after the removal");
    // Removing every value but the empty one, then the empty one, leaves no key.
    let all_but_empty = Interval {
        prefix: b"",
        lower: Excluded(b""),
        upper: Unbounded,
    };
    let update = remove_value_range(&mut tx, root, key, all_but_empty).unwrap();
    assert_eq!((update.values, update.key_changed), (3, false));
    let update = remove_value_range(&mut tx, update.root, key, Owned::all().borrow()).unwrap();
    assert_eq!((update.values, update.key_changed), (1, true));
    assert_eq!(value_count(&tx, update.root, key).unwrap(), 0);
    tx.commit().unwrap();
}

/// D2: a key of 2,048 bytes with one value of 2,048 bytes exceeds every promotion threshold, so it goes straight to
/// a subtree; and a key whose only value is in its subtree keeps it until the key goes.
#[test]
fn a_longest_key_with_a_longest_value_starts_in_a_subtree() {
    let engine = temporary();
    let key = vec![0xAB; MAX_KEY];
    let value = vec![0xCD; MAX_KEY];
    let mut tx = engine.write(None).unwrap();
    let root = create(&mut tx).unwrap();
    let update = add(&mut tx, root, &key, &value).unwrap();
    assert_eq!((update.values, update.key_changed), (1, true));
    let root = update.root;
    assert!(subtree_of(&tx, root, &key).unwrap().is_some());
    agree(&tx, root, &key, &BTreeSet::from([value.clone()]), "long");
    let update = remove_value(&mut tx, root, &key, &value).unwrap();
    assert_eq!((update.values, update.key_changed), (1, true));
    assert_eq!(value_count(&tx, update.root, &key).unwrap(), 0);
    tx.commit().unwrap();
}

/// A page source that records the pages it reads.
struct Counting<'a, 'e> {
    tx: &'a ReadTransaction<'e>,
    pages: RefCell<Vec<u64>>,
}

impl PageSource for Counting<'_, '_> {
    fn page(&self, page_number: u64) -> Result<&[u8]> {
        self.pages.borrow_mut().push(page_number);
        PageSource::page(self.tx, page_number)
    }

    fn run(&self, first: u64, count: u64) -> Result<&[u8]> {
        self.pages.borrow_mut().push(first);
        PageSource::run(self.tx, first, count)
    }

    fn slot(&self, slot: usize) -> Result<u64> {
        PageSource::slot(self.tx, slot)
    }

    fn may_hold_data(&self, page_number: u64) -> bool {
        PageSource::may_hold_data(self.tx, page_number)
    }

    fn may_hold_run(&self, first: u64, count: u64) -> bool {
        PageSource::may_hold_run(self.tx, first, count)
    }

    fn damaged(&self, page_number: u64) -> krummi_tx::Error {
        PageSource::damaged(self.tx, page_number)
    }

    fn fail(&self, error: krummi_tx::Error) -> krummi_tx::Error {
        error
    }
}

/// Every page of the tree at `page`.
fn pages_of<S: PageSource>(tx: &S, page: u64, out: &mut Vec<u64>) {
    out.push(page);
    let node = tx.page(page).unwrap();
    if !node::is_leaf(node) {
        for index in 0..=node::count(node) {
            pages_of(tx, node::child(node, index), out);
        }
    }
}

/// AC-M12, M11 and D3 at the engine: the value count of a key and the key scan read the parent tree only, never a
/// page of a subtree (here of two levels).
#[test]
fn value_counts_and_key_scans_never_read_a_subtree() {
    let engine = temporary();
    let mut tx = engine.write(None).unwrap();
    let mut root = create(&mut tx).unwrap();
    for i in 0..1000 {
        root = add(&mut tx, root, b"big", &sixteen(i)).unwrap().root;
    }
    for key in [&b"a"[..], b"z"] {
        root = add(&mut tx, root, key, b"v").unwrap().root;
    }
    tx.commit().unwrap();
    let read = engine.read().unwrap();
    let subtree = subtree_of(&read, root, b"big").unwrap().unwrap();
    assert!(node::level(read.page(subtree).unwrap()) >= 1, "two levels");
    let mut subtree_pages = Vec::new();
    pages_of(&read, subtree, &mut subtree_pages);
    let counting = Counting {
        tx: &read,
        pages: RefCell::new(Vec::new()),
    };
    assert_eq!(value_count(&counting, root, b"big").unwrap(), 1000);
    let all = scan_keys(&counting, root, &Owned::all(), 2);
    assert_eq!(
        all,
        vec![
            (b"a".to_vec(), 1),
            (b"big".to_vec(), 1000),
            (b"z".to_vec(), 1)
        ]
    );
    let read_pages = counting.pages.borrow();
    assert!(!read_pages.is_empty());
    assert!(
        read_pages.iter().all(|p| !subtree_pages.contains(p)),
        "read {read_pages:?}, subtree {subtree_pages:?}"
    );
}

/// M7 and R3: an empty value interval (inverted bounds, equal bounds with one excluded, a prefix outside the bounds,
/// nothing before the empty value) is decided before reading any page, even with the key's entry page damaged; a
/// non-empty one then fails with the damage.
#[test]
fn an_empty_value_interval_reads_nothing() {
    let engine = temporary();
    let mut tx = engine.write(None).unwrap();
    let mut root = create(&mut tx).unwrap();
    root = add(&mut tx, root, b"k", b"value").unwrap().root;
    let leaf = crate::leaf_of(&tx, root, b"k").unwrap();
    tx.commit().unwrap();
    let mut tx = engine.write(None).unwrap();
    tx.page_mut(leaf).unwrap()[0] = 7;
    tx.commit().unwrap();
    let empties: [Interval<'_>; 5] = [
        Interval {
            prefix: b"",
            lower: Included(b"b"),
            upper: Included(b"a"),
        },
        Interval {
            prefix: b"",
            lower: Included(b"a"),
            upper: Excluded(b"a"),
        },
        Interval {
            prefix: b"",
            lower: Excluded(b""),
            upper: Included(b""),
        },
        Interval {
            prefix: b"",
            lower: Unbounded,
            upper: Excluded(b""),
        },
        Interval {
            prefix: b"x",
            lower: Unbounded,
            upper: Included(b"a"),
        },
    ];
    let read = engine.read().unwrap();
    for interval in empties {
        assert!(
            values(&read, root, b"k", interval)
                .unwrap()
                .next()
                .is_none(),
            "{interval:?}"
        );
    }
    let only_empty = Interval {
        prefix: b"",
        lower: Unbounded,
        upper: Included(b""),
    };
    assert!(matches!(
        values(&read, root, b"k", only_empty).unwrap().next(),
        Some(Err(krummi_tx::Error::Damaged { .. }))
    ));
    drop(read);
    let mut tx = engine.write(None).unwrap();
    for interval in empties {
        let update = remove_value_range(&mut tx, root, b"k", interval).unwrap();
        assert_eq!(
            update,
            MultiUpdate {
                root,
                values: 0,
                key_changed: false
            }
        );
    }
    tx.commit().unwrap();
}

/// Pages in use: the pages of the store minus the free pages of its information.
fn in_use(engine: &Engine) -> u64 {
    let info = engine.info();
    info.page_count - info.free_pages
}

/// AC-M8 and D6 at the engine: a tree with keys inline and in subtrees, emptied by each removal operation, uses the
/// pages of an empty tree; freeing it whole gives back every page it used.
#[test]
fn removing_every_key_or_freeing_the_tree_returns_every_page() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Engine::open(tmp.path(), &OpenOptions::default()).unwrap();
    let fill = |tx: &mut krummi_tx::WriteTransaction<'_>, mut root: u64| -> (u64, Model) {
        let mut model = Model::new();
        for k in 0..12u8 {
            let key = vec![b'k', k];
            let n = if k % 3 == 0 { 400 } else { 5 };
            for i in 0..n {
                let value = if i == 0 { Vec::new() } else { sixteen(i) };
                root = add(tx, root, &key, &value).unwrap().root;
                model.entry(key.clone()).or_default().insert(value);
            }
        }
        (root, model)
    };
    for form in 0..6 {
        let mut tx = engine.write(None).unwrap();
        let empty_root = create(&mut tx).unwrap();
        tx.commit().unwrap();
        let empty = in_use(&engine);
        let mut tx = engine.write(None).unwrap();
        let (mut root, model) = fill(&mut tx, empty_root);
        assert!(
            model
                .keys()
                .any(|k| subtree_of(&tx, root, k).unwrap().is_some())
        );
        tx.commit().unwrap();
        assert!(in_use(&engine) >= empty + 8, "{} {empty}", in_use(&engine));
        let mut tx = engine.write(None).unwrap();
        match form {
            0 => {
                for (key, set) in &model {
                    for value in set {
                        root = remove_value(&mut tx, root, key, value).unwrap().root;
                    }
                }
            }
            1 => {
                for key in model.keys() {
                    root = remove_key(&mut tx, root, key).unwrap().root;
                }
            }
            2 => {
                for key in model.keys() {
                    root = remove_value_range(&mut tx, root, key, Owned::all().borrow())
                        .unwrap()
                        .root;
                }
            }
            3 => {
                let update = remove_key_range(&mut tx, root, Owned::all().borrow()).unwrap();
                assert_eq!(update.removed, model.len() as u64);
                root = update.root;
            }
            4 => {
                multi::free_tree(&mut tx, root).unwrap();
                tx.commit().unwrap();
                assert_eq!(in_use(&engine), empty - 1, "freed whole");
                continue;
            }
            _ => {
                // Demote every subtree key by removing values, then remove the keys.
                for (key, set) in &model {
                    for value in set.iter().skip(4) {
                        root = remove_value(&mut tx, root, key, value).unwrap().root;
                    }
                    assert!(subtree_of(&tx, root, key).unwrap().is_none());
                }
                for key in model.keys() {
                    root = remove_key(&mut tx, root, key).unwrap().root;
                }
            }
        }
        assert!(
            scan_keys(&tx, root, &Owned::all(), 0).is_empty(),
            "form {form}"
        );
        tx.commit().unwrap();
        assert_eq!(in_use(&engine), empty, "form {form}");
        let mut tx = engine.write(None).unwrap();
        crate::free_all(&mut tx, root).unwrap();
        tx.commit().unwrap();
    }
}

/// AC-M8 at the engine: a key moved back to its entry from a subtree uses the same pages as one whose values never
/// reached a subtree.
#[test]
fn a_demoted_key_uses_the_pages_of_one_never_promoted() {
    let (promotion, _) = multi_thresholds();
    let mut used = Vec::new();
    for promote in [true, false] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = Engine::open(tmp.path(), &OpenOptions::default()).unwrap();
        let mut tx = engine.write(None).unwrap();
        let mut root = create(&mut tx).unwrap();
        let mut set = BTreeSet::new();
        for i in 0..10 {
            root = add(&mut tx, root, b"k", &sixteen(i)).unwrap().root;
            set.insert(sixteen(i));
        }
        tx.commit().unwrap();
        if promote {
            let mut tx = engine.write(None).unwrap();
            let mut i = 10;
            while model_volume(b"k", &set) <= promotion {
                root = add(&mut tx, root, b"k", &sixteen(i)).unwrap().root;
                set.insert(sixteen(i));
                i += 1;
            }
            assert!(subtree_of(&tx, root, b"k").unwrap().is_some());
            let interval = Interval {
                prefix: b"",
                lower: Included(&sixteen(10)),
                upper: Unbounded,
            };
            root = remove_value_range(&mut tx, root, b"k", interval)
                .unwrap()
                .root;
            assert!(subtree_of(&tx, root, b"k").unwrap().is_none());
            tx.commit().unwrap();
        }
        used.push(in_use(&engine));
        let _ = root;
    }
    assert_eq!(used[0], used[1]);
}

/// The blob of a subtree with these fields, laid out as D1 says.
fn subtree_blob(flags: u8, count: u64, inline_len: u64, root: u64) -> Vec<u8> {
    let mut blob = vec![SUBTREE, flags];
    for field in [count, inline_len, root] {
        blob.extend_from_slice(&field.to_le_bytes());
    }
    blob
}

/// The inline blob that claims `count` values and holds `elements`, each written as its length and its bytes.
fn inline_blob(count: u16, elements: &[&[u8]]) -> Vec<u8> {
    let mut blob = vec![INLINE];
    blob.extend_from_slice(&count.to_le_bytes());
    for element in elements {
        blob.extend_from_slice(&(element.len() as u16).to_le_bytes());
        blob.extend_from_slice(element);
    }
    blob
}

/// The page of a damage error.
#[track_caller]
fn damaged_page<T: std::fmt::Debug>(result: Result<T>) -> u64 {
    match result {
        Err(krummi_tx::Error::Damaged {
            page: Some(page), ..
        }) => page,
        other => panic!("expected damage with a page, got {other:?}"),
    }
}

/// G6 and M11 at the engine: an entry of a multi-value tree that is not a consistent blob, in a leaf whose checksum
/// is valid, is damage of that leaf for every operation that reads it: an unknown tag, an empty blob, an inline blob
/// with no values, a count that does not match its elements (fewer, or trailing bytes), an element longer than
/// [`MAX_KEY`] or than what is left, a subtree blob of the wrong length, with unknown flags, with no values or with
/// inline bytes that cannot be those of its values, and an entry kept off its leaf. The checks are structural, as
/// those of the tree's nodes: the order of the values is not checked.
#[test]
fn inconsistent_blobs_are_damage_of_their_leaf() {
    let long = vec![b'x'; MAX_KEY + 1];
    // Five values of 1,000 bytes: a consistent blob, but longer than a leaf cell, so its entry is off its leaf.
    let off_leaf: Vec<Vec<u8>> = (b'a'..=b'e').map(|c| vec![c; 1000]).collect();
    let off_leaf: Vec<&[u8]> = off_leaf.iter().map(Vec::as_slice).collect();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("unknown tag", vec![2, 1, 0, 1, 0, b'a']),
        ("empty", Vec::new()),
        ("inline header cut short", vec![0, 1]),
        ("no values", inline_blob(0, &[])),
        ("fewer values than counted", inline_blob(2, &[b"a"])),
        ("more values than counted", inline_blob(1, &[b"a", b"b"])),
        ("element too long", inline_blob(1, &[&long])),
        ("element past the end", vec![0, 1, 0, 5, 0, b'a']),
        (
            "subtree blob too short",
            subtree_blob(0, 1, 5, 9)[..25].to_vec(),
        ),
        (
            "subtree blob too long",
            [subtree_blob(0, 1, 5, 9), vec![0]].concat(),
        ),
        ("unknown subtree flags", subtree_blob(2, 1, 5, 9)),
        ("subtree with no values", subtree_blob(0, 0, 3, 9)),
        ("inline bytes below the values'", subtree_blob(0, 2, 6, 9)),
        (
            "inline bytes above the values'",
            subtree_blob(0, 1, 3 + 2 + MAX_KEY as u64 + 1, 9),
        ),
        ("inline bytes without a header", subtree_blob(0, 1, 2, 9)),
        ("off its leaf", inline_blob(5, &off_leaf)),
    ];
    let engine = temporary();
    let all = Owned::all();
    for (label, blob) in cases {
        let mut tx = engine.write(None).unwrap();
        // Keys of 1,000 bytes around the damaged one, so that its leaf is not the root.
        let mut root = create(&mut tx).unwrap();
        for i in 0..12u8 {
            root = add(&mut tx, root, &[i; 1000], b"v").unwrap().root;
        }
        let key = [6, 1];
        root = crate::set(&mut tx, root, &key, &blob).unwrap().root;
        let off = crate::run_of(&tx, root, &key).unwrap().is_some();
        assert_eq!(off, label == "off its leaf", "{label}");
        let leaf = crate::leaf_of(&tx, root, &key).unwrap();
        assert_ne!(leaf, root, "{label}");
        assert_eq!(damaged_page(value_count(&tx, root, &key)), leaf, "{label}");
        assert_eq!(
            damaged_page(contains(&tx, root, &key, b"a")),
            leaf,
            "{label}"
        );
        let mut scan = values(&tx, root, &key, all.borrow()).unwrap();
        assert_eq!(damaged_page(scan.next().unwrap()), leaf, "{label}");
        let mut scan = keys(&tx, root, all.borrow()).unwrap();
        assert_eq!(scan.next().unwrap().unwrap().0, [0; 1000], "{label}");
        let damaged = scan.find(Result::is_err).unwrap();
        assert_eq!(damaged_page(damaged), leaf, "{label}: key scan");
        assert!(scan.next().is_none(), "{label}: the scan ends");
        assert_eq!(
            damaged_page(add(&mut tx, root, &key, b"z")),
            leaf,
            "{label}"
        );
        let removed = remove_value(&mut tx, root, &key, b"a");
        assert_eq!(damaged_page(removed), leaf, "{label}");
        assert_eq!(
            damaged_page(remove_key(&mut tx, root, &key)),
            leaf,
            "{label}"
        );
        let removed = remove_value_range(&mut tx, root, &key, all.borrow());
        assert_eq!(damaged_page(removed), leaf, "{label}");
        let removed = remove_key_range(&mut tx, root, all.borrow());
        assert_eq!(damaged_page(removed), leaf, "{label}");
        let freed = multi::free_tree(&mut tx, root);
        assert_eq!(damaged_page(freed), leaf, "{label}");
        assert_eq!(damaged_page(tx.commit()), leaf, "{label}: T11");
    }
}
