//! Column projections (`docs/prd/krummi/tables/tables-projection.prd.md`): AC-PY1 and AC-PY3 to AC-PY8, AC-PY10 and
//! AC-PY12 through the public interface. The criteria that need a malformed row or a history without a version
//! (AC-PY2's damage, AC-PY9) are internal tests of `table::projection_tests`.

mod support;

use std::borrow::Cow;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use krummi::ids::{Ulid, Uuid};
use krummi::table::projection::{
    Bytes, Date, Duration, MAX_PROJECTIONS, Text, TextBytes, Time, Timestamp, Vector,
};
use krummi::table::value::{ColumnType, Decimal, UuidOrder, Value, VectorElement, VectorType};
use krummi::{
    Column, Error, ErrorKind, ProjectedRow, Projection, Schema, SchemaDifference, SizeSubject,
    Store, StoreOptions, TableMode, TableRow, ValueRule,
};
use support::*;

/// A vector column type.
fn vector(element: VectorElement, dimension: u16, cosine: bool) -> ColumnType {
    ColumnType::Vector(VectorType {
        element,
        dimension,
        cosine,
    })
}

/// The little-endian bytes of `values`.
fn f32s(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// A column of every type of N4, with vectors of each element type, `uuid` in both orders and `decimal` of several
/// scales, each with a value and nullable; the key is `k1` and `k2`, of two columns.
fn every_type() -> (Schema, Vec<(&'static str, Value<'static>)>) {
    let columns: Vec<(&'static str, ColumnType, Value<'static>)> = vec![
        ("bool", ColumnType::Bool, Value::Bool(true)),
        ("i8", ColumnType::I8, Value::I8(-8)),
        ("i16", ColumnType::I16, Value::I16(-16)),
        ("i32", ColumnType::I32, Value::I32(-32)),
        ("i64", ColumnType::I64, Value::I64(-64)),
        ("u8", ColumnType::U8, Value::U8(8)),
        ("u16", ColumnType::U16, Value::U16(16)),
        ("u32", ColumnType::U32, Value::U32(32)),
        ("u64", ColumnType::U64, Value::U64(u64::MAX)),
        ("f32", ColumnType::F32, Value::F32(3.5)),
        ("f64", ColumnType::F64, Value::F64(-6.25)),
        (
            "dec2",
            ColumnType::Decimal {
                precision: 10,
                scale: 2,
            },
            Value::Decimal(Decimal {
                mantissa: -12_345,
                scale: 2,
            }),
        ),
        (
            "dec0",
            ColumnType::Decimal {
                precision: 38,
                scale: 0,
            },
            Value::Decimal(Decimal {
                mantissa: 10_i128.pow(37),
                scale: 0,
            }),
        ),
        (
            "dec9",
            ColumnType::Decimal {
                precision: 18,
                scale: 9,
            },
            Value::Decimal(Decimal {
                mantissa: 1_000_000_001,
                scale: 9,
            }),
        ),
        ("text", ColumnType::String, Value::String("texto".into())),
        (
            "bytes",
            ColumnType::Bytes,
            Value::Bytes(vec![1, 0, 2].into()),
        ),
        ("date", ColumnType::Date, Value::Date(20_000)),
        ("time", ColumnType::Time, Value::Time(3_600_000_000_000)),
        (
            "ts",
            ColumnType::Timestamp,
            Value::Timestamp(1_700_000_000_000_000_000),
        ),
        ("dur", ColumnType::Duration, Value::Duration(-5_000)),
        (
            "uuid_rfc",
            ColumnType::Uuid(UuidOrder::Rfc9562),
            Value::Uuid(Uuid([7; 16])),
        ),
        (
            "uuid_sql",
            ColumnType::Uuid(UuidOrder::SqlServer),
            Value::Uuid(Uuid([8; 16])),
        ),
        ("ulid", ColumnType::Ulid, Value::Ulid(Ulid([9; 16]))),
        (
            "v_f32",
            vector(VectorElement::F32, 3, false),
            Value::Vector(f32s(&[1.0, -2.0, 0.5]).into()),
        ),
        (
            "v_cos",
            vector(VectorElement::F32, 2, true),
            Value::Vector(f32s(&[3.0, 4.0]).into()),
        ),
        (
            "v_f16",
            vector(VectorElement::F16, 2, false),
            Value::Vector(vec![0x00, 0x3C, 0x00, 0xC0].into()),
        ),
        (
            "v_i8",
            vector(VectorElement::I8, 3, false),
            Value::Vector(
                [&[1u8, 0xFF, 7][..], &0.25f32.to_le_bytes()]
                    .concat()
                    .into(),
            ),
        ),
        (
            "v_bit",
            vector(VectorElement::Bit, 16, false),
            Value::Vector(vec![0b1010_0101, 0xFF].into()),
        ),
    ];
    let mut schema = vec![
        Column::new("k1", ColumnType::I64).not_null(),
        Column::new("k2", ColumnType::String).not_null(),
    ];
    schema.extend(columns.iter().map(|(name, ty, _)| Column::new(*name, *ty)));
    let values = columns
        .into_iter()
        .map(|(name, _, value)| (name, value))
        .collect();
    (Schema::new(schema, &["k1", "k2"]), values)
}

/// The rows of [`every_type`]: 1 with every value, 2 with nulls, 3 with every value again.
fn every_type_rows(
    values: &[(&'static str, Value<'static>)],
) -> Vec<Vec<(&'static str, Value<'static>)>> {
    (1..=3)
        .map(|k| {
            let mut row = vec![
                ("k1", Value::I64(k)),
                ("k2", Value::String(format!("k{k}").into())),
            ];
            if k != 2 {
                row.extend(values.iter().cloned());
            }
            row
        })
        .collect()
}

/// The value of the column at `at` of `projection`, of type `ty`, in `row`, read with a prepared column of the type
/// that reads `ty`, as a [`Value`]; [`Value::Null`] for a null.
fn read<'t>(
    projection: &Projection<'_>,
    row: &ProjectedRow<'_, 't>,
    at: usize,
    ty: ColumnType,
) -> Value<'t> {
    macro_rules! get {
        ($read:ty, $variant:ident) => {
            row.get(&projection.column::<$read>(at).unwrap())
                .map(Value::$variant)
        };
    }
    match ty {
        ColumnType::Bool => get!(bool, Bool),
        ColumnType::I8 => get!(i8, I8),
        ColumnType::I16 => get!(i16, I16),
        ColumnType::I32 => get!(i32, I32),
        ColumnType::I64 => get!(i64, I64),
        ColumnType::U8 => get!(u8, U8),
        ColumnType::U16 => get!(u16, U16),
        ColumnType::U32 => get!(u32, U32),
        ColumnType::U64 => get!(u64, U64),
        ColumnType::F32 => get!(f32, F32),
        ColumnType::F64 => get!(f64, F64),
        ColumnType::Decimal { .. } => get!(Decimal, Decimal),
        ColumnType::String => get!(Text, String),
        ColumnType::Bytes => get!(Bytes, Bytes),
        ColumnType::Date => get!(Date, Date),
        ColumnType::Time => get!(Time, Time),
        ColumnType::Timestamp => get!(Timestamp, Timestamp),
        ColumnType::Duration => get!(Duration, Duration),
        ColumnType::Uuid(_) => get!(Uuid, Uuid),
        ColumnType::Ulid => get!(Ulid, Ulid),
        ColumnType::Vector(_) => get!(Vector, Vector),
    }
    .unwrap_or(Value::Null)
}

/// The text, bytes or vector a value borrows, if it borrows one.
fn borrowed<'v>(value: &'v Value<'_>) -> Option<&'v [u8]> {
    match value {
        Value::String(Cow::Borrowed(text)) => Some(text.as_bytes()),
        Value::Bytes(Cow::Borrowed(bytes)) | Value::Vector(Cow::Borrowed(bytes)) => Some(bytes),
        _ => None,
    }
}

/// AC-PY1 and AC-PY2 on `row`: each column of `projection`, whose live columns in schema order are `schema`, gives
/// what reading the column by its position gives, null included, borrowing the same text, bytes and vectors; and a
/// `string` column prepared as bytes gives its bytes.
fn reads_as_by_position(
    projection: &Projection<'_>,
    names: &[&str],
    schema: &[(String, ColumnType)],
    row: &TableRow<'_>,
) {
    let bound = projection.bind(row).unwrap();
    for (at, name) in names.iter().enumerate() {
        let position = schema.iter().position(|(n, _)| n == name).unwrap();
        let by_position = row.value(position).unwrap();
        let projected = read(projection, &bound, at, schema[position].1);
        assert_eq!(projected, by_position, "column {name}");
        if let Some(bytes) = borrowed(&by_position) {
            let projected = borrowed(&projected).expect("borrowed as by its position");
            assert_eq!(projected.as_ptr(), bytes.as_ptr(), "column {name}: no copy");
        }
        if schema[position].1 == ColumnType::String {
            let as_bytes = bound.get(&projection.column::<TextBytes>(at).unwrap());
            let text = row.text(position);
            assert_eq!(
                as_bytes.as_deref(),
                text.as_deref().map(str::as_bytes),
                "column {name} as bytes"
            );
        }
    }
}

/// The live columns of `table` in `tx`, a read or a write transaction, with their types, in schema order.
macro_rules! live_columns {
    ($tx:expr, $table:expr) => {
        $tx.table_schema(&$table)
            .unwrap()
            .columns()
            .iter()
            .map(|c| (c.name.clone(), c.ty))
            .collect::<Vec<_>>()
    };
}

/// Reads every row of `$table` in `$tx` by its key and scanning forwards and backwards, and checks each with
/// [`reads_as_by_position`] for each of `$lists`.
macro_rules! check_lists {
    ($tx:expr, $table:expr, $lists:expr) => {{
        let schema = live_columns!($tx, $table);
        for names in $lists {
            let projection = $tx.projection(&$table, names).unwrap();
            let rows: Vec<_> = $tx
                .range_rows(&$table, &[], ..)
                .unwrap()
                .map(Result::unwrap)
                .collect();
            assert!(!rows.is_empty());
            for row in &rows {
                reads_as_by_position(&projection, names, &schema, row);
                let key = [row.value(0).unwrap(), row.value(1).unwrap()];
                let by_key = $tx.get_row(&$table, &key).unwrap().unwrap();
                reads_as_by_position(&projection, names, &schema, &by_key);
            }
            for row in $tx.range_rows(&$table, &[], ..).unwrap().rev() {
                reads_as_by_position(&projection, names, &schema, &row.unwrap());
            }
            let mut lent = $tx.range_rows(&$table, &[], ..).unwrap();
            while let Some(row) = lent.next_row().unwrap() {
                reads_as_by_position(&projection, names, &schema, row);
            }
        }
    }};
}

/// AC-PY1: every type, nulls, a key of two columns, lists in another order, of one column, of the key only and of some
/// columns, by key and in scans both ways, in a read transaction and in a write one that changed rows before.
#[test]
fn ac_py1_each_prepared_column_reads_as_by_its_position() {
    stores(|store, _| {
        let (schema, values) = every_type();
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .unwrap();
        for row in every_type_rows(&values) {
            write.insert_row(&table, &row).unwrap();
        }
        write.commit().unwrap();
        let mut all: Vec<&str> = values.iter().map(|(name, _)| *name).collect();
        all.extend(["k2", "k1"]);
        all.reverse();
        let lists: [&[&str]; 4] = [
            &all,
            &["text"],
            &["k2", "k1"],
            &["f64", "bytes", "k1", "v_cos", "uuid_sql"],
        ];

        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        check_lists!(read, table, lists);
        drop(read);

        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let mut changed = every_type_rows(&values).remove(0);
        changed[0] = ("k1", Value::I64(4));
        write.insert_row(&table, &changed).unwrap();
        write
            .update_row(
                &table,
                &[
                    ("k1", Value::I64(3)),
                    ("k2", Value::String("k3".into())),
                    ("text", Value::Null),
                ],
            )
            .unwrap();
        check_lists!(write, table, lists);
        write.commit().unwrap();
    });
}

/// AC-PY2: rows written before adding a column with a default, before adding a nullable one (and a text default the
/// rows borrow from the catalog) and before dropping one read with a projection of the current schema the values of
/// their columns by position, the added columns their default or null.
#[test]
fn ac_py2_older_rows_read_their_defaults_and_nulls() {
    stores(|store, _| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("a", ColumnType::String),
                Column::new("gone", ColumnType::I32),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .unwrap();
        let row = |id: i64| {
            [
                ("id", Value::I64(id)),
                ("a", Value::String(format!("a{id}").into())),
            ]
        };
        write.insert_row(&table, &row(1)).unwrap();
        write
            .add_column(
                &table,
                &Column::new("n", ColumnType::I32)
                    .not_null()
                    .with_default(Value::I32(7)),
            )
            .unwrap();
        write.insert_row(&table, &row(2)).unwrap();
        write
            .add_column(
                &table,
                &Column::new("s", ColumnType::String).with_default(Value::String("dflt".into())),
            )
            .unwrap();
        write
            .add_column(&table, &Column::new("z", ColumnType::F64))
            .unwrap();
        write.insert_row(&table, &row(3)).unwrap();
        write
            .insert_row(&table, &[("id", Value::I64(5)), ("z", Value::F64(2.5))])
            .unwrap();
        write.drop_column(&table, "gone").unwrap();
        write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(4)),
                    ("n", Value::I32(9)),
                    ("z", Value::F64(1.5)),
                ],
            )
            .unwrap();
        write.insert_row(&table, &[("id", Value::I64(6))]).unwrap();
        write.commit().unwrap();

        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let schema = live_columns!(read, table);
        let names = ["z", "s", "n", "a", "id"];
        let projection = read.projection(&table, &names).unwrap();
        let mut seen = 0;
        for row in read.range_rows(&table, &[], ..).unwrap() {
            let row = row.unwrap();
            reads_as_by_position(&projection, &names, &schema, &row);
            seen += 1;
        }
        assert_eq!(seen, 6);
        let old = read.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
        let bound = projection.bind(&old).unwrap();
        assert_eq!(bound.get(&projection.column::<i32>(2).unwrap()), Some(7));
        assert_eq!(
            bound.get(&projection.column::<Text>(1).unwrap()).as_deref(),
            Some("dflt")
        );
        assert_eq!(bound.get(&projection.column::<f64>(0).unwrap()), None);
        // The nullable fixed-width `z`, null and not null, in a row of an older version (3 and 5) and of the current
        // one (6 and 4), read from its byte range and null bit (R3-002 of the second RDD review).
        let z = projection.column::<f64>(0).unwrap();
        for (id, expected) in [(3, None), (5, Some(2.5)), (6, None), (4, Some(1.5))] {
            let row = read.get_row(&table, &[Value::I64(id)]).unwrap().unwrap();
            assert_eq!(projection.bind(&row).unwrap().get(&z), expected, "row {id}");
        }
    });
}

/// `id i64 not null, name string, n i64`, primary key `id`, with the rows 1 to 3, committed.
fn simple(store: &Store, table: &str) {
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String),
            Column::new("n", ColumnType::I64),
        ],
        &["id"],
    );
    let mut write = store.write().unwrap();
    let handle = write
        .open_table_with(table, TableMode::CreateNew(&schema))
        .unwrap();
    for id in 1..=3 {
        write
            .insert_row(
                &handle,
                &[
                    ("id", Value::I64(id)),
                    ("name", Value::String(format!("n{id}").into())),
                    ("n", Value::I64(id * 10)),
                ],
            )
            .unwrap();
    }
    write.commit().unwrap();
}

#[track_caller]
fn invalid_size(error: Error, folder: Option<&Path>) -> u64 {
    match error {
        Error::InvalidSize {
            folder: f,
            what,
            received,
            min,
            max,
        } => {
            assert_eq!(f, folder_of(folder));
            assert_eq!((what, min, max), (SizeSubject::ColumnName, 1, 1_024));
            received
        }
        other => panic!("expected an invalid size, got {other}"),
    }
}

#[track_caller]
fn different(
    error: Error,
    folder: Option<&Path>,
) -> (String, Option<String>, SchemaDifference, String, String) {
    match error {
        Error::DifferentSchema {
            folder: f,
            table,
            column,
            difference,
            expected,
            found,
        } => {
            assert_eq!(f, folder_of(folder));
            (
                table,
                column,
                difference,
                expected.to_string(),
                found.to_string(),
            )
        }
        other => panic!("expected a different schema, got {other}"),
    }
}

#[track_caller]
fn invalid_value(
    error: Error,
    folder: Option<&Path>,
) -> (Option<String>, Option<String>, ValueRule) {
    match error {
        Error::InvalidValue {
            folder: f,
            table,
            column,
            sequence,
            rule,
        } => {
            assert_eq!(f, folder_of(folder));
            assert_eq!(sequence, None);
            (table, column, rule)
        }
        other => panic!("expected an invalid value, got {other}"),
    }
}

/// AC-PY3: each failure of preparing, its data and their precedence; a name of 1,024 bytes; the write in which they
/// fail commits as if they had not happened, a write that only prepares changes nothing, and a row read before
/// preparing is still valid after.
#[test]
fn ac_py3_preparing_fails_with_the_first_rule_in_order() {
    stores(|store, folder| {
        simple(store, "t");
        let long = "c".repeat(1_024);
        let mut write = store.write().unwrap();
        write
            .open_table_with(
                "long",
                TableMode::CreateNew(&Schema::new(
                    vec![Column::new(&long, ColumnType::I64).not_null()],
                    &[&long],
                )),
            )
            .unwrap();
        write
            .add_column(
                &write.open_table("t").unwrap(),
                &Column::new("dropped", ColumnType::I32),
            )
            .unwrap();
        write
            .drop_column(&write.open_table("t").unwrap(), "dropped")
            .unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let mut late = store.write().unwrap();
        late.add_column(
            &late.open_table("t").unwrap(),
            &Column::new("late", ColumnType::I32),
        )
        .unwrap();
        late.commit().unwrap();

        let too_long = "x".repeat(1_025);
        let fails = |columns: &[&str]| read.projection(&table, columns).unwrap_err();
        assert_eq!(invalid_size(fails(&["id", ""]), folder), 0);
        assert_eq!(invalid_size(fails(&[too_long.as_str()]), folder), 1_025);
        assert_eq!(
            invalid_size(fails(&["", too_long.as_str()]), folder),
            0,
            "the first in the list"
        );
        for name in ["nope", "dropped", "late"] {
            let (t, column, difference, _, _) = different(fails(&["id", name]), folder);
            assert_eq!(
                (t.as_str(), column.as_deref(), difference),
                ("t", Some(name), SchemaDifference::MissingColumn)
            );
        }
        let (_, column, ..) = different(fails(&["a", "id", "b"]), folder);
        assert_eq!(column.as_deref(), Some("a"), "the first unknown");
        let (_, column, ..) = different(fails(&["id", "id", "nope"]), folder);
        assert_eq!(column.as_deref(), Some("nope"), "unknown before repeated");
        assert_eq!(
            invalid_value(fails(&["n", "id", "n"]), folder),
            (
                Some("t".to_owned()),
                Some("n".to_owned()),
                ValueRule::RepeatedColumn
            )
        );
        assert_eq!(
            invalid_value(fails(&[]), folder),
            (Some("t".to_owned()), None, ValueRule::EmptyProjection)
        );
        let read_long = read.open_table("long").unwrap();
        assert!(
            read.projection(&read_long, &[long.as_str()]).is_ok(),
            "1,024 bytes"
        );
        drop(read);

        let before = store.info();
        let write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let row = write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
        assert!(
            write
                .projection(&table, &["n", "name"])
                .unwrap()
                .bind(&row)
                .is_ok()
        );
        assert_eq!(row.i64(2), Some(10), "a row read before preparing");
        assert!(write.projection(&table, &["nope"]).is_err());
        write.commit().unwrap();
        assert_eq!(
            store.info(),
            before,
            "a write that only prepares changes nothing"
        );

        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.delete_table("t").unwrap();
        let gone = |columns: &[&str]| write.projection(&table, columns).unwrap_err();
        assert_eq!(gone(&["id"]).kind(), ErrorKind::TreeNotFound);
        assert_eq!(
            invalid_size(gone(&["nope", ""]), folder),
            0,
            "a long name before a deleted table"
        );
        assert_eq!(
            gone(&["nope"]).kind(),
            ErrorKind::TreeNotFound,
            "a deleted table before an unknown name"
        );
        assert_eq!(gone(&[]).kind(), ErrorKind::TreeNotFound);
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(
            read.open_table("t").err().unwrap().kind(),
            ErrorKind::TreeNotFound
        );
    });
}

/// AC-PY4: a prepared column of a type of another class fails with the table, the column and the difference, and the
/// write commits after; with its type it is obtained; past the list it panics.
#[test]
fn ac_py4_a_prepared_column_of_another_type_fails() {
    stores(|store, folder| {
        simple(store, "t");
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write
            .add_column(
                &table,
                &Column::new(
                    "d",
                    ColumnType::Decimal {
                        precision: 10,
                        scale: 2,
                    },
                ),
            )
            .unwrap();
        let projection = write.projection(&table, &["name", "n", "d"]).unwrap();
        let wrong = [
            (
                projection.column::<i64>(0).unwrap_err(),
                "name",
                "i64",
                ColumnType::String,
            ),
            (
                projection.column::<Text>(1).unwrap_err(),
                "n",
                "string",
                ColumnType::I64,
            ),
            (
                projection.column::<f64>(2).unwrap_err(),
                "d",
                "f64",
                ColumnType::Decimal {
                    precision: 10,
                    scale: 2,
                },
            ),
        ];
        for (error, column, expected, found) in wrong {
            let (t, c, difference, e, f) = different(error, folder);
            assert_eq!(
                (t.as_str(), c.as_deref(), difference),
                ("t", Some(column), SchemaDifference::Type)
            );
            assert_eq!((e.as_str(), f), (expected, found.to_string()));
        }
        assert!(projection.column::<Text>(0).is_ok());
        assert!(projection.column::<TextBytes>(0).is_ok());
        assert!(projection.column::<Decimal>(2).is_ok());
        assert!(catch_unwind(AssertUnwindSafe(|| projection.column::<i64>(3))).is_err());
        write.insert_row(&table, &[("id", Value::I64(9))]).unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert!(read.get_row(&table, &[Value::I64(9)]).unwrap().is_some());
    });
}

/// Whether `error` is the failure of binding a row of another table, store or schema to a projection of `table`.
#[track_caller]
fn wrong_schema(error: Error, table: &str) {
    match error {
        Error::DifferentSchema {
            table: t,
            column,
            difference,
            ..
        } => {
            assert_eq!(
                (t.as_str(), column, difference),
                (table, None, SchemaDifference::Projection)
            );
        }
        other => panic!("expected a different schema, got {other}"),
    }
}

/// AC-PY5: rows of another table with the same columns, of a table recreated with the same name, of another store
/// opened from a copy of the files, and of another schema of the table (in another write and in the same one, both
/// ways) fail to bind, and the write commits after; a projection of each row's schema reads it; renaming and emptying
/// the table keep the projection, whose errors carry its name at preparation; an abandoned write's projection does
/// not bind the rows of another write at the same version number with another schema.
#[test]
fn ac_py5_rows_of_another_table_store_or_schema_do_not_bind() {
    let tmp = tempfile::tempdir().unwrap();
    let (first, copy) = (tmp.path().join("a"), tmp.path().join("b"));
    let store = open(&first);
    simple(&store, "t");
    simple(&store, "u");
    store.close().unwrap();
    std::fs::create_dir(&copy).unwrap();
    for entry in std::fs::read_dir(&first).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), copy.join(entry.file_name())).unwrap();
    }
    let (store, other) = (open(&first), open(&copy));

    let read = store.read().unwrap();
    let t = read.open_table("t").unwrap();
    let projection = read.projection(&t, &["n", "id"]).unwrap();
    let u = read.open_table("u").unwrap();
    let u_row = read.get_row(&u, &[Value::I64(1)]).unwrap().unwrap();
    wrong_schema(projection.bind(&u_row).unwrap_err(), "t");
    drop(u_row);
    let other_read = other.read().unwrap();
    let other_t = other_read.open_table("t").unwrap();
    let copied = other_read
        .get_row(&other_t, &[Value::I64(1)])
        .unwrap()
        .unwrap();
    wrong_schema(projection.bind(&copied).unwrap_err(), "t");
    drop(copied);
    drop(other_read);

    // Another write changes the schema: neither the old projection binds its rows nor the new one the old rows.
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .add_column(&table, &Column::new("x", ColumnType::I32))
        .unwrap();
    write.commit().unwrap();
    let after = store.read().unwrap();
    let after_t = after.open_table("t").unwrap();
    let new_row = after.get_row(&after_t, &[Value::I64(1)]).unwrap().unwrap();
    wrong_schema(projection.bind(&new_row).unwrap_err(), "t");
    let new = after.projection(&after_t, &["n", "id"]).unwrap();
    assert_eq!(
        new.bind(&new_row)
            .unwrap()
            .get(&new.column::<i64>(0).unwrap()),
        Some(10)
    );
    let old_row = read.get_row(&t, &[Value::I64(1)]).unwrap().unwrap();
    wrong_schema(new.bind(&old_row).unwrap_err(), "t");
    assert_eq!(
        projection
            .bind(&old_row)
            .unwrap()
            .get(&projection.column::<i64>(1).unwrap()),
        Some(1)
    );
    drop(new_row);
    drop(old_row);
    drop(after);
    drop(read);

    // In the same write; then the write commits.
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let before = write.projection(&table, &["id", "name"]).unwrap();
    write
        .add_column(&table, &Column::new("y", ColumnType::I32))
        .unwrap();
    let row = write.get_row(&table, &[Value::I64(2)]).unwrap().unwrap();
    wrong_schema(before.bind(&row).unwrap_err(), "t");
    let now = write.projection(&table, &["id", "name"]).unwrap();
    assert_eq!(
        now.bind(&row)
            .unwrap()
            .get(&now.column::<Text>(1).unwrap())
            .as_deref(),
        Some("n2")
    );
    drop(row);
    write.insert_row(&table, &[("id", Value::I64(7))]).unwrap();
    write.commit().unwrap();

    // Renaming and emptying keep the projection; its errors keep the name it was prepared with.
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let projection = write.projection(&table, &["name", "id"]).unwrap();
    write.rename_table("t", "renamed", false).unwrap();
    let row = write.get_row(&table, &[Value::I64(3)]).unwrap().unwrap();
    assert_eq!(
        projection
            .bind(&row)
            .unwrap()
            .get(&projection.column::<Text>(0).unwrap())
            .as_deref(),
        Some("n3")
    );
    let (t, ..) = different(projection.column::<i64>(0).unwrap_err(), Some(&first));
    assert_eq!(t, "t");
    // The same list prepared again after the rename is the kept projection, with the name of this preparation in its
    // errors (R3-001 of the first RDD review).
    let again = write.projection(&table, &["name", "id"]).unwrap();
    assert!(again.same_as(&projection));
    let (t, ..) = different(again.column::<i64>(0).unwrap_err(), Some(&first));
    assert_eq!(t, "renamed");
    drop(row);
    let emptied = write.open_table_with("renamed", TableMode::Empty).unwrap();
    write
        .insert_row(
            &emptied,
            &[
                ("id", Value::I64(5)),
                ("name", Value::String("five".into())),
            ],
        )
        .unwrap();
    let row = write.get_row(&emptied, &[Value::I64(5)]).unwrap().unwrap();
    assert_eq!(
        projection
            .bind(&row)
            .unwrap()
            .get(&projection.column::<Text>(0).unwrap())
            .as_deref(),
        Some("five")
    );
    drop(row);
    // A table created with the same name after deleting it is another table.
    write.delete_table("renamed").unwrap();
    simple_in(&mut write, "renamed");
    let recreated = write.open_table("renamed").unwrap();
    let row = write
        .get_row(&recreated, &[Value::I64(1)])
        .unwrap()
        .unwrap();
    wrong_schema(projection.bind(&row).unwrap_err(), "t");
    drop(row);
    write.commit().unwrap();

    // An abandoned write's projection, and another write that reaches the same version with another schema.
    let mut abandoned = store.write().unwrap();
    let table = abandoned.open_table("renamed").unwrap();
    abandoned
        .add_column(&table, &Column::new("a", ColumnType::I32))
        .unwrap();
    let lost = abandoned.projection(&table, &["id"]).unwrap();
    let lost_version = abandoned.table_schema(&table).unwrap().version();
    drop(abandoned);
    let mut write = store.write().unwrap();
    let table = write.open_table("renamed").unwrap();
    write
        .add_column(&table, &Column::new("b", ColumnType::I64))
        .unwrap();
    assert_eq!(write.table_schema(&table).unwrap().version(), lost_version);
    let row = write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
    wrong_schema(lost.bind(&row).unwrap_err(), "renamed");
    drop(row);
    write.commit().unwrap();
    drop(projection);
    drop(lost);
    store.close().unwrap();
    other.close().unwrap();
}

/// An identity source that gives `values` in order.
fn identities(values: Vec<u64>) -> impl FnMut(&mut [u8]) -> krummi::Result<()> {
    let mut next = values.into_iter();
    move |out| {
        out.copy_from_slice(
            &next
                .next()
                .expect("identity source exhausted")
                .to_le_bytes(),
        );
        Ok(())
    }
}

/// AC-PY5 and PY1 with the generator forced to offer an identity again (R2 of the Codex code review): the store refuses
/// an abandoned schema change's state for the next change of the table, the identity of a table it deleted (read from
/// the files of a reopened store) and the identity of a table created in an abandoned write for the next table created
/// with the name, and draws the next value; so no schema binds another's rows or shares its kept projection.
#[test]
fn ac_py5_a_repeated_identity_never_aliases_a_schema() {
    use krummi::table::testing::with_identity_source;
    let tmp = tempfile::tempdir().unwrap();
    let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);

    // An abandoned write's schema state, 40, offered again for the next write's schema change: 60 is drawn instead.
    let store = open(&tmp.path().join("t"));
    let mut seed = store.write().unwrap();
    with_identity_source(identities(vec![10, 20, 30]), || {
        seed.open_table_with("t", TableMode::CreateNew(&schema))
    })
    .unwrap();
    seed.commit().unwrap();
    let mut abandoned = store.write().unwrap();
    let t = abandoned.open_table("t").unwrap();
    with_identity_source(identities(vec![40, 50]), || {
        abandoned.add_column(&t, &Column::new("a", ColumnType::I64))
    })
    .unwrap();
    let old = abandoned.projection(&t, &["a"]).unwrap();
    drop(abandoned);
    let mut write = store.write().unwrap();
    let t = write.open_table("t").unwrap();
    with_identity_source(identities(vec![40, 60, 70]), || {
        write.add_column(&t, &Column::new("b", ColumnType::I64))
    })
    .unwrap();
    assert_eq!(write.table_schema(&t).unwrap().schema_state(), 60);
    write
        .insert_row(&t, &[("id", Value::I64(1)), ("b", Value::I64(77))])
        .unwrap();
    let row = write.get_row(&t, &[Value::I64(1)]).unwrap().unwrap();
    wrong_schema(old.bind(&row).unwrap_err(), "t");
    let new = write.projection(&t, &["b"]).unwrap();
    assert!(
        !new.same_as(&old),
        "the new schema shares the abandoned one's kept projection"
    );
    let b = new.column::<i64>(0).unwrap();
    assert_eq!(new.bind(&row).unwrap().get(&b), Some(77));
    drop(row);
    write.commit().unwrap();
    store.close().unwrap();

    // A deleted table's identity, 100, read from the files, offered again first for the table created with its name:
    // 110 is drawn instead (a new table's state may repeat another table's, D6, so only its identity matters).
    let folder = tmp.path().join("u");
    let store = open(&folder);
    let mut write = store.write().unwrap();
    with_identity_source(identities(vec![100, 110, 120]), || {
        write.open_table_with("u", TableMode::CreateNew(&schema))
    })
    .unwrap();
    write.commit().unwrap();
    store.close().unwrap();
    let store = open(&folder);
    let read = store.read().unwrap();
    let old = read
        .projection(&read.open_table("u").unwrap(), &["id"])
        .unwrap();
    drop(read);
    let mut write = store.write().unwrap();
    write.delete_table("u").unwrap();
    write.commit().unwrap();
    let mut write = store.write().unwrap();
    let u = with_identity_source(identities(vec![100, 110, 120, 130]), || {
        write.open_table_with("u", TableMode::CreateNew(&schema))
    })
    .unwrap();
    let described = write.table_schema(&u).unwrap();
    assert_eq!((described.table_id(), described.schema_state()), (110, 130));
    write.insert_row(&u, &[("id", Value::I64(1))]).unwrap();
    let row = write.get_row(&u, &[Value::I64(1)]).unwrap().unwrap();
    wrong_schema(old.bind(&row).unwrap_err(), "u");
    let new = write.projection(&u, &["id"]).unwrap();
    assert!(
        !new.same_as(&old),
        "the new table shares the deleted one's kept projection"
    );
    drop(row);
    write.commit().unwrap();

    // A table created in an abandoned write, with identity 200 and state 220, offered again for the next table
    // created with its name: 210 is drawn instead.
    let mut abandoned = store.write().unwrap();
    let v = with_identity_source(identities(vec![200, 210, 220]), || {
        abandoned.open_table_with("v", TableMode::CreateNew(&schema))
    })
    .unwrap();
    let old = abandoned.projection(&v, &["id"]).unwrap();
    drop(abandoned);
    let mut write = store.write().unwrap();
    let v = with_identity_source(identities(vec![200, 210, 220, 230]), || {
        write.open_table_with("v", TableMode::CreateNew(&schema))
    })
    .unwrap();
    let described = write.table_schema(&v).unwrap();
    assert_eq!((described.table_id(), described.schema_state()), (210, 230));
    write.insert_row(&v, &[("id", Value::I64(1))]).unwrap();
    let row = write.get_row(&v, &[Value::I64(1)]).unwrap().unwrap();
    wrong_schema(old.bind(&row).unwrap_err(), "v");
    let new = write.projection(&v, &["id"]).unwrap();
    assert!(
        !new.same_as(&old),
        "the new table shares the abandoned one's kept projection"
    );
    drop(row);
    write.commit().unwrap();
    store.close().unwrap();
}

/// The table `name` of [`simple`] with only the row 1, created in `write`.
fn simple_in(write: &mut krummi::WriteTransaction<'_>, name: &str) {
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String),
            Column::new("n", ColumnType::I64),
        ],
        &["id"],
    );
    let table = write
        .open_table_with(name, TableMode::CreateNew(&schema))
        .unwrap();
    write.insert_row(&table, &[("id", Value::I64(1))]).unwrap();
}

/// Whether `read` panics.
fn panics<T>(read: impl FnOnce() -> T) -> bool {
    catch_unwind(AssertUnwindSafe(read)).is_err()
}

/// AC-PY6: a prepared column of another projection of the same table, or of the same columns in another order,
/// panics when it reads a bound row, in a read and in a write transaction; captured around the read the transaction
/// is intact, and through it, it ends or is discarded as in AC-L5.
#[test]
fn ac_py6_a_column_of_another_projection_panics() {
    stores(|store, _| {
        simple(store, "t");
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let projection = read.projection(&table, &["id", "n"]).unwrap();
        let other = read
            .projection(&table, &["n"])
            .unwrap()
            .column::<i64>(0)
            .unwrap();
        let reordered = read
            .projection(&table, &["n", "id"])
            .unwrap()
            .column::<i64>(1)
            .unwrap();
        let row = read.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
        let bound = projection.bind(&row).unwrap();
        assert!(panics(|| bound.get(&other)));
        assert!(panics(|| bound.get(&reordered)));
        assert_eq!(
            bound.get(&projection.column::<i64>(0).unwrap()),
            Some(1),
            "the transaction is intact"
        );
        assert_eq!(read.range_rows(&table, &[], ..).unwrap().count(), 3);
        drop(row);
        drop(read);

        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let projection = write.projection(&table, &["id", "n"]).unwrap();
        let row = write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
        assert!(panics(|| projection.bind(&row).unwrap().get(&other)));
        drop(row);
        write.insert_row(&table, &[("id", Value::I64(4))]).unwrap();
        write.commit().unwrap();

        assert!(panics(|| {
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            write.insert_row(&table, &[("id", Value::I64(5))]).unwrap();
            let projection = write.projection(&table, &["id", "n"]).unwrap();
            let row = write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
            projection.bind(&row).unwrap().get(&reordered)
        }));
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert!(read.get_row(&table, &[Value::I64(4)]).unwrap().is_some());
        assert!(
            read.get_row(&table, &[Value::I64(5)]).unwrap().is_none(),
            "nothing applied"
        );
        drop(read);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.insert_row(&table, &[("id", Value::I64(6))]).unwrap();
        write.commit().unwrap();
    });
}

/// AC-PY7: the same list in the same order over the same schema in another transaction is the kept projection, and
/// another order is another; four threads and two read transactions share it; after closing and opening the store
/// nothing is kept until it is prepared.
#[test]
fn ac_py7_the_store_keeps_and_shares_a_projection() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    simple(&store, "t");
    {
        let (first, second) = (store.read().unwrap(), store.read().unwrap());
        let (t1, t2) = (
            first.open_table("t").unwrap(),
            second.open_table("t").unwrap(),
        );
        let projection = first.projection(&t1, &["name", "n"]).unwrap();
        assert!(projection.is_kept());
        assert!(
            second
                .projection(&t2, &["name", "n"])
                .unwrap()
                .same_as(&projection)
        );
        assert!(
            !second
                .projection(&t2, &["n", "name"])
                .unwrap()
                .same_as(&projection)
        );
        let name = projection.column::<Text>(0).unwrap();
        let n = projection.column::<i64>(1).unwrap();
        std::thread::scope(|scope| {
            for thread in 0..4 {
                let (tx, table) = if thread % 2 == 0 {
                    (&first, &t1)
                } else {
                    (&second, &t2)
                };
                let (projection, name, n) = (&projection, &name, &n);
                scope.spawn(move || {
                    for _ in 0..100 {
                        for row in tx.range_rows(table, &[], ..).unwrap() {
                            let row = row.unwrap();
                            let bound = projection.bind(&row).unwrap();
                            assert_eq!(bound.get(name), row.text(1));
                            assert_eq!(bound.get(n), row.i64(2));
                        }
                    }
                });
            }
        });
        assert_eq!(store.kept_projections(), 2);
    }
    store.close().unwrap();
    let store = open(tmp.path());
    assert_eq!(store.kept_projections(), 0);
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    assert!(read.projection(&table, &["name", "n"]).unwrap().is_kept());
    assert_eq!(store.kept_projections(), 1);
}

/// AC-PY8: with 1,023 projections kept, of an abandoned write and of an earlier schema among them, of two new lists the
/// first is kept and the second is not; the second reads the same values, and a prepared column of one preparation of
/// it serves another; the 1,024 kept stay until the store closes.
#[test]
fn ac_py8_the_store_keeps_at_most_1024_projections() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    // 12 × 11 × 10 ordered lists of three columns, more than the store keeps.
    let names: Vec<String> = (0..12).map(|i| format!("c{i}")).collect();
    let mut columns = vec![Column::new("id", ColumnType::I64).not_null()];
    columns.extend(names.iter().map(|name| Column::new(name, ColumnType::I64)));
    let schema = Schema::new(columns, &["id"]);
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema))
        .unwrap();
    let mut row = vec![("id", Value::I64(1))];
    row.extend(
        names
            .iter()
            .zip(0..)
            .map(|(name, i)| (name.as_str(), Value::I64(i))),
    );
    write.insert_row(&table, &row).unwrap();
    // One kept projection of the schema before the next change, and one of an abandoned write.
    write.projection(&table, &["id"]).unwrap();
    write.commit().unwrap();
    let mut abandoned = store.write().unwrap();
    let table = abandoned.open_table("t").unwrap();
    abandoned
        .add_column(&table, &Column::new("lost", ColumnType::I32))
        .unwrap();
    assert!(abandoned.projection(&table, &["lost"]).unwrap().is_kept());
    drop(abandoned);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .add_column(&table, &Column::new("extra", ColumnType::I32))
        .unwrap();
    write.commit().unwrap();
    assert_eq!(store.kept_projections(), 2);

    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let mut lists = Vec::new();
    for a in &names {
        for b in &names {
            for c in &names {
                if a != b && b != c && a != c {
                    lists.push([a.as_str(), b.as_str(), c.as_str()]);
                }
            }
        }
    }
    let mut kept = Vec::new();
    for list in lists.into_iter().take(MAX_PROJECTIONS - 3) {
        let projection = read.projection(&table, &list).unwrap();
        assert!(projection.is_kept());
        kept.push((list, projection));
    }
    assert_eq!(store.kept_projections(), MAX_PROJECTIONS - 1);
    let last = read.projection(&table, &["c0"]).unwrap();
    assert!(last.is_kept());
    let over = read.projection(&table, &["c1"]).unwrap();
    assert!(!over.is_kept());
    let again = read.projection(&table, &["c1"]).unwrap();
    assert!(!again.is_kept() && !again.same_as(&over));
    let row = read.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
    let column = over.column::<i64>(0).unwrap();
    assert_eq!(again.bind(&row).unwrap().get(&column), Some(1));
    assert_eq!(over.bind(&row).unwrap().get(&column), row.i64(2));
    for (list, projection) in &kept {
        assert!(read.projection(&table, list).unwrap().same_as(projection));
    }
    assert!(read.projection(&table, &["c0"]).unwrap().same_as(&last));
    assert_eq!(store.kept_projections(), MAX_PROJECTIONS);
}

/// AC-PY10: a copied row, read after the next change of its transaction, gives what reading its columns by position
/// gives, and its text and bytes outlive the copied row; bound to a projection of another schema it fails.
#[test]
fn ac_py10_a_copied_row_reads_its_own_copies() {
    stores(|store, _| {
        simple(store, "t");
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let projection = write.projection(&table, &["name", "id", "n"]).unwrap();
        let copied = write
            .get_row(&table, &[Value::I64(2)])
            .unwrap()
            .unwrap()
            .into_owned();
        write.insert_row(&table, &[("id", Value::I64(8))]).unwrap();
        let name = projection.column::<Text>(0).unwrap();
        let id = projection.column::<i64>(1).unwrap();
        let bytes = projection.column::<TextBytes>(0).unwrap();
        let (text, raw) = {
            let bound = projection.bind(&copied).unwrap();
            assert_eq!(bound.get(&id), copied.i64(0));
            assert_eq!(bound.get(&name), copied.text(1));
            (bound.get(&name).unwrap(), bound.get(&bytes).unwrap())
        };
        drop(copied);
        assert_eq!(
            (&*text, &*raw),
            ("n2", &b"n2"[..]),
            "they outlive the copied row"
        );
        let copied = write
            .get_row(&table, &[Value::I64(2)])
            .unwrap()
            .unwrap()
            .into_owned();
        write
            .add_column(&table, &Column::new("z", ColumnType::I32))
            .unwrap();
        let new = write.projection(&table, &["name"]).unwrap();
        wrong_schema(new.bind(&copied).unwrap_err(), "t");
        write.commit().unwrap();
    });
}

/// AC-PY12: a store on disk written, closed and opened again reads with projections what reading by position gives;
/// the row and key formats are those of 7.1 (E20).
#[test]
fn ac_py12_a_reopened_store_reads_with_projections() {
    let tmp = tempfile::tempdir().unwrap();
    let (schema, values) = every_type();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema))
        .unwrap();
    for row in every_type_rows(&values) {
        write.insert_row(&table, &row).unwrap();
    }
    write.commit().unwrap();
    store.close().unwrap();
    let store = open(tmp.path());
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let names: Vec<&str> = ["k1", "k2"]
        .into_iter()
        .chain(values.iter().map(|(name, _)| *name))
        .collect();
    check_lists!(read, table, [&names[..]]);
}
