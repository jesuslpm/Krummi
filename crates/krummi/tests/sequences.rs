//! Sequences through the public API (tables with schema PRD 7.1, E14): their values across commits, aborts, row
//! deletes, an emptied table and a crash of the process, the first value, an exhausted sequence and the classes of the
//! catalog (AC11, the sequence half of AC-N4), Exists and List across the three classes (AC2), writes that change
//! nothing (E16, AC13), the order of the errors (E17, AC14) and the operations that never read a sequence's own page
//! (E19, AC19). That a read cannot ask for a value, and the references that asking ends, are compile-time tests in the
//! documentation of the sequences (`src/table/sequence.rs`).
//!
//! Each criterion runs on a store on disk and on a temporary one, except the parts that reopen, crash or damage the
//! store's files, which run on disk only.

#[path = "../../krummi-tx/tests/common/mod.rs"]
mod common;
mod support;

use std::path::Path;

use common::Run;
use krummi::table::value::{ColumnType, Value};
use krummi::{
    Column, Error, ErrorKind, OpenMode, Schema, SequenceMode, SizeSubject, Store, TableMode,
    TreeKind, ValueRule, WriteSequence, WriteTransaction,
};
use krummi_tree::catalog;
use support::*;

// ---------------------------------------------------------------------------------------------------------------
// Helpers

const SEQ: TreeKind = TreeKind::Sequence;

/// A table with one column `k`, of type `i64` without nulls, as its primary key.
fn single() -> Schema {
    Schema::new(vec![Column::new("k", ColumnType::I64).not_null()], &["k"])
}

fn create<'s>(
    write: &mut WriteTransaction<'s>,
    name: &str,
    first: Option<i64>,
) -> WriteSequence<'s> {
    write
        .open_sequence_with(name, SequenceMode::CreateNew(first))
        .unwrap()
}

/// Commits a new sequence `name` whose first value is `first`.
fn commit_sequence(store: &Store, name: &str, first: Option<i64>) {
    let mut write = store.write().unwrap();
    create(&mut write, name, first);
    write.commit().unwrap();
}

/// The next `n` values of the sequence `name` in a new write, which then commits or aborts.
fn take(store: &Store, name: &str, n: usize, commit: bool) -> Vec<i64> {
    let mut write = store.write().unwrap();
    let sequence = write.open_sequence(name).unwrap();
    let values = (0..n)
        .map(|_| write.next_value(&sequence).unwrap())
        .collect();
    if commit {
        write.commit().unwrap();
    } else {
        write.abort();
    }
    values
}

/// The sequence of an exhausted-sequence error, after checking its folder and that it names no table or column.
#[track_caller]
fn exhausted(error: Error, folder: Option<&Path>) -> String {
    match error {
        Error::InvalidValue {
            folder: f,
            table: None,
            column: None,
            sequence: Some(sequence),
            rule: ValueRule::SequenceExhausted,
        } => {
            assert_eq!(f, folder_of(folder));
            sequence
        }
        other => panic!("expected an exhausted sequence, got {other}"),
    }
}

/// Copies the closed store at `from` into a new folder.
fn copy_of(from: &Path) -> tempfile::TempDir {
    let copy = tempfile::tempdir().unwrap();
    for (path, bytes) in files(from) {
        std::fs::write(copy.path().join(path.file_name().unwrap()), bytes).unwrap();
    }
    copy
}

// ---------------------------------------------------------------------------------------------------------------
// AC11: values

/// AC11 and AC-N4 (E14, E3): the values of a sequence continue from the last committed one after a committed write,
/// an aborted write (whose values are handed out again), deleting the row of the highest value, emptying a table and
/// a crash of the process, and never repeat a committed value.
#[test]
fn ac11_values_continue_after_aborts_rows_and_a_crash() {
    let source = Run::begin();
    let store = open(source.path());
    let mut committed = Vec::new();

    let mut write = store.write().unwrap();
    let ids = create(&mut write, "ids", None);
    let table = write
        .open_table_with("t", TableMode::CreateNew(&single()))
        .unwrap();
    for _ in 0..3 {
        let k = write.next_value(&ids).unwrap();
        write.insert_row(&table, &[("k", Value::I64(k))]).unwrap();
        committed.push(k);
    }
    write.commit().unwrap();
    assert_eq!(committed, [1, 2, 3]);

    assert_eq!(take(&store, "ids", 2, false), [4, 5], "an aborted write");
    let mut write = store.write().unwrap();
    let ids = write.open_sequence("ids").unwrap();
    assert_eq!(
        write.next_value(&ids).unwrap(),
        4,
        "the aborted values again"
    );
    committed.push(4);
    let table = write.open_table("t").unwrap();
    assert!(write.delete_row(&table, &[Value::I64(3)]).unwrap());
    write.commit().unwrap();

    let mut write = store.write().unwrap();
    write.open_table_with("t", TableMode::Empty).unwrap();
    write.commit().unwrap();
    assert_eq!(take(&store, "ids", 1, true), [5], "after the emptied table");
    committed.push(5);
    // A write that the crash interrupts before its commit.
    let mut pending = store.write().unwrap();
    let ids = pending.open_sequence("ids").unwrap();
    assert_eq!(pending.next_value(&ids).unwrap(), 6);

    let run = Run::finish(source, Vec::new());
    drop(pending);
    drop(store);
    let killed = run.killed();
    let store = open(killed.path());
    let after = take(&store, "ids", 3, true);
    assert_eq!(after, [6, 7, 8], "they continue from the last committed");
    assert!(after.iter().all(|value| !committed.contains(value)));
    let read = store.read().unwrap();
    assert_eq!(read.row_count(&read.open_table("t").unwrap()).unwrap(), 0);
    drop(read);
    store.close().unwrap();
}

/// AC11 (E14): a sequence created with first value 1,000 gives 1,000 and then one more each time; Open or create with
/// another first value opens it unchanged; without a first value it starts at 1; any first value works, negative ones
/// too; and the values are the transaction's, seen by a later write and by every handle of the sequence.
#[test]
fn ac11_the_first_value_and_open_or_create() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let thousand = create(&mut write, "thousand", Some(1_000));
        assert_eq!(write.next_value(&thousand).unwrap(), 1_000);
        let again = write
            .open_sequence_with("thousand", SequenceMode::OpenOrCreate(Some(5)))
            .unwrap();
        assert_eq!(write.next_value(&again).unwrap(), 1_001);
        assert_eq!(write.next_value(&thousand).unwrap(), 1_002);
        let one = write
            .open_sequence_with("one", SequenceMode::OpenOrCreate(None))
            .unwrap();
        assert_eq!(write.next_value(&one).unwrap(), 1);
        let negative = create(&mut write, "negative", Some(i64::MIN));
        assert_eq!(write.next_value(&negative).unwrap(), i64::MIN);
        assert_eq!(write.next_value(&negative).unwrap(), i64::MIN + 1);
        let error = write
            .open_sequence_with("one", SequenceMode::CreateNew(Some(9)))
            .err()
            .unwrap();
        assert_eq!(already_exists(error, folder), "one");
        assert_eq!(
            not_found(write.open_sequence("missing").err().unwrap(), folder),
            "missing"
        );
        write.commit().unwrap();

        let mut write = store.write().unwrap();
        let thousand = write
            .open_sequence_with("thousand", SequenceMode::OpenOrCreate(Some(-3)))
            .unwrap();
        assert_eq!(write.next_value(&thousand).unwrap(), 1_003);
        write.commit().unwrap();
        assert_eq!(take(store, "one", 1, true), [2]);
        let read = store.read().unwrap();
        read.open_sequence("thousand").unwrap();
        assert_eq!(
            not_found(read.open_sequence("missing").err().unwrap(), folder),
            "missing"
        );
    });
}

/// AC11 (E14, E17): after handing out `i64::MAX`, asking for a value fails with an invalid value that names the
/// sequence, without effect: again and again, in a later write too, and the transaction commits its other changes.
#[test]
fn ac11_an_exhausted_sequence_fails_without_effect() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let full = create(&mut write, "full", Some(i64::MAX - 1));
        assert_eq!(write.next_value(&full).unwrap(), i64::MAX - 1);
        assert_eq!(write.next_value(&full).unwrap(), i64::MAX);
        for _ in 0..2 {
            let error = write.next_value(&full).err().unwrap();
            assert_eq!(exhausted(error, folder), "full");
        }
        let last = create(&mut write, "last", Some(i64::MAX));
        assert_eq!(write.next_value(&last).unwrap(), i64::MAX);
        write.rename_sequence("last", "renamed", false).unwrap();
        let error = write.next_value(&last).err().unwrap();
        assert_eq!(exhausted(error, folder), "renamed", "its current name");
        write.open_tree_with("other", OpenMode::CreateNew).unwrap();
        write.commit().unwrap();

        let mut write = store.write().unwrap();
        let full = write.open_sequence("full").unwrap();
        let error = write.next_value(&full).err().unwrap();
        assert_eq!(exhausted(error, folder), "full");
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert!(read.tree_exists("other").unwrap());
    });
}

/// B1 (E14): a handle of a sequence used in another write fails with misuse, without effect.
#[test]
fn ac11_a_handle_of_another_write_is_misuse() {
    stores(|store, folder| {
        commit_sequence(store, "s", None);
        let first = store.write().unwrap();
        let handle = first.open_sequence("s").unwrap();
        first.abort();
        let mut write = store.write().unwrap();
        misuse(write.next_value(&handle).err().unwrap(), folder);
        let own = write.open_sequence("s").unwrap();
        assert_eq!(write.next_value(&own).unwrap(), 1, "no effect");
        write.commit().unwrap();
    });
}

// ---------------------------------------------------------------------------------------------------------------
// E14 and N1: the catalog

/// E14 (E2, A3, A6): Rename keeps the sequence's next value, its identity and its handles; renaming to its own name
/// does nothing; with overwrite, a sequence at the destination is deleted with its handles; Delete removes the
/// sequence, its handles fail with not found, and a sequence created again with its name starts anew.
#[test]
fn e14_rename_and_delete_a_sequence() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let a = create(&mut write, "a", None);
        let c = create(&mut write, "c", Some(100));
        assert_eq!(write.next_value(&a).unwrap(), 1);
        let other = write.open_sequence("a").unwrap();
        write.rename_sequence("a", "a", false).unwrap();
        write.rename_sequence("a", "a", true).unwrap();
        write.rename_sequence("a", "b", false).unwrap();
        assert_eq!(write.next_value(&other).unwrap(), 2, "the handles follow");
        assert!(!write.tree_exists("a").unwrap());
        let error = write.rename_sequence("a", "a", false).err().unwrap();
        assert_eq!(not_found(error, folder), "a");
        let error = write.rename_sequence("b", "c", false).err().unwrap();
        assert_eq!(already_exists(error, folder), "c");
        assert_eq!(write.next_value(&c).unwrap(), 100, "no effect");
        write.rename_sequence("b", "c", true).unwrap();
        assert_eq!(not_found(write.next_value(&c).err().unwrap(), folder), "c");
        assert_eq!(write.next_value(&a).unwrap(), 3, "the renamed one");
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(read.list_trees().unwrap(), [("c".to_owned(), SEQ)]);
        drop(read);

        let mut write = store.write().unwrap();
        let c = write.open_sequence("c").unwrap();
        write.delete_sequence("c").unwrap();
        write.delete_sequence("c").unwrap();
        assert_eq!(not_found(write.next_value(&c).err().unwrap(), folder), "c");
        assert_eq!(
            not_found(write.open_sequence("c").err().unwrap(), folder),
            "c"
        );
        let new = create(&mut write, "c", None);
        assert_eq!(write.next_value(&new).unwrap(), 1, "a new sequence");
        assert_eq!(not_found(write.next_value(&c).err().unwrap(), folder), "c");
        write.commit().unwrap();
        assert_eq!(take(store, "c", 1, true), [2]);
    });
}

/// AC11, AC2 and AC-N5 (E14, N1, B3): sequence operations on a tree or a table, and tree or table operations on a
/// sequence, fail with a different type without effect, also renaming with overwrite onto another class; renaming
/// without overwrite onto another class fails with already exists; Exists says the three names exist; and List shows
/// tables, trees and sequences with their class in name order.
#[test]
fn ac11_sequences_tables_and_trees_never_touch_each_other() {
    stores(|store, folder| {
        commit_tree(store, "tree", &[(b"k", b"v")]);
        commit_sequence(store, "seq", Some(10));
        let mut write = store.write().unwrap();
        write
            .open_table_with("table", TableMode::CreateNew(&single()))
            .unwrap();
        write.commit().unwrap();
        let schema = single();
        let mut write = store.write().unwrap();

        // Sequence operations on a tree and on a table.
        for (name, found) in [("tree", NORMAL), ("table", TreeKind::Table)] {
            let expected = (name.to_owned(), found, SEQ);
            let error = write.open_sequence(name).err().unwrap();
            assert_eq!(different(error, folder), expected);
            for mode in [
                SequenceMode::CreateNew(None),
                SequenceMode::OpenOrCreate(Some(3)),
            ] {
                let error = write.open_sequence_with(name, mode).err().unwrap();
                assert_eq!(different(error, folder), expected, "{mode:?}");
            }
            let error = write.delete_sequence(name).err().unwrap();
            assert_eq!(different(error, folder), expected);
            let error = write.rename_sequence(name, "x", false).err().unwrap();
            assert_eq!(different(error, folder), expected);
            let error = write.rename_sequence(name, name, false).err().unwrap();
            assert_eq!(different(error, folder), expected);
            let error = write.rename_sequence("seq", name, true).err().unwrap();
            assert_eq!(different(error, folder), expected);
            let error = write.rename_sequence("seq", name, false).err().unwrap();
            assert_eq!(already_exists(error, folder), name);
        }

        // Tree operations on a sequence.
        let as_tree = |requested| ("seq".to_owned(), SEQ, requested);
        assert_eq!(
            different(write.open_tree("seq").err().unwrap(), folder),
            as_tree(NORMAL)
        );
        for mode in support::MODES {
            let error = write.open_tree_with("seq", mode).err().unwrap();
            assert_eq!(different(error, folder), as_tree(NORMAL), "{mode:?}");
            let error = write.open_multi_tree_with("seq", mode).err().unwrap();
            assert_eq!(different(error, folder), as_tree(MULTI));
            let error = write.open_fixed_tree_with("seq", 8, mode).err().unwrap();
            assert_eq!(different(error, folder), as_tree(TreeKind::FixedSize(8)));
        }
        assert_eq!(
            different(write.delete_tree("seq").err().unwrap(), folder),
            as_tree(NORMAL)
        );
        let error = write.rename_tree("seq", "x", false).err().unwrap();
        assert_eq!(different(error, folder), as_tree(NORMAL));
        let error = write.rename_tree("tree", "seq", true).err().unwrap();
        assert_eq!(different(error, folder), as_tree(NORMAL));
        let error = write.rename_tree("tree", "seq", false).err().unwrap();
        assert_eq!(already_exists(error, folder), "seq");

        // Table operations on a sequence.
        let as_table = ("seq".to_owned(), SEQ, TreeKind::Table);
        assert_eq!(
            different(write.open_table("seq").err().unwrap(), folder),
            as_table
        );
        for mode in [
            TableMode::CreateNew(&schema),
            TableMode::OpenOrCreate(&schema),
            TableMode::CreateOrEmpty(&schema),
            TableMode::Empty,
        ] {
            let error = write.open_table_with("seq", mode).err().unwrap();
            assert_eq!(different(error, folder), as_table, "{mode:?}");
        }
        assert_eq!(
            different(write.describe_table("seq").err().unwrap(), folder),
            as_table
        );
        assert_eq!(
            different(write.delete_table("seq").err().unwrap(), folder),
            as_table
        );
        let error = write.rename_table("seq", "x", false).err().unwrap();
        assert_eq!(different(error, folder), as_table);
        let error = write.rename_table("table", "seq", true).err().unwrap();
        assert_eq!(different(error, folder), as_table);
        let error = write.rename_table("table", "seq", false).err().unwrap();
        assert_eq!(already_exists(error, folder), "seq");

        // Nothing changed, and Exists sees the three classes.
        for name in ["seq", "table", "tree"] {
            assert!(write.tree_exists(name).unwrap(), "{name}");
        }
        assert!(!write.tree_exists("x").unwrap());
        assert_eq!(write.tree_kind("seq").unwrap(), Some(SEQ));
        let tree = write.open_tree("tree").unwrap();
        assert_eq!(write.get(&tree, b"k").unwrap(), Some(&b"v"[..]));
        assert_eq!(write.describe_table("table").unwrap().version(), 1);
        let seq = write.open_sequence("seq").unwrap();
        assert_eq!(write.next_value(&seq).unwrap(), 10, "no effect");
        write.commit().unwrap();

        let read = store.read().unwrap();
        let error = read.open_sequence("tree").err().unwrap();
        assert_eq!(different(error, folder), ("tree".to_owned(), NORMAL, SEQ));
        let error = read.open_sequence("table").err().unwrap();
        assert_eq!(
            different(error, folder),
            ("table".to_owned(), TreeKind::Table, SEQ)
        );
        assert_eq!(
            different(read.open_tree("seq").err().unwrap(), folder),
            as_tree(NORMAL)
        );
        assert_eq!(
            different(read.open_table("seq").err().unwrap(), folder),
            as_table
        );
        assert_eq!(
            different(read.describe_table("seq").err().unwrap(), folder),
            as_table
        );
        assert_eq!(
            read.list_trees().unwrap(),
            [
                ("seq".to_owned(), SEQ),
                ("table".to_owned(), TreeKind::Table),
                ("tree".to_owned(), NORMAL),
            ]
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// E16 and E17

/// E16 (AC13, T10): opening a sequence, Open or create on one that exists, Delete of a missing one, renaming to its
/// own name and every operation that fails public validation, an exhausted sequence included, write nothing; creating
/// a sequence, asking for a value, deleting one that exists and renaming to another name write.
#[test]
fn e16_what_changes_nothing_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_sequence(&store, "s", None);
    commit_sequence(&store, "u", None);
    commit_sequence(&store, "full", Some(i64::MAX));
    assert_eq!(take(&store, "full", 1, true), [i64::MAX]);
    commit_tree(&store, "tree", EMPTY);
    store.close().unwrap();
    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        write.open_sequence("s").unwrap();
        write
            .open_sequence_with("s", SequenceMode::OpenOrCreate(Some(9)))
            .unwrap();
        write.delete_sequence("missing").unwrap();
        write.rename_sequence("s", "s", false).unwrap();
        write.rename_sequence("s", "s", true).unwrap();
        let full = write.open_sequence("full").unwrap();
        let fails = [
            write.next_value(&full).err(),
            write
                .open_sequence_with("s", SequenceMode::CreateNew(None))
                .err(),
            write.open_sequence("missing").err(),
            write.rename_sequence("s", "u", false).err(),
            write.rename_sequence("missing", "v", false).err(),
            write.rename_sequence("s", "tree", true).err(),
            write.delete_sequence("tree").err(),
            write.delete_tree("s").err(),
            write.open_sequence("").err(),
        ];
        let kinds = fails.map(|e| e.unwrap().kind());
        use ErrorKind::*;
        assert_eq!(
            kinds,
            [
                InvalidValue,
                TreeAlreadyExists,
                TreeNotFound,
                TreeAlreadyExists,
                TreeNotFound,
                DifferentTreeType,
                DifferentTreeType,
                DifferentTreeType,
                InvalidSize,
            ]
        );
        write.commit().unwrap();
    });
    assert!(!changed, "a write that changes nothing writes nothing");

    let changes: [&dyn Fn(&mut WriteTransaction<'_>); 6] = [
        &|w| {
            create(w, "new", None);
        },
        &|w| {
            w.open_sequence_with("new", SequenceMode::OpenOrCreate(None))
                .unwrap();
        },
        &|w| {
            let s = w.open_sequence("s").unwrap();
            w.next_value(&s).unwrap();
        },
        &|w| w.delete_sequence("u").unwrap(),
        &|w| w.rename_sequence("u", "v", false).unwrap(),
        &|w| w.rename_sequence("s", "u", true).unwrap(),
    ];
    for (i, change) in changes.iter().enumerate() {
        let copy = copy_of(tmp.path());
        let changed = files_change(copy.path(), |store| {
            let mut write = store.write().unwrap();
            change(&mut write);
            write.commit().unwrap();
        });
        assert!(changed, "change {i}");
    }
}

/// E17 (AC14): the names' sizes come first, the source's before the destination's; then a handle of a deleted
/// sequence, which fails with not found without reading anything; then the catalog.
#[test]
fn e17_precedence_of_names_handles_and_catalog() {
    stores(|store, folder| {
        commit_tree(store, "tree", EMPTY);
        let long = "n".repeat(MAX_NAME + 1);
        let mut write = store.write().unwrap();
        let name_size = |error| invalid_size(error, folder);
        let too_long = (
            SizeSubject::TreeName,
            (MAX_NAME + 1) as u64,
            1,
            MAX_NAME as u64,
        );
        let empty = (SizeSubject::TreeName, 0, 1, MAX_NAME as u64);
        assert_eq!(
            name_size(write.open_sequence(&long).err().unwrap()),
            too_long
        );
        let error = write
            .open_sequence_with("", SequenceMode::CreateNew(None))
            .err()
            .unwrap();
        assert_eq!(name_size(error), empty);
        assert_eq!(name_size(write.delete_sequence("").err().unwrap()), empty);
        let error = write.rename_sequence("", &long, true).err().unwrap();
        assert_eq!(name_size(error), empty, "the source first");
        let error = write.rename_sequence("tree", &long, true).err().unwrap();
        assert_eq!(name_size(error), too_long, "before the catalog");
        let read = store.read().unwrap();
        assert_eq!(name_size(read.open_sequence("").err().unwrap()), empty);
        drop(read);

        let s = create(&mut write, "s", None);
        write.delete_sequence("s").unwrap();
        write.open_tree_with("s", OpenMode::CreateNew).unwrap();
        assert_eq!(
            not_found(write.next_value(&s).err().unwrap(), folder),
            "s",
            "not the tree that has its name now"
        );
        write.commit().unwrap();
    });
}

// ---------------------------------------------------------------------------------------------------------------
// E19 and G6: damage

/// E19 and AC19 (A9, D4): with the page of a sequence damaged, Open, Create new, Open or create, Exists, List and
/// Rename without overwrite or to its own name do not fail with damage; asking for a value fails with damage of that
/// page, and the write then cannot commit (T11).
#[test]
fn e19_only_asking_for_a_value_reads_the_sequences_page() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_sequence(&store, "s", Some(7));
    assert_eq!(take(&store, "s", 1, true), [7]);
    store.close().unwrap();
    let page = root_page(tmp.path(), "s");
    damage(tmp.path(), page);
    let store = open(tmp.path());
    let read = store.read().unwrap();
    read.open_sequence("s").unwrap();
    assert!(read.tree_exists("s").unwrap());
    assert_eq!(read.tree_kind("s").unwrap(), Some(SEQ));
    assert_eq!(read.list_trees().unwrap(), [("s".to_owned(), SEQ)]);
    drop(read);
    let mut write = store.write().unwrap();
    write.open_sequence("s").unwrap();
    let error = write
        .open_sequence_with("s", SequenceMode::CreateNew(None))
        .err()
        .unwrap();
    assert_eq!(error.kind(), ErrorKind::TreeAlreadyExists);
    write
        .open_sequence_with("s", SequenceMode::OpenOrCreate(Some(1)))
        .unwrap();
    write.rename_sequence("s", "s", false).unwrap();
    write.rename_sequence("s", "u", false).unwrap();
    assert!(write.tree_exists("u").unwrap());
    write.commit().unwrap();

    let mut write = store.write().unwrap();
    let u = write.open_sequence("u").unwrap();
    assert_eq!(damaged_page(write.next_value(&u).err().unwrap()), page);
    assert_eq!(write.commit().err().unwrap().kind(), ErrorKind::Damaged);
    store.close().unwrap();
}

/// G6 (D4): a sequence's page whose checksum is valid but whose entry is not a next value (a value of another length,
/// or no entry) is damage of that page when asking for a value; opening still works.
#[test]
fn g6_a_malformed_value_is_damage_of_the_sequences_page() {
    for (label, value) in [
        ("three bytes", Some(&[1u8, 2, 3][..])),
        ("nine bytes", Some(&[0u8; 9][..])),
        ("no entry", None),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let store = open(tmp.path());
        commit_sequence(&store, "s", None);
        store.close().unwrap();
        // Rewrite the sequence's tree through the engine, which seals the page with a valid checksum.
        {
            let engine = krummi_tx::Engine::open(tmp.path(), &Default::default()).unwrap();
            let mut write = engine.write(None).unwrap();
            let mut entry = catalog::lookup(&write, "s").unwrap().unwrap();
            let mut keys = Vec::new();
            krummi_tree::for_each(&write, entry.root, &mut |key, _| keys.push(key.to_vec()))
                .unwrap();
            assert_eq!(keys.len(), 1, "one entry");
            entry.root = match value {
                Some(value) => krummi_tree::set(&mut write, entry.root, &keys[0], value).unwrap(),
                None => krummi_tree::remove(&mut write, entry.root, &keys[0]).unwrap(),
            }
            .root;
            catalog::put(&mut write, "s", &entry).unwrap();
            write.commit().unwrap();
            engine.close().unwrap();
        }
        let page = root_page(tmp.path(), "s");
        let store = open(tmp.path());
        let mut write = store.write().unwrap();
        let s = write.open_sequence("s").unwrap();
        assert_eq!(
            damaged_page(write.next_value(&s).err().unwrap()),
            page,
            "{label}"
        );
        write.abort();
        store.close().unwrap();
    }
}
