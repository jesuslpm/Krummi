//! The typed reads of a JSON number (JS7): its exact value computed on its text, never expanding its exponent, so
//! that the time and memory of a read grow with the length of the text, not with its value.

use crate::table::value::{Decimal, fits_precision};

/// The largest magnitude kept of an exponent: a text up to the row's 256 MiB has fewer digits than this, so an
/// exponent beyond it gives the same results as its exact value (a zero, or more digits than any type holds).
const EXPONENT_LIMIT: i64 = 1 << 60;

/// The exact value of a number text: `digits` × 10^`exponent`, where the significant digits are those of the
/// integer part followed by the fraction, from `start` to `end`, without leading or trailing zeros. Zero has none.
struct Exact<'a> {
    negative: bool,
    integer: &'a [u8],
    fraction: &'a [u8],
    start: usize,
    end: usize,
    exponent: i64,
}

impl Exact<'_> {
    /// The value of a text of RFC 8259's number grammar, or `None` for another text.
    fn parse(text: &str) -> Option<Exact<'_>> {
        let bytes = text.as_bytes();
        let (negative, bytes) = match bytes {
            [b'-', rest @ ..] => (true, rest),
            _ => (false, bytes),
        };
        let (integer, bytes) = bytes.split_at(leading_digits(bytes));
        if integer.is_empty() {
            return None;
        }
        let (fraction, bytes) = match bytes {
            [b'.', rest @ ..] => {
                let split = rest.split_at(leading_digits(rest));
                if split.0.is_empty() {
                    return None;
                }
                split
            }
            _ => (&[][..], bytes),
        };
        let exponent = match bytes {
            [] => 0,
            [b'e' | b'E', rest @ ..] => {
                let (negative, digits) = match rest {
                    [b'-', digits @ ..] => (true, digits),
                    [b'+', digits @ ..] => (false, digits),
                    _ => (false, rest),
                };
                if digits.is_empty() || leading_digits(digits) != digits.len() {
                    return None;
                }
                let magnitude = digits.iter().fold(0i64, |e, d| {
                    e.saturating_mul(10)
                        .saturating_add(i64::from(d - b'0'))
                        .min(EXPONENT_LIMIT)
                });
                if negative { -magnitude } else { magnitude }
            }
            _ => return None,
        };
        let mut exact = Exact {
            negative,
            integer,
            fraction,
            start: 0,
            end: integer.len() + fraction.len(),
            exponent: 0,
        };
        let trailing = (0..exact.end)
            .rev()
            .take_while(|&i| exact.at(i) == 0)
            .count();
        exact.end -= trailing;
        exact.start = (0..exact.end).take_while(|&i| exact.at(i) == 0).count();
        exact.exponent = exponent - len_i64(fraction.len()) + len_i64(trailing);
        Some(exact)
    }

    /// The digit at position `i` of the integer part followed by the fraction.
    fn at(&self, i: usize) -> u8 {
        let digit = match i.checked_sub(self.integer.len()) {
            None => self.integer[i],
            Some(i) => self.fraction[i],
        };
        digit - b'0'
    }

    /// The number of significant digits.
    fn len(&self) -> i64 {
        len_i64(self.end - self.start)
    }

    /// The integer of the first `count` significant digits, at most 38 of them.
    fn leading(&self, count: usize) -> u128 {
        (self.start..self.start + count).fold(0, |n, i| n * 10 + u128::from(self.at(i)))
    }

    /// The magnitude of an exact integer of at most 20 digits; `None` for another value. Zero is never negative.
    fn integer(&self) -> Option<(bool, u128)> {
        if self.len() == 0 {
            return Some((false, 0));
        }
        if self.exponent < 0 || self.len() + self.exponent > 20 {
            return None;
        }
        let exponent = u32::try_from(self.exponent).expect("at most 19");
        Some((
            self.negative,
            self.leading(self.end - self.start) * 10u128.pow(exponent),
        ))
    }
}

fn leading_digits(bytes: &[u8]) -> usize {
    bytes.iter().take_while(|b| b.is_ascii_digit()).count()
}

fn len_i64(len: usize) -> i64 {
    i64::try_from(len).expect("a text is shorter than i64::MAX")
}

fn signed(negative: bool, magnitude: u128) -> Option<i128> {
    let magnitude = i128::try_from(magnitude).ok()?;
    Some(if negative { -magnitude } else { magnitude })
}

/// A number whose exact value is an integer that `i64` holds.
pub(super) fn as_i64(text: &str) -> Option<i64> {
    let (negative, magnitude) = Exact::parse(text)?.integer()?;
    i64::try_from(signed(negative, magnitude)?).ok()
}

/// A number whose exact value is an integer that `u64` holds.
pub(super) fn as_u64(text: &str) -> Option<u64> {
    let (negative, magnitude) = Exact::parse(text)?.integer()?;
    u64::try_from(signed(negative, magnitude)?).ok()
}

/// A number correctly rounded to the nearest `f64` by the standard library, `-0` as `+0`; `None` outside the finite
/// range.
pub(super) fn as_f64(text: &str) -> Option<f64> {
    Exact::parse(text)?;
    let value: f64 = text.parse().ok()?;
    value.is_finite().then_some(value + 0.0)
}

/// A number rounded to `scale` with ties away from zero, as E8 rounds a `decimal` given to a column, computed on its
/// digits; `None` when the rounded value has more than `precision` digits. `precision` and `scale` are within N4's
/// limits.
pub(super) fn as_decimal(text: &str, precision: u8, scale: u8) -> Option<Decimal> {
    let exact = Exact::parse(text)?;
    let precision_digits = i64::from(precision);
    // The value times 10^scale is digits × 10^shift.
    let shift = exact.exponent + i64::from(scale);
    let magnitude = if exact.len() == 0 {
        0
    } else if shift >= 0 {
        if exact.len() + shift > precision_digits {
            return None;
        }
        let digits = usize::try_from(exact.len()).expect("at most 38");
        exact.leading(digits) * 10u128.pow(u32::try_from(shift).expect("at most 37"))
    } else {
        // Drop the last -shift digits; the first of them decides the rounding: half or more rounds away from zero.
        let kept = exact.len() + shift;
        if kept > precision_digits {
            return None;
        }
        match usize::try_from(kept) {
            Err(_) => 0,
            Ok(kept) => {
                let round_up = exact.at(exact.start + kept) >= 5;
                exact.leading(kept) + u128::from(round_up)
            }
        }
    };
    let mantissa = signed(exact.negative, magnitude).expect("at most 10^38");
    fits_precision(mantissa, precision).then_some(Decimal { mantissa, scale })
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;
    use std::time::Instant;

    use krummi_pager::{Error, ValueRule};

    use crate::table::json::JsonValue;
    use crate::table::value::Decimal;

    fn i64_of(text: &str) -> Option<i64> {
        JsonValue::new(text).as_i64()
    }

    fn u64_of(text: &str) -> Option<u64> {
        JsonValue::new(text).as_u64()
    }

    fn f64_of(text: &str) -> Option<f64> {
        JsonValue::new(text).as_f64()
    }

    fn decimal_of(text: &str, precision: u8, scale: u8) -> Option<i128> {
        let decimal = JsonValue::new(text).as_decimal(precision, scale).unwrap();
        decimal.map(|Decimal { mantissa, scale: s }| {
            assert_eq!(s, scale);
            mantissa
        })
    }

    #[test]
    fn js7_integer_examples() {
        for text in [
            "100", "100.0", "1e2", "1.00e2", "1E2", "1e+2", "10000e-2", "0.1e3",
        ] {
            assert_eq!(i64_of(text), Some(100), "{text}");
            assert_eq!(u64_of(text), Some(100), "{text}");
        }
        for text in [
            "-0",
            "0",
            "0e999999999",
            "-0.0",
            "0.000e-5",
            "0E+999999999999999999999",
        ] {
            assert_eq!(i64_of(text), Some(0), "{text}");
            assert_eq!(u64_of(text), Some(0), "{text}");
        }
        for text in ["100.6", "1e-1", "0.5", "-1.5", "1e-999999999999"] {
            assert_eq!(i64_of(text), None, "{text}");
            assert_eq!(u64_of(text), None, "{text}");
        }
        assert_eq!(i64_of("9223372036854775808"), None);
        assert_eq!(u64_of("-1"), None);
        assert_eq!(i64_of("-1"), Some(-1));
        assert_eq!(i64_of("-1e1"), Some(-10));
    }

    #[test]
    fn ac_js6_integer_limits_in_every_notation() {
        let i64_max = [
            "9223372036854775807",
            "9223372036854775807.0",
            "9.223372036854775807e18",
            "9.223372036854775807E+18",
            "92233720368547758070e-1",
            "0.9223372036854775807e19",
        ];
        for text in i64_max {
            assert_eq!(i64_of(text), Some(i64::MAX), "{text}");
            assert_eq!(u64_of(text), Some(i64::MAX as u64), "{text}");
        }
        let past_i64_max = [
            "9223372036854775808",
            "9223372036854775808.0",
            "9.223372036854775808e18",
            "92233720368547758080e-1",
        ];
        for text in past_i64_max {
            assert_eq!(i64_of(text), None, "{text}");
            assert_eq!(u64_of(text), Some(1 << 63), "{text}");
        }
        let i64_min = [
            "-9223372036854775808",
            "-9223372036854775808.000",
            "-9.223372036854775808e18",
            "-92233720368547758080E-1",
        ];
        for text in i64_min {
            assert_eq!(i64_of(text), Some(i64::MIN), "{text}");
            assert_eq!(u64_of(text), None, "{text}");
        }
        let past_i64_min = [
            "-9223372036854775809",
            "-9223372036854775809.0",
            "-9.223372036854775809e18",
            "-92233720368547758090e-1",
        ];
        for text in past_i64_min {
            assert_eq!(i64_of(text), None, "{text}");
        }
        let u64_max = [
            "18446744073709551615",
            "18446744073709551615.0",
            "1.8446744073709551615e19",
            "184467440737095516150e-1",
        ];
        for text in u64_max {
            assert_eq!(u64_of(text), Some(u64::MAX), "{text}");
            assert_eq!(i64_of(text), None, "{text}");
        }
        let past_u64_max = [
            "18446744073709551616",
            "18446744073709551616.0",
            "1.8446744073709551616e19",
            "184467440737095516160e-1",
            "1e20",
            "1e999999999999",
        ];
        for text in past_u64_max {
            assert_eq!(u64_of(text), None, "{text}");
            assert_eq!(i64_of(text), None, "{text}");
        }
        assert_eq!(u64_of("1e19"), Some(10_000_000_000_000_000_000));
        // Many digits are still read exactly.
        let long = format!("1{}e-200", "0".repeat(200));
        assert_eq!(i64_of(&long), Some(1));
        let long = format!("0.{}1e201", "0".repeat(199));
        assert_eq!(i64_of(&long), Some(10));
    }

    #[test]
    fn js7_decimal_examples_and_rounding() {
        assert_eq!(decimal_of("1.235", 10, 2), Some(124));
        assert_eq!(decimal_of("99.995", 4, 2), None);
        assert_eq!(decimal_of("99.994", 4, 2), Some(9999));
        // Ties away from zero, as E8, never to even.
        assert_eq!(decimal_of("-1.235", 10, 2), Some(-124));
        assert_eq!(decimal_of("1.225", 10, 2), Some(123));
        assert_eq!(decimal_of("0.5", 1, 0), Some(1));
        assert_eq!(decimal_of("-0.5", 1, 0), Some(-1));
        assert_eq!(
            decimal_of("0.4999999999999999999999999999999999999999999", 1, 0),
            Some(0)
        );
        assert_eq!(decimal_of("-0.001", 5, 2), Some(0));
        assert_eq!(decimal_of("1.5e-1", 5, 0), Some(0));
        assert_eq!(decimal_of("5e-1", 5, 0), Some(1));
        assert_eq!(decimal_of("1e-999999999999", 10, 2), Some(0));
        assert_eq!(decimal_of("-0", 10, 2), Some(0));
        assert_eq!(decimal_of("0e999999999999", 10, 2), Some(0));
        assert_eq!(decimal_of("12", 4, 2), Some(1200));
        assert_eq!(decimal_of("120", 4, 2), None);
        assert_eq!(decimal_of("1.2e1", 4, 2), Some(1200));
        assert_eq!(decimal_of("0.5", 38, 38), Some(5 * 10i128.pow(37)));
        assert_eq!(decimal_of("1", 38, 38), None);
        assert_eq!(decimal_of("123.456", 38, 0), Some(123));
        // With more than 38 digits, rounded on the digits.
        let digits = format!("1.{}5", "0".repeat(60));
        assert_eq!(decimal_of(&digits, 38, 37), Some(10i128.pow(37)));
        let digits = format!("1.{}5", "4".repeat(60));
        assert_eq!(
            decimal_of(&digits, 38, 37),
            Some(14_444_444_444_444_444_444_444_444_444_444_444_444)
        );
    }

    #[test]
    fn ac_js6_decimal_with_38_digits_and_39_after_rounding() {
        let nines = "9".repeat(38);
        let max = 10i128.pow(38) - 1;
        assert_eq!(decimal_of(&nines, 38, 0), Some(max));
        assert_eq!(decimal_of(&format!("-{nines}"), 38, 0), Some(-max));
        assert_eq!(decimal_of(&format!("{nines}.4"), 38, 0), Some(max));
        assert_eq!(decimal_of(&format!("{nines}.5"), 38, 0), None);
        assert_eq!(decimal_of(&format!("-{nines}.5"), 38, 0), None);
        assert_eq!(decimal_of(&format!("{nines}0"), 38, 0), None);
        assert_eq!(decimal_of(&format!("{}.94", &nines[1..]), 38, 1), Some(max));
        assert_eq!(decimal_of(&format!("{}.95", &nines[1..]), 38, 1), None);
        assert_eq!(decimal_of("1e37", 38, 0), Some(10i128.pow(37)));
        assert_eq!(decimal_of("1e38", 38, 0), None);
        assert_eq!(
            decimal_of("9.9999999999999999999999999999999999999e37", 38, 0),
            Some(max)
        );
        assert_eq!(decimal_of(&format!("0.{nines}"), 38, 38), Some(max));
        assert_eq!(decimal_of(&format!("0.{nines}5"), 38, 38), None);
    }

    #[test]
    fn ac_js6_decimal_precision_and_scale_out_of_limits_fail_first() {
        for (precision, scale) in [(0, 0), (39, 0), (5, 6), (0, 1), (255, 0)] {
            for text in ["1", r#""a""#, "null", "{}"] {
                let err = JsonValue::new(text)
                    .as_decimal(precision, scale)
                    .unwrap_err();
                assert!(
                    matches!(
                        err,
                        Error::InvalidValue {
                            folder: None,
                            table: None,
                            column: None,
                            sequence: None,
                            rule: ValueRule::InvalidReadScale,
                            json: None,
                        }
                    ),
                    "{err:?}"
                );
            }
        }
        for (precision, scale) in [(1, 0), (1, 1), (38, 0), (38, 38)] {
            assert!(JsonValue::new("0").as_decimal(precision, scale).is_ok());
        }
    }

    #[test]
    fn js7_f64_examples_and_limits() {
        assert_eq!(f64_of("0.1"), f64_of("0.10000000000000001"));
        assert_eq!(f64_of("0.1"), Some(0.1));
        for zero in ["1e-400", "-1e-400", "-0", "-0.0e5", "0"] {
            assert_eq!(f64_of(zero).map(f64::to_bits), Some(0), "{zero}");
        }
        assert_eq!(f64_of("1e400"), None);
        assert_eq!(f64_of("-1e400"), None);
        assert_eq!(f64_of("1.7976931348623157e308"), Some(f64::MAX));
        assert_eq!(f64_of("-1.7976931348623157e308"), Some(f64::MIN));
        assert_eq!(f64_of("1.7976931348623158e308"), Some(f64::MAX));
        assert_eq!(f64_of("1.7976931348623159e308"), None);
        assert_eq!(f64_of("5e-324"), Some(f64::from_bits(1)));
        assert_eq!(f64_of("1e999999999999"), None);
        assert_eq!(f64_of("1E2"), Some(100.0));
        assert_eq!(f64_of("-2.5e-3"), Some(-0.0025));
    }

    fn median_nanos(read: impl Fn()) -> u128 {
        let mut runs: Vec<u128> = (0..5)
            .map(|_| {
                let start = Instant::now();
                for _ in 0..20_000 {
                    read();
                }
                start.elapsed().as_nanos()
            })
            .collect();
        runs.sort_unstable();
        runs[2]
    }

    /// AC-JS6: a number is read without expanding its exponent (JS7).
    #[test]
    fn ac_js6_huge_exponent_is_read_without_expanding_it() {
        let huge = JsonValue::new("1e999999999999");
        let small = JsonValue::new("1e9");
        assert_eq!(huge.as_i64(), None);
        assert_eq!(huge.as_decimal(38, 0).unwrap(), None);
        let huge_i64 = median_nanos(|| {
            black_box(black_box(huge).as_i64());
        });
        let small_i64 = median_nanos(|| {
            black_box(black_box(small).as_i64());
        });
        let huge_decimal = median_nanos(|| {
            black_box(black_box(huge).as_decimal(38, 0).unwrap());
        });
        let small_decimal = median_nanos(|| {
            black_box(black_box(small).as_decimal(38, 0).unwrap());
        });
        assert!(
            huge_i64 <= small_i64 * 10,
            "{huge_i64} ns against {small_i64} ns"
        );
        assert!(
            huge_decimal <= small_decimal * 10,
            "{huge_decimal} ns against {small_decimal} ns"
        );
    }
}
