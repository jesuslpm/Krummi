//! AC-JS8 (JS1, JS9, E4, E5, E6, IX3): JSON columns in keys, in indexes and in schema changes, and their defaults; and
//! AC-JS13 (JS10, ST12, ST18): JSON columns have no statistics.

use krummi::table::value::{ColumnType, Value, VectorElement, VectorType};
use krummi::{Condition, ErrorKind, IndexDefinition, IndexKey, IndexMode, SchemaRule};

use super::*;

/// The table, index, column and rule of an invalid schema.
#[track_caller]
fn invalid_schema(error: Error) -> (String, Option<String>, Option<String>, SchemaRule) {
    match error {
        Error::InvalidSchema {
            table,
            column,
            rule,
            index,
            ..
        } => (table, index.map(String::from), column, rule),
        other => panic!("expected an invalid schema, got {other:?}"),
    }
}

fn some(text: &str) -> Option<String> {
    Some(text.to_owned())
}

/// AC-JS8: a JSON primary key column and a JSON index key column are refused without effect; a JSON included column is
/// created and read back, and dropping it fails; changing a type to or from `json` fails.
#[test]
fn ac_js8_json_columns_in_keys_indexes_and_type_changes() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let keyed = Schema::new(
            vec![Column::new("id", ColumnType::Json).not_null()],
            &["id"],
        );
        let error = write
            .open_table_with("k", TableMode::CreateNew(&keyed))
            .err()
            .unwrap();
        assert_eq!(
            invalid_schema(error),
            (
                "k".to_owned(),
                None,
                some("id"),
                SchemaRule::InvalidKeyColumn
            )
        );
        assert_eq!(
            write.open_table("k").err().unwrap().kind(),
            ErrorKind::TreeNotFound
        );
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("doc", ColumnType::Json),
                Column::new("s", ColumnType::String),
            ],
            &["id"],
        );
        let table = create(&mut write, "t", &schema);
        write
            .insert_row(&table, &[("id", Value::I64(1)), ("doc", doc("[1]"))])
            .unwrap();
        let before = write.table_schema(&table).unwrap();
        let error = write
            .create_index(
                &table,
                &IndexDefinition::new("ix", vec![IndexKey::new("doc")]),
                IndexMode::CreateNew,
            )
            .err()
            .unwrap();
        assert_eq!(
            invalid_schema(error),
            (
                "t".to_owned(),
                some("ix"),
                some("doc"),
                SchemaRule::VectorIndexKey
            )
        );
        assert_eq!(write.table_schema(&table).unwrap(), before, "no effect");
        let included = IndexDefinition::new("ix", vec![IndexKey::new("s")]).with_included(&["doc"]);
        write
            .create_index(&table, &included, IndexMode::CreateNew)
            .unwrap();
        let described = write.table_schema(&table).unwrap();
        let index = described.index("ix").unwrap();
        assert_eq!(
            index
                .included
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["doc"]
        );
        let entries: Vec<String> = write
            .index_entries(&table, "ix", &[], ..)
            .unwrap()
            .map(|entry| text_of(&entry.unwrap().get("doc").unwrap()).to_owned())
            .collect();
        assert_eq!(entries, ["[1]"]);
        let error = write.drop_column(&table, "doc").unwrap_err();
        assert_eq!(
            invalid_schema(error),
            (
                "t".to_owned(),
                some("ix"),
                some("doc"),
                SchemaRule::DropKeyColumn
            )
        );
        for (column, ty) in [("s", ColumnType::Json), ("doc", ColumnType::String)] {
            let error = write.modify_column(&table, column, ty, true).unwrap_err();
            assert_eq!(
                invalid_schema(error),
                (
                    "t".to_owned(),
                    None,
                    some(column),
                    SchemaRule::UnsupportedChange
                )
            );
        }
        // The key column modified to JSON: the key rule comes first (E5).
        let error = write
            .modify_column(&table, "id", ColumnType::Json, false)
            .err()
            .unwrap();
        assert_eq!(invalid_schema(error).3, SchemaRule::InvalidKeyColumn);
        write.commit().unwrap();
    });
}

/// AC-JS8 (JS9): JSON columns added to a table with rows, nullable, with the default `{"a": 1}` and with the default
/// document `null`, read null, `{"a":1}` and a `null` value in the earlier rows, and Describe gives `json` and the
/// normalized default; an invalid default fails with the rule of E5; and opening the table with `{ "a" : 1 }` as
/// default is compatible, while no default in the column of `null` is a different schema.
#[test]
fn ac_js8_json_defaults() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &docs());
        write
            .insert_row(&table, &[("id", Value::I64(1)), ("doc", doc("{}"))])
            .unwrap();
        let added = [
            Column::new("a", ColumnType::Json),
            Column::new("b", ColumnType::Json).with_default(doc(r#"{"a": 1}"#).into_owned()),
            Column::new("c", ColumnType::Json)
                .not_null()
                .with_default(doc("null").into_owned()),
        ];
        for column in &added {
            write.add_column(&table, column).unwrap();
        }
        let check = |row: &TableRow<'_>| {
            assert_eq!(row.get("a"), Some(Value::Null));
            assert_eq!(text_of(&row.get("b").unwrap()), r#"{"a":1}"#);
            assert_eq!(
                row.json(4).unwrap().document().unwrap().kind(),
                JsonKind::Null
            );
        };
        check(&write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap());
        // A new row takes the defaults too.
        write.insert_row(&table, &[("id", Value::I64(2))]).unwrap();
        check(&write.get_row(&table, &[Value::I64(2)]).unwrap().unwrap());
        let described = write.describe_table("t").unwrap();
        let b = described.column("b").unwrap();
        assert_eq!(b.ty.to_string(), "json");
        assert_eq!(
            b.default.as_ref().map(ToString::to_string).as_deref(),
            Some(r#"{"a":1}"#)
        );
        let invalid = Column::new("d", ColumnType::Json).with_default(doc("{").into_owned());
        let error = write.add_column(&table, &invalid).unwrap_err();
        assert_eq!(
            invalid_schema(error),
            ("t".to_owned(), None, some("d"), SchemaRule::InvalidDefault)
        );
        let invalid = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("d", ColumnType::Json).with_default(doc_bytes(b"\xFF").into_owned()),
            ],
            &["id"],
        );
        let error = write
            .open_table_with("u", TableMode::CreateNew(&invalid))
            .err()
            .unwrap();
        assert_eq!(
            invalid_schema(error),
            ("u".to_owned(), None, some("d"), SchemaRule::InvalidDefault)
        );
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        check(&read.get_row(&table, &[Value::I64(1)]).unwrap().unwrap());
        drop(read);
        let compatible = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("b", ColumnType::Json).with_default(doc(r#"{ "a" : 1 }"#).into_owned()),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        write
            .open_table_with("t", TableMode::OpenOrCreate(&compatible))
            .unwrap();
        let without = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("c", ColumnType::Json).not_null(),
            ],
            &["id"],
        );
        match write
            .open_table_with("t", TableMode::OpenOrCreate(&without))
            .err()
            .unwrap()
        {
            Error::DifferentSchema {
                column,
                expected,
                found,
                ..
            } => assert_eq!(
                (column.as_deref(), &*expected, &*found),
                (Some("c"), "no default", "null")
            ),
            other => panic!("expected a different schema, got {other:?}"),
        }
    });
}

/// A table `name` of an `i64` key `id`, an `i64` `n`, a `string` `s` and, with `json`, a JSON column `doc`, with the
/// same 50 rows.
fn statistics_table(store: &Store, name: &str, json: bool) {
    let mut columns = vec![
        Column::new("id", ColumnType::I64).not_null(),
        Column::new("n", ColumnType::I64),
        Column::new("s", ColumnType::String),
    ];
    if json {
        columns.push(Column::new("doc", ColumnType::Json));
    }
    let mut write = store.write().unwrap();
    let table = create(&mut write, name, &Schema::new(columns, &["id"]));
    for id in 0..50 {
        let s = Value::String(format!("s{}", id % 7).into());
        let mut row = vec![("id", Value::I64(id)), ("n", Value::I64(id % 5)), ("s", s)];
        if json {
            row.push(("doc", doc(r#"{"n":1}"#)));
        }
        write.insert_row(&table, &row).unwrap();
    }
    write.commit().unwrap();
}

/// AC-JS13 (JS10): estimating, reading the statistics of or analyzing a JSON column, or estimating an equality on
/// several columns that includes one, fails with the rule JSON column; analyzing the table builds the other columns'
/// statistics, the same as in the table without the JSON column, and none of the JSON column.
#[test]
fn ac_js13_json_columns_have_no_statistics() {
    stores(|store, _| {
        statistics_table(store, "t", true);
        statistics_table(store, "u", false);
        let json_column = |error: Error| {
            assert_eq!(
                invalid_schema(error),
                ("t".to_owned(), None, some("doc"), SchemaRule::JsonColumn)
            );
        };
        let equal = Condition::Equal {
            column: "doc",
            value: doc("{}"),
        };
        let columns = Condition::EqualColumns(vec![("n", Value::I64(1)), ("doc", doc("{}"))]);
        {
            let read = store.read().unwrap();
            let table = read.open_table("t").unwrap();
            json_column(read.estimate(&table, &equal).unwrap_err());
            json_column(read.estimate(&table, &columns).unwrap_err());
            json_column(read.column_statistics(&table, "doc").unwrap_err());
        }
        json_column(store.analyze("t", Some("doc")).unwrap_err());
        {
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            json_column(write.estimate(&table, &equal).unwrap_err());
            json_column(write.estimate(&table, &columns).unwrap_err());
            json_column(write.column_statistics(&table, "doc").unwrap_err());
            json_column(write.analyze(&table, Some("doc")).unwrap_err());
        }
        store.analyze("t", None).unwrap();
        store.analyze("u", None).unwrap();
        let read = store.read().unwrap();
        let (t, u) = (read.open_table("t").unwrap(), read.open_table("u").unwrap());
        for column in ["id", "n", "s"] {
            let with = read.column_statistics(&t, column).unwrap();
            assert!(with.is_some(), "{column}");
            assert_eq!(
                with,
                read.column_statistics(&u, column).unwrap(),
                "{column}"
            );
        }
        json_column(read.column_statistics(&t, "doc").unwrap_err());
        // An estimate of the other columns works as without the JSON column.
        let n = Condition::EqualColumns(vec![
            ("n", Value::I64(1)),
            ("s", Value::String("s1".into())),
        ]);
        assert_eq!(
            read.estimate(&t, &n).unwrap(),
            read.estimate(&u, &n).unwrap()
        );
    });
}

/// The rule of a vector column, which shares the check of a JSON column, stays the vector column's (ST12, ST18): to
/// estimate it, alone and among several columns, to read its statistics and to analyze it.
#[test]
fn vector_columns_keep_the_rule_vector_column() {
    stores(|store, _| {
        let vector = ColumnType::Vector(VectorType {
            element: VectorElement::F32,
            dimension: 2,
            cosine: false,
        });
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("v", vector),
                Column::new("doc", ColumnType::Json),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        create(&mut write, "t", &schema);
        write.commit().unwrap();
        let elements: Vec<u8> = [1f32, 2.0].iter().flat_map(|v| v.to_le_bytes()).collect();
        let v = || Value::Vector(elements.as_slice().into());
        let rule = |error: Error| invalid_schema(error).3;
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let equal = Condition::Equal {
            column: "v",
            value: v(),
        };
        assert_eq!(
            rule(read.estimate(&table, &equal).unwrap_err()),
            SchemaRule::VectorColumn
        );
        // Among several columns, the first one without statistics in the order of the list.
        for (pairs, expected) in [
            (
                vec![("id", Value::I64(1)), ("v", v())],
                SchemaRule::VectorColumn,
            ),
            (
                vec![("v", v()), ("doc", doc("1"))],
                SchemaRule::VectorColumn,
            ),
            (vec![("doc", doc("1")), ("v", v())], SchemaRule::JsonColumn),
        ] {
            let condition = Condition::EqualColumns(pairs);
            assert_eq!(
                rule(read.estimate(&table, &condition).unwrap_err()),
                expected
            );
        }
        assert_eq!(
            rule(read.column_statistics(&table, "v").unwrap_err()),
            SchemaRule::VectorColumn
        );
        drop(read);
        assert_eq!(
            rule(store.analyze("t", Some("v")).unwrap_err()),
            SchemaRule::VectorColumn
        );
    });
}
