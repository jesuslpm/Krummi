//! Column types and values (N4 of the tables PRD; E7 and E8 of the tables with schema PRD): the schema limits of a
//! type, the validation and normalization of one value for its column, and the text form of values for error data
//! (D5 of the execution design).

use std::borrow::Cow;
use std::fmt;

use krummi_pager::{SchemaRule, ValueRule};

use super::json::{self, Json};
use crate::ids::{Ulid, Uuid};

/// The largest dimension of a vector column, for every element type (N4).
pub const MAX_VECTOR_DIMENSION: u16 = 4096;
/// The largest precision of a `decimal(p, s)` (N4, E5).
pub const MAX_DECIMAL_PRECISION: u8 = 38;

/// The first `date`, 0001-01-01, in days since 1970-01-01 (E8).
pub const DATE_MIN: i32 = -719_162;
/// The last `date`, 9999-12-31, in days since 1970-01-01 (E8).
pub const DATE_MAX: i32 = 2_932_896;
/// The last `time`, 23:59:59.999999, in microseconds since midnight (E8); the first is 0.
pub const TIME_MAX: i64 = 86_400_000_000 - 1;
/// The first `timestamp`, 0001-01-01T00:00:00Z, in microseconds since the Unix epoch (E8).
pub const TIMESTAMP_MIN: i64 = -62_135_596_800_000_000;
/// The last `timestamp`, 9999-12-31T23:59:59.999999Z, in microseconds since the Unix epoch (E8).
pub const TIMESTAMP_MAX: i64 = 253_402_300_799_999_999;
/// The largest `duration` in microseconds, .NET's `TimeSpan` truncated to the microsecond (E8); the smallest is its
/// negation.
pub const DURATION_MAX: i64 = 922_337_203_685_477_580;

/// The one NaN a `f32` column stores (N4): the positive quiet NaN.
pub(crate) const CANONICAL_NAN_F32: u32 = 0x7FC0_0000;
/// The one NaN a `f64` column stores (N4): the positive quiet NaN.
pub(crate) const CANONICAL_NAN_F64: u64 = 0x7FF8_0000_0000_0000;
/// Nanoseconds per microsecond: temporal [`Value`]s hold nanoseconds, rows and keys store microseconds.
pub(crate) const NANOS_PER_MICRO: i128 = 1000;

/// The type of a column (N4; document 1 of the table design, *Column types*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnType {
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    /// A fixed-point number of up to `precision` digits, `scale` of them after the point, stored as a 128-bit
    /// integer scaled by 10^`scale`.
    Decimal {
        precision: u8,
        scale: u8,
    },
    /// UTF-8 text, U+0000 included (E7).
    String,
    Bytes,
    /// Days since 1970-01-01, proleptic Gregorian.
    Date,
    /// Microseconds since midnight.
    Time,
    /// A UTC instant in microseconds since the Unix epoch.
    Timestamp,
    /// Microseconds.
    Duration,
    /// A `uuid`, given and stored in the byte order of RFC 9562 (E7); its order in a key is the column's.
    Uuid(UuidOrder),
    Ulid,
    Vector(VectorType),
    /// A JSON document, stored as its normalized text (the JSON columns PRD, JS1 and JS4).
    Json,
}

/// The order of a `uuid` column's key (N5; document 3, *UUID and ULID*).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UuidOrder {
    /// The 16 bytes in the order of RFC 9562.
    Rfc9562,
    /// The order in which SQL Server compares a `uniqueidentifier`.
    SqlServer,
}

/// The element of a vector column (N4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VectorElement {
    /// IEEE binary32, little-endian.
    F32,
    /// IEEE binary16, little-endian (E7).
    F16,
    /// One signed byte per element, then the vector's `f32` scale (document 5).
    I8,
    /// One bit per element: element i is bit i mod 8, from the least significant, of byte i div 8 (E7).
    Bit,
}

/// The type of a vector column: its element, its dimension and whether it declares the cosine metric (N4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VectorType {
    pub element: VectorElement,
    pub dimension: u16,
    pub cosine: bool,
}

impl VectorType {
    /// The bytes of a vector value as the application gives and reads it: its elements and, for `i8`, its scale
    /// (document 5, *Storage format of one value*, without the norm).
    pub fn value_len(&self) -> usize {
        let dim = usize::from(self.dimension);
        match self.element {
            VectorElement::F32 => dim * 4,
            VectorElement::F16 => dim * 2,
            VectorElement::I8 => dim + 4,
            VectorElement::Bit => dim.div_ceil(8),
        }
    }

    /// The bytes of a vector in a row: the value and, for a cosine column, the 8-byte `f64` norm after it.
    pub fn stored_len(&self) -> usize {
        self.value_len() + if self.cosine { 8 } else { 0 }
    }
}

impl ColumnType {
    /// The width of the type in the fixed section of a row (16 bytes at most), or `None` for the types of the
    /// variable section: `string`, `bytes`, vectors and JSON (document 1, *Row layout*).
    pub fn fixed_width(&self) -> Option<usize> {
        Some(match self {
            ColumnType::Bool | ColumnType::I8 | ColumnType::U8 => 1,
            ColumnType::I16 | ColumnType::U16 => 2,
            ColumnType::I32 | ColumnType::U32 | ColumnType::F32 | ColumnType::Date => 4,
            ColumnType::I64
            | ColumnType::U64
            | ColumnType::F64
            | ColumnType::Time
            | ColumnType::Timestamp
            | ColumnType::Duration => 8,
            ColumnType::Decimal { .. } | ColumnType::Uuid(_) | ColumnType::Ulid => 16,
            ColumnType::String | ColumnType::Bytes | ColumnType::Vector(_) | ColumnType::Json => {
                return None;
            }
        })
    }

    /// Whether the type has an order, so that it can be in a primary key or in the key of an index: every type but
    /// vectors (N5) and JSON (JS1 of the JSON columns PRD).
    pub fn is_orderable(&self) -> bool {
        !matches!(self, ColumnType::Vector(_) | ColumnType::Json)
    }
}

/// Whether `value` is a value of type `ty`, not null: its variant is the type's and a vector has the column's
/// number of bytes. It says nothing of the value's domain.
pub fn is_of_type(ty: &ColumnType, value: &Value<'_>) -> bool {
    match (ty, value) {
        (ColumnType::Vector(vector), Value::Vector(bytes)) => bytes.len() == vector.value_len(),
        (ColumnType::Bool, Value::Bool(_))
        | (ColumnType::I8, Value::I8(_))
        | (ColumnType::I16, Value::I16(_))
        | (ColumnType::I32, Value::I32(_))
        | (ColumnType::I64, Value::I64(_))
        | (ColumnType::U8, Value::U8(_))
        | (ColumnType::U16, Value::U16(_))
        | (ColumnType::U32, Value::U32(_))
        | (ColumnType::U64, Value::U64(_))
        | (ColumnType::F32, Value::F32(_))
        | (ColumnType::F64, Value::F64(_))
        | (ColumnType::Decimal { .. }, Value::Decimal(_))
        | (ColumnType::String, Value::String(_))
        | (ColumnType::Bytes, Value::Bytes(_))
        | (ColumnType::Date, Value::Date(_))
        | (ColumnType::Time, Value::Time(_))
        | (ColumnType::Timestamp, Value::Timestamp(_))
        | (ColumnType::Duration, Value::Duration(_))
        | (ColumnType::Uuid(_), Value::Uuid(_))
        | (ColumnType::Ulid, Value::Ulid(_))
        | (ColumnType::Json, Value::Json(_)) => true,
        _ => false,
    }
}

/// Whether a stored temporal value, in its stored unit (days of a `date`, microseconds of the others), is within
/// the domain of its type (E8); every other type is within its domain.
pub(crate) fn stored_in_domain(ty: &ColumnType, stored: i128) -> bool {
    let (min, max) = match ty {
        ColumnType::Date => (DATE_MIN.into(), DATE_MAX.into()),
        ColumnType::Time => (0, TIME_MAX.into()),
        ColumnType::Timestamp => (TIMESTAMP_MIN.into(), TIMESTAMP_MAX.into()),
        ColumnType::Duration => ((-DURATION_MAX).into(), DURATION_MAX.into()),
        _ => return true,
    };
    (min..=max).contains(&stored)
}

/// Whether a `decimal` mantissa has at most `precision` digits.
pub(crate) fn fits_precision(mantissa: i128, precision: u8) -> bool {
    10u128
        .checked_pow(u32::from(precision))
        .is_none_or(|limit| mantissa.unsigned_abs() < limit)
}

/// The text of a type, for error data (E6): `i64`, `decimal(10, 2)`, `uuid`, `uuid(sql server)`,
/// `vector(f32, 3, cosine)`.
impl fmt::Display for ColumnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            ColumnType::Bool => "bool",
            ColumnType::I8 => "i8",
            ColumnType::I16 => "i16",
            ColumnType::I32 => "i32",
            ColumnType::I64 => "i64",
            ColumnType::U8 => "u8",
            ColumnType::U16 => "u16",
            ColumnType::U32 => "u32",
            ColumnType::U64 => "u64",
            ColumnType::F32 => "f32",
            ColumnType::F64 => "f64",
            ColumnType::String => "string",
            ColumnType::Bytes => "bytes",
            ColumnType::Date => "date",
            ColumnType::Time => "time",
            ColumnType::Timestamp => "timestamp",
            ColumnType::Duration => "duration",
            ColumnType::Uuid(UuidOrder::Rfc9562) => "uuid",
            ColumnType::Uuid(UuidOrder::SqlServer) => "uuid(sql server)",
            ColumnType::Ulid => "ulid",
            ColumnType::Json => "json",
            ColumnType::Decimal { precision, scale } => {
                return write!(f, "decimal({precision}, {scale})");
            }
            ColumnType::Vector(vector) => {
                let element = match vector.element {
                    VectorElement::F32 => "f32",
                    VectorElement::F16 => "f16",
                    VectorElement::I8 => "i8",
                    VectorElement::Bit => "bit",
                };
                let cosine = if vector.cosine { ", cosine" } else { "" };
                return write!(f, "vector({element}, {}{cosine})", vector.dimension);
            }
        };
        f.write_str(name)
    }
}

/// Checks the limits of a type that a schema declares (E5): a vector's dimension of 1 to 4,096, a multiple of 8
/// for `bit`; a `decimal(p, s)` with 1 <= p <= 38 and 0 <= s <= p; no cosine metric on `bit`. When several fail,
/// the first in that order (E5's) is reported.
pub fn check_type(ty: &ColumnType) -> Result<(), SchemaRule> {
    match *ty {
        ColumnType::Vector(vector) => {
            let dim = vector.dimension;
            if !(1..=MAX_VECTOR_DIMENSION).contains(&dim)
                || (vector.element == VectorElement::Bit && !dim.is_multiple_of(8))
            {
                return Err(SchemaRule::VectorOutOfLimits);
            }
            if vector.cosine && vector.element == VectorElement::Bit {
                return Err(SchemaRule::CosineOnBit);
            }
            Ok(())
        }
        ColumnType::Decimal { precision, scale } => {
            if (1..=MAX_DECIMAL_PRECISION).contains(&precision) && scale <= precision {
                Ok(())
            } else {
                Err(SchemaRule::DecimalOutOfLimits)
            }
        }
        _ => Ok(()),
    }
}

/// A `decimal` value: `mantissa` × 10^−`scale`. A value given to a column may have any scale; it is rounded to the
/// column's (E8). A value read from a row has the column's scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Decimal {
    pub mantissa: i128,
    pub scale: u8,
}

/// A value of a column, as given to Krummi and as read back (N4). Text, bytes, vectors and JSON borrow when they can:
/// a value read from a row borrows the page (D11), a default value is owned by its schema.
///
/// Temporal values carry nanoseconds, so that a value given with a fraction below the microsecond is rounded by the
/// validation of its column (E8) and not before; a value read from a row is always a whole number of microseconds.
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    Null,
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    Decimal(Decimal),
    String(Cow<'a, str>),
    Bytes(Cow<'a, [u8]>),
    /// Days since 1970-01-01.
    Date(i32),
    /// Nanoseconds since midnight.
    Time(i64),
    /// Nanoseconds since the Unix epoch, UTC.
    Timestamp(i128),
    /// Nanoseconds.
    Duration(i128),
    Uuid(Uuid),
    Ulid(Ulid),
    /// The elements of a vector and, for `i8`, its scale: [`VectorType::value_len`] bytes in the layout of
    /// document 5. A column with the cosine metric computes and stores the norm itself.
    Vector(Cow<'a, [u8]>),
    /// A JSON document (the JSON columns PRD, JS3 and JS5): as given, a text or bytes that a write validates and
    /// normalizes; as read back, its normalized text.
    Json(Json<'a>),
}

impl Value<'_> {
    /// The value borrowing its text, bytes or vector from this one, without copying them.
    pub fn reborrow(&self) -> Value<'_> {
        match self {
            Value::String(text) => Value::String(Cow::Borrowed(text)),
            Value::Bytes(bytes) => Value::Bytes(Cow::Borrowed(bytes)),
            Value::Vector(bytes) => Value::Vector(Cow::Borrowed(bytes)),
            Value::Json(json) => Value::Json(json.reborrow()),
            other => other.clone(),
        }
    }

    /// Whether two normalized values are the same value, as a row reads them (E10, E11): floats by their bits, so
    /// that the one NaN equals itself.
    pub(crate) fn same(&self, other: &Value<'_>) -> bool {
        match (self, other) {
            (Value::F32(a), Value::F32(b)) => a.to_bits() == b.to_bits(),
            (Value::F64(a), Value::F64(b)) => a.to_bits() == b.to_bits(),
            (a, b) => a == b,
        }
    }

    /// The value with its borrowed parts copied, so that it outlives what it borrowed.
    pub fn into_owned(self) -> Value<'static> {
        match self {
            Value::String(text) => Value::String(Cow::Owned(text.into_owned())),
            Value::Bytes(bytes) => Value::Bytes(Cow::Owned(bytes.into_owned())),
            Value::Vector(bytes) => Value::Vector(Cow::Owned(bytes.into_owned())),
            Value::Json(json) => Value::Json(json.into_owned()),
            Value::Null => Value::Null,
            Value::Bool(v) => Value::Bool(v),
            Value::I8(v) => Value::I8(v),
            Value::I16(v) => Value::I16(v),
            Value::I32(v) => Value::I32(v),
            Value::I64(v) => Value::I64(v),
            Value::U8(v) => Value::U8(v),
            Value::U16(v) => Value::U16(v),
            Value::U32(v) => Value::U32(v),
            Value::U64(v) => Value::U64(v),
            Value::F32(v) => Value::F32(v),
            Value::F64(v) => Value::F64(v),
            Value::Decimal(v) => Value::Decimal(v),
            Value::Date(v) => Value::Date(v),
            Value::Time(v) => Value::Time(v),
            Value::Timestamp(v) => Value::Timestamp(v),
            Value::Duration(v) => Value::Duration(v),
            Value::Uuid(v) => Value::Uuid(v),
            Value::Ulid(v) => Value::Ulid(v),
        }
    }
}

/// Validates `value` for a column of type `ty` that allows nulls when `nullable` and returns it normalized, or the
/// first rule it breaks in the order of E17: null not allowed, wrong type, out of domain (also after rounding),
/// decimal precision, vector dimension, NaN or infinity in a vector, zero vector with the cosine metric; and, in a
/// JSON column, a text that is not a JSON document (JS8 of the JSON columns PRD).
///
/// Normalization (N4, E7, E8): `-0` becomes `+0` and every NaN the one canonical NaN in `f32` and `f64`, and `-0`
/// becomes `+0` inside `f32` and `f16` vectors and in an `i8` vector's scale; a `decimal` is rounded to the column's
/// scale with ties away from zero; a `time`, `timestamp` or `duration` is rounded to the nearest microsecond with
/// ties up; a JSON document given by the application becomes its normalized text (JS4), and one read back, already
/// normalized, is kept as it is. `ty` must pass [`check_type`].
pub fn normalize<'a>(
    ty: &ColumnType,
    nullable: bool,
    value: Value<'a>,
) -> Result<Value<'a>, ValueRule> {
    let wrong = Err(ValueRule::WrongType);
    Ok(match (ty, value) {
        (_, Value::Null) if nullable => Value::Null,
        (_, Value::Null) => return Err(ValueRule::NullNotAllowed),
        (ColumnType::Bool, v @ Value::Bool(_))
        | (ColumnType::I8, v @ Value::I8(_))
        | (ColumnType::I16, v @ Value::I16(_))
        | (ColumnType::I32, v @ Value::I32(_))
        | (ColumnType::I64, v @ Value::I64(_))
        | (ColumnType::U8, v @ Value::U8(_))
        | (ColumnType::U16, v @ Value::U16(_))
        | (ColumnType::U32, v @ Value::U32(_))
        | (ColumnType::U64, v @ Value::U64(_))
        | (ColumnType::String, v @ Value::String(_))
        | (ColumnType::Bytes, v @ Value::Bytes(_))
        | (ColumnType::Uuid(_), v @ Value::Uuid(_))
        | (ColumnType::Ulid, v @ Value::Ulid(_)) => v,
        (ColumnType::F32, Value::F32(v)) => Value::F32(canonical_f32(v)),
        (ColumnType::F64, Value::F64(v)) => Value::F64(canonical_f64(v)),
        (ColumnType::Decimal { precision, scale }, Value::Decimal(d)) => {
            Value::Decimal(round_decimal(d, *precision, *scale)?)
        }
        (ColumnType::Date, Value::Date(days)) => {
            in_domain(i128::from(days), DATE_MIN.into(), DATE_MAX.into())?;
            Value::Date(days)
        }
        (ColumnType::Time, Value::Time(nanos)) => {
            let micros = in_domain(round_micros(nanos.into()), 0, TIME_MAX.into())?;
            Value::Time(i64::try_from(micros * NANOS_PER_MICRO).expect("within a day"))
        }
        (ColumnType::Timestamp, Value::Timestamp(nanos)) => {
            let micros = round_micros(nanos);
            let micros = in_domain(micros, TIMESTAMP_MIN.into(), TIMESTAMP_MAX.into())?;
            Value::Timestamp(micros * NANOS_PER_MICRO)
        }
        (ColumnType::Duration, Value::Duration(nanos)) => {
            let micros = round_micros(nanos);
            let micros = in_domain(micros, (-DURATION_MAX).into(), DURATION_MAX.into())?;
            Value::Duration(micros * NANOS_PER_MICRO)
        }
        (ColumnType::Vector(vector), Value::Vector(bytes)) => {
            Value::Vector(normalize_vector(vector, bytes)?)
        }
        (ColumnType::Json, Value::Json(document)) if document.is_normalized() => {
            Value::Json(document)
        }
        (ColumnType::Json, Value::Json(document)) => match json::normalize(document.as_bytes()) {
            Ok(text) => Value::Json(Json::normalized(Cow::Owned(text))),
            Err(_) => return Err(ValueRule::InvalidJson),
        },
        _ => return wrong,
    })
}

/// `-0` as `+0` and every NaN as the canonical NaN (N4).
fn canonical_f32(v: f32) -> f32 {
    if v.is_nan() {
        f32::from_bits(CANONICAL_NAN_F32)
    } else {
        v + 0.0
    }
}

/// `-0` as `+0` and every NaN as the canonical NaN (N4).
fn canonical_f64(v: f64) -> f64 {
    if v.is_nan() {
        f64::from_bits(CANONICAL_NAN_F64)
    } else {
        v + 0.0
    }
}

fn in_domain(v: i128, min: i128, max: i128) -> Result<i128, ValueRule> {
    if (min..=max).contains(&v) {
        Ok(v)
    } else {
        Err(ValueRule::OutOfDomain)
    }
}

/// Nanoseconds rounded to the nearest microsecond, ties toward the greater value (E8).
pub(crate) fn round_micros(nanos: i128) -> i128 {
    // Every i128 the API takes is far from the ends of i128, but saturate rather than overflow.
    nanos
        .saturating_add(NANOS_PER_MICRO / 2)
        .div_euclid(NANOS_PER_MICRO)
}

/// `d` at the column's `scale`, rounded with ties away from zero, as SQL Server does (E8), and checked against
/// the column's `precision`.
fn round_decimal(d: Decimal, precision: u8, scale: u8) -> Result<Decimal, ValueRule> {
    let mantissa = if d.scale > scale {
        let shift = u32::from(d.scale - scale);
        // 10^38 is the largest power of ten in an i128; every mantissa is below half of 10^39, so it rounds to 0.
        match 10u128.checked_pow(shift) {
            None => 0,
            Some(divisor) => {
                let magnitude = d.mantissa.unsigned_abs();
                let (quotient, remainder) = (magnitude / divisor, magnitude % divisor);
                let rounded = quotient + u128::from(remainder >= divisor - remainder);
                // The quotient of an i128 by at least 10 fits in an i128, and so does one more.
                let rounded = i128::try_from(rounded).expect("smaller than the mantissa");
                if d.mantissa < 0 { -rounded } else { rounded }
            }
        }
    } else {
        let shift = u32::from(scale - d.scale);
        10i128
            .checked_pow(shift)
            .and_then(|factor| d.mantissa.checked_mul(factor))
            .ok_or(ValueRule::DecimalPrecision)?
    };
    if fits_precision(mantissa, precision) {
        Ok(Decimal { mantissa, scale })
    } else {
        Err(ValueRule::DecimalPrecision)
    }
}

/// Checks a vector value against its column (dimension, finite elements, nonzero with the cosine metric, E17's
/// order) and writes `+0` for every `-0` (E7), copying only when there is one.
fn normalize_vector<'a>(
    vector: &VectorType,
    bytes: Cow<'a, [u8]>,
) -> Result<Cow<'a, [u8]>, ValueRule> {
    if bytes.len() != vector.value_len() {
        return Err(ValueRule::VectorDimension);
    }
    let dim = usize::from(vector.dimension);
    // The floating parts: the elements of f32 and f16, the scale of i8.
    let (width, floats) = match vector.element {
        VectorElement::F32 => (4, 0..dim * 4),
        VectorElement::F16 => (2, 0..dim * 2),
        VectorElement::I8 => (4, dim..dim + 4),
        VectorElement::Bit => (1, 0..0),
    };
    let mut has_negative_zero = false;
    for chunk in bytes[floats.clone()].chunks_exact(width) {
        let (finite, negative_zero) = if width == 4 {
            let v = f32::from_le_bytes(chunk.try_into().expect("4 bytes"));
            (v.is_finite(), v.to_bits() == 0x8000_0000)
        } else {
            let bits = u16::from_le_bytes(chunk.try_into().expect("2 bytes"));
            (bits & 0x7C00 != 0x7C00, bits == 0x8000)
        };
        if !finite {
            return Err(ValueRule::VectorNotFinite);
        }
        has_negative_zero |= negative_zero;
    }
    let mut bytes = bytes;
    if has_negative_zero {
        for chunk in bytes.to_mut()[floats].chunks_exact_mut(width) {
            if chunk.iter().take(width - 1).all(|&b| b == 0) && chunk[width - 1] == 0x80 {
                chunk[width - 1] = 0;
            }
        }
    }
    if vector.cosine && is_zero_vector(vector, &bytes) {
        return Err(ValueRule::ZeroVectorCosine);
    }
    Ok(bytes)
}

/// Whether a normalized vector is zero: all its elements are `+0` (no `-0` is left), or an `i8` vector's scale is.
fn is_zero_vector(vector: &VectorType, bytes: &[u8]) -> bool {
    let dim = usize::from(vector.dimension);
    match vector.element {
        VectorElement::I8 => {
            bytes[..dim].iter().all(|&b| b == 0) || bytes[dim..].iter().all(|&b| b == 0)
        }
        _ => bytes.iter().all(|&b| b == 0),
    }
}

/// The lanes of the fixed-order kernel (document 5, *Determinism*).
const LANES: usize = 16;

/// The L2 norm of a normalized vector value of `vector`, as a cosine column stores it (document 5): every element
/// converted to `f64` (exactly), squared and added in 16 lanes, lane j taking the elements j, j + 16, ...; the lanes
/// added in a fixed tree (8, 4, 2, 1); then the tail in order; then one square root. An `i8` vector's norm is that of
/// the dequantized vector: the exact integer sum of squares times the scale squared, then the square root.
pub fn vector_norm(vector: &VectorType, bytes: &[u8]) -> f64 {
    let dim = usize::from(vector.dimension);
    match vector.element {
        VectorElement::F32 => fixed_order_norm(
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| f64::from(f32::from_le_bytes(*c))),
        ),
        VectorElement::F16 => fixed_order_norm(
            bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| f16_to_f64(u16::from_le_bytes(*c))),
        ),
        VectorElement::I8 => {
            let sum: i64 = bytes[..dim]
                .iter()
                .map(|&b| i64::from(b as i8) * i64::from(b as i8))
                .sum();
            let scale = f64::from(f32::from_le_bytes(
                bytes[dim..dim + 4].try_into().expect("4 bytes"),
            ));
            (sum as f64 * (scale * scale)).sqrt()
        }
        VectorElement::Bit => (bytes.iter().map(|b| b.count_ones()).sum::<u32>() as f64).sqrt(),
    }
}

/// Lane `j` accumulates elements `j, j + LANES, ...`; the lanes are reduced with a fixed tree, then each tail
/// square is added on its own, in order (document 5).
fn fixed_order_norm(elements: impl ExactSizeIterator<Item = f64>) -> f64 {
    let len = elements.len();
    let full = len - len % LANES;
    let mut elements = elements.map(|x| x * x);
    let mut lanes = [0f64; LANES];
    for (i, square) in elements.by_ref().take(full).enumerate() {
        lanes[i % LANES] += square;
    }
    let mut width = LANES / 2;
    while width > 0 {
        for j in 0..width {
            lanes[j] += lanes[j + width];
        }
        width /= 2;
    }
    elements.fold(lanes[0], |sum, square| sum + square).sqrt()
}

/// An IEEE binary16 as a `f64`, exactly.
pub fn f16_to_f64(bits: u16) -> f64 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = i32::from((bits >> 10) & 0x1F);
    let fraction = f64::from(bits & 0x3FF);
    sign * match exponent {
        0 => fraction * 2f64.powi(-24),
        0x1F if fraction == 0.0 => f64::INFINITY,
        0x1F => f64::NAN,
        _ => (1.0 + fraction / 1024.0) * 2f64.powi(exponent - 15),
    }
}

/// The text form of a value for error data (D5): short and unambiguous within its column's type. `null`; numbers in
/// decimal (floats with Rust's shortest round-trip form, `NaN`, `inf`); text quoted with Rust's escapes; bytes as
/// `0x` and lowercase hexadecimal; `date` as `YYYY-MM-DD`, `time` as `HH:MM:SS[.f]`, `timestamp` as
/// `YYYY-MM-DDTHH:MM:SS[.f]Z` and `duration` as seconds with an `s`, the fraction without trailing zeros; `uuid` as
/// RFC 9562 text and `ulid` as Crockford Base32 (E9); a vector by its size; a JSON document as its text, with the
/// bytes that are not UTF-8 of one given by the application replaced by U+FFFD.
impl fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => f.write_str("null"),
            Value::Bool(v) => write!(f, "{v}"),
            Value::I8(v) => write!(f, "{v}"),
            Value::I16(v) => write!(f, "{v}"),
            Value::I32(v) => write!(f, "{v}"),
            Value::I64(v) => write!(f, "{v}"),
            Value::U8(v) => write!(f, "{v}"),
            Value::U16(v) => write!(f, "{v}"),
            Value::U32(v) => write!(f, "{v}"),
            Value::U64(v) => write!(f, "{v}"),
            Value::F32(v) => write!(f, "{v:?}"),
            Value::F64(v) => write!(f, "{v:?}"),
            Value::Decimal(d) => write_decimal(f, *d),
            Value::String(text) => write!(f, "{text:?}"),
            Value::Bytes(bytes) => {
                f.write_str("0x")?;
                bytes.iter().try_for_each(|b| write!(f, "{b:02x}"))
            }
            Value::Date(days) => write_date(f, i128::from(*days)),
            Value::Time(nanos) => write_clock(f, i128::from(*nanos)),
            Value::Timestamp(nanos) => {
                let day = 86_400 * 1_000_000_000;
                write_date(f, nanos.div_euclid(day))?;
                f.write_str("T")?;
                write_clock(f, nanos.rem_euclid(day))?;
                f.write_str("Z")
            }
            Value::Duration(nanos) => {
                let sign = if *nanos < 0 { "-" } else { "" };
                let magnitude = nanos.unsigned_abs();
                write!(f, "{sign}{}", magnitude / 1_000_000_000)?;
                write_fraction(f, magnitude % 1_000_000_000)?;
                f.write_str("s")
            }
            Value::Uuid(uuid) => write!(f, "{uuid}"),
            Value::Ulid(ulid) => write!(f, "{ulid}"),
            Value::Vector(bytes) => write!(f, "vector({} bytes)", bytes.len()),
            Value::Json(json) => f.write_str(&String::from_utf8_lossy(json.as_bytes())),
        }
    }
}

/// The text form of a primary key for error data (D5): its values in key order, separated by `, ` in parentheses.
pub fn key_text(values: &[Value<'_>]) -> String {
    let parts: Vec<String> = values.iter().map(ToString::to_string).collect();
    format!("({})", parts.join(", "))
}

fn write_decimal(f: &mut fmt::Formatter<'_>, d: Decimal) -> fmt::Result {
    let sign = if d.mantissa < 0 { "-" } else { "" };
    let digits = d.mantissa.unsigned_abs().to_string();
    let scale = usize::from(d.scale);
    if scale == 0 {
        return write!(f, "{sign}{digits}");
    }
    let digits = format!("{digits:0>width$}", width = scale + 1);
    let (whole, fraction) = digits.split_at(digits.len() - scale);
    write!(f, "{sign}{whole}.{fraction}")
}

/// `YYYY-MM-DD` of `days` since 1970-01-01 (proleptic Gregorian; Howard Hinnant's `civil_from_days`).
fn write_date(f: &mut fmt::Formatter<'_>, days: i128) -> fmt::Result {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i128::from(month <= 2);
    write!(f, "{year:04}-{month:02}-{day:02}")
}

/// `HH:MM:SS[.f]` of `nanos` since midnight.
fn write_clock(f: &mut fmt::Formatter<'_>, nanos: i128) -> fmt::Result {
    let seconds = nanos.div_euclid(1_000_000_000);
    let (h, m, s) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    write!(f, "{h:02}:{m:02}:{s:02}")?;
    write_fraction(f, nanos.rem_euclid(1_000_000_000).unsigned_abs())
}

/// `.f`, the fraction of a second without trailing zeros, or nothing when it is zero.
fn write_fraction(f: &mut fmt::Formatter<'_>, nanos: u128) -> fmt::Result {
    if nanos == 0 {
        return Ok(());
    }
    let digits = format!("{nanos:09}");
    write!(f, ".{}", digits.trim_end_matches('0'))
}
