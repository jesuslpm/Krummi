//! Failures of the vector indexes' worker (VI13, AC-VI14 of the vector indexes PRD): a batch that fails to read or to
//! save, without making the store refuse writes, leaves the index as it was, the searches stay right, and the worker
//! catches up later; a batch whose commit fails to write makes the store refuse writes, the next write of the application
//! reports it, and the worker stops.

use std::io;
use std::time::{Duration, Instant};

use krummi_pager::testing::{self, Action, Op};

use crate::table::value::{ColumnType, Value, VectorElement, VectorType};
use crate::{
    Column, Error, ErrorKind, IndexMode, Nearest, Schema, Store, StoreOptions, TableMode,
    VectorIndexDefinition, VectorIndexState, VectorMetric,
};

const DIM: usize = 4;

fn point(id: i64) -> Value<'static> {
    let values: Vec<f32> = (0..DIM)
        .map(|d| (id % 8) as f32 * 10.0 + ((id * 31) % 17) as f32 / 17.0 + d as f32)
        .collect();
    Value::Vector(
        values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>()
            .into(),
    )
}

fn ready_store(folder: &std::path::Path) -> Store {
    let store = Store::open(folder, &StoreOptions::default()).unwrap();
    store.set_vector_search_threshold(Some(10));
    let ty = VectorType {
        element: VectorElement::F32,
        dimension: DIM as u16,
        cosine: false,
    };
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("v", ColumnType::Vector(ty)),
        ],
        &["id"],
    );
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema))
        .unwrap();
    for id in 0..10_500 {
        write
            .insert_row(&table, &[("id", Value::I64(id)), ("v", point(id))])
            .unwrap();
    }
    let definition = VectorIndexDefinition::new("near", "v", VectorMetric::L2);
    write
        .create_vector_index(&table, &definition, IndexMode::CreateNew)
        .unwrap();
    write.commit().unwrap();
    wait_ready(&store);
    store
}

fn info(store: &Store) -> crate::VectorIndexInfo {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    read.vector_index_info(&table, "near").unwrap()
}

fn wait_ready(store: &Store) {
    let start = Instant::now();
    loop {
        store.wait_vector_idle();
        let info = info(store);
        if info.state == VectorIndexState::Ready && info.pending == 0 {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(120), "{info:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The search of `query` through the index and exactly, which must agree on the rows that pass and their distances.
fn search_is_right(store: &Store) {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let query = Nearest::new(point(3), 20);
    let found = read.nearest(&table, "near", &query).unwrap();
    let exact = read
        .nearest_exact(
            &table,
            "v",
            VectorMetric::L2,
            &Nearest::new(point(3), 10_000),
        )
        .unwrap();
    assert_eq!(found.rows.len(), 20);
    for row in &found.rows {
        let same = exact
            .rows
            .iter()
            .find(|n| n.key == row.key)
            .expect("a visible row");
        assert_eq!(same.distance.to_bits(), row.distance.to_bits());
    }
    assert!(read.check_vector_index(&table, "near").unwrap().is_empty());
}

/// Changes the vectors of 200 rows while the worker is paused, so that they are pending.
fn pending_rows(store: &Store, round: i64) {
    store.pause_vector_work(true);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    for id in 0..200 {
        write
            .update_row(
                &table,
                &[("id", Value::I64(id)), ("v", point(id + round * 7 + 1))],
            )
            .unwrap();
    }
    write.commit().unwrap();
    assert_eq!(info(store).pending, 200);
}

/// VI13: a batch whose save fails with a full disk, memory running out or the maximum size, and one whose read fails
/// for lack of memory, change nothing; the searches stay right, and the worker reconciles the rows later.
#[test]
fn ac_vi14_a_failed_batch_is_retried() {
    let tmp = tempfile::tempdir().unwrap();
    let store = ready_store(tmp.path());
    let faults = [
        Error::DiskFull {
            folder: tmp.path().to_path_buf(),
        },
        Error::OutOfMemory { requested: 8_192 },
        Error::MaxSize {
            max_size: 1 << 20,
            needed: 1 << 21,
        },
    ];
    for (round, fault) in faults.into_iter().enumerate() {
        pending_rows(&store, round as i64);
        store.shared().engine.set_page_fault(Some((3, fault)));
        search_is_right(&store);
        store.pause_vector_work(false);
        let start = Instant::now();
        while store.shared().engine.page_fault_pending() {
            assert!(
                start.elapsed() < Duration::from_secs(60),
                "the batch never met its fault"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        wait_ready(&store);
        search_is_right(&store);
        assert!(store.write().is_ok(), "the store accepts writes");
    }
    // A read of the worker that runs out of memory: its snapshots fail a few times, then it goes on.
    pending_rows(&store, 9);
    let next = testing::count(Op::Snapshot);
    for nth in next..next + 3 {
        testing::inject(Op::Snapshot, nth, Action::Fail(io::ErrorKind::OutOfMemory));
    }
    store.pause_vector_work(false);
    let start = Instant::now();
    while testing::count(Op::Snapshot) < next + 3 {
        assert!(start.elapsed() < Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(2));
    }
    wait_ready(&store);
    testing::reset();
    search_is_right(&store);
    store.close().unwrap();
}

/// T12, VI13: a batch whose commit fails to write makes the store refuse writes: the next write of the application
/// reports it with the input/output class, and the worker stops.
#[test]
fn ac_vi14_a_failed_commit_refuses_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let store = ready_store(tmp.path());
    pending_rows(&store, 0);
    store.shared().engine.checkpoint().unwrap();
    testing::reset();
    for nth in 0..1_000 {
        testing::inject(Op::Write, nth, Action::Fail(io::ErrorKind::Other));
    }
    store.pause_vector_work(false);
    let start = Instant::now();
    while store.shared().engine.accepts_writes() {
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "the batch never committed"
        );
        let _ = store.shared().engine.make_durable();
        std::thread::sleep(Duration::from_millis(2));
    }
    testing::reset();
    assert_eq!(store.write().err().map(|e| e.kind()), Some(ErrorKind::Io));
    let start = Instant::now();
    while store.vector_work_alive() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the worker stops"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    // Reading goes on, and the searches are right with what was durable.
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    assert_eq!(
        read.nearest(&table, "near", &Nearest::new(point(3), 5))
            .unwrap()
            .rows
            .len(),
        5
    );
    drop(read);
    let _ = store.close();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    store.set_vector_search_threshold(Some(10));
    wait_ready(&store);
    search_is_right(&store);
    store.close().unwrap();
}
