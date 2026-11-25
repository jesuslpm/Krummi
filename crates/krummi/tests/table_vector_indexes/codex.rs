//! Regressions of the Codex code review of 2026-11-25 (`docs/reviews/tables-vector-indexes/2026-11-25-code.md`).

use std::time::{Duration, Instant};

use krummi::table::value::{ColumnType, Value};
use krummi::{
    Column, IndexMode, Nearest, Schema, Store, StoreOptions, TableMode, VectorIndexDefinition,
    VectorIndexState, VectorMetric, VectorPoint,
};

use super::{f32_vector, ids, vector};

fn one_dimension() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64),
            Column::new("v", ColumnType::Vector(f32_vector(1, false))),
        ],
        &["id"],
    )
}

/// Creates the table `name` with 10,000 rows `v = [id]` and the L2 index `near` over `v`.
fn indexed(store: &Store, name: &str) {
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with(name, TableMode::CreateNew(&one_dimension()))
        .unwrap();
    for id in 0..10_000 {
        let row = [
            ("id", Value::I64(id)),
            ("n", Value::I64(0)),
            ("v", vector(&[id as f32])),
        ];
        write.insert_row(&table, &row).unwrap();
    }
    let definition = VectorIndexDefinition::new("near", "v", VectorMetric::L2);
    write
        .create_vector_index(&table, &definition, IndexMode::CreateNew)
        .unwrap();
    write.commit().unwrap();
}

fn state(store: &Store, name: &str) -> krummi::VectorIndexInfo {
    let read = store.read().unwrap();
    let table = read.open_table(name).unwrap();
    read.vector_index_info(&table, "near").unwrap()
}

fn wait_ready(store: &Store, name: &str) {
    let start = Instant::now();
    loop {
        store.wait_vector_idle();
        let info = state(store, name);
        if info.state == VectorIndexState::Ready && info.pending == 0 {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(120), "{info:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// R1: dropping a column before the vector column, while the worker holds a target it listed before, moves the vector
/// column's place; the worker reads the vector by its column's identity in each snapshot, so a changed row keeps its
/// entry and the search through the index finds it.
#[test]
fn r1_dropping_another_column_keeps_the_entries() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    indexed(&store, "b");
    wait_ready(&store, "b");
    // The worker lists `b` and `a`, then holds while it trains `a`: its target for `b` is from before the drop.
    store.pause_vector_work(true);
    store.hold_vector_work(VectorPoint::Sampled, true);
    indexed(&store, "a");
    store.pause_vector_work(false);
    assert!(store.wait_vector_held(VectorPoint::Sampled));
    let mut write = store.write().unwrap();
    let table = write.open_table("b").unwrap();
    write.drop_column(&table, "n").unwrap();
    write
        .update_row(&table, &[("id", Value::I64(42)), ("v", vector(&[-100.0]))])
        .unwrap();
    write.commit().unwrap();
    store.hold_vector_work(VectorPoint::Sampled, false);
    wait_ready(&store, "b");
    wait_ready(&store, "a");
    let read = store.read().unwrap();
    let table = read.open_table("b").unwrap();
    assert_eq!(
        read.check_vector_index(&table, "near").unwrap(),
        Vec::<String>::new()
    );
    store.set_vector_search_threshold(Some(0));
    store.set_vector_rerank_all(true);
    let lists = state(&store, "b").lists.unwrap() as usize;
    let found = read
        .nearest(
            &table,
            "near",
            &Nearest::new(vector(&[-100.0]), 1).with_probes(lists),
        )
        .unwrap();
    assert_eq!((ids(&found), found.rows[0].distance), (vec![42], 0.0));
}

/// R3: with rows without a vector, the rows with one decide an automatic rebuild: 1,000 vectors among 13,000 rows
/// start none, and the index stays ready.
#[test]
fn r3_rows_without_a_vector_start_no_rebuild() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    indexed(&store, "t");
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    for id in 10_000..22_000 {
        write
            .insert_row(&table, &[("id", Value::I64(id)), ("n", Value::I64(0))])
            .unwrap();
    }
    write.commit().unwrap();
    wait_ready(&store, "t");
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write
        .delete_range(&table, &[], Value::I64(0)..Value::I64(9_000))
        .unwrap();
    write.commit().unwrap();
    wait_ready(&store, "t");
    let after = state(&store, "t");
    assert_eq!(after.state, VectorIndexState::Ready, "{after:?}");
    assert_eq!(after.rows_against_trained, Some(0.1));
}
