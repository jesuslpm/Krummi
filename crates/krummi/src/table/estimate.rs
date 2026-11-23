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
use super::key::KeyColumn;
use super::schema::{SchemaColumn, TableSchema, TableStats};
use super::statistics::{Built, stats_column, stored, stored_prefix};
use super::stats::{ColumnStats, element_column, is_large_folded};
use super::value::{Value, normalize};
use super::worker::Job;
#[cfg(test)]
#[path = "estimate_tests.rs"]
mod tests;
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
    /// Each column equals its value, a null asking for the rows where it is null (ST18).
    EqualColumns(Vec<(&'a str, Value<'a>)>),
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
pub(crate) type PrivateStatistics = HashMap<(u64, Target), Arc<ColumnStats>>;

/// The statistics the store keeps in memory, by table and column identity (ST1): the newest built of each.
#[derive(Default)]
pub(crate) struct StatsMemory {
    /// Under one lock, so that retiring and publishing are atomic (R11 of the code review).
    state: Mutex<MemoryState>,
    /// How many samples the store read, for the tests (ST14).
    samples: AtomicU64,
    /// The pages of value runs its samples read, for the tests (ST14).
    pages: AtomicU64,
    /// The seed of the samples of large tables that a test fixed, or 0 (ST14).
    seed: AtomicU64,
}

#[derive(Default)]
struct MemoryState {
    columns: HashMap<(u64, u64), Arc<ColumnStats>>,
    /// Those of the index prefixes, by table, index and number of key columns (ST17).
    prefixes: HashMap<(u64, u64, u8), Arc<ColumnStats>>,
    /// Per table, the id of the last commit that emptied or deleted it: a build from a snapshot that does not see that
    /// commit is dropped (ST8, R11 of the code review).
    retired_at: HashMap<u64, u64>,
    /// Per table, the counter of its last committed emptying, or `u64::MAX` once deleted: statistics of an earlier
    /// snapshot are not published (R11 of the code review).
    floors: HashMap<u64, u64>,
    /// The columns and indexes removed from tables by committed writes: never published again (R11). Those of a
    /// table are forgotten when it is deleted, which its floor covers; the rest stay until the store closes, one entry
    /// per removed column or index, like `retired_at` and `floors`, one entry per table emptied or deleted.
    removed: std::collections::HashSet<(u64, u64)>,
}

fn valid(state: &MemoryState, table: u64, epoch: u64) -> bool {
    state
        .retired_at
        .get(&table)
        .is_none_or(|retired| *retired <= epoch)
}

/// What a commit retires from memory (ST8): the tables it emptied or deleted, whole, and the columns and indexes it
/// removed from the tables it kept.
#[derive(Default)]
pub(crate) struct Retired {
    /// The tables emptied or deleted, with the counter of the emptying or `u64::MAX` for a deletion.
    pub tables: Vec<(u64, u64)>,
    pub columns: Vec<(u64, u64)>,
    pub indexes: Vec<(u64, u64)>,
}

impl StatsMemory {
    fn state(&self) -> std::sync::MutexGuard<'_, MemoryState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The seed of the next sample of a large table: the one a test fixed, or a random one.
    pub(crate) fn seed(&self) -> u64 {
        match self.seed.load(Ordering::Relaxed) {
            0 => {
                use std::hash::{BuildHasher, Hasher};
                std::collections::hash_map::RandomState::new()
                    .build_hasher()
                    .finish()
            }
            seed => seed,
        }
    }

    /// Counts the pages of value runs a sample read.
    pub(crate) fn read_pages(&self, pages: u64) {
        self.pages.fetch_add(pages, Ordering::Relaxed);
    }

    /// Counts a sample read.
    pub(crate) fn sampled(&self) {
        self.samples.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn get(&self, table: u64, column: u64) -> Option<Arc<ColumnStats>> {
        self.state().columns.get(&(table, column)).cloned()
    }

    pub(crate) fn get_prefix(
        &self,
        table: u64,
        index: u64,
        columns: u8,
    ) -> Option<Arc<ColumnStats>> {
        self.state().prefixes.get(&(table, index, columns)).cloned()
    }

    /// Whether a build of the table `table` from a snapshot that sees the commits up to `epoch` is still valid: it
    /// sees the last commit that emptied or deleted the table.
    pub(crate) fn valid(&self, table: u64, epoch: u64) -> bool {
        valid(&self.state(), table, epoch)
    }

    /// Forgets what `retired` names (ST8), before the write that changed it lets go of the turn: whole tables, whose
    /// epoch grows so that a build sampled before is dropped, and removed columns and indexes. A commit that then fails
    /// only made the store sample again.
    pub(crate) fn retire(&self, retired: &Retired, txid: u64) {
        if retired.tables.is_empty() && retired.columns.is_empty() && retired.indexes.is_empty() {
            return;
        }
        let mut state = self.state();
        for (table, floor) in &retired.tables {
            state.retired_at.insert(*table, txid);
            state.floors.insert(*table, *floor);
        }
        state.removed.extend(retired.columns.iter().copied());
        state.removed.extend(retired.indexes.iter().copied());
        // A deleted table's floor keeps every statistic of it out, so its removed targets are no longer needed.
        let deleted = |table: &u64| {
            (retired.tables.iter()).any(|(t, floor)| t == table && *floor == u64::MAX)
        };
        if retired.tables.iter().any(|(_, floor)| *floor == u64::MAX) {
            state.removed.retain(|(table, _)| !deleted(table));
        }
        let gone = |table: &u64| retired.tables.iter().any(|(t, _)| t == table);
        state.columns.retain(|(table, column), _| {
            !gone(table) && !retired.columns.contains(&(*table, *column))
        });
        state.prefixes.retain(|(table, index, _), _| {
            !gone(table) && !retired.indexes.contains(&(*table, *index))
        });
    }

    /// Keeps the columns and prefixes of `built`, each unless memory has its statistics of a later snapshot (N8), and
    /// nothing when the table's epoch moved since the build started; under the same lock as [`retire`](Self::retire).
    pub(crate) fn publish(&self, built: &Built) {
        let mut state = self.state();
        if !valid(&state, built.table, built.epoch) {
            return;
        }
        // Statistics of a snapshot before a committed emptying or deletion, or of a removed target, are not published.
        let floor = state.floors.get(&built.table).copied().unwrap_or(0);
        for ((index, columns), stats) in &built.prefixes {
            if stats.counter < floor || state.removed.contains(&(built.table, *index)) {
                continue;
            }
            let key = (built.table, *index, *columns);
            if state
                .prefixes
                .get(&key)
                .is_none_or(|kept| kept.counter <= stats.counter)
            {
                state.prefixes.insert(key, Arc::new(stats.clone()));
            }
        }
        for (column, stats) in &built.columns {
            if stats.counter < floor || state.removed.contains(&(built.table, *column)) {
                continue;
            }
            let key = (built.table, *column);
            if state
                .columns
                .get(&key)
                .is_none_or(|kept| kept.counter <= stats.counter)
            {
                state.columns.insert(key, Arc::new(stats.clone()));
            }
        }
    }
}

/// The newest of `saved` and `kept`, the saved statistics a transaction sees and those of memory, with `table`'s
/// statistics part as it sees it: memory's are ignored when built before its last emptying; and whether those are
/// only in memory, newer than the saved ones.
pub(crate) fn newest(
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

/// The saved statistics of `target` in the statistics tree of `stats`.
pub(crate) fn saved_target<S: krummi_tree::PageSource + ?Sized>(
    tx: &S,
    stats: TableStats,
    target: Target,
) -> Result<Option<ColumnStats>> {
    match target {
        Target::Column(column) => stored(tx, stats, column),
        Target::Prefix(index, k) => stored_prefix(tx, stats, index, k),
    }
}

impl crate::Shared {
    /// The statistics of `target` of the table `table`, named `name`, that a transaction sees (ST1): the newest of
    /// the saved ones `saved`, those of memory and, in a write, its own `private`, those built before an emptying it
    /// sees left out; when the newest are only in memory, it asks for them to be saved again (ST9).
    pub(crate) fn seen(
        &self,
        table: u64,
        name: &str,
        target: Target,
        saved: Option<ColumnStats>,
        private: Option<Arc<ColumnStats>>,
        stats: TableStats,
    ) -> Option<Arc<ColumnStats>> {
        let kept = match target {
            Target::Column(column) => self.statistics.get(table, column),
            Target::Prefix(index, k) => self.statistics.get_prefix(table, index, k),
        };
        let (found, unsaved) = newest(saved, kept, stats);
        let private = private.filter(|p| p.counter >= stats.emptied);
        match (found, private) {
            (Some(found), Some(private)) if found.counter > private.counter => {
                if unsaved {
                    self.request_save(&one_target(table, name, target, &found, found.snapshot));
                }
                Some(found)
            }
            (_, Some(private)) => Some(private),
            (found, None) => {
                if let Some(found) = found.as_ref().filter(|_| unsaved) {
                    self.request_save(&one_target(table, name, target, found, found.snapshot));
                }
                found
            }
        }
    }
}

/// The statistics `found` of `target` of the table `table`, named `name`, as a save request.
fn one_target(table: u64, name: &str, target: Target, found: &ColumnStats, epoch: u64) -> Built {
    let (columns, prefixes) = match target {
        Target::Column(column) => (vec![(column, found.clone())], Vec::new()),
        Target::Prefix(index, k) => (Vec::new(), vec![((index, k), found.clone())]),
    };
    Built {
        table,
        name: name.to_owned(),
        columns,
        prefixes,
        epoch,
        pages: 0,
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
    /// Whether the value is large (ST1, IC8 of the collation PRD), and its element.
    Equal {
        large: bool,
        element: Vec<u8>,
    },
    Range(Bound<Vec<u8>>, Bound<Vec<u8>>),
}

impl Condition<'_> {
    fn column(&self) -> &str {
        match self {
            Condition::Equal { column, .. } | Condition::Range { column, .. } => column,
            Condition::EqualColumns(_) => unreachable!("one column"),
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
        json: None,
        folder: folder.owned(),
        table: Some(table.to_owned()),
        column: Some(column.name.clone()),
        sequence: None,
        rule,
    };
    let key = element_column(column);
    let encode = |value: &Value<'_>| -> Result<(Value<'static>, Vec<u8>)> {
        let value = normalize(&column.ty, false, value.clone().into_owned()).map_err(fail)?;
        let mut element = Vec::new();
        key.encode(&value, &mut element).map_err(fail)?;
        Ok((value, element))
    };
    match condition {
        Condition::Equal {
            value: Value::Null, ..
        } => Ok(Checked::Null),
        Condition::Equal { value, .. } => {
            let (value, element) = encode(value)?;
            let large = is_large_folded(&key, &value);
            Ok(Checked::Equal { large, element })
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
        Condition::EqualColumns(_) => unreachable!("checked column by column"),
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
        Checked::Equal { large: true, .. } => found.large,
        Checked::Equal { element, .. } => {
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
                // A shortened bound is greater than the previous bound and not greater than the bucket's values, so
                // it keeps the buckets' order (ST1).
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
        Checked::Equal { .. } if alone_unique(schema, column.id) => estimate = estimate.min(1.0),
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

    /// A test hook, not part of the interface: how many pages of value runs the store's samples read; a large value is
    /// counted without reading its pages (ST14, AC-ST12).
    #[doc(hidden)]
    pub fn statistics_pages_read(&self) -> u64 {
        self.shared().statistics.pages.load(Ordering::Relaxed)
    }

    /// A test hook, not part of the interface: fixes the seed of the samples of tables of more than 30,000 rows, or
    /// with 0 makes it random again (ST14).
    #[doc(hidden)]
    pub fn set_statistics_seed(&self, seed: u64) {
        self.shared().statistics.seed.store(seed, Ordering::Relaxed);
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
        if let Condition::EqualColumns(pairs) = condition {
            let rows = self.row_count(table)?;
            let source = Source {
                lookup: &|target| {
                    let saved = saved_target(&self.inner, table.stats, target)?;
                    Ok(self
                        .store
                        .seen(table.id, &table.name, target, saved, None, table.stats))
                },
                sample: &|| {
                    let built = self.build_statistics(table, None)?;
                    self.store
                        .work
                        .pass(super::StatisticsPoint::EstimateSampled);
                    self.store.statistics.publish(&built);
                    self.store.request_save(&built);
                    Ok(built)
                },
            };
            let estimate = estimate_columns(
                self.folder,
                &table.name,
                &table.schema,
                table.stats,
                rows,
                pairs,
                &source,
            )?;
            if estimate.stale {
                self.store.request_renewal(table.id, &table.name);
            }
            return Ok(estimate);
        }
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
        let target = Target::Column(column);
        Ok(self
            .store
            .seen(table.id, &table.name, target, saved, None, table.stats))
    }
}

impl WriteTransaction<'_> {
    /// An estimate of how many rows of `table` meet `condition`, with the failures and rules of
    /// [`ReadTransaction::estimate`], counting the rows this transaction sees. When it samples, it reads the snapshot a
    /// new read transaction would see, without the write's changes; when that snapshot lacks the table, the column or
    /// the rows after an emptying of this write, it reads what the write sees, and that statistic serves only this
    /// write (ST3). It is not a change for T10 and ends no reference.
    pub fn estimate(&self, table: &WriteTable<'_>, condition: &Condition<'_>) -> Result<Estimate> {
        self.check_write_table(table)?;
        condition_names(self.folder, condition)?;
        let state = self.live_table(table)?;
        if let Condition::EqualColumns(pairs) = condition {
            let (rows, stats, name, schema, id) = (
                state.count,
                state.stats,
                state.name.clone(),
                state.schema.clone(),
                state.id(),
            );
            drop(state);
            let source = Source {
                lookup: &|target| {
                    let saved = saved_target(&self.inner, stats, target)?;
                    let private = self.private_statistics.borrow().get(&(id, target)).cloned();
                    Ok(self.store.seen(id, &name, target, saved, private, stats))
                },
                sample: &|| self.sample_built(table, id, &name, &schema, stats),
            };
            let estimate =
                estimate_columns(self.folder, &name, &schema, stats, rows, pairs, &source)?;
            if estimate.stale {
                self.store.request_renewal(id, &name);
            }
            return Ok(estimate);
        }
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
        let target = Target::Column(column.id);
        let private = self.private_statistics.borrow().get(&(id, target)).cloned();
        let saved = stored(&self.inner, stats, column.id)?;
        let found = self.store.seen(id, &name, target, saved, private, stats);
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
        let built = self.sample_built(table, id, name, schema, stats)?;
        let found = built.columns.into_iter().find(|(c, _)| *c == column);
        Ok(Arc::new(found.expect("every column but vectors").1))
    }

    /// Samples the table `id` for an estimate of this write (ST3): the snapshot a new read sees, published for the
    /// other transactions, when it has the table with every column and index of `schema` and no emptying of this
    /// write came after it; otherwise what this write sees, for this write only.
    fn sample_built(
        &self,
        table: &WriteTable<'_>,
        id: u64,
        name: &str,
        schema: &TableSchema,
        stats: TableStats,
    ) -> Result<Built> {
        let read = self.store.read().map_err(|error| self.inner.fail(error))?;
        let committed = match read.open_table(name) {
            Ok(handle)
                if handle.id == id
                    && handle.stats.counter >= stats.emptied
                    && (schema.columns().iter())
                        .all(|c| handle.schema.columns().iter().any(|h| h.id == c.id))
                    && (schema.indexes().iter())
                        .all(|i| handle.schema.indexes().iter().any(|h| h.id == i.id)) =>
            {
                Some(
                    read.build_statistics(&handle, None)
                        .map_err(|error| self.inner.fail(error))?,
                )
            }
            Ok(_) => None,
            Err(error)
                if matches!(
                    error.kind(),
                    crate::ErrorKind::TreeNotFound | crate::ErrorKind::DifferentTreeType
                ) =>
            {
                None
            }
            // A sample that fails is this write's failure (T11).
            Err(error) => return Err(self.inner.fail(error)),
        };
        Ok(match committed {
            Some(built) => {
                self.store
                    .work
                    .pass(super::StatisticsPoint::EstimateSampled);
                self.store.statistics.publish(&built);
                self.store.request_save(&built);
                built
            }
            None => {
                self.store.statistics.sampled();
                let count = self.row_count(table)?;
                let built = self.build_own(table, name, schema, None, count, stats.counter)?;
                let mut private = self.private_statistics.borrow_mut();
                for (column, found) in &built.columns {
                    private.insert((id, Target::Column(*column)), Arc::new(found.clone()));
                }
                for ((index, k), found) in &built.prefixes {
                    private.insert((id, Target::Prefix(*index, *k)), Arc::new(found.clone()));
                }
                built
            }
        })
    }
}

/// Fails with the invalid size of a column name of `condition` (ST12), before the table and its columns.
fn condition_names(folder: Folder<'_>, condition: &Condition<'_>) -> Result<()> {
    match condition {
        Condition::Equal { column, .. } | Condition::Range { column, .. } => {
            folder.check_column_name(column)
        }
        Condition::EqualColumns(pairs) => pairs
            .iter()
            .try_for_each(|(name, _)| folder.check_column_name(name)),
    }
}

/// What statistics are of: a column, or a prefix of an index's key (ST17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Target {
    Column(u64),
    Prefix(u64, u8),
}

/// Where an estimate on several columns finds statistics: those the transaction sees, and a sample of the table when
/// one is missing (ST3).
struct Source<'a> {
    lookup: &'a dyn Fn(Target) -> Result<Option<Arc<ColumnStats>>>,
    sample: &'a dyn Fn() -> Result<Built>,
}

/// The estimate of the equality in several columns `pairs` (ST18) on the table `table` of `schema`, whose statistics
/// part `stats` and `rows` rows the transaction sees.
fn estimate_columns(
    folder: Folder<'_>,
    table: &str,
    schema: &TableSchema,
    stats: TableStats,
    rows: u64,
    pairs: &[(&str, Value<'_>)],
    source: &Source<'_>,
) -> Result<Estimate> {
    let invalid = |column: Option<&str>, rule| Error::InvalidValue {
        json: None,
        folder: folder.owned(),
        table: Some(table.to_owned()),
        column: column.map(str::to_owned),
        sequence: None,
        rule,
    };
    // The names' sizes and the columns first, then the list's own rules, then the values (ST12).
    for (name, _) in pairs {
        folder.check_column_name(name)?;
    }
    let mut columns: Vec<(&SchemaColumn, Checked)> = Vec::with_capacity(pairs.len());
    for (name, _) in pairs {
        columns.push((stats_column(folder, table, schema, name)?, Checked::Null));
    }
    if pairs.is_empty() {
        return Err(invalid(None, ValueRule::InvalidRange));
    }
    for (at, (name, _)) in pairs.iter().enumerate() {
        if pairs[..at].iter().any(|(other, _)| other == name) {
            return Err(invalid(Some(name), ValueRule::RepeatedColumn));
        }
    }
    for ((column, checked), (_, value)) in columns.iter_mut().zip(pairs) {
        let condition = Condition::Equal {
            column: &column.name,
            value: value.clone(),
        };
        *checked = check(folder, table, column, &condition)?;
    }
    let zero = Estimate {
        rows: 0.0,
        stale: false,
        joint: true,
    };
    if rows == 0 {
        return Ok(zero);
    }
    // The groups: the longest index prefixes whose columns are all in the condition and not yet covered, the first
    // index by name on a tie; then each column left on its own.
    let mut covered = vec![false; columns.len()];
    let mut groups: Vec<(Target, Vec<usize>)> = Vec::new();
    loop {
        let mut best: Option<(u64, u8, Vec<usize>)> = None;
        for index in schema.indexes() {
            for k in (2..=index.key.len()).rev() {
                let at: Option<Vec<usize>> = (index.key[..k].iter())
                    .map(|key| {
                        columns
                            .iter()
                            .position(|(c, _)| c.id == key.id)
                            .filter(|at| !covered[*at])
                    })
                    .collect();
                if let Some(at) = at
                    && best
                        .as_ref()
                        .is_none_or(|(_, best_k, _)| usize::from(*best_k) < k)
                {
                    best = Some((index.id, u8::try_from(k).expect("at most 32"), at));
                    break;
                }
            }
        }
        let Some((index, k, at)) = best else {
            break;
        };
        for a in &at {
            covered[*a] = true;
        }
        groups.push((Target::Prefix(index, k), at));
    }
    for (at, (column, _)) in columns.iter().enumerate() {
        if !covered[at] {
            groups.push((Target::Column(column.id), vec![at]));
        }
    }
    // Their statistics, sampling once when one is missing or too small (ST2, ST3).
    let mut found: Vec<Option<Arc<ColumnStats>>> = Vec::with_capacity(groups.len());
    for (target, _) in &groups {
        found.push((source.lookup)(*target)?.filter(|f| !too_small(f, rows)));
    }
    if found.iter().any(Option::is_none) {
        let built = (source.sample)()?;
        for ((target, _), slot) in groups.iter().zip(&mut found) {
            let stats = match target {
                Target::Column(column) => built
                    .columns
                    .iter()
                    .find(|(c, _)| c == column)
                    .map(|(_, s)| s.clone()),
                Target::Prefix(index, k) => (built.prefixes.iter())
                    .find(|((i, n), _)| i == index && n == k)
                    .map(|(_, s)| s.clone()),
            };
            *slot = stats.map(Arc::new);
        }
    }
    let mut stale = false;
    let mut fractions = Vec::with_capacity(groups.len());
    for ((target, at), found) in groups.iter().zip(&found) {
        let found = found.as_ref().expect("built for every column and prefix");
        stale |= super::stats::is_stale(found.rows, found.counter, stats.counter);
        let fraction = match target {
            Target::Column(_) => fraction(found, &columns[at[0]].1),
            Target::Prefix(index, _) => {
                // The combination in the index's own encoding, with its directions and null orders (ST17).
                let index = (schema.indexes().iter())
                    .find(|i| i.id == *index)
                    .expect("a group of an index of the schema");
                let mut element = Vec::new();
                for (position, a) in at.iter().enumerate() {
                    let (column, _) = columns[*a];
                    let value = normalize(&column.ty, true, pairs[*a].1.clone().into_owned())
                        .map_err(|rule| invalid(Some(&column.name), rule))?;
                    let key = KeyColumn {
                        ty: column.ty,
                        direction: index.key[position].direction,
                        nulls: index.key[position].nulls,
                        nullable: true,
                        collation: column.collation.unwrap_or_default(),
                    };
                    key.encode(&value, &mut element)
                        .map_err(|rule| invalid(Some(&column.name), rule))?;
                }
                // A group with a large value is the prefix's large fraction, and the other combinations share what
                // the frequent ones and the large values leave, as an equality of ST2 (ST18, IC7 of the collation
                // PRD).
                let large = (at.iter())
                    .any(|a| matches!(columns[*a].1, Checked::Equal { large: true, .. }));
                if large {
                    found.large
                } else if let Some((_, f)) = found.frequent.iter().find(|(e, _)| *e == element) {
                    *f
                } else {
                    let frequent: f64 = found.frequent.iter().map(|(_, f)| f).sum();
                    (1.0 - found.large - frequent).max(0.0)
                        / (found.distinct - found.frequent.len() as f64).max(1.0)
                }
            }
        };
        fractions.push(fraction);
    }
    // Exact rules: a null in a column without nulls; every column of the primary key or of a unique index given.
    if columns
        .iter()
        .any(|(c, checked)| matches!(checked, Checked::Null) && !c.nullable)
    {
        return Ok(Estimate { stale, ..zero });
    }
    // The exponential backoff biased to overestimate: f1 × f2^(1/4) × f3^(1/16) × f4^(1/64).
    fractions.sort_by(f64::total_cmp);
    let combined: f64 = (fractions.iter().take(4).enumerate())
        .map(|(at, f)| f.powf(0.25f64.powi(i32::try_from(at).expect("four"))))
        .product();
    let rows_f = rows as f64;
    let mut estimate = (combined * rows_f).clamp(0.0, rows_f);
    let given = |id: u64| {
        (columns.iter()).any(|(c, checked)| c.id == id && !matches!(checked, Checked::Null))
    };
    let key = schema.primary_key();
    let whole_key = key.iter().all(|name| {
        schema
            .columns()
            .iter()
            .any(|c| c.name == *name && given(c.id))
    });
    let unique = (schema.indexes().iter())
        .any(|index| index.unique && index.key.iter().all(|k| given(k.id)));
    if whole_key || unique {
        estimate = estimate.min(1.0);
    }
    Ok(Estimate {
        rows: estimate,
        stale,
        joint: groups.len() == 1,
    })
}
