//! The row format "row v1" (document 1 of the table design, *Decided design*). A row is one tree value, the table
//! tree's key being the primary key, laid out by its schema version, all integers little-endian:
//!
//! ```text
//! 2 bytes          schema version
//! 1 byte           flags: bits 0-1 the width of the end offsets (0 = 1 byte, 1 = 2, 2 = 4); bit 2 "compressed",
//!                  the hook of document 7, always clear; bits 3-7 zero
//! ceil(N / 8)      null bitmap, one bit per nullable column in version order, 1 = null; absent when N = 0
//! F                fixed section: the columns of up to 16 bytes, by descending width, ties in version order
//! (V - 1) × w      end offsets of the variable columns but the last, relative to the variable section
//! rest             variable section: `string`, `bytes` and vectors in version order
//! ```
//!
//! Fixed-width primary key columns are not in the row, only in the key; `string` and `bytes` primary key columns
//! are in both, so that they read without a copy. A null fixed column keeps its slot (zeros); a null variable column
//! takes no bytes. Reads are zero-copy and O(1) per column, and never panic on bytes that do not fit the layout:
//! they report [`RowDamaged`].

use std::borrow::Cow;
use std::cmp::Reverse;
use std::convert::Infallible;

use super::value::{
    ColumnType, Decimal, NANOS_PER_MICRO, Value, fits_precision, is_of_type, stored_in_domain,
    vector_norm,
};
use crate::ids::{Ulid, Uuid};

/// The largest row, in its stored form: the largest value of a tree (B6, E18).
pub const MAX_ROW: usize = krummi_tree::MAX_VALUE;

/// The bytes of the header: the schema version and the flags.
const HEADER: usize = 3;

/// A row whose bytes do not fit its layout: the caller reports it as damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowDamaged;

/// A column of a schema version, as the row layout needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct RowColumn {
    /// The column's identity (document 11, P12), which maps the columns of rows of older versions.
    pub id: u64,
    pub ty: ColumnType,
    pub nullable: bool,
    /// Whether the column is in the primary key.
    pub key: bool,
}

/// Where a column of a version is in its rows.
#[derive(Debug, Clone, Copy)]
enum Place {
    /// A fixed-width primary key column: only in the key.
    Key,
    /// In the fixed section, at this offset from the row's start.
    Fixed { offset: usize, width: usize },
    /// The variable column of this index.
    Variable(usize),
}

/// Where a column of a row of one layout is, for [`RowLayout::slot_bytes`]: its place and, when it may be null, the
/// byte and mask of its bit in the null bitmap. Computed once per column ([`RowLayout::slot`]).
#[derive(Debug, Clone, Copy)]
pub struct Slot {
    place: SlotPlace,
    /// The byte of its bit in the null bitmap, and the bit's mask: 0 for a column that does not take nulls.
    null_byte: u32,
    null_mask: u8,
}

/// A column's [`Place`] in a [`Slot`], in 32-bit numbers: a row is at most 256 MiB.
#[derive(Debug, Clone, Copy)]
enum SlotPlace {
    Key,
    Fixed { offset: u32, width: u32 },
    Variable(u32),
}

/// A column of the fixed section of one layout's rows: its bytes `start..end`, null when `null_mask` is set in the
/// byte `null_byte` (a mask of 0 for a column without nulls). The one read of that section, shared by
/// [`RowLayout::slot_bytes`] and the projection.
#[derive(Debug, Clone, Copy)]
pub struct FixedSlot {
    start: u32,
    end: u32,
    null_byte: u32,
    null_mask: u8,
}

impl FixedSlot {
    /// The column's bytes in `bytes`, a row of its layout, or `None` for a null.
    #[inline]
    pub fn read(self, bytes: &[u8]) -> Option<&[u8]> {
        if null_bit(bytes, self.null_byte, self.null_mask) {
            return None;
        }
        Some(&bytes[self.start as usize..self.end as usize])
    }
}

/// Whether the null bit of mask `mask` in the byte `byte` of `bytes` is set; never for a mask of 0.
#[inline]
fn null_bit(bytes: &[u8], byte: u32, mask: u8) -> bool {
    mask != 0 && bytes[byte as usize] & mask != 0
}

impl Slot {
    /// Whether the column is null in `bytes`.
    #[inline]
    fn is_null(self, bytes: &[u8]) -> bool {
        null_bit(bytes, self.null_byte, self.null_mask)
    }

    /// Whether the column is in the fixed section of its rows.
    #[inline]
    pub fn is_fixed(self) -> bool {
        matches!(self.place, SlotPlace::Fixed { .. })
    }

    /// For a column of the fixed section, where it is: what [`RowLayout::slot_bytes`] reads.
    #[inline]
    pub fn fixed(self) -> Option<FixedSlot> {
        match self.place {
            SlotPlace::Fixed { offset, width } => Some(self.fixed_at(offset, width)),
            _ => None,
        }
    }

    /// This fixed-section column, at `offset` with `width` bytes, as a [`FixedSlot`].
    #[inline]
    fn fixed_at(self, offset: u32, width: u32) -> FixedSlot {
        FixedSlot {
            start: offset,
            end: offset + width,
            null_byte: self.null_byte,
            null_mask: self.null_mask,
        }
    }

    /// Writes `value`, normalized for this fixed-width column, into `row`, a row of its layout, as
    /// [`RowLayout::encode`] writes it: its bytes and a clear null bit, or for a null, zeros and a set null bit.
    /// Panics for a column outside the fixed section, or a null where the column takes none.
    pub fn write(self, row: &mut [u8], value: &Value<'_>) {
        let SlotPlace::Fixed { offset, width } = self.place else {
            panic!("a fixed-width column");
        };
        let null = matches!(value, Value::Null);
        if self.null_mask != 0 {
            let bits = &mut row[self.null_byte as usize];
            *bits = if null {
                *bits | self.null_mask
            } else {
                *bits & !self.null_mask
            };
        } else {
            assert!(!null, "a null in a column that does not allow nulls");
        }
        let bytes = if null {
            [0; FIXED_BYTES]
        } else {
            fixed_bytes(value)
        };
        let (offset, width) = (offset as usize, width as usize);
        row[offset..offset + width].copy_from_slice(&bytes[..width]);
    }
}

/// The layout of the rows of one schema version: computed from its columns, never stored (document 1).
#[derive(Debug, Clone)]
pub struct RowLayout {
    version: u16,
    columns: Vec<RowColumn>,
    places: Vec<Place>,
    /// The bit of each nullable column in the null bitmap.
    null_bits: Vec<Option<usize>>,
    /// Where the end offsets start: after the header, the bitmap and the fixed section.
    offsets_start: usize,
    variables: usize,
}

impl RowLayout {
    /// The layout of `version`, whose columns are `columns` in version order (the order they were added).
    pub fn new(version: u16, columns: Vec<RowColumn>) -> RowLayout {
        let in_row = |c: &RowColumn| !c.key || c.ty.fixed_width().is_none();
        let mut null_bits = Vec::with_capacity(columns.len());
        let mut nullable = 0;
        for column in &columns {
            null_bits.push((in_row(column) && column.nullable).then(|| {
                nullable += 1;
                nullable - 1
            }));
        }
        let bitmap_len = usize::div_ceil(nullable, 8);
        let mut fixed: Vec<(usize, usize)> = columns
            .iter()
            .enumerate()
            .filter(|(_, c)| in_row(c))
            .filter_map(|(i, c)| c.ty.fixed_width().map(|width| (i, width)))
            .collect();
        fixed.sort_by_key(|&(i, width)| (Reverse(width), i));
        let mut places = vec![Place::Key; columns.len()];
        let mut offset = HEADER + bitmap_len;
        for (i, width) in fixed {
            places[i] = Place::Fixed { offset, width };
            offset += width;
        }
        let mut variables = 0;
        for (i, column) in columns.iter().enumerate() {
            if column.ty.fixed_width().is_none() {
                places[i] = Place::Variable(variables);
                variables += 1;
            }
        }
        RowLayout {
            version,
            columns,
            places,
            null_bits,
            offsets_start: offset,
            variables,
        }
    }

    pub fn version(&self) -> u16 {
        self.version
    }

    /// The columns of the version, in version order.
    pub fn columns(&self) -> &[RowColumn] {
        &self.columns
    }

    /// Whether [`Row::check`] can reject the column at `position`: false for a key column and for a fixed-width one
    /// whose every value decodes.
    pub fn needs_check(&self, position: usize) -> bool {
        match self.places.get(position) {
            Some(Place::Fixed { .. }) => !always_decodes(&self.columns[position].ty),
            Some(Place::Variable(_)) => true,
            Some(Place::Key) | None => false,
        }
    }

    /// Where the column at `position` is in the rows of this layout, if the version has it.
    pub fn slot(&self, position: usize) -> Option<Slot> {
        let narrow = |n: usize| u32::try_from(n).expect("a row of at most 256 MiB");
        let place = match *self.places.get(position)? {
            Place::Key => SlotPlace::Key,
            Place::Fixed { offset, width } => SlotPlace::Fixed {
                offset: narrow(offset),
                width: narrow(width),
            },
            Place::Variable(j) => SlotPlace::Variable(narrow(j)),
        };
        let (null_byte, null_mask) = self.null_bits[position]
            .map_or((0, 0), |bit| (narrow(HEADER + bit / 8), 1 << (bit % 8)));
        Some(Slot {
            place,
            null_byte,
            null_mask,
        })
    }

    /// The stored bytes of the column at `slot` in `bytes`, a row of this layout that [`RowLayout::row`] accepted: as
    /// [`Row::column_bytes`], without reading the header again and without its checks. `None` for a null, or a key
    /// column. Panics, or gives other bytes, on a column that [`Row::check`] rejects.
    #[inline]
    pub fn slot_bytes<'r>(&self, bytes: &'r [u8], slot: Slot) -> Option<&'r [u8]> {
        match slot.place {
            SlotPlace::Key => None,
            SlotPlace::Fixed { offset, width } => slot.fixed_at(offset, width).read(bytes),
            SlotPlace::Variable(j) => {
                if slot.is_null(bytes) {
                    return None;
                }
                let j = j as usize;
                let width = 1usize << bytes[2];
                let var_start = self.offsets_start + (self.variables - 1) * width;
                let end_of = |j: usize| {
                    let at = self.offsets_start + j * width;
                    var_start + read_offset(&bytes[at..at + width])
                };
                let start = if j == 0 { var_start } else { end_of(j - 1) };
                let end = if j + 1 == self.variables {
                    bytes.len()
                } else {
                    end_of(j)
                };
                Some(&bytes[start..end])
            }
        }
    }

    /// The position of the column `id` in this version, if it has it. A scan computes it once per version and
    /// column (document 1, *Schema evolution*).
    pub fn position(&self, id: u64) -> Option<usize> {
        self.columns.iter().position(|c| c.id == id)
    }

    /// The stored length of the row of `values` (see [`RowLayout::encode`]), without encoding it: the size that
    /// [`MAX_ROW`] limits (E18).
    pub fn encoded_len(&self, values: &[Value<'_>]) -> usize {
        let variable = self.variable_len(values);
        self.offsets_start + self.variables.saturating_sub(1) * offset_width(variable).1 + variable
    }

    /// The row of `values`, one per column in version order, normalized for their columns (see
    /// [`super::value::normalize`]); the values of fixed-width primary key columns are not stored. Panics if
    /// `values` does not have one value per column, a value is not of its column's type, or a null is in a column
    /// that does not allow nulls: a slot is never written with another type's bytes, nor a null as zeros.
    pub fn encode(&self, values: &[Value<'_>]) -> Vec<u8> {
        let mut row = Vec::new();
        self.encode_into(values, &mut row);
        row
    }

    /// Writes the row of `values` (see [`RowLayout::encode`]) into the empty `row`, which allocates nothing when it
    /// already has room for [`RowLayout::encoded_len`] bytes: the payload of an index entry's included columns is
    /// reserved first, so that a refused allocation fails the write instead of the process (IX2, P13).
    pub(crate) fn encode_into(&self, values: &[Value<'_>], row: &mut Vec<u8>) {
        debug_assert!(row.is_empty(), "the row is written from its start");
        assert_eq!(values.len(), self.columns.len(), "one value per column");
        for (column, value) in self.columns.iter().zip(values) {
            match value {
                Value::Null => assert!(
                    column.nullable && !column.key,
                    "a null in a column that does not allow nulls"
                ),
                value => assert!(
                    is_of_type(&column.ty, value),
                    "a value of another type than its column"
                ),
            }
        }
        let variable = self.variable_len(values);
        let (code, width) = offset_width(variable);
        let var_start = self.offsets_start + self.variables.saturating_sub(1) * width;
        row.reserve_exact(var_start + variable);
        row.resize(var_start, 0);
        row[..2].copy_from_slice(&self.version.to_le_bytes());
        row[2] = code;
        // The end of each variable column; a null one takes no bytes and ends where the previous one ends.
        let mut end = 0;
        for (i, value) in values.iter().enumerate() {
            let null = matches!(value, Value::Null);
            if let (true, Some(bit)) = (null, self.null_bits[i]) {
                row[HEADER + bit / 8] |= 1 << (bit % 8);
            }
            match self.places[i] {
                Place::Key => {}
                Place::Fixed { .. } if null => {}
                Place::Fixed { offset, width } => {
                    row[offset..offset + width].copy_from_slice(&fixed_bytes(value)[..width]);
                }
                Place::Variable(j) => {
                    end += variable_bytes_len(&self.columns[i].ty, value);
                    if j + 1 < self.variables {
                        let at = self.offsets_start + j * width;
                        row[at..at + width].copy_from_slice(&(end as u32).to_le_bytes()[..width]);
                    }
                }
            }
        }
        for (i, value) in values.iter().enumerate() {
            if let Place::Variable(_) = self.places[i] {
                write_variable(&self.columns[i].ty, value, row);
            }
        }
    }

    /// The row in `bytes`, after checking its header and that it holds the fixed part of its layout; the columns are
    /// checked as they are read. Fails if the row is of another version (see [`row_version`]).
    #[inline]
    pub fn row<'r>(&self, bytes: &'r [u8]) -> Result<Row<'_, 'r>, RowDamaged> {
        if row_version(bytes)? != self.version {
            return Err(RowDamaged);
        }
        let flags = bytes[2];
        let width = match flags {
            0 => 1,
            1 => 2,
            2 => 4,
            // An unknown width, the compressed bit (not until document 7) or a reserved bit.
            _ => return Err(RowDamaged),
        };
        let var_start = self.offsets_start + self.variables.saturating_sub(1) * width;
        if bytes.len() < var_start {
            return Err(RowDamaged);
        }
        Ok(Row {
            layout: self,
            bytes,
            width,
            var_start,
        })
    }

    fn variable_len(&self, values: &[Value<'_>]) -> usize {
        values
            .iter()
            .zip(&self.columns)
            .filter(|(_, c)| c.ty.fixed_width().is_none())
            .map(|(v, c)| variable_bytes_len(&c.ty, v))
            .sum()
    }
}

/// The schema version of a row, which picks its layout.
pub fn row_version(bytes: &[u8]) -> Result<u16, RowDamaged> {
    match bytes {
        [low, high, _, ..] => Ok(u16::from_le_bytes([*low, *high])),
        _ => Err(RowDamaged),
    }
}

/// The flag code and width of the end offsets: the smallest that holds every offset, the largest being the size of
/// the variable section (document 1: written by the builder, not inferred from the size).
fn offset_width(variable: usize) -> (u8, usize) {
    if variable <= usize::from(u8::MAX) {
        (0, 1)
    } else if variable <= usize::from(u16::MAX) {
        (1, 2)
    } else {
        (2, 4)
    }
}

fn read_offset(bytes: &[u8]) -> usize {
    let mut le = [0u8; 4];
    le[..bytes.len()].copy_from_slice(bytes);
    u32::from_le_bytes(le) as usize
}

/// The bytes of a value of the variable section: a vector of a cosine column takes its norm too.
fn variable_bytes_len(ty: &ColumnType, value: &Value<'_>) -> usize {
    match (ty, value) {
        (_, Value::String(text)) => text.len(),
        (_, Value::Bytes(bytes)) => bytes.len(),
        (ColumnType::Vector(vector), Value::Vector(bytes)) => {
            bytes.len() + if vector.cosine { 8 } else { 0 }
        }
        _ => 0,
    }
}

fn write_variable(ty: &ColumnType, value: &Value<'_>, row: &mut Vec<u8>) {
    match (ty, value) {
        (_, Value::String(text)) => row.extend_from_slice(text.as_bytes()),
        (_, Value::Bytes(bytes)) => row.extend_from_slice(bytes),
        (ColumnType::Vector(vector), Value::Vector(bytes)) => {
            row.extend_from_slice(bytes);
            if vector.cosine {
                row.extend_from_slice(&vector_norm(vector, bytes).to_le_bytes());
            }
        }
        (_, Value::Null) => {}
        _ => panic!("a value of another type than its variable column"),
    }
}

/// The bytes of the widest fixed-width value, a `decimal`, `uuid` or `ulid`.
const FIXED_BYTES: usize = 16;

/// The little-endian bytes of a fixed-width value (document 1, *Column types*): temporal values in microseconds,
/// a `decimal` as its scaled 128-bit integer, `uuid` and `ulid` as their 16 bytes.
fn fixed_bytes(value: &Value<'_>) -> [u8; FIXED_BYTES] {
    let mut out = [0u8; FIXED_BYTES];
    let micros = |nanos: i128| {
        i64::try_from(nanos / NANOS_PER_MICRO).expect("a temporal value within its domain")
    };
    let bytes: &[u8] = match value {
        Value::Bool(v) => &[u8::from(*v)],
        Value::I8(v) => &v.to_le_bytes(),
        Value::I16(v) => &v.to_le_bytes(),
        Value::I32(v) => &v.to_le_bytes(),
        Value::I64(v) => &v.to_le_bytes(),
        Value::U8(v) => &v.to_le_bytes(),
        Value::U16(v) => &v.to_le_bytes(),
        Value::U32(v) => &v.to_le_bytes(),
        Value::U64(v) => &v.to_le_bytes(),
        Value::F32(v) => &v.to_le_bytes(),
        Value::F64(v) => &v.to_le_bytes(),
        Value::Decimal(d) => &d.mantissa.to_le_bytes(),
        Value::Date(days) => &days.to_le_bytes(),
        Value::Time(nanos) => &micros(i128::from(*nanos)).to_le_bytes(),
        Value::Timestamp(nanos) | Value::Duration(nanos) => &micros(*nanos).to_le_bytes(),
        Value::Uuid(uuid) => &uuid.0,
        Value::Ulid(ulid) => &ulid.0,
        Value::Null | Value::String(_) | Value::Bytes(_) | Value::Vector(_) => {
            panic!("a value of another type than its fixed column")
        }
    };
    out[..bytes.len()].copy_from_slice(bytes);
    out
}

/// A row read with its layout. Every access is O(1) and borrows from the row's bytes.
#[derive(Debug, Clone, Copy)]
pub struct Row<'l, 'r> {
    layout: &'l RowLayout,
    bytes: &'r [u8],
    width: usize,
    var_start: usize,
}

impl<'r> Row<'_, 'r> {
    /// The stored length of the row: the size that [`MAX_ROW`] limits (E18).
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the row has no bytes, which a row never has: it always has its header.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Whether the column at `position` is null.
    #[inline]
    pub fn is_null(&self, position: usize) -> bool {
        self.layout
            .null_bits
            .get(position)
            .copied()
            .flatten()
            .is_some_and(|bit| self.bytes[HEADER + bit / 8] & (1 << (bit % 8)) != 0)
    }

    /// The stored bytes of the column at `position`: its fixed slot or its part of the variable section, a vector
    /// with its norm in a cosine column. `None` when the column is null or not in the row (a fixed-width primary key
    /// column, or a position past the version's columns).
    #[inline]
    pub fn column_bytes(&self, position: usize) -> Result<Option<&'r [u8]>, RowDamaged> {
        let Some(place) = self.layout.places.get(position) else {
            return Ok(None);
        };
        let null = self.is_null(position);
        match *place {
            Place::Key => Ok(None),
            Place::Fixed { .. } if null => Ok(None),
            Place::Fixed { offset, width } => Ok(Some(&self.bytes[offset..offset + width])),
            Place::Variable(j) => {
                let bytes = self.variable(j)?;
                match (null, bytes.is_empty()) {
                    (true, true) => Ok(None),
                    (true, false) => Err(RowDamaged),
                    (false, _) => Ok(Some(bytes)),
                }
            }
        }
    }

    /// The value of the column at `position`: [`Value::Null`] when it is null, `None` when it is not in the row
    /// (see [`Row::column_bytes`]). Text, bytes and vectors borrow the row; a vector comes without its norm.
    pub fn value(&self, position: usize) -> Result<Option<Value<'r>>, RowDamaged> {
        let Some(bytes) = self.column_bytes(position)? else {
            return Ok(match self.layout.places.get(position) {
                Some(Place::Fixed { .. } | Place::Variable(_)) => Some(Value::Null),
                _ => None,
            });
        };
        decode(&self.layout.columns[position].ty, bytes).map(Some)
    }

    /// Checks the column at `position` as [`Row::value`] reads it, without building its value: `Ok` for a column of
    /// the row, null or not, and an error for one that does not decode or is not in the row.
    #[inline]
    pub fn check(&self, position: usize) -> Result<(), RowDamaged> {
        match self.layout.places.get(position) {
            // Its slot is inside the fixed part that `RowLayout::row` checked, and every value of its width decodes.
            Some(Place::Fixed { .. }) if always_decodes(&self.layout.columns[position].ty) => {
                Ok(())
            }
            Some(Place::Fixed { .. } | Place::Variable(_)) => match self.column_bytes(position)? {
                Some(bytes) => decode(&self.layout.columns[position].ty, bytes).map(drop),
                None => Ok(()),
            },
            _ => Err(RowDamaged),
        }
    }

    /// Checks the column at `slot` of type `ty`, as [`Row::check`] does for its position, from its slot.
    #[inline]
    pub fn check_slot(&self, slot: Slot, ty: &ColumnType) -> Result<(), RowDamaged> {
        let null = slot.is_null(self.bytes);
        let stored = match slot.place {
            SlotPlace::Key => return Err(RowDamaged),
            SlotPlace::Fixed { .. } if null => return Ok(()),
            SlotPlace::Fixed { offset, width } => {
                let offset = offset as usize;
                &self.bytes[offset..offset + width as usize]
            }
            SlotPlace::Variable(j) => {
                let stored = self.variable(j as usize)?;
                match (null, stored.is_empty()) {
                    (true, true) => return Ok(()),
                    (true, false) => return Err(RowDamaged),
                    (false, _) => stored,
                }
            }
        };
        check_stored(ty, stored)
    }

    /// The value of the column at `position` without a [`Result`]: [`Row::value`] for a column that [`Row::check`]
    /// accepts, and a panic for one it rejects, for a row whose content was not checked (L2 of the row reads PRD).
    #[inline]
    pub fn read(&self, position: usize) -> Value<'r> {
        const MALFORMED: &str = "a malformed row: a column that does not decode";
        let malformed = || -> Infallible { panic!("{MALFORMED}") };
        match self.column_bytes(position) {
            Ok(Some(bytes)) => {
                match decode_with(&self.layout.columns[position].ty, bytes, malformed) {
                    Ok(value) => value,
                    Err(never) => match never {},
                }
            }
            Ok(None) => Value::Null,
            Err(RowDamaged) => panic!("{MALFORMED}"),
        }
    }

    /// The value of the column `id` (see [`Row::value`]), or `default` when the row's version does not have the
    /// column: an added column reads null or its default (N3). Finding the column costs a search of the version's
    /// columns; a scan uses [`RowLayout::position`] once and [`Row::value`].
    pub fn value_by_id<'d>(
        &self,
        id: u64,
        default: &Value<'d>,
    ) -> Result<Option<Value<'d>>, RowDamaged>
    where
        'r: 'd,
    {
        match self.layout.position(id) {
            Some(position) => self.value(position),
            None => Ok(Some(default.clone())),
        }
    }

    /// The bytes of variable column `j`: two offset reads.
    #[inline]
    fn variable(&self, j: usize) -> Result<&'r [u8], RowDamaged> {
        let end_of = |j: usize| {
            let at = self.layout.offsets_start + j * self.width;
            read_offset(&self.bytes[at..at + self.width])
        };
        let section = &self.bytes[self.var_start..];
        let start = if j == 0 { 0 } else { end_of(j - 1) };
        let end = if j + 1 == self.layout.variables {
            section.len()
        } else {
            end_of(j)
        };
        section.get(start..end).ok_or(RowDamaged)
    }
}

/// The bytes of a fixed-width slot as an array of its width.
pub(crate) fn slot_array<const N: usize>(bytes: &[u8]) -> [u8; N] {
    bytes.try_into().expect("the slot's width")
}

/// Whether `bytes` are a stored value of type `ty` that [`decode`] accepts, checked without building the value: the
/// same rules, which `check_stored_accepts_what_decode_accepts` pins.
#[inline]
fn check_stored(ty: &ColumnType, bytes: &[u8]) -> Result<(), RowDamaged> {
    let valid = match ty {
        ColumnType::Bool => matches!(bytes, [0] | [1]),
        ColumnType::Date => stored_in_domain(ty, i128::from(i32::from_le_bytes(slot_array(bytes)))),
        ColumnType::Time | ColumnType::Timestamp | ColumnType::Duration => {
            stored_in_domain(ty, i128::from(i64::from_le_bytes(slot_array(bytes))))
        }
        ColumnType::Decimal { precision, .. } => {
            fits_precision(i128::from_le_bytes(slot_array(bytes)), *precision)
        }
        ColumnType::String => std::str::from_utf8(bytes).is_ok(),
        ColumnType::Vector(vector) => bytes.len() == vector.stored_len(),
        _ => true,
    };
    if valid { Ok(()) } else { Err(RowDamaged) }
}

/// Whether [`decode`] accepts every stored value of a fixed-width type `ty` of its width: true of the integers, the
/// floats, `uuid` and `ulid`; a `bool` other than 0 or 1, a temporal value outside its domain and a `decimal` over its
/// precision are damage.
fn always_decodes(ty: &ColumnType) -> bool {
    matches!(
        ty,
        ColumnType::I8
            | ColumnType::I16
            | ColumnType::I32
            | ColumnType::I64
            | ColumnType::U8
            | ColumnType::U16
            | ColumnType::U32
            | ColumnType::U64
            | ColumnType::F32
            | ColumnType::F64
            | ColumnType::Uuid(_)
            | ColumnType::Ulid
    )
}

/// The value of the stored bytes of a column of type `ty`. A temporal value outside its domain or a `decimal` over
/// its precision, which Krummi never writes, is damage.
#[inline]
fn decode<'r>(ty: &ColumnType, bytes: &'r [u8]) -> Result<Value<'r>, RowDamaged> {
    decode_with(ty, bytes, || RowDamaged)
}

/// [`decode`], with `damaged()` as the error of bytes that are not such a value: a read that panics instead
/// passes an error that cannot exist, so that the result has the value's own layout.
#[inline]
fn decode_with<'r, E>(
    ty: &ColumnType,
    bytes: &'r [u8],
    damaged: impl Fn() -> E,
) -> Result<Value<'r>, E> {
    let stored = match ty {
        ColumnType::Date => Some(i128::from(i32::from_le_bytes(slot_array(bytes)))),
        ColumnType::Time | ColumnType::Timestamp | ColumnType::Duration => {
            Some(i128::from(i64::from_le_bytes(slot_array(bytes))))
        }
        _ => None,
    };
    if stored.is_some_and(|stored| !stored_in_domain(ty, stored)) {
        return Err(damaged());
    }
    let nanos = |micros: [u8; 8]| i128::from(i64::from_le_bytes(micros)) * NANOS_PER_MICRO;
    Ok(match ty {
        ColumnType::Bool => match bytes {
            [0] => Value::Bool(false),
            [1] => Value::Bool(true),
            _ => return Err(damaged()),
        },
        ColumnType::I8 => Value::I8(i8::from_le_bytes(slot_array(bytes))),
        ColumnType::I16 => Value::I16(i16::from_le_bytes(slot_array(bytes))),
        ColumnType::I32 => Value::I32(i32::from_le_bytes(slot_array(bytes))),
        ColumnType::I64 => Value::I64(i64::from_le_bytes(slot_array(bytes))),
        ColumnType::U8 => Value::U8(u8::from_le_bytes(slot_array(bytes))),
        ColumnType::U16 => Value::U16(u16::from_le_bytes(slot_array(bytes))),
        ColumnType::U32 => Value::U32(u32::from_le_bytes(slot_array(bytes))),
        ColumnType::U64 => Value::U64(u64::from_le_bytes(slot_array(bytes))),
        ColumnType::F32 => Value::F32(f32::from_le_bytes(slot_array(bytes))),
        ColumnType::F64 => Value::F64(f64::from_le_bytes(slot_array(bytes))),
        ColumnType::Decimal { precision, scale } => {
            let mantissa = i128::from_le_bytes(slot_array(bytes));
            if !fits_precision(mantissa, *precision) {
                return Err(damaged());
            }
            Value::Decimal(Decimal {
                mantissa,
                scale: *scale,
            })
        }
        ColumnType::Date => Value::Date(i32::from_le_bytes(slot_array(bytes))),
        ColumnType::Time => {
            Value::Time(i64::try_from(nanos(slot_array(bytes))).map_err(|_| damaged())?)
        }
        ColumnType::Timestamp => Value::Timestamp(nanos(slot_array(bytes))),
        ColumnType::Duration => Value::Duration(nanos(slot_array(bytes))),
        ColumnType::Uuid(_) => Value::Uuid(Uuid(slot_array(bytes))),
        ColumnType::Ulid => Value::Ulid(Ulid(slot_array(bytes))),
        ColumnType::String => Value::String(Cow::Borrowed(
            std::str::from_utf8(bytes).map_err(|_| damaged())?,
        )),
        ColumnType::Bytes => Value::Bytes(Cow::Borrowed(bytes)),
        ColumnType::Vector(vector) => {
            if bytes.len() != vector.stored_len() {
                return Err(damaged());
            }
            Value::Vector(Cow::Borrowed(&bytes[..vector.value_len()]))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::value::{UuidOrder, VectorElement, VectorType};

    /// `check_stored`, which the check of a row read runs without building values, accepts exactly the stored
    /// bytes that `decode` decodes, for every type, over edge patterns of each width and texts, bytes and vectors.
    #[test]
    fn check_stored_accepts_what_decode_accepts() {
        let vector = ColumnType::Vector(VectorType {
            element: VectorElement::F32,
            dimension: 2,
            cosine: true,
        });
        let types = [
            ColumnType::Bool,
            ColumnType::I8,
            ColumnType::I64,
            ColumnType::U64,
            ColumnType::F64,
            ColumnType::Decimal {
                precision: 3,
                scale: 1,
            },
            ColumnType::Date,
            ColumnType::Time,
            ColumnType::Timestamp,
            ColumnType::Duration,
            ColumnType::Uuid(UuidOrder::SqlServer),
            ColumnType::Ulid,
            ColumnType::String,
            ColumnType::Bytes,
            vector,
        ];
        let mut patterns: Vec<Vec<u8>> = vec![
            b"text".to_vec(),
            "é\0".as_bytes().to_vec(),
            vec![0xC3],
            vec![0xFF, 0xFE],
            vec![],
        ];
        for width in [1, 2, 4, 8, 12, 16, 20] {
            for b in [0x00, 0x01, 0x02, 0x7F, 0x80, 0xFE, 0xFF] {
                patterns.push(vec![b; width]);
            }
            patterns.push((0..width as u8).collect());
            patterns.push(999_i128.to_le_bytes()[..width.min(16)].to_vec());
        }
        for ty in &types {
            for bytes in &patterns {
                if ty.fixed_width().is_some_and(|width| width != bytes.len()) {
                    continue;
                }
                assert_eq!(
                    check_stored(ty, bytes).is_ok(),
                    decode(ty, bytes).is_ok(),
                    "{ty:?} {bytes:02x?}"
                );
            }
        }
    }

    /// Every fixed-width type that `always_decodes` names decodes any bytes of its width, which lets `Row::check`
    /// skip it; every other one rejects some.
    #[test]
    fn always_decodes_names_the_types_whose_every_value_decodes() {
        let types = [
            ColumnType::Bool,
            ColumnType::I8,
            ColumnType::I16,
            ColumnType::I32,
            ColumnType::I64,
            ColumnType::U8,
            ColumnType::U16,
            ColumnType::U32,
            ColumnType::U64,
            ColumnType::F32,
            ColumnType::F64,
            ColumnType::Decimal {
                precision: 1,
                scale: 0,
            },
            ColumnType::Date,
            ColumnType::Time,
            ColumnType::Timestamp,
            ColumnType::Duration,
            ColumnType::Uuid(UuidOrder::Rfc9562),
            ColumnType::Uuid(UuidOrder::SqlServer),
            ColumnType::Ulid,
        ];
        for ty in &types {
            let width = ty.fixed_width().expect("a fixed-width type");
            let patterns = [0x00, 0x01, 0x02, 0x7F, 0x80, 0xFE, 0xFF].map(|b| vec![b; width]);
            let all = patterns.iter().all(|bytes| decode(ty, bytes).is_ok());
            assert_eq!(always_decodes(ty), all, "{ty:?}");
        }
    }
}
