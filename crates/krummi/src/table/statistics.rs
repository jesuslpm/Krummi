//! Building, saving and reading the statistics of a table's columns (ST1, ST7, ST8, ST16 of the statistics PRD).
//!
//! A build reads the rows of one snapshot and gives the statistics of the table's columns that are not vectors, with
//! the snapshot's row count and modification counter. Saving keeps, per column, the statistics of the latest
//! snapshot: it drops a build of a column that no longer exists, of a table that is no longer the same or was emptied
//! after the build's snapshot, and one older than the statistics already saved (ST6, N8).

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use super::catalog::{ReadTable, WriteTable};
use super::estimate::{StatsMemory, newest};
use super::key::KeyColumn;
use super::rows::{TableRow, Versions};
use super::schema::{SchemaColumn, TableSchema, TableStats};
use super::stats::{
    self, ColumnStats, Gather, WHOLE, column_key, element_column, has_stats, prefix_key,
};
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

/// The statistics of a prefix of an index's key (ST16, ST17), in a copy of the application's.
#[derive(Debug, Clone, PartialEq)]
pub struct PrefixStatistics {
    /// The key columns of the prefix, from the first.
    pub columns: usize,
    pub origin_rows: u64,
    pub rows_read: u64,
    pub origin_counter: u64,
    pub stale: bool,
    /// The estimated distinct combinations of the prefix's values, a null counting as a value.
    pub distinct: f64,
    /// The most frequent combinations, in the order of the prefix's columns, and their fractions.
    pub frequent: Vec<(Vec<Value<'static>>, f64)>,
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
#[derive(Clone)]
pub(crate) struct Built {
    pub table: u64,
    /// The table's name in the snapshot.
    pub name: String,
    pub columns: Vec<(u64, ColumnStats)>,
    /// The index prefixes', by index identity and number of key columns (ST17).
    pub prefixes: Vec<((u64, u8), ColumnStats)>,
    /// The epoch of the table in memory when the build started; one of an older epoch is dropped (ST8).
    pub epoch: u64,
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
#[expect(
    clippy::too_many_arguments,
    reason = "what a build reads: the table, its schema, the snapshot's counts and the cancel check"
)]
pub(crate) fn build<'r>(
    table: u64,
    name: &str,
    schema: &TableSchema,
    only: Option<u64>,
    count: u64,
    counter: u64,
    rows: impl Iterator<Item = Result<TableRow<'r>>>,
    cancel: &dyn Fn() -> bool,
) -> Result<Built> {
    let wanted: Vec<(usize, &SchemaColumn)> = (schema.columns().iter().enumerate())
        .filter(|(_, c)| has_stats(&c.ty) && only.is_none_or(|id| id == c.id))
        .collect();
    let mut gathers: Vec<Gather> = wanted.iter().map(|(_, c)| Gather::new(c.ty)).collect();
    // The prefixes of 2 or more key columns of every index, unless one column is asked for (ST17).
    let prefixes: Vec<(u64, Vec<(usize, KeyColumn)>)> = if only.is_some() {
        Vec::new()
    } else {
        (schema.indexes().iter())
            .filter(|index| index.key.len() >= 2)
            .map(|index| {
                let key = (index.key.iter())
                    .map(|k| {
                        let at = (schema.columns().iter())
                            .position(|c| c.id == k.id)
                            .expect("an index names live columns");
                        let ty = schema.columns()[at].ty;
                        (
                            at,
                            KeyColumn {
                                ty,
                                direction: k.direction,
                                nulls: k.nulls,
                                nullable: true,
                            },
                        )
                    })
                    .collect();
                (index.id, key)
            })
            .collect()
    };
    let mut combinations: Vec<Vec<Gather>> = (prefixes.iter())
        .map(|(_, key)| {
            (2..=key.len())
                .map(|_| Gather::new(schema.columns()[key[0].0].ty))
                .collect()
        })
        .collect();
    let mut read = 0;
    for row in rows {
        if cancel() {
            // The caller drops a cancelled build: no gathering is finished.
            return Ok(Built {
                table,
                name: name.to_owned(),
                columns: Vec::new(),
                prefixes: Vec::new(),
                epoch: 0,
            });
        }
        let row = row?;
        read += 1;
        for ((position, _), gather) in wanted.iter().zip(&mut gathers) {
            gather.add(&row.value(*position).unwrap_or(Value::Null));
        }
        for ((_, key), gathers) in prefixes.iter().zip(&mut combinations) {
            let mut element = Vec::new();
            for (at, (position, column)) in key.iter().enumerate() {
                // A stored value always encodes in its column's key encoding.
                let _ = column.encode(&row.value(*position).unwrap_or(Value::Null), &mut element);
                if at >= 1 {
                    gathers[at - 1].add_element(element.clone());
                }
            }
        }
    }
    // Each gathering finishes only while the build is not cancelled (R12 of the code review).
    let cancelled = || Built {
        table,
        name: name.to_owned(),
        columns: Vec::new(),
        prefixes: Vec::new(),
        epoch: 0,
    };
    let mut columns = Vec::with_capacity(wanted.len());
    for ((_, column), gather) in wanted.iter().zip(gathers) {
        match gather.finish(count, read, counter, cancel) {
            Some(stats) => columns.push((column.id, stats)),
            None => return Ok(cancelled()),
        }
    }
    let mut finished = Vec::new();
    for ((index, _), gathers) in prefixes.iter().zip(combinations) {
        for (at, gather) in gathers.into_iter().enumerate() {
            let k = u8::try_from(at + 2).expect("at most 32 key columns");
            match gather.finish_combinations(count, read, counter, cancel) {
                Some(stats) => finished.push(((*index, k), stats)),
                None => return Ok(cancelled()),
            }
        }
    }
    Ok(Built {
        table,
        name: name.to_owned(),
        columns,
        prefixes: finished,
        epoch: 0,
    })
}

/// The statistics of the column `column` in the statistics tree of `stats`, as `tx` sees it, or `None`; damage of the
/// tree's root page when they do not decode.
pub(crate) fn stored<S: krummi_tree::PageSource + ?Sized>(
    tx: &S,
    stats: TableStats,
    column: u64,
) -> Result<Option<ColumnStats>> {
    stored_at(tx, stats, &column_key(column))
}

/// The statistics of the prefix of `columns` key columns of the index `index`, as [`stored`] (ST17).
pub(crate) fn stored_prefix<S: krummi_tree::PageSource + ?Sized>(
    tx: &S,
    stats: TableStats,
    index: u64,
    columns: u8,
) -> Result<Option<ColumnStats>> {
    stored_at(tx, stats, &prefix_key(index, columns))
}

fn stored_at<S: krummi_tree::PageSource + ?Sized>(
    tx: &S,
    stats: TableStats,
    key: &[u8],
) -> Result<Option<ColumnStats>> {
    if stats.root == 0 {
        return Ok(None);
    }
    let Some(bytes) = krummi_tree::get(tx, stats.root, key)? else {
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

/// What a transaction sees of a table for reading its statistics.
pub(crate) struct Seen<'a> {
    pub table: u64,
    pub name: &'a str,
    pub schema: &'a TableSchema,
    pub stats: TableStats,
}

/// The statistics of the prefixes of the index `index` that `tx` sees of the table `seen`, with those of `memory`
/// (see [`ReadTransaction::index_statistics`]).
fn index_statistics<S: krummi_tree::PageSource + ?Sized>(
    folder: Folder<'_>,
    tx: &S,
    memory: &StatsMemory,
    seen: &Seen<'_>,
    index: &str,
    private: &dyn Fn(u64, u8) -> Option<Arc<ColumnStats>>,
) -> Result<Option<Vec<PrefixStatistics>>> {
    let (table_id, table_name, table_schema, table_stats) =
        (seen.table, seen.name, seen.schema, seen.stats);
    folder.check_index_name(index)?;
    let Some(found) = table_schema.indexes().iter().find(|i| i.name == index) else {
        return Err(Error::DifferentIndex {
            folder: folder.owned(),
            table: table_name.to_owned(),
            index: index.to_owned(),
            difference: SchemaDifference::MissingIndex,
            expected: "an index".into(),
            found: "no index".into(),
        });
    };
    let mut prefixes = Vec::new();
    for k in 2..=found.key.len() {
        let columns = u8::try_from(k).expect("at most 32 key columns");
        let saved = stored_prefix(tx, table_stats, found.id, columns)?;
        let kept = memory.get_prefix(table_id, found.id, columns);
        let newest = match (
            saved,
            kept.filter(|kept| kept.counter >= table_stats.emptied),
        ) {
            (Some(saved), Some(kept)) if kept.counter > saved.counter => Some((*kept).clone()),
            (Some(saved), _) => Some(saved),
            (None, kept) => kept.map(|kept| (*kept).clone()),
        };
        // A write's own, when not older (R9 of the first code review).
        let newest = match (newest, private(found.id, columns)) {
            (Some(found), Some(own)) if found.counter > own.counter => Some(found),
            (_, Some(own)) => Some((*own).clone()),
            (found, None) => found,
        };
        if let Some(stats) = newest {
            let keys: Vec<KeyColumn> = (found.key[..k].iter())
                .map(|key| KeyColumn {
                    ty: (table_schema.columns().iter())
                        .find(|c| c.id == key.id)
                        .expect("an index names live columns")
                        .ty,
                    direction: key.direction,
                    nulls: key.nulls,
                    nullable: true,
                })
                .collect();
            prefixes.push(PrefixStatistics {
                columns: k,
                origin_rows: stats.rows,
                rows_read: stats.read,
                origin_counter: stats.counter,
                stale: stats::is_stale(stats.rows, stats.counter, table_stats.counter),
                distinct: stats.distinct,
                frequent: (stats.frequent.iter())
                    .map(|(element, fraction)| {
                        let mut rest = &element[..];
                        let values = (keys.iter())
                            .map(|key| key.decode(&mut rest).unwrap_or(Value::Null))
                            .collect();
                        (values, *fraction)
                    })
                    .collect(),
            });
        }
    }
    Ok((!prefixes.is_empty()).then_some(prefixes))
}

/// The rows of a sample of the rows tree `at` of more than 30,000 rows: whole leaves reached at random from `seed`,
/// about 30,000 rows, decoded by `reader`, until `cancel` says so (ST1, ST10).
pub(crate) fn sampled_rows<'t, S: krummi_tree::PageSource + ?Sized>(
    tx: &'t S,
    reader: &'t Arc<super::rows::Reader>,
    at: super::rows::At<'_>,
    seed: u64,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Result<TableRow<'t>>>> {
    let mut versions = Versions::default();
    let mut rows = Vec::new();
    krummi_tree::sample_leaves(tx, at.rows_root, WHOLE, seed, &mut |key, bytes| {
        rows.push(reader.decode(
            tx,
            super::rows::At { ..at },
            &mut versions,
            Cow::Borrowed(key),
            bytes,
            false,
        ));
        Ok(!cancel())
    })?;
    Ok(rows)
}

/// The number of key columns of the index `index` among `indexes`, or 0.
fn indexes_len(indexes: &[(u64, usize)], index: u64) -> usize {
    (indexes.iter())
        .find(|(id, _)| *id == index)
        .map_or(0, |(_, len)| *len)
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
        let saved = stored(&self.inner, table.stats, column.id)?;
        let kept = self.store.statistics.get(table.id, column.id);
        let found = newest(saved, kept, table.stats).0;
        Ok(found.map(|found| statistics(column, &found, table.stats.counter)))
    }

    /// The statistics of the prefixes of 2 key columns up to all of the index `index` of `table` that the transaction
    /// sees, in that order, or `None` when it sees none of them or the index has one key column (ST16). Fails with
    /// [`ErrorKind::InvalidSize`](crate::ErrorKind::InvalidSize) for an index name of other than 1 to 1,024 bytes and
    /// with [`ErrorKind::DifferentSchema`](crate::ErrorKind::DifferentSchema) for an index the table does not have.
    pub fn index_statistics(
        &self,
        table: &ReadTable<'_>,
        index: &str,
    ) -> Result<Option<Vec<PrefixStatistics>>> {
        self.check_table(table)?;
        let seen = Seen {
            table: table.id,
            name: &table.name,
            schema: &table.schema,
            stats: table.stats,
        };
        index_statistics(
            self.folder,
            &self.inner,
            &self.store.statistics,
            &seen,
            index,
            &|_, _| None,
        )
    }

    /// Builds the statistics of `table`, or of its column `column`, from the transaction's snapshot.
    pub(crate) fn build_statistics(
        &self,
        table: &ReadTable<'_>,
        column: Option<&str>,
    ) -> Result<Built> {
        self.build_statistics_until(table, column, &|| false)
    }

    /// [`build_statistics`](Self::build_statistics), reading no more rows once `cancel` says so (ST10): the build is
    /// then of part of the sample, and the caller drops it.
    pub(crate) fn build_statistics_until(
        &self,
        table: &ReadTable<'_>,
        column: Option<&str>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Built> {
        let only = column
            .map(|name| stats_column(self.folder, &table.name, &table.schema, name).map(|c| c.id))
            .transpose()?;
        let count = self.row_count(table)?;
        self.store.statistics.sampled();
        let epoch = self.store.statistics.epoch(table.id);
        let mut built = self.build_rows(table, only, count, cancel)?;
        built.epoch = epoch;
        Ok(built)
    }

    /// The rows part of [`build_statistics_until`](Self::build_statistics_until).
    fn build_rows(
        &self,
        table: &ReadTable<'_>,
        only: Option<u64>,
        count: u64,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Built> {
        if count <= WHOLE {
            let rows = self.range_rows(table, &[], ..)?.take_while(|_| !cancel());
            return build(
                table.id,
                &table.name,
                &table.schema,
                only,
                count,
                table.stats.counter,
                rows,
                cancel,
            );
        }
        // A larger table is sampled by whole leaves reached at random (ST1).
        let reader = &self
            .readers
            .keep(table.history, || Arc::clone(&table.reader))
            .reader;
        let rows = sampled_rows(
            &self.inner,
            reader,
            table.at(),
            self.store.statistics.seed(),
            cancel,
        )?;
        build(
            table.id,
            &table.name,
            &table.schema,
            only,
            count,
            table.stats.counter,
            rows.into_iter(),
            cancel,
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
        self.check_write_table(table)?;
        self.folder.check_column_name(column)?;
        let state = self.live_table(table)?;
        let column = stats_column(self.folder, &state.name, &state.schema, column)?;
        let target = super::estimate::Target::Column(column.id);
        let private = (self.private_statistics.borrow())
            .get(&(state.id(), target))
            .cloned()
            .filter(|private| private.counter >= state.stats.emptied);
        let saved = stored(&self.inner, state.stats, column.id)?;
        let kept = self.store.statistics.get(state.id(), column.id);
        let found = match (newest(saved, kept, state.stats).0, private) {
            (Some(found), Some(private)) if found.counter > private.counter => Some(found),
            (_, Some(private)) => Some(private),
            (found, None) => found,
        };
        Ok(found.map(|found| statistics(column, &found, state.stats.counter)))
    }

    /// The statistics of the prefixes of the index `index` of `table` that the transaction sees, with the failures
    /// of [`ReadTransaction::index_statistics`] (ST16).
    pub fn index_statistics(
        &self,
        table: &WriteTable<'_>,
        index: &str,
    ) -> Result<Option<Vec<PrefixStatistics>>> {
        self.check_write_table(table)?;
        self.folder.check_index_name(index)?;
        let state = self.live_table(table)?;
        let private = self.private_statistics.borrow();
        let private = |index: u64, k: u8| {
            private
                .get(&(state.id(), super::estimate::Target::Prefix(index, k)))
                .filter(|p| p.counter >= state.stats.emptied)
                .cloned()
        };
        let seen = Seen {
            table: state.id(),
            name: &state.name,
            schema: &state.schema,
            stats: state.stats,
        };
        index_statistics(
            self.folder,
            &self.inner,
            &self.store.statistics,
            &seen,
            index,
            &private,
        )
    }

    /// Analyzes `table`, or its column `column`, in this write (ST7): builds the statistics from what the write sees,
    /// its changes included, with the counter of its view, and saves them in it, without asking for another turn. An
    /// abort leaves them as they were. It is a change for T10 when it saves, and ends the earlier references even when
    /// it fails (E16). Its failures are those of [`column_statistics`](Self::column_statistics), then those of
    /// reading the rows and writing the pages, the write's (T11).
    pub fn analyze(&mut self, table: &WriteTable<'_>, column: Option<&str>) -> Result<()> {
        self.check_write_table(table)?;
        if let Some(column) = column {
            self.folder.check_column_name(column)?;
        }
        let built = {
            let state = self.live_table(table)?;
            let only = column
                .map(|name| {
                    stats_column(self.folder, &state.name, &state.schema, name).map(|c| c.id)
                })
                .transpose()?;
            let (schema, count, counter) = (state.schema.clone(), state.count, state.stats.counter);
            let name = state.name.clone();
            drop(state);
            self.store.statistics.sampled();
            self.build_own(table, &name, &schema, only, count, counter)?
        };
        self.save_statistics(table, built)
    }

    /// Builds the statistics of `table`, named `name` with `schema`, from what this write sees: whole up to 30,000
    /// rows, by a sample above (ST1, ST7).
    pub(crate) fn build_own(
        &self,
        table: &WriteTable<'_>,
        name: &str,
        schema: &TableSchema,
        only: Option<u64>,
        count: u64,
        counter: u64,
    ) -> Result<Built> {
        let epoch = self.store.statistics.epoch(table.id);
        if count <= WHOLE {
            let rows = self.range_rows(table, &[], ..)?;
            let mut built = build(table.id, name, schema, only, count, counter, rows, &|| {
                false
            })?;
            built.epoch = epoch;
            return Ok(built);
        }
        let (rows_root, reader) = self.live_rows(table)?;
        let history = self.live_table(table)?.at().history;
        let at = super::rows::At {
            name,
            rows_root,
            history,
        };
        let rows = sampled_rows(
            &self.inner,
            &reader,
            at,
            self.store.statistics.seed(),
            &|| false,
        )?;
        let mut built = build(
            table.id,
            name,
            schema,
            only,
            count,
            counter,
            rows.into_iter(),
            &|| false,
        )?;
        built.epoch = epoch;
        Ok(built)
    }

    /// Saves the columns of `built` in the statistics tree of `table`, each unless its column no longer exists, the
    /// table was emptied after the build's snapshot, or the tree has statistics of the column of a later snapshot
    /// (ST6). A build of another table, or of a deleted one, saves nothing.
    pub(crate) fn save_statistics(&mut self, table: &WriteTable<'_>, built: Built) -> Result<()> {
        if built.epoch != self.store.statistics.epoch(built.table) {
            // Sampled before a committed change retired the table's statistics (ST8).
            return Ok(());
        }
        let (stats, columns, indexes) = {
            let state = self.live_table(table)?;
            if state.id() != built.table {
                return Ok(());
            }
            let columns: Vec<u64> = state.schema.columns().iter().map(|c| c.id).collect();
            let indexes: Vec<(u64, usize)> = (state.schema.indexes().iter())
                .map(|index| (index.id, index.key.len()))
                .collect();
            (state.stats, columns, indexes)
        };
        let entries = (built.columns.into_iter())
            .filter(|(column, _)| columns.contains(column))
            .map(|(column, found)| (column_key(column).to_vec(), found))
            .chain(
                (built.prefixes.into_iter())
                    .filter(|((index, k), _)| {
                        indexes.contains(&(*index, indexes_len(&indexes, *index)))
                            && indexes_len(&indexes, *index) >= usize::from(*k)
                    })
                    .map(|((index, k), found)| (prefix_key(index, k).to_vec(), found)),
            );
        let mut root = stats.root;
        for (key, found) in entries {
            if found.counter < stats.emptied {
                continue;
            }
            let current = TableStats { root, ..stats };
            if stored_at(&self.inner, current, &key)?
                .is_some_and(|saved| saved.counter > found.counter)
            {
                continue;
            }
            if root == 0 {
                root = krummi_tree::create(&mut self.inner)?;
            }
            root = krummi_tree::set(&mut self.inner, root, &key, &found.encode())?.root;
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
        let shared = self.shared();
        if shared.engine.holds_write() {
            // The misuse of T5, before sampling.
            return self.write().map(drop);
        }
        let built = {
            let read = self.read()?;
            // The names' sizes before the catalog (ST12).
            if let Some(column) = column {
                read.folder.check_column_name(column)?;
            }
            let handle = read.open_table(table)?;
            read.build_statistics(&handle, column)?
        };
        shared.work.pass(super::StatisticsPoint::WorkSampled);
        // Published first: a failure to save leaves them in memory (ST7).
        shared.statistics.publish(&built);
        let mut write = match limit {
            Some(limit) => self.write_with_timeout(limit)?,
            None => self.write()?,
        };
        // The sampled table by identity, also renamed; one deleted or replaced while sampling drops the build (ST6).
        let (id, name) = (built.table, built.name.clone());
        if let Some(handle) = super::worker::find_write_table(&write, id, &name)? {
            write.save_statistics(&handle, built)?;
            shared.work.pass(super::StatisticsPoint::SaveTurn);
        }
        write.commit()
    }
}
