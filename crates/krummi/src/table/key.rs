//! Primary key encoding (document 3 of the table design, *Key encoding, per type*): ascending columns without
//! nulls, as the elements of FoundationDB's tuple layer, so that the byte order of keys is the order of N5. A key
//! is the concatenation of its elements, each a type code and its bytes; every element is self-delimiting.

use std::borrow::Cow;
use std::ops::Bound::{self, Excluded, Included, Unbounded};

use krummi_pager::ValueRule;

use super::value::{
    CANONICAL_NAN_F32, CANONICAL_NAN_F64, ColumnType, Decimal, NANOS_PER_MICRO, UuidOrder, Value,
    fits_precision, is_of_type, normalize, stored_in_domain,
};
use crate::ids::{Ulid, Uuid};

/// The largest primary key, in its encoded form (B5, E18).
pub const MAX_KEY: usize = krummi_tree::MAX_KEY;

const BYTES: u8 = 0x01;
const STRING: u8 = 0x02;
/// The integer zero; `ZERO + n` and `ZERO - n` start the integers of n bytes.
const ZERO: u8 = 0x14;
const F32: u8 = 0x20;
const F64: u8 = 0x21;
const FALSE: u8 = 0x26;
const TRUE: u8 = 0x27;
const UUID: u8 = 0x30;
const DECIMAL: u8 = 0x40;
/// The byte that follows `00` inside an escaped `string` or `bytes`, and that no element starts with.
const ESCAPE: u8 = 0xFF;

/// The bytes of a `uuid` key in SQL Server's order: key byte i is RFC 9562 byte `SQL_SERVER[i]` (document 3,
/// *UUID and ULID*: `r10..r15, r8 r9, r7 r6, r5 r4, r3 r2 r1 r0`).
const SQL_SERVER: [usize; 16] = [10, 11, 12, 13, 14, 15, 8, 9, 7, 6, 5, 4, 3, 2, 1, 0];

/// Why a key or a range of keys could not be encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyError {
    /// A value breaks `rule`; `column` is its position in the primary key, `None` for a malformed range (E12).
    Value {
        column: Option<usize>,
        rule: ValueRule,
    },
    /// The key, or a prefix or bound of a range, has `received` bytes encoded, more than [`MAX_KEY`] (E12, E18).
    Size { received: usize },
}

/// The byte bounds of a range of primary keys, each 1 to [`MAX_KEY`] bytes, for the tree's range API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRange {
    pub lower: Bound<Vec<u8>>,
    pub upper: Bound<Vec<u8>>,
}

impl KeyRange {
    /// The bounds borrowed, as the tree's `range` methods take them.
    pub fn bounds(&self) -> (Bound<&[u8]>, Bound<&[u8]>) {
        (
            self.lower.as_ref().map(Vec::as_slice),
            self.upper.as_ref().map(Vec::as_slice),
        )
    }
}

/// The encoded length of the key of `values`, without encoding it, so that a value too large for a key is
/// measured without copying it. `values` are those of [`encode_key`]. Panics if there is not one value per column.
pub fn key_len(types: &[ColumnType], values: &[Value<'_>]) -> usize {
    assert_eq!(
        types.len(),
        values.len(),
        "one value per primary key column"
    );
    values.iter().map(element_len).sum()
}

/// The key of `values`, normalized values (see [`normalize`]) of the primary key columns of `types`, in key order.
/// A value of another type than its column fails with [`ValueRule::WrongType`]; a key of more than [`MAX_KEY`]
/// bytes, with its size. Panics if there is not one value per column.
pub fn encode_key(types: &[ColumnType], values: &[Value<'_>]) -> Result<Vec<u8>, KeyError> {
    let len = key_len(types, values);
    if len > MAX_KEY {
        return Err(KeyError::Size { received: len });
    }
    let mut key = Vec::with_capacity(len);
    for (column, (ty, value)) in types.iter().zip(values).enumerate() {
        if !encode_element(ty, value, &mut key) {
            return Err(KeyError::Value {
                column: Some(column),
                rule: ValueRule::WrongType,
            });
        }
    }
    Ok(key)
}

/// The values of a key of the primary key columns `types`, or `None` when the bytes are not such a key: a code of
/// another type, a truncated or non-minimal element, a value outside its column's type or domain, text that is not
/// UTF-8 or bytes after the last element. `string` and `bytes` values are copied, since the key escapes them; the
/// row keeps them raw (document 1).
pub fn decode_key(types: &[ColumnType], key: &[u8]) -> Option<Vec<Value<'static>>> {
    let mut rest = key;
    let values = types
        .iter()
        .map(|ty| decode_element(ty, &mut rest))
        .collect::<Option<Vec<_>>>()?;
    rest.is_empty().then_some(values)
}

/// Whether `key` is a key of the primary key columns `types`, with the checks of [`decode_key`], without decoding its
/// text and byte elements.
pub fn is_key(types: &[ColumnType], key: &[u8]) -> bool {
    walk_key(types, key, |_| {})
}

/// Where each element of `key` starts, when [`is_key`] accepts it, so that each column can be read with
/// [`key_value`] without walking the elements before it; `None` also for an element that starts past `u16::MAX`,
/// which no key of at most [`MAX_KEY`] bytes has.
pub fn key_starts(types: &[ColumnType], key: &[u8]) -> Option<Vec<u16>> {
    let mut starts = Vec::with_capacity(types.len());
    let mut fits = true;
    let start = |at: usize| match u16::try_from(at) {
        Ok(at) => starts.push(at),
        Err(_) => fits = false,
    };
    (walk_key(types, key, start) && fits).then_some(starts)
}

/// Takes every element of `key` with the checks of [`skip_element`], calling `start` with the offset of each, and
/// says whether they make the whole key.
fn walk_key(types: &[ColumnType], key: &[u8], mut start: impl FnMut(usize)) -> bool {
    let mut rest = key;
    types.iter().all(|ty| {
        start(key.len() - rest.len());
        skip_element(ty, &mut rest).is_some()
    }) && rest.is_empty()
}

/// The value of a fixed-width key column of type `ty`, where `element` is the rest of a key that [`is_key`] accepts
/// from that column's start (its [`key_starts`] entry, or 0 for a key of one column). Panics on another slice, or for
/// a `string` or `bytes` column, which the row keeps instead.
pub fn key_value(ty: &ColumnType, element: &[u8]) -> Value<'static> {
    assert!(ty.fixed_width().is_some(), "a fixed-width key column");
    decode_element(ty, &mut &element[..]).expect("a checked key")
}

/// Takes the element of a value of type `ty` from the front of `rest` with the checks of [`decode_element`]; a text or
/// byte element is checked in place instead of copied.
fn skip_element(ty: &ColumnType, rest: &mut &[u8]) -> Option<()> {
    let code = match ty {
        ColumnType::String => STRING,
        ColumnType::Bytes => BYTES,
        _ => return decode_element(ty, rest).map(drop),
    };
    if take(rest, 1)?[0] != code {
        return None;
    }
    // A run between escapes holds no `00`, and a UTF-8 sequence never contains one, so checking each run on its own
    // checks the unescaped text.
    loop {
        let end = rest.iter().position(|&b| b == 0)?;
        let run = take(rest, end + 1).expect("the run and its 00");
        if matches!(ty, ColumnType::String) && std::str::from_utf8(&run[..end]).is_err() {
            return None;
        }
        if rest.first() == Some(&ESCAPE) {
            *rest = &rest[1..];
        } else {
            return Some(());
        }
    }
}

/// The byte bounds of a range of the primary key `types` (E12): equal values `prefix` for its first k columns and,
/// when k is less than the number of columns, a `lower` and an `upper` bound on column k. Bounds follow the table of
/// document 3 (*Composite keys and prefix scans*) for ascending columns without nulls, and a bound `B ‖ FF` of 2,049
/// bytes becomes `B` with the opposite inclusiveness, so that every bound has at most [`MAX_KEY`] bytes.
///
/// Fails, in this order, with the first value that breaks a rule of its column ([`normalize`]; a null breaks
/// [`ValueRule::NullNotAllowed`]) in key order, prefix first; with [`ValueRule::InvalidRange`] for a prefix longer
/// than the key or a bound when the prefix gives every column; and with the size of a prefix or bound that does not
/// fit in a key, even when the range is empty. An inverted range is valid: it selects nothing.
pub fn key_range<'v>(
    types: &[ColumnType],
    prefix: &[Value<'v>],
    lower: Bound<&Value<'v>>,
    upper: Bound<&Value<'v>>,
) -> Result<KeyRange, KeyError> {
    let normal = |column: usize, value: &Value<'v>| {
        normalize(&types[column], false, value.clone()).map_err(|rule| KeyError::Value {
            column: Some(column),
            rule,
        })
    };
    let k = prefix.len();
    let prefix = prefix
        .iter()
        .take(types.len())
        .enumerate()
        .map(|(column, value)| normal(column, value))
        .collect::<Result<Vec<_>, _>>()?;
    let has_bound = !matches!((lower, upper), (Unbounded, Unbounded));
    let (lower, upper) = if k < types.len() {
        (
            bound_map(lower, |v| normal(k, v))?,
            bound_map(upper, |v| normal(k, v))?,
        )
    } else {
        (Unbounded, Unbounded)
    };
    if k > types.len() || (k == types.len() && has_bound) {
        return Err(KeyError::Value {
            column: None,
            rule: ValueRule::InvalidRange,
        });
    }
    let p = encode_key(&types[..k], &prefix)?;
    let with = |value: &Value<'_>, escape: bool| -> Result<Vec<u8>, KeyError> {
        let ty = &types[k..=k];
        let received = p.len() + key_len(ty, std::slice::from_ref(value));
        if received > MAX_KEY {
            return Err(KeyError::Size { received });
        }
        let mut bound = p.clone();
        bound.extend(encode_key(ty, std::slice::from_ref(value))?);
        if escape {
            bound.push(ESCAPE);
        }
        Ok(bound)
    };
    // `start` and `end` as document 3 writes them: an included start and an excluded end, or none for an empty P.
    let start = match &lower {
        Included(lo) => Some(with(lo, false)?),
        Excluded(lo) => Some(with(lo, true)?),
        Unbounded => (!p.is_empty()).then(|| p.clone()),
    };
    let end = match &upper {
        Included(hi) => Some(with(hi, true)?),
        Excluded(hi) => Some(with(hi, false)?),
        Unbounded => (!p.is_empty()).then(|| [p.as_slice(), &[ESCAPE]].concat()),
    };
    Ok(KeyRange {
        lower: start.map_or(Unbounded, |start| fit(start, Included, Excluded)),
        upper: end.map_or(Unbounded, |end| fit(end, Excluded, Included)),
    })
}

fn bound_map<'a, T, E>(
    bound: Bound<&Value<'a>>,
    f: impl Fn(&Value<'a>) -> Result<T, E>,
) -> Result<Bound<T>, E> {
    Ok(match bound {
        Included(v) => Included(f(v)?),
        Excluded(v) => Excluded(f(v)?),
        Unbounded => Unbounded,
    })
}

/// A bound of the side `as_is`, unless it is `B ‖ FF` with B of [`MAX_KEY`] bytes: no key extends such a B, so
/// the keys at or after `B ‖ FF` are those after B, and the bound becomes B on the side `shrunk` (document 3).
fn fit(
    bound: Vec<u8>,
    as_is: fn(Vec<u8>) -> Bound<Vec<u8>>,
    shrunk: fn(Vec<u8>) -> Bound<Vec<u8>>,
) -> Bound<Vec<u8>> {
    if bound.len() > MAX_KEY {
        let mut b = bound;
        b.pop();
        shrunk(b)
    } else {
        as_is(bound)
    }
}

/// An integer value of any width, as the integer element encodes it.
fn integer(value: &Value<'_>) -> Option<i128> {
    Some(match *value {
        Value::I8(v) => v.into(),
        Value::I16(v) => v.into(),
        Value::I32(v) => v.into(),
        Value::I64(v) => v.into(),
        Value::U8(v) => v.into(),
        Value::U16(v) => v.into(),
        Value::U32(v) => v.into(),
        Value::U64(v) => v.into(),
        Value::Date(v) => v.into(),
        Value::Time(nanos) => i128::from(nanos) / NANOS_PER_MICRO,
        Value::Timestamp(nanos) | Value::Duration(nanos) => nanos / NANOS_PER_MICRO,
        _ => return None,
    })
}

/// The bytes that hold the magnitude of an integer element.
fn magnitude_len(magnitude: u128) -> usize {
    (128 - magnitude.leading_zeros() as usize).div_ceil(8)
}

fn element_len(value: &Value<'_>) -> usize {
    match value {
        Value::Bool(_) => 1,
        Value::F32(_) => 5,
        Value::F64(_) => 9,
        Value::Decimal(_) | Value::Uuid(_) | Value::Ulid(_) => 17,
        Value::String(text) => escaped_len(text.as_bytes()),
        Value::Bytes(bytes) => escaped_len(bytes),
        _ => integer(value).map_or(0, |v| 1 + magnitude_len(v.unsigned_abs())),
    }
}

/// A type code, the bytes with each `00` as `00 FF`, and the `00` terminator.
fn escaped_len(bytes: &[u8]) -> usize {
    2 + bytes.len() + bytes.iter().filter(|&&b| b == 0).count()
}

/// Appends the element of `value` to `key`; false when `value` is not of type `ty`.
fn encode_element(ty: &ColumnType, value: &Value<'_>, key: &mut Vec<u8>) -> bool {
    match (ty, value) {
        (ColumnType::Bool, Value::Bool(v)) => key.push(if *v { TRUE } else { FALSE }),
        (ColumnType::F32, Value::F32(v)) => {
            let bits = v.to_bits();
            let ordered = if bits >> 31 == 1 {
                !bits
            } else {
                bits ^ (1 << 31)
            };
            key.push(F32);
            key.extend(ordered.to_be_bytes());
        }
        (ColumnType::F64, Value::F64(v)) => {
            let bits = v.to_bits();
            let ordered = if bits >> 63 == 1 {
                !bits
            } else {
                bits ^ (1 << 63)
            };
            key.push(F64);
            key.extend(ordered.to_be_bytes());
        }
        (ColumnType::Decimal { .. }, Value::Decimal(d)) => {
            key.push(DECIMAL);
            key.extend(((d.mantissa as u128) ^ (1 << 127)).to_be_bytes());
        }
        (ColumnType::Uuid(order), Value::Uuid(uuid)) => {
            key.push(UUID);
            match order {
                UuidOrder::Rfc9562 => key.extend(uuid.0),
                UuidOrder::SqlServer => key.extend(SQL_SERVER.map(|i| uuid.0[i])),
            }
        }
        (ColumnType::Ulid, Value::Ulid(ulid)) => {
            key.push(UUID);
            key.extend(ulid.0);
        }
        (ColumnType::String, Value::String(text)) => escape(STRING, text.as_bytes(), key),
        (ColumnType::Bytes, Value::Bytes(bytes)) => escape(BYTES, bytes, key),
        (
            ColumnType::I8
            | ColumnType::I16
            | ColumnType::I32
            | ColumnType::I64
            | ColumnType::U8
            | ColumnType::U16
            | ColumnType::U32
            | ColumnType::U64
            | ColumnType::Date
            | ColumnType::Time
            | ColumnType::Timestamp
            | ColumnType::Duration,
            _,
        ) if is_of_type(ty, value) => {
            let v = integer(value).expect("an integer type");
            let magnitude = v.unsigned_abs();
            let n = magnitude_len(magnitude);
            let bytes = if v < 0 { !magnitude } else { magnitude }.to_be_bytes();
            key.push(if v < 0 {
                ZERO - n as u8
            } else {
                ZERO + n as u8
            });
            key.extend(&bytes[16 - n..]);
        }
        _ => return false,
    }
    true
}

fn escape(code: u8, bytes: &[u8], key: &mut Vec<u8>) {
    key.push(code);
    for &b in bytes {
        key.push(b);
        if b == 0 {
            key.push(ESCAPE);
        }
    }
    key.push(0);
}

/// Takes `n` bytes from the front of `rest`.
fn take<'k>(rest: &mut &'k [u8], n: usize) -> Option<&'k [u8]> {
    if rest.len() < n {
        return None;
    }
    let (head, tail) = rest.split_at(n);
    *rest = tail;
    Some(head)
}

fn take_array<const N: usize>(rest: &mut &[u8]) -> Option<[u8; N]> {
    take(rest, N).map(|bytes| bytes.try_into().expect("N bytes"))
}

/// The element of a value of type `ty` at the front of `rest`, or `None` when it starts with another type's code or
/// is not an element Krummi writes: a `-0` or a NaN other than the canonical one (N4), a `decimal` over its column's
/// precision, a value outside its domain.
fn decode_element(ty: &ColumnType, rest: &mut &[u8]) -> Option<Value<'static>> {
    let code = take(rest, 1)?[0];
    let expected = match ty {
        ColumnType::Bool if code == FALSE => return Some(Value::Bool(false)),
        ColumnType::Bool if code == TRUE => return Some(Value::Bool(true)),
        ColumnType::Bool | ColumnType::Vector(_) => return None,
        ColumnType::F32 => F32,
        ColumnType::F64 => F64,
        ColumnType::Decimal { .. } => DECIMAL,
        ColumnType::Uuid(_) | ColumnType::Ulid => UUID,
        ColumnType::String => STRING,
        ColumnType::Bytes => BYTES,
        ColumnType::I8
        | ColumnType::I16
        | ColumnType::I32
        | ColumnType::I64
        | ColumnType::U8
        | ColumnType::U16
        | ColumnType::U32
        | ColumnType::U64
        | ColumnType::Date
        | ColumnType::Time
        | ColumnType::Timestamp
        | ColumnType::Duration => return decode_integer(ty, code, rest),
    };
    if code != expected {
        return None;
    }
    Some(match ty {
        ColumnType::F32 => {
            let ordered = u32::from_be_bytes(take_array(rest)?);
            let bits = if ordered >> 31 == 1 {
                ordered ^ (1 << 31)
            } else {
                !ordered
            };
            let v = f32::from_bits(bits);
            if bits == 1 << 31 || (v.is_nan() && bits != CANONICAL_NAN_F32) {
                return None;
            }
            Value::F32(v)
        }
        ColumnType::F64 => {
            let ordered = u64::from_be_bytes(take_array(rest)?);
            let bits = if ordered >> 63 == 1 {
                ordered ^ (1 << 63)
            } else {
                !ordered
            };
            let v = f64::from_bits(bits);
            if bits == 1 << 63 || (v.is_nan() && bits != CANONICAL_NAN_F64) {
                return None;
            }
            Value::F64(v)
        }
        ColumnType::Decimal { precision, scale } => {
            let mantissa = (u128::from_be_bytes(take_array(rest)?) ^ (1 << 127)) as i128;
            if !fits_precision(mantissa, *precision) {
                return None;
            }
            Value::Decimal(Decimal {
                mantissa,
                scale: *scale,
            })
        }
        ColumnType::Uuid(order) => {
            let bytes: [u8; 16] = take_array(rest)?;
            Value::Uuid(Uuid(match order {
                UuidOrder::Rfc9562 => bytes,
                UuidOrder::SqlServer => {
                    let mut rfc = [0; 16];
                    for (i, &r) in SQL_SERVER.iter().enumerate() {
                        rfc[r] = bytes[i];
                    }
                    rfc
                }
            }))
        }
        ColumnType::Ulid => Value::Ulid(Ulid(take_array(rest)?)),
        ColumnType::String => Value::String(Cow::Owned(String::from_utf8(unescape(rest)?).ok()?)),
        ColumnType::Bytes => Value::Bytes(Cow::Owned(unescape(rest)?)),
        _ => unreachable!("the other types return above"),
    })
}

/// The bytes of an escaped element up to its terminator, which it consumes.
fn unescape(rest: &mut &[u8]) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    loop {
        let b = take(rest, 1)?[0];
        if b != 0 {
            bytes.push(b);
        } else if rest.first() == Some(&ESCAPE) {
            *rest = &rest[1..];
            bytes.push(0);
        } else {
            return Some(bytes);
        }
    }
}

fn decode_integer(ty: &ColumnType, code: u8, rest: &mut &[u8]) -> Option<Value<'static>> {
    if !(ZERO - 8..=ZERO + 8).contains(&code) {
        return None;
    }
    let n = usize::from(code.abs_diff(ZERO));
    let mut bytes = [0u8; 16];
    bytes[16 - n..].copy_from_slice(take(rest, n)?);
    let raw = u128::from_be_bytes(bytes);
    let mask = if n == 0 {
        0
    } else {
        u128::MAX >> (128 - 8 * n)
    };
    let magnitude = if code < ZERO { !raw & mask } else { raw };
    if magnitude_len(magnitude) != n {
        return None;
    }
    let v = if code < ZERO {
        -i128::try_from(magnitude).ok()?
    } else {
        i128::try_from(magnitude).ok()?
    };
    if !stored_in_domain(ty, v) {
        return None;
    }
    Some(match ty {
        ColumnType::I8 => Value::I8(v.try_into().ok()?),
        ColumnType::I16 => Value::I16(v.try_into().ok()?),
        ColumnType::I32 => Value::I32(v.try_into().ok()?),
        ColumnType::I64 => Value::I64(v.try_into().ok()?),
        ColumnType::U8 => Value::U8(v.try_into().ok()?),
        ColumnType::U16 => Value::U16(v.try_into().ok()?),
        ColumnType::U32 => Value::U32(v.try_into().ok()?),
        ColumnType::U64 => Value::U64(v.try_into().ok()?),
        ColumnType::Date => Value::Date(v.try_into().ok()?),
        ColumnType::Time => Value::Time((v * NANOS_PER_MICRO).try_into().ok()?),
        ColumnType::Timestamp => Value::Timestamp(v * NANOS_PER_MICRO),
        ColumnType::Duration => Value::Duration(v * NANOS_PER_MICRO),
        _ => return None,
    })
}
