//! Helpers shared by the tree tests: stores on disk and temporary, error matchers, file comparison and page damage.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use krummi_pager::DATA_FILE;
use krummi_pager::format::PAGE_SIZE;
use krummi_tree::catalog;
use krummi_tx::{Engine, OpenOptions as EngineOptions};

use krummi::{
    Error, OpenMode, SizeSubject, Store, StoreOptions, TreeKind, WriteTransaction, WriteTree,
};

pub const NORMAL: TreeKind = TreeKind::Normal;
// The size limits are written out on purpose instead of taken from the engine: they pin the PRDs' limits (B4, B5,
// V1 of the large values PRD, which replaced A11), so a change in the engine's constants fails these tests.
pub const MAX_NAME: usize = 1024;
pub const MAX_KEY: usize = 2048;
pub const MAX_VALUE: usize = 268_435_456;

/// Runs `test` on a store on disk and on a temporary one; `folder` is the store's folder, `None` when temporary.
pub fn stores(test: impl Fn(&Store, Option<&Path>)) {
    let tmp = tempfile::tempdir().unwrap();
    let on_disk = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    test(&on_disk, Some(tmp.path()));
    on_disk.close().unwrap();
    let in_memory = Store::open_temporary(&StoreOptions::default()).unwrap();
    test(&in_memory, None);
    in_memory.close().unwrap();
}

pub fn open(folder: &Path) -> Store {
    Store::open(folder, &StoreOptions::default()).unwrap()
}

pub fn create<'s>(write: &mut WriteTransaction<'s>, name: &str) -> WriteTree<'s> {
    write
        .open_tree_with(name, NORMAL, OpenMode::CreateNew)
        .unwrap()
}

/// No entries, for [`commit_tree`].
pub const EMPTY: &[(&[u8], &[u8])] = &[];

/// Commits a tree `name` with `entries`.
pub fn commit_tree(store: &Store, name: &str, entries: &[(impl AsRef<[u8]>, impl AsRef<[u8]>)]) {
    let mut write = store.write().unwrap();
    let tree = write
        .open_tree_with(name, NORMAL, OpenMode::OpenOrCreate)
        .unwrap();
    for (key, value) in entries {
        write.set(&tree, key.as_ref(), value.as_ref()).unwrap();
    }
    write.commit().unwrap();
}

pub fn names(store: &Store) -> Vec<String> {
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

pub fn folder_of(folder: Option<&Path>) -> Option<PathBuf> {
    folder.map(Path::to_path_buf)
}

/// The tree name of a tree-not-found error, after checking its folder.
#[track_caller]
pub fn not_found(error: Error, folder: Option<&Path>) -> String {
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
pub fn already_exists(error: Error, folder: Option<&Path>) -> String {
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
pub fn invalid_size(error: Error, folder: Option<&Path>) -> (SizeSubject, u64, u64, u64) {
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
pub fn misuse(error: Error, folder: Option<&Path>) {
    match error {
        Error::Misuse { folder: f, .. } => assert_eq!(f, folder_of(folder)),
        other => panic!("expected misuse, got {other}"),
    }
}

/// Every file of the store's folder with its bytes, in name order, subfolders included.
pub fn files(folder: &Path) -> Vec<(PathBuf, Vec<u8>)> {
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
pub fn files_change(folder: &Path, change: impl FnOnce(&Store)) -> bool {
    let before = files(folder);
    let store = open(folder);
    change(&store);
    store.close().unwrap();
    files(folder) != before
}

/// Runs `look` on a read of the closed store through the engine's internals, which opening and closing without
/// changes leave as they were.
pub fn inspect<T>(folder: &Path, look: impl FnOnce(&krummi_tx::ReadTransaction<'_>) -> T) -> T {
    let engine = Engine::open(folder, &EngineOptions::default()).unwrap();
    let read = engine.read().unwrap();
    let seen = look(&read);
    drop(read);
    engine.close().unwrap();
    seen
}

/// The page of the leaf where `key` is or would be in the closed store: in the tree `tree`, or in the catalog when
/// `tree` is `None` (then `key` is a tree name).
pub fn leaf_page(folder: &Path, tree: Option<&str>, key: &[u8]) -> u64 {
    inspect(folder, |read| {
        let root = match tree {
            Some(name) => catalog::lookup(read, name).unwrap().unwrap().root,
            None => read.slot(catalog::ROOT_SLOT).unwrap(),
        };
        krummi_tree::leaf_of(read, root, key).unwrap()
    })
}

/// The value run of `key` in the tree `tree` of the closed store: its first page and its number of pages, or `None`
/// when the value is in its leaf.
pub fn value_run(folder: &Path, tree: &str, key: &[u8]) -> Option<(u64, u64)> {
    inspect(folder, |read| {
        let root = catalog::lookup(read, tree).unwrap().unwrap().root;
        krummi_tree::run_of(read, root, key).unwrap()
    })
}

/// The root page of the tree `tree` in the closed store.
pub fn root_page(folder: &Path, tree: &str) -> u64 {
    inspect(folder, |read| {
        catalog::lookup(read, tree).unwrap().unwrap().root
    })
}

/// Flips a byte of `page` in the closed store's data file, so that its checksum fails.
pub fn damage(folder: &Path, page: u64) {
    let path = folder.join(DATA_FILE);
    let mut bytes = fs::read(&path).unwrap();
    bytes[page as usize * PAGE_SIZE + 100] ^= 0xFF;
    fs::write(&path, bytes).unwrap();
}

#[track_caller]
pub fn damaged_page(error: Error) -> u64 {
    match error {
        Error::Damaged {
            page: Some(page), ..
        } => page,
        other => panic!("expected damage with a page, got {other}"),
    }
}

/// A key of `len` bytes that starts with `i`, so that keys sort by `i`.
pub fn long_key(i: usize, len: usize) -> Vec<u8> {
    let mut key = format!("{i:05}").into_bytes();
    key.resize(len, b'.');
    key
}
