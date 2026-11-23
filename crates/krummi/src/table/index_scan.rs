//! Scans of the secondary indexes of a table (IX8, IX9, IX10 and IX12 of the secondary indexes PRD; document 3 of the
//! table design, *Index structure*): an interval of an index, forward in the index's order or backward in its exact
//! reverse, giving the table's rows or only the index's entries.
//!
//! An interval gives values for the first k key columns of the index (the prefix, from none to all; a null asks for
//! the entries whose column is null) and, when k is less than the number of key columns, an optional lower and upper
//! bound on the next one, as any [`RangeBounds`] of [`Value`]s. The bounds are values in the order of N5 of the
//! column's type, whatever the column's direction in the index: the lower is the smaller value. With a bound, the
//! entries whose column is null are outside the interval; without bounds, inside. The values are normalized and
//! rounded as those of a row, so `-0` finds `+0` and any NaN the stored NaN. An empty or inverted interval gives
//! nothing and no error. Looking up the rows of a value is scanning the interval with that value in every key column.
//!
//! Forward, the entries come in the order of the index's key (N5, with each column's direction and null order), and
//! entries with the same key, nulls included, in the ascending order of their primary key; backward, in the exact
//! reverse.
//!
//! The rows ([`ReadTransaction::index_rows`]) are the table's [`TableRow`](super::TableRow)s, read as a scan of the primary key reads
//! them and bound to a [`Projection`](super::Projection) as they are. The entries
//! ([`ReadTransaction::index_entries`]) are read without reading the rows: each [`IndexEntry`] has the index's key
//! columns, its included columns and the primary key columns that are not in its key, in that order.
//!
//! ```
//! use krummi::table::value::{ColumnType, Value};
//! use krummi::{Column, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions, TableMode};
//!
//! let store = Store::open_temporary(&StoreOptions::default())?;
//! let mut write = store.write()?;
//! let schema = Schema::new(
//!     vec![
//!         Column::new("id", ColumnType::I64).not_null(),
//!         Column::new("city", ColumnType::String),
//!         Column::new("age", ColumnType::I32),
//!     ],
//!     &["id"],
//! );
//! let table = write.open_table_with("people", TableMode::CreateNew(&schema))?;
//! for (id, city, age) in [(1, "Oslo", 30), (2, "Lima", 41), (3, "Oslo", 25), (4, "Oslo", 52)] {
//!     let row = [("id", Value::I64(id)), ("city", Value::String(city.into())), ("age", Value::I32(age))];
//!     write.insert_row(&table, &row)?;
//! }
//! let by_city = IndexDefinition::new("by_city", vec![IndexKey::new("city"), IndexKey::new("age").descending()]);
//! write.create_index(&table, &by_city, IndexMode::CreateNew)?;
//! write.commit()?;
//!
//! let read = store.read()?;
//! let table = read.open_table("people")?;
//! // The people of Oslo aged 30 or more, the oldest first: the rows.
//! let ids: Vec<Value<'static>> = read
//!     .index_rows(&table, "by_city", &[Value::String("Oslo".into())], Value::I32(30)..)?
//!     .map(|row| row.map(|row| row.get("id").unwrap().into_owned()))
//!     .collect::<krummi::Result<_>>()?;
//! assert_eq!(ids, [Value::I64(4), Value::I64(1)]);
//! // The same through the entries, without reading the rows: city, age, then id.
//! let entry = read.index_entries(&table, "by_city", &[Value::String("Oslo".into())], ..)?.next().unwrap()?;
//! assert_eq!(entry.name(1), Some("age"));
//! assert_eq!(entry.value(1), Some(Value::I32(52)));
//! assert_eq!(entry.get("id"), Some(Value::I64(4)));
//! # Ok::<(), krummi::Error>(())
//! ```
//!
//! Scanning is not a change and ends no references (IX13), but the rows and the entries it lends stop being valid at
//! the next operation that may modify the transaction:
//!
//! ```compile_fail
//! # use krummi::table::value::{ColumnType, Value};
//! # use krummi::{Column, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions, TableMode};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null(), Column::new("n", ColumnType::I64)], &["id"]);
//! # let table = write.open_table_with("t", TableMode::CreateNew(&schema)).unwrap();
//! # write.create_index(&table, &IndexDefinition::new("ix", vec![IndexKey::new("n")]), IndexMode::CreateNew).unwrap();
//! let rows = write.index_rows(&table, "ix", &[], ..).unwrap();
//! write.insert_row(&table, &[("id", Value::I64(1))]).unwrap();
//! drop(rows);
//! ```
//!
//! ```compile_fail
//! # use krummi::table::value::{ColumnType, Value};
//! # use krummi::{Column, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions, TableMode};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null(), Column::new("n", ColumnType::I64)], &["id"]);
//! # let table = write.open_table_with("t", TableMode::CreateNew(&schema)).unwrap();
//! # write.create_index(&table, &IndexDefinition::new("ix", vec![IndexKey::new("n")]), IndexMode::CreateNew).unwrap();
//! let entry = write.index_entries(&table, "ix", &[], ..).unwrap().next();
//! write.delete_row(&table, &[Value::I64(1)]).unwrap();
//! drop(entry);
//! ```
//!
//! ```compile_fail
//! # use krummi::table::value::{ColumnType, Value};
//! # use krummi::{Column, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions, TableMode};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null(), Column::new("n", ColumnType::I64)], &["id"]);
//! # let table = write.open_table_with("t", TableMode::CreateNew(&schema)).unwrap();
//! let definition = IndexDefinition::new("ix", vec![IndexKey::new("n")]);
//! # write.create_index(&table, &definition, IndexMode::CreateNew).unwrap();
//! let entries = write.index_entries(&table, "ix", &[], ..).unwrap();
//! write.drop_index(&table, "ix").unwrap();
//! drop(entries);
//! ```

use std::fmt;
use std::ops::{Bound, RangeBounds};
use std::sync::Arc;

use krummi_pager::{Error, Result, SchemaDifference, SizeSubject};
use krummi_tree::PageSource;

use super::catalog::{ReadTable, WriteTable};
use super::index::{EntryLayout, SchemaIndex};
use super::index_key::{decode_entry_value, index_range};
use super::key::{KeyColumn, KeyError, KeyRange, MAX_KEY};
use super::ranges::{ReadIndexRows, Scan, Through, WriteIndexRows, interval};
use super::rows::write_key;
use super::schema::TableSchema;
use super::value::Value;
use crate::{Folder, ReadTransaction, WriteTransaction};

/// The message of a value read from a malformed entry (IX12, L2).
const MALFORMED: &str = "a malformed index entry: a value that does not decode";

/// An entry of an index, read without reading its row (IX8): the values of the index's key columns, in key order, of
/// its included columns, in their order, and of the primary key columns that are not in its key, in key order; each
/// column once, by its position in that order or by its name. A primary key column given among the included columns
/// is not one of them: it comes, once, among the primary key columns.
///
/// The values have the normalization of a row's (N4, E7: a `uuid` in the byte order of RFC 9562 also in a column of
/// SQL Server's order). The included columns are read as a row's: text, bytes, vectors and JSON borrow the
/// transaction's pages without copying them (D11). The values of the key columns and of the primary key are decoded
/// from the entry's key when they are read, each into a copy of its own, text and bytes included, since the key
/// escapes them (IX8's exception to D11). An entry binds to no projection, and it lives until the next operation that
/// may modify its transaction.
///
/// A scan does not check an entry's content (IX12): a malformed entry, in a page whose integrity holds, comes without
/// error, and reading one of its values gives a value that is not the written one or panics, as a malformed row does
/// (L2 of the row reads PRD).
pub struct IndexEntry<'t> {
    columns: Arc<EntryColumns>,
    /// The entry's key: the elements of the index's key columns, then the row's primary key.
    key: &'t [u8],
    /// The entry's value: its included columns in the row format of document 1.
    included: &'t [u8],
}

/// The columns of the entries of a scan, shared by them.
struct EntryColumns {
    layout: EntryLayout,
    /// The elements of an entry's key: the index's key columns, then the primary key columns.
    elements: Vec<KeyColumn>,
    /// The name of each column of an entry, in its order, and where its value is.
    names: Vec<String>,
    places: Vec<Place>,
}

/// Where a column of an entry is: its position among the key columns, the included columns or the primary key.
#[derive(Clone, Copy)]
enum Place {
    Key(usize),
    Included(usize),
    Primary(usize),
}

impl EntryColumns {
    /// The columns of the entries of `index`, an index of `schema`.
    fn new(schema: &TableSchema, index: &SchemaIndex) -> EntryColumns {
        let name = |id: u64| {
            let column = schema.column_by_id(id).expect("a live column of the index");
            column.name.clone()
        };
        let mut names = Vec::new();
        let mut places = Vec::new();
        for (i, key) in index.key.iter().enumerate() {
            names.push(name(key.id));
            places.push(Place::Key(i));
        }
        for (i, column) in index.included.iter().enumerate() {
            names.push(name(column.id));
            places.push(Place::Included(i));
        }
        for (i, id) in schema.key_ids().iter().enumerate() {
            if !index.key.iter().any(|key| key.id == *id) {
                names.push(name(*id));
                places.push(Place::Primary(i));
            }
        }
        let layout = EntryLayout::new(schema, index);
        let primary = layout.primary.iter().map(|ty| KeyColumn::primary(*ty));
        EntryColumns {
            elements: layout.key.iter().copied().chain(primary).collect(),
            layout,
            names,
            places,
        }
    }
}

impl<'t> IndexEntry<'t> {
    /// The number of its columns.
    pub fn len(&self) -> usize {
        self.columns.names.len()
    }

    /// Whether it has no columns, which an entry never has: an index has at least one key column.
    pub fn is_empty(&self) -> bool {
        self.columns.names.is_empty()
    }

    /// The name of its column at `position`.
    pub fn name(&self, position: usize) -> Option<&str> {
        self.columns.names.get(position).map(String::as_str)
    }

    /// The value of its column at `position`: a key or primary key column's decoded into a copy of its own, also text
    /// and bytes; an included column's read as a row's, text, bytes, vectors and JSON borrowing the page.
    ///
    /// # Panics
    ///
    /// For a value of a malformed entry that does not decode (IX12, L2).
    pub fn value(&self, position: usize) -> Option<Value<'t>> {
        Some(match *self.columns.places.get(position)? {
            Place::Key(i) => self.element(i),
            Place::Primary(i) => self.element(self.columns.layout.key.len() + i),
            Place::Included(i) => (self.columns.layout.included.as_ref())
                .expect("an index with included columns")
                .row(self.included)
                .unwrap_or_else(|_| panic!("{MALFORMED}"))
                .read(i),
        })
    }

    /// The value of the element at `at` of its key, decoded into a copy; a `ci` or `ci_ai` text as the row holds
    /// it, from the entry's value, never its folded form (IC5 of the collation PRD).
    fn element(&self, at: usize) -> Value<'t> {
        if let Some(original) = self.columns.layout.original(at, self.included) {
            return original;
        }
        decode_entry_value(&self.columns.elements, self.key, at)
            .unwrap_or_else(|| panic!("{MALFORMED}"))
    }

    /// The value of its column `name`, with the panics of [`IndexEntry::value`].
    pub fn get(&self, name: &str) -> Option<Value<'t>> {
        let position = self.columns.names.iter().position(|n| n == name)?;
        self.value(position)
    }

    /// The values of its columns, in its order.
    pub fn values(&self) -> impl ExactSizeIterator<Item = Value<'t>> + '_ {
        (0..self.len()).map(|i| self.value(i).expect("a column of the entry"))
    }

    /// Its columns' names with their values, in its order.
    pub fn columns(&self) -> impl ExactSizeIterator<Item = (&str, Value<'t>)> + '_ {
        (self.columns.names.iter().map(String::as_str)).zip(self.values())
    }
}

impl fmt::Debug for IndexEntry<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.columns()).finish()
    }
}

/// A scan of the entries of an interval of an index.
struct EntryScan<'t, S: PageSource + ?Sized> {
    entries: krummi_tree::Range<'t, S>,
    columns: Arc<EntryColumns>,
    failed: bool,
}

impl<'t, S: PageSource + ?Sized> EntryScan<'t, S> {
    fn new(tx: &'t S, root: u64, range: &KeyRange, columns: EntryColumns) -> Result<Self> {
        Ok(EntryScan {
            entries: krummi_tree::Range::new(tx, root, interval(range))?,
            columns: Arc::new(columns),
            failed: false,
        })
    }

    /// The entry that `step` takes from an end of the scan, without checking it: its values are decoded when they are
    /// read. After a failure, nothing more from either end.
    #[inline]
    fn entry(
        &mut self,
        step: impl FnOnce(&mut krummi_tree::Range<'t, S>) -> Option<Result<(&'t [u8], &'t [u8])>>,
    ) -> Option<Result<IndexEntry<'t>>> {
        if self.failed {
            return None;
        }
        let entry = step(&mut self.entries)?.map(|(key, included)| IndexEntry {
            columns: Arc::clone(&self.columns),
            key,
            included,
        });
        self.failed = entry.is_err();
        Some(entry)
    }
}

/// A scan of the entries of an interval of an index in a read transaction, from [`ReadTransaction::index_entries`]: a
/// double-ended iterator of [`IndexEntry`]s in the index's order (IX8). It can be sent to another thread, like its
/// transaction. After a failure it returns nothing more from either end.
pub struct ReadIndexEntries<'t>(EntryScan<'t, krummi_tx::ReadTransaction<'t>>);

/// A scan of the entries of an interval of an index in a write transaction, from [`WriteTransaction::index_entries`],
/// like [`ReadIndexEntries`]. It stays on its transaction's thread and ends at the transaction's next operation that
/// may modify it.
pub struct WriteIndexEntries<'t>(EntryScan<'t, krummi_tx::WriteTransaction<'t>>);

macro_rules! entries_iterator {
    ($scan:ident) => {
        impl<'t> Iterator for $scan<'t> {
            type Item = Result<IndexEntry<'t>>;

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                self.0.entry(|entries| entries.next())
            }
        }

        impl DoubleEndedIterator for $scan<'_> {
            #[inline]
            fn next_back(&mut self) -> Option<Self::Item> {
                self.0.entry(|entries| entries.next_back())
            }
        }

        impl std::iter::FusedIterator for $scan<'_> {}
    };
}

entries_iterator!(ReadIndexEntries);
entries_iterator!(WriteIndexEntries);

/// An index of a table as a scan finds it, after the checks of IX10 that read only the catalog: its place among the
/// table's indexes, its key columns and the byte bounds of the interval.
struct Found {
    at: usize,
    key: Vec<KeyColumn>,
    range: KeyRange,
}

impl Folder<'_> {
    /// The error of a scan of the index `index` that `table` does not have (IX9): a different schema.
    fn missing_index(self, table: &str, index: &str) -> Error {
        Error::DifferentIndex {
            folder: self.owned(),
            table: table.to_owned(),
            index: index.to_owned(),
            difference: SchemaDifference::MissingIndex,
            expected: "an index".into(),
            found: "no index".into(),
        }
    }

    /// The index `name` of `table`, whose schema is `schema`, and the interval of `prefix`, `lower` and `upper` on
    /// it (IX9), with the failures of IX10 after the name's size and the handle: a different schema for an index the
    /// table does not have; then an invalid value, with the table and the column when a value breaks it, or the size
    /// of a prefix, with the table and the index (B8).
    fn find_index<'v>(
        self,
        table: &str,
        schema: &TableSchema,
        name: &str,
        prefix: &[Value<'v>],
        lower: Bound<&Value<'v>>,
        upper: Bound<&Value<'v>>,
    ) -> Result<Found> {
        let at = (schema.index_position(name)).ok_or_else(|| self.missing_index(table, name))?;
        let index = &schema.indexes()[at];
        let key = EntryLayout::key_columns(schema, index);
        let range = index_range(&key, prefix, lower, upper).map_err(|error| match error {
            KeyError::Value { column, rule } => Error::InvalidValue {
                json: None,
                folder: self.owned(),
                table: Some(table.to_owned()),
                column: column.map(|k| index.key[k].name.clone()),
                sequence: None,
                rule,
            },
            KeyError::Size { received } => Error::InvalidSize {
                folder: self.owned(),
                what: SizeSubject::Prefix,
                received: received as u64,
                min: 1,
                max: MAX_KEY as u64,
                table: Some(table.into()),
                index: Some(name.into()),
            },
        })?;
        Ok(Found { at, key, range })
    }
}

/// The scans of an index of a read transaction (IX8, IX9). Each fails, in this order (IX10), with
/// [`ErrorKind::InvalidSize`](crate::ErrorKind::InvalidSize) for an index name that does not have 1 to 1,024 bytes;
/// with [`ErrorKind::Misuse`](crate::ErrorKind::Misuse) for a table opened in another transaction; with
/// [`Error::DifferentIndex`] ([`SchemaDifference::MissingIndex`], of the class
/// [`ErrorKind::DifferentSchema`](crate::ErrorKind::DifferentSchema)) for an index the table does not have; with
/// [`ErrorKind::InvalidValue`](crate::ErrorKind::InvalidValue) for a malformed interval (a prefix longer than the key
/// or a bound when the prefix gives every column, with [`ValueRule::InvalidRange`](crate::ValueRule::InvalidRange) and
/// no column; then the first value that fails, the prefix in key order, the lower and the upper bound: a value of
/// another type or out of its domain, a null in the prefix of a column without nulls, and a null bound, with
/// `InvalidRange`, each with its column); with [`ErrorKind::InvalidSize`](crate::ErrorKind::InvalidSize)
/// ([`SizeSubject::Prefix`], with the table and the index) for a prefix, or a prefix with a bound, of more than 2,048
/// bytes encoded as in an entry, even when the interval is empty; and, while scanning, with damage of a page it needs
/// (IX12). Scanning checks neither the entries nor the rows.
impl ReadTransaction<'_> {
    /// A scan of the rows of `table` whose entries in its index `index` are in the interval of `prefix` and `bounds`
    /// (see the module's documentation), as the snapshot has them: a double-ended iterator of
    /// [`TableRow`](super::TableRow)s in the index's order, read as [`range_rows`](Self::range_rows) reads them and bound to a
    /// projection as they are. An entry whose row is not in the table fails with damage of the index page that holds
    /// the entry (IX12).
    pub fn index_rows<'v>(
        &self,
        table: &ReadTable<'_>,
        index: &str,
        prefix: &[Value<'v>],
        bounds: impl RangeBounds<Value<'v>>,
    ) -> Result<ReadIndexRows<'_>> {
        self.folder.check_index_name(index)?;
        self.check_table(table)?;
        let found = self.folder.find_index(
            &table.name,
            &table.schema,
            index,
            prefix,
            bounds.start_bound(),
            bounds.end_bound(),
        )?;
        let reader = &self
            .readers
            .keep(table.history, || Arc::clone(&table.reader))
            .reader;
        let through = Through {
            root: table.index_roots[found.at],
            key: found.key,
        };
        Ok(ReadIndexRows(Scan::new(
            &self.inner,
            reader,
            table.at(),
            &found.range,
            through,
        )?))
    }

    /// A scan of the entries of the index `index` of `table` in the interval of `prefix` and `bounds` (see the module's
    /// documentation), as the snapshot has them, without reading the rows: a double-ended iterator of
    /// [`IndexEntry`]s in the index's order.
    pub fn index_entries<'v>(
        &self,
        table: &ReadTable<'_>,
        index: &str,
        prefix: &[Value<'v>],
        bounds: impl RangeBounds<Value<'v>>,
    ) -> Result<ReadIndexEntries<'_>> {
        self.folder.check_index_name(index)?;
        self.check_table(table)?;
        let found = self.folder.find_index(
            &table.name,
            &table.schema,
            index,
            prefix,
            bounds.start_bound(),
            bounds.end_bound(),
        )?;
        let columns = EntryColumns::new(&table.schema, &table.schema.indexes()[found.at]);
        Ok(ReadIndexEntries(EntryScan::new(
            &self.inner,
            table.index_roots[found.at],
            &found.range,
            columns,
        )?))
    }
}

/// The scans of an index of a write transaction, with this transaction's changes and the failures of the read
/// transaction's; a handle of a deleted table fails with [`ErrorKind::TreeNotFound`](crate::ErrorKind::TreeNotFound)
/// after the index name's size and before the index. They take `&self`, so they are not a change and end no
/// references (IX13); they and what they lend live until the next operation that may modify the transaction. A failure
/// while scanning is the transaction's (T11).
impl WriteTransaction<'_> {
    /// The rows of an interval of an index, as [`ReadTransaction::index_rows`] gives them.
    pub fn index_rows<'v>(
        &self,
        table: &WriteTable<'_>,
        index: &str,
        prefix: &[Value<'v>],
        bounds: impl RangeBounds<Value<'v>>,
    ) -> Result<WriteIndexRows<'_>> {
        self.folder.check_index_name(index)?;
        let state = self.live_table(table)?;
        let found = self.folder.find_index(
            &state.name,
            &state.schema,
            index,
            prefix,
            bounds.start_bound(),
            bounds.end_bound(),
        )?;
        let reader = &self
            .readers
            .keep(write_key(&state.reader), || Arc::clone(&state.reader))
            .reader;
        let through = Through {
            root: state.index_roots[found.at],
            key: found.key,
        };
        Ok(WriteIndexRows(Scan::new(
            &self.inner,
            reader,
            state.at(),
            &found.range,
            through,
        )?))
    }

    /// The entries of an interval of an index, as [`ReadTransaction::index_entries`] gives them.
    pub fn index_entries<'v>(
        &self,
        table: &WriteTable<'_>,
        index: &str,
        prefix: &[Value<'v>],
        bounds: impl RangeBounds<Value<'v>>,
    ) -> Result<WriteIndexEntries<'_>> {
        self.folder.check_index_name(index)?;
        let state = self.live_table(table)?;
        let found = self.folder.find_index(
            &state.name,
            &state.schema,
            index,
            prefix,
            bounds.start_bound(),
            bounds.end_bound(),
        )?;
        let columns = EntryColumns::new(&state.schema, &state.schema.indexes()[found.at]);
        Ok(WriteIndexEntries(EntryScan::new(
            &self.inner,
            state.index_roots[found.at],
            &found.range,
            columns,
        )?))
    }
}
