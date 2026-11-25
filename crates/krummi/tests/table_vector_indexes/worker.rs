//! The worker of the vector indexes (VI3, VI4, VI12, VI13): training from 10,000 rows with a vector and the lists of
//! VI4 (AC-VI15), building while other writes go on (AC-VI3, its acceptance run at 100,000 rows apart), closing and
//! opening in each state (AC-VI17), and emptying or dropping while it works. Crashes are in `crash.rs` (AC-VI4) and the
//! failures of a batch in the crate's `vector_fault_tests.rs` (AC-VI14).

use std::time::{Duration, Instant};

use krummi::table::value::Value;
use krummi::{
    IndexMode, Store, TableMode, VectorIndexDefinition, VectorIndexState, VectorMetric,
    WriteTransaction,
};

use super::support::{in_use, open, stores};
use super::{f32_vector, vector};

/// The dimension of the vectors of these tests.
pub const DIM: u16 = 8;

/// The vector of row `id`: one of 16 clusters, with a small spread.
pub fn point(id: i64) -> Value<'static> {
    let cluster = (id % 16) as f32;
    let spread = ((id * 7919) % 101) as f32 / 101.0;
    let values: Vec<f32> = (0..DIM)
        .map(|d| cluster * 10.0 + if d % 2 == 0 { spread } else { -spread } + d as f32)
        .collect();
    vector(&values)
}

/// Creates the table `t` with the vector column `v` of [`DIM`] elements.
pub fn create_table(write: &mut WriteTransaction<'_>) {
    write
        .open_table_with(
            "t",
            TableMode::CreateNew(&super::schema(f32_vector(DIM, false))),
        )
        .unwrap();
}

/// Inserts the rows `ids` with their points and `n` = `id`, committing every 5,000.
pub fn fill(store: &Store, ids: std::ops::Range<i64>) {
    let ids: Vec<i64> = ids.collect();
    for chunk in ids.chunks(5000) {
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        for &id in chunk {
            let row = [
                ("id", Value::I64(id)),
                ("n", Value::I64(id)),
                ("v", point(id)),
            ];
            write.insert_row(&table, &row).unwrap();
        }
        write.commit().unwrap();
    }
}

/// Creates the vector index `near` (L2) over `v`.
pub fn create_index(store: &Store) {
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let definition = VectorIndexDefinition::new("near", "v", VectorMetric::L2);
    write
        .create_vector_index(&table, &definition, IndexMode::CreateNew)
        .unwrap();
    write.commit().unwrap();
}

pub fn info(store: &Store) -> krummi::VectorIndexInfo {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    read.vector_index_info(&table, "near").unwrap()
}

/// The differences between the index `near` and the rows, as a new read sees them.
pub fn problems(store: &Store) -> Vec<String> {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    read.check_vector_index(&table, "near").unwrap()
}

/// Waits until the worker is idle and the index is ready with nothing pending, then checks it against the rows.
pub fn ready(store: &Store) {
    let start = Instant::now();
    loop {
        store.wait_vector_idle();
        let info = info(store);
        if info.state == VectorIndexState::Ready && info.pending == 0 {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "never ready: {info:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(problems(store), Vec::<String>::new());
}

/// AC-VI15: with 9,999 rows with a vector the index stays in training, and nothing is pending; with the row 10,000 it
/// trains, builds and becomes ready with the 10 lists of VI4 for 10,000 rows.
#[test]
fn ac_vi15_the_index_trains_from_10000_rows() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        create_table(&mut write);
        write.commit().unwrap();
        fill(store, 0..9_999);
        // A row without a vector does not count.
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.insert_row(&table, &[("id", Value::I64(-1))]).unwrap();
        write.commit().unwrap();
        create_index(store);
        store.wait_vector_idle();
        let info_before = info(store);
        assert_eq!(info_before.state, VectorIndexState::Training);
        assert_eq!(
            (
                info_before.lists,
                info_before.saturated,
                info_before.largest_list
            ),
            (None, None, None)
        );
        assert_eq!(info_before.rows_against_trained, None);
        fill(store, 9_999..10_000);
        ready(store);
        let info = info(store);
        assert_eq!(info.lists, Some(10));
        assert_eq!(info.rows_against_trained, Some(1.0));
        assert!(info.saturated.is_some() && info.largest_list.is_some());
    });
}

/// AC-VI15: the lists for 1 M and 1,000,001 rows, with the hook that gives the rows the training counts.
#[test]
fn ac_vi15_the_lists_of_large_tables() {
    for (rows, lists) in [(1_000_000, 1000), (1_000_001, 1000)] {
        let store = Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
        store.set_vector_training_rows(Some(rows));
        let mut write = store.write().unwrap();
        create_table(&mut write);
        write.commit().unwrap();
        fill(&store, 0..10_000);
        create_index(&store);
        let start = Instant::now();
        while info(&store).state != VectorIndexState::Ready {
            store.wait_vector_idle();
            assert!(start.elapsed() < Duration::from_secs(120));
        }
        assert_eq!(info(&store).lists, Some(lists), "{rows}");
        store.close().unwrap();
    }
}

/// VI3, VI4: rows written while the index trains and builds, and after it is ready, end up in it, each with the entry of
/// its vector, through the pending rows; the states pass from training through building to ready.
#[test]
fn ac_vi3_the_index_follows_the_writes() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        create_table(&mut write);
        write.commit().unwrap();
        fill(store, 0..12_000);
        create_index(store);
        let mut seen = vec![info(store).state];
        let mut next = 12_000;
        let start = Instant::now();
        // Writes go on while it builds: inserts, updates of the vector and of another column, deletes.
        while seen.last() != Some(&VectorIndexState::Ready) {
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            for _ in 0..20 {
                let row = [("id", Value::I64(next)), ("v", point(next))];
                write.insert_row(&table, &row).unwrap();
                write
                    .update_row(
                        &table,
                        &[("id", Value::I64(next - 6_000)), ("v", point(next + 3))],
                    )
                    .unwrap();
                write
                    .update_row(
                        &table,
                        &[("id", Value::I64(next - 7_000)), ("n", Value::I64(0))],
                    )
                    .unwrap();
                write
                    .delete_row(&table, &[Value::I64(next - 9_000)])
                    .unwrap();
                next += 1;
            }
            write.commit().unwrap();
            let state = info(store).state;
            if seen.last() != Some(&state) {
                seen.push(state);
            }
            assert!(start.elapsed() < Duration::from_secs(120), "{seen:?}");
        }
        assert_eq!(seen.first(), Some(&VectorIndexState::Training));
        assert_eq!(seen.last(), Some(&VectorIndexState::Ready));
        fill(store, 100_000..100_050);
        ready(store);
    });
}

/// AC-VI17 and VI12: closing the store cleanly while the index trains, while it builds and when it is ready, and opening
/// it again, the worker goes on from where it was and the index becomes ready, as Describe lists it.
#[test]
fn ac_vi17_close_and_open_in_each_state() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    create_table(&mut write);
    write.commit().unwrap();
    fill(&store, 0..12_000);
    // Closed while it trains: the training is abandoned, and done again after opening.
    store.hold_vector_work(krummi::VectorPoint::Sampled, true);
    create_index(&store);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(info(&store).state, VectorIndexState::Training);
    store.close().unwrap();
    // Closed while it builds, a batch holding the turn: the batch commits, and the build goes on after opening.
    let store = open(tmp.path());
    store.hold_vector_work(krummi::VectorPoint::BatchTurn, true);
    let start = Instant::now();
    while info(&store).state == VectorIndexState::Training {
        std::thread::sleep(Duration::from_millis(2));
        assert!(start.elapsed() < Duration::from_secs(120));
    }
    store.close().unwrap();
    let store = open(tmp.path());
    let state = info(&store).state;
    assert!(
        matches!(state, VectorIndexState::Building | VectorIndexState::Ready),
        "{state:?}"
    );
    ready(&store);
    let described = store.read().unwrap().describe_table("t").unwrap();
    assert_eq!(described.vector_indexes()[0].name, "near");
    store.close().unwrap();
    // Ready, it stays ready, and Describe lists it as before.
    let store = open(tmp.path());
    assert_eq!(info(&store).state, VectorIndexState::Ready);
    assert_eq!(
        store.read().unwrap().describe_table("t").unwrap(),
        described
    );
    assert_eq!(problems(&store), Vec::<String>::new());
    store.close().unwrap();
}

/// VI10, VI1: emptying the table sends the index back to training and drops its entries; dropping the index or the
/// table while the worker builds frees every page, and the worker drops what it prepared.
#[test]
fn emptying_and_dropping_while_the_worker_works() {
    stores(|store, _| {
        // The pages of a store whose table was created and deleted, which keeps its catalog's root.
        let mut write = store.write().unwrap();
        create_table(&mut write);
        write.delete_table("t").unwrap();
        write.commit().unwrap();
        let base = in_use(store);
        let mut write = store.write().unwrap();
        create_table(&mut write);
        write.commit().unwrap();
        fill(store, 0..12_000);
        create_index(store);
        ready(store);
        let mut write = store.write().unwrap();
        write.open_table_with("t", TableMode::Empty).unwrap();
        write.commit().unwrap();
        store.wait_vector_idle();
        let emptied = info(store);
        assert_eq!(
            (emptied.state, emptied.pending, emptied.lists),
            (VectorIndexState::Training, 0, None)
        );
        fill(store, 0..12_000);
        ready(store);
        // Dropped while a batch waits with the turn held by this write.
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        for id in 0..2_000 {
            write
                .update_row(&table, &[("id", Value::I64(id)), ("v", point(id + 1))])
                .unwrap();
        }
        write.commit().unwrap();
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.drop_index(&table, "near").unwrap();
        write.delete_table("t").unwrap();
        write.commit().unwrap();
        store.wait_vector_idle();
        assert_eq!(in_use(store), base, "every page freed");
    });
}

/// AC-VI3, acceptance (run in release): a table of 100,000 rows gets its index while another thread writes rows without
/// stop; no batch of the worker holds the turn more than 15 ms, the index passes through training, building and ready,
/// and once the writes end and nothing is pending each row has its entry.
#[test]
#[ignore = "acceptance: 100,000 rows and the 15 ms turn of a batch, in release"]
fn ac_vi3_acceptance_100000_rows_and_the_turn() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    let mut write = store.write().unwrap();
    create_table(&mut write);
    write.commit().unwrap();
    fill(&store, 0..100_000);
    store.take_vector_turns();
    let stop = std::sync::atomic::AtomicBool::new(false);
    let seen = std::thread::scope(|scope| {
        let writer = scope.spawn(|| {
            let mut next = 100_000i64;
            while !stop.load(std::sync::atomic::Ordering::Acquire) {
                let mut write = store.write().unwrap();
                let table = write.open_table("t").unwrap();
                for _ in 0..10 {
                    let row = [("id", Value::I64(next)), ("v", point(next))];
                    write.insert_row(&table, &row).unwrap();
                    write
                        .update_row(
                            &table,
                            &[("id", Value::I64(next - 50_000)), ("v", point(next + 1))],
                        )
                        .unwrap();
                    next += 1;
                }
                write.commit().unwrap();
            }
        });
        create_index(&store);
        let mut seen = vec![info(&store).state];
        let start = Instant::now();
        while seen.last() != Some(&VectorIndexState::Ready) {
            let state = info(&store).state;
            if seen.last() != Some(&state) {
                seen.push(state);
            }
            assert!(start.elapsed() < Duration::from_secs(1200), "{seen:?}");
            std::thread::sleep(Duration::from_millis(1));
        }
        stop.store(true, std::sync::atomic::Ordering::Release);
        writer.join().unwrap();
        seen
    });
    assert_eq!(seen.first(), Some(&VectorIndexState::Training));
    assert!(seen.contains(&VectorIndexState::Building), "{seen:?}");
    ready(&store);
    let (longest, batches) = store.take_vector_turns();
    println!("AC-VI3: {batches} batches, the longest held the turn {longest:?}");
    assert!(longest <= Duration::from_millis(15), "{longest:?}");
    store.close().unwrap();
}
