//! Temporal values in microseconds (`docs/prd/krummi/tables/tables-temporal-micros.prd.md`): `time`, `timestamp` and
//! `duration` values are given and read in microseconds, as `i64`, everywhere (AC-TU1-AC-TU3, AC-TU5, AC-TU6), and the
//! bytes that rows and primary keys store are the ones Krummi stored before the change (AC-TU4). The expected bytes were
//! captured from the code before the change, when values were given in nanoseconds.

use std::ops::Bound::{Excluded, Included, Unbounded};

use krummi::table::json::Json;
use krummi::table::key::encode_key;
use krummi::table::projection::{Duration as ProjectedDuration, Time, Timestamp};
use krummi::table::row::{RowColumn, RowLayout};
use krummi::table::value::{
    ColumnType, DURATION_MAX, TIME_MAX, TIMESTAMP_MAX, TIMESTAMP_MIN, Value,
};
use krummi::{
    Column, Condition, Error, ErrorKind, IndexDefinition, IndexKey, IndexMode, Schema, SchemaRule,
    Store, StoreOptions, TableMode, ValueRule,
};

/// The value of a `time` of `micros` microseconds since midnight.
fn time(micros: i64) -> Value<'static> {
    Value::Time(micros)
}

/// The value of a `timestamp` of `micros` microseconds since the Unix epoch.
fn timestamp(micros: i64) -> Value<'static> {
    Value::Timestamp(micros)
}

/// The value of a `duration` of `micros` microseconds.
fn duration(micros: i64) -> Value<'static> {
    Value::Duration(micros)
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

// ---------------------------------------------------------------------------------------------------------------
// AC-TU1–AC-TU3, AC-TU5 and AC-TU6: the public API in microseconds.

/// The table of the acceptance tests: a `u64` key and one column of each temporal type, each with its index.
fn temporal_table(store: &Store) -> Result<(), Error> {
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::U64).not_null(),
            Column::new("t", ColumnType::Time),
            Column::new("ts", ColumnType::Timestamp),
            Column::new("d", ColumnType::Duration),
        ],
        &["id"],
    );
    let table = write.open_table_with("temporal", TableMode::CreateNew(&schema))?;
    for column in ["t", "ts", "d"] {
        let definition = IndexDefinition::new(format!("ix_{column}"), vec![IndexKey::new(column)]);
        write.create_index(&table, &definition, IndexMode::CreateNew)?;
    }
    write.commit()
}

/// The values of each column in microseconds: the extremes of its domain and a point between them.
fn domain_values() -> [[i64; 3]; 3] {
    [
        [0, 32_400_500_000, TIME_MAX],
        [TIMESTAMP_MIN, 1_794_392_580_123_457, TIMESTAMP_MAX],
        [-DURATION_MAX, -1_500_000, DURATION_MAX],
    ]
}

fn row_of(id: u64, t: i64, ts: i64, d: i64) -> Vec<(&'static str, Value<'static>)> {
    vec![
        ("id", Value::U64(id)),
        ("t", Value::Time(t)),
        ("ts", Value::Timestamp(ts)),
        ("d", Value::Duration(d)),
    ]
}

/// The value of column `position` (1 `t`, 2 `ts`, 3 `d`) of `micros` microseconds.
fn temporal(position: usize, micros: i64) -> Value<'static> {
    match position {
        1 => Value::Time(micros),
        2 => Value::Timestamp(micros),
        _ => Value::Duration(micros),
    }
}

#[test]
fn values_are_given_and_read_back_in_microseconds_by_every_write_and_read() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    temporal_table(&store).unwrap();
    let [t, ts, d] = domain_values();
    // Rows 0..3 by insert, 3..6 by insert then update, 6..9 by upsert, 9..12 by a bulk load.
    let mut write = store.write().unwrap();
    let table = write.open_table("temporal").unwrap();
    for i in 0..3 {
        write
            .insert_row(&table, &row_of(i as u64, t[i], ts[i], d[i]))
            .unwrap();
        write
            .insert_row(&table, &row_of(3 + i as u64, 0, 0, 0))
            .unwrap();
        assert!(
            write
                .update_row(&table, &row_of(3 + i as u64, t[i], ts[i], d[i]))
                .unwrap()
        );
        write
            .upsert_row(&table, &row_of(6 + i as u64, t[i], ts[i], d[i]))
            .unwrap();
    }
    write.commit().unwrap();
    let rows: Vec<Result<_, std::convert::Infallible>> = (0..3)
        .map(|i| Ok(row_of(9 + i as u64, t[i], ts[i], d[i])))
        .collect();
    assert_eq!(store.bulk_load("temporal", rows).unwrap(), 3);

    let read = store.read().unwrap();
    let table = read.open_table("temporal").unwrap();
    let projection = read.projection(&table, &["t", "ts", "d"]).unwrap();
    let pt = projection.column::<Time>(0).unwrap();
    let pts = projection.column::<Timestamp>(1).unwrap();
    let pd = projection.column::<ProjectedDuration>(2).unwrap();
    for id in 0..12u64 {
        let i = (id % 3) as usize;
        let row = read.get_row(&table, &[Value::U64(id)]).unwrap().unwrap();
        // Typed reads are i64 microseconds (AC-TU3).
        let typed: (Option<i64>, Option<i64>, Option<i64>) =
            (row.time(1), row.timestamp(2), row.duration(3));
        assert_eq!(typed, (Some(t[i]), Some(ts[i]), Some(d[i])), "row {id}");
        assert_eq!(row.get("ts"), Some(Value::Timestamp(ts[i])));
        let bound = projection.bind(&row).unwrap();
        let projected: (Option<i64>, Option<i64>, Option<i64>) =
            (bound.get(&pt), bound.get(&pts), bound.get(&pd));
        assert_eq!(projected, (Some(t[i]), Some(ts[i]), Some(d[i])));
    }
    // By a range of the primary key, and by each index in its order.
    let ids: Vec<u64> = read
        .range_rows(&table, &[], Value::U64(3)..Value::U64(6))
        .unwrap()
        .map(|row| row.unwrap().u64(0).unwrap())
        .collect();
    assert_eq!(ids, [3, 4, 5]);
    for (index, position, values) in [("ix_t", 1, t), ("ix_ts", 2, ts), ("ix_d", 3, d)] {
        let found: Vec<Value<'static>> = read
            .index_rows(&table, index, &[], ..)
            .unwrap()
            .map(|row| row.unwrap().value(position).unwrap().into_owned())
            .collect();
        let expected: Vec<Value<'static>> = values
            .iter()
            .flat_map(|&v| std::iter::repeat_n(temporal(position, v), 4))
            .collect();
        assert_eq!(found, expected, "{index}");
    }
}

#[test]
fn values_outside_their_domain_and_the_integer_extremes_fail_without_effect() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    temporal_table(&store).unwrap();
    let mut write = store.write().unwrap();
    let table = write.open_table("temporal").unwrap();
    let outside = [
        (1, [-1, TIME_MAX + 1, i64::MIN, i64::MAX]),
        (
            2,
            [TIMESTAMP_MIN - 1, TIMESTAMP_MAX + 1, i64::MIN, i64::MAX],
        ),
        (3, [-DURATION_MAX - 1, DURATION_MAX + 1, i64::MIN, i64::MAX]),
    ];
    for (position, values) in outside {
        for value in values {
            let column = ["", "t", "ts", "d"][position];
            let error = write
                .insert_row(
                    &table,
                    &[("id", Value::U64(1)), (column, temporal(position, value))],
                )
                .unwrap_err();
            assert!(
                matches!(
                    error,
                    Error::InvalidValue {
                        rule: ValueRule::OutOfDomain,
                        ..
                    }
                ),
                "{column} {value}: {error:?}"
            );
        }
    }
    assert!(write.get_row(&table, &[Value::U64(1)]).unwrap().is_none());
}

#[test]
fn defaults_bounds_conditions_and_statistics_are_in_microseconds() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    temporal_table(&store).unwrap();
    let [t, ts, d] = domain_values();
    let mut write = store.write().unwrap();
    let table = write.open_table("temporal").unwrap();
    for i in 0..3 {
        write
            .insert_row(&table, &row_of(i as u64, t[i], ts[i], d[i]))
            .unwrap();
    }
    // A default of an added column, read by a row written before it; one outside its domain fails.
    write
        .add_column(
            &table,
            &Column::new("added", ColumnType::Timestamp).with_default(Value::Timestamp(ts[1])),
        )
        .unwrap();
    let error = write
        .add_column(
            &table,
            &Column::new("bad", ColumnType::Duration).with_default(Value::Duration(i64::MAX)),
        )
        .unwrap_err();
    assert!(
        matches!(
            &error,
            Error::InvalidSchema {
                rule: SchemaRule::InvalidDefault,
                ..
            }
        ),
        "{error:?}"
    );
    let table = write.open_table("temporal").unwrap();
    let row = write.get_row(&table, &[Value::U64(0)]).unwrap().unwrap();
    assert_eq!(row.timestamp(4), Some(ts[1]));
    drop(row);

    // Bounds of a primary key interval and of an index interval; one outside its domain fails.
    let between: Vec<u64> = write
        .index_rows(
            &table,
            "ix_ts",
            &[],
            (
                Excluded(Value::Timestamp(ts[0])),
                Included(Value::Timestamp(ts[2])),
            ),
        )
        .unwrap()
        .map(|row| row.unwrap().u64(0).unwrap())
        .collect();
    assert_eq!(between, [1, 2]);
    let error = write
        .index_rows(
            &table,
            "ix_d",
            &[],
            (Included(Value::Duration(i64::MIN)), Unbounded),
        )
        .err()
        .unwrap();
    assert_eq!(error.kind(), ErrorKind::InvalidValue);

    // Statistics and estimates: the table is read whole, so its values are the rows' values.
    write.analyze(&table, None).unwrap();
    let stats = write.column_statistics(&table, "d").unwrap().unwrap();
    let mut seen: Vec<Value<'static>> = stats.frequent.iter().map(|(v, _)| v.clone()).collect();
    seen.extend(stats.histogram.iter().map(|bucket| bucket.upper.clone()));
    for value in d {
        assert!(
            seen.contains(&Value::Duration(value)),
            "{value} in {seen:?}"
        );
    }
    let equal = Condition::Equal {
        column: "t",
        value: Value::Time(t[1]),
    };
    let estimate = write.estimate(&table, &equal).unwrap();
    assert!(estimate.rows > 0.0, "{estimate:?}");
    let outside = Condition::Equal {
        column: "ts",
        value: Value::Timestamp(TIMESTAMP_MAX + 1),
    };
    assert_eq!(
        write.estimate(&table, &outside).unwrap_err().kind(),
        ErrorKind::InvalidValue
    );
}

#[test]
fn json_instants_are_read_in_microseconds() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::U64).not_null(),
            Column::new("doc", ColumnType::Json),
        ],
        &["id"],
    );
    let table = write
        .open_table_with("docs", TableMode::CreateNew(&schema))
        .unwrap();
    let doc = concat!(
        r#"["2026-11-11T10:23:00.1234567Z","2026-11-11T00:30:00+01:00","1969-12-31T23:59:59.9999995Z","#,
        r#""0001-01-01T00:59:59.9999996+01:00","0001-01-01T00:59:59.9999994+01:00","#,
        r#""9999-12-31T23:59:59.9999995Z","2026-11-11T10:23:00","2026-02-30T00:00:00Z"]"#
    );
    write
        .insert_row(
            &table,
            &[("id", Value::U64(1)), ("doc", Value::Json(Json::new(doc)))],
        )
        .unwrap();
    let row = write.get_row(&table, &[Value::U64(1)]).unwrap().unwrap();
    let json = row.json(1).unwrap();
    let instants: Vec<Option<i64>> = json
        .document()
        .unwrap()
        .elements()
        .unwrap()
        .map(|v| v.as_timestamp())
        .collect();
    assert_eq!(
        instants,
        [
            Some(1_794_392_580_123_457),
            Some(1_794_353_400_000_000),
            Some(0),
            Some(TIMESTAMP_MIN),
            None,
            None,
            None,
            None,
        ]
    );
}

#[test]
fn text_forms_and_duplicate_keys_keep_their_text() {
    let texts = [
        (Value::Time(32_400_500_000), ColumnType::Time, "09:00:00.5"),
        (Value::Time(TIME_MAX), ColumnType::Time, "23:59:59.999999"),
        (
            Value::Timestamp(0),
            ColumnType::Timestamp,
            "1970-01-01T00:00:00Z",
        ),
        (
            Value::Timestamp(1_794_392_580_123_457),
            ColumnType::Timestamp,
            "2026-11-11T10:23:00.123457Z",
        ),
        (Value::Duration(-1_500_000), ColumnType::Duration, "-1.5s"),
        (Value::Duration(1), ColumnType::Duration, "0.000001s"),
    ];
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    for (i, (value, ty, text)) in texts.into_iter().enumerate() {
        assert_eq!(value.to_string(), text);
        let schema = Schema::new(vec![Column::new("k", ty).not_null()], &["k"]);
        let table = write
            .open_table_with(&format!("keys{i}"), TableMode::CreateNew(&schema))
            .unwrap();
        write.insert_row(&table, &[("k", value.clone())]).unwrap();
        match write.insert_row(&table, &[("k", value)]).unwrap_err() {
            Error::DuplicateKey { key, .. } => assert_eq!(key, format!("({text})")),
            other => panic!("{other:?}"),
        }
    }
}

/// A key whose integer does not fit in an `i64`, such as one written for `u64::MAX`, is not a temporal key Krummi
/// writes: decoding it as a `time`, `timestamp` or `duration` refuses it.
#[test]
fn keys_whose_integer_does_not_fit_in_an_i64_are_not_temporal_keys() {
    let key = encode_key(&[ColumnType::U64], &[Value::U64(u64::MAX)]).unwrap();
    for ty in [
        ColumnType::Time,
        ColumnType::Timestamp,
        ColumnType::Duration,
    ] {
        assert_eq!(krummi::table::key::decode_key(&[ty], &key), None, "{ty:?}");
    }
    let fits = encode_key(&[ColumnType::I64], &[Value::I64(TIMESTAMP_MAX)]).unwrap();
    assert_eq!(
        krummi::table::key::decode_key(&[ColumnType::Timestamp], &fits),
        Some(vec![Value::Timestamp(TIMESTAMP_MAX)])
    );
}
