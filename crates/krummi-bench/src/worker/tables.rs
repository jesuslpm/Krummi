//! The table loads W23–W29 on Krummi (E21 and *Cargas del banco* of tables-schema.prd.md). Only Krummi and Voron run
//! them, so they use Krummi's table API directly instead of [`crate::engines::Store`]; the Voron side is
//! `voron/Tables.cs`. The data, the orders, the expected checksums and the digests come from [`crate::table_data`] and
//! are generated before any timer starts (K4); building a row's values from them is part of the operation, as Voron's
//! builder is.
//!
//! The table `table` has the primary key `id` (`ulid`, or `uuid` in the order of RFC 9562 for W24) and the columns
//! `n` (`i64`), `x` (`f64`), `ts` (`timestamp`), `flag` (`bool`) and `name` (`string`), none nullable. Every commit
//! is durable, as in the other Krummi loads (K3).

use std::borrow::Cow;
use std::convert::Infallible;

use krummi::ids::{Ulid, Uuid};
use krummi::table::value::{ColumnType, UuidOrder, Value};
use krummi::{Column, Schema, TableMode, TableRow, WriteTable, WriteTransaction};

use super::{Context, check_count, check_sum, hex, missing, timed};
use crate::engines::{self, Result, Tree, cause};
use crate::table_data::{self, ID_LEN, KeyKind, Row};
use crate::workload::{Load, params};

type Krummi = engines::krummi::Store;

/// The name of the table of every table load.
const TABLE: &str = "table";

/// Runs the table load of `cx` (W23–W29).
pub(super) fn run(cx: &Context) -> Result<()> {
    let load = cx.load;
    let p = params(load, cx.scale);
    let n = p.entries;
    cx.data(
        load,
        table_data::digest(load.number(), n, p.ranges, p.range_len),
    )?;
    match load {
        Load::W23 | Load::W24 | Load::W25 => {
            let kind = if load == Load::W24 {
                KeyKind::Uuid
            } else {
                KeyKind::Ulid
            };
            let rows = table_data::rows(n, kind);
            let schema = schema(kind);
            cx.modifying::<Krummi>(Tree::Normal, |store| {
                let store = store.inner();
                let time = if load == Load::W25 {
                    // The empty table exists before the measured bulk load, whose time includes its commit (E13).
                    create_table(store, &schema)?;
                    let source = rows.iter().map(|r| Ok::<_, Infallible>(values(r, kind)));
                    let (loaded, time) = timed(|| store.bulk_load(TABLE, source).map_err(cause))?;
                    check_count(n, loaded)?;
                    time
                } else {
                    let ((), time) = timed(|| {
                        batches(store, rows.len(), p.batch, Some(&schema), |tx, table, j| {
                            tx.insert_row(table, &values(&rows[j], kind)).map_err(cause)
                        })
                    })?;
                    time
                };
                Ok(vec![(load, n, time)])
            })
        }
        Load::W26 => {
            let keys = ulid_keys(table_data::order(load.number(), n));
            let expected = table_data::expected_read_sum(n);
            cx.read_only::<Krummi>(Tree::Normal, |store| {
                let (sum, time) = timed(|| {
                    let tx = store.inner().read().map_err(cause)?;
                    let table = tx.open_table(TABLE).map_err(cause)?;
                    let mut sum = 0u64;
                    for key in &keys {
                        let row = tx.get_row(&table, &[ulid(key)]).map_err(cause)?;
                        let row = row.ok_or_else(|| missing(key))?;
                        sum = sum.wrapping_add(checksum(&row)?);
                    }
                    Ok(sum)
                })?;
                check_sum(expected, sum)?;
                Ok(vec![(load, n, time)])
            })
        }
        Load::W27 => {
            let starts = ulid_keys(table_data::range_starts(n, p.ranges));
            let (rows, expected) = table_data::expected_ranges(n, p.ranges, p.range_len);
            let limit = usize::try_from(p.range_len).unwrap_or(usize::MAX);
            cx.read_only::<Krummi>(Tree::Normal, |store| {
                let ((count, sum), time) = timed(|| {
                    let tx = store.inner().read().map_err(cause)?;
                    let table = tx.open_table(TABLE).map_err(cause)?;
                    let (mut count, mut sum) = (0u64, 0u64);
                    for start in &starts {
                        let range = tx.range_rows(&table, &[], ulid(start)..).map_err(cause)?;
                        let mut read = 0;
                        for row in range.take(limit) {
                            let row = row.map_err(cause)?;
                            // A range starts at an existing key: its first row is that key's.
                            if read == 0 && id_of(&row).as_ref() != Some(start) {
                                return Err(missing(start));
                            }
                            sum = sum.wrapping_add(checksum(&row)?);
                            read += 1;
                        }
                        if read == 0 {
                            return Err(missing(start));
                        }
                        count += read;
                    }
                    Ok((count, sum))
                })?;
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

/// The checksum of a row read, every column ([`table_data::checksum`]), each read with its typed accessor (the table
/// has the columns of [`schema`], or the accessor panics); `ts` back in microseconds.
fn checksum(row: &TableRow<'_>) -> Result<u64> {
    let unexpected = || format!("la fila leída no tiene las columnas de la tabla: {row:?}");
    let (COLUMNS, Some(id), Some(n), Some(x), Some(ts), Some(flag), Some(name)) = (
        row.len(),
        id_of(row),
        row.i64(1),
        row.f64(2),
        row.timestamp(3),
        row.bool(4),
        row.text_bytes(5),
    ) else {
        return Err(unexpected());
    };
    // A 64-bit division when the nanoseconds fit, as they do in the loads' data; the 128-bit one costs several times more.
    let ts = match i64::try_from(ts) {
        Ok(ts) => ts / 1_000,
        Err(_) => i64::try_from(ts / 1_000).map_err(|_| unexpected())?,
    };
    Ok(table_data::checksum(&id, n, x, ts, flag, &name))
}

/// Creates the empty table with `schema` in a write of its own.
fn create_table(store: &krummi::Store, schema: &Schema) -> Result<()> {
    let mut tx = store.write().map_err(cause)?;
    tx.open_table_with(TABLE, TableMode::CreateNew(schema))
        .map_err(cause)?;
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
