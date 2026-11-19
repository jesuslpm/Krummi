//! Building, saving and reading the statistics of a table's columns (ST1, ST7, ST8, ST16 of the statistics PRD).
//!
//! A build reads the rows of one snapshot and gives the statistics of the table's columns that are not vectors, with
//! the snapshot's row count and modification counter. Saving keeps, per column, the statistics of the latest
//! snapshot: it drops a build of a column that no longer exists, of a table that is no longer the same or was emptied
//! after the build's snapshot, and one older than the statistics already saved (ST6, N8).

use std::time::Duration;

use super::catalog::{ReadTable, WriteTable};
use super::rows::TableRow;
use super::schema::{SchemaColumn, TableSchema, TableStats};
use super::stats::{self, ColumnStats, Gather, column_key, element_column, has_stats};
use super::value::Value;
use crate::{
    Error, Folder, ReadTransaction, Result, SchemaDifference, SchemaRule, Store, WriteTransaction,
};

/// The statistics of a column as a transaction sees them (ST16), in a copy of the application's.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnStatistics {
    /// The table's rows in the snapshot the statistics were built from.
    pub origin_rows: u64,
    /// The rows read to build them: all of them, up to 30,000 (ST1).
    pub rows_read: u64,
    /// The table's modification counter in that snapshot (ST4).
    pub origin_counter: u64,
    /// Whether they are stale for the transaction (ST5).
    pub stale: bool,
    pub null_fraction: f64,
    /// The fraction of large values, `string` or `bytes` of more than 2,048 bytes; zero in other columns.
    pub large_fraction: f64,
    /// The estimated distinct values that are not null.
    pub distinct: f64,
    /// The most frequent values and their fractions, from the most frequent; equal fractions in the column's order.
    pub frequent: Vec<(Value<'static>, f64)>,
    /// The histogram of the other values that are not null, in the column's order.
    pub histogram: Vec<HistogramBucket>,
}

/// A bucket of a [`ColumnStatistics`] histogram.
#[derive(Debug, Clone, PartialEq)]
pub struct HistogramBucket {
    /// The greatest value of the bucket, or a prefix of it when `shortened`.
    pub upper: Value<'static>,
    pub fraction: f64,
    pub shortened: bool,
}

/// The statistics a build gave: of the table `table` and its columns, by identity.
pub(crate) struct Built {
    pub table: u64,
    pub columns: Vec<(u64, ColumnStats)>,
}

/// The column `name` of `schema` that statistics are asked for: a column name of 1 to 1,024 bytes, of the table, not a
/// vector (ST12).
pub(crate) fn stats_column<'c>(
    folder: Folder<'_>,
    table: &str,
    schema: &'c TableSchema,
    name: &str,
) -> Result<&'c SchemaColumn> {
    folder.check_column_name(name)?;
    let Some(column) = schema.columns().iter().find(|c| c.name == name) else {
        return Err(Error::DifferentSchema {
            folder: folder.owned(),
            table: table.to_owned(),
            column: Some(name.to_owned()),
            difference: SchemaDifference::MissingColumn,
            expected: "a column".into(),
            found: "no column".into(),
        });
    };
    if !has_stats(&column.ty) {
        return Err(Error::InvalidSchema {
            folder: folder.owned(),
            table: table.to_owned(),
            column: Some(name.to_owned()),
            rule: SchemaRule::VectorColumn,
            index: None,
        });
    }
    Ok(column)
}

/// Builds the statistics of the columns of `schema` (only `only` when given) from `rows`, the rows of a snapshot that
/// has `count` rows and the modification counter `counter`.
pub(crate) fn build<'r>(
    table: u64,
    schema: &TableSchema,
    only: Option<u64>,
    count: u64,
    counter: u64,
    rows: impl Iterator<Item = Result<TableRow<'r>>>,
) -> Result<Built> {
    let wanted: Vec<(usize, &SchemaColumn)> = (schema.columns().iter().enumerate())
        .filter(|(_, c)| has_stats(&c.ty) && only.is_none_or(|id| id == c.id))
        .collect();
    let mut gathers: Vec<Gather> = wanted.iter().map(|(_, c)| Gather::new(c.ty)).collect();
    let mut read = 0;
    for row in rows {
        let row = row?;
        read += 1;
        for ((position, _), gather) in wanted.iter().zip(&mut gathers) {
            gather.add(&row.value(*position).unwrap_or(Value::Null));
        }
    }
    Ok(Built {
        table,
        columns: (wanted.iter().zip(gathers))
            .map(|((_, column), gather)| (column.id, gather.finish(count, read, counter)))
            .collect(),
    })
}

/// The statistics of the column `column` in the statistics tree of `stats`, as `tx` sees it, or `None`; damage of the
/// tree's root page when they do not decode.
pub(crate) fn stored<S: krummi_tree::PageSource + ?Sized>(
    tx: &S,
    stats: TableStats,
    column: u64,
) -> Result<Option<ColumnStats>> {
    if stats.root == 0 {
        return Ok(None);
    }
    let Some(bytes) = krummi_tree::get(tx, stats.root, &column_key(column))? else {
        return Ok(None);
    };
    let found = ColumnStats::decode(bytes).ok_or_else(|| tx.damaged(stats.root))?;
    Ok((found.counter >= stats.emptied).then_some(found))
}

/// The application's copy of `found`, statistics of a column of `column`'s type, for a transaction that sees the
/// counter `seen`.
pub(crate) fn statistics(
    column: &SchemaColumn,
    found: &ColumnStats,
    seen: u64,
) -> ColumnStatistics {
    let key = element_column(column.ty);
    let value = |element: &[u8]| key.decode(&mut &element[..]).unwrap_or(Value::Null);
    ColumnStatistics {
        origin_rows: found.rows,
        rows_read: found.read,
        origin_counter: found.counter,
        stale: stats::is_stale(found.rows, found.counter, seen),
        null_fraction: found.nulls,
        large_fraction: found.large,
        distinct: found.distinct,
        frequent: (found.frequent.iter())
            .map(|(element, fraction)| (value(element), *fraction))
            .collect(),
        histogram: (found.histogram.iter())
            .map(|bucket| HistogramBucket {
                upper: value(&bucket.upper),
                fraction: bucket.fraction,
                shortened: bucket.shortened,
            })
            .collect(),
    }
}

impl ReadTransaction<'_> {
    /// The statistics of the column `column` of `table` that the transaction sees, or `None` (ST16). It samples
    /// nothing and asks for no work, reads no page of the rows or the indexes, and fails with
    /// [`ErrorKind::InvalidSize`](crate::ErrorKind::InvalidSize) for a column name of other than 1 to 1,024 bytes,
    /// [`ErrorKind::DifferentSchema`](crate::ErrorKind::DifferentSchema) for a column the table does not have and
    /// [`ErrorKind::InvalidSchema`](crate::ErrorKind::InvalidSchema) for a vector column (ST12).
    pub fn column_statistics(
        &self,
        table: &ReadTable<'_>,
        column: &str,
    ) -> Result<Option<ColumnStatistics>> {
        self.check_table(table)?;
        let column = stats_column(self.folder, &table.name, &table.schema, column)?;
        let found = stored(&self.inner, table.stats, column.id)?;
        Ok(found.map(|found| statistics(column, &found, table.stats.counter)))
    }

    /// Builds the statistics of `table`, or of its column `column`, from the transaction's snapshot.
    pub(crate) fn build_statistics(
        &self,
        table: &ReadTable<'_>,
        column: Option<&str>,
    ) -> Result<Built> {
        let only = column
            .map(|name| stats_column(self.folder, &table.name, &table.schema, name).map(|c| c.id))
            .transpose()?;
        let count = self.row_count(table)?;
        let rows = self.range_rows(table, &[], ..)?;
        self.store.statistics.sampled();
        build(
            table.id,
            &table.schema,
            only,
            count,
            table.stats.counter,
            rows,
        )
    }
}

impl WriteTransaction<'_> {
    /// The statistics of the column `column` of `table` that the transaction sees, with the failures of
    /// [`ReadTransaction::column_statistics`]; a handle of a deleted table fails with
    /// [`ErrorKind::TreeNotFound`](crate::ErrorKind::TreeNotFound).
    pub fn column_statistics(
        &self,
        table: &WriteTable<'_>,
        column: &str,
    ) -> Result<Option<ColumnStatistics>> {
        let state = self.live_table(table)?;
        let column = stats_column(self.folder, &state.name, &state.schema, column)?;
        let found = stored(&self.inner, state.stats, column.id)?;
        Ok(found.map(|found| statistics(column, &found, state.stats.counter)))
    }

    /// Analyzes `table`, or its column `column`, in this write (ST7): builds the statistics from what the write sees,
    /// its changes included, with the counter of its view, and saves them in it, without asking for another turn. An
    /// abort leaves them as they were. It is a change for T10 when it saves, and ends the earlier references even when
    /// it fails (E16). Its failures are those of [`column_statistics`](Self::column_statistics), then those of
    /// reading the rows and writing the pages, the write's (T11).
    pub fn analyze(&mut self, table: &WriteTable<'_>, column: Option<&str>) -> Result<()> {
        let built = {
            let state = self.live_table(table)?;
            let only = column
                .map(|name| {
                    stats_column(self.folder, &state.name, &state.schema, name).map(|c| c.id)
                })
                .transpose()?;
            let (schema, count, counter) = (state.schema.clone(), state.count, state.stats.counter);
            drop(state);
            let rows = self.range_rows(table, &[], ..)?;
            self.store.statistics.sampled();
            build(table.id, &schema, only, count, counter, rows)?
        };
        self.save_statistics(table, built)
    }

    /// Saves the columns of `built` in the statistics tree of `table`, each unless its column no longer exists, the
    /// table was emptied after the build's snapshot, or the tree has statistics of the column of a later snapshot
    /// (ST6). A build of another table, or of a deleted one, saves nothing.
    pub(crate) fn save_statistics(&mut self, table: &WriteTable<'_>, built: Built) -> Result<()> {
        let (stats, columns) = {
            let state = self.live_table(table)?;
            if state.id() != built.table {
                return Ok(());
            }
            let columns: Vec<u64> = state.schema.columns().iter().map(|c| c.id).collect();
            (state.stats, columns)
        };
        let mut root = stats.root;
        for (column, found) in built.columns {
            if !columns.contains(&column) || found.counter < stats.emptied {
                continue;
            }
            let current = TableStats { root, ..stats };
            if stored(&self.inner, current, column)?
                .is_some_and(|saved| saved.counter > found.counter)
            {
                continue;
            }
            if root == 0 {
                root = krummi_tree::create(&mut self.inner)?;
            }
            root =
                krummi_tree::set(&mut self.inner, root, &column_key(column), &found.encode())?.root;
        }
        if root != stats.root {
            self.set_stats_root(table, root);
        }
        Ok(())
    }
}

impl Store {
    /// Analyzes the table `table`, or its column `column`, outside any transaction (ST7): builds the statistics from a
    /// new snapshot, then saves them with a short write of its own that waits for its turn like any write (T4), and
    /// returns when they are saved or dropped (ST6). Fails at once with [`ErrorKind::Misuse`](crate::ErrorKind::Misuse)
    /// on a thread that has a write open on this store (T5), then with the failures of
    /// [`ReadTransaction::column_statistics`], [`ErrorKind::TreeNotFound`](crate::ErrorKind::TreeNotFound) and
    /// [`ErrorKind::DifferentTreeType`](crate::ErrorKind::DifferentTreeType) for the table, those of reading the
    /// rows, and those of the write and its commit.
    pub fn analyze(&self, table: &str, column: Option<&str>) -> Result<()> {
        self.analyze_within(table, column, None)
    }

    /// Like [`Store::analyze`], but waits for the write turn at most `limit`, counted from the end of the sampling,
    /// with the rules of [`Store::write_with_timeout`].
    pub fn analyze_with_timeout(
        &self,
        table: &str,
        column: Option<&str>,
        limit: Duration,
    ) -> Result<()> {
        self.analyze_within(table, column, Some(limit))
    }

    fn analyze_within(
        &self,
        table: &str,
        column: Option<&str>,
        limit: Option<Duration>,
    ) -> Result<()> {
        if self.engine.holds_write() {
            // The misuse of T5, before sampling.
            return self.write().map(drop);
        }
        let built = {
            let read = self.read()?;
            let handle = read.open_table(table)?;
            read.build_statistics(&handle, column)?
        };
        let mut write = match limit {
            Some(limit) => self.write_with_timeout(limit)?,
            None => self.write()?,
        };
        match write.open_table(table) {
            Ok(handle) => write.save_statistics(&handle, built)?,
            // A table deleted or replaced while sampling: the build is dropped (ST6).
            Err(error) if error.kind() == crate::ErrorKind::TreeNotFound => {}
            Err(error) if error.kind() == crate::ErrorKind::DifferentTreeType => {}
            Err(error) => return Err(error),
        }
        write.commit()
    }
}
