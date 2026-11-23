//! The collation of text columns (7.7, `docs/prd/krummi/tables/tables-collation.prd.md`) through the public API: the
//! declaration, Describe, the schema rules and differences (AC-IC1), the format (AC-IC9) and, in the submodules, the
//! primary key, the indexes, the order and the statistics. Each criterion runs on a store on disk and on a temporary
//! one, except the parts that rewrite the store's files, which run on disk only.

mod support;

use krummi::table::value::{ColumnType, Value, VectorElement, VectorType};
use krummi::{
    Collation, Column, Error, Schema, SchemaDifference, SchemaRule, Store, TableMode, WriteTable,
    WriteTransaction,
};
use krummi_pager::format::{Header, PAGE_SIZE, header_copies, write_header_pages};
use support::*;

fn text(name: &str) -> Column {
    Column::new(name, ColumnType::String)
}

fn create<'s>(write: &mut WriteTransaction<'s>, name: &str, schema: &Schema) -> WriteTable<'s> {
    write
        .open_table_with(name, TableMode::CreateNew(schema))
        .unwrap()
}

/// The rule and column of an invalid schema.
#[track_caller]
fn invalid_schema<T>(result: Result<T, Error>) -> (SchemaRule, Option<String>) {
    match result.err() {
        Some(Error::InvalidSchema { rule, column, .. }) => (rule, column),
        other => panic!("expected an invalid schema, got {other:?}"),
    }
}

/// The column, difference, expected and found of a different schema.
#[track_caller]
fn different_schema<T>(
    result: Result<T, Error>,
) -> (Option<String>, SchemaDifference, String, String) {
    match result.err() {
        Some(Error::DifferentSchema {
            column,
            difference,
            expected,
            found,
            ..
        }) => (column, difference, expected.into(), found.into()),
        other => panic!("expected a different schema, got {other:?}"),
    }
}

/// A table with a key, `plain` (no collation declared), `bin`, `ci` and `ciai` text columns and an `n` integer.
fn collated() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            text("plain"),
            text("bin").with_collation(Collation::Binary),
            text("ci").with_collation(Collation::Ci),
            text("ciai").with_collation(Collation::CiAi),
            Column::new("n", ColumnType::I64),
        ],
        &["id"],
    )
}

#[test]
fn ac_ic1_describe_shows_the_collation_of_each_text_column() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &collated());
        write
            .add_column(&table, &text("late").with_collation(Collation::Ci))
            .unwrap();
        let schema = write.describe_table("t").unwrap();
        let collations: Vec<(&str, Option<Collation>)> = schema
            .columns()
            .iter()
            .map(|c| (c.name.as_str(), c.collation))
            .collect();
        assert_eq!(
            collations,
            [
                ("id", None),
                ("plain", Some(Collation::Binary)),
                ("bin", Some(Collation::Binary)),
                ("ci", Some(Collation::Ci)),
                ("ciai", Some(Collation::CiAi)),
                ("n", None),
                ("late", Some(Collation::Ci)),
            ]
        );
        write.commit().unwrap();
        // The collation survives the catalog.
        let read = store.read().unwrap();
        let again = read.describe_table("t").unwrap();
        assert_eq!(
            again.column("ciai").unwrap().collation,
            Some(Collation::CiAi)
        );
        assert_eq!(again.column("late").unwrap().collation, Some(Collation::Ci));
        assert_eq!(
            [Collation::Binary, Collation::Ci, Collation::CiAi].map(|c| c.to_string()),
            ["binary", "ci", "ci_ai"]
        );
    });
}

#[test]
fn ac_ic1_renaming_and_modifying_keep_the_collation() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &collated());
        write.rename_column(&table, "ci", "ci2").unwrap();
        // Allowing nulls in `ciai`, which already allows them, giving its collation changes nothing; making `ci2`
        // allow nulls is a no-op too, so test a not-null column.
        let strict = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                text("s").not_null().with_collation(Collation::Ci),
            ],
            &["id"],
        );
        let other = create(&mut write, "u", &strict);
        write
            .modify_column_with_collation(&other, "s", ColumnType::String, Collation::Ci, true)
            .unwrap();
        let u = write.describe_table("u").unwrap();
        let s = u.column("s").unwrap();
        assert!(s.nullable);
        assert_eq!(s.collation, Some(Collation::Ci));
        let t = write.describe_table("t").unwrap();
        assert_eq!(t.column("ci2").unwrap().collation, Some(Collation::Ci));
        // Giving another collation, or none on a column that is not binary, is a change N3 does not allow.
        for collation in [Collation::Binary, Collation::CiAi] {
            assert_eq!(
                invalid_schema(write.modify_column_with_collation(
                    &table,
                    "ci2",
                    ColumnType::String,
                    collation,
                    true
                )),
                (SchemaRule::UnsupportedChange, Some("ci2".to_owned()))
            );
        }
        assert_eq!(
            invalid_schema(write.modify_column(&table, "ci2", ColumnType::String, true)),
            (SchemaRule::UnsupportedChange, Some("ci2".to_owned()))
        );
        // A collation given to a column of another type.
        assert_eq!(
            invalid_schema(write.modify_column_with_collation(
                &table,
                "n",
                ColumnType::I64,
                Collation::Binary,
                true
            )),
            (SchemaRule::CollationOnNonText, Some("n".to_owned()))
        );
        // `modify_column` on a binary column still works.
        write
            .modify_column(&table, "bin", ColumnType::String, true)
            .unwrap();
        assert_eq!(write.describe_table("t").unwrap(), t, "nothing changed");
    });
}

#[test]
fn ac_ic1_a_collation_on_a_column_that_is_not_text_is_an_invalid_schema() {
    let vector = ColumnType::Vector(VectorType {
        element: VectorElement::F32,
        dimension: 3,
        cosine: false,
    });
    let others = [ColumnType::Bytes, ColumnType::Json, ColumnType::I32, vector];
    stores(|store, _| {
        for ty in others {
            for collation in [Collation::Binary, Collation::Ci] {
                let schema = Schema::new(
                    vec![
                        Column::new("id", ColumnType::I64).not_null(),
                        Column::new("x", ty).with_collation(collation),
                    ],
                    &["id"],
                );
                let mut write = store.write().unwrap();
                assert_eq!(
                    invalid_schema(write.open_table_with("t", TableMode::CreateNew(&schema))),
                    (SchemaRule::CollationOnNonText, Some("x".to_owned())),
                    "{ty} {collation}"
                );
                let table = create(
                    &mut write,
                    "t",
                    &Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]),
                );
                assert_eq!(
                    invalid_schema(
                        write.add_column(&table, &Column::new("x", ty).with_collation(collation))
                    ),
                    (SchemaRule::CollationOnNonText, Some("x".to_owned()))
                );
                // A mode that receives a schema validates it before comparing (E1).
                assert_eq!(
                    invalid_schema(write.open_table_with("t", TableMode::OpenOrCreate(&schema))),
                    (SchemaRule::CollationOnNonText, Some("x".to_owned()))
                );
                write.abort();
            }
        }
        // Precedence (IC9): the cosine metric on `bit` wins over a collation; a collation wins over a bad default.
        let bit_cosine = ColumnType::Vector(VectorType {
            element: VectorElement::Bit,
            dimension: 8,
            cosine: true,
        });
        let precedence = [
            (
                vec![
                    Column::new("a", ColumnType::Bytes).with_collation(Collation::Ci),
                    Column::new("b", bit_cosine),
                ],
                (SchemaRule::CosineOnBit, Some("b".to_owned())),
            ),
            (
                vec![
                    Column::new("a", ColumnType::I32).with_default(Value::String("x".into())),
                    Column::new("b", ColumnType::Bytes).with_collation(Collation::Ci),
                ],
                (SchemaRule::CollationOnNonText, Some("b".to_owned())),
            ),
        ];
        for (columns, expected) in precedence {
            let mut all = vec![Column::new("id", ColumnType::I64).not_null()];
            all.extend(columns);
            let schema = Schema::new(all, &["id"]);
            let mut write = store.write().unwrap();
            assert_eq!(
                invalid_schema(write.open_table_with("t", TableMode::CreateNew(&schema))),
                expected
            );
            write.abort();
        }
    });
}

#[test]
fn ac_ic1_another_collation_is_a_different_schema() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                text("name").with_collation(Collation::Ci),
                text("d")
                    .with_collation(Collation::Ci)
                    .with_default(Value::String("Ana".into())),
            ],
            &["id"],
        );
        create(&mut write, "t", &schema);
        write.commit().unwrap();
        let with = |name: Column, d: Column| {
            Schema::new(
                vec![Column::new("id", ColumnType::I64).not_null(), name, d],
                &["id"],
            )
        };
        let d = || {
            text("d")
                .with_collation(Collation::Ci)
                .with_default(Value::String("Ana".into()))
        };
        let cases = [
            (
                with(text("name").with_collation(Collation::CiAi), d()),
                ("name", SchemaDifference::Collation, "ci_ai", "ci"),
            ),
            (
                with(text("name"), d()),
                ("name", SchemaDifference::Collation, "binary", "ci"),
            ),
            (
                with(Column::new("name", ColumnType::Bytes), d()),
                ("name", SchemaDifference::Type, "bytes", "string"),
            ),
            (
                with(
                    text("name").with_collation(Collation::Ci),
                    text("d")
                        .with_collation(Collation::Ci)
                        .with_default(Value::String("ANA".into())),
                ),
                ("d", SchemaDifference::Default, "\"ANA\"", "\"Ana\""),
            ),
        ];
        for (given, (column, difference, expected, found)) in cases {
            let mut write = store.write().unwrap();
            assert_eq!(
                different_schema(write.open_table_with("t", TableMode::OpenOrCreate(&given))),
                (
                    Some(column.to_owned()),
                    difference,
                    expected.to_owned(),
                    found.to_owned()
                )
            );
            write.abort();
        }
        // The same schema opens.
        let mut write = store.write().unwrap();
        write
            .open_table_with("t", TableMode::OpenOrCreate(&schema))
            .unwrap();
        write.abort();
    });
}

#[test]
fn ac_ic9_a_store_of_format_14_is_of_a_previous_format() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    create(
        &mut write,
        "t",
        &Schema::new(vec![text("id").not_null()], &["id"]),
    );
    write.commit().unwrap();
    store.close().unwrap();
    let data = tmp.path().join(krummi_pager::DATA_FILE);
    let mut bytes = std::fs::read(&data).unwrap();
    let [Some(a), Some(b)] = header_copies(&bytes[..2 * PAGE_SIZE]) else {
        panic!("both header copies")
    };
    assert_eq!((a.format_version, b.format_version), (15, 15));
    let current = if a.revision >= b.revision { a } else { b };
    write_header_pages(
        &mut bytes[..2 * PAGE_SIZE],
        &Header {
            format_version: 14,
            ..current
        },
    );
    std::fs::write(&data, &bytes).unwrap();
    match Store::open(tmp.path(), &krummi::StoreOptions::default()).err() {
        Some(Error::DifferentFormat {
            found, expected, ..
        }) => assert_eq!((found, expected), (14, 15)),
        other => panic!("expected a different format, got {other:?}"),
    }
    assert_eq!(std::fs::read(&data).unwrap(), bytes, "left as it was");
    let new = Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
    assert_eq!(new.info().format_version, 15);
    let _: &Store = &new;
}
