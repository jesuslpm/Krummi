//! Internal tests of the trees: a randomized model test against `BTreeMap`, the extremes of A11, freeing a whole
//! tree and the snapshot of an older read. Deterministic: the random sequences come from fixed seeds.

use std::collections::BTreeMap;

use krummi_tx::{Engine, OpenOptions, WriteTransaction};

use super::*;

type Model = BTreeMap<Vec<u8>, Vec<u8>>;

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

    /// A key over a few bytes that include the extremes, so that keys repeat and many are prefixes of others; a
    /// quarter of them are extended to a length fixed by their start, up to [`MAX_KEY`] bytes.
    fn key(&mut self) -> Vec<u8> {
        const BYTES: [u8; 5] = [0x00, 0x01, 0x7f, 0x80, 0xff];
        let len = 1 + self.below(5) as usize;
        let mut key: Vec<u8> = (0..len).map(|_| BYTES[self.below(5) as usize]).collect();
        if self.below(4) == 0 {
            let seed = key.iter().fold(7usize, |h, &b| h * 31 + usize::from(b));
            key.resize(len + 1 + seed % (MAX_KEY - len), key[0] ^ 0x55);
        }
        key
    }

    fn value(&mut self) -> Vec<u8> {
        let len = self.below(MAX_VALUE as u64 + 1) as usize;
        let byte = self.next() as u8;
        (0..len).map(|i| byte.wrapping_add(i as u8)).collect()
    }
}

fn temporary() -> Engine {
    Engine::open_temporary(&OpenOptions::default()).unwrap()
}

fn new_tree(engine: &Engine) -> u64 {
    let mut tx = engine.write(None).unwrap();
    let root = create(&mut tx).unwrap();
    tx.commit().unwrap();
    root
}

fn height<S: PageSource>(tx: &S, root: u64) -> u8 {
    node::level(tx.page(root).unwrap()) + 1
}

fn entries<S: PageSource>(tx: &S, root: u64) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut out = Vec::new();
    for_each(tx, root, &mut |k, v| out.push((k.to_vec(), v.to_vec()))).unwrap();
    out
}

/// Checks the structure of the subtree at `page`: levels, key order within the bounds its parents give, no empty
/// node but the root, and every node's bytes within a page. Returns its level.
fn validate<S: PageSource>(
    tx: &S,
    page: u64,
    low: Option<&[u8]>,
    high: Option<&[u8]>,
    root: bool,
) -> u8 {
    let node = tx.page(page).unwrap();
    let count = node::count(node);
    assert!(node::used(node) <= PAYLOAD_SIZE);
    let keys: Vec<&[u8]> = (0..count).map(|i| node::key(node, i)).collect();
    assert!(keys.windows(2).all(|w| w[0] < w[1]), "page {page}: order");
    assert!(
        keys.iter()
            .all(|k| low.is_none_or(|l| *k >= l) && high.is_none_or(|h| *k < h))
    );
    if node::is_leaf(node) {
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
        let child = validate(tx, node::child(node, index), low, high, false);
        assert_eq!(child + 1, level, "page {page}: child {index}");
    }
    level
}

/// Checks every key of the model, a few absent ones, the order of all entries and the structure.
fn check<S: PageSource>(tx: &S, root: u64, model: &Model, rng: &mut Rng, label: &str) {
    for (key, value) in model {
        assert_eq!(
            get(tx, root, key).unwrap(),
            Some(&value[..]),
            "{label}: {key:?}"
        );
    }
    for _ in 0..20 {
        let key = rng.key();
        assert_eq!(
            get(tx, root, &key).unwrap(),
            model.get(&key).map(|v| &v[..]),
            "{label}"
        );
    }
    let expected: Vec<(Vec<u8>, Vec<u8>)> =
        model.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    assert!(entries(tx, root) == expected, "{label}: entries differ");
    validate(tx, root, None, None, true);
}

/// Applies a random operation to the tree and the model, checking the flag it reports.
/// Half of the keys are drawn from the model, so that overwrites, failed adds and removals of present keys are
/// frequent; `removes` is the percentage of removals.
fn random_op(
    tx: &mut WriteTransaction<'_>,
    root: &mut u64,
    model: &mut Model,
    rng: &mut Rng,
    removes: u64,
) {
    let key = if !model.is_empty() && rng.below(2) == 0 {
        let index = rng.below(model.len() as u64) as usize;
        model.keys().nth(index).unwrap().clone()
    } else {
        rng.key()
    };
    let present = model.contains_key(&key);
    let dice = rng.below(100);
    let update = if dice < removes {
        let update = remove(tx, *root, &key).unwrap();
        assert_eq!(model.remove(&key).is_some(), update.count_changed);
        update
    } else if dice < removes + (100 - removes) / 4 {
        let value = rng.value();
        let update = try_add(tx, *root, &key, &value).unwrap();
        assert_eq!(update.count_changed, !present);
        model.entry(key).or_insert(value);
        update
    } else {
        let value = rng.value();
        let update = set(tx, *root, &key, &value).unwrap();
        assert_eq!(update.count_changed, !present);
        model.insert(key, value);
        update
    };
    *root = update.root;
}

/// A randomized sequence of set, try_add and remove against `BTreeMap`: the tree grows until it has several levels
/// and shrinks back to one leaf, and every key matches the model after each batch, in the write and in a read of
/// the commit.
#[test]
fn random_operations_match_a_btreemap_while_the_tree_grows_and_shrinks() {
    let engine = temporary();
    let mut rng = Rng(0x6b72_756d_6d69);
    let mut model = Model::new();
    let mut root = new_tree(&engine);
    let mut heights = vec![1];
    for (phase, removes) in [("grow", 10), ("shrink", 80), ("empty", 0)] {
        let mut batch = 0;
        let unfinished = |model: &Model| match phase {
            "grow" => model.len() < 900,
            "shrink" => model.len() > 30,
            _ => !model.is_empty(),
        };
        while unfinished(&model) {
            batch += 1;
            let label = format!("{phase} batch {batch}");
            let mut tx = engine.write(None).unwrap();
            if phase == "empty" {
                for key in std::mem::take(&mut model).keys() {
                    let update = remove(&mut tx, root, key).unwrap();
                    assert!(update.count_changed, "{label}: {key:?}");
                    root = update.root;
                }
            } else {
                for _ in 0..250 {
                    random_op(&mut tx, &mut root, &mut model, &mut rng, removes);
                }
            }
            check(&tx, root, &model, &mut rng, &label);
            tx.commit().unwrap();
            let read = engine.read().unwrap();
            check(&read, root, &model, &mut rng, &label);
            heights.push(height(&read, root));
        }
    }
    let highest = *heights.iter().max().unwrap();
    assert!(highest >= 3, "the tree reached {highest} levels");
    assert_eq!(heights.last(), Some(&1), "the tree shrank back to one leaf");
    assert!(
        heights.windows(2).any(|w| w[1] < w[0]),
        "the tree lost levels"
    );
    let read = engine.read().unwrap();
    assert!(entries(&read, root).is_empty());
}

/// The extremes of A11: 2,048-byte keys with 1,024-byte values, enough of them for several levels, empty values,
/// keys of every byte and keys that are prefixes of others, in the order of B5.
#[test]
fn extreme_sizes_and_byte_order() {
    let engine = temporary();
    let mut root = new_tree(&engine);
    let mut model = Model::new();
    let mut tx = engine.write(None).unwrap();
    for i in 0..60u8 {
        let mut key = vec![i.wrapping_mul(37); MAX_KEY];
        key[MAX_KEY - 1] = i;
        let value = vec![i; MAX_VALUE];
        root = set(&mut tx, root, &key, &value).unwrap().root;
        model.insert(key, value);
    }
    assert!(height(&tx, root) >= 3);
    for byte in 0..=255u8 {
        root = set(&mut tx, root, &[byte], b"").unwrap().root;
        model.insert(vec![byte], Vec::new());
    }
    for len in 1..=40 {
        for byte in [0x00, 0xff] {
            let key = vec![byte; len];
            let value = vec![len as u8; len];
            let update = try_add(&mut tx, root, &key, &value).unwrap();
            assert_eq!(update.count_changed, len > 1, "{key:?}");
            root = update.root;
            model.entry(key).or_insert(value);
        }
    }
    let mut rng = Rng(1);
    check(&tx, root, &model, &mut rng, "filled");
    assert_eq!(
        get(&tx, root, &[0x00]).unwrap(),
        Some(&b""[..]),
        "an empty value is not an absent key"
    );
    let keys: Vec<Vec<u8>> = entries(&tx, root).into_iter().map(|(k, _)| k).collect();
    assert_eq!(
        &keys[..3],
        [vec![0x00], vec![0x00, 0x00], vec![0x00, 0x00, 0x00]]
    );
    assert_eq!(keys.last().unwrap(), &vec![0xff; 40]);
    tx.commit().unwrap();
    let read = engine.read().unwrap();
    check(&read, root, &model, &mut rng, "committed");
    drop(read);

    let mut tx = engine.write(None).unwrap();
    for key in model.keys() {
        let update = remove(&mut tx, root, key).unwrap();
        assert!(update.count_changed);
        root = update.root;
        assert!(!remove(&mut tx, root, key).unwrap().count_changed);
    }
    assert_eq!(height(&tx, root), 1);
    assert!(entries(&tx, root).is_empty());
    tx.commit().unwrap();
}

/// Pages in use: the pages of the store minus the free pages of its information.
fn in_use(engine: &Engine) -> u64 {
    let info = engine.info();
    info.page_count - info.free_pages
}

/// Freeing a whole tree of several levels gives back every page it used: the pages in use are those before it
/// was created, on disk and in a temporary store.
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
        let mut root = create(&mut tx).unwrap();
        for _ in 0..800 {
            root = set(&mut tx, root, &rng.key(), &rng.value()).unwrap().root;
        }
        assert!(height(&tx, root) >= 3);
        tx.commit().unwrap();
        let filled = in_use(&engine);
        assert!(filled > before + 50, "{before} {filled}");
        let mut tx = engine.write(None).unwrap();
        free_all(&mut tx, root).unwrap();
        tx.commit().unwrap();
        assert_eq!(in_use(&engine), before);
    }
}

/// A read opened before a write that splits and merges pages until the height changes keeps seeing its snapshot,
/// before and after the commit; a new read sees the write (T14, G3).
#[test]
fn an_older_read_keeps_its_snapshot_across_splits_and_merges() {
    let engine = temporary();
    let mut rng = Rng(42);
    let mut model = Model::new();
    let mut root = new_tree(&engine);
    let mut tx = engine.write(None).unwrap();
    for _ in 0..300 {
        random_op(&mut tx, &mut root, &mut model, &mut rng, 0);
    }
    tx.commit().unwrap();
    let snapshot_root = root;
    let snapshot = model.clone();
    let read = engine.read().unwrap();
    let start = height(&read, root);

    let mut tx = engine.write(None).unwrap();
    for _ in 0..1500 {
        random_op(&mut tx, &mut root, &mut model, &mut rng, 0);
    }
    let grown = height(&tx, root);
    let keys: Vec<Vec<u8>> = model.keys().cloned().collect();
    for key in keys.iter().skip(5) {
        root = remove(&mut tx, root, key).unwrap().root;
        model.remove(key);
    }
    let shrunk = height(&tx, root);
    assert!(grown > start && shrunk < grown, "{start} {grown} {shrunk}");
    check(
        &read,
        snapshot_root,
        &snapshot,
        &mut rng,
        "before the commit",
    );
    tx.commit().unwrap();
    check(
        &read,
        snapshot_root,
        &snapshot,
        &mut rng,
        "after the commit",
    );
    check(
        &engine.read().unwrap(),
        root,
        &model,
        &mut rng,
        "a new read",
    );
}
