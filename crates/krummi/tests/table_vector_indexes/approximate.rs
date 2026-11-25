//! The search through a vector index (VI5, VI6, VI7): the same rows as the exact search when every list is probed and
//! every candidate reranked, exact distances and order otherwise (AC-VI5); the pending rows (AC-VI2); filter first, exact
//! under the threshold and approximate over it (AC-VI6); snapshots (AC-VI7); the widened probe and the null vectors
//! (AC-VI13); the search while the index trains (AC-VI15); the same results from the same file (AC-VI10, on Linux).

use krummi::table::value::Value;
use krummi::{
    ErrorKind, FilterCondition, IndexDefinition, IndexKey, IndexMode, Nearest, Neighbours, Store,
    VectorIndexState, VectorMetric,
};

use super::support::{open, stores};
use super::worker::{DIM, create_index, create_table, fill, point, ready};
use super::{ids, vector};

fn query(k: usize) -> Nearest<'static> {
    let values: Vec<f32> = (0..DIM).map(|d| 31.0 + d as f32).collect();
    Nearest::new(vector(&values), k)
}

fn near(store: &Store, query: &Nearest<'_>) -> Neighbours {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    read.nearest(&table, "near", query).unwrap()
}

fn exact(store: &Store, query: &Nearest<'_>) -> Neighbours {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    read.nearest_exact(&table, "v", VectorMetric::L2, query)
        .unwrap()
}

/// A ready index over 12,000 rows, with the threshold of VI6 lowered so that the index serves the searches.
fn ready_store(store: &Store) {
    store.set_vector_search_threshold(Some(100));
    let mut write = store.write().unwrap();
    create_table(&mut write);
    write.commit().unwrap();
    fill(store, 0..12_000);
    create_index(store);
    ready(store);
}

/// Each row of `found` has the distance the exact search gives it, and they come in the order of VI5.
fn exact_distances(store: &Store, found: &Neighbours) {
    let all = exact(store, &query(10_000));
    for row in &found.rows {
        let same = all
            .rows
            .iter()
            .find(|n| n.key == row.key)
            .expect("a visible row");
        assert_eq!(
            same.distance.to_bits(),
            row.distance.to_bits(),
            "{:?}",
            row.key
        );
    }
    for pair in found.rows.windows(2) {
        assert!(pair[0].distance <= pair[1].distance);
    }
}

/// AC-VI5: probing every list and reranking every candidate gives the exact search; with fewer lists, every row comes
/// with its exact distance in the order of VI5, and the search says approximate.
#[test]
fn ac_vi5_the_index_and_the_exact_search() {
    stores(|store, _| {
        ready_store(store);
        store.set_vector_rerank_all(true);
        let lists = super::worker::info(store).lists.unwrap() as usize;
        let every = query(25).with_probes(lists);
        let found = near(store, &every);
        assert!(!found.exact);
        assert_eq!(
            found,
            Neighbours {
                exact: false,
                ..exact(store, &every)
            }
        );
        store.set_vector_rerank_all(false);
        let few = query(25).with_probes(1);
        let found = near(store, &few);
        assert!(!found.exact);
        assert_eq!(found.rows.len(), 25);
        exact_distances(store, &found);
        // A number of lists above the index's probes them all.
        assert_eq!(
            near(store, &query(25).with_probes(1_000_000)).rows.len(),
            25
        );
    });
}

/// AC-VI2: before the worker processes them, a later search finds the new rows, not the deleted ones, and the new
/// vectors with their exact distances.
#[test]
fn ac_vi2_the_search_sees_the_pending_rows() {
    stores(|store, _| {
        ready_store(store);
        store.pause_vector_work(true);
        let target: Vec<f32> = (0..DIM).map(|d| 31.0 + d as f32).collect();
        let nearest_before = ids(&near(store, &query(1)));
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        // A new row on the query, an old row moved onto it, the nearest row deleted.
        write
            .insert_row(
                &table,
                &[("id", Value::I64(50_000)), ("v", vector(&target))],
            )
            .unwrap();
        write
            .update_row(&table, &[("id", Value::I64(7)), ("v", vector(&target))])
            .unwrap();
        write
            .delete_row(&table, &[Value::I64(nearest_before[0])])
            .unwrap();
        write.commit().unwrap();
        let info = super::worker::info(store);
        assert_eq!(info.pending, 3);
        let found = near(store, &query(5));
        assert_eq!(&ids(&found)[..2], [7, 50_000]);
        assert_eq!((found.rows[0].distance, found.rows[1].distance), (0.0, 0.0));
        assert!(!ids(&found).contains(&nearest_before[0]) || nearest_before[0] == 7);
        exact_distances(store, &found);
        store.pause_vector_work(false);
        ready(store);
        let reflected = near(store, &query(5));
        assert_eq!(
            &ids(&reflected)[..2],
            [7, 50_000],
            "the same rows once the worker reflects them"
        );
        exact_distances(store, &reflected);
    });
}

/// A filter and the ids it passes.
type Class<'a> = (Vec<FilterCondition<'a>>, Box<dyn Fn(i64) -> bool>);

/// AC-VI6: with a secondary index, a filter of each class of VI7 returns only rows that meet it; a filter that passes
/// fewer rows than the threshold gives the exact search over them and says exact, one that passes more says approximate.
#[test]
fn ac_vi6_filter_first() {
    stores(|store, _| {
        ready_store(store);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let by_n = IndexDefinition::new("by_n", vec![IndexKey::new("n")]);
        write
            .create_index(&table, &by_n, IndexMode::CreateNew)
            .unwrap();
        write.commit().unwrap();
        store.set_vector_search_threshold(Some(500));
        let int = Value::I64;
        let few = vec![FilterCondition::Range {
            column: "n",
            lower: std::ops::Bound::Included(int(0)),
            upper: std::ops::Bound::Excluded(int(300)),
        }];
        let many = vec![FilterCondition::Range {
            column: "n",
            lower: std::ops::Bound::Included(int(0)),
            upper: std::ops::Bound::Excluded(int(6_000)),
        }];
        let classes: Vec<Class<'_>> = vec![
            (few.clone(), Box::new(|n| n < 300)),
            (many.clone(), Box::new(|n| n < 6_000)),
            (
                vec![FilterCondition::Equal {
                    column: "n",
                    value: int(42),
                }],
                Box::new(|n| n == 42),
            ),
            (
                vec![FilterCondition::EqualColumns(vec![
                    ("n", int(43)),
                    ("id", int(43)),
                ])],
                Box::new(|n| n == 43),
            ),
            (
                vec![FilterCondition::In {
                    column: "n",
                    values: (0..800).map(int).collect(),
                }],
                Box::new(|n| n < 800),
            ),
            (
                vec![FilterCondition::NotIn {
                    column: "n",
                    values: (0..800).map(int).collect(),
                }],
                Box::new(|n| n >= 800),
            ),
        ];
        for (at, (filter, pass)) in classes.into_iter().enumerate() {
            let q = query(20).with_filter(filter);
            let found = near(store, &q);
            assert!(ids(&found).iter().all(|&id| pass(id)), "case {at}");
            let expected = exact(store, &q);
            if [0, 2, 3].contains(&at) {
                assert!(found.exact, "case {at}");
                assert_eq!(found, expected, "case {at}");
            } else {
                assert!(!found.exact, "case {at}");
                assert_eq!(found.rows.len(), 20, "case {at}");
                exact_distances(store, &found);
            }
        }
    });
}

/// AC-VI7: a read open before other writes change rows and the worker reflects them gives the same result as before.
#[test]
fn ac_vi7_an_open_read_keeps_its_snapshot() {
    stores(|store, _| {
        ready_store(store);
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let before = read.nearest(&table, "near", &query(15)).unwrap();
        let mut write = store.write().unwrap();
        let other = write.open_table("t").unwrap();
        let target: Vec<f32> = (0..DIM).map(|d| 31.0 + d as f32).collect();
        for id in 0..300 {
            write
                .update_row(&other, &[("id", Value::I64(id)), ("v", vector(&target))])
                .unwrap();
        }
        write.commit().unwrap();
        ready(store);
        assert_eq!(read.nearest(&table, "near", &query(15)).unwrap(), before);
        assert_ne!(
            near(store, &query(15)),
            before,
            "a new read sees the change"
        );
    });
}

/// AC-VI13: rows that pass but are far from the lists probed come back k; a filter that k rows meet, one of them with a
/// null vector, gives k − 1 rows and that row does not count for the threshold.
#[test]
fn ac_vi13_the_widened_probe_and_the_null_vectors() {
    stores(|store, _| {
        ready_store(store);
        store.set_vector_search_threshold(Some(50));
        // The query sits by cluster 3; the rows of cluster 12 are far from the list it probes first.
        let far: Vec<Value<'static>> = (0..12_000)
            .filter(|id| id % 16 == 12)
            .map(Value::I64)
            .collect();
        let q = query(30)
            .with_probes(1)
            .with_filter(vec![FilterCondition::In {
                column: "id",
                values: far,
            }]);
        let found = near(store, &q);
        assert!(!found.exact);
        assert_eq!(found.rows.len(), 30);
        assert!(ids(&found).iter().all(|id| id % 16 == 12));
        exact_distances(store, &found);
        // Ten rows meet the filter, one of them without a vector: nine come back, exact under a threshold of nine.
        store.set_vector_search_threshold(Some(9));
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write
            .update_row(&table, &[("id", Value::I64(5)), ("v", Value::Null)])
            .unwrap();
        write.commit().unwrap();
        let ten = vec![FilterCondition::In {
            column: "id",
            values: (0..10).map(Value::I64).collect(),
        }];
        let found = near(store, &query(10).with_filter(ten));
        assert!(found.exact, "the null vector does not count");
        assert_eq!(found.rows.len(), 9);
        assert!(!ids(&found).contains(&5));
    });
}

/// AC-VI15 and VI5: while the index trains, a search is exact and gives the exact search's rows.
#[test]
fn ac_vi15_a_search_while_training_is_exact() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        create_table(&mut write);
        write.commit().unwrap();
        fill(store, 0..9_999);
        create_index(store);
        store.wait_vector_idle();
        let found = near(store, &query(7));
        assert!(found.exact);
        assert_eq!(found, exact(store, &query(7)));
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert_eq!(
            read.vector_index_info(&table, "near").unwrap().state,
            VectorIndexState::Training
        );
    });
}

/// AC-VI10 (on Linux, with its annotation): the same store file, closed and opened again, gives the same approximate
/// and exact results bit for bit; the exact search over the same data loaded into another store too.
#[test]
fn ac_vi10_the_same_file_gives_the_same_results() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    ready_store(&store);
    let q = query(40).with_probes(2);
    let (approximate, exact_before) = (near(&store, &q), exact(&store, &q));
    store.close().unwrap();
    let store = open(tmp.path());
    store.set_vector_search_threshold(Some(100));
    store.wait_vector_idle();
    let bits = |found: &Neighbours| -> Vec<(Vec<Value<'static>>, u64)> {
        found
            .rows
            .iter()
            .map(|n| (n.key.clone(), n.distance.to_bits()))
            .collect()
    };
    assert_eq!(bits(&near(&store, &q)), bits(&approximate));
    assert_eq!(bits(&exact(&store, &q)), bits(&exact_before));
    let other = Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
    let mut write = other.write().unwrap();
    create_table(&mut write);
    write.commit().unwrap();
    fill(&other, 0..12_000);
    assert_eq!(bits(&exact(&other, &q)), bits(&exact_before));
    let _ = point(0);
}

/// The failures of *Validaciones* for a search through an index.
#[test]
fn the_errors_of_a_search_through_an_index() {
    stores(|store, _| {
        ready_store(store);
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let by_n = IndexDefinition::new("by_n", vec![IndexKey::new("n")]);
        write
            .create_index(&table, &by_n, IndexMode::CreateNew)
            .unwrap();
        let kind = |name: &str, q: &Nearest<'_>| write.nearest(&table, name, q).unwrap_err().kind();
        assert_eq!(kind("", &query(1)), ErrorKind::InvalidSize);
        assert_eq!(kind("missing", &query(1)), ErrorKind::DifferentSchema);
        assert_eq!(kind("by_n", &query(1)), ErrorKind::DifferentSchema);
        assert_eq!(
            kind("near", &Nearest::new(Value::Null, 1)),
            ErrorKind::InvalidValue
        );
        assert_eq!(
            kind("near", &Nearest::new(vector(&[1.0]), 1)),
            ErrorKind::InvalidValue
        );
        assert_eq!(kind("near", &query(0)), ErrorKind::InvalidValue);
        assert_eq!(kind("near", &query(10_001)), ErrorKind::InvalidValue);
        assert_eq!(
            kind("near", &query(1).with_probes(0)),
            ErrorKind::InvalidValue
        );
        let bad = query(1).with_filter(vec![FilterCondition::Equal {
            column: "v",
            value: Value::I64(1),
        }]);
        assert_eq!(kind("near", &bad), ErrorKind::InvalidSchema);
        write.delete_table("t").unwrap();
        let deleted = write.nearest(&table, "near", &query(1)).unwrap_err();
        assert_eq!(deleted.kind(), ErrorKind::TreeNotFound);
    });
}
