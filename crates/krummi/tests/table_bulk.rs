//! Bulk load of a table through the public API (tables with schema PRD 7.1, T8): a load is a write transaction of
//! its own that loads every row with the rules of insert and fails whole (E13, AC10); a load with rows in key order
//! or out of order gives the same table as inserting its rows one by one (AC-N3); misuse and the load's timeout come
//! before the catalog (E17).
//!
//! The criteria run on a store on disk and on a temporary one, except the empty load, which compares the files of a
//! store on disk, and the loads of 100,000 rows, which run on disk only.

mod support;

use std::borrow::Cow;
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use krummi::table::BulkLoadError;
use krummi::table::value::{ColumnType, Value};
use krummi::{
    Column, Error, OpenMode, Schema, SequenceMode, Store, TableMode, TreeKind, ValueRule,
};
use support::*;

// ---------------------------------------------------------------------------------------------------------------
// Helpers

/// A row as the tests give it: column names with their values.
type Row = Vec<(&'static str, Value<'static>)>;

/// A table's rows as read: the values of each key that has a row.
type Rows = Vec<(i64, Vec<Value<'static>>)>;

/// Why a test source stops a load.
#[derive(Debug, PartialEq)]
enum Stop {
    Failed(&'static str),
    Cancelled,
}

/// `id i64 not null, name string, n i32 not null default 7`, primary key `id`.
fn people() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String),
            Column::new("n", ColumnType::I32)
                .not_null()
                .with_default(Value::I32(7)),
        ],
        &["id"],
    )
}

fn row(id: i64) -> Row {
    vec![
        ("id", Value::I64(id)),
        ("name", Value::String(Cow::Owned(format!("row {id}")))),
        ("n", Value::I32((id % 1000) as i32)),
    ]
}

/// The rows of `ids` as an infallible source.
fn rows(ids: impl IntoIterator<Item = i64>) -> impl Iterator<Item = Result<Row, Infallible>> {
    ids.into_iter().map(|id| Ok(row(id)))
}

/// A source that fails the test if the load reads it.
fn never_read() -> impl Iterator<Item = Result<Row, Infallible>> {
    std::iter::from_fn(|| panic!("the load read its source"))
}

/// Creates the table `name` with `people` and the rows of `ids`, and commits.
fn create_with(store: &Store, name: &str, ids: impl IntoIterator<Item = i64>) {
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with(name, TableMode::CreateNew(&people()))
        .unwrap();
    for id in ids {
        write.insert_row(&table, &row(id)).unwrap();
    }
    write.commit().unwrap();
}

/// The row count of the table `name` and its rows with a key in `ids`, as a new read sees them.
fn read_rows(store: &Store, name: &str, ids: impl IntoIterator<Item = i64>) -> (u64, Rows) {
    let read = store.read().unwrap();
    let table = read.open_table(name).unwrap();
    let count = read.row_count(&table).unwrap();
    let mut found = Vec::new();
    for id in ids {
        if let Some(row) = read.get_row(&table, &[Value::I64(id)]).unwrap() {
            found.push((id, row.into_owned().values().to_vec()));
        }
    }
    (count, found)
}

/// What a failing load of AC10 must leave as it was: the row count and rows of `t` and the next value of `s`.
type State = ((u64, Rows), i64);

/// Creates the table `t` with the rows 1 to 3 and the sequence `s` with 5 values handed out, commits, and returns
/// their state.
fn with_rows_and_sequence(store: &Store) -> State {
    create_with(store, "t", 1..=3);
    let mut write = store.write().unwrap();
    let sequence = write
        .open_sequence_with("s", SequenceMode::CreateNew(None))
        .unwrap();
    for _ in 0..5 {
        write.next_value(&sequence).unwrap();
    }
    write.commit().unwrap();
    state(store)
}

/// The state of `t` and `s`: the next value of `s` is taken in a write that aborts, so it hands it out again.
fn state(store: &Store) -> State {
    let mut write = store.write_with_timeout(Duration::ZERO).unwrap();
    let sequence = write.open_sequence("s").unwrap();
    let next = write.next_value(&sequence).unwrap();
    write.abort();
    (read_rows(store, "t", KEYS), next)
}

/// Asserts that another write gets its turn at once.
#[track_caller]
fn turn_is_free(store: &Store) {
    store.write_with_timeout(Duration::ZERO).unwrap().abort();
}

/// The engine's error of a failed load.
#[track_caller]
fn store_error<E: std::fmt::Debug>(error: BulkLoadError<E>) -> Error {
    match error {
        BulkLoadError::Store(error) => error,
        other => panic!("expected an error of the engine, got {other:?}"),
    }
}

/// The table and key text of a duplicate-key error, after checking its folder.
#[track_caller]
fn duplicate(error: Error, folder: Option<&Path>) -> (String, String) {
    match error {
        Error::DuplicateKey {
            folder: f,
            table,
            key,
        } => {
            assert_eq!(f, folder_of(folder));
            (table, key)
        }
        other => panic!("expected a duplicate key, got {other}"),
    }
}

/// The table, column and rule of an invalid-value error, after checking its folder.
#[track_caller]
fn invalid_value(
    error: Error,
    folder: Option<&Path>,
) -> (Option<String>, Option<String>, ValueRule) {
    match error {
        Error::InvalidValue {
            folder: f,
            table,
            column,
            sequence,
            rule,
        } => {
            assert_eq!(f, folder_of(folder));
            assert_eq!(sequence, None);
            (table, column, rule)
        }
        other => panic!("expected an invalid value, got {other}"),
    }
}

/// The keys the failing loads of AC10 try, around the table's own rows 1 to 3.
const KEYS: std::ops::RangeInclusive<i64> = 0..=40;

// ---------------------------------------------------------------------------------------------------------------
// A load that succeeds

/// E13: a load of a table with rows adds its rows, commits them for every later transaction and reports how many.
#[test]
fn a_load_adds_its_rows_to_a_table_with_rows_and_reports_how_many() {
    stores(|store, _| {
        create_with(store, "t", 1..=3);
        let loaded = store.bulk_load("t", rows((10..=19).rev())).unwrap();
        assert_eq!(loaded, 10);
        let (count, found) = read_rows(store, "t", KEYS);
        assert_eq!(count, 13);
        let expected: Rows = (1..=3)
            .chain(10..=19)
            .map(|id| (id, read_rows_of(&row(id))))
            .collect();
        assert_eq!(found, expected);
        turn_is_free(store);
    });
}

/// The values a read gives for a row given whole in schema order.
fn read_rows_of(row: &Row) -> Vec<Value<'static>> {
    row.iter().map(|(_, value)| value.clone()).collect()
}

// ---------------------------------------------------------------------------------------------------------------
// AC10: a load that fails before its commit changes nothing

/// AC10: a row with a duplicate key (of a row of the table or of an earlier row of the load), an invalid row and an
/// invalid last row abort the whole load with the row's error; the table, its row count and a sequence are as
/// before and another write gets its turn.
#[test]
fn a_failing_row_aborts_the_whole_load() {
    stores(|store, folder| {
        let before = with_rows_and_sequence(store);
        let wrong_type =
            |id: i64| -> Row { vec![("id", Value::I64(id)), ("name", Value::I64(id))] };

        // A row whose key the table has.
        let error = store.bulk_load("t", rows([10, 11, 2, 12])).unwrap_err();
        assert_eq!(
            duplicate(store_error(error), folder),
            ("t".to_owned(), "(2)".to_owned())
        );
        assert_eq!(state(store), before);
        turn_is_free(store);

        // A row whose key an earlier row of the load has.
        let error = store
            .bulk_load("t", rows([10, 11, 12, 11, 13]))
            .unwrap_err();
        assert_eq!(
            duplicate(store_error(error), folder),
            ("t".to_owned(), "(11)".to_owned())
        );
        assert_eq!(state(store), before);
        turn_is_free(store);

        // An invalid row in the middle.
        let source = rows(10..=12)
            .chain([Ok(wrong_type(13))])
            .chain(rows(14..=16));
        let error = store.bulk_load("t", source).unwrap_err();
        assert_eq!(
            invalid_value(store_error(error), folder),
            (
                Some("t".to_owned()),
                Some("name".to_owned()),
                ValueRule::WrongType
            )
        );
        assert_eq!(state(store), before);
        turn_is_free(store);

        // An invalid last row.
        let source = rows(10..=30).chain([Ok(wrong_type(31))]);
        let error = store.bulk_load("t", source).unwrap_err();
        assert_eq!(
            invalid_value(store_error(error), folder),
            (
                Some("t".to_owned()),
                Some("name".to_owned()),
                ValueRule::WrongType
            )
        );
        assert_eq!(state(store), before);
        turn_is_free(store);

        // A duplicate last row.
        let error = store
            .bulk_load("t", rows((10..=30).chain([3])))
            .unwrap_err();
        assert_eq!(
            duplicate(store_error(error), folder),
            ("t".to_owned(), "(3)".to_owned())
        );
        assert_eq!(state(store), before);
        turn_is_free(store);
    });
}

/// AC10: a source that fails aborts the whole load, which returns the source's error; the table, its row count
/// and a sequence are as before and another write gets its turn.
#[test]
fn a_failing_source_aborts_the_load_with_its_error() {
    stores(|store, _| {
        let before = with_rows_and_sequence(store);
        let source = rows(10..=15)
            .map(|row| row.map_err(|never| match never {}))
            .chain([Err(Stop::Failed("the file is gone"))])
            .chain(rows(16..=20).map(|row| row.map_err(|never| match never {})));
        match store.bulk_load("t", source).unwrap_err() {
            BulkLoadError::Source(stop) => assert_eq!(stop, Stop::Failed("the file is gone")),
            other => panic!("expected the source's error, got {other:?}"),
        }
        assert_eq!(state(store), before);
        turn_is_free(store);
    });
}

/// AC10: the application cancels a load from another thread through its source, which stops with an error; the load
/// aborts whole and returns that error; the table, its row count and a sequence are as before and another write
/// gets its turn.
#[test]
fn a_load_cancelled_through_its_source_aborts_whole() {
    stores(|store, _| {
        let before = with_rows_and_sequence(store);
        let cancel = AtomicBool::new(false);
        let (started, wait) = mpsc::channel();
        let result = thread::scope(|scope| {
            let cancel = &cancel;
            scope.spawn(move || {
                wait.recv().unwrap();
                cancel.store(true, Ordering::SeqCst);
            });
            // The source owns the sender, so that the canceller stops waiting if the load drops the source.
            let mut next = 10;
            let source = std::iter::from_fn(move || {
                if cancel.load(Ordering::SeqCst) {
                    return Some(Err(Stop::Cancelled));
                }
                if next == 20 {
                    started.send(()).unwrap();
                }
                next += 1;
                Some(Ok(row(next - 1)))
            });
            store.bulk_load("t", source)
        });
        match result.unwrap_err() {
            BulkLoadError::Source(stop) => assert_eq!(stop, Stop::Cancelled),
            other => panic!("expected the cancellation, got {other:?}"),
        }
        assert_eq!(state(store), before);
        turn_is_free(store);
    });
}

/// AC10: a panic in the source aborts the whole load; the table, its row count and a sequence are as before and
/// another write gets its turn.
#[test]
fn a_panic_in_the_source_aborts_the_load() {
    stores(|store, _| {
        let before = with_rows_and_sequence(store);
        let source = rows(10..=20).chain(std::iter::from_fn(|| panic!("the source panicked")));
        let outcome = catch_unwind(AssertUnwindSafe(|| store.bulk_load("t", source)));
        assert!(outcome.is_err());
        assert_eq!(state(store), before);
        turn_is_free(store);
    });
}

/// A row of a load whose slice the load takes through `AsRef`: a panic there is a panic of the load itself, not of
/// its source.
enum LoadRow {
    Good(Row),
    Panics,
}

impl AsRef<[(&'static str, Value<'static>)]> for LoadRow {
    fn as_ref(&self) -> &[(&'static str, Value<'static>)] {
        match self {
            LoadRow::Good(row) => row,
            LoadRow::Panics => panic!("the row panicked"),
        }
    }
}

/// AC10: a panic in the load itself, here when it takes a row's columns, aborts the whole load; the table, its row
/// count and a sequence are as before and another write gets its turn.
#[test]
fn a_panic_in_the_load_aborts_it() {
    stores(|store, _| {
        let before = with_rows_and_sequence(store);
        let source = (10..=20)
            .map(|id| LoadRow::Good(row(id)))
            .chain([LoadRow::Panics, LoadRow::Good(row(21))])
            .map(Ok::<_, Infallible>);
        let outcome = catch_unwind(AssertUnwindSafe(|| store.bulk_load("t", source)));
        assert!(outcome.is_err());
        assert_eq!(state(store), before);
        turn_is_free(store);
    });
}

/// The error of a source that fails, with a message.
#[derive(Debug)]
struct SourceFailed;

impl std::fmt::Display for SourceFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the file is gone")
    }
}

impl std::error::Error for SourceFailed {}

/// An error of the source with a cause of its own.
#[derive(Debug)]
struct ReadFailed(SourceFailed);

impl std::fmt::Display for ReadFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cannot read the rows")
    }
}

impl std::error::Error for ReadFailed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// E13: the chain of a load's error goes on with the cause of the source's error, so a reporter that prints every
/// source shows each message once.
#[test]
fn the_chain_of_a_failing_source_goes_on_with_its_cause() {
    stores(|store, _| {
        create_with(store, "t", 1..=3);
        let source = rows(10..=12)
            .map(|row| row.map_err(|never| match never {}))
            .chain([Err(ReadFailed(SourceFailed))]);
        let error = store.bulk_load("t", source).unwrap_err();
        assert_eq!(
            error.to_string(),
            "the source of the bulk load failed: cannot read the rows"
        );
        let cause = std::error::Error::source(&error).expect("the cause of the source's error");
        assert_eq!(cause.to_string(), "the file is gone");
        assert!(cause.source().is_none());
    });
}

/// E13: the message of a load that its source failed names the source's error.
#[test]
fn the_message_of_a_failing_source_includes_its_error() {
    stores(|store, _| {
        create_with(store, "t", 1..=3);
        let source = rows(10..=12)
            .map(|row| row.map_err(|never| match never {}))
            .chain([Err(SourceFailed)]);
        let error = store.bulk_load("t", source).unwrap_err();
        assert_eq!(
            error.to_string(),
            "the source of the bulk load failed: the file is gone"
        );
        // The message already holds the source's error, so the chain goes on with that error's own source and
        // reporters that print every source show it once.
        assert!(std::error::Error::source(&error).is_none());
        assert_eq!(read_rows(store, "t", KEYS).0, 3);
    });
}

/// AC10: an empty load changes nothing: it reports 0 rows and the store's files stay as they were.
#[test]
fn an_empty_load_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    create_with(&store, "t", 1..=3);
    store.close().unwrap();
    let changed = files_change(tmp.path(), |store| {
        assert_eq!(store.bulk_load("t", rows([])).unwrap(), 0);
        turn_is_free(store);
    });
    assert!(!changed);
}

// ---------------------------------------------------------------------------------------------------------------
// AC10 and AC-N3: 100,000 rows give the same table as inserting them one by one

/// Loads the rows of `ids` into one table and inserts them one by one into another, and checks that both tables
/// have the same rows and that the load reported every row.
fn load_equals_single_inserts(ids: &[i64]) {
    let tmp = tempfile::tempdir().unwrap();
    let store = open(tmp.path());
    create_with(&store, "single", ids.iter().copied());
    create_with(&store, "loaded", []);
    let loaded = store
        .bulk_load("loaded", rows(ids.iter().copied()))
        .unwrap();
    assert_eq!(loaded, ids.len() as u64);
    let range = 0..ids.len() as i64;
    let single = read_rows(&store, "single", range.clone());
    let bulk = read_rows(&store, "loaded", range);
    assert_eq!(bulk.0, ids.len() as u64);
    assert_eq!(bulk.1.len(), ids.len());
    assert_eq!(bulk, single);
    store.close().unwrap();
}

/// AC10: 100,000 rows out of key order give the same table as inserting them one by one, and the load reports
/// 100,000.
#[test]
fn rows_out_of_key_order_give_the_table_of_single_inserts() {
    let mut ids: Vec<i64> = (0..100_000).collect();
    Lcg(7).shuffle(&mut ids);
    load_equals_single_inserts(&ids);
}

/// AC-N3: 100,000 rows in key order give the same table as inserting them one by one.
#[test]
fn rows_in_key_order_give_the_table_of_single_inserts() {
    let ids: Vec<i64> = (0..100_000).collect();
    load_equals_single_inserts(&ids);
}

// ---------------------------------------------------------------------------------------------------------------
// E13 and E17: the write turn and the catalog

/// E13 and T5: a thread that already holds the write turn cannot start a load, with or without a limit, and fails at
/// once with misuse, before the catalog (E17: here the table does not exist); a read open on the same thread does
/// not prevent a load.
#[test]
fn a_thread_holding_the_write_turn_cannot_start_a_load() {
    stores(|store, folder| {
        create_with(store, "t", 1..=3);
        let write = store.write().unwrap();
        misuse(
            store_error(store.bulk_load("missing", never_read()).unwrap_err()),
            folder,
        );
        let error = store
            .bulk_load_with_timeout("missing", never_read(), Duration::from_secs(60))
            .unwrap_err();
        misuse(store_error(error), folder);
        write.abort();

        let read = store.read().unwrap();
        assert_eq!(store.bulk_load("t", rows([10])).unwrap(), 1);
        drop(read);
        assert_eq!(read_rows(store, "t", KEYS).0, 4);
    });
}

/// E13 and T4: a load that does not get its turn within its limit fails with a timeout carrying the limit, before
/// the catalog (E17: here the table does not exist) and without reading its source.
#[test]
fn a_load_that_does_not_get_its_turn_in_time_times_out() {
    stores(|store, folder| {
        let limit = Duration::from_millis(20);
        let (held, wait_held) = mpsc::channel();
        let (done, wait_done) = mpsc::channel::<()>();
        let result = thread::scope(|scope| {
            scope.spawn(move || {
                let write = store.write().unwrap();
                held.send(()).unwrap();
                wait_done.recv().unwrap();
                write.abort();
            });
            wait_held.recv().unwrap();
            let result = store.bulk_load_with_timeout("missing", never_read(), limit);
            done.send(()).unwrap();
            result
        });
        match store_error(result.unwrap_err()) {
            Error::Timeout {
                folder: f,
                limit: l,
            } => {
                assert_eq!(f, folder_of(folder));
                assert_eq!(l, limit);
            }
            other => panic!("expected a timeout, got {other}"),
        }
        turn_is_free(store);
    });
}

/// E13 and T4: a load that gets its turn within its limit, after another write held it for a while, loads its rows,
/// commits them and reports how many.
#[test]
fn a_load_that_gets_its_turn_in_time_commits() {
    stores(|store, _| {
        create_with(store, "t", 1..=3);
        let (held, wait_held) = mpsc::channel();
        let loaded = thread::scope(|scope| {
            scope.spawn(move || {
                let write = store.write().unwrap();
                held.send(()).unwrap();
                thread::sleep(Duration::from_millis(50));
                write.abort();
            });
            wait_held.recv().unwrap();
            store.bulk_load_with_timeout("t", rows(10..=14), Duration::from_secs(60))
        });
        assert_eq!(loaded.unwrap(), 5);
        let (count, found) = read_rows(store, "t", KEYS);
        assert_eq!(count, 8);
        let ids: Vec<i64> = found.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, [1, 2, 3, 10, 11, 12, 13, 14]);
        turn_is_free(store);
    });
}

/// E13: a load of a missing table fails with tree not found, and of the name of another class with a different tree
/// type, without reading its source; another write then gets its turn.
#[test]
fn a_load_of_a_missing_table_or_of_another_class_fails() {
    stores(|store, folder| {
        let error = store.bulk_load("missing", never_read()).unwrap_err();
        assert_eq!(not_found(store_error(error), folder), "missing");
        turn_is_free(store);

        let mut write = store.write().unwrap();
        write.open_tree_with("tree", OpenMode::CreateNew).unwrap();
        write.commit().unwrap();
        let error = store.bulk_load("tree", never_read()).unwrap_err();
        assert_eq!(
            different(store_error(error), folder),
            ("tree".to_owned(), TreeKind::Normal, TreeKind::Table)
        );
        turn_is_free(store);
    });
}
