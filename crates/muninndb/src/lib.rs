//! MuninnDB: an embedded transactional storage engine inspired by Voron.
//!
//! Module 1 (the Pager) lets an application create, open, inspect and close stores. Module 2 adds transactions:
//! read transactions that see a snapshot of the store, from any number of threads, and one write transaction at a
//! time, which the others wait for in order of arrival. Storing data in them arrives with the data structures of
//! the next modules.
//!
//! ```
//! use muninndb::{Store, StoreOptions};
//!
//! let store = Store::open_temporary(&StoreOptions::default())?;
//! let read = store.read()?;      // sees the store as it is now
//! let write = store.write()?;    // the one write transaction
//! write.commit()?;               // visible to reads opened from now on
//! drop(read);
//! store.close()?;
//! # Ok::<(), muninndb::Error>(())
//! ```

use std::path::Path;
use std::time::Duration;

pub use muninndb_pager::{Error, ErrorKind, Info as StoreInfo, Result};

/// Options for opening a store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreOptions {
    /// Maximum size of the store in bytes while it stays open: at least 1 MiB, rounded down to the page size. It
    /// counts the data file and the page versions the store keeps in memory for open reads.
    pub max_size: Option<u64>,
}

/// An open store. Only one can be open per folder at a time, in any process. It can be used from several threads
/// at once; it cannot be closed while a transaction is open.
pub struct Store {
    engine: muninndb_tx::Engine,
}

impl Store {
    /// Opens the store in `folder`. A missing folder is created with its parents, and an empty folder gets a
    /// new store.
    pub fn open(folder: impl AsRef<Path>, options: &StoreOptions) -> Result<Store> {
        let engine = muninndb_tx::Engine::open(folder, &engine_options(options))?;
        Ok(Store { engine })
    }

    /// Opens a store that lives only in memory and keeps nothing when it closes.
    pub fn open_temporary(options: &StoreOptions) -> Result<Store> {
        let engine = muninndb_tx::Engine::open_temporary(&engine_options(options))?;
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

    /// Closes the store: writes what was committed and releases its lock, even if writing fails. Dropping a
    /// `Store` writes it too, but does not report a failure.
    pub fn close(self) -> Result<()> {
        self.engine.close()
    }

    fn open_write(&self, limit: Option<Duration>) -> Result<WriteTransaction<'_>> {
        Ok(WriteTransaction {
            inner: self.engine.write(limit)?,
        })
    }
}

/// A read transaction: a snapshot of the store as it was when it opened. It can be sent to other threads and
/// shared between them, which see the same snapshot. It ends when it is dropped.
pub struct ReadTransaction<'s> {
    _inner: muninndb_tx::ReadTransaction<'s>,
}

/// The write transaction of a store. It stays on the thread that opened it. Dropping it without committing
/// aborts it.
pub struct WriteTransaction<'s> {
    inner: muninndb_tx::WriteTransaction<'s>,
}

impl WriteTransaction<'_> {
    /// Commits the transaction: its changes become visible to every transaction opened after this returns, and
    /// reach the store's file at the latest when the store closes. If one of its operations failed, the commit
    /// fails with the class of the first failure and applies nothing.
    pub fn commit(self) -> Result<()> {
        self.inner.commit()
    }

    /// Aborts the transaction: none of its changes apply, and the next request waiting gets the turn.
    pub fn abort(self) {
        self.inner.abort();
    }
}

fn engine_options(options: &StoreOptions) -> muninndb_tx::OpenOptions {
    muninndb_tx::OpenOptions {
        max_size: options.max_size,
    }
}
