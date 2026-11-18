//! The keys of a secondary index's entries (document 3 of the table design, *Index structure*; IX8–IX10 of the
//! secondary indexes PRD): the elements of the index columns, each with its direction and null order
//! ([`KeyColumn`]), followed by the row's primary key encoded as the table tree's key. The index columns alone are
//! the uniqueness prefix; their elements and the primary key decode back into owned values; and an interval of IX9
//! is a range of entry keys.

use std::ops::Bound::{self, Unbounded};

use krummi_pager::ValueRule;

use super::key::{KeyColumn, KeyError, KeyRange, MAX_KEY, bound_map, decode_key, range_bounds};
use super::value::{ColumnType, Value, normalize};

/// The encoded length of the index columns `columns` of `values`, without encoding them: the length of an entry's
/// uniqueness prefix. `values` are those of [`encode_entry`]. Panics if there is not one value per column.
pub(crate) fn index_key_len(columns: &[KeyColumn], values: &[Value<'_>]) -> usize {
    assert_eq!(columns.len(), values.len(), "one value per index column");
    columns
        .iter()
        .zip(values)
        .map(|(column, value)| column.element_len(value))
        .sum()
}

/// The size of an entry (IX10): the index columns of `values` followed by the encoded primary key `primary`, in the
/// form Krummi stores keys; included columns do not count.
pub(crate) fn entry_len(columns: &[KeyColumn], values: &[Value<'_>], primary: &[u8]) -> usize {
    index_key_len(columns, values) + primary.len()
}

/// The key of the entry of a row: `values`, normalized values of the index columns `columns` in key order, then
/// `primary`, the row's encoded primary key (its key in the table tree). Fails with the size of an entry over
/// [`MAX_KEY`] bytes (IX10), or with the first value, in key order, that is not of its column's type or is a null in a
/// column without nulls. Panics if there is not one value per column.
pub(crate) fn encode_entry(
    columns: &[KeyColumn],
    values: &[Value<'_>],
    primary: &[u8],
) -> Result<Vec<u8>, KeyError> {
    let len = entry_len(columns, values, primary);
    if len > MAX_KEY {
        return Err(KeyError::Size { received: len });
    }
    let mut entry = Vec::with_capacity(len);
    for (column, (key_column, value)) in columns.iter().zip(values).enumerate() {
        key_column
            .encode(value, &mut entry)
            .map_err(|rule| KeyError::Value {
                column: Some(column),
                rule,
            })?;
    }
    entry.extend_from_slice(primary);
    Ok(entry)
}

/// The entries of a unique index that collide with `entry`, the key [`encode_entry`] gives for `values`: those whose
/// index columns have the same values, the range `[P, P ‖ FF)` of the uniqueness prefix P (document 3, *Unique
/// index*); or `None` when a value is null, since an entry with a null never collides (N7).
pub(crate) fn uniqueness_range(
    columns: &[KeyColumn],
    values: &[Value<'_>],
    entry: &[u8],
) -> Option<KeyRange> {
    if values.iter().any(|value| matches!(value, Value::Null)) {
        return None;
    }
    let prefix = entry[..index_key_len(columns, values)].to_vec();
    Some(range_bounds(prefix, None, Unbounded, Unbounded).expect("a range without bounds"))
}

/// The bytes of the index columns of `entry` and those of its primary key, the row's key in the table tree; `None`
/// when the entry's index elements are not elements of `columns`.
pub(crate) fn split_entry<'e>(
    columns: &[KeyColumn],
    entry: &'e [u8],
) -> Option<(&'e [u8], &'e [u8])> {
    let mut rest = entry;
    for column in columns {
        column.skip(&mut rest)?;
    }
    Some(entry.split_at(entry.len() - rest.len()))
}

/// The values of `entry`, in copies of their own (IX8): those of its index columns `columns`, then those of the
/// primary key columns `primary`; `None` when the bytes are not such an entry (the checks of [`decode_key`]).
#[allow(clippy::type_complexity)]
pub(crate) fn decode_entry(
    columns: &[KeyColumn],
    primary: &[ColumnType],
    entry: &[u8],
) -> Option<(Vec<Value<'static>>, Vec<Value<'static>>)> {
    let mut rest = entry;
    let index = columns
        .iter()
        .map(|column| column.decode(&mut rest))
        .collect::<Option<Vec<_>>>()?;
    Some((index, decode_key(primary, rest)?))
}

/// The byte bounds of an interval of an index (IX9): the values `prefix` for its first k columns, a null asking for
/// the entries whose column is null, and, when k is less than the number of columns, a `lower` and an `upper` bound on
/// column k in the order of N5 of its type, whatever its direction. With a bound, the nulls of column k are outside the
/// interval; without bounds, inside. Values are normalized and rounded (N4, E7, E8) as those of a row.
///
/// Fails, in this order (IX9), with [`ValueRule::InvalidRange`] and no column for a prefix longer than the key or a
/// bound when the prefix gives every column; with the first value that fails, the prefix in key order, then the lower
/// and the upper bound: a value that breaks a rule of its column ([`normalize`]; a null in the prefix of a column
/// without nulls breaks [`ValueRule::NullNotAllowed`]) or a null bound ([`ValueRule::InvalidRange`]), with its column's
/// position in the index; and with the size of the prefix, or of the prefix and a bound, encoded as in an entry, that
/// does not fit in a key, even when the interval is empty. An inverted interval is valid: it selects nothing.
pub(crate) fn index_range<'v>(
    columns: &[KeyColumn],
    prefix: &[Value<'v>],
    lower: Bound<&Value<'v>>,
    upper: Bound<&Value<'v>>,
) -> Result<KeyRange, KeyError> {
    let k = prefix.len();
    let has_bound = !matches!((lower, upper), (Unbounded, Unbounded));
    if k > columns.len() || (k == columns.len() && has_bound) {
        return Err(KeyError::Value {
            column: None,
            rule: ValueRule::InvalidRange,
        });
    }
    let fault = |column: usize| {
        move |rule| KeyError::Value {
            column: Some(column),
            rule,
        }
    };
    let prefix = columns
        .iter()
        .zip(prefix)
        .enumerate()
        .map(|(i, (column, value))| {
            normalize(&column.ty, column.nullable, value.clone()).map_err(fault(i))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let bound = |value: &Value<'v>| match value {
        Value::Null => Err(fault(k)(ValueRule::InvalidRange)),
        value => normalize(&columns[k].ty, false, value.clone()).map_err(fault(k)),
    };
    let lower = bound_map(lower, bound)?;
    let upper = bound_map(upper, bound)?;
    // An entry without a primary key: the prefix, with the check of its size.
    let p = encode_entry(&columns[..k], &prefix, &[])?;
    range_bounds(p, columns.get(k), lower.as_ref(), upper.as_ref())
}
