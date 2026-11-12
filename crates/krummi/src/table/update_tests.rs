//! Internal tests of how an update writes its row (T6 and T7 of the row reads run): whatever path it takes, the row it
//! stores has the bytes that encoding the whole merged row gives, an identical update writes nothing, and the rows
//! tree keeps its root.

use crate::table::key::encode_key;
use crate::table::value::{ColumnType, Decimal, Value};
use crate::{Column, Schema, Store, StoreOptions, TableMode};

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
    assert!(write.update_row(&table, &given).unwrap());
    let (after_root, _) = write.live_rows(&table).unwrap();
    assert_eq!(after_root, root, "{change:?}: the root");
    let stored = krummi_tree::get(&write.inner, root, &key)
        .unwrap()
        .unwrap()
        .to_vec();
    let expected = write.describe_table("t").unwrap().layout().encode(&merged);
    if identical {
        assert_eq!(
            stored, before,
            "{change:?}: an identical update writes nothing"
        );
    } else {
        assert_eq!(stored, expected, "{change:?}: the merged row");
    }
    write.commit().unwrap();
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
