//! Temporal values in microseconds (`docs/prd/krummi/tables/tables-temporal-micros.prd.md`): the bytes that rows and
//! primary keys store for `time`, `timestamp` and `duration` values are the ones Krummi stored before the change
//! (AC-TU4). The expected bytes were captured from the code before the change, when values were given in nanoseconds.

use krummi::table::key::encode_key;
use krummi::table::row::{RowColumn, RowLayout};
use krummi::table::value::{
    ColumnType, DURATION_MAX, TIME_MAX, TIMESTAMP_MAX, TIMESTAMP_MIN, Value,
};

/// The value of a `time` of `micros` microseconds since midnight.
fn time(micros: i64) -> Value<'static> {
    Value::Time(micros * 1_000)
}

/// The value of a `timestamp` of `micros` microseconds since the Unix epoch.
fn timestamp(micros: i64) -> Value<'static> {
    Value::Timestamp(i128::from(micros) * 1_000)
}

/// The value of a `duration` of `micros` microseconds.
fn duration(micros: i64) -> Value<'static> {
    Value::Duration(i128::from(micros) * 1_000)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn column(id: u64, ty: ColumnType) -> RowColumn {
    RowColumn {
        id,
        ty,
        nullable: true,
        key: false,
    }
}

/// The samples of each type: the extremes of its domain, values around zero and one with a fraction of a second.
fn samples() -> Vec<(ColumnType, Value<'static>)> {
    vec![
        (ColumnType::Time, time(0)),
        (ColumnType::Time, time(1)),
        (ColumnType::Time, time(32_400_500_000)),
        (ColumnType::Time, time(TIME_MAX)),
        (ColumnType::Timestamp, timestamp(TIMESTAMP_MIN)),
        (ColumnType::Timestamp, timestamp(-1)),
        (ColumnType::Timestamp, timestamp(0)),
        (ColumnType::Timestamp, timestamp(1_794_392_580_123_457)),
        (ColumnType::Timestamp, timestamp(TIMESTAMP_MAX)),
        (ColumnType::Duration, duration(-DURATION_MAX)),
        (ColumnType::Duration, duration(-1_500_000)),
        (ColumnType::Duration, duration(0)),
        (ColumnType::Duration, duration(1)),
        (ColumnType::Duration, duration(DURATION_MAX)),
    ]
}

#[test]
fn primary_keys_store_the_bytes_stored_before_the_change() {
    let expected = [
        "14",
        "1501",
        "19078b386520",
        "19141dd75fff",
        "0d23400100d43fff",
        "13fe",
        "14",
        "1b065ffd94536b41",
        "1c0384440ccc735fff",
        "0cf333333333333333",
        "11e91c9f",
        "14",
        "1501",
        "1c0ccccccccccccccc",
    ];
    let actual: Vec<String> = samples()
        .iter()
        .map(|(ty, value)| hex(&encode_key(&[*ty], std::slice::from_ref(value)).unwrap()))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn rows_store_the_bytes_stored_before_the_change() {
    let layout = RowLayout::new(
        1,
        vec![
            column(0, ColumnType::Time),
            column(1, ColumnType::Timestamp),
            column(2, ColumnType::Duration),
        ],
    );
    let rows = [
        vec![time(0), timestamp(TIMESTAMP_MIN), duration(-DURATION_MAX)],
        vec![
            time(32_400_500_000),
            timestamp(1_794_392_580_123_457),
            duration(-1_500_000),
        ],
        vec![
            time(TIME_MAX),
            timestamp(TIMESTAMP_MAX),
            duration(DURATION_MAX),
        ],
        vec![Value::Null, timestamp(-1), Value::Null],
    ];
    let expected = [
        "0100000000000000000000000040d400014023ff34333333333333f3",
        "010000002065388b07000000416b5394fd5f0600a01ce9ffffffffff",
        "01000000ff5fd71d14000000ff5f73cc0c448403cccccccccccccc0c",
        "010000050000000000000000ffffffffffffffff0000000000000000",
    ];
    let actual: Vec<String> = rows.iter().map(|row| hex(&layout.encode(row))).collect();
    assert_eq!(actual, expected);
}
