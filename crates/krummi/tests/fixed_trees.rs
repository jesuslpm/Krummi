//! Fixed-size trees through the public API (Fixed-size trees PRD, X1–X15): AC-X1–AC-X6, AC-X8–AC-X11, AC-X15–AC-X18
//! and AC-X20. What must not compile (AC-X7, and the lifetime part of AC-X14) is in `trees_compile.rs`; the crashes
//! during a commit (AC-X12, its first part) and the altered bytes (AC-X13) are in `tree_integrity.rs`; where the
//! references of Leer and of the scans point (AC-X14), a store that refuses writes (the T12 parts of AC-X17 and
//! AC-X20), the failures of T11 at the page operations (AC-X12, its second part) and the remembered layout check
//! (AC-X19) need the engine's test hooks and are in the library's own tests (`src/fixed_tests.rs`, `src/tests.rs`
//! and `src/check_tests.rs`).
//!
//! Each criterion runs on a store on disk and on a temporary one, except the parts that reopen or damage the
//! store's files, which run on disk only. The tests that depend on the layout of the pages take it from the
//! implementation (`krummi_tree::fixed`, D1 of the execution).

mod support;

use std::ops::Bound::{self, Excluded, Included, Unbounded};
use std::path::Path;

use krummi::{Error, ErrorKind, OpenMode, SizeSubject, Store, StoreOptions, TreeKind};
use krummi_pager::DATA_FILE;
use krummi_pager::format::PAGE_SIZE;
use krummi_tree::catalog;
use krummi_tx::{Engine, OpenOptions as EngineOptions};
use support::*;

const MAX: u64 = u64::MAX;

/// The kind of a fixed-size tree of values of `len` bytes.
fn fixed(len: u16) -> TreeKind {
    TreeKind::FixedSize(len)
}

fn own((key, value): (u64, &[u8])) -> (u64, Vec<u8>) {
    (key, value.to_vec())
}

/// The entries of `model`, in order.
fn entries(model: &FixedModel) -> Vec<(u64, Vec<u8>)> {
    model
        .iter()
        .map(|(key, value)| (*key, value.clone()))
        .collect()
}

/// Checks that the fixed-size tree `tree` of the transaction `tx`, read or write, holds exactly `model`: its number of
/// entries, a whole scan from the front, from the back and from both ends in turn, and Leer of every key.
macro_rules! check_fixed {
    ($tx:expr, $tree:expr, $model:expr) => {{
        let (tx, tree, model): (_, _, &FixedModel) = (&$tx, &$tree, &$model);
        let want = entries(model);
        assert_eq!(tx.len(tree).unwrap(), want.len() as u64, "entries");
        assert_eq!(
            collect(tx.range_fixed(tree, ..).unwrap(), own),
            want,
            "forwards"
        );
        let back = collect(tx.range_fixed(tree, ..).unwrap().rev(), own);
        assert_eq!(reversed(back), want, "backwards");
        let both = alternate(tx.range_fixed(tree, ..).unwrap(), own);
        assert_eq!(both, want, "from both ends");
        for (key, value) in model {
            assert_eq!(tx.get_fixed(tree, *key).unwrap(), Some(&value[..]), "{key}");
        }
    }};
}

/// Checks in a new read that the fixed-size tree `name` of values of `len` bytes holds exactly `model`.
fn check_read(store: &Store, name: &str, len: usize, model: &FixedModel) {
    let read = store.read().unwrap();
    let tree = read.open_fixed_tree(name, len).unwrap();
    check_fixed!(read, tree, model);
}

/// The fields of an invalid-size error of a value, after checking its folder.
#[track_caller]
fn value_size(error: Error, folder: Option<&Path>) -> (u64, u64, u64) {
    match invalid_size(error, folder) {
        (SizeSubject::Value, received, min, max) => (received, min, max),
        (what, ..) => panic!("expected an invalid size of a value, got one of a {what}"),
    }
}

/// A requested length over the maximum: the invalid size of X2.
const OVER: (u64, u64, u64) = (2049, 0, 2048);

/// AC-X1 (X5, AC-B7): the keys `0`, `1`, `2^63−1`, `2^63`, `2^64−2` and `2^64−1` and random others, stored in a
/// random order, come back from a whole scan in numeric order from the front and in reverse from the back, in the
/// write, in a later read and, on disk, after reopening.
#[test]
fn ac_x1_keys_come_back_in_numeric_order_from_both_ends() {
    let mut rng = Lcg(0x5eed_0001);
    let keys = [0, 1, (1 << 63) - 1, 1 << 63, MAX - 1, MAX]
        .into_iter()
        .chain((0..1500).map(|_| rng.next_u64()));
    let model = fixed_model(keys, 8);
    let mut order: Vec<u64> = model.keys().copied().collect();
    rng.shuffle(&mut order);
    let scenario = |store: &Store| {
        let mut write = store.write().unwrap();
        let tree = create_fixed(&mut write, "f", 8);
        for key in &order {
            write.set_fixed(&tree, *key, &model[key]).unwrap();
        }
        check_fixed!(write, tree, model);
        write.commit().unwrap();
        check_read(store, "f", 8, &model);
    };
    stores(|store, _| scenario(store));

    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    scenario(&store);
    store.close().unwrap();
    assert!(fixed_height(tmp.path(), "f", 8) >= 2, "several leaves");
    let store = open(tmp.path());
    check_read(&store, "f", 8, &model);
    store.close().unwrap();
}

/// AC-X2 (X1, X2): a write creates fixed-size trees of lengths `0`, `1`, `8` and `2048` and stores and reads values of
/// their length; Leer tells a key of the tree of length `0`, with its empty value, from a key that is not there; and
/// opening or creating one of length `2049`, in every mode and in Open of a read, with and without a tree of that
/// name, fails without effect with the invalid size of a value, `2049` received, minimum `0` and maximum `2048`.
#[test]
fn ac_x2_lengths_from_0_to_2048_and_a_requested_length_over_2048() {
    stores(|store, folder| {
        let lengths = [0, 1, 8, 2048];
        let model = |len| fixed_model([0, 7, 1 << 40, MAX], len);
        let mut write = store.write().unwrap();
        for len in lengths {
            let tree = create_fixed(&mut write, &format!("len{len}"), len);
            for (key, value) in &model(len) {
                write.set_fixed(&tree, *key, value).unwrap();
            }
            check_fixed!(write, tree, model(len));
        }
        let zero = write.open_fixed_tree("len0", 0).unwrap();
        assert_eq!(write.get_fixed(&zero, 7).unwrap(), Some(&[][..]));
        assert_eq!(write.get_fixed(&zero, 8).unwrap(), None);

        let trees = write.list_trees().unwrap();
        let kinds: Vec<_> = [0, 1, 2048, 8]
            .map(|len| (format!("len{len}"), fixed(len)))
            .into();
        assert_eq!(trees, kinds, "in byte order of the names");
        for name in ["len8", "absent"] {
            let error = write.open_fixed_tree(name, 2049).err().unwrap();
            assert_eq!(value_size(error, folder), OVER, "{name}");
            for mode in MODES {
                let error = write.open_fixed_tree_with(name, 2049, mode).err().unwrap();
                assert_eq!(value_size(error, folder), OVER, "{name} {mode:?}");
            }
        }
        assert_eq!(write.list_trees().unwrap(), trees, "nothing created");
        let tree = write.open_fixed_tree("len8", 8).unwrap();
        check_fixed!(write, tree, model(8));
        write.commit().unwrap();

        let read = store.read().unwrap();
        for name in ["len8", "absent"] {
            let error = read.open_fixed_tree(name, 2049).err().unwrap();
            assert_eq!(value_size(error, folder), OVER, "{name}");
        }
        assert_eq!(read.list_trees().unwrap(), trees);
        for len in lengths {
            let tree = read.open_fixed_tree(&format!("len{len}"), len).unwrap();
            check_fixed!(read, tree, model(len));
        }
        let zero = read.open_fixed_tree("len0", 0).unwrap();
        assert_eq!(read.get_fixed(&zero, 7).unwrap(), Some(&[][..]));
        assert_eq!(read.get_fixed(&zero, 8).unwrap(), None);
    });
}

/// AC-X3 (X1, X6, AC-B16): storing or adding a value of 0, 7 or 9 bytes in a tree of length 8, with a key that is
/// there and one that is not, fails without effect with the invalid size of a value, the size received, minimum `8`
/// and maximum `8`, and the transaction stays usable; in a tree of length 0, a value of 1 byte fails the same with
/// minimum and maximum `0`.
#[test]
fn ac_x3_a_value_of_another_length_fails_without_effect() {
    stores(|store, folder| {
        let mut eight = fixed_model([1], 8);
        let mut zero = fixed_model([1], 0);
        commit_fixed(store, "f", 8, &eight);
        commit_fixed(store, "z", 0, &zero);
        let mut write = store.write().unwrap();
        let tree = write.open_fixed_tree("f", 8).unwrap();
        let empty = write.open_fixed_tree("z", 0).unwrap();
        for key in [1, 2] {
            for len in [0, 7, 9] {
                let value = vec![0xEE; len];
                let want = (len as u64, 8, 8);
                let error = write.set_fixed(&tree, key, &value).unwrap_err();
                assert_eq!(value_size(error, folder), want, "set {key} {len}");
                let error = write.try_add_fixed(&tree, key, &value).unwrap_err();
                assert_eq!(value_size(error, folder), want, "add {key} {len}");
            }
            let error = write.set_fixed(&empty, key, b"x").unwrap_err();
            assert_eq!(value_size(error, folder), (1, 0, 0));
            let error = write.try_add_fixed(&empty, key, b"x").unwrap_err();
            assert_eq!(value_size(error, folder), (1, 0, 0));
        }
        check_fixed!(write, tree, eight);
        check_fixed!(write, empty, zero);
        write.set_fixed(&tree, 2, &fixed_value(2, 8)).unwrap();
        assert!(write.try_add_fixed(&empty, 2, b"").unwrap());
        write.commit().unwrap();
        eight.insert(2, fixed_value(2, 8));
        zero.insert(2, Vec::new());
        check_read(store, "f", 8, &eight);
        check_read(store, "z", 0, &zero);
    });
}

/// Checks that the write `$write` cannot open the fixed-size tree `f` of length 8 as one of 16 bytes, as normal or as
/// multi-value, nor the normal tree `n` and the multi-value tree `m` as fixed-size of 8 bytes, in any mode.
macro_rules! check_kinds_in_write {
    ($write:expr, $folder:expr) => {{
        let folder = $folder;
        let fails = |error: Error, (name, found, requested): (&str, TreeKind, TreeKind)| {
            assert_eq!(
                different(error, folder),
                (name.to_owned(), found, requested)
            );
        };
        let (f, n, m) = (
            ("f", fixed(8), fixed(16)),
            ("n", NORMAL, fixed(8)),
            ("m", MULTI, fixed(8)),
        );
        fails($write.open_fixed_tree("f", 16).err().unwrap(), f);
        fails(
            $write.open_tree("f").err().unwrap(),
            ("f", fixed(8), NORMAL),
        );
        fails(
            $write.open_multi_tree("f").err().unwrap(),
            ("f", fixed(8), MULTI),
        );
        fails($write.open_fixed_tree("n", 8).err().unwrap(), n);
        fails($write.open_fixed_tree("m", 8).err().unwrap(), m);
        for mode in MODES {
            fails($write.open_fixed_tree_with("f", 16, mode).err().unwrap(), f);
            let error = $write.open_tree_with("f", mode).err().unwrap();
            fails(error, ("f", fixed(8), NORMAL));
            let error = $write.open_multi_tree_with("f", mode).err().unwrap();
            fails(error, ("f", fixed(8), MULTI));
            fails($write.open_fixed_tree_with("n", 8, mode).err().unwrap(), n);
            fails($write.open_fixed_tree_with("m", 8, mode).err().unwrap(), m);
        }
    }};
}

/// Checks that the read `$read` cannot open `f`, `n` and `m` as another kind, as [`check_kinds_in_write`] does.
macro_rules! check_kinds_in_read {
    ($read:expr, $folder:expr) => {{
        let folder = $folder;
        let found = |error: Error| different(error, folder);
        let f = |kind| ("f".to_owned(), fixed(8), kind);
        assert_eq!(
            found($read.open_fixed_tree("f", 16).err().unwrap()),
            f(fixed(16))
        );
        assert_eq!(found($read.open_tree("f").err().unwrap()), f(NORMAL));
        assert_eq!(found($read.open_multi_tree("f").err().unwrap()), f(MULTI));
        let n = ("n".to_owned(), NORMAL, fixed(8));
        assert_eq!(found($read.open_fixed_tree("n", 8).err().unwrap()), n);
        let m = ("m".to_owned(), MULTI, fixed(8));
        assert_eq!(found($read.open_fixed_tree("m", 8).err().unwrap()), m);
    }};
}

/// The trees of AC-X4: `f` of length 8 with `model`, a normal `n` and a multi-value `m`, each committed.
fn kinds_base(store: &Store, model: &FixedModel) {
    commit_fixed(store, "f", 8, model);
    commit_tree(store, "n", &[(b"k", b"v")]);
    commit_multi(store, "m", &Model::from([(b"k".to_vec(), set_of(&[b"v"]))]));
}

/// AC-X4 (X3, X4, AC-B8): opening or creating, in every mode, a fixed-size tree of length 8 as one of 16 bytes, as
/// normal or as multi-value, and a normal and a multi-value tree as fixed-size of 8 bytes, fails with the
/// different-tree-type class, the name, the kind the tree has and the kind requested, with their lengths, and leaves
/// the tree intact, also with a page of the tree damaged and the catalog intact; deleting takes no kind; Listar
/// reports each kind with the length of the fixed-size one; renaming and emptying keep the length; Rename with
/// overwrite deletes a fixed-size target; and a deleted fixed-size tree gives its name to one of another length.
#[test]
fn ac_x4_a_fixed_size_tree_opens_only_with_its_kind_and_length() {
    let model = fixed_model(0..600, 8);
    stores(|store, folder| {
        kinds_base(store, &model);
        let mut write = store.write().unwrap();
        let tree = write.open_fixed_tree("f", 8).unwrap();
        write.set_fixed(&tree, 1000, &fixed_value(1000, 8)).unwrap();
        check_kinds_in_write!(write, folder);
        write.set_fixed(&tree, 1001, &fixed_value(1001, 8)).unwrap();
        let mut model = model.clone();
        model.extend(fixed_model([1000, 1001], 8));
        check_fixed!(write, tree, model);
        write.commit().unwrap();

        let read = store.read().unwrap();
        check_kinds_in_read!(read, folder);
        let tree = read.open_fixed_tree("f", 8).unwrap();
        check_fixed!(read, tree, model);
        let normal = read.open_tree("n").unwrap();
        assert_eq!(read.get(&normal, b"k").unwrap(), Some(&b"v"[..]));
        let multi = read.open_multi_tree("m").unwrap();
        assert!(read.contains(&multi, b"k", b"v").unwrap());
        let kinds = [("f", fixed(8)), ("m", MULTI), ("n", NORMAL)].map(|(n, k)| (n.to_owned(), k));
        assert_eq!(read.list_trees().unwrap(), kinds);
        drop(read);

        let mut write = store.write().unwrap();
        let tree = write.open_fixed_tree("f", 8).unwrap();
        write.rename_tree("f", "g", false).unwrap();
        let kinds = [("g", fixed(8)), ("m", MULTI), ("n", NORMAL)].map(|(n, k)| (n.to_owned(), k));
        assert_eq!(
            write.list_trees().unwrap(),
            kinds,
            "renaming keeps the length"
        );
        assert_eq!(
            write.len(&tree).unwrap(),
            model.len() as u64,
            "the handle goes on"
        );
        let renamed = write.open_fixed_tree_with("g", 8, OpenMode::Empty).unwrap();
        assert_eq!(write.len(&renamed).unwrap(), 0);
        assert_eq!(write.len(&tree).unwrap(), 0, "the same tree");
        assert_eq!(
            write.list_trees().unwrap(),
            kinds,
            "emptying keeps the length"
        );
        write.set_fixed(&renamed, 5, &fixed_value(5, 8)).unwrap();
        write.rename_tree("n", "g", true).unwrap();
        assert_eq!(not_found(write.len(&tree).unwrap_err(), folder), "g");
        let kinds = [("g", NORMAL), ("m", MULTI)].map(|(n, k)| (n.to_owned(), k));
        assert_eq!(
            write.list_trees().unwrap(),
            kinds,
            "the fixed-size target is gone"
        );
        create_fixed(&mut write, "h", 8);
        write.delete_tree("h").unwrap();
        let reborn = create_fixed(&mut write, "h", 16);
        write.set_fixed(&reborn, 1, &fixed_value(1, 16)).unwrap();
        write.commit().unwrap();

        let read = store.read().unwrap();
        let kinds = [("g", NORMAL), ("h", fixed(16)), ("m", MULTI)].map(|(n, k)| (n.to_owned(), k));
        assert_eq!(read.list_trees().unwrap(), kinds);
        let reborn = read.open_fixed_tree("h", 16).unwrap();
        check_fixed!(read, reborn, fixed_model([1], 16));
        drop(read);

        let mut write = store.write().unwrap();
        for name in ["g", "h", "m"] {
            write.delete_tree(name).unwrap();
        }
        assert!(
            write.list_trees().unwrap().is_empty(),
            "deleting takes no kind"
        );
        write.commit().unwrap();
    });

    // A damaged page of each tree, with the catalog intact.
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    kinds_base(&store, &model);
    store.close().unwrap();
    assert!(fixed_height(tmp.path(), "f", 8) >= 2);
    let leaf = fixed_leaf_page(tmp.path(), "f", 8, 0);
    for name in ["f", "n", "m"] {
        damage(tmp.path(), root_page(tmp.path(), name));
    }
    damage(tmp.path(), leaf);
    let store = open(tmp.path());
    let folder = Some(tmp.path());
    let mut write = store.write().unwrap();
    check_kinds_in_write!(write, folder);
    write.commit().unwrap();
    let read = store.read().unwrap();
    check_kinds_in_read!(read, folder);
    drop(read);
    store.close().unwrap();
}

/// AC-X5 (X6, A4, A5): in an empty fixed-size tree, with the keys `0` and `2^64−1` among others, Guardar creates or
/// overwrites, Añadir reports that it added the first time and that it did not the second without changing anything,
/// Leer returns the value stored or that the key is not there, Quitar reports that the key was there the first time
/// and that it was not the second, and the number of entries is at every step the number of keys stored and not
/// removed.
#[test]
fn ac_x5_set_add_get_and_remove_by_key() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let tree = create_fixed(&mut write, "f", 8);
        let mut model = FixedModel::new();
        macro_rules! step {
            ($key:expr) => {{
                let key: u64 = $key;
                assert_eq!(write.len(&tree).unwrap(), model.len() as u64, "entries");
                let want = model.get(&key).map(Vec::as_slice);
                assert_eq!(write.get_fixed(&tree, key).unwrap(), want, "{key}");
            }};
        }
        for key in [0, MAX, 1 << 63] {
            step!(key);
            write.set_fixed(&tree, key, &fixed_value(key, 8)).unwrap();
            model.insert(key, fixed_value(key, 8));
            step!(key);
            write.set_fixed(&tree, key, &[0xAA; 8]).unwrap();
            model.insert(key, vec![0xAA; 8]);
            step!(key);
        }
        for key in [1, MAX - 1, 0] {
            let added = !model.contains_key(&key);
            let value = fixed_value(key, 8);
            assert_eq!(
                write.try_add_fixed(&tree, key, &value).unwrap(),
                added,
                "{key}"
            );
            model.entry(key).or_insert(value);
            step!(key);
            assert!(
                !write.try_add_fixed(&tree, key, &[0xBB; 8]).unwrap(),
                "{key}"
            );
            step!(key);
        }
        for key in [0, MAX, 1, 42] {
            let was = model.remove(&key).is_some();
            assert_eq!(write.remove_fixed(&tree, key).unwrap(), was, "{key}");
            step!(key);
            assert!(!write.remove_fixed(&tree, key).unwrap(), "{key}");
            step!(key);
        }
        check_fixed!(write, tree, model);
        write.commit().unwrap();
        check_read(store, "f", 8, &model);
    });
}

/// Whether `key` is within `lower` and `upper` (R1).
fn within(key: u64, (lower, upper): (Bound<u64>, Bound<u64>)) -> bool {
    let above = match lower {
        Included(bound) => key >= bound,
        Excluded(bound) => key > bound,
        Unbounded => true,
    };
    let below = match upper {
        Included(bound) => key <= bound,
        Excluded(bound) => key < bound,
        Unbounded => true,
    };
    above && below
}

/// The intervals that X7 calls empty: the lower bound above the upper one, the same integer with one excluded, `5`
/// and `6` excluded, from `2^64−1` excluded and up to `0` excluded.
const EMPTY_INTERVALS: [(Bound<u64>, Bound<u64>); 12] = [
    (Included(8), Included(5)),
    (Excluded(8), Excluded(5)),
    (Included(MAX), Included(0)),
    (Included(5), Excluded(5)),
    (Excluded(5), Included(5)),
    (Excluded(5), Excluded(5)),
    (Excluded(5), Excluded(6)),
    (Excluded(MAX), Unbounded),
    (Excluded(MAX), Included(MAX)),
    (Unbounded, Excluded(0)),
    (Included(0), Excluded(0)),
    (Excluded(MAX - 1), Excluded(MAX)),
];

/// The step between the keys of [`scan_model`].
const STEP: u64 = MAX / 1000;

/// The tree of AC-X6: `0`, `1`, `2^64−2`, `2^64−1` and the multiples of [`STEP`], over several leaves.
fn scan_model() -> FixedModel {
    fixed_model(
        [0, 1, MAX - 1, MAX]
            .into_iter()
            .chain((1..1000).map(|i| i * STEP)),
        8,
    )
}

/// Every pair of bounds made of `items`, each included, excluded or open.
fn bound_pairs(items: &[u64]) -> Vec<(Bound<u64>, Bound<u64>)> {
    let bounds: Vec<Bound<u64>> = items
        .iter()
        .flat_map(|item| [Included(*item), Excluded(*item)])
        .chain([Unbounded])
        .collect();
    bounds
        .iter()
        .flat_map(|lower| bounds.iter().map(move |upper| (*lower, *upper)))
        .collect()
}

/// Checks that every scan of `$tree` in `$tx` between the bounds of `$pairs` returns from the front, from the back
/// and from both ends in turn exactly the entries of `$model` within them.
macro_rules! check_scans {
    ($tx:expr, $tree:expr, $model:expr, $pairs:expr) => {{
        let (tx, tree, model) = (&$tx, &$tree, &$model);
        for bounds in $pairs {
            let want: Vec<(u64, Vec<u8>)> = entries(model)
                .into_iter()
                .filter(|(key, _)| within(*key, bounds))
                .collect();
            assert_eq!(
                collect(tx.range_fixed(tree, bounds).unwrap(), own),
                want,
                "{bounds:?}"
            );
            let back = collect(tx.range_fixed(tree, bounds).unwrap().rev(), own);
            assert_eq!(reversed(back), want, "{bounds:?} backwards");
            let both = alternate(tx.range_fixed(tree, bounds).unwrap(), own);
            assert_eq!(both, want, "{bounds:?} from both ends");
        }
    }};
}

/// AC-X6 (X7, R1–R3, R7): scans with included, excluded and open bounds, `0` and `2^64−1` among them and bounds that
/// are not keys, taken from both ends, return exactly what R1 and R2 say in numeric order; every empty interval of X7
/// scans nothing and removes `0`, also with the root page of the tree damaged (on disk); removing intervals, the whole
/// tree too, returns how many entries it removed, the number of entries drops by that much and a scan returns the
/// others. It also runs the example of the PRD's *Salida*, the tree `contadores`.
#[test]
fn ac_x6_scans_and_range_removals_by_integer_bounds() {
    let model = scan_model();
    let items = [0, 1, 2, 500 * STEP, 500 * STEP + 1, MAX - 2, MAX - 1, MAX];
    let pairs = bound_pairs(&items);
    stores(|store, _| {
        commit_fixed(store, "f", 8, &model);
        let mut write = store.write().unwrap();
        let tree = write.open_fixed_tree("f", 8).unwrap();
        check_scans!(write, tree, model, pairs.iter().copied());
        for bounds in EMPTY_INTERVALS {
            let mut scan = write.range_fixed(&tree, bounds).unwrap();
            assert!(
                scan.next().is_none() && scan.next_back().is_none(),
                "{bounds:?}"
            );
            assert_eq!(
                write.remove_range_fixed(&tree, bounds).unwrap(),
                0,
                "{bounds:?}"
            );
        }
        let read = store.read().unwrap();
        let committed = read.open_fixed_tree("f", 8).unwrap();
        check_scans!(read, committed, model, pairs.iter().copied());
        drop(read);

        let mut model = model.clone();
        let removals = [
            (Included(10 * STEP), Excluded(20 * STEP)),
            (Excluded(MAX - 1), Unbounded),
            (Unbounded, Included(1)),
            (Excluded(30 * STEP), Excluded(31 * STEP)),
            (Included(500 * STEP + 1), Included(700 * STEP)),
            (Unbounded, Unbounded),
        ];
        for bounds in removals {
            let want: Vec<u64> = model
                .keys()
                .copied()
                .filter(|key| within(*key, bounds))
                .collect();
            let before = write.len(&tree).unwrap();
            assert_eq!(
                write.remove_range_fixed(&tree, bounds).unwrap(),
                want.len() as u64,
                "{bounds:?}"
            );
            model.retain(|key, _| !want.contains(key));
            assert_eq!(write.len(&tree).unwrap(), before - want.len() as u64);
            check_fixed!(write, tree, model);
        }
        assert!(model.is_empty());
        write.commit().unwrap();
        check_read(store, "f", 8, &model);
    });

    // Empty intervals read no page: with the tree's root damaged they still scan nothing and remove nothing.
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_fixed(&store, "f", 8, &model);
    store.close().unwrap();
    let root = root_page(tmp.path(), "f");
    damage(tmp.path(), root);
    let store = open(tmp.path());
    let read = store.read().unwrap();
    let tree = read.open_fixed_tree("f", 8).unwrap();
    for bounds in EMPTY_INTERVALS {
        let mut scan = read.range_fixed(&tree, bounds).unwrap();
        assert!(
            scan.next().is_none() && scan.next_back().is_none(),
            "{bounds:?}"
        );
    }
    let mut scan = read.range_fixed(&tree, ..).unwrap();
    assert_eq!(
        damaged_page(scan.next().unwrap().unwrap_err()),
        root,
        "the damage is there"
    );
    drop(scan);
    drop(read);
    let mut write = store.write().unwrap();
    let tree = write.open_fixed_tree("f", 8).unwrap();
    for bounds in EMPTY_INTERVALS {
        assert_eq!(
            write.remove_range_fixed(&tree, bounds).unwrap(),
            0,
            "{bounds:?}"
        );
    }
    write.commit().unwrap();
    store.close().unwrap();

    salida_example();
}

/// The example of the PRD's *Salida*: a tree `contadores` of length 8 with the keys `0`, `7` and `2^64−1`.
fn salida_example() {
    stores(|store, folder| {
        let model = fixed_model([0, 7, MAX], 8);
        commit_fixed(store, "contadores", 8, &model);
        let read = store.read().unwrap();
        let kind = fixed(8);
        assert_eq!(kind.to_string(), "fixed-size of 8 bytes");
        assert_eq!(
            read.list_trees().unwrap(),
            [("contadores".to_owned(), kind)]
        );
        assert_eq!(read.tree_kind("contadores").unwrap(), Some(kind));
        assert_eq!(read.tree_kind("pedidos").unwrap(), None);
        let tree = read.open_fixed_tree("contadores", 8).unwrap();
        assert_eq!(
            collect(read.range_fixed(&tree, ..).unwrap(), own),
            entries(&model)
        );
        let back = collect(
            read.range_fixed(&tree, (Excluded(7), Unbounded))
                .unwrap()
                .rev(),
            own,
        );
        assert_eq!(back, [(MAX, fixed_value(MAX, 8))]);
        // The example's interval: from 8 to 5, both included, which is empty on purpose.
        #[allow(clippy::reversed_empty_ranges)]
        let eight_to_five = 8..=5;
        let mut scan = read.range_fixed(&tree, eight_to_five).unwrap();
        assert!(scan.next().is_none() && scan.next_back().is_none());
        assert_eq!(read.get_fixed(&tree, 3).unwrap(), None);
        drop(read);

        let mut write = store.write().unwrap();
        let tree = write.open_fixed_tree("contadores", 8).unwrap();
        assert!(!write.try_add_fixed(&tree, 7, &[1; 8]).unwrap());
        assert_eq!(write.remove_range_fixed(&tree, 1..=MAX).unwrap(), 2);
        assert_eq!(write.len(&tree).unwrap(), 1);
        let error = write.set_fixed(&tree, 0, &[0; 9]).unwrap_err();
        assert_eq!(invalid_size(error, folder), (SizeSubject::Value, 9, 8, 8));
        let error = write.open_fixed_tree("contadores", 16).err().unwrap();
        let want = ("contadores".to_owned(), fixed(8), fixed(16));
        assert_eq!(different(error, folder), want);
        write.commit().unwrap();
        check_read(store, "contadores", 8, &fixed_model([0], 8));
    });
}

/// The keys of AC-X8: values of 2,048 bytes take a page each three, so 2,000 of them make a tree of three levels.
fn space_model() -> FixedModel {
    fixed_model((0..2000).map(|i| i * 7), SPACE_LEN)
}

const SPACE_LEN: usize = 2048;

/// A tree `a` and the fixed-size tree `f` created empty, each committed.
fn space_base(store: &Store) {
    commit_tree(store, "a", &[(b"k", b"v")]);
    commit_fixed(store, "f", SPACE_LEN, &FixedModel::new());
}

/// AC-X8 (X9, A2, A3): within one region of free space, a fixed-size tree of several levels that loses all its
/// entries, by Quitar, Quitar intervalo, a mix of both or emptying, takes the pages it took created empty; and
/// deleting it or overwriting it with Rename gives back the pages it took.
#[test]
fn ac_x8_the_pages_of_a_fixed_size_tree_go_back_to_free_space() {
    let model = space_model();
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    space_base(&store);
    commit_fixed(&store, "f", SPACE_LEN, &model);
    store.close().unwrap();
    assert_eq!(
        fixed_height(tmp.path(), "f", SPACE_LEN),
        3,
        "several levels"
    );

    for form in 0..4 {
        stores(|store, _| {
            space_base(store);
            let empty = in_use(store);
            commit_fixed(store, "f", SPACE_LEN, &model);
            assert!(in_use(store) > empty + 600, "pages of the tree");
            let mut write = store.write().unwrap();
            let tree = write.open_fixed_tree("f", SPACE_LEN).unwrap();
            match form {
                0 => {
                    for key in model.keys() {
                        assert!(write.remove_fixed(&tree, *key).unwrap());
                    }
                }
                1 => assert_eq!(write.remove_range_fixed(&tree, ..).unwrap(), 2000),
                2 => {
                    for key in model.keys().step_by(3) {
                        assert!(write.remove_fixed(&tree, *key).unwrap());
                    }
                    let mut removed = 0;
                    for bounds in [0..700 * 7, 700 * 7..1500 * 7, 1500 * 7..MAX] {
                        removed += write.remove_range_fixed(&tree, bounds).unwrap();
                    }
                    assert_eq!(removed, 2000 - 667);
                }
                _ => drop(
                    write
                        .open_fixed_tree_with("f", SPACE_LEN, OpenMode::Empty)
                        .unwrap(),
                ),
            }
            assert_eq!(write.len(&tree).unwrap(), 0);
            write.commit().unwrap();
            assert_eq!(in_use(store), empty, "form {form}");
        });
    }

    stores(|store, _| {
        commit_tree(store, "a", &[(b"k", b"v")]);
        commit_tree(store, "src", &[(b"k", b"v")]);
        let before = in_use(store);
        for overwrite in [false, true] {
            commit_fixed(store, "f", SPACE_LEN, &model);
            assert!(in_use(store) > before + 600);
            let mut write = store.write().unwrap();
            if overwrite {
                write.rename_tree("src", "f", true).unwrap();
            } else {
                write.delete_tree("f").unwrap();
            }
            write.commit().unwrap();
            assert_eq!(in_use(store), before, "overwrite {overwrite}");
        }
    });
}

/// Checks that every operation of a fixed-size tree that both transactions have fails on `$tree` with `$expect`.
macro_rules! fixed_read_operations_fail {
    ($tx:expr, $tree:expr, $expect:expr) => {{
        let (tx, tree, expect) = (&$tx, &$tree, $expect);
        expect(tx.get_fixed(tree, 1).unwrap_err());
        expect(tx.len(tree).unwrap_err());
        expect(tx.range_fixed(tree, ..).err().unwrap());
        expect(
            tx.range_fixed(tree, (Excluded(MAX), Unbounded))
                .err()
                .unwrap(),
        );
    }};
}

/// Checks that every operation of a fixed-size tree of values of `$len` bytes fails on `$tree` with `$expect` in the
/// write `$write`, and that a value of another length fails with the invalid size instead.
macro_rules! fixed_write_operations_fail {
    ($write:expr, $tree:expr, $len:expr, $expect:expr) => {{
        let expect = $expect;
        fixed_read_operations_fail!($write, $tree, &expect);
        let (tree, value) = (&$tree, vec![1; $len]);
        expect($write.set_fixed(tree, 1, &value).unwrap_err());
        expect($write.try_add_fixed(tree, 1, &value).unwrap_err());
        expect($write.remove_fixed(tree, 1).unwrap_err());
        expect($write.remove_range_fixed(tree, ..).unwrap_err());
        expect($write.remove_range_fixed(tree, 5..5).unwrap_err());
        let other = vec![1; $len + 1];
        let size = |error: Error| error.kind() == ErrorKind::InvalidSize;
        assert!(
            size($write.set_fixed(tree, 1, &other).unwrap_err()),
            "the size wins"
        );
        assert!(size($write.try_add_fixed(tree, 1, &other).unwrap_err()));
    }};
}

/// AC-X9 (X4, X11, A6, A7, AC-B14): in a write with a fixed-size tree opened twice, deleting it through one handle
/// makes every operation through the other fail without effect with tree not found and the tree's last name, also
/// after another tree is created with that name; a handle of another transaction fails without effect with misuse;
/// and a value of another length wins over both with the invalid size.
#[test]
fn ac_x9_handles_of_a_deleted_tree_and_of_another_transaction() {
    stores(|store, folder| {
        let model = fixed_model([1, 2], 8);
        commit_fixed(store, "f", 8, &model);
        let mut write = store.write().unwrap();
        let first = write.open_fixed_tree("f", 8).unwrap();
        let second = write.open_fixed_tree("f", 8).unwrap();
        write.delete_tree("f").unwrap();
        let gone = |error: Error| assert_eq!(not_found(error, folder), "f");
        fixed_write_operations_fail!(write, second, 8, &gone);
        let current = create_fixed(&mut write, "f", 8);
        fixed_write_operations_fail!(write, second, 8, &gone);
        fixed_write_operations_fail!(write, first, 8, &gone);
        assert_eq!(
            write.len(&current).unwrap(),
            0,
            "the new tree does not change"
        );
        write.set_fixed(&current, 7, &fixed_value(7, 8)).unwrap();
        write.rename_tree("f", "g", false).unwrap();
        let renamed = |error: Error| assert_eq!(not_found(error, folder), "f");
        fixed_write_operations_fail!(write, second, 8, &renamed);
        write.rename_tree("g", "f", false).unwrap();
        write.commit().unwrap();
        let model = fixed_model([7], 8);
        check_read(store, "f", 8, &model);

        let old = store.write().unwrap();
        let stale = old.open_fixed_tree("f", 8).unwrap();
        old.commit().unwrap();
        let mut write = store.write().unwrap();
        let wrong = |error: Error| misuse(error, folder);
        fixed_write_operations_fail!(write, stale, 8, &wrong);
        let tree = write.open_fixed_tree("f", 8).unwrap();
        check_fixed!(write, tree, model);
        write.commit().unwrap();

        let first = store.read().unwrap();
        let stale = first.open_fixed_tree("f", 8).unwrap();
        drop(first);
        let read = store.read().unwrap();
        fixed_read_operations_fail!(read, stale, &wrong);
        let tree = read.open_fixed_tree("f", 8).unwrap();
        check_fixed!(read, tree, model);
    });
}

/// AC-X15 (X8, T10): a write on disk that only opens an existing fixed-size tree in mode OpenOrCreate, adds keys that
/// are there, removes keys that are not, removes ranges that remove nothing, reads, counts and scans, and fails for
/// invalid sizes and different tree types, commits without error and leaves the store's files byte-identical.
#[test]
fn ac_x15_a_write_that_changes_nothing_leaves_the_files_identical() {
    let model = fixed_model((0..1500).map(|i| i * 3), 8);
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_fixed(&store, "f", 8, &model);
    commit_tree(&store, "n", &[(b"k", b"v")]);
    store.close().unwrap();
    assert!(fixed_height(tmp.path(), "f", 8) >= 2);
    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        let tree = write
            .open_fixed_tree_with("f", 8, OpenMode::OpenOrCreate)
            .unwrap();
        for key in [0, 1500, 4497] {
            assert!(!write.try_add_fixed(&tree, key, &[0xCC; 8]).unwrap());
        }
        for key in [1, 1501, 4498, MAX] {
            assert!(!write.remove_fixed(&tree, key).unwrap());
        }
        assert_eq!(write.remove_range_fixed(&tree, 1..3).unwrap(), 0);
        assert_eq!(write.remove_range_fixed(&tree, 4498..).unwrap(), 0);
        for bounds in EMPTY_INTERVALS {
            assert_eq!(write.remove_range_fixed(&tree, bounds).unwrap(), 0);
        }
        check_fixed!(write, tree, model);
        assert!(write.set_fixed(&tree, 0, &[0; 9]).is_err());
        assert!(write.try_add_fixed(&tree, 5, &[0; 7]).is_err());
        assert!(write.open_fixed_tree("f", 2049).is_err());
        assert!(
            write
                .open_fixed_tree_with("x", 2049, OpenMode::CreateNew)
                .is_err()
        );
        assert!(
            write
                .open_fixed_tree_with("f", 16, OpenMode::Empty)
                .is_err()
        );
        assert!(
            write
                .open_fixed_tree_with("n", 8, OpenMode::CreateOrEmpty)
                .is_err()
        );
        assert!(write.open_tree_with("f", OpenMode::Empty).is_err());
        assert!(
            write
                .open_multi_tree_with("f", OpenMode::OpenOrCreate)
                .is_err()
        );
        write.commit().unwrap();
    });
    assert!(!changed, "a write that changes nothing writes nothing");
}

/// Copies the store kept as test data in `tests/data/<fixture>` into a new folder.
fn fixture(fixture: &str) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(fixture)
        .join(DATA_FILE);
    std::fs::copy(source, tmp.path().join(DATA_FILE)).unwrap();
    tmp
}

/// Whether `page` holds `bytes` somewhere.
fn holds(page: &[u8], bytes: &[u8]) -> bool {
    page.windows(bytes.len()).any(|window| window == bytes)
}

/// AC-X16 (X12, A12, AC-B12): the store of format `8` that the code before this change created (`tests/data/store-5.4`:
/// a normal tree `t` with the keys `0..20` as 8 bytes big-endian and values `[i; 8]`, and a multi-value tree `m` with
/// the key `key` and the values `0..5` of one byte, made by the code of commit `2125833` compiled apart), and the
/// stores of formats `6` and `7` kept before, are refused as previous formats and left byte-identical, with nothing
/// added to their folders; a new store reports format `9`, with the size and the free pages of a new store of format
/// `8` (65,536 bytes, 8 pages, 4 of them free, measured with the code of format `8`); and on disk the leaf of a
/// fixed-size tree holds its keys and the tree's length in little endian, and so does its catalog entry, as the
/// implementation lays them out (D1 and D5).
#[test]
fn ac_x16_formats_6_to_8_are_previous_and_a_new_store_has_the_current_format() {
    for (name, format) in [("store-5.1", 6), ("store-5.3", 7), ("store-5.4", 8)] {
        let tmp = fixture(name);
        let data = tmp.path().join(DATA_FILE);
        let before = std::fs::read(&data).unwrap();
        match Store::open(tmp.path(), &StoreOptions::default()).err() {
            Some(Error::DifferentFormat {
                folder,
                found,
                expected,
            }) => {
                assert_eq!(folder, tmp.path(), "{name}");
                assert_eq!((found, expected), (format, 10), "{name}");
            }
            other => panic!("{name}: expected a previous format, got {other:?}"),
        }
        assert_eq!(std::fs::read(&data).unwrap(), before, "{name}");
        let entries: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(entries, vec![DATA_FILE], "{name}");
    }

    stores(|store, _| {
        let info = store.info();
        assert_eq!(info.format_version, 10);
        assert_eq!(
            (info.size_bytes, info.page_count, info.free_pages),
            (65_536, 8, 4)
        );
    });

    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let (low, high) = (0x0102_0304_0506_0708, 0x1112_1314_1516_1718);
    let model = FixedModel::from([(low, vec![0xA1; 8]), (high, vec![0xB2; 8])]);
    commit_fixed(&store, "f", 8, &model);
    store.close().unwrap();
    let (leaf, entry) = inspect(tmp.path(), |read| {
        let entry = catalog::lookup(read, "f").unwrap().unwrap();
        (
            krummi_tree::fixed::leaf_of(read, entry.root, 8, low).unwrap(),
            entry,
        )
    });
    let catalog_leaf = leaf_page(tmp.path(), None, b"f");
    let bytes = std::fs::read(tmp.path().join(DATA_FILE)).unwrap();
    let page = |number: u64| &bytes[number as usize * PAGE_SIZE..][..PAGE_SIZE];
    // The node header (kind 3, level 0, two entries, length 8) and the entries, each key and its value.
    let mut node = vec![3, 0, 2, 0, 8, 0, 0, 0];
    node.extend([0; 8]);
    node.extend([8, 7, 6, 5, 4, 3, 2, 1]);
    node.extend([0xA1; 8]);
    node.extend([0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11]);
    node.extend([0xB2; 8]);
    assert!(holds(page(leaf), &node), "the leaf holds {node:?}");
    // The catalog entry: identity, kind 3, root, number of entries, and the length.
    let mut blob = entry.id.to_le_bytes().to_vec();
    blob.push(3);
    blob.extend(entry.root.to_le_bytes());
    blob.extend(2u64.to_le_bytes());
    blob.extend(8u16.to_le_bytes());
    assert!(
        holds(page(catalog_leaf), &blob),
        "the catalog holds {blob:?}"
    );
}

/// D5 and G6: a catalog entry of a fixed-size tree has 27 bytes, the length last, of up to 2,048; any other entry of
/// kind `3`, an entry of 27 bytes of another kind, and a length over 2,048 are damage of the catalog's page when
/// looking the tree up and when listing; normal entries keep their 25 bytes.
#[test]
fn catalog_entries_of_fixed_size_trees_are_checked_by_kind() {
    let entry = |kind: u8, len: usize, length: u16| {
        let mut bytes = vec![0; len];
        bytes[8] = kind;
        if len >= 27 {
            bytes[25..27].copy_from_slice(&length.to_le_bytes());
        }
        bytes
    };
    let good = [
        (entry(3, 27, 2048), fixed(2048)),
        (entry(3, 27, 0), fixed(0)),
    ];
    let bad = [
        entry(3, 27, 2049),
        entry(3, 27, u16::MAX),
        entry(3, 25, 0),
        entry(3, 26, 0),
        entry(3, 28, 8),
        entry(1, 27, 8),
        entry(2, 27, 8),
        entry(4, 27, 8),
    ];
    let normal = catalog::Entry {
        id: 0,
        kind: NORMAL,
        root: 0,
        count: 0,
    };
    let cases = good
        .iter()
        .map(|(bytes, kind)| (bytes, Some(*kind)))
        .chain(bad.iter().map(|bytes| (bytes, None)));
    for (bytes, kind) in cases {
        let engine = Engine::open_temporary(&EngineOptions::default()).unwrap();
        let mut tx = engine.write(None).unwrap();
        catalog::put(&mut tx, "t", &normal).unwrap();
        let root = tx.slot(catalog::ROOT_SLOT).unwrap();
        krummi_tree::set(&mut tx, root, b"t", bytes).unwrap();
        tx.commit().unwrap();
        let read = engine.read().unwrap();
        match kind {
            Some(kind) => {
                assert_eq!(catalog::lookup(&read, "t").unwrap().unwrap().kind, kind);
                assert_eq!(catalog::list(&read).unwrap(), [("t".to_owned(), kind)]);
            }
            None => {
                assert_eq!(
                    damaged_page(catalog::lookup(&read, "t").unwrap_err()),
                    root,
                    "{bytes:?}"
                );
                assert_eq!(
                    damaged_page(catalog::list(&read).unwrap_err()),
                    root,
                    "{bytes:?}"
                );
            }
        }
        drop(read);
        engine.close().unwrap();
    }
}

/// The fields of an invalid-size error of a tree name, after checking its folder.
#[track_caller]
fn name_size(error: Error, folder: Option<&Path>) -> (u64, u64, u64) {
    match invalid_size(error, folder) {
        (SizeSubject::TreeName, received, min, max) => (received, min, max),
        (what, ..) => panic!("expected an invalid size of a tree name, got one of a {what}"),
    }
}

/// A name of 1,025 bytes: the invalid size of B4.
const LONG_NAME: (u64, u64, u64) = (1025, 1, 1024);

/// AC-X17 (X2, X11, AC-B4, AC-B9, AC-B13): opening a fixed-size tree with a name of 1,025 bytes and a length of 2,049
/// fails with the invalid size of the name; with a valid name and a length of 2,049, over a fixed-size tree of another
/// length, a normal one, a missing one in mode Open, an existing one in mode CreateNew and, on disk, with the
/// catalog's page damaged, it fails without effect with the invalid size of the length; and a write with changes
/// before and after failures of X1 and X2 commits them all. The part of a store that refuses writes (T12) is in the
/// library's tests.
#[test]
fn ac_x17_the_name_and_then_the_requested_length_win_over_the_catalog() {
    let long = "x".repeat(1025);
    stores(|store, folder| {
        commit_fixed(store, "f", 4, &fixed_model([1], 4));
        commit_tree(store, "n", &[(b"k", b"v")]);
        let mut write = store.write().unwrap();
        let tree = write.open_fixed_tree("f", 4).unwrap();
        write.set_fixed(&tree, 2, &fixed_value(2, 4)).unwrap();
        let trees = write.list_trees().unwrap();
        let error = write.open_fixed_tree(&long, 2049).err().unwrap();
        assert_eq!(name_size(error, folder), LONG_NAME);
        for mode in MODES {
            let error = write.open_fixed_tree_with(&long, 2049, mode).err().unwrap();
            assert_eq!(name_size(error, folder), LONG_NAME, "{mode:?}");
        }
        for name in ["f", "n", "absent"] {
            let error = write.open_fixed_tree(name, 2049).err().unwrap();
            assert_eq!(value_size(error, folder), OVER, "{name}");
            for mode in MODES {
                let error = write.open_fixed_tree_with(name, 2049, mode).err().unwrap();
                assert_eq!(value_size(error, folder), OVER, "{name} {mode:?}");
            }
        }
        let error = write.set_fixed(&tree, 3, &[3; 5]).unwrap_err();
        assert_eq!(value_size(error, folder), (5, 4, 4));
        write.set_fixed(&tree, 4, &fixed_value(4, 4)).unwrap();
        assert_eq!(write.list_trees().unwrap(), trees, "nothing created");
        write.commit().unwrap();

        let read = store.read().unwrap();
        let error = read.open_fixed_tree(&long, 2049).err().unwrap();
        assert_eq!(name_size(error, folder), LONG_NAME);
        for name in ["f", "n", "absent"] {
            let error = read.open_fixed_tree(name, 2049).err().unwrap();
            assert_eq!(value_size(error, folder), OVER, "{name}");
        }
        drop(read);
        check_read(store, "f", 4, &fixed_model([1, 2, 4], 4));
    });

    // The catalog's page damaged: the length wins over reading it.
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_fixed(&store, "f", 4, &fixed_model([1], 4));
    store.close().unwrap();
    let catalog_leaf = leaf_page(tmp.path(), None, b"f");
    damage(tmp.path(), catalog_leaf);
    let folder = Some(tmp.path());
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    for name in ["f", "absent"] {
        let error = write.open_fixed_tree(name, 2049).err().unwrap();
        assert_eq!(value_size(error, folder), OVER, "{name}");
        for mode in MODES {
            let error = write.open_fixed_tree_with(name, 2049, mode).err().unwrap();
            assert_eq!(value_size(error, folder), OVER, "{name} {mode:?}");
        }
    }
    let error = write.open_fixed_tree(&long, 2049).err().unwrap();
    assert_eq!(name_size(error, folder), LONG_NAME);
    write.commit().unwrap();
    let read = store.read().unwrap();
    let error = read.open_fixed_tree("f", 2049).err().unwrap();
    assert_eq!(value_size(error, folder), OVER);
    let error = read.open_fixed_tree("f", 4).err().unwrap();
    assert_eq!(damaged_page(error), catalog_leaf, "the damage is there");
    drop(read);
    store.close().unwrap();
}

/// The trees of AC-X20: a normal `n`, a multi-value `m`, and fixed-size `z` of length 0 and `f` of length 8.
fn kind_base(store: &Store) {
    commit_tree(store, "n", &[(b"k", b"v")]);
    commit_multi(store, "m", &Model::from([(b"k".to_vec(), set_of(&[b"v"]))]));
    commit_fixed(store, "z", 0, &fixed_model([1], 0));
    commit_fixed(store, "f", 8, &fixed_model(0..600, 8));
}

/// Checks that Tipo in `$tx` gives the kind of every tree of its list and nothing for `absent`, and that a name out
/// of its sizes fails with the invalid size of a name.
macro_rules! check_tree_kinds {
    ($tx:expr, $folder:expr) => {{
        let (tx, folder) = (&$tx, $folder);
        let list = tx.list_trees().unwrap();
        for (name, kind) in &list {
            assert_eq!(tx.tree_kind(name).unwrap(), Some(*kind), "{name}");
        }
        assert_eq!(tx.tree_kind("absent").unwrap(), None);
        assert_eq!(
            name_size(tx.tree_kind("").unwrap_err(), folder),
            (0, 1, 1024)
        );
        let long = "x".repeat(1025);
        assert_eq!(
            name_size(tx.tree_kind(&long).unwrap_err(), folder),
            LONG_NAME
        );
        list
    }};
}

/// AC-X20 (X15, X8, T10): Tipo gives, in a read and in a write, the kind of a normal tree, a multi-value one and two
/// fixed-size ones of lengths `0` and `8`, with their lengths, as Listar does; a name without a tree, also one deleted
/// in the same write, gives none without failing; after a rename, and after deleting a tree and creating it again with
/// another kind in the same write, it gives the current kind under the new name; an empty name or one of 1,025 bytes
/// fails without effect with the invalid size of a name; with a page of one of the trees damaged and the catalog
/// intact (on disk) it does not fail; with the catalog's page damaged (on disk) it fails with damage, and with a name
/// of 1,025 bytes with the invalid size; and a write on disk that only asks for kinds leaves the files byte-identical.
/// The part of a store that refuses writes (T12) is in the library's tests, and that it keeps references in
/// `tests/ui/trees_pass`.
#[test]
fn ac_x20_tipo_gives_the_kind_of_a_tree_by_its_name() {
    let kinds = [
        ("f", fixed(8)),
        ("m", MULTI),
        ("n", NORMAL),
        ("z", fixed(0)),
    ]
    .map(|(n, k)| (n.to_owned(), k));
    stores(|store, folder| {
        kind_base(store);
        let read = store.read().unwrap();
        assert_eq!(check_tree_kinds!(read, folder), kinds);
        drop(read);

        let mut write = store.write().unwrap();
        assert_eq!(check_tree_kinds!(write, folder), kinds);
        write.delete_tree("z").unwrap();
        assert_eq!(write.tree_kind("z").unwrap(), None, "deleted");
        write.rename_tree("f", "g", false).unwrap();
        assert_eq!(write.tree_kind("f").unwrap(), None, "renamed");
        assert_eq!(write.tree_kind("g").unwrap(), Some(fixed(8)));
        write.delete_tree("n").unwrap();
        create_fixed(&mut write, "n", 16);
        write.delete_tree("m").unwrap();
        create(&mut write, "m");
        let now =
            [("g", fixed(8)), ("m", NORMAL), ("n", fixed(16))].map(|(n, k)| (n.to_owned(), k));
        assert_eq!(check_tree_kinds!(write, folder), now);
        write.commit().unwrap();

        let read = store.read().unwrap();
        assert_eq!(check_tree_kinds!(read, folder), now);
    });

    // A damaged page of a tree, then the catalog's page damaged.
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    kind_base(&store);
    store.close().unwrap();
    for name in ["f", "m", "n", "z"] {
        damage(tmp.path(), root_page(tmp.path(), name));
    }
    let folder = Some(tmp.path());
    let store = open(tmp.path());
    let read = store.read().unwrap();
    assert_eq!(check_tree_kinds!(read, folder), kinds);
    drop(read);
    let write = store.write().unwrap();
    assert_eq!(check_tree_kinds!(write, folder), kinds);
    write.commit().unwrap();
    store.close().unwrap();
    let catalog_leaf = leaf_page(tmp.path(), None, b"f");
    damage(tmp.path(), catalog_leaf);
    let store = open(tmp.path());
    let read = store.read().unwrap();
    assert_eq!(damaged_page(read.tree_kind("f").unwrap_err()), catalog_leaf);
    assert_eq!(
        damaged_page(read.tree_kind("absent").unwrap_err()),
        catalog_leaf
    );
    let long = "x".repeat(1025);
    assert_eq!(
        name_size(read.tree_kind(&long).unwrap_err(), folder),
        LONG_NAME
    );
    drop(read);
    let write = store.write().unwrap();
    assert_eq!(
        damaged_page(write.tree_kind("f").unwrap_err()),
        catalog_leaf
    );
    assert_eq!(
        name_size(write.tree_kind(&long).unwrap_err(), folder),
        LONG_NAME
    );
    write.abort();
    store.close().unwrap();

    // A write that only asks for kinds writes nothing.
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    kind_base(&store);
    store.close().unwrap();
    let changed = files_change(tmp.path(), |store| {
        let write = store.write().unwrap();
        assert_eq!(check_tree_kinds!(write, Some(tmp.path())), kinds);
        write.commit().unwrap();
    });
    assert!(!changed, "asking for kinds writes nothing");
}

/// The length of the values of the tree of AC-X10: 113 entries fill a leaf.
const DAMAGE_LEN: usize = 64;

/// The tree of AC-X10: the multiples of 3 below 1,200, 400 keys added in order, so that they fill four leaves under a
/// root branch, the first key of each leaf being its separator in the root.
fn damage_model() -> FixedModel {
    fixed_model((0..400).map(|i| i * 3), DAMAGE_LEN)
}

/// The key of AC-X10 whose leaf is damaged: one in the second leaf.
const HIT: u64 = 600;

/// A closed store on disk with the normal tree `other` and the fixed-size tree `f` of [`damage_model`], with the leaf
/// of [`HIT`] damaged; and that page with the keys of the model in it, in order.
fn damaged_fixed_store() -> (tempfile::TempDir, u64, Vec<u64>) {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_tree(&store, "other", &[(b"k", b"v")]);
    commit_fixed(&store, "f", DAMAGE_LEN, &damage_model());
    store.close().unwrap();
    assert_eq!(fixed_height(tmp.path(), "f", DAMAGE_LEN), 2, "two levels");
    let (page, in_the_leaf) = inspect(tmp.path(), |read| {
        let root = catalog::lookup(read, "f").unwrap().unwrap().root;
        let leaf = |key: u64| krummi_tree::fixed::leaf_of(read, root, DAMAGE_LEN, key).unwrap();
        let page = leaf(HIT);
        assert_ne!(page, root, "a leaf below the root");
        let keys: Vec<u64> = damage_model()
            .into_keys()
            .filter(|&key| leaf(key) == page)
            .collect();
        (page, keys)
    });
    damage(tmp.path(), page);
    (tmp, page, in_the_leaf)
}

/// Checks that a new read sees the trees of [`damaged_fixed_store`] as they were: their list, their kinds, the number
/// of entries and Leer of every key of `model` outside the damaged leaf, whose keys are `skip`.
fn unchanged(store: &Store, model: &FixedModel, skip: &[u64]) {
    let read = store.read().unwrap();
    let kinds = vec![
        ("f".to_owned(), fixed(DAMAGE_LEN as u16)),
        ("other".to_owned(), NORMAL),
    ];
    assert_eq!(read.list_trees().unwrap(), kinds);
    let tree = read.open_fixed_tree("f", DAMAGE_LEN).unwrap();
    assert_eq!(read.len(&tree).unwrap(), model.len() as u64);
    for (key, value) in model.iter().filter(|(key, _)| !skip.contains(key)) {
        assert_eq!(
            read.get_fixed(&tree, *key).unwrap(),
            Some(&value[..]),
            "{key}"
        );
    }
}

/// Checks that the catalog operations of X10 on the tree `f` of [`damaged_fixed_store`], in the transaction `$tx`,
/// never fail with the damage of its pages and give their usual results: Open, Exists, Tipo, Listar and the number of
/// entries `$entries`.
macro_rules! catalog_reads_work {
    ($tx:expr, $entries:expr) => {{
        let tx = &$tx;
        let tree = tx.open_fixed_tree("f", DAMAGE_LEN).unwrap();
        assert_eq!(tx.len(&tree).unwrap(), $entries);
        assert!(tx.tree_exists("f").unwrap());
        let kind = fixed(DAMAGE_LEN as u16);
        assert_eq!(tx.tree_kind("f").unwrap(), Some(kind));
        let list = tx.list_trees().unwrap();
        assert_eq!(list, [("f".to_owned(), kind), ("other".to_owned(), NORMAL)]);
    }};
}

/// Checks that every operation of the transaction `$tx` on the keys of the damaged leaf `$page` fails with it, twice:
/// Leer of `$present`, which is in it, and of `$absent`, which would be; and scans of the whole tree and of an
/// interval around the leaf, which return the entries of `$model` before it from either end before failing with it.
macro_rules! key_reads_fail {
    ($tx:expr, $tree:expr, $model:expr, $present:expr, $absent:expr, $page:expr) => {{
        let (tx, tree, page) = (&$tx, &$tree, $page);
        let all = entries(&$model);
        let around: Vec<(u64, Vec<u8>)> = all
            .iter()
            .filter(|(key, _)| (HIT - 300..=HIT + 300).contains(key))
            .cloned()
            .collect();
        for _ in 0..2 {
            for key in [$present, $absent] {
                assert_eq!(damaged_page(tx.get_fixed(tree, key).unwrap_err()), page);
            }
            for front in [true, false] {
                let scan = tx.range_fixed(tree, ..).unwrap();
                scan_fails_at(scan, &all, own, front, page);
                let scan = tx.range_fixed(tree, HIT - 300..=HIT + 300).unwrap();
                scan_fails_at(scan, &around, own, front, page);
            }
        }
    }};
}

/// AC-X10 (X10, A9, R8, G6, F9), on disk, with a damaged leaf of a fixed-size tree of two levels on the way to some
/// keys: Leer, Guardar, Añadir and Quitar of a key of that leaf that is there and of one that is not, the scans and
/// Quitar intervalo of intervals that hold them fail with damage and that page, again when repeated, also in a new
/// scan of another read; a scan returns the right entries before that leaf from each end, then fails and ends; a
/// write that failed cannot be committed and applies nothing (T11); the modes Open, CreateNew and OpenOrCreate,
/// Exists, Tipo, Listar, Rename without overwrite or to its own name and the number of entries never fail with that
/// damage and give their usual result or error (CreateNew: tree already exists); the keys whose path does not go
/// through that leaf are read and modified, and scans and range removals whose bounds stop at the separator next to
/// the damaged leaf, from the leaf before it and from the leaf after it, never read it (finding R3-002 of the first
/// RDD review); and emptying, deleting or overwriting the tree with Rename fails with damage and that page, and the
/// write follows T11.
#[test]
fn ac_x10_a_damaged_leaf_on_the_way_to_some_keys() {
    let mut model = damage_model();
    let (tmp, page, in_the_leaf) = damaged_fixed_store();
    let (first, last) = (in_the_leaf[0], *in_the_leaf.last().unwrap());
    assert!(first > 0 && last < 1197, "a leaf between two others");
    let absent = HIT + 1;
    let store = open(tmp.path());
    unchanged(&store, &model, &in_the_leaf);

    let read = store.read().unwrap();
    catalog_reads_work!(read, 400);
    let tree = read.open_fixed_tree("f", DAMAGE_LEN).unwrap();
    key_reads_fail!(read, tree, model, HIT, absent, page);
    drop(read);
    let read = store.read().unwrap();
    let tree = read.open_fixed_tree("f", DAMAGE_LEN).unwrap();
    let scan = read.range_fixed(&tree, ..).unwrap();
    scan_fails_at(scan, &entries(&model), own, true, page);
    drop(read);

    // The catalog operations of the tree work in a write, which commits.
    let folder = Some(tmp.path());
    let mut write = store.write().unwrap();
    catalog_reads_work!(write, 400);
    write
        .open_fixed_tree_with("f", DAMAGE_LEN, OpenMode::OpenOrCreate)
        .unwrap();
    let error = write
        .open_fixed_tree_with("f", DAMAGE_LEN, OpenMode::CreateNew)
        .err()
        .unwrap();
    assert_eq!(already_exists(error, folder), "f");
    write.rename_tree("f", "f", false).unwrap();
    write.rename_tree("f", "f", true).unwrap();
    let error = write.rename_tree("f", "other", false).unwrap_err();
    assert_eq!(already_exists(error, folder), "other");
    write.rename_tree("f", "g", false).unwrap();
    assert_eq!(
        write.tree_kind("g").unwrap(),
        Some(fixed(DAMAGE_LEN as u16))
    );
    write.rename_tree("g", "f", false).unwrap();
    catalog_reads_work!(write, 400);
    write.commit().unwrap();
    unchanged(&store, &model, &in_the_leaf);

    // The operations on the keys of the leaf fail, the write goes on, and its commit fails with the first failure.
    let mut write = store.write().unwrap();
    let tree = write.open_fixed_tree("f", DAMAGE_LEN).unwrap();
    let value = [0xAB; DAMAGE_LEN];
    for _ in 0..2 {
        for key in [HIT, absent] {
            let error = write.set_fixed(&tree, key, &value).unwrap_err();
            assert_eq!(damaged_page(error), page, "set {key}");
            let error = write.try_add_fixed(&tree, key, &value).unwrap_err();
            assert_eq!(damaged_page(error), page, "try_add {key}");
            let error = write.remove_fixed(&tree, key).unwrap_err();
            assert_eq!(damaged_page(error), page, "remove {key}");
        }
    }
    key_reads_fail!(write, tree, model, HIT, absent, page);
    assert_eq!(write.len(&tree).unwrap(), 400);
    // Last, because a range removal that fails may have changed the leaves at its bounds in the write, whose content
    // is then left to the implementation (it cannot commit, T11).
    for _ in 0..2 {
        for bounds in [
            (Included(HIT), Included(HIT)),
            (Included(absent), Excluded(absent + 1)),
            (Excluded(first - 3), Excluded(last + 3)),
            (Included(0), Included(1197)),
            (Unbounded, Unbounded),
        ] {
            let error = write.remove_range_fixed(&tree, bounds).unwrap_err();
            assert_eq!(damaged_page(error), page, "remove_range {bounds:?}");
        }
    }
    assert_eq!(damaged_page(write.commit().unwrap_err()), page, "T11");
    unchanged(&store, &model, &in_the_leaf);

    // Bounds that stop at the separators next to the leaf: before it, its first key; after it, the first key of the
    // next leaf. Neither scans nor range removals read the damaged leaf. (A bound between the leaf's last key and the
    // next separator is within the leaf's part of the tree, which must be read.)
    let next = last + 3;
    let near = [
        (Unbounded, Excluded(first)),
        (Unbounded, Included(first - 1)),
        (Included(first - 30), Excluded(first)),
        (Included(first - 9), Included(first - 1)),
        (Included(next), Unbounded),
        (Excluded(next - 1), Unbounded),
        (Included(next), Included(next + 30)),
    ];
    let read = store.read().unwrap();
    let tree = read.open_fixed_tree("f", DAMAGE_LEN).unwrap();
    check_scans!(read, tree, model, near);
    drop(read);
    let mut write = store.write().unwrap();
    let tree = write.open_fixed_tree("f", DAMAGE_LEN).unwrap();
    check_scans!(write, tree, model, near);
    for bounds in [
        (Included(first - 9), Excluded(first)),
        (Included(next), Included(next + 6)),
    ] {
        assert_eq!(write.remove_range_fixed(&tree, bounds).unwrap(), 3);
        model.retain(|key, _| !within(*key, bounds));
    }
    check_scans!(write, tree, model, near);
    write.commit().unwrap();
    unchanged(&store, &model, &in_the_leaf);

    // The keys of the other leaves: Leer, Guardar, Añadir and Quitar.
    let mut write = store.write().unwrap();
    let tree = write.open_fixed_tree("f", DAMAGE_LEN).unwrap();
    for key in [0, first - 30, next + 30, 1197] {
        assert_eq!(write.get_fixed(&tree, key).unwrap(), Some(&model[&key][..]));
        write.set_fixed(&tree, key, &value).unwrap();
        model.insert(key, value.to_vec());
        assert!(write.try_add_fixed(&tree, key + 1, &value).unwrap());
        assert!(!write.try_add_fixed(&tree, key + 1, &value).unwrap());
        model.insert(key + 1, value.to_vec());
        assert_eq!(write.remove_fixed(&tree, key + 3).unwrap(), key < 1197);
        model.remove(&(key + 3));
        assert_eq!(write.get_fixed(&tree, key).unwrap(), Some(&value[..]));
    }
    assert_eq!(write.len(&tree).unwrap(), model.len() as u64);
    write.commit().unwrap();
    unchanged(&store, &model, &in_the_leaf);

    for operation in [
        "create or empty",
        "empty",
        "delete",
        "overwrite with rename",
    ] {
        let mut write = store.write().unwrap();
        let error = match operation {
            "create or empty" => write
                .open_fixed_tree_with("f", DAMAGE_LEN, OpenMode::CreateOrEmpty)
                .err()
                .unwrap(),
            "empty" => write
                .open_fixed_tree_with("f", DAMAGE_LEN, OpenMode::Empty)
                .err()
                .unwrap(),
            "delete" => write.delete_tree("f").unwrap_err(),
            _ => write.rename_tree("other", "f", true).unwrap_err(),
        };
        assert_eq!(damaged_page(error), page, "{operation}");
        let error = write.commit().unwrap_err();
        assert_eq!(damaged_page(error), page, "{operation}: T11");
        unchanged(&store, &model, &in_the_leaf);
    }
    store.close().unwrap();
}

/// The length of the values of AC-X11: eight entries fill a leaf, and a leaf of one or two is under a quarter full,
/// so that removals merge leaves.
const SNAPSHOT_LEN: usize = 1000;

/// AC-X11 (G3, T14, AC-B17): a read opened before a write whose insertions and removals split and merge pages of a
/// fixed-size tree until its height changes sees exactly its snapshot, its entries and their number, through a handle
/// opened before the write and one opened after, before and after the commit; and a new read sees the write's. The
/// tree grows from one leaf to two levels by both kinds of split (past a leaf's last key and into halves), shrinks to
/// two entries, which merges its leaves back into one, and grows again; on disk, its heights before and after are
/// checked.
#[test]
fn ac_x11_a_read_keeps_its_snapshot_while_a_write_splits_and_merges() {
    let scenario = |store: &Store| {
        let snapshot = fixed_model([10, 20, 30], SNAPSHOT_LEN);
        commit_fixed(store, "f", SNAPSHOT_LEN, &snapshot);
        let read = store.read().unwrap();
        let before = read.open_fixed_tree("f", SNAPSHOT_LEN).unwrap();
        check_fixed!(read, before, snapshot);
        let check_snapshot = || {
            let tree = read.open_fixed_tree("f", SNAPSHOT_LEN).unwrap();
            check_fixed!(read, tree, snapshot);
            check_fixed!(read, before, snapshot);
        };
        let mut model = snapshot.clone();
        let mut write = store.write().unwrap();
        let tree = write.open_fixed_tree("f", SNAPSHOT_LEN).unwrap();
        let grow =
            |write: &mut krummi::WriteTransaction<'_>, model: &mut FixedModel, keys: &[u64]| {
                for &key in keys {
                    let value = fixed_value(key, SNAPSHOT_LEN);
                    write.set_fixed(&tree, key, &value).unwrap();
                    model.insert(key, value);
                }
            };
        let keys: Vec<u64> = (31..=120).chain((0..10).rev()).collect();
        grow(&mut write, &mut model, &keys);
        check_fixed!(write, tree, model);
        check_snapshot();
        let gone: Vec<u64> = model
            .keys()
            .copied()
            .filter(|&k| k != 0 && k != 100)
            .collect();
        for key in gone {
            assert!(write.remove_fixed(&tree, key).unwrap());
            model.remove(&key);
        }
        check_fixed!(write, tree, model);
        check_snapshot();
        let keys: Vec<u64> = (200..=300).collect();
        grow(&mut write, &mut model, &keys);
        check_fixed!(write, tree, model);
        check_snapshot();
        write.commit().unwrap();
        check_snapshot();
        check_read(store, "f", SNAPSHOT_LEN, &model);
    };
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_fixed(&store, "f", SNAPSHOT_LEN, &fixed_model([10], SNAPSHOT_LEN));
    store.close().unwrap();
    assert_eq!(fixed_height(tmp.path(), "f", SNAPSHOT_LEN), 1, "before");
    let store = open(tmp.path());
    scenario(&store);
    store.close().unwrap();
    assert_eq!(fixed_height(tmp.path(), "f", SNAPSHOT_LEN), 2, "after");

    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    scenario(&store);
    store.close().unwrap();
}

/// AC-X18 (X8, T10): a write on disk that only makes one change, in separate writes, Guardar with the value the key
/// already had, emptying an empty tree, Añadir of a new key, Quitar of a key that is there or Quitar intervalo that
/// removes entries, changes the store's files once committed, as any write with changes does.
#[test]
fn ac_x18_a_write_with_one_change_changes_the_files() {
    let model = fixed_model((0..1500).map(|i| i * 3), 8);
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_fixed(&store, "f", 8, &model);
    commit_fixed(&store, "e", 8, &FixedModel::new());
    store.close().unwrap();
    assert!(fixed_height(tmp.path(), "f", 8) >= 2);
    for change in [
        "set with the same value",
        "empty an empty tree",
        "add a new key",
        "remove a key that is there",
        "remove a range",
    ] {
        let changed = files_change(tmp.path(), |store| {
            let mut write = store.write().unwrap();
            let tree = write.open_fixed_tree("f", 8).unwrap();
            match change {
                "set with the same value" => write.set_fixed(&tree, 300, &model[&300]).unwrap(),
                "empty an empty tree" => {
                    let empty = write.open_fixed_tree_with("e", 8, OpenMode::Empty).unwrap();
                    assert_eq!(write.len(&empty).unwrap(), 0);
                }
                "add a new key" => assert!(write.try_add_fixed(&tree, 301, &[1; 8]).unwrap()),
                "remove a key that is there" => assert!(write.remove_fixed(&tree, 600).unwrap()),
                _ => assert_eq!(write.remove_range_fixed(&tree, 900..=903).unwrap(), 2),
            }
            write.commit().unwrap();
        });
        assert!(changed, "{change}: a write with a change writes");
    }
}
