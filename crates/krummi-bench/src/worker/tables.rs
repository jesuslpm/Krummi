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

use std::borrow::Cow;
use std::convert::Infallible;

use krummi::ids::{Ulid, Uuid};
use krummi::table::projection::{TextBytes, Timestamp};
use krummi::table::value::{ColumnType, UuidOrder, Value};
use krummi::{
    Column, IndexDefinition, IndexKey, IndexMode, ProjectedColumn, Projection, ReadTable,
    ReadTransaction, Schema, TableMode, TableRow, WriteTable, WriteTransaction,
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
            cx.modifying::<Krummi>(Tree::Normal, |store| {
                let ((), time) = timed(|| {
                    batches(store.inner(), keys.len(), p.batch, None, |tx, table, j| {
                        // Only `n` (E10): the other columns keep their values.
                        let row = [("id", ulid(&keys[j])), ("n", Value::I64(new_n[j]))];
                        match tx.update_row(table, &row).map_err(cause)? {
                            true => Ok(()),
                            false => Err(missing(&keys[j])),
                        }
                    })
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

/// Every column of `row`, with its key of `kind`; `ts` goes from microseconds to the nanoseconds of [`Value`].
fn values(row: &Row, kind: KeyKind) -> [(&'static str, Value<'_>); COLUMNS] {
    let id = match kind {
        KeyKind::Ulid => Value::Ulid(Ulid(row.id)),
        KeyKind::Uuid => Value::Uuid(Uuid(row.id)),
    };
    [
        ("id", id),
        ("n", Value::I64(row.n)),
        ("x", Value::F64(row.x)),
        ("ts", Value::Timestamp(i128::from(row.ts) * 1_000)),
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
        let nanos = i128::from(start) * 1_000;
        let mut range = tx
            .index_rows(&table, IX_TS, &[], Value::Timestamp(nanos)..)
            .map_err(cause)?;
        let mut read = 0u64;
        while read < range_len {
            let Some(row) = range.next_row().map_err(cause)? else {
                break;
            };
            // An interval starts at an existing value: its first row has that `ts`.
            if read == 0 && row.timestamp(3) != Some(nanos) {
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
    // A 64-bit division when the nanoseconds fit, as they do in the loads' data; the 128-bit one costs several times more.
    let ts = match i64::try_from(ts) {
        Ok(ts) => ts / 1_000,
        Err(_) => i64::try_from(ts / 1_000).map_err(|_| unexpected())?,
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
        start = end;
    }
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
