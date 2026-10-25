//! Trees and key operations through the public API (Trees and key operations PRD: A1–A12, the 2026-10-25
//! annotation; AC-A1–AC-A15, AC-A17; Trees PRD: AC-B1–AC-B6, AC-B9, AC-B10, AC-B13, AC-B14, AC-B17; root PRD:
//! AC20). Crashes, power cuts and altered bytes are in `tree_integrity.rs`.
//!
//! Each criterion runs on a store on disk and on a temporary one, except the parts that reopen or damage the
//! store's files, which run on disk only. What must not compile is in `trees_compile.rs`.

use std::fs;
use std::path::{Path, PathBuf};

use krummi_pager::DATA_FILE;
use krummi_pager::format::PAGE_SIZE;
use krummi_tree::catalog;
use krummi_tx::{Engine, OpenOptions as EngineOptions};

use krummi::{
    Error, ErrorKind, OpenMode, ReadTransaction, ReadTree, SizeSubject, Store, StoreOptions,
    TreeKind, WriteTransaction, WriteTree,
};

const NORMAL: TreeKind = TreeKind::Normal;
// The size limits are written out on purpose instead of taken from the engine: they pin the PRDs' limits (B4, B5,
// A11), so a change in the engine's constants fails these tests.
const MAX_NAME: usize = 1024;
const MAX_KEY: usize = 2048;
const MAX_VALUE: usize = 1024;

/// Runs `test` on a store on disk and on a temporary one; `folder` is the store's folder, `None` when temporary.
fn stores(test: impl Fn(&Store, Option<&Path>)) {
    let tmp = tempfile::tempdir().unwrap();
    let on_disk = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    test(&on_disk, Some(tmp.path()));
    on_disk.close().unwrap();
    let in_memory = Store::open_temporary(&StoreOptions::default()).unwrap();
    test(&in_memory, None);
    in_memory.close().unwrap();
}

fn open(folder: &Path) -> Store {
    Store::open(folder, &StoreOptions::default()).unwrap()
}

fn create<'s>(write: &mut WriteTransaction<'s>, name: &str) -> WriteTree<'s> {
    write
        .open_tree_with(name, NORMAL, OpenMode::CreateNew)
        .unwrap()
}

/// No entries, for [`commit_tree`].
const EMPTY: &[(&[u8], &[u8])] = &[];

/// Commits a tree `name` with `entries`.
fn commit_tree(store: &Store, name: &str, entries: &[(impl AsRef<[u8]>, impl AsRef<[u8]>)]) {
    let mut write = store.write().unwrap();
    let tree = write
        .open_tree_with(name, NORMAL, OpenMode::OpenOrCreate)
        .unwrap();
    for (key, value) in entries {
        write.set(&tree, key.as_ref(), value.as_ref()).unwrap();
    }
    write.commit().unwrap();
}

fn names(store: &Store) -> Vec<String> {
    let read = store.read().unwrap();
    read.list_trees()
        .unwrap()
        .into_iter()
        .map(|(name, kind)| {
            assert_eq!(kind, NORMAL);
            name
        })
        .collect()
}

fn folder_of(folder: Option<&Path>) -> Option<PathBuf> {
    folder.map(Path::to_path_buf)
}

/// The tree name of a tree-not-found error, after checking its folder.
#[track_caller]
fn not_found(error: Error, folder: Option<&Path>) -> String {
    match error {
        Error::TreeNotFound { folder: f, name } => {
            assert_eq!(f, folder_of(folder));
            name
        }
        other => panic!("expected tree not found, got {other}"),
    }
}

/// The tree name of a tree-already-exists error, after checking its folder.
#[track_caller]
fn already_exists(error: Error, folder: Option<&Path>) -> String {
    match error {
        Error::TreeAlreadyExists { folder: f, name } => {
            assert_eq!(f, folder_of(folder));
            name
        }
        other => panic!("expected tree already exists, got {other}"),
    }
}

/// The subject, received size, minimum and maximum of an invalid-size error, after checking its folder.
#[track_caller]
fn invalid_size(error: Error, folder: Option<&Path>) -> (SizeSubject, u64, u64, u64) {
    match error {
        Error::InvalidSize {
            folder: f,
            what,
            received,
            min,
            max,
        } => {
            assert_eq!(f, folder_of(folder));
            (what, received, min, max)
        }
        other => panic!("expected invalid size, got {other}"),
    }
}

#[track_caller]
fn misuse(error: Error, folder: Option<&Path>) {
    match error {
        Error::Misuse { folder: f, .. } => assert_eq!(f, folder_of(folder)),
        other => panic!("expected misuse, got {other}"),
    }
}

/// Every file of the store's folder with its bytes, in name order, subfolders included.
fn files(folder: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut pending = vec![folder.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = fs::read(&path).unwrap();
                out.push((path, bytes));
            }
        }
    }
    out.sort();
    out
}

/// Runs `change` on a closed store on disk and returns whether its files changed: they are taken after a clean
/// close before and after, and a store reopened and closed without changes keeps them (S1 of the journal).
fn files_change(folder: &Path, change: impl FnOnce(&Store)) -> bool {
    let before = files(folder);
    let store = open(folder);
    change(&store);
    store.close().unwrap();
    files(folder) != before
}

/// Runs `look` on a read of the closed store through the engine's internals, which opening and closing without
/// changes leave as they were.
fn inspect<T>(folder: &Path, look: impl FnOnce(&krummi_tx::ReadTransaction<'_>) -> T) -> T {
    let engine = Engine::open(folder, &EngineOptions::default()).unwrap();
    let read = engine.read().unwrap();
    let seen = look(&read);
    drop(read);
    engine.close().unwrap();
    seen
}

/// The page of the leaf where `key` is or would be in the closed store: in the tree `tree`, or in the catalog when
/// `tree` is `None` (then `key` is a tree name).
fn leaf_page(folder: &Path, tree: Option<&str>, key: &[u8]) -> u64 {
    inspect(folder, |read| {
        let root = match tree {
            Some(name) => catalog::lookup(read, name).unwrap().unwrap().root,
            None => read.slot(catalog::ROOT_SLOT).unwrap(),
        };
        krummi_tree::leaf_of(read, root, key).unwrap()
    })
}

/// The root page of the tree `tree` in the closed store.
fn root_page(folder: &Path, tree: &str) -> u64 {
    inspect(folder, |read| {
        catalog::lookup(read, tree).unwrap().unwrap().root
    })
}

/// Flips a byte of `page` in the closed store's data file, so that its checksum fails.
fn damage(folder: &Path, page: u64) {
    let path = folder.join(DATA_FILE);
    let mut bytes = fs::read(&path).unwrap();
    bytes[page as usize * PAGE_SIZE + 100] ^= 0xFF;
    fs::write(&path, bytes).unwrap();
}

#[track_caller]
fn damaged_page(error: Error) -> u64 {
    match error {
        Error::Damaged {
            page: Some(page), ..
        } => page,
        other => panic!("expected damage with a page, got {other}"),
    }
}

/// A key of `len` bytes that starts with `i`, so that keys sort by `i`.
fn long_key(i: usize, len: usize) -> Vec<u8> {
    let mut key = format!("{i:05}").into_bytes();
    key.resize(len, b'.');
    key
}

/// The 2026-10-25 annotation (B1 exception): a handle of another transaction of the same class, also of one that
/// ended, fails with misuse without effect, and the transaction stays usable; the invalid size wins over it (A7).
#[test]
fn s0_a_handle_of_another_transaction_fails_with_misuse_without_effect() {
    stores(|store, folder| {
        commit_tree(store, "t", &[(b"k", b"v")]);
        let old = store.write().unwrap();
        let stale = old.open_tree("t", NORMAL).unwrap();
        old.commit().unwrap();

        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        misuse(write.get(&stale, b"k").unwrap_err(), folder);
        misuse(write.set(&stale, b"k", b"x").unwrap_err(), folder);
        misuse(write.try_add(&stale, b"n", b"x").unwrap_err(), folder);
        misuse(write.remove(&stale, b"k").unwrap_err(), folder);
        misuse(write.len(&stale).unwrap_err(), folder);
        let too_long = vec![0; MAX_KEY + 1];
        let size = invalid_size(write.set(&stale, &too_long, b"").unwrap_err(), folder);
        assert_eq!(size.0, SizeSubject::Key, "the size wins over the misuse");
        assert_eq!(
            write.get(&tree, b"k").unwrap(),
            Some(&b"v"[..]),
            "no effect"
        );
        assert_eq!(write.len(&tree).unwrap(), 1);
        write.set(&tree, b"other", b"1").unwrap();
        write.commit().unwrap();

        let first = store.read().unwrap();
        let read_handle = first.open_tree("t", NORMAL).unwrap();
        drop(first);
        let read = store.read().unwrap();
        misuse(read.get(&read_handle, b"k").unwrap_err(), folder);
        misuse(read.len(&read_handle).unwrap_err(), folder);
        let tree = read.open_tree("t", NORMAL).unwrap();
        assert_eq!(read.get(&tree, b"k").unwrap(), Some(&b"v"[..]));
        assert_eq!(read.get(&tree, b"other").unwrap(), Some(&b"1"[..]));
        assert_eq!(read.len(&tree).unwrap(), 2);
    });
}

#[test]
fn ac_a1_open_modes_open_create_and_fail_by_existence() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let error = write.open_tree("t", NORMAL).err().unwrap();
        assert_eq!(not_found(error, folder), "t");
        let tree = create(&mut write, "t");
        assert_eq!(write.len(&tree).unwrap(), 0);
        assert!(write.tree_exists("t").unwrap());
        let error = write
            .open_tree_with("t", NORMAL, OpenMode::CreateNew)
            .err()
            .unwrap();
        assert_eq!(already_exists(error, folder), "t");
        write.set(&tree, b"k", b"v").unwrap();
        let created = write
            .open_tree_with("u", NORMAL, OpenMode::OpenOrCreate)
            .unwrap();
        assert!(write.tree_exists("u").unwrap());
        assert_eq!(write.len(&created).unwrap(), 0);
        let opened = write
            .open_tree_with("t", NORMAL, OpenMode::OpenOrCreate)
            .unwrap();
        assert_eq!(write.get(&opened, b"k").unwrap(), Some(&b"v"[..]));
        assert_eq!(write.len(&opened).unwrap(), 1);
        write.commit().unwrap();

        let read = store.read().unwrap();
        let error = read.open_tree("missing", NORMAL).err().unwrap();
        assert_eq!(not_found(error, folder), "missing");
        let tree = read.open_tree("t", NORMAL).unwrap();
        assert_eq!(read.get(&tree, b"k").unwrap(), Some(&b"v"[..]));
        assert_eq!(read.len(&tree).unwrap(), 1);
        assert_eq!(names(store), ["t", "u"]);
    });
}

#[test]
fn ac_a2_emptying_keeps_the_tree_and_its_handles() {
    stores(|store, folder| {
        commit_tree(store, "t", &[(b"a", b"1"), (b"b", b"2"), (b"c", b"3")]);
        commit_tree(store, "e", &[(b"a", b"1")]);
        let mut write = store.write().unwrap();
        let before = write.open_tree("t", NORMAL).unwrap();
        let emptied = write
            .open_tree_with("t", NORMAL, OpenMode::CreateOrEmpty)
            .unwrap();
        assert_eq!(write.len(&emptied).unwrap(), 0);
        assert_eq!(
            write.len(&before).unwrap(),
            0,
            "the earlier handle sees it empty"
        );
        assert_eq!(write.get(&before, b"a").unwrap(), None);
        write.set(&before, b"x", b"9").unwrap();
        assert_eq!(write.len(&emptied).unwrap(), 1, "and keeps working");

        let e = write.open_tree_with("e", NORMAL, OpenMode::Empty).unwrap();
        assert_eq!(write.len(&e).unwrap(), 0);
        let new = write
            .open_tree_with("new", NORMAL, OpenMode::CreateOrEmpty)
            .unwrap();
        assert_eq!(write.len(&new).unwrap(), 0);
        let error = write
            .open_tree_with("missing", NORMAL, OpenMode::Empty)
            .err()
            .unwrap();
        assert_eq!(not_found(error, folder), "missing");

        let filled = create(&mut write, "f");
        for key in [b"a", b"b", b"c"] {
            write.set(&filled, key, b"v").unwrap();
        }
        let f = write.open_tree_with("f", NORMAL, OpenMode::Empty).unwrap();
        assert_eq!(write.len(&f).unwrap(), 0);
        assert_eq!(write.len(&filled).unwrap(), 0);
        write.commit().unwrap();

        let read = store.read().unwrap();
        for (name, count) in [("e", 0), ("f", 0), ("new", 0), ("t", 1)] {
            let tree = read.open_tree(name, NORMAL).unwrap();
            assert_eq!(read.len(&tree).unwrap(), count, "{name}");
        }
        let t = read.open_tree("t", NORMAL).unwrap();
        assert_eq!(read.get(&t, b"x").unwrap(), Some(&b"9"[..]));
        assert_eq!(read.get(&t, b"a").unwrap(), None);
        let f = read.open_tree("f", NORMAL).unwrap();
        assert_eq!(read.get(&f, b"a").unwrap(), None);
        assert_eq!(names(store), ["e", "f", "new", "t"]);
    });
}

/// AC-A3: a read transaction opens, checks, lists, reads and counts; the rest does not compile in it
/// (`trees_compile.rs`, case `read_transaction_cannot_modify_trees`).
#[test]
fn ac_a3_a_read_transaction_opens_checks_lists_reads_and_counts() {
    stores(|store, _| {
        commit_tree(store, "t", &[(b"k", b"v")]);
        let read = store.read().unwrap();
        let tree = read.open_tree("t", NORMAL).unwrap();
        assert!(read.tree_exists("t").unwrap());
        assert!(!read.tree_exists("u").unwrap());
        assert_eq!(read.list_trees().unwrap(), [("t".to_owned(), NORMAL)]);
        assert_eq!(read.get(&tree, b"k").unwrap(), Some(&b"v"[..]));
        assert_eq!(read.get(&tree, b"x").unwrap(), None);
        assert_eq!(read.len(&tree).unwrap(), 1);
    });
}

#[test]
fn ac_a4_exists_and_delete() {
    stores(|store, folder| {
        commit_tree(store, "t", &[(b"k", b"v")]);
        let mut write = store.write().unwrap();
        assert!(write.tree_exists("t").unwrap());
        write.delete_tree("t").unwrap();
        assert!(!write.tree_exists("t").unwrap());
        write.delete_tree("missing").unwrap();
        write.commit().unwrap();
        assert!(!store.read().unwrap().tree_exists("t").unwrap());

        let long = "n".repeat(MAX_NAME + 1);
        let mut write = store.write().unwrap();
        for (name, len) in [("", 0), (&long[..], 1025)] {
            let expected = (SizeSubject::TreeName, len, 1, 1024);
            assert_eq!(
                invalid_size(write.tree_exists(name).unwrap_err(), folder),
                expected
            );
            assert_eq!(
                invalid_size(write.delete_tree(name).unwrap_err(), folder),
                expected
            );
            let read = store.read().unwrap();
            assert_eq!(
                invalid_size(read.tree_exists(name).unwrap_err(), folder),
                expected
            );
        }
        write.commit().unwrap();
    });

    let tmp = tempfile::tempdir().unwrap();
    commit_tree(&open(tmp.path()), "t", &[(b"k", b"v")]);
    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        write.delete_tree("missing").unwrap();
        write.commit().unwrap();
    });
    assert!(!changed, "deleting a missing tree writes nothing");
}

#[test]
fn ac_a5_rename() {
    stores(|store, folder| {
        commit_tree(store, "a", &[(b"k", b"a")]);
        commit_tree(store, "b", &[(b"k", b"b")]);
        let mut write = store.write().unwrap();
        let a = write.open_tree("a", NORMAL).unwrap();
        let b = write.open_tree("b", NORMAL).unwrap();
        write.rename_tree("a", "c", false).unwrap();
        let c = write.open_tree("c", NORMAL).unwrap();
        assert_eq!(write.get(&c, b"k").unwrap(), Some(&b"a"[..]));
        assert!(!write.tree_exists("a").unwrap());
        assert_eq!(
            write.get(&a, b"k").unwrap(),
            Some(&b"a"[..]),
            "the handle of a still works"
        );
        write.set(&a, b"k2", b"a2").unwrap();
        assert_eq!(write.len(&c).unwrap(), 2);

        let error = write.rename_tree("c", "b", false).unwrap_err();
        assert_eq!(already_exists(error, folder), "b");
        write.rename_tree("c", "b", true).unwrap();
        let new_b = write.open_tree("b", NORMAL).unwrap();
        assert_eq!(write.get(&new_b, b"k").unwrap(), Some(&b"a"[..]));
        assert_eq!(
            write.get(&c, b"k2").unwrap(),
            Some(&b"a2"[..]),
            "the handle of c still works"
        );
        assert_eq!(not_found(write.get(&b, b"k").unwrap_err(), folder), "b");
        assert_eq!(not_found(write.len(&b).unwrap_err(), folder), "b");
        assert_eq!(
            not_found(write.set(&b, b"k", b"x").unwrap_err(), folder),
            "b"
        );
        assert!(!write.tree_exists("c").unwrap());

        for overwrite in [false, true] {
            let error = write.rename_tree("missing", "x", overwrite).unwrap_err();
            assert_eq!(not_found(error, folder), "missing");
            let error = write
                .rename_tree("missing", "missing", overwrite)
                .unwrap_err();
            assert_eq!(not_found(error, folder), "missing");
            write.rename_tree("b", "b", overwrite).unwrap();
            assert_eq!(
                write.len(&new_b).unwrap(),
                2,
                "renaming b to b changes nothing"
            );
        }
        assert_eq!(write.list_trees().unwrap(), [("b".to_owned(), NORMAL)]);

        let t = create(&mut write, "t");
        write.rename_tree("t", "d", false).unwrap();
        write.delete_tree("d").unwrap();
        assert_eq!(not_found(write.get(&t, b"k").unwrap_err(), folder), "d");
        write.commit().unwrap();

        assert_eq!(names(store), ["b"]);
        let read = store.read().unwrap();
        let b = read.open_tree("b", NORMAL).unwrap();
        assert_eq!(read.get(&b, b"k").unwrap(), Some(&b"a"[..]));
        assert_eq!(read.get(&b, b"k2").unwrap(), Some(&b"a2"[..]));
        assert_eq!(read.len(&b).unwrap(), 2);
    });
}

#[test]
fn ac_a6_identity_survives_renames_and_ends_when_overwritten() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let a = create(&mut write, "a");
        write.set(&a, b"k", b"1").unwrap();
        write.rename_tree("a", "c", false).unwrap();
        write.rename_tree("c", "a", false).unwrap();
        assert_eq!(write.get(&a, b"k").unwrap(), Some(&b"1"[..]));

        write.rename_tree("a", "c", false).unwrap();
        let new_a = create(&mut write, "a");
        write.rename_tree("c", "a", true).unwrap();
        assert_eq!(not_found(write.get(&new_a, b"k").unwrap_err(), folder), "a");
        assert_eq!(write.get(&a, b"k").unwrap(), Some(&b"1"[..]));
        write.commit().unwrap();
        let read = store.read().unwrap();
        let a = read.open_tree("a", NORMAL).unwrap();
        assert_eq!(read.get(&a, b"k").unwrap(), Some(&b"1"[..]));
        assert_eq!(names(store), ["a"]);
    });
}

#[test]
fn ac_a7_list() {
    stores(|store, _| {
        assert!(store.read().unwrap().list_trees().unwrap().is_empty());
        assert!(store.write().unwrap().list_trees().unwrap().is_empty());
        let mut write = store.write().unwrap();
        for name in ["pedidos", "clientes", "b", "Clientes", "a", "é", "B"] {
            create(&mut write, name);
        }
        write.commit().unwrap();
        let expected = ["B", "Clientes", "a", "b", "clientes", "pedidos", "é"];
        let read = store.read().unwrap();
        let listed = read.list_trees().unwrap();
        assert_eq!(listed, expected.map(|n| (n.to_owned(), NORMAL)));

        let mut write = store.write().unwrap();
        create(&mut write, "z");
        write.rename_tree("a", "y", false).unwrap();
        write.delete_tree("b").unwrap();
        let in_write = write.list_trees().unwrap();
        let expected = ["B", "Clientes", "clientes", "pedidos", "y", "z", "é"];
        assert_eq!(in_write, expected.map(|n| (n.to_owned(), NORMAL)));
        write.delete_tree("pedidos").unwrap();
        write.commit().unwrap();
        drop(read);
        assert_eq!(
            in_write,
            expected.map(|n| (n.to_owned(), NORMAL)),
            "the list is a copy"
        );
        assert_eq!(listed.len(), 7, "also after its transaction ended");
        assert_eq!(names(store), ["B", "Clientes", "clientes", "y", "z", "é"]);
    });
}

#[test]
fn ac_a8_key_operations() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let tree = create(&mut write, "t");
        write.set(&tree, b"k", b"first").unwrap();
        write.set(&tree, b"k", b"second").unwrap();
        assert_eq!(write.get(&tree, b"k").unwrap(), Some(&b"second"[..]));
        assert!(!write.try_add(&tree, b"k", b"third").unwrap());
        assert_eq!(write.get(&tree, b"k").unwrap(), Some(&b"second"[..]));
        assert!(write.try_add(&tree, b"n", b"new").unwrap());
        assert_eq!(write.get(&tree, b"n").unwrap(), Some(&b"new"[..]));
        assert_eq!(write.get(&tree, b"missing").unwrap(), None);
        write.set(&tree, b"e", b"").unwrap();
        assert_eq!(write.get(&tree, b"e").unwrap(), Some(&b""[..]));
        assert!(write.remove(&tree, b"k").unwrap());
        assert!(!write.remove(&tree, b"k").unwrap());
        let error = write.try_add(&tree, b"n", &[0; MAX_VALUE + 1]).unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::Value, 1025, 0, 1024)
        );
        assert_eq!(write.len(&tree).unwrap(), 2);
        write.commit().unwrap();

        let read = store.read().unwrap();
        let tree = read.open_tree("t", NORMAL).unwrap();
        assert_eq!(read.get(&tree, b"k").unwrap(), None);
        assert_eq!(read.get(&tree, b"n").unwrap(), Some(&b"new"[..]));
        assert_eq!(read.get(&tree, b"e").unwrap(), Some(&b""[..]));
        assert_eq!(read.len(&tree).unwrap(), 2);
    });
}

#[test]
fn ac_a9_writes_that_change_nothing_leave_the_files_identical() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_tree(&store, "t", &[(b"k", b"v")]);
    commit_tree(&store, "u", EMPTY);
    store.close().unwrap();
    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        let t = write.open_tree("t", NORMAL).unwrap();
        assert!(!write.try_add(&t, b"k", b"other").unwrap());
        assert!(!write.remove(&t, b"missing").unwrap());
        write
            .open_tree_with("t", NORMAL, OpenMode::OpenOrCreate)
            .unwrap();
        let fails = [
            write.open_tree("missing", NORMAL).err(),
            write
                .open_tree_with("missing", NORMAL, OpenMode::Empty)
                .err(),
            write.open_tree_with("t", NORMAL, OpenMode::CreateNew).err(),
            write.rename_tree("missing", "x", false).err(),
            write.rename_tree("t", "u", false).err(),
        ];
        let kinds = fails.map(|e| e.unwrap().kind());
        use ErrorKind::{TreeAlreadyExists as Exists, TreeNotFound as Missing};
        assert_eq!(kinds, [Missing, Missing, Exists, Missing, Exists]);
        write.commit().unwrap();
    });
    assert!(!changed, "a write that changes nothing writes nothing");

    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        write.open_tree_with("u", NORMAL, OpenMode::Empty).unwrap();
        write.commit().unwrap();
    });
    assert!(changed, "emptying an empty tree is a change");
}

/// AC-A10 (A5, A9): keys of 2,048 bytes put two or three entries in each leaf and two to four children in each
/// branch, so 60 keys build a tree of four levels or more, which removing them all brings back to one leaf.
#[test]
fn ac_a10_the_count_follows_every_operation() {
    const KEYS: usize = 60;
    let key = |i: usize| long_key(i, MAX_KEY);
    stores(|store, _| {
        let universe: Vec<Vec<u8>> = (0..KEYS + 20).map(key).collect();
        let check = |write: &WriteTransaction<'_>, tree: &WriteTree<'_>, label: &str| {
            let found = universe
                .iter()
                .filter(|k| write.get(tree, k).unwrap().is_some())
                .count();
            assert_eq!(write.len(tree).unwrap(), found as u64, "{label}");
        };
        let mut write = store.write().unwrap();
        let tree = create(&mut write, "t");
        let other = write.open_tree("t", NORMAL).unwrap();
        for i in 0..KEYS {
            write.set(&tree, &key(i), &[1; 100]).unwrap();
            check(&write, &other, "set a new key");
        }
        for i in (0..KEYS).step_by(3) {
            write.set(&tree, &key(i), &[2; 10]).unwrap();
            check(&write, &tree, "overwrite with a shorter value");
        }
        for i in KEYS - 10..KEYS + 10 {
            write.try_add(&tree, &key(i), &[3; 50]).unwrap();
            check(&write, &tree, "add new and existing keys");
        }
        write.commit().unwrap();
        let count = KEYS as u64 + 10;
        let snapshot = store.read().unwrap();
        let before = snapshot.open_tree("t", NORMAL).unwrap();

        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        for i in (0..KEYS + 20).rev().step_by(2) {
            write.remove(&tree, &key(i)).unwrap();
            check(&write, &tree, "remove present and absent keys");
        }
        write.abort();
        let read = store.read().unwrap();
        let tree = read.open_tree("t", NORMAL).unwrap();
        assert_eq!(
            read.len(&tree).unwrap(),
            count,
            "an aborted write keeps the count"
        );

        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        for i in 0..KEYS + 20 {
            write.remove(&tree, &key(i)).unwrap();
            check(&write, &tree, "remove every key");
        }
        assert_eq!(write.len(&tree).unwrap(), 0);
        for i in 0..20 {
            write.set(&tree, &key(i), b"again").unwrap();
        }
        check(&write, &tree, "fill again");
        let emptied = write.open_tree_with("t", NORMAL, OpenMode::Empty).unwrap();
        check(&write, &emptied, "empty");
        write.set(&tree, &key(7), b"last").unwrap();
        check(&write, &tree, "after emptying");
        write.commit().unwrap();
        assert_eq!(
            snapshot.len(&before).unwrap(),
            count,
            "an older read keeps its snapshot"
        );
        let read = store.read().unwrap();
        let tree = read.open_tree("t", NORMAL).unwrap();
        assert_eq!(read.len(&tree).unwrap(), 1);
    });

    // Committed and reopened, the count is kept; with one of the tree's pages damaged, it still does not fail.
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let tree = create(&mut write, "t");
    for i in 0..KEYS {
        write.set(&tree, &key(i), b"value").unwrap();
    }
    write.commit().unwrap();
    store.close().unwrap();
    let page = leaf_page(tmp.path(), Some("t"), &key(30));
    damage(tmp.path(), page);
    let store = open(tmp.path());
    let read = store.read().unwrap();
    let tree = read.open_tree("t", NORMAL).unwrap();
    assert_eq!(read.len(&tree).unwrap(), KEYS as u64);
    assert_eq!(damaged_page(read.get(&tree, &key(30)).unwrap_err()), page);
    let write = store.write().unwrap();
    let tree = write.open_tree("t", NORMAL).unwrap();
    assert_eq!(write.len(&tree).unwrap(), KEYS as u64);
}

#[test]
fn ac_a11_ten_alternating_replacements() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        create(&mut write, "t");
        let mut stale = Vec::new();
        for step in 0..10 {
            stale.push(write.open_tree("t", NORMAL).unwrap());
            if step % 2 == 0 {
                write.delete_tree("t").unwrap();
                stale.push(create(&mut write, "t"));
            } else {
                create(&mut write, "u");
                write.rename_tree("u", "t", true).unwrap();
            }
        }
        // The tree created by the last delete-and-create was replaced by the last rename.
        for (i, handle) in stale.iter().enumerate() {
            let error = write.set(handle, b"k", b"v").unwrap_err();
            assert_eq!(not_found(error, folder), "t", "handle {i}");
        }
        let current = write.open_tree("t", NORMAL).unwrap();
        write.set(&current, b"k", b"v").unwrap();
        assert_eq!(write.len(&current).unwrap(), 1);
        write.commit().unwrap();
        assert_eq!(names(store), ["t"]);
    });
}

/// AC-A12 (A9, T11): with a damaged page of a tree, opening it, asking whether it exists, listing and renaming it
/// without overwriting do not fail; reading an entry of that page fails with the damage class and the page number,
/// again when repeated; and a write that deletes it, empties it, overwrites it with Renombrar or stores in that page
/// and fails with damage cannot be committed: its commit fails with damage and applies nothing. A damaged leaf
/// shows reading and storing; a damaged root, which deleting, emptying and overwriting read (D4), the rest.
#[test]
fn ac_a12_a_damaged_page_of_a_tree() {
    let key = |i: usize| long_key(i, 1000);
    // A closed store with a tree of several levels whose page `page_of` gives is damaged, and that page.
    let damaged_store = |page_of: &dyn Fn(&Path) -> u64| {
        let tmp = tempfile::tempdir().unwrap();
        let store = open(tmp.path());
        let mut write = store.write().unwrap();
        let tree = create(&mut write, "t");
        for i in 0..60 {
            write.set(&tree, &key(i), b"value").unwrap();
        }
        let other = create(&mut write, "other");
        write.set(&other, b"k", b"v").unwrap();
        write.commit().unwrap();
        store.close().unwrap();
        let page = page_of(tmp.path());
        damage(tmp.path(), page);
        (tmp, page)
    };
    // Opening, Existe, Listar and the count do not fail, and nothing changed.
    let unchanged = |store: &Store, name: &str| {
        let read = store.read().unwrap();
        let mut list: Vec<String> = read
            .list_trees()
            .unwrap()
            .into_iter()
            .map(|t| t.0)
            .collect();
        list.sort();
        assert_eq!(list, ["other", name]);
        assert!(read.tree_exists(name).unwrap());
        let tree = read.open_tree(name, NORMAL).unwrap();
        assert_eq!(read.len(&tree).unwrap(), 60);
        let other = read.open_tree("other", NORMAL).unwrap();
        assert_eq!(read.len(&other).unwrap(), 1);
        assert_eq!(read.get(&other, b"changed").unwrap(), None);
    };

    // A damaged leaf: reading or storing in it fails, also in a write, which then cannot be committed.
    let (tmp, page) = damaged_store(&|folder| leaf_page(folder, Some("t"), &key(30)));
    let store = open(tmp.path());
    unchanged(&store, "t");
    let read = store.read().unwrap();
    let tree = read.open_tree("t", NORMAL).unwrap();
    for _ in 0..2 {
        assert_eq!(damaged_page(read.get(&tree, &key(30)).unwrap_err()), page);
    }
    assert_eq!(
        read.get(&tree, &key(0)).unwrap(),
        Some(&b"value"[..]),
        "another leaf"
    );
    drop(read);
    let mut write = store.write().unwrap();
    let other = write.open_tree("other", NORMAL).unwrap();
    write.set(&other, b"changed", b"x").unwrap();
    let tree = write.open_tree("t", NORMAL).unwrap();
    assert!(write.tree_exists("t").unwrap());
    assert_eq!(write.list_trees().unwrap().len(), 2);
    assert_eq!(write.len(&tree).unwrap(), 60);
    for _ in 0..2 {
        assert_eq!(damaged_page(write.get(&tree, &key(30)).unwrap_err()), page);
    }
    let error = write.set(&tree, &key(30), b"new").unwrap_err();
    assert_eq!(damaged_page(error), page);
    assert_eq!(damaged_page(write.commit().unwrap_err()), page, "T11");
    unchanged(&store, "t");
    let mut write = store.write().unwrap();
    write.rename_tree("t", "t", false).unwrap();
    write.rename_tree("t", "renamed", false).unwrap();
    write.commit().unwrap();
    unchanged(&store, "renamed");
    let read = store.read().unwrap();
    let tree = read.open_tree("renamed", NORMAL).unwrap();
    assert_eq!(damaged_page(read.get(&tree, &key(30)).unwrap_err()), page);
    drop(read);
    store.close().unwrap();

    // A damaged root: deleting, emptying and overwriting the tree fail, and the write cannot be committed.
    let (tmp, page) = damaged_store(&|folder| root_page(folder, "t"));
    let store = open(tmp.path());
    unchanged(&store, "t");
    let read = store.read().unwrap();
    let tree = read.open_tree("t", NORMAL).unwrap();
    for _ in 0..2 {
        assert_eq!(damaged_page(read.get(&tree, &key(0)).unwrap_err()), page);
    }
    drop(read);
    for operation in [
        "delete",
        "create or empty",
        "empty",
        "overwrite with rename",
    ] {
        let mut write = store.write().unwrap();
        let other = write.open_tree("other", NORMAL).unwrap();
        write.set(&other, b"changed", b"x").unwrap();
        let error = match operation {
            "delete" => write.delete_tree("t").unwrap_err(),
            "create or empty" => write
                .open_tree_with("t", NORMAL, OpenMode::CreateOrEmpty)
                .err()
                .unwrap(),
            "empty" => write
                .open_tree_with("t", NORMAL, OpenMode::Empty)
                .err()
                .unwrap(),
            _ => write.rename_tree("other", "t", true).unwrap_err(),
        };
        assert_eq!(damaged_page(error), page, "{operation}");
        let error = write.commit().unwrap_err();
        assert_eq!(damaged_page(error), page, "{operation}: T11");
        unchanged(&store, "t");
    }
    let mut write = store.write().unwrap();
    write.rename_tree("t", "renamed", false).unwrap();
    write.commit().unwrap();
    unchanged(&store, "renamed");
}

#[test]
fn ac_a13_precedence() {
    stores(|store, folder| {
        commit_tree(store, "dest", EMPTY);
        let long = "n".repeat(MAX_NAME + 1);
        let mut write = store.write().unwrap();
        let error = write.rename_tree(&long, "dest", true).unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::TreeName, 1025, 1, 1024)
        );
        let error = write.rename_tree("missing", &long, true).unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::TreeName, 1025, 1, 1024)
        );
        let error = write.rename_tree("missing", "dest", false).unwrap_err();
        assert_eq!(not_found(error, folder), "missing");
        let deleted = write.open_tree("dest", NORMAL).unwrap();
        write.delete_tree("dest").unwrap();
        let error = write.set(&deleted, &[0; MAX_KEY + 1], b"").unwrap_err();
        assert_eq!(invalid_size(error, folder).0, SizeSubject::Key);
        write.commit().unwrap();
    });

    // A catalog of several pages, with the destination's page damaged and the source's intact.
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let name = |i: usize| format!("tree-{i:03}-{}", "x".repeat(990));
    let mut write = store.write().unwrap();
    for i in 0..40 {
        create(&mut write, &name(i));
    }
    write.commit().unwrap();
    store.close().unwrap();
    let dest = name(39);
    let page = leaf_page(tmp.path(), None, dest.as_bytes());
    assert_ne!(
        page,
        leaf_page(tmp.path(), None, b"a-missing"),
        "the source's leaf is another"
    );
    damage(tmp.path(), page);
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    for overwrite in [false, true] {
        let error = write
            .rename_tree("a-missing", &dest, overwrite)
            .unwrap_err();
        assert_eq!(not_found(error, Some(tmp.path())), "a-missing");
    }
    write.commit().unwrap();
    let read = store.read().unwrap();
    assert_eq!(
        damaged_page(read.tree_exists(&dest).unwrap_err()),
        page,
        "the page is damaged"
    );
    drop(read);
    store.close().unwrap();

    // A handle of a deleted tree whose page is damaged: deleting reads only the branch pages.
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let key = |i: usize| long_key(i, 1000);
    let mut write = store.write().unwrap();
    let tree = create(&mut write, "t");
    for i in 0..60 {
        write.set(&tree, &key(i), b"value").unwrap();
    }
    write.commit().unwrap();
    store.close().unwrap();
    let page = leaf_page(tmp.path(), Some("t"), &key(30));
    assert_ne!(
        page,
        leaf_page(tmp.path(), Some("t"), &key(0)),
        "the tree has several leaves"
    );
    damage(tmp.path(), page);
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let tree = write.open_tree("t", NORMAL).unwrap();
    write.delete_tree("t").unwrap();
    let error = write.get(&tree, &key(30)).unwrap_err();
    assert_eq!(not_found(error, Some(tmp.path())), "t");
    write.commit().unwrap();
    assert!(!store.read().unwrap().tree_exists("t").unwrap());
}

#[test]
fn ac_a14_a_new_store_takes_no_page_for_the_catalog() {
    stores(|store, _| {
        let info = store.info();
        assert!(info.size_bytes <= 1 << 20, "{info:?}");
        assert_eq!(info.free_pages, info.page_count - krummi_tx::FIRST_PAGE);
        assert!(store.read().unwrap().list_trees().unwrap().is_empty());
        let write = store.write().unwrap();
        assert!(write.list_trees().unwrap().is_empty());
        assert!(!write.tree_exists("t").unwrap());
        write.commit().unwrap();
        assert_eq!(store.info(), info, "looking at the catalog takes no page");
    });
}

/// The pages in use: the pages of the store minus its free pages, as its information gives them.
fn in_use(store: &Store) -> u64 {
    let info = store.info();
    info.page_count - info.free_pages
}

/// AC-A15 (A2, A3, F10, F12): in a store with a tree, within one region of free space, creating, filling over
/// several pages and levels, committing, deleting and committing a tree twice leaves the same pages in use after
/// both deletions, and those of before the first cycle, since the catalog keeps its only page (its entries are
/// small); and a tree filled the same way and emptied takes as many pages as when it was created empty. A commit
/// is durable when it returns, and a temporary store has its pages then.
#[test]
fn ac_a15_deleting_or_emptying_a_tree_gives_its_pages_back() {
    let fill = |write: &mut WriteTransaction<'_>, tree: &WriteTree<'_>| {
        for i in 0..60 {
            write.set(tree, &long_key(i, MAX_KEY), &[7; 100]).unwrap();
        }
    };
    stores(|store, _| {
        commit_tree(store, "a", &[(b"k", b"v")]);
        let before = in_use(store);
        let mut after_deleting = Vec::new();
        for _ in 0..2 {
            let mut write = store.write().unwrap();
            let tree = create(&mut write, "x");
            fill(&mut write, &tree);
            write.commit().unwrap();
            assert!(in_use(store) > before + 40, "several pages and levels");
            let mut write = store.write().unwrap();
            write.delete_tree("x").unwrap();
            write.commit().unwrap();
            after_deleting.push(in_use(store));
        }
        assert_eq!(after_deleting, [before, before]);

        commit_tree(store, "e", EMPTY);
        let empty = in_use(store);
        let mut write = store.write().unwrap();
        let tree = write.open_tree("e", NORMAL).unwrap();
        fill(&mut write, &tree);
        write.commit().unwrap();
        assert!(in_use(store) > empty + 40);
        let mut write = store.write().unwrap();
        write.open_tree_with("e", NORMAL, OpenMode::Empty).unwrap();
        write.commit().unwrap();
        assert_eq!(in_use(store), empty);
    });
}

#[test]
fn ac_a17_ac_b3_values_up_to_the_provisional_maximum() {
    stores(|store, folder| {
        let key = vec![b'k'; MAX_KEY];
        let max = vec![0xAB; MAX_VALUE];
        let mut write = store.write().unwrap();
        let tree = create(&mut write, "t");
        write.set(&tree, &key, &max).unwrap();
        write.set(&tree, b"empty", b"").unwrap();
        let mut empty_key = key.clone();
        empty_key[0] = b'e';
        write.set(&tree, &empty_key, b"").unwrap();
        let error = write.set(&tree, b"over", &[0; MAX_VALUE + 1]).unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::Value, 1025, 0, 1024)
        );
        assert_eq!(write.get(&tree, b"over").unwrap(), None, "nothing stored");
        write.commit().unwrap();
        let read = store.read().unwrap();
        let tree = read.open_tree("t", NORMAL).unwrap();
        assert_eq!(read.get(&tree, &key).unwrap(), Some(&max[..]));
        assert_eq!(read.get(&tree, &empty_key).unwrap(), Some(&b""[..]));
        assert_eq!(read.len(&tree).unwrap(), 3);
    });
    // On disk, also after reopening.
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let key = vec![b'k'; MAX_KEY];
    let max = vec![0xCD; MAX_VALUE];
    let mut write = store.write().unwrap();
    let tree = create(&mut write, "t");
    write.set(&tree, &key, &max).unwrap();
    write.set(&tree, &key[1..], b"").unwrap();
    write.commit().unwrap();
    store.close().unwrap();
    let store = open(tmp.path());
    let read = store.read().unwrap();
    let tree = read.open_tree("t", NORMAL).unwrap();
    assert_eq!(read.get(&tree, &key).unwrap(), Some(&max[..]));
    assert_eq!(read.get(&tree, &key[1..]).unwrap(), Some(&b""[..]));
}

#[test]
fn ac_b1_keys_of_1_to_2048_bytes() {
    stores(|store, folder| {
        let max = [7; MAX_VALUE];
        let keys = [vec![1], vec![2; 1025], vec![3; MAX_KEY]];
        let mut write = store.write().unwrap();
        let tree = create(&mut write, "t");
        for key in &keys {
            write.set(&tree, key, &max).unwrap();
        }
        for bad in [vec![], vec![4; MAX_KEY + 1]] {
            let len = bad.len() as u64;
            let expected = (SizeSubject::Key, len, 1, 2048);
            assert_eq!(
                invalid_size(write.set(&tree, &bad, b"v").unwrap_err(), folder),
                expected
            );
            assert_eq!(
                invalid_size(write.try_add(&tree, &bad, b"v").unwrap_err(), folder),
                expected
            );
            assert_eq!(
                invalid_size(write.get(&tree, &bad).unwrap_err(), folder),
                expected
            );
            assert_eq!(
                invalid_size(write.remove(&tree, &bad).unwrap_err(), folder),
                expected
            );
        }
        assert_eq!(write.len(&tree).unwrap(), 3, "without effect");
        write.commit().unwrap();
        let read = store.read().unwrap();
        let tree = read.open_tree("t", NORMAL).unwrap();
        for key in &keys {
            assert_eq!(read.get(&tree, key).unwrap(), Some(&max[..]));
        }
        let error = read.get(&tree, &[0; MAX_KEY + 1]).unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::Key, 2049, 1, 2048)
        );
        assert_eq!(read.len(&tree).unwrap(), 3);
    });
}

#[test]
fn ac_b2_names_of_1_to_1024_bytes() {
    stores(|store, folder| {
        let multibyte = format!("{}a", "€".repeat(341));
        assert_eq!(multibyte.len(), MAX_NAME);
        let good = ["x".to_owned(), "y".repeat(MAX_NAME), multibyte];
        let mut write = store.write().unwrap();
        for name in &good {
            create(&mut write, name);
        }
        let bad = [String::new(), "z".repeat(MAX_NAME + 1), "é".repeat(600)];
        for (name, len) in bad.iter().zip([0, 1025, 1200]) {
            for mode in [
                OpenMode::CreateNew,
                OpenMode::OpenOrCreate,
                OpenMode::CreateOrEmpty,
            ] {
                let error = write.open_tree_with(name, NORMAL, mode).err().unwrap();
                assert_eq!(
                    invalid_size(error, folder),
                    (SizeSubject::TreeName, len, 1, 1024)
                );
            }
        }
        write.commit().unwrap();
        let mut expected = good.to_vec();
        expected.sort();
        assert_eq!(names(store), expected, "nothing else was created");
    });
}

#[test]
fn ac_b4_validation_failures_do_not_prevent_the_commit() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let tree = create(&mut write, "t");
        write.set(&tree, b"before", b"1").unwrap();
        let error = write.set(&tree, b"", b"x").unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidSize);
        let error = write
            .open_tree_with("", NORMAL, OpenMode::CreateNew)
            .err()
            .unwrap();
        assert_eq!(invalid_size(error, folder).0, SizeSubject::TreeName);
        write.set(&tree, b"after", b"2").unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        let tree = read.open_tree("t", NORMAL).unwrap();
        assert_eq!(
            read.get(&tree, &[0; MAX_KEY + 1]).unwrap_err().kind(),
            ErrorKind::InvalidSize
        );
        assert_eq!(
            read.get(&tree, b"before").unwrap(),
            Some(&b"1"[..]),
            "the read goes on"
        );
        assert_eq!(read.get(&tree, b"after").unwrap(), Some(&b"2"[..]));
    });

    let tmp = tempfile::tempdir().unwrap();
    commit_tree(&open(tmp.path()), "t", &[(b"k", b"v")]);
    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        assert!(write.set(&tree, b"k", &[0; MAX_VALUE + 1]).is_err());
        assert!(write.remove(&tree, b"").is_err());
        assert!(
            write
                .open_tree_with("", NORMAL, OpenMode::OpenOrCreate)
                .is_err()
        );
        assert!(write.rename_tree("t", "", true).is_err());
        write.commit().unwrap();
    });
    assert!(
        !changed,
        "a write whose only operations failed validation writes nothing"
    );
}

#[test]
fn ac_b5_a_max_size_failure_wins_over_invalid_sizes() {
    let tmp = tempfile::tempdir().unwrap();
    let options = StoreOptions {
        max_size: Some(1 << 20),
    };
    let on_disk = Store::open(tmp.path(), &options).unwrap();
    let in_memory = Store::open_temporary(&options).unwrap();
    for store in [&on_disk, &in_memory] {
        for invalid_first in [false, true] {
            let mut write = store.write().unwrap();
            let tree = create(&mut write, "t");
            let invalid = |write: &mut WriteTransaction<'_>| {
                let error = write.set(&tree, &[0; MAX_KEY + 1], b"").unwrap_err();
                assert_eq!(error.kind(), ErrorKind::InvalidSize);
            };
            if invalid_first {
                invalid(&mut write);
            }
            let failure = (0..10_000)
                .find_map(|i| {
                    write
                        .set(&tree, &long_key(i, MAX_KEY), &[1; MAX_VALUE])
                        .err()
                })
                .unwrap();
            assert_eq!(failure.kind(), ErrorKind::MaxSize);
            if !invalid_first {
                invalid(&mut write);
            }
            assert_eq!(write.commit().unwrap_err().kind(), ErrorKind::MaxSize);
            assert!(names(store).is_empty(), "nothing applied");
        }
    }
    on_disk.close().unwrap();
}

#[test]
fn ac_b6_names_are_bytes_without_normalization() {
    stores(|store, _| {
        let names_ = ["pedidos", "Pedidos", "\u{e9}", "e\u{301}"];
        let mut write = store.write().unwrap();
        for name in names_ {
            let tree = create(&mut write, name);
            write.set(&tree, name.as_bytes(), b"mine").unwrap();
        }
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(read.list_trees().unwrap().len(), 4);
        for name in names_ {
            let tree = read.open_tree(name, NORMAL).unwrap();
            assert_eq!(read.len(&tree).unwrap(), 1, "{name}");
            for other in names_ {
                let expected = (other == name).then_some(&b"mine"[..]);
                assert_eq!(read.get(&tree, other.as_bytes()).unwrap(), expected);
            }
        }
    });
}

#[test]
fn ac_b9_the_invalid_size_wins_over_a_damaged_catalog() {
    let tmp = tempfile::tempdir().unwrap();
    commit_tree(&open(tmp.path()), "catalog-marker", EMPTY);
    let page = leaf_page(tmp.path(), None, b"catalog-marker");
    damage(tmp.path(), page);
    let store = open(tmp.path());
    let long = "n".repeat(MAX_NAME + 1);
    let read = store.read().unwrap();
    let error = read.open_tree(&long, NORMAL).err().unwrap();
    assert_eq!(invalid_size(error, Some(tmp.path())).1, 1025);
    assert_eq!(
        damaged_page(read.open_tree("catalog-marker", NORMAL).err().unwrap()),
        page
    );
    let mut write = store.write().unwrap();
    let error = write
        .open_tree_with(&long, NORMAL, OpenMode::OpenOrCreate)
        .err()
        .unwrap();
    assert_eq!(invalid_size(error, Some(tmp.path())).1, 1025);
    assert_eq!(
        damaged_page(write.open_tree("catalog-marker", NORMAL).err().unwrap()),
        page
    );
    assert_eq!(
        write.commit().unwrap_err().kind(),
        ErrorKind::Damaged,
        "damage follows T11"
    );
}

#[test]
fn ac_b10_every_handle_sees_the_changes_of_the_others() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let a = create(&mut write, "a");
        let b = create(&mut write, "b");
        let a_again = write.open_tree("a", NORMAL).unwrap();
        write.set(&a, b"1", b"a").unwrap();
        write.set(&a_again, b"2", b"a again").unwrap();
        write.set(&b, b"1", b"b").unwrap();
        assert_eq!(write.get(&a_again, b"1").unwrap(), Some(&b"a"[..]));
        assert_eq!(write.get(&a, b"2").unwrap(), Some(&b"a again"[..]));
        assert_eq!(write.len(&a).unwrap(), 2);
        assert_eq!(write.len(&a_again).unwrap(), 2);
        assert_eq!(write.get(&b, b"2").unwrap(), None);
        write.commit().unwrap();
        let read = store.read().unwrap();
        let a = read.open_tree("a", NORMAL).unwrap();
        let b = read.open_tree("b", NORMAL).unwrap();
        assert_eq!(read.get(&a, b"1").unwrap(), Some(&b"a"[..]));
        assert_eq!(read.get(&a, b"2").unwrap(), Some(&b"a again"[..]));
        assert_eq!(read.get(&b, b"1").unwrap(), Some(&b"b"[..]));
        assert_eq!((read.len(&a).unwrap(), read.len(&b).unwrap()), (2, 1));
    });
}

/// AC-B13 (B7): the key is checked before the value. No operation of 5.1 takes both a tree name and a key (a key
/// goes through a handle), so the name-before-key order has no operation to show it; the source name before the
/// destination is AC-A13.
#[test]
fn ac_b13_the_key_is_reported_before_the_value() {
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let tree = create(&mut write, "t");
        let (key, value) = (vec![0; MAX_KEY + 1], vec![0; MAX_VALUE + 1]);
        let error = write.set(&tree, &key, &value).unwrap_err();
        assert_eq!(
            invalid_size(error, folder),
            (SizeSubject::Key, 2049, 1, 2048)
        );
        let error = write.try_add(&tree, b"", &value).unwrap_err();
        assert_eq!(invalid_size(error, folder), (SizeSubject::Key, 0, 1, 2048));
    });
}

#[test]
fn ac_b14_handles_of_a_deleted_tree_never_reach_a_new_one() {
    stores(|store, folder| {
        commit_tree(store, "t", &[(b"k", b"original")]);
        let read = store.read().unwrap();
        let original = read.open_tree("t", NORMAL).unwrap();
        let mut write = store.write().unwrap();
        let first = write.open_tree("t", NORMAL).unwrap();
        let mut stale = vec![write.open_tree("t", NORMAL).unwrap()];
        write.delete_tree("t").unwrap();
        assert_eq!(not_found(write.get(&first, b"k").unwrap_err(), folder), "t");
        let mut current = create(&mut write, "t");
        for _ in 0..3 {
            for handle in stale.iter().chain([&first]) {
                assert_eq!(
                    not_found(write.set(handle, b"k", b"x").unwrap_err(), folder),
                    "t"
                );
                assert_eq!(not_found(write.len(handle).unwrap_err(), folder), "t");
            }
            assert_eq!(
                write.len(&current).unwrap(),
                0,
                "the new tree does not change"
            );
            write.delete_tree("t").unwrap();
            stale.push(current);
            current = create(&mut write, "t");
        }
        for handle in &stale {
            assert_eq!(not_found(write.get(handle, b"k").unwrap_err(), folder), "t");
        }
        write.set(&current, b"k", b"newest").unwrap();
        write.commit().unwrap();
        assert_eq!(read.get(&original, b"k").unwrap(), Some(&b"original"[..]));
        assert_eq!(read.len(&original).unwrap(), 1);
        let after = store.read().unwrap();
        let tree = after.open_tree("t", NORMAL).unwrap();
        assert_eq!(after.get(&tree, b"k").unwrap(), Some(&b"newest"[..]));
    });
}

/// Every key of `keys` with what `read` finds for it in `tree`, and the tree's count.
fn snapshot_of(
    read: &ReadTransaction<'_>,
    tree: &ReadTree<'_>,
    keys: &[Vec<u8>],
) -> (u64, Vec<Option<Vec<u8>>>) {
    let values = keys
        .iter()
        .map(|key| read.get(tree, key).unwrap().map(<[u8]>::to_vec))
        .collect();
    (read.len(tree).unwrap(), values)
}

/// AC-B17 (G3, T14): a read opened before a write whose insertions and removals split and merge pages of a tree
/// until its height changes sees exactly the entries of its snapshot, before and after the commit, and a new read
/// sees the write's. With keys of 2,048 bytes, three to a leaf and to a branch, 6 entries make two levels, 90 at
/// least four, and the 10 left at the end fewer.
#[test]
fn ac_b17_a_read_keeps_its_snapshot_while_a_write_changes_the_height() {
    let key = |i: usize| long_key(i, MAX_KEY);
    let keys: Vec<Vec<u8>> = (0..90).map(key).collect();
    stores(|store, _| {
        let first: Vec<(Vec<u8>, &[u8])> = (0..6).map(|i| (key(i), &b"first"[..])).collect();
        commit_tree(store, "t", &first);
        let read = store.read().unwrap();
        let tree = read.open_tree("t", NORMAL).unwrap();
        let snapshot = snapshot_of(&read, &tree, &keys);
        assert_eq!(snapshot.0, 6);

        let mut write = store.write().unwrap();
        let written = write.open_tree("t", NORMAL).unwrap();
        for i in 6..90 {
            write.set(&written, &key(i), b"grown").unwrap();
        }
        assert!(
            snapshot_of(&read, &tree, &keys) == snapshot,
            "while the tree grows"
        );
        for i in (0..90).filter(|i| i % 9 != 0) {
            assert!(write.remove(&written, &key(i)).unwrap());
        }
        write.set(&written, &key(0), b"changed").unwrap();
        assert!(
            snapshot_of(&read, &tree, &keys) == snapshot,
            "before the commit"
        );
        write.commit().unwrap();
        assert!(
            snapshot_of(&read, &tree, &keys) == snapshot,
            "after the commit"
        );

        let new = store.read().unwrap();
        let tree = new.open_tree("t", NORMAL).unwrap();
        let expected: Vec<Option<Vec<u8>>> = (0..90)
            .map(|i| match i {
                0 => Some(b"changed".to_vec()),
                _ if i % 9 != 0 => None,
                _ if i < 6 => Some(b"first".to_vec()),
                _ => Some(b"grown".to_vec()),
            })
            .collect();
        assert!(
            snapshot_of(&new, &tree, &keys) == (10, expected),
            "a new read"
        );
    });
}

/// AC20 of the root (D11): a value read from a tree is the engine's memory, not a copy made for the read: two
/// reads of it, in one transaction or in two that see the same version, return the same address, and two values
/// of one leaf lie within one page.
#[test]
fn ac20_values_read_from_a_tree_point_into_the_engine_s_memory() {
    stores(|store, _| {
        commit_tree(store, "t", &[(b"a", &[0; 100]), (b"b", &[0; 100])]);
        // An older read keeps the next version out of `krummi.data`, so the two reads below see it in one place.
        let oldest = store.read().unwrap();
        commit_tree(store, "t", &[(b"a", &[1; 100]), (b"b", &[2; 100])]);
        let first = store.read().unwrap();
        let second = store.read().unwrap();
        let (t1, t2) = (
            first.open_tree("t", NORMAL).unwrap(),
            second.open_tree("t", NORMAL).unwrap(),
        );
        let a = first.get(&t1, b"a").unwrap().unwrap();
        assert_eq!(a.as_ptr(), first.get(&t1, b"a").unwrap().unwrap().as_ptr());
        assert_eq!(a.as_ptr(), second.get(&t2, b"a").unwrap().unwrap().as_ptr());
        let b = first.get(&t1, b"b").unwrap().unwrap();
        let distance = (b.as_ptr() as usize).abs_diff(a.as_ptr() as usize);
        assert!(distance < store.info().page_size as usize, "{distance}");
        let write = store.write().unwrap();
        let tree = write.open_tree("t", NORMAL).unwrap();
        let a = write.get(&tree, b"a").unwrap().unwrap();
        assert_eq!(
            a.as_ptr(),
            write.get(&tree, b"a").unwrap().unwrap().as_ptr()
        );
        drop(oldest);
    });
}
