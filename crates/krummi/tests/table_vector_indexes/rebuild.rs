//! Rebuilds and the information of a vector index (VI9, VI11): a requested rebuild keeps the searches on the previous
//! generation until it publishes (AC-VI8); automatic rebuilds, requests that do nothing and a request that survives the
//! close (AC-VI16); the indicators that cannot be computed (AC-VI13); the information in each state (AC-VI17).

use std::time::{Duration, Instant};

use krummi::table::value::Value;
use krummi::{
    ErrorKind, Nearest, RebuildThresholds, Store, VectorIndexInfo, VectorIndexState, VectorPoint,
};

use super::support::{open, stores};
use super::vector;
use super::worker::{DIM, create_index, create_table, fill, info, problems, ready};

fn query() -> Nearest<'static> {
    let values: Vec<f32> = (0..DIM).map(|d| 51.0 + d as f32).collect();
    Nearest::new(vector(&values), 10).with_probes(2)
}

fn near(store: &Store) -> krummi::Neighbours {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    read.nearest(&table, "near", &query()).unwrap()
}

fn request(store: &Store) {
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write.rebuild_vector_index(&table, "near").unwrap();
    write.commit().unwrap();
}

fn ready_store(store: &Store, rows: i64) {
    store.set_vector_search_threshold(Some(100));
    let mut write = store.write().unwrap();
    create_table(&mut write);
    write.commit().unwrap();
    fill(store, 0..rows);
    create_index(store);
    ready(store);
}

/// Lets the paused worker train and commit its training, then pauses it before its next batch.
fn trained_and_paused(store: &Store) {
    store.hold_vector_work(VectorPoint::BatchTurn, true);
    store.pause_vector_work(false);
    assert!(
        store.wait_vector_held(VectorPoint::BatchTurn),
        "the training holds the turn"
    );
    store.pause_vector_work(true);
    store.take_vector_turns();
    store.hold_vector_work(VectorPoint::BatchTurn, false);
    let start = Instant::now();
    while store.take_vector_turns().1 == 0 {
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "the training commits"
        );
        std::thread::yield_now();
    }
    store.wait_vector_idle();
}

fn wait_state(store: &Store, state: VectorIndexState) -> VectorIndexInfo {
    let start = Instant::now();
    loop {
        let found = info(store);
        if found.state == state {
            return found;
        }
        assert!(start.elapsed() < Duration::from_secs(120), "{found:?}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// AC-VI8: a requested rebuild leaves the index ready and rebuilding, the searches answer with the previous generation
/// until it publishes and with the new one after it, and the information reports its counters before and after.
#[test]
fn ac_vi8_a_requested_rebuild() {
    stores(|store, _| {
        ready_store(store, 12_000);
        let before = info(store);
        let found = near(store);
        store.pause_vector_work(true);
        request(store);
        let during = info(store);
        assert_eq!(during.state, VectorIndexState::ReadyRebuilding);
        assert_eq!(
            (during.lists, during.saturated, during.largest_list),
            (before.lists, before.saturated, before.largest_list)
        );
        assert_eq!(near(store), found, "the previous generation serves");
        // The new generation trained and building, while the searches use the previous one.
        trained_and_paused(store);
        assert_eq!(info(store).state, VectorIndexState::ReadyRebuilding);
        assert_eq!(near(store), found);
        store.pause_vector_work(false);
        let after = wait_state(store, VectorIndexState::Ready);
        ready(store);
        assert_eq!(after.lists, Some(12));
        assert_eq!(after.rows_against_trained, Some(1.0));
        assert!(after.saturated.is_some() && after.largest_list.is_some());
        assert_eq!(near(store).rows.len(), 10);
    });
}

/// AC-VI16: thresholds lowered under the indicators make the worker rebuild by itself and publish; a request with a
/// rebuild in progress, with the index training or with fewer than 10,000 rows with a vector does nothing; a request
/// committed before a close is done after the store opens again.
#[test]
fn ac_vi16_automatic_rebuilds_and_requests() {
    stores(|store, _| {
        ready_store(store, 12_000);
        store.pause_vector_work(true);
        store.set_vector_rebuild_thresholds(Some(RebuildThresholds {
            saturated: -1.0,
            largest_list: 0.0,
            rows: 100.0,
            growth: 0.0,
        }));
        store.hold_vector_work(VectorPoint::Sampled, true);
        store.pause_vector_work(false);
        // Kick the worker with a write.
        fill(store, 20_000..20_001);
        wait_state(store, VectorIndexState::ReadyRebuilding);
        // A request with one in progress does nothing.
        request(store);
        assert_eq!(info(store).state, VectorIndexState::ReadyRebuilding);
        store.set_vector_rebuild_thresholds(None);
        store.hold_vector_work(VectorPoint::Sampled, false);
        wait_state(store, VectorIndexState::Ready);
        ready(store);
        // Fewer than 10,000 rows with a vector: nothing.
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write
            .delete_range(&table, &[], Value::I64(0)..Value::I64(3_000))
            .unwrap();
        write.commit().unwrap();
        ready(store);
        request(store);
        assert_eq!(info(store).state, VectorIndexState::Ready);
    });
    // Training: nothing; a request before a close is done after opening.
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    create_table(&mut write);
    write.commit().unwrap();
    fill(&store, 0..9_000);
    create_index(&store);
    store.wait_vector_idle();
    request(&store);
    assert_eq!(info(&store).state, VectorIndexState::Training);
    fill(&store, 9_000..12_000);
    ready(&store);
    store.pause_vector_work(true);
    request(&store);
    assert_eq!(info(&store).state, VectorIndexState::ReadyRebuilding);
    store.close().unwrap();
    let store = open(tmp.path());
    wait_state(&store, VectorIndexState::Ready);
    ready(&store);
    store.close().unwrap();
}

/// AC-VI13: deleting every row of a table with a ready index leaves the indicators that cannot be computed as none,
/// and no rebuild.
#[test]
fn ac_vi13_an_emptied_index_has_no_indicators() {
    stores(|store, _| {
        ready_store(store, 12_000);
        for start in (0..12_000).step_by(1_000) {
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            for id in start..start + 1_000 {
                write.delete_row(&table, &[Value::I64(id)]).unwrap();
            }
            write.commit().unwrap();
        }
        ready(store);
        let emptied = info(store);
        assert_eq!(emptied.state, VectorIndexState::Ready);
        assert_eq!((emptied.saturated, emptied.largest_list), (None, None));
        assert_eq!(
            (emptied.lists, emptied.rows_against_trained, emptied.pending),
            (Some(12), Some(0.0), 0)
        );
        assert!(near(store).rows.is_empty());
    });
}

/// AC-VI17: the information in each state, with no generation ready included, and its errors.
#[test]
fn ac_vi17_the_information_in_each_state() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        create_table(&mut write);
        write.commit().unwrap();
        fill(store, 0..12_000);
        store.hold_vector_work(VectorPoint::Sampled, true);
        create_index(store);
        let training = info(store);
        assert_eq!(training.state, VectorIndexState::Training);
        assert_eq!(
            (
                training.lists,
                training.saturated,
                training.largest_list,
                training.rows_against_trained
            ),
            (None, None, None, None)
        );
        store.pause_vector_work(true);
        store.hold_vector_work(VectorPoint::Sampled, false);
        trained_and_paused(store);
        let building = wait_state(store, VectorIndexState::Building);
        assert_eq!((building.lists, building.saturated), (None, None));
        store.pause_vector_work(false);
        let ready_info = wait_state(store, VectorIndexState::Ready);
        assert_eq!(ready_info.lists, Some(12));
        ready(store);
        store.pause_vector_work(true);
        request(store);
        let rebuilding = info(store);
        assert_eq!(rebuilding.state, VectorIndexState::ReadyRebuilding);
        assert_eq!(rebuilding.lists, Some(12));
        store.pause_vector_work(false);
        wait_state(store, VectorIndexState::Ready);
        assert_eq!(problems(store), Vec::<String>::new());
        // Its errors: a missing index, a secondary index's name, a name of a wrong size, a deleted table.
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        assert_eq!(
            write
                .rebuild_vector_index(&table, "missing")
                .unwrap_err()
                .kind(),
            ErrorKind::DifferentSchema
        );
        assert_eq!(
            write.rebuild_vector_index(&table, "").unwrap_err().kind(),
            ErrorKind::InvalidSize
        );
        write.delete_table("t").unwrap();
        assert_eq!(
            write
                .rebuild_vector_index(&table, "near")
                .unwrap_err()
                .kind(),
            ErrorKind::TreeNotFound
        );
    });
}
