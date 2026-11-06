//! Internal tests of the fixed-size trees: keys in numeric order at the extremes, the operations by key at every
//! length, scans and range removals with every kind of bound and the empty intervals of D4, a seeded model test
//! against `BTreeMap` that grows and shrinks the tree by several levels, freeing every page, and the layout check
//! with the remembered check it shares with the slotted nodes. Deterministic: the random sequences come from fixed
//! seeds.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::ops::Bound::{self, Excluded, Included, Unbounded};
use std::ops::RangeBounds;

use krummi_tx::{Engine, Error, OpenOptions, PAYLOAD_SIZE, Result, WriteTransaction};

use crate::fixed::{
    self, FixedRange, MAX_LEN, create, free_all, get, height, interval, leaf_of, remove,
    remove_range, set, try_add,
};
use crate::{PageSource, RangeUpdate, node};

type Model = BTreeMap<u64, Vec<u8>>;

/// The keys at the extremes of the order and around its middle.
const EXTREMES: [u64; 6] = [0, 1, (1 << 63) - 1, 1 << 63, u64::MAX - 1, u64::MAX];

/// SplitMix64: a small, deterministic generator.
struct Rng(u64);

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

    /// A random key, one time in eight one of the [`EXTREMES`].
    fn key(&mut self) -> u64 {
        if self.below(8) == 0 {
            EXTREMES[self.below(6) as usize]
        } else {
            self.next()
        }
    }

    /// A value of `len` bytes.
    fn value(&mut self, len: usize) -> Vec<u8> {
        let byte = self.next() as u8;
        (0..len).map(|i| byte.wrapping_add(i as u8)).collect()
    }

    /// A key of `model` when it has some and the dice say so, or else a random one.
    fn key_of(&mut self, model: &Model) -> u64 {
        if !model.is_empty() && self.below(2) == 0 {
            let index = self.below(model.len() as u64) as usize;
            *model.keys().nth(index).unwrap()
        } else {
            self.key()
        }
    }

    /// A bound: open, or a key of `model` or a random one, included or excluded.
    fn bound(&mut self, model: &Model) -> Bound<u64> {
        match self.below(5) {
            0 => Unbounded,
            1 | 2 => Included(self.key_of(model)),
            _ => Excluded(self.key_of(model)),
        }
    }

    fn shuffle(&mut self, keys: &mut [u64]) {
        for i in (1..keys.len()).rev() {
            keys.swap(i, self.below(i as u64 + 1) as usize);
        }
    }
}

/// The value of `key` in the trees of the tests that do not draw values: its bytes, repeated and varied.
fn value_of(key: u64, len: usize) -> Vec<u8> {
    let bytes = key.to_le_bytes();
    (0..len).map(|i| bytes[i % 8] ^ (i / 8) as u8).collect()
}

fn temporary() -> Engine {
    Engine::open_temporary(&OpenOptions::default()).unwrap()
}

fn new_tree(engine: &Engine, len: usize) -> u64 {
    let mut tx = engine.write(None).unwrap();
    let root = create(&mut tx, len).unwrap();
    tx.commit().unwrap();
    root
}

/// A tree of length `len` with `keys`, stored in that order in one write of `engine`, and the model of it.
fn tree_with(engine: &Engine, len: usize, keys: &[u64]) -> (u64, Model) {
    let mut tx = engine.write(None).unwrap();
    let mut root = create(&mut tx, len).unwrap();
    let mut model = Model::new();
    for &key in keys {
        root = set(&mut tx, root, len, key, &value_of(key, len))
            .unwrap()
            .root;
        model.insert(key, value_of(key, len));
    }
    tx.commit().unwrap();
    (root, model)
}

type Entries = Vec<(u64, Vec<u8>)>;

fn owned(entry: Result<(u64, &[u8])>) -> (u64, Vec<u8>) {
    let (key, value) = entry.unwrap();
    (key, value.to_vec())
}

fn entries<S: PageSource>(tx: &S, root: u64, len: usize) -> Entries {
    FixedRange::new(tx, root, len, Unbounded, Unbounded)
        .map(owned)
        .collect()
}

/// The entries of `model` between `lower` and `upper`, in key order.
fn expected(model: &Model, lower: Bound<u64>, upper: Bound<u64>) -> Entries {
    model
        .iter()
        .filter(|(key, _)| (lower, upper).contains(*key))
        .map(|(key, value)| (*key, value.clone()))
        .collect()
}

/// A scan of the interval that takes each entry from the end `rng` chooses, put back in key order.
fn alternating<S: PageSource>(
    tx: &S,
    root: u64,
    len: usize,
    lower: Bound<u64>,
    upper: Bound<u64>,
    rng: &mut Rng,
) -> Entries {
    let mut range = FixedRange::new(tx, root, len, lower, upper);
    let (mut front, mut back) = (Vec::new(), Vec::new());
    loop {
        if rng.below(2) == 0 {
            match range.next() {
                Some(entry) => front.push(owned(entry)),
                None => break,
            }
        } else {
            match range.next_back() {
                Some(entry) => back.push(owned(entry)),
                None => break,
            }
        }
    }
    assert!(
        range.next().is_none() && range.next_back().is_none(),
        "fused"
    );
    back.reverse();
    front.extend(back);
    front
}

/// Checks the structure of the subtree at `page`: the layout check, levels, key order within the bounds its parents
/// give, the tree's length in every leaf and no empty node but the root. Returns its level.
fn validate<S: PageSource>(
    tx: &S,
    page: u64,
    len: usize,
    low: Option<u64>,
    high: Option<u64>,
    root: bool,
) -> u8 {
    let node = tx.page(page).unwrap();
    assert!(fixed::is_valid(node), "page {page}: layout");
    let count = fixed::count(node);
    let keys: Vec<u64> = (0..count).map(|i| fixed::key(node, i)).collect();
    assert!(keys.windows(2).all(|w| w[0] < w[1]), "page {page}: order");
    assert!(
        keys.iter()
            .all(|&k| low.is_none_or(|l| k >= l) && high.is_none_or(|h| k < h)),
        "page {page}: bounds"
    );
    if fixed::is_leaf(node) {
        assert_eq!(fixed::value_len(node), len, "page {page}: length");
        assert!(
            root || count > 0,
            "page {page}: an empty leaf that is not the root"
        );
        return 0;
    }
    let level = node::level(node);
    for index in 0..=count {
        let low = if index == 0 {
            low
        } else {
            Some(keys[index - 1])
        };
        let high = if index == count {
            high
        } else {
            Some(keys[index])
        };
        let child = validate(tx, fixed::child(node, index), len, low, high, false);
        assert_eq!(child + 1, level, "page {page}: child {index}");
    }
    level
}

/// Checks every key of the model, a few others, the entries both ways, a few scans and the structure.
fn check<S: PageSource>(tx: &S, root: u64, len: usize, model: &Model, rng: &mut Rng, label: &str) {
    for (&key, value) in model {
        assert_eq!(
            get(tx, root, len, key).unwrap(),
            Some(&value[..]),
            "{label}: {key}"
        );
    }
    for _ in 0..20 {
        let key = rng.key();
        assert_eq!(
            get(tx, root, len, key).unwrap(),
            model.get(&key).map(|v| &v[..]),
            "{label}: {key}"
        );
    }
    let all = expected(model, Unbounded, Unbounded);
    assert!(entries(tx, root, len) == all, "{label}: entries differ");
    let backwards: Entries = FixedRange::new(tx, root, len, Unbounded, Unbounded)
        .rev()
        .map(owned)
        .collect();
    assert!(
        backwards.into_iter().rev().eq(all),
        "{label}: backwards differ"
    );
    for _ in 0..10 {
        let (lower, upper) = (rng.bound(model), rng.bound(model));
        assert!(
            alternating(tx, root, len, lower, upper, rng) == expected(model, lower, upper),
            "{label}: scan {lower:?} {upper:?}"
        );
    }
    validate(tx, root, len, None, None, true);
}

/// An interval of a few keys of `model` around a random one, with bounds of every kind.
fn narrow(rng: &mut Rng, model: &Model) -> (Bound<u64>, Bound<u64>) {
    let keys: Vec<u64> = model.keys().copied().collect();
    if keys.is_empty() {
        return (rng.bound(model), rng.bound(model));
    }
    let start = rng.below(keys.len() as u64) as usize;
    let end = (start + rng.below(12) as usize).min(keys.len() - 1);
    let side = |rng: &mut Rng, key: u64| match rng.below(4) {
        0 => Excluded(key),
        1 => Included(key.wrapping_add(1)),
        _ => Included(key),
    };
    // A side is open only at the first or the last key, so that the interval stays narrow.
    let lower = if start == 0 && rng.below(2) == 0 {
        Unbounded
    } else {
        side(rng, keys[start])
    };
    let upper = if end == keys.len() - 1 && rng.below(2) == 0 {
        Unbounded
    } else {
        side(rng, keys[end])
    };
    (lower, upper)
}

/// Applies a random operation to the tree and the model, checking what it reports; `removes` is the percentage of
/// removals, one in five of them a range removal.
fn random_op(
    tx: &mut WriteTransaction<'_>,
    root: &mut u64,
    len: usize,
    model: &mut Model,
    rng: &mut Rng,
    removes: u64,
) {
    let key = rng.key_of(model);
    let present = model.contains_key(&key);
    let dice = rng.below(100);
    if dice < removes / 5 {
        let (lower, upper) = narrow(rng, model);
        let gone = expected(model, lower, upper);
        let update = remove_range(tx, *root, len, lower, upper).unwrap();
        assert_eq!(update.removed, gone.len() as u64, "{lower:?} {upper:?}");
        for (key, _) in gone {
            model.remove(&key);
        }
        *root = update.root;
        return;
    }
    let update = if dice < removes {
        let update = remove(tx, *root, len, key).unwrap();
        assert_eq!(model.remove(&key).is_some(), update.count_changed);
        update
    } else if dice < removes + (100 - removes) / 4 {
        let value = rng.value(len);
        let update = try_add(tx, *root, len, key, &value).unwrap();
        assert_eq!(update.count_changed, !present);
        model.entry(key).or_insert(value);
        update
    } else {
        let value = rng.value(len);
        let update = set(tx, *root, len, key, &value).unwrap();
        assert_eq!(update.count_changed, !present);
        model.insert(key, value);
        update
    };
    *root = update.root;
}

/// A seeded sequence of set, try_add, remove, remove_range and scans against `BTreeMap`, at several lengths: the
/// tree grows to several levels and shrinks back to one empty leaf, every node passes the layout check, and every
/// key and scan matches the model after each batch, in the write and in a read of the commit.
#[test]
fn random_operations_match_a_btreemap_while_the_tree_grows_and_shrinks() {
    for (len, target, levels) in [(2048, 1800, 3), (100, 1000, 2), (8, 1200, 2), (0, 1500, 2)] {
        let engine = temporary();
        let mut rng = Rng(0x0066_6978_6564 + len as u64);
        let mut model = Model::new();
        let mut root = new_tree(&engine, len);
        let mut heights = vec![1];
        for (phase, removes) in [("grow", 10), ("shrink", 85), ("empty", 0)] {
            let mut batch = 0;
            let unfinished = |model: &Model| match phase {
                "grow" => model.len() < target,
                "shrink" => model.len() > 40,
                _ => !model.is_empty(),
            };
            while unfinished(&model) {
                batch += 1;
                let label = format!("length {len}, {phase} batch {batch}");
                let mut tx = engine.write(None).unwrap();
                if phase == "empty" {
                    // Half of what is left by a range removal, the rest key by key.
                    let middle = *model.keys().nth(model.len() / 2).unwrap();
                    let update =
                        remove_range(&mut tx, root, len, Unbounded, Excluded(middle)).unwrap();
                    assert_eq!(update.removed, model.len() as u64 / 2, "{label}");
                    root = update.root;
                    for key in model.split_off(&middle).into_keys() {
                        let update = remove(&mut tx, root, len, key).unwrap();
                        assert!(update.count_changed, "{label}: {key}");
                        root = update.root;
                    }
                    model.clear();
                } else {
                    for _ in 0..250 {
                        random_op(&mut tx, &mut root, len, &mut model, &mut rng, removes);
                    }
                }
                check(&tx, root, len, &model, &mut rng, &label);
                tx.commit().unwrap();
                let read = engine.read().unwrap();
                check(&read, root, len, &model, &mut rng, &label);
                heights.push(height(&read, root, len).unwrap());
            }
        }
        let highest = *heights.iter().max().unwrap();
        assert!(
            highest >= levels,
            "length {len}: the tree reached {highest} levels"
        );
        assert!(
            heights.windows(2).any(|w| w[1] < w[0]),
            "length {len}: the tree lost levels"
        );
        let read = engine.read().unwrap();
        let node = read.page(root).unwrap();
        assert!(
            fixed::is_leaf(node) && fixed::count(node) == 0,
            "length {len}: one empty leaf"
        );
    }
}

/// S1 (AC-X1, engine part): the extreme keys and random ones, stored in random order, come out in numeric order
/// forwards and in the reverse order backwards, in the write and in a later read.
#[test]
fn keys_come_out_in_numeric_order_both_ways() {
    for (len, count, levels) in [(8, 1500, 2), (2048, 1500, 3)] {
        let mut rng = Rng(len as u64);
        let mut keys = EXTREMES.to_vec();
        keys.extend((0..count).map(|_| rng.next()));
        rng.shuffle(&mut keys);
        let engine = temporary();
        let (root, _) = tree_with(&engine, len, &keys);
        keys.sort_unstable();
        keys.dedup();
        let reversed: Vec<u64> = keys.iter().rev().copied().collect();
        let mut write = engine.write(None).unwrap();
        let read = engine.read().unwrap();
        assert!(height(&read, root, len).unwrap() >= levels, "length {len}");
        for (label, tx) in [
            ("write", &write as &dyn PageSource),
            ("read", &read as &dyn PageSource),
        ] {
            let forwards: Vec<u64> = FixedRange::new(tx, root, len, Unbounded, Unbounded)
                .map(|entry| entry.unwrap().0)
                .collect();
            assert!(forwards == keys, "{label}, length {len}: forwards");
            let backwards: Vec<u64> = FixedRange::new(tx, root, len, Unbounded, Unbounded)
                .rev()
                .map(|entry| entry.unwrap().0)
                .collect();
            assert!(backwards == reversed, "{label}, length {len}: backwards");
            for &key in &EXTREMES {
                assert_eq!(
                    get(tx, root, len, key).unwrap(),
                    Some(&value_of(key, len)[..])
                );
            }
        }
        // A key added to the write comes out in its place in the write only.
        let new = keys[keys.len() / 2] + 1;
        set(&mut write, root, len, new, &value_of(new, len)).unwrap();
        assert!(
            entries(&write, root, len)
                .windows(2)
                .all(|w| w[0].0 < w[1].0)
        );
        assert_eq!(entries(&write, root, len).len(), keys.len() + 1);
        assert_eq!(entries(&read, root, len).len(), keys.len());
    }
}

/// S5 (AC-X5, engine part): set creates or overwrites, try_add adds the first time only and changes nothing the
/// second, get returns the value or nothing, remove removes the first time only, and the flags they report keep the
/// number of entries, at the lengths 0, 1, 8 and 2,048, until the tree has several levels.
#[test]
fn set_try_add_get_and_remove_at_every_length() {
    for len in [0, 1, 8, MAX_LEN] {
        let engine = temporary();
        let mut root = new_tree(&engine, len);
        let mut tx = engine.write(None).unwrap();
        let mut count = 0u64;
        let mut model = Model::new();
        let mut rng = Rng(len as u64 + 5);
        let mut keys = EXTREMES.to_vec();
        keys.extend((0..300).map(|_| rng.next()));
        for &key in &keys {
            assert_eq!(get(&tx, root, len, key).unwrap(), None);
            let (first, second, third) = (rng.value(len), rng.value(len), rng.value(len));
            let update = set(&mut tx, root, len, key, &first).unwrap();
            assert!(update.count_changed, "length {len}: set a new key");
            count += 1;
            root = update.root;
            assert_eq!(get(&tx, root, len, key).unwrap(), Some(&first[..]));
            let update = set(&mut tx, root, len, key, &second).unwrap();
            assert!(!update.count_changed, "length {len}: overwrite");
            root = update.root;
            assert_eq!(get(&tx, root, len, key).unwrap(), Some(&second[..]));
            let update = try_add(&mut tx, root, len, key, &third).unwrap();
            assert_eq!(
                update,
                crate::Update {
                    root,
                    count_changed: false
                }
            );
            assert_eq!(get(&tx, root, len, key).unwrap(), Some(&second[..]));
            model.insert(key, second);
        }
        // Adding new keys, then removing them, twice each.
        for key in [5u64, 6, 7] {
            let value = rng.value(len);
            let update = try_add(&mut tx, root, len, key, &value).unwrap();
            assert!(update.count_changed);
            count += 1;
            root = update.root;
            assert!(
                !try_add(&mut tx, root, len, key, &rng.value(len))
                    .unwrap()
                    .count_changed
            );
            assert_eq!(get(&tx, root, len, key).unwrap(), Some(&value[..]));
            model.insert(key, value);
        }
        assert_eq!(count, model.len() as u64);
        let mut rng_check = Rng(9);
        check(&tx, root, len, &model, &mut rng_check, "filled");
        if len == MAX_LEN {
            assert!(height(&tx, root, len).unwrap() >= 2);
        }
        if len == 0 {
            assert_eq!(get(&tx, root, len, 0).unwrap(), Some(&b""[..]), "present");
            assert_eq!(get(&tx, root, len, 4).unwrap(), None, "absent");
        }
        tx.commit().unwrap();
        let read = engine.read().unwrap();
        check(&read, root, len, &model, &mut rng_check, "committed");
        drop(read);

        let mut tx = engine.write(None).unwrap();
        for &key in model.keys() {
            let update = remove(&mut tx, root, len, key).unwrap();
            assert!(update.count_changed, "length {len}: {key}");
            count -= 1;
            root = update.root;
            assert_eq!(get(&tx, root, len, key).unwrap(), None);
            assert!(!remove(&mut tx, root, len, key).unwrap().count_changed);
        }
        assert_eq!(count, 0);
        assert_eq!(height(&tx, root, len).unwrap(), 1);
        assert!(entries(&tx, root, len).is_empty());
        tx.commit().unwrap();
    }
}

/// Keys every ten from 0 to `10 * count`, and `u64::MAX`: bounds every five are keys and keys that are not.
fn tens(count: u64) -> Vec<u64> {
    let mut keys: Vec<u64> = (0..=count).map(|i| i * 10).collect();
    keys.push(u64::MAX);
    keys
}

/// S6 (AC-X6, engine part): scans with included, excluded and open bounds, `0` and `u64::MAX` among them and bounds
/// that are not keys, give exactly the entries between them, forwards, backwards and alternating both ends.
#[test]
fn scans_give_the_entries_between_their_bounds_from_both_ends() {
    for (len, count) in [(8, 1200), (MAX_LEN, 300)] {
        let engine = temporary();
        let mut keys = tens(count);
        let mut rng = Rng(3);
        rng.shuffle(&mut keys);
        let (root, model) = tree_with(&engine, len, &keys);
        let read = engine.read().unwrap();
        assert!(height(&read, root, len).unwrap() >= 2);
        let last = count * 10;
        let points = [
            0,
            1,
            5,
            10,
            15,
            last / 2,
            last / 2 + 5,
            last,
            last + 5,
            u64::MAX - 1,
            u64::MAX,
        ];
        let mut bounds = vec![Unbounded];
        for point in points {
            bounds.extend([Included(point), Excluded(point)]);
        }
        for &lower in &bounds {
            for &upper in &bounds {
                let want = expected(&model, lower, upper);
                let label = format!("length {len}: {lower:?} {upper:?}");
                let forwards: Entries = FixedRange::new(&read, root, len, lower, upper)
                    .map(owned)
                    .collect();
                assert!(forwards == want, "{label}: forwards");
                let backwards: Entries = FixedRange::new(&read, root, len, lower, upper)
                    .rev()
                    .map(owned)
                    .collect();
                assert!(
                    backwards.into_iter().rev().eq(want.clone()),
                    "{label}: backwards"
                );
                assert!(
                    alternating(&read, root, len, lower, upper, &mut rng) == want,
                    "{label}: alternating"
                );
            }
        }
    }
}

/// The cases of X7 where no integer lies between the bounds, each with a damaged root.
const EMPTY: [(Bound<u64>, Bound<u64>); 9] = [
    (Included(8), Included(5)),
    (Excluded(8), Excluded(5)),
    (Included(5), Excluded(5)),
    (Excluded(5), Included(5)),
    (Excluded(5), Excluded(5)),
    (Excluded(5), Excluded(6)),
    (Excluded(u64::MAX), Unbounded),
    (Excluded(u64::MAX), Included(u64::MAX)),
    (Unbounded, Excluded(0)),
];

/// D4: an interval is normalised to inclusive bounds, empty exactly in the cases of X7; their neighbours are not.
#[test]
fn intervals_are_empty_exactly_in_the_cases_of_x7() {
    for (lower, upper) in EMPTY {
        assert_eq!(interval(lower, upper), None, "{lower:?} {upper:?}");
    }
    assert_eq!(interval(Included(0), Excluded(0)), None);
    let full = [
        ((Unbounded, Unbounded), (0, u64::MAX)),
        ((Included(5), Included(8)), (5, 8)),
        ((Included(5), Included(5)), (5, 5)),
        ((Excluded(5), Excluded(7)), (6, 6)),
        ((Excluded(5), Included(6)), (6, 6)),
        ((Included(5), Excluded(6)), (5, 5)),
        ((Excluded(u64::MAX - 1), Unbounded), (u64::MAX, u64::MAX)),
        ((Included(u64::MAX), Unbounded), (u64::MAX, u64::MAX)),
        ((Unbounded, Excluded(1)), (0, 0)),
        ((Unbounded, Included(0)), (0, 0)),
        ((Included(0), Included(u64::MAX)), (0, u64::MAX)),
    ];
    for ((lower, upper), bounds) in full {
        assert_eq!(interval(lower, upper), Some(bounds), "{lower:?} {upper:?}");
    }
}

/// Writes `value` as the little-endian `u16` at byte `at` of a node.
fn put16(node: &mut [u8], at: usize, value: usize) {
    node[at..at + 2].copy_from_slice(&(value as u16).to_le_bytes());
}

#[track_caller]
fn damaged_page<T: std::fmt::Debug>(result: Result<T>) -> u64 {
    match result {
        Err(Error::Damaged {
            page: Some(page), ..
        }) => page,
        other => panic!("expected damage with a page, got {other:?}"),
    }
}

/// S6 (AC-X6, engine part): every empty interval of X7 gives an empty scan and a range removal of nothing without
/// reading a page, so a damaged root does not fail them; any other interval reads the root and fails.
#[test]
fn empty_intervals_read_no_page() {
    let engine = temporary();
    let (root, _) = tree_with(&engine, 8, &tens(50));
    let mut tx = engine.write(None).unwrap();
    tx.page_mut(root).unwrap()[0] = 9;
    tx.commit().unwrap();
    for (lower, upper) in EMPTY.into_iter().chain([(Included(0), Excluded(0))]) {
        let read = engine.read().unwrap();
        let mut scan = FixedRange::new(&read, root, 8, lower, upper);
        assert!(scan.next().is_none() && scan.next_back().is_none());
        assert!(
            FixedRange::new(&read, root, 8, lower, upper)
                .next_back()
                .is_none()
        );
        drop(read);
        let mut tx = engine.write(None).unwrap();
        assert_eq!(
            remove_range(&mut tx, root, 8, lower, upper).unwrap(),
            RangeUpdate { root, removed: 0 }
        );
        tx.commit().unwrap();
    }
    let read = engine.read().unwrap();
    let mut scan = FixedRange::new(&read, root, 8, Included(5), Included(5));
    assert_eq!(damaged_page(scan.next().unwrap()), root);
    assert!(
        scan.next().is_none() && scan.next_back().is_none(),
        "stopped"
    );
    drop(read);
    let mut tx = engine.write(None).unwrap();
    assert_eq!(
        damaged_page(remove_range(&mut tx, root, 8, Included(5), Included(5))),
        root
    );
}

/// S6 (AC-X6, engine part): range removals, with bounds of every kind, inside a leaf, across several levels and of
/// the whole tree, report how many entries they removed and leave the others; one that finds nothing removes 0.
#[test]
fn range_removals_report_how_many_entries_they_removed() {
    for (len, count) in [(8, 1500), (MAX_LEN, 1800)] {
        let engine = temporary();
        let (mut root, mut model) = tree_with(&engine, len, &tens(count));
        let mut tx = engine.write(None).unwrap();
        if len == MAX_LEN {
            assert!(height(&tx, root, len).unwrap() >= 3);
        }
        let last = count * 10;
        let intervals = [
            (Included(101), Included(109)),
            (Included(100), Excluded(200)),
            (Excluded(1000), Included(2000)),
            (Unbounded, Included(55)),
            (Included(u64::MAX), Unbounded),
            (Excluded(last / 3), Excluded(2 * last / 3 + 5)),
            (Included(last - 1000), Included(last + 5)),
            (Unbounded, Unbounded),
        ];
        let mut rng = Rng(11);
        for (lower, upper) in intervals {
            let label = format!("length {len}: {lower:?} {upper:?}");
            let gone = expected(&model, lower, upper);
            let update = remove_range(&mut tx, root, len, lower, upper).unwrap();
            assert_eq!(update.removed, gone.len() as u64, "{label}");
            root = update.root;
            for (key, _) in gone {
                model.remove(&key);
            }
            check(&tx, root, len, &model, &mut rng, &label);
        }
        assert!(model.is_empty());
        assert_eq!(height(&tx, root, len).unwrap(), 1);
        tx.commit().unwrap();
    }
}

/// Pages in use: the pages of the store minus the free pages of its information.
fn in_use(engine: &Engine) -> u64 {
    let info = engine.info();
    info.page_count - info.free_pages
}

/// Freeing a whole tree of several levels gives back every page it used, on disk and in a temporary store.
#[test]
fn freeing_a_tree_returns_every_page() {
    let tmp = tempfile::tempdir().unwrap();
    for engine in [
        Engine::open(tmp.path(), &OpenOptions::default()).unwrap(),
        temporary(),
    ] {
        let before = in_use(&engine);
        let mut rng = Rng(7);
        let mut tx = engine.write(None).unwrap();
        let mut root = create(&mut tx, MAX_LEN).unwrap();
        for _ in 0..1500 {
            root = set(&mut tx, root, MAX_LEN, rng.next(), &rng.value(MAX_LEN))
                .unwrap()
                .root;
        }
        assert!(height(&tx, root, MAX_LEN).unwrap() >= 3);
        tx.commit().unwrap();
        let filled = in_use(&engine);
        assert!(filled > before + 500, "{before} {filled}");
        let mut tx = engine.write(None).unwrap();
        free_all(&mut tx, root, MAX_LEN).unwrap();
        tx.commit().unwrap();
        assert_eq!(in_use(&engine), before);
    }
}

/// A fixed node with `count` entries of a length `len`, or a branch when `level` is above 0.
fn fixed_node(level: u8, len: usize, count: usize) -> Vec<u8> {
    let entry = if level == 0 { 8 + len } else { 16 };
    let mut entries = vec![0; count * entry];
    for (i, chunk) in entries.chunks_mut(entry).enumerate() {
        chunk[..8].copy_from_slice(&(i as u64).to_le_bytes());
    }
    let mut node = vec![0; PAYLOAD_SIZE];
    fixed::fill(
        &mut node,
        level,
        len,
        if level == 0 { 0 } else { 77 },
        &entries,
    );
    node
}

/// D2: the layout check accepts consistent fixed nodes up to their capacity and rejects any kind, level, count or
/// header field that would let an accessor leave the page.
#[test]
fn the_layout_check_keeps_every_accessor_within_the_page() {
    let leaf_capacity = |len: usize| (PAYLOAD_SIZE - 16) / (8 + len);
    assert_eq!(
        (
            leaf_capacity(MAX_LEN),
            leaf_capacity(0),
            (PAYLOAD_SIZE - 16) / 16
        ),
        (3, 1020, 510)
    );
    for len in [0, 1, 8, MAX_LEN] {
        let full = fixed_node(0, len, leaf_capacity(len));
        assert!(fixed::is_valid(&full), "length {len}");
        assert!(fixed::is_valid(&fixed_node(0, len, 0)));
        let mut over = full.clone();
        put16(&mut over, 2, leaf_capacity(len) + 1);
        assert!(!fixed::is_valid(&over), "length {len}: count");
    }
    assert!(fixed::is_valid(&fixed_node(1, 0, 510)));
    assert!(fixed::is_valid(&fixed_node(254, 0, 0)));
    let mut over = fixed_node(1, 0, 510);
    put16(&mut over, 2, 511);
    assert!(!fixed::is_valid(&over), "a branch over its capacity");

    type Corrupt = fn(&mut [u8]);
    let leaf: &[(&str, Corrupt)] = &[
        ("a slotted leaf kind", |n| n[0] = 1),
        ("a slotted branch kind", |n| n[0] = 2),
        ("an unknown kind", |n| n[0] = 5),
        ("a leaf above level 0", |n| n[1] = 1),
        ("a length over the maximum", |n| put16(n, 4, MAX_LEN + 1)),
        ("a length that leaves its entries past the page", |n| {
            put16(n, 4, 100)
        }),
        ("a nonzero byte 6", |n| n[6] = 1),
        ("a nonzero byte 7", |n| n[7] = 1),
    ];
    for (label, corrupt) in leaf {
        let mut node = fixed_node(0, 8, 300);
        corrupt(&mut node);
        assert!(!fixed::is_valid(&node), "{label}");
    }
    let branch: &[(&str, Corrupt)] = &[
        ("a branch at level 0", |n| n[1] = 0),
        ("a branch at level 255", |n| n[1] = 255),
        ("a branch with a length", |n| put16(n, 4, 8)),
        ("a nonzero byte 6", |n| n[6] = 1),
    ];
    for (label, corrupt) in branch {
        let mut node = fixed_node(2, 0, 30);
        corrupt(&mut node);
        assert!(!fixed::is_valid(&node), "{label}");
    }
}

/// A page source with one page, served as page 1 with an integrity code, that remembers the layout check as a store
/// does: it lets a test put a node of either format in the shared memory of checked nodes (X14).
struct OnePage {
    page: Vec<u8>,
    remembered: Cell<bool>,
}

impl OnePage {
    fn new(page: Vec<u8>, remembered: bool) -> Self {
        OnePage {
            page,
            remembered: Cell::new(remembered),
        }
    }
}

impl PageSource for OnePage {
    fn page(&self, _: u64) -> Result<&[u8]> {
        Ok(&self.page)
    }

    fn page_and_check(&self, _: u64) -> Result<(&[u8], krummi_tx::PageCheck)> {
        Ok((&self.page, krummi_tx::PageCheck::Verified(42)))
    }

    fn is_checked(&self, _: u64, _: u64) -> bool {
        self.remembered.get()
    }

    fn remember_checked(&self, _: u64, _: u64) {
        self.remembered.set(true);
    }

    fn run(&self, _: u64, _: u64) -> Result<&[u8]> {
        unreachable!("no runs")
    }

    fn slot(&self, _: usize) -> Result<u64> {
        unreachable!("no slots")
    }

    fn may_hold_data(&self, page_number: u64) -> bool {
        page_number == 1
    }

    fn may_hold_run(&self, _: u64, _: u64) -> bool {
        false
    }

    fn damaged(&self, page_number: u64) -> Error {
        Error::Damaged {
            folder: None,
            page: Some(page_number),
        }
    }

    fn fail(&self, error: Error) -> Error {
        error
    }
}

/// D2 and X14: a fixed node is remembered once it passes its check, like a slotted one; a remembered node of one
/// format is damage for the reader of the other, because every read checks the kind family after the remembered
/// check; and a remembered fixed leaf of another length is damage for the tree's length.
#[test]
fn the_remembered_check_is_shared_but_each_format_rejects_the_other() {
    let fixed_leaf = fixed_node(0, 8, 3);
    let source = OnePage::new(fixed_leaf.clone(), false);
    assert_eq!(
        get(&source, 1, 8, 2).unwrap(),
        Some(&fixed_leaf[16 + 2 * 16 + 8..16 + 3 * 16])
    );
    assert!(
        source.remembered.get(),
        "a fixed node passes and is remembered"
    );
    assert_eq!(damaged_page(get(&source, 1, 16, 2)), 1, "another length");
    assert_eq!(
        damaged_page(crate::get(&source, 1, b"key")),
        1,
        "slotted reader"
    );
    assert_eq!(damaged_page(crate::height(&source, 1)), 1);

    let mut slotted_leaf = vec![0; PAYLOAD_SIZE];
    node::init(&mut slotted_leaf, 0, 0);
    let source = OnePage::new(slotted_leaf, false);
    assert_eq!(crate::get(&source, 1, b"key").unwrap(), None);
    assert!(
        source.remembered.get(),
        "a slotted node passes and is remembered"
    );
    assert_eq!(damaged_page(get(&source, 1, 8, 2)), 1, "fixed reader");
    assert_eq!(damaged_page(height(&source, 1, 8)), 1);
    assert!(
        FixedRange::new(&source, 1, 8, Unbounded, Unbounded)
            .next()
            .unwrap()
            .is_err()
    );

    // A fixed leaf whose count still fits passes and is remembered; one whose count does not is damage and is not.
    let mut broken = fixed_leaf;
    put16(&mut broken, 2, 2);
    let source = OnePage::new(broken.clone(), false);
    assert_eq!(get(&source, 1, 8, 2).unwrap(), None);
    put16(&mut broken, 2, 4000);
    let source = OnePage::new(broken, false);
    assert_eq!(damaged_page(get(&source, 1, 8, 2)), 1);
    assert!(
        !source.remembered.get(),
        "a node that fails is not remembered"
    );
}

/// G6 in the engine: a fixed leaf of a tree of several levels whose count or length is inconsistent makes every
/// operation that needs it fail with the damage class and its page, again on each repetition; keys elsewhere are
/// still read.
#[test]
fn an_inconsistent_fixed_node_is_damage_and_not_a_panic() {
    type Corrupt = fn(&mut [u8]);
    let cases: &[(&str, Corrupt)] = &[
        ("a count over the capacity", |n| put16(n, 2, 0xffff)),
        ("another length", |n| put16(n, 4, 16)),
        ("a slotted kind", |n| n[0] = 1),
        ("a level", |n| n[1] = 1),
    ];
    for (label, corrupt) in cases {
        let engine = temporary();
        let keys = tens(200);
        let (root, _) = tree_with(&engine, MAX_LEN, &keys);
        let read = engine.read().unwrap();
        assert!(height(&read, root, MAX_LEN).unwrap() >= 2);
        let page = leaf_of(&read, root, MAX_LEN, 1000).unwrap();
        let elsewhere = keys
            .iter()
            .copied()
            .find(|&key| leaf_of(&read, root, MAX_LEN, key).unwrap() != page)
            .unwrap();
        drop(read);
        let mut tx = engine.write(None).unwrap();
        corrupt(tx.page_mut(page).unwrap());
        tx.commit().unwrap();

        let read = engine.read().unwrap();
        for _ in 0..2 {
            assert_eq!(
                damaged_page(get(&read, root, MAX_LEN, 1000)),
                page,
                "{label}"
            );
        }
        assert!(get(&read, root, MAX_LEN, elsewhere).unwrap().is_some());
        let mut scan = FixedRange::new(&read, root, MAX_LEN, Unbounded, Unbounded);
        let failure = scan.find_map(|entry| entry.err()).unwrap();
        assert_eq!(damaged_page(Err::<(), _>(failure)), page, "{label}");
        assert!(
            scan.next().is_none() && scan.next_back().is_none(),
            "stopped"
        );
        drop(read);
        let value = vec![1; MAX_LEN];
        for op in 0..5 {
            let mut tx = engine.write(None).unwrap();
            let result = match op {
                0 => set(&mut tx, root, MAX_LEN, 1000, &value).map(|_| ()),
                1 => try_add(&mut tx, root, MAX_LEN, 1000, &value).map(|_| ()),
                2 => remove(&mut tx, root, MAX_LEN, 1001).map(|_| ()),
                3 => {
                    remove_range(&mut tx, root, MAX_LEN, Included(995), Included(1005)).map(|_| ())
                }
                _ => free_all(&mut tx, root, MAX_LEN),
            };
            assert_eq!(damaged_page(result), page, "{label}: operation {op}");
            assert_eq!(damaged_page(tx.commit()), page, "{label}: T11");
        }
    }
}
