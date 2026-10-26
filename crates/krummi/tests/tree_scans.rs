//! In-order scans, prefix scans and range removal through the public API (In-order scan and prefix search PRD:
//! R1–R11; AC-R1–AC-R7, AC-R9–AC-R11, AC-R13 for damaged pages; Trees PRD: AC-B7 in its 5.2 part; root PRD: AC20
//! with scans). Crashes, power cuts and altered bytes (AC-R12, root AC10) are in `tree_integrity.rs`, running out of
//! memory (AC-R12, AC-R13) in `tree_memory.rs`, the failures of T11 and of input and output in the middle of a range
//! removal or a scan (AC-R12, AC-R13) in the library's own tests, and what must not compile (AC-R8) in
//! `trees_compile.rs`.
//!
//! Each criterion runs on a store on disk and on a temporary one, except the parts that reopen or damage the
//! store's files, which run on disk only. Expected results come from `std::collections::BTreeMap`, whose order
//! is the same byte order (B5).

mod support;

use std::collections::BTreeMap;
use std::ops::Bound::{self, Excluded, Included, Unbounded};

use krummi::{OpenMode, ReadRange, Result, SizeSubject, Store, WriteRange, WriteTransaction};
use support::*;

type Entries = Vec<(Vec<u8>, Vec<u8>)>;
type Bounds<'a> = (Bound<&'a [u8]>, Bound<&'a [u8]>);
/// A row of the *Salida* table: the prefix, the bounds, whether from the front end, and the keys.
type Row<'a> = (&'a [u8], Bounds<'a>, bool, &'a [&'a [u8]]);
/// A size case: the prefix, the bounds and the expected subject, received size, minimum and maximum.
type SizeCase<'a> = (&'a [u8], Bounds<'a>, (SizeSubject, u64, u64, u64));

/// Every entry a scan returns from its front end, unwrapped.
fn forward<'t>(scan: impl Iterator<Item = Result<(&'t [u8], &'t [u8])>>) -> Entries {
    scan.map(|entry| {
        let (key, value) = entry.unwrap();
        (key.to_vec(), value.to_vec())
    })
    .collect()
}

/// Every key a scan returns from its back end, unwrapped.
fn backward_keys<'t>(
    scan: impl DoubleEndedIterator<Item = Result<(&'t [u8], &'t [u8])>>,
) -> Vec<Vec<u8>> {
    scan.rev().map(|entry| entry.unwrap().0.to_vec()).collect()
}

fn keys(entries: &Entries) -> Vec<Vec<u8>> {
    entries.iter().map(|(key, _)| key.clone()).collect()
}

/// The value stored under `key` in these tests.
fn value_of(key: &[u8]) -> Vec<u8> {
    let mut value = b"v-".to_vec();
    value.extend_from_slice(&key[..key.len().min(MAX_VALUE - 2)]);
    value
}

/// Commits the tree `name` with `keys`, each with [`value_of`], and returns the model of its entries.
fn fill(store: &Store, name: &str, keys: &[Vec<u8>]) -> BTreeMap<Vec<u8>, Vec<u8>> {
    let entries: Vec<_> = keys.iter().map(|k| (k.clone(), value_of(k))).collect();
    commit_tree(store, name, &entries);
    entries.into_iter().collect()
}

/// The entries of `model` within `bounds` and starting with `prefix`, in order.
fn expected(model: &BTreeMap<Vec<u8>, Vec<u8>>, prefix: &[u8], bounds: Bounds<'_>) -> Entries {
    let (lower, upper) = (bounds.0.map(<[u8]>::to_vec), bounds.1.map(<[u8]>::to_vec));
    model
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .filter(|(k, _)| {
            let after_lower = match &lower {
                Included(l) => *k >= l,
                Excluded(l) => *k > l,
                Unbounded => true,
            };
            let before_upper = match &upper {
                Included(u) => *k <= u,
                Excluded(u) => *k < u,
                Unbounded => true,
            };
            after_lower && before_upper
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn read_scan<'t>(
    read: &'t krummi::ReadTransaction<'_>,
    tree: &krummi::ReadTree<'_>,
    prefix: &[u8],
    bounds: Bounds<'_>,
) -> ReadRange<'t> {
    read.range_prefix(tree, prefix, bounds).unwrap()
}

/// A simple deterministic generator, so that random orders repeat.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
}

const EXAMPLE: [&[u8]; 5] = [b"a", b"ab", b"b", b"ba", b"c"];

/// AC-R1: the scans of the *Salida* table, the alternation of both ends and the two range removals.
#[test]
fn ac_r1_the_output_examples() {
    let rows: [Row<'_>; 11] = [
        (b"", (Included(b"ab"), Included(b"b")), true, &[b"ab", b"b"]),
        (
            b"",
            (Included(b"ab"), Included(b"b")),
            false,
            &[b"b", b"ab"],
        ),
        (
            b"",
            (Excluded(b"a"), Included(b"ba")),
            true,
            &[b"ab", b"b", b"ba"],
        ),
        (
            b"",
            (Excluded(b"aa"), Unbounded),
            true,
            &[b"ab", b"b", b"ba", b"c"],
        ),
        (b"", (Unbounded, Excluded(b"b")), false, &[b"ab", b"a"]),
        (b"b", (Unbounded, Unbounded), true, &[b"b", b"ba"]),
        (b"b", (Unbounded, Unbounded), false, &[b"ba", b"b"]),
        (b"a", (Included(b"aa"), Unbounded), true, &[b"ab"]),
        (
            b"",
            (Unbounded, Unbounded),
            true,
            &[b"a", b"ab", b"b", b"ba", b"c"],
        ),
        (b"", (Included(b"c"), Included(b"a")), true, &[]),
        (b"d", (Unbounded, Unbounded), true, &[]),
    ];
    stores(|store, _| {
        let model = fill(store, "t", &EXAMPLE.map(<[u8]>::to_vec));
        let read = store.read().unwrap();
        let tree = tree_t(&read);
        for (prefix, bounds, front, want) in rows {
            let scan = read_scan(&read, &tree, prefix, bounds);
            let got = if front {
                keys(&forward(scan))
            } else {
                backward_keys(scan)
            };
            let want: Vec<Vec<u8>> = want.iter().map(|k| k.to_vec()).collect();
            assert_eq!(
                got, want,
                "prefix {prefix:?}, bounds {bounds:?}, front {front}"
            );
            if front {
                assert_eq!(
                    forward(read_scan(&read, &tree, prefix, bounds)),
                    expected(&model, prefix, bounds)
                );
            }
        }
        // Inverted bounds return nothing from either end.
        assert_eq!(
            read_scan(&read, &tree, b"", (Included(b"c"), Included(b"a")))
                .next_back()
                .map(|e| e.unwrap()),
            None
        );
        assert_eq!(
            read_scan(&read, &tree, b"d", (Unbounded, Unbounded))
                .next_back()
                .map(|e| e.unwrap()),
            None
        );

        // Alternating ends, starting with the front, until both meet.
        let mut scan = read.range(&tree, ..).unwrap();
        let mut got = Vec::new();
        for turn in 0..5 {
            let entry = if turn % 2 == 0 {
                scan.next()
            } else {
                scan.next_back()
            };
            got.push(entry.unwrap().unwrap().0.to_vec());
        }
        assert_eq!(
            got,
            [&b"a"[..], b"c", b"ab", b"ba", b"b"].map(<[u8]>::to_vec)
        );
        assert!(scan.next().is_none() && scan.next_back().is_none() && scan.next().is_none());
    });
}

/// Opens the tree `t` of the tests in a read.
fn tree_t<'s>(read: &krummi::ReadTransaction<'s>) -> krummi::ReadTree<'s> {
    read.open_tree("t", NORMAL).unwrap()
}

/// `count` keys of `len` bytes that sort by their number.
fn numbered(count: usize, len: usize) -> Vec<Vec<u8>> {
    (0..count).map(|i| long_key(i, len)).collect()
}

/// AC-R2: a tree of several levels, scanned whole in both directions and alternating at random, and from bounds at
/// every existing key, between every two neighbours, and before the first and after the last key.
#[test]
fn ac_r2_scans_of_a_tree_of_several_levels() {
    stores(|store, _| {
        let all = numbered(3000, 200);
        let model = fill(store, "t", &all);
        let read = store.read().unwrap();
        let tree = tree_t(&read);
        assert_eq!(read.len(&tree).unwrap(), 3000);
        let whole = forward(read.range(&tree, ..).unwrap());
        assert_eq!(whole, model.clone().into_iter().collect::<Entries>());
        let mut reversed = keys(&whole);
        reversed.reverse();
        assert_eq!(backward_keys(read.range(&tree, ..).unwrap()), reversed);

        let mut random = Lcg(7);
        let mut scan = read.range(&tree, ..).unwrap();
        let (mut front, mut back) = (Vec::new(), Vec::new());
        loop {
            let entry = if random.next().is_multiple_of(2) {
                scan.next().map(|e| front.push(e.unwrap().0.to_vec()))
            } else {
                scan.next_back().map(|e| back.push(e.unwrap().0.to_vec()))
            };
            if entry.is_none() {
                break;
            }
        }
        assert!(scan.next().is_none() && scan.next_back().is_none());
        back.reverse();
        front.extend(back);
        assert_eq!(front, keys(&whole), "every key once, in order");

        // A key between key i and key i + 1, and keys before the first and after the last.
        let between = |i: usize| {
            let mut key = long_key(i, 200);
            key.push(0x01);
            key
        };
        let mut probes: Vec<Vec<u8>> = vec![vec![0x00], vec![0xFF]];
        for (i, key) in all.iter().enumerate() {
            probes.push(key.clone());
            probes.push(between(i));
        }
        for probe in &probes {
            for lower in [Included(&probe[..]), Excluded(&probe[..])] {
                let want = model
                    .range::<[u8], _>((lower, Unbounded))
                    .next()
                    .map(|(k, v)| (k.clone(), v.clone()));
                let got = read
                    .range(&tree, (lower, Unbounded))
                    .unwrap()
                    .next()
                    .map(|e| {
                        let (k, v) = e.unwrap();
                        (k.to_vec(), v.to_vec())
                    });
                assert_eq!(got, want, "front from {lower:?}");
            }
            for upper in [Included(&probe[..]), Excluded(&probe[..])] {
                let want = model
                    .range::<[u8], _>((Unbounded, upper))
                    .next_back()
                    .map(|(k, v)| (k.clone(), v.clone()));
                let got = read
                    .range(&tree, (Unbounded, upper))
                    .unwrap()
                    .next_back()
                    .map(|e| {
                        let (k, v) = e.unwrap();
                        (k.to_vec(), v.to_vec())
                    });
                assert_eq!(got, want, "back from {upper:?}");
            }
        }
    });
}

/// AC-R3: prefixes with `0xFF` bytes, the empty prefix, a prefix of 2,048 bytes equal to a key and a prefix with no
/// keys.
#[test]
fn ac_r3_prefixes() {
    stores(|store, _| {
        let mut long = vec![b'k'; MAX_KEY];
        let mut sibling = long.clone();
        sibling[MAX_KEY - 1] = b'l';
        let mut all: Vec<Vec<u8>> = (0..=255u8).map(|b| vec![b]).collect();
        all.extend([
            vec![0xFF, 0xFF],
            vec![0xFF, 0xFF, 0x00],
            vec![0xFF, 0xFF, 0xFF],
            vec![0xFF, 0xFE, 0xFF],
        ]);
        all.push(long.clone());
        all.push(sibling);
        let model = fill(store, "t", &all);
        let read = store.read().unwrap();
        let tree = tree_t(&read);
        let ff = [
            vec![0xFF, 0xFF],
            vec![0xFF, 0xFF, 0x00],
            vec![0xFF, 0xFF, 0xFF],
        ];
        assert_eq!(
            keys(&forward(
                read.range_prefix(&tree, &[0xFF, 0xFF], ..).unwrap()
            )),
            ff
        );
        let mut reversed = ff.to_vec();
        reversed.reverse();
        assert_eq!(
            backward_keys(read.range_prefix(&tree, &[0xFF, 0xFF], ..).unwrap()),
            reversed
        );
        assert_eq!(
            forward(read.range_prefix(&tree, b"", ..).unwrap()),
            model.clone().into_iter().collect::<Entries>()
        );
        assert_eq!(
            keys(&forward(read.range_prefix(&tree, &long, ..).unwrap())),
            [long.clone()]
        );
        long[MAX_KEY - 1] = b'z';
        assert!(forward(read.range_prefix(&tree, &long, ..).unwrap()).is_empty());
        assert!(forward(read.range_prefix(&tree, b"zz", ..).unwrap()).is_empty());
    });
}

/// AC-B7 (5.2 part): keys stored in random order, with bytes from `0x00` to `0xFF` and keys that are prefixes of
/// others, come out in unsigned byte order, each prefix before its extensions.
#[test]
fn ac_b7_keys_come_out_in_unsigned_byte_order() {
    stores(|store, _| {
        let mut all: Vec<Vec<u8>> = Vec::new();
        for b in 0..=255u8 {
            all.push(vec![b]);
            all.push(vec![b, 0x00]);
            all.push(vec![b, 0xFF, b]);
        }
        let mut random = Lcg(42);
        for i in (1..all.len()).rev() {
            let j = (random.next() as usize) % (i + 1);
            all.swap(i, j);
        }
        let mut write = store.write().unwrap();
        let tree = write
            .open_tree_with("t", NORMAL, OpenMode::CreateNew)
            .unwrap();
        for key in &all {
            write.set(&tree, key, &value_of(key)).unwrap();
        }
        write.commit().unwrap();
        let read = store.read().unwrap();
        let tree = tree_t(&read);
        let got = keys(&forward(read.range(&tree, ..).unwrap()));
        let mut sorted = all.clone();
        sorted.sort();
        assert_eq!(got, sorted);
        for pair in got.windows(2) {
            assert!(pair[0] < pair[1]);
            if pair[1].starts_with(&pair[0]) {
                assert!(
                    pair[0].len() < pair[1].len(),
                    "a prefix goes before its extensions"
                );
            }
        }
    });
}

/// AC-R5 (scans; the removal part is in [`ac_r5_sizes_of_removals`]): bounds of 0 and 2,049 bytes and a prefix of
/// 2,049 bytes fail with invalid size, also on an empty tree and an inverted interval; bounds of 1 and 2,048 bytes
/// and prefixes of 0 and 2,048 bytes do not; the order is lower, upper, prefix; the transaction stays usable.
#[test]
fn ac_r5_sizes_of_scans() {
    stores(|store, folder| {
        commit_tree(store, "t", &[(b"k", b"v")]);
        commit_tree(store, "empty", EMPTY);
        let read = store.read().unwrap();
        let tree = tree_t(&read);
        let empty = read.open_tree("empty", NORMAL).unwrap();
        let mut write = store.write().unwrap();
        let wtree = write.open_tree("t", NORMAL).unwrap();
        let key = |len: usize| vec![b'k'; len];
        let (short, long, max, big_prefix) =
            (key(0), key(MAX_KEY + 1), key(MAX_KEY), key(MAX_KEY + 1));
        let cases: [SizeCase<'_>; 6] = [
            (
                b"",
                (Included(&short), Unbounded),
                (SizeSubject::Key, 0, 1, 2048),
            ),
            (
                b"",
                (Excluded(&long), Unbounded),
                (SizeSubject::Key, 2049, 1, 2048),
            ),
            (
                b"",
                (Unbounded, Included(&long)),
                (SizeSubject::Key, 2049, 1, 2048),
            ),
            (
                b"",
                (Unbounded, Excluded(&short)),
                (SizeSubject::Key, 0, 1, 2048),
            ),
            (
                &big_prefix,
                (Unbounded, Unbounded),
                (SizeSubject::Prefix, 2049, 0, 2048),
            ),
            // Both bounds: the lower one; the upper bound and the prefix: the upper one.
            (
                b"",
                (Included(&short), Included(&long)),
                (SizeSubject::Key, 0, 1, 2048),
            ),
        ];
        let more: [SizeCase<'_>; 3] = [
            (
                &big_prefix,
                (Unbounded, Included(&long)),
                (SizeSubject::Key, 2049, 1, 2048),
            ),
            (
                &big_prefix,
                (Included(&long), Unbounded),
                (SizeSubject::Key, 2049, 1, 2048),
            ),
            // An inverted interval does not skip the check.
            (
                b"",
                (Included(b"z"), Included(&short)),
                (SizeSubject::Key, 0, 1, 2048),
            ),
        ];
        for (prefix, bounds, want) in cases.iter().chain(more.iter()) {
            for target in [&tree, &empty] {
                let error = read
                    .range_prefix(target, prefix, *bounds)
                    .map(|_| ())
                    .unwrap_err();
                assert_eq!(invalid_size(error, folder), *want, "read, {bounds:?}");
            }
            let error = write
                .range_prefix(&wtree, prefix, *bounds)
                .map(|_| ())
                .unwrap_err();
            assert_eq!(invalid_size(error, folder), *want, "write, {bounds:?}");
        }
        // The limits themselves are valid.
        let one = key(1);
        for (prefix, bounds) in [
            (&b""[..], (Included(&one[..]), Included(&max[..]))),
            (&max[..], (Unbounded, Unbounded)),
            (&b""[..], (Excluded(&max[..]), Unbounded)),
        ] {
            assert!(
                read.range_prefix(&tree, prefix, bounds)
                    .unwrap()
                    .all(|e| e.is_ok())
            );
            assert!(
                write
                    .range_prefix(&wtree, prefix, bounds)
                    .unwrap()
                    .all(|e| e.is_ok())
            );
        }
        assert_eq!(read.get(&tree, b"k").unwrap(), Some(&b"v"[..]));
        write.set(&wtree, b"k2", b"v").unwrap();
        write.commit().unwrap();
    });
}

/// AC-R6 (scan part; the removal part is in [`ac_r6_other_handles_see_a_range_removal`]): a write's scan sees the
/// changes made before it.
#[test]
fn ac_r6_a_write_scan_sees_its_changes() {
    stores(|store, _| {
        fill(store, "t", &EXAMPLE.map(<[u8]>::to_vec));
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        write.set(&tree, b"aa", b"new").unwrap();
        write.remove(&tree, b"b").unwrap();
        write.set(&tree, b"c", b"changed").unwrap();
        let got = forward(write.range(&tree, ..).unwrap());
        let want: Entries = [
            (&b"a"[..], value_of(b"a")),
            (b"aa", b"new".to_vec()),
            (b"ab", value_of(b"ab")),
            (b"ba", value_of(b"ba")),
            (b"c", b"changed".to_vec()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_vec(), v))
        .collect();
        assert_eq!(got, want);
        assert_eq!(
            backward_keys(write.range_prefix(&tree, b"a", ..).unwrap()),
            [&b"ab"[..], b"aa", b"a"].map(<[u8]>::to_vec)
        );
        write.commit().unwrap();
    });
}

/// AC-R11 (scans): a scan through a handle of a deleted tree fails with tree not found and the tree's last name,
/// through a handle of an ended transaction with misuse; invalid size wins over both, and both win over damage.
#[test]
fn ac_r11_scans_through_deleted_and_foreign_handles() {
    stores(|store, folder| {
        fill(store, "t", &EXAMPLE.map(<[u8]>::to_vec));
        let mut write = store.write().unwrap();
        let first = write.open_tree("t", NORMAL).unwrap();
        let second = write.open_tree("t", NORMAL).unwrap();
        write.rename_tree("t", "renamed", false).unwrap();
        write.delete_tree("renamed").unwrap();
        let error = write.range(&second, ..).map(|_| ()).unwrap_err();
        assert_eq!(not_found(error, folder), "renamed");
        let error = write
            .range_prefix(&first, b"a", ..)
            .map(|_| ())
            .unwrap_err();
        assert_eq!(not_found(error, folder), "renamed");
        let long = vec![0; MAX_KEY + 1];
        let error = write
            .range(&second, (Included(&long[..]), Unbounded))
            .map(|_| ())
            .unwrap_err();
        assert_eq!(invalid_size(error, folder).0, SizeSubject::Key);
        write.abort();

        let old = store.read().unwrap();
        let foreign = tree_t(&old);
        drop(old);
        let read = store.read().unwrap();
        let error = read.range(&foreign, ..).map(|_| ()).unwrap_err();
        misuse(error, folder);
        let error = read
            .range(&foreign, (Unbounded, Excluded(&long[..])))
            .map(|_| ())
            .unwrap_err();
        assert_eq!(invalid_size(error, folder).0, SizeSubject::Key);
        assert_eq!(
            forward(read.range(&tree_t(&read), ..).unwrap()).len(),
            5,
            "the read stays usable"
        );
    });
}

/// AC-R11 with damage (disk only): misuse and tree not found win over a damaged page, which they never read.
#[test]
fn ac_r11_handle_failures_win_over_damage() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let all = numbered(60, 1000);
    fill(&store, "t", &all);
    store.close().unwrap();
    let leaf = leaf_page(tmp.path(), Some("t"), &all[30]);
    assert_ne!(
        leaf,
        root_page(tmp.path(), "t"),
        "the tree has several levels"
    );
    let root = root_page(tmp.path(), "t");

    // A handle of a deleted tree whose leaf is damaged (deleting reads only the branch pages).
    damage(tmp.path(), leaf);
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let tree = write.open_tree("t", NORMAL).unwrap();
    write.delete_tree("t").unwrap();
    let error = write.range(&tree, ..).map(|_| ()).unwrap_err();
    assert_eq!(not_found(error, Some(tmp.path())), "t");
    write.abort();
    store.close().unwrap();

    // A handle of another read, with the root damaged (the leaf repaired: flipping the byte again restores it).
    damage(tmp.path(), leaf);
    damage(tmp.path(), root);
    let store = open(tmp.path());
    let old = store.read().unwrap();
    let foreign = tree_t(&old);
    drop(old);
    let read = store.read().unwrap();
    misuse(
        read.range(&foreign, ..).map(|_| ()).unwrap_err(),
        Some(tmp.path()),
    );
    let mut scan = read.range(&tree_t(&read), ..).unwrap();
    assert_eq!(damaged_page(scan.next().unwrap().unwrap_err()), root);
}

/// AC-R13 (scans, disk only): a scan that needs a damaged leaf or a damaged branch fails with damage and its page
/// number, keeps the entries returned before, then ends at both ends; the read stays usable and a new scan fails
/// again; a scan far from the damaged page does not fail; in a write, the commit then fails with damage.
#[test]
fn ac_r13_scans_and_damaged_pages() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let all = numbered(3000, 200);
    let model = fill(&store, "t", &all);
    store.close().unwrap();
    let leaf = leaf_page(tmp.path(), Some("t"), &all[1500]);
    // The first and the last key of the damaged leaf.
    let in_leaf = |i: usize| leaf_page(tmp.path(), Some("t"), &all[i]) == leaf;
    let (mut first, mut last) = (1500, 1500);
    while in_leaf(first - 1) {
        first -= 1;
    }
    while in_leaf(last + 1) {
        last += 1;
    }
    damage(tmp.path(), leaf);
    let store = open(tmp.path());
    let read = store.read().unwrap();
    let tree = tree_t(&read);

    // Scans that end at the leaf's neighbours stop without reading it, from either end.
    let before = read.range(&tree, ..=&all[first - 1][..]).unwrap();
    assert_eq!(forward(before).len(), first);
    let after = read.range(&tree, &all[last + 1][..]..).unwrap();
    assert_eq!(backward_keys(after).len(), all.len() - last - 1);
    let mut both = read.range(&tree, ..=&all[first - 1][..]).unwrap();
    assert!(both.next_back().unwrap().is_ok());
    assert_eq!(forward(both).len(), first - 1);

    // Forward from the start: the entries before the damaged leaf, then the failure, then nothing.
    let mut scan = read.range(&tree, ..).unwrap();
    let mut returned = Vec::new();
    let error = loop {
        match scan.next().unwrap() {
            Ok((k, v)) => returned.push((k.to_vec(), v.to_vec())),
            Err(error) => break error,
        }
    };
    assert_eq!(damaged_page(error), leaf);
    // Key 1500 is in the damaged leaf, so it does not come out (it may be the leaf's first key).
    assert!(returned.len() <= 1500, "{} returned", returned.len());
    assert_eq!(
        returned,
        model
            .iter()
            .take(returned.len())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Entries>()
    );
    assert!(
        scan.next().is_none() && scan.next_back().is_none(),
        "the scan ended at both ends"
    );

    // Backward from the end, the same.
    let mut scan = read.range(&tree, ..).unwrap();
    let error = scan.by_ref().rev().find_map(|e| e.err()).unwrap();
    assert_eq!(damaged_page(error), leaf);
    assert!(scan.next().is_none());

    // A scan far from the damaged leaf does not fail.
    let far = forward(read.range(&tree, &all[10][..]..&all[20][..]).unwrap());
    assert_eq!(far.len(), 10);
    let far = forward(read.range(&tree, &all[2900][..]..).unwrap());
    assert_eq!(far.len(), 100);

    // In a write, the failure is the transaction's (T11).
    drop(read);
    let write = store.write().unwrap();
    let wtree = write.open_tree("t", NORMAL).unwrap();
    let error = write
        .range(&wtree, ..)
        .unwrap()
        .find_map(|e| e.err())
        .unwrap();
    assert_eq!(damaged_page(error), leaf);
    let error = write.commit().unwrap_err();
    assert_eq!(damaged_page(error), leaf);
    store.close().unwrap();

    // A damaged branch: the root of a tree of several levels.
    let root = root_page(tmp.path(), "t");
    damage(tmp.path(), leaf);
    damage(tmp.path(), root);
    let store = open(tmp.path());
    let read = store.read().unwrap();
    let tree = tree_t(&read);
    for front in [true, false] {
        let mut scan = read.range(&tree, ..).unwrap();
        let first = if front { scan.next() } else { scan.next_back() };
        assert_eq!(damaged_page(first.unwrap().unwrap_err()), root);
        assert!(scan.next().is_none() && scan.next_back().is_none());
    }
}

/// Root AC20 with scans: the keys and values a scan returns point into the engine's memory, like a read's.
#[test]
fn ac20_scans_return_references_into_the_engine_s_memory() {
    stores(|store, _| {
        fill(store, "t", &EXAMPLE.map(<[u8]>::to_vec));
        let read = store.read().unwrap();
        let tree = tree_t(&read);
        let (key, value) = read
            .range(&tree, &b"ab"[..]..)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        let read_value = read.get(&tree, b"ab").unwrap().unwrap();
        assert_eq!(value.as_ptr(), read_value.as_ptr());
        let (again, _) = read
            .range(&tree, ..=&b"ab"[..])
            .unwrap()
            .next_back()
            .unwrap()
            .unwrap();
        assert_eq!(key.as_ptr(), again.as_ptr());
        // The references outlive the scan that returned them, within the read.
        assert_eq!(key, b"ab");
    });
}

/// The types of a scan: double-ended and fused iterators of entries.
#[allow(dead_code)]
fn scan_types<'t>(read: ReadRange<'t>, write: WriteRange<'t>, _tx: &WriteTransaction<'_>) {
    fn check<
        't,
        I: DoubleEndedIterator<Item = Result<(&'t [u8], &'t [u8])>> + std::iter::FusedIterator,
    >(
        _: I,
    ) {
    }
    check(read);
    check(write);
}

/// AC-R1 (removals): on the example tree, removing the prefix `b` removes 2 entries, and removing from `ab`
/// included to `ba` excluded with the prefix `b` removes 1 (only `b`), each on the original tree.
#[test]
fn ac_r1_the_output_removals() {
    stores(|store, _| {
        let all = EXAMPLE.map(<[u8]>::to_vec);
        fill(store, "t", &all);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        assert_eq!(write.remove_range_prefix(&tree, b"b", ..).unwrap(), 2);
        let left = keys(&forward(write.range(&tree, ..).unwrap()));
        assert_eq!(left, [&b"a"[..], b"ab", b"c"].map(<[u8]>::to_vec));
        write.abort();
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        let bounds: Bounds<'_> = (Included(b"ab"), Excluded(b"ba"));
        assert_eq!(write.remove_range_prefix(&tree, b"b", bounds).unwrap(), 1);
        assert_eq!(write.get(&tree, b"b").unwrap(), None);
        assert_eq!(write.len(&tree).unwrap(), 4);
        write.commit().unwrap();
    });
}

/// AC-R4: empty intervals scan and remove nothing without failing, also with the root damaged; through a deleted
/// tree's handle they fail with tree not found; equal included bounds give the entry once; a write that only
/// scans, removes nothing and fails with invalid size leaves the files as they were, and one that removes an
/// entry writes.
#[test]
fn ac_r4_empty_intervals() {
    let empties: [Bounds<'_>; 3] = [
        (Included(b"c"), Included(b"a")),
        (Included(b"b"), Excluded(b"b")),
        (Excluded(b"b"), Included(b"b")),
    ];
    stores(|store, folder| {
        fill(store, "t", &EXAMPLE.map(<[u8]>::to_vec));
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        for bounds in empties {
            assert!(forward(write.range(&tree, bounds).unwrap()).is_empty());
            assert_eq!(write.remove_range(&tree, bounds).unwrap(), 0);
        }
        assert!(forward(write.range_prefix(&tree, b"a", &b"c"[..]..).unwrap()).is_empty());
        assert_eq!(
            write.remove_range_prefix(&tree, b"a", &b"c"[..]..).unwrap(),
            0
        );
        let same: Bounds<'_> = (Included(b"b"), Included(b"b"));
        let mut scan = write.range(&tree, same).unwrap();
        assert_eq!(scan.next().unwrap().unwrap().0, b"b");
        assert!(scan.next_back().is_none() && scan.next().is_none());
        let second = write.open_tree("t", NORMAL).unwrap();
        write.delete_tree("t").unwrap();
        let error = write.remove_range(&second, empties[0]).unwrap_err();
        assert_eq!(not_found(error, folder), "t");
        let error = write.range(&second, empties[0]).map(|_| ()).unwrap_err();
        assert_eq!(not_found(error, folder), "t");
        write.abort();
    });

    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    fill(&store, "t", &EXAMPLE.map(<[u8]>::to_vec));
    store.close().unwrap();
    // Only scans, empty removals, removals of absent keys and invalid sizes: nothing is written.
    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        assert_eq!(forward(write.range(&tree, ..).unwrap()).len(), 5);
        for bounds in empties {
            assert_eq!(write.remove_range(&tree, bounds).unwrap(), 0);
        }
        assert_eq!(write.remove_range(&tree, &b"bb"[..]..&b"c"[..]).unwrap(), 0);
        assert_eq!(write.remove_range_prefix(&tree, b"z", ..).unwrap(), 0);
        let long = vec![0; MAX_KEY + 1];
        write.remove_range(&tree, &long[..]..).unwrap_err();
        write.commit().unwrap();
    });
    assert!(!changed, "a write that removes nothing writes nothing");
    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        assert_eq!(write.remove_range(&tree, &b"c"[..]..).unwrap(), 1);
        write.commit().unwrap();
    });
    assert!(changed, "a write that removes an entry writes");

    // The root damaged: empty intervals still read nothing.
    let root = root_page(tmp.path(), "t");
    damage(tmp.path(), root);
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let tree = write.open_tree("t", NORMAL).unwrap();
    for bounds in empties {
        assert!(forward(write.range(&tree, bounds).unwrap()).is_empty());
        assert_eq!(write.remove_range(&tree, bounds).unwrap(), 0);
    }
    assert_eq!(
        write.remove_range_prefix(&tree, b"a", &b"c"[..]..).unwrap(),
        0
    );
    write.commit().unwrap();
}

/// AC-R5 (removals): the sizes of a removal's bounds and prefix are checked as a scan's, and it fails without
/// effect.
#[test]
fn ac_r5_sizes_of_removals() {
    stores(|store, folder| {
        fill(store, "t", &EXAMPLE.map(<[u8]>::to_vec));
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        let (short, long) = (Vec::new(), vec![b'k'; MAX_KEY + 1]);
        let error = write
            .remove_range(&tree, (Included(&short[..]), Unbounded))
            .unwrap_err();
        assert_eq!(invalid_size(error, folder), (SizeSubject::Key, 0, 1, 2048));
        let error = write
            .remove_range(&tree, (Unbounded, Excluded(&long[..])))
            .unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::Key, 2049, 1, 2048)
        );
        let error = write.remove_range_prefix(&tree, &long, ..).unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::Prefix, 2049, 0, 2048)
        );
        let bounds = (Unbounded, Included(&short[..]));
        let error = write.remove_range_prefix(&tree, &long, bounds).unwrap_err();
        assert_eq!(invalid_size(error, folder), (SizeSubject::Key, 0, 1, 2048));
        assert_eq!(write.len(&tree).unwrap(), 5);
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(read.len(&tree_t(&read)).unwrap(), 5);
    });
}

/// AC-R6 (removals): another handle of the same tree sees the tree without the removed entries and the new count.
#[test]
fn ac_r6_other_handles_see_a_range_removal() {
    stores(|store, _| {
        fill(store, "t", &EXAMPLE.map(<[u8]>::to_vec));
        let mut write = store.write().unwrap();
        let first = write.open_tree("t", NORMAL).unwrap();
        let second = write.open_tree("t", NORMAL).unwrap();
        assert_eq!(
            write.remove_range(&first, &b"ab"[..]..=&b"ba"[..]).unwrap(),
            3
        );
        let left = keys(&forward(write.range(&second, ..).unwrap()));
        assert_eq!(left, [&b"a"[..], b"c"].map(<[u8]>::to_vec));
        assert_eq!(write.len(&second).unwrap(), 2);
        write.commit().unwrap();
    });
}

/// The keys of a removal test: `count` keys of 300 bytes, so that the tree has several levels.
fn removal_keys(count: usize) -> Vec<Vec<u8>> {
    numbered(count, 300)
}

/// Checks that the tree `t` of `write` holds exactly `model`: a whole scan in both directions, the count and a read
/// of every key the test used.
fn check_tree(write: &WriteTransaction<'_>, model: &BTreeMap<Vec<u8>, Vec<u8>>, used: &[Vec<u8>]) {
    let tree = write.open_tree("t", NORMAL).unwrap();
    let want: Entries = model.clone().into_iter().collect();
    assert_eq!(forward(write.range(&tree, ..).unwrap()), want);
    let mut reversed = keys(&want);
    reversed.reverse();
    assert_eq!(backward_keys(write.range(&tree, ..).unwrap()), reversed);
    assert_eq!(write.len(&tree).unwrap(), model.len() as u64);
    for key in used {
        assert_eq!(
            write.get(&tree, key).unwrap(),
            model.get(key).map(Vec::as_slice)
        );
    }
}

/// AC-R9: removals within one page, across pages and levels, with a prefix and without, return how many entries
/// they removed, remove only those, lower the count, and persist after commit and reopening.
#[test]
fn ac_r9_range_removals_across_pages_and_levels() {
    stores(|store, _| {
        let all = removal_keys(2000);
        let mut model = fill(store, "t", &all);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        let steps: [(&[u8], Bounds<'_>); 6] = [
            // Within one page.
            (b"", (Included(&all[100]), Excluded(&all[103]))),
            // Across several pages.
            (b"", (Excluded(&all[200]), Included(&all[320]))),
            // Across a large part of the tree, so that whole subtrees go.
            (b"", (Included(&all[400]), Excluded(&all[1600]))),
            // A prefix: the keys from 01700 to 01799.
            (b"017", (Unbounded, Unbounded)),
            // A prefix and a bound.
            (b"018", (Included(&all[1850]), Unbounded)),
            // Everything before a key.
            (b"", (Unbounded, Excluded(&all[50]))),
        ];
        for (prefix, bounds) in steps {
            let want = expected(&model, prefix, bounds);
            let removed = write.remove_range_prefix(&tree, prefix, bounds).unwrap();
            assert_eq!(removed, want.len() as u64, "prefix {prefix:?}");
            for (key, _) in &want {
                model.remove(key);
            }
            check_tree(&write, &model, &all);
        }
        write.commit().unwrap();
        let read = store.read().unwrap();
        let tree = tree_t(&read);
        let want: Entries = model.clone().into_iter().collect();
        assert_eq!(forward(read.range(&tree, ..).unwrap()), want);
        assert_eq!(read.len(&tree).unwrap(), model.len() as u64);
    });
    // Reopening: the removals persist.
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let all = removal_keys(2000);
    let mut model = fill(&store, "t", &all);
    let mut write = store.write().unwrap();
    let tree = write.open_tree("t", NORMAL).unwrap();
    assert_eq!(
        write
            .remove_range(&tree, &all[10][..]..&all[1990][..])
            .unwrap(),
        1980
    );
    write.commit().unwrap();
    store.close().unwrap();
    model.retain(|k, _| *k < all[10] || *k >= all[1990]);
    let store = open(tmp.path());
    let read = store.read().unwrap();
    let tree = tree_t(&read);
    assert_eq!(
        forward(read.range(&tree, ..).unwrap()),
        model.into_iter().collect::<Entries>()
    );
    assert_eq!(read.len(&tree).unwrap(), 20);
}

/// Removals at random intervals of a tree of several levels, compared with a model after each one, until the
/// tree is empty, with keys stored back in between so that pages split and merge again.
#[test]
fn range_removals_match_a_model() {
    stores(|store, _| {
        let all = removal_keys(1500);
        let mut model = fill(store, "t", &all);
        let mut random = Lcg(3);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        for round in 0..40 {
            let a = (random.next() as usize) % all.len();
            let b = (random.next() as usize) % all.len();
            let (a, b) = (a.min(b), a.max(b));
            let lower = match random.next() % 3 {
                0 => Included(&all[a][..]),
                1 => Excluded(&all[a][..]),
                _ => Unbounded,
            };
            let upper = match random.next() % 4 {
                0 => Included(&all[b][..]),
                1 => Excluded(&all[b][..]),
                2 if round % 5 == 0 => Unbounded,
                _ => Excluded(&all[(b + 1).min(all.len() - 1)][..]),
            };
            let want = expected(&model, b"", (lower, upper));
            assert_eq!(
                write.remove_range(&tree, (lower, upper)).unwrap(),
                want.len() as u64
            );
            for (key, _) in &want {
                model.remove(key);
            }
            // Store some keys back.
            for _ in 0..(random.next() % 200) {
                let key = &all[(random.next() as usize) % all.len()];
                write.set(&tree, key, &value_of(key)).unwrap();
                model.insert(key.clone(), value_of(key));
            }
            check_tree(&write, &model, &all);
        }
        assert_eq!(write.remove_range(&tree, ..).unwrap(), model.len() as u64);
        model.clear();
        check_tree(&write, &model, &all);
        write.commit().unwrap();
    });
}

/// AC-R10: removing every entry of a tree of several levels, with one interval or with ten contiguous ones in
/// separate writes, leaves the pages in use as with the tree empty.
#[test]
fn ac_r10_removing_every_entry_gives_the_pages_back() {
    stores(|store, _| {
        commit_tree(store, "other", &[(b"k", b"v")]);
        let in_use = |store: &Store| {
            let info = store.info();
            info.page_count - info.free_pages
        };
        let all = removal_keys(1500);
        for intervals in [1, 10] {
            commit_tree(store, "t", EMPTY);
            let empty = in_use(store);
            fill(store, "t", &all);
            assert!(in_use(store) > empty + 20, "the tree takes many pages");
            let cuts: Vec<usize> = (0..=intervals).map(|i| i * all.len() / intervals).collect();
            for pair in cuts.windows(2) {
                let mut write = store.write().unwrap();
                let tree = write.open_tree("t", NORMAL).unwrap();
                let lower = if pair[0] == 0 {
                    Unbounded
                } else {
                    Included(&all[pair[0]][..])
                };
                let upper = if pair[1] == all.len() {
                    Unbounded
                } else {
                    Excluded(&all[pair[1]][..])
                };
                let removed = write.remove_range(&tree, (lower, upper)).unwrap();
                assert_eq!(removed, (pair[1] - pair[0]) as u64);
                write.commit().unwrap();
            }
            assert_eq!(in_use(store), empty, "{intervals} intervals");
            let mut write = store.write().unwrap();
            write.delete_tree("t").unwrap();
            write.commit().unwrap();
        }
    });
}

/// AC-R11 (removals): a removal through a handle of a deleted tree fails with tree not found and the last name,
/// through a handle of an ended write with misuse; invalid size wins over both; the write stays usable.
#[test]
fn ac_r11_removals_through_deleted_and_foreign_handles() {
    stores(|store, folder| {
        fill(store, "t", &EXAMPLE.map(<[u8]>::to_vec));
        let mut write = store.write().unwrap();
        let first = write.open_tree("t", NORMAL).unwrap();
        let second = write.open_tree("t", NORMAL).unwrap();
        write.delete_tree("t").unwrap();
        assert_eq!(
            not_found(write.remove_range(&second, ..).unwrap_err(), folder),
            "t"
        );
        let long = vec![0; MAX_KEY + 1];
        let error = write.remove_range_prefix(&first, &long, ..).unwrap_err();
        assert_eq!(invalid_size(error, folder).0, SizeSubject::Prefix);
        write.abort();

        let old = store.write().unwrap();
        let foreign = old.open_tree("t", NORMAL).unwrap();
        old.commit().unwrap();
        let mut write = store.write().unwrap();
        misuse(write.remove_range(&foreign, ..).unwrap_err(), folder);
        let error = write.remove_range(&foreign, &long[..]..).unwrap_err();
        assert_eq!(invalid_size(error, folder).0, SizeSubject::Key);
        let tree = write.open_tree("t", NORMAL).unwrap();
        assert_eq!(write.remove_range(&tree, ..).unwrap(), 5);
        write.commit().unwrap();
    });
}

/// AC-R13 (removals, disk only): a removal far from a damaged leaf does not fail; one that needs it fails with
/// damage, and the commit then fails with damage and applies nothing.
#[test]
fn ac_r13_removals_and_damaged_pages() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let all = removal_keys(2000);
    fill(&store, "t", &all);
    store.close().unwrap();
    let leaf = leaf_page(tmp.path(), Some("t"), &all[1000]);
    damage(tmp.path(), leaf);
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let tree = write.open_tree("t", NORMAL).unwrap();
    assert_eq!(
        write
            .remove_range(&tree, &all[10][..]..&all[30][..])
            .unwrap(),
        20
    );
    assert_eq!(write.remove_range(&tree, &all[1950][..]..).unwrap(), 50);
    write.commit().unwrap();
    let mut write = store.write().unwrap();
    let tree = write.open_tree("t", NORMAL).unwrap();
    let error = write
        .remove_range(&tree, &all[900][..]..&all[1100][..])
        .unwrap_err();
    assert_eq!(damaged_page(error), leaf);
    assert_eq!(damaged_page(write.commit().unwrap_err()), leaf);
    let read = store.read().unwrap();
    assert_eq!(
        read.len(&tree_t(&read)).unwrap(),
        1930,
        "the failed write applied nothing"
    );
}

/// The height of the tree `t` in the closed store at `folder`.
fn height(folder: &std::path::Path) -> u8 {
    inspect(folder, |read| {
        let root = krummi_tree::catalog::lookup(read, "t")
            .unwrap()
            .unwrap()
            .root;
        krummi_tree::height(read, root).unwrap()
    })
}

/// AC-R7 (G3, AC-B17): a read with a scan half consumed and another not started keeps returning exactly the
/// entries of its snapshot while a write removes intervals and stores entries until the tree changes height and
/// commits; a new read sees the write's entries.
#[test]
fn ac_r7_scans_keep_their_snapshot_while_a_write_changes_the_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let all = removal_keys(1500);
    let snapshot = fill(&store, "t", &all);
    store.close().unwrap();
    let tall = height(tmp.path());
    assert!(tall >= 3, "the tree has {tall} levels");
    let store = open(tmp.path());
    let read = store.read().unwrap();
    let tree = tree_t(&read);
    let mut half = read.range(&tree, ..).unwrap();
    let first: Entries = forward(half.by_ref().take(100));
    let mut later = read.range_prefix(&tree, b"01", ..).unwrap();

    let mut write = store.write().unwrap();
    let wtree = write.open_tree("t", NORMAL).unwrap();
    assert_eq!(
        write
            .remove_range(&wtree, &all[50][..]..&all[1450][..])
            .unwrap(),
        1400
    );
    write.set(&wtree, b"new", b"entry").unwrap();
    assert_eq!(write.remove_range_prefix(&wtree, b"0145", ..).unwrap(), 10);
    let model_after = {
        let mut model = snapshot.clone();
        model.retain(|k, _| *k < all[50] || *k >= all[1450]);
        model.retain(|k, _| !k.starts_with(b"0145"));
        model.insert(b"new".to_vec(), b"entry".to_vec());
        model
    };
    write.commit().unwrap();

    let mut seen = first;
    seen.extend(forward(half));
    assert_eq!(seen, snapshot.clone().into_iter().collect::<Entries>());
    let want: Entries = snapshot
        .iter()
        .filter(|(k, _)| k.starts_with(b"01"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    assert_eq!(forward(later.by_ref()), want);
    drop(read);
    let read = store.read().unwrap();
    let tree = tree_t(&read);
    assert_eq!(
        forward(read.range(&tree, ..).unwrap()),
        model_after.into_iter().collect::<Entries>()
    );
    drop(read);
    store.close().unwrap();
    assert!(height(tmp.path()) < tall, "the write lowered the tree");
}
