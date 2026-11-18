//! Column projections (the projection PRD, `tables-projection.prd.md`): an application names once the columns it will
//! read from the rows of a table, Krummi prepares once what depends on the schema and not on each row, and each row is
//! then read without repeating that work.
//!
//! A projection is prepared from a table opened in a read or a write transaction with a list of live column names
//! (PY1), and stays tied to the table and the schema that opening sees. From it, a [`ProjectedColumn`] is obtained for
//! each position of the list with the type it is read as (PY2, [`ReadAs`]); a row read by key or scanned, or a copied
//! row, is bound to the projection and read column by column with them (PY3), with the same values as reading the
//! column by its position. The store keeps up to [`MAX_PROJECTIONS`] projections in memory, shared by all its
//! transactions and threads, until it closes (PY5).
//!
//! ```
//! use krummi::table::projection::Text;
//! use krummi::table::value::{ColumnType, Value};
//! use krummi::{Column, Schema, Store, StoreOptions, TableMode};
//!
//! let store = Store::open_temporary(&StoreOptions::default())?;
//! let mut write = store.write()?;
//! let schema = Schema::new(
//!     vec![
//!         Column::new("id", ColumnType::I64).not_null(),
//!         Column::new("name", ColumnType::String),
//!     ],
//!     &["id"],
//! );
//! let table = write.open_table_with("t", TableMode::CreateNew(&schema))?;
//! write.insert_row(&table, &[("id", Value::I64(1)), ("name", Value::String("one".into()))])?;
//! write.commit()?;
//!
//! let read = store.read()?;
//! let table = read.open_table("t")?;
//! let projection = read.projection(&table, &["name", "id"])?;
//! let (name, id) = (projection.column::<Text>(0)?, projection.column::<i64>(1)?);
//! let row = read.get_row(&table, &[Value::I64(1)])?.expect("inserted");
//! let row = projection.bind(&row)?;
//! assert_eq!(row.get(&name).as_deref(), Some("one"));
//! assert_eq!(row.get(&id), Some(1));
//! # Ok::<(), krummi::Error>(())
//! ```
//!
//! A projection cannot outlive its store (PY5, T8):
//!
//! ```compile_fail
//! # use krummi::table::value::ColumnType;
//! # use krummi::{Column, Schema, Store, StoreOptions, TableMode};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
//! # let table = write.open_table_with("t", TableMode::CreateNew(&schema)).unwrap();
//! let projection = write.projection(&table, &["id"]).unwrap();
//! drop(table);
//! drop(write);
//! store.close().unwrap();
//! drop(projection);
//! ```

use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use krummi_pager::{Error, Result, SchemaDifference, ValueRule};

use super::catalog::{ReadTable, WriteTable};
use super::key::key_value;
use super::row::{FixedSlot, RowLayout, Slot, slot_array};
use super::rows::{Fault, OldVersion, Reader, SchemaId, Stored, TableRow, needs_stored};
use super::schema::Different;
use super::value::{ColumnType, Decimal, NANOS_PER_MICRO, Value};
use crate::ids::{Ulid, Uuid};
use crate::{Folder, ReadTransaction, Store, WriteTransaction};

/// The most projections a store keeps until it closes (PY5): the first ones prepared, of any table, schema or
/// transaction; a projection prepared after them works but is not kept.
pub const MAX_PROJECTIONS: usize = 1_024;

/// The projections a store keeps (PY5), with the store's serial, which the schemas of its tables carry.
pub(crate) struct Projections {
    serial: u64,
    kept: Mutex<HashMap<Arc<ListKey>, Arc<Prepared>>>,
    /// The schema identities that a projection or a row of the store may still carry although no tree of a later
    /// write lists them, which the store never draws again, so that two schemas it shows never share one (PY1, PY3).
    retired: Mutex<Retired>,
}

impl Projections {
    pub(crate) fn new(serial: u64) -> Projections {
        Projections {
            serial,
            kept: Mutex::default(),
            retired: Mutex::default(),
        }
    }

    /// The serial of the store, unique in the process.
    pub(crate) fn serial(&self) -> u64 {
        self.serial
    }

    fn retired(&self) -> std::sync::MutexGuard<'_, Retired> {
        self.retired.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether the store must not draw `id` again for a new table.
    pub(crate) fn retired_table(&self, id: u64) -> bool {
        self.retired().tables.contains(&id)
    }

    /// Records that the store must not draw `id` again for a new table: one it created, also in a write it
    /// abandoned, or deleted.
    pub(crate) fn retire_table(&self, id: u64) {
        self.retired().tables.insert(id);
    }

    /// Whether the store must not draw `state` again for a new schema of the table `table`.
    pub(crate) fn retired_schema(&self, table: u64, state: u64) -> bool {
        self.retired().schemas.contains(&(table, state))
    }

    /// Records that the store must not draw `state` again for a new schema of the table `table`: a state drawn by a
    /// schema change, also in a write it abandoned.
    pub(crate) fn retire_schema(&self, table: u64, state: u64) {
        self.retired().schemas.insert((table, state));
    }

    /// How many projections the store keeps.
    pub(crate) fn kept(&self) -> usize {
        self.kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// The projection of `key` that the store keeps, or `make()`, kept while there is room.
    fn keep(
        &self,
        key: ListKey,
        make: impl FnOnce(Arc<ListKey>, bool) -> Prepared,
    ) -> Arc<Prepared> {
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(prepared) = kept.get(&key) {
            return Arc::clone(prepared);
        }
        let room = kept.len() < MAX_PROJECTIONS && kept.try_reserve(1).is_ok();
        let key = Arc::new(key);
        let prepared = Arc::new(make(Arc::clone(&key), room));
        if room && let Entry::Vacant(entry) = kept.entry(key) {
            entry.insert(Arc::clone(&prepared));
        }
        prepared
    }
}

/// The schema identities a store retires (see [`Projections`]): the identities of the tables it created, also in
/// abandoned writes, or deleted, and the schema states that its schema changes drew for each table, also in abandoned
/// writes. A schema's identity is its table's and its state (`SchemaId`), so a new table needs only an identity not
/// retired, and a new schema of a table a state that neither its history nor this set knows. One entry per table
/// created or deleted and per schema change while the store is open.
#[derive(Default)]
struct Retired {
    tables: HashSet<u64>,
    schemas: HashSet<(u64, u64)>,
}

/// What identifies a projection: the schema it is tied to and the live columns of its list, by position, in its
/// order. Every preparation of the same list over the same schema has the same key, kept or not (PY3).
#[derive(Debug, PartialEq, Eq, Hash)]
struct ListKey {
    schema: SchemaId,
    positions: Box<[usize]>,
}

/// A projection as it is prepared once, shared by every handle of it.
struct Prepared {
    key: Arc<ListKey>,
    /// The store's folder, for the errors.
    folder: Option<PathBuf>,
    /// The reader of the schema it was prepared with: its columns, its layout and their defaults.
    reader: Arc<Reader>,
    /// How each column of the list is read from a row of the current version.
    current: Box<[Planned]>,
    /// The same for the rows of each older version, by version number, prepared when the first such row is read.
    older: Box<[OnceLock<Box<[Planned]>>]>,
    /// Whether the store keeps it (a test hook).
    kept: bool,
}

/// A column of the list as it is read from a row of one version: everything a read needs, in one record.
struct Planned {
    access: Access,
    ty: ColumnType,
    /// Its position among the live columns.
    position: usize,
}

/// How a column of the list is read from a row of one version.
enum Access {
    /// From the fixed section of the row's bytes.
    Fixed(FixedSlot),
    /// From the variable section of the row's bytes, at this slot of the version's layout.
    Variable(Slot),
    /// From the row's key: a fixed-width primary key column, this element of the key.
    Key(usize),
    /// A column added after the row's version, with this default (null, or one the reader keeps).
    Constant(Value<'static>),
    /// A column added after the row's version, with a text, bytes or vector default that the row borrows from the
    /// catalog (D11).
    Default,
}

impl Prepared {
    /// How the columns at `positions` are read from the rows of `old`, or of the current version without it.
    fn plan(reader: &Reader, positions: &[usize], old: Option<&OldVersion>) -> Box<[Planned]> {
        positions
            .iter()
            .map(|&i| {
                let column = &reader.columns[i];
                let (layout, place) = match old {
                    None => (&reader.layout, Some(i)),
                    Some(old) => (&old.layout, old.places[i]),
                };
                let access = match (column.in_key, place) {
                    (Some(k), _) => Access::Key(k),
                    (None, Some(place)) => {
                        let slot = layout.slot(place).expect("a column of the version");
                        match slot.fixed() {
                            Some(fixed) => Access::Fixed(fixed),
                            None => Access::Variable(slot),
                        }
                    }
                    (None, None) if needs_stored(&column.default) => Access::Default,
                    (None, None) => Access::Constant(column.default.clone()),
                };
                Planned {
                    access,
                    ty: column.ty,
                    position: i,
                }
            })
            .collect()
    }
}

/// A projection of columns of a table (PY1): a list of its live columns, tied to the table and the schema it was
/// prepared with, with which rows of that schema are read. It is shared by the transactions and the threads of its
/// store, which may keep it (PY5), and cannot outlive the store.
pub struct Projection<'s> {
    prepared: Arc<Prepared>,
    /// The table's name when this handle was prepared, for its errors (PY1): a kept projection prepared again after
    /// the table was renamed names it as it is now.
    table: Arc<str>,
    _store: PhantomData<&'s Store>,
}

impl Clone for Projection<'_> {
    fn clone(&self) -> Self {
        Projection {
            prepared: Arc::clone(&self.prepared),
            table: Arc::clone(&self.table),
            _store: PhantomData,
        }
    }
}

impl fmt::Debug for Projection<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reader = &self.prepared.reader;
        f.debug_struct("Projection")
            .field("table", &self.table)
            .field(
                "columns",
                &self
                    .prepared
                    .key
                    .positions
                    .iter()
                    .map(|&i| reader.columns[i].name.as_str())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// A column of a projection prepared to be read as `T` (PY2), from [`Projection::column`]. It reads the rows bound
/// to any projection of the same list, in the same order, over the same schema.
pub struct ProjectedColumn<T: ReadAs> {
    key: Arc<ListKey>,
    at: usize,
    _type: PhantomData<fn() -> T>,
}

impl<T: ReadAs> Clone for ProjectedColumn<T> {
    fn clone(&self) -> Self {
        ProjectedColumn {
            key: Arc::clone(&self.key),
            at: self.at,
            _type: PhantomData,
        }
    }
}

impl<T: ReadAs> fmt::Debug for ProjectedColumn<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProjectedColumn")
            .field("position", &self.at)
            .field("type", &T::NAME)
            .finish()
    }
}

/// A row bound to a projection (PY3), from [`Projection::bind`], whose columns read with the projection's
/// [`ProjectedColumn`]s. Its values live as the row's do: text, bytes and vectors of a row read or scanned borrow the
/// transaction's pages (D11), and those of a copied row are copies of their own.
pub struct ProjectedRow<'p, 't> {
    prepared: &'p Prepared,
    plan: &'p [Planned],
    row: Bound<'p, 't>,
}

impl fmt::Debug for ProjectedRow<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProjectedRow")
            .field("columns", &self.plan.len())
            .finish_non_exhaustive()
    }
}

/// What a [`ProjectedRow`] reads from.
enum Bound<'p, 't> {
    Row {
        key: &'p [u8],
        starts: Option<&'p [u16]>,
        bytes: &'t [u8],
        layout: &'p RowLayout,
        defaults: Option<&'p [Value<'t>]>,
    },
    Owned(&'p [Value<'static>]),
}

impl<'s> Projection<'s> {
    /// The [`Error::DifferentSchema`] of the projection's table.
    fn different(&self, different: Different) -> Error {
        Folder(self.prepared.folder.as_deref()).different_schema(&self.table, different)
    }

    /// The error of binding a row of another table, store or schema (PY3).
    #[cold]
    #[inline(never)]
    fn wrong_row(&self) -> Error {
        self.different(Different {
            column: None,
            difference: SchemaDifference::Projection,
            expected: "a row of the projection's table and schema".to_owned(),
            found: "a row of another table, store or schema".to_owned(),
            index: None,
        })
    }

    /// The column at `position` of the list, prepared to be read as `T`, which must be read from the column's type
    /// ([`ReadAs`]). Fails with [`Error::DifferentSchema`] with the table, the column and the difference otherwise,
    /// without effect (PY2). Panics if `position` is past the list: a fault of the program.
    pub fn column<T: ReadAs>(&self, position: usize) -> Result<ProjectedColumn<T>> {
        let prepared = &*self.prepared;
        let positions = &prepared.key.positions;
        assert!(
            position < positions.len(),
            "the projection has {} columns, not a column at position {position}",
            positions.len()
        );
        let column = &prepared.reader.columns[positions[position]];
        if !T::accepts(&column.ty) {
            return Err(self.different(Different {
                column: Some(column.name.clone()),
                difference: SchemaDifference::Type,
                expected: T::NAME.to_owned(),
                found: column.ty.to_string(),
                index: None,
            }));
        }
        Ok(ProjectedColumn {
            key: Arc::clone(&prepared.key),
            at: position,
            _type: PhantomData,
        })
    }

    /// `row` bound to this projection, to read its columns with the projection's prepared columns. Fails with
    /// [`Error::DifferentSchema`] with the table, without effect, when the row is of another table, of another
    /// store or of another schema of the table (PY3). It reads no page.
    #[inline]
    pub fn bind<'p, 't>(&'p self, row: &'p TableRow<'t>) -> Result<ProjectedRow<'p, 't>> {
        let prepared = &*self.prepared;
        if row.reader.schema != prepared.key.schema {
            return Err(self.wrong_row());
        }
        let (plan, row) = match &row.stored {
            Stored::Owned(values) => (&prepared.current[..], Bound::Owned(values)),
            Stored::Row {
                key,
                starts,
                bytes,
                old,
                defaults,
            } => {
                let (plan, layout) = match old {
                    None => (&prepared.current[..], &prepared.reader.layout),
                    Some(old) => {
                        let plan =
                            prepared.older[usize::from(old.layout.version())].get_or_init(|| {
                                Prepared::plan(&prepared.reader, &prepared.key.positions, Some(old))
                            });
                        (&plan[..], &old.layout)
                    }
                };
                (
                    plan,
                    Bound::Row {
                        key,
                        starts: starts.as_deref(),
                        bytes,
                        layout,
                        defaults: defaults.as_deref().map(Vec::as_slice),
                    },
                )
            }
        };
        Ok(ProjectedRow {
            prepared,
            plan,
            row,
        })
    }

    /// Whether the store keeps this projection (a test hook, PY5).
    #[doc(hidden)]
    pub fn is_kept(&self) -> bool {
        self.prepared.kept
    }

    /// Whether `other` is the same projection as this one, the one the store keeps for both (a test hook).
    #[doc(hidden)]
    pub fn same_as(&self, other: &Projection<'_>) -> bool {
        Arc::ptr_eq(&self.prepared, &other.prepared)
    }
}

impl<'t> ProjectedRow<'_, 't> {
    /// The value of `column` in the row, or `None` for a null (PY3): the value reading the column by its position
    /// gives, as `T`. Panics if `column` is of another projection, or of the same columns in another order: a fault
    /// of the program.
    #[inline]
    pub fn get<T: ReadAs>(&self, column: &ProjectedColumn<T>) -> Option<T::Output<'t>> {
        let list = &self.prepared.key;
        if !Arc::ptr_eq(&column.key, list) && *column.key != **list {
            panic!("the prepared column is of another projection");
        }
        let planned = &self.plan[column.at];
        let ty = &planned.ty;
        let value = match self.row {
            Bound::Row {
                key,
                starts,
                bytes,
                layout,
                defaults,
            } => match planned.access {
                Access::Fixed(fixed) => return fixed.read(bytes).map(|b| T::stored(ty, b)),
                Access::Variable(slot) => {
                    return layout.slot_bytes(bytes, slot).map(|b| T::stored(ty, b));
                }
                Access::Key(k) => {
                    let start = starts.map_or(0, |starts| usize::from(starts[k]));
                    return Some(T::key(ty, &key[start..]));
                }
                Access::Constant(ref value) => value.clone(),
                Access::Default => {
                    defaults.expect("the stored defaults of an older row")[planned.position].clone()
                }
            },
            Bound::Owned(values) => values[planned.position].clone(),
        };
        match value {
            Value::Null => None,
            value => Some(T::value(value)),
        }
    }
}

impl<'s> ReadTransaction<'s> {
    /// A projection of `columns` of `table` (PY1), as the snapshot sees it, or the one the store keeps for the same
    /// columns in the same order and the same schema (PY5). Fails, without effect, with [`Error::InvalidSize`] for a
    /// name outside 1 to 1,024 bytes, with [`Error::DifferentSchema`] for a name that is not of a live column, and
    /// with [`Error::InvalidValue`] for a repeated column or an empty list, in that order, the first name in the
    /// list's order within each. It reads no page.
    pub fn projection(&self, table: &ReadTable<'_>, columns: &[&str]) -> Result<Projection<'s>> {
        for name in columns {
            self.folder.check_column_name(name)?;
        }
        self.check_table(table)?;
        prepare(
            self.projections,
            self.folder,
            &table.name,
            &table.reader,
            columns,
        )
    }
}

impl<'s> WriteTransaction<'s> {
    /// A projection of `columns` of `table` (PY1), as this transaction sees it, with the failures of
    /// [`ReadTransaction::projection`] and with [`Error::TreeNotFound`] after the size of the names when the table
    /// was deleted. It does not change the transaction (E16, T10).
    pub fn projection(&self, table: &WriteTable<'_>, columns: &[&str]) -> Result<Projection<'s>> {
        for name in columns {
            self.folder.check_column_name(name)?;
        }
        let state = self.live_table(table)?;
        prepare(
            self.projections,
            self.folder,
            &state.name,
            &state.reader,
            columns,
        )
    }
}

/// The projection of `columns` of the table `name`, whose current schema `reader` reads, after the checks of the
/// names' sizes and of the handle: an unknown column, the first in the list's order (E6); a repeated column, the
/// first that repeats; an empty list (E17).
fn prepare<'s>(
    projections: &'s Projections,
    folder: Folder<'_>,
    name: &str,
    reader: &Arc<Reader>,
    columns: &[&str],
) -> Result<Projection<'s>> {
    let fail = |fault: Fault| fault.error(folder, name);
    let positions = columns
        .iter()
        .map(|column| {
            reader
                .names
                .get(*column)
                .copied()
                .ok_or_else(|| fail(Fault::Unknown((*column).to_owned())))
        })
        .collect::<Result<Box<[usize]>>>()?;
    for (at, position) in positions.iter().enumerate() {
        if positions[..at].contains(position) {
            return Err(fail(Fault::Value(
                Some(columns[at].to_owned()),
                ValueRule::RepeatedColumn,
            )));
        }
    }
    if positions.is_empty() {
        return Err(fail(Fault::Value(None, ValueRule::EmptyProjection)));
    }
    let key = ListKey {
        schema: reader.schema,
        positions,
    };
    let prepared = projections.keep(key, |key, kept| Prepared {
        current: Prepared::plan(reader, &key.positions, None),
        older: (0..reader.layout.version())
            .map(|_| OnceLock::new())
            .collect(),
        key,
        folder: folder.owned(),
        reader: Arc::clone(reader),
        kept,
    });
    Ok(Projection {
        prepared,
        table: name.into(),
        _store: PhantomData,
    })
}

mod sealed {
    pub trait Sealed {}
}

/// A type a column is read as, with a [`ProjectedColumn`] or a typed read of [`TableRow`] (PY2): the column's Rust
/// type where it has one, and a marker of this module otherwise ([`Text`], [`TextBytes`], [`Bytes`], [`Date`],
/// [`Time`], [`Timestamp`], [`Duration`], [`Vector`]). `Output` is the value read, as [`Value`] holds it. A `decimal`,
/// a `uuid` and a vector column are read as [`Decimal`], [`Uuid`] and [`Vector`] whatever their precision and scale,
/// key order, element type or dimension; a `string` column also as [`TextBytes`], its bytes without checking them.
pub trait ReadAs: sealed::Sealed + 'static {
    /// The value read, borrowing the row's text, bytes or vector for `'t`.
    type Output<'t>;
    /// The type's name, for the errors and the panics.
    #[doc(hidden)]
    const NAME: &'static str;
    /// Whether a column of type `ty` is read as this type.
    #[doc(hidden)]
    fn accepts(ty: &ColumnType) -> bool;
    /// The value from its stored bytes in a row, of a column of type `ty` that this type accepts: as the row's
    /// decoding gives it, or a value that is not the written one, or a panic, for a malformed row (L2).
    #[doc(hidden)]
    fn stored<'t>(ty: &ColumnType, bytes: &'t [u8]) -> Self::Output<'t>;
    /// The value from a [`Value`] of a column this type accepts, not null.
    #[doc(hidden)]
    fn value(value: Value<'_>) -> Self::Output<'_>;
    /// The value from its element in a primary key, of a fixed-width column of type `ty`.
    #[doc(hidden)]
    #[inline]
    fn key<'t>(ty: &ColumnType, element: &[u8]) -> Self::Output<'t> {
        Self::value(key_value(ty, element))
    }
}

/// A `string` column read as its text, which panics on bytes that are not UTF-8 (L2).
pub struct Text;
/// A `string` column read as the bytes of its text, without checking them (L2).
pub struct TextBytes;
/// A `bytes` column.
pub struct Bytes;
/// A `date` column, read as days since 1970-01-01.
pub struct Date;
/// A `time` column, read as nanoseconds since midnight.
pub struct Time;
/// A `timestamp` column, read as nanoseconds since the Unix epoch, UTC.
pub struct Timestamp;
/// A `duration` column, read as nanoseconds.
pub struct Duration;
/// A vector column, read as its elements and, for `i8`, its scale, as [`Value::Vector`] holds them.
pub struct Vector;

/// Implements [`ReadAs`] for `$read`, read as `$out` from columns of types `$ty`, with `$stored` from the stored bytes
/// and from `Value::$variant` through `$value`.
macro_rules! read_as {
    ($($read:ty, $name:literal, $ty:pat, $out:ty, $variant:ident, $value:expr, $stored:expr;)*) => {
        $(
            impl sealed::Sealed for $read {}

            impl ReadAs for $read {
                type Output<'t> = $out;
                const NAME: &'static str = $name;

                #[inline]
                fn accepts(ty: &ColumnType) -> bool {
                    matches!(ty, $ty)
                }

                #[inline]
                fn stored<'t>(ty: &ColumnType, bytes: &'t [u8]) -> Self::Output<'t> {
                    ($stored)(ty, bytes)
                }

                #[inline]
                fn value(value: Value<'_>) -> Self::Output<'_> {
                    match value {
                        Value::$variant(v) => ($value)(v),
                        _ => unreachable!("a value of its column's type"),
                    }
                }
            }
        )*
    };
}

/// The fixed-width types, whose value is a copy: `$stored` decodes its little-endian bytes.
macro_rules! read_as_number {
    ($($read:ty, $name:literal, $ty:pat, $variant:ident;)*) => {
        read_as! {
            $($read, $name, $ty, $read, $variant, |v| v, |_: &ColumnType, b: &[u8]| {
                <$read>::from_le_bytes(slot_array(b))
            };)*
        }
    };
}

read_as_number! {
    i8, "i8", ColumnType::I8, I8;
    i16, "i16", ColumnType::I16, I16;
    i32, "i32", ColumnType::I32, I32;
    i64, "i64", ColumnType::I64, I64;
    u8, "u8", ColumnType::U8, U8;
    u16, "u16", ColumnType::U16, U16;
    u32, "u32", ColumnType::U32, U32;
    u64, "u64", ColumnType::U64, U64;
    f32, "f32", ColumnType::F32, F32;
    f64, "f64", ColumnType::F64, F64;
}

read_as! {
    bool, "bool", ColumnType::Bool, bool, Bool, |v| v, |_: &ColumnType, b: &[u8]| b == [1];
    Decimal, "decimal", ColumnType::Decimal { .. }, Decimal, Decimal, |v| v, |ty: &ColumnType, b: &[u8]| {
        let ColumnType::Decimal { scale, .. } = *ty else {
            unreachable!("a decimal column")
        };
        Decimal {
            mantissa: i128::from_le_bytes(slot_array(b)),
            scale,
        }
    };
    Date, "date", ColumnType::Date, i32, Date, |v| v, |_: &ColumnType, b: &[u8]| {
        i32::from_le_bytes(slot_array(b))
    };
    Time, "time", ColumnType::Time, i64, Time, |v| v, |_: &ColumnType, b: &[u8]| {
        i64::from_le_bytes(slot_array(b)) * i64::try_from(NANOS_PER_MICRO).expect("1,000")
    };
    Timestamp, "timestamp", ColumnType::Timestamp, i128, Timestamp, |v| v, |_: &ColumnType, b: &[u8]| {
        i128::from(i64::from_le_bytes(slot_array(b))) * NANOS_PER_MICRO
    };
    Duration, "duration", ColumnType::Duration, i128, Duration, |v| v, |_: &ColumnType, b: &[u8]| {
        i128::from(i64::from_le_bytes(slot_array(b))) * NANOS_PER_MICRO
    };
    Uuid, "uuid", ColumnType::Uuid(_), Uuid, Uuid, |v| v, |_: &ColumnType, b: &[u8]| Uuid(slot_array(b));
    Text, "string", ColumnType::String, Cow<'t, str>, String, |v| v, |_: &ColumnType, b: &'t [u8]| {
        Cow::Borrowed(std::str::from_utf8(b).expect("a malformed row: a text that is not UTF-8"))
    };
    TextBytes, "string", ColumnType::String, Cow<'t, [u8]>, String, text_bytes, |_: &ColumnType, b: &'t [u8]| {
        Cow::Borrowed(b)
    };
    Bytes, "bytes", ColumnType::Bytes, Cow<'t, [u8]>, Bytes, |v| v, |_: &ColumnType, b: &'t [u8]| {
        Cow::Borrowed(b)
    };
    Vector, "vector", ColumnType::Vector(_), Cow<'t, [u8]>, Vector, |v| v, |ty: &ColumnType, b: &'t [u8]| {
        let ColumnType::Vector(vector) = ty else {
            unreachable!("a vector column")
        };
        // The stored bytes of a cosine column end with the norm, which the value does not have.
        Cow::Borrowed(&b[..vector.value_len()])
    };
}

/// The bytes of a text, borrowed or owned as the text is.
fn text_bytes(text: Cow<'_, str>) -> Cow<'_, [u8]> {
    match text {
        Cow::Borrowed(text) => Cow::Borrowed(text.as_bytes()),
        Cow::Owned(text) => Cow::Owned(text.into_bytes()),
    }
}

impl sealed::Sealed for Ulid {}

impl ReadAs for Ulid {
    type Output<'t> = Ulid;
    const NAME: &'static str = "ulid";

    #[inline]
    fn accepts(ty: &ColumnType) -> bool {
        matches!(ty, ColumnType::Ulid)
    }

    #[inline]
    fn stored<'t>(_: &ColumnType, bytes: &'t [u8]) -> Self::Output<'t> {
        Ulid(slot_array(bytes))
    }

    #[inline]
    fn value(value: Value<'_>) -> Ulid {
        match value {
            Value::Ulid(v) => v,
            _ => unreachable!("a value of its column's type"),
        }
    }

    /// Its element in a key is a type code and the 16 bytes.
    #[inline]
    fn key<'t>(_: &ColumnType, element: &[u8]) -> Self::Output<'t> {
        Ulid(slot_array(&element[1..17]))
    }
}
