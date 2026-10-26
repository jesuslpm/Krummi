//! Krummi: an embedded transactional storage engine inspired by Voron.
//!
//! Module 1 (the Pager) lets an application create, open, inspect and close stores. Module 2 adds transactions:
//! read transactions that see a snapshot of the store, from any number of threads, and one write transaction at a
//! time, which the others wait for in order of arrival. Module 3 adds the journal: a commit that returns survives
//! a crash of the process or the system and a power cut, and opening a store after a crash recovers it without
//! any action of the application. Module 4 adds free space: the pages that writes free are reused before the store
//! grows, and [`StoreInfo`] reports how many pages are free. Module 5 adds named trees: inside a transaction an
//! application opens a tree by its name and kind, stores, adds, reads and removes entries by key, and creates,
//! empties, renames, lists and deletes trees. It scans a tree in key order from both ends, between two bounds and
//! with an optional prefix, and removes every entry of an interval at once.
//!
//! ```
//! use krummi::{OpenMode, Store, StoreOptions, TreeKind};
//!
//! let store = Store::open_temporary(&StoreOptions::default())?;
//! let read = store.read()?;      // sees the store as it is now
//! let mut write = store.write()?; // the one write transaction
//! let tree = write.open_tree_with("orders", TreeKind::Normal, OpenMode::OpenOrCreate)?;
//! write.set(&tree, b"key", b"value")?;
//! write.commit()?;               // visible to reads opened from now on
//! assert!(!read.tree_exists("orders")?);
//! drop(read);
//! let read = store.read()?;
//! let tree = read.open_tree("orders", TreeKind::Normal)?;
//! assert_eq!(read.get(&tree, b"key")?, Some(&b"value"[..]));
//! drop(read);
//! store.close()?;
//! # Ok::<(), krummi::Error>(())
//! ```

use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops::{Bound, RangeBounds};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub use krummi_pager::{Error, ErrorKind, Info as StoreInfo, Result, SizeSubject, TreeKind};
use krummi_tree::catalog::{self, Entry};
use krummi_tree::{Interval, PageSource};

/// Maximum bytes of a tree name in UTF-8 (B4).
const MAX_NAME: usize = 1024;

#[cfg(test)]
mod tests;

/// Options for opening a store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreOptions {
    /// Maximum size of the store in bytes while it stays open: at least 1 MiB, rounded down to the page size. It
    /// counts the data file and the committed page versions that are not in it yet (in the `krummi.temp`
    /// subfolder), not the journal. Recovery after a crash restores the store even beyond it.
    pub max_size: Option<u64>,
}

/// An open store. Only one can be open per folder at a time, in any process. It can be used from several threads
/// at once; it cannot be closed while a transaction is open.
pub struct Store {
    engine: krummi_tx::Engine,
}

impl Store {
    /// Opens the store in `folder`. A missing folder is created with its parents, and an empty folder gets a
    /// new store. If the store closed in a crash, opening recovers it first: every commit that was durable is
    /// there, and no part of one that was not. Besides `krummi.data`, an open store may keep in its folder the
    /// journal files `krummi.journal.<number>` and the `krummi.temp` subfolder; a clean close removes them.
    pub fn open(folder: impl AsRef<Path>, options: &StoreOptions) -> Result<Store> {
        let engine = krummi_tx::Engine::open(folder, &engine_options(options))?;
        Ok(Store { engine })
    }

    /// Opens a store that lives only in memory and keeps nothing when it closes.
    pub fn open_temporary(options: &StoreOptions) -> Result<Store> {
        let engine = krummi_tx::Engine::open_temporary(&engine_options(options))?;
        Ok(Store { engine })
    }

    /// Information about the store.
    pub fn info(&self) -> StoreInfo {
        self.engine.info()
    }

    /// Opens a read transaction, which sees the store as it is now: every write committed before, none after.
    /// There is no limit on how many are open at once; it fails only when memory runs out.
    pub fn read(&self) -> Result<ReadTransaction<'_>> {
        Ok(ReadTransaction {
            inner: self.engine.read()?,
            folder: Folder(self.engine.folder()),
            serial: next_serial(),
        })
    }

    /// Opens the write transaction. While another one is open, or others are already waiting, it waits for its
    /// turn in order of arrival, without limit. Fails with [`ErrorKind::Misuse`] if this thread already has one
    /// open on this store, and with [`ErrorKind::Io`] if the store refuses writes after a failed write.
    pub fn write(&self) -> Result<WriteTransaction<'_>> {
        self.open_write(None)
    }

    /// Like [`Store::write`], but waits at most `limit`; when it expires the request fails with
    /// [`ErrorKind::Timeout`] and has no effect. A zero limit gets the write transaction only if it is free and
    /// nobody is waiting. Limits of up to 24 hours are always accepted; one too large to count from now fails
    /// with [`ErrorKind::Misuse`].
    pub fn write_with_timeout(&self, limit: Duration) -> Result<WriteTransaction<'_>> {
        self.open_write(Some(limit))
    }

    /// Makes every commit made so far durable, deferred ones included, and also the ones still waiting to be
    /// durable: when this returns, they survive a crash of the system or a power cut. It does not wait for the
    /// open write transaction. Fails with
    /// [`ErrorKind::Io`] if the store refuses writes after a failed write, even with nothing pending; a failure
    /// to flush makes the store refuse writes until it is reopened. A temporary store has nothing to do.
    pub fn make_durable(&self) -> Result<()> {
        self.engine.make_durable()
    }

    /// Closes the store: moves what was committed into `krummi.data`, removes the journal and the temporary
    /// files, and releases its lock, even if writing fails; then the journal stays and the next opening recovers
    /// every commit. Dropping a `Store` does the same, but does not report a failure.
    pub fn close(self) -> Result<()> {
        self.engine.close()
    }

    fn open_write(&self, limit: Option<Duration>) -> Result<WriteTransaction<'_>> {
        Ok(WriteTransaction {
            inner: self.engine.write(limit)?,
            folder: Folder(self.engine.folder()),
            serial: next_serial(),
            trees: RefCell::default(),
        })
    }
}

/// A read transaction: a snapshot of the store as it was when it opened. It can be sent to other threads and
/// shared between them, which see the same snapshot. It ends when it is dropped.
pub struct ReadTransaction<'s> {
    inner: krummi_tx::ReadTransaction<'s>,
    folder: Folder<'s>,
    serial: u64,
}

/// The write transaction of a store. It stays on the thread that opened it. Dropping it without committing
/// aborts it.
pub struct WriteTransaction<'s> {
    inner: krummi_tx::WriteTransaction<'s>,
    folder: Folder<'s>,
    serial: u64,
    /// The state of every tree this transaction opened, shared by all the tree's handles (B9).
    trees: RefCell<Trees>,
}

/// How [`WriteTransaction::open_tree_with`] opens a tree; [`WriteTransaction::open_tree`] only opens. Opening an
/// existing tree of another kind fails with [`ErrorKind::DifferentTreeType`] in every mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpenMode {
    /// Creates an empty tree; fails with [`ErrorKind::TreeAlreadyExists`] if it exists.
    CreateNew,
    /// Opens the tree if it exists, otherwise creates it empty.
    OpenOrCreate,
    /// Creates the tree empty if it does not exist, otherwise removes all its entries.
    CreateOrEmpty,
    /// Removes all the entries of the tree; fails with [`ErrorKind::TreeNotFound`] if it does not exist.
    Empty,
}

/// A scan of a tree in a read transaction, from [`ReadTransaction::range`]: a double-ended iterator of keys
/// with their values, in key order. It can be sent to another thread, like its transaction. After a failure it
/// returns nothing more from either end.
pub struct ReadRange<'t>(krummi_tree::Range<'t, krummi_tx::ReadTransaction<'t>>);

/// A scan of a tree in a write transaction, from [`WriteTransaction::range`], like [`ReadRange`]. It stays on its
/// transaction's thread and ends at the transaction's next operation that may modify it.
pub struct WriteRange<'t>(krummi_tree::Range<'t, krummi_tx::WriteTransaction<'t>>);

macro_rules! scan_iterator {
    ($range:ident) => {
        impl<'t> Iterator for $range<'t> {
            type Item = Result<(&'t [u8], &'t [u8])>;

            fn next(&mut self) -> Option<Self::Item> {
                self.0.next()
            }
        }

        impl DoubleEndedIterator for $range<'_> {
            fn next_back(&mut self) -> Option<Self::Item> {
                self.0.next_back()
            }
        }

        impl std::iter::FusedIterator for $range<'_> {}
    };
}

scan_iterator!(ReadRange);
scan_iterator!(WriteRange);

/// A tree opened in a read transaction: it sees the transaction's snapshot. It can be sent to other threads and
/// shared between them, like the transaction, and cannot outlive its store. Using it in another transaction
/// fails with [`ErrorKind::Misuse`] without effect.
pub struct ReadTree<'s> {
    serial: u64,
    root: u64,
    count: u64,
    _store: PhantomData<&'s Store>,
}

/// A tree opened in a write transaction. Every handle of the same tree in that transaction sees the same state,
/// with the changes made through any of them. It stays on the thread that opened it, like the transaction, and
/// cannot outlive its store. Using it in another transaction fails with [`ErrorKind::Misuse`] without effect,
/// and using it after its tree was deleted fails with [`ErrorKind::TreeNotFound`].
pub struct WriteTree<'s> {
    serial: u64,
    /// The tree's identity, distinct from its name (A8).
    id: u64,
    /// The raw pointer keeps the handle on its thread: it is neither `Send` nor `Sync`.
    _store: PhantomData<(&'s Store, *const ())>,
}

/// The trees a write transaction opened, sorted by identity. Growing it is a reservation that can fail with
/// [`ErrorKind::OutOfMemory`] instead of aborting the process (P13).
#[derive(Default)]
struct Trees(Vec<TreeState>);

impl Trees {
    /// The position of the tree `id`, or where it would go.
    fn find(&self, id: u64) -> std::result::Result<usize, usize> {
        self.0.binary_search_by_key(&id, |state| state.id)
    }

    fn get(&self, id: u64) -> Option<&TreeState> {
        self.find(id).ok().map(|at| &self.0[at])
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut TreeState> {
        self.find(id).ok().map(|at| &mut self.0[at])
    }
}

/// A tree as its write transaction sees it.
struct TreeState {
    /// Its identity (A8).
    id: u64,
    /// Its current name or, once deleted, the last one it had (A6).
    name: String,
    kind: TreeKind,
    root: u64,
    count: u64,
    deleted: bool,
    /// Its root or count differ from its catalog entry, which the commit then rewrites.
    dirty: bool,
}

impl<'s> ReadTransaction<'s> {
    /// Opens the tree `name` of kind `kind`. Fails with [`ErrorKind::InvalidSize`] if the name is empty or longer
    /// than 1,024 bytes, with [`ErrorKind::DifferentTreeType`] if the tree has another kind, and with
    /// [`ErrorKind::TreeNotFound`] if the snapshot does not have it. It reads only the catalog.
    pub fn open_tree(&self, name: &str, kind: TreeKind) -> Result<ReadTree<'s>> {
        self.folder.check_name(name)?;
        let entry = find(self.folder, &self.inner, name, kind)?;
        let entry = entry.ok_or_else(|| self.folder.not_found(name))?;
        Ok(ReadTree {
            serial: self.serial,
            root: entry.root,
            count: entry.count,
            _store: PhantomData,
        })
    }

    /// Whether the snapshot has a tree named `name`.
    pub fn tree_exists(&self, name: &str) -> Result<bool> {
        self.folder.check_name(name)?;
        Ok(catalog::lookup(&self.inner, name)?.is_some())
    }

    /// The name and kind of every tree in the snapshot, in byte order of the names. The list is a copy: running
    /// out of memory for it fails with [`ErrorKind::OutOfMemory`] and the transaction stays usable (P13).
    pub fn list_trees(&self) -> Result<Vec<(String, TreeKind)>> {
        catalog::list(&self.inner)
    }

    /// The value of `key` in `tree`, or `None` if the key is not there. The reference points into the engine's
    /// memory, not to a copy, and lives as long as the transaction.
    pub fn get(&self, tree: &ReadTree<'_>, key: &[u8]) -> Result<Option<&[u8]>> {
        self.folder.check_key(key)?;
        self.check_handle(tree)?;
        krummi_tree::get(&self.inner, tree.root, key)
    }

    /// The number of entries of `tree` in the snapshot, counted without walking the tree.
    pub fn len(&self, tree: &ReadTree<'_>) -> Result<u64> {
        self.check_handle(tree)?;
        Ok(tree.count)
    }

    /// A scan of the entries of `tree` whose keys are within `bounds`, in key order from its front end
    /// ([`Iterator::next`]) and in reverse order from its back end ([`DoubleEndedIterator::next_back`]); each entry
    /// comes out once. A bound's key need not be in the tree. Bounds that leave no key between them, such as a
    /// lower bound after the upper one, give an empty scan, not an error. Fails with [`ErrorKind::InvalidSize`] if
    /// a bound's key is empty or longer than 2,048 bytes, also when the tree or the interval is empty. The keys and
    /// values point into the engine's memory, not to copies, and live as long as the transaction.
    pub fn range<'k>(
        &self,
        tree: &ReadTree<'_>,
        bounds: impl RangeBounds<&'k [u8]>,
    ) -> Result<ReadRange<'_>> {
        self.range_prefix(tree, &[], bounds)
    }

    /// Like [`range`](Self::range), only with the keys that start with `prefix`; an empty prefix limits nothing.
    /// Fails with [`ErrorKind::InvalidSize`] if `prefix` is longer than 2,048 bytes, after checking the bounds.
    pub fn range_prefix<'k>(
        &self,
        tree: &ReadTree<'_>,
        prefix: &'k [u8],
        bounds: impl RangeBounds<&'k [u8]>,
    ) -> Result<ReadRange<'_>> {
        let interval = self.folder.interval(prefix, &bounds)?;
        self.check_handle(tree)?;
        Ok(ReadRange(krummi_tree::Range::new(
            &self.inner,
            tree.root,
            interval,
        )?))
    }

    fn check_handle(&self, tree: &ReadTree<'_>) -> Result<()> {
        if tree.serial == self.serial {
            Ok(())
        } else {
            Err(self.folder.foreign_handle())
        }
    }
}

/// The operations on trees of a write transaction. The ones that take `&self` do not modify the transaction and
/// keep the references read before; the ones that take `&mut self` may modify it, so the compiler ends those
/// references, even when the operation fails or changes nothing.
///
/// The failures of validation, [`ErrorKind::InvalidSize`], [`ErrorKind::DifferentTreeType`],
/// [`ErrorKind::TreeNotFound`], [`ErrorKind::TreeAlreadyExists`] and [`ErrorKind::Misuse`] for a handle of another
/// transaction, have no effect and do not prevent the commit; any other failure does (T11).
impl<'s> WriteTransaction<'s> {
    /// Opens the tree `name` of kind `kind`. Fails with [`ErrorKind::InvalidSize`] if the name is empty or longer
    /// than 1,024 bytes, with [`ErrorKind::DifferentTreeType`] if the tree has another kind, and with
    /// [`ErrorKind::TreeNotFound`] if it does not exist. It reads only the catalog.
    pub fn open_tree(&self, name: &str, kind: TreeKind) -> Result<WriteTree<'s>> {
        self.folder.check_name(name)?;
        let entry = find(self.folder, &self.inner, name, kind)?;
        let entry = entry.ok_or_else(|| self.folder.not_found(name))?;
        self.handle(name, &entry)
    }

    /// Opens the tree `name` of kind `kind` in `mode`, with the failures of [`open_tree`](Self::open_tree)
    /// except that the creating modes create a missing tree. Emptying a tree frees its pages; it keeps its
    /// handles, which see it empty.
    pub fn open_tree_with(
        &mut self,
        name: &str,
        kind: TreeKind,
        mode: OpenMode,
    ) -> Result<WriteTree<'s>> {
        self.folder.check_name(name)?;
        let entry = match (find(self.folder, &self.inner, name, kind)?, mode) {
            (None, OpenMode::Empty) => return Err(self.folder.not_found(name)),
            (None, _) => self.create(name, kind)?,
            (Some(_), OpenMode::CreateNew) => return Err(self.folder.already_exists(name)),
            (Some(entry), OpenMode::OpenOrCreate) => entry,
            (Some(entry), OpenMode::CreateOrEmpty | OpenMode::Empty) => self.empty(name, entry)?,
        };
        self.handle(name, &entry)
    }

    /// Whether this transaction sees a tree named `name`, with its own changes.
    pub fn tree_exists(&self, name: &str) -> Result<bool> {
        self.folder.check_name(name)?;
        Ok(catalog::lookup(&self.inner, name)?.is_some())
    }

    /// The name and kind of every tree this transaction sees, with its own changes, in byte order of the names.
    /// The list is a copy: running out of memory for it fails with [`ErrorKind::OutOfMemory`] (P13).
    pub fn list_trees(&self) -> Result<Vec<(String, TreeKind)>> {
        catalog::list(&self.inner)
    }

    /// Deletes the tree `name` with all its entries and frees its pages; a missing tree is not an error. The
    /// tree's handles then fail with [`ErrorKind::TreeNotFound`].
    pub fn delete_tree(&mut self, name: &str) -> Result<()> {
        self.folder.check_name(name)?;
        match catalog::lookup(&self.inner, name)? {
            Some(entry) => self.delete(name, entry),
            None => Ok(()),
        }
    }

    /// Renames the tree `from` to `to`; it keeps its kind, its entries and its handles. Fails with
    /// [`ErrorKind::TreeNotFound`] if `from` does not exist, also when `to` is the same name, and with
    /// [`ErrorKind::TreeAlreadyExists`] if `to` exists, unless `overwrite` is set: then `to` is deleted first,
    /// whatever its kind. Renaming a tree to its own name does nothing.
    pub fn rename_tree(&mut self, from: &str, to: &str, overwrite: bool) -> Result<()> {
        self.folder.check_name(from)?;
        self.folder.check_name(to)?;
        let source = catalog::lookup(&self.inner, from)?;
        let source = source.ok_or_else(|| self.folder.not_found(from))?;
        if from == to {
            return Ok(());
        }
        // The destination is rejected before anything is reserved (A7); the new name is copied before any change.
        let target = catalog::lookup(&self.inner, to)?;
        if target.is_some() && !overwrite {
            return Err(self.folder.already_exists(to));
        }
        let renamed = match self.trees.get_mut().get(source.id) {
            Some(_) => Some(krummi_tree::copy_str(&self.inner, to)?),
            None => None,
        };
        if let Some(target) = target {
            self.delete(to, target)?;
        }
        let source = self.current(source);
        catalog::put(&mut self.inner, to, &source)?;
        catalog::remove(&mut self.inner, from)?;
        if let (Some(name), Some(state)) = (renamed, self.trees.get_mut().get_mut(source.id)) {
            state.name = name;
            state.dirty = false;
        }
        Ok(())
    }

    /// The value of `key` in `tree`, with this transaction's changes, or `None` if the key is not there. The
    /// reference points into the engine's memory, not to a copy, and lives until the next operation that may
    /// modify the transaction.
    pub fn get(&self, tree: &WriteTree<'_>, key: &[u8]) -> Result<Option<&[u8]>> {
        self.folder.check_key(key)?;
        let (root, _) = self.live(tree)?;
        krummi_tree::get(&self.inner, root, key)
    }

    /// The number of entries of `tree`, with this transaction's changes, counted without walking the tree.
    pub fn len(&self, tree: &WriteTree<'_>) -> Result<u64> {
        Ok(self.live(tree)?.1)
    }

    /// A scan of `tree` within `bounds`, with this transaction's changes, as [`ReadTransaction::range`] describes.
    /// It and the keys and values it returns live until the next operation that may modify the transaction. A
    /// failure while scanning, other than invalid size, is the transaction's (T11).
    pub fn range<'k>(
        &self,
        tree: &WriteTree<'_>,
        bounds: impl RangeBounds<&'k [u8]>,
    ) -> Result<WriteRange<'_>> {
        self.range_prefix(tree, &[], bounds)
    }

    /// Like [`range`](Self::range), only with the keys that start with `prefix`, as
    /// [`ReadTransaction::range_prefix`] describes.
    pub fn range_prefix<'k>(
        &self,
        tree: &WriteTree<'_>,
        prefix: &'k [u8],
        bounds: impl RangeBounds<&'k [u8]>,
    ) -> Result<WriteRange<'_>> {
        let interval = self.folder.interval(prefix, &bounds)?;
        let (root, _) = self.live(tree)?;
        Ok(WriteRange(krummi_tree::Range::new(
            &self.inner,
            root,
            interval,
        )?))
    }

    /// Stores `value` under `key` in `tree`, replacing the value if the key is there.
    pub fn set(&mut self, tree: &WriteTree<'_>, key: &[u8], value: &[u8]) -> Result<()> {
        self.folder.check_key(key)?;
        self.folder.check_value(value)?;
        let (root, _) = self.live(tree)?;
        let update = krummi_tree::set(&mut self.inner, root, key, value)?;
        self.apply(tree, update, true);
        Ok(())
    }

    /// Stores `value` under `key` in `tree` only if the key is not there, and returns whether it did. The sizes
    /// are checked first, also when the key is there.
    pub fn try_add(&mut self, tree: &WriteTree<'_>, key: &[u8], value: &[u8]) -> Result<bool> {
        self.folder.check_key(key)?;
        self.folder.check_value(value)?;
        let (root, _) = self.live(tree)?;
        let update = krummi_tree::try_add(&mut self.inner, root, key, value)?;
        self.apply(tree, update, true);
        Ok(update.count_changed)
    }

    /// Removes `key` from `tree`, and returns whether it was there.
    pub fn remove(&mut self, tree: &WriteTree<'_>, key: &[u8]) -> Result<bool> {
        self.folder.check_key(key)?;
        let (root, _) = self.live(tree)?;
        let update = krummi_tree::remove(&mut self.inner, root, key)?;
        self.apply(tree, update, false);
        Ok(update.count_changed)
    }

    /// Removes from `tree` every entry whose key is within `bounds`, and returns how many it removed. The bounds
    /// follow [`ReadTransaction::range`]: bounds that leave no key between them remove nothing and read nothing.
    /// The tree keeps its identity and its handles, which see the entries gone; the pages it no longer uses are
    /// freed. Fails with [`ErrorKind::InvalidSize`] if a bound's key is empty or longer than 2,048 bytes.
    pub fn remove_range<'k>(
        &mut self,
        tree: &WriteTree<'_>,
        bounds: impl RangeBounds<&'k [u8]>,
    ) -> Result<u64> {
        self.remove_range_prefix(tree, &[], bounds)
    }

    /// Like [`remove_range`](Self::remove_range), only with the keys that start with `prefix`; an empty prefix
    /// limits nothing. Fails with [`ErrorKind::InvalidSize`] if `prefix` is longer than 2,048 bytes, after checking
    /// the bounds.
    pub fn remove_range_prefix<'k>(
        &mut self,
        tree: &WriteTree<'_>,
        prefix: &'k [u8],
        bounds: impl RangeBounds<&'k [u8]>,
    ) -> Result<u64> {
        let interval = self.folder.interval(prefix, &bounds)?;
        let (root, _) = self.live(tree)?;
        let update = krummi_tree::remove_range(&mut self.inner, root, interval)?;
        let state = self
            .trees
            .get_mut()
            .get_mut(tree.id)
            .expect("a live handle");
        if update.removed > 0 || update.root != state.root {
            // After a failure of T11 the tree may be inconsistent and the transaction cannot commit (left to the
            // implementation): the count must not wrap. In a healthy write it never removes more than it counts.
            debug_assert!(
                update.removed <= state.count || self.inner.has_failed(),
                "removed {} of {} entries",
                update.removed,
                state.count
            );
            state.count = state.count.saturating_sub(update.removed);
            state.root = update.root;
            state.dirty = true;
        }
        Ok(update.removed)
    }

    /// A handle of the tree of `entry`, now named `name`; the tree's state joins the table on its first opening.
    fn handle(&self, name: &str, entry: &Entry) -> Result<WriteTree<'s>> {
        let mut trees = self.trees.borrow_mut();
        if let Err(at) = trees.find(entry.id) {
            let name = krummi_tree::copy_str(&self.inner, name)?;
            krummi_tree::reserve(&self.inner, &mut trees.0, 1)?;
            trees.0.insert(
                at,
                TreeState {
                    id: entry.id,
                    name,
                    kind: entry.kind,
                    root: entry.root,
                    count: entry.count,
                    deleted: false,
                    dirty: false,
                },
            );
        }
        Ok(WriteTree {
            serial: self.serial,
            id: entry.id,
            _store: PhantomData,
        })
    }

    /// `entry` with the root and the count this transaction gave its tree, which the catalog gets at commit.
    fn current(&self, entry: Entry) -> Entry {
        match self.trees.borrow().get(entry.id) {
            Some(state) => Entry {
                root: state.root,
                count: state.count,
                ..entry
            },
            None => entry,
        }
    }

    /// The root and the count of the tree of `tree`. Fails with misuse for a handle of another transaction and
    /// with not found once the tree was deleted, without reading anything (A7).
    fn live(&self, tree: &WriteTree<'_>) -> Result<(u64, u64)> {
        if tree.serial != self.serial {
            return Err(self.folder.foreign_handle());
        }
        let trees = self.trees.borrow();
        let state = trees.get(tree.id).expect("a live handle");
        if state.deleted {
            return Err(self.folder.not_found(&state.name));
        }
        Ok((state.root, state.count))
    }

    /// Records what a key operation did to the tree of `tree`; `added` tells which way the count moved.
    fn apply(&mut self, tree: &WriteTree<'_>, update: krummi_tree::Update, added: bool) {
        let state = self
            .trees
            .get_mut()
            .get_mut(tree.id)
            .expect("a live handle");
        if update.count_changed {
            state.count = if added {
                state.count + 1
            } else {
                state.count - 1
            };
        }
        if update.count_changed || update.root != state.root {
            state.root = update.root;
            state.dirty = true;
        }
    }

    /// Creates the empty tree `name` with a new identity (A8).
    fn create(&mut self, name: &str, kind: TreeKind) -> Result<Entry> {
        let id = catalog::new_identity(&mut self.inner)?;
        let root = krummi_tree::create(&mut self.inner)?;
        let entry = Entry {
            id,
            kind,
            root,
            count: 0,
        };
        catalog::put(&mut self.inner, name, &entry)?;
        Ok(entry)
    }

    /// Empties the tree `name` of `entry`: it keeps its identity, and its pages give way to one empty root (A2).
    fn empty(&mut self, name: &str, entry: Entry) -> Result<Entry> {
        let entry = self.current(entry);
        krummi_tree::free_all(&mut self.inner, entry.root)?;
        let root = krummi_tree::create(&mut self.inner)?;
        let emptied = Entry {
            root,
            count: 0,
            ..entry
        };
        self.handle(name, &emptied)?;
        let state = self.trees.get_mut().get_mut(entry.id).expect("added above");
        state.root = root;
        state.count = 0;
        state.dirty = true;
        Ok(emptied)
    }

    /// Deletes the tree `name` of `entry`, ending its identity: its handles fail from now on (A6, A8).
    fn delete(&mut self, name: &str, entry: Entry) -> Result<()> {
        let entry = self.current(entry);
        krummi_tree::free_all(&mut self.inner, entry.root)?;
        catalog::remove(&mut self.inner, name)?;
        if let Some(state) = self.trees.get_mut().get_mut(entry.id) {
            state.deleted = true;
            state.dirty = false;
        }
        Ok(())
    }

    /// Writes into the catalog the root and the count of every tree whose key operations changed them, and
    /// returns the transaction to commit. A transaction that already failed writes nothing: its commit reports
    /// that failure (T11). A failure here aborts the transaction.
    fn finish(mut self) -> Result<krummi_tx::WriteTransaction<'s>> {
        if !self.inner.has_failed() {
            let trees = self.trees.get_mut();
            for state in trees.0.iter().filter(|s| s.dirty && !s.deleted) {
                let entry = Entry {
                    id: state.id,
                    kind: state.kind,
                    root: state.root,
                    count: state.count,
                };
                catalog::put(&mut self.inner, &state.name, &entry)?;
            }
        }
        Ok(self.inner)
    }
}

impl WriteTransaction<'_> {
    /// Commits the transaction durably: it returns once its changes survive a crash of the process or the
    /// system and a power cut, and they become visible to every transaction opened after this returns. The next
    /// write transaction may start, and see them, while this one waits for the disk; commits that wait at the same
    /// time share one write of the journal. If one of its operations failed, the commit fails with the class of
    /// the first failure and applies nothing; once it waits for the disk it can only fail with [`ErrorKind::Io`],
    /// and then the store refuses writes and the commit may still be there, entire, after the close or the next
    /// opening: its outcome is unknown, never partial.
    /// A transaction without changes writes nothing, and waits until the commits it saw are durable.
    pub fn commit(self) -> Result<()> {
        self.finish()?.commit()
    }

    /// Commits the transaction in deferred mode: it returns without waiting for the disk, and the journal makes it
    /// durable as soon as it would a durable commit made at the same point, usually within one write of the disk.
    /// The next write transaction sees its changes at once; read transactions see them once they are durable,
    /// also on this thread. Until then a crash of the process or of the system, or a power cut, may lose them,
    /// always together with every later commit and never in part. It may wait, keeping the write turn, while the
    /// journal's queue is full.
    pub fn commit_deferred(self) -> Result<()> {
        self.finish()?.commit_deferred()
    }

    /// Aborts the transaction: none of its changes apply, and the next request waiting gets the turn.
    pub fn abort(self) {
        self.inner.abort();
    }
}

/// A serial number for a new transaction, unique in the process, so that a tree handle can tell whether it is
/// used in the transaction that opened it, also across stores.
fn next_serial() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The entry of the tree `name` that `tx` sees, failing with [`ErrorKind::DifferentTreeType`] when it is not of
/// kind `kind` (B3). It reads only the catalog.
fn find<S: PageSource>(
    folder: Folder<'_>,
    tx: &S,
    name: &str,
    kind: TreeKind,
) -> Result<Option<Entry>> {
    match catalog::lookup(tx, name)? {
        Some(entry) if entry.kind != kind => Err(Error::DifferentTreeType {
            folder: folder.owned(),
            name: name.to_owned(),
            found: entry.kind,
            requested: kind,
        }),
        entry => Ok(entry),
    }
}

/// The store's folder, `None` for a temporary store: the data that the errors of the trees carry (T18, B8). It
/// builds them.
#[derive(Clone, Copy)]
struct Folder<'s>(Option<&'s Path>);

impl Folder<'_> {
    fn owned(self) -> Option<PathBuf> {
        self.0.map(Path::to_path_buf)
    }

    /// Fails with [`ErrorKind::InvalidSize`] unless `len` is within `min..=max` (B4–B8).
    fn check_size(self, what: SizeSubject, len: usize, min: usize, max: usize) -> Result<()> {
        if (min..=max).contains(&len) {
            return Ok(());
        }
        Err(Error::InvalidSize {
            folder: self.owned(),
            what,
            received: len as u64,
            min: min as u64,
            max: max as u64,
        })
    }

    fn check_name(self, name: &str) -> Result<()> {
        self.check_size(SizeSubject::TreeName, name.len(), 1, MAX_NAME)
    }

    fn check_key(self, key: &[u8]) -> Result<()> {
        self.check_size(SizeSubject::Key, key.len(), 1, krummi_tree::MAX_KEY)
    }

    fn check_value(self, value: &[u8]) -> Result<()> {
        self.check_size(SizeSubject::Value, value.len(), 0, krummi_tree::MAX_VALUE)
    }

    /// The interval of `bounds` and `prefix`, after checking their sizes in the order lower bound, upper bound,
    /// prefix (R5).
    fn interval<'a>(
        self,
        prefix: &'a [u8],
        bounds: &impl RangeBounds<&'a [u8]>,
    ) -> Result<Interval<'a>> {
        let (lower, upper) = (
            bounds.start_bound().map(|k| *k),
            bounds.end_bound().map(|k| *k),
        );
        for bound in [lower, upper] {
            if let Bound::Included(key) | Bound::Excluded(key) = bound {
                self.check_key(key)?;
            }
        }
        self.check_size(SizeSubject::Prefix, prefix.len(), 0, krummi_tree::MAX_KEY)?;
        Ok(Interval {
            prefix,
            lower,
            upper,
        })
    }

    fn not_found(self, name: &str) -> Error {
        Error::TreeNotFound {
            folder: self.owned(),
            name: name.to_owned(),
        }
    }

    fn already_exists(self, name: &str) -> Error {
        Error::TreeAlreadyExists {
            folder: self.owned(),
            name: name.to_owned(),
        }
    }

    /// A handle used in a transaction other than the one that opened it (the B1 exception).
    fn foreign_handle(self) -> Error {
        Error::Misuse {
            folder: self.owned(),
            reason: "the tree was opened in another transaction",
        }
    }
}

fn engine_options(options: &StoreOptions) -> krummi_tx::OpenOptions {
    krummi_tx::OpenOptions {
        max_size: options.max_size,
    }
}
