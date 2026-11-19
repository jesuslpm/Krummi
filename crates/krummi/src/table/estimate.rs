//! The estimate (ST2, ST3, ST5, ST12 of the statistics PRD): how many rows of a table meet a condition on one column,
//! from the statistics the transaction sees. It never answers with a guess: without statistics it samples at once
//! (ST3), and with stale ones it uses them.
//!
//! The statistics a transaction sees of a column are the newest, by their origin counter, of the saved ones it sees
//! and those the store keeps in memory, ignoring those built before an emptying of the table that the transaction sees
//! (ST1).

use std::collections::HashMap;
use std::ops::Bound;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use super::catalog::{ReadTable, WriteTable};
use super::schema::{SchemaColumn, TableSchema, TableStats};
use super::statistics::{Built, build, stats_column, stored};
use super::stats::{ColumnStats, element_column, is_large};
use super::value::{Value, normalize};
use super::worker::Job;
use crate::{Error, Folder, ReadTransaction, Result, ValueRule, WriteTransaction};

/// A condition on one column of a table (ST2).
#[derive(Debug, Clone, PartialEq)]
pub enum Condition<'a> {
    /// The column equals the value; a null asks for the rows where the column is null.
    Equal { column: &'a str, value: Value<'a> },
    /// The column is within the bounds; with any bound, the rows where it is null are out.
    Range {
        column: &'a str,
        lower: Bound<Value<'a>>,
        upper: Bound<Value<'a>>,
    },
}

/// What an estimate gives (ST2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    /// The estimated rows, from zero to the table's rows that the transaction sees.
    pub rows: f64,
    /// Whether a statistic it used is stale (ST5).
    pub stale: bool,
    /// Whether one statistic covers all its columns, as always for a condition on one column.
    pub joint: bool,
}

/// The statistics a write's estimates built from its own view, by table and column identity (ST3).
pub(crate) type PrivateStatistics = HashMap<(u64, u64), Arc<ColumnStats>>;

/// The statistics the store keeps in memory, by table and column identity (ST1): the newest built of each.
#[derive(Default)]
pub(crate) struct StatsMemory {
    kept: Mutex<HashMap<(u64, u64), Arc<ColumnStats>>>,
    /// How many samples the store read, for the tests (ST14).
    samples: AtomicU64,
}

impl StatsMemory {
    /// Counts a sample read.
    pub(crate) fn sampled(&self) {
        self.samples.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn get(&self, table: u64, column: u64) -> Option<Arc<ColumnStats>> {
        let memory = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        memory.get(&(table, column)).cloned()
    }

    /// Keeps the columns of `built`, each unless memory has its statistics of a later snapshot (N8).
    pub(crate) fn publish(&self, built: &Built) {
        let mut memory = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        for (column, stats) in &built.columns {
            let key = (built.table, *column);
            if memory
                .get(&key)
                .is_none_or(|kept| kept.counter <= stats.counter)
            {
                memory.insert(key, Arc::new(stats.clone()));
            }
        }
    }
}

/// The newest of `saved` and `kept`, the saved statistics a transaction sees and those of memory, with `table`'s
/// statistics part as it sees it: memory's are ignored when built before its last emptying.
fn newest(
    saved: Option<ColumnStats>,
    kept: Option<Arc<ColumnStats>>,
    table: TableStats,
) -> (Option<Arc<ColumnStats>>, bool) {
    let kept = kept.filter(|kept| kept.counter >= table.emptied);
    match (saved, kept) {
        (Some(saved), Some(kept)) if kept.counter > saved.counter => (Some(kept), true),
        (Some(saved), _) => (Some(Arc::new(saved)), false),
        (None, kept) => {
            let unsaved = kept.is_some();
            (kept, unsaved)
        }
    }
}

/// The statistics of the column `column` of the table `table`, named `name`, as a save request.
fn one_column(table: u64, name: &str, column: u64, found: &ColumnStats) -> Built {
    Built {
        table,
        name: name.to_owned(),
        columns: vec![(column, found.clone())],
    }
}

/// Whether statistics `found` must be sampled again for a transaction that sees `rows` rows (ST2): those of fewer than
/// 500 rows when the table has more than doubled.
fn too_small(found: &ColumnStats, rows: u64) -> bool {
    found.rows < 500 && rows > 2 * found.rows
}

/// A condition checked against its column: its values normalized (N4, E7, E8) and encoded as the column's elements.
enum Checked {
    Null,
    Equal(Value<'static>, Vec<u8>),
    Range(Bound<Vec<u8>>, Bound<Vec<u8>>),
}

impl Condition<'_> {
    fn column(&self) -> &str {
        match self {
            Condition::Equal { column, .. } | Condition::Range { column, .. } => column,
        }
    }
}

/// The values of `condition` on `column` of `table`, with the failures of ST12: a value of another type or out of
/// its domain, a null as a bound; the first value that fails, equality, then the lower bound and the upper one.
fn check(
    folder: Folder<'_>,
    table: &str,
    column: &SchemaColumn,
    condition: &Condition<'_>,
) -> Result<Checked> {
    let fail = |rule| Error::InvalidValue {
        folder: folder.owned(),
        table: Some(table.to_owned()),
        column: Some(column.name.clone()),
        sequence: None,
        rule,
    };
    let encode = |value: &Value<'_>| -> Result<(Value<'static>, Vec<u8>)> {
        let value = normalize(&column.ty, false, value.clone().into_owned()).map_err(fail)?;
        let mut element = Vec::new();
        element_column(column.ty)
            .encode(&value, &mut element)
            .map_err(fail)?;
        Ok((value, element))
    };
    match condition {
        Condition::Equal {
            value: Value::Null, ..
        } => Ok(Checked::Null),
        Condition::Equal { value, .. } => {
            let (value, element) = encode(value)?;
            Ok(Checked::Equal(value, element))
        }
        Condition::Range { lower, upper, .. } => {
            let bound = |bound: &Bound<Value<'_>>| -> Result<Bound<Vec<u8>>> {
                Ok(match bound {
                    Bound::Unbounded => Bound::Unbounded,
                    Bound::Included(Value::Null) | Bound::Excluded(Value::Null) => {
                        return Err(fail(ValueRule::InvalidRange));
                    }
                    Bound::Included(value) => Bound::Included(encode(value)?.1),
                    Bound::Excluded(value) => Bound::Excluded(encode(value)?.1),
                })
            };
            Ok(Checked::Range(bound(lower)?, bound(upper)?))
        }
    }
}

/// Whether `element` is within `lower` and `upper`.
fn within(element: &[u8], lower: &Bound<Vec<u8>>, upper: &Bound<Vec<u8>>) -> bool {
    let above = match lower {
        Bound::Unbounded => true,
        Bound::Included(bound) => element >= bound.as_slice(),
        Bound::Excluded(bound) => element > bound.as_slice(),
    };
    let below = match upper {
        Bound::Unbounded => true,
        Bound::Included(bound) => element <= bound.as_slice(),
        Bound::Excluded(bound) => element < bound.as_slice(),
    };
    above && below
}

/// Whether the range from `lower` to `upper` holds no value.
fn empty_range(lower: &Bound<Vec<u8>>, upper: &Bound<Vec<u8>>) -> bool {
    match (lower, upper) {
        (Bound::Included(a), Bound::Included(b)) => a > b,
        (Bound::Included(a) | Bound::Excluded(a), Bound::Excluded(b))
        | (Bound::Excluded(a), Bound::Included(b)) => a >= b,
        _ => false,
    }
}

/// The fraction of the rows that `checked` meets according to `found` (ST2).
fn fraction(found: &ColumnStats, checked: &Checked) -> f64 {
    let frequent: f64 = found.frequent.iter().map(|(_, f)| f).sum();
    match checked {
        Checked::Null => found.nulls,
        Checked::Equal(value, _) if is_large(value) => found.large,
        Checked::Equal(_, element) => {
            if let Some((_, f)) = found.frequent.iter().find(|(e, _)| e == element) {
                return *f;
            }
            let rest = (1.0 - found.nulls - found.large - frequent).max(0.0);
            let others = (found.distinct - found.frequent.len() as f64).max(1.0);
            rest / others
        }
        Checked::Range(lower, upper) => {
            let mut total = found.large;
            total += (found.frequent.iter())
                .filter(|(e, _)| within(e, lower, upper))
                .map(|(_, f)| f)
                .sum::<f64>();
            // A bucket holds the values after the previous bucket's bound, up to its own: it counts whole when the
            // range covers it, half when the range covers part of it.
            let mut previous: Option<&[u8]> = None;
            for bucket in &found.histogram {
                let starts_in = match lower {
                    Bound::Unbounded => true,
                    Bound::Included(l) | Bound::Excluded(l) => {
                        previous.is_some_and(|p| p >= l.as_slice())
                    }
                };
                let ends_in = below(&bucket.upper, upper);
                let meets = above(&bucket.upper, lower)
                    && previous.is_none_or(|p| below_strictly(p, upper));
                total += bucket.fraction
                    * if starts_in && ends_in {
                        1.0
                    } else if meets {
                        0.5
                    } else {
                        0.0
                    };
                previous = Some(&bucket.upper);
            }
            total
        }
    }
}

/// Whether `element` meets the lower bound `lower`.
fn above(element: &[u8], lower: &Bound<Vec<u8>>) -> bool {
    within(element, lower, &Bound::Unbounded)
}

/// Whether `element` meets the upper bound `upper`.
fn below(element: &[u8], upper: &Bound<Vec<u8>>) -> bool {
    within(element, &Bound::Unbounded, upper)
}

/// Whether some value greater than `element` may meet the upper bound `upper`.
fn below_strictly(element: &[u8], upper: &Bound<Vec<u8>>) -> bool {
    match upper {
        Bound::Unbounded => true,
        Bound::Included(u) | Bound::Excluded(u) => element < u.as_slice(),
    }
}

/// The estimate of `checked` on `column` of a table whose schema is `schema`, with `found` and `rows` rows seen; the
/// exact rules of ST2 last.
fn estimate_with(
    schema: &TableSchema,
    column: &SchemaColumn,
    checked: &Checked,
    found: &ColumnStats,
    rows: u64,
    stale: bool,
) -> Estimate {
    let rows_f = rows as f64;
    let mut estimate = (fraction(found, checked) * rows_f).clamp(0.0, rows_f);
    match checked {
        Checked::Range(Bound::Unbounded, Bound::Unbounded) => estimate = rows_f,
        Checked::Range(lower, upper) if empty_range(lower, upper) => estimate = 0.0,
        Checked::Null if !column.nullable => estimate = 0.0,
        Checked::Equal(..) if alone_unique(schema, column.id) => estimate = estimate.min(1.0),
        _ => {}
    }
    Estimate {
        rows: estimate,
        stale,
        joint: true,
    }
}

/// Whether the column `column` is by itself the primary key or the key of a unique index of `schema`.
pub(crate) fn alone_unique(schema: &TableSchema, column: u64) -> bool {
    let key = schema.primary_key();
    let alone_key = key.len() == 1
        && schema
            .columns()
            .iter()
            .any(|c| c.id == column && c.name == key[0]);
    alone_key
        || (schema.indexes().iter())
            .any(|index| index.unique && index.key.len() == 1 && index.key[0].id == column)
}

impl crate::Shared {
    /// Asks the statistics thread to save the statistics of `built` (ST6).
    pub(crate) fn request_save(&self, built: &Built) {
        self.work.request(self, Job::Save(built.clone()));
    }

    /// Asks the statistics thread to renew the stale statistics of the table `table`, named `name` (ST6).
    pub(crate) fn request_renewal(&self, table: u64, name: &str) {
        self.work.request(
            self,
            Job::Renew {
                table,
                name: name.to_owned(),
            },
        );
    }
}

impl crate::Store {
    /// A test hook, not part of the interface: how many samples the store read for statistics (ST14).
    #[doc(hidden)]
    pub fn statistics_samples(&self) -> u64 {
        self.shared().statistics.samples.load(Ordering::Relaxed)
    }

    /// A test hook, not part of the interface: holds or releases a point of the statistics work (ST14).
    #[doc(hidden)]
    pub fn hold_statistics(&self, point: super::StatisticsPoint, held: bool) {
        self.shared().work.hold(point, held);
    }

    /// A test hook, not part of the interface: whether the statistics thread runs, and the saves and renewals queued
    /// besides the one it runs (ST14).
    #[doc(hidden)]
    pub fn statistics_work(&self) -> (bool, usize, usize) {
        self.shared().work.status()
    }

    /// A test hook, not part of the interface: waits until the statistics thread has nothing to do (ST14).
    #[doc(hidden)]
    pub fn wait_statistics_idle(&self) {
        self.shared().work.wait_idle();
    }
}

impl ReadTransaction<'_> {
    /// An estimate of how many rows of `table` meet `condition` (ST2). It uses the statistics of the column the
    /// transaction sees, stale or not; without any, or with those of fewer than 500 rows when the table has more than
    /// doubled, it samples the transaction's snapshot at once and waits for the sample (ST3), and keeps the result in
    /// the store's memory for the other transactions. A table without rows estimates zero. Fails as
    /// [`column_statistics`](Self::column_statistics), then with
    /// [`ErrorKind::InvalidValue`](crate::ErrorKind::InvalidValue) for a value of another type, out of its domain or a
    /// null as a bound, and then with the failures of reading the sample.
    pub fn estimate(&self, table: &ReadTable<'_>, condition: &Condition<'_>) -> Result<Estimate> {
        self.check_table(table)?;
        let column = stats_column(self.folder, &table.name, &table.schema, condition.column())?;
        let checked = check(self.folder, &table.name, column, condition)?;
        let rows = self.row_count(table)?;
        if rows == 0 {
            return Ok(Estimate {
                rows: 0.0,
                stale: false,
                joint: true,
            });
        }
        let found = self.seen_statistics(table, column.id)?;
        let found = match found {
            Some(found) if !too_small(&found, rows) => found,
            _ => {
                let built = self.build_statistics(table, None)?;
                self.store
                    .work
                    .pass(super::StatisticsPoint::EstimateSampled);
                self.store.statistics.publish(&built);
                self.store.request_save(&built);
                let found = built.columns.into_iter().find(|(id, _)| *id == column.id);
                Arc::new(found.expect("every column but vectors").1)
            }
        };
        let stale = super::stats::is_stale(found.rows, found.counter, table.stats.counter);
        if stale {
            self.store.request_renewal(table.id, &table.name);
        }
        Ok(estimate_with(
            &table.schema,
            column,
            &checked,
            &found,
            rows,
            stale,
        ))
    }

    /// The statistics of the column `column` of `table` that the transaction sees (ST1).
    pub(crate) fn seen_statistics(
        &self,
        table: &ReadTable<'_>,
        column: u64,
    ) -> Result<Option<Arc<ColumnStats>>> {
        let saved = stored(&self.inner, table.stats, column)?;
        let (found, unsaved) = newest(
            saved,
            self.store.statistics.get(table.id, column),
            table.stats,
        );
        if let Some(found) = found.as_ref().filter(|_| unsaved) {
            // Statistics only in memory, whose save failed or is pending, are asked to be saved again (ST9).
            self.store
                .request_save(&one_column(table.id, &table.name, column, found));
        }
        Ok(found)
    }
}

impl WriteTransaction<'_> {
    /// An estimate of how many rows of `table` meet `condition`, with the failures and rules of
    /// [`ReadTransaction::estimate`], counting the rows this transaction sees. When it samples, it reads the snapshot a
    /// new read transaction would see, without the write's changes; when that snapshot lacks the table, the column or
    /// the rows after an emptying of this write, it reads what the write sees, and that statistic serves only this
    /// write (ST3). It is not a change for T10 and ends no reference.
    pub fn estimate(&self, table: &WriteTable<'_>, condition: &Condition<'_>) -> Result<Estimate> {
        let state = self.live_table(table)?;
        let column =
            stats_column(self.folder, &state.name, &state.schema, condition.column())?.clone();
        let checked = check(self.folder, &state.name, &column, condition)?;
        let (rows, stats, name, schema, id) = (
            state.count,
            state.stats,
            state.name.clone(),
            state.schema.clone(),
            state.id(),
        );
        drop(state);
        if rows == 0 {
            return Ok(Estimate {
                rows: 0.0,
                stale: false,
                joint: true,
            });
        }
        let private = self
            .private_statistics
            .borrow()
            .get(&(id, column.id))
            .cloned();
        let found = match private {
            Some(found) => Some(found),
            None => {
                let saved = stored(&self.inner, stats, column.id)?;
                let (found, unsaved) =
                    newest(saved, self.store.statistics.get(id, column.id), stats);
                if let Some(found) = found.as_ref().filter(|_| unsaved) {
                    self.store
                        .request_save(&one_column(id, &name, column.id, found));
                }
                found
            }
        };
        let found = match found {
            Some(found) if !too_small(&found, rows) => found,
            _ => self.sample_for(table, id, &name, &schema, stats, column.id)?,
        };
        let stale = super::stats::is_stale(found.rows, found.counter, stats.counter);
        if stale {
            self.store.request_renewal(id, &name);
        }
        Ok(estimate_with(
            &schema, &column, &checked, &found, rows, stale,
        ))
    }

    /// Samples the table `id`, named `name` with `schema` and the statistics part `stats` in this write, for the
    /// column `column` (ST3).
    fn sample_for(
        &self,
        table: &WriteTable<'_>,
        id: u64,
        name: &str,
        schema: &TableSchema,
        stats: TableStats,
        column: u64,
    ) -> Result<Arc<ColumnStats>> {
        let read = self.store.read()?;
        let committed = match read.open_table(name) {
            Ok(handle)
                if handle.id == id
                    && handle.stats.counter >= stats.emptied
                    && handle.schema.columns().iter().any(|c| c.id == column) =>
            {
                Some(read.build_statistics(&handle, None)?)
            }
            Ok(_) => None,
            Err(error) if error.kind() == crate::ErrorKind::TreeNotFound => None,
            Err(error) => return Err(error),
        };
        let built = match committed {
            Some(built) => {
                self.store
                    .work
                    .pass(super::StatisticsPoint::EstimateSampled);
                self.store.statistics.publish(&built);
                self.store.request_save(&built);
                built
            }
            None => {
                let rows = self.range_rows(table, &[], ..)?;
                self.store.statistics.sampled();
                let built = build(
                    id,
                    name,
                    schema,
                    None,
                    self.row_count(table)?,
                    stats.counter,
                    rows,
                )?;
                let mut private = self.private_statistics.borrow_mut();
                for (column, found) in &built.columns {
                    private.insert((id, *column), Arc::new(found.clone()));
                }
                built
            }
        };
        let found = built.columns.into_iter().find(|(c, _)| *c == column);
        Ok(Arc::new(found.expect("every column but vectors").1))
    }
}
