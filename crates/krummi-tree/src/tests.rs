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
        // The reference is the 16 bytes after the key: the run's first page and the value's length.
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
