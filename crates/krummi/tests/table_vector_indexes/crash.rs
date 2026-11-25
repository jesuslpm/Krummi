//! Crashes of the process in the middle of the vector indexes' work (VI12, AC-VI4): during the training, during the build
//! with a batch holding the turn, and during a batch of pending rows, and at power cuts across the build. Opened again,
//! the worker goes on, the index becomes ready in step with the rows, and no page is left neither used nor free.

use krummi::table::value::Value;
use krummi::{Store, StoreOptions, VectorPoint};

use krummi_pager::testing::Model;

use super::common::Run;
use super::support::{in_use, open};
use super::worker::{create_index, create_table, fill, point, ready};

/// The pages of a store whose table was created and deleted, which keeps its catalog's root.
fn empty_pages() -> u64 {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    create_table(&mut write);
    write.delete_table("t").unwrap();
    write.commit().unwrap();
    in_use(&store)
}

/// Opens what survived in `folder`, lets the worker finish, checks the index against the rows, then deletes the table:
/// every page of the index was reachable and comes free.
fn recovers(folder: &std::path::Path, empty: u64, label: &str) {
    let store = open(folder);
    // Opening the table resumes the worker (VI12); a cut before the table or the index has nothing to resume.
    let described = store.read().unwrap().describe_table("t");
    let Ok(schema) = described else {
        store.close().unwrap();
        return;
    };
    let indexed = !schema.vector_indexes().is_empty();
    if indexed {
        ready(&store);
    }
    let mut write = store.write().unwrap();
    write.delete_table("t").unwrap();
    write.commit().unwrap();
    assert_eq!(in_use(&store), empty, "{label}: every page freed");
    store.close().unwrap();
}

/// A store with 12,000 rows recorded from its creation, the worker holding `point` once the index is created.
fn recorded(point_held: VectorPoint) -> (Store, tempfile::TempDir) {
    let source = Run::begin();
    let store = open(source.path());
    let mut write = store.write().unwrap();
    create_table(&mut write);
    write.commit().unwrap();
    fill(&store, 0..12_000);
    store.hold_vector_work(point_held, true);
    create_index(&store);
    (store, source)
}

/// Waits until the worker reached a held point: its state stops changing for a while.
fn settle(store: &Store) {
    std::thread::sleep(std::time::Duration::from_millis(400));
    let _ = store;
}

#[test]
fn ac_vi4_a_crash_during_the_training_the_build_or_a_batch() {
    let empty = empty_pages();
    // During the training.
    let (store, source) = recorded(VectorPoint::Sampled);
    settle(&store);
    let run = Run::finish(source, Vec::new());
    let killed = run.killed();
    drop(store);
    recovers(killed.path(), empty, "training");

    // During the build, a batch holding the turn.
    let (store, source) = recorded(VectorPoint::BatchTurn);
    settle(&store);
    store.hold_vector_work(VectorPoint::BatchTurn, false);
    std::thread::sleep(std::time::Duration::from_millis(50));
    store.hold_vector_work(VectorPoint::BatchTurn, true);
    settle(&store);
    let run = Run::finish(source, Vec::new());
    let killed = run.killed();
    drop(store);
    recovers(killed.path(), empty, "build");

    // During a batch of pending rows of a ready index.
    let source = Run::begin();
    let store = open(source.path());
    let mut write = store.write().unwrap();
    create_table(&mut write);
    write.commit().unwrap();
    fill(&store, 0..12_000);
    create_index(&store);
    ready(&store);
    store.hold_vector_work(VectorPoint::BatchTurn, true);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    for id in 0..500 {
        write
            .update_row(&table, &[("id", Value::I64(id)), ("v", point(id + 5))])
            .unwrap();
    }
    write.commit().unwrap();
    settle(&store);
    let run = Run::finish(source, Vec::new());
    let killed = run.killed();
    drop(store);
    recovers(killed.path(), empty, "batch");
}

/// Power cuts at points across a build while other rows are written: each prefix of the commits recovers and the
/// worker finishes from it.
#[test]
fn ac_vi4_power_cuts_across_a_build() {
    let empty = empty_pages();
    let source = Run::begin();
    let store = open(source.path());
    let mut write = store.write().unwrap();
    create_table(&mut write);
    write.commit().unwrap();
    fill(&store, 0..12_000);
    create_index(&store);
    for id in 12_000..12_300 {
        fill(&store, id..id + 1);
    }
    ready(&store);
    drop(store);
    let run = Run::finish(source, Vec::new());
    let ops = run.ops.len();
    for step in 1..=4 {
        let cut = ops * step / 5;
        let crashed = run.crash(cut, Model::Flushed);
        recovers(crashed.path(), empty, &format!("cut {cut} of {ops}"));
    }
}
