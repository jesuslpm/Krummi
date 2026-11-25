//! The pending rows of a vector index (VI3, VI10): every write that inserts, deletes or changes the vector of a row marks
//! it in the same transaction, an update that leaves the vector as it was does not, and the bulk load marks each loaded
//! row or, when it fails whole, nothing (the pending part of AC-VI2 and of AC-VI9).

use krummi::table::value::{ColumnType, Value};
use krummi::{
    BulkLoadError, Column, IndexMode, Schema, Store, TableMode, VectorIndexDefinition,
    VectorIndexState, VectorMetric, WriteTransaction,
};

use super::support::stores;
use super::{f32_vector, insert, vector};

/// A table `t` of `id`, `n`, `tag`, a vector `v` and a second vector `w`, with the rows 0 to 9 and a vector index
/// `near` over `v` and `other` over `w` created after them, so that nothing is pending.
fn table_with_rows(store: &Store) {
    let mut write = store.write().unwrap();
    let mut schema = super::schema(f32_vector(2, false));
    schema
        .columns
        .push(Column::new("w", ColumnType::Vector(f32_vector(2, false))));
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema))
        .unwrap();
    for id in 0..10 {
        insert(
            &mut write,
            &table,
            id,
            Some(id),
            None,
            Some(vector(&[id as f32, 0.0])),
        );
    }
    for (name, column) in [("near", "v"), ("other", "w")] {
        let definition = VectorIndexDefinition::new(name, column, VectorMetric::L2);
        write
            .create_vector_index(&table, &definition, IndexMode::CreateNew)
            .unwrap();
    }
    write.commit().unwrap();
}

fn pending(write: &WriteTransaction<'_>, index: &str) -> u64 {
    let table = write.open_table("t").unwrap();
    write.vector_index_info(&table, index).unwrap().pending
}

fn committed_pending(store: &Store, index: &str) -> u64 {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    read.vector_index_info(&table, index).unwrap().pending
}

/// VI3: inserts, deletes, range deletes and updates that change the vector mark their rows in the write, once each;
/// updates that leave the vector as it was, and identical ones, do not; another index's column marks only that index;
/// an abort marks nothing; the marks commit with the rows.
#[test]
fn ac_vi2_every_row_write_marks_the_rows_it_changes() {
    stores(|store, _| {
        store.pause_vector_work(true);
        table_with_rows(store);
        assert_eq!(committed_pending(store, "near"), 0);
        let info = {
            let read = store.read().unwrap();
            let table = read.open_table("t").unwrap();
            read.vector_index_info(&table, "near").unwrap()
        };
        assert_eq!(info.state, VectorIndexState::Training);

        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let row = |id: i64, n: i64, v: f32| {
            vec![
                ("id", Value::I64(id)),
                ("n", Value::I64(n)),
                ("v", vector(&[v, 0.0])),
            ]
        };
        // An update of `n` alone, an identical update and an upsert with the same vector: nothing pending. The update of
        // `n` alone takes the patch of fixed-width columns, which never changes a vector; the updates of a vector take the
        // merge of the whole row (`Reader::patch`).
        write
            .update_row(&table, &[("id", Value::I64(1)), ("n", Value::I64(100))])
            .unwrap();
        write.update_row(&table, &row(2, 2, 2.0)).unwrap();
        write.upsert_row(&table, &row(3, 30, 3.0)).unwrap();
        assert_eq!(pending(&write, "near"), 0);
        // The vector changes: row 4, by update; row 5, by upsert; the second index's column of row 6.
        write.update_row(&table, &row(4, 4, 40.0)).unwrap();
        write.upsert_row(&table, &row(5, 5, 50.0)).unwrap();
        write
            .update_row(&table, &[("id", Value::I64(6)), ("w", vector(&[1.0, 1.0]))])
            .unwrap();
        assert_eq!((pending(&write, "near"), pending(&write, "other")), (2, 1));
        // Twice the same row counts once.
        write.update_row(&table, &row(4, 4, 41.0)).unwrap();
        // A null vector changes it too.
        write
            .update_row(&table, &[("id", Value::I64(7)), ("v", Value::Null)])
            .unwrap();
        // Inserts and deletes mark every index, a delete of a missing row none.
        insert(&mut write, &table, 20, None, None, None);
        write.upsert_row(&table, &row(21, 21, 21.0)).unwrap();
        write.delete_row(&table, &[Value::I64(8)]).unwrap();
        write.delete_row(&table, &[Value::I64(99)]).unwrap();
        assert_eq!((pending(&write, "near"), pending(&write, "other")), (6, 4));
        write.commit().unwrap();
        assert_eq!(
            (
                committed_pending(store, "near"),
                committed_pending(store, "other")
            ),
            (6, 4)
        );

        // A range delete marks each row it deletes, already pending or not; an abort marks nothing.
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        assert_eq!(
            write
                .delete_range(&table, &[], Value::I64(0)..Value::I64(4))
                .unwrap(),
            4
        );
        assert_eq!(pending(&write, "near"), 10);
        assert_eq!(pending(&write, "other"), 8, "every vector index");
        write.abort();
        assert_eq!(committed_pending(store, "near"), 6);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write
            .delete_range(&table, &[], Value::I64(0)..Value::I64(4))
            .unwrap();
        write.commit().unwrap();
        assert_eq!(committed_pending(store, "near"), 10);

        // Emptying the table empties the pending rows and the index goes back to training (VI10).
        let mut write = store.write().unwrap();
        let table = write.open_table_with("t", TableMode::Empty).unwrap();
        let info = write.vector_index_info(&table, "near").unwrap();
        assert_eq!((info.state, info.pending), (VectorIndexState::Training, 0));
        write.commit().unwrap();
        assert_eq!(committed_pending(store, "near"), 0);
    });
}

/// VI10, E13: the bulk load marks every row it loads; a load that fails whole leaves the pending rows as they were.
#[test]
fn ac_vi9_the_bulk_load_marks_each_loaded_row() {
    stores(|store, _| {
        store.pause_vector_work(true);
        table_with_rows(store);
        let rows: Vec<Result<Vec<(&str, Value<'static>)>, std::convert::Infallible>> = (100..150)
            .map(|id| {
                Ok(vec![
                    ("id", Value::I64(id)),
                    ("v", vector(&[id as f32, 1.0])),
                ])
            })
            .collect();
        assert_eq!(store.bulk_load("t", rows).unwrap(), 50);
        assert_eq!(committed_pending(store, "near"), 50);
        let failing = (200..210).map(|id| -> Result<Vec<(&str, Value<'static>)>, &str> {
            if id == 205 {
                Err("source")
            } else {
                Ok(vec![("id", Value::I64(id))])
            }
        });
        assert!(matches!(
            store.bulk_load("t", failing),
            Err(BulkLoadError::Source(_))
        ));
        assert_eq!(committed_pending(store, "near"), 50);
        let duplicate = (300..310).map(|id| -> Result<Vec<(&str, Value<'static>)>, &str> {
            Ok(vec![("id", Value::I64(if id == 305 { 100 } else { id }))])
        });
        assert!(matches!(
            store.bulk_load("t", duplicate),
            Err(BulkLoadError::Store(_))
        ));
        assert_eq!(committed_pending(store, "near"), 50);
    });
}

/// The information of a vector index fails as Validaciones says: a name without 1 to 1,024 bytes, an index the table
/// does not have or a secondary index's name, and a handle of a table the write deleted.
#[test]
fn the_errors_of_the_information() {
    stores(|store, _| {
        store.pause_vector_work(true);
        table_with_rows(store);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let secondary = krummi::IndexDefinition::new("by_n", vec![krummi::IndexKey::new("n")]);
        write
            .create_index(&table, &secondary, IndexMode::CreateNew)
            .unwrap();
        let kind = |write: &WriteTransaction<'_>, name: &str| {
            write.vector_index_info(&table, name).unwrap_err().kind()
        };
        assert_eq!(kind(&write, ""), krummi::ErrorKind::InvalidSize);
        assert_eq!(kind(&write, "missing"), krummi::ErrorKind::DifferentSchema);
        assert_eq!(kind(&write, "by_n"), krummi::ErrorKind::DifferentSchema);
        write.delete_table("t").unwrap();
        assert_eq!(kind(&write, "near"), krummi::ErrorKind::TreeNotFound);
        let _ = Schema::default();
    });
}
