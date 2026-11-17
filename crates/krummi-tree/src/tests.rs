//! Internal tests of the trees: a randomized model test against `BTreeMap`, with values inside and off their leaf,
//! the extremes of a leaf, freeing a whole tree, the snapshot of an older read, and inconsistent pages as damage.
//! Deterministic: the random sequences come from fixed seeds.

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

    /// A value of up to 1,024 bytes, or one time in eight of up to three pages, mostly off its leaf.
    fn value(&mut self) -> Vec<u8> {
        let max = if self.below(8) == 0 { 3 * 8192 } else { 1024 };
        let len = self.below(max + 1) as usize;
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

/// The extremes of a leaf: 2,048-byte keys with the longest values kept in their leaf, enough of them for several
/// levels, and one byte longer, off their leaf; empty values, keys of every byte and keys that are prefixes of
/// others, in the order of B5.
#[test]
fn extreme_sizes_and_byte_order() {
    let engine = temporary();
    let mut root = new_tree(&engine);
    let mut model = Model::new();
    let mut tx = engine.write(None).unwrap();
    for i in 0..60u8 {
        let mut key = vec![i.wrapping_mul(37); MAX_KEY];
        key[MAX_KEY - 1] = i;
        // Every other entry keeps its value in its leaf, at the threshold; the others are one byte over it.
        let value = vec![i; max_inline_value(MAX_KEY) + usize::from(i % 2)];
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

/// A tree of three levels or more whose keys are `key(i)` for `i` below `count`, in a new temporary store.
fn long_key(i: usize) -> Vec<u8> {
    let mut key = format!("{i:05}").into_bytes();
    key.resize(MAX_KEY, b'.');
    key
}

fn committed_tree(engine: &Engine, count: usize) -> u64 {
    let mut tx = engine.write(None).unwrap();
    let mut root = create(&mut tx).unwrap();
    for i in 0..count {
        root = set(&mut tx, root, &long_key(i), b"value").unwrap().root;
    }
    assert!(height(&tx, root) >= 3);
    tx.commit().unwrap();
    root
}

#[track_caller]
fn damaged_page<T: std::fmt::Debug>(result: Result<T>) -> u64 {
    match result {
        Err(krummi_tx::Error::Damaged {
            page: Some(page), ..
        }) => page,
        other => panic!("expected damage with a page, got {other:?}"),
    }
}

/// Writes `value` as the little-endian `u16` at byte `at` of a node.
fn put16(node: &mut [u8], at: usize, value: usize) {
    node[at..at + 2].copy_from_slice(&(value as u16).to_le_bytes());
}

fn get16(node: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes([node[at], node[at + 1]]))
}

/// R1-001 of the first review (G6, A9): a node whose page has a valid checksum but an inconsistent layout makes
/// every operation that reads it fail with the damage class and a page number, again on each repetition, without
/// panicking; a write that met it can no longer be committed (T11). The page is the inconsistent one, or the page
/// number a branch names when that number cannot be a page of a tree.
#[test]
fn an_inconsistent_node_layout_is_damage_and_not_a_panic() {
    /// Corrupts the node of a page and returns the page the damage names.
    type Corrupt = fn(&mut [u8], u64) -> u64;
    fn set_leftmost(node: &mut [u8], child: u64) -> u64 {
        node[8..16].copy_from_slice(&child.to_le_bytes());
        child
    }
    let leaf: &[(&str, Corrupt)] = &[
        ("an unknown kind", |n, page| {
            n[0] = 7;
            page
        }),
        ("a leaf above level 0", |n, page| {
            n[1] = 3;
            page
        }),
        ("more cells than fit", |n, page| {
            put16(n, 2, 0xffff);
            page
        }),
        ("cells past the end", |n, page| {
            put16(n, 4, PAYLOAD_SIZE + 1);
            page
        }),
        ("cells over the offsets", |n, page| {
            put16(n, 4, node::HEADER);
            page
        }),
        ("an offset past the end", |n, page| {
            put16(n, node::HEADER, PAYLOAD_SIZE - 1);
            page
        }),
        ("a cell longer than the page", |n, page| {
            let at = get16(n, node::HEADER);
            put16(n, at, 0xffff);
            page
        }),
        ("a key longer than the maximum", |n, page| {
            let at = get16(n, node::HEADER);
            let value = get16(n, at + 2);
            put16(n, at, MAX_KEY + 1);
            put16(n, at + 2, value - 1);
            page
        }),
        ("two offsets of one cell", |n, page| {
            let first = get16(n, node::HEADER);
            put16(n, node::HEADER + 2, first);
            page
        }),
        ("a gap before the cells", |n, page| {
            let top = get16(n, 4);
            put16(n, 4, top - 1);
            page
        }),
    ];
    let branch: &[(&str, Corrupt)] = &[
        ("a branch at level 0", |n, page| {
            n[1] = 0;
            page
        }),
        ("a child among the store's own pages", |n, _| {
            set_leftmost(n, 1)
        }),
        ("a child past the last page", |n, _| {
            set_leftmost(n, 1 << 40)
        }),
        ("a child that is the branch itself", |n, page| {
            set_leftmost(n, page)
        }),
    ];
    for (cases, on_leaf) in [(leaf, true), (branch, false)] {
        // The branch cases change the root's leftmost child, which the first key goes through.
        let key = long_key(if on_leaf { 30 } else { 0 });
        for (label, corrupt) in cases {
            let engine = temporary();
            let root = committed_tree(&engine, 60);
            let read = engine.read().unwrap();
            let page = if on_leaf {
                leaf_of(&read, root, &key).unwrap()
            } else {
                root
            };
            drop(read);
            let mut tx = engine.write(None).unwrap();
            let named = corrupt(tx.page_mut(page).unwrap(), page);
            tx.commit().unwrap();

            let read = engine.read().unwrap();
            for _ in 0..2 {
                assert_eq!(damaged_page(get(&read, root, &key)), named, "{label}");
            }
            assert_eq!(
                damaged_page(for_each(&read, root, &mut |_, _| {})),
                named,
                "{label}"
            );
            drop(read);
            for op in 0..3 {
                let mut tx = engine.write(None).unwrap();
                let result = match op {
                    0 => set(&mut tx, root, &key, b"new").map(|_| ()),
                    1 => remove(&mut tx, root, &key).map(|_| ()),
                    _ => free_all(&mut tx, root),
                };
                assert_eq!(damaged_page(result), named, "{label}: operation {op}");
                assert_eq!(damaged_page(tx.commit()), named, "{label}: T11");
            }
        }
    }
}

/// R3-001 of the third review (G6, A9): freeing a tree whose lowest branch names itself or an ancestor as a leaf
/// child is damage of that page, never a second free.
#[test]
fn freeing_a_tree_whose_branch_names_itself_or_an_ancestor_is_damage() {
    for ancestor in [false, true] {
        let engine = temporary();
        let root = committed_tree(&engine, 60);
        let read = engine.read().unwrap();
        let mut page = root;
        while node::level(read.page(page).unwrap()) > 1 {
            page = node::child(read.page(page).unwrap(), 0);
        }
        drop(read);
        assert_ne!(page, root, "the tree has three levels or more");
        let target = if ancestor { root } else { page };
        let mut tx = engine.write(None).unwrap();
        node::set_leftmost(tx.page_mut(page).unwrap(), target);
        tx.commit().unwrap();
        let mut tx = engine.write(None).unwrap();
        assert_eq!(
            damaged_page(free_all(&mut tx, root)),
            target,
            "ancestor: {ancestor}"
        );
        assert_eq!(
            damaged_page(tx.commit()),
            target,
            "T11, ancestor: {ancestor}"
        );
    }
}

/// G6, V7: a leaf whose checksum is valid but whose entry names a run that cannot be one (past the frontier, or
/// among the store's own pages) makes reading, scanning and freeing that value fail with damage of the named page,
/// never a panic; and a value off its leaf that is empty or longer than the maximum makes the leaf inconsistent.
#[test]
fn an_inconsistent_value_reference_is_damage_and_not_a_panic() {
    for (label, first, len) in [
        ("past the frontier", 1u64 << 40, 20_000u64),
        ("a header page", 1, 20_000),
        ("too long a value", 0, MAX_VALUE as u64 + 1),
        ("an empty value", 0, 0),
    ] {
        let engine = temporary();
        let root = new_tree(&engine);
        let mut tx = engine.write(None).unwrap();
        set(&mut tx, root, b"a", b"inline").unwrap();
        set(&mut tx, root, b"k", &[5; 20_000]).unwrap();
        let (real, _) = run_of(&tx, root, b"k").unwrap().unwrap();
        tx.commit().unwrap();
        // The reference is the 16 bytes after the key: the run's first page and the value's length. A write that
        // changes the leaf as a page, not as a node, commits the damage (H2 of the commit checks PRD).
        let mut tx = engine.write(None).unwrap();
        let leaf = tx.page_mut(root).unwrap();
        let mut reference = real.to_le_bytes().to_vec();
        reference.extend_from_slice(&20_000u64.to_le_bytes());
        let at = leaf.windows(16).position(|w| w == reference).unwrap();
        let first = if first == 0 { real } else { first };
        leaf[at..at + 8].copy_from_slice(&first.to_le_bytes());
        leaf[at + 8..at + 16].copy_from_slice(&len.to_le_bytes());
        tx.commit().unwrap();
        let leaf_inconsistent = len == 0 || len > MAX_VALUE as u64;
        let named = if leaf_inconsistent { root } else { first };

        let read = engine.read().unwrap();
        assert_eq!(damaged_page(get(&read, root, b"k")), named, "{label}");
        let mut scan = Range::new(
            &read,
            root,
            Interval {
                prefix: b"",
                lower: Bound::Unbounded,
                upper: Bound::Unbounded,
            },
        )
        .unwrap();
        if !leaf_inconsistent {
            assert_eq!(scan.next().unwrap().unwrap().1, b"inline");
        }
        assert_eq!(damaged_page(scan.next().unwrap()), named, "{label}: scan");
        drop(scan);
        drop(read);
        for op in 0..3 {
            let mut tx = engine.write(None).unwrap();
            let result = match op {
                0 => set(&mut tx, root, b"k", b"new").map(|_| ()),
                1 => remove(&mut tx, root, b"k").map(|_| ()),
                _ => free_all(&mut tx, root),
            };
            assert_eq!(damaged_page(result), named, "{label}: operation {op}");
            assert_eq!(damaged_page(tx.commit()), named, "{label}: T11");
        }
    }
}

/// R1-002 of the second review (G6, A9): a catalog entry of an unknown kind or of the wrong length in a page whose
/// checksum is valid is damage of that page, when looking a tree up and when listing, and follows T11 in a write.
#[test]
fn an_inconsistent_catalog_entry_is_damage_and_not_a_panic() {
    let normal = catalog::Entry {
        id: 0,
        kind: krummi_tx::TreeKind::Normal,
        root: 0,
        count: 0,
    };
    let mut wrong_kind = [0u8; 25];
    wrong_kind[8] = 99;
    for bad in [&wrong_kind[..], &[1u8; 24][..], &[1u8; 26][..]] {
        let engine = temporary();
        let mut tx = engine.write(None).unwrap();
        catalog::put(&mut tx, "t", &normal).unwrap();
        let root = tx.slot(catalog::ROOT_SLOT).unwrap();
        set(&mut tx, root, b"t", bad).unwrap();
        tx.commit().unwrap();
        let read = engine.read().unwrap();
        for _ in 0..2 {
            assert_eq!(damaged_page(catalog::lookup(&read, "t")), root);
            assert_eq!(damaged_page(catalog::list(&read)), root);
        }
        assert_eq!(catalog::lookup(&read, "u").unwrap(), None);
        drop(read);
        let tx = engine.write(None).unwrap();
        assert_eq!(damaged_page(catalog::lookup(&tx, "t")), root);
        assert_eq!(damaged_page(tx.commit()), root, "T11");
    }
}

/// Whether `key` is the only entry of its leaf and the leaf's parent is a branch without keys, which D4's merges
/// leave when a branch cannot merge with its sibling: removing the key empties a leaf that has no sibling.
fn alone_under_a_branch_without_keys<S: PageSource>(tx: &S, root: u64, key: &[u8]) -> bool {
    let (mut page, mut parent) = (root, None);
    loop {
        let node = tx.page(page).unwrap();
        if node::is_leaf(node) {
            let alone = node::count(node) == 1 && node::search(node, key).is_ok();
            return alone && parent.is_some_and(|p| node::count(p) == 0);
        }
        parent = Some(node);
        page = node::child(node, node::child_index(node, key));
    }
}

/// R3-empty-nonroot-leaf of the first review: with keys of 2,048 bytes a branch holds three keys at most, so a
/// branch left with one child often cannot merge with its sibling and stays without keys (D4). Removing the last
/// entry of that child frees the leaf and the branch instead of leaving an empty leaf that is not the root; the
/// structure is checked after every operation, and the test reaches that case many times.
#[test]
fn removing_the_last_entry_under_a_branch_without_keys_leaves_no_empty_leaf() {
    let engine = temporary();
    let mut rng = Rng(0x0e3b_7a1d);
    let mut model = Model::new();
    let mut root = new_tree(&engine);
    let mut reached = 0;
    let key = |rng: &mut Rng| {
        let mut key = format!("{:03}", rng.below(400)).into_bytes();
        key.resize(MAX_KEY, b'.');
        key
    };
    for round in 0..8 {
        let mut tx = engine.write(None).unwrap();
        while model.len() < 150 {
            let (k, value) = (key(&mut rng), vec![round; rng.below(9) as usize]);
            root = set(&mut tx, root, &k, &value).unwrap().root;
            model.insert(k, value);
            validate(&tx, root, None, None, true);
        }
        while !model.is_empty() {
            let index = rng.below(model.len() as u64) as usize;
            let k = model.keys().nth(index).unwrap().clone();
            reached += usize::from(alone_under_a_branch_without_keys(&tx, root, &k));
            let update = remove(&mut tx, root, &k).unwrap();
            assert!(update.count_changed);
            root = update.root;
            model.remove(&k);
            validate(&tx, root, None, None, true);
        }
        assert_eq!(height(&tx, root), 1);
        tx.commit().unwrap();
        check(&engine.read().unwrap(), root, &model, &mut rng, "emptied");
    }
    assert!(reached >= 10, "the case was reached {reached} times");
}

/// Bytes of the keys of [`ordered_key`]: long, so that a branch holds few keys and a few thousand entries reach
/// several levels.
pub(crate) const ORDERED_KEY: usize = 1000;

/// The `i`-th key of `stream`, in numeric order of `i` within the stream: the stream's byte, then `i` in big endian.
pub(crate) fn ordered_key(stream: u8, i: u64) -> Vec<u8> {
    let mut key = vec![stream; ORDERED_KEY - 8];
    key.extend_from_slice(&i.to_be_bytes());
    key
}

/// The pages of the tree at `root`, level by level from the root down, each level in key order.
pub(crate) fn levels<S: PageSource>(tx: &S, root: u64) -> Vec<Vec<u64>> {
    let mut levels = vec![vec![root]];
    while !node::is_leaf(tx.page(levels[levels.len() - 1][0]).unwrap()) {
        let below = levels[levels.len() - 1]
            .iter()
            .flat_map(|&page| {
                let node = tx.page(page).unwrap();
                (0..=node::count(node)).map(|index| node::child(node, index))
            })
            .collect();
        levels.push(below);
    }
    levels
}

/// Whether the node at `page` of a tree whose keys have `key` bytes and its values `value` bytes is full: a leaf
/// has no room for another entry, and a branch for two more keys, since a branch that overflows at its end moves
/// its last key to its new sibling.
pub(crate) fn full<S: PageSource>(tx: &S, page: u64, key: usize, value: usize) -> bool {
    let node = tx.page(page).unwrap();
    let room = if node::is_leaf(node) {
        node::LEAF_CELL_HEADER + key + value + 2
    } else {
        2 * (2 + key + 8 + 2)
    };
    node::free_space(node) < room
}

/// The pages of the tree at `root` that are not [`full`], but the last of each level.
pub(crate) fn not_full_but_the_last<S: PageSource>(
    tx: &S,
    root: u64,
    key: usize,
    value: usize,
) -> Vec<u64> {
    levels(tx, root)
        .iter()
        .flat_map(|level| level[..level.len() - 1].to_vec())
        .filter(|&page| !full(tx, page, key, value))
        .collect()
}

/// Entries added in increasing key order leave every node full but the last of its level: a leaf or a branch
/// that overflows at its end starts a new sibling instead of splitting into halves.
#[test]
fn entries_added_in_order_fill_every_node_but_the_last_of_its_level() {
    let engine = temporary();
    let mut tx = engine.write(None).unwrap();
    let mut root = create(&mut tx).unwrap();
    let mut model = Model::new();
    for i in 0..2000 {
        let (key, value) = (ordered_key(1, i), i.to_le_bytes().to_vec());
        root = set(&mut tx, root, &key, &value).unwrap().root;
        model.insert(key, value);
    }
    assert!(height(&tx, root) >= 4);
    let not_full = not_full_but_the_last(&tx, root, ORDERED_KEY, 8);
    assert!(not_full.is_empty(), "not full: {not_full:?}");
    tx.commit().unwrap();
    check(
        &engine.read().unwrap(),
        root,
        &model,
        &mut Rng(3),
        "in order",
    );
}

/// A stream of increasing keys that resumes after another stream took the keys past it fills its leaves too: a
/// leaf that overflows at its end starts a new sibling wherever it is in the tree, not only at the tree's end. The
/// first stream fills whole leaves, so the second starts a leaf of its own and the first resumes at the end of a
/// leaf that is not the last. The branches between both streams receive keys before their own and split into
/// halves, so only the leaves are checked.
#[test]
fn a_stream_that_resumes_before_another_fills_its_leaves() {
    let engine = temporary();
    let mut tx = engine.write(None).unwrap();
    let mut root = create(&mut tx).unwrap();
    let mut model = Model::new();
    for (stream, from) in [(1, 0), (2, 0), (1, 1000)] {
        for i in from..from + 1000 {
            let (key, value) = (ordered_key(stream, i), vec![stream; 8]);
            root = set(&mut tx, root, &key, &value).unwrap().root;
            model.insert(key, value);
        }
    }
    let leaves = levels(&tx, root).pop().unwrap();
    let last_of_first = leaves
        .iter()
        .rposition(|&page| node::key(tx.page(page).unwrap(), 0)[0] == 1)
        .unwrap();
    let not_full: Vec<u64> = leaves[..leaves.len() - 1]
        .iter()
        .enumerate()
        .filter(|&(index, &page)| index != last_of_first && !full(&tx, page, ORDERED_KEY, 8))
        .map(|(_, &page)| page)
        .collect();
    assert!(not_full.is_empty(), "not full: {not_full:?}");
    tx.commit().unwrap();
    check(
        &engine.read().unwrap(),
        root,
        &model,
        &mut Rng(4),
        "resumed stream",
    );
}

/// Entries added in decreasing key order still split their leaves into halves, as before: every leaf but the
/// first, the one that keeps receiving them, has room for another entry.
#[test]
fn entries_added_in_reverse_order_still_split_into_halves() {
    let engine = temporary();
    let mut tx = engine.write(None).unwrap();
    let mut root = create(&mut tx).unwrap();
    let mut model = Model::new();
    for i in (0..2000).rev() {
        let (key, value) = (ordered_key(1, i), i.to_le_bytes().to_vec());
        root = set(&mut tx, root, &key, &value).unwrap().root;
        model.insert(key, value);
    }
    assert!(height(&tx, root) >= 3);
    let leaves = levels(&tx, root).pop().unwrap();
    let full_leaves: Vec<u64> = leaves[1..]
        .iter()
        .copied()
        .filter(|&page| full(&tx, page, ORDERED_KEY, 8))
        .collect();
    assert!(full_leaves.is_empty(), "full: {full_leaves:?}");
    tx.commit().unwrap();
    check(
        &engine.read().unwrap(),
        root,
        &model,
        &mut Rng(5),
        "reverse order",
    );
}

/// AC-Y5 (Y4 of the journal page diffs PRD): a fixed sequence of inserts, removals in the middle and replacements
/// with values of another size, in a normal tree and in a multi-value tree, reads back what was written, reorganizes
/// some node to use its holes, and leaves every tree with the same pages, level by level, as when every removal
/// moves the cells against the end of the payload.
#[test]
fn trees_with_holes_have_the_pages_of_trees_without_them() {
    let run = |compact: bool| {
        node::testing::COMPACT_ON_REMOVE.set(compact);
        node::testing::COMPACTIONS.set(0);
        let engine = temporary();
        let mut rng = Rng(0x6B72_756D_6D69_0005);
        let mut model = Model::new();
        let mut tx = engine.write(None).unwrap();
        let mut root = create(&mut tx).unwrap();
        let mut multi_root = create(&mut tx).unwrap();
        for step in 0..12_000u64 {
            let key = format!("{:06}", rng.below(4_000)).into_bytes();
            let value = vec![step as u8; rng.below(200) as usize];
            if rng.below(3) == 0 {
                root = remove(&mut tx, root, &key).unwrap().root;
                model.remove(&key);
                multi_root = multi::remove_value(&mut tx, multi_root, &key[..2], &key)
                    .unwrap()
                    .root;
            } else {
                root = set(&mut tx, root, &key, &value).unwrap().root;
                model.insert(key.clone(), value);
                multi_root = multi::add(&mut tx, multi_root, &key[..2], &key)
                    .unwrap()
                    .root;
            }
        }
        assert_eq!(entries(&tx, root), model.into_iter().collect::<Vec<_>>());
        let shape: Vec<usize> = levels(&tx, root).iter().map(Vec::len).collect();
        tx.commit().unwrap();
        node::testing::COMPACT_ON_REMOVE.set(false);
        (shape, in_use(&engine), node::testing::COMPACTIONS.get())
    };
    let (holes, compacted) = (run(false), run(true));
    assert_eq!(
        (&holes.0, holes.1),
        (&compacted.0, compacted.1),
        "the same pages"
    );
    assert!(holes.2 > 0, "some node used its holes");
    assert!(compacted.2 > holes.2, "fewer moves of the cells with holes");
}

/// The located overwrite that row updates use (T7 of the row reads run): `get_located` gives a key's value and, for a
/// value on its leaf, where it is; `overwrite` replaces it there when the new value has the same length and the entry
/// is still the key's, and otherwise changes nothing and says so. A read opened before keeps the old value.
#[test]
fn a_located_value_of_the_same_length_is_overwritten_in_place() {
    let engine = temporary();
    let root = committed_tree(&engine, 200);
    let before = engine.read().unwrap();
    let mut tx = engine.write(None).unwrap();
    let (value, at) = get_located(&tx, root, &long_key(7)).unwrap().unwrap();
    assert_eq!(value, b"value");
    let at = at.expect("a value on its leaf");
    assert!(overwrite(&mut tx, at, &long_key(7), b"VALUE").unwrap());
    assert_eq!(get(&tx, root, &long_key(7)).unwrap(), Some(&b"VALUE"[..]));
    // Another length, or another key at that place, change nothing.
    assert!(!overwrite(&mut tx, at, &long_key(7), b"longer").unwrap());
    assert!(!overwrite(&mut tx, at, &long_key(8), b"OTHER").unwrap());
    assert_eq!(get(&tx, root, &long_key(7)).unwrap(), Some(&b"VALUE"[..]));
    assert_eq!(get(&tx, root, &long_key(8)).unwrap(), Some(&b"value"[..]));
    // A missing key has no value, and a value off its leaf no place.
    assert!(get_located(&tx, root, &long_key(500)).unwrap().is_none());
    let big = vec![9u8; 3 * 8192];
    let grown = set(&mut tx, root, &long_key(500), &big).unwrap().root;
    let (value, at) = get_located(&tx, grown, &long_key(500)).unwrap().unwrap();
    assert_eq!((value, at), (&big[..], None));
    tx.commit().unwrap();
    assert_eq!(
        get(&before, root, &long_key(7)).unwrap(),
        Some(&b"value"[..])
    );
    let after = engine.read().unwrap();
    assert_eq!(
        get(&after, grown, &long_key(7)).unwrap(),
        Some(&b"VALUE"[..])
    );
    validate(&after, grown, None, None, true);
}

/// The located replace that multi-value adds use (S2 of the write margin execution): at a place `get_located` gave,
/// a value of another length that stays on its leaf is written there, leaving every node as `set` leaves it; another
/// key at that place, a page that is no leaf, a place past the leaf's entries, a value of the same length
/// (`overwrite`'s case) and one too long to stay on its leaf change nothing and say so.
#[test]
fn a_located_value_of_another_length_is_replaced_in_place() {
    let (engine, twin) = (temporary(), temporary());
    let (root, twin_root) = (committed_tree(&engine, 200), committed_tree(&twin, 200));
    let mut tx = engine.write(None).unwrap();
    let (_, at) = get_located(&tx, root, &long_key(7)).unwrap().unwrap();
    let at = at.expect("a value on its leaf");
    assert!(!node::is_leaf(tx.page(root).unwrap()));
    let pages = node_pages(&tx, root);
    let too_long = vec![1; max_inline_value(MAX_KEY) + 1];
    let refused = [
        (at, long_key(8), &b"longer"[..]),
        (
            Located {
                leaf: root,
                index: 0,
            },
            long_key(7),
            b"longer",
        ),
        (Located { index: 1000, ..at }, long_key(7), b"longer"),
        (at, long_key(7), b"VALUE"),
        (at, long_key(7), &too_long[..]),
    ];
    for (place, key, value) in refused {
        assert!(!replace(&mut tx, root, place, &key, value).unwrap());
    }
    assert_eq!(node_pages(&tx, root), pages);
    assert!(replace(&mut tx, root, at, &long_key(7), b"a longer value").unwrap());
    assert_eq!(
        get(&tx, root, &long_key(7)).unwrap(),
        Some(&b"a longer value"[..])
    );
    let mut twin_tx = twin.write(None).unwrap();
    let update = set(&mut twin_tx, twin_root, &long_key(7), b"a longer value").unwrap();
    assert_eq!((root, update.root), (twin_root, twin_root));
    assert_eq!(node_pages(&tx, root), node_pages(&twin_tx, twin_root));
}

/// The page number and bytes of every node of the tree at `root`, in depth-first order.
fn node_pages<S: PageSource>(tx: &S, root: u64) -> Vec<(u64, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(page) = stack.pop() {
        let node = tx.page(page).unwrap();
        if !node::is_leaf(node) {
            stack.extend((0..=node::count(node)).map(|i| node::child(node, i)));
        }
        out.push((page, node.to_vec()));
    }
    out
}

/// `append_last` adds an entry at the end of the rightmost leaf exactly as `try_add` would: a run of increasing keys,
/// added with it whenever it applies (and with `try_add` otherwise, finding the rightmost leaf again after each),
/// leaves every node byte for byte as `try_add` alone, through splits, values off their leaf, an empty root and a
/// key that is not greater than the leaf's last.
#[test]
fn appending_to_the_last_leaf_changes_the_tree_as_adding_does() {
    let (plain, fast) = (temporary(), temporary());
    let (plain_root, fast_root) = (new_tree(&plain), new_tree(&fast));
    let mut a = plain.write(None).unwrap();
    let mut b = fast.write(None).unwrap();
    let (mut root_a, mut root_b) = (plain_root, fast_root);
    let mut leaf = last_leaf(&b, root_b).unwrap().0;
    let (mut appended, mut added, mut reported) = (0, 0, 0);
    let key_of = |i: usize| format!("{i:08}").into_bytes();
    for i in 0..4_000usize {
        let key = key_of(i);
        let len = match i % 97 {
            0 => 5_000,
            n if n % 3 == 0 => 300,
            _ => 20,
        };
        let value: Vec<u8> = (0..len).map(|j| (i + j) as u8).collect();
        root_a = try_add(&mut a, root_a, &key, &value).unwrap().root;
        if append_last(&mut b, &leaf, &key, &value).unwrap() {
            appended += 1;
        } else {
            let (update, found) = try_add_at_last(&mut b, root_b, &key, &value).unwrap();
            assert!(update.count_changed);
            root_b = update.root;
            // The insert's own path finds the last leaf when it added at its end and nothing above changed.
            let (last, greatest) = last_leaf(&b, root_b).unwrap();
            if let Some(found) = found {
                assert_eq!((found, greatest.as_deref()), (last, Some(&key[..])), "{i}");
                reported += 1;
            }
            leaf = last;
            added += 1;
        }
        assert_eq!(root_a, root_b, "{i}");
    }
    assert!(appended > 30 * added && added > 40, "{appended} {added}");
    assert!(reported > added / 3, "{reported} of {added}");
    // A key that is not greater than the last one is never appended, and changes nothing.
    assert!(!append_last(&mut b, &leaf, &key_of(3_999), b"x").unwrap());
    assert!(!append_last(&mut b, &leaf, &key_of(10), b"x").unwrap());
    assert_eq!(node_pages(&a, root_a), node_pages(&b, root_b));
    assert_eq!(last_leaf(&b, root_b).unwrap().1, Some(key_of(3_999)));
    a.commit().unwrap();
    b.commit().unwrap();
    let read = fast.read().unwrap();
    validate(&read, root_b, None, None, true);
    assert_eq!(entries(&read, root_b).len(), 4_000);
}

/// `append_last` against `try_add` with removals in between, which leave leaves under a quarter full beside the last
/// one: after any change but its own, the caller finds the last leaf again; the trees stay the same, node for node.
#[test]
fn appending_after_removals_changes_the_tree_as_adding_does() {
    let mut rng = Rng(7);
    let (plain, fast) = (temporary(), temporary());
    let (mut root_a, mut root_b) = (new_tree(&plain), new_tree(&fast));
    let mut a = plain.write(None).unwrap();
    let mut b = fast.write(None).unwrap();
    let mut leaf = last_leaf(&b, root_b).unwrap().0;
    let key_of = |i: u64| format!("{i:08}").into_bytes();
    let (mut next, mut appended) = (0u64, 0);
    for _ in 0..20_000 {
        if rng.below(10) < 2 && next > 0 {
            let key = key_of(rng.below(next));
            root_a = remove(&mut a, root_a, &key).unwrap().root;
            root_b = remove(&mut b, root_b, &key).unwrap().root;
            leaf = last_leaf(&b, root_b).unwrap().0;
        } else {
            let key = key_of(next);
            next += 1;
            let value = vec![next as u8; rng.below(200) as usize];
            root_a = try_add(&mut a, root_a, &key, &value).unwrap().root;
            if append_last(&mut b, &leaf, &key, &value).unwrap() {
                appended += 1;
            } else {
                root_b = try_add(&mut b, root_b, &key, &value).unwrap().root;
                leaf = last_leaf(&b, root_b).unwrap().0;
            }
        }
        assert_eq!(root_a, root_b);
    }
    assert!(appended > 10_000, "{appended}");
    assert_eq!(node_pages(&a, root_a), node_pages(&b, root_b));
    validate(&b, root_b, None, None, true);
}

/// The state where adding would merge the last leaf: after a split leaves it with one entry, removals bring its left
/// sibling down to just over a quarter, so that both fit in one page. `append_last` declines, and `try_add` merges them.
#[test]
fn appending_declines_where_adding_would_merge_the_last_leaf() {
    let engine = temporary();
    let mut root = new_tree(&engine);
    let mut tx = engine.write(None).unwrap();
    let key_of = |i: u64| format!("{i:08}").into_bytes();
    let value = [7u8; 100];
    let mut next = 0;
    // Until a split leaves the last leaf with one entry and a left sibling.
    let (leaf, left) = loop {
        root = try_add(&mut tx, root, &key_of(next), &value).unwrap().root;
        next += 1;
        let (leaf, _) = last_leaf(&tx, root).unwrap();
        if let (Beside::Left(left), 1) = (leaf.beside, node::count(tx.page(leaf.page).unwrap())) {
            break (leaf, left);
        }
    };
    let mut gone = next - 2;
    while node::used(tx.page(left).unwrap()) > UNDERFULL + 2 * (value.len() + 20) {
        root = remove(&mut tx, root, &key_of(gone)).unwrap().root;
        gone -= 1;
    }
    let (now, _) = last_leaf(&tx, root).unwrap();
    assert_eq!(now, leaf, "no merge yet");
    assert!(!append_last(&mut tx, &now, &key_of(next), &value).unwrap());
    let pages = node_pages(&tx, root).len();
    root = try_add(&mut tx, root, &key_of(next), &value).unwrap().root;
    assert!(
        node_pages(&tx, root).len() < pages,
        "adding merged the two leaves"
    );
    validate(&tx, root, None, None, true);
}

/// `try_add_at_last` reports the last leaf also when the insert leaves it under a quarter full and `merge_child` finds
/// that the pair does not fit, changing nothing: right after an ordered run splits the last leaf.
#[test]
fn the_last_leaf_is_reported_when_the_merge_does_not_fit() {
    let engine = temporary();
    let mut root = new_tree(&engine);
    let mut tx = engine.write(None).unwrap();
    let mut underfull_reported = 0;
    // About a hundred entries of an 8-byte key and a 60-byte value fill a leaf, so 3,000 ordered inserts split the last
    // leaf some thirty times, and each split leaves a new last leaf under a quarter full beside a full one, a pair that
    // does not fit; more than ten such reports shows the arm runs again and again, not once by chance.
    for i in 0..3_000u64 {
        let key = format!("{i:08}").into_bytes();
        let (update, found) = try_add_at_last(&mut tx, root, &key, &[3u8; 60]).unwrap();
        root = update.root;
        let (last, _) = last_leaf(&tx, root).unwrap();
        if let Some(found) = found {
            assert_eq!(found, last, "{i}");
            if matches!(found.beside, Beside::Left(_))
                && node::used(tx.page(found.page).unwrap()) < UNDERFULL
            {
                underfull_reported += 1;
            }
        }
    }
    assert!(underfull_reported > 10, "{underfull_reported}");
    validate(&tx, root, None, None, true);
}

/// A write transaction remembers one leaf, the last of its tree, for keys from the first key it was given on, and
/// only for its tree's root; a key of that tree below it forgets the leaf, a key of another tree does not; it forgets
/// it too when it reserves or frees pages or an operation fails (S1 of the ordered-write cache execution).
#[test]
fn a_transaction_remembers_a_leaf_until_it_reserves_or_frees_pages() {
    let engine = temporary();
    let mut tx = engine.write(None).unwrap();
    let (root, leaf) = (5, 9);
    assert_eq!(tx.recent_leaf(root, b"m"), None);
    tx.remember_leaf(root, leaf, b"c");
    for key in [&b"c"[..], b"m", b"p", b"\xff\xff"] {
        assert_eq!(tx.recent_leaf(root, key), Some(leaf), "{key:?}");
    }
    assert_eq!(tx.recent_leaf(root + 1, b"a"), None);
    assert_eq!(
        tx.recent_leaf(root, b"m"),
        Some(leaf),
        "another tree's key forgets nothing"
    );
    assert_eq!(tx.recent_leaf(root, b"bz"), None);
    assert_eq!(
        tx.recent_leaf(root, b"m"),
        None,
        "a key below the first one forgets the leaf"
    );
    tx.remember_leaf(root, leaf, b"c");
    assert_eq!(tx.recent_leaf(root, b"c"), Some(leaf));
    tx.remember_leaf(root + 1, leaf + 1, b"x");
    assert_eq!(
        tx.recent_leaf(root, b"m"),
        None,
        "remembering another leaf replaces it"
    );
    assert_eq!(tx.recent_leaf(root + 1, b"y"), Some(leaf + 1));

    let page = tx.allocate().unwrap();
    assert_eq!(tx.recent_leaf(root + 1, b"y"), None);
    tx.remember_leaf(root, leaf, b"c");
    tx.free(page, 1).unwrap();
    assert_eq!(tx.recent_leaf(root, b"m"), None);
    tx.remember_leaf(root, leaf, b"c");
    tx.allocate_value_run(1).unwrap();
    assert_eq!(tx.recent_leaf(root, b"m"), None);
    tx.remember_leaf(root, leaf, b"c");
    // Reading a page the transaction freed is a misuse, which fails the transaction (F8, T11).
    assert!(tx.page(page).is_err());
    assert_eq!(tx.recent_leaf(root, b"m"), None);
}

/// The results, roots and every node byte of three trees changed in the same transactions are the same whether an
/// operation goes to the leaf the transaction remembers or always walks from the root (S1 of the ordered-write
/// cache execution). The operations move between the trees, so the remembered leaf is often another tree's, and
/// alternate runs of increasing keys, which find it, with keys at random: sets of short, same-length and long values
/// (off their leaf), adds (also through `try_add_at_last`, which never goes to it), removals, range removals and
/// reads, and now and then a tree freed and created again, so that leaves split and merge and roots change under a
/// remembered leaf, every other transaction removing more than it adds; commits in between.
#[test]
fn the_remembered_leaf_changes_trees_as_walking_does() {
    type Outcome = (Vec<(u64, u64, Option<Vec<u8>>)>, Vec<(u64, Vec<u8>)>);
    let run = |walk: bool| -> (Outcome, usize) {
        testing::WALK_FROM_ROOT.set(walk);
        testing::REMEMBERED.set(0);
        let engine = temporary();
        let mut rng = Rng(23);
        let mut roots = [new_tree(&engine), new_tree(&engine), new_tree(&engine)];
        let mut results = Vec::new();
        let key_of = |i: u64| format!("{i:06}").into_bytes();
        let mut next = [0u64; 3];
        for round in 0..8 {
            let mut tx = engine.write(None).unwrap();
            for i in 0..3_000u64 {
                let t = if rng.below(5) == 0 {
                    rng.below(3)
                } else {
                    i / 40 % 3
                } as usize;
                let n = if i / 150 % 2 == 0 {
                    // Past the keys at random: adds at the end of the tree's last leaf.
                    next[t] = next[t].max(2_000) + 1 + rng.below(3);
                    next[t]
                } else {
                    rng.below(2_000)
                };
                let key = key_of(n);
                let len = [8, 8, 8, 9, 40, 200, 5_000][rng.below(7) as usize];
                let value: Vec<u8> = (0..len).map(|j| (n + j) as u8 ^ rng.next() as u8).collect();
                let root = roots[t];
                let op = rng.below(20);
                // Every other transaction removes more than it adds, so that leaves fall under a quarter full and
                // merge, also the leaf remembered.
                let op = if round % 2 == 1 && op < 9 && !op.is_multiple_of(3) {
                    11 + op % 5
                } else {
                    op
                };
                let (root, changed, read) = match op {
                    0..=8 => {
                        let update = set(&mut tx, root, &key, &value).unwrap();
                        (update.root, u64::from(update.count_changed), None)
                    }
                    9 => {
                        let update = try_add(&mut tx, root, &key, &value).unwrap();
                        (update.root, u64::from(update.count_changed), None)
                    }
                    // The add that walks without the remembered leaf, below or above its first key.
                    10 => {
                        let (update, _) = try_add_at_last(&mut tx, root, &key, &value).unwrap();
                        (update.root, u64::from(update.count_changed), None)
                    }
                    11..=15 => {
                        let update = remove(&mut tx, root, &key).unwrap();
                        (update.root, u64::from(update.count_changed), None)
                    }
                    16..=17 => {
                        let value = get(&tx, root, &key).unwrap().map(<[u8]>::to_vec);
                        (root, 0, value)
                    }
                    18 => {
                        let upper = key_of(n + rng.below(40));
                        let interval = Interval {
                            prefix: b"",
                            lower: Bound::Included(&key),
                            upper: Bound::Excluded(&upper),
                        };
                        let update = remove_range(&mut tx, root, interval).unwrap();
                        (update.root, update.removed, None)
                    }
                    _ if rng.below(40) == 0 => {
                        free_all(&mut tx, root).unwrap();
                        (create(&mut tx).unwrap(), u64::MAX, None)
                    }
                    _ => (
                        root,
                        0,
                        get(&tx, root, &key_of(n + 1)).unwrap().map(<[u8]>::to_vec),
                    ),
                };
                roots[t] = root;
                results.push((root, changed, read));
            }
            tx.commit().unwrap();
        }
        let read = engine.read().unwrap();
        let mut pages = Vec::new();
        for root in roots {
            validate(&read, root, None, None, true);
            pages.extend(node_pages(&read, root));
            for (key, value) in entries(&read, root) {
                pages.push((u64::MAX, [key, value].concat()));
            }
        }
        testing::WALK_FROM_ROOT.set(false);
        ((results, pages), testing::REMEMBERED.get())
    };
    let (walked, none) = run(true);
    let (remembered, found) = run(false);
    assert_eq!(none, 0);
    // Only changes in a tree's last leaf find it, and random keys and moves between the three trees forget it, so only
    // a share of the changes go there; enough still do to compare both paths.
    assert!(
        found > 1_000,
        "only {found} changes went to the remembered leaf"
    );
    assert!(walked.0 == remembered.0, "the results differ");
    assert_eq!(walked.1.len(), remembered.1.len());
    for ((page, a), (other, b)) in walked.1.iter().zip(&remembered.1) {
        assert_eq!(page, other);
        assert!(a == b, "page {page} differs");
    }
}

/// An ordered load walks the tree about once per leaf: after a walk, the sets that follow go to the leaf it reached
/// until it splits, and every node byte is the same as with a walk for every set (S1 of the ordered-write
/// cache execution).
#[test]
fn an_ordered_load_walks_once_per_leaf() {
    let run = |walk: bool| {
        testing::WALK_FROM_ROOT.set(walk);
        testing::REMEMBERED.set(0);
        let engine = temporary();
        let mut root = new_tree(&engine);
        let mut tx = engine.write(None).unwrap();
        // Keys of 100 bytes make branches of some seventy children, so the tree has three levels.
        for i in 0..20_000u64 {
            let mut key = [b'.'; 100];
            key[..8].copy_from_slice(&i.to_be_bytes());
            root = set(&mut tx, root, &key, &[i as u8; 100]).unwrap().root;
        }
        assert!(height(&tx, root) >= 3);
        let leaves = levels(&tx, root).pop().unwrap().len();
        let pages = node_pages(&tx, root);
        testing::WALK_FROM_ROOT.set(false);
        (pages, leaves, testing::REMEMBERED.get())
    };
    let (walked, _, none) = run(true);
    let (remembered, leaves, found) = run(false);
    assert_eq!(none, 0);
    // Every set finds the remembered leaf but the first after each split, which walks to the new leaf, and the sets of
    // the first leaf, the root.
    assert!(
        found >= 20_000 - 2 * leaves,
        "{found} of 20,000 with {leaves} leaves"
    );
    assert!(walked == remembered, "the trees differ");
}

/// A remembered leaf that a removal leaves under a quarter full is merged as a walk would merge it, and once the merge
/// frees it, the transaction no longer goes there (S1 of the ordered-write cache execution): the last leaf of a tree,
/// beside a sibling emptied while the pair did not fit, loses entries until it merges into that sibling; a read of one
/// of its keys then finds it in the sibling.
#[test]
fn a_leaf_merged_away_is_not_remembered() {
    let engine = temporary();
    let mut tx = engine.write(None).unwrap();
    let mut root = create(&mut tx).unwrap();
    let key = |i: u64| i.to_be_bytes();
    let value = |i: u64| [i as u8; 100];
    // The leaf of `key` as a walk from the root finds it, which neither uses nor changes the remembered leaf.
    let walked = |tx: &WriteTransaction<'_>, root: u64, i: u64| {
        testing::WALK_FROM_ROOT.set(true);
        let leaf = leaf_of(tx, root, &key(i)).unwrap();
        testing::WALK_FROM_ROOT.set(false);
        leaf
    };
    for i in 0..1_000 {
        root = set(&mut tx, root, &key(i), &value(i)).unwrap().root;
    }
    // Fill the last leaf up, then empty its left sibling but for a few entries.
    let mut greatest = 999;
    let last = walked(&tx, root, greatest);
    while node::used(tx.page(last).unwrap()) < PAYLOAD_SIZE - 200 {
        greatest += 1;
        root = set(&mut tx, root, &key(greatest), &value(greatest))
            .unwrap()
            .root;
    }
    assert_eq!(walked(&tx, root, greatest), last);
    let first_of_last = node::key(tx.page(last).unwrap(), 0).to_vec();
    let first_of_last = u64::from_be_bytes(first_of_last.try_into().unwrap());
    let sibling = walked(&tx, root, first_of_last - 1);
    let first_of_sibling = node::key(tx.page(sibling).unwrap(), 0).to_vec();
    let first_of_sibling = u64::from_be_bytes(first_of_sibling.try_into().unwrap());
    for i in first_of_sibling..first_of_last - 5 {
        root = remove(&mut tx, root, &key(i)).unwrap().root;
    }
    assert_eq!(walked(&tx, root, first_of_last - 1), sibling);
    assert_eq!(walked(&tx, root, greatest), last);
    // Removals from the end of the last leaf, which the transaction remembers, until it merges into its sibling.
    let mut removed = greatest;
    while walked(&tx, root, first_of_last) == last {
        assert!(removed > first_of_last, "the last leaf never merged");
        root = remove(&mut tx, root, &key(removed)).unwrap().root;
        removed -= 1;
    }
    assert!(
        node::used(tx.page(sibling).unwrap()) >= UNDERFULL,
        "the merge left the sibling under a quarter full"
    );
    for i in first_of_last..=removed {
        assert_eq!(walked(&tx, root, i), sibling);
        assert_eq!(get(&tx, root, &key(i)).unwrap(), Some(&value(i)[..]));
    }
    validate(&tx, root, None, None, true);
}

/// A measurement of the remembered leaf, not a check: the time per operation of loads shaped as the bench's, with the
/// leaf remembered and with every operation walking from the root (`testing::WALK_FROM_ROOT`), alternated, best of
/// five each. W1: 1,000,000 sets of 16-byte keys and 100-byte values in increasing order, 1,000 per transaction; W2:
/// the same keys in a scrambled order; W3: those sets in one transaction; W19: a value added to each of 100,000 keys
/// spread by a hash, ten times over, 1,000 adds per transaction. It prints for W19 how often an add lands on the leaf of the add before it. Run it in a
/// release build, on an otherwise idle machine; `RECENT_LOADS` (as `W2,W3`) picks the loads and `RECENT_ROUNDS` the
/// rounds:
///
/// `cargo test -p krummi-tree --release --lib -- --ignored --nocapture recent_leaf_measurement`
#[test]
#[ignore = "a measurement; run it in a release build"]
fn recent_leaf_measurement() {
    use std::time::{Duration, Instant};

    const ENTRIES: u64 = 1_000_000;
    const BATCH: u64 = 1_000;
    let ordered = |i: u64| -> [u8; 16] {
        let mut key = [0; 16];
        key[..8].copy_from_slice(&i.to_be_bytes());
        key[8..].copy_from_slice(&i.wrapping_mul(0x9E37_79B9_7F4A_7C15).to_le_bytes());
        key
    };
    let splitmix = |mut z: u64| {
        z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    // The bench's keys (`krummi-bench`'s `data::key`).
    let hashed = |i: u64| -> [u8; 16] {
        let mut k = [0u8; 16];
        k[..8].copy_from_slice(&splitmix(i ^ 0x5EED_0001).to_be_bytes());
        k[8..].copy_from_slice(&splitmix(!i).to_be_bytes());
        k
    };
    // An operation: on the tree at the root given, the one of index given; it returns the root afterwards.
    type Load<'a> = &'a dyn Fn(&mut WriteTransaction<'_>, u64, u64) -> u64;
    // The time of the operations of a load, `per_write` in each transaction, without the commits, from an empty tree.
    let run = |op: Load<'_>, per_write: u64| {
        let engine = temporary();
        let mut root = new_tree(&engine);
        let mut time = Duration::ZERO;
        for batch in 0..ENTRIES / per_write {
            let mut tx = engine.write(None).unwrap();
            let start = Instant::now();
            for i in batch * per_write..(batch + 1) * per_write {
                root = op(&mut tx, root, i);
            }
            time += start.elapsed();
            tx.commit().unwrap();
        }
        time
    };
    let value = [7u8; 100];
    let w1 =
        |tx: &mut WriteTransaction<'_>, root, i| set(tx, root, &ordered(i), &value).unwrap().root;
    let w2 = |tx: &mut WriteTransaction<'_>, root, i| {
        let n = i * 7919 % ENTRIES;
        set(tx, root, &ordered(n), &value).unwrap().root
    };
    let base = 19u64 << 40;
    let w19 = |tx: &mut WriteTransaction<'_>, root, i: u64| {
        let (v, k) = (i / 100_000, i % 100_000);
        let value = (base + k * 10 + v).to_be_bytes();
        let value = [value, value].concat();
        crate::multi::add(tx, root, &hashed(base + k), &value)
            .unwrap()
            .root
    };
    let loads: [(&str, Load<'_>, u64); 4] = [
        ("W1", &w1, BATCH),
        ("W2", &w2, BATCH),
        ("W3", &w2, ENTRIES),
        ("W19", &w19, BATCH),
    ];
    let picked = std::env::var("RECENT_LOADS").unwrap_or_default();
    let rounds = std::env::var("RECENT_ROUNDS").map_or(5, |n| n.parse().unwrap());
    for (name, op, per_write) in loads {
        if !picked.is_empty() && !picked.split(',').any(|load| load == name) {
            continue;
        }
        let (mut remembered, mut walked) = (Duration::MAX, Duration::MAX);
        for _ in 0..rounds {
            for walk in [false, true] {
                testing::WALK_FROM_ROOT.set(walk);
                let time = run(op, per_write);
                let best = if walk { &mut walked } else { &mut remembered };
                *best = (*best).min(time);
            }
        }
        testing::WALK_FROM_ROOT.set(false);
        let per = |time: Duration| time.as_secs_f64() * 1e9 / ENTRIES as f64;
        println!(
            "{name:<4} remembered {:>7.1} ns/op, walked {:>7.1} ns/op, {:+.1} %",
            per(remembered),
            per(walked),
            (per(remembered) / per(walked) - 1.0) * 100.0
        );
    }

    if !picked.is_empty() && !picked.split(',').any(|load| load == "W19") {
        return;
    }
    // W19: where consecutive adds land.
    let engine = temporary();
    let mut root = new_tree(&engine);
    let (mut same, mut total, mut previous) = (0u64, 0u64, None);
    for batch in 0..ENTRIES / BATCH {
        let mut tx = engine.write(None).unwrap();
        for i in batch * BATCH..(batch + 1) * BATCH {
            root = w19(&mut tx, root, i);
            let page = leaf_of(&tx, root, &hashed(base + i % 100_000)).unwrap();
            same += u64::from(previous == Some(page));
            total += 1;
            previous = Some(page);
        }
        tx.commit().unwrap();
    }
    println!("W19 adds on the leaf of the add before: {same} of {total}");
}
