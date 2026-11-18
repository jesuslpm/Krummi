//! The secondary indexes of a table kept in step with its rows in every row write (IX7, IX10 and IX12 of the secondary
//! indexes PRD; document 3 of the table design, *Index structure* and *Unique index*): insert, update, insert or
//! update, delete, range delete and the bulk load, which inserts.
//!
//! A row write on a table with indexes plans what it does to each index before it changes anything: the entry of each
//! index for the row it writes (whose size it checks, IX10), and, for a unique index whose entry changes, a probe of the
//! entries with the same key columns, which decides with the entries alone, without reading the other row (IX12).
//! Only then does it write the row and each index's entry. An identical update (E10) plans nothing; an update that
//! changes no column of an index leaves that index as it was. A delete reads the row it removes without checking its
//! content (L1) to find its entries, and an entry it must remove that is not there is damage of the index page where it
//! should be (T11).

use std::borrow::Cow;
use std::ops::Bound::Included;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;

use krummi_pager::{Error, Result};
use krummi_tree::Interval;

use super::catalog::WriteTable;
use super::index::{EntryLayout, RowEntry, TooLarge};
use super::key::KeyRange;
use super::ranges::interval;
use super::rows::{Reader, RowCount, Versions};
use super::schema::TableSchema;
use super::value::Value;
use crate::WriteTransaction;

/// The rows a range delete of a table with indexes reads before it removes them and their entries, so that it never
/// holds the entries of more rows than these.
#[cfg(not(test))]
const RANGE_BATCH: usize = 1024;
/// Fewer in the crate's tests, so that a range of a few rows takes several batches.
#[cfg(test)]
const RANGE_BATCH: usize = 4;

/// The indexes of a table as its row writes keep them, in the order of their names (B5).
pub(crate) struct Maintained(Vec<Index>);

/// An index of a table as its row writes keep it.
struct Index {
    name: String,
    unique: bool,
    layout: EntryLayout,
}

impl Maintained {
    /// The indexes of `schema`.
    pub(super) fn new(schema: &TableSchema) -> Maintained {
        Maintained(
            (schema.indexes().iter())
                .map(|index| Index {
                    name: index.name.clone(),
                    unique: index.unique,
                    layout: EntryLayout::new(schema, index),
                })
                .collect(),
        )
    }
}

/// What a row write does to one index.
pub(super) enum Step {
    /// Nothing: the entry stays as it is.
    Keep,
    /// Adds the entry of a new row.
    Add(RowEntry),
    /// Replaces the entry of an updated row, whose key columns changed, with another.
    Move { old: Vec<u8>, new: RowEntry },
    /// Rewrites the value of the entry of an updated row, whose included columns changed.
    Rewrite(RowEntry),
    /// Removes the entry of a deleted row.
    Remove(Vec<u8>),
}

impl WriteTransaction<'_> {
    /// How the row writes of `table` keep its indexes, or `None` when it has none.
    pub(super) fn maintained(&mut self, table: &WriteTable<'_>) -> Option<Arc<Maintained>> {
        let state = self
            .tables
            .get_mut()
            .get_mut(table.id)
            .expect("a live handle");
        if state.index_roots.is_empty() {
            return None;
        }
        let schema = &state.schema;
        Some(Arc::clone(
            state
                .maintained
                .get_or_insert_with(|| Arc::new(Maintained::new(schema))),
        ))
    }

    /// The steps that inserting the row of `key` with `values`, one per live column, takes in the indexes `indexes`
    /// of `table`, after the checks that come before changing anything (IX7, IX10): the size of each entry, in the
    /// order of the indexes' names; with `check_key`, a primary key that the rows tree at `rows_root` has (an insert,
    /// whose tree addition would find it only after the unique indexes were checked; without a unique index, that
    /// addition finds it first, before anything changes); then each unique index that already has the row's key
    /// columns, in the order of their names. `reader` gives the text of the primary key.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn insert_steps(
        &self,
        table: &WriteTable<'_>,
        indexes: &Maintained,
        reader: &Reader,
        rows_root: u64,
        values: &[Value<'_>],
        key: &[u8],
        check_key: bool,
    ) -> Result<Vec<Step>> {
        let value = |i: usize| values[i].reborrow();
        let steps = (indexes.0.iter())
            .map(|index| {
                (index.layout.entry(value, key))
                    .map(Step::Add)
                    .map_err(|fault| self.entry_too_large(table, &index.name, fault))
            })
            .collect::<Result<Vec<_>>>()?;
        if check_key
            && indexes.0.iter().any(|index| index.unique)
            && !self.beyond_greatest(table, key)
            && krummi_tree::get(&self.inner, rows_root, key)?.is_some()
        {
            return Err(Error::DuplicateKey {
                folder: self.folder.owned(),
                table: self.table_name(table),
                key: reader.key_text(values),
                index: None,
                index_key: None,
            });
        }
        self.check_unique_steps(table, indexes, &value, &steps)?;
        Ok(steps)
    }

    /// The steps that updating the row of `key` from the values `old(i)` to `new(i)` of its live columns takes in the
    /// indexes `indexes` of `table`, after the checks that come before changing anything (IX7, IX10): the size of each
    /// new entry, in the order of the indexes' names, then each unique index whose key columns change to values that
    /// another row has. An index whose entry does not change keeps it.
    pub(super) fn update_steps<'o, 'n>(
        &self,
        table: &WriteTable<'_>,
        indexes: &Maintained,
        old: impl Fn(usize) -> Value<'o>,
        new: impl Fn(usize) -> Value<'n>,
        key: &[u8],
    ) -> Result<Vec<Step>> {
        let steps = (indexes.0.iter())
            .map(|index| {
                let new = (index.layout.entry(&new, key))
                    .map_err(|fault| self.entry_too_large(table, &index.name, fault))?;
                // The row was checked (L3) and its entry was checked when it was written.
                let Ok(old) = index.layout.entry(&old, key) else {
                    panic!("a stored row whose entry its index cannot hold");
                };
                Ok(if old.key != new.key {
                    Step::Move { old: old.key, new }
                } else if old.value != new.value {
                    Step::Rewrite(new)
                } else {
                    Step::Keep
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.check_unique_steps(table, indexes, &new, &steps)?;
        Ok(steps)
    }

    /// Fails with the duplicate key of the first unique index of `indexes`, in the order of their names, where a new
    /// entry of `steps`, of the row whose live column at place `i` has `value(i)`, has key columns that an entry of
    /// the index already has, none of them null (N7). The entries decide, without reading the other row (IX12).
    fn check_unique_steps<'v>(
        &self,
        table: &WriteTable<'_>,
        indexes: &Maintained,
        value: &impl Fn(usize) -> Value<'v>,
        steps: &[Step],
    ) -> Result<()> {
        for (at, (index, step)) in indexes.0.iter().zip(steps).enumerate() {
            let (Step::Add(entry) | Step::Move { new: entry, .. }) = step else {
                continue;
            };
            if !index.unique {
                continue;
            }
            let Some(range) = index.layout.collisions(value, &entry.key) else {
                continue;
            };
            if self.has_entry(self.index_root(table, at), &range)? {
                return Err((self.folder).index_duplicate(
                    &self.table_name(table),
                    &index.name,
                    &index.layout,
                    &entry.key,
                ));
            }
        }
        Ok(())
    }

    /// The error of an entry of the index `index` of `table` that passes its maximum (IX10).
    fn entry_too_large(&self, table: &WriteTable<'_>, index: &str, fault: TooLarge) -> Error {
        (self.folder).entry_too_large(&self.table_name(table), index, fault)
    }

    /// Whether the index tree at `root` has an entry in `range`.
    fn has_entry(&self, root: u64, range: &KeyRange) -> Result<bool> {
        let mut entries = krummi_tree::Range::new(&self.inner, root, interval(range))?;
        Ok(entries.next().transpose()?.is_some())
    }

    /// Whether `key` is greater than every key of the rows of `table`, as far as the write knows them, so that the
    /// table does not have it.
    fn beyond_greatest(&self, table: &WriteTable<'_>, key: &[u8]) -> bool {
        let tables = self.tables.borrow();
        let state = tables.get(table.id).expect("a live handle");
        state
            .greatest
            .as_deref()
            .is_some_and(|greatest| key > greatest)
    }

    /// The steps that deleting the row of `key`, stored as `bytes`, takes in the indexes `indexes` of `table`: the
    /// removal of each of its entries. The row is read without checking its content (L1, IX12): the damage that the
    /// reads keep fails here, and a malformed row may give entries that are not those of its written values, or panic,
    /// before anything changes.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn delete_steps<'t>(
        &'t self,
        table: &WriteTable<'_>,
        indexes: &Maintained,
        reader: &'t Arc<Reader>,
        versions: &mut Versions<'t>,
        key: &'t [u8],
        bytes: &'t [u8],
    ) -> Result<Vec<Step>> {
        let state = self.live_table(table)?;
        let row = reader.decode(
            &self.inner,
            state.at(),
            versions,
            Cow::Borrowed(key),
            bytes,
            false,
        )?;
        let value = |i| row.value(i).expect("a live column");
        Ok((indexes.0.iter())
            .map(|index| Step::Remove(index.layout.entry_key(value, key)))
            .collect())
    }

    /// Takes `steps`, one per index of `table` in their order, in its indexes' trees. An entry to remove that is not
    /// there, or one to add that is, means the index is not in step with the rows: damage of the index page where the
    /// entry should, or should not, be (IX12, T11).
    pub(super) fn apply_steps(&mut self, table: &WriteTable<'_>, steps: Vec<Step>) -> Result<()> {
        for (at, step) in steps.into_iter().enumerate() {
            match step {
                Step::Keep => {}
                Step::Add(entry) => self.add_entry(table, at, &entry)?,
                Step::Move { old, new } => {
                    self.remove_entry(table, at, &old)?;
                    self.add_entry(table, at, &new)?;
                }
                Step::Rewrite(entry) => {
                    let root = self.index_root(table, at);
                    let update = krummi_tree::set(&mut self.inner, root, &entry.key, &entry.value)?;
                    self.set_index_root(table, at, update.root);
                    if update.count_changed {
                        return Err(self.entry_damage(update.root, &entry.key));
                    }
                }
                Step::Remove(key) => self.remove_entry(table, at, &key)?,
            }
        }
        Ok(())
    }

    fn add_entry(&mut self, table: &WriteTable<'_>, at: usize, entry: &RowEntry) -> Result<()> {
        let root = self.index_root(table, at);
        let update = krummi_tree::try_add(&mut self.inner, root, &entry.key, &entry.value)?;
        self.set_index_root(table, at, update.root);
        if !update.count_changed {
            return Err(self.entry_damage(update.root, &entry.key));
        }
        Ok(())
    }

    fn remove_entry(&mut self, table: &WriteTable<'_>, at: usize, key: &[u8]) -> Result<()> {
        let root = self.index_root(table, at);
        let update = krummi_tree::remove(&mut self.inner, root, key)?;
        self.set_index_root(table, at, update.root);
        if !update.count_changed {
            return Err(self.entry_damage(update.root, key));
        }
        Ok(())
    }

    /// The damage of the leaf of the index tree at `root` where the entry `key` is or would be, as the transaction's
    /// failure.
    fn entry_damage(&self, root: u64, key: &[u8]) -> Error {
        match krummi_tree::leaf_of(&self.inner, root, key) {
            Ok(page) => self.inner.damaged(page),
            Err(error) => error,
        }
    }

    fn index_root(&self, table: &WriteTable<'_>, at: usize) -> u64 {
        self.tables
            .borrow()
            .get(table.id)
            .expect("a live handle")
            .index_roots[at]
    }

    fn set_index_root(&mut self, table: &WriteTable<'_>, at: usize, root: u64) {
        let state = self
            .tables
            .get_mut()
            .get_mut(table.id)
            .expect("a live handle");
        if state.index_roots[at] != root {
            state.index_roots[at] = root;
            state.dirty = true;
        }
    }

    /// Deletes the rows of `range` of `table`, whose indexes are `indexes`, with their entries, and returns how many it
    /// deleted (E12, IX7). It reads at most [`RANGE_BATCH`] rows at a time, without checking their content (L1, IX12),
    /// then removes them and their entries, until the range has no rows. A panic while reading a row (a malformed one,
    /// L2) that comes after some rows were removed leaves the write unable to commit, as damage does (T11; IX12's
    /// exception to L2): the panic goes on, and the commit fails.
    pub(super) fn delete_indexed_range(
        &mut self,
        table: &WriteTable<'_>,
        reader: &Arc<Reader>,
        indexes: &Maintained,
        range: &KeyRange,
    ) -> Result<u64> {
        let mut changed = false;
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            self.delete_batches(table, reader, indexes, range, &mut changed)
        }));
        match outcome {
            Ok(result) => result,
            Err(payload) => {
                if changed {
                    self.inner.fail(Error::Damaged {
                        folder: self.folder.owned(),
                        page: None,
                    });
                }
                panic::resume_unwind(payload)
            }
        }
    }

    /// The batches of [`WriteTransaction::delete_indexed_range`]; `changed` turns true before the first change.
    fn delete_batches(
        &mut self,
        table: &WriteTable<'_>,
        reader: &Arc<Reader>,
        indexes: &Maintained,
        range: &KeyRange,
        changed: &mut bool,
    ) -> Result<u64> {
        let mut deleted = 0;
        loop {
            let mut keys = Vec::new();
            let mut steps = Vec::new();
            {
                let rows_root = self.live_table(table)?.rows_root;
                let mut versions = Versions::default();
                let rows = krummi_tree::Range::new(&self.inner, rows_root, interval(range))?;
                for row in rows.take(RANGE_BATCH) {
                    let (key, bytes) = row?;
                    steps.push(self.delete_steps(
                        table,
                        indexes,
                        reader,
                        &mut versions,
                        key,
                        bytes,
                    )?);
                    keys.push(key.to_vec());
                }
            }
            let (Some(first), Some(last)) = (keys.first(), keys.last()) else {
                return Ok(deleted);
            };
            *changed = true;
            // The batch is the first rows of the range, in key order: every row from its first key to its last.
            let rows_root = self.live_table(table)?.rows_root;
            let batch = Interval {
                prefix: &[],
                lower: Included(first),
                upper: Included(last),
            };
            let update = krummi_tree::remove_range(&mut self.inner, rows_root, batch)?;
            self.apply_rows(table, update.root, RowCount::Removed(update.removed))?;
            deleted += update.removed;
            for row in steps {
                self.apply_steps(table, row)?;
            }
            if keys.len() < RANGE_BATCH {
                return Ok(deleted);
            }
        }
    }
}
