//! What the public tests cannot reach without the engine's test hooks: a store that refuses writes (T12) while a
//! write transaction is open, and the failures of T11 at the page operations of a write (AC-B18, AC-R12, AC-R13,
//! with values off their leaf AC-V8 and with a multi-value tree AC-M14) and during its commit.

use std::io;
use std::thread;

use krummi_pager::testing::{self, Action, Op};

use super::*;
use crate::value_tests::mixed_value;

/// AC-B4 (B7, T12) and the T12 part of AC-M10 (M4, M12): once the store refuses writes, an invalid size and an
/// opening of a tree as the other kind, in every mode, in the open write still fail with their own classes, not with
/// the I/O class, and committing that write without changes ends without error.
#[test]
fn ac_b4_ac_m10_validation_failures_in_a_store_that_refuses_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    // The transfer that fails is the one the test runs on this thread, whose faults it injects.
    store.engine.set_transfer_paused(true);
    let mut write = store.write().unwrap();
    let tree = write.open_tree_with("t", OpenMode::CreateNew).unwrap();
    write.set(&tree, b"k", b"v").unwrap();
    let multi = write
        .open_multi_tree_with("m", OpenMode::CreateNew)
        .unwrap();
    write.add(&multi, b"k", b"v").unwrap();
    write.commit().unwrap();

    let mut write = store.write().unwrap();
    let tree = write.open_tree("t").unwrap();
    // The next eight writes fail, a margin over the first write of the transfer pass, which already makes it fail,
    // whichever thread sharing these faults writes first (the journal's writer adopts them); the transaction tests
    // use the same margin.
    for nth in 0..8 {
        testing::inject(Op::Write, nth, Action::Fail(io::ErrorKind::Other));
    }
    assert!(!store.engine.transfer_now(), "the transfer failed");
    testing::reset();
    let refused = thread::scope(|s| {
        s.spawn(|| {
            store
                .write_with_timeout(Duration::ZERO)
                .err()
                .map(|e| e.kind())
        })
        .join()
        .unwrap()
    });
    assert_eq!(refused, Some(ErrorKind::Io), "the store refuses writes");

    let error = write
        .set(&tree, &[0; krummi_tree::MAX_KEY + 1], b"v")
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidSize);
    let error = write
        .open_tree_with("", OpenMode::OpenOrCreate)
        .err()
        .unwrap();
    assert_eq!(error.kind(), ErrorKind::InvalidSize);
    let different = ErrorKind::DifferentTreeType;
    assert_eq!(write.open_tree("m").err().unwrap().kind(), different);
    assert_eq!(write.open_multi_tree("t").err().unwrap().kind(), different);
    for mode in [
        OpenMode::CreateNew,
        OpenMode::OpenOrCreate,
        OpenMode::CreateOrEmpty,
        OpenMode::Empty,
    ] {
        assert_eq!(
            write.open_tree_with("m", mode).err().unwrap().kind(),
            different
        );
        let error = write.open_multi_tree_with("t", mode).err().unwrap();
        assert_eq!(error.kind(), different);
    }
    write.commit().unwrap();

    store.close().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let tree = read.open_tree("t").unwrap();
    assert_eq!(read.get(&tree, b"k").unwrap(), Some(&b"v"[..]));
    let multi = read.open_multi_tree("m").unwrap();
    assert!(read.contains(&multi, b"k", b"v").unwrap());
}

/// A key of 2,048 bytes that starts with `i`: three fit in a leaf and three in a branch, so a few dozen of them
/// build a tree of several levels.
pub(crate) fn long_key(i: usize) -> Vec<u8> {
    let mut key = format!("{i:05}").into_bytes();
    key.resize(krummi_tree::MAX_KEY, b'.');
    key
}

/// Each tree a new read sees, with its count and what it holds for every key of the test, in order.
type Seen = Vec<(String, u64, Vec<Option<Vec<u8>>>)>;

/// What a new read sees of the trees, for the keys `keys` (see [`Seen`]).
fn trees_seen(store: &Store, keys: &[Vec<u8>]) -> Seen {
    let read = store.read().unwrap();
    let list = read.list_trees().unwrap();
    list.into_iter()
        .map(|(name, kind)| {
            assert_eq!(kind, TreeKind::Normal);
            let tree = read.open_tree(&name).unwrap();
            let values = keys
                .iter()
                .map(|key| read.get(&tree, key).unwrap().map(<[u8]>::to_vec))
                .collect();
            (name, read.len(&tree).unwrap(), values)
        })
        .collect()
}

/// The write of the T11 sweep: through `tree`, splits that reach the root, so the tree gains levels, and removals
/// whose merges bring it back down, with its operations' results. It goes on after a failure, as an application
/// may (T11).
fn splits_and_merges(write: &mut WriteTransaction<'_>) -> Vec<Result<()>> {
    let tree = match write.open_tree("t") {
        Ok(tree) => tree,
        Err(error) => return vec![Err(error)],
    };
    let mut results = Vec::new();
    for i in 12..48 {
        results.push(write.set(&tree, &long_key(i), b"split"));
    }
    for i in (0..48).filter(|i| i % 8 != 0) {
        results.push(write.remove(&tree, &long_key(i)).map(drop));
    }
    results.push(write.try_add(&tree, &long_key(100), b"added").map(drop));
    results
}

/// The write of the 5.2 sweep (In-order scan PRD: AC-R12, AC-R13): it splits pages, scans the tree from both
/// ends, removes intervals that merge pages and free subtrees over several levels, with a prefix and without, and
/// scans again. A scan that fails stops at both ends (R8).
fn range_removals(write: &mut WriteTransaction<'_>) -> Vec<Result<()>> {
    let tree = match write.open_tree("t") {
        Ok(tree) => tree,
        Err(error) => return vec![Err(error)],
    };
    let mut results = Vec::new();
    for i in 12..48 {
        results.push(write.set(&tree, &long_key(i), b"split"));
    }
    let ok = |results: &[Result<()>]| results.iter().all(Result::is_ok);
    results.push(scan_whole(write, &tree, ok(&results).then_some(48)));
    let (lower, upper) = (long_key(5), long_key(30));
    results.push(write.remove_range(&tree, &lower[..]..&upper[..]).map(drop));
    // The keys from 40 to 47.
    results.push(write.remove_range_prefix(&tree, b"0004", ..).map(drop));
    results.push(scan_whole(write, &tree, ok(&results).then_some(15)));
    results
}

/// Scans `tree` whole, alternating ends, and, when `count` is given, checks that it holds `count` entries in key
/// order; it is not given after an earlier failure of the write, because what the write's tree then holds is left to
/// the implementation (the write cannot commit). After a failure the scan returns nothing more from either end.
fn scan_whole(
    write: &WriteTransaction<'_>,
    tree: &WriteTree<'_>,
    count: Option<usize>,
) -> Result<()> {
    let mut scan = write.range(tree, ..)?;
    let (mut front, mut back) = (Vec::new(), Vec::new());
    loop {
        let step = if (front.len() + back.len()) % 2 == 0 {
            scan.next().map(|e| e.map(|(k, _)| front.push(k)))
        } else {
            scan.next_back().map(|e| e.map(|(k, _)| back.push(k)))
        };
        match step {
            None => break,
            Some(Ok(())) => {}
            Some(Err(error)) => {
                assert!(
                    scan.next().is_none() && scan.next_back().is_none(),
                    "the scan stopped"
                );
                return Err(error);
            }
        }
    }
    back.reverse();
    front.extend(back);
    if let Some(count) = count {
        assert_eq!(front.len(), count);
        assert!(
            front.windows(2).all(|pair| pair[0] < pair[1]),
            "in key order"
        );
    }
    Ok(())
}

/// AC-B18, second part, with its 2026-10-25 annotation (T11): a failure of disk full or maximum size at each
/// point where the write of [`splits_and_merges`] reads, changes, reserves or frees a page, also after it changed
/// many and in the middle of splits and merges over several levels, or where its commit writes the catalog, makes
/// that operation fail with it; the commit fails with that class and applies nothing. After the abort, or after
/// that failed commit, the tree, the catalog and the count are as before, and a new write reads and modifies them.
/// On disk and in a temporary store, at one point in `step`; once no point is left, the write commits.
fn t11_sweep(step: u64) {
    t11_sweep_of(
        step,
        named,
        splits_and_merges,
        &[Class::DiskFull, Class::MaxSize],
        700,
        |store| {
            let read = store.read().unwrap();
            let tree = read.open_tree("t").unwrap();
            assert_eq!(read.len(&tree).unwrap(), 7, "0, 8, 16, 24, 32, 40 and 100");
            assert_eq!(read.get(&tree, &long_key(40)).unwrap(), Some(&b"split"[..]));
        },
    );
}

/// AC-R12 with AC-B18's second part, and AC-R13's I/O failure in the middle of a scan: a failure of disk full,
/// maximum size or input and output at each point where the write of [`range_removals`] reads, changes, reserves
/// or frees a page, in its range removals and in its scans, fails that operation, and its commit fails with that
/// class and applies nothing (the rest as in [`t11_sweep`]).
fn t11_range_sweep(step: u64) {
    let classes = [Class::DiskFull, Class::MaxSize, Class::Io];
    t11_sweep_of(step, named, range_removals, &classes, 500, |store| {
        let read = store.read().unwrap();
        let tree = read.open_tree("t").unwrap();
        assert_eq!(read.len(&tree).unwrap(), 15, "0 to 4 and 30 to 39");
        assert_eq!(read.get(&tree, &long_key(30)).unwrap(), Some(&b"split"[..]));
        assert_eq!(read.get(&tree, &long_key(29)).unwrap(), None);
    });
}

/// The value of entry `i` of the tree `name` before the write of [`values_off_their_leaf`]: off its leaf for odd `i`,
/// in a run of one, two or three pages, and in its leaf for even `i`, different in each tree.
fn mixed(name: &str, i: usize) -> Vec<u8> {
    mixed_value(i, name.as_bytes()[0])
}

/// A value off its leaf for any `i`, in a run of one, two or three pages.
fn off_leaf(i: usize) -> Vec<u8> {
    mixed_value(2 * i + 1, 9)
}

/// The write of the T11 sweep with values off their leaf (Large values PRD: AC-V8), over the tree `t` of [`mixed`]:
/// it overwrites values off their leaf with larger and smaller ones and with a value in its leaf, a value in its leaf
/// with one off it, reads values off their leaf, adds over one (which reserves nothing, V1) and removes it, stores new
/// ones whose leaves split, and removes an interval that holds some. It goes on after a failure (T11).
fn values_off_their_leaf(write: &mut WriteTransaction<'_>) -> Vec<Result<()>> {
    let tree = match write.open_tree("t") {
        Ok(tree) => tree,
        Err(error) => return vec![Err(error)],
    };
    let mut results = Vec::new();
    let overwrites = [
        (1, off_leaf(2)),
        (5, off_leaf(1)),
        (3, b"in its leaf".to_vec()),
        (0, off_leaf(0)),
    ];
    for (i, value) in overwrites {
        results.push(write.set(&tree, &long_key(i), &value));
    }
    for i in [1, 7, 11] {
        results.push(write.get(&tree, &long_key(i)).map(drop));
    }
    results.push(write.try_add(&tree, &long_key(9), &off_leaf(3)).map(drop));
    results.push(write.remove(&tree, &long_key(9)).map(drop));
    for i in 12..20 {
        results.push(write.set(&tree, &long_key(i), &off_leaf(i)));
    }
    let (lower, upper) = (long_key(4), long_key(7));
    results.push(write.remove_range(&tree, &lower[..]..&upper[..]).map(drop));
    results
}

/// Checks the tree `t` once the write of [`values_off_their_leaf`] committed.
fn values_off_their_leaf_committed(store: &Store) {
    let read = store.read().unwrap();
    let tree = read.open_tree("t").unwrap();
    assert_eq!(read.len(&tree).unwrap(), 16, "0 to 3, 7, 8, 10 to 19");
    let get = |i: usize| read.get(&tree, &long_key(i)).unwrap().map(<[u8]>::to_vec);
    assert_eq!(get(1), Some(off_leaf(2)));
    assert_eq!(get(3), Some(b"in its leaf".to_vec()));
    assert_eq!(get(5), None);
    assert_eq!(get(7), Some(mixed("t", 7)));
    assert_eq!(get(9), None);
    assert_eq!(get(19), Some(off_leaf(19)));
}

/// AC-V8, second part (V8, T11, AC-B18): a failure of disk full, maximum size, memory exhausted or input and output
/// at each point where the write of [`values_off_their_leaf`] reserves (a value run included), reads or frees a page,
/// or where its commit writes the catalog, fails that operation; the commit fails with that class and applies
/// nothing, and after the abort, or that failed commit, the trees and their counts are as before and a new write
/// works (the rest as in [`t11_sweep`]).
fn t11_values_sweep(step: u64) {
    let classes = [
        Class::DiskFull,
        Class::MaxSize,
        Class::OutOfMemory,
        Class::Io,
    ];
    t11_sweep_of(
        step,
        mixed,
        values_off_their_leaf,
        &classes,
        140,
        values_off_their_leaf_committed,
    );
}

/// AC-V8 at every 13th point of the write with values off their leaf.
#[test]
fn ac_v8_a_failure_at_a_sample_of_page_operations_with_values_off_their_leaf_applies_nothing() {
    t11_values_sweep(13);
}

/// `cargo test -p krummi --lib -- --ignored ac_v8_a_failure_at_every`.
#[test]
#[ignore = "acceptance: a failure at every page operation of the write and its commit"]
fn ac_v8_a_failure_at_every_page_operation_with_values_off_their_leaf_applies_nothing() {
    t11_values_sweep(1);
}

/// Bytes of `len` that do not compress, different for each `seed`, so that a journal record holding them is as long.
fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect()
}

/// AC-V8, third part (V8, G7, P13, T11): the write of [`values_off_their_leaf`] plus, in the tree `other`, a value
/// off its leaf larger than
/// a journal file and than the first chunk of the temporary area, so that its commit makes the temporary area grow
/// and creates a journal file for its record. Running out of disk where the commit grows the temporary area or creates
/// that file (on disk), or out of memory where it reserves memory (on disk and in a temporary store), at each such
/// point, fails the commit with that class and applies nothing, and a new write works.
#[test]
fn ac_v8_a_full_disk_or_no_memory_during_the_commit_of_values_off_their_leaf_applies_nothing() {
    use krummi_pager::format::PAGE_SIZE;
    let tmp = tempfile::tempdir().unwrap();
    let large = noise(48 * PAGE_SIZE, 7);
    let keys = sweep_keys();
    for (on_disk, op, io_kind, class) in [
        (
            true,
            Op::Temp,
            io::ErrorKind::StorageFull,
            ErrorKind::DiskFull,
        ),
        (
            true,
            Op::Reserve,
            io::ErrorKind::StorageFull,
            ErrorKind::DiskFull,
        ),
        (
            true,
            Op::Alloc,
            io::ErrorKind::OutOfMemory,
            ErrorKind::OutOfMemory,
        ),
        (
            false,
            Op::Alloc,
            io::ErrorKind::OutOfMemory,
            ErrorKind::OutOfMemory,
        ),
    ] {
        let store = if on_disk {
            let folder = tmp.path().join(format!("{op:?}"));
            Store::open(&folder, &StoreOptions::default()).unwrap()
        } else {
            Store::open_temporary(&StoreOptions::default()).unwrap()
        };
        create_trees(&store, mixed);
        let before = trees_seen(&store, &keys);
        let mut failures = 0;
        loop {
            let label = format!("{op:?} on disk {on_disk}, failure {failures}");
            let mut write = store.write().unwrap();
            let mut results = values_off_their_leaf(&mut write);
            let other = write.open_tree("other").unwrap();
            results.push(write.set(&other, b"large", &large));
            assert!(results.iter().all(Result::is_ok), "{label}");
            testing::reset();
            testing::inject(op, failures, Action::Fail(io_kind));
            let committed = write.commit();
            let fired = testing::count(op) > failures;
            testing::reset();
            match committed {
                Ok(()) => {
                    // The commit succeeds only once the failure no longer fires: an injected failure is never
                    // swallowed.
                    assert!(
                        !fired,
                        "{label}: the injected failure fired and the commit succeeded"
                    );
                    values_off_their_leaf_committed(&store);
                    let read = store.read().unwrap();
                    let other = read.open_tree("other").unwrap();
                    assert!(read.get(&other, b"large").unwrap() == Some(&large[..]));
                    break;
                }
                Err(error) => {
                    assert!(fired, "{label}: {error}");
                    assert_eq!(error.kind(), class, "{label}: {error}");
                    assert!(
                        trees_seen(&store, &keys) == before,
                        "{label}: nothing applied"
                    );
                    a_new_write_works(&store, mixed, &label);
                    failures += 1;
                }
            }
        }
        assert!(failures > 0, "{op:?}: the commit failed at least once");
        store.close().unwrap();
    }
}

/// The failures a T11 sweep injects, in turns.
#[derive(Clone, Copy)]
enum Class {
    DiskFull,
    MaxSize,
    OutOfMemory,
    Io,
}

impl Class {
    fn error(self, folder: &std::path::Path) -> Error {
        match self {
            Class::DiskFull => Error::DiskFull {
                folder: folder.to_path_buf(),
            },
            Class::MaxSize => Error::MaxSize {
                max_size: 1 << 20,
                needed: 2 << 20,
            },
            Class::OutOfMemory => Error::OutOfMemory { requested: 1 << 20 },
            Class::Io => Error::Io {
                folder: folder.to_path_buf(),
                source: io::Error::other("injected"),
            },
        }
    }
}

/// The value of entry `i` of the tree `name` before the write of a T11 sweep: the tree's name, in its leaf.
fn named(name: &str, _: usize) -> Vec<u8> {
    name.as_bytes().to_vec()
}

/// The keys the T11 sweeps look up: every key their writes use.
fn sweep_keys() -> Vec<Vec<u8>> {
    (0..48).chain([100]).map(long_key).collect()
}

/// Commits the trees `other` and `t` of 12 entries each, the value of entry `i` of tree `name` being `value(name, i)`.
fn create_trees(store: &Store, value: fn(&str, usize) -> Vec<u8>) {
    let mut write = store.write().unwrap();
    for name in ["other", "t"] {
        let tree = write.open_tree_with(name, OpenMode::CreateNew).unwrap();
        for i in 0..12 {
            write.set(&tree, &long_key(i), &value(name, i)).unwrap();
        }
    }
    write.commit().unwrap();
}

/// Checks that a new write reads, modifies and commits the tree `t` of [`create_trees`], unchanged.
fn a_new_write_works(store: &Store, value: fn(&str, usize) -> Vec<u8>, label: &str) {
    let mut write = store.write().unwrap();
    let tree = write.open_tree("t").unwrap();
    assert_eq!(write.len(&tree).unwrap(), 12, "{label}");
    write.set(&tree, b"probe", b"x").unwrap();
    let last = value("t", 11);
    assert_eq!(
        write.get(&tree, &long_key(11)).unwrap(),
        Some(&last[..]),
        "{label}"
    );
    assert!(write.remove(&tree, b"probe").unwrap());
    write.commit().unwrap();
}

/// The T11 sweep of the write `change` on a store with the trees `other` and `t` of 12 entries each, the value of
/// entry `i` of tree `name` being `value(name, i)`, with the failures of `classes` in turns; the write must have more
/// than `min_points` points, and `finished` checks the store once it commits.
fn t11_sweep_of(
    step: u64,
    value: fn(&str, usize) -> Vec<u8>,
    change: fn(&mut WriteTransaction<'_>) -> Vec<Result<()>>,
    classes: &[Class],
    min_points: u64,
    finished: impl Fn(&Store),
) {
    let keys = sweep_keys();
    let fixture = Fixture {
        create: &|store| create_trees(store, value),
        seen: &|store| trees_seen(store, &keys),
        new_write_works: &|store, label| a_new_write_works(store, value, label),
    };
    t11_sweep_with(step, &fixture, change, classes, min_points, finished);
}

/// The trees of a T11 sweep: how it creates them, what a new read sees of them, and the check that a new write reads,
/// modifies and commits them, unchanged, with a label for its failures.
struct Fixture<'a, T> {
    create: &'a dyn Fn(&Store),
    seen: &'a dyn Fn(&Store) -> T,
    new_write_works: &'a dyn Fn(&Store, &str),
}

/// The T11 sweep of [`t11_sweep_of`] on the trees of `fixture`: on disk and in a temporary store, a failure of
/// `classes`, in turns, at each point in `step` where the write `change` reads, changes, reserves or frees a page or
/// its commit does, until no point is left and the write commits.
fn t11_sweep_with<T: PartialEq>(
    step: u64,
    fixture: &Fixture<'_, T>,
    change: fn(&mut WriteTransaction<'_>) -> Vec<Result<()>>,
    classes: &[Class],
    min_points: u64,
    finished: impl Fn(&Store),
) {
    // The checks are about T11, not about durability: the store's flushes to disk are skipped.
    testing::skip_syncs(true);
    let tmp = tempfile::tempdir().unwrap();
    let on_disk = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let in_memory = Store::open_temporary(&StoreOptions::default()).unwrap();
    for store in [&on_disk, &in_memory] {
        (fixture.create)(store);
        let before = (fixture.seen)(store);
        let mut points = 0;
        loop {
            let failure = classes[(points / step) as usize % classes.len()].error(tmp.path());
            let class = failure.kind();
            let mut write = store.write().unwrap();
            store.engine.set_page_fault(Some((points, failure)));
            let results = change(&mut write);
            let label = format!("point {points}");
            if store.engine.page_fault_pending() {
                assert!(results.iter().all(Result::is_ok), "{label}");
                let committed = write.commit();
                if store.engine.page_fault_pending() {
                    store.engine.set_page_fault(None);
                    committed.unwrap();
                    break;
                }
                assert_eq!(
                    committed.unwrap_err().kind(),
                    class,
                    "{label}: in the commit"
                );
            } else {
                let first = results.iter().find_map(|r| r.as_ref().err());
                assert_eq!(
                    first.map(Error::kind),
                    Some(class),
                    "{label}: the failing operation"
                );
                if points % 4 < 2 {
                    write.abort();
                } else {
                    assert_eq!(write.commit().unwrap_err().kind(), class, "{label}: commit");
                }
            }
            assert!((fixture.seen)(store) == before, "{label}: nothing applied");
            (fixture.new_write_works)(store, &label);
            points += step;
        }
        assert!(points > min_points, "{points} points");
        finished(store);
    }
    on_disk.close().unwrap();
    testing::reset();
}

/// AC-B18 at every 13th point: an odd step, so that both classes, aborts and failed commits take turns.
#[test]
fn ac_b18_a_failure_at_a_sample_of_page_operations_of_splits_and_merges_applies_nothing() {
    t11_sweep(13);
}

/// AC-R12 and AC-R13 at every 13th point of the write with range removals and scans.
#[test]
fn ac_r12_a_failure_at_a_sample_of_page_operations_of_range_removals_and_scans_applies_nothing() {
    t11_range_sweep(13);
}

/// `cargo test -p krummi --lib -- --ignored ac_r12_a_failure_at_every`.
#[test]
#[ignore = "acceptance: a failure at every page operation of the write and its commit"]
fn ac_r12_a_failure_at_every_page_operation_of_range_removals_and_scans_applies_nothing() {
    t11_range_sweep(1);
}

/// `cargo test -p krummi --lib -- --ignored ac_b18_a_failure_at_every`.
#[test]
#[ignore = "acceptance: a failure at every page operation of the write and its commit"]
fn ac_b18_a_failure_at_every_page_operation_of_splits_and_merges_applies_nothing() {
    t11_sweep(1);
}

/// A value of 16 bytes, the decimal digits of `i`, so that values sort by `i`.
fn value16(i: usize) -> Vec<u8> {
    format!("{i:016}").into_bytes()
}

/// A key of 2,000 bytes that starts with `i`: the keys of the multi-value sweep that make its tree split and merge.
fn filler(i: usize) -> Vec<u8> {
    let mut key = long_key(i);
    key.truncate(2000);
    key
}

/// Commits the multi-value tree `m` of the T11 sweep: `p` with its values in its entry, two additions below the
/// promotion threshold; `d` with its values in a subtree, two removals above the demotion threshold; `s` with values
/// of 2,048 bytes in a subtree of two leaves; and six keys of 2,000 bytes.
fn create_multi_tree(store: &Store) {
    let mut write = store.write().unwrap();
    let tree = write
        .open_multi_tree_with("m", OpenMode::CreateNew)
        .unwrap();
    for i in 0..150 {
        if i < 112 {
            write.add(&tree, b"p", &value16(i)).unwrap();
        }
        write.add(&tree, b"d", &value16(i)).unwrap();
    }
    for i in 58..150 {
        write.remove_value(&tree, b"d", &value16(i)).unwrap();
    }
    for j in 0..4 {
        write.add(&tree, b"s", &long_key(j)).unwrap();
    }
    for i in 0..6 {
        write.add(&tree, &filler(i), b"x").unwrap();
    }
    assert!(write.subtree_of(&tree, b"p").unwrap().is_none());
    assert!(write.subtree_of(&tree, b"d").unwrap().is_some());
    write.commit().unwrap();
}

/// The number of entries of a multi-value tree, and each key with its number of values and its values.
type MultiSeen = (u64, Vec<(Vec<u8>, u64, Vec<Vec<u8>>)>);

/// What a new read sees of the multi-value tree `m`: its number of entries and each key with its number of values and
/// its values.
fn multi_seen(store: &Store) -> MultiSeen {
    let read = store.read().unwrap();
    let tree = read.open_multi_tree("m").unwrap();
    let keys = read
        .keys(&tree, ..)
        .unwrap()
        .map(|item| {
            let (key, count) = item.unwrap();
            let values = read
                .values(&tree, key, ..)
                .unwrap()
                .map(|value| value.unwrap().to_vec())
                .collect();
            (key.to_vec(), count, values)
        })
        .collect();
    (read.len(&tree).unwrap(), keys)
}

/// Checks that a new write reads, modifies and commits the multi-value tree `m` of [`create_multi_tree`].
fn a_new_multi_write_works(store: &Store, label: &str) {
    let mut write = store.write().unwrap();
    let tree = write.open_multi_tree("m").unwrap();
    assert_eq!(write.len(&tree).unwrap(), 9, "{label}");
    assert_eq!(write.value_count(&tree, b"p").unwrap(), 112, "{label}");
    assert!(write.add(&tree, b"p", b"probe").unwrap(), "{label}");
    assert!(
        write.contains(&tree, b"s", &long_key(3)).unwrap(),
        "{label}"
    );
    assert!(
        write.remove_value(&tree, b"p", b"probe").unwrap(),
        "{label}"
    );
    write.commit().unwrap();
}

/// The write of the multi-value T11 sweep (Multi-value trees PRD: AC-M14), over the tree of [`create_multi_tree`]: it
/// moves the values of `p` to a subtree and those of `d` back to its entry, splits and merges the subtree of `s` and
/// the tree, reads and scans keys and values, removes a range of keys, and moves `p` back to its entry with a range of
/// values. It goes on after a failure (T11).
fn multi_values(write: &mut WriteTransaction<'_>) -> Vec<Result<()>> {
    let tree = match write.open_multi_tree("m") {
        Ok(tree) => tree,
        Err(error) => return vec![Err(error)],
    };
    let mut results = Vec::new();
    for i in 112..116 {
        results.push(write.add(&tree, b"p", &value16(i)).map(drop));
    }
    for i in (53..58).rev() {
        results.push(write.remove_value(&tree, b"d", &value16(i)).map(drop));
    }
    results.push(write.contains(&tree, b"p", &value16(0)).map(drop));
    results.push(write.value_count(&tree, b"d").map(drop));
    for j in 4..12 {
        results.push(write.add(&tree, b"s", &long_key(j)).map(drop));
    }
    for j in (0..12).filter(|j| j % 3 != 0) {
        results.push(write.remove_value(&tree, b"s", &long_key(j)).map(drop));
    }
    for i in 6..14 {
        results.push(write.add(&tree, &filler(i), b"x").map(drop));
    }
    for i in (0..14).filter(|i| i % 2 == 1) {
        results.push(write.remove_key(&tree, &filler(i)).map(drop));
    }
    let scanned = write.values(&tree, b"s", ..).and_then(|scan| {
        scan.collect::<Result<Vec<_>>>()
            .map(|values| assert!(values.len() <= 12))
    });
    results.push(scanned);
    let scanned = write
        .keys(&tree, ..)
        .and_then(|mut scan| scan.try_for_each(|item| item.map(drop)));
    results.push(scanned);
    let (lower, upper) = (filler(10), filler(14));
    let removed = write.remove_key_range(&tree, &lower[..]..&upper[..]);
    results.push(removed.map(drop));
    let from = value16(40);
    let removed = write.remove_value_range(&tree, b"p", &from[..]..);
    results.push(removed.map(drop));
    results
}

/// Checks the tree `m` once the write of [`multi_values`] committed: what the same write commits on a new store, and
/// where the values of `p`, `d` and `s` are.
fn multi_values_committed(store: &Store) {
    let fresh = Store::open_temporary(&StoreOptions::default()).unwrap();
    create_multi_tree(&fresh);
    let mut write = fresh.write().unwrap();
    assert!(multi_values(&mut write).iter().all(Result::is_ok));
    write.commit().unwrap();
    assert!(multi_seen(store) == multi_seen(&fresh), "the write's tree");
    let read = store.read().unwrap();
    let tree = read.open_multi_tree("m").unwrap();
    assert_eq!(read.value_count(&tree, b"p").unwrap(), 40);
    assert!(read.subtree_of(&tree, b"p").unwrap().is_none(), "p demoted");
    assert!(read.subtree_of(&tree, b"d").unwrap().is_none(), "d demoted");
    assert!(read.subtree_of(&tree, b"s").unwrap().is_some());
}

/// AC-M14, second part (T11, AC-B18): a failure of disk full, maximum size or memory exhausted at each point where
/// the write of [`multi_values`] reads, changes, reserves or frees a page, also while it moves the values of a key to
/// a subtree or back, or where its commit writes the catalog, fails that operation; the commit fails with that class
/// and applies nothing, and after the abort, or that failed commit, the tree, its keys, their values and the number
/// of entries are as before and a new write works (the rest as in [`t11_sweep`]).
fn t11_multi_sweep(step: u64) {
    let classes = [Class::DiskFull, Class::MaxSize, Class::OutOfMemory];
    let fixture = Fixture {
        create: &create_multi_tree,
        seen: &multi_seen,
        new_write_works: &a_new_multi_write_works,
    };
    t11_sweep_with(
        step,
        &fixture,
        multi_values,
        &classes,
        MULTI_POINTS,
        multi_values_committed,
    );
}

/// Fewer points than the write of [`multi_values`] has (about 920), so that its sweep fails if the write stops reaching
/// the pages it reaches now.
const MULTI_POINTS: u64 = 900;

/// AC-M14 at every 13th point of the write of a multi-value tree.
#[test]
fn ac_m14_a_failure_at_a_sample_of_page_operations_of_a_multi_value_tree_applies_nothing() {
    t11_multi_sweep(13);
}

/// `cargo test -p krummi --lib -- --ignored ac_m14_a_failure_at_every`.
#[test]
#[ignore = "acceptance: a failure at every page operation of the write and its commit"]
fn ac_m14_a_failure_at_every_page_operation_of_a_multi_value_tree_applies_nothing() {
    t11_multi_sweep(1);
}
