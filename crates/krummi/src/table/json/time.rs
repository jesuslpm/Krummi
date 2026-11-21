//! The typed reads of a JSON string as a date or an instant (JS7).

use crate::table::value::{ColumnType, NANOS_PER_MICRO, round_micros, stored_in_domain};

const NANOS_PER_SECOND: i128 = 1_000_000_000;

/// A string `YYYY-MM-DD` (JS7) as days since 1970-01-01, or `None` when it is not one or is outside E8's domain.
pub(super) fn date(text: &str) -> Option<i32> {
    let days = i32::try_from(civil_days(text.as_bytes())?).ok()?;
    stored_in_domain(&ColumnType::Date, days.into()).then_some(days)
}

/// A string `YYYY-MM-DDTHH:MM:SS[.f]` followed by `Z`, `+HH:MM` or `-HH:MM` (JS7), converted to UTC and rounded to
/// the microsecond as E8 rounds instants, as nanoseconds since the Unix epoch; `None` when it is not one or is
/// outside E8's domain after the conversion and the rounding.
pub(super) fn timestamp(text: &str) -> Option<i128> {
    let bytes = text.as_bytes();
    let (date, rest) = bytes.split_at_checked(10)?;
    let days = civil_days(date)?;
    let [b'T', h1, h2, b':', m1, m2, b':', s1, s2, rest @ ..] = rest else {
        return None;
    };
    let (hour, minute, second) = (two(*h1, *h2, 23)?, two(*m1, *m2, 59)?, two(*s1, *s2, 59)?);
    // The fraction's first nine digits: those past the nanosecond never change the rounding to the microsecond,
    // which only looks at whole nanoseconds against a half of a microsecond.
    let (fraction_nanos, rest) = match rest {
        [b'.', fraction @ ..] => {
            let count = fraction.iter().take_while(|b| b.is_ascii_digit()).count();
            if count == 0 {
                return None;
            }
            let nanos = fraction[..count]
                .iter()
                .chain(std::iter::repeat(&b'0'))
                .take(9)
                .fold(0i128, |n, d| n * 10 + i128::from(d - b'0'));
            (nanos, &fraction[count..])
        }
        _ => (0, rest),
    };
    let offset = match rest {
        [b'Z'] => 0,
        [sign @ (b'+' | b'-'), h1, h2, b':', m1, m2] => {
            let offset = i64::from(two(*h1, *h2, 23)?) * 3600 + i64::from(two(*m1, *m2, 59)?) * 60;
            if *sign == b'+' { offset } else { -offset }
        }
        _ => return None,
    };
    let local = days * 86_400 + i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second);
    let nanos = i128::from(local - offset) * NANOS_PER_SECOND + fraction_nanos;
    let micros = round_micros(nanos);
    stored_in_domain(&ColumnType::Timestamp, micros).then_some(micros * NANOS_PER_MICRO)
}

/// The days since 1970-01-01 of exactly `YYYY-MM-DD`, with a year from 0001 and a day that exists in its month.
fn civil_days(bytes: &[u8]) -> Option<i64> {
    let [y1, y2, y3, y4, b'-', m1, m2, b'-', d1, d2] = *bytes else {
        return None;
    };
    let year = i64::from(two(y1, y2, 99)?) * 100 + i64::from(two(y3, y4, 99)?);
    let month = two(m1, m2, 12)?;
    let day = two(d1, d2, 31)?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if year == 0 || month == 0 || day == 0 || day > month_days {
        return None;
    }
    Some(days_from_civil(year, i64::from(month), i64::from(day)))
}

/// Two decimal digits as a number up to `max`.
fn two(tens: u8, units: u8, max: u8) -> Option<u8> {
    if !tens.is_ascii_digit() || !units.is_ascii_digit() {
        return None;
    }
    let value = (tens - b'0') * 10 + (units - b'0');
    (value <= max).then_some(value)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's `days_from_civil`, the inverse of the
/// `civil_from_days` that formats dates in `value.rs`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year.rem_euclid(400);
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use crate::table::json::JsonValue;
    use crate::table::value::{DATE_MAX, DATE_MIN, TIMESTAMP_MAX, TIMESTAMP_MIN, Value};

    fn date(text: &str) -> Option<String> {
        let json = format!("\"{text}\"");
        JsonValue::new(&json)
            .as_date()
            .map(|days| Value::Date(days).to_string())
    }

    fn instant(text: &str) -> Option<String> {
        let json = format!("\"{text}\"");
        JsonValue::new(&json).as_timestamp().map(|nanos| {
            assert_eq!(nanos % 1000, 0, "a whole number of microseconds");
            Value::Timestamp(nanos).to_string()
        })
    }

    #[test]
    fn js7_date_examples_and_limits() {
        assert_eq!(JsonValue::new(r#""2026-11-11""#).as_date(), Some(20_768));
        assert_eq!(JsonValue::new(r#""1970-01-01""#).as_date(), Some(0));
        assert_eq!(JsonValue::new(r#""0001-01-01""#).as_date(), Some(DATE_MIN));
        assert_eq!(JsonValue::new(r#""9999-12-31""#).as_date(), Some(DATE_MAX));
        for text in [
            "2024-02-29",
            "2000-02-29",
            "1969-12-31",
            "2026-01-31",
            "2026-04-30",
        ] {
            assert_eq!(date(text).as_deref(), Some(text));
        }
        for text in [
            "0000-12-31",
            "10000-01-01",
            "999-12-31",
            "2026-02-30",
            "2023-02-29",
            "1900-02-29",
            "2026-04-31",
            "2026-13-01",
            "2026-00-10",
            "2026-01-00",
            "2026-01-32",
            "2026-1-01",
            "2026-01-1",
            " 2026-11-11",
            "2026-11-11 ",
            "2026/11/11",
            "11/11/2026",
            "2026-11-11T00:00:00Z",
            "+2026-11-11",
            "2026-11-1a",
            "",
        ] {
            assert_eq!(date(text), None, "{text}");
        }
        for other in ["20261111", "null", "true", "[\"2026-11-11\"]"] {
            assert_eq!(JsonValue::new(other).as_date(), None, "{other}");
            assert_eq!(JsonValue::new(other).as_timestamp(), None, "{other}");
        }
    }

    #[test]
    fn js7_instant_examples() {
        assert_eq!(
            instant("2026-11-11T10:23:00.1234567Z").as_deref(),
            Some("2026-11-11T10:23:00.123457Z")
        );
        assert_eq!(
            instant("2026-11-11T00:30:00+01:00").as_deref(),
            Some("2026-11-10T23:30:00Z")
        );
        assert_eq!(
            instant("1970-01-01T00:00:00Z").as_deref(),
            Some("1970-01-01T00:00:00Z")
        );
        assert_eq!(
            JsonValue::new(r#""1970-01-01T00:00:01Z""#).as_timestamp(),
            Some(1_000_000_000)
        );
        for text in [
            "2019-07-26T00:00:00",
            "2026-11-11t10:23:00Z",
            "2026-11-11T10:23:00z",
            "2026-11-11 10:23:00Z",
            "2026-11-11T10:23:60Z",
            "2026-11-11T24:00:00Z",
            "2026-11-11T10:60:00Z",
            "2026-11-11T10:23:00.Z",
            "2026-11-11T10:23Z",
            "2026-11-11T10:23:00.1.2Z",
            "2026-11-11T10:23:00ZZ",
            "2026-11-11T10:23:00+0100",
            "2026-11-11T10:23:00+01",
            "2026-11-11T10:23:00 Z",
            "2026-02-30T10:23:00Z",
            "0000-01-01T10:23:00Z",
            "2026-11-11T1:23:00Z",
            "",
        ] {
            assert_eq!(instant(text), None, "{text}");
        }
        assert_eq!(JsonValue::new("1762856580").as_timestamp(), None);
    }

    #[test]
    fn ac_js6_offsets() {
        assert_eq!(
            instant("2026-11-11T00:30:00+23:59").as_deref(),
            Some("2026-11-10T00:31:00Z")
        );
        assert_eq!(
            instant("2026-11-11T00:30:00-23:59").as_deref(),
            Some("2026-11-12T00:29:00Z")
        );
        assert_eq!(
            instant("2026-11-11T00:30:00-00:00"),
            instant("2026-11-11T00:30:00Z")
        );
        assert_eq!(
            instant("2026-11-11T00:30:00+00:00"),
            instant("2026-11-11T00:30:00Z")
        );
        for text in [
            "2026-11-11T00:30:00+24:00",
            "2026-11-11T00:30:00+00:60",
            "2026-11-11T00:30:00-24:00",
            "2026-11-11T00:30:00-00:60",
        ] {
            assert_eq!(instant(text), None, "{text}");
        }
    }

    #[test]
    fn ac_js6_leap_and_non_leap_29_february() {
        assert_eq!(
            instant("2024-02-29T12:00:00Z").as_deref(),
            Some("2024-02-29T12:00:00Z")
        );
        assert_eq!(instant("2023-02-29T12:00:00Z"), None);
        assert_eq!(
            instant("2024-03-01T00:30:00+01:00").as_deref(),
            Some("2024-02-29T23:30:00Z")
        );
    }

    #[test]
    fn ac_js6_fraction_rounds_to_the_microsecond_ties_up() {
        assert_eq!(
            instant("2026-11-11T10:23:00.0000005Z").as_deref(),
            Some("2026-11-11T10:23:00.000001Z")
        );
        assert_eq!(
            instant("2026-11-11T10:23:00.00000049999999999999Z").as_deref(),
            Some("2026-11-11T10:23:00Z")
        );
        assert_eq!(
            instant("2026-11-11T10:23:59.9999995Z").as_deref(),
            Some("2026-11-11T10:24:00Z")
        );
        // Before the epoch the tie also goes to the greater instant.
        assert_eq!(
            instant("1969-12-31T23:59:59.9999995Z").as_deref(),
            Some("1970-01-01T00:00:00Z")
        );
        assert_eq!(
            instant("1969-12-31T23:59:59.9999994Z").as_deref(),
            Some("1969-12-31T23:59:59.999999Z")
        );
        let long = format!("2026-11-11T10:23:00.{}Z", "1".repeat(1000));
        assert_eq!(
            instant(&long).as_deref(),
            Some("2026-11-11T10:23:00.111111Z")
        );
        assert_eq!(
            instant("2026-11-11T10:23:00.5Z").as_deref(),
            Some("2026-11-11T10:23:00.5Z")
        );
    }

    #[test]
    fn ac_js6_instants_at_the_ends_of_the_domain() {
        let at = |text: &str| JsonValue::new(&format!("\"{text}\"")).as_timestamp();
        let min = Some(i128::from(TIMESTAMP_MIN) * 1000);
        let max = Some(i128::from(TIMESTAMP_MAX) * 1000);
        assert_eq!(at("0001-01-01T00:00:00Z"), min);
        assert_eq!(at("0001-01-01T01:00:00+01:00"), min);
        assert_eq!(at("0001-01-01T00:00:00.0000004Z"), min);
        assert_eq!(at("9999-12-31T23:59:59.999999Z"), max);
        assert_eq!(at("9999-12-31T23:59:59.9999994999Z"), max);
        assert_eq!(at("9999-12-31T22:59:59.999999-01:00"), max);
        // Out, also when the offset or the rounding takes it out.
        for text in [
            "0001-01-01T00:00:00+00:01",
            "0001-01-01T00:59:59.999999+01:00",
            "9999-12-31T23:59:59.9999995Z",
            "9999-12-31T23:59:59-00:01",
            "9999-12-31T23:00:00-01:00",
            "0000-12-31T23:59:59-01:00",
        ] {
            assert_eq!(at(text), None, "{text}");
        }
    }
}
