//! The definitions of vector indexes (VI1, VI2, VI10, VI14): creating them with every admitted element and metric and
//! failing with every invalid definition (AC-VI1); Describe (part of AC-VI17); the changes of the table that keep,
//! refuse or free them (the catalog parts of AC-VI9); and the format (part of AC-VI14).

use krummi::table::value::{ColumnType, Value, VectorElement, VectorType};
use krummi::{
    Column, Error, ErrorKind, IndexDefinition, IndexKey, IndexMode, Schema, SchemaDifference,
    SchemaRule, Store, StoreOptions, TableMode, VectorIndexDefinition, VectorMetric,
    WriteTransaction,
};
use krummi_pager::format::{Header, PAGE_SIZE, header_copies, write_header_pages};

use super::support::{in_use, open, stores};
use super::{f32_vector, insert, vector};

/// A table `t` with a vector column of each element and the cosine metric: `a` f32, `b` f16, `c` i8, `ca` f32
/// cosine, `cb` f16 cosine, `cc` i8 cosine, `bits` bit, and an `i64` column `n`.
fn every_vector() -> Schema {
    let ty = |element, cosine| {
        ColumnType::Vector(VectorType {
            element,
            dimension: 8,
            cosine,
        })
    };
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64),
            Column::new("a", ty(VectorElement::F32, false)),
            Column::new("b", ty(VectorElement::F16, false)),
            Column::new("c", ty(VectorElement::I8, false)),
            Column::new("ca", ty(VectorElement::F32, true)),
            Column::new("cb", ty(VectorElement::F16, true)),
            Column::new("cc", ty(VectorElement::I8, true)),
            Column::new("bits", ty(VectorElement::Bit, false)),
        ],
        &["id"],
    )
}

fn definition(name: &str, column: &str, metric: VectorMetric) -> VectorIndexDefinition {
    VectorIndexDefinition::new(name, column, metric)
}

fn rule(error: Error) -> (SchemaRule, Option<String>) {
    match error {
        Error::InvalidSchema { rule, index, .. } => (rule, index.map(|i| i.to_string())),
        other => panic!("expected an invalid schema, got {other:?}"),
    }
}

fn difference(error: Error) -> SchemaDifference {
    match error {
        Error::DifferentIndex { difference, .. } => difference,
        other => panic!("expected a different index, got {other:?}"),
    }
}

/// AC-VI1: every admitted element and metric creates an index, two over the same column with different metrics
/// included; Describe lists them in the order of their names with their identities, columns and metrics, apart from the
/// secondary ones; and each invalid definition fails with its class, without effect.
#[test]
fn ac_vi1_create_every_admitted_index_and_refuse_the_invalid_ones() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&every_vector()))
            .unwrap();
        let created = [
            ("i1", "a", VectorMetric::L2),
            ("i2", "a", VectorMetric::Dot),
            ("i3", "b", VectorMetric::L2),
            ("i4", "b", VectorMetric::Dot),
            ("i5", "c", VectorMetric::L2),
            ("i6", "c", VectorMetric::Dot),
            ("i7", "ca", VectorMetric::Cosine),
            ("i8", "cb", VectorMetric::Cosine),
            ("i9", "cc", VectorMetric::Cosine),
        ];
        for (name, column, metric) in created.iter().rev() {
            let state = write.describe_table("t").unwrap().index_state();
            write
                .create_vector_index(
                    &table,
                    &definition(name, column, *metric),
                    IndexMode::CreateNew,
                )
                .unwrap();
            assert_ne!(
                write.describe_table("t").unwrap().index_state(),
                state,
                "a new index state"
            );
        }
        let by_n = IndexDefinition::new("by_n", vec![IndexKey::new("n")]);
        write
            .create_index(&table, &by_n, IndexMode::CreateNew)
            .unwrap();
        let described = write.describe_table("t").unwrap();
        let listed: Vec<(&str, &str, VectorMetric)> = (described.vector_indexes().iter())
            .map(|i| (i.name.as_str(), i.column.name.as_str(), i.metric))
            .collect();
        assert_eq!(listed, created.to_vec());
        assert_eq!(described.indexes().len(), 1, "the secondary ones apart");
        for index in described.vector_indexes() {
            assert_eq!(
                index.column.id,
                described.column(&index.column.name).unwrap().id
            );
            assert_ne!(index.id, 0);
        }
        let ids: std::collections::HashSet<u64> =
            described.vector_indexes().iter().map(|i| i.id).collect();
        assert_eq!(ids.len(), 9);

        let pages = in_use(store);
        let before = write.describe_table("t").unwrap();
        let refuse = |write: &mut WriteTransaction<'_>, def: VectorIndexDefinition, mode| {
            write.create_vector_index(&table, &def, mode).unwrap_err()
        };
        let cases = [
            (
                definition("x", "missing", VectorMetric::L2),
                SchemaRule::ColumnNotFound,
            ),
            (
                definition("x", "n", VectorMetric::L2),
                SchemaRule::VectorIndexColumn,
            ),
            (
                definition("x", "bits", VectorMetric::L2),
                SchemaRule::VectorIndexColumn,
            ),
            (
                definition("x", "bits", VectorMetric::Hamming),
                SchemaRule::VectorIndexColumn,
            ),
            (
                definition("x", "ca", VectorMetric::L2),
                SchemaRule::VectorIndexMetric,
            ),
            (
                definition("x", "ca", VectorMetric::Dot),
                SchemaRule::VectorIndexMetric,
            ),
            (
                definition("x", "a", VectorMetric::Cosine),
                SchemaRule::VectorIndexMetric,
            ),
            (
                definition("x", "a", VectorMetric::Hamming),
                SchemaRule::VectorIndexMetric,
            ),
            (
                definition("x", "a", VectorMetric::L2),
                SchemaRule::SameVectorIndex,
            ),
        ];
        for (def, expected) in cases {
            let error = refuse(&mut write, def.clone(), IndexMode::CreateNew);
            assert_eq!(rule(error), (expected, Some("x".to_owned())), "{def:?}");
        }
        for name in ["", &"n".repeat(1025)] {
            let error = refuse(
                &mut write,
                definition(name, "a", VectorMetric::L2),
                IndexMode::CreateNew,
            );
            assert_eq!(error.kind(), ErrorKind::InvalidSize);
        }
        // One name space with the secondary indexes (VI1, IX2).
        for name in ["i1", "by_n"] {
            let error = refuse(
                &mut write,
                definition(name, "b", VectorMetric::Dot),
                IndexMode::CreateNew,
            );
            assert_eq!(error.kind(), ErrorKind::TreeAlreadyExists, "{name}");
        }
        let error = refuse(
            &mut write,
            definition("i1", "b", VectorMetric::L2),
            IndexMode::CreateIfNotExists,
        );
        assert_eq!(difference(error), SchemaDifference::VectorIndexColumn);
        let error = refuse(
            &mut write,
            definition("i1", "a", VectorMetric::Dot),
            IndexMode::CreateIfNotExists,
        );
        assert_eq!(difference(error), SchemaDifference::VectorIndexMetric);
        let error = refuse(
            &mut write,
            definition("by_n", "a", VectorMetric::L2),
            IndexMode::CreateIfNotExists,
        );
        assert_eq!(difference(error), SchemaDifference::IndexKind);
        let secondary = IndexDefinition::new("i1", vec![IndexKey::new("n")]);
        let error = write
            .create_index(&table, &secondary, IndexMode::CreateNew)
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::TreeAlreadyExists);
        let error = write
            .create_index(&table, &secondary, IndexMode::CreateIfNotExists)
            .unwrap_err();
        assert_eq!(difference(error), SchemaDifference::IndexKind);
        // Create if not exists with the same definition leaves it as it is.
        write
            .create_vector_index(
                &table,
                &definition("i1", "a", VectorMetric::L2),
                IndexMode::CreateIfNotExists,
            )
            .unwrap();
        assert_eq!(write.describe_table("t").unwrap(), before, "no effect");
        assert_eq!(in_use(store), pages, "no page");
        // A vector index is not scanned and has no statistics: named so, it is an index the table does not have.
        let scan = write.index_entries(&table, "i1", &[], ..).err().unwrap();
        assert_eq!(difference(scan), SchemaDifference::MissingIndex);
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let stats = read.index_statistics(&table, "i1").unwrap_err();
        assert_eq!(stats.kind(), ErrorKind::DifferentSchema);
        assert_eq!(read.describe_table("t").unwrap(), before);
    });
}

/// VI1: the 64 indexes of a table count the secondary and the vector ones together.
#[test]
fn the_most_indexes_count_both_kinds() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let mut columns = vec![Column::new("id", ColumnType::I64).not_null()];
        for i in 0..40 {
            columns.push(Column::new(format!("n{i}"), ColumnType::I64));
        }
        for i in 0..20 {
            columns.push(Column::new(
                format!("v{i}"),
                ColumnType::Vector(f32_vector(2, false)),
            ));
        }
        let table = write
            .open_table_with("t", TableMode::CreateNew(&Schema::new(columns, &["id"])))
            .unwrap();
        for i in 0..40 {
            let def =
                IndexDefinition::new(format!("s{i:02}"), vec![IndexKey::new(format!("n{i}"))]);
            write
                .create_index(&table, &def, IndexMode::CreateNew)
                .unwrap();
        }
        for i in 0..20 {
            let def = definition(&format!("v{i:02}"), &format!("v{i}"), VectorMetric::L2);
            write
                .create_vector_index(&table, &def, IndexMode::CreateNew)
                .unwrap();
        }
        for i in 0..4 {
            let def = definition(&format!("w{i:02}"), &format!("v{i}"), VectorMetric::Dot);
            write
                .create_vector_index(&table, &def, IndexMode::CreateNew)
                .unwrap();
        }
        let more = definition("w99", "v5", VectorMetric::Dot);
        let error = write
            .create_vector_index(&table, &more, IndexMode::CreateNew)
            .unwrap_err();
        assert_eq!(rule(error).0, SchemaRule::TooManyIndexes);
        let secondary = IndexDefinition::new("s99", vec![IndexKey::new("n0"), IndexKey::new("n1")]);
        let error = write
            .create_index(&table, &secondary, IndexMode::CreateNew)
            .unwrap_err();
        assert_eq!(rule(error).0, SchemaRule::TooManyIndexes);
        write.drop_index(&table, "w00").unwrap();
        write
            .create_vector_index(&table, &more, IndexMode::CreateNew)
            .unwrap();
        write.commit().unwrap();
    });
}

/// The catalog parts of AC-VI9 and VI10, on disk and in a temporary store: renaming the column or the table keeps the
/// index; dropping its column fails with an invalid schema that names it; dropping the index, a missing one, or the
/// table frees every page; emptying the table keeps the index with new trees; all of it survives reopening.
#[test]
fn ac_vi9_the_changes_of_the_table() {
    // The test owns its store, on disk and temporary, since it closes the one on disk to reopen it: Windows locks the
    // files of an open store, so they cannot be copied while it is open.
    for on_disk in [true, false] {
        let folder = tempfile::tempdir().unwrap();
        let mut store = if on_disk {
            open(folder.path())
        } else {
            Store::open_temporary(&StoreOptions::default()).unwrap()
        };
        let schema = super::schema(f32_vector(3, false));
        // The pages of a store whose table was created and deleted, which keeps its catalog's root.
        let mut write = store.write().unwrap();
        write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .unwrap();
        write.delete_table("t").unwrap();
        write.commit().unwrap();
        let base = in_use(&store);
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .unwrap();
        for id in 0..50 {
            insert(
                &mut write,
                &table,
                id,
                Some(id),
                None,
                Some(vector(&[id as f32, 1.0, 2.0])),
            );
        }
        let def = definition("near", "v", VectorMetric::L2);
        write
            .create_vector_index(&table, &def, IndexMode::CreateNew)
            .unwrap();
        write.commit().unwrap();

        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.rename_column(&table, "v", "embedding").unwrap();
        let described = write.describe_table("t").unwrap();
        assert_eq!(described.vector_indexes()[0].column.name, "embedding");
        let error = write.drop_column(&table, "embedding").unwrap_err();
        assert_eq!(
            rule(error),
            (SchemaRule::DropKeyColumn, Some("near".to_owned()))
        );
        write.rename_table("t", "u", false).unwrap();
        assert_eq!(write.describe_table("u").unwrap().vector_indexes().len(), 1);
        write.commit().unwrap();

        // Emptying keeps the definition and its identity.
        let mut write = store.write().unwrap();
        let table = write.open_table_with("u", TableMode::Empty).unwrap();
        let described = write.describe_table("u").unwrap();
        assert_eq!(described.vector_indexes().len(), 1);
        write.drop_index(&table, "missing").unwrap();
        write.commit().unwrap();

        if on_disk {
            // Reopening keeps the definitions.
            store.close().unwrap();
            store = open(folder.path());
            let read = store.read().unwrap();
            assert_eq!(read.describe_table("u").unwrap(), described);
        }

        let mut write = store.write().unwrap();
        let table = write.open_table("u").unwrap();
        write.drop_index(&table, "near").unwrap();
        assert!(
            write
                .describe_table("u")
                .unwrap()
                .vector_indexes()
                .is_empty()
        );
        write
            .create_vector_index(
                &table,
                &definition("near", "embedding", VectorMetric::Dot),
                IndexMode::CreateNew,
            )
            .unwrap();
        write.delete_table("u").unwrap();
        write.commit().unwrap();
        assert_eq!(in_use(&store), base, "every page freed");

        // A table that another overwrites when it is renamed frees its vector indexes too (E2).
        let mut write = store.write().unwrap();
        for name in ["a", "b"] {
            let table = write
                .open_table_with(name, TableMode::CreateNew(&schema))
                .unwrap();
            write
                .create_vector_index(&table, &def, IndexMode::CreateNew)
                .unwrap();
        }
        write.rename_table("a", "b", true).unwrap();
        write.delete_table("b").unwrap();
        write.commit().unwrap();
        assert_eq!(in_use(&store), base, "every page freed");
        store.close().unwrap();
    }
}

/// AC-VI14 (the format, VI14): a store of format 15, written before the vector indexes, is of a previous format and
/// this engine does not open it, leaving it as it was; a new store has format 16.
#[test]
fn ac_vi14_a_store_of_format_15_is_of_a_previous_format() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    write
        .open_table_with(
            "t",
            TableMode::CreateNew(&super::schema(f32_vector(2, false))),
        )
        .unwrap();
    write.commit().unwrap();
    store.close().unwrap();
    let data = tmp.path().join(krummi_pager::DATA_FILE);
    let mut bytes = std::fs::read(&data).unwrap();
    let [Some(a), Some(b)] = header_copies(&bytes[..2 * PAGE_SIZE]) else {
        panic!("both header copies")
    };
    assert_eq!((a.format_version, b.format_version), (16, 16));
    let current = if a.revision >= b.revision { a } else { b };
    write_header_pages(
        &mut bytes[..2 * PAGE_SIZE],
        &Header {
            format_version: 15,
            ..current
        },
    );
    std::fs::write(&data, &bytes).unwrap();
    match Store::open(tmp.path(), &StoreOptions::default()).err() {
        Some(Error::DifferentFormat {
            found, expected, ..
        }) => assert_eq!((found, expected), (15, 16)),
        other => panic!("expected a different format, got {other:?}"),
    }
    assert_eq!(std::fs::read(&data).unwrap(), bytes, "left as it was");
    let _ = Value::Null;
}
