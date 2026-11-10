//! Rows of a table (E10, E11, E16, E17 and E18 of the tables with schema PRD; document 2 of the table design,
//! *Decided design*): insert, update, insert or update, read and delete by the primary key.
//!
//! A row is given as pairs of column name and [`Value`], with every primary key column; a read row
//! ([`TableRow`]) has every live column in schema order. The rows tree of a table is keyed by the encoded primary key
//! (document 3) and holds each row in the format of document 1, written with the schema version current at that
//! moment; a row of an older version is read through the table's schema history, with null or the default in the
//! columns added after it and without the dropped ones.
//!
//! Every write of a row runs in two phases (document 2): a check phase that changes nothing and finds the failures
//! of public validation in the order of E17 (the sizes of the column names; a handle of a deleted table; a column the
//! table does not have; an invalid value, in E17's three steps; the size of the primary key, then of the row), and
//! then the write, where the page failures and the duplicate key come up. A public validation error leaves the table
//! as it was and the transaction usable (B7); any other failure is the transaction's (T11). Two checks need the
//! existing row and come after reading it: the size of the row that an update, or an insert or update of an existing
//! row, would give (E17's exception); and, for an insert or update of a missing row, a column without nulls nor a
//! default that the row does not give, which is only a failure when it inserts.
//!
//! The writes take `&mut self`, so they end the references read before, also when they fail or change nothing
//! (E16, A10); reading a row and the row count do not:
//!
//! ```
//! use krummi::table::value::{ColumnType, Value};
//! use krummi::{Column, Schema, Store, StoreOptions, TableMode, Upserted};
//!
//! let store = Store::open_temporary(&StoreOptions::default())?;
//! let mut write = store.write()?;
//! let schema = Schema::new(
//!     vec![
//!         Column::new("id", ColumnType::I64).not_null(),
//!         Column::new("n", ColumnType::I32).not_null().with_default(Value::I32(7)),
//!     ],
//!     &["id"],
//! );
//! let table = write.open_table_with("t", TableMode::CreateNew(&schema))?;
//! write.insert_row(&table, &[("id", Value::I64(1))])?;
//! let row = write.get_row(&table, &[Value::I64(1)])?.expect("inserted");
//! assert_eq!(row.get("n"), Some(&Value::I32(7)));
//! assert_eq!(write.row_count(&table)?, 1);
//! drop(row);
//! assert!(write.update_row(&table, &[("id", Value::I64(1)), ("n", Value::I32(8))])?);
//! assert_eq!(write.upsert_row(&table, &[("id", Value::I64(2))])?, Upserted::Inserted);
//! assert!(write.delete_row(&table, &[Value::I64(2)])?);
//! write.commit()?;
//! # Ok::<(), krummi::Error>(())
//! ```
//!
//! A row read before an insert cannot be used after it:
//!
//! ```compile_fail
//! # use krummi::table::value::{ColumnType, Value};
//! # use krummi::{Column, Schema, Store, StoreOptions, TableMode};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
//! # let table = write.open_table_with("t", TableMode::CreateNew(&schema)).unwrap();
//! let row = write.get_row(&table, &[Value::I64(1)]).unwrap();
//! write.insert_row(&table, &[("id", Value::I64(2))]).unwrap();
//! drop(row);
//! ```
//!
//! nor after an update,
//!
//! ```compile_fail
//! # use krummi::table::value::{ColumnType, Value};
//! # use krummi::{Column, Schema, Store, StoreOptions, TableMode};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
//! # let table = write.open_table_with("t", TableMode::CreateNew(&schema)).unwrap();
//! let row = write.get_row(&table, &[Value::I64(1)]).unwrap();
//! write.update_row(&table, &[("id", Value::I64(2))]).unwrap();
//! drop(row);
//! ```
//!
//! an insert or update,
//!
//! ```compile_fail
//! # use krummi::table::value::{ColumnType, Value};
//! # use krummi::{Column, Schema, Store, StoreOptions, TableMode};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
//! # let table = write.open_table_with("t", TableMode::CreateNew(&schema)).unwrap();
//! let row = write.get_row(&table, &[Value::I64(1)]).unwrap();
//! write.upsert_row(&table, &[("id", Value::I64(2))]).unwrap();
//! drop(row);
//! ```
//!
//! or a delete:
//!
//! ```compile_fail
//! # use krummi::table::value::{ColumnType, Value};
//! # use krummi::{Column, Schema, Store, StoreOptions, TableMode};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
//! # let table = write.open_table_with("t", TableMode::CreateNew(&schema)).unwrap();
//! let row = write.get_row(&table, &[Value::I64(1)]).unwrap();
//! write.delete_row(&table, &[Value::I64(2)]).unwrap();
//! drop(row);
//! ```
//!
//! The same holds for the catalog operations and the schema changes, which take `&mut self` too:
//!
//! ```compile_fail
//! # use krummi::table::value::{ColumnType, Value};
//! # use krummi::{Column, Schema, Store, StoreOptions, TableMode};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
//! # let table = write.open_table_with("t", TableMode::CreateNew(&schema)).unwrap();
//! let row = write.get_row(&table, &[Value::I64(1)]).unwrap();
//! write.add_column(&table, &Column::new("x", ColumnType::Bool)).unwrap();
//! drop(row);
//! ```
//!
//! ```compile_fail
//! # use krummi::table::value::{ColumnType, Value};
//! # use krummi::{Column, Schema, Store, StoreOptions, TableMode};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
//! # let table = write.open_table_with("t", TableMode::CreateNew(&schema)).unwrap();
//! let row = write.get_row(&table, &[Value::I64(1)]).unwrap();
//! write.open_table_with("u", TableMode::OpenOrCreate(&schema)).unwrap();
//! drop(row);
//! ```

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::ops::Bound;
use std::sync::Arc;

use krummi_pager::{Error, Result, SchemaDifference, SizeSubject, ValueRule};
use krummi_tree::PageSource;

use super::catalog::{ReadTable, WriteTable};
use super::key::{KeyError, KeyRange, MAX_KEY, decode_key, encode_key, key_range};
use super::row::{MAX_ROW, RowLayout, row_version};
use super::schema::{Different, TableSchema, decode_version, version_key};
use super::value::{ColumnType, Value, key_text, normalize};
use crate::{Folder, ReadTransaction, WriteTransaction};

/// A live column of a table, as its rows need it.
struct LiveColumn {
    id: u64,
    name: String,
    ty: ColumnType,
    nullable: bool,
    /// Its default value, [`Value::Null`] when it has none: what a row that does not give it stores, and what a
    /// row written before it was added reads (N3).
    default: Value<'static>,
    /// Its place in the primary key when it is a fixed-width key column, which the rows do not store: it is read
    /// from the key (document 1).
    in_key: Option<usize>,
}

impl LiveColumn {
    /// The value of the column when a row to insert does not give it: its default, or null; a column without nulls
    /// and without a default fails (E10).
    fn missing(&self) -> std::result::Result<Value<'static>, Fault> {
        match &self.default {
            Value::Null if !self.nullable => Err(self.fault(ValueRule::NullNotAllowed)),
            default => Ok(default.clone()),
        }
    }

    fn fault(&self, rule: ValueRule) -> Fault {
        Fault::Value(Some(self.name.clone()), rule)
    }
}

/// What reading and writing the rows of a table's current schema needs, built once per schema version and shared by
/// the handles of the table and the rows they read.
pub(crate) struct Reader {
    /// The live columns, in schema order.
    columns: Vec<LiveColumn>,
    /// The position in `columns` of each column name.
    names: HashMap<String, usize>,
    /// The positions in `columns` of the primary key columns, in key order.
    key_columns: Vec<usize>,
    /// The types of the primary key columns, in key order.
    key_types: Vec<ColumnType>,
    /// The layout of the rows of the current version, whose columns are `columns`, in the same order.
    layout: RowLayout,
}

/// A failure of public validation of a row or a key, before it knows the table's name and the store's folder.
enum Fault {
    /// A column that the table does not have (E6).
    Unknown(String),
    /// A value that breaks `rule`, in the column when there is one (E17).
    Value(Option<String>, ValueRule),
    /// A primary key or a row with this stored size, over its maximum (E18).
    Size(SizeSubject, usize),
}

impl Fault {
    fn error(self, folder: Folder<'_>, table: &str) -> Error {
        match self {
            Fault::Unknown(column) => folder.different_schema(
                table,
                Different {
                    column: Some(column),
                    difference: SchemaDifference::MissingColumn,
                    expected: "a column".to_owned(),
                    found: "no column".to_owned(),
                },
            ),
            Fault::Value(column, rule) => Error::InvalidValue {
                folder: folder.owned(),
                table: Some(table.to_owned()),
                column,
                sequence: None,
                rule,
            },
            Fault::Size(what, received) => {
                let (min, max) = match what {
                    SizeSubject::PrimaryKey => (1, MAX_KEY),
                    _ => (0, MAX_ROW),
                };
                folder
                    .check_size(what, received, min, max)
                    .expect_err("over its maximum")
            }
        }
    }
}

impl From<KeyError> for Fault {
    fn from(error: KeyError) -> Fault {
        match error {
            KeyError::Size { received } => Fault::Size(SizeSubject::PrimaryKey, received),
            // The values were normalized for their columns before: a value of another type cannot reach the key.
            KeyError::Value { rule, .. } => Fault::Value(None, rule),
        }
    }
}

/// A row given to a write after its check phase (E17): one normalized value per live column, in schema order, `None`
/// for a column it does not give when nothing fills it, and its encoded primary key.
struct Checked<'v> {
    values: Vec<Option<Value<'v>>>,
    key: Vec<u8>,
}

impl Reader {
    pub(crate) fn new(schema: &TableSchema) -> Reader {
        let key = schema.primary_key();
        let columns: Vec<LiveColumn> = schema
            .columns()
            .iter()
            .map(|c| LiveColumn {
                id: c.id,
                name: c.name.clone(),
                ty: c.ty,
                nullable: c.nullable,
                default: c.default.clone().unwrap_or(Value::Null),
                in_key: key
                    .iter()
                    .position(|name| *name == c.name)
                    .filter(|_| c.ty.fixed_width().is_some()),
            })
            .collect();
        let names = columns
            .iter()
            .enumerate()
            .map(|(i, c)| (c.name.clone(), i))
            .collect();
        let key_columns: Vec<usize> = key
            .iter()
            .map(|name| {
                columns
                    .iter()
                    .position(|c| c.name == *name)
                    .expect("a live key column")
            })
            .collect();
        let key_types = key_columns.iter().map(|&i| columns[i].ty).collect();
        Reader {
            columns,
            names,
            key_columns,
            key_types,
            layout: schema.layout(),
        }
    }

    /// The check phase of a write of `row` (E17), which changes nothing: a column the table does not have, the first
    /// in the order given (E6); then the three steps of E17: a repeated column, the first that repeats in the order
    /// given; a missing primary key column, the first in key order; and the first column in schema order that breaks
    /// another rule, with its first rule in E17's order; then the size of the primary key. With `fill`, a column that
    /// the row does not give takes its default or null in that schema order (an insert).
    fn check<'r>(
        &self,
        row: &'r [(&str, Value<'_>)],
        fill: bool,
    ) -> std::result::Result<Checked<'r>, Fault> {
        let mut at = Vec::with_capacity(row.len());
        for (name, _) in row {
            match self.names.get(*name) {
                Some(&i) => at.push(i),
                None => return Err(Fault::Unknown((*name).to_owned())),
            }
        }
        let mut given: Vec<Option<&'r Value<'_>>> = vec![None; self.columns.len()];
        for (&i, (name, value)) in at.iter().zip(row) {
            if given[i].replace(value).is_some() {
                return Err(Fault::Value(
                    Some((*name).to_owned()),
                    ValueRule::RepeatedColumn,
                ));
            }
        }
        if let Some(&i) = self.key_columns.iter().find(|&&i| given[i].is_none()) {
            return Err(self.columns[i].fault(ValueRule::MissingKeyColumn));
        }
        let mut values = Vec::with_capacity(self.columns.len());
        for (column, value) in self.columns.iter().zip(given) {
            values.push(match value {
                Some(value) => Some(
                    normalize(&column.ty, column.nullable, value.reborrow())
                        .map_err(|rule| column.fault(rule))?,
                ),
                None if fill => Some(column.missing()?),
                None => None,
            });
        }
        let key = self.encode_key_of(&values)?;
        Ok(Checked { values, key })
    }

    /// The encoded primary key of `values`, which give every key column.
    fn encode_key_of(&self, values: &[Option<Value<'_>>]) -> std::result::Result<Vec<u8>, Fault> {
        let key: Vec<Value<'_>> = self
            .key_columns
            .iter()
            .map(|&i| values[i].as_ref().expect("a key column").reborrow())
            .collect();
        Ok(encode_key(&self.key_types, &key)?)
    }

    /// The text of the primary key of `values`, one per live column, for the data of a duplicate key (E17).
    fn key_text(&self, values: &[Value<'_>]) -> String {
        let key: Vec<Value<'_>> = self
            .key_columns
            .iter()
            .map(|&i| values[i].reborrow())
            .collect();
        key_text(&key)
    }

    /// `values` with a column the row does not give filled with its default or null, the rules of an insert (E10).
    fn fill<'v>(
        &self,
        values: Vec<Option<Value<'v>>>,
    ) -> std::result::Result<Vec<Value<'v>>, Fault> {
        values
            .into_iter()
            .zip(&self.columns)
            .map(|(value, column)| match value {
                Some(value) => Ok(value),
                None => column.missing(),
            })
            .collect()
    }

    /// The row of `values`, one per live column, in the current version, after checking its size (E18).
    fn encode(&self, values: &[Value<'_>]) -> std::result::Result<Vec<u8>, Fault> {
        let len = self.layout.encoded_len(values);
        if len > MAX_ROW {
            return Err(Fault::Size(SizeSubject::Row, len));
        }
        Ok(self.layout.encode(values))
    }

    /// The encoded primary key of `key`, its values in key order, to read or delete a row: a key with fewer values
    /// than the primary key fails with its first missing column, one with more with a malformed range (E12), and
    /// each value is checked for its column, then the key's size.
    fn lookup_key(&self, key: &[Value<'_>]) -> std::result::Result<Vec<u8>, Fault> {
        if key.len() > self.key_columns.len() {
            return Err(Fault::Value(None, ValueRule::InvalidRange));
        }
        if let Some(&i) = self.key_columns.get(key.len()) {
            return Err(self.columns[i].fault(ValueRule::MissingKeyColumn));
        }
        let values = key
            .iter()
            .zip(&self.key_columns)
            .map(|(value, &i)| {
                let column = &self.columns[i];
                normalize(&column.ty, false, value.reborrow()).map_err(|rule| column.fault(rule))
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(encode_key(&self.key_types, &values)?)
    }

    /// The byte bounds of the range of `prefix` and the bounds `lower` and `upper` on the next key column (E12), as
    /// [`key_range`] gives them, with the failures of public validation named for `table` in `folder`: a value that
    /// breaks its column's rules, with the column; a malformed range; a prefix or bound over the maximum of a key.
    pub(super) fn key_range<'v>(
        &self,
        folder: Folder<'_>,
        table: &str,
        prefix: &[Value<'v>],
        lower: Bound<&Value<'v>>,
        upper: Bound<&Value<'v>>,
    ) -> Result<KeyRange> {
        key_range(&self.key_types, prefix, lower, upper).map_err(|error| {
            let fault = match error {
                KeyError::Value {
                    column: Some(k),
                    rule,
                } => self.columns[self.key_columns[k]].fault(rule),
                error => Fault::from(error),
            };
            fault.error(folder, table)
        })
    }

    /// The row stored under `key` as `bytes` in the rows tree at `rows_root`, with every live column (E11): a column
    /// of the row's version by its place in that version, which the history tree at `history` gives for an older
    /// one, resolved once into `versions` (document 1, *Cost of mixed versions*); a fixed-width key column from the
    /// key; a column added after the row was written, its default or null. Bytes that are not such a row are damage
    /// of the page that holds them (G6).
    pub(super) fn decode<'t, S: PageSource + ?Sized>(
        self: &Arc<Self>,
        tx: &'t S,
        rows_root: u64,
        history: u64,
        versions: &mut Versions,
        key: &[u8],
        bytes: &'t [u8],
    ) -> Result<TableRow<'t>> {
        let damaged = || row_damage(tx, rows_root, key);
        let version = row_version(bytes).map_err(|_| damaged())?;
        let current = self.layout.version();
        let (layout, places) = if version == current {
            (&self.layout, None)
        } else if (1..current).contains(&version) {
            let old = match versions.0.entry(version) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => entry.insert(self.version_layout(tx, history, version)?),
            };
            (&old.layout, Some(&old.places))
        } else {
            return Err(damaged());
        };
        let row = layout.row(bytes).map_err(|_| damaged())?;
        let key_values = decode_key(&self.key_types, key).ok_or_else(damaged)?;
        let mut values = Vec::new();
        krummi_tree::reserve(tx, &mut values, self.columns.len())?;
        for (i, column) in self.columns.iter().enumerate() {
            let value = match column.in_key {
                Some(k) => key_values[k].clone(),
                None => match places.map_or(Some(i), |places| places[i]) {
                    Some(place) => row
                        .value(place)
                        .map_err(|_| damaged())?
                        .ok_or_else(damaged)?,
                    None => column.default.clone(),
                },
            };
            values.push(value);
        }
        Ok(TableRow {
            reader: Arc::clone(self),
            values,
        })
    }

    /// The layout of the rows of `version`, an older one, from the history tree at `history`, and the place in it of
    /// each live column (`None` for a column added after it). A version that is not there, or does not decode, is
    /// damage of the history's page (G6).
    fn version_layout<S: PageSource + ?Sized>(
        &self,
        tx: &S,
        history: u64,
        version: u16,
    ) -> Result<OldVersion> {
        let key = version_key(version);
        let damaged = || match krummi_tree::leaf_of(tx, history, &key) {
            Ok(page) => tx.damaged(page),
            Err(error) => error,
        };
        let bytes = krummi_tree::get(tx, history, &key)?.ok_or_else(damaged)?;
        let (_, columns) = decode_version(bytes).ok_or_else(damaged)?;
        let layout = RowLayout::new(version, columns);
        let at: HashMap<u64, usize> = layout
            .columns()
            .iter()
            .enumerate()
            .map(|(place, c)| (c.id, place))
            .collect();
        let places = self
            .columns
            .iter()
            .map(|c| at.get(&c.id).copied())
            .collect();
        Ok(OldVersion { layout, places })
    }
}

/// The layouts of the older row versions that a read has met, each read from the history tree once: a scan keeps one
/// for all its rows (document 1, *Cost of mixed versions*).
#[derive(Default)]
pub(super) struct Versions(HashMap<u16, OldVersion>);

/// The layout of an older row version, and the place in it of each live column (`None` for one added after it).
struct OldVersion {
    layout: RowLayout,
    places: Vec<Option<usize>>,
}

/// The damage error of the page that holds the row of `key` in the rows tree at `rows_root`: the first page of its value
/// run, or its leaf.
fn row_damage<S: PageSource + ?Sized>(tx: &S, rows_root: u64, key: &[u8]) -> Error {
    let page = krummi_tree::run_of(tx, rows_root, key).and_then(|run| match run {
        Some((first, _)) => Ok(first),
        None => krummi_tree::leaf_of(tx, rows_root, key),
    });
    match page {
        Ok(page) => tx.damaged(page),
        Err(error) => error,
    }
}

/// A row read from a table (E11): every live column in schema order, each by its name or by its position among the
/// live columns, with null or its default in a column added after the row was written; a dropped column never
/// appears. Text, bytes and vectors borrow the transaction's pages without copying them (D11), so the row lives until
/// the next operation that may modify the transaction.
pub struct TableRow<'t> {
    reader: Arc<Reader>,
    values: Vec<Value<'t>>,
}

impl<'t> TableRow<'t> {
    /// The number of live columns.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the row has no columns, which a row never has: a table has at least one column (E4).
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The value of the live column at `position`, in schema order.
    pub fn value(&self, position: usize) -> Option<&Value<'t>> {
        self.values.get(position)
    }

    /// The value of the live column `name`.
    pub fn get(&self, name: &str) -> Option<&Value<'t>> {
        self.reader
            .names
            .get(name)
            .and_then(|&i| self.values.get(i))
    }

    /// The name of the live column at `position`.
    pub fn name(&self, position: usize) -> Option<&str> {
        self.reader.columns.get(position).map(|c| c.name.as_str())
    }

    /// The values of the live columns, in schema order.
    pub fn values(&self) -> &[Value<'t>] {
        &self.values
    }

    /// The live columns' names with their values, in schema order.
    pub fn columns(&self) -> impl Iterator<Item = (&str, &Value<'t>)> {
        self.reader
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .zip(&self.values)
    }

    /// The row with its borrowed values copied, so that it outlives its transaction's next change.
    pub fn into_owned(self) -> TableRow<'static> {
        TableRow {
            reader: self.reader,
            values: self.values.into_iter().map(Value::into_owned).collect(),
        }
    }
}

impl fmt::Debug for TableRow<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.columns()).finish()
    }
}

/// What [`WriteTransaction::upsert_row`] did (E10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Upserted {
    /// The row did not exist: it was inserted.
    Inserted,
    /// The row existed: the columns given were updated, or nothing changed for an identical update.
    Updated,
}

/// A write of a row.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Insert,
    Update,
    Upsert,
}

/// How a write changed the row count of a table.
#[derive(Clone, Copy)]
pub(super) enum RowCount {
    Kept,
    Added,
    Removed(u64),
}

/// What a write of a row did.
#[derive(PartialEq, Eq)]
enum Written {
    Inserted,
    Updated,
    NotFound,
}

impl ReadTransaction<'_> {
    /// The row of `table` whose primary key is `key`, its values in key order, as the snapshot has it, or `None`
    /// (E11). Fails, in this order, with [`ErrorKind::Misuse`](crate::ErrorKind::Misuse) for a table opened in
    /// another transaction; with [`ErrorKind::InvalidValue`](crate::ErrorKind::InvalidValue) for a key that misses
    /// a column, has more values than the key, or a value that breaks its column's rules (a null among them); with
    /// [`ErrorKind::InvalidSize`](crate::ErrorKind::InvalidSize) for a key of more than 2,048 bytes in its stored
    /// form; and with damage of a page it needs (E19).
    pub fn get_row(
        &self,
        table: &ReadTable<'_>,
        key: &[Value<'_>],
    ) -> Result<Option<TableRow<'_>>> {
        self.check_table(table)?;
        let key = table
            .reader
            .lookup_key(key)
            .map_err(|fault| fault.error(self.folder, &table.name))?;
        match krummi_tree::get(&self.inner, table.rows_root, &key)? {
            Some(bytes) => table
                .reader
                .decode(
                    &self.inner,
                    table.rows_root,
                    table.history,
                    &mut Versions::default(),
                    &key,
                    bytes,
                )
                .map(Some),
            None => Ok(None),
        }
    }
}

/// The operations on rows of a write transaction (E10, E11, E16, E17). The failures of public validation
/// ([`ErrorKind::InvalidSize`](crate::ErrorKind::InvalidSize) of a column name, a primary key or a row;
/// [`ErrorKind::TreeNotFound`](crate::ErrorKind::TreeNotFound) for a handle of a deleted table;
/// [`ErrorKind::DifferentSchema`](crate::ErrorKind::DifferentSchema),
/// [`ErrorKind::InvalidValue`](crate::ErrorKind::InvalidValue) and
/// [`ErrorKind::DuplicateKey`](crate::ErrorKind::DuplicateKey); and the misuse of a handle of another transaction)
/// have no effect and do not prevent the commit; any other failure does (T11).
impl WriteTransaction<'_> {
    /// Inserts `row` into `table` (E10): a column the row does not give takes its default or, without one, null.
    /// Fails, after the checks of E17 (see the module's documentation), with
    /// [`ErrorKind::DuplicateKey`](crate::ErrorKind::DuplicateKey), naming the table and the key's values, if the
    /// primary key exists, even when the row is identical.
    pub fn insert_row(&mut self, table: &WriteTable<'_>, row: &[(&str, Value<'_>)]) -> Result<()> {
        self.write_row(table, row, Op::Insert).map(|_| ())
    }

    /// Updates the row of `table` with the primary key of `row`, changing only the columns `row` gives (E10), and
    /// returns whether the row was there: a missing row changes nothing. An update that leaves the row as it reads
    /// is identical and writes nothing. The row is written in the current schema version. The size of the row it
    /// would give is checked after reading the row (E17).
    pub fn update_row(
        &mut self,
        table: &WriteTable<'_>,
        row: &[(&str, Value<'_>)],
    ) -> Result<bool> {
        Ok(self.write_row(table, row, Op::Update)? != Written::NotFound)
    }

    /// Updates the row of `table` with the primary key of `row` as [`update_row`](Self::update_row) does if it is
    /// there, and otherwise inserts it as [`insert_row`](Self::insert_row) does, and says which (E10).
    pub fn upsert_row(
        &mut self,
        table: &WriteTable<'_>,
        row: &[(&str, Value<'_>)],
    ) -> Result<Upserted> {
        Ok(match self.write_row(table, row, Op::Upsert)? {
            Written::Inserted => Upserted::Inserted,
            _ => Upserted::Updated,
        })
    }

    /// The row of `table` whose primary key is `key`, with this transaction's changes, or `None`, with the failures
    /// of [`ReadTransaction::get_row`] (E11); a handle of a deleted table fails with
    /// [`ErrorKind::TreeNotFound`](crate::ErrorKind::TreeNotFound). It lives until the next operation that may
    /// modify the transaction.
    pub fn get_row(
        &self,
        table: &WriteTable<'_>,
        key: &[Value<'_>],
    ) -> Result<Option<TableRow<'_>>> {
        let (rows_root, history, reader) = self.live_rows(table)?;
        let key = reader
            .lookup_key(key)
            .map_err(|fault| fault.error(self.folder, &self.table_name(table)))?;
        match krummi_tree::get(&self.inner, rows_root, &key)? {
            Some(bytes) => reader
                .decode(
                    &self.inner,
                    rows_root,
                    history,
                    &mut Versions::default(),
                    &key,
                    bytes,
                )
                .map(Some),
            None => Ok(None),
        }
    }

    /// Deletes the row of `table` whose primary key is `key`, and returns whether it was there (E11), with the
    /// failures of the key of [`get_row`](Self::get_row).
    pub fn delete_row(&mut self, table: &WriteTable<'_>, key: &[Value<'_>]) -> Result<bool> {
        let (rows_root, _, reader) = self.live_rows(table)?;
        let key = reader
            .lookup_key(key)
            .map_err(|fault| fault.error(self.folder, &self.table_name(table)))?;
        let update = krummi_tree::remove(&mut self.inner, rows_root, &key)?;
        let removed = u64::from(update.count_changed);
        self.apply_rows(table, update.root, RowCount::Removed(removed))?;
        Ok(update.count_changed)
    }

    /// The two phases of a write of `row` (document 2): the check phase, which changes nothing, then the write. An
    /// insert, and an insert or update of a missing row, add the row with the columns it does not give filled (for an
    /// insert, the check phase filled them already); an update, and an insert or update of a row that is there, merge
    /// it with the stored row.
    fn write_row(
        &mut self,
        table: &WriteTable<'_>,
        row: &[(&str, Value<'_>)],
        op: Op,
    ) -> Result<Written> {
        for (name, _) in row {
            self.folder.check_column_name(name)?;
        }
        let (rows_root, history, reader) = self.live_rows(table)?;
        let fail = |tx: &Self, fault: Fault| fault.error(tx.folder, &tx.table_name(table));
        let Checked { values, key } = reader
            .check(row, op == Op::Insert)
            .map_err(|fault| fail(self, fault))?;
        let stored = match op {
            // The insert learns whether the key is there from the tree's own addition.
            Op::Insert => None,
            Op::Update | Op::Upsert => krummi_tree::get(&self.inner, rows_root, &key)?,
        };
        let Some(bytes) = stored else {
            if op == Op::Update {
                return Ok(Written::NotFound);
            }
            let values = reader.fill(values).map_err(|fault| fail(self, fault))?;
            let encoded = reader.encode(&values).map_err(|fault| fail(self, fault))?;
            let update = krummi_tree::try_add(&mut self.inner, rows_root, &key, &encoded)?;
            if !update.count_changed {
                return Err(Error::DuplicateKey {
                    folder: self.folder.owned(),
                    table: self.table_name(table),
                    key: reader.key_text(&values),
                });
            }
            self.apply_rows(table, update.root, RowCount::Added)?;
            return Ok(Written::Inserted);
        };
        let old = reader.decode(
            &self.inner,
            rows_root,
            history,
            &mut Versions::default(),
            &key,
            bytes,
        )?;
        let merged: Vec<Value<'_>> = values
            .into_iter()
            .zip(old.values())
            .map(|(new, old)| new.unwrap_or_else(|| old.reborrow()))
            .collect();
        if merged
            .iter()
            .zip(old.values())
            .all(|(new, old)| new.same(old))
        {
            return Ok(Written::Updated);
        }
        let encoded = reader.encode(&merged).map_err(|fault| fail(self, fault))?;
        let update = krummi_tree::set(&mut self.inner, rows_root, &key, &encoded)?;
        self.apply_rows(table, update.root, RowCount::Kept)?;
        Ok(Written::Updated)
    }

    /// The rows tree, the history tree and the reader of the table of `table`, with the failures of a handle (misuse
    /// for one of another transaction, not found once the table was deleted), without reading anything.
    pub(super) fn live_rows(&self, table: &WriteTable<'_>) -> Result<(u64, u64, Arc<Reader>)> {
        let state = self.live_table(table)?;
        Ok((state.rows_root, state.history, Arc::clone(&state.reader)))
    }

    /// The name of the table of `table`, for the data of an error.
    pub(super) fn table_name(&self, table: &WriteTable<'_>) -> String {
        self.tables
            .borrow()
            .get(table.id)
            .expect("a live handle")
            .name
            .clone()
    }

    /// Records what a write did to the rows tree of `table`: its new `root` and the change of its row count. A
    /// removal of more rows than the count says is damage of the catalog page that holds the table's entry, whose
    /// count it read (G6), and the transaction's failure (T11).
    pub(super) fn apply_rows(
        &mut self,
        table: &WriteTable<'_>,
        root: u64,
        change: RowCount,
    ) -> Result<()> {
        let state = self
            .tables
            .get_mut()
            .get_mut(table.id)
            .expect("a live handle");
        let count = match change {
            RowCount::Kept => Some(state.count),
            RowCount::Added => state.count.checked_add(1),
            RowCount::Removed(n) => state.count.checked_sub(n),
        };
        let Some(count) = count else {
            return Err(self.count_damage(table));
        };
        if count != state.count || root != state.rows_root {
            state.count = count;
            state.rows_root = root;
            state.dirty = true;
        }
        Ok(())
    }

    /// The damage of the catalog page that holds the entry of the table of `table`, as the transaction's failure.
    fn count_damage(&self, table: &WriteTable<'_>) -> Error {
        match krummi_tree::catalog::lookup_with_extra(&self.inner, &self.table_name(table)) {
            Ok(Some(located)) => self.inner.damaged(located.page),
            Ok(None) => self.inner.fail(Error::Damaged {
                folder: self.folder.owned(),
                page: None,
            }),
            Err(error) => error,
        }
    }
}
