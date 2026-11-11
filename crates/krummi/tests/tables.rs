//! Tables in the catalog through the public API (tables with schema PRD 7.1): the opening modes, Describe, Exists,
//! List, Delete, Rename and Empty (AC1, AC2, AC3), the schema and its changes (AC4, AC-N6), trees and tables in one
//! catalog (AC-N5, the catalog part of AC13), the format (AC18), writes that change nothing (E16) and the catalog
//! operations that never read a table's own pages (E19). The parts of these criteria that need rows are in the row
//! tests (T6).
//!
//! Each criterion runs on a store on disk and on a temporary one, except the parts that reopen or damage the store's
//! files, which run on disk only.

mod support;

use std::collections::HashSet;
use std::path::Path;

use krummi::table::schema::{decode_version, identity_key, version_key};
use krummi::table::testing::{TableTrees, with_identity_source};
use krummi::table::value::{ColumnType, Decimal, UuidOrder, Value, VectorElement, VectorType};
use krummi::{
    Column, Error, ErrorKind, OpenMode, ReadTransaction, Schema, SchemaDifference, SchemaRule,
    SizeSubject, Store, TableMode, TableSchema, TreeKind, WriteTable, WriteTransaction,
};
use krummi_tree::catalog;
use support::*;

// ---------------------------------------------------------------------------------------------------------------
// Helpers

/// A table `id i64 not null, name string, n i32 not null default 7, score f64` with primary key `id`.
fn people() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String),
            Column::new("n", ColumnType::I32)
                .not_null()
                .with_default(Value::I32(7)),
            Column::new("score", ColumnType::F64),
        ],
        &["id"],
    )
}

/// A schema with one column `k`, of type `i64` without nulls, as its primary key.
fn single() -> Schema {
    Schema::new(vec![Column::new("k", ColumnType::I64).not_null()], &["k"])
}

fn create_table<'s>(
    write: &mut WriteTransaction<'s>,
    name: &str,
    schema: &Schema,
) -> WriteTable<'s> {
    write
        .open_table_with(name, TableMode::CreateNew(schema))
        .unwrap()
}

fn commit_table(store: &Store, name: &str, schema: &Schema) {
    let mut write = store.write().unwrap();
    create_table(&mut write, name, schema);
    write.commit().unwrap();
}

/// The table, column and rule of an invalid-schema error, after checking its folder.
#[track_caller]
fn invalid_schema(error: Error, folder: Option<&Path>) -> (String, Option<String>, SchemaRule) {
    match error {
        Error::InvalidSchema {
            folder: f,
            table,
            column,
            rule,
        } => {
            assert_eq!(f, folder_of(folder));
            (table, column, rule)
        }
        other => panic!("expected an invalid schema, got {other}"),
    }
}

/// The table, column, difference, expected and found text of a different-schema error, after checking its folder.
#[track_caller]
fn different_schema(
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
            (table, column, difference, expected.into(), found.into())
        }
        other => panic!("expected a different schema, got {other}"),
    }
}

/// A schema's columns, its primary key, and the column and rule its creation fails with.
type CreationCase = (
    Vec<Column>,
    Vec<&'static str>,
    Option<&'static str>,
    SchemaRule,
);

/// An operation of a write transaction that may fail.
type Operation<'a> = Box<dyn Fn(&mut WriteTransaction<'_>) -> krummi::Result<()> + 'a>;

fn names_of(schema: &TableSchema) -> Vec<&str> {
    schema.columns().iter().map(|c| c.name.as_str()).collect()
}

fn ids_of(schema: &TableSchema) -> Vec<u64> {
    schema.columns().iter().map(|c| c.id).collect()
}

fn list(read: &ReadTransaction<'_>) -> Vec<(String, TreeKind)> {
    read.list_trees().unwrap()
}

fn some(name: &str) -> Option<String> {
    Some(name.to_owned())
}

fn vector(element: VectorElement, dimension: u16, cosine: bool) -> ColumnType {
    ColumnType::Vector(VectorType {
        element,
        dimension,
        cosine,
    })
}

// ---------------------------------------------------------------------------------------------------------------
// AC1: the opening modes

/// AC1 (E1, A1, A6): each mode on a table that exists and on one that does not; a compatible schema that names fewer
/// columns than the table has opens it; a schema that differs in a column's type, nullability or default, names a
/// column the table does not have or changes the primary key fails without effect with a different schema and its
/// data, in Open or create and in Create or empty; and a tree's name fails with a different type in every mode.
#[test]
fn ac1_opening_modes_of_a_table() {
    stores(|store, folder| {
        let schema = people();
        // A table that does not exist.
        let read = store.read().unwrap();
        assert_eq!(not_found(read.open_table("t").err().unwrap(), folder), "t");
        drop(read);
        let mut write = store.write().unwrap();
        assert_eq!(not_found(write.open_table("t").err().unwrap(), folder), "t");
        let empty = write.open_table_with("t", TableMode::Empty).err().unwrap();
        assert_eq!(not_found(empty, folder), "t");
        assert!(!write.tree_exists("t").unwrap(), "no effect");
        let created = create_table(&mut write, "t", &schema);
        let described = write.describe_table("t").unwrap();
        assert_eq!(described.version(), 1);
        assert_eq!(names_of(&described), ["id", "name", "n", "score"]);
        assert_eq!(described.primary_key(), ["id"]);
        assert_eq!(write.row_count(&created).unwrap(), 0);
        write
            .open_table_with("u", TableMode::OpenOrCreate(&schema))
            .unwrap();
        write
            .open_table_with("v", TableMode::CreateOrEmpty(&schema))
            .unwrap();
        write.commit().unwrap();

        // A table that exists: every mode reaches the same table.
        let id = store
            .read()
            .unwrap()
            .describe_table("t")
            .unwrap()
            .table_id();
        let mut write = store.write().unwrap();
        let exists = write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .err()
            .unwrap();
        assert_eq!(already_exists(exists, folder), "t");
        for mode in [
            TableMode::OpenOrCreate(&schema),
            TableMode::CreateOrEmpty(&schema),
            TableMode::Empty,
        ] {
            let table = write.open_table_with("t", mode).unwrap();
            assert_eq!(
                write.table_schema(&table).unwrap().table_id(),
                id,
                "{mode:?}"
            );
        }
        let opened = write.open_table("t").unwrap();
        assert_eq!(write.table_schema(&opened).unwrap().table_id(), id);
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert_eq!(read.table_schema(&table).unwrap().table_id(), id);
        assert_eq!(read.row_count(&table).unwrap(), 0);
        let names: Vec<String> = list(&read).into_iter().map(|e| e.0).collect();
        assert_eq!(names, ["t", "u", "v"]);
    });
}

/// AC1: a schema that names only some of the table's columns, in another order, is compatible; the table keeps
/// every column.
#[test]
fn ac1_a_compatible_schema_may_miss_columns_of_the_table() {
    stores(|store, _| {
        commit_table(store, "t", &people());
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write
            .add_column(&table, &Column::new("later", ColumnType::Bool))
            .unwrap();
        write.commit().unwrap();

        let older = Schema::new(
            vec![
                Column::new("n", ColumnType::I32)
                    .not_null()
                    .with_default(Value::I32(7)),
                Column::new("id", ColumnType::I64).not_null(),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        for mode in [
            TableMode::OpenOrCreate(&older),
            TableMode::CreateOrEmpty(&older),
        ] {
            let table = write.open_table_with("t", mode).unwrap();
            let schema = write.table_schema(&table).unwrap();
            assert_eq!(names_of(&schema), ["id", "name", "n", "score", "later"]);
        }
        // The default compares by value after normalization: a null default is no default.
        let same = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("name", ColumnType::String).with_default(Value::Null),
            ],
            &["id"],
        );
        write
            .open_table_with("t", TableMode::OpenOrCreate(&same))
            .unwrap();
        write.commit().unwrap();
    });
}

/// AC1 (E6): each difference fails without effect with its data, what the schema says and what the table has.
#[test]
fn ac1_an_incompatible_schema_fails_with_a_different_schema() {
    // `people` with one more column without nulls or default, which a primary key may name.
    let keyed = || {
        let mut schema = people();
        schema
            .columns
            .push(Column::new("k2", ColumnType::I64).not_null());
        schema
    };
    let with = |change: &dyn Fn(&mut Schema)| {
        let mut schema = keyed();
        change(&mut schema);
        schema
    };
    use SchemaDifference::*;
    let cases: Vec<(Schema, Option<String>, SchemaDifference, &str, &str)> = vec![
        (
            with(&|s| s.columns[1].ty = ColumnType::Bytes),
            some("name"),
            Type,
            "bytes",
            "string",
        ),
        (
            with(&|s| {
                s.columns[3].ty = ColumnType::Decimal {
                    precision: 10,
                    scale: 2,
                }
            }),
            some("score"),
            Type,
            "decimal(10, 2)",
            "f64",
        ),
        (
            with(&|s| s.columns[1].nullable = false),
            some("name"),
            Nullability,
            "not null",
            "nulls allowed",
        ),
        (
            with(&|s| s.columns[2].default = Some(Value::I32(8))),
            some("n"),
            Default,
            "8",
            "7",
        ),
        (
            with(&|s| s.columns[1].default = Some(Value::String("x".into()))),
            some("name"),
            Default,
            "\"x\"",
            "no default",
        ),
        (
            with(&|s| s.columns.push(Column::new("extra", ColumnType::Bool))),
            some("extra"),
            MissingColumn,
            "bool",
            "no column",
        ),
        (
            with(&|s| s.primary_key = vec!["id".into(), "k2".into()]),
            None,
            PrimaryKey,
            "(id, k2)",
            "(id)",
        ),
        (
            with(&|s| s.primary_key = vec!["k2".into()]),
            None,
            PrimaryKey,
            "(k2)",
            "(id)",
        ),
    ];
    stores(|store, folder| {
        commit_table(store, "t", &keyed());
        let before = store.read().unwrap().describe_table("t").unwrap();
        let mut write = store.write().unwrap();
        for (schema, column, difference, expected, found) in &cases {
            for mode in [
                TableMode::OpenOrCreate(schema),
                TableMode::CreateOrEmpty(schema),
            ] {
                let error = write.open_table_with("t", mode).err().unwrap();
                assert_eq!(
                    different_schema(error, folder),
                    (
                        "t".to_owned(),
                        column.clone(),
                        *difference,
                        (*expected).to_owned(),
                        (*found).to_owned()
                    ),
                    "{mode:?}"
                );
            }
        }
        assert_eq!(write.describe_table("t").unwrap(), before, "no effect");
        let other = write.open_tree_with("other", OpenMode::CreateNew).unwrap();
        write.set(&other, b"k", b"v").unwrap();
        write.commit().unwrap();
        assert!(store.read().unwrap().tree_exists("other").unwrap());
    });
}

/// AC1 (N1, E1): a tree's name opened as a table fails with a different type in every mode, with any schema, also an
/// invalid one, which wins over already exists; read and write Open too.
#[test]
fn ac1_a_tree_name_is_not_a_table_in_any_mode() {
    stores(|store, folder| {
        commit_tree(store, "tree", &[(b"k", b"v")]);
        let schema = people();
        let invalid = Schema::new(vec![Column::new("c", ColumnType::I64)], &[]);
        let expected = ("tree".to_owned(), TreeKind::Normal, TreeKind::Table);
        let read = store.read().unwrap();
        assert_eq!(
            different(read.open_table("tree").err().unwrap(), folder),
            expected
        );
        assert_eq!(
            different(read.describe_table("tree").err().unwrap(), folder),
            expected
        );
        drop(read);
        let mut write = store.write().unwrap();
        assert_eq!(
            different(write.open_table("tree").err().unwrap(), folder),
            expected
        );
        for mode in [
            TableMode::CreateNew(&schema),
            TableMode::OpenOrCreate(&schema),
            TableMode::CreateOrEmpty(&schema),
            TableMode::CreateNew(&invalid),
            TableMode::OpenOrCreate(&invalid),
            TableMode::Empty,
        ] {
            let error = write.open_table_with("tree", mode).err().unwrap();
            assert_eq!(different(error, folder), expected, "{mode:?}");
        }
        let tree = write.open_tree("tree").unwrap();
        assert_eq!(
            write.get(&tree, b"k").unwrap(),
            Some(&b"v"[..]),
            "no effect"
        );
        write.commit().unwrap();
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC2: Rename, Describe, Exists, List

/// AC2 (E2, A3): renaming a table to another without overwriting fails with already exists; with overwriting, the
/// destination goes away and the renamed table keeps its identity, its schema and its handles; renaming to its own
/// name does nothing; a missing source fails with not found, also to its own name.
#[test]
fn ac2_rename_a_table() {
    stores(|store, folder| {
        commit_table(store, "a", &people());
        commit_table(store, "b", &single());
        let read = store.read().unwrap();
        let (a, b) = (
            read.describe_table("a").unwrap(),
            read.describe_table("b").unwrap(),
        );
        drop(read);
        let mut write = store.write().unwrap();
        let handle = write.open_table("a").unwrap();
        let b_handle = write.open_table("b").unwrap();
        let exists = write.rename_table("a", "b", false).err().unwrap();
        assert_eq!(already_exists(exists, folder), "b");
        assert_eq!(write.describe_table("b").unwrap(), b, "no effect");
        let missing = write
            .rename_table("missing", "missing", false)
            .err()
            .unwrap();
        assert_eq!(not_found(missing, folder), "missing");
        write.rename_table("a", "a", true).unwrap();
        write.rename_table("a", "b", true).unwrap();
        assert!(!write.tree_exists("a").unwrap());
        assert_eq!(
            write.describe_table("b").unwrap(),
            a,
            "the same identity and schema"
        );
        assert_eq!(
            write.table_schema(&handle).unwrap(),
            a,
            "the handle still works"
        );
        assert_eq!(write.row_count(&handle).unwrap(), 0);
        assert_eq!(
            not_found(write.row_count(&b_handle).err().unwrap(), folder),
            "b"
        );
        let missing = write.describe_table("a").err().unwrap();
        assert_eq!(not_found(missing, folder), "a");
        write.rename_table("b", "c", false).unwrap();
        assert_eq!(write.table_schema(&handle).unwrap(), a);
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(read.describe_table("c").unwrap(), a);
        assert_eq!(list(&read), [("c".to_owned(), TreeKind::Table)]);
    });
}

/// AC2 (E2, P12): Describe gives the schema in a read and in a write, the write with its own changes: each column's
/// identity, name, type, nullability and default in schema order, the primary key, the version and the schema state.
/// A column keeps its identity when renamed; the schema state changes with each new version and only then.
#[test]
fn ac2_describe_a_table() {
    stores(|store, folder| {
        commit_table(store, "t", &people());
        let read = store.read().unwrap();
        let first = read.describe_table("t").unwrap();
        drop(read);
        let column = |schema: &TableSchema, name: &str| schema.column(name).unwrap().clone();
        let n = column(&first, "n");
        assert_eq!(
            (n.ty, n.nullable, n.default),
            (ColumnType::I32, false, Some(Value::I32(7)))
        );
        let score = column(&first, "score");
        assert_eq!((score.nullable, score.default), (true, None));
        let ids = ids_of(&first);
        assert_eq!(
            ids.iter().collect::<HashSet<_>>().len(),
            4,
            "distinct identities"
        );
        assert_ne!(first.schema_state(), first.table_id());

        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.rename_column(&table, "name", "label").unwrap();
        let renamed = write.describe_table("t").unwrap();
        assert_eq!(renamed.version(), 2);
        assert_eq!(names_of(&renamed), ["id", "label", "n", "score"]);
        assert_eq!(ids_of(&renamed), ids, "the identity survives the rename");
        assert_ne!(renamed.schema_state(), first.schema_state());
        write.drop_column(&table, "score").unwrap();
        write
            .add_column(&table, &Column::new("score", ColumnType::F64))
            .unwrap();
        write
            .modify_column(&table, "n", ColumnType::I32, true)
            .unwrap();
        let changed = write.describe_table("t").unwrap();
        assert_eq!(changed.version(), 5);
        assert_eq!(names_of(&changed), ["id", "label", "n", "score"]);
        assert_ne!(
            changed.column("score").unwrap().id,
            score.id,
            "a new column"
        );
        assert!(changed.column("n").unwrap().nullable);
        assert_eq!(changed.table_id(), first.table_id());
        let states: HashSet<u64> = [
            first.schema_state(),
            renamed.schema_state(),
            changed.schema_state(),
        ]
        .into_iter()
        .collect();
        assert_eq!(states.len(), 3);
        // The read opened before the write sees its snapshot; Describe is a copy.
        let read = store.read().unwrap();
        assert_eq!(read.describe_table("t").unwrap(), first);
        drop(read);
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(read.describe_table("t").unwrap(), changed);
        let missing = read.describe_table("missing").err().unwrap();
        assert_eq!(not_found(missing, folder), "missing");
    });
}

/// AC2 (E2, A3, B5, N1): Exists says the names of trees and tables exist; List shows every name with its class in
/// byte order of the names, the tables among the trees.
#[test]
fn ac2_exists_and_list_show_tables_and_trees() {
    stores(|store, _| {
        commit_tree(store, "b-tree", EMPTY);
        commit_table(store, "a-table", &single());
        commit_table(store, "c-table", &people());
        let mut write = store.write().unwrap();
        write
            .open_multi_tree_with("d", OpenMode::CreateNew)
            .unwrap();
        write
            .open_fixed_tree_with("B", 8, OpenMode::CreateNew)
            .unwrap();
        write.commit().unwrap();
        let read = store.read().unwrap();
        for name in ["a-table", "b-tree", "c-table", "d", "B"] {
            assert!(read.tree_exists(name).unwrap(), "{name}");
        }
        assert!(!read.tree_exists("e").unwrap());
        assert_eq!(read.tree_kind("a-table").unwrap(), Some(TreeKind::Table));
        assert_eq!(
            list(&read),
            [
                ("B".to_owned(), TreeKind::FixedSize(8)),
                ("a-table".to_owned(), TreeKind::Table),
                ("b-tree".to_owned(), TreeKind::Normal),
                ("c-table".to_owned(), TreeKind::Table),
                ("d".to_owned(), TreeKind::MultiValue),
            ]
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC3: emptying

/// AC3 (E3; the catalog part, rows are T6's): emptying a table, in Empty and in Create or empty, keeps its schema and
/// its identity, and its other handle sees it; and it frees the pages of its rows tree.
#[test]
fn ac3_emptying_keeps_the_schema_the_identity_and_the_handles() {
    stores(|store, _| {
        commit_table(store, "t", &people());
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write
            .add_column(&table, &Column::new("later", ColumnType::Bool))
            .unwrap();
        write.commit().unwrap();
        let before = store.read().unwrap().describe_table("t").unwrap();
        let pages = in_use(store);
        for mode in [TableMode::Empty, TableMode::CreateOrEmpty(&people())] {
            let mut write = store.write().unwrap();
            let other = write.open_table("t").unwrap();
            write.open_table_with("t", mode).unwrap();
            assert_eq!(write.row_count(&other).unwrap(), 0);
            assert_eq!(write.table_schema(&other).unwrap(), before, "{mode:?}");
            write.commit().unwrap();
            let read = store.read().unwrap();
            assert_eq!(read.describe_table("t").unwrap(), before);
            assert_eq!(in_use(store), pages, "one empty root for the rows");
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC4: invalid schemas and changes

/// AC4 (E5): each invalid schema of E5 that a creation can give fails without effect with the table, the column and
/// the rule, and the transaction commits its other changes.
#[test]
fn ac4_each_invalid_schema_fails_the_creation() {
    use SchemaRule::*;
    let id = || Column::new("id", ColumnType::I64).not_null();
    let col = |name: &str, ty| Column::new(name, ty);
    let decimal = |precision, scale| ColumnType::Decimal { precision, scale };
    let many: Vec<Column> = std::iter::once(id())
        .chain((1..1025).map(|i| col(&format!("c{i}"), ColumnType::Bool)))
        .collect();
    assert_eq!(many.len(), 1025);
    let cases: Vec<CreationCase> = vec![
        (vec![id()], vec![], None, NoPrimaryKey),
        (
            vec![col("id", ColumnType::I64)],
            vec!["id"],
            Some("id"),
            InvalidKeyColumn,
        ),
        (
            vec![col("v", vector(VectorElement::F32, 2, false)).not_null()],
            vec!["v"],
            Some("v"),
            InvalidKeyColumn,
        ),
        (
            vec![id().with_default(Value::I64(1))],
            vec!["id"],
            Some("id"),
            InvalidKeyColumn,
        ),
        (vec![id()], vec!["id", "x"], Some("x"), InvalidKeyReference),
        (
            vec![id()],
            vec!["id", "id"],
            Some("id"),
            InvalidKeyReference,
        ),
        (
            vec![id(), col("a", ColumnType::Bool), col("a", ColumnType::I8)],
            vec!["id"],
            Some("a"),
            DuplicateColumnName,
        ),
        (many, vec!["id"], None, TooManyColumns),
        (
            vec![id(), col("v", vector(VectorElement::F32, 0, false))],
            vec!["id"],
            Some("v"),
            VectorOutOfLimits,
        ),
        (
            vec![id(), col("v", vector(VectorElement::F16, 4097, false))],
            vec!["id"],
            Some("v"),
            VectorOutOfLimits,
        ),
        (
            vec![id(), col("v", vector(VectorElement::Bit, 12, false))],
            vec!["id"],
            Some("v"),
            VectorOutOfLimits,
        ),
        (
            vec![id(), col("d", decimal(0, 0))],
            vec!["id"],
            Some("d"),
            DecimalOutOfLimits,
        ),
        (
            vec![id(), col("d", decimal(39, 2))],
            vec!["id"],
            Some("d"),
            DecimalOutOfLimits,
        ),
        (
            vec![id(), col("d", decimal(4, 5))],
            vec!["id"],
            Some("d"),
            DecimalOutOfLimits,
        ),
        (
            vec![id(), col("v", vector(VectorElement::Bit, 16, true))],
            vec!["id"],
            Some("v"),
            CosineOnBit,
        ),
        (
            vec![id(), col("b", ColumnType::Bool).with_default(Value::I8(1))],
            vec!["id"],
            Some("b"),
            InvalidDefault,
        ),
        (
            vec![
                id(),
                col("d", decimal(4, 2)).with_default(Value::Decimal(Decimal {
                    mantissa: 99_995,
                    scale: 3,
                })),
            ],
            vec!["id"],
            Some("d"),
            InvalidDefault,
        ),
        (
            vec![
                id(),
                col("date", ColumnType::Date).with_default(Value::Date(3_000_000)),
            ],
            vec!["id"],
            Some("date"),
            InvalidDefault,
        ),
        // Several rules: the first in E5's order, and within it the first column in schema order.
        (
            vec![
                col("a", decimal(40, 0)),
                col("b", vector(VectorElement::Bit, 3, true)),
                col("b", ColumnType::Bool),
            ],
            vec!["a"],
            Some("a"),
            InvalidKeyColumn,
        ),
        (
            vec![
                id(),
                col("x", vector(VectorElement::Bit, 8, true)),
                col("y", decimal(40, 0)),
                col("z", vector(VectorElement::F32, 0, false)),
            ],
            vec!["id"],
            Some("z"),
            VectorOutOfLimits,
        ),
        (
            vec![
                id(),
                col("x", decimal(40, 0)),
                col("y", decimal(41, 0)),
                col("c", ColumnType::Bool).with_default(Value::I8(0)),
            ],
            vec!["id"],
            Some("x"),
            DecimalOutOfLimits,
        ),
    ];
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        for (columns, key, column, rule) in &cases {
            let schema = Schema::new(columns.clone(), key);
            for mode in [
                TableMode::CreateNew(&schema),
                TableMode::OpenOrCreate(&schema),
                TableMode::CreateOrEmpty(&schema),
            ] {
                let error = write.open_table_with("t", mode).err().unwrap();
                assert_eq!(
                    invalid_schema(error, folder),
                    ("t".to_owned(), column.map(str::to_owned), *rule),
                    "{rule:?} {mode:?}"
                );
            }
        }
        assert!(!write.tree_exists("t").unwrap(), "no effect");
        create_table(&mut write, "other", &single());
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(list(&read), [("other".to_owned(), TreeKind::Table)]);
    });
}

/// AC4 (E4, E5, AC-N6): each change that would give an invalid schema fails without effect with the table, the column
/// and the rule, and the transaction commits its other changes.
#[test]
fn ac4_each_invalid_change_fails_without_effect() {
    use SchemaRule::*;
    type Apply = Box<dyn Fn(&mut WriteTransaction<'_>, &WriteTable<'_>) -> krummi::Result<()>>;
    let add = |column: Column| -> Apply { Box::new(move |w, t| w.add_column(t, &column)) };
    let modify = |name: &'static str, ty: ColumnType, nullable: bool| -> Apply {
        Box::new(move |w, t| w.modify_column(t, name, ty, nullable))
    };
    let cases: Vec<(Apply, &str, SchemaRule)> = vec![
        (
            add(Column::new("name", ColumnType::Bool)),
            "name",
            DuplicateColumnName,
        ),
        (
            Box::new(|w, t| w.rename_column(t, "name", "score")),
            "score",
            DuplicateColumnName,
        ),
        (
            add(Column::new("v", vector(VectorElement::I8, 5000, false))),
            "v",
            VectorOutOfLimits,
        ),
        (
            add(Column::new(
                "d",
                ColumnType::Decimal {
                    precision: 39,
                    scale: 0,
                },
            )),
            "d",
            DecimalOutOfLimits,
        ),
        (
            add(Column::new("v", vector(VectorElement::Bit, 8, true))),
            "v",
            CosineOnBit,
        ),
        (
            add(Column::new("u", ColumnType::Uuid(UuidOrder::SqlServer)).with_default(Value::I8(1))),
            "u",
            InvalidDefault,
        ),
        (
            add(Column::new("x", ColumnType::I8).not_null()),
            "x",
            NotNullWithoutDefault,
        ),
        (
            add(Column::new("x", ColumnType::I8)
                .not_null()
                .with_default(Value::Null)),
            "x",
            NotNullWithoutDefault,
        ),
        (
            modify("name", ColumnType::Bytes, true),
            "name",
            UnsupportedChange,
        ),
        (modify("n", ColumnType::I64, false), "n", UnsupportedChange),
        (
            modify("name", ColumnType::String, false),
            "name",
            UnsupportedChange,
        ),
        (
            modify("id", ColumnType::I32, false),
            "id",
            UnsupportedChange,
        ),
        (modify("id", ColumnType::I64, true), "id", InvalidKeyColumn),
        (
            modify(
                "score",
                ColumnType::Decimal {
                    precision: 0,
                    scale: 0,
                },
                true,
            ),
            "score",
            DecimalOutOfLimits,
        ),
        (
            Box::new(|w, t| w.drop_column(t, "missing")),
            "missing",
            ColumnNotFound,
        ),
        (
            Box::new(|w, t| w.rename_column(t, "missing", "name")),
            "missing",
            ColumnNotFound,
        ),
        (
            modify("missing", ColumnType::I8, true),
            "missing",
            ColumnNotFound,
        ),
        (Box::new(|w, t| w.drop_column(t, "id")), "id", DropKeyColumn),
    ];
    stores(|store, folder| {
        commit_table(store, "t", &people());
        let before = store.read().unwrap().describe_table("t").unwrap();
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        for (apply, column, rule) in &cases {
            let error = apply(&mut write, &table).err().unwrap();
            assert_eq!(
                invalid_schema(error, folder),
                ("t".to_owned(), some(column), *rule),
                "{rule:?}"
            );
            assert_eq!(
                write.describe_table("t").unwrap(),
                before,
                "{rule:?}: no effect"
            );
        }
        commit_tree_in(&mut write, "other");
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(read.describe_table("t").unwrap(), before);
        assert!(read.tree_exists("other").unwrap());
    });
}

fn commit_tree_in(write: &mut WriteTransaction<'_>, name: &str) {
    let tree = write.open_tree_with(name, OpenMode::CreateNew).unwrap();
    write.set(&tree, b"k", b"v").unwrap();
}

/// AC4 (E4, E5): more than 1,024 live columns fails, but dropped columns do not count.
#[test]
fn ac4_at_most_1024_live_columns() {
    stores(|store, folder| {
        let columns: Vec<Column> = std::iter::once(Column::new("id", ColumnType::I64).not_null())
            .chain((1..1024).map(|i| Column::new(format!("c{i}"), ColumnType::Bool)))
            .collect();
        let mut write = store.write().unwrap();
        let table = create_table(&mut write, "t", &Schema::new(columns, &["id"]));
        let error = write
            .add_column(&table, &Column::new("one-more", ColumnType::Bool))
            .err()
            .unwrap();
        assert_eq!(
            invalid_schema(error, folder),
            ("t".to_owned(), None, SchemaRule::TooManyColumns)
        );
        write.drop_column(&table, "c1").unwrap();
        write
            .add_column(&table, &Column::new("one-more", ColumnType::Bool))
            .unwrap();
        let schema = write.table_schema(&table).unwrap();
        assert_eq!(schema.columns().len(), 1024);
        assert_eq!(schema.columns().last().unwrap().name, "one-more");
        write.commit().unwrap();
    });
}

/// AC4 (E4, P19): a table created at version 1 takes 65,534 changes; the next one fails without effect with
/// version exhausted, while a change that changes nothing still succeeds.
#[test]
fn ac4_the_schema_version_ends_at_65535() {
    let store = Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let table = create_table(&mut write, "t", &people());
    for i in 0..65_534u32 {
        let (from, to) = if i % 2 == 0 {
            ("name", "label")
        } else {
            ("label", "name")
        };
        write.rename_column(&table, from, to).unwrap();
    }
    let last = write.table_schema(&table).unwrap();
    assert_eq!(last.version(), 65_535);
    assert_eq!(names_of(&last), ["id", "name", "n", "score"]);
    let fails: [Operation<'_>; 4] = [
        Box::new(|w| w.rename_column(&table, "name", "label")),
        Box::new(|w| w.add_column(&table, &Column::new("x", ColumnType::Bool))),
        Box::new(|w| w.drop_column(&table, "score")),
        Box::new(|w| w.modify_column(&table, "n", ColumnType::I32, true)),
    ];
    for (fail, column) in fails.iter().zip(["name", "x", "score", "n"]) {
        let error = fail(&mut write).err().unwrap();
        assert_eq!(
            invalid_schema(error, None),
            ("t".to_owned(), some(column), SchemaRule::VersionExhausted)
        );
        assert_eq!(write.table_schema(&table).unwrap(), last, "no effect");
    }
    write.rename_column(&table, "name", "name").unwrap();
    write
        .modify_column(&table, "score", ColumnType::F64, true)
        .unwrap();
    assert_eq!(write.table_schema(&table).unwrap(), last);
    write.commit().unwrap();
}

/// AC4 (E4, E16): a change that changes nothing gives no version and writes nothing, nor does a change that fails,
/// Open or create of an existing table, Delete of a missing one or a rename to the same name; creating, emptying,
/// deleting, renaming and each change that gives a version are changes.
#[test]
fn ac4_e16_what_changes_nothing_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_table(&store, "t", &people());
    commit_table(&store, "u", &single());
    commit_tree(&store, "tree", EMPTY);
    store.close().unwrap();
    let changed = files_change(tmp.path(), |store| {
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let before = write.table_schema(&table).unwrap();
        write.rename_column(&table, "name", "name").unwrap();
        write
            .modify_column(&table, "score", ColumnType::F64, true)
            .unwrap();
        write
            .modify_column(&table, "n", ColumnType::I32, false)
            .unwrap();
        write
            .modify_column(&table, "id", ColumnType::I64, false)
            .unwrap();
        assert_eq!(write.table_schema(&table).unwrap(), before);
        write
            .open_table_with("t", TableMode::OpenOrCreate(&people()))
            .unwrap();
        write.delete_table("missing").unwrap();
        write.rename_table("t", "t", false).unwrap();
        write.rename_table("t", "t", true).unwrap();
        let fails = [
            write.drop_column(&table, "id").err(),
            write
                .add_column(&table, &Column::new("x", ColumnType::I8).not_null())
                .err(),
            write
                .open_table_with("t", TableMode::CreateNew(&people()))
                .err(),
            write.open_table_with("missing", TableMode::Empty).err(),
            write
                .open_table_with("t", TableMode::OpenOrCreate(&single()))
                .err(),
            write.rename_table("t", "u", false).err(),
            write.rename_table("t", "tree", true).err(),
            write.delete_table("tree").err(),
            write.delete_tree("t").err(),
        ];
        let kinds = fails.map(|e| e.unwrap().kind());
        use ErrorKind::*;
        assert_eq!(
            kinds,
            [
                InvalidSchema,
                InvalidSchema,
                TreeAlreadyExists,
                TreeNotFound,
                DifferentSchema,
                TreeAlreadyExists,
                DifferentTreeType,
                DifferentTreeType,
                DifferentTreeType
            ]
        );
        write.commit().unwrap();
    });
    assert!(!changed, "a write that changes nothing writes nothing");

    let changes: [&dyn Fn(&mut WriteTransaction<'_>); 8] = [
        &|w| {
            create_table(w, "new", &single());
        },
        &|w| {
            w.open_table_with("u", TableMode::Empty).unwrap();
        },
        &|w| w.delete_table("u").unwrap(),
        &|w| w.rename_table("u", "v", false).unwrap(),
        &|w| {
            let t = w.open_table("t").unwrap();
            w.rename_column(&t, "name", "label").unwrap();
        },
        &|w| {
            let t = w.open_table("t").unwrap();
            w.add_column(&t, &Column::new("x", ColumnType::Bool))
                .unwrap();
        },
        &|w| {
            let t = w.open_table("t").unwrap();
            w.drop_column(&t, "score").unwrap();
        },
        &|w| {
            let t = w.open_table("t").unwrap();
            w.modify_column(&t, "n", ColumnType::I32, true).unwrap();
        },
    ];
    for (i, change) in changes.iter().enumerate() {
        let copy = tempfile::tempdir().unwrap();
        for (path, bytes) in files(tmp.path()) {
            std::fs::write(copy.path().join(path.file_name().unwrap()), bytes).unwrap();
        }
        let changed = files_change(copy.path(), |store| {
            let mut write = store.write().unwrap();
            change(&mut write);
            write.commit().unwrap();
        });
        assert!(changed, "change {i}");
    }
}

/// AC4 (E4, B9): every handle of the table in the write sees the new schema at once, and Describe too.
#[test]
fn ac4_another_handle_sees_the_new_schema() {
    stores(|store, _| {
        commit_table(store, "t", &people());
        let mut write = store.write().unwrap();
        let first = write.open_table("t").unwrap();
        let second = write
            .open_table_with("t", TableMode::OpenOrCreate(&people()))
            .unwrap();
        write
            .add_column(&first, &Column::new("x", ColumnType::Bool))
            .unwrap();
        write.rename_column(&second, "x", "y").unwrap();
        for handle in [&first, &second] {
            let schema = write.table_schema(handle).unwrap();
            assert_eq!(names_of(&schema), ["id", "name", "n", "score", "y"]);
            assert_eq!(schema.version(), 3);
        }
        assert_eq!(
            write.describe_table("t").unwrap(),
            write.table_schema(&first).unwrap()
        );
        write.commit().unwrap();
    });
}

/// AC-N6 (E4, N3): a column added with a default keeps it normalized; one added with a dropped column's name is new:
/// another identity, at the end of the schema order.
#[test]
fn ac_n6_added_columns_and_a_dropped_name() {
    stores(|store, _| {
        commit_table(store, "t", &people());
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let old = write
            .table_schema(&table)
            .unwrap()
            .column("name")
            .unwrap()
            .id;
        write.drop_column(&table, "name").unwrap();
        write
            .add_column(
                &table,
                &Column::new(
                    "price",
                    ColumnType::Decimal {
                        precision: 10,
                        scale: 2,
                    },
                )
                .not_null()
                .with_default(Value::Decimal(Decimal {
                    mantissa: 1_235,
                    scale: 3,
                })),
            )
            .unwrap();
        write
            .add_column(&table, &Column::new("name", ColumnType::String))
            .unwrap();
        let schema = write.table_schema(&table).unwrap();
        assert_eq!(names_of(&schema), ["id", "n", "score", "price", "name"]);
        assert_ne!(schema.column("name").unwrap().id, old);
        assert_eq!(
            schema.column("price").unwrap().default,
            Some(Value::Decimal(Decimal {
                mantissa: 124,
                scale: 2
            })),
            "rounded to the column's scale (E8)"
        );
        write.commit().unwrap();
    });
}

/// D3: the schema history has one entry per version, keyed by its big-endian number, with that version's columns
/// and schema state, and the identities the table used; nothing reaches it by a name.
#[test]
fn d3_the_schema_history_has_one_entry_per_version() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    let table = create_table(&mut write, "t", &people());
    let mut versions = vec![write.table_schema(&table).unwrap()];
    write.rename_column(&table, "name", "label").unwrap();
    versions.push(write.table_schema(&table).unwrap());
    write.drop_column(&table, "score").unwrap();
    versions.push(write.table_schema(&table).unwrap());
    write
        .add_column(&table, &Column::new("x", ColumnType::Bool))
        .unwrap();
    versions.push(write.table_schema(&table).unwrap());
    let TableTrees { history, .. } = write.table_trees(&table).unwrap();
    write.commit().unwrap();
    store.close().unwrap();
    inspect(tmp.path(), |read| {
        for schema in &versions {
            let value = krummi_tree::get(read, history, &version_key(schema.version()))
                .unwrap()
                .expect("an entry per version");
            let (state, columns) = decode_version(value).unwrap();
            assert_eq!(state, schema.schema_state());
            assert_eq!(columns, schema.row_columns());
            for id in ids_of(schema).into_iter().chain([state]) {
                let known = krummi_tree::get(read, history, &identity_key(id)).unwrap();
                assert_eq!(known, Some(&[][..]));
            }
        }
        assert_eq!(
            krummi_tree::get(read, history, &version_key(5)).unwrap(),
            None
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Identities (P12, D6)

/// A source that gives the 8-byte little-endian values of `values` in turn, then fails with input and output.
fn repeating(values: Vec<u64>) -> impl FnMut(&mut [u8]) -> krummi::Result<()> {
    let mut next = values.into_iter();
    move |bytes| match next.next() {
        Some(value) => {
            bytes.copy_from_slice(&value.to_le_bytes());
            Ok(())
        }
        None => Err(Error::Io {
            folder: Default::default(),
            source: std::io::Error::other("the test source ended"),
        }),
    }
}

/// D6 (P12): a drawn table identity equal to another table's in the catalog, or a column identity or schema state
/// equal to one the table already used, is drawn again.
#[test]
fn d6_identities_equal_to_known_ones_are_drawn_again() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let two = Schema::new(
            vec![
                Column::new("k", ColumnType::I64).not_null(),
                Column::new("v", ColumnType::Bool),
            ],
            &["k"],
        );
        // Table 10 with columns 20 and 20 again, so 21, and schema state 20 or 21 again, so 22.
        with_identity_source(repeating(vec![10, 20, 20, 21, 20, 21, 22]), || {
            create_table(&mut write, "a", &two);
        });
        let a = write.describe_table("a").unwrap();
        assert_eq!(
            (a.table_id(), ids_of(&a), a.schema_state()),
            (10, vec![20, 21], 22)
        );
        // Another table: 10 is taken, 11 is not; its columns may repeat another table's.
        with_identity_source(repeating(vec![10, 11, 20, 21, 22]), || {
            create_table(&mut write, "b", &two);
        });
        let b = write.describe_table("b").unwrap();
        assert_eq!(
            (b.table_id(), ids_of(&b), b.schema_state()),
            (11, vec![20, 21], 22)
        );
        // A change: its state is new to the table's history, and so is an added column's identity.
        let table = write.open_table("a").unwrap();
        with_identity_source(repeating(vec![22, 20, 30, 30, 21, 31]), || {
            write
                .add_column(&table, &Column::new("x", ColumnType::Bool))
                .unwrap();
        });
        let a = write.describe_table("a").unwrap();
        assert_eq!((ids_of(&a), a.schema_state()), (vec![20, 21, 31], 30));
        // A dropped column's identity stays known.
        write.drop_column(&table, "x").unwrap();
        with_identity_source(repeating(vec![31, 22, 30, 40]), || {
            write.rename_column(&table, "v", "w").unwrap();
        });
        assert_eq!(write.describe_table("a").unwrap().schema_state(), 40);
        write.commit().unwrap();
    });
}

/// D6 (document 11, §2.2): when the operating system's generator fails, the change fails with input and output,
/// nothing weaker is drawn, and the transaction cannot commit (T11).
#[test]
fn d6_a_failing_generator_fails_the_change() {
    stores(|store, _| {
        commit_table(store, "t", &people());
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let error = with_identity_source(repeating(vec![]), || {
            write.rename_column(&table, "name", "label").err().unwrap()
        });
        assert_eq!(error.kind(), ErrorKind::Io);
        assert_eq!(
            write.table_schema(&table).unwrap().version(),
            1,
            "no effect"
        );
        assert_eq!(write.commit().err().unwrap().kind(), ErrorKind::Io);
        let mut write = store.write().unwrap();
        let error = with_identity_source(repeating(vec![]), || {
            write
                .open_table_with("new", TableMode::CreateNew(&single()))
                .err()
                .unwrap()
        });
        assert_eq!(error.kind(), ErrorKind::Io);
        assert!(!write.tree_exists("new").unwrap());
        write.abort();
        assert_eq!(
            store.read().unwrap().describe_table("t").unwrap().version(),
            1
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Deleting, handles and precedence

/// E2, A6, B9: deleting a table frees its pages, a missing name does nothing, and its handles fail with not found
/// and its last name, also after another table takes its name, without reaching it; a handle of another transaction
/// fails with misuse.
#[test]
fn e2_delete_a_table_and_its_handles() {
    stores(|store, folder| {
        commit_tree(store, "tree", EMPTY);
        let pages = in_use(store);
        let mut write = store.write().unwrap();
        let table = create_table(&mut write, "t", &people());
        write
            .add_column(&table, &Column::new("x", ColumnType::Bool))
            .unwrap();
        write.commit().unwrap();
        assert!(in_use(store) > pages);

        let mut write = store.write().unwrap();
        let old = write.open_table("t").unwrap();
        write.delete_table("t").unwrap();
        write.delete_table("t").unwrap();
        create_table(&mut write, "t", &single());
        assert_eq!(not_found(write.row_count(&old).err().unwrap(), folder), "t");
        assert_eq!(
            not_found(write.table_schema(&old).err().unwrap(), folder),
            "t"
        );
        let gone = write.drop_column(&old, "name").err().unwrap();
        assert_eq!(not_found(gone, folder), "t");
        let gone = write
            .add_column(&old, &Column::new("id", ColumnType::Bool))
            .err()
            .unwrap();
        assert_eq!(
            not_found(gone, folder),
            "t",
            "not found wins over invalid schema"
        );
        let size = write
            .add_column(&old, &Column::new("", ColumnType::Bool))
            .err()
            .unwrap();
        assert_eq!(
            invalid_size(size, folder).0,
            SizeSubject::ColumnName,
            "the size wins"
        );
        assert_eq!(
            names_of(&write.describe_table("t").unwrap()),
            ["k"],
            "the new table"
        );
        write.delete_table("t").unwrap();
        write.commit().unwrap();
        assert_eq!(in_use(store), pages, "its pages are free");

        let mut write = store.write().unwrap();
        let mine = create_table(&mut write, "t", &single());
        write.commit().unwrap();
        let mut write = store.write().unwrap();
        misuse(write.row_count(&mine).err().unwrap(), folder);
        misuse(write.drop_column(&mine, "k").err().unwrap(), folder);
        let read = store.read().unwrap();
        let read_table = read.open_table("t").unwrap();
        drop(read);
        let read = store.read().unwrap();
        misuse(read.row_count(&read_table).err().unwrap(), folder);
        assert_eq!(write.describe_table("t").unwrap().version(), 1, "no effect");
    });
}

/// E17, E18, AC15 (column names): names are checked before the catalog, the table's first; a column name of 1 or
/// 1,024 bytes works and one of 0 or 1,025 fails without effect with an invalid size of a column name, in a schema,
/// its primary key and each change; then the catalog wins over the schema, and an invalid schema over a different one.
#[test]
fn e17_precedence_of_names_catalog_and_schema() {
    stores(|store, folder| {
        commit_tree(store, "tree", EMPTY);
        commit_table(store, "t", &people());
        let long = "c".repeat(MAX_NAME);
        let too_long = "c".repeat(MAX_NAME + 1);
        let named = |name: &str| {
            Schema::new(
                vec![
                    Column::new("id", ColumnType::I64).not_null(),
                    Column::new(name, ColumnType::Bool),
                ],
                &["id"],
            )
        };
        let mut write = store.write().unwrap();
        for (name, received) in [("", 0), (too_long.as_str(), 1025)] {
            for schema in [named(name), Schema::new(vec![], &[name])] {
                for table in ["tree", "t", "new"] {
                    let error = write
                        .open_table_with(table, TableMode::OpenOrCreate(&schema))
                        .err()
                        .unwrap();
                    assert_eq!(
                        invalid_size(error, folder),
                        (SizeSubject::ColumnName, received, 1, 1024)
                    );
                }
            }
            let table = write.open_table("t").unwrap();
            let changes = [
                write
                    .add_column(&table, &Column::new(name, ColumnType::Bool))
                    .err(),
                write.drop_column(&table, name).err(),
                write.rename_column(&table, name, "x").err(),
                write.rename_column(&table, "name", name).err(),
                write
                    .modify_column(&table, name, ColumnType::Bool, true)
                    .err(),
            ];
            for error in changes {
                assert_eq!(
                    invalid_size(error.unwrap(), folder),
                    (SizeSubject::ColumnName, received, 1, 1024)
                );
            }
        }
        let error = write
            .open_table_with("", TableMode::OpenOrCreate(&named("")))
            .err()
            .unwrap();
        assert_eq!(
            invalid_size(error, folder).0,
            SizeSubject::TreeName,
            "the table's name first"
        );
        for name in ["c", long.as_str()] {
            create_table(&mut write, &format!("n{}", name.len()), &named(name));
            let table = write.open_table("t").unwrap();
            write
                .add_column(&table, &Column::new(name, ColumnType::Bool))
                .unwrap();
            write.rename_column(&table, name, "renamed").unwrap();
            write.drop_column(&table, "renamed").unwrap();
        }
        // The catalog before the schema, the invalid schema before the different one.
        let invalid = Schema::new(vec![Column::new("id", ColumnType::I64)], &["id"]);
        let error = write
            .open_table_with("t", TableMode::CreateNew(&invalid))
            .err()
            .unwrap();
        assert_eq!(already_exists(error, folder), "t");
        let error = write
            .open_table_with("t", TableMode::OpenOrCreate(&invalid))
            .err()
            .unwrap();
        assert_eq!(
            invalid_schema(error, folder).2,
            SchemaRule::InvalidKeyColumn
        );
        let error = write
            .open_table_with("tree", TableMode::CreateNew(&invalid))
            .err()
            .unwrap();
        assert_eq!(different(error, folder).1, TreeKind::Normal);
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert!(read.tree_exists("n1").unwrap() && read.tree_exists("n1024").unwrap());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// AC-N5 and AC13: trees and tables in one catalog

/// AC-N5 (N1, E1, E2): creating a table with a table's name fails with already exists; with a tree's name, or a tree
/// with a table's name, with a different type, which wins; opening, deleting, renaming or emptying a table as a tree
/// and a tree as a table, or renaming one to the other's name with overwrite, fail without effect with a different
/// type, and without overwrite with already exists; Exists says both names exist.
#[test]
fn ac_n5_trees_and_tables_never_touch_each_other() {
    stores(|store, folder| {
        commit_tree(store, "tree", &[(b"k", b"v")]);
        commit_table(store, "table", &people());
        let schema = people();
        let mut write = store.write().unwrap();
        let exists = write
            .open_table_with("table", TableMode::CreateNew(&schema))
            .err()
            .unwrap();
        assert_eq!(already_exists(exists, folder), "table");
        let as_table = ("tree".to_owned(), TreeKind::Normal, TreeKind::Table);
        let as_tree = |requested| ("table".to_owned(), TreeKind::Table, requested);
        let error = write
            .open_table_with("tree", TableMode::CreateNew(&schema))
            .err()
            .unwrap();
        assert_eq!(different(error, folder), as_table);
        for mode in support::MODES {
            let error = write.open_tree_with("table", mode).err().unwrap();
            assert_eq!(
                different(error, folder),
                as_tree(TreeKind::Normal),
                "{mode:?}"
            );
            let error = write.open_multi_tree_with("table", mode).err().unwrap();
            assert_eq!(different(error, folder), as_tree(TreeKind::MultiValue));
            let error = write.open_fixed_tree_with("table", 8, mode).err().unwrap();
            assert_eq!(different(error, folder), as_tree(TreeKind::FixedSize(8)));
        }
        assert_eq!(
            different(write.open_tree("table").err().unwrap(), folder),
            as_tree(TreeKind::Normal)
        );
        assert_eq!(
            different(write.delete_tree("table").err().unwrap(), folder),
            as_tree(TreeKind::Normal)
        );
        let error = write.rename_tree("table", "x", false).err().unwrap();
        assert_eq!(different(error, folder), as_tree(TreeKind::Normal));
        let error = write.rename_tree("tree", "table", true).err().unwrap();
        assert_eq!(different(error, folder), as_tree(TreeKind::Normal));
        let error = write.rename_tree("tree", "table", false).err().unwrap();
        assert_eq!(already_exists(error, folder), "table");

        assert_eq!(
            different(write.open_table("tree").err().unwrap(), folder),
            as_table
        );
        assert_eq!(
            different(write.delete_table("tree").err().unwrap(), folder),
            as_table
        );
        let error = write
            .open_table_with("tree", TableMode::Empty)
            .err()
            .unwrap();
        assert_eq!(different(error, folder), as_table);
        let error = write.rename_table("tree", "x", false).err().unwrap();
        assert_eq!(different(error, folder), as_table);
        let error = write.rename_table("table", "tree", true).err().unwrap();
        assert_eq!(different(error, folder), as_table);
        let error = write.rename_table("table", "tree", false).err().unwrap();
        assert_eq!(already_exists(error, folder), "tree");

        assert!(write.tree_exists("tree").unwrap() && write.tree_exists("table").unwrap());
        let tree = write.open_tree("tree").unwrap();
        assert_eq!(
            write.get(&tree, b"k").unwrap(),
            Some(&b"v"[..]),
            "no effect"
        );
        assert_eq!(
            write.describe_table("table").unwrap().version(),
            1,
            "no effect"
        );
        assert!(!write.tree_exists("x").unwrap());
        write.commit().unwrap();
        let read = store.read().unwrap();
        assert_eq!(
            list(&read),
            [
                ("table".to_owned(), TreeKind::Table),
                ("tree".to_owned(), TreeKind::Normal)
            ]
        );
    });
}

/// A raw entry of the catalog, as its bytes give it: its name, kind byte and root, and the root of the schema
/// history tree that a table's bytes after the fixed part give.
#[derive(Debug)]
struct RawEntry {
    name: String,
    kind: u8,
    root: u64,
    history: Option<u64>,
}

/// The kind byte of a table in the catalog (D2).
const TABLE_KIND: u8 = 4;

/// Every entry of the catalog of the closed store, read from the catalog tree's leaves: what any name of the catalog
/// reaches, listed or not.
fn raw_entries(folder: &Path) -> Vec<RawEntry> {
    inspect(folder, |read| {
        let root = read.slot(catalog::ROOT_SLOT).unwrap();
        let mut entries = Vec::new();
        krummi_tree::for_each(read, root, &mut |key, value| {
            let field = |at: usize| u64::from_le_bytes(value[at..at + 8].try_into().unwrap());
            let (id, kind, root) = (field(0), value[8], field(9));
            let history = (kind == TABLE_KIND).then(|| {
                TableSchema::decode(id, &value[25..])
                    .expect("a table's schema")
                    .1
            });
            entries.push(RawEntry {
                name: String::from_utf8(key.to_vec()).unwrap(),
                kind,
                root,
                history,
            });
        })
        .unwrap();
        entries
    })
}

/// AC13 and AC-N5 (the catalog part; N1, D3): listing the catalog shows the tables and none of their internal trees,
/// and no entry of the catalog, listed or not, reaches them as a tree: through the test hook that gives their roots,
/// every raw entry is either a table's, whose kind is a table and whose rows and history roots are the hook's, or
/// one whose root is no table's internal tree.
#[test]
fn ac13_no_name_reaches_the_internal_trees_of_a_table() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_tree(&store, "tree", &[(b"k", b"v")]);
    let mut write = store.write().unwrap();
    let table = create_table(&mut write, "table", &people());
    write.rename_column(&table, "name", "label").unwrap();
    let other = create_table(&mut write, "other", &single());
    write.insert_row(&other, &[("k", Value::I64(1))]).unwrap();
    write
        .open_multi_tree_with("multi", OpenMode::CreateNew)
        .unwrap();
    write
        .open_sequence_with("seq", krummi::SequenceMode::CreateNew(None))
        .unwrap();
    write.commit().unwrap();
    let read = store.read().unwrap();
    let mut internal: Vec<(String, TableTrees)> = Vec::new();
    for name in ["other", "table"] {
        let trees = read.table_trees(&read.open_table(name).unwrap()).unwrap();
        assert_ne!(trees.rows, trees.history);
        internal.push((name.to_owned(), trees));
    }
    let listed = list(&read);
    drop(read);
    store.close().unwrap();

    let raw = raw_entries(tmp.path());
    let names: Vec<(String, TreeKind)> = raw
        .iter()
        .map(|entry| {
            let kind = match entry.kind {
                1 => TreeKind::Normal,
                2 => TreeKind::MultiValue,
                TABLE_KIND => TreeKind::Table,
                5 => TreeKind::Sequence,
                other => panic!("kind byte {other} of {:?}", entry.name),
            };
            (entry.name.clone(), kind)
        })
        .collect();
    assert_eq!(names, listed, "every entry is listed");
    let roots: Vec<u64> = internal
        .iter()
        .flat_map(|(_, trees)| [trees.rows, trees.history])
        .collect();
    for entry in &raw {
        match internal.iter().find(|(name, _)| *name == entry.name) {
            Some((_, trees)) => {
                assert_eq!(entry.kind, TABLE_KIND, "{}", entry.name);
                assert_eq!(
                    (entry.root, entry.history),
                    (trees.rows, Some(trees.history)),
                    "only through the table's entry"
                );
            }
            None => {
                assert_ne!(entry.kind, TABLE_KIND, "{}", entry.name);
                assert!(
                    !roots.contains(&entry.root),
                    "{} reaches a table's tree",
                    entry.name
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// E19: catalog operations do not read a table's own pages

/// E19 (A9): with both pages of a table's internal trees damaged, Open, Create new, Open or create, Exists,
/// Describe, List, Rename without overwrite or to its own name and the row count do not fail with damage.
#[test]
fn e19_catalog_operations_do_not_read_the_tables_pages() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_table(&store, "t", &people());
    let read = store.read().unwrap();
    let trees = read.table_trees(&read.open_table("t").unwrap()).unwrap();
    drop(read);
    store.close().unwrap();
    damage(tmp.path(), trees.rows);
    damage(tmp.path(), trees.history);
    let store = open(tmp.path());
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    assert_eq!(read.row_count(&table).unwrap(), 0);
    assert_eq!(read.describe_table("t").unwrap().version(), 1);
    assert!(read.tree_exists("t").unwrap());
    assert_eq!(list(&read), [("t".to_owned(), TreeKind::Table)]);
    drop(read);
    let mut write = store.write().unwrap();
    let exists = write
        .open_table_with("t", TableMode::CreateNew(&people()))
        .err()
        .unwrap();
    assert_eq!(exists.kind(), ErrorKind::TreeAlreadyExists);
    let table = write
        .open_table_with("t", TableMode::OpenOrCreate(&people()))
        .unwrap();
    assert_eq!(write.row_count(&table).unwrap(), 0);
    write.rename_table("t", "t", false).unwrap();
    write.rename_table("t", "u", false).unwrap();
    assert_eq!(write.describe_table("u").unwrap().version(), 1);
    write.commit().unwrap();
    store.close().unwrap();
}

/// G6: a table entry whose bytes after the catalog's fixed part do not decode, in a page whose checksum is valid, is
/// damage of that catalog page for the operations that need the schema; Exists and List still work.
#[test]
fn g6_a_malformed_table_entry_is_damage_of_its_catalog_page() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_table(&store, "t", &people());
    store.close().unwrap();
    // Rewrite the entry with a truncated schema through the engine, which seals the page with a valid checksum.
    {
        let engine = krummi_tx::Engine::open(tmp.path(), &Default::default()).unwrap();
        let mut write = engine.write(None).unwrap();
        let located = catalog::lookup_with_extra(&write, "t").unwrap().unwrap();
        let (entry, extra) = (located.entry, located.extra.to_vec());
        catalog::put_with_extra(&mut write, "t", &entry, &extra[..extra.len() - 1]).unwrap();
        write.commit().unwrap();
        engine.close().unwrap();
    }
    // The catalog leaf as the rewrite left it: copy on write may have moved the entry to another page.
    let page = leaf_page(tmp.path(), None, b"t");
    let store = open(tmp.path());
    let read = store.read().unwrap();
    assert_eq!(damaged_page(read.open_table("t").err().unwrap()), page);
    assert_eq!(damaged_page(read.describe_table("t").err().unwrap()), page);
    assert!(read.tree_exists("t").unwrap());
    assert_eq!(list(&read), [("t".to_owned(), TreeKind::Table)]);
}

/// G6 (E5): a table entry whose bytes after the catalog's fixed part are garbage (a valid schema with a byte more,
/// bytes of no schema, a schema that breaks E5, a single byte) is damage of its catalog page for Open and Describe,
/// which never panic.
#[test]
fn g6_garbage_after_a_table_entry_is_damage_of_its_catalog_page() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    commit_table(&store, "t", &people());
    store.close().unwrap();
    let valid = inspect(tmp.path(), |read| {
        catalog::lookup_with_extra(read, "t")
            .unwrap()
            .unwrap()
            .extra
            .to_vec()
    });
    let mut longer = valid.clone();
    longer.push(0);
    let noise: Vec<u8> = (0..valid.len())
        .map(|i| (i as u8).wrapping_mul(151).wrapping_add(7))
        .collect();
    // The version (bytes 8 and 9, after the history root) set to 0, which no schema has (E4).
    let mut version_zero = valid.clone();
    version_zero[8..10].copy_from_slice(&[0, 0]);
    for (label, extra) in [
        ("a byte more", longer),
        ("noise", noise),
        ("version 0", version_zero),
        ("one byte", vec![0xFF]),
    ] {
        {
            let engine = krummi_tx::Engine::open(tmp.path(), &Default::default()).unwrap();
            let mut write = engine.write(None).unwrap();
            let entry = catalog::lookup(&write, "t").unwrap().unwrap();
            catalog::put_with_extra(&mut write, "t", &entry, &extra).unwrap();
            write.commit().unwrap();
            engine.close().unwrap();
        }
        // The catalog leaf as this rewrite left it: copy on write may have moved the entry to another page.
        let page = leaf_page(tmp.path(), None, b"t");
        let store = open(tmp.path());
        let read = store.read().unwrap();
        assert_eq!(
            damaged_page(read.open_table("t").err().unwrap()),
            page,
            "{label}"
        );
        assert_eq!(
            damaged_page(read.describe_table("t").err().unwrap()),
            page,
            "{label}"
        );
        drop(read);
        store.close().unwrap();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// AC18: the format

/// AC18 (E20, B12, D8): a store with tables has format 11 in its header, so an engine of format 10, which opens
/// only its own format, refuses it; this engine refuses the same store marked as format 10 the same way, with the
/// different-format class; and the store's information keeps its shape, with the tables' pages in its totals.
#[test]
fn ac18_a_store_with_tables_is_of_format_11() {
    use krummi_pager::DATA_FILE;
    use krummi_pager::format::{Header, PAGE_SIZE, header_copies, write_header_pages};
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let empty = in_use(&store);
    commit_table(&store, "t", &people());
    let krummi::StoreInfo {
        format_version,
        page_size,
        size_bytes: _,
        page_count,
        max_size: _,
        temporary,
        free_pages: _,
    } = store.info();
    assert_eq!((format_version, page_size, temporary), (11, 8192, false));
    assert!(in_use(&store) > empty, "the table's pages count");
    assert!(page_count >= in_use(&store));
    store.close().unwrap();

    let data = tmp.path().join(DATA_FILE);
    let mut bytes = std::fs::read(&data).unwrap();
    let [Some(a), Some(b)] = header_copies(&bytes[..2 * PAGE_SIZE]) else {
        panic!("both header copies")
    };
    assert_eq!((a.format_version, b.format_version), (11, 11));
    let current = if a.revision >= b.revision { a } else { b };
    write_header_pages(
        &mut bytes[..2 * PAGE_SIZE],
        &Header {
            format_version: 10,
            ..current
        },
    );
    std::fs::write(&data, &bytes).unwrap();
    match Store::open(tmp.path(), &krummi::StoreOptions::default()).err() {
        Some(Error::DifferentFormat {
            found, expected, ..
        }) => assert_eq!((found, expected), (10, 11)),
        other => panic!("expected a different format, got {other:?}"),
    }
    assert_eq!(std::fs::read(&data).unwrap(), bytes, "left as it was");
}
