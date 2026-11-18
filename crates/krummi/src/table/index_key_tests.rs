//! Internal tests of the index key encoding (document 3 of the table design; IX8–IX10 of the secondary indexes PRD):
//! the byte order of entries is the order of N5 for every direction and null order, entries decode back to their
//! values, and the intervals of IX9 select exactly the entries of the model. Deterministic: the random sequences come
//! from fixed seeds.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::ops::Bound::{self, Excluded, Included, Unbounded};

use krummi_pager::ValueRule;

use super::index_key::{
    decode_entry, encode_entry, entry_len, index_key_len, index_range, split_entry,
    uniqueness_range,
};
use super::key::{Direction, KeyColumn, KeyError, KeyRange, MAX_KEY, Nulls, encode_key};
use super::value::{
    ColumnType, DATE_MAX, DATE_MIN, DURATION_MAX, Decimal, TIME_MAX, TIMESTAMP_MAX, TIMESTAMP_MIN,
    UuidOrder, Value, normalize,
};
use crate::ids::{Ulid, Uuid};

// ---------------------------------------------------------------------------------------------------------------
// Helpers

/// SplitMix64: a small, deterministic generator.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn pick<T: Clone>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize].clone()
    }

    fn chance(&mut self, one_in: u64) -> bool {
        self.below(one_in) == 0
    }
}

fn s(text: &str) -> Value<'static> {
    Value::String(Cow::Owned(text.to_owned()))
}

fn b(bytes: &[u8]) -> Value<'static> {
    Value::Bytes(Cow::Owned(bytes.to_vec()))
}

fn col(ty: ColumnType, direction: Direction, nulls: Nulls, nullable: bool) -> KeyColumn {
    KeyColumn {
        ty,
        direction,
        nulls,
        nullable,
    }
}

const SHAPES: [(Direction, Nulls); 4] = [
    (Direction::Ascending, Nulls::First),
    (Direction::Ascending, Nulls::Last),
    (Direction::Descending, Nulls::First),
    (Direction::Descending, Nulls::Last),
];

/// Values compared by their debug form, so that the canonical NaN equals itself and `-0` differs from `+0`.
fn same(a: &[Value<'_>], b: &[Value<'_>]) -> bool {
    format!("{a:?}") == format!("{b:?}")
}

fn contains(range: &KeyRange, key: &[u8]) -> bool {
    let (lower, upper) = range.bounds();
    let after = match lower {
        Included(b) => key >= b,
        Excluded(b) => key > b,
        Unbounded => true,
    };
    let before = match upper {
        Included(b) => key <= b,
        Excluded(b) => key < b,
        Unbounded => true,
    };
    after && before
}

/// Every bound of `range` is a key the tree's range API takes: 1 to [`MAX_KEY`] bytes.
fn assert_fits(range: &KeyRange) {
    let (lower, upper) = range.bounds();
    for bound in [lower, upper] {
        if let Included(b) | Excluded(b) = bound {
            assert!((1..=MAX_KEY).contains(&b.len()), "a bound of {}", b.len());
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The reference model of N5

/// SQL Server's order of a `uniqueidentifier` given in RFC 9562 bytes, as `SqlGuid` compares it.
fn sql_server_cmp(a: &[u8; 16], b: &[u8; 16]) -> Ordering {
    let dotnet = |r: &[u8; 16]| -> [u8; 16] {
        let mut d = *r;
        d[..4].reverse();
        d[4..6].reverse();
        d[6..8].reverse();
        d
    };
    let (a, b) = (dotnet(a), dotnet(b));
    const ORDER: [usize; 16] = [10, 11, 12, 13, 14, 15, 8, 9, 6, 7, 4, 5, 0, 1, 2, 3];
    ORDER
        .iter()
        .map(|&i| a[i].cmp(&b[i]))
        .find(|o| o.is_ne())
        .unwrap_or(Ordering::Equal)
}

/// N5 on two normalized values, not null, of a column of type `ty`, by their type and never by their encoding.
fn value_cmp(ty: &ColumnType, a: &Value<'_>, b: &Value<'_>) -> Ordering {
    fn float(a: f64, b: f64) -> Ordering {
        match (a.is_nan(), b.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => a.partial_cmp(&b).unwrap(),
        }
    }
    match (a, b) {
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::I8(a), Value::I8(b)) => a.cmp(b),
        (Value::I32(a), Value::I32(b)) => a.cmp(b),
        (Value::I64(a), Value::I64(b)) => a.cmp(b),
        (Value::U16(a), Value::U16(b)) => a.cmp(b),
        (Value::U64(a), Value::U64(b)) => a.cmp(b),
        (Value::F32(a), Value::F32(b)) => float(f64::from(*a), f64::from(*b)),
        (Value::F64(a), Value::F64(b)) => float(*a, *b),
        (Value::Decimal(a), Value::Decimal(b)) => a.mantissa.cmp(&b.mantissa),
        (Value::String(a), Value::String(b)) => a.as_bytes().cmp(b.as_bytes()),
        (Value::Bytes(a), Value::Bytes(b)) => a.cmp(b),
        (Value::Date(a), Value::Date(b)) => a.cmp(b),
        (Value::Time(a), Value::Time(b)) => a.cmp(b),
        (Value::Timestamp(a), Value::Timestamp(b)) => a.cmp(b),
        (Value::Duration(a), Value::Duration(b)) => a.cmp(b),
        (Value::Ulid(a), Value::Ulid(b)) => a.0.cmp(&b.0),
        (Value::Uuid(a), Value::Uuid(b)) => match ty {
            ColumnType::Uuid(UuidOrder::SqlServer) => sql_server_cmp(&a.0, &b.0),
            _ => a.0.cmp(&b.0),
        },
        _ => panic!("values of different types: {a:?} {b:?}"),
    }
}

/// N5 on two values of an index column, in the forward order of the index: nulls first or last whatever the
/// direction, and the values in their type's order or its reverse.
fn column_cmp(column: &KeyColumn, a: &Value<'_>, b: &Value<'_>) -> Ordering {
    let null_first = if column.nulls == Nulls::First {
        Ordering::Less
    } else {
        Ordering::Greater
    };
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => null_first,
        (_, Value::Null) => null_first.reverse(),
        _ => {
            let order = value_cmp(&column.ty, a, b);
            match column.direction {
                Direction::Ascending => order,
                Direction::Descending => order.reverse(),
            }
        }
    }
}

fn values_cmp(columns: &[KeyColumn], a: &[Value<'_>], b: &[Value<'_>]) -> Ordering {
    columns
        .iter()
        .zip(a.iter().zip(b))
        .map(|(column, (a, b))| column_cmp(column, a, b))
        .find(|o| o.is_ne())
        .unwrap_or(Ordering::Equal)
}

/// An entry of the model: its index values and its primary key values.
type Entry = (Vec<Value<'static>>, Vec<Value<'static>>);

/// IX8: entries by their index key, then by their primary key ascending.
fn entry_cmp(columns: &[KeyColumn], primary: &[KeyColumn], a: &Entry, b: &Entry) -> Ordering {
    values_cmp(columns, &a.0, &b.0).then_with(|| values_cmp(primary, &a.1, &b.1))
}

/// IX9: the index values equal `prefix` on its k columns (a null prefix value asks for the nulls) and, on column k,
/// a value within the bounds, or anything when there is no bound.
fn model_in_range(
    columns: &[KeyColumn],
    values: &[Value<'_>],
    prefix: &[Value<'_>],
    lower: &Bound<Value<'_>>,
    upper: &Bound<Value<'_>>,
) -> bool {
    let k = prefix.len();
    if values_cmp(&columns[..k], &values[..k], prefix).is_ne() {
        return false;
    }
    if matches!((lower, upper), (Unbounded, Unbounded)) {
        return true;
    }
    let value = &values[k];
    if *value == Value::Null {
        return false;
    }
    let cmp = |bound: &Value<'_>| value_cmp(&columns[k].ty, value, bound);
    let after = match lower {
        Included(v) => cmp(v).is_ge(),
        Excluded(v) => cmp(v).is_gt(),
        Unbounded => true,
    };
    let before = match upper {
        Included(v) => cmp(v).is_le(),
        Excluded(v) => cmp(v).is_lt(),
        Unbounded => true,
    };
    after && before
}

// ---------------------------------------------------------------------------------------------------------------
// Random values

const ORDERABLE: [ColumnType; 18] = [
    ColumnType::Bool,
    ColumnType::I8,
    ColumnType::I32,
    ColumnType::I64,
    ColumnType::U16,
    ColumnType::U64,
    ColumnType::F32,
    ColumnType::F64,
    ColumnType::Decimal {
        precision: 38,
        scale: 3,
    },
    ColumnType::String,
    ColumnType::Bytes,
    ColumnType::Date,
    ColumnType::Time,
    ColumnType::Timestamp,
    ColumnType::Duration,
    ColumnType::Uuid(UuidOrder::Rfc9562),
    ColumnType::Uuid(UuidOrder::SqlServer),
    ColumnType::Ulid,
];

/// A random value of `ty` before normalization, often an extreme: `-0`, NaNs with different bits, the ends of each
/// domain, bytes `00` and `FF`, text with U+0000.
fn random_value(rng: &mut Rng, ty: &ColumnType) -> Value<'static> {
    let (r1, r2) = (rng.next(), rng.next());
    match ty {
        ColumnType::Bool => Value::Bool(rng.chance(2)),
        ColumnType::I8 => Value::I8(rng.pick(&[i8::MIN, -1, 0, 1, i8::MAX, r1 as i8])),
        ColumnType::I32 => Value::I32(rng.pick(&[i32::MIN, -256, -1, 0, 255, i32::MAX, r1 as i32])),
        ColumnType::I64 => {
            Value::I64(rng.pick(&[i64::MIN, -1, 0, i64::MAX, r1 as i64, r2 as i64 >> 30]))
        }
        ColumnType::U16 => Value::U16(rng.pick(&[0, u16::MAX, r1 as u16])),
        ColumnType::U64 => Value::U64(rng.pick(&[0, u64::MAX, i64::MAX as u64 + 1, r1, r2 >> 40])),
        ColumnType::F32 => Value::F32(f32::from_bits(rng.pick(&[
            0x8000_0000, // -0
            0x0000_0000,
            0x7FC0_0000,
            0xFFC0_0001, // NaNs with other bits
            0x7F80_0001,
            0x7F80_0000, // +inf
            0xFF80_0000,
            0x0000_0001, // subnormals
            0x8000_0001,
            0xBF80_0000,
            r1 as u32,
        ]))),
        ColumnType::F64 => Value::F64(f64::from_bits(rng.pick(&[
            0x8000_0000_0000_0000,
            0,
            0x7FF8_0000_0000_0000,
            0xFFF8_0000_0000_0001,
            0x7FF0_0000_0000_0000,
            0xFFF0_0000_0000_0000,
            1,
            r1,
            r2,
        ]))),
        ColumnType::Decimal { .. } => {
            let max = 10i128.pow(38) - 1;
            let random = i128::from(r1 as i64) * i128::from(r2 >> 1);
            Value::Decimal(Decimal {
                mantissa: rng.pick(&[-max, max, 0, 1, -1, random]),
                scale: 3,
            })
        }
        ColumnType::String => {
            let parts = ["", "\0", "a", "b", "é", "\u{10FFFF}", "\0\0"];
            let n = rng.below(4);
            Value::String(Cow::Owned((0..n).map(|_| rng.pick(&parts)).collect()))
        }
        ColumnType::Bytes => {
            let n = rng.below(5);
            Value::Bytes(Cow::Owned(
                (0..n)
                    .map(|_| rng.pick(&[0x00, 0x01, 0xFE, 0xFF, 0x41]))
                    .collect(),
            ))
        }
        ColumnType::Date => Value::Date(rng.pick(&[
            DATE_MIN,
            DATE_MAX,
            0,
            -1,
            DATE_MIN + (r1 % (DATE_MAX - DATE_MIN) as u64) as i32,
        ])),
        ColumnType::Time => {
            let max = TIME_MAX * 1000;
            Value::Time(rng.pick(&[0, max, -500, max + 499, (r1 % max as u64) as i64]))
        }
        ColumnType::Timestamp => {
            let (min, max) = (
                i128::from(TIMESTAMP_MIN) * 1000,
                i128::from(TIMESTAMP_MAX) * 1000,
            );
            Value::Timestamp(rng.pick(&[min, max, 0, -1, -500, i128::from(r1 as i64)]))
        }
        ColumnType::Duration => {
            let max = i128::from(DURATION_MAX) * 1000;
            Value::Duration(rng.pick(&[-max, max, 0, -1_000, i128::from(r1 as i64)]))
        }
        ColumnType::Uuid(_) | ColumnType::Ulid => {
            // Each group of bytes that SQL Server compares as a unit is all 00, all FF or random, so that many values
            // tie on the groups compared first and the later groups decide.
            let mut bytes = [0u8; 16];
            for group in [0..4, 4..6, 6..8, 8..10, 10..16] {
                let fill = rng.below(3);
                for byte in &mut bytes[group] {
                    *byte = match fill {
                        0 => 0x00,
                        1 => 0xFF,
                        _ => rng.next() as u8,
                    };
                }
            }
            if *ty == ColumnType::Ulid {
                Value::Ulid(Ulid(bytes))
            } else {
                Value::Uuid(Uuid(bytes))
            }
        }
        _ => unreachable!("not in ORDERABLE"),
    }
}

fn normalized(column: &KeyColumn, value: Value<'static>) -> Value<'static> {
    normalize(&column.ty, column.nullable, value).unwrap()
}

/// A random value of `column`, normalized, null one time in four when the column admits nulls.
fn random_column_value(rng: &mut Rng, column: &KeyColumn) -> Value<'static> {
    if column.nullable && rng.chance(4) {
        Value::Null
    } else {
        normalized(column, random_value(rng, &column.ty))
    }
}

fn random_column(rng: &mut Rng) -> KeyColumn {
    let (direction, nulls) = rng.pick(&SHAPES);
    col(rng.pick(&ORDERABLE), direction, nulls, rng.chance(2))
}

// ---------------------------------------------------------------------------------------------------------------
// Encoding (document 3, *Key encoding, per type*, *Nulls* and *Descending columns*)

#[test]
fn null_elements_follow_the_nulls_table_of_document_3() {
    let pk = encode_key(&[ColumnType::I64], &[Value::I64(5)]).unwrap();
    for ((direction, nulls), element) in
        SHAPES
            .iter()
            .zip([&[0x00][..], &[0x4F], &[0xB0], &[0xFE, 0xFF, 0xFF]])
    {
        let columns = [col(ColumnType::String, *direction, *nulls, true)];
        let entry = encode_entry(&columns, &[Value::Null], &pk).unwrap();
        assert_eq!(
            entry,
            [element, &pk[..]].concat(),
            "{direction:?} {nulls:?}"
        );
        assert_eq!(index_key_len(&columns, &[Value::Null]), element.len());
        assert_eq!(entry_len(&columns, &[Value::Null], &pk), entry.len());
    }
}

#[test]
fn descending_elements_invert_the_ascending_ones_with_00_01_after_text_and_bytes() {
    let pk = encode_key(&[ColumnType::I64], &[Value::I64(0)]).unwrap();
    assert_eq!(pk, [0x14]);
    let cases: [(ColumnType, Value<'static>, &[u8], &[u8]); 7] = [
        (
            ColumnType::String,
            s("a"),
            &[0x02, 0x61, 0x00],
            &[0xFD, 0x9E, 0xFF, 0xFE],
        ),
        (
            ColumnType::String,
            s("ab"),
            &[0x02, 0x61, 0x62, 0x00],
            &[0xFD, 0x9E, 0x9D, 0xFF, 0xFE],
        ),
        (
            ColumnType::Bytes,
            b(&[0x00]),
            &[0x01, 0x00, 0xFF, 0x00],
            &[0xFE, 0xFF, 0x00, 0xFF, 0xFE],
        ),
        (
            ColumnType::Bytes,
            b(&[]),
            &[0x01, 0x00],
            &[0xFE, 0xFF, 0xFE],
        ),
        (ColumnType::I64, Value::I64(5), &[0x15, 0x05], &[0xEA, 0xFA]),
        (
            ColumnType::I64,
            Value::I64(-256),
            &[0x12, 0xFE, 0xFF],
            &[0xED, 0x01, 0x00],
        ),
        (ColumnType::Bool, Value::Bool(true), &[0x27], &[0xD8]),
    ];
    for (ty, value, ascending, descending) in cases {
        for (direction, element) in [
            (Direction::Ascending, ascending),
            (Direction::Descending, descending),
        ] {
            let columns = [col(ty, direction, Nulls::First, false)];
            let values = [value.clone()];
            let entry = encode_entry(&columns, &values, &pk).unwrap();
            assert_eq!(
                entry,
                [element, &pk[..]].concat(),
                "{value:?} {direction:?}"
            );
            assert_eq!(entry_len(&columns, &values, &pk), entry.len());
            let (index, primary) = decode_entry(&columns, &[ColumnType::I64], &entry).unwrap();
            assert!(same(&index, &values) && same(&primary, &[Value::I64(0)]));
        }
    }
}

#[test]
fn an_ascending_column_encodes_as_the_primary_key_does() {
    let mut rng = Rng(0x1D_E7);
    for _ in 0..2000 {
        let ty = rng.pick(&ORDERABLE);
        let column = col(
            ty,
            Direction::Ascending,
            rng.pick(&[Nulls::First, Nulls::Last]),
            false,
        );
        let value = normalized(&column, random_value(&mut rng, &ty));
        let entry = encode_entry(&[column], std::slice::from_ref(&value), &[]).unwrap();
        assert_eq!(entry, encode_key(&[ty], &[value]).unwrap());
    }
}

#[test]
fn a_value_of_another_type_or_a_null_in_a_not_null_column_is_not_encoded() {
    let columns = [
        col(ColumnType::I64, Direction::Ascending, Nulls::First, true),
        col(
            ColumnType::String,
            Direction::Descending,
            Nulls::Last,
            false,
        ),
    ];
    assert_eq!(
        encode_entry(&columns, &[Value::Null, Value::I64(1)], &[0x14]),
        Err(KeyError::Value {
            column: Some(1),
            rule: ValueRule::WrongType
        })
    );
    assert_eq!(
        encode_entry(&columns, &[Value::I64(1), Value::Null], &[0x14]),
        Err(KeyError::Value {
            column: Some(1),
            rule: ValueRule::NullNotAllowed
        })
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Size (IX10)

#[test]
fn the_entry_size_is_the_index_key_and_the_primary_key_with_descending_bytes_counted() {
    let pk = encode_key(&[ColumnType::I64], &[Value::I64(7)]).unwrap();
    // An ascending string of n bytes takes n + 2; descending, n + 3.
    let fits_ascending = "x".repeat(MAX_KEY - 2 - pk.len());
    let value = [s(&fits_ascending)];
    let ascending = [col(
        ColumnType::String,
        Direction::Ascending,
        Nulls::First,
        false,
    )];
    let descending = [col(
        ColumnType::String,
        Direction::Descending,
        Nulls::First,
        false,
    )];
    assert_eq!(entry_len(&ascending, &value, &pk), MAX_KEY);
    assert_eq!(
        encode_entry(&ascending, &value, &pk).unwrap().len(),
        MAX_KEY
    );
    assert_eq!(entry_len(&descending, &value, &pk), MAX_KEY + 1);
    assert_eq!(
        encode_entry(&descending, &value, &pk),
        Err(KeyError::Size {
            received: MAX_KEY + 1
        })
    );
    // A descending null that sorts last takes three bytes, every other null one.
    let nulls_last = [
        col(ColumnType::I64, Direction::Descending, Nulls::Last, true),
        col(ColumnType::I64, Direction::Ascending, Nulls::Last, true),
    ];
    assert_eq!(
        entry_len(&nulls_last, &[Value::Null, Value::Null], &pk),
        3 + 1 + pk.len()
    );
    // A `00` in the text takes one byte more, in both directions.
    let zeros = [s("\0\0")];
    assert_eq!(entry_len(&ascending, &zeros, &pk), 6 + pk.len());
    assert_eq!(entry_len(&descending, &zeros, &pk), 7 + pk.len());
}

// ---------------------------------------------------------------------------------------------------------------
// Order, decoding, uniqueness and intervals against the model

/// Checks every entry of `rows` (index values, primary key values) under `columns` and a primary key of `primary`:
/// the byte order is the model's, each entry decodes and splits back, the uniqueness range holds exactly the entries
/// with equal index values and no null, and each of `ranges` holds exactly the entries the model puts in it.
#[allow(clippy::type_complexity)]
fn check(
    columns: &[KeyColumn],
    primary: &[KeyColumn],
    rows: &[Entry],
    ranges: &[(
        Vec<Value<'static>>,
        Bound<Value<'static>>,
        Bound<Value<'static>>,
    )],
) {
    let primary_types: Vec<ColumnType> = primary.iter().map(|c| c.ty).collect();
    let mut entries = Vec::with_capacity(rows.len());
    for (index, pk) in rows {
        let pk_key = encode_key(&primary_types, pk).unwrap();
        let entry = encode_entry(columns, index, &pk_key).unwrap();
        assert_eq!(entry.len(), entry_len(columns, index, &pk_key));
        let (decoded_index, decoded_pk) = decode_entry(columns, &primary_types, &entry).unwrap();
        assert!(same(&decoded_index, index), "{decoded_index:?} {index:?}");
        assert!(same(&decoded_pk, pk), "{decoded_pk:?} {pk:?}");
        let (index_bytes, pk_bytes) = split_entry(columns, &entry).unwrap();
        assert_eq!(index_bytes.len(), index_key_len(columns, index));
        assert_eq!(pk_bytes, pk_key);
        entries.push(entry);
    }
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&a, &b| entry_cmp(columns, primary, &rows[a], &rows[b]));
    for pair in order.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let expected = entry_cmp(columns, primary, &rows[a], &rows[b]);
        assert_eq!(
            entries[a].cmp(&entries[b]),
            expected,
            "{:?} {:?} of {columns:?}",
            rows[a],
            rows[b]
        );
    }
    for (a, (index, _)) in rows.iter().enumerate() {
        let probe = uniqueness_range(columns, index, &entries[a]);
        if index.contains(&Value::Null) {
            assert!(probe.is_none(), "{index:?}");
            continue;
        }
        let probe = probe.expect("a range without nulls");
        assert_fits(&probe);
        for (other, entry) in rows.iter().zip(&entries) {
            let equal = values_cmp(columns, index, &other.0).is_eq();
            assert_eq!(contains(&probe, entry), equal, "{index:?} {:?}", other.0);
        }
    }
    for (prefix, lower, upper) in ranges {
        let range = index_range(columns, prefix, lower.as_ref(), upper.as_ref()).unwrap();
        assert_fits(&range);
        for ((index, _), entry) in rows.iter().zip(&entries) {
            assert_eq!(
                contains(&range, entry),
                model_in_range(columns, index, prefix, lower, upper),
                "{index:?} in {prefix:?} {lower:?} {upper:?} of {columns:?}"
            );
        }
    }
}

/// Every bound on the column of `values`, with no bound included.
fn bounds(values: &[Value<'static>]) -> Vec<Bound<Value<'static>>> {
    let mut bounds = vec![Unbounded];
    for value in values {
        bounds.push(Included(value.clone()));
        bounds.push(Excluded(value.clone()));
    }
    bounds
}

#[test]
fn ac_ix1_one_column_of_each_type_sorts_as_n5_in_each_direction_and_null_order() {
    let mut rng = Rng(0x0001_C015);
    let primary = [col(
        ColumnType::I64,
        Direction::Ascending,
        Nulls::First,
        false,
    )];
    for ty in ORDERABLE {
        for (direction, nulls) in SHAPES {
            let column = col(ty, direction, nulls, true);
            let columns = [column];
            let mut rows: Vec<Entry> = (0..60)
                .map(|i| {
                    (
                        vec![random_column_value(&mut rng, &column)],
                        vec![Value::I64(i % 7)],
                    )
                })
                .collect();
            rows.sort_by(|a, b| entry_cmp(&columns, &primary, a, b));
            rows.dedup_by(|a, b| entry_cmp(&columns, &primary, a, b).is_eq());
            let values: Vec<Value<'static>> = rows
                .iter()
                .map(|(index, _)| index[0].clone())
                .filter(|v| *v != Value::Null)
                .take(6)
                .collect();
            let mut ranges = Vec::new();
            for lower in bounds(&values) {
                for upper in bounds(&values) {
                    ranges.push((vec![], lower.clone(), upper));
                }
            }
            ranges.push((vec![Value::Null], Unbounded, Unbounded));
            for value in &values {
                ranges.push((vec![value.clone()], Unbounded, Unbounded));
            }
            check(&columns, &primary, &rows, &ranges);
        }
    }
}

#[test]
fn ac_ix1_two_consecutive_columns_of_every_shape_sort_as_n5() {
    // Text and bytes with `00` (U+0000) and prefixes of each other, and an integer, in every direction, null order
    // and nullability, followed by an integer primary key: the cases where a terminator or a null could read as an
    // escape (document 3, *Descending columns*).
    let texts = ["", "\0", "\0\0", "a", "a\0", "a\0b", "ab", "b"];
    let values_of = |ty: ColumnType| -> Vec<Value<'static>> {
        match ty {
            ColumnType::String => texts.iter().map(|t| s(t)).collect(),
            ColumnType::Bytes => texts.iter().map(|t| b(t.as_bytes())).collect(),
            _ => [-256, -1, 0, 1, 255, 256].map(Value::I32).to_vec(),
        }
    };
    let types = [ColumnType::String, ColumnType::Bytes, ColumnType::I32];
    let mut specs = Vec::new();
    for ty in types {
        for (direction, nulls) in SHAPES {
            for nullable in [false, true] {
                specs.push(col(ty, direction, nulls, nullable));
            }
        }
    }
    let primary = [col(
        ColumnType::I64,
        Direction::Ascending,
        Nulls::First,
        false,
    )];
    let with_null = |column: &KeyColumn| {
        let mut values = values_of(column.ty);
        if column.nullable {
            values.push(Value::Null);
        }
        values
    };
    for first in &specs {
        for second in &specs {
            let columns = [*first, *second];
            let (firsts, seconds) = (with_null(first), with_null(second));
            let mut rows = Vec::new();
            for x in &firsts {
                for y in &seconds {
                    for id in [3, 1] {
                        rows.push((vec![x.clone(), y.clone()], vec![Value::I64(id)]));
                    }
                }
            }
            // Bounds on the first column, and on the second after each prefix of the first.
            let picks = |values: &[Value<'static>]| -> Vec<Value<'static>> {
                [0, 3, 4].iter().map(|&i| values[i].clone()).collect()
            };
            let mut ranges = Vec::new();
            for lower in bounds(&picks(&firsts)) {
                for upper in bounds(&picks(&firsts)) {
                    ranges.push((vec![], lower.clone(), upper));
                }
            }
            for x in &firsts {
                for lower in bounds(&picks(&seconds)) {
                    for upper in [
                        Unbounded,
                        Included(seconds[4].clone()),
                        Excluded(seconds[3].clone()),
                    ] {
                        ranges.push((vec![x.clone()], lower.clone(), upper));
                    }
                }
                for y in &seconds {
                    ranges.push((vec![x.clone(), y.clone()], Unbounded, Unbounded));
                }
            }
            check(&columns, &primary, &rows, &ranges);
        }
    }
}

/// Pads the last `string` or `bytes` index value of `index`, when it has one not null, so that the entry has `target`
/// bytes.
fn pad_to(columns: &[KeyColumn], index: &mut [Value<'static>], pk_len: usize, target: usize) {
    let Some(i) = (0..columns.len()).rev().find(|&i| {
        matches!(columns[i].ty, ColumnType::String | ColumnType::Bytes) && index[i] != Value::Null
    }) else {
        return;
    };
    let len = index_key_len(columns, index) + pk_len;
    if len >= target {
        return;
    }
    let extra = target - len;
    match &mut index[i] {
        Value::String(text) => text.to_mut().push_str(&"a".repeat(extra)),
        Value::Bytes(bytes) => bytes.to_mut().extend(std::iter::repeat_n(0x41, extra)),
        _ => unreachable!(),
    }
}

#[test]
fn ac_ix5_random_indexes_follow_the_model_of_n5() {
    let mut rng = Rng(0x0001_DEC5);
    let mut checked = 0;
    let mut at_limit = 0;
    for _ in 0..300 {
        let columns: Vec<KeyColumn> = (0..1 + rng.below(4))
            .map(|_| random_column(&mut rng))
            .collect();
        let primary: Vec<KeyColumn> = (0..1 + rng.below(2))
            .map(|_| {
                col(
                    rng.pick(&ORDERABLE),
                    Direction::Ascending,
                    Nulls::First,
                    false,
                )
            })
            .collect();
        let primary_types: Vec<ColumnType> = primary.iter().map(|c| c.ty).collect();
        let mut rows: Vec<Entry> = Vec::new();
        for _ in 0..40 {
            let mut index: Vec<Value<'static>> = if !rows.is_empty() && rng.chance(4) {
                rows[rng.below(rows.len() as u64) as usize].0.clone()
            } else {
                columns
                    .iter()
                    .map(|c| random_column_value(&mut rng, c))
                    .collect()
            };
            let pk: Vec<Value<'static>> = primary
                .iter()
                .map(|c| random_column_value(&mut rng, c))
                .collect();
            let Ok(pk_key) = encode_key(&primary_types, &pk) else {
                continue;
            };
            if rng.chance(4) {
                pad_to(
                    &columns,
                    &mut index,
                    pk_key.len(),
                    rng.pick(&[MAX_KEY - 1, MAX_KEY, MAX_KEY + 1]),
                );
            }
            let len = entry_len(&columns, &index, &pk_key);
            if len > MAX_KEY {
                assert_eq!(
                    encode_entry(&columns, &index, &pk_key),
                    Err(KeyError::Size { received: len })
                );
                continue;
            }
            at_limit += usize::from(len == MAX_KEY);
            let entry = (index, pk);
            if !rows
                .iter()
                .any(|row| entry_cmp(&columns, &primary, row, &entry).is_eq())
            {
                rows.push(entry);
            }
        }
        // Intervals: a prefix of k columns from an entry (nulls included) or random, bounds on column k from entries
        // or random, given before normalization (`-0`, NaNs with other bits).
        let mut ranges = Vec::new();
        for _ in 0..20 {
            let k = rng.below(columns.len() as u64 + 1) as usize;
            let raw: Vec<Value<'static>> = if rows.is_empty() || rng.chance(5) {
                columns
                    .iter()
                    .map(|c| random_value(&mut rng, &c.ty))
                    .collect()
            } else {
                rows[rng.below(rows.len() as u64) as usize].0.clone()
            };
            let prefix = raw[..k].to_vec();
            let bound = |rng: &mut Rng| -> (Bound<Value<'static>>, Bound<Value<'static>>) {
                if k == columns.len() {
                    return (Unbounded, Unbounded);
                }
                let value = rows
                    .iter()
                    .map(|row| row.0[k].clone())
                    .find(|v| *v != Value::Null && rng.chance(2))
                    .unwrap_or_else(|| random_value(rng, &columns[k].ty));
                let model = normalize(&columns[k].ty, false, value.clone()).unwrap();
                match rng.below(3) {
                    0 => (Included(value), Included(model)),
                    1 => (Excluded(value), Excluded(model)),
                    _ => (Unbounded, Unbounded),
                }
            };
            let ((lower, lower_model), (upper, upper_model)) = (bound(&mut rng), bound(&mut rng));
            let model_prefix: Vec<Value<'static>> = prefix
                .iter()
                .zip(&columns)
                .map(|(v, c)| normalized(c, v.clone()))
                .collect();
            let range = index_range(&columns, &prefix, lower.as_ref(), upper.as_ref());
            match range {
                Err(KeyError::Size { received }) => {
                    assert!(received > MAX_KEY);
                    continue;
                }
                Err(error) => panic!("{error:?}"),
                Ok(range) => {
                    // Each value given before normalization selects what its normalized value does.
                    let normal = index_range(
                        &columns,
                        &model_prefix,
                        lower_model.as_ref(),
                        upper_model.as_ref(),
                    )
                    .unwrap();
                    assert_eq!(range, normal);
                }
            }
            ranges.push((model_prefix, lower_model, upper_model));
        }
        checked += ranges.len();
        check(&columns, &primary, &rows, &ranges);
    }
    assert!(checked > 4000, "{checked} intervals");
    assert!(at_limit > 30, "{at_limit} entries of 2,048 bytes");
}

// ---------------------------------------------------------------------------------------------------------------
// Intervals (IX9)

#[test]
fn ix9_bounds_follow_the_table_of_document_3_in_each_direction() {
    let pk = encode_key(&[ColumnType::I64], &[Value::I64(1)]).unwrap();
    let ascending = [col(
        ColumnType::I64,
        Direction::Ascending,
        Nulls::First,
        false,
    )];
    let descending = [col(
        ColumnType::I64,
        Direction::Descending,
        Nulls::First,
        false,
    )];
    let enc = |columns: &[KeyColumn], v: i64| {
        let entry = encode_entry(columns, &[Value::I64(v)], &pk).unwrap();
        entry[..entry.len() - pk.len()].to_vec()
    };
    let with = |element: Vec<u8>, ff: bool| {
        let mut bound = element;
        if ff {
            bound.push(0xFF);
        }
        bound
    };
    let (lo, hi) = (Value::I64(2), Value::I64(9));
    let range = index_range(&ascending, &[], Included(&lo), Excluded(&hi)).unwrap();
    assert_eq!(
        range,
        KeyRange {
            lower: Included(with(enc(&ascending, 2), false)),
            upper: Excluded(with(enc(&ascending, 9), false)),
        }
    );
    let range = index_range(&ascending, &[], Excluded(&lo), Included(&hi)).unwrap();
    assert_eq!(
        range,
        KeyRange {
            lower: Included(with(enc(&ascending, 2), true)),
            upper: Excluded(with(enc(&ascending, 9), true)),
        }
    );
    // A descending column swaps them: the upper value bounds the start of the bytes.
    let range = index_range(&descending, &[], Included(&lo), Excluded(&hi)).unwrap();
    assert_eq!(
        range,
        KeyRange {
            lower: Included(with(enc(&descending, 9), true)),
            upper: Excluded(with(enc(&descending, 2), true)),
        }
    );
    let range = index_range(&descending, &[], Excluded(&lo), Included(&hi)).unwrap();
    assert_eq!(
        range,
        KeyRange {
            lower: Included(with(enc(&descending, 9), false)),
            upper: Excluded(with(enc(&descending, 2), false)),
        }
    );
    // No prefix and no bound: the whole index.
    let range = index_range(&descending, &[], Unbounded, Unbounded).unwrap();
    assert_eq!(range.bounds(), (Unbounded, Unbounded));
}

#[test]
fn ix9_with_a_bound_the_nulls_are_outside_and_without_bounds_inside() {
    let pk = encode_key(&[ColumnType::I64], &[Value::I64(1)]).unwrap();
    for (direction, nulls) in SHAPES {
        let columns = [col(ColumnType::I32, direction, nulls, true)];
        let null = encode_entry(&columns, &[Value::Null], &pk).unwrap();
        let five = encode_entry(&columns, &[Value::I32(5)], &pk).unwrap();
        let shape = format!("{direction:?} {nulls:?}");
        let all = index_range(&columns, &[], Unbounded, Unbounded).unwrap();
        assert!(contains(&all, &null) && contains(&all, &five), "{shape}");
        let v = Value::I32(0);
        for (lower, upper) in [
            (Included(&v), Unbounded),
            (Unbounded, Included(&Value::I32(9))),
        ] {
            let range = index_range(&columns, &[], lower, upper).unwrap();
            assert!(!contains(&range, &null), "{shape} {range:?}");
            assert!(
                contains(&range, &five) == matches!(lower, Included(_))
                    || matches!(upper, Included(_)),
                "{shape}"
            );
        }
        let nulls_only = index_range(&columns, &[Value::Null], Unbounded, Unbounded).unwrap();
        assert!(
            contains(&nulls_only, &null) && !contains(&nulls_only, &five),
            "{shape}"
        );
    }
}

#[test]
fn ix9_negative_zero_finds_zero_and_any_nan_the_stored_nan() {
    let pk = encode_key(&[ColumnType::I64], &[Value::I64(1)]).unwrap();
    for (direction, nulls) in SHAPES {
        let columns = [col(ColumnType::F64, direction, nulls, true)];
        let zero = encode_entry(&columns, &[Value::F64(0.0)], &pk).unwrap();
        let nan = encode_entry(&columns, &[Value::F64(f64::NAN)], &pk).unwrap();
        let other_nan = Value::F64(f64::from_bits(0xFFF8_0000_0000_0001));
        let range = index_range(&columns, &[Value::F64(-0.0)], Unbounded, Unbounded).unwrap();
        assert!(contains(&range, &zero) && !contains(&range, &nan));
        let range = index_range(
            &columns,
            std::slice::from_ref(&other_nan),
            Unbounded,
            Unbounded,
        )
        .unwrap();
        assert!(contains(&range, &nan) && !contains(&range, &zero));
        let range = index_range(
            &columns,
            &[],
            Included(&Value::F64(-0.0)),
            Included(&other_nan),
        )
        .unwrap();
        assert!(contains(&range, &nan) && contains(&range, &zero));
        let range = index_range(
            &columns,
            &[],
            Excluded(&Value::F64(-0.0)),
            Excluded(&other_nan),
        )
        .unwrap();
        assert!(!contains(&range, &nan) && !contains(&range, &zero));
    }
}

#[test]
fn ix9_malformed_intervals_fail_in_the_order_of_ix9() {
    let columns = [
        col(ColumnType::I32, Direction::Descending, Nulls::Last, false),
        col(ColumnType::Date, Direction::Ascending, Nulls::First, true),
    ];
    let invalid = |column| KeyError::Value {
        column,
        rule: ValueRule::InvalidRange,
    };
    let rule = |column, rule| KeyError::Value {
        column: Some(column),
        rule,
    };
    let wrong = Value::Bool(true);
    let out = Value::Date(DATE_MAX + 1);
    // A prefix longer than the key, or a bound with the whole key as prefix, before any value.
    assert_eq!(
        index_range(
            &columns,
            &[wrong.clone(), out.clone(), Value::I32(0)],
            Unbounded,
            Unbounded
        ),
        Err(invalid(None))
    );
    assert_eq!(
        index_range(
            &columns,
            &[wrong.clone(), out.clone()],
            Included(&wrong),
            Unbounded
        ),
        Err(invalid(None))
    );
    // Then the first value that fails: the prefix in key order, the lower bound, the upper bound.
    assert_eq!(
        index_range(&columns, &[Value::Null], Included(&out), Unbounded),
        Err(rule(0, ValueRule::NullNotAllowed))
    );
    assert_eq!(
        index_range(
            &columns,
            std::slice::from_ref(&wrong),
            Included(&out),
            Unbounded
        ),
        Err(rule(0, ValueRule::WrongType))
    );
    assert_eq!(
        index_range(&columns, &[Value::I32(0)], Included(&out), Included(&wrong)),
        Err(rule(1, ValueRule::OutOfDomain))
    );
    assert_eq!(
        index_range(
            &columns,
            &[Value::I32(0)],
            Included(&Value::Date(0)),
            Included(&wrong)
        ),
        Err(rule(1, ValueRule::WrongType))
    );
    // A null as a bound, even of a column that admits nulls.
    assert_eq!(
        index_range(
            &columns,
            &[Value::I32(0)],
            Unbounded,
            Excluded(&Value::Null)
        ),
        Err(rule(1, ValueRule::InvalidRange))
    );
    assert_eq!(
        index_range(&columns, &[], Included(&Value::Null), Included(&wrong)),
        Err(rule(0, ValueRule::InvalidRange))
    );
    // A null prefix value of a column that admits nulls, and an inverted interval, are valid.
    assert!(
        index_range(
            &columns,
            &[Value::I32(0), Value::Null],
            Unbounded,
            Unbounded
        )
        .is_ok()
    );
    assert!(
        index_range(
            &columns,
            &[],
            Included(&Value::I32(9)),
            Included(&Value::I32(1))
        )
        .is_ok()
    );
}

#[test]
fn ix9_a_prefix_or_bound_over_2048_bytes_fails_with_its_size_even_when_empty() {
    let columns = [
        col(
            ColumnType::String,
            Direction::Descending,
            Nulls::First,
            false,
        ),
        col(ColumnType::I64, Direction::Ascending, Nulls::First, false),
    ];
    // A descending string of n bytes takes n + 3.
    let full = s(&"x".repeat(MAX_KEY - 3));
    let over = s(&"x".repeat(MAX_KEY - 2));
    assert!(index_range(&columns, std::slice::from_ref(&full), Unbounded, Unbounded).is_ok());
    assert_eq!(
        index_range(&columns, &[over], Unbounded, Unbounded),
        Err(KeyError::Size {
            received: MAX_KEY + 1
        })
    );
    // The prefix fits; with its bound it does not, even with the bounds inverted.
    let (lo, hi) = (Value::I64(9), Value::I64(1));
    assert_eq!(
        index_range(
            &columns,
            std::slice::from_ref(&full),
            Included(&lo),
            Included(&hi)
        ),
        Err(KeyError::Size {
            received: MAX_KEY + 2
        })
    );
    // A bound on the first column: the bound is measured as a prefix.
    assert_eq!(
        index_range(
            &columns,
            &[],
            Unbounded,
            Excluded(&s(&"x".repeat(MAX_KEY - 2)))
        ),
        Err(KeyError::Size {
            received: MAX_KEY + 1
        })
    );
}

#[test]
fn ix9_bounds_past_2048_bytes_become_2048_byte_bounds_of_the_same_entries() {
    // A prefix of 2,047 bytes followed by a nullable column with a descending null that sorts last: the bound
    // `P ‖ null` passes the maximum key, so it is cut to 2,048 bytes; a null in the prefix makes it too large.
    let columns = [
        col(ColumnType::Bytes, Direction::Ascending, Nulls::First, false),
        col(ColumnType::Bool, Direction::Descending, Nulls::Last, true),
    ];
    let prefix = [b(&vec![0x41; MAX_KEY - 3])];
    for (lower, upper) in [
        (Included(&Value::Bool(false)), Unbounded),
        (Unbounded, Excluded(&Value::Bool(true))),
    ] {
        let range = index_range(&columns, &prefix, lower, upper).unwrap();
        assert_fits(&range);
    }
    assert_eq!(
        index_range(
            &columns,
            &[prefix[0].clone(), Value::Null],
            Unbounded,
            Unbounded
        ),
        Err(KeyError::Size {
            received: MAX_KEY + 2
        })
    );
    // A prefix of 2,046 bytes: its entries take 2,048 bytes (a null one would not fit), and `P ‖ null`, the end of an
    // interval with no upper value, takes 2,049; the cut bound selects the same entries as the model.
    let prefix = vec![b(&vec![0x41; MAX_KEY - 4])];
    let rows: Vec<Entry> = [Value::Bool(false), Value::Bool(true)]
        .into_iter()
        .map(|v| ([prefix.clone(), vec![v]].concat(), vec![Value::I64(0)]))
        .collect();
    let ranges = [Value::Bool(false), Value::Bool(true)]
        .into_iter()
        .flat_map(|v| {
            [
                (prefix.clone(), Included(v.clone()), Unbounded),
                (prefix.clone(), Excluded(v.clone()), Unbounded),
                (prefix.clone(), Unbounded, Included(v.clone())),
                (prefix.clone(), Unbounded, Excluded(v)),
            ]
        })
        .collect::<Vec<_>>();
    let primary = [col(
        ColumnType::I64,
        Direction::Ascending,
        Nulls::First,
        false,
    )];
    check(&columns, &primary, &rows, &ranges);
}
