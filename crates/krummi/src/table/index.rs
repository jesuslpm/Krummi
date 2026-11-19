//! Secondary indexes of a table (IX1–IX6 and IX10–IX14 of the secondary indexes PRD; document 3 of the table design,
//! *Index structure*; document 2, H15; document 11, P12): their definitions, creating and dropping them in a write,
//! and Describe's list of them.
//!
//! An index is a tree of unique keys that no catalog name reaches (N1): the key of an entry is the values of the
//! index's key columns, each encoded with its direction and its null order, followed by the row's encoded primary key
//! (`index_key`); its value holds the included columns in the row format of document 1, or nothing. The table's
//! catalog entry holds the definitions, their identities, the root of each index's tree and the table's index state
//! ([`TableSchema::encode`]), so Describe reads only the catalog.
//!
//! Creating an index over a table with rows reads every row as the reads do, without checking its content (L1, IX12),
//! encodes and sorts the entries (in memory and, past the sort's memory, in the files of `krummi.sort`, `sort`), and
//! finds an entry too large and two rows that repeat the key of a unique index before it allocates any page; only
//! then does it write the tree, in order. So a failure of public validation leaves
//! the store as it was and the transaction free to commit its other changes (IX11, H15).
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
//!         Column::new("email", ColumnType::String),
//!         Column::new("at", ColumnType::Timestamp),
//!     ],
//!     &["id"],
//! );
//! let table = write.open_table_with("users", TableMode::CreateNew(&schema))?;
//! write.insert_row(&table, &[("id", Value::I64(1)), ("email", Value::String("a@b.c".into()))])?;
//! let by_email = IndexDefinition::new("by_email", vec![IndexKey::new("email")]).unique();
//! write.create_index(&table, &by_email, IndexMode::CreateIfNotExists)?;
//! let latest = IndexDefinition::new("latest", vec![IndexKey::new("at").descending().nulls_last()])
//!     .with_included(&["email"]);
//! write.create_index(&table, &latest, IndexMode::CreateNew)?;
//! let described = write.describe_table("users")?;
//! let names: Vec<&str> = described.indexes().iter().map(|index| index.name.as_str()).collect();
//! assert_eq!(names, ["by_email", "latest"]);
//! write.drop_index(&table, "latest")?;
//! write.commit()?;
//! # Ok::<(), krummi::Error>(())
//! ```
//!
//! A read transaction neither creates nor drops an index:
//!
//! ```compile_fail
//! # use krummi::table::value::ColumnType;
//! # use krummi::{Column, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions, TableMode};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! let read = store.read().unwrap();
//! let table = read.open_table("t").unwrap();
//! let definition = IndexDefinition::new("ix", vec![IndexKey::new("n")]);
//! read.create_index(&table, &definition, IndexMode::CreateNew).unwrap();
//! ```
//!
//! ```compile_fail
//! # use krummi::{Store, StoreOptions};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! let read = store.read().unwrap();
//! let table = read.open_table("t").unwrap();
//! read.drop_index(&table, "ix").unwrap();
//! ```
//!
//! Creating and dropping an index take `&mut self`, so they end the references read before, also when they fail or
//! change nothing (IX13):
//!
//! ```compile_fail
//! # use krummi::table::value::{ColumnType, Value};
//! # use krummi::{Column, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions, TableMode};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null(), Column::new("n", ColumnType::I64)], &["id"]);
//! # let table = write.open_table_with("t", TableMode::CreateNew(&schema)).unwrap();
//! let row = write.get_row(&table, &[Value::I64(1)]).unwrap();
//! let definition = IndexDefinition::new("ix", vec![IndexKey::new("n")]);
//! write.create_index(&table, &definition, IndexMode::CreateIfNotExists).unwrap();
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
//! write.drop_index(&table, "ix").unwrap();
//! drop(row);
//! ```

use std::borrow::Cow;
use std::ops::Bound::Unbounded;
use std::sync::Arc;

use krummi_pager::{Error, Result, SchemaDifference, SchemaRule, SizeSubject};
use krummi_tree::catalog::MAX_EXTRA;
use krummi_tree::{Interval, LastLeaf, PageSource};

use super::catalog::{ReadTable, WriteTable};
use super::index_key::{decode_entry, entry_len, index_key_len, uniqueness_range, write_entry};
use super::key::{Direction, KeyColumn, KeyRange, MAX_KEY, Nulls};
use super::row::{MAX_ROW, RowColumn, RowLayout};
use super::rows::{At, Reader, Versions};
use super::schema::{Different, Invalid, TableSchema};
use super::sort::{Sorted, Sorter};
use super::value::{ColumnType, Value, key_text};
use crate::{Folder, ReadTransaction, WriteTransaction};

/// The most indexes of a table (IX1).
pub(crate) const MAX_INDEXES: usize = 64;
/// The most key columns of an index (IX1).
pub(crate) const MAX_INDEX_KEY: usize = 32;
/// The longest index name, in bytes of UTF-8 (IX1, as a column name, E4); the shortest has 1.
pub(crate) const MAX_INDEX_NAME: usize = 1024;

/// A key column of an index as the application defines it (IX1): a column, its direction and the order of its nulls,
/// ascending with the nulls first unless it says otherwise (N5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexKey {
    pub column: String,
    pub direction: Direction,
    pub nulls: Nulls,
}

impl IndexKey {
    /// The column `column`, ascending, with its nulls first.
    pub fn new(column: impl Into<String>) -> IndexKey {
        IndexKey {
            column: column.into(),
            direction: Direction::Ascending,
            nulls: Nulls::First,
        }
    }

    /// The column descending.
    pub fn descending(self) -> IndexKey {
        IndexKey {
            direction: Direction::Descending,
            ..self
        }
    }

    /// The column with its nulls last.
    pub fn nulls_last(self) -> IndexKey {
        IndexKey {
            nulls: Nulls::Last,
            ..self
        }
    }
}

/// The definition of an index, as the application gives it to [`WriteTransaction::create_index`] (IX1): a name of 1
/// to 1,024 bytes, unique within its table; whether it is unique; its key, 1 to 32 distinct columns, none a vector,
/// in the order the index sorts by; and its included columns, distinct and not in the key, in the order given. Primary
/// key columns may be in the key; among the included columns they are accepted and do nothing, since every entry holds
/// the whole primary key: the stored definition leaves them out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDefinition {
    pub name: String,
    pub unique: bool,
    pub key: Vec<IndexKey>,
    pub included: Vec<String>,
}

impl IndexDefinition {
    /// An index `name` that is not unique, with the key `key` and no included columns.
    pub fn new(name: impl Into<String>, key: Vec<IndexKey>) -> IndexDefinition {
        IndexDefinition {
            name: name.into(),
            unique: false,
            key,
            included: Vec::new(),
        }
    }

    /// The index unique: two rows whose key columns have the same values, none of them null, collide (N7).
    pub fn unique(self) -> IndexDefinition {
        IndexDefinition {
            unique: true,
            ..self
        }
    }

    /// The index with the included columns `columns`, in that order.
    pub fn with_included(self, columns: &[&str]) -> IndexDefinition {
        IndexDefinition {
            included: columns.iter().map(|name| (*name).to_owned()).collect(),
            ..self
        }
    }

    /// The column names the definition gives: its key's, then its included ones (IX3's order).
    fn columns(&self) -> impl Iterator<Item = &str> {
        (self.key.iter().map(|key| key.column.as_str()))
            .chain(self.included.iter().map(String::as_str))
    }
}

/// How [`WriteTransaction::create_index`] creates an index (IX2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndexMode {
    /// Creates the index; fails with [`ErrorKind::TreeAlreadyExists`](crate::ErrorKind::TreeAlreadyExists) if the
    /// table has an index of its name.
    CreateNew,
    /// Creates the index unless the table has an index of its name with the same definition, which it leaves as it
    /// is; with another definition, it fails with [`ErrorKind::DifferentSchema`](crate::ErrorKind::DifferentSchema).
    CreateIfNotExists,
}

/// A secondary index of a table, as Describe gives it (IX6): its identity, a random 64-bit value drawn when it is
/// created (P12), its name, whether it is unique, its key columns and its included columns, each named as the schema
/// the transaction sees names it. The primary key columns are never among its included columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaIndex {
    pub id: u64,
    pub name: String,
    pub unique: bool,
    pub key: Vec<SchemaIndexKey>,
    pub included: Vec<SchemaIndexColumn>,
}

/// A key column of an index, as Describe gives it: the column's identity and name, and its direction and null order
/// as the index defines them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaIndexKey {
    pub id: u64,
    pub name: String,
    pub direction: Direction,
    pub nulls: Nulls,
}

/// An included column of an index, as Describe gives it: the column's identity and name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaIndexColumn {
    pub id: u64,
    pub name: String,
}

impl SchemaIndex {
    /// Whether the index has the column of identity `id`, in its key or among its included columns.
    pub(crate) fn uses(&self, id: u64) -> bool {
        self.key.iter().any(|key| key.id == id) || self.included.iter().any(|c| c.id == id)
    }

    /// Names `name` the column of identity `id`, which a rename gave it (IX5).
    pub(crate) fn rename_column(&mut self, id: u64, name: &str) {
        for key in self.key.iter_mut().filter(|key| key.id == id) {
            key.name = name.to_owned();
        }
        for column in self.included.iter_mut().filter(|c| c.id == id) {
            column.name = name.to_owned();
        }
    }

    /// Whether the index has the key of `other`: the same columns in the same order, with the same directions and
    /// null orders (IX3).
    fn same_key(&self, other: &[SchemaIndexKey]) -> bool {
        self.key.len() == other.len()
            && (self.key.iter().zip(other))
                .all(|(a, b)| (a.id, a.direction, a.nulls) == (b.id, b.direction, b.nulls))
    }
}

/// The index that `definition` adds to `schema`, with a placeholder identity, or the first rule of IX3 it breaks, in
/// IX3's order and, within a rule, at the first column in the order the definition gives them (its key, then its
/// included columns); a repeated column is the first that repeats an earlier one. The rules are checked on the
/// definition as given, before the primary key columns among its included ones are left out.
///
/// `adding` is false for Create if not exists over the index of its name with the same definition (IX2): then another
/// index is one of another name, and the number of indexes and the size of the schema do not change.
pub(super) fn check_definition(
    schema: &TableSchema,
    definition: &IndexDefinition,
    adding: bool,
) -> std::result::Result<SchemaIndex, Invalid> {
    let fail = |rule, column: Option<&str>| {
        Err(Invalid {
            rule,
            column: column.map(str::to_owned),
            index: Some(definition.name.clone()),
        })
    };
    if definition.key.is_empty() {
        return fail(SchemaRule::NoIndexKey, None);
    }
    let mut columns = Vec::with_capacity(definition.key.len() + definition.included.len());
    for name in definition.columns() {
        match schema.column(name) {
            Some(column) => columns.push(column),
            None => return fail(SchemaRule::ColumnNotFound, Some(name)),
        }
    }
    let k = definition.key.len();
    if let Some(column) = columns[..k]
        .iter()
        .find(|c| matches!(c.ty, ColumnType::Vector(_)))
    {
        return fail(SchemaRule::VectorIndexKey, Some(&column.name));
    }
    for (i, column) in columns.iter().enumerate() {
        if columns[..i].iter().any(|earlier| earlier.id == column.id) {
            return fail(SchemaRule::RepeatedIndexColumn, Some(&column.name));
        }
    }
    if k > MAX_INDEX_KEY {
        return fail(SchemaRule::TooManyIndexKeyColumns, None);
    }
    // The primary key, or a prefix of it in an index that is not unique, in its order and ascending: whatever its
    // nulls and included columns, it would only repeat the rows tree (a unique prefix is a constraint).
    let primary = schema.key_ids();
    let along_primary = k <= primary.len()
        && (definition.key.iter().zip(&columns).zip(primary))
            .all(|((key, column), id)| key.direction == Direction::Ascending && column.id == *id);
    if along_primary && (k == primary.len() || !definition.unique) {
        return fail(SchemaRule::RedundantIndexKey, None);
    }
    let key: Vec<SchemaIndexKey> = (definition.key.iter().zip(&columns))
        .map(|(key, column)| SchemaIndexKey {
            id: column.id,
            name: column.name.clone(),
            direction: key.direction,
            nulls: key.nulls,
        })
        .collect();
    let same_key = (schema.indexes().iter())
        .any(|other| other.name != definition.name && other.same_key(&key));
    if same_key {
        return fail(SchemaRule::SameIndexKey, None);
    }
    if adding && schema.indexes().len() >= MAX_INDEXES {
        return fail(SchemaRule::TooManyIndexes, None);
    }
    let index = SchemaIndex {
        id: 0,
        name: definition.name.clone(),
        unique: definition.unique,
        key,
        included: (columns[k..].iter())
            .filter(|column| !primary.contains(&column.id))
            .map(|column| SchemaIndexColumn {
                id: column.id,
                name: column.name.clone(),
            })
            .collect(),
    };
    // E5's schema too large, with the definition as it is stored, naming the index and no column.
    if adding && schema.stored_len() + index.stored_len() > MAX_EXTRA {
        return fail(SchemaRule::SchemaTooLarge, None);
    }
    Ok(index)
}

/// Whether `definition`, given to Create if not exists, is the definition of `index`, the table's index of its name,
/// once the primary key columns among its included ones are left out (IX2), or its first difference: the uniqueness;
/// the key columns position by position (the column, its direction and its nulls), then their number; and the
/// included columns position by position, then their number.
pub(super) fn compare_definition(
    schema: &TableSchema,
    index: &SchemaIndex,
    definition: &IndexDefinition,
) -> std::result::Result<(), Different> {
    let different = |difference, expected: String, found: String| {
        Err(Different {
            column: None,
            difference,
            expected,
            found,
            index: Some(index.name.clone()),
        })
    };
    let uniqueness = |unique: bool| if unique { "unique" } else { "not unique" }.to_owned();
    if definition.unique != index.unique {
        return different(
            SchemaDifference::IndexUniqueness,
            uniqueness(definition.unique),
            uniqueness(index.unique),
        );
    }
    let key_text = |at: usize, name: &str, direction: Direction, nulls: Nulls| {
        format!("{name:?} {direction} {nulls} at position {}", at + 1)
    };
    for (at, (given, found)) in definition.key.iter().zip(&index.key).enumerate() {
        if (given.column.as_str(), given.direction, given.nulls)
            != (found.name.as_str(), found.direction, found.nulls)
        {
            return different(
                SchemaDifference::IndexKey,
                key_text(at, &given.column, given.direction, given.nulls),
                key_text(at, &found.name, found.direction, found.nulls),
            );
        }
    }
    let count = |n: usize| match n {
        1 => "1 column".to_owned(),
        n => format!("{n} columns"),
    };
    if definition.key.len() != index.key.len() {
        return different(
            SchemaDifference::IndexKey,
            count(definition.key.len()),
            count(index.key.len()),
        );
    }
    let primary = schema.primary_key();
    let given: Vec<&str> = (definition.included.iter())
        .map(String::as_str)
        .filter(|name| !primary.contains(name))
        .collect();
    let included_text = |at: usize, name: &str| format!("{name:?} at position {}", at + 1);
    for (at, (given, found)) in given.iter().zip(&index.included).enumerate() {
        if *given != found.name {
            return different(
                SchemaDifference::IndexIncluded,
                included_text(at, given),
                included_text(at, &found.name),
            );
        }
    }
    if given.len() != index.included.len() {
        return different(
            SchemaDifference::IndexIncluded,
            count(given.len()),
            count(index.included.len()),
        );
    }
    Ok(())
}

/// How the entries of an index are made from the rows of its table's current schema, and read back.
pub(super) struct EntryLayout {
    /// The index's key columns as their elements are encoded, in key order.
    pub(super) key: Vec<KeyColumn>,
    /// The place in schema order of each key column, in key order, and of each included column, in their order.
    key_places: Vec<usize>,
    included_places: Vec<usize>,
    /// The row format of an entry's value: the included columns, every one allowing nulls, so that allowing nulls in
    /// a column later (N3) keeps the layout of the entries written before; `None` without included columns, whose
    /// entries have an empty value.
    pub(super) included: Option<RowLayout>,
    /// The types of the primary key columns, in key order, which end every entry's key.
    pub(super) primary: Vec<ColumnType>,
}

impl EntryLayout {
    /// The layout of the entries of `index`, an index of `schema`.
    pub(super) fn new(schema: &TableSchema, index: &SchemaIndex) -> EntryLayout {
        let column = |id: u64| schema.column_by_id(id).expect("a live column of the index");
        let place = |id: u64| {
            schema
                .column_position(id)
                .expect("a live column of the index")
        };
        let included: Vec<RowColumn> = (index.included.iter())
            .map(|c| RowColumn {
                id: c.id,
                ty: column(c.id).ty,
                nullable: true,
                key: false,
            })
            .collect();
        EntryLayout {
            key: EntryLayout::key_columns(schema, index),
            key_places: index.key.iter().map(|key| place(key.id)).collect(),
            included_places: index.included.iter().map(|c| place(c.id)).collect(),
            included: (!included.is_empty()).then(|| RowLayout::new(0, included)),
            primary: (schema.key_ids().iter()).map(|id| column(*id).ty).collect(),
        }
    }

    /// The key columns of `index`, an index of `schema`, as their elements are encoded, in key order.
    pub(super) fn key_columns(schema: &TableSchema, index: &SchemaIndex) -> Vec<KeyColumn> {
        (index.key.iter())
            .map(|key| {
                let column = schema
                    .column_by_id(key.id)
                    .expect("a live column of the index");
                KeyColumn {
                    ty: column.ty,
                    direction: key.direction,
                    nulls: key.nulls,
                    nullable: column.nullable,
                }
            })
            .collect()
    }

    /// The text of the values of the key columns of the entry `key`, and that of its primary key, for the data of a
    /// duplicate key; `(?)` for bytes that do not decode, which only a malformed row gives (IX12).
    fn key_texts(&self, key: &[u8]) -> (String, String) {
        match decode_entry(&self.key, &self.primary, key) {
            Some((index, primary)) => (key_text(&index), key_text(&primary)),
            None => ("(?)".to_owned(), "(?)".to_owned()),
        }
    }

    /// The values of the key columns of a row whose live column at place `i`, in schema order, has `value(i)`.
    fn key_values<'v>(&self, value: &impl Fn(usize) -> Value<'v>) -> Vec<Value<'v>> {
        self.key_places.iter().map(|&i| value(i)).collect()
    }

    /// The entry of the row whose live column at place `i`, in schema order, has `value(i)` and whose encoded primary
    /// key is `primary`, with the values a read of the row gives (E11). Gives [`TooLarge::Entry`] for an entry over the
    /// maximum of a key (IX10), then [`TooLarge::Included`] for included columns over the maximum of a row: defaults
    /// added after a row was written can give it, and so can a row near the maximum, whose included columns each take
    /// a null flag that the row may not have. Its key and its value are reserved in `tx`'s fallible way, so that memory
    /// the system does not give fails with out of memory and T11 (IX2, P13) instead of ending the process.
    ///
    /// # Panics
    ///
    /// For a value its key column cannot hold, which only a malformed row gives (IX12, L2).
    pub(super) fn entry<'v, S: PageSource + ?Sized>(
        &self,
        tx: &S,
        value: impl Fn(usize) -> Value<'v>,
        primary: &[u8],
    ) -> Result<std::result::Result<RowEntry, TooLarge>> {
        let index_values = self.key_values(&value);
        let len = entry_len(&self.key, &index_values, primary);
        if len > MAX_KEY {
            return Ok(Err(TooLarge::Entry(len)));
        }
        let included = match &self.included {
            None => None,
            Some(included) => {
                let values: Vec<Value<'v>> =
                    self.included_places.iter().map(|&i| value(i)).collect();
                let len = included.encoded_len(&values);
                if len > MAX_ROW {
                    return Ok(Err(TooLarge::Included(len)));
                }
                Some((included, values, len))
            }
        };
        let mut entry_value = Vec::new();
        if let Some((included, values, len)) = included {
            krummi_tree::reserve(tx, &mut entry_value, len)?;
            included.encode_into(&values, &mut entry_value);
        }
        let has_null = index_values.iter().any(|v| matches!(v, Value::Null));
        let prefix = (!has_null).then(|| index_key_len(&self.key, &index_values));
        Ok(Ok(RowEntry {
            key: self.encode(tx, &index_values, primary, len)?,
            prefix,
            value: entry_value,
        }))
    }

    /// The key of the entry of a stored row, as [`EntryLayout::entry`] gives it, to remove the entry.
    ///
    /// # Panics
    ///
    /// For a value its key column cannot hold or an entry over the maximum of a key, which only a malformed row gives
    /// (IX12, L2).
    pub(super) fn entry_key<'v, S: PageSource + ?Sized>(
        &self,
        tx: &S,
        value: impl Fn(usize) -> Value<'v>,
        primary: &[u8],
    ) -> Result<Vec<u8>> {
        let index_values = self.key_values(&value);
        let len = entry_len(&self.key, &index_values, primary);
        self.encode(tx, &index_values, primary, len)
    }

    /// The key of the entry of `index_values` and `primary`, of `len` bytes ([`entry_len`]), reserved in `tx`'s
    /// fallible way (IX2, P13).
    fn encode<S: PageSource + ?Sized>(
        &self,
        tx: &S,
        index_values: &[Value<'_>],
        primary: &[u8],
        len: usize,
    ) -> Result<Vec<u8>> {
        let malformed =
            || -> ! { panic!("a malformed row gives a value its index column cannot hold") };
        if len > MAX_KEY {
            malformed();
        }
        let mut entry = Vec::new();
        krummi_tree::reserve(tx, &mut entry, len)?;
        write_entry(&self.key, index_values, primary, &mut entry).unwrap_or_else(|_| malformed());
        Ok(entry)
    }

    /// The entries that collide with `key`, the entry of the row whose live column at place `i` has `value(i)`, in a
    /// unique index (document 3, *Unique index*); `None` when one of its key columns is null (N7).
    pub(super) fn collisions<'v>(
        &self,
        value: impl Fn(usize) -> Value<'v>,
        key: &[u8],
    ) -> Option<KeyRange> {
        uniqueness_range(&self.key, &self.key_values(&value), key)
    }
}

/// The entry of a row in an index: its key, the length of its uniqueness prefix (the key columns; `None` when one of
/// them is null, since such an entry never collides, N7) and its value.
pub(super) struct RowEntry {
    pub(super) key: Vec<u8>,
    pub(super) prefix: Option<usize>,
    pub(super) value: Vec<u8>,
}

/// What of an entry passes its maximum, with the size it has (IX10).
pub(super) enum TooLarge {
    /// The entry's key: its key columns and the primary key, over the maximum of a key.
    Entry(usize),
    /// Its included columns, over the maximum of a row.
    Included(usize),
}

/// What creating an index needs once its definition passed the checks that read only the catalog.
struct Build {
    /// The index, with a placeholder identity.
    index: SchemaIndex,
    layout: EntryLayout,
    /// The table's name and the roots of its rows and history trees.
    table: String,
    rows_root: u64,
    history: u64,
    reader: Arc<Reader>,
}

impl Folder<'_> {
    /// Fails with [`Error::InvalidSize`] unless the index name has 1 to 1,024 bytes (IX1, IX3).
    pub(super) fn check_index_name(self, name: &str) -> Result<()> {
        self.check_size(SizeSubject::IndexName, name.len(), 1, MAX_INDEX_NAME)
    }

    /// The error of Create new with the name of an index of the table (IX2).
    fn index_exists(self, table: &str, index: &str) -> Error {
        Error::TreeAlreadyExists {
            folder: self.owned(),
            name: table.to_owned(),
            index: Some(index.into()),
        }
    }

    /// The error of an entry of the index `index` of `table` that passes its maximum (IX10): its key, with the
    /// index-entry subject, or its included columns, with the size of a row.
    pub(super) fn entry_too_large(self, table: &str, index: &str, fault: TooLarge) -> Error {
        match fault {
            TooLarge::Entry(received) => Error::InvalidSize {
                folder: self.owned(),
                what: SizeSubject::IndexEntry,
                received: received as u64,
                min: 1,
                max: MAX_KEY as u64,
                table: Some(table.into()),
                index: Some(index.into()),
            },
            TooLarge::Included(len) => self
                .check_size(SizeSubject::Row, len, 0, MAX_ROW)
                .expect_err("over the maximum of a row"),
        }
    }

    /// The duplicate key of the entry `key` of the unique index `index` of `table`, whose layout is `layout` (IX10):
    /// the values of its key columns and its primary key, the row's that repeats them.
    pub(super) fn index_duplicate(
        self,
        table: &str,
        index: &str,
        layout: &EntryLayout,
        key: &[u8],
    ) -> Error {
        let (index_key, primary) = layout.key_texts(key);
        Error::DuplicateKey {
            folder: self.owned(),
            table: table.to_owned(),
            key: primary,
            index: Some(index.into()),
            index_key: Some(index_key.into()),
        }
    }
}

/// The operations on the indexes of a write transaction (IX2, IX4), with the rules of its operations on tables: they
/// take `&mut self`, so they end the references read before, also when they fail or change nothing (IX13). The
/// failures of public validation ([`Error::InvalidSize`], [`Error::TreeNotFound`], [`Error::TreeAlreadyExists`],
/// [`Error::DifferentIndex`], [`Error::InvalidSchema`], [`Error::DuplicateKey`] and [`Error::Misuse`] for a handle of
/// another transaction) have no effect and do not prevent the commit; any other failure does (T11), damage included.
impl<'s> WriteTransaction<'s> {
    /// Creates the index of `definition` in `table`, in `mode` (IX2), and reads every row of the table to write its
    /// entries, so that the index has the entries of the rows there are, each with the values a read of its row gives
    /// (E11): an added column's default or null. Creating an index changes neither the schema version nor the schema
    /// state, so prepared projections keep reading the rows (IX5); it draws the index's identity and a new index state
    /// (IX6). The index is seen at once by every handle of the table in this transaction (B9).
    ///
    /// Fails, in this order (IX10), with [`Error::InvalidSize`] for an index name or a column name that does not have
    /// 1 to 1,024 bytes; with [`Error::TreeNotFound`] for a handle of a deleted table; when the table has an index of
    /// the name, with [`Error::TreeAlreadyExists`] in [`IndexMode::CreateNew`] and, in
    /// [`IndexMode::CreateIfNotExists`], with [`Error::DifferentIndex`], of the class of a different schema, if the
    /// definition is another, naming the first difference (see [`SchemaDifference::IndexUniqueness`], [`SchemaDifference::IndexKey`] and
    /// [`SchemaDifference::IndexIncluded`]); with [`Error::InvalidSchema`] for a definition that breaks a rule of IX3,
    /// the first in this order: no key columns, a column that does not exist, a vector in the key, a repeated column,
    /// more than 32 key columns, a key that is the primary key or, not unique, an ascending prefix of it, the key of
    /// another index, more than 64 indexes, and a schema too large for its catalog entry (see [`SchemaRule`]); with the
    /// failures of reading the table's pages, damage included, and in a store on disk those of creating, writing or
    /// reading the files of the sort in the subfolder `krummi.sort`, where entries past the sort's memory go (access,
    /// disk full, out of memory or I/O, an I/O failure writing them also making the store refuse writes until it is
    /// reopened, T12), the first that happens; then, after reading every row, with
    /// [`Error::InvalidSize`] for an entry of more than 2,048 bytes ([`SizeSubject::IndexEntry`]: the key columns and
    /// the primary key in their stored form), the first row's in the order of the primary key; and last, for a unique
    /// index, with [`Error::DuplicateKey`] for two rows with the same values in its key columns, none of them null:
    /// the second of the first two in the index's order. None of these allocates a page (IX11). Reading the files of
    /// the sort while the index is written, and writing its pages, can fail too, as the transaction's failures (T11).
    pub fn create_index(
        &mut self,
        table: &WriteTable<'_>,
        definition: &IndexDefinition,
        mode: IndexMode,
    ) -> Result<()> {
        self.folder.check_index_name(&definition.name)?;
        for column in definition.columns() {
            self.folder.check_column_name(column)?;
        }
        let Some(build) = self.plan_index(table, definition, mode)? else {
            return Ok(());
        };
        let sorted = self.sorted_entries(&build)?;
        let history = build.history;
        let known =
            move |tx: &Self, id: u64| Ok(tx.known_identity(history, id)? || tx.store_knows(id)?);
        let id = self.draw_identity(known)?;
        let index_state = self.draw_identity(|tx, state| Ok(state == id || known(tx, state)?))?;
        let root = self.write_index(sorted)?;
        let history = self.record_identities(history, [id, index_state])?;
        let state = self
            .tables
            .get_mut()
            .get_mut(table.id)
            .expect("a live handle");
        let at = state
            .schema
            .add_index(SchemaIndex { id, ..build.index }, index_state);
        state.index_roots.insert(at, root);
        state.maintained = None;
        state.emptied = true;
        state.history = history;
        self.write_table_entry(table.id)
    }

    /// Drops the index `name` of `table` and frees its pages (IX4); a table without an index of the name changes
    /// nothing. It draws a new index state (IX6) and changes neither the schema version nor the schema state. Fails
    /// with [`Error::InvalidSize`] for a name that does not have 1 to 1,024 bytes, then with [`Error::TreeNotFound`]
    /// for a handle of a deleted table, and with damage of a page of the index it frees (T11); it reads no page of
    /// the table's indexes when there is no index of the name (IX12).
    pub fn drop_index(&mut self, table: &WriteTable<'_>, name: &str) -> Result<()> {
        self.folder.check_index_name(name)?;
        let (root, history) = {
            let state = self.live_table(table)?;
            let Some(at) = state.schema.index_position(name) else {
                return Ok(());
            };
            (state.index_roots[at], state.history)
        };
        let index_state = self.draw_identity(|tx, state| {
            Ok(tx.known_identity(history, state)? || tx.store_knows(state)?)
        })?;
        krummi_tree::free_all(&mut self.inner, root)?;
        let history = self.record_identities(history, [index_state])?;
        let state = self
            .tables
            .get_mut()
            .get_mut(table.id)
            .expect("a live handle");
        let at = state.schema.index_position(name).expect("found above");
        state.schema.remove_index(at, index_state);
        state.index_roots.remove(at);
        state.maintained = None;
        state.emptied = true;
        state.history = history;
        self.write_table_entry(table.id)
    }

    /// The index that `definition` creates in `table` in `mode`, after the checks that read only the catalog (the
    /// handle, the index of the name, the rules of IX3), or `None` when Create if not exists finds the index of the
    /// name with the same definition.
    fn plan_index(
        &self,
        table: &WriteTable<'_>,
        definition: &IndexDefinition,
        mode: IndexMode,
    ) -> Result<Option<Build>> {
        let state = self.live_table(table)?;
        let invalid = |invalid| self.folder.invalid_schema(&state.name, invalid);
        if let Some(existing) = state.schema.index(&definition.name) {
            return match mode {
                IndexMode::CreateNew => {
                    Err(self.folder.index_exists(&state.name, &definition.name))
                }
                IndexMode::CreateIfNotExists => {
                    compare_definition(&state.schema, existing, definition).map_err(
                        |different| self.folder.different_schema(&state.name, different),
                    )?;
                    check_definition(&state.schema, definition, false).map_err(invalid)?;
                    Ok(None)
                }
            };
        }
        let index = check_definition(&state.schema, definition, true).map_err(invalid)?;
        Ok(Some(Build {
            layout: EntryLayout::new(&state.schema, &index),
            index,
            table: state.name.clone(),
            rows_root: state.rows_root,
            history: state.history,
            reader: Arc::clone(&state.reader),
        }))
    }

    /// The entries of the index of `build` for every row of its table, sorted (in memory, and past its memory in the
    /// files of `krummi.sort`, IX2), with its failures in this order (IX10, IX11): first, as they happen while every row
    /// is read, a page failure or a failure of the sort while it takes the entries (its memory, or writing a run file
    /// until an entry is too large); then an entry too large, the first row's in the order of the primary key; then a
    /// failure of the files while the sort finishes (writing its last run and merging groups of runs); and last, for
    /// a unique index, two rows that repeat its key or a failure reading the sorted entries, whichever comes first in
    /// the index's order. It changes nothing in the store; the files of the sort go when the entries are dropped, and
    /// a failure reading them while the tree is written comes later, from [`Self::write_index`].
    ///
    /// The rows are read as a scan reads them, without checking their content (L1, IX12): a malformed row may give
    /// values that are not the written ones, or panic, before anything changes.
    fn sorted_entries(&self, build: &Build) -> Result<Sorted<'s>> {
        let layout = &build.layout;
        let at = At {
            name: &build.table,
            rows_root: build.rows_root,
            history: build.history,
        };
        let every_row = Interval {
            prefix: &[],
            lower: Unbounded,
            upper: Unbounded,
        };
        let mut versions = Versions::default();
        let mut sorter = Sorter::new(self.inner.sort_files());
        let mut too_large = None;
        for entry in krummi_tree::Range::new(&self.inner, build.rows_root, every_row)? {
            let (key, bytes) = entry?;
            let row = build.reader.decode(
                &self.inner,
                at,
                &mut versions,
                Cow::Borrowed(key),
                bytes,
                false,
            )?;
            if too_large.is_some() {
                continue;
            }
            match layout.entry(&self.inner, |i| row.value(i).expect("a live column"), key)? {
                Ok(entry) => sorter.push(&self.inner, entry)?,
                Err(fault) => {
                    too_large = Some(self.folder.entry_too_large(
                        &build.table,
                        &build.index.name,
                        fault,
                    ));
                }
            }
        }
        if let Some(error) = too_large {
            return Err(error);
        }
        let sorted = sorter.sorted(&self.inner)?;
        if build.index.unique {
            self.check_unique(build, &sorted)?;
        }
        Ok(sorted)
    }

    /// Fails with [`Error::DuplicateKey`] when two of the `sorted` entries of the unique index of `build` have the same
    /// key columns, none of them null (N7): the first two, which are neighbours in the index's order, with the primary
    /// key of the second. Their key columns are compared whole, never as a byte prefix (document 3).
    fn check_unique(&self, build: &Build, sorted: &Sorted<'_>) -> Result<()> {
        let mut cursor = sorted.cursor().map_err(|e| self.inner.fail(e))?;
        // The key columns of the entry before, when none of them is null.
        let mut previous = Vec::new();
        krummi_tree::reserve(&self.inner, &mut previous, MAX_KEY)?;
        let mut comparable = false;
        while let Some(entry) = cursor.next().map_err(|e| self.inner.fail(e))? {
            let Some(len) = entry.prefix else {
                comparable = false;
                continue;
            };
            let columns = &entry.key[..len];
            if comparable && previous == columns {
                return Err(self.folder.index_duplicate(
                    &build.table,
                    &build.index.name,
                    &build.layout,
                    &entry.key,
                ));
            }
            previous.clear();
            previous.extend_from_slice(columns);
            comparable = true;
        }
        Ok(())
    }

    /// Writes the `sorted` entries into a new tree and returns its root: each goes to the end of the tree's last leaf
    /// while it fits there, so the tree is written in one pass. The files of the sort go once it returns.
    fn write_index(&mut self, sorted: Sorted<'_>) -> Result<u64> {
        let mut root = krummi_tree::create(&mut self.inner)?;
        let mut last: Option<LastLeaf> = None;
        let mut cursor = sorted.cursor().map_err(|e| self.inner.fail(e))?;
        while let Some(entry) = cursor.next().map_err(|e| self.inner.fail(e))? {
            if let Some(leaf) = &last
                && krummi_tree::append_last(&mut self.inner, leaf, &entry.key, &entry.value)?
            {
                continue;
            }
            let (update, leaf) =
                krummi_tree::try_add_at_last(&mut self.inner, root, &entry.key, &entry.value)?;
            debug_assert!(
                update.count_changed,
                "each key ends with its row's primary key"
            );
            root = update.root;
            last = leaf;
        }
        Ok(root)
    }
}

/// An entry of an index as a test hook reads it, its values decoded into copies: the values of its key columns, in
/// key order, those of its primary key, in key order, and those of its included columns, in their order, with its key
/// as the tree stores it.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub struct StoredIndexEntry {
    pub key: Vec<Value<'static>>,
    pub primary_key: Vec<Value<'static>>,
    pub included: Vec<Value<'static>>,
    pub bytes: Vec<u8>,
}

/// The entries of the index `name` of the table with `schema` and the index roots `roots`, in the order of the
/// index's tree, or `None` when the table has no index of the name.
///
/// # Panics
///
/// For an entry that does not decode.
fn stored_index_entries<S: PageSource + ?Sized>(
    tx: &S,
    schema: &TableSchema,
    roots: &[u64],
    name: &str,
) -> Result<Option<Vec<StoredIndexEntry>>> {
    let Some(at) = schema.index_position(name) else {
        return Ok(None);
    };
    let layout = EntryLayout::new(schema, &schema.indexes()[at]);
    let mut entries = Vec::new();
    krummi_tree::for_each(tx, roots[at], &mut |key, value| {
        let (index, primary) =
            decode_entry(&layout.key, &layout.primary, key).expect("an entry that decodes");
        let included = match &layout.included {
            None => Vec::new(),
            Some(included) => {
                let row = included.row(value).expect("a value that decodes");
                (0..layout.included_places.len())
                    .map(|i| {
                        row.value(i)
                            .expect("a value that decodes")
                            .expect("an included column")
                            .into_owned()
                    })
                    .collect()
            }
        };
        entries.push(StoredIndexEntry {
            key: index,
            primary_key: primary,
            included,
            bytes: key.to_vec(),
        });
    })?;
    Ok(Some(entries))
}

impl ReadTransaction<'_> {
    /// A test hook, not part of the interface: the entries of the index `name` of `table`, or `None` when the table
    /// has no index of the name.
    #[doc(hidden)]
    pub fn stored_index_entries(
        &self,
        table: &ReadTable<'_>,
        name: &str,
    ) -> Result<Option<Vec<StoredIndexEntry>>> {
        self.check_table(table)?;
        stored_index_entries(&self.inner, &table.schema, &table.index_roots, name)
    }

    /// A test hook, not part of the interface: the name and the tree root of each index of `table`, in the order of
    /// their names.
    #[doc(hidden)]
    pub fn index_trees(&self, table: &ReadTable<'_>) -> Result<Vec<(String, u64)>> {
        self.check_table(table)?;
        Ok(index_trees(&table.schema, &table.index_roots))
    }
}

impl WriteTransaction<'_> {
    /// A test hook, not part of the interface: the entries of the index `name` of `table`, with this transaction's
    /// changes, or `None` when the table has no index of the name.
    #[doc(hidden)]
    pub fn stored_index_entries(
        &self,
        table: &WriteTable<'_>,
        name: &str,
    ) -> Result<Option<Vec<StoredIndexEntry>>> {
        let state = self.live_table(table)?;
        stored_index_entries(&self.inner, &state.schema, &state.index_roots, name)
    }

    /// A test hook, not part of the interface: the name and the tree root of each index of `table`, in the order of
    /// their names.
    #[doc(hidden)]
    pub fn index_trees(&self, table: &WriteTable<'_>) -> Result<Vec<(String, u64)>> {
        let state = self.live_table(table)?;
        Ok(index_trees(&state.schema, &state.index_roots))
    }
}

fn index_trees(schema: &TableSchema, roots: &[u64]) -> Vec<(String, u64)> {
    (schema.indexes().iter())
        .zip(roots)
        .map(|(index, root)| (index.name.clone(), *root))
        .collect()
}
