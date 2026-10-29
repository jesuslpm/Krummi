//! Large values through the public API (large values PRD: V1, V2, V5–V7; AC-V1, AC-V2, AC-V4, AC-V5, AC-V7,
//! AC-V10, and the leaf part of AC-V12). Where a reference points (AC-V3) and the frontier of AC-V9 need the
//! engine's test hooks and are in the library's own tests (`src/value_tests.rs`); what must not compile (AC-V3's
//! last part) is in `trees_compile.rs`, whose references have the same types whatever the size of the value.
//!
//! Each criterion runs on a store on disk and on a temporary one, one after the other, except the parts that reopen
//! or damage the store's files, which run on disk only. The limits around the leaf's threshold and the value runs
//! come from the implementation, as the PRD allows. Values are compared with `==` and not `assert_eq!`, which would
//! print megabytes on a failure.

mod support;

use std::collections::BTreeMap;
use std::path::Path;

use krummi::{ErrorKind, OpenMode, SizeSubject, Store, StoreOptions, WriteTree};
use krummi_pager::format::{PAGE_HEADER_SIZE, PAGE_SIZE};
use krummi_tx::run_pages;
use support::*;

type Model = BTreeMap<Vec<u8>, Vec<u8>>;

/// A value of `len` bytes, different for each `seed`. Its period, 251 bytes, does not divide the page size, so two
/// pages of a value never hold the same bytes.
fn pattern(len: usize, seed: u8) -> Vec<u8> {
    let block: Vec<u8> = (0..251u8).map(|i| i.wrapping_mul(7) ^ seed).collect();
    let mut value = Vec::with_capacity(len);
    while value.len() < len {
        let more = (len - value.len()).min(block.len());
        value.extend_from_slice(&block[..more]);
    }
    value
}

/// Runs `test` on a store on disk and then on a temporary one; `test` returns the store, which it may have reopened
/// with [`reopen`], and the store is closed before the next one opens, so that a large value takes memory once.
fn modes(test: impl Fn(Store, Option<&Path>) -> Store) {
    let tmp = tempfile::tempdir().unwrap();
    test(open(tmp.path()), Some(tmp.path())).close().unwrap();
    let in_memory = Store::open_temporary(&StoreOptions::default()).unwrap();
    test(in_memory, None).close().unwrap();
}

/// A store on disk closed and opened again; a temporary one as it is.
fn reopen(store: Store, folder: Option<&Path>) -> Store {
    match folder {
        Some(folder) => {
            store.close().unwrap();
            open(folder)
        }
        None => store,
    }
}

/// Pages in use: the pages of the store minus the free pages of its information, as in AC-A15.
fn in_use(store: &Store) -> u64 {
    let info = store.info();
    info.page_count - info.free_pages
}

/// The largest value that a run of `pages` pages holds, from the implementation (D1).
fn run_capacity(pages: usize) -> usize {
    let capacity = pages * PAGE_SIZE - PAGE_HEADER_SIZE;
    assert_eq!(run_pages(capacity as u64), pages as u64);
    assert_eq!(run_pages(capacity as u64 + 1), pages as u64 + 1);
    capacity
}

/// Checks every key of `model` and `absent` in the committed tree `t`, and that its count is the number of keys
/// found.
#[track_caller]
fn check_read(store: &Store, model: &Model, absent: &[Vec<u8>], label: &str) {
    let read = store.read().unwrap();
    let tree = read.open_tree("t").unwrap();
    for (key, value) in model {
        let found = read.get(&tree, key).unwrap();
        assert!(found == Some(&value[..]), "{label}: {} bytes", value.len());
    }
    for key in absent {
        assert_eq!(read.get(&tree, key).unwrap(), None, "{label}");
    }
    assert_eq!(read.len(&tree).unwrap(), model.len() as u64, "{label}");
}

/// AC-V1 (V1, AC-B3): a 2,048-byte key holds an empty value and one of 268,435,456 bytes, read back identical after
/// the commit and, on disk, after reopening, and returned by a scan; the large one is overwritten with one byte and
/// removed; a value of 268,435,457 bytes fails with the invalid size, as a value, with the size received, minimum
/// `0` and maximum `268435456`, storing nothing, and the write stays usable.
#[test]
fn ac_v1_a_value_of_256_mib_under_a_2048_byte_key() {
    let key = vec![b'k'; MAX_KEY];
    let mut empty_key = key.clone();
    empty_key[0] = b'e';
    modes(|store, folder| {
        let big = pattern(MAX_VALUE, 1);
        let mut write = store.write().unwrap();
        let tree = create(&mut write, "t");
        write.set(&tree, &empty_key, b"").unwrap();
        write.set(&tree, &key, &big).unwrap();
        // Zeroed by the allocator and never read: it takes no memory.
        let over = vec![0u8; MAX_VALUE + 1];
        let expected = (SizeSubject::Value, 268_435_457, 0, 268_435_456);
        let error = write.set(&tree, b"over", &over).unwrap_err();
        assert_eq!(invalid_size(error, folder), expected);
        let error = write.try_add(&tree, b"over", &over).unwrap_err();
        assert_eq!(invalid_size(error, folder), expected);
        drop(over);
        assert_eq!(write.get(&tree, b"over").unwrap(), None, "nothing stored");
        write.set(&tree, b"usable", b"yes").unwrap();
        write.commit().unwrap();

        let check = |store: &Store, label: &str| {
            let read = store.read().unwrap();
            let tree = read.open_tree("t").unwrap();
            assert!(read.get(&tree, &key).unwrap() == Some(&big[..]), "{label}");
            assert_eq!(read.get(&tree, &empty_key).unwrap(), Some(&b""[..]));
            let scanned: Vec<(&[u8], &[u8])> = read
                .range(&tree, ..)
                .unwrap()
                .map(|entry| entry.unwrap())
                .collect();
            let expected: [(&[u8], &[u8]); 3] =
                [(&empty_key, b""), (&key, &big), (b"usable", b"yes")];
            assert!(scanned == expected, "{label}: a scan");
            assert_eq!(read.len(&tree).unwrap(), 3, "{label}");
        };
        check(&store, "after the commit");
        let store = reopen(store, folder);
        check(&store, "after reopening");
        drop(big);

        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        write.set(&tree, &key, b"1").unwrap();
        assert_eq!(write.get(&tree, &key).unwrap(), Some(&b"1"[..]));
        write.commit().unwrap();
        let read = store.read().unwrap();
        let read_tree = read.open_tree("t").unwrap();
        assert_eq!(read.get(&read_tree, &key).unwrap(), Some(&b"1"[..]));
        drop(read);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        assert!(write.remove(&tree, &key).unwrap());
        write.commit().unwrap();
        let read = store.read().unwrap();
        let read_tree = read.open_tree("t").unwrap();
        assert_eq!(read.get(&read_tree, &key).unwrap(), None);
        assert_eq!(read.len(&read_tree).unwrap(), 2);
        drop(read);
        store
    });
}

/// The sizes of AC-V2 for keys of `key_len` bytes: 1,024 and 1,025 bytes, 1 MiB and 8 MiB, and one byte less, equal
/// and one more than the leaf's threshold and the largest value that fits in 1, 2 and 3 pages off its leaf.
fn sizes(key_len: usize) -> Vec<usize> {
    let threshold = krummi_tree::max_inline_value(key_len);
    let mut sizes = vec![1024, 1025, 1 << 20, 8 << 20];
    for limit in [threshold, run_capacity(1), run_capacity(2), run_capacity(3)] {
        sizes.extend([limit - 1, limit, limit + 1]);
    }
    sizes
}

/// AC-V2 (V1, V2, A4, A5): with keys of 1 and of 2,048 bytes, every size around the limits reads back identical in
/// the same write, in a later read and, on disk, after reopening; overwritten with larger, smaller and empty values,
/// added over existing keys and removed, a key always reads its last value or is not there, and the count is the
/// number of keys found.
#[test]
fn ac_v2_values_around_the_threshold_and_the_run_boundaries() {
    let entries: Vec<(Vec<u8>, usize)> = [1, MAX_KEY]
        .into_iter()
        .flat_map(|key_len| {
            sizes(key_len).into_iter().enumerate().map(move |(i, len)| {
                let key = if key_len == 1 {
                    vec![b'a' + i as u8]
                } else {
                    long_key(i, MAX_KEY)
                };
                (key, len)
            })
        })
        .collect();
    assert_eq!(entries.len(), 32);
    let keys: Vec<Vec<u8>> = entries.iter().map(|(key, _)| key.clone()).collect();
    modes(|store, folder| {
        let mut model = Model::new();
        let mut write = store.write().unwrap();
        let tree = create(&mut write, "t");
        for (i, (key, len)) in entries.iter().enumerate() {
            let value = pattern(*len, i as u8);
            write.set(&tree, key, &value).unwrap();
            let found = write.get(&tree, key).unwrap();
            assert!(found == Some(&value[..]), "the same write: {len} bytes");
            model.insert(key.clone(), value);
        }
        write.commit().unwrap();
        check_read(&store, &model, &[], "a later read");
        let mut store = reopen(store, folder);
        check_read(&store, &model, &[], "after reopening");

        // Each round changes every key in one write, checked after each operation and in a read after the commit.
        type Round = fn(usize) -> Option<usize>;
        let rounds: [(&str, Round); 5] = [
            ("larger", |len| Some(len + 3 * PAGE_SIZE + 1)),
            ("smaller", |len| Some(len / 3)),
            ("added over existing keys", |_| Some(5_000)),
            ("empty", |_| Some(0)),
            ("removed", |_| None),
        ];
        for (round, (label, next)) in rounds.into_iter().enumerate() {
            let mut write = store.write().unwrap();
            let tree = write.open_tree("t").unwrap();
            for (i, key) in keys.iter().enumerate() {
                // The removal round removes every other key, so the tree keeps entries.
                if label == "removed" && i % 2 == 1 {
                    continue;
                }
                let old = model.get(key).map_or(0, Vec::len);
                match next(old) {
                    Some(len) if label == "added over existing keys" => {
                        let value = pattern(len, 100 + round as u8);
                        assert!(!write.try_add(&tree, key, &value).unwrap(), "{label}");
                    }
                    Some(len) => {
                        let value = pattern(len, (10 * round + i) as u8);
                        write.set(&tree, key, &value).unwrap();
                        model.insert(key.clone(), value);
                    }
                    None => {
                        assert!(write.remove(&tree, key).unwrap(), "{label}");
                        model.remove(key);
                    }
                }
                let found = write.get(&tree, key).unwrap();
                assert!(found == model.get(key).map(|v| &v[..]), "{label}");
            }
            let found = keys
                .iter()
                .filter(|key| write.get(&tree, key).unwrap().is_some())
                .count();
            assert_eq!(write.len(&tree).unwrap(), found as u64, "{label}");
            assert_eq!(found, model.len(), "{label}");
            write.commit().unwrap();
            let absent: Vec<Vec<u8>> = keys
                .iter()
                .filter(|key| !model.contains_key(*key))
                .cloned()
                .collect();
            check_read(&store, &model, &absent, label);
            store = reopen(store, folder);
            check_read(&store, &model, &absent, label);
        }
        store
    });
}

/// The entries of AC-V4: keys `a0`–`a7` and `b0`–`b7`, with values inside their leaf and off it.
fn scan_model() -> Model {
    let lens = [17, 5_000, 0, 20_000, 3_000, 70_000, 4_000, 1 << 20];
    (0..16)
        .map(|i| {
            let key = vec![if i < 8 { b'a' } else { b'b' }, b'0' + (i % 8) as u8];
            (key, pattern(lens[i % 8], i as u8))
        })
        .collect()
}

/// AC-V4 (V2, R1, R7): forward, backward and prefix scans return every value whole and identical, as the same
/// memory a read returns; range removals over values off their leaf return how many entries they removed, and a
/// scan of the whole tree returns the others with their values.
#[test]
fn ac_v4_scans_and_range_removals_over_values_off_their_leaf() {
    modes(|store, _| {
        let mut model = scan_model();
        commit_tree(&store, "t", EMPTY);
        // An older read keeps the next version out of `krummi.data`, so the reads below see it in one place.
        let oldest = store.read().unwrap();
        commit_tree(&store, "t", &model.iter().collect::<Vec<_>>());
        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        let forward: Vec<(&[u8], &[u8])> =
            read.range(&tree, ..).unwrap().map(|e| e.unwrap()).collect();
        let expected: Vec<(&[u8], &[u8])> = model.iter().map(|(k, v)| (&k[..], &v[..])).collect();
        assert!(forward == expected, "forward");
        for (key, value) in &forward {
            let got = read.get(&tree, key).unwrap().unwrap();
            assert_eq!(value.as_ptr(), got.as_ptr(), "no copy");
        }
        let mut backward: Vec<(&[u8], &[u8])> = read
            .range(&tree, ..)
            .unwrap()
            .rev()
            .map(|e| e.unwrap())
            .collect();
        backward.reverse();
        assert!(backward == expected, "backward");
        let b: Vec<(&[u8], &[u8])> = expected
            .iter()
            .copied()
            .filter(|(k, _)| k[0] == b'b')
            .collect();
        let prefixed: Vec<(&[u8], &[u8])> = read
            .range_prefix(&tree, b"b", ..)
            .unwrap()
            .map(|e| e.unwrap())
            .collect();
        assert!(prefixed == b, "prefix");
        let mut prefixed: Vec<(&[u8], &[u8])> = read
            .range_prefix(&tree, b"b", ..)
            .unwrap()
            .rev()
            .map(|e| e.unwrap())
            .collect();
        prefixed.reverse();
        assert!(prefixed == b, "prefix backward");
        drop((read, oldest));

        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        let removed = write.remove_range(&tree, &b"a3"[..]..&b"b2"[..]).unwrap();
        assert_eq!(removed, 7, "a3 to a7, b0 and b1");
        model.retain(|k, _| !(k[..] >= b"a3"[..] && k[..] < b"b2"[..]));
        let whole = |write: &krummi::WriteTransaction<'_>, tree: &WriteTree<'_>| {
            let scanned: Vec<(Vec<u8>, Vec<u8>)> = write
                .range(tree, ..)
                .unwrap()
                .map(|e| e.map(|(k, v)| (k.to_vec(), v.to_vec())).unwrap())
                .collect();
            scanned
        };
        let rest: Vec<(Vec<u8>, Vec<u8>)> = model.clone().into_iter().collect();
        assert!(whole(&write, &tree) == rest, "after the first removal");
        let removed = write.remove_range_prefix(&tree, b"b", ..).unwrap();
        assert_eq!(removed, 6, "b2 to b7");
        model.retain(|k, _| k[0] != b'b');
        let rest: Vec<(Vec<u8>, Vec<u8>)> = model.clone().into_iter().collect();
        assert!(whole(&write, &tree) == rest, "after the prefix removal");
        write.commit().unwrap();
        check_read(&store, &model, &[], "committed");
        store
    });
}

/// Stores values inside and off their leaf in the tree `name`, overwrites them with larger and smaller ones, and
/// commits.
fn fill_and_overwrite(store: &Store, name: &str) {
    let lens = [100, 5_000, 20_000, 100_000];
    let mut write = store.write().unwrap();
    let tree = write.open_tree(name).unwrap();
    for i in 0..24 {
        write
            .set(&tree, &long_key(i, 40), &pattern(lens[i % 4], 1))
            .unwrap();
    }
    for i in 0..24 {
        let len = lens[i % 4];
        let len = if i % 2 == 0 { 2 * len + 1 } else { len / 2 };
        write
            .set(&tree, &long_key(i, 40), &pattern(len, 2))
            .unwrap();
    }
    write.commit().unwrap();
}

/// AC-V5 (V5, A2, A3, R7, F10): emptying a tree that held values off their leaf, removing all its entries with a
/// range removal, deleting it and overwriting it with a rename give back every page, as in AC-A15; and a value
/// off its leaf of 1 MiB, then of 64 KiB, grows the pages in use by at most the pages its bytes fill, rounded up,
/// plus one (the leaf has room for its entry).
#[test]
fn ac_v5_values_off_their_leaf_give_their_pages_back() {
    modes(|store, _| {
        commit_tree(&store, "a", &[(b"k", b"v")]);
        let before = in_use(&store);

        commit_tree(&store, "e", EMPTY);
        let empty = in_use(&store);
        fill_and_overwrite(&store, "e");
        assert!(in_use(&store) > empty + 50, "values off their leaf");
        let mut write = store.write().unwrap();
        write.open_tree_with("e", OpenMode::Empty).unwrap();
        write.commit().unwrap();
        assert_eq!(in_use(&store), empty, "emptied");

        fill_and_overwrite(&store, "e");
        let mut write = store.write().unwrap();
        let tree = write.open_tree("e").unwrap();
        assert_eq!(write.remove_range(&tree, ..).unwrap(), 24);
        write.commit().unwrap();
        assert_eq!(in_use(&store), empty, "removed by a range removal");
        let mut write = store.write().unwrap();
        write.delete_tree("e").unwrap();
        write.commit().unwrap();
        assert_eq!(in_use(&store), before, "deleted");

        // Deleted, and overwritten by a rename: the catalog keeps one entry, of a name of the same length.
        for rename in [false, true] {
            commit_tree(&store, "x", EMPTY);
            fill_and_overwrite(&store, "x");
            assert!(in_use(&store) > before + 50);
            let mut write = store.write().unwrap();
            if rename {
                write.rename_tree("a", "x", true).unwrap();
            } else {
                write.delete_tree("x").unwrap();
            }
            write.commit().unwrap();
            assert_eq!(in_use(&store), before, "rename: {rename}");
        }

        // Pages a value off its leaf needs: its bytes' pages rounded up, plus one.
        commit_tree(&store, "v", &[(b"a", b"small")]);
        let base = in_use(&store);
        for len in [1 << 20, 64 << 10] {
            commit_tree(&store, "v", &[(&b"b"[..], &pattern(len, 3)[..])]);
            let grown = in_use(&store) - base;
            let bound = (len as u64).div_ceil(PAGE_SIZE as u64) + 1;
            assert!(grown > 0 && grown <= bound, "{len} bytes: {grown} pages");
        }
        store
    });
}

/// A closed store on disk with the tree `t` of `a` and `c`, inline, and `b` off its leaf in 4 pages, with a byte of
/// page `at` of `b`'s run altered, and the tree `other`; returns the run's first page.
fn damaged_store(folder: &Path, at: u64) -> u64 {
    let store = open(folder);
    commit_tree(
        &store,
        "t",
        &[
            (&b"a"[..], &b"first"[..]),
            (b"b", &pattern(run_capacity(4), 4)),
            (b"c", b"last"),
        ],
    );
    commit_tree(&store, "other", &[(b"k", b"v")]);
    store.close().unwrap();
    let (first, count) = value_run(folder, "t", b"b").unwrap();
    assert_eq!(count, 4);
    damage(folder, first + at);
    first
}

/// AC-V7 (V7, A9, G6, R8): with a byte altered in the first, an interior and the last page of a value's run, a read
/// and a scan that needs its entry fail with damage and the run's first page, again when repeated, after the entries
/// returned before and leaving the other keys readable; storing over the key, removing it, removing a range with it,
/// adding over it, emptying, deleting and overwriting the tree with a rename never fail and commit.
#[test]
fn ac_v7_a_damaged_value_fails_its_reads_but_never_its_removal() {
    for at in [0, 2, 3] {
        let tmp = tempfile::tempdir().unwrap();
        let first = damaged_store(tmp.path(), at);
        let store = open(tmp.path());
        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        for _ in 0..2 {
            assert_eq!(damaged_page(read.get(&tree, b"b").unwrap_err()), first);
            let mut scan = read.range(&tree, ..).unwrap();
            assert_eq!(scan.next().unwrap().unwrap(), (&b"a"[..], &b"first"[..]));
            assert_eq!(damaged_page(scan.next().unwrap().unwrap_err()), first);
            assert!(scan.next().is_none());
            let mut scan = read.range(&tree, ..).unwrap();
            assert_eq!(
                scan.next_back().unwrap().unwrap(),
                (&b"c"[..], &b"last"[..])
            );
            assert_eq!(damaged_page(scan.next_back().unwrap().unwrap_err()), first);
        }
        assert_eq!(read.get(&tree, b"a").unwrap(), Some(&b"first"[..]));
        assert_eq!(read.get(&tree, b"c").unwrap(), Some(&b"last"[..]));
        drop(read);
        store.close().unwrap();

        type Change = fn(&mut krummi::WriteTransaction<'_>) -> krummi::Result<()>;
        let changes: [(&str, Change); 7] = [
            ("store over it", |w| {
                let t = w.open_tree("t")?;
                w.set(&t, b"b", b"new")
            }),
            ("remove it", |w| {
                let t = w.open_tree("t")?;
                assert!(w.remove(&t, b"b")?);
                Ok(())
            }),
            ("remove a range with it", |w| {
                let t = w.open_tree("t")?;
                assert_eq!(w.remove_range(&t, &b"b"[..]..=&b"c"[..])?, 2);
                Ok(())
            }),
            ("add over it", |w| {
                let t = w.open_tree("t")?;
                assert!(!w.try_add(&t, b"b", b"added")?);
                Ok(())
            }),
            ("empty the tree", |w| {
                w.open_tree_with("t", OpenMode::Empty).map(|_| ())
            }),
            ("delete the tree", |w| w.delete_tree("t")),
            ("overwrite the tree with a rename", |w| {
                w.rename_tree("other", "t", true)
            }),
        ];
        for (label, change) in changes {
            let tmp = tempfile::tempdir().unwrap();
            let first = damaged_store(tmp.path(), at);
            let store = open(tmp.path());
            let mut write = store.write().unwrap();
            change(&mut write).unwrap();
            write.commit().unwrap();
            let read = store.read().unwrap();
            if label == "delete the tree" {
                assert!(!read.tree_exists("t").unwrap());
            } else {
                let tree = read.open_tree("t").unwrap();
                let b = read.get(&tree, b"b");
                match label {
                    "store over it" => assert_eq!(b.unwrap(), Some(&b"new"[..])),
                    "add over it" => assert_eq!(damaged_page(b.unwrap_err()), first),
                    _ => assert_eq!(b.unwrap(), None, "{label}"),
                }
            }
            drop(read);
            store.close().unwrap();
        }
    }
}

/// AC-V10 (V1, A7, A10, B7, T10): a write on disk that only adds an 8 MiB value over an existing key, removes keys
/// that are not there and fails with invalid sizes commits without error and leaves the files identical; adding a
/// value of the maximum size over an existing key with a maximum size that leaves it no room returns that it did
/// not add it; and a value of 268,435,457 bytes through a handle of a deleted tree, or in a write that already had a
/// failure of T11, fails with the invalid size.
#[test]
fn ac_v10_writes_that_change_nothing_with_large_values() {
    let tmp = tempfile::tempdir().unwrap();
    commit_tree(&open(tmp.path()), "t", &[(b"k", b"v")]);
    let over = vec![0u8; MAX_VALUE + 1];
    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        assert!(!write.try_add(&tree, b"k", &pattern(8 << 20, 5)).unwrap());
        assert!(!write.remove(&tree, b"missing").unwrap());
        assert!(!write.remove(&tree, b"absent").unwrap());
        let error = write.set(&tree, b"k", &over).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidSize);
        let error = write.try_add(&tree, b"n", &over).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidSize);
        write.commit().unwrap();
    });
    assert!(!changed, "the files are identical");

    // A maximum size of 1 MiB leaves no room for 256 MiB; the value is never read.
    let options = StoreOptions {
        max_size: Some(1 << 20),
    };
    let store = Store::open(tmp.path(), &options).unwrap();
    let max = vec![0u8; MAX_VALUE];
    let mut write = store.write().unwrap();
    let tree = write.open_tree("t").unwrap();
    assert!(!write.try_add(&tree, b"k", &max).unwrap(), "not added");
    write.commit().unwrap();

    let mut write = store.write().unwrap();
    let tree = write.open_tree("t").unwrap();
    write.delete_tree("t").unwrap();
    let error = write.set(&tree, b"k", &over).unwrap_err();
    assert_eq!(invalid_size(error, Some(tmp.path())).1, 268_435_457);
    let error = write.try_add(&tree, b"k", &over).unwrap_err();
    assert_eq!(invalid_size(error, Some(tmp.path())).1, 268_435_457);
    write.abort();

    let mut write = store.write().unwrap();
    let tree = write.open_tree("t").unwrap();
    let failure = write.set(&tree, b"big", &pattern(4 << 20, 6)).unwrap_err();
    assert_eq!(failure.kind(), ErrorKind::MaxSize, "a failure of T11");
    let error = write.set(&tree, b"k", &over).unwrap_err();
    assert_eq!(invalid_size(error, Some(tmp.path())).1, 268_435_457);
    let error = write.try_add(&tree, b"k", &over).unwrap_err();
    assert_eq!(invalid_size(error, Some(tmp.path())).1, 268_435_457);
    assert_eq!(write.commit().unwrap_err().kind(), ErrorKind::MaxSize);
    let read = store.read().unwrap();
    let tree = read.open_tree("t").unwrap();
    assert_eq!(read.get(&tree, b"k").unwrap(), Some(&b"v"[..]));
    assert_eq!(read.len(&tree).unwrap(), 1);
}

/// The leaf part of AC-V12 (V3, D2): on disk, the entry of a value off its leaf holds its run's first page and its
/// length in little endian, after the key.
#[test]
fn ac_v12_the_entry_of_a_value_off_its_leaf_is_little_endian() {
    let tmp = tempfile::tempdir().unwrap();
    let value = pattern(70_000, 7);
    commit_tree(&open(tmp.path()), "t", &[(&b"key"[..], &value[..])]);
    let (first, _) = value_run(tmp.path(), "t", b"key").unwrap();
    let leaf = leaf_page(tmp.path(), Some("t"), b"key");
    let data = std::fs::read(tmp.path().join(krummi_pager::DATA_FILE)).unwrap();
    let page = &data[leaf as usize * PAGE_SIZE..][..PAGE_SIZE];
    let mut entry = b"key".to_vec();
    entry.extend_from_slice(&first.to_le_bytes());
    entry.extend_from_slice(&(value.len() as u64).to_le_bytes());
    assert!(
        page.windows(entry.len()).any(|w| w == entry),
        "the key, then the page and the length in little endian"
    );
}
