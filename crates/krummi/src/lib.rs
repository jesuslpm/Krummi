//! Krummi: an embedded transactional storage engine inspired by Voron.
//!
//! Module 1 (the Pager) lets an application create, open, inspect and close stores. Module 2 adds transactions:
//! read transactions that see a snapshot of the store, from any number of threads, and one write transaction at a
//! time, which the others wait for in order of arrival. Module 3 adds the journal: a commit that returns survives
//! a crash of the process or the system and a power cut, and opening a store after a crash recovers it without
//! any action of the application. Module 4 adds free space: the pages that writes free are reused before the store
//! grows, and [`StoreInfo`] reports how many pages are free. Storing data in transactions arrives with the data
//! structures of the next modules.
//!
//! ```
//! use krummi::{Store, StoreOptions};
//!
//! let store = Store::open_temporary(&StoreOptions::default())?;
//! let read = store.read()?;      // sees the store as it is now
//! let write = store.write()?;    // the one write transaction
//! write.commit()?;               // visible to reads opened from now on
//! drop(read);
//! store.close()?;
//! # Ok::<(), krummi::Error>(())
//! ```

use std::marker::PhantomData;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub use krummi_pager::{Error, ErrorKind, Info as StoreInfo, Result, SizeSubject, TreeKind};

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
            _inner: self.engine.read()?,
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
            serial: next_serial(),
        })
    }
}

/// A read transaction: a snapshot of the store as it was when it opened. It can be sent to other threads and
/// shared between them, which see the same snapshot. It ends when it is dropped.
pub struct ReadTransaction<'s> {
    _inner: krummi_tx::ReadTransaction<'s>,
    #[expect(dead_code, reason = "T3 compares serials (S0)")]
    serial: u64,
}

/// The write transaction of a store. It stays on the thread that opened it. Dropping it without committing
/// aborts it.
pub struct WriteTransaction<'s> {
    inner: krummi_tx::WriteTransaction<'s>,
    #[expect(dead_code, reason = "T3 compares serials (S0)")]
    serial: u64,
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

/// A tree opened in a read transaction: it sees the transaction's snapshot. It can be sent to other threads and
/// shared between them, like the transaction, and cannot outlive its store. Using it in another transaction
/// fails with [`ErrorKind::Misuse`] without effect.
pub struct ReadTree<'s> {
    #[expect(dead_code, reason = "T3 compares serials (S0)")]
    serial: u64,
    _store: PhantomData<&'s Store>,
}

/// A tree opened in a write transaction. Every handle of the same tree in that transaction sees the same state,
/// with the changes made through any of them. It stays on the thread that opened it, like the transaction, and
/// cannot outlive its store. Using it in another transaction fails with [`ErrorKind::Misuse`] without effect,
/// and using it after its tree was deleted fails with [`ErrorKind::TreeNotFound`].
pub struct WriteTree<'s> {
    #[expect(dead_code, reason = "T3 compares serials (S0)")]
    serial: u64,
    /// The raw pointer keeps the handle on its thread: it is neither `Send` nor `Sync`.
    _store: PhantomData<(&'s Store, *const ())>,
}

impl<'s> ReadTransaction<'s> {
    /// Opens the tree `name` of kind `kind`. Fails with [`ErrorKind::TreeNotFound`] if the snapshot does not have
    /// it, and with [`ErrorKind::InvalidSize`] if the name is empty or longer than 1,024 bytes.
    pub fn open_tree(&self, name: &str, kind: TreeKind) -> Result<ReadTree<'s>> {
        let _ = (name, kind);
        todo!("T3")
    }

    /// Whether the snapshot has a tree named `name`.
    pub fn tree_exists(&self, name: &str) -> Result<bool> {
        let _ = name;
        todo!("T3")
    }

    /// The name and kind of every tree in the snapshot, in byte order of the names. The list is a copy.
    pub fn list_trees(&self) -> Result<Vec<(String, TreeKind)>> {
        todo!("T3")
    }

    /// The value of `key` in `tree`, or `None` if the key is not there. The reference points into the engine's
    /// memory, not to a copy, and lives as long as the transaction.
    pub fn get(&self, tree: &ReadTree<'_>, key: &[u8]) -> Result<Option<&[u8]>> {
        let _ = (tree, key);
        todo!("T3")
    }

    /// The number of entries of `tree` in the snapshot, counted without walking the tree.
    pub fn len(&self, tree: &ReadTree<'_>) -> Result<u64> {
        let _ = tree;
        todo!("T3")
    }
}

/// The operations on trees of a write transaction. The ones that take `&self` do not modify the transaction and
/// keep the references read before; the ones that take `&mut self` may modify it, so the compiler ends those
/// references, even when the operation fails or changes nothing.
impl<'s> WriteTransaction<'s> {
    /// Opens the tree `name` of kind `kind`. Fails with [`ErrorKind::TreeNotFound`] if it does not exist.
    pub fn open_tree(&self, name: &str, kind: TreeKind) -> Result<WriteTree<'s>> {
        let _ = (name, kind);
        todo!("T3")
    }

    /// Opens the tree `name` of kind `kind` in `mode`.
    pub fn open_tree_with(
        &mut self,
        name: &str,
        kind: TreeKind,
        mode: OpenMode,
    ) -> Result<WriteTree<'s>> {
        let _ = (name, kind, mode);
        todo!("T3")
    }

    /// Whether this transaction sees a tree named `name`, with its own changes.
    pub fn tree_exists(&self, name: &str) -> Result<bool> {
        let _ = name;
        todo!("T3")
    }

    /// The name and kind of every tree this transaction sees, with its own changes, in byte order of the names.
    /// The list is a copy.
    pub fn list_trees(&self) -> Result<Vec<(String, TreeKind)>> {
        todo!("T3")
    }

    /// Deletes the tree `name` with all its entries and frees its pages; a missing tree is not an error. The
    /// tree's handles then fail with [`ErrorKind::TreeNotFound`].
    pub fn delete_tree(&mut self, name: &str) -> Result<()> {
        let _ = name;
        todo!("T3")
    }

    /// Renames the tree `from` to `to`; it keeps its kind, its entries and its handles. Fails with
    /// [`ErrorKind::TreeNotFound`] if `from` does not exist, and with [`ErrorKind::TreeAlreadyExists`] if `to`
    /// exists, unless `overwrite` is set: then `to` is deleted first. Renaming a tree to its own name does
    /// nothing.
    pub fn rename_tree(&mut self, from: &str, to: &str, overwrite: bool) -> Result<()> {
        let _ = (from, to, overwrite);
        todo!("T3")
    }

    /// The value of `key` in `tree`, with this transaction's changes, or `None` if the key is not there. The
    /// reference lives until the next operation that may modify the transaction.
    pub fn get(&self, tree: &WriteTree<'_>, key: &[u8]) -> Result<Option<&[u8]>> {
        let _ = (tree, key);
        todo!("T3")
    }

    /// The number of entries of `tree`, with this transaction's changes, counted without walking the tree.
    pub fn len(&self, tree: &WriteTree<'_>) -> Result<u64> {
        let _ = tree;
        todo!("T3")
    }

    /// Stores `value` under `key` in `tree`, replacing the value if the key is there.
    pub fn set(&mut self, tree: &WriteTree<'_>, key: &[u8], value: &[u8]) -> Result<()> {
        let _ = (tree, key, value);
        todo!("T3")
    }

    /// Stores `value` under `key` in `tree` only if the key is not there, and returns whether it did. The sizes
    /// are checked first, also when the key is there.
    pub fn try_add(&mut self, tree: &WriteTree<'_>, key: &[u8], value: &[u8]) -> Result<bool> {
        let _ = (tree, key, value);
        todo!("T3")
    }

    /// Removes `key` from `tree`, and returns whether it was there.
    pub fn remove(&mut self, tree: &WriteTree<'_>, key: &[u8]) -> Result<bool> {
        let _ = (tree, key);
        todo!("T3")
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
        self.inner.commit()
    }

    /// Commits the transaction in deferred mode: it returns without waiting for the disk, and the journal makes it
    /// durable as soon as it would a durable commit made at the same point, usually within one write of the disk.
    /// The next write transaction sees its changes at once; read transactions see them once they are durable,
    /// also on this thread. Until then a crash of the process or of the system, or a power cut, may lose them,
    /// always together with every later commit and never in part. It may wait, keeping the write turn, while the
    /// journal's queue is full.
    pub fn commit_deferred(self) -> Result<()> {
        self.inner.commit_deferred()
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

fn engine_options(options: &StoreOptions) -> krummi_tx::OpenOptions {
    krummi_tx::OpenOptions {
        max_size: options.max_size,
    }
}
