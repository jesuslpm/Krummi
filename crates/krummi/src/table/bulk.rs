//! Bulk load of a table (E13 of the tables with schema PRD; document 2 of the table design, *Bulk load*): a write
//! transaction of its own that reads each row of a fallible source once and inserts it with the two phases of
//! [`WriteTransaction::insert_row`](crate::WriteTransaction::insert_row), so a duplicate inside the load, or with a
//! row the table had, is found by the same probe as any duplicate. Any failure aborts that transaction, so the load
//! fails whole; the end of the source is the only way to commit.
//!
//! ```
//! use std::convert::Infallible;
//! use krummi::table::value::{ColumnType, Value};
//! use krummi::{Column, Schema, Store, StoreOptions, TableMode};
//!
//! let store = Store::open_temporary(&StoreOptions::default())?;
//! let mut write = store.write()?;
//! let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
//! write.open_table_with("t", TableMode::CreateNew(&schema))?;
//! write.commit()?;
//! let rows = (0..1000).map(|id| Ok::<_, Infallible>([("id", Value::I64(id))]));
//! assert_eq!(store.bulk_load("t", rows).unwrap(), 1000);
//! # Ok::<(), krummi::Error>(())
//! ```

use std::error;
use std::fmt;
use std::time::Duration;

use krummi_pager::Error;

use super::value::Value;
use crate::Store;

/// The failure of a bulk load ([`Store::bulk_load`]). Either way the load was aborted whole: the table, its row count
/// and the sequences are as before, and the write turn has gone to the next writer (E13).
#[derive(Debug)]
pub enum BulkLoadError<E> {
    /// The load failed with this error of the engine: misuse or a timeout getting the write turn, the catalog (the
    /// table does not exist, or the name is of another class), a row that breaks the rules of insert (the first that
    /// does), or the failures of any write and of its commit.
    Store(Error),
    /// The source of the rows failed with this error, which is also how the application cancels a load.
    Source(E),
}

impl<E> From<Error> for BulkLoadError<E> {
    fn from(error: Error) -> Self {
        BulkLoadError::Store(error)
    }
}

impl<E> fmt::Display for BulkLoadError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BulkLoadError::Store(error) => fmt::Display::fmt(error, f),
            BulkLoadError::Source(_) => f.write_str("the source of the bulk load failed"),
        }
    }
}

impl<E: error::Error + 'static> error::Error for BulkLoadError<E> {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            BulkLoadError::Store(error) => error.source(),
            BulkLoadError::Source(error) => Some(error),
        }
    }
}

impl Store {
    /// Loads the rows of `rows` into the table `table`, as one write transaction of its own, and returns how many
    /// it loaded (E13). Each row is given as for
    /// [`WriteTransaction::insert_row`](crate::WriteTransaction::insert_row), with the same rules, in any order; the
    /// primary key order is faster. It waits for the write turn without limit, then fails, in this order, with
    /// [`BulkLoadError::Store`] holding [`ErrorKind::Misuse`](crate::ErrorKind::Misuse) at once if this thread
    /// already holds the write turn (a read open on it does not prevent the load), then with the failures of
    /// [`WriteTransaction::open_table`](crate::WriteTransaction::open_table) (the name's size, tree not found,
    /// different tree type), then with the first row that breaks the rules of insert, or with
    /// [`BulkLoadError::Source`] at the first error of the source. Any failure, a panic in the source or in the load
    /// included, aborts the load whole and gives the write turn to the next writer. The end of the source commits,
    /// with the rules and failures of [`WriteTransaction::commit`](crate::WriteTransaction::commit); an empty source
    /// changes nothing.
    pub fn bulk_load<'n, 'v, R, E>(
        &self,
        table: &str,
        rows: impl IntoIterator<Item = Result<R, E>>,
    ) -> Result<u64, BulkLoadError<E>>
    where
        R: AsRef<[(&'n str, Value<'v>)]>,
    {
        self.load(table, rows, None)
    }

    /// Like [`Store::bulk_load`], but waits for the write turn at most `limit`, with the rules of
    /// [`Store::write_with_timeout`]: when it expires the load fails with
    /// [`ErrorKind::Timeout`](crate::ErrorKind::Timeout) before looking at the catalog or the source (T4, E17).
    pub fn bulk_load_with_timeout<'n, 'v, R, E>(
        &self,
        table: &str,
        rows: impl IntoIterator<Item = Result<R, E>>,
        limit: Duration,
    ) -> Result<u64, BulkLoadError<E>>
    where
        R: AsRef<[(&'n str, Value<'v>)]>,
    {
        self.load(table, rows, Some(limit))
    }

    /// The load: an early return, or an unwinding panic, drops the write transaction, which aborts it.
    fn load<'n, 'v, R, E>(
        &self,
        table: &str,
        rows: impl IntoIterator<Item = Result<R, E>>,
        limit: Option<Duration>,
    ) -> Result<u64, BulkLoadError<E>>
    where
        R: AsRef<[(&'n str, Value<'v>)]>,
    {
        let mut write = match limit {
            Some(limit) => self.write_with_timeout(limit)?,
            None => self.write()?,
        };
        let handle = write.open_table(table)?;
        let mut loaded = 0;
        for row in rows {
            let row = row.map_err(BulkLoadError::Source)?;
            write.insert_row(&handle, row.as_ref())?;
            loaded += 1;
        }
        if loaded == 0 {
            write.abort();
        } else {
            write.commit()?;
        }
        Ok(loaded)
    }
}
