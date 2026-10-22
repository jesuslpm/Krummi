//! MuninnDB: an embedded transactional storage engine inspired by Voron.
//!
//! Module 1 (the Pager) lets an application create, open, inspect and close stores. Storing data arrives with
//! the transactions of module 2.

use std::path::Path;

pub use muninndb_pager::{Error, ErrorKind, Info as StoreInfo, Result};

/// Options for opening a store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoreOptions {
    /// Maximum size of the store in bytes while it stays open: at least 1 MiB, rounded down to the page size.
    pub max_size: Option<u64>,
}

/// An open store. Only one can be open per folder at a time, in any process.
pub struct Store {
    pager: muninndb_pager::Pager,
}

impl Store {
    /// Opens the store in `folder`. A missing folder is created with its parents, and an empty folder gets a
    /// new store.
    pub fn open(folder: impl AsRef<Path>, options: &StoreOptions) -> Result<Store> {
        let pager = muninndb_pager::Pager::open(folder, &pager_options(options))?;
        Ok(Store { pager })
    }

    /// Opens a store that lives only in memory and keeps nothing when it closes.
    pub fn open_temporary(options: &StoreOptions) -> Result<Store> {
        let pager = muninndb_pager::Pager::open_temporary(&pager_options(options))?;
        Ok(Store { pager })
    }

    /// Information about the store.
    pub fn info(&self) -> StoreInfo {
        self.pager.info()
    }

    /// Closes the store and releases its lock. Dropping a `Store` also releases the lock, but does not report
    /// a failed flush.
    pub fn close(self) -> Result<()> {
        self.pager.close()
    }
}

fn pager_options(options: &StoreOptions) -> muninndb_pager::OpenOptions {
    muninndb_pager::OpenOptions {
        max_size: options.max_size,
    }
}
