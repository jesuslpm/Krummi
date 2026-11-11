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
//! assert_eq!(row.get("n"), Some(Value::I32(7)));
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

use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Bound;
use std::ops::Deref;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use krummi_pager::{Error, Result, SchemaDifference, SizeSubject, ValueRule};
use krummi_tree::PageSource;

use super::catalog::{ReadTable, WriteTable};
use super::key::{
    KeyError, KeyRange, MAX_KEY, encode_key, is_key, key_range, key_starts, key_value,
};
use super::row::{MAX_ROW, RowLayout, Slot, row_version};
use super::schema::{Different, TableSchema, decode_defaults, decode_version, version_key};
use super::value::{ColumnType, NANOS_PER_MICRO, Value, key_text, normalize};
use crate::ids::{Ulid, Uuid};
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
    /// Where each live column is in the rows of the current version, for the typed reads.
    slots: Vec<Slot>,
    /// The live columns that the check of a row of the current version reads, those it can reject, with their types.
    checks: Vec<(Slot, ColumnType)>,
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
        let layout = schema.layout();
        let slots: Vec<Slot> = (0..columns.len())
            .map(|i| layout.slot(i).expect("a column of the current version"))
            .collect();
        let checks = (0..columns.len())
            .filter(|&i| layout.needs_check(i))
            .map(|i| (slots[i], columns[i].ty))
            .collect();
        Reader {
            columns,
            names,
            key_columns,
            key_types,
            layout,
            slots,
            checks,
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

    /// The row stored under `key` as `bytes` in the rows tree of `table`, with every live column (E11): a column of
    /// the row's version by its place in that version, which the history tree gives for an older one, resolved once
    /// into `versions` (document 1, *Cost of mixed versions*); a fixed-width key column from the key; a column added
    /// after the row was written, null or its default, a text, bytes or vector default borrowed from the table's
    /// catalog entry (D11). The whole row is checked here, and its values are decoded when they are read: bytes that
    /// are not such a row are damage of the page that holds them (G6), and reading a column of the row cannot fail.
    pub(super) fn decode<'t, S: PageSource + ?Sized>(
        self: &'t Arc<Self>,
        tx: &'t S,
        table: At<'_>,
        versions: &mut Versions<'t>,
        key: Cow<'t, [u8]>,
        bytes: &'t [u8],
        leaf_passed: bool,
    ) -> Result<TableRow<'t>> {
        let damaged = || row_damage(tx, table.rows_root, &key);
        let version = row_version(bytes).map_err(|_| damaged())?;
        let current = self.layout.version();
        let old =
            if version == current {
                None
            } else if (1..current).contains(&version) {
                Some(match versions.old.entry(version) {
                    Entry::Occupied(entry) => Arc::clone(entry.get()),
                    Entry::Vacant(entry) => Arc::clone(
                        entry.insert(Arc::new(self.version_layout(tx, table.history, version)?)),
                    ),
                })
            } else {
                return Err(damaged());
            };
        let layout = old.as_ref().map_or(&self.layout, |old| &old.layout);
        let row = layout.row(bytes).map_err(|_| damaged())?;
        // A key of one column starts at 0; a longer one keeps where each column starts, so that reading every column
        // walks it once.
        let starts = if self.key_types.len() == 1 {
            if !(leaf_passed && old.is_none()) && !is_key(&self.key_types, &key) {
                return Err(damaged());
            }
            None
        } else {
            Some(key_starts(&self.key_types, &key).ok_or_else(damaged)?)
        };
        let mut defaults = None;
        let Some(old_version) = &old else {
            // The current version: only the columns the check can reject, every live column being in its rows.
            if !leaf_passed {
                for (slot, ty) in &self.checks {
                    row.check_slot(*slot, ty).map_err(|_| damaged())?;
                }
            }
            return Ok(self.stored_row(key, starts, bytes, None, None));
        };
        for (i, column) in self.columns.iter().enumerate() {
            if column.in_key.is_some() {
                continue;
            }
            match old_version.places[i] {
                Some(place) => row.check(place).map_err(|_| damaged())?,
                None if needs_stored(&column.default) && defaults.is_none() => {
                    defaults = Some(match &versions.defaults {
                        Some(stored) => Arc::clone(stored),
                        None => Arc::clone(
                            versions
                                .defaults
                                .insert(Arc::new(self.stored_defaults(tx, table.name)?)),
                        ),
                    });
                }
                None => {}
            }
        }
        Ok(self.stored_row(key, starts, bytes, old, defaults))
    }

    /// A row that [`Reader::decode`] checked, borrowing this reader.
    fn stored_row<'t>(
        self: &'t Arc<Self>,
        key: Cow<'t, [u8]>,
        starts: Option<Vec<u16>>,
        bytes: &'t [u8],
        old: Option<Arc<OldVersion>>,
        defaults: Option<Arc<Vec<Value<'t>>>>,
    ) -> TableRow<'t> {
        TableRow {
            reader: RowReader::Kept(self),
            stored: Stored::Row {
                key,
                starts,
                bytes,
                old,
                defaults,
            },
        }
    }

    /// Whether `key` and `bytes` are a row of the current version that [`Reader::decode`] accepts.
    fn check_current(&self, key: &[u8], bytes: &[u8]) -> bool {
        let Ok(version) = row_version(bytes) else {
            return false;
        };
        let Ok(row) = self.layout.row(bytes) else {
            return false;
        };
        let key_valid = if self.key_types.len() == 1 {
            is_key(&self.key_types, key)
        } else {
            key_starts(&self.key_types, key).is_some()
        };
        version == self.layout.version()
            && key_valid
            && self
                .checks
                .iter()
                .all(|(slot, ty)| row.check_slot(*slot, ty).is_ok())
    }

    /// The value of the live column `i` of the row `key`, `bytes`, which [`Reader::decode`] checked.
    #[inline]
    fn stored_value<'t>(
        &self,
        key: &[u8],
        starts: Option<&[u16]>,
        bytes: &'t [u8],
        old: Option<&OldVersion>,
        defaults: Option<&[Value<'t>]>,
        i: usize,
    ) -> Value<'t> {
        let column = &self.columns[i];
        if let Some(k) = column.in_key {
            let start = starts.map_or(0, |starts| usize::from(starts[k]));
            return key_value(&self.key_types[k], &key[start..]);
        }
        let (layout, place) = match old {
            None => (&self.layout, Some(i)),
            Some(old) => (&old.layout, old.places[i]),
        };
        match place {
            Some(place) => layout
                .row(bytes)
                .expect("a checked row")
                .checked_value(place),
            None if needs_stored(&column.default) => {
                defaults.expect("the stored defaults of a checked row")[i].clone()
            }
            None => column.default.clone(),
        }
    }

    /// The default of each live column, [`Value::Null`] for one without, borrowed from the catalog entry `name` of
    /// the table, whose schema is this reader's (D11, AC20). An entry that is not there or does not hold this schema is
    /// damage of the catalog page that should hold it (G6).
    fn stored_defaults<'t, S: PageSource + ?Sized>(
        &self,
        tx: &'t S,
        name: &str,
    ) -> Result<Vec<Value<'t>>> {
        let Some(located) = krummi_tree::catalog::lookup_with_extra(tx, name)? else {
            let leaf = tx
                .slot(krummi_tree::catalog::ROOT_SLOT)
                .and_then(|root| krummi_tree::leaf_of(tx, root, name.as_bytes()));
            return Err(match leaf {
                Ok(page) => tx.damaged(page),
                Err(error) => error,
            });
        };
        let stored = decode_defaults(located.extra)
            .filter(|stored| {
                stored.len() == self.columns.len()
                    && stored.iter().zip(&self.columns).all(|((id, default), c)| {
                        *id == c.id && default.is_some() != matches!(c.default, Value::Null)
                    })
            })
            .ok_or_else(|| tx.damaged(located.page))?;
        Ok(stored
            .into_iter()
            .map(|(_, default)| default.unwrap_or(Value::Null))
            .collect())
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

/// What a read of rows resolves once for all its rows (document 1, *Cost of mixed versions*): the layouts of the older
/// row versions it has met, each read from the history tree once, and the defaults of the live columns borrowed from
/// the table's catalog entry, read when the first older row needs a text, bytes or vector default. A scan keeps one
/// for all its rows.
#[derive(Default)]
pub(super) struct Versions<'t> {
    old: HashMap<u16, Arc<OldVersion>>,
    defaults: Option<Arc<Vec<Value<'t>>>>,
}

/// Whether a default is borrowed from the table's catalog entry (D11) rather than kept by the reader: a text, bytes or
/// vector one.
fn needs_stored(default: &Value<'_>) -> bool {
    matches!(
        default,
        Value::String(_) | Value::Bytes(_) | Value::Vector(_)
    )
}

/// The readers of the tables whose rows a transaction reads, kept until it ends, so that its rows borrow their reader
/// instead of counting a reference to it, which costs two atomic operations per row. An append-only list, each reader
/// under a key: a read transaction keeps one per table, by the root of its history tree, since its snapshot does not
/// change; a write transaction keeps one per schema a table had in it, by its reader's address. A kept reader never
/// moves, so a reference to it lives as long as the transaction.
#[derive(Default)]
pub(crate) struct Readers {
    head: OnceLock<Box<Kept>>,
}

/// A reader of [`Readers`], with its key, and the next one.
struct Kept {
    key: u64,
    reader: Arc<Reader>,
    next: OnceLock<Box<Kept>>,
}

impl Readers {
    /// The reader kept under `key`, or `make()` kept under it.
    pub(crate) fn keep(&self, key: u64, make: impl FnOnce() -> Arc<Reader>) -> &Arc<Reader> {
        let mut make = Some(make);
        let mut made = None;
        let mut slot = &self.head;
        loop {
            match slot.get() {
                Some(kept) if kept.key == key => return &kept.reader,
                Some(kept) => slot = &kept.next,
                None => {
                    let reader = made
                        .take()
                        .unwrap_or_else(|| make.take().expect("made once")());
                    // Another thread of a shared read transaction may have kept one first: the loop then reads it,
                    // and keeps this one further on if it is under another key.
                    if let Err(not_kept) = slot.set(Box::new(Kept {
                        key,
                        reader,
                        next: OnceLock::new(),
                    })) {
                        made = Some(not_kept.reader);
                    }
                }
            }
        }
    }
}

/// The leaves of rows trees whose every row a read transaction found valid for a table's current schema, so that its
/// scans do not check those rows again (E19 holds: a row of such a leaf passed the same check). A leaf is told apart
/// by its address, which no other page has while the transaction lives, and a table by its reader's: one leaf passes
/// for one table only. It holds at most [`CHECKED_LEAVES`] leaves, and forgets them all when full.
#[derive(Default)]
pub(crate) struct CheckedLeaves(Mutex<HashSet<(usize, usize)>>);

/// Leaves a read transaction remembers at most: 1 MiB or so of addresses.
const CHECKED_LEAVES: usize = 1 << 16;

impl CheckedLeaves {
    /// Whether the rows of `leaf` passed the check of `reader`, or, if not yet known, checks them all and remembers
    /// it when they do: rows of an older version or kept off the leaf make a leaf whose rows are checked one by one.
    pub(super) fn passes(&self, leaf: &[u8], reader: &Reader) -> bool {
        let key = (leaf.as_ptr() as usize, std::ptr::from_ref(reader) as usize);
        let mut leaves = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if leaves.contains(&key) {
            return true;
        }
        let passes = krummi_tree::leaf_entries(leaf)
            .all(|(key, bytes)| bytes.is_some_and(|bytes| reader.check_current(key, bytes)));
        if passes {
            if leaves.len() >= CHECKED_LEAVES {
                leaves.clear();
            }
            leaves.insert(key);
        }
        passes
    }
}

/// The key of the reader of a table of a write transaction in its [`Readers`]: the reader's address, which no other
/// reader has while it is kept.
pub(crate) fn write_key(reader: &Arc<Reader>) -> u64 {
    Arc::as_ptr(reader) as usize as u64
}

/// Where the rows of a table are: the name of its catalog entry, which holds its current schema, and the roots of its
/// rows tree and its history tree.
#[derive(Clone, Copy)]
pub(super) struct At<'a> {
    pub(super) name: &'a str,
    pub(super) rows_root: u64,
    pub(super) history: u64,
}

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
/// appears. The row was checked when it was read, and each value is decoded when it is asked for. Text, bytes and
/// vectors borrow the transaction's pages without copying them (D11), so the row and its values live until the next
/// operation that may modify the transaction.
pub struct TableRow<'t> {
    reader: RowReader<'t>,
    stored: Stored<'t>,
}

/// The reader of a [`TableRow`]: the one its transaction keeps, or its own once the row is owned.
enum RowReader<'t> {
    Kept(&'t Arc<Reader>),
    Owned(Arc<Reader>),
}

impl Deref for RowReader<'_> {
    type Target = Reader;

    #[inline]
    fn deref(&self) -> &Reader {
        match self {
            RowReader::Kept(reader) => reader,
            RowReader::Owned(reader) => reader,
        }
    }
}

/// What a [`TableRow`] holds.
enum Stored<'t> {
    /// The stored row: its encoded primary key and its bytes, the layout of its version when it is an older one, and
    /// the defaults borrowed from the catalog when it needs them.
    Row {
        key: Cow<'t, [u8]>,
        /// Where each column of a key of several columns starts ([`key_starts`]); `None` for one column.
        starts: Option<Vec<u16>>,
        bytes: &'t [u8],
        old: Option<Arc<OldVersion>>,
        defaults: Option<Arc<Vec<Value<'t>>>>,
    },
    /// Its values, owned ([`TableRow::into_owned`]).
    Owned(Vec<Value<'static>>),
}

impl<'t> TableRow<'t> {
    /// The number of live columns.
    pub fn len(&self) -> usize {
        self.reader.columns.len()
    }

    /// Whether the row has no columns, which a row never has: a table has at least one column (E4).
    pub fn is_empty(&self) -> bool {
        self.reader.columns.is_empty()
    }

    /// The value of the live column at `position`, in schema order. A row read decodes it from the stored bytes, so a
    /// text, bytes or vector value borrows the page; a row made by [`into_owned`](Self::into_owned) gives a copy of
    /// the value it owns.
    #[inline]
    pub fn value(&self, position: usize) -> Option<Value<'t>> {
        if position >= self.len() {
            return None;
        }
        Some(match &self.stored {
            Stored::Row {
                key,
                starts,
                bytes,
                old,
                defaults,
            } => self.reader.stored_value(
                key,
                starts.as_deref(),
                bytes,
                old.as_deref(),
                defaults.as_ref().map(|d| d.as_slice()),
                position,
            ),
            Stored::Owned(values) => values[position].clone(),
        })
    }

    /// The value of the live column `name`.
    pub fn get(&self, name: &str) -> Option<Value<'t>> {
        self.reader.names.get(name).and_then(|&i| self.value(i))
    }

    /// The name of the live column at `position`.
    pub fn name(&self, position: usize) -> Option<&str> {
        self.reader.columns.get(position).map(|c| c.name.as_str())
    }

    /// The values of the live columns, in schema order.
    pub fn values(&self) -> impl ExactSizeIterator<Item = Value<'t>> + '_ {
        (0..self.len()).map(|i| self.value(i).expect("a live column"))
    }

    /// The live columns' names with their values, in schema order.
    pub fn columns(&self) -> impl ExactSizeIterator<Item = (&str, Value<'t>)> + '_ {
        self.reader
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .zip(self.values())
    }

    /// The row with its borrowed values copied, so that it outlives its transaction's next change.
    pub fn into_owned(self) -> TableRow<'static> {
        let values = match self.stored {
            Stored::Owned(values) => values,
            Stored::Row { .. } => self.values().map(Value::into_owned).collect(),
        };
        TableRow {
            reader: RowReader::Owned(match self.reader {
                RowReader::Kept(reader) => Arc::clone(reader),
                RowReader::Owned(reader) => reader,
            }),
            stored: Stored::Owned(values),
        }
    }
}

impl<'t> TableRow<'t> {
    /// The live column at `position` read as `T` without building a [`Value`]: the column's stored bytes, through
    /// `stored`, for a column of the current version outside the key; otherwise its value, through `value`. `None`
    /// for a null or a position past the live columns. Panics when `matches` rejects the column's type.
    #[inline]
    fn typed<T>(
        &self,
        position: usize,
        type_name: &str,
        matches: impl Fn(&ColumnType) -> bool,
        stored: impl Fn(&'t [u8]) -> T,
        value: impl Fn(Value<'t>) -> T,
    ) -> Option<T> {
        let column = self.reader.columns.get(position)?;
        assert!(
            matches(&column.ty),
            "the column {} is a {}, not a {type_name}",
            column.name,
            column.ty
        );
        if let Stored::Row {
            bytes, old: None, ..
        } = &self.stored
            && column.in_key.is_none()
        {
            let slot = self.reader.slots[position];
            return self.reader.layout.checked_bytes(bytes, slot).map(stored);
        }
        match self.value(position)? {
            Value::Null => None,
            other => Some(value(other)),
        }
    }
}

impl<'t> TableRow<'t> {
    /// The value of the `ulid` column at `position` (see the typed reads); a key column is read from its element in
    /// the key, a type code and the 16 bytes.
    #[inline]
    pub fn ulid(&self, position: usize) -> Option<Ulid> {
        if let Some(column) = self.reader.columns.get(position)
            && let (Some(k), ColumnType::Ulid, Stored::Row { key, starts, .. }) =
                (column.in_key, &column.ty, &self.stored)
        {
            let start = starts.as_ref().map_or(0, |starts| usize::from(starts[k]));
            return Some(Ulid(array(&key[start + 1..start + 17])));
        }
        self.typed(
            position,
            "ulid",
            |ty| matches!(ty, ColumnType::Ulid),
            |b: &[u8]| Ulid(array(b)),
            |value| match value {
                Value::Ulid(v) => v,
                _ => unreachable!("a value of its column's type"),
            },
        )
    }

    /// The stored bytes of the `string` column at `position`, the UTF-8 of its text (see the typed reads), checked
    /// when the row was read: as [`text`](Self::text) without checking them again.
    #[inline]
    pub fn text_bytes(&self, position: usize) -> Option<Cow<'t, [u8]>> {
        self.typed(
            position,
            "string",
            |ty| matches!(ty, ColumnType::String),
            |b: &'t [u8]| Cow::Borrowed(b),
            |value| match value {
                Value::String(Cow::Borrowed(text)) => Cow::Borrowed(text.as_bytes()),
                Value::String(Cow::Owned(text)) => Cow::Owned(text.into_bytes()),
                _ => unreachable!("a value of its column's type"),
            },
        )
    }
}

/// The bytes of a stored fixed-width column as an array of its width.
fn array<const N: usize>(bytes: &[u8]) -> [u8; N] {
    bytes.try_into().expect("the slot's width")
}

/// Typed accessors of [`TableRow`]: `$method` reads a column of `$ty`, stored as `$stored` decodes it, as the
/// `Value::$variant` that [`TableRow::value`] would give.
macro_rules! typed_accessors {
    ($($method:ident, $name:literal, $ty:pat, $variant:ident, $out:ty, $stored:expr;)*) => {
        /// Typed reads of a column: each returns the column's value as its Rust type, without building a [`Value`],
        /// which costs less in a scan that reads every column; `None` for a null or a position past the live
        /// columns. Each panics on a column of another type; [`TableRow::value`] reads any column.
        impl<'t> TableRow<'t> {
            $(
                #[doc = concat!("The value of the `", $name, "` column at `position` (see the typed reads).")]
                #[inline]
                pub fn $method(&self, position: usize) -> Option<$out> {
                    self.typed(position, $name, |ty| matches!(ty, $ty), $stored, |value| match value {
                        Value::$variant(v) => v,
                        _ => unreachable!("a value of its column's type"),
                    })
                }
            )*
        }
    };
}

typed_accessors! {
    bool, "bool", ColumnType::Bool, Bool, bool, |b: &[u8]| b == [1];
    i8, "i8", ColumnType::I8, I8, i8, |b: &[u8]| i8::from_le_bytes(array(b));
    i16, "i16", ColumnType::I16, I16, i16, |b: &[u8]| i16::from_le_bytes(array(b));
    i32, "i32", ColumnType::I32, I32, i32, |b: &[u8]| i32::from_le_bytes(array(b));
    i64, "i64", ColumnType::I64, I64, i64, |b: &[u8]| i64::from_le_bytes(array(b));
    u8, "u8", ColumnType::U8, U8, u8, |b: &[u8]| u8::from_le_bytes(array(b));
    u16, "u16", ColumnType::U16, U16, u16, |b: &[u8]| u16::from_le_bytes(array(b));
    u32, "u32", ColumnType::U32, U32, u32, |b: &[u8]| u32::from_le_bytes(array(b));
    u64, "u64", ColumnType::U64, U64, u64, |b: &[u8]| u64::from_le_bytes(array(b));
    f32, "f32", ColumnType::F32, F32, f32, |b: &[u8]| f32::from_le_bytes(array(b));
    f64, "f64", ColumnType::F64, F64, f64, |b: &[u8]| f64::from_le_bytes(array(b));
    date, "date", ColumnType::Date, Date, i32, |b: &[u8]| i32::from_le_bytes(array(b));
    time, "time", ColumnType::Time, Time, i64, |b: &[u8]| {
        i64::from_le_bytes(array(b)) * i64::try_from(NANOS_PER_MICRO).expect("1,000")
    };
    timestamp, "timestamp", ColumnType::Timestamp, Timestamp, i128, |b: &[u8]| {
        i128::from(i64::from_le_bytes(array(b))) * NANOS_PER_MICRO
    };
    duration, "duration", ColumnType::Duration, Duration, i128, |b: &[u8]| {
        i128::from(i64::from_le_bytes(array(b))) * NANOS_PER_MICRO
    };
    uuid, "uuid", ColumnType::Uuid(_), Uuid, Uuid, |b: &[u8]| Uuid(array(b));
    text, "string", ColumnType::String, String, Cow<'t, str>, |b: &'t [u8]| {
        Cow::Borrowed(std::str::from_utf8(b).expect("a checked row"))
    };
    bytes, "bytes", ColumnType::Bytes, Bytes, Cow<'t, [u8]>, |b: &'t [u8]| Cow::Borrowed(b);
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
        let reader = self
            .readers
            .keep(table.history, || Arc::clone(&table.reader));
        match krummi_tree::get(&self.inner, table.rows_root, &key)? {
            Some(bytes) => reader
                .decode(
                    &self.inner,
                    table.at(),
                    &mut Versions::default(),
                    Cow::Owned(key),
                    bytes,
                    false,
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
        let state = self.live_table(table)?;
        let key = state
            .reader
            .lookup_key(key)
            .map_err(|fault| fault.error(self.folder, &state.name))?;
        let reader = self
            .readers
            .keep(write_key(&state.reader), || Arc::clone(&state.reader));
        match krummi_tree::get(&self.inner, state.rows_root, &key)? {
            Some(bytes) => reader
                .decode(
                    &self.inner,
                    state.at(),
                    &mut Versions::default(),
                    Cow::Owned(key),
                    bytes,
                    false,
                )
                .map(Some),
            None => Ok(None),
        }
    }

    /// Deletes the row of `table` whose primary key is `key`, and returns whether it was there (E11), with the
    /// failures of the key of [`get_row`](Self::get_row).
    pub fn delete_row(&mut self, table: &WriteTable<'_>, key: &[Value<'_>]) -> Result<bool> {
        let (rows_root, reader) = self.live_rows(table)?;
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
        let (rows_root, reader) = self.live_rows(table)?;
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
            self.live_table(table)?.at(),
            &mut Versions::default(),
            Cow::Borrowed(&key),
            bytes,
            false,
        )?;
        // Decoded once: the merge and the comparison both read every old value.
        let old: Vec<Value<'_>> = old.values().collect();
        let merged: Vec<Value<'_>> = values
            .into_iter()
            .zip(&old)
            .map(|(new, old)| new.unwrap_or_else(|| old.reborrow()))
            .collect();
        if merged.iter().zip(&old).all(|(new, old)| new.same(old)) {
            return Ok(Written::Updated);
        }
        let encoded = reader.encode(&merged).map_err(|fault| fail(self, fault))?;
        let update = krummi_tree::set(&mut self.inner, rows_root, &key, &encoded)?;
        self.apply_rows(table, update.root, RowCount::Kept)?;
        Ok(Written::Updated)
    }

    /// The rows tree and the reader of the table of `table`, with the failures of a handle (misuse for one of another
    /// transaction, not found once the table was deleted), without reading anything.
    pub(super) fn live_rows(&self, table: &WriteTable<'_>) -> Result<(u64, Arc<Reader>)> {
        let state = self.live_table(table)?;
        Ok((state.rows_root, Arc::clone(&state.reader)))
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
