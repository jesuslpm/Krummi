//! Ranges of the primary key of a table (E12 of the tables with schema PRD; document 3 of the table design,
//! *Composite keys and prefix scans*; document 2, *Scans* and *Range delete*): scans forward and backward, and range
//! removal.
//!
//! A range gives equal values for the first k columns of the primary key (the prefix, from none to all) and, when k
//! is less than the number of key columns, an optional lower and upper bound on the next column, each included or
//! excluded, as any [`RangeBounds`] of [`Value`]s. Its rows come out in the order of N5 from the front of the scan and
//! in reverse from its back; an empty or inverted range gives no rows and no error. The values follow the rules of
//! their columns (a null breaks them, NaN does not); a prefix with more values than the key, or a bound when the
//! prefix gives every column, is a malformed range; and a prefix or bound that does not fit in the 2,048 bytes of a
//! key fails with its size, also when the range is empty. A scan resolves the layout of each older row version once
//! (document 1, *Cost of mixed versions*).
//!
//! ```
//! use krummi::table::value::{ColumnType, Value};
//! use krummi::{Column, Schema, Store, StoreOptions, TableMode};
//!
//! let store = Store::open_temporary(&StoreOptions::default())?;
//! let mut write = store.write()?;
//! let schema = Schema::new(
//!     vec![
//!         Column::new("tenant", ColumnType::I32).not_null(),
//!         Column::new("at", ColumnType::I64).not_null(),
//!     ],
//!     &["tenant", "at"],
//! );
//! let table = write.open_table_with("t", TableMode::CreateNew(&schema))?;
//! for (tenant, at) in [(1, 10), (1, 20), (1, 30), (2, 10)] {
//!     write.insert_row(&table, &[("tenant", Value::I32(tenant)), ("at", Value::I64(at))])?;
//! }
//! // The rows of tenant 1 from 20 on, backward.
//! let at: Vec<Value<'static>> = write
//!     .range_rows(&table, &[Value::I32(1)], Value::I64(20)..)?
//!     .rev()
//!     .map(|row| row.map(|row| row.get("at").unwrap().clone().into_owned()))
//!     .collect::<krummi::Result<_>>()?;
//! assert_eq!(at, [Value::I64(30), Value::I64(20)]);
//! assert_eq!(write.delete_range(&table, &[Value::I32(1)], ..)?, 3);
//! assert_eq!(write.row_count(&table)?, 1);
//! write.commit()?;
//! # Ok::<(), krummi::Error>(())
//! ```
//!
//! A range delete takes `&mut self`, so it ends the references read before (E16):
//!
//! ```compile_fail
//! # use krummi::table::value::{ColumnType, Value};
//! # use krummi::{Column, Schema, Store, StoreOptions, TableMode};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
//! # let table = write.open_table_with("t", TableMode::CreateNew(&schema)).unwrap();
//! let rows = write.range_rows(&table, &[], ..).unwrap();
//! write.delete_range(&table, &[], ..).unwrap();
//! drop(rows);
//! ```

use std::ops::RangeBounds;
use std::sync::Arc;

use krummi_pager::Result;
use krummi_tree::{Interval, PageSource};

use super::catalog::{ReadTable, WriteTable};
use super::key::KeyRange;
use super::rows::{At, Reader, RowCount, TableRow, Versions};
use super::value::Value;
use crate::{ReadTransaction, WriteTransaction};

/// A scan of the rows of a range of a table's primary key.
struct Scan<'t, S: PageSource + ?Sized> {
    tx: &'t S,
    entries: krummi_tree::Range<'t, S>,
    reader: Arc<Reader>,
    /// The name of the table's catalog entry and the roots of its trees.
    name: String,
    rows_root: u64,
    history: u64,
    /// What the scan resolves once for all its rows.
    versions: Versions<'t>,
    failed: bool,
}

impl<'t, S: PageSource + ?Sized> Scan<'t, S> {
    fn new(tx: &'t S, reader: Arc<Reader>, table: At<'_>, range: &KeyRange) -> Result<Self> {
        Ok(Scan {
            tx,
            entries: krummi_tree::Range::new(tx, table.rows_root, interval(range))?,
            reader,
            name: krummi_tree::copy_str(tx, table.name)?,
            rows_root: table.rows_root,
            history: table.history,
            versions: Versions::default(),
            failed: false,
        })
    }

    /// The row of the entry that `step` takes from an end of the scan; after a failure, nothing more from either
    /// end.
    fn row(
        &mut self,
        step: impl FnOnce(&mut krummi_tree::Range<'t, S>) -> Option<Result<(&'t [u8], &'t [u8])>>,
    ) -> Option<Result<TableRow<'t>>> {
        if self.failed {
            return None;
        }
        let row = step(&mut self.entries)?.and_then(|(key, bytes)| {
            let table = At {
                name: &self.name,
                rows_root: self.rows_root,
                history: self.history,
            };
            self.reader
                .decode(self.tx, table, &mut self.versions, key, bytes)
        });
        self.failed = row.is_err();
        Some(row)
    }
}

/// The tree's interval of the byte bounds of `range`, which fold the prefix in.
fn interval(range: &KeyRange) -> Interval<'_> {
    let (lower, upper) = range.bounds();
    Interval {
        prefix: &[],
        lower,
        upper,
    }
}

/// A scan of a range of a table's primary key in a read transaction, from [`ReadTransaction::range_rows`]: a
/// double-ended iterator of rows in the order of N5 (E12). It can be sent to another thread, like its transaction.
/// After a failure it returns nothing more from either end.
pub struct ReadRows<'t>(Scan<'t, krummi_tx::ReadTransaction<'t>>);

/// A scan of a range of a table's primary key in a write transaction, from [`WriteTransaction::range_rows`], like
/// [`ReadRows`]. It stays on its transaction's thread and ends at the transaction's next operation that may modify
/// it.
pub struct WriteRows<'t>(Scan<'t, krummi_tx::WriteTransaction<'t>>);

macro_rules! rows_iterator {
    ($scan:ident) => {
        impl<'t> Iterator for $scan<'t> {
            type Item = Result<TableRow<'t>>;

            fn next(&mut self) -> Option<Self::Item> {
                self.0.row(|entries| entries.next())
            }
        }

        impl DoubleEndedIterator for $scan<'_> {
            fn next_back(&mut self) -> Option<Self::Item> {
                self.0.row(|entries| entries.next_back())
            }
        }

        impl std::iter::FusedIterator for $scan<'_> {}
    };
}

rows_iterator!(ReadRows);
rows_iterator!(WriteRows);

impl ReadTransaction<'_> {
    /// A scan of the rows of `table` whose primary key starts with the values `prefix` and whose next key column is
    /// within `bounds`, in key order, as the snapshot has them (E12; see the module's documentation). Fails, in this
    /// order, with [`ErrorKind::Misuse`](crate::ErrorKind::Misuse) for a table opened in another transaction; with
    /// [`ErrorKind::InvalidValue`](crate::ErrorKind::InvalidValue) for a value that breaks its column's rules, with
    /// the column, and then for a malformed range ([`ValueRule::InvalidRange`](crate::ValueRule::InvalidRange)); with
    /// [`ErrorKind::InvalidSize`](crate::ErrorKind::InvalidSize) for a prefix or bound of more than 2,048 bytes in its
    /// stored form, even when the range is empty; and, while scanning, with damage of a page it needs (E19).
    pub fn range_rows<'v>(
        &self,
        table: &ReadTable<'_>,
        prefix: &[Value<'v>],
        bounds: impl RangeBounds<Value<'v>>,
    ) -> Result<ReadRows<'_>> {
        self.check_table(table)?;
        let range = table.reader.key_range(
            self.folder,
            &table.name,
            prefix,
            bounds.start_bound(),
            bounds.end_bound(),
        )?;
        Ok(ReadRows(Scan::new(
            &self.inner,
            Arc::clone(&table.reader),
            table.at(),
            &range,
        )?))
    }
}

impl WriteTransaction<'_> {
    /// A scan of a range of `table` with this transaction's changes, with the failures of
    /// [`ReadTransaction::range_rows`]; a handle of a deleted table fails with
    /// [`ErrorKind::TreeNotFound`](crate::ErrorKind::TreeNotFound). It and its rows live until the next operation
    /// that may modify the transaction. A failure while scanning is the transaction's (T11).
    pub fn range_rows<'v>(
        &self,
        table: &WriteTable<'_>,
        prefix: &[Value<'v>],
        bounds: impl RangeBounds<Value<'v>>,
    ) -> Result<WriteRows<'_>> {
        let state = self.live_table(table)?;
        let range = state.reader.key_range(
            self.folder,
            &state.name,
            prefix,
            bounds.start_bound(),
            bounds.end_bound(),
        )?;
        Ok(WriteRows(Scan::new(
            &self.inner,
            Arc::clone(&state.reader),
            state.at(),
            &range,
        )?))
    }

    /// Deletes the rows of a range of `table`, given as for [`range_rows`](Self::range_rows) and with its failures,
    /// and returns how many it deleted (E12). A range without rows changes nothing (E16); the pages that the deleted
    /// rows no longer use are freed.
    pub fn delete_range<'v>(
        &mut self,
        table: &WriteTable<'_>,
        prefix: &[Value<'v>],
        bounds: impl RangeBounds<Value<'v>>,
    ) -> Result<u64> {
        let (rows_root, reader) = self.live_rows(table)?;
        let range = reader.key_range(
            self.folder,
            &self.table_name(table),
            prefix,
            bounds.start_bound(),
            bounds.end_bound(),
        )?;
        let update = krummi_tree::remove_range(&mut self.inner, rows_root, interval(&range))?;
        self.apply_rows(table, update.root, RowCount::Removed(update.removed))?;
        Ok(update.removed)
    }
}

#[cfg(test)]
mod tests {
    use crate::table::value::{ColumnType, Value};
    use crate::{Column, ErrorKind, Schema, Store, StoreOptions, TableMode};

    /// A temporary store with the table `t` (`id i64`, its primary key) of the rows 0 to 2, committed.
    fn store() -> Store {
        let store = Store::open_temporary(&StoreOptions::default()).unwrap();
        let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .unwrap();
        for id in 0..3 {
            write.insert_row(&table, &[("id", Value::I64(id))]).unwrap();
        }
        write.commit().unwrap();
        store
    }

    /// A row count in the catalog lower than the rows the table has (a damaged count) makes a delete fail with
    /// damage and the transaction's failure, instead of wrapping the count.
    #[test]
    fn deleting_a_row_under_a_damaged_row_count_is_damage() {
        let store = store();
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.tables.get_mut().get_mut(table.id).unwrap().count = 0;
        let error = write.delete_row(&table, &[Value::I64(0)]).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Damaged);
        assert!(write.inner.has_failed());
    }

    #[test]
    fn deleting_a_range_under_a_damaged_row_count_is_damage() {
        let store = store();
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.tables.get_mut().get_mut(table.id).unwrap().count = 2;
        let error = write.delete_range(&table, &[], ..).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Damaged);
        assert!(write.inner.has_failed());
    }
}
