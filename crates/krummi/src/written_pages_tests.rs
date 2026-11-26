//! Remembered checks of what the engine writes (the written pages PRD, AC-Q1 to AC-Q9), through the public API and the
//! engine's test hooks: how many integrity and layout checks the engine makes, where pages live, which layout checks a
//! write transaction records, failed writes of the transfer and damage that appears while the store is open.
//!
//! The counting tests (AC-Q1, AC-Q5, AC-Q6) give the same counts in debug and production builds (AC-Q8): run them with
//! `cargo test --release -p krummi written_pages` too.

use std::collections::BTreeSet;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

use krummi_pager::testing::{self, Action, Op};
use krummi_tree::{catalog, max_inline_value};
use krummi_tx::{LayoutEvent, MAP_PAGE, Memory};

use super::*;
use crate::check_tests::{
    FIXED_LEN, damaged_page, fixed_leaf, fixed_value, leaf, long_key, on_both, root_of, transfer,
};
use crate::table::key::encode_key;
use crate::table::value::{ColumnType, Value};
use crate::value_tests::pattern;

/// The checks the store made so far: of integrity, of the layout of contents of the store's pages (one per pass), and
/// of the layout of versions of the scratch area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Checks {
    integrity: u64,
    stored_layout: u64,
    version_layout: u64,
}

fn checks(store: &Store) -> Checks {
    Checks {
        integrity: store.shared().engine.integrity_checks(),
        stored_layout: store.shared().engine.checked_remembered(),
        version_layout: store.shared().engine.version_checks(),
    }
}

/// The checks that `f` makes.
fn checks_of<T>(store: &Store, f: impl FnOnce() -> T) -> (T, Checks) {
    let before = checks(store);
    let result = f();
    let after = checks(store);
    (
        result,
        Checks {
            integrity: after.integrity - before.integrity,
            stored_layout: after.stored_layout - before.stored_layout,
            version_layout: after.version_layout - before.version_layout,
        },
    )
}

/// No check at all.
const NONE: Checks = Checks {
    integrity: 0,
    stored_layout: 0,
    version_layout: 0,
};

/// The value of key `i` of the normal trees here, in its leaf: every one has the same length.
fn value_of(i: usize, round: u8) -> Vec<u8> {
    format!("value {round:02} of {i:04}").into_bytes()
}

/// Commits the normal tree `name` with keys `0..count` of 64 bytes and the values of round 0, then removes every
/// fourth key, so that its leaves have room for a few more entries and stay over a quarter full.
fn commit_spaced_tree(store: &Store, name: &str, count: usize) {
    let mut write = store.write().unwrap();
    let tree = write.open_tree_with(name, OpenMode::CreateNew).unwrap();
    for i in 0..count {
        write.set(&tree, &long_key(i, 64), &value_of(i, 0)).unwrap();
    }
    write.commit().unwrap();
    let mut write = store.write().unwrap();
    let tree = write.open_tree(name).unwrap();
    for i in (3..count).step_by(4) {
        assert!(write.remove(&tree, &long_key(i, 64)).unwrap());
    }
    write.commit().unwrap();
}

/// The table of AC-Q5: rows keyed by `id` with a fixed-width column `n` and a variable one `s`.
fn table_schema() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64).not_null(),
            Column::new("s", ColumnType::String),
        ],
        &["id"],
    )
}

/// Commits table `rows` with 40 rows, `n` of row `id` being `id`.
fn commit_table(store: &Store) {
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("rows", TableMode::CreateNew(&table_schema()))
        .unwrap();
    for id in 0..40 {
        let row = [
            ("id", Value::I64(id)),
            ("n", Value::I64(id)),
            ("s", Value::String(format!("row {id}").into())),
        ];
        write.insert_row(&table, &row).unwrap();
    }
    write.commit().unwrap();
}

/// The leaf of row `id` of table `rows`.
fn row_leaf(store: &Store, id: i64) -> u64 {
    let read = store.read().unwrap();
    let rows = read
        .table_trees(&read.open_table("rows").unwrap())
        .unwrap()
        .rows;
    let key = encode_key(&[ColumnType::I64], &[Value::I64(id)]).unwrap();
    krummi_tree::leaf_of(&read.inner, rows, &key).unwrap()
}

/// The first page and the page count of the value run of `key` in tree `name`.
fn run_of(store: &Store, name: &str, key: &[u8]) -> (u64, u64) {
    let read = store.read().unwrap();
    let root = catalog::lookup(&read.inner, name).unwrap().unwrap().root;
    krummi_tree::run_of(&read.inner, root, key)
        .unwrap()
        .unwrap()
}

/// Where the bytes of page `page` that a new read transaction sees live.
fn memory_of_page(store: &Store, page: u64) -> Memory {
    let read = store.read().unwrap();
    store
        .shared()
        .engine
        .memory_of(read.inner.page(page).unwrap())
        .unwrap()
}

/// AC-Q1 (Q1, Q2, I5): a leaf in the store's pages gets an entry from a write; a read reads its version in the scratch
/// area, and the transfer writes it in place and takes it out of the scratch area. A new read then reads it, and the
/// other pages the write changed, without checking their integrity or their layout, and finds the entry; after
/// reopening a store on disk, the first read of the leaf checks both.
#[test]
fn ac_q1_a_leaf_the_transfer_wrote_is_read_without_checks_until_the_store_reopens() {
    let tmp = tempfile::tempdir().unwrap();
    let disk = crate::check_tests::open_settled(tmp.path(), &StoreOptions::default()).unwrap();
    let temporary = Store::open_temporary(&StoreOptions::default()).unwrap();
    for (on_disk, store) in [(true, disk), (false, temporary)] {
        store.shared().engine.set_transfer_paused(true);
        commit_spaced_tree(&store, "t", 3);
        transfer(&store);
        let page = root_of(&store, "t");
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        write.set(&tree, b"added", b"entry").unwrap();
        write.commit().unwrap();

        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        assert_eq!(
            store
                .shared()
                .engine
                .memory_of(read.inner.page(page).unwrap()),
            Some(Memory::Versions)
        );
        assert_eq!(read.get(&tree, b"added").unwrap(), Some(&b"entry"[..]));
        drop(read);
        transfer(&store);
        assert_eq!(
            store.shared().engine.scratch_bytes(),
            0,
            "out of the scratch area"
        );

        let read = store.read().unwrap();
        let (value, made) = checks_of(&store, || {
            let tree = read.open_tree("t").unwrap();
            read.get(&tree, b"added").unwrap().map(<[u8]>::to_vec)
        });
        assert_eq!(value.as_deref(), Some(&b"entry"[..]));
        assert_eq!(
            made, NONE,
            "on disk {on_disk}: no check of what the transfer wrote"
        );
        assert_eq!(
            store
                .shared()
                .engine
                .memory_of(read.inner.page(page).unwrap()),
            Some(Memory::Data)
        );
        drop(read);
        if !on_disk {
            store.close().unwrap();
            continue;
        }
        store.close().unwrap();

        let store = crate::check_tests::open_settled(tmp.path(), &StoreOptions::default()).unwrap();
        // The vector index worker reads the catalog when the store opens (VI12), so its root is checked then.
        assert!(
            !store.shared().engine.is_remembered(page),
            "nothing remembered after reopening"
        );
        let read = store.read().unwrap();
        let (value, made) = checks_of(&store, || {
            let tree = read.open_tree("t").unwrap();
            read.get(&tree, b"added").unwrap().map(<[u8]>::to_vec)
        });
        assert_eq!(value.as_deref(), Some(&b"entry"[..]));
        assert!(
            store.shared().engine.is_remembered(page),
            "its integrity, checked"
        );
        assert!(made.integrity >= 1, "the leaf: {made:?}");
        assert_eq!(made.stored_layout, 1, "the leaf: {made:?}");
        drop(read);
        store.close().unwrap();
    }
}

/// AC-Q2 (Q1, I4): a free-space map page and a branch that a write changes, and values off their leaf that it stores,
/// of one page and of three: once the transfer writes them in place, the first read of the map page and of the branch
/// checks no integrity, and the first read of each value checks its integrity.
#[test]
fn ac_q2_the_transfer_remembers_pages_but_not_values_off_their_leaf() {
    on_both(|store| {
        store.shared().engine.set_transfer_paused(true);
        commit_spaced_tree(store, "t", 200);
        transfer(store);
        let root = root_of(store, "t");
        let one_page = pattern(max_inline_value(4) + 1, 1);
        let three_pages = pattern(
            3 * krummi_pager::format::PAGE_SIZE - krummi_pager::format::PAGE_HEADER_SIZE,
            3,
        );
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t").unwrap();
        // Keys after the last ones split the last leaf, which changes the root branch and takes pages from the map.
        for i in 1000..1040 {
            write.set(&tree, &long_key(i, 64), b"split").unwrap();
        }
        write.set(&tree, b"big1", &one_page).unwrap();
        write.set(&tree, b"big3", &three_pages).unwrap();
        write.commit().unwrap();
        assert_eq!(root_of(store, "t"), root, "the root keeps its page");
        transfer(store);
        assert_eq!(store.shared().engine.scratch_bytes(), 0);
        let runs = [run_of(store, "t", b"big1"), run_of(store, "t", b"big3")];
        assert_eq!(
            [runs[0].1, runs[1].1],
            [1, 3],
            "runs of one page and of three"
        );

        for page in [MAP_PAGE, root] {
            let (read, made) = checks_of(store, || store.shared().engine.read_in_place(page));
            read.unwrap();
            assert_eq!(made.integrity, 0, "page {page} is remembered");
        }
        for ((first, count), value) in runs.into_iter().zip([&one_page, &three_pages]) {
            assert!(!store.shared().engine.is_run_remembered(first, count));
            let read = store.read().unwrap();
            let tree = read.open_tree("t").unwrap();
            let key: &[u8] = if count == 1 { b"big1" } else { b"big3" };
            let (got, made) =
                checks_of(store, || read.get(&tree, key).unwrap().map(<[u8]>::to_vec));
            assert!(got.as_ref() == Some(value), "the value of {count} pages");
            assert_eq!(
                made.integrity, 1,
                "the value of {count} pages alone is checked"
            );
            assert!(store.shared().engine.is_run_remembered(first, count));
        }
        store.shared().engine.set_transfer_paused(false);
    });
}

/// AC-Q3 (Q1, G12): a page whose write in place fails with a system error stays forgotten: a read of it from the
/// store's pages checks its integrity; a page the same pass wrote before the failure is remembered.
#[test]
fn ac_q3_a_page_whose_transfer_write_fails_stays_forgotten() {
    let tmp = tempfile::tempdir().unwrap();
    let store = crate::check_tests::open_settled(tmp.path(), &StoreOptions::default()).unwrap();
    // The transfer runs on the transfer thread, which shares the faults of the thread that opened the store.
    store.shared().engine.set_transfer_paused(true);
    commit_spaced_tree(&store, "a", 3);
    commit_spaced_tree(&store, "b", 3);
    transfer(&store);
    let (a, b) = (root_of(&store, "a"), root_of(&store, "b"));
    let (first, second) = (a.min(b), a.max(b));
    for page in [first, second] {
        store.shared().engine.read_in_place(page).unwrap();
        assert!(store.shared().engine.is_remembered(page));
    }
    // One commit changes both leaves, and nothing else: the pass writes them in the order of their pages.
    let mut write = store.write().unwrap();
    for name in ["a", "b"] {
        let tree = write.open_tree(name).unwrap();
        write
            .set(&tree, &long_key(0, 64), b"another length")
            .unwrap();
    }
    write.commit().unwrap();

    // The first write of the pass succeeds and the next ones fail: the journal is idle, its commit durable.
    testing::reset();
    for nth in 1..9 {
        testing::inject(Op::Write, nth, Action::Fail(io::ErrorKind::Other));
    }
    assert!(!store.shared().engine.transfer_now(), "the transfer failed");
    testing::reset();
    assert!(
        store.shared().engine.is_remembered(first),
        "written before the failure"
    );
    assert!(
        !store.shared().engine.is_remembered(second),
        "its write failed"
    );
    let (read, made) = checks_of(&store, || store.shared().engine.read_in_place(second));
    read.unwrap();
    assert_eq!(made.integrity, 1, "its integrity is checked");
    store.close().unwrap();
}

/// AC-Q4 (Q1, Q5): a leaf the transfer wrote and remembered, with no version left to transfer, whose value of an entry
/// a byte of which is altered with the store open, keeping its layout: a read returns the altered byte without
/// failing; after reopening, the first read of the leaf fails with the damage class and its page.
#[test]
fn ac_q4_damage_of_a_written_page_is_found_after_reopening() {
    let tmp = tempfile::tempdir().unwrap();
    let mut store = crate::check_tests::open_settled(tmp.path(), &StoreOptions::default()).unwrap();
    commit_spaced_tree(&store, "t", 3);
    transfer(&store);
    let page = root_of(&store, "t");
    assert!(
        store.shared().engine.is_remembered(page),
        "the transfer wrote it"
    );
    assert_eq!(store.shared().engine.scratch_bytes(), 0);
    let value = value_of(1, 0);
    let read = store.read().unwrap();
    let payload = read.inner.page(page).unwrap();
    let at = payload
        .windows(value.len())
        .position(|w| w == value)
        .expect("the value in its leaf");
    drop(read);
    store
        .with_engine_mut(|engine| engine.flip_byte_in_place(page, at))
        .unwrap();
    let mut altered = value.clone();
    altered[0] ^= 0xFF;

    let read = store.read().unwrap();
    let tree = read.open_tree("t").unwrap();
    assert_eq!(
        read.get(&tree, &long_key(1, 64)).unwrap(),
        Some(&altered[..])
    );
    drop(read);
    store.close().unwrap();

    let store = crate::check_tests::open_settled(tmp.path(), &StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let tree = read.open_tree("t").unwrap();
    assert_eq!(damaged_page(read.get(&tree, &long_key(1, 64))), page);
    drop(read);
    store.close().unwrap();
}

/// The leaves of AC-Q5 and AC-Q7: of a normal tree `n`, a fixed-size tree `f` and table `rows`.
struct Leaves {
    normal: u64,
    fixed: u64,
    table: u64,
}

/// Commits the trees and the table of AC-Q5, moves them to the store's pages and reads them all once, so that every
/// node is remembered.
fn commit_q5(store: &Store) -> Leaves {
    commit_spaced_tree(store, "n", 200);
    crate::check_tests::commit_fixed(store, "f", 1500);
    commit_table(store);
    transfer(store);
    let leaves = Leaves {
        normal: leaf(store, root_of(store, "n"), &long_key(100, 64)),
        fixed: fixed_leaf(store, root_of(store, "f"), 700),
        table: row_leaf(store, 10),
    };
    assert_eq!(
        leaf(store, root_of(store, "n"), &long_key(101, 64)),
        leaves.normal
    );
    assert_eq!(fixed_leaf(store, root_of(store, "f"), 701), leaves.fixed);
    assert_eq!(row_leaf(store, 11), leaves.table);
    read_q5(store, 0);
    leaves
}

/// The fixed-size value of key `key` in round `round` of AC-Q5.
fn fixed_of(key: u64, round: u8) -> [u8; FIXED_LEN] {
    if round == 0 {
        fixed_value(key)
    } else {
        (key * 1000 + u64::from(round)).to_le_bytes()
    }
}

/// The `n` of row `id` in round `round` of AC-Q5: the same width always.
fn n_of(id: i64, round: u8) -> i64 {
    if round == 0 {
        id
    } else {
        -id - i64::from(round)
    }
}

/// Overwrites in one write two values of each leaf of AC-Q5 with others of the same length, the first one twice, as
/// round `round`, and commits.
fn overwrite_q5(store: &Store, round: u8) {
    let mut write = store.write().unwrap();
    let normal = write.open_tree("n").unwrap();
    let fixed = write.open_fixed_tree("f", FIXED_LEN).unwrap();
    let table = write.open_table("rows").unwrap();
    for (i, key, id) in [(100, 700, 10), (101, 701, 11), (100, 700, 10)] {
        write
            .set(&normal, &long_key(i, 64), &value_of(i, round))
            .unwrap();
        write.set_fixed(&fixed, key, &fixed_of(key, round)).unwrap();
        let row = [("id", Value::I64(id)), ("n", Value::I64(n_of(id, round)))];
        assert!(write.update_row(&table, &row).unwrap());
    }
    write.commit().unwrap();
}

/// Reads the values of AC-Q5 of round `round` in a new read transaction and checks them.
fn read_q5(store: &Store, round: u8) {
    let read = store.read().unwrap();
    let normal = read.open_tree("n").unwrap();
    let fixed = read.open_fixed_tree("f", FIXED_LEN).unwrap();
    let table = read.open_table("rows").unwrap();
    for (i, key, id) in [(100, 700, 10), (101, 701, 11)] {
        assert_eq!(
            read.get(&normal, &long_key(i, 64)).unwrap(),
            Some(&value_of(i, round)[..])
        );
        assert_eq!(
            read.get_fixed(&fixed, key).unwrap(),
            Some(&fixed_of(key, round)[..])
        );
        let row = read.get_row(&table, &[Value::I64(id)]).unwrap().unwrap();
        assert_eq!(row.value(1), Some(Value::I64(n_of(id, round))));
    }
}

/// AC-Q5 (Q2, Q3): leaves of several entries of a normal tree, of a fixed-size tree and of a table, where one write
/// overwrites two values of each with others of the same length, one of them twice, and commits, and another write
/// does the same. The first read of each published version by another transaction, from the scratch area and, after
/// the transfer, from the store's pages, makes no layout check and returns the new values.
#[test]
fn ac_q5_same_length_overwrites_keep_the_layout_check() {
    on_both(|store| {
        store.shared().engine.set_transfer_paused(true);
        let leaves = commit_q5(store);
        for round in [1, 2] {
            overwrite_q5(store, round);
            for page in [leaves.normal, leaves.fixed, leaves.table] {
                assert_eq!(
                    memory_of_page(store, page),
                    Memory::Versions,
                    "round {round}"
                );
            }
            let ((), made) = checks_of(store, || read_q5(store, round));
            assert_eq!(
                made.version_layout, 0,
                "round {round}: no layout check of the versions"
            );
            assert_eq!(made.stored_layout, 0, "round {round}");
        }
        transfer(store);
        let ((), made) = checks_of(store, || read_q5(store, 2));
        assert_eq!(made, NONE, "no check of what the transfer wrote");
        for page in [leaves.normal, leaves.fixed, leaves.table] {
            assert_eq!(memory_of_page(store, page), Memory::Data);
        }
        store.shared().engine.set_transfer_paused(false);
    });
}

/// What a write of AC-Q6 does besides overwriting the value of key 100 with one of the same length.
#[derive(Debug, Clone, Copy)]
enum Besides {
    /// Nothing else: the leaf keeps its check, the control of the others.
    Nothing,
    /// (a) Adds an entry to the leaf.
    Add,
    /// (b) Removes one.
    Remove,
    /// (c) Removes one and adds it back with the same value.
    RemoveAndAdd,
    /// (d) Changes another value to one of another length.
    OtherLength,
    /// (e) Stores a value off the leaf.
    OffLeaf,
    /// (f) Splits the leaf.
    Split,
    /// (g) Merges it.
    Merge,
}

/// AC-Q6 (Q3, H2): a leaf as in AC-Q5 where a write overwrites a value with one of the same length and, in the same
/// write, also changes the leaf otherwise: the first read of the committed leaf by another transaction checks its
/// layout. The other transaction is a write, whose record shows which versions a read checked.
#[test]
fn ac_q6_any_other_change_of_the_leaf_drops_the_check() {
    for besides in [
        Besides::Nothing,
        Besides::Add,
        Besides::Remove,
        Besides::RemoveAndAdd,
        Besides::OtherLength,
        Besides::OffLeaf,
        Besides::Split,
        Besides::Merge,
    ] {
        on_both(|store| {
            store.shared().engine.set_transfer_paused(true);
            commit_spaced_tree(store, "n", 200);
            transfer(store);
            let key = long_key(100, 64);
            let root = root_of(store, "n");
            let page = leaf(store, root, &key);
            // The keys of the leaf before the change.
            let read = store.read().unwrap();
            let tree = read.open_tree("n").unwrap();
            let leaf_keys: Vec<usize> = (0..200)
                .filter(|&i| {
                    let k = long_key(i, 64);
                    read.get(&tree, &k).unwrap().is_some()
                        && krummi_tree::leaf_of(&read.inner, root, &k).unwrap() == page
                })
                .collect();
            drop(read);
            let mut write = store.write().unwrap();
            let tree = write.open_tree("n").unwrap();
            assert!(
                leaf_keys.len() > 4,
                "{besides:?}: a leaf of several entries"
            );
            let other = *leaf_keys.iter().find(|&&i| i != 100).unwrap();
            write.set(&tree, &key, &value_of(100, 1)).unwrap();
            match besides {
                Besides::Nothing => {}
                Besides::Add => {
                    let mut added = long_key(100, 64);
                    added[4] = b'a';
                    write.set(&tree, &added, &value_of(100, 1)).unwrap();
                }
                Besides::Remove => assert!(write.remove(&tree, &long_key(other, 64)).unwrap()),
                Besides::RemoveAndAdd => {
                    assert!(write.remove(&tree, &long_key(other, 64)).unwrap());
                    write
                        .set(&tree, &long_key(other, 64), &value_of(other, 0))
                        .unwrap();
                }
                Besides::OtherLength => write.set(&tree, &long_key(other, 64), b"short").unwrap(),
                Besides::OffLeaf => {
                    let big = pattern(max_inline_value(64) + 1, 7);
                    write.set(&tree, &long_key(other, 64), &big).unwrap();
                }
                Besides::Split => {
                    for n in 0..40u8 {
                        let mut added = long_key(100, 64);
                        added[4] = b'a' + n % 26;
                        added[5] = b'a' + n / 26;
                        write.set(&tree, &added, &value_of(100, 1)).unwrap();
                    }
                }
                Besides::Merge => {
                    for &i in leaf_keys.iter().filter(|&&i| i != 100) {
                        assert!(write.remove(&tree, &long_key(i, 64)).unwrap());
                    }
                }
            }
            write.commit().unwrap();

            let other_write = store.write().unwrap();
            other_write.inner.record_layout();
            let tree = other_write.open_tree("n").unwrap();
            assert_eq!(
                other_write.get(&tree, &key).unwrap(),
                Some(&value_of(100, 1)[..])
            );
            let checked: BTreeSet<u64> = other_write
                .inner
                .take_layout_log()
                .into_iter()
                .filter_map(|event| match event {
                    LayoutEvent::Check(page) => Some(page),
                    _ => None,
                })
                .collect();
            drop(other_write);
            let page = leaf(store, root_of(store, "n"), &key);
            assert_eq!(
                memory_of_page(store, page),
                Memory::Versions,
                "{besides:?}: the committed version"
            );
            assert_eq!(
                checked.contains(&page),
                !matches!(besides, Besides::Nothing),
                "{besides:?}: whether its first read checks the leaf {page} (checked {checked:?})"
            );
            store.shared().engine.set_transfer_paused(false);
        });
    }
}

/// AC-Q7 (Q4): a leaf as in AC-Q5 whose copy the test damages, breaking its layout, after overwriting a value with one
/// of the same length, without the write reading it again: in debug builds, a read of it by another transaction fails
/// with the damage class and its page, from the scratch area and, after the transfer, from the store's pages.
#[cfg(debug_assertions)]
#[test]
fn ac_q7_debug_builds_check_a_leaf_that_kept_its_check() {
    on_both(|store| {
        store.shared().engine.set_transfer_paused(true);
        let leaves = commit_q5(store);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("n").unwrap();
        write
            .set(&tree, &long_key(100, 64), &value_of(100, 1))
            .unwrap();
        // More entries than the leaf holds, through the same-length path, so that the copy still keeps its check.
        write.inner.page_mut_same_layout(leaves.normal).unwrap()[2..4]
            .copy_from_slice(&u16::MAX.to_le_bytes());
        write.commit().unwrap();
        for transferred in [false, true] {
            if transferred {
                transfer(store);
            }
            let expected = if transferred {
                Memory::Data
            } else {
                Memory::Versions
            };
            assert_eq!(memory_of_page(store, leaves.normal), expected);
            let read = store.read().unwrap();
            let tree = read.open_tree("n").unwrap();
            assert_eq!(
                damaged_page(read.get(&tree, &long_key(100, 64))),
                leaves.normal,
                "transferred {transferred}"
            );
        }
        store.shared().engine.set_transfer_paused(false);
    });
}

/// Q4 and C6 of the remembered node layout check PRD (R4 of the code review of the written pages): four threads read at
/// once, many rounds each, the leaves of AC-Q5 that the transfer wrote from versions that kept their check, which debug
/// builds check on every read, and every other key of tree `n`, whose leaves passed a check of their own: every read
/// returns the committed values and none fails or waits for another.
#[cfg(debug_assertions)]
#[test]
fn debug_reads_of_kept_and_checked_leaves_run_at_once() {
    const READERS: usize = 4;
    const ROUNDS: usize = 50;
    on_both(|store| {
        store.shared().engine.set_transfer_paused(true);
        let leaves = commit_q5(store);
        overwrite_q5(store, 1);
        transfer(store);
        // Whether the read of a leaf of `n` finds it kept, so that debug builds check its layout.
        let kept = |page: u64| {
            let read = store.read().unwrap();
            let krummi_tx::PageCheck::Verified(code) = read.inner.page_and_check(page).unwrap().1
            else {
                panic!("leaf {page} in the store's pages");
            };
            read.inner.layout_kept(page, code)
        };
        let root = root_of(store, "n");
        let other = leaf(store, root, &long_key(0, 64));
        assert_ne!(other, leaves.normal, "a leaf of its own");
        assert!(kept(leaves.normal), "the overwritten leaf kept its check");
        assert!(!kept(other), "the other leaf passed a check of its own");
        thread::scope(|scope| {
            let readers: Vec<_> = (0..READERS)
                .map(|_| {
                    scope.spawn(|| {
                        for _ in 0..ROUNDS {
                            read_q5(store, 1);
                            let read = store.read().unwrap();
                            let tree = read.open_tree("n").unwrap();
                            for i in (0..200).filter(|i| i % 4 != 3) {
                                let round = u8::from(i == 100 || i == 101);
                                assert_eq!(
                                    read.get(&tree, &long_key(i, 64)).unwrap(),
                                    Some(&value_of(i, round)[..]),
                                    "key {i}"
                                );
                            }
                        }
                    })
                })
                .collect();
            for reader in readers {
                reader.join().unwrap();
            }
        });
        assert!(kept(leaves.normal), "reads add nothing to what was kept");
        store.shared().engine.set_transfer_paused(false);
    });
}

/// AC-Q9 (Q1–Q3, I7): four threads read every key of a tree of several leaves at once, a thousand rounds each at
/// least, while a write commits overwrites of the same length, values of another length and added and removed keys,
/// and the transfer writes them: every read returns the data of its snapshot and none fails.
#[test]
fn ac_q9_concurrent_reads_while_overwrites_are_transferred() {
    const KEYS: usize = 24;
    const GENERATIONS: usize = 30;
    const READERS: usize = 4;
    const ROUNDS: usize = 1000;
    let tmp = tempfile::tempdir().unwrap();
    for owned in [
        crate::check_tests::open_settled(tmp.path(), &StoreOptions::default()).unwrap(),
        Store::open_temporary(&StoreOptions::default()).unwrap(),
    ] {
        let store = &owned;
        let key = |i: usize| long_key(i, 1000);
        // Generation `g` stores this in every key: most generations keep the length of the one before, some change
        // it, and every third one also stores the key `extra`, which the others remove.
        let value = |g: usize| {
            let mut value = format!("g{g:03}").into_bytes();
            value.resize(4 + g / 7, b'.');
            value
        };
        let has_extra = |g: usize| g.is_multiple_of(3);
        let mut write = store.write().unwrap();
        let tree = write.open_tree_with("t", OpenMode::CreateNew).unwrap();
        for i in 0..KEYS {
            write.set(&tree, &key(i), &value(0)).unwrap();
        }
        write.set(&tree, b"extra", b"x").unwrap();
        write.commit().unwrap();
        transfer(store);

        let rounds: [AtomicUsize; READERS] = Default::default();
        let done = AtomicBool::new(false);
        let failed = AtomicBool::new(false);
        thread::scope(|scope| {
            // A failure on this thread stops the readers while it unwinds, before the scope waits for them.
            struct StopOnFailure<'a>(&'a AtomicBool);
            impl Drop for StopOnFailure<'_> {
                fn drop(&mut self) {
                    if thread::panicking() {
                        self.0.store(true, Ordering::SeqCst);
                    }
                }
            }
            let _stop = StopOnFailure(&failed);
            let readers: Vec<_> = rounds
                .iter()
                .map(|finished| {
                    let (done, failed) = (&done, &failed);
                    scope.spawn(move || {
                        let mut seen = BTreeSet::new();
                        let mut count = 0;
                        while !failed.load(Ordering::SeqCst)
                            && (count < ROUNDS || !done.load(Ordering::SeqCst))
                        {
                            let read = store.read().unwrap();
                            let tree = read.open_tree("t").unwrap();
                            let first = read.get(&tree, &key(0)).unwrap().unwrap().to_vec();
                            let generation: usize =
                                std::str::from_utf8(&first[1..4]).unwrap().parse().unwrap();
                            assert_eq!(first, value(generation), "a value of its generation");
                            for i in 1..KEYS {
                                assert_eq!(
                                    read.get(&tree, &key(i)).unwrap(),
                                    Some(&first[..]),
                                    "one snapshot"
                                );
                            }
                            assert_eq!(
                                read.get(&tree, b"extra").unwrap().is_some(),
                                has_extra(generation),
                                "generation {generation}"
                            );
                            seen.insert(generation);
                            count += 1;
                            finished.store(count, Ordering::SeqCst);
                        }
                        seen
                    })
                })
                .collect();
            // Waits until every reader has finished two more rounds, so that one of them started after this call.
            let two_rounds = || {
                let from: Vec<usize> = rounds.iter().map(|r| r.load(Ordering::SeqCst)).collect();
                while rounds
                    .iter()
                    .zip(&from)
                    .any(|(r, &from)| r.load(Ordering::SeqCst) < from + 2)
                {
                    assert!(
                        !readers.iter().any(|reader| reader.is_finished()),
                        "a reader stopped while the writer ran"
                    );
                    thread::yield_now();
                }
            };
            two_rounds();
            for generation in 1..=GENERATIONS {
                let mut write = store.write().unwrap();
                let tree = write.open_tree("t").unwrap();
                for i in 0..KEYS {
                    write.set(&tree, &key(i), &value(generation)).unwrap();
                }
                if has_extra(generation) {
                    write.set(&tree, b"extra", b"x").unwrap();
                } else {
                    write.remove(&tree, b"extra").unwrap();
                }
                write.commit().unwrap();
                two_rounds();
                transfer(store);
                assert_eq!(
                    store.shared().engine.scratch_bytes(),
                    0,
                    "the generation moved while the readers read"
                );
                two_rounds();
            }
            done.store(true, Ordering::SeqCst);
            let every: BTreeSet<usize> = (0..=GENERATIONS).collect();
            for reader in readers {
                assert_eq!(
                    reader.join().unwrap(),
                    every,
                    "a reader saw every generation"
                );
            }
        });
        for finished in &rounds {
            assert!(finished.load(Ordering::SeqCst) >= ROUNDS);
        }
        owned.close().unwrap();
    }
}
