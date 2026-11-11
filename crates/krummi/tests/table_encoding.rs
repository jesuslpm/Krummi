//! The encodings of tables with schema (7.1): values and their rules (AC6, AC7, AC-N2), the primary key encoding of
//! document 3 with its range bounds (AC-N1, AC17) and the row format of document 1. Deterministic: the random
//! sequences come from fixed seeds.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::ops::Bound::{self, Excluded, Included, Unbounded};

use krummi::ids::{Ulid, Uuid};
use krummi::table::key::{
    KeyError, KeyRange, MAX_KEY, decode_key, encode_key, is_key, key_len, key_range, key_starts,
    key_value,
};
use krummi::table::row::{MAX_ROW, RowColumn, RowDamaged, RowLayout, row_version};
use krummi::table::value::{
    ColumnType, DATE_MAX, DATE_MIN, DURATION_MAX, Decimal, TIME_MAX, TIMESTAMP_MAX, TIMESTAMP_MIN,
    UuidOrder, Value, VectorElement, VectorType, check_type, f16_to_f64, key_text, normalize,
    vector_norm,
};
use krummi::{SchemaRule, ValueRule};

// ---------------------------------------------------------------------------------------------------------------
// Helpers

fn s(text: &str) -> Value<'_> {
    Value::String(Cow::Borrowed(text))
}

fn b(bytes: &[u8]) -> Value<'_> {
    Value::Bytes(Cow::Borrowed(bytes))
}

fn dec(mantissa: i128, scale: u8) -> Value<'static> {
    Value::Decimal(Decimal { mantissa, scale })
}

fn vector(element: VectorElement, dimension: u16, cosine: bool) -> ColumnType {
    ColumnType::Vector(VectorType {
        element,
        dimension,
        cosine,
    })
}

fn vec_value(bytes: Vec<u8>) -> Value<'static> {
    Value::Vector(Cow::Owned(bytes))
}

/// The normalized value of a not-null column, or the rule it breaks.
fn norm(ty: ColumnType, value: Value<'_>) -> Result<Value<'_>, ValueRule> {
    normalize(&ty, false, value)
}

/// Values compared by their debug form, so that the canonical NaN equals itself and `-0` differs from `+0`.
fn same(a: &[Value<'_>], b: &[Value<'_>]) -> bool {
    format!("{a:?}") == format!("{b:?}")
}

const NANOS: i128 = 1000;

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

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u64) as usize]
    }

    fn chance(&mut self, one_in: u64) -> bool {
        self.below(one_in) == 0
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Values: domains and rounding (AC6, E8)

#[test]
fn date_domain_is_0001_to_9999() {
    for days in [DATE_MIN, 0, DATE_MAX] {
        assert_eq!(
            norm(ColumnType::Date, Value::Date(days)),
            Ok(Value::Date(days))
        );
    }
    for days in [DATE_MIN - 1, DATE_MAX + 1] {
        assert_eq!(
            norm(ColumnType::Date, Value::Date(days)),
            Err(ValueRule::OutOfDomain)
        );
    }
    assert_eq!(Value::Date(DATE_MIN).to_string(), "0001-01-01");
    assert_eq!(Value::Date(DATE_MAX).to_string(), "9999-12-31");
    assert_eq!(Value::Date(0).to_string(), "1970-01-01");
}

#[test]
fn time_domain_and_rounding() {
    let max = i64::try_from(i128::from(TIME_MAX) * NANOS).unwrap();
    assert_eq!(norm(ColumnType::Time, Value::Time(0)), Ok(Value::Time(0)));
    assert_eq!(
        norm(ColumnType::Time, Value::Time(max)),
        Ok(Value::Time(max))
    );
    assert_eq!(Value::Time(max).to_string(), "23:59:59.999999");
    // One microsecond past each end.
    assert_eq!(
        norm(ColumnType::Time, Value::Time(-1000)),
        Err(ValueRule::OutOfDomain)
    );
    assert_eq!(
        norm(ColumnType::Time, Value::Time(max + 1000)),
        Err(ValueRule::OutOfDomain)
    );
    // Rounding to the microsecond with ties up: -500 ns is 0, -501 ns is before midnight, 23:59:59.9999995 is 24:00.
    assert_eq!(
        norm(ColumnType::Time, Value::Time(-500)),
        Ok(Value::Time(0))
    );
    assert_eq!(
        norm(ColumnType::Time, Value::Time(-501)),
        Err(ValueRule::OutOfDomain)
    );
    assert_eq!(
        norm(ColumnType::Time, Value::Time(max + 499)),
        Ok(Value::Time(max))
    );
    assert_eq!(
        norm(ColumnType::Time, Value::Time(max + 500)),
        Err(ValueRule::OutOfDomain)
    );
    assert_eq!(
        norm(ColumnType::Time, Value::Time(1_100)),
        Ok(Value::Time(1_000))
    );
}

#[test]
fn timestamp_domain_and_rounding() {
    let min = i128::from(TIMESTAMP_MIN) * NANOS;
    let max = i128::from(TIMESTAMP_MAX) * NANOS;
    let ts = |nanos| norm(ColumnType::Timestamp, Value::Timestamp(nanos));
    assert_eq!(ts(min), Ok(Value::Timestamp(min)));
    assert_eq!(ts(max), Ok(Value::Timestamp(max)));
    assert_eq!(ts(min - NANOS), Err(ValueRule::OutOfDomain));
    assert_eq!(ts(max + NANOS), Err(ValueRule::OutOfDomain));
    assert_eq!(Value::Timestamp(min).to_string(), "0001-01-01T00:00:00Z");
    assert_eq!(
        Value::Timestamp(max).to_string(),
        "9999-12-31T23:59:59.999999Z"
    );
    // An instant with a fraction of 100 ns reads rounded to the microsecond below; 500 ns is a tie, which goes up.
    let instant = 1_762_810_359_123_456 * NANOS;
    assert_eq!(ts(instant + 100), Ok(Value::Timestamp(instant)));
    assert_eq!(ts(instant + 499), Ok(Value::Timestamp(instant)));
    assert_eq!(ts(instant + 500), Ok(Value::Timestamp(instant + NANOS)));
    assert_eq!(ts(-100), Ok(Value::Timestamp(0)));
    assert_eq!(ts(-500), Ok(Value::Timestamp(0)));
    assert_eq!(ts(-501), Ok(Value::Timestamp(-NANOS)));
    // 0001-01-01T00:00:00.0000005Z reads 0001-01-01T00:00:00.000001Z; 9999-12-31T23:59:59.9999995Z is past the end.
    let rounded = ts(min + 500).unwrap();
    assert_eq!(rounded.to_string(), "0001-01-01T00:00:00.000001Z");
    assert_eq!(ts(min - 500), Ok(Value::Timestamp(min)));
    assert_eq!(ts(max + 500), Err(ValueRule::OutOfDomain));
    assert_eq!(ts(max + 499), Ok(Value::Timestamp(max)));
}

#[test]
fn duration_domain_and_rounding() {
    let max = i128::from(DURATION_MAX) * NANOS;
    let d = |nanos| norm(ColumnType::Duration, Value::Duration(nanos));
    assert_eq!(d(max), Ok(Value::Duration(max)));
    assert_eq!(d(-max), Ok(Value::Duration(-max)));
    assert_eq!(d(max + NANOS), Err(ValueRule::OutOfDomain));
    assert_eq!(d(-max - NANOS), Err(ValueRule::OutOfDomain));
    assert_eq!(d(-max - 500), Ok(Value::Duration(-max)));
    assert_eq!(d(-max - 501), Err(ValueRule::OutOfDomain));
    assert_eq!(d(max + 500), Err(ValueRule::OutOfDomain));
    assert_eq!(d(-1_500), Ok(Value::Duration(-1_000)));
    assert_eq!(Value::Duration(-1_500_000_000).to_string(), "-1.5s");
    assert_eq!(Value::Duration(0).to_string(), "0s");
}

#[test]
fn decimal_rounds_ties_away_from_zero() {
    let ty = ColumnType::Decimal {
        precision: 10,
        scale: 2,
    };
    // 1.235 -> 1.24 and -1.235 -> -1.24 (E8), each tie and sign, and the non-ties around them.
    for (given, expected) in [
        (dec(1235, 3), 124),
        (dec(-1235, 3), -124),
        (dec(1225, 3), 123),
        (dec(-1225, 3), -123),
        (dec(1234, 3), 123),
        (dec(-1234, 3), -123),
        (dec(1236, 3), 124),
        (dec(-1236, 3), -124),
        (dec(12_349_999, 7), 123),
        (dec(5, 3), 1),
        (dec(-5, 3), -1),
        (dec(4, 3), 0),
        (dec(12, 1), 120),
        (dec(7, 0), 700),
        (dec(i128::MAX, 200), 0),
        (dec(i128::MIN, 41), 0),
        (dec(i128::MAX, 40), 2),
    ] {
        assert_eq!(norm(ty, given.clone()), Ok(dec(expected, 2)), "{given:?}");
    }
    assert_eq!(dec(-124, 2).to_string(), "-1.24");
    assert_eq!(dec(5, 3).to_string(), "0.005");
    assert_eq!(dec(-7, 0).to_string(), "-7");
}

#[test]
fn decimal_precision_is_checked_after_rounding() {
    let ty = ColumnType::Decimal {
        precision: 4,
        scale: 2,
    };
    assert_eq!(norm(ty, dec(9999, 2)), Ok(dec(9999, 2)));
    assert_eq!(norm(ty, dec(-9999, 2)), Ok(dec(-9999, 2)));
    assert_eq!(norm(ty, dec(10000, 2)), Err(ValueRule::DecimalPrecision));
    // 99.995 in a decimal(4, 2) is 100.00, past its precision.
    assert_eq!(norm(ty, dec(99_995, 3)), Err(ValueRule::DecimalPrecision));
    assert_eq!(norm(ty, dec(-99_995, 3)), Err(ValueRule::DecimalPrecision));
    assert_eq!(norm(ty, dec(99_994, 3)), Ok(dec(9999, 2)));
    // Scaling up past an i128.
    assert_eq!(
        norm(ty, dec(i128::MAX, 0)),
        Err(ValueRule::DecimalPrecision)
    );
    // 38 digits fit a decimal(38, s), and 10^38 does not.
    let wide = ColumnType::Decimal {
        precision: 38,
        scale: 10,
    };
    let largest = 10i128.pow(38) - 1;
    assert_eq!(norm(wide, dec(largest, 10)), Ok(dec(largest, 10)));
    assert_eq!(norm(wide, dec(-largest, 10)), Ok(dec(-largest, 10)));
    assert_eq!(
        norm(wide, dec(largest + 1, 10)),
        Err(ValueRule::DecimalPrecision)
    );
}

#[test]
fn floats_store_one_zero_and_one_nan() {
    let canonical_f32 = 0x7FC0_0000;
    for bits in [0x7FC0_0000u32, 0xFFC0_0001, 0x7F80_0001, 0xFFFF_FFFF] {
        let Ok(Value::F32(v)) = norm(ColumnType::F32, Value::F32(f32::from_bits(bits))) else {
            panic!("a NaN is valid");
        };
        assert_eq!(v.to_bits(), canonical_f32);
    }
    let Ok(Value::F32(zero)) = norm(ColumnType::F32, Value::F32(-0.0)) else {
        panic!()
    };
    assert_eq!(zero.to_bits(), 0);
    let canonical_f64 = 0x7FF8_0000_0000_0000;
    for bits in [
        0x7FF8_0000_0000_0000u64,
        0xFFF0_0000_0000_0001,
        0x7FF0_0000_0000_0001,
    ] {
        let Ok(Value::F64(v)) = norm(ColumnType::F64, Value::F64(f64::from_bits(bits))) else {
            panic!("a NaN is valid");
        };
        assert_eq!(v.to_bits(), canonical_f64);
    }
    let Ok(Value::F64(zero)) = norm(ColumnType::F64, Value::F64(-0.0)) else {
        panic!()
    };
    assert_eq!(zero.to_bits(), 0);
    for v in [
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MAX,
        f64::MIN_POSITIVE,
        5e-324,
    ] {
        assert_eq!(norm(ColumnType::F64, Value::F64(v)), Ok(Value::F64(v)));
    }
}

#[test]
fn integers_and_other_types_keep_their_values() {
    let cases = [
        (ColumnType::I8, Value::I8(i8::MIN)),
        (ColumnType::I16, Value::I16(i16::MAX)),
        (ColumnType::I32, Value::I32(i32::MIN)),
        (ColumnType::I64, Value::I64(i64::MAX)),
        (ColumnType::U8, Value::U8(u8::MAX)),
        (ColumnType::U16, Value::U16(0)),
        (ColumnType::U32, Value::U32(u32::MAX)),
        (ColumnType::U64, Value::U64(u64::MAX)),
        (ColumnType::Bool, Value::Bool(true)),
        (ColumnType::String, s("a\0b")),
        (ColumnType::String, s("")),
        (ColumnType::Bytes, b(&[])),
        (
            ColumnType::Uuid(UuidOrder::SqlServer),
            Value::Uuid(Uuid([0xAB; 16])),
        ),
        (ColumnType::Ulid, Value::Ulid(Ulid([7; 16]))),
    ];
    for (ty, value) in cases {
        assert_eq!(norm(ty, value.clone()), Ok(value));
    }
}

#[test]
fn rules_are_reported_in_the_order_of_e17() {
    // Null not allowed before wrong type; a nullable column takes the null.
    assert_eq!(
        norm(ColumnType::I32, Value::Null),
        Err(ValueRule::NullNotAllowed)
    );
    assert_eq!(
        normalize(&ColumnType::I32, true, Value::Null),
        Ok(Value::Null)
    );
    assert_eq!(
        norm(ColumnType::I32, Value::I64(1)),
        Err(ValueRule::WrongType)
    );
    assert_eq!(
        norm(
            ColumnType::Uuid(UuidOrder::Rfc9562),
            Value::Ulid(Ulid([0; 16]))
        ),
        Err(ValueRule::WrongType)
    );
    assert_eq!(
        norm(ColumnType::Date, Value::Timestamp(0)),
        Err(ValueRule::WrongType)
    );
    // Vector: dimension before NaN or infinity before a zero vector with cosine.
    let ty = vector(VectorElement::F32, 2, true);
    let mut wrong_len_with_nan = f32::NAN.to_le_bytes().to_vec();
    wrong_len_with_nan.extend(0f32.to_le_bytes());
    wrong_len_with_nan.extend(0f32.to_le_bytes());
    assert_eq!(
        norm(ty, vec_value(wrong_len_with_nan)),
        Err(ValueRule::VectorDimension)
    );
    let nan_and_zero = [f32::NAN, 0.0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    assert_eq!(
        norm(ty, vec_value(nan_and_zero)),
        Err(ValueRule::VectorNotFinite)
    );
    let zeros = [-0.0f32, 0.0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    assert_eq!(norm(ty, vec_value(zeros)), Err(ValueRule::ZeroVectorCosine));
}

#[test]
fn vectors_reject_nan_infinity_and_zero_with_cosine() {
    for element in [VectorElement::F32, VectorElement::F16] {
        let ty = vector(element, 2, false);
        let cases: [(&[u8], ValueRule); 3] = match element {
            VectorElement::F32 => [
                (&[0, 0, 0x80, 0x7F, 0, 0, 0, 0], ValueRule::VectorNotFinite),
                (&[0, 0, 0x80, 0xFF, 0, 0, 0, 0], ValueRule::VectorNotFinite),
                (&[0, 0, 0xC0, 0x7F, 0, 0, 0, 0], ValueRule::VectorNotFinite),
            ],
            _ => [
                (&[0x00, 0x7C, 0, 0], ValueRule::VectorNotFinite),
                (&[0x00, 0xFC, 0, 0], ValueRule::VectorNotFinite),
                (&[0x00, 0x7E, 0, 0], ValueRule::VectorNotFinite),
            ],
        };
        for (bytes, rule) in cases {
            assert_eq!(norm(ty, vec_value(bytes.to_vec())), Err(rule));
        }
    }
    // i8: the scale must be finite, and a zero scale or zero elements are a zero vector.
    let ty = vector(VectorElement::I8, 2, true);
    let with_scale = |elements: [u8; 2], scale: f32| {
        let mut v = elements.to_vec();
        v.extend(scale.to_le_bytes());
        vec_value(v)
    };
    assert_eq!(
        norm(ty, with_scale([1, 2], f32::INFINITY)),
        Err(ValueRule::VectorNotFinite)
    );
    assert_eq!(
        norm(ty, with_scale([1, 2], 0.0)),
        Err(ValueRule::ZeroVectorCosine)
    );
    assert_eq!(
        norm(ty, with_scale([0, 0], 1.0)),
        Err(ValueRule::ZeroVectorCosine)
    );
    assert!(norm(ty, with_scale([0, 0xFF], 0.5)).is_ok());
    // Without the cosine metric a zero vector is valid.
    let plain = vector(VectorElement::F32, 1, false);
    assert!(norm(plain, vec_value(vec![0; 4])).is_ok());
}

#[test]
fn negative_zero_in_a_vector_is_stored_as_positive_zero() {
    let f32s = |values: &[f32]| {
        values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>()
    };
    let ty = vector(VectorElement::F32, 3, false);
    let given = f32s(&[1.0, -0.0, -2.0]);
    let Ok(Value::Vector(stored)) = norm(ty, vec_value(given)) else {
        panic!()
    };
    assert_eq!(stored.as_ref(), f32s(&[1.0, 0.0, -2.0]).as_slice());
    // f16 -0 is 0x8000.
    let ty = vector(VectorElement::F16, 2, false);
    let Ok(Value::Vector(stored)) = norm(ty, vec_value(vec![0x00, 0x80, 0x00, 0xBC])) else {
        panic!()
    };
    assert_eq!(stored.as_ref(), &[0x00, 0x00, 0x00, 0xBC]);
    // The scale of an i8 vector.
    let ty = vector(VectorElement::I8, 1, false);
    let mut given = vec![5];
    given.extend((-0.0f32).to_le_bytes());
    let Ok(Value::Vector(stored)) = norm(ty, vec_value(given)) else {
        panic!()
    };
    assert_eq!(stored.as_ref(), &[5, 0, 0, 0, 0]);
    // A vector without -0 is not copied.
    let bytes = f32s(&[1.0, 2.0, 3.0]);
    let ty = vector(VectorElement::F32, 3, false);
    let Ok(Value::Vector(Cow::Borrowed(_))) = norm(ty, Value::Vector(Cow::Borrowed(&bytes))) else {
        panic!("borrowed")
    };
}

#[test]
fn f16_elements_are_ieee_binary16() {
    assert_eq!(f16_to_f64(0x3C00), 1.0);
    assert_eq!(f16_to_f64(0x3E00), 1.5);
    assert_eq!(f16_to_f64(0xC000), -2.0);
    assert_eq!(f16_to_f64(0x7BFF), 65504.0);
    assert_eq!(f16_to_f64(0x0001), 2f64.powi(-24));
    assert_eq!(f16_to_f64(0x0400), 2f64.powi(-14));
    assert_eq!(f16_to_f64(0x7C00), f64::INFINITY);
    assert!(f16_to_f64(0x7E00).is_nan());
}

#[test]
fn vector_norms() {
    let f32s = |values: &[f32]| {
        values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>()
    };
    let ty = VectorType {
        element: VectorElement::F32,
        dimension: 2,
        cosine: true,
    };
    assert_eq!(vector_norm(&ty, &f32s(&[3.0, 4.0])), 5.0);
    // Identical [1e20] and [1e-30] stay finite and nonzero in f64 (document 5, R3).
    let one = VectorType {
        element: VectorElement::F32,
        dimension: 1,
        cosine: true,
    };
    assert_eq!(vector_norm(&one, &f32s(&[1e20])), f64::from(1e20f32));
    assert!(vector_norm(&one, &f32s(&[1e-30])) > 0.0);
    // 40 elements: two full lanes and a tail of 8.
    let ty = VectorType {
        element: VectorElement::F32,
        dimension: 40,
        cosine: true,
    };
    assert_eq!(vector_norm(&ty, &f32s(&[2.0; 40])), 160f64.sqrt());
    let ty = VectorType {
        element: VectorElement::F16,
        dimension: 2,
        cosine: true,
    };
    assert_eq!(vector_norm(&ty, &[0x00, 0x42, 0x00, 0x44]), 5.0); // 3.0 and 4.0
    let ty = VectorType {
        element: VectorElement::I8,
        dimension: 2,
        cosine: true,
    };
    let mut i8s = vec![3, (-4i8) as u8];
    i8s.extend(0.5f32.to_le_bytes());
    assert_eq!(vector_norm(&ty, &i8s), 2.5);
}

/// The fixed-order kernel reduces the 16 lanes first and then adds each tail square on its own, in order
/// (document 5): `2^54 + 1` rounds back to `2^54` seven times, while a tail summed apart (`+ 7`) would not.
#[test]
fn vector_norm_adds_the_tail_after_the_lanes_in_order() {
    let mut f32s = 2f32.powi(27).to_le_bytes().to_vec();
    f32s.extend(
        [0f32; 15]
            .iter()
            .chain(&[1f32; 7])
            .flat_map(|v| v.to_le_bytes()),
    );
    let ty = VectorType {
        element: VectorElement::F32,
        dimension: 23,
        cosine: true,
    };
    assert_eq!(vector_norm(&ty, &f32s).to_bits(), 0x41a0_0000_0000_0000);
    // The same shape in f16: 2^15, fifteen zeros and seven 2^-12 (each square under half an ulp of 2^30).
    let mut f16s = 0x7800u16.to_le_bytes().to_vec();
    f16s.extend(
        [0u16; 15]
            .iter()
            .chain(&[0x0C00u16; 7])
            .flat_map(|v| v.to_le_bytes()),
    );
    let ty = VectorType {
        element: VectorElement::F16,
        dimension: 23,
        cosine: true,
    };
    assert_eq!(vector_norm(&ty, &f16s).to_bits(), 32768f64.to_bits());
}

#[test]
fn type_limits_of_a_schema() {
    for (element, dimension) in [
        (VectorElement::F32, 1),
        (VectorElement::F16, 4096),
        (VectorElement::I8, 4096),
        (VectorElement::Bit, 8),
        (VectorElement::Bit, 4096),
    ] {
        assert_eq!(check_type(&vector(element, dimension, false)), Ok(()));
    }
    for (element, dimension) in [
        (VectorElement::F32, 0),
        (VectorElement::F32, 4097),
        (VectorElement::Bit, 12),
        (VectorElement::Bit, 4104),
    ] {
        assert_eq!(
            check_type(&vector(element, dimension, false)),
            Err(SchemaRule::VectorOutOfLimits)
        );
    }
    assert_eq!(
        check_type(&vector(VectorElement::Bit, 8, true)),
        Err(SchemaRule::CosineOnBit)
    );
    // A vector out of limits comes before cosine on bit (E5).
    assert_eq!(
        check_type(&vector(VectorElement::Bit, 7, true)),
        Err(SchemaRule::VectorOutOfLimits)
    );
    assert_eq!(check_type(&vector(VectorElement::I8, 3, true)), Ok(()));
    for (precision, scale) in [(1, 0), (1, 1), (38, 0), (38, 38), (10, 2)] {
        assert_eq!(
            check_type(&ColumnType::Decimal { precision, scale }),
            Ok(())
        );
    }
    for (precision, scale) in [(0, 0), (39, 0), (5, 6)] {
        assert_eq!(
            check_type(&ColumnType::Decimal { precision, scale }),
            Err(SchemaRule::DecimalOutOfLimits)
        );
    }
    assert_eq!(check_type(&ColumnType::String), Ok(()));
}

#[test]
fn text_forms_for_error_data() {
    let uuid = Uuid([
        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd,
        0xef,
    ]);
    assert_eq!(
        Value::Uuid(uuid).to_string(),
        "01234567-89ab-cdef-0123-456789abcdef"
    );
    assert_eq!(
        Value::Ulid(Ulid([0xFF; 16])).to_string(),
        "7ZZZZZZZZZZZZZZZZZZZZZZZZZ"
    );
    assert_eq!(
        Value::Ulid(Ulid([0; 16])).to_string(),
        "00000000000000000000000000"
    );
    assert_eq!(s("a\"\0").to_string(), r#""a\"\0""#);
    assert_eq!(b(&[0, 0xAB]).to_string(), "0x00ab");
    assert_eq!(b(&[]).to_string(), "0x");
    assert_eq!(Value::F64(1.0).to_string(), "1.0");
    assert_eq!(Value::F32(f32::NAN).to_string(), "NaN");
    assert_eq!(Value::Null.to_string(), "null");
    assert_eq!(
        Value::Time(3_723_000_001_000).to_string(),
        "01:02:03.000001"
    );
    assert_eq!(key_text(&[Value::I32(-5), s("x")]), r#"(-5, "x")"#);
}

#[test]
fn vectors_at_their_limits_read_back_equal() {
    // Dimension 1 (8 for bit) and 4,096 of each element type, in a row.
    for element in [
        VectorElement::F32,
        VectorElement::F16,
        VectorElement::I8,
        VectorElement::Bit,
    ] {
        for dimension in [if element == VectorElement::Bit { 8 } else { 1 }, 4096] {
            let cosine = element != VectorElement::Bit;
            let ty = vector(element, dimension, cosine);
            let ColumnType::Vector(vt) = ty else {
                unreachable!()
            };
            let mut bytes: Vec<u8> = (0..vt.value_len()).map(|i| (i % 7 + 1) as u8).collect();
            if element == VectorElement::I8 {
                let n = bytes.len();
                bytes[n - 4..].copy_from_slice(&0.25f32.to_le_bytes());
            }
            let value = norm(ty, Value::Vector(Cow::Borrowed(&bytes))).unwrap();
            let layout = RowLayout::new(
                1,
                vec![key_column(1, ColumnType::I64), column(2, ty, false)],
            );
            let row_bytes = layout.encode(&[Value::I64(1), value.clone()]);
            let row = layout.row(&row_bytes).unwrap();
            assert_eq!(row.value(1), Ok(Some(value)));
            assert_eq!(row.len(), 3 + vt.stored_len());
            if cosine {
                let norm_bytes = &row_bytes[row_bytes.len() - 8..];
                assert_eq!(
                    f64::from_le_bytes(norm_bytes.try_into().unwrap()),
                    vector_norm(&vt, &bytes)
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Keys (document 3)

fn key1(ty: ColumnType, value: Value<'_>) -> Vec<u8> {
    encode_key(&[ty], &[value]).unwrap()
}

#[test]
fn key_examples_of_document_3() {
    assert_eq!(key1(ColumnType::I32, Value::I32(5)), [0x15, 0x05]);
    assert_eq!(key1(ColumnType::I32, Value::I32(-5)), [0x13, 0xFA]);
    assert_eq!(key1(ColumnType::I32, Value::I32(-256)), [0x12, 0xFE, 0xFF]);
    assert_eq!(key1(ColumnType::I32, Value::I32(0)), [0x14]);
    assert_eq!(key1(ColumnType::String, s("ab")), [0x02, 0x61, 0x62, 0x00]);
    assert_eq!(
        key1(ColumnType::String, s("a\0b")),
        [0x02, 0x61, 0x00, 0xFF, 0x62, 0x00]
    );
    assert_eq!(key1(ColumnType::String, s("")), [0x02, 0x00]);
    assert_eq!(key1(ColumnType::Bytes, b(&[])), [0x01, 0x00]);
    assert_eq!(
        key1(ColumnType::Bytes, b(&[0x41, 0, 0x42])),
        [0x01, 0x41, 0x00, 0xFF, 0x42, 0x00]
    );
    assert_eq!(key1(ColumnType::Bool, Value::Bool(false)), [0x26]);
    assert_eq!(key1(ColumnType::Bool, Value::Bool(true)), [0x27]);
    assert_eq!(key1(ColumnType::I16, Value::I16(255)), [0x15, 0xFF]);
    assert_eq!(key1(ColumnType::I16, Value::I16(256)), [0x16, 0x01, 0x00]);
    assert_eq!(key1(ColumnType::I16, Value::I16(-255)), [0x13, 0x00]);
    let mut min = vec![0x0C, 0x7F];
    min.extend([0xFF; 7]);
    assert_eq!(key1(ColumnType::I64, Value::I64(i64::MIN)), min);
    let mut max = vec![0x1C];
    max.extend([0xFF; 8]);
    assert_eq!(key1(ColumnType::U64, Value::U64(u64::MAX)), max);
    assert_eq!(
        key1(ColumnType::F32, Value::F32(1.0)),
        [0x20, 0xBF, 0x80, 0, 0]
    );
    assert_eq!(
        key1(ColumnType::F32, Value::F32(-1.0)),
        [0x20, 0x40, 0x7F, 0xFF, 0xFF]
    );
    assert_eq!(
        key1(ColumnType::F32, Value::F32(0.0)),
        [0x20, 0x80, 0, 0, 0]
    );
    assert_eq!(
        key1(ColumnType::F32, Value::F32(f32::from_bits(0x7FC0_0000))),
        [0x20, 0xFF, 0xC0, 0, 0]
    );
    assert_eq!(
        key1(ColumnType::F64, Value::F64(1.0)),
        [0x21, 0xBF, 0xF0, 0, 0, 0, 0, 0, 0]
    );
    // Temporal types are integers: days and microseconds.
    assert_eq!(key1(ColumnType::Date, Value::Date(-1)), [0x13, 0xFE]);
    assert_eq!(key1(ColumnType::Time, Value::Time(1000)), [0x15, 0x01]);
    assert_eq!(
        key1(ColumnType::Timestamp, Value::Timestamp(-256_000)),
        [0x12, 0xFE, 0xFF]
    );
    assert_eq!(
        key1(ColumnType::Duration, Value::Duration(5_000)),
        [0x15, 0x05]
    );
    // decimal: 40 and the scaled i128 with its top bit flipped, big-endian.
    let ty = ColumnType::Decimal {
        precision: 10,
        scale: 2,
    };
    let mut one = vec![0x40, 0x80];
    one.extend([0; 14]);
    one.push(0x7C);
    assert_eq!(key1(ty, dec(124, 2)), one);
    let mut minus_one = vec![0x40, 0x7F];
    minus_one.extend([0xFF; 15]);
    assert_eq!(key1(ty, dec(-1, 2)), minus_one);
    // uuid and ulid: 30 and 16 bytes; SQL Server's order permutes them.
    let r: [u8; 16] = std::array::from_fn(|i| i as u8);
    let mut rfc = vec![0x30];
    rfc.extend(r);
    assert_eq!(
        key1(ColumnType::Uuid(UuidOrder::Rfc9562), Value::Uuid(Uuid(r))),
        rfc
    );
    assert_eq!(key1(ColumnType::Ulid, Value::Ulid(Ulid(r))), rfc);
    assert_eq!(
        key1(ColumnType::Uuid(UuidOrder::SqlServer), Value::Uuid(Uuid(r))),
        [0x30, 10, 11, 12, 13, 14, 15, 8, 9, 7, 6, 5, 4, 3, 2, 1, 0]
    );
}

#[test]
fn key_of_several_columns_and_its_decoding() {
    let types = [
        ColumnType::String,
        ColumnType::I64,
        ColumnType::Uuid(UuidOrder::SqlServer),
        ColumnType::Bytes,
    ];
    let uuid = Uuid(std::array::from_fn(|i| (i * 17) as u8));
    let values = [
        s("a\0"),
        Value::I64(-7),
        Value::Uuid(uuid),
        b(&[0, 0xFF, 0]),
    ];
    let key = encode_key(&types, &values).unwrap();
    assert_eq!(key.len(), key_len(&types, &values));
    assert_eq!(&key[..5], &[0x02, 0x61, 0x00, 0xFF, 0x00]);
    let decoded = decode_key(&types, &key).unwrap();
    assert!(same(&decoded, &values));
}

#[test]
fn key_decoding_rejects_bytes_that_are_not_a_key() {
    let types = [ColumnType::I32, ColumnType::String];
    let good = encode_key(&types, &[Value::I32(300), s("é")]).unwrap();
    assert!(decode_key(&types, &good).is_some());
    for bad in [
        &good[..good.len() - 1],                          // no terminator
        &[0x15, 0x05, 0x02, 0xC3, 0x00],                  // invalid UTF-8
        &[0x16, 0x00, 0x05, 0x02, 0x00],                  // a non-minimal integer
        &[0x1D, 0x02, 0x00],                              // no such integer code
        &[0x26, 0x02, 0x00],                              // another type's code
        &[0x15, 0x05, 0x02, 0x00, 0x00],                  // a byte after the key
        &[0x17, 0x7F, 0xFF, 0xFF, 0xFF, 0x02, 0x00][..1], // truncated integer
    ] {
        assert_eq!(decode_key(&types, bad), None, "{bad:02x?}");
    }
    // An integer out of its column's range.
    assert_eq!(decode_key(&[ColumnType::I8], &[0x16, 0x01, 0x00]), None);
    assert_eq!(decode_key(&[ColumnType::U8], &[0x13, 0xFE]), None);
    assert_eq!(decode_key(&[ColumnType::Bool], &[0x28]), None);
    assert_eq!(decode_key(&[ColumnType::Time], &[0x13, 0xFE]), None);
}

#[test]
fn key_size_is_at_most_2048_bytes() {
    // A bytes key of 2,046 bytes without 00 encodes in exactly 2,048 bytes; one byte more is too long.
    let fits = vec![0x41u8; 2046];
    assert_eq!(
        encode_key(&[ColumnType::Bytes], &[b(&fits)]).map(|k| k.len()),
        Ok(MAX_KEY)
    );
    let long = vec![0x41u8; 2047];
    assert_eq!(
        encode_key(&[ColumnType::Bytes], &[b(&long)]),
        Err(KeyError::Size { received: 2049 })
    );
    // Escapes count: 1,023 zero bytes encode in 2 + 2,046 bytes.
    let zeros = vec![0u8; 1023];
    assert_eq!(
        encode_key(&[ColumnType::Bytes], &[b(&zeros)]).map(|k| k.len()),
        Ok(2048)
    );
    let zeros = vec![0u8; 1024];
    assert_eq!(key_len(&[ColumnType::Bytes], &[b(&zeros)]), 2050);
    assert_eq!(
        encode_key(&[ColumnType::Bytes], &[b(&zeros)]),
        Err(KeyError::Size { received: 2050 })
    );
}

#[test]
fn empty_text_and_empty_bytes_are_keys_before_the_others() {
    let types = [ColumnType::Bytes, ColumnType::String, ColumnType::I32];
    let keys: Vec<Vec<u8>> = [
        [b(&[]), s(""), Value::I32(1)],
        [b(&[]), s("\0"), Value::I32(0)],
        [b(&[]), s("a"), Value::I32(0)],
        [b(&[0]), s(""), Value::I32(0)],
        [b(&[0, 0]), s(""), Value::I32(0)],
        [b(&[0, 1]), s(""), Value::I32(0)],
        [b(&[1]), s(""), Value::I32(0)],
    ]
    .iter()
    .map(|v| encode_key(&types, v).unwrap())
    .collect();
    assert!(keys.windows(2).all(|w| w[0] < w[1]));
}

fn bounds_of(range: &KeyRange) -> (Bound<&[u8]>, Bound<&[u8]>) {
    range.bounds()
}

fn contains(range: &KeyRange, key: &[u8]) -> bool {
    let (lower, upper) = bounds_of(range);
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

#[test]
fn key_range_bounds_of_document_3() {
    let types = [ColumnType::String, ColumnType::I32];
    let p = vec![0x02, 0x61, 0x00]; // "a"
    let with = |tail: &[u8]| [p.as_slice(), tail].concat();
    let five = [0x15, 0x05];
    let r = key_range(
        &types,
        &[s("a")],
        Included(&Value::I32(5)),
        Included(&Value::I32(5)),
    )
    .unwrap();
    assert_eq!(r.lower, Included(with(&five)));
    assert_eq!(r.upper, Excluded(with(&[0x15, 0x05, 0xFF])));
    let r = key_range(
        &types,
        &[s("a")],
        Excluded(&Value::I32(5)),
        Excluded(&Value::I32(5)),
    )
    .unwrap();
    assert_eq!(r.lower, Included(with(&[0x15, 0x05, 0xFF])));
    assert_eq!(r.upper, Excluded(with(&five)));
    let r = key_range(&types, &[s("a")], Unbounded, Unbounded).unwrap();
    assert_eq!(r.lower, Included(p.clone()));
    assert_eq!(r.upper, Excluded(with(&[0xFF])));
    let r = key_range(&types, &[], Unbounded, Unbounded).unwrap();
    assert_eq!((r.lower, r.upper), (Unbounded, Unbounded));
    let r = key_range(&types, &[], Included(&s("a")), Unbounded).unwrap();
    assert_eq!((r.lower, r.upper), (Included(p.clone()), Unbounded));
    // The whole key: one row.
    let r = key_range(&types, &[s("a"), Value::I32(5)], Unbounded, Unbounded).unwrap();
    assert_eq!(r.lower, Included(with(&five)));
    assert_eq!(r.upper, Excluded(with(&[0x15, 0x05, 0xFF])));
    // An equality on "a" does not take "a\0", whose encoding starts with "a"'s.
    let a_nul = encode_key(&types, &[s("a\0"), Value::I32(0)]).unwrap();
    let r = key_range(&types, &[s("a")], Unbounded, Unbounded).unwrap();
    assert!(!contains(&r, &a_nul));
    assert!(contains(
        &r,
        &encode_key(&types, &[s("a"), Value::I32(i32::MAX)]).unwrap()
    ));
    // Bounds are normalized: -0 is +0, and NaN is a valid bound.
    let types = [ColumnType::F64];
    let r = key_range(
        &types,
        &[],
        Included(&Value::F64(-0.0)),
        Included(&Value::F64(f64::NAN)),
    )
    .unwrap();
    assert_eq!(r.lower, Included(vec![0x21, 0x80, 0, 0, 0, 0, 0, 0, 0]));
    assert_eq!(
        r.upper,
        Excluded(vec![0x21, 0xFF, 0xF8, 0, 0, 0, 0, 0, 0, 0xFF])
    );
}

#[test]
fn key_range_bounds_of_2049_bytes_become_2048() {
    let types = [ColumnType::Bytes];
    let value = vec![0x41u8; 2046];
    let key = encode_key(&types, &[b(&value)]).unwrap();
    assert_eq!(key.len(), MAX_KEY);
    // The whole key: [B, B ‖ FF) is [B, B].
    let r = key_range(&types, &[b(&value)], Unbounded, Unbounded).unwrap();
    assert_eq!(
        (r.lower, r.upper),
        (Included(key.clone()), Included(key.clone()))
    );
    // lo excluded: B ‖ FF as a start is B excluded; hi included: B ‖ FF as an end is B included.
    let r = key_range(&types, &[], Excluded(&b(&value)), Included(&b(&value))).unwrap();
    assert_eq!(
        (r.lower, r.upper),
        (Excluded(key.clone()), Included(key.clone()))
    );
    let r = key_range(&types, &[], Included(&b(&value)), Excluded(&b(&value))).unwrap();
    assert_eq!((r.lower, r.upper), (Included(key.clone()), Excluded(key)));
    // A value that does not fit fails with its size, even though the range is empty.
    let long = vec![0x41u8; 2047];
    let range = key_range(&types, &[], Excluded(&b(&long)), Excluded(&b(&value)));
    assert_eq!(range, Err(KeyError::Size { received: 2049 }));
    let range = key_range(&types, &[b(&long)], Unbounded, Unbounded);
    assert_eq!(range, Err(KeyError::Size { received: 2049 }));
}

#[test]
fn malformed_ranges_fail_with_their_rule() {
    let types = [ColumnType::I32, ColumnType::String];
    let value_error = |column, rule| Err(KeyError::Value { column, rule });
    // A value of another type, a null and a value out of its domain.
    assert_eq!(
        key_range(&types, &[Value::I64(1)], Unbounded, Unbounded),
        value_error(Some(0), ValueRule::WrongType)
    );
    assert_eq!(
        key_range(&types, &[Value::Null], Unbounded, Unbounded),
        value_error(Some(0), ValueRule::NullNotAllowed)
    );
    assert_eq!(
        key_range(
            &types,
            &[Value::I32(1)],
            Included(&Value::I32(1)),
            Unbounded
        ),
        value_error(Some(1), ValueRule::WrongType)
    );
    assert_eq!(
        key_range(&types, &[Value::I32(1)], Unbounded, Excluded(&Value::Null)),
        value_error(Some(1), ValueRule::NullNotAllowed)
    );
    let dates = [ColumnType::Date];
    assert_eq!(
        key_range(&dates, &[], Included(&Value::Date(DATE_MAX + 1)), Unbounded),
        value_error(Some(0), ValueRule::OutOfDomain)
    );
    // A prefix with more columns than the key, and a bound when the prefix gives every column.
    let full = [Value::I32(1), s("a")];
    assert_eq!(
        key_range(
            &types,
            &[Value::I32(1), s("a"), s("b")],
            Unbounded,
            Unbounded
        ),
        value_error(None, ValueRule::InvalidRange)
    );
    assert_eq!(
        key_range(&types, &full, Included(&s("a")), Unbounded),
        value_error(None, ValueRule::InvalidRange)
    );
    assert_eq!(
        key_range(&types, &full, Unbounded, Excluded(&s("a"))),
        value_error(None, ValueRule::InvalidRange)
    );
    // An inverted range is not an error.
    assert!(
        key_range(
            &types,
            &[],
            Included(&Value::I32(5)),
            Included(&Value::I32(1))
        )
        .is_ok()
    );
}

/// The uuids of `crates/krummi/tests/data/sqlserver-order/`, in RFC 9562 byte order, sorted with .NET's `SqlGuid`
/// by `tools/sqlguid-vectors` (AC17).
#[test]
fn sql_server_order_matches_sqlguid_vectors() {
    let text = include_str!("data/sqlserver-order/sqlguid-order.txt");
    let uuids: Vec<Uuid> = text
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let hex = line.trim();
            Uuid(std::array::from_fn(|i| {
                u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()
            }))
        })
        .collect();
    assert!(uuids.len() >= 1000, "{} vectors", uuids.len());
    let sql = [ColumnType::Uuid(UuidOrder::SqlServer)];
    let keys: Vec<Vec<u8>> = uuids
        .iter()
        .map(|u| encode_key(&sql, &[Value::Uuid(*u)]).unwrap())
        .collect();
    for (i, pair) in keys.windows(2).enumerate() {
        assert!(pair[0] < pair[1], "vectors {i} and {} out of order", i + 1);
    }
    // The model of the property test agrees with SqlGuid too.
    assert!(
        uuids
            .windows(2)
            .all(|w| sql_server_cmp(&w[0].0, &w[1].0) == Ordering::Less)
    );
    // The RFC 9562 order differs on these vectors, so the test tells the two apart.
    let rfc = [ColumnType::Uuid(UuidOrder::Rfc9562)];
    let rfc_keys: Vec<Vec<u8>> = uuids
        .iter()
        .map(|u| encode_key(&rfc, &[Value::Uuid(*u)]).unwrap())
        .collect();
    assert!(rfc_keys.windows(2).any(|w| w[0] > w[1]));
    for (u, key) in uuids.iter().zip(&keys) {
        assert!(same(&decode_key(&sql, key).unwrap(), &[Value::Uuid(*u)]));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Property test against a reference model of N5 (AC-N1)

/// SQL Server's order of a `uniqueidentifier` given in RFC 9562 bytes, written as `SqlGuid` compares: the bytes of
/// `Guid.ToByteArray()` (the first three fields little-endian) in the order 10-15, 8-9, 6-7, 4-5, 0-3.
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

/// N5 on two normalized values of a column of type `ty`, by their type and never by their encoding.
fn model_cmp(ty: &ColumnType, a: &Value<'_>, b: &Value<'_>) -> Ordering {
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
        (Value::I16(a), Value::I16(b)) => a.cmp(b),
        (Value::I32(a), Value::I32(b)) => a.cmp(b),
        (Value::I64(a), Value::I64(b)) => a.cmp(b),
        (Value::U8(a), Value::U8(b)) => a.cmp(b),
        (Value::U16(a), Value::U16(b)) => a.cmp(b),
        (Value::U32(a), Value::U32(b)) => a.cmp(b),
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
        _ => panic!("values of different types"),
    }
}

fn model_cmp_key(types: &[ColumnType], a: &[Value<'_>], b: &[Value<'_>]) -> Ordering {
    types
        .iter()
        .zip(a.iter().zip(b))
        .map(|(ty, (a, b))| model_cmp(ty, a, b))
        .find(|o| o.is_ne())
        .unwrap_or(Ordering::Equal)
}

const ORDERABLE: [ColumnType; 21] = [
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
        ColumnType::I16 => {
            Value::I16(rng.pick(&[i16::MIN, -256, -255, 0, 255, 256, i16::MAX, r1 as i16]))
        }
        ColumnType::I32 => Value::I32(rng.pick(&[i32::MIN, -1, 0, 1, i32::MAX, r1 as i32])),
        ColumnType::I64 => Value::I64(rng.pick(&[
            i64::MIN,
            i64::MIN + 1,
            -1,
            0,
            i64::MAX,
            r1 as i64,
            r2 as i64 >> 30,
        ])),
        ColumnType::U8 => Value::U8(rng.pick(&[0, 1, u8::MAX, r1 as u8])),
        ColumnType::U16 => Value::U16(rng.pick(&[0, u16::MAX, r1 as u16])),
        ColumnType::U32 => Value::U32(rng.pick(&[0, u32::MAX, r1 as u32])),
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
            0x3F80_0000,
            0xBF80_0000,
            r1 as u32,
            r2 as u32,
        ]))),
        ColumnType::F64 => Value::F64(f64::from_bits(rng.pick(&[
            0x8000_0000_0000_0000,
            0,
            0x7FF8_0000_0000_0000,
            0xFFF8_0000_0000_0001,
            0x7FF0_0000_0000_0001,
            0x7FF0_0000_0000_0000,
            0xFFF0_0000_0000_0000,
            1,
            0x8000_0000_0000_0001,
            r1,
            r2,
        ]))),
        ColumnType::Decimal { .. } => {
            let max = 10i128.pow(38) - 1;
            let random = i128::from(r1 as i64) * i128::from(r2 >> 1);
            dec(rng.pick(&[-max, max, 0, 1, -1, random, random / 7]), 3)
        }
        ColumnType::String => {
            let parts = ["", "\0", "a", "b", "é", "\u{FFFF}", "\u{10FFFF}", "\0\0"];
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
        ColumnType::Date => {
            let random = DATE_MIN + (r1 % (DATE_MAX - DATE_MIN) as u64) as i32;
            Value::Date(rng.pick(&[DATE_MIN, DATE_MAX, 0, -1, random]))
        }
        ColumnType::Time => {
            let max = TIME_MAX * 1000;
            Value::Time(rng.pick(&[0, max, -500, max + 499, 1_000, (r1 % max as u64) as i64]))
        }
        ColumnType::Timestamp => {
            let (min, max) = (
                i128::from(TIMESTAMP_MIN) * NANOS,
                i128::from(TIMESTAMP_MAX) * NANOS,
            );
            let span = (max - min) as u128;
            let random = min + (u128::from(r1) * u128::from(r2) % span) as i128;
            Value::Timestamp(rng.pick(&[min, max, 0, -1, -500, min + 500, random]))
        }
        ColumnType::Duration => {
            let max = i128::from(DURATION_MAX) * NANOS;
            Value::Duration(rng.pick(&[-max, max, 0, -1_000, 1_000, i128::from(r1 as i64)]))
        }
        ColumnType::Uuid(_) | ColumnType::Ulid => {
            // Each group of bytes that SQL Server compares as a unit is all 00, all FF, all 01 or random, so that
            // many values tie on the groups compared first and the later groups decide.
            let mut bytes = [0u8; 16];
            for group in [0..4, 4..6, 6..8, 8..10, 10..16] {
                let fill = rng.below(4);
                for byte in &mut bytes[group] {
                    *byte = match fill {
                        0 => 0x00,
                        1 => 0xFF,
                        2 => 0x01,
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
        ColumnType::Vector(_) => unreachable!("vectors are not orderable"),
    }
}

/// Pads the last `string` or `bytes` column of `values` so that the key has `target` bytes, when it has one.
fn pad_to(types: &[ColumnType], values: &mut [Value<'static>], target: usize) {
    let Some(i) = types
        .iter()
        .rposition(|t| matches!(t, ColumnType::String | ColumnType::Bytes))
    else {
        return;
    };
    let len = key_len(types, values);
    if len >= target {
        return;
    }
    let extra = target - len;
    match &mut values[i] {
        Value::String(text) => text.to_mut().push_str(&"a".repeat(extra)),
        Value::Bytes(bytes) => bytes.to_mut().extend(std::iter::repeat_n(0x41, extra)),
        _ => unreachable!(),
    }
}

/// Whether the model puts `row` inside the range of `prefix` and the bounds on the next column.
fn model_in_range(
    types: &[ColumnType],
    row: &[Value<'_>],
    prefix: &[Value<'_>],
    lower: &Bound<Value<'_>>,
    upper: &Bound<Value<'_>>,
) -> bool {
    let k = prefix.len();
    if model_cmp_key(&types[..k], &row[..k], prefix).is_ne() {
        return false;
    }
    if k == types.len() {
        return true;
    }
    let cmp = |bound: &Value<'_>| model_cmp(&types[k], &row[k], bound);
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

fn normalized(ty: &ColumnType, value: Value<'static>) -> Value<'static> {
    normalize(ty, false, value).unwrap()
}

#[test]
fn keys_order_decode_and_ranges_follow_the_model_of_n5() {
    let mut rng = Rng(0x007A_B1E5);
    let mut checked_ranges = 0;
    let mut at_limit = 0;
    for _ in 0..300 {
        let columns = 1 + rng.below(4) as usize;
        let types: Vec<ColumnType> = (0..columns).map(|_| rng.pick(&ORDERABLE)).collect();
        let mut rows: Vec<Vec<Value<'static>>> = Vec::new();
        let mut keys: Vec<Vec<u8>> = Vec::new();
        for _ in 0..40 {
            let mut row: Vec<Value<'static>> = types
                .iter()
                .map(|ty| normalized(ty, random_value(&mut rng, ty)))
                .collect();
            if rng.chance(4) {
                pad_to(
                    &types,
                    &mut row,
                    rng.pick(&[MAX_KEY - 1, MAX_KEY, MAX_KEY + 1]),
                );
            }
            let len = key_len(&types, &row);
            match encode_key(&types, &row) {
                Err(error) => {
                    assert!(len > MAX_KEY);
                    assert_eq!(error, KeyError::Size { received: len });
                }
                Ok(key) => {
                    assert_eq!(key.len(), len);
                    at_limit += usize::from(len == MAX_KEY);
                    // Decoding gives back the normalized values.
                    let decoded = decode_key(&types, &key).expect("a key decodes");
                    assert!(same(&decoded, &row), "{decoded:?} {row:?}");
                    rows.push(row);
                    keys.push(key);
                }
            }
        }
        // The byte order of the keys is the model's order, equal keys included (duplicates: -0 and +0, NaNs).
        for i in 0..rows.len() {
            for j in 0..rows.len() {
                let model = model_cmp_key(&types, &rows[i], &rows[j]);
                assert_eq!(
                    keys[i].cmp(&keys[j]),
                    model,
                    "{:?} {:?} of {types:?}",
                    rows[i],
                    rows[j]
                );
            }
        }
        // Ranges: a prefix of k columns taken from a row (or random), bounds on column k included, excluded or
        // absent, from rows or random values.
        for _ in 0..20 {
            let k = rng.below(columns as u64 + 1) as usize;
            let source: Vec<Value<'static>> = if rows.is_empty() || rng.chance(5) {
                types
                    .iter()
                    .map(|ty| normalized(ty, random_value(&mut rng, ty)))
                    .collect()
            } else {
                rows[rng.below(rows.len() as u64) as usize].clone()
            };
            let prefix = &source[..k];
            let bound = |rng: &mut Rng| -> Bound<Value<'static>> {
                if k == columns {
                    return Unbounded;
                }
                let value = if !rows.is_empty() && rng.chance(2) {
                    rows[rng.below(rows.len() as u64) as usize][k].clone()
                } else {
                    normalized(&types[k], random_value(rng, &types[k]))
                };
                match rng.below(3) {
                    0 => Included(value),
                    1 => Excluded(value),
                    _ => Unbounded,
                }
            };
            let (lower, upper) = (bound(&mut rng), bound(&mut rng));
            let range = key_range(&types, prefix, lower.as_ref(), upper.as_ref());
            let prefix_len = key_len(&types[..k], prefix);
            let bound_len = |b: &Bound<Value<'_>>| match b {
                Included(v) | Excluded(v) => {
                    prefix_len + key_len(&types[k..=k], std::slice::from_ref(v))
                }
                Unbounded => prefix_len,
            };
            let longest = prefix_len.max(bound_len(&lower)).max(bound_len(&upper));
            let range = match range {
                Err(KeyError::Size { received }) => {
                    assert!(longest > MAX_KEY && received > MAX_KEY);
                    continue;
                }
                Err(error) => panic!("{error:?}"),
                Ok(range) => {
                    assert!(longest <= MAX_KEY);
                    range
                }
            };
            let (lo, hi) = range.bounds();
            for b in [lo, hi] {
                if let Included(b) | Excluded(b) = b {
                    assert!((1..=MAX_KEY).contains(&b.len()));
                }
            }
            for (row, key) in rows.iter().zip(&keys) {
                let model = model_in_range(&types, row, prefix, &lower, &upper);
                assert_eq!(
                    contains(&range, key),
                    model,
                    "{row:?} in {prefix:?} {lower:?} {upper:?}"
                );
            }
            checked_ranges += 1;
        }
    }
    assert!(checked_ranges > 4000, "{checked_ranges} ranges");
    assert!(at_limit > 50, "{at_limit} keys of 2,048 bytes");
}

// ---------------------------------------------------------------------------------------------------------------
// Rows (document 1)

fn column(id: u64, ty: ColumnType, nullable: bool) -> RowColumn {
    RowColumn {
        id,
        ty,
        nullable,
        key: false,
    }
}

fn key_column(id: u64, ty: ColumnType) -> RowColumn {
    RowColumn {
        id,
        ty,
        nullable: false,
        key: true,
    }
}

#[test]
fn row_header_bitmap_fixed_section_and_offsets() {
    let layout = RowLayout::new(
        7,
        vec![
            key_column(10, ColumnType::U64),
            column(11, ColumnType::I16, true),
            column(12, ColumnType::I64, false),
            column(13, ColumnType::String, true),
            column(14, ColumnType::I16, false),
            column(15, ColumnType::Bytes, false),
            key_column(16, ColumnType::String),
        ],
    );
    let values = [
        Value::U64(1),
        Value::Null,
        Value::I64(-2),
        s("xy"),
        Value::I16(3),
        b(&[]),
        s("k"),
    ];
    let row = layout.encode(&values);
    #[rustfmt::skip]
    let expected = [
        0x07, 0x00,                                     // schema version 7
        0x00,                                           // flags: offsets of one byte
        0b01,                                           // null bitmap: the first nullable column is null
        0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // i64 first (widest)
        0x00, 0x00,                                     // the null i16 keeps its slot
        0x03, 0x00,                                     // the other i16, later in the version
        0x02, 0x02,                                     // end offsets of the first two variable columns
        b'x', b'y', b'k',                               // variable section; the empty bytes take nothing
    ];
    assert_eq!(row, expected);
    assert_eq!(layout.encoded_len(&values), row.len());
    assert_eq!(row_version(&row), Ok(7));
    let read = layout.row(&row).unwrap();
    assert_eq!(read.len(), expected.len());
    // The fixed-width key column is not in the row; the string key column is.
    assert_eq!(read.value(0), Ok(None));
    let expected_values = [
        Value::Null,
        Value::I64(-2),
        s("xy"),
        Value::I16(3),
        b(&[]),
        s("k"),
    ];
    for (i, expected) in expected_values.iter().enumerate() {
        assert_eq!(read.value(i + 1), Ok(Some(expected.clone())));
    }
    // Text and bytes borrow from the row.
    let Ok(Some(Value::String(Cow::Borrowed(text)))) = read.value(3) else {
        panic!()
    };
    assert!(std::ptr::eq(text.as_ptr(), row[18..].as_ptr()));
    assert_eq!(layout.position(13), Some(3));
    assert_eq!(layout.position(99), None);
}

#[test]
fn row_round_trip_of_every_type() {
    let f32s = |values: &[f32]| {
        values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>()
    };
    let types_and_values: Vec<(ColumnType, Value<'static>)> = vec![
        (ColumnType::Bool, Value::Bool(true)),
        (ColumnType::I8, Value::I8(-8)),
        (ColumnType::I16, Value::I16(-16)),
        (ColumnType::I32, Value::I32(-32)),
        (ColumnType::I64, Value::I64(i64::MIN)),
        (ColumnType::U8, Value::U8(8)),
        (ColumnType::U16, Value::U16(16)),
        (ColumnType::U32, Value::U32(32)),
        (ColumnType::U64, Value::U64(u64::MAX)),
        (ColumnType::F32, Value::F32(-1.5)),
        (ColumnType::F64, Value::F64(f64::INFINITY)),
        (
            ColumnType::Decimal {
                precision: 38,
                scale: 4,
            },
            dec(-(10i128.pow(38) - 1), 4),
        ),
        (ColumnType::String, s("héllo\0")),
        (ColumnType::Bytes, b(&[0, 1, 0xFF])),
        (ColumnType::Date, Value::Date(DATE_MIN)),
        (ColumnType::Time, Value::Time(TIME_MAX * 1000)),
        (
            ColumnType::Timestamp,
            Value::Timestamp(i128::from(TIMESTAMP_MAX) * NANOS),
        ),
        (
            ColumnType::Duration,
            Value::Duration(-i128::from(DURATION_MAX) * NANOS),
        ),
        (
            ColumnType::Uuid(UuidOrder::SqlServer),
            Value::Uuid(Uuid(std::array::from_fn(|i| i as u8))),
        ),
        (ColumnType::Ulid, Value::Ulid(Ulid([0xEE; 16]))),
        (
            vector(VectorElement::F32, 2, true),
            vec_value(f32s(&[1.0, -2.0])),
        ),
        (
            vector(VectorElement::F16, 1, false),
            vec_value(vec![0x00, 0x3E]),
        ),
        (
            vector(VectorElement::I8, 2, false),
            vec_value([vec![1, 0xFF], 0.5f32.to_le_bytes().to_vec()].concat()),
        ),
        (
            vector(VectorElement::Bit, 16, false),
            vec_value(vec![0x01, 0x80]),
        ),
    ];
    // Each type as not null and as nullable, set and null.
    for nullable in [false, true] {
        let columns: Vec<RowColumn> = types_and_values
            .iter()
            .enumerate()
            .map(|(i, (ty, _))| column(i as u64, *ty, nullable))
            .collect();
        let layout = RowLayout::new(2, columns);
        let values: Vec<Value<'static>> = types_and_values.iter().map(|(_, v)| v.clone()).collect();
        let row = layout.encode(&values);
        assert_eq!(row.len(), layout.encoded_len(&values));
        let read = layout.row(&row).unwrap();
        for (i, value) in values.iter().enumerate() {
            assert_eq!(read.value(i), Ok(Some(value.clone())), "column {i}");
        }
        if nullable {
            let nulls = vec![Value::Null; values.len()];
            let row = layout.encode(&nulls);
            let read = layout.row(&row).unwrap();
            for i in 0..values.len() {
                assert_eq!(read.value(i), Ok(Some(Value::Null)));
            }
            // A null takes its fixed slot and nothing in the variable section.
            let fixed: usize = types_and_values
                .iter()
                .filter_map(|(t, _)| t.fixed_width())
                .sum();
            assert_eq!(row.len(), 3 + values.len().div_ceil(8) + fixed + 5);
        }
    }
}

#[test]
fn bit_vectors_f16_and_text_read_back_as_given() {
    // A bit vector with one bit at each position of a byte: element i is bit i mod 8 of byte i div 8 (E7).
    let ty = vector(VectorElement::Bit, 16, false);
    let layout = RowLayout::new(
        1,
        vec![key_column(1, ColumnType::I32), column(2, ty, false)],
    );
    for position in 0..16usize {
        let mut bytes = vec![0u8; 2];
        bytes[position / 8] = 1 << (position % 8);
        let value = norm(ty, vec_value(bytes.clone())).unwrap();
        let row = layout.encode(&[Value::I32(0), value]);
        assert_eq!(&row[3..], bytes.as_slice());
        let Ok(Some(Value::Vector(read))) = layout.row(&row).unwrap().value(1) else {
            panic!()
        };
        let bits: Vec<usize> = (0..16)
            .filter(|&i| (read[i / 8] >> (i % 8)) & 1 == 1)
            .collect();
        assert_eq!(bits, [position]);
    }
    // A known f16: 1.5 is 0x3E00, little-endian; -0 reads +0.
    let ty = vector(VectorElement::F16, 2, false);
    let layout = RowLayout::new(1, vec![column(1, ty, false)]);
    let value = norm(ty, vec_value(vec![0x00, 0x3E, 0x00, 0x80])).unwrap();
    let row = layout.encode(&[value]);
    let Ok(Some(Value::Vector(read))) = layout.row(&row).unwrap().value(0) else {
        panic!()
    };
    assert_eq!(read.as_ref(), &[0x00, 0x3E, 0x00, 0x00]);
    assert_eq!(f16_to_f64(u16::from_le_bytes([read[0], read[1]])), 1.5);
    // Text with U+0000 and empty text read back equal.
    let layout = RowLayout::new(
        1,
        vec![
            column(1, ColumnType::String, false),
            column(2, ColumnType::String, false),
        ],
    );
    let row = layout.encode(&[s("a\0b\0"), s("")]);
    let read = layout.row(&row).unwrap();
    assert_eq!(read.value(0), Ok(Some(s("a\0b\0"))));
    assert_eq!(read.value(1), Ok(Some(s(""))));
}

#[test]
fn offset_width_is_the_smallest_that_fits_the_variable_section() {
    let layout = RowLayout::new(
        1,
        vec![
            column(1, ColumnType::Bytes, false),
            column(2, ColumnType::Bytes, false),
        ],
    );
    for (total, flags, width) in [
        (0, 0, 1),
        (255, 0, 1),
        (256, 1, 2),
        (65_535, 1, 2),
        (65_536, 2, 4),
        (300_000, 2, 4),
    ] {
        let data = vec![0x5Au8; total];
        let (first, second) = data.split_at(total / 3);
        let values = [b(first), b(second)];
        let row = layout.encode(&values);
        assert_eq!(row[2], flags, "total {total}");
        assert_eq!(row.len(), 3 + width + total);
        assert_eq!(layout.encoded_len(&values), row.len());
        let read = layout.row(&row).unwrap();
        assert_eq!(read.value(0), Ok(Some(b(first))));
        assert_eq!(read.value(1), Ok(Some(b(second))));
    }
}

#[test]
fn a_row_of_an_older_version_reads_with_the_newer_columns() {
    // Version 1: id (key), x, gone. Version 3: id, x, an added nullable column, an added column with a default;
    // `gone` was dropped.
    let v1 = RowLayout::new(
        1,
        vec![
            key_column(100, ColumnType::I64),
            column(101, ColumnType::I32, false),
            column(102, ColumnType::String, true),
        ],
    );
    let v3_columns: [(u64, Value<'static>); 4] = [
        (100, Value::Null),
        (101, Value::Null),
        (103, Value::Null),
        (104, s("default")),
    ];
    let row = v1.encode(&[Value::I64(5), Value::I32(7), s("old")]);
    assert_eq!(row_version(&row), Ok(1));
    let read = v1.row(&row).unwrap();
    let values: Vec<Option<Value<'_>>> = v3_columns
        .iter()
        .map(|(id, default)| read.value_by_id(*id, default).unwrap())
        .collect();
    assert_eq!(
        values,
        [
            None,
            Some(Value::I32(7)),
            Some(Value::Null),
            Some(s("default"))
        ]
    );
    // A layout reads only rows of its own version.
    let v3 = RowLayout::new(
        3,
        vec![
            key_column(100, ColumnType::I64),
            column(101, ColumnType::I32, false),
        ],
    );
    assert_eq!(v3.row(&row).err(), Some(RowDamaged));
}

#[test]
fn a_row_of_256_mib_is_measured_in_its_stored_form() {
    // One variable column: 3 bytes of header and the value; one allocation for both measurements.
    let layout = RowLayout::new(
        1,
        vec![
            key_column(1, ColumnType::I64),
            column(2, ColumnType::Bytes, false),
        ],
    );
    let data = vec![0u8; MAX_ROW - 2];
    assert_eq!(
        layout.encoded_len(&[Value::I64(1), b(&data[..MAX_ROW - 3])]),
        MAX_ROW
    );
    assert_eq!(layout.encoded_len(&[Value::I64(1), b(&data)]), MAX_ROW + 1);
    // Two variable columns past 65,535 bytes take offsets of 4 bytes.
    let layout = RowLayout::new(
        1,
        vec![
            column(1, ColumnType::Bytes, false),
            column(2, ColumnType::String, true),
        ],
    );
    assert_eq!(
        layout.encoded_len(&[b(&data[..MAX_ROW - 8]), Value::Null]),
        MAX_ROW
    );
    assert_eq!(
        layout.encoded_len(&[b(&data[..MAX_ROW - 9]), s("ab")]),
        MAX_ROW + 1
    );
}

#[test]
fn rows_that_do_not_fit_their_layout_are_damaged() {
    let layout = RowLayout::new(
        4,
        vec![
            column(1, ColumnType::Bool, false),
            column(2, ColumnType::String, true),
            column(3, vector(VectorElement::F32, 1, false), true),
            column(4, ColumnType::Bytes, false),
        ],
    );
    let good = layout.encode(&[
        Value::Bool(true),
        s("ab"),
        vec_value(vec![0, 0, 0x80, 0x3F]),
        b(&[9]),
    ]);
    let read = layout.row(&good).unwrap();
    assert_eq!(read.value(0), Ok(Some(Value::Bool(true))));
    assert_eq!(row_version(&good[..1]), Err(RowDamaged));
    // The header: too short, another version, the compressed bit, reserved bits, an unknown offset width.
    for bad in [&good[..2], &[5, 0, 0, 0, 1, 2, 6, 0][..]] {
        assert_eq!(layout.row(bad).err(), Some(RowDamaged));
    }
    for flags in [0b100, 0b1000, 0x80, 3] {
        let mut bad = good.clone();
        bad[2] = flags;
        assert_eq!(layout.row(&bad).err(), Some(RowDamaged), "flags {flags:#b}");
    }
    // Shorter than its fixed part and offsets.
    assert_eq!(layout.row(&good[..5]).err(), Some(RowDamaged));
    // Each damaged column is found when it is read, and the others still read.
    let offsets = 3 + 1 + 1; // header, bitmap, the bool
    let damaged = |change: &dyn Fn(&mut Vec<u8>)| {
        let mut bad = good.clone();
        change(&mut bad);
        bad
    };
    let bad_bool = damaged(&|r| r[4] = 2);
    assert_eq!(layout.row(&bad_bool).unwrap().value(0), Err(RowDamaged));
    let bad_utf8 = damaged(&|r| r[offsets + 2] = 0xC3);
    assert_eq!(layout.row(&bad_utf8).unwrap().value(1), Err(RowDamaged));
    assert_eq!(
        layout.row(&bad_utf8).unwrap().value(0),
        Ok(Some(Value::Bool(true)))
    );
    let decreasing = damaged(&|r| r[offsets + 1] = 1);
    assert_eq!(layout.row(&decreasing).unwrap().value(2), Err(RowDamaged));
    let past_end = damaged(&|r| r[offsets + 1] = 200);
    assert_eq!(layout.row(&past_end).unwrap().value(3), Err(RowDamaged));
    let short_vector = damaged(&|r| r[offsets] = 3);
    assert_eq!(layout.row(&short_vector).unwrap().value(2), Err(RowDamaged));
    let null_with_bytes = damaged(&|r| r[3] |= 0b01);
    assert_eq!(
        layout.row(&null_with_bytes).unwrap().value(1),
        Err(RowDamaged)
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Fixes of the RDD review of T3 (R3-row-encode-silent-type-mismatch, R3-row-decode-temporal-domain,
// R3-decode-key-decimal-precision, R3-key-len-zip-truncation, R2-decode-fallthrough)

/// A value of another type than its fixed column is not truncated into its slot: encoding panics.
#[test]
#[should_panic(expected = "of another type than its column")]
fn row_encode_refuses_a_value_of_another_type() {
    let layout = RowLayout::new(1, vec![column(1, ColumnType::I8, false)]);
    layout.encode(&[Value::I64(300)]);
}

/// A null in a column that does not allow nulls is never written as zeros: encoding panics.
#[test]
#[should_panic(expected = "null in a column that does not allow nulls")]
fn row_encode_refuses_a_null_in_a_not_null_column() {
    let layout = RowLayout::new(1, vec![column(1, ColumnType::I32, false)]);
    layout.encode(&[Value::Null]);
}

/// A vector of another dimension than its column is refused too.
#[test]
#[should_panic(expected = "of another type than its column")]
fn row_encode_refuses_a_vector_of_another_dimension() {
    let layout = RowLayout::new(
        1,
        vec![column(1, vector(VectorElement::F32, 2, false), false)],
    );
    layout.encode(&[vec_value(vec![0; 4])]);
}

/// A stored temporal value outside its domain, or a decimal over its precision, reads as damage.
#[test]
fn rows_with_values_outside_their_domains_are_damaged() {
    let cases: [(ColumnType, Value<'static>, Vec<u8>); 6] = [
        (
            ColumnType::Date,
            Value::Date(0),
            (DATE_MAX + 1).to_le_bytes().to_vec(),
        ),
        (
            ColumnType::Date,
            Value::Date(0),
            (DATE_MIN - 1).to_le_bytes().to_vec(),
        ),
        (
            ColumnType::Time,
            Value::Time(0),
            (TIME_MAX + 1).to_le_bytes().to_vec(),
        ),
        (
            ColumnType::Timestamp,
            Value::Timestamp(0),
            (TIMESTAMP_MIN - 1).to_le_bytes().to_vec(),
        ),
        (
            ColumnType::Duration,
            Value::Duration(0),
            (DURATION_MAX + 1).to_le_bytes().to_vec(),
        ),
        (
            ColumnType::Decimal {
                precision: 4,
                scale: 2,
            },
            dec(0, 2),
            10_000i128.to_le_bytes().to_vec(),
        ),
    ];
    for (ty, value, stored) in cases {
        let layout = RowLayout::new(1, vec![column(1, ty, false)]);
        let mut row = layout.encode(&[value]);
        let at = row.len() - stored.len();
        assert_eq!(
            layout.row(&row).unwrap().value(0).map(|v| v.is_some()),
            Ok(true),
            "{ty:?}"
        );
        row[at..].copy_from_slice(&stored);
        assert_eq!(
            layout.row(&row).unwrap().value(0),
            Err(RowDamaged),
            "{ty:?}"
        );
    }
    // The ends of each domain still read.
    let layout = RowLayout::new(1, vec![column(1, ColumnType::Duration, false)]);
    let row = layout.encode(&[Value::Duration(-i128::from(DURATION_MAX) * NANOS)]);
    assert!(layout.row(&row).unwrap().value(0).is_ok());
}

/// A key whose decimal does not fit the column's precision, or whose float is `-0` or a NaN other than the canonical
/// one, is not a key that Krummi writes: decoding refuses it.
#[test]
fn key_decoding_rejects_decimals_over_precision_and_non_canonical_floats() {
    let decimal = ColumnType::Decimal {
        precision: 4,
        scale: 2,
    };
    let fits = encode_key(&[decimal], &[dec(9_999, 2)]).unwrap();
    assert!(decode_key(&[decimal], &fits).is_some());
    let wide = ColumnType::Decimal {
        precision: 5,
        scale: 2,
    };
    let over = encode_key(&[wide], &[dec(10_000, 2)]).unwrap();
    assert_eq!(decode_key(&[decimal], &over), None);

    for bits in [0x8000_0000u32, 0x7FC0_0001, 0xFFC0_0000] {
        let key = encode_key(&[ColumnType::F32], &[Value::F32(f32::from_bits(bits))]).unwrap();
        assert_eq!(decode_key(&[ColumnType::F32], &key), None, "{bits:#x}");
    }
    for bits in [0x8000_0000_0000_0000u64, 0x7FF8_0000_0000_0001] {
        let key = encode_key(&[ColumnType::F64], &[Value::F64(f64::from_bits(bits))]).unwrap();
        assert_eq!(decode_key(&[ColumnType::F64], &key), None, "{bits:#x}");
    }
    let nan = encode_key(&[ColumnType::F64], &[Value::F64(f64::NAN)]).unwrap();
    assert!(decode_key(&[ColumnType::F64], &nan).is_some());
}

/// A float column refuses the element of the other float width, and an integer column a float's code.
#[test]
fn key_decoding_rejects_the_code_of_another_type() {
    let f64_key = encode_key(&[ColumnType::F64], &[Value::F64(1.0)]).unwrap();
    assert_eq!(decode_key(&[ColumnType::F32], &f64_key), None);
    assert_eq!(decode_key(&[ColumnType::I64], &f64_key), None);
    let int_key = encode_key(&[ColumnType::I64], &[Value::I64(1)]).unwrap();
    assert_eq!(decode_key(&[ColumnType::F64], &int_key), None);
    assert_eq!(decode_key(&[ColumnType::Bool], &int_key), None);
}

#[test]
#[should_panic(expected = "one value per primary key column")]
fn key_len_refuses_fewer_values_than_columns() {
    key_len(&[ColumnType::I64, ColumnType::I64], &[Value::I64(1)]);
}

#[test]
#[should_panic(expected = "one value per primary key column")]
fn encode_key_refuses_more_values_than_columns() {
    let _ = encode_key(&[ColumnType::I64], &[Value::I64(1), Value::I64(2)]);
}

/// `is_key` and `key_starts`, which check the key of a row read without decoding its text and bytes, accept exactly the
/// keys that `decode_key` decodes, and `key_value` at each start gives the fixed-width columns as `decode_key` does: over good keys with
/// escaped zeros and multi-byte characters, each truncation, each byte changed to an edge value and each byte inserted.
#[test]
fn key_checks_and_starts_agree_with_decode_key() {
    let cases: [(&[ColumnType], Vec<Value<'_>>); 4] = [
        (
            &[ColumnType::I32, ColumnType::String],
            vec![Value::I32(300), s("é\0é")],
        ),
        (
            &[ColumnType::String, ColumnType::I64],
            vec![s("a\0\0b"), Value::I64(-7)],
        ),
        (
            &[ColumnType::Bytes, ColumnType::Ulid],
            vec![b(&[0, 0xFF, 0, 1]), Value::Ulid(Ulid([9; 16]))],
        ),
        (
            &[ColumnType::Uuid(UuidOrder::SqlServer), ColumnType::Bool],
            vec![Value::Uuid(Uuid([3; 16])), Value::Bool(true)],
        ),
    ];
    for (types, values) in &cases {
        let good = encode_key(types, values).unwrap();
        let mut keys = vec![good.clone()];
        for end in 0..good.len() {
            keys.push(good[..end].to_vec());
        }
        for at in 0..good.len() {
            for byte in [0x00, 0x01, 0x02, 0x80, 0xC3, 0xFE, 0xFF, good[at] ^ 1] {
                let mut changed = good.clone();
                changed[at] = byte;
                keys.push(changed);
            }
            for byte in [0x00, 0xFF, 0xC3] {
                let mut longer = good.clone();
                longer.insert(at, byte);
                keys.push(longer);
            }
        }
        keys.push([good.as_slice(), &[0]].concat());
        for key in &keys {
            let decoded = decode_key(types, key);
            assert_eq!(
                is_key(types, key),
                decoded.is_some(),
                "{types:?} {key:02x?}"
            );
            let starts = key_starts(types, key);
            assert_eq!(starts.is_some(), decoded.is_some(), "{key:02x?}");
            let (Some(decoded), Some(starts)) = (decoded, starts) else {
                continue;
            };
            for (k, ty) in types.iter().enumerate() {
                if ty.fixed_width().is_some() {
                    let value = key_value(ty, &key[usize::from(starts[k])..]);
                    assert!(same(&[value], &decoded[k..=k]), "{key:02x?}");
                }
            }
        }
    }
}
