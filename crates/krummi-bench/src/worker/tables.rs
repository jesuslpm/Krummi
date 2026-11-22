//! The table loads W23–W32 on Krummi (E21 and *Cargas del banco* of tables-schema.prd.md; IX15 and *Cargas del
//! banco* of tables-indexes.prd.md). Only Krummi and Voron run them, so they use Krummi's table API directly instead of [`crate::engines::Store`]; the Voron side is
//! `voron/Tables.cs`. The data, the orders, the expected checksums and the digests come from [`crate::table_data`] and
//! are generated before any timer starts (K4); building a row's values from them is part of the operation, as Voron's
//! builder is.
//!
//! The table `table` has the primary key `id` (`ulid`, or `uuid` in the order of RFC 9562 for W24) and the columns
//! `n` (`i64`), `x` (`f64`), `ts` (`timestamp`), `flag` (`bool`) and `name` (`string`), none nullable. Every commit
//! is durable, as in the other Krummi loads (K3). W30's table also has two indexes that are not unique, ascending and
//! without included columns: `ix_n` on `n` and `ix_ts` on `ts` ([`indexes`]); W31 looks rows up in `ix_n` and W32
//! scans intervals of `ix_ts`.
//!
//! W26, W27, W31 and W32 read the columns of their rows with a projection prepared at the start of each repetition,
//! inside its measured time (PY7 of tables-projection.prd.md). `KRUMMI_BENCH_PROJECTION=off` reads them by position
//! instead, a diagnostic hook to measure the projection (AC-PY11), not part of the benchmark's interface.
//!
//! `KRUMMI_BENCH_ESTIMATES` adds statistics estimates to W23 and W28 for the measurements of ST15 of
//! tables-statistics.prd.md, a measuring hook outside the benchmark's interface that writes only to stderr:
//! `thread` runs a second thread that, after each write of the load, asks for an estimate of each column; `first`
//! times the first estimate on W23's table after its load, outside its measured time. Any other value fails W23 and
//! W28 before they run.

use std::borrow::Cow;
use std::convert::Infallible;
use std::sync::mpsc;

use krummi::ids::{Ulid, Uuid};
use krummi::table::projection::{TextBytes, Timestamp};
use krummi::table::value::{ColumnType, UuidOrder, Value};
use krummi::{
    Column, Condition, IndexDefinition, IndexKey, IndexMode, ProjectedColumn, Projection,
    ReadTable, ReadTransaction, Schema, TableMode, TableRow, WriteTable, WriteTransaction,
};

use super::{Context, check_count, check_sum, hex, missing, timed};
use crate::engines::{self, Result, Tree, cause};
use crate::table_data::{self, ID_LEN, KeyKind, Row};
use crate::workload::{Load, params};

type Krummi = engines::krummi::Store;

/// The name of the table of every table load.
const TABLE: &str = "table";
/// The indexes of W30's table, on `n` and on `ts`.
const IX_N: &str = "ix_n";
const IX_TS: &str = "ix_ts";

/// The variable whose value `off` makes W26, W27, W31 and W32 read their columns by position instead of with a
/// projection.
pub const PROJECTION_VARIABLE: &str = "KRUMMI_BENCH_PROJECTION";

/// Whether the loads that read rows do it with a projection: unless [`PROJECTION_VARIABLE`] is `off`.
fn with_projection() -> bool {
    std::env::var_os(PROJECTION_VARIABLE).is_none_or(|value| value != "off")
}

/// The variable whose value adds statistics estimates to W23 and W28: `thread` or `first` (ST15).
pub const ESTIMATES_VARIABLE: &str = "KRUMMI_BENCH_ESTIMATES";

/// The estimates [`ESTIMATES_VARIABLE`] asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Estimates {
    /// No estimates: the variable is not set.
    None,
    /// The estimating thread beside the load's writes.
    Thread,
    /// The first estimate after W23's load.
    First,
}

/// The estimates [`ESTIMATES_VARIABLE`] asks for; a value other than `thread` or `first` fails.
fn estimates() -> Result<Estimates> {
    estimates_of(std::env::var_os(ESTIMATES_VARIABLE).as_deref())
}

/// The estimates the value `value` of [`ESTIMATES_VARIABLE`] asks for.
fn estimates_of(value: Option<&std::ffi::OsStr>) -> Result<Estimates> {
    match value.map(|value| value.to_str()) {
        None => Ok(Estimates::None),
        Some(Some("thread")) => Ok(Estimates::Thread),
        Some(Some("first")) => Ok(Estimates::First),
        Some(value) => Err(format!(
            "{ESTIMATES_VARIABLE} vale {}: se espera thread o first",
            value.map_or_else(|| "algo que no es UTF-8".to_owned(), |v| format!("«{v}»"))
        )),
    }
}

/// Runs the table load of `cx` (W23–W32).
pub(super) fn run(cx: &Context) -> Result<()> {
    let load = cx.load;
    let p = params(load, cx.scale);
    let n = p.entries;
    cx.data(
        load,
        table_data::digest(load.number(), n, p.ranges, p.range_len),
    )?;
    match load {
        Load::W23 | Load::W24 | Load::W25 | Load::W30 => {
            let kind = if load == Load::W24 {
                KeyKind::Uuid
            } else {
                KeyKind::Ulid
            };
            let rows = table_data::rows(n, kind);
            let schema = schema(kind);
            let estimates = if load == Load::W23 {
                estimates()?
            } else {
                Estimates::None
            };
            cx.modifying::<Krummi>(Tree::Normal, |store| {
                let store = store.inner();
                let insert = |tx: &mut WriteTransaction<'_>, table: &WriteTable<'_>, j: usize| {
                    tx.insert_row(table, &values(&rows[j], kind)).map_err(cause)
                };
                let time = match load {
                    Load::W25 => {
                        // The empty table exists before the measured bulk load, whose time includes its commit (E13).
                        create_table(store, &schema, &[])?;
                        let source = rows.iter().map(|r| Ok::<_, Infallible>(values(r, kind)));
                        let (loaded, time) =
                            timed(|| store.bulk_load(TABLE, source).map_err(cause))?;
                        check_count(n, loaded)?;
                        time
                    }
                    Load::W30 => {
                        // The empty table and its two indexes exist before the measured inserts (IX15).
                        create_table(store, &schema, &indexes())?;
                        timed(|| batches(store, rows.len(), p.batch, None, insert))?.1
                    }
                    Load::W23 => {
                        let row = values(&rows[0], kind);
                        let thread = estimates == Estimates::Thread;
                        let time = with_estimates(store, thread, &row, |notify| {
                            timed(|| {
                                let schema = Some(&schema);
                                batches_notifying(
                                    store,
                                    rows.len(),
                                    p.batch,
                                    schema,
                                    notify,
                                    insert,
                                )
                            })
                        })?
                        .1;
                        if estimates == Estimates::First {
                            first_estimate(store, &row)?;
                        }
                        time
                    }
                    _ => timed(|| batches(store, rows.len(), p.batch, Some(&schema), insert))?.1,
                };
                Ok(vec![(load, n, time)])
            })
        }
        Load::W26 => {
            let keys = ulid_keys(table_data::order(load.number(), n));
            let expected = table_data::expected_read_sum(n);
            let projected = with_projection();
            cx.read_only::<Krummi>(Tree::Normal, |store| {
                let (sum, time) = timed(|| read_by_key(store.inner(), &keys, projected))?;
                check_sum(expected, sum)?;
                Ok(vec![(load, n, time)])
            })
        }
        Load::W27 => {
            let starts = ulid_keys(table_data::range_starts(n, p.ranges));
            let (rows, expected) = table_data::expected_ranges(n, p.ranges, p.range_len);
            let projected = with_projection();
            cx.read_only::<Krummi>(Tree::Normal, |store| {
                let ((count, sum), time) =
                    timed(|| read_ranges(store.inner(), &starts, p.range_len, projected))?;
                check_count(rows, count)?;
                check_sum(expected, sum)?;
                Ok(vec![(load, count, time)])
            })
        }
        Load::W28 => {
            let order = table_data::order(load.number(), n);
            let new_n: Vec<i64> = order
                .iter()
                .map(|&r| table_data::new_n(u64::from(r)))
                .collect();
            let keys = ulid_keys(order);
            let first = table_data::rows(1, KeyKind::Ulid);
            let condition_row = values(&first[0], KeyKind::Ulid);
            let thread = estimates()? == Estimates::Thread;
            cx.modifying::<Krummi>(Tree::Normal, |store| {
                let ((), time) = with_estimates(store.inner(), thread, &condition_row, |notify| {
                    timed(|| update_n(store.inner(), &keys, &new_n, p.batch, notify))
                })?;
                // After the measured time: every row holds its new `n`.
                let tx = store.inner().read().map_err(cause)?;
                let table = tx.open_table(TABLE).map_err(cause)?;
                for (key, &n) in keys.iter().zip(&new_n) {
                    let row = tx.get_row(&table, &[ulid(key)]).map_err(cause)?;
                    let row = row.ok_or_else(|| missing(key))?;
                    if row.get("n") != Some(Value::I64(n)) {
                        return Err(format!("la fila {} no tiene su n nuevo", hex(key)));
                    }
                }
                Ok(vec![(load, n, time)])
            })
        }
        Load::W31 => {
            let lookups: Vec<i64> = table_data::order(load.number(), n)
                .into_iter()
                .map(|r| table_data::n(u64::from(r)))
                .collect();
            let expected = table_data::expected_read_sum(n);
            let projected = with_projection();
            cx.read_only::<Krummi>(Tree::Normal, |store| {
                let (sum, time) = timed(|| look_up(store.inner(), &lookups, projected))?;
                check_sum(expected, sum)?;
                Ok(vec![(load, n, time)])
            })
        }
        Load::W32 => {
            let starts: Vec<i64> = table_data::index_range_starts(n, p.ranges)
                .into_iter()
                .map(|r| table_data::ts(u64::from(r)))
                .collect();
            let (rows, expected) = table_data::expected_index_ranges(n, p.ranges, p.range_len);
            let projected = with_projection();
            cx.read_only::<Krummi>(Tree::Normal, |store| {
                let ((count, sum), time) =
                    timed(|| read_index_ranges(store.inner(), &starts, p.range_len, projected))?;
                check_count(rows, count)?;
                check_sum(expected, sum)?;
                Ok(vec![(load, count, time)])
            })
        }
        Load::W29 => {
            let keys = ulid_keys(table_data::order(load.number(), n));
            cx.modifying::<Krummi>(Tree::Normal, |store| {
                let ((), time) = timed(|| {
                    batches(
                        store.inner(),
                        keys.len(),
                        p.batch,
                        None,
                        |tx, table, j| match tx
                            .delete_row(table, &[ulid(&keys[j])])
                            .map_err(cause)?
                        {
                            true => Ok(()),
                            false => Err(missing(&keys[j])),
                        },
                    )
                })?;
                // After the measured time: the table is empty.
                let tx = store.inner().read().map_err(cause)?;
                let table = tx.open_table(TABLE).map_err(cause)?;
                match tx.row_count(&table).map_err(cause)? {
                    0 => Ok(vec![(load, n, time)]),
                    left => Err(format!("la tabla conserva {left} filas")),
                }
            })
        }
        _ => unreachable!("{load:?} is not a table load"),
    }
}

/// The number of columns of [`schema`].
const COLUMNS: usize = 6;

/// The schema of the table of every table load, with its key of `kind`.
fn schema(kind: KeyKind) -> Schema {
    let id = match kind {
        KeyKind::Ulid => ColumnType::Ulid,
        KeyKind::Uuid => ColumnType::Uuid(UuidOrder::Rfc9562),
    };
    let column = |name: &str, ty| Column::new(name, ty).not_null();
    Schema::new(
        vec![
            column("id", id),
            column("n", ColumnType::I64),
            column("x", ColumnType::F64),
            column("ts", ColumnType::Timestamp),
            column("flag", ColumnType::Bool),
            column("name", ColumnType::String),
        ],
        &["id"],
    )
}

/// The indexes of W30's table: `ix_n` on `n` and `ix_ts` on `ts`, not unique, ascending and without included
/// columns. An entry holds its column's value followed by the row's key, so rows with the same `ts` follow their key.
fn indexes() -> [IndexDefinition; 2] {
    [
        IndexDefinition::new(IX_N, vec![IndexKey::new("n")]),
        IndexDefinition::new(IX_TS, vec![IndexKey::new("ts")]),
    ]
}

/// Every column of `row`, with its key of `kind`; `ts` in microseconds, the unit of [`Value::Timestamp`].
fn values(row: &Row, kind: KeyKind) -> [(&'static str, Value<'_>); COLUMNS] {
    let id = match kind {
        KeyKind::Ulid => Value::Ulid(Ulid(row.id)),
        KeyKind::Uuid => Value::Uuid(Uuid(row.id)),
    };
    [
        ("id", id),
        ("n", Value::I64(row.n)),
        ("x", Value::F64(row.x)),
        ("ts", Value::Timestamp(row.ts)),
        ("flag", Value::Bool(row.flag)),
        ("name", Value::String(Cow::Borrowed(row.name_str()))),
    ]
}

fn ulid(key: &[u8; ID_LEN]) -> Value<'static> {
    Value::Ulid(Ulid(*key))
}

/// The `ulid` keys of W23's rows, in the order of `rows`.
fn ulid_keys(rows: Vec<u32>) -> Vec<[u8; ID_LEN]> {
    rows.into_iter()
        .map(|r| table_data::key(u64::from(r), KeyKind::Ulid))
        .collect()
}

/// The key of a row read from W23's table, a `ulid` (W26, W27).
fn id_of(row: &TableRow<'_>) -> Option<[u8; ID_LEN]> {
    row.ulid(0).map(|id| id.0)
}

/// The sum of the checksums of the rows of `keys`, each read by its key (W26), with a projection or by position.
fn read_by_key(store: &krummi::Store, keys: &[[u8; ID_LEN]], projected: bool) -> Result<u64> {
    let tx = store.read().map_err(cause)?;
    let table = tx.open_table(TABLE).map_err(cause)?;
    let columns = Columns::prepare(&tx, &table, projected)?;
    let mut sum = 0u64;
    for key in keys {
        let row = tx.get_row(&table, &[ulid(key)]).map_err(cause)?;
        let row = row.ok_or_else(|| missing(key))?;
        sum = sum.wrapping_add(checksum(&row, columns.as_ref())?);
    }
    Ok(sum)
}

/// The number of rows read and the sum of their checksums of a range of up to `range_len` rows from each of `starts`
/// (W27), with a projection or by position.
fn read_ranges(
    store: &krummi::Store,
    starts: &[[u8; ID_LEN]],
    range_len: u64,
    projected: bool,
) -> Result<(u64, u64)> {
    let tx = store.read().map_err(cause)?;
    let table = tx.open_table(TABLE).map_err(cause)?;
    let columns = Columns::prepare(&tx, &table, projected)?;
    let (mut count, mut sum) = (0u64, 0u64);
    for start in starts {
        // The rows are lent one after another, as Voron's scan lends its holder.
        let mut range = tx.range_rows(&table, &[], ulid(start)..).map_err(cause)?;
        let mut read = 0u64;
        while read < range_len {
            let Some(row) = range.next_row().map_err(cause)? else {
                break;
            };
            // A range starts at an existing key: its first row is that key's.
            if read == 0 && id_of(row).as_ref() != Some(start) {
                return Err(missing(start));
            }
            sum = sum.wrapping_add(checksum(row, columns.as_ref())?);
            read += 1;
        }
        if read == 0 {
            return Err(missing(start));
        }
        count += read;
    }
    Ok((count, sum))
}

/// The sum of the checksums of the rows whose `n` is each of `lookups`, each looked up in `ix_n` with the interval of
/// that value (W31), with a projection or by position. Each lookup must give exactly one row.
fn look_up(store: &krummi::Store, lookups: &[i64], projected: bool) -> Result<u64> {
    let tx = store.read().map_err(cause)?;
    let table = tx.open_table(TABLE).map_err(cause)?;
    let columns = Columns::prepare(&tx, &table, projected)?;
    let mut sum = 0u64;
    for &n in lookups {
        let mut rows = tx
            .index_rows(&table, IX_N, &[Value::I64(n)], ..)
            .map_err(cause)?;
        let mut found = 0u64;
        while let Some(row) = rows.next_row().map_err(cause)? {
            sum = sum.wrapping_add(checksum(row, columns.as_ref())?);
            found += 1;
        }
        if found != 1 {
            return Err(format!(
                "{IX_N} da {found} filas con n = {n} y se esperaba 1"
            ));
        }
    }
    Ok(sum)
}

/// The number of rows read and the sum of their checksums of an interval of `ix_ts` from each of `starts` (values of
/// `ts` in microseconds), included, of up to `range_len` rows (W32), with a projection or by position.
fn read_index_ranges(
    store: &krummi::Store,
    starts: &[i64],
    range_len: u64,
    projected: bool,
) -> Result<(u64, u64)> {
    let tx = store.read().map_err(cause)?;
    let table = tx.open_table(TABLE).map_err(cause)?;
    let columns = Columns::prepare(&tx, &table, projected)?;
    let (mut count, mut sum) = (0u64, 0u64);
    for &start in starts {
        let mut range = tx
            .index_rows(&table, IX_TS, &[], Value::Timestamp(start)..)
            .map_err(cause)?;
        let mut read = 0u64;
        while read < range_len {
            let Some(row) = range.next_row().map_err(cause)? else {
                break;
            };
            // An interval starts at an existing value: its first row has that `ts`.
            if read == 0 && row.timestamp(3) != Some(start) {
                return Err(missing_ts(start));
            }
            sum = sum.wrapping_add(checksum(row, columns.as_ref())?);
            read += 1;
        }
        if read == 0 {
            return Err(missing_ts(start));
        }
        count += read;
    }
    Ok((count, sum))
}

/// The message of an interval of W32 that does not start at its value of `ts`, in microseconds.
fn missing_ts(ts: i64) -> String {
    format!("no se encuentra ts = {ts} en {IX_TS}")
}

/// The columns of [`schema`] prepared to be read with a projection (PY7).
struct Columns<'s> {
    projection: Projection<'s>,
    id: ProjectedColumn<Ulid>,
    n: ProjectedColumn<i64>,
    x: ProjectedColumn<f64>,
    ts: ProjectedColumn<Timestamp>,
    flag: ProjectedColumn<bool>,
    name: ProjectedColumn<TextBytes>,
}

impl<'s> Columns<'s> {
    /// The projection of every column of `table`, when the load reads with one.
    fn prepare(
        tx: &ReadTransaction<'s>,
        table: &ReadTable<'_>,
        projected: bool,
    ) -> Result<Option<Self>> {
        if !projected {
            return Ok(None);
        }
        const NAMES: [&str; COLUMNS] = ["id", "n", "x", "ts", "flag", "name"];
        let projection = tx.projection(table, &NAMES).map_err(cause)?;
        // Each field takes the position of its own name in the list.
        let at = |name| {
            NAMES
                .iter()
                .position(|n| *n == name)
                .expect("a listed column")
        };
        Ok(Some(Columns {
            id: projection.column(at("id")).map_err(cause)?,
            n: projection.column(at("n")).map_err(cause)?,
            x: projection.column(at("x")).map_err(cause)?,
            ts: projection.column(at("ts")).map_err(cause)?,
            flag: projection.column(at("flag")).map_err(cause)?,
            name: projection.column(at("name")).map_err(cause)?,
            projection,
        }))
    }
}

/// The checksum of a row read, every column ([`table_data::checksum`]): with `columns`, through the projection;
/// otherwise each with its typed accessor (the table has the columns of [`schema`], or the accessor panics); `ts` back
/// in microseconds.
fn checksum(row: &TableRow<'_>, columns: Option<&Columns<'_>>) -> Result<u64> {
    let unexpected = || format!("la fila leída no tiene las columnas de la tabla: {row:?}");
    let read = match columns {
        Some(c) => {
            let r = c.projection.bind(row).map_err(cause)?;
            (
                r.get(&c.id).map(|id| id.0),
                r.get(&c.n),
                r.get(&c.x),
                r.get(&c.ts),
                r.get(&c.flag),
                r.get(&c.name),
            )
        }
        None if row.len() == COLUMNS => (
            id_of(row),
            row.i64(1),
            row.f64(2),
            row.timestamp(3),
            row.bool(4),
            row.text_bytes(5),
        ),
        None => return Err(unexpected()),
    };
    let (Some(id), Some(n), Some(x), Some(ts), Some(flag), Some(name)) = read else {
        return Err(unexpected());
    };
    Ok(table_data::checksum(&id, n, x, ts, flag, &name))
}

/// Creates the empty table with `schema` and the indexes `indexes` in a write of its own.
fn create_table(store: &krummi::Store, schema: &Schema, indexes: &[IndexDefinition]) -> Result<()> {
    let mut tx = store.write().map_err(cause)?;
    let table = tx
        .open_table_with(TABLE, TableMode::CreateNew(schema))
        .map_err(cause)?;
    for index in indexes {
        tx.create_index(&table, index, IndexMode::CreateNew)
            .map_err(cause)?;
    }
    tx.commit().map_err(cause)
}

/// Writes `n` items in write transactions of `batch`, `op(tx, table, j)` for item `j`, with the table opened in each
/// write: opened or created with `schema` when it is given (W23, W24), otherwise only opened.
fn batches(
    store: &krummi::Store,
    n: usize,
    batch: u64,
    schema: Option<&Schema>,
    op: impl FnMut(&mut WriteTransaction<'_>, &WriteTable<'_>, usize) -> Result<()>,
) -> Result<()> {
    batches_notifying(store, n, batch, schema, None, op)
}

/// [`batches`], sending to `notify`, when it is given, after each commit.
fn batches_notifying(
    store: &krummi::Store,
    n: usize,
    batch: u64,
    schema: Option<&Schema>,
    notify: Option<&mpsc::Sender<()>>,
    mut op: impl FnMut(&mut WriteTransaction<'_>, &WriteTable<'_>, usize) -> Result<()>,
) -> Result<()> {
    let batch = usize::try_from(batch).unwrap_or(usize::MAX).max(1);
    let mut start = 0;
    while start < n {
        let end = n.min(start + batch);
        let mut tx = store.write().map_err(cause)?;
        let table = match schema {
            Some(schema) => tx.open_table_with(TABLE, TableMode::OpenOrCreate(schema)),
            None => tx.open_table(TABLE),
        }
        .map_err(cause)?;
        for j in start..end {
            op(&mut tx, &table, j)?;
        }
        tx.commit().map_err(cause)?;
        if let Some(notify) = notify {
            // The estimating thread is gone only after an error of its own, which it reports.
            let _ = notify.send(());
        }
        start = end;
    }
    Ok(())
}

/// One equality condition for each column of `row`.
fn equal_conditions<'r>(row: &'r [(&'static str, Value<'r>)]) -> Vec<Condition<'r>> {
    row.iter()
        .map(|(column, value)| Condition::Equal {
            column,
            value: value.clone(),
        })
        .collect()
}

/// W28's measured write: sets `n` of the rows of `keys` to `new_n`, in writes of `batch` rows, waking `notify` after
/// each commit. Only `n` (E10): the other columns keep their values.
fn update_n(
    store: &krummi::Store,
    keys: &[[u8; ID_LEN]],
    new_n: &[i64],
    batch: u64,
    notify: Option<&mpsc::Sender<()>>,
) -> Result<()> {
    let update = |tx: &mut WriteTransaction<'_>, table: &WriteTable<'_>, j: usize| {
        let row = [("id", ulid(&keys[j])), ("n", Value::I64(new_n[j]))];
        match tx.update_row(table, &row).map_err(cause)? {
            true => Ok(()),
            false => Err(missing(&keys[j])),
        }
    };
    batches_notifying(store, keys.len(), batch, None, notify, update)
}

/// Runs `write(notify)`, with `thread` (from [`ESTIMATES_VARIABLE`]) beside the estimating thread of [`estimating`];
/// then writes its rounds and the samples the statistics work took to stderr.
fn with_estimates<T>(
    store: &krummi::Store,
    thread: bool,
    row: &[(&'static str, Value<'_>)],
    write: impl FnOnce(Option<&mpsc::Sender<()>>) -> Result<T>,
) -> Result<T> {
    if !thread {
        return write(None);
    }
    let samples = store.statistics_samples();
    let (written, rounds) = estimating(store, row, write)?;
    eprintln!(
        "estimates rounds {rounds} samples {}",
        store.statistics_samples() - samples
    );
    Ok(written)
}

/// Runs `write(notify)` beside a second thread that, each time `notify` wakes it after a commit, asks in a new read for
/// an estimate of each column with the values of `row` (ST15): the first samples and the stale ones ask for a renewal
/// in the background. Each wake gets its own round, in order: those that arrive while it estimates wait their turn.
/// After `write`, it waits for the thread and the statistics work, and returns what `write` returned and the thread's
/// rounds; when both fail, the error names both.
fn estimating<T>(
    store: &krummi::Store,
    row: &[(&'static str, Value<'_>)],
    write: impl FnOnce(Option<&mpsc::Sender<()>>) -> Result<T>,
) -> Result<(T, u64)> {
    let conditions = equal_conditions(row);
    let (sender, receiver) = mpsc::channel::<()>();
    std::thread::scope(|scope| {
        let estimator = scope.spawn(move || -> Result<u64> {
            let mut rounds = 0;
            while receiver.recv().is_ok() {
                let tx = store.read().map_err(cause)?;
                let table = tx.open_table(TABLE).map_err(cause)?;
                for condition in &conditions {
                    tx.estimate(&table, condition).map_err(cause)?;
                }
                rounds += 1;
            }
            Ok(rounds)
        });
        let written = write(Some(&sender));
        drop(sender);
        let rounds = estimator
            .join()
            .map_err(|_| "el hilo de estimaciones falló".to_owned())?;
        store.wait_statistics_idle();
        match (written, rounds) {
            (Ok(written), Ok(rounds)) => Ok((written, rounds)),
            (Err(error), Ok(_)) | (Ok(_), Err(error)) => Err(error),
            (Err(write), Err(thread)) => Err(format!(
                "{write}; el hilo de estimaciones también falló: {thread}"
            )),
        }
    })
}

/// Times the first estimate on the table just loaded, of `n` with its value in `row`, which samples it (ST3), and
/// writes the time in nanoseconds to stderr; then waits for the statistics work it asked for.
fn first_estimate(store: &krummi::Store, row: &[(&'static str, Value<'_>)]) -> Result<()> {
    let samples = store.statistics_samples();
    let tx = store.read().map_err(cause)?;
    let table = tx.open_table(TABLE).map_err(cause)?;
    let (column, value) = row
        .iter()
        .find(|(column, _)| *column == "n")
        .ok_or_else(|| "la fila no tiene la columna n".to_owned())?;
    let condition = Condition::Equal {
        column,
        value: value.clone(),
    };
    let (_, time) = timed(|| tx.estimate(&table, &condition).map_err(cause))?;
    drop(tx);
    store.wait_statistics_idle();
    eprintln!(
        "first-estimate nanos {} samples {}",
        time.as_nanos(),
        store.statistics_samples() - samples
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store with the table of W23's first `n` rows.
    fn loaded(n: u64) -> krummi::Store {
        let store = krummi::Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
        let rows = table_data::rows(n, KeyKind::Ulid);
        batches(
            &store,
            rows.len(),
            1_000,
            Some(&schema(KeyKind::Ulid)),
            |tx, table, j| {
                tx.insert_row(table, &values(&rows[j], KeyKind::Ulid))
                    .map_err(cause)
            },
        )
        .unwrap();
        store
    }

    /// PY7 and AC-PY11 of the projection PRD: W26 and W27 read with a projection, which the store keeps once
    /// prepared (the test hook), and give the checksums both engines check; by position (the diagnostic hook) they
    /// prepare none and give the same checksums.
    #[test]
    fn w26_and_w27_read_with_a_projection() {
        let n = 2_000;
        let keys = ulid_keys(table_data::order(Load::W26.number(), n));
        let starts = ulid_keys(table_data::range_starts(n, 20));
        let (rows, ranges) = table_data::expected_ranges(n, 20, 50);
        for projected in [false, true] {
            let store = loaded(n);
            assert_eq!(
                read_by_key(&store, &keys, projected).unwrap(),
                table_data::expected_read_sum(n)
            );
            assert_eq!(
                read_ranges(&store, &starts, 50, projected).unwrap(),
                (rows, ranges)
            );
            assert_eq!(store.kept_projections(), usize::from(projected));
        }
    }

    /// ST15 of the statistics PRD: the estimating thread estimates after each commit of a W23 load, sampling the
    /// table, and the first estimate after a load samples it; without the thread nothing samples.
    #[test]
    fn the_estimating_thread_and_the_first_estimate_sample() {
        let n = 5_000;
        let rows = table_data::rows(n, KeyKind::Ulid);
        let row = values(&rows[0], KeyKind::Ulid);
        let insert = |tx: &mut WriteTransaction<'_>, table: &WriteTable<'_>, j: usize| {
            tx.insert_row(table, &values(&rows[j], KeyKind::Ulid))
                .map_err(cause)
        };
        let schema = schema(KeyKind::Ulid);
        let store = krummi::Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
        with_estimates(&store, false, &row, |notify| {
            batches_notifying(&store, rows.len(), 1_000, Some(&schema), notify, insert)
        })
        .unwrap();
        assert_eq!(store.statistics_samples(), 0);
        let store = krummi::Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
        let ((), rounds) = estimating(&store, &row, |notify| {
            batches_notifying(&store, rows.len(), 1_000, Some(&schema), notify, insert)?;
            // Wakes that arrive together, as when commits outrun the estimates, each get their own round.
            for _ in 0..5 {
                notify.expect("the estimating thread").send(()).unwrap();
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(
            rounds, 10,
            "one round of estimates after each of the 5 commits and each of the 5 wakes"
        );
        assert!(store.statistics_samples() > 0);
        let store = loaded(n);
        first_estimate(&store, &row).unwrap();
        assert_eq!(store.statistics_samples(), 1);
        assert_eq!(
            first_estimate(&store, &row[..1]).unwrap_err(),
            "la fila no tiene la columna n"
        );
    }

    /// ST15: W28's updates run beside the estimating thread, which estimates after each of their commits and samples
    /// the table; every row ends with its new `n`.
    #[test]
    fn w28_updates_beside_the_estimating_thread() {
        let n = 5_000;
        let store = loaded(n);
        let order = table_data::order(Load::W28.number(), n);
        let new_n: Vec<i64> = (order.iter())
            .map(|&r| table_data::new_n(u64::from(r)))
            .collect();
        let keys = ulid_keys(order);
        let first = table_data::rows(1, KeyKind::Ulid);
        let row = values(&first[0], KeyKind::Ulid);
        let samples = store.statistics_samples();
        let ((), rounds) = estimating(&store, &row, |notify| {
            update_n(&store, &keys, &new_n, 1_000, notify)
        })
        .unwrap();
        assert_eq!(rounds, 5, "one round after each of the 5 commits");
        assert!(store.statistics_samples() > samples);
        let tx = store.read().unwrap();
        let table = tx.open_table(TABLE).unwrap();
        for (key, &n) in keys.iter().zip(&new_n) {
            let row = tx.get_row(&table, &[ulid(key)]).unwrap().unwrap();
            assert_eq!(row.get("n"), Some(Value::I64(n)));
        }
    }

    /// When the write and the estimating thread both fail, the error names both.
    #[test]
    fn a_failed_write_and_thread_report_both_errors() {
        // No table: the thread fails to open it at its first wake.
        let store = krummi::Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
        let rows = table_data::rows(1, KeyKind::Ulid);
        let row = values(&rows[0], KeyKind::Ulid);
        let error = estimating(&store, &row, |notify| -> Result<()> {
            notify.expect("the estimating thread").send(()).unwrap();
            Err("la escritura falló".to_owned())
        })
        .unwrap_err();
        let prefix = "la escritura falló; el hilo de estimaciones también falló: ";
        assert!(error.starts_with(prefix), "{error}");
        assert!(error.len() > prefix.len(), "{error}");
    }

    /// The values of `KRUMMI_BENCH_ESTIMATES`: unset, `thread` or `first`; any other fails.
    #[test]
    fn the_estimates_variable_takes_only_its_modes() {
        use std::ffi::OsStr;
        assert_eq!(estimates_of(None), Ok(Estimates::None));
        assert_eq!(
            estimates_of(Some(OsStr::new("thread"))),
            Ok(Estimates::Thread)
        );
        assert_eq!(
            estimates_of(Some(OsStr::new("first"))),
            Ok(Estimates::First)
        );
        assert_eq!(
            estimates_of(Some(OsStr::new("Thread"))),
            Err("KRUMMI_BENCH_ESTIMATES vale «Thread»: se espera thread o first".to_owned())
        );
        assert_eq!(
            estimates_of(Some(OsStr::new(""))),
            Err("KRUMMI_BENCH_ESTIMATES vale «»: se espera thread o first".to_owned())
        );
    }

    /// A store with W30's table, with its two indexes, and W23's first `n` rows.
    fn indexed(n: u64) -> krummi::Store {
        let store = krummi::Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
        create_table(&store, &schema(KeyKind::Ulid), &indexes()).unwrap();
        let rows = table_data::rows(n, KeyKind::Ulid);
        batches(&store, rows.len(), 1_000, None, |tx, table, j| {
            tx.insert_row(table, &values(&rows[j], KeyKind::Ulid))
                .map_err(cause)
        })
        .unwrap();
        store
    }

    /// IX15 and PY7: W31 looks every row of W30's table up in `ix_n` and W32 scans intervals of `ix_ts`, with a
    /// projection or by position, and both give the checksums both engines check; a value that no row has fails.
    #[test]
    fn w31_and_w32_read_through_the_indexes_of_w30() {
        let n = 2_000;
        let lookups: Vec<i64> = table_data::order(Load::W31.number(), n)
            .into_iter()
            .map(|r| table_data::n(u64::from(r)))
            .collect();
        let starts: Vec<i64> = table_data::index_range_starts(n, 20)
            .into_iter()
            .map(|r| table_data::ts(u64::from(r)))
            .collect();
        for projected in [false, true] {
            let store = indexed(n);
            assert_eq!(
                look_up(&store, &lookups, projected).unwrap(),
                table_data::expected_read_sum(n)
            );
            assert_eq!(
                read_index_ranges(&store, &starts, 50, projected).unwrap(),
                table_data::expected_index_ranges(n, 20, 50)
            );
            assert_eq!(store.kept_projections(), usize::from(projected));
        }
        let store = indexed(n);
        assert_eq!(
            look_up(&store, &[i64::MIN], true).unwrap_err(),
            "ix_n da 0 filas con n = -9223372036854775808 y se esperaba 1"
        );
        let before = table_data::TS_MIN - 1;
        assert_eq!(
            read_index_ranges(&store, &[before], 50, true).unwrap_err(),
            format!("no se encuentra ts = {before} en ix_ts")
        );
    }
}
