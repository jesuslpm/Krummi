//! Internal tests of the row reads PRD (`docs/prd/krummi/tables/tables-row-reads.prd.md`): reading by key and scanning do not
//! check a row's content (L1, L2), update and upsert still check the row they replace (L3), and a panic of a column
//! read leaves the transaction intact (L2). The malformed rows are written through the tree, so that the commit
//! computes their pages' integrity codes over them, as a Krummi defect or a file altered with its codes recomputed
//! would leave them.

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::table::key::encode_key;
use crate::table::rows::RowCount;
use crate::table::schema::version_key;
use crate::table::value::{ColumnType, Value};
use crate::{Column, Error, ErrorKind, Result, Schema, Store, StoreOptions, TableMode, TableRow};

pub(super) type Row = Vec<(&'static str, Value<'static>)>;

/// A change of a row's stored bytes.
type Change<'a> = &'a dyn Fn(&mut Vec<u8>);

/// The rows of every table here are `0..ROWS`; the one made malformed is `BAD`.
pub(super) const ROWS: i64 = 10;
pub(super) const BAD: i64 = 5;

/// A temporary store with the table `t` of `schema` and `rows`, committed.
pub(super) fn store_with(schema: &Schema, rows: impl IntoIterator<Item = Row>) -> Store {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(schema))
        .unwrap();
    for row in rows {
        write.insert_row(&table, &row).unwrap();
    }
    write.commit().unwrap();
    store
}

/// The stored key of the row `id` of a table keyed by one `i64` column.
fn id_key(id: i64) -> Vec<u8> {
    encode_key(&[ColumnType::I64], &[Value::I64(id)]).unwrap()
}

/// The stored bytes of the row of `key` in the table `t`.
fn stored(store: &Store, key: &[u8]) -> Vec<u8> {
    let read = store.read().unwrap();
    let rows = read
        .table_trees(&read.open_table("t").unwrap())
        .unwrap()
        .rows;
    krummi_tree::get(&read.inner, rows, key)
        .unwrap()
        .unwrap()
        .to_vec()
}

/// Rewrites the stored row of `key` in the table `t` with `change` applied to its bytes, in a write that commits.
fn alter(store: &Store, key: &[u8], change: impl FnOnce(&mut Vec<u8>)) {
    let mut bytes = stored(store, key);
    change(&mut bytes);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let (rows_root, _) = write.live_rows(&table).unwrap();
    let update = krummi_tree::set(&mut write.inner, rows_root, key, &bytes).unwrap();
    write
        .apply_rows(&table, update.root, RowCount::Kept)
        .unwrap();
    write.commit().unwrap();
}

/// Moves the stored row of `from` in the table `t` under the key `to`, in a write that commits.
fn rekey(store: &Store, from: &[u8], to: &[u8]) {
    let bytes = stored(store, from);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let (rows_root, _) = write.live_rows(&table).unwrap();
    let update = krummi_tree::remove(&mut write.inner, rows_root, from).unwrap();
    write
        .apply_rows(&table, update.root, RowCount::Removed(1))
        .unwrap();
    let (rows_root, _) = write.live_rows(&table).unwrap();
    let update = krummi_tree::try_add(&mut write.inner, rows_root, to, &bytes).unwrap();
    write
        .apply_rows(&table, update.root, RowCount::Added)
        .unwrap();
    write.commit().unwrap();
}

/// The page that holds the row of `key` in the table `t`: its leaf, the rows here being small.
fn row_page(store: &Store, key: &[u8]) -> u64 {
    let read = store.read().unwrap();
    let rows = read
        .table_trees(&read.open_table("t").unwrap())
        .unwrap()
        .rows;
    krummi_tree::leaf_of(&read.inner, rows, key).unwrap()
}

pub(super) fn damaged_page<T: std::fmt::Debug>(outcome: Result<T>) -> u64 {
    match outcome.unwrap_err() {
        Error::Damaged {
            page: Some(page), ..
        } => page,
        other => panic!("expected damage with a page, got {other}"),
    }
}

/// Whether `read` panics.
pub(super) fn panics<T>(read: impl FnOnce() -> T) -> bool {
    catch_unwind(AssertUnwindSafe(read)).is_err()
}

/// `id i64 not null, n i32, s string`, primary key `id`: the text is the last stored byte of a row.
pub(super) fn texts() -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I32),
            Column::new("s", ColumnType::String),
        ],
        &["id"],
    )
}

pub(super) fn text_row(id: i64) -> Row {
    vec![
        ("id", Value::I64(id)),
        ("n", Value::I32(i32::try_from(id).unwrap() * 10)),
        ("s", Value::String(format!("row{id}").into())),
    ]
}

/// A store of [`texts`] whose row `BAD` has a text that is not UTF-8, `"row\xFF"`.
pub(super) fn bad_text_store() -> Store {
    let store = store_with(&texts(), (0..ROWS).map(text_row));
    alter(&store, &id_key(BAD), |bytes| {
        *bytes.last_mut().unwrap() = 0xFF
    });
    store
}

/// L2 on the row `BAD` of [`bad_text_store`]: its other columns read their values, its text read as text panics, and
/// read as its bytes gives them as they are.
fn bad_text_reads(row: &TableRow<'_>) {
    assert_eq!(row.i64(0), Some(BAD));
    assert_eq!(row.get("n"), Some(Value::I32(50)));
    assert_eq!(row.text_bytes(2).as_deref(), Some(&b"row\xFF"[..]));
    assert!(panics(|| row.text(2)), "text");
    assert!(panics(|| row.value(2)), "value");
    assert!(panics(|| row.get("s")), "get");
}

/// Reads the rows of `$table` in `$tx`, a read or a write transaction, by key and in scans from both ends and lent,
/// passing each to `$check` and comparing the ids `$id` gives in key order with `$ids`.
macro_rules! reads_and_scans {
    ($tx:expr, $table:expr, $ids:expr, $id:expr, $check:expr) => {{
        let ids: Vec<i64> = $ids;
        for front in [true, false] {
            let mut scan = $tx.range_rows(&$table, &[], ..).unwrap();
            let mut got = Vec::new();
            while let Some(row) = if front { scan.next() } else { scan.next_back() } {
                let row = row.unwrap();
                $check(&row);
                got.push($id(&row));
            }
            if !front {
                got.reverse();
            }
            assert_eq!(got, ids, "front {front}");
        }
        let mut scan = $tx.range_rows(&$table, &[], ..).unwrap();
        let mut got = Vec::new();
        while let Some(row) = scan.next_row().unwrap() {
            $check(row);
            got.push($id(row));
        }
        assert_eq!(got, ids, "lent");
    }};
}

/// A lent scan (`next_row`) of every row of [`texts`] in `$tx` that must fail at `BAD` with damage of `$page`: the
/// rows before it, the failure once, then nothing. The rows before it are of the current version, so the scan meets
/// `BAD` with the last of them lent.
macro_rules! lent_scan_fails {
    ($tx:expr, $table:expr, $page:expr, $what:expr) => {{
        let mut scan = $tx.range_rows(&$table, &[], ..).unwrap();
        for id in 0..BAD {
            let row = scan.next_row().unwrap().unwrap();
            assert_eq!(row.i64(0), Some(id), "{}, lent", $what);
        }
        assert_eq!(
            damaged_page(scan.next_row().map(drop)),
            $page,
            "{}, lent",
            $what
        );
        assert!(
            scan.next_row().unwrap().is_none(),
            "{}: the lent scan ends",
            $what
        );
    }};
}

/// AC-L1: a row whose text is not UTF-8, in a page whose integrity holds, is returned by a read by its key and by
/// scans from both ends, in a read and in a write transaction; its other columns read their values, its text read as
/// text panics and read as its bytes gives them as they are.
#[test]
fn ac_l1_a_text_that_is_not_utf8_is_returned_and_panics_only_when_read_as_text() {
    let store = bad_text_store();
    let check = |row: &TableRow<'_>| {
        if row.i64(0) == Some(BAD) {
            bad_text_reads(row);
        }
    };
    let id = |row: &TableRow<'_>| row.i64(0).unwrap();
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    bad_text_reads(&read.get_row(&table, &[Value::I64(BAD)]).unwrap().unwrap());
    reads_and_scans!(read, table, (0..ROWS).collect(), id, check);
    drop(read);
    let write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    bad_text_reads(&write.get_row(&table, &[Value::I64(BAD)]).unwrap().unwrap());
    reads_and_scans!(write, table, (0..ROWS).collect(), id, check);
}

/// AC-L1 on a row of an older schema version (R2 of the third Codex code review): renaming a column makes the row
/// with the text that is not UTF-8 an older one, which still reads its other columns, panics when its text is read as
/// text and gives its bytes as they are, by key and in scans.
#[test]
fn ac_l1_an_older_row_with_a_text_that_is_not_utf8_reads_it_as_bytes() {
    let store = bad_text_store();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write.rename_column(&table, "n", "number").unwrap();
    write.commit().unwrap();
    let reads = |row: &TableRow<'_>| {
        if row.i64(0) == Some(BAD) {
            assert_eq!(row.i32(1), Some(50));
            assert_eq!(row.text_bytes(2).as_deref(), Some(&b"row\xFF"[..]));
            assert!(panics(|| row.text(2)), "text");
            assert!(panics(|| row.value(2)), "value");
        }
    };
    let id = |row: &TableRow<'_>| row.i64(0).unwrap();
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    reads(&read.get_row(&table, &[Value::I64(BAD)]).unwrap().unwrap());
    reads_and_scans!(read, table, (0..ROWS).collect(), id, reads);
    drop(read);
    let write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    reads(&write.get_row(&table, &[Value::I64(BAD)]).unwrap().unwrap());
    reads_and_scans!(write, table, (0..ROWS).collect(), id, reads);
}

/// AC-L2: a `bool` that is neither 0 nor 1, and a variable value whose offset is past the row, are returned by a read
/// by key and by scans, in a read and in a write transaction.
#[test]
fn ac_l2_a_malformed_value_or_offset_is_returned() {
    // `id` in the key, `flag` the only stored byte after the header: no nulls, so no null bitmap.
    let bools = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("flag", ColumnType::Bool).not_null(),
        ],
        &["id"],
    );
    let bool_row =
        |id: i64| -> Row { vec![("id", Value::I64(id)), ("flag", Value::Bool(id % 2 == 0))] };
    // Two variable columns: after the header, the end of `a`, then `a` and `s`.
    let offsets = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("a", ColumnType::Bytes).not_null(),
            Column::new("s", ColumnType::String).not_null(),
        ],
        &["id"],
    );
    let offset_row = |id: i64| -> Row {
        vec![
            ("id", Value::I64(id)),
            ("a", Value::Bytes(vec![7; 3].into())),
            ("s", Value::String(format!("row{id}").into())),
        ]
    };
    let cases: [(Store, Change<'_>); 2] = [
        (
            store_with(&bools, (0..ROWS).map(bool_row)),
            &|b: &mut Vec<u8>| {
                assert_eq!((b.len(), b[3]), (4, 0), "the flag's byte");
                b[3] = 2;
            },
        ),
        (
            store_with(&offsets, (0..ROWS).map(offset_row)),
            &|b: &mut Vec<u8>| {
                assert_eq!(b[3], 3, "the end of a");
                b[3] = 200;
            },
        ),
    ];
    let id = |row: &TableRow<'_>| row.i64(0).unwrap();
    for (store, change) in cases {
        alter(&store, &id_key(BAD), change);
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert!(read.get_row(&table, &[Value::I64(BAD)]).unwrap().is_some());
        reads_and_scans!(read, table, (0..ROWS).collect(), id, |_| ());
        drop(read);
        let write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        assert!(write.get_row(&table, &[Value::I64(BAD)]).unwrap().is_some());
        reads_and_scans!(write, table, (0..ROWS).collect(), id, |_| ());
    }
}

/// AC-L2: a row under a malformed key, a truncated element of a key of one column or of the last column of a compound
/// key, is returned by scans in the order of the stored keys' bytes, in a read and in a write transaction; the
/// columns before the malformed element read their values. (A malformed key cannot be asked for by its values.)
#[test]
fn ac_l2_a_row_under_a_malformed_key_is_returned_in_key_byte_order() {
    let pairs = Schema::new(
        vec![
            Column::new("a", ColumnType::I64).not_null(),
            Column::new("b", ColumnType::I64).not_null(),
            Column::new("s", ColumnType::String),
        ],
        &["a", "b"],
    );
    let pair_key = |id: i64| {
        encode_key(
            &[ColumnType::I64, ColumnType::I64],
            &[Value::I64(id), Value::I64(id)],
        )
        .unwrap()
    };
    let pair_row = |id: i64| -> Row {
        vec![
            ("a", Value::I64(id)),
            ("b", Value::I64(id)),
            ("s", Value::String(format!("row{id}").into())),
        ]
    };
    let single = (
        store_with(&texts(), (0..ROWS).map(text_row)),
        &id_key as &dyn Fn(i64) -> Vec<u8>,
    );
    let compound = (
        store_with(&pairs, (0..ROWS).map(pair_row)),
        &pair_key as &dyn Fn(i64) -> Vec<u8>,
    );
    for (compound, (store, key)) in [(false, single), (true, compound)] {
        let good = key(BAD);
        let truncated = &good[..good.len() - 1];
        rekey(&store, &good, truncated);
        // The ids in the order of the stored keys' bytes.
        let mut keys: Vec<(Vec<u8>, i64)> = (0..ROWS)
            .map(|id| {
                (
                    if id == BAD {
                        truncated.to_vec()
                    } else {
                        key(id)
                    },
                    id,
                )
            })
            .collect();
        keys.sort();
        let ids: Vec<i64> = keys.into_iter().map(|(_, id)| id).collect();
        // A truncated key of one column is a prefix of another key, before which it goes.
        assert!(
            compound || ids != (0..ROWS).collect::<Vec<_>>(),
            "the truncated key moves"
        );
        // Both schemas keep the text at position 2; it names the row.
        let id = |row: &TableRow<'_>| {
            let text = row.text_bytes(2).unwrap();
            std::str::from_utf8(&text[3..])
                .unwrap()
                .parse::<i64>()
                .unwrap()
        };
        let check = |row: &TableRow<'_>| {
            if compound && id(row) == BAD {
                assert_eq!(
                    row.i64(0),
                    Some(BAD),
                    "the column before the malformed element"
                );
            }
        };
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        reads_and_scans!(read, table, ids.clone(), id, check);
        drop(read);
        let write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        reads_and_scans!(write, table, ids.clone(), id, check);
    }
}

/// AC-L3: a row of a schema version the table never had, one with a header Krummi does not write and one shorter than
/// its fixed part fail a read by key and the scans that reach it with damage of the row's page, in a read and in a
/// write transaction; a scan returns the rows before it from its end, then the failure, then nothing from either end.
#[test]
fn ac_l3_an_unknown_version_header_or_fixed_part_is_damage_of_the_row_page() {
    // `id` in the key, then the header, a null bitmap of one byte and `n`: a fixed part of 8 bytes.
    let changes: [(&str, Change<'_>); 4] = [
        ("a later version", &|b| {
            b[..2].copy_from_slice(&2u16.to_le_bytes())
        }),
        ("version 0", &|b| {
            b[..2].copy_from_slice(&0u16.to_le_bytes())
        }),
        ("an unknown offset width", &|b| b[2] = 3),
        ("short of its fixed part", &|b| b.truncate(6)),
    ];
    for (what, change) in changes {
        let store = store_with(&texts(), (0..ROWS).map(text_row));
        alter(&store, &id_key(BAD), change);
        let page = row_page(&store, &id_key(BAD));
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert_eq!(
            damaged_page(read.get_row(&table, &[Value::I64(BAD)])),
            page,
            "{what}"
        );
        for front in [true, false] {
            scan_fails(read.range_rows(&table, &[], ..).unwrap(), front, page, what);
        }
        lent_scan_fails!(read, table, page, what);
        drop(read);
        let write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        assert_eq!(
            damaged_page(write.get_row(&table, &[Value::I64(BAD)])),
            page,
            "{what}"
        );
        scan_fails(write.range_rows(&table, &[], ..).unwrap(), true, page, what);
        lent_scan_fails!(write, table, page, what);
    }
}

/// AC-L3: a row of a version missing from the table's history fails a read by key and the scans that reach it with
/// damage of the history page that should have it.
#[test]
fn ac_l3_a_version_missing_from_the_history_is_damage_of_the_history_page() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&texts()))
        .unwrap();
    // `BAD` at version 1, the others at version 2.
    write.insert_row(&table, &text_row(BAD)).unwrap();
    write
        .add_column(&table, &Column::new("extra", ColumnType::I32))
        .unwrap();
    for id in (0..ROWS).filter(|&id| id != BAD) {
        write.insert_row(&table, &text_row(id)).unwrap();
    }
    write.commit().unwrap();
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let history = write.table_trees(&table).unwrap().history;
    let update = krummi_tree::remove(&mut write.inner, history, &version_key(1)).unwrap();
    let state = write.tables.get_mut().get_mut(table.id).unwrap();
    state.history = update.root;
    state.dirty = true;
    write.commit().unwrap();

    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let history = read.table_trees(&table).unwrap().history;
    let page = krummi_tree::leaf_of(&read.inner, history, &version_key(1)).unwrap();
    assert!(
        read.get_row(&table, &[Value::I64(BAD - 1)])
            .unwrap()
            .is_some()
    );
    assert_eq!(damaged_page(read.get_row(&table, &[Value::I64(BAD)])), page);
    for front in [true, false] {
        scan_fails(
            read.range_rows(&table, &[], ..).unwrap(),
            front,
            page,
            "missing version",
        );
    }
    lent_scan_fails!(read, table, page, "missing version");
}

/// A lent scan reads a row of an older version that comes after rows of the current version with the older version's
/// layout, as the iterator does (R3-lent-fast-path-version-transition-untested of the RDD review of the row reads run).
#[test]
fn a_lent_scan_reads_an_older_row_after_current_ones() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("t", TableMode::CreateNew(&texts()))
        .unwrap();
    // `BAD` at version 1, the others at version 2, with a value in the added column.
    write.insert_row(&table, &text_row(BAD)).unwrap();
    write
        .add_column(&table, &Column::new("extra", ColumnType::I32))
        .unwrap();
    for id in (0..ROWS).filter(|&id| id != BAD) {
        let mut row = text_row(id);
        row.push(("extra", Value::I32(-1)));
        write.insert_row(&table, &row).unwrap();
    }
    write.commit().unwrap();
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let iterated: Vec<Vec<Value<'_>>> = read
        .range_rows(&table, &[], ..)
        .unwrap()
        .map(|row| row.unwrap().values().collect())
        .collect();
    let extra = read.describe_table("t").unwrap().columns().len() - 1;
    assert_eq!(
        iterated[usize::try_from(BAD).unwrap()][extra],
        Value::Null,
        "the added column"
    );
    let mut scan = read.range_rows(&table, &[], ..).unwrap();
    let mut lent = Vec::new();
    while let Some(row) = scan.next_row().unwrap() {
        lent.push(row.values().collect::<Vec<_>>());
    }
    assert_eq!(lent, iterated);
}

/// A scan of every row of [`texts`] from one end that must fail at `BAD` with damage of `page`: the rows before it
/// from that end, the failure, then nothing from either end.
#[track_caller]
fn scan_fails<'t>(
    mut scan: impl DoubleEndedIterator<Item = Result<TableRow<'t>>>,
    front: bool,
    page: u64,
    what: &str,
) {
    let before: Vec<i64> = if front {
        (0..BAD).collect()
    } else {
        (BAD + 1..ROWS).rev().collect()
    };
    for id in before {
        let row = if front { scan.next() } else { scan.next_back() };
        assert_eq!(
            row.unwrap().unwrap().i64(0),
            Some(id),
            "{what}, front {front}"
        );
    }
    let failure = if front { scan.next() } else { scan.next_back() };
    assert_eq!(
        damaged_page(failure.unwrap()),
        page,
        "{what}, front {front}"
    );
    assert!(
        scan.next().is_none() && scan.next_back().is_none(),
        "{what}: the scan ends"
    );
}

/// AC-L4: updating, and inserting or updating, a malformed existing row, of the current version or an older one, fail
/// with damage of its page, also after reading it in the same transaction, without changing it; the transaction then
/// fails its commit (T11).
#[test]
fn ac_l4_update_and_upsert_still_check_the_row_they_replace() {
    let older = || {
        let store = Store::open_temporary(&StoreOptions::default()).unwrap();
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&texts()))
            .unwrap();
        for id in 0..ROWS {
            write.insert_row(&table, &text_row(id)).unwrap();
        }
        write
            .add_column(&table, &Column::new("extra", ColumnType::I32))
            .unwrap();
        write.commit().unwrap();
        alter(&store, &id_key(BAD), |bytes| {
            *bytes.last_mut().unwrap() = 0xFF
        });
        store
    };
    for (version, store) in [("current", bad_text_store()), ("older", older())] {
        let page = row_page(&store, &id_key(BAD));
        let malformed = stored(&store, &id_key(BAD));
        let change: Row = vec![("id", Value::I64(BAD)), ("n", Value::I32(1))];
        for (op, read_first) in [
            ("update", false),
            ("upsert", false),
            ("update", true),
            ("upsert", true),
        ] {
            let mut write = store.write().unwrap();
            let table = write.open_table("t").unwrap();
            if read_first {
                assert!(write.get_row(&table, &[Value::I64(BAD)]).unwrap().is_some());
            }
            let outcome = match op {
                "update" => write.update_row(&table, &change).map(drop),
                _ => write.upsert_row(&table, &change).map(drop),
            };
            assert_eq!(
                damaged_page(outcome),
                page,
                "{version} {op}, read first {read_first}"
            );
            assert_eq!(
                write.commit().unwrap_err().kind(),
                ErrorKind::Damaged,
                "T11"
            );
            assert_eq!(
                stored(&store, &id_key(BAD)),
                malformed,
                "{version} {op}: unchanged"
            );
        }
    }
}

/// AC-L5: a panic of reading a text that is not UTF-8, captured around that read, leaves the transaction intact: a
/// read transaction goes on reading, and a write transaction goes on writing and commits what it did.
#[test]
fn ac_l5_a_panic_captured_around_the_read_leaves_the_transaction_intact() {
    let store = bad_text_store();
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let row = read.get_row(&table, &[Value::I64(BAD)]).unwrap().unwrap();
    assert!(panics(|| row.text(2)));
    assert_eq!(
        read.get_row(&table, &[Value::I64(3)])
            .unwrap()
            .unwrap()
            .text(2)
            .as_deref(),
        Some("row3")
    );
    assert_eq!(read.range_rows(&table, &[], ..).unwrap().count(), 10);
    drop(read);

    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let row = write.get_row(&table, &[Value::I64(BAD)]).unwrap().unwrap();
    assert!(panics(|| row.text(2)));
    write.insert_row(&table, &text_row(100)).unwrap();
    write.commit().unwrap();

    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    assert!(read.get_row(&table, &[Value::I64(100)]).unwrap().is_some());
    drop(read);
    store.close().unwrap();
}

/// AC-L5: a panic of reading a text that is not UTF-8 that goes through its transaction ends a read transaction and
/// discards a write one without applying anything, freeing its turn (T10); other reads, the next write and closing the
/// store work.
#[test]
fn ac_l5_a_panic_through_the_transaction_discards_it() {
    let store = bad_text_store();
    assert!(panics(|| {
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let row = read.get_row(&table, &[Value::I64(BAD)]).unwrap().unwrap();
        row.text(2).map(|text| text.len())
    }));
    assert!(panics(|| {
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write.insert_row(&table, &text_row(100)).unwrap();
        let row = write.get_row(&table, &[Value::I64(BAD)]).unwrap().unwrap();
        row.text(2).map(|text| text.len())
    }));
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    assert!(
        read.get_row(&table, &[Value::I64(100)]).unwrap().is_none(),
        "nothing applied"
    );
    assert_eq!(read.range_rows(&table, &[], ..).unwrap().count(), 10);
    drop(read);
    let mut write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    write.insert_row(&table, &text_row(101)).unwrap();
    write.commit().unwrap();
    store.close().unwrap();
}
