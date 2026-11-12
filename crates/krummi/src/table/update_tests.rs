//! Internal tests of how an update writes its row (T6 and T7 of the row reads run): whatever path it takes, the row it
//! stores has the bytes that encoding the whole merged row gives, an identical update keeps the row as it was, the rows
//! tree keeps its root, and an invalid value fails before any of it.

use crate::table::key::encode_key;
use crate::table::value::{ColumnType, Decimal, Value};
use crate::{Column, ErrorKind, Schema, Store, StoreOptions, TableMode, Upserted, ValueRule};

type Row = Vec<(&'static str, Value<'static>)>;

/// Fixed-width columns of every kind, nullable or not, and two variable ones, keyed by `id`.
fn schema() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64).not_null(),
            Column::new("small", ColumnType::I16),
            Column::new("score", ColumnType::F64),
            Column::new("flag", ColumnType::Bool),
            Column::new(
                "price",
                ColumnType::Decimal {
                    precision: 10,
                    scale: 2,
                },
            ),
            Column::new("at", ColumnType::Timestamp),
            Column::new("s", ColumnType::String),
            Column::new("b", ColumnType::Bytes).not_null(),
        ],
        &["id"],
    )
}

fn row(id: i64) -> Row {
    vec![
        ("id", Value::I64(id)),
        ("n", Value::I64(id * 10)),
        ("small", Value::I16(7)),
        ("score", Value::F64(0.5)),
        ("flag", Value::Bool(true)),
        (
            "price",
            Value::Decimal(Decimal {
                mantissa: 1_250,
                scale: 2,
            }),
        ),
        ("at", Value::Timestamp(1_000_000)),
        ("s", Value::String(format!("row{id}").into())),
        ("b", Value::Bytes(vec![1, 2, 3].into())),
    ]
}

/// Updates the row `id` with `change` and checks what it stored: the bytes of the whole merged row in the current
/// version, or, for an identical update, the row as it was; and the rows tree's root kept.
#[track_caller]
fn update_writes_the_merged_row(
    store: &Store,
    id: i64,
    change: &[(&str, Value<'_>)],
    identical: bool,
) {
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let key = encode_key(&[ColumnType::I64], &[Value::I64(id)]).unwrap();
    let names: Vec<String> = write
        .describe_table("t")
        .unwrap()
        .columns()
        .iter()
        .map(|c| c.name.clone())
        .collect();
    let old = write.get_row(&table, &[Value::I64(id)]).unwrap().unwrap();
    let mut merged: Vec<Value<'static>> = old.values().map(Value::into_owned).collect();
    for (name, value) in change {
        let i = names.iter().position(|n| n == name).unwrap();
        merged[i] = value.clone().into_owned();
    }
    let (root, _) = write.live_rows(&table).unwrap();
    let before = krummi_tree::get(&write.inner, root, &key)
        .unwrap()
        .unwrap()
        .to_vec();
    let mut given: Vec<(&str, Value<'_>)> = vec![("id", Value::I64(id))];
    given.extend(change.iter().map(|(n, v)| (*n, v.clone())));
    // Even rows are updated, odd ones inserted or updated: both take the same paths.
    if id % 2 == 0 {
        assert!(write.update_row(&table, &given).unwrap());
    } else {
        assert_eq!(write.upsert_row(&table, &given).unwrap(), Upserted::Updated);
    }
    let (after_root, _) = write.live_rows(&table).unwrap();
    assert_eq!(after_root, root, "{change:?}: the root");
    let stored = krummi_tree::get(&write.inner, root, &key)
        .unwrap()
        .unwrap()
        .to_vec();
    let expected = if identical {
        before
    } else {
        write.describe_table("t").unwrap().layout().encode(&merged)
    };
    assert_eq!(stored, expected, "{change:?}: the stored row");
    write.commit().unwrap();
    // A new read finds the row the commit stored.
    let read = store.read().unwrap();
    let rows = read
        .table_trees(&read.open_table("t").unwrap())
        .unwrap()
        .rows;
    assert_eq!(
        krummi_tree::get(&read.inner, rows, &key).unwrap().unwrap(),
        &expected[..],
        "{change:?}: after the commit"
    );
}

#[test]
fn an_update_stores_the_bytes_of_the_whole_merged_row() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema()))
        .unwrap();
    for id in 0..50 {
        write.insert_row(&table, &row(id)).unwrap();
    }
    write.commit().unwrap();
    let dec = |mantissa| Value::Decimal(Decimal { mantissa, scale: 2 });
    // Fixed-width columns: values, nulls set and cleared, several at once.
    update_writes_the_merged_row(&store, 3, &[("n", Value::I64(-1))], false);
    update_writes_the_merged_row(&store, 3, &[("n", Value::I64(-1))], true);
    update_writes_the_merged_row(&store, 4, &[("small", Value::Null)], false);
    update_writes_the_merged_row(&store, 4, &[("small", Value::I16(-3))], false);
    update_writes_the_merged_row(
        &store,
        5,
        &[("score", Value::Null), ("flag", Value::Null)],
        false,
    );
    update_writes_the_merged_row(
        &store,
        5,
        &[("score", Value::F64(2.0)), ("flag", Value::Bool(false))],
        false,
    );
    update_writes_the_merged_row(
        &store,
        6,
        &[("price", dec(99_999)), ("at", Value::Timestamp(-5_000))],
        false,
    );
    update_writes_the_merged_row(
        &store,
        6,
        &[("price", dec(99_999)), ("n", Value::I64(60))],
        true,
    );
    // Variable columns, and a fixed one with them.
    update_writes_the_merged_row(
        &store,
        7,
        &[("s", Value::String("a longer text".into()))],
        false,
    );
    update_writes_the_merged_row(
        &store,
        7,
        &[("s", Value::Null), ("n", Value::I64(1))],
        false,
    );
    update_writes_the_merged_row(&store, 8, &[("b", Value::Bytes(vec![9].into()))], false);
    // A row of an older schema version, after a column is added.
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .add_column(&table, &Column::new("extra", ColumnType::I32))
        .unwrap();
    write.commit().unwrap();
    update_writes_the_merged_row(&store, 9, &[("n", Value::I64(90))], true);
    update_writes_the_merged_row(&store, 9, &[("n", Value::I64(91))], false);
    update_writes_the_merged_row(&store, 9, &[("extra", Value::I32(5))], false);
    update_writes_the_merged_row(
        &store,
        9,
        &[("extra", Value::I32(6)), ("n", Value::I64(92))],
        false,
    );
}

/// A value that breaks its column's rules fails the update before the row is read, as before the patch of fixed-width
/// columns: a null in a column without nulls and a value of another type fail with invalid value and their rule, the
/// row stays, and the transaction goes on and commits (B7).
#[test]
fn an_update_with_an_invalid_fixed_width_value_fails_before_the_patch() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema()))
        .unwrap();
    write.insert_row(&table, &row(1)).unwrap();
    write.commit().unwrap();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    for (value, rule) in [
        (Value::Null, ValueRule::NullNotAllowed),
        (Value::I32(5), ValueRule::WrongType),
    ] {
        let given = [("id", Value::I64(1)), ("n", value.clone())];
        for upsert in [false, true] {
            let error = if upsert {
                write.upsert_row(&table, &given).map(drop).unwrap_err()
            } else {
                write.update_row(&table, &given).map(drop).unwrap_err()
            };
            assert_eq!(
                error.kind(),
                ErrorKind::InvalidValue,
                "{value:?}, upsert {upsert}"
            );
            assert!(
                matches!(error, crate::Error::InvalidValue { rule: r, .. } if r == rule),
                "{error:?}"
            );
        }
    }
    let kept = write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
    assert_eq!(kept.i64(1), Some(10));
    drop(kept);
    write.insert_row(&table, &row(2)).unwrap();
    write.commit().unwrap();
}

/// An update of a table whose primary key has many columns (R1 of the Codex review of T6 and T7): it stores the merged
/// row, by update and by upsert, finding each given key column in constant time.
#[test]
fn an_update_under_a_wide_compound_key_stores_the_merged_row() {
    let keys = 300;
    let names: Vec<String> = (0..keys).map(|k| format!("k{k}")).collect();
    let mut columns: Vec<Column> = names
        .iter()
        .map(|name| Column::new(name.as_str(), ColumnType::Bool).not_null())
        .collect();
    columns.push(Column::new("n", ColumnType::I64));
    let key_names: Vec<&str> = names.iter().map(String::as_str).collect();
    let schema = Schema::new(columns, &key_names);
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema))
        .unwrap();
    let key_values: Vec<(&str, Value<'_>)> = key_names
        .iter()
        .enumerate()
        .map(|(k, name)| (*name, Value::Bool(k % 3 == 0)))
        .collect();
    let with_n = |n: i64| {
        let mut row = key_values.clone();
        row.push(("n", Value::I64(n)));
        row
    };
    write.insert_row(&table, &with_n(1)).unwrap();
    assert!(write.update_row(&table, &with_n(2)).unwrap());
    assert_eq!(
        write.upsert_row(&table, &with_n(3)).unwrap(),
        Upserted::Updated
    );
    let key: Vec<Value<'_>> = key_values.iter().map(|(_, v)| v.clone()).collect();
    let row = write.get_row(&table, &key).unwrap().unwrap();
    assert_eq!(row.i64(keys), Some(3));
    let mut expected: Vec<Value<'_>> = key.clone();
    expected.push(Value::I64(3));
    assert_eq!(row.values().collect::<Vec<_>>(), expected);
    drop(row);
    write.commit().unwrap();
}
