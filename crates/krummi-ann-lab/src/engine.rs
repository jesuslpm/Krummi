//! The measurements of VI15 of the vector indexes PRD (7.6) on Krummi itself, with the data the experiment prepared:
//! the load, the insert of one row with and without the index, the training and the build with the longest turn of a
//! batch, recall@10 and latency without a filter and with a filter that lets few rows pass and one that lets many pass,
//! and the threshold of VI6. One thread searches; the store keeps its pages warm between the warm-up and the timing.
//!
//! Each step writes a Markdown section to the report and leaves the store in `<folder>/motor`, which the next step uses:
//! `cargar` (load, inserts, index), `buscar` (searches), `umbral` (the exact search against the index over filters of
//! growing size), `indice` and `sin-indice` (create or drop the secondary index `by_cat` that the filters can use).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use krummi::table::value::{ColumnType, Value, VectorElement, VectorType};
use krummi::{
    Column, FilterCondition, IndexMode, Nearest, Schema, Store, StoreOptions, TableMode,
    VectorIndexDefinition, VectorIndexState, VectorMetric,
};

use crate::data::Vectors;
use crate::files::read_ivecs;
use crate::kernels::Metric;
use crate::run::{Options, Set, load, truth_path};

/// The rows of each load transaction.
const LOAD_BATCH: usize = 10_000;
/// The single-row inserts timed with and without the index.
const INSERTS: usize = 2_000;
/// The queries of the filtered searches.
const FILTERED_QUERIES: usize = 200;
/// The categories of the column `cat`: a row's is its id modulo this.
const CATEGORIES: i64 = 100;

fn vector_value(v: &[f32]) -> Value<'static> {
    Value::Vector(
        v.iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<u8>>()
            .into(),
    )
}

fn fail(e: impl std::fmt::Display) -> String {
    format!("Fallo del motor: {e}")
}

/// The percentile `p` (0 to 100) of `times`, in milliseconds.
fn percentile(times: &mut [Duration], p: f64) -> f64 {
    times.sort();
    let at = ((times.len() as f64 - 1.0) * p / 100.0).round() as usize;
    times[at].as_secs_f64() * 1000.0
}

fn append(report: &Path, text: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(report)
        .map_err(fail)?;
    file.write_all(text.as_bytes()).map_err(fail)?;
    print!("{text}");
    Ok(())
}

fn open(folder: &Path) -> Result<Store, String> {
    Store::open(folder.join("motor"), &StoreOptions::default()).map_err(fail)
}

fn schema(dim: usize) -> Schema {
    let ty = VectorType {
        element: VectorElement::F32,
        dimension: u16::try_from(dim).expect("at most 4,096"),
        cosine: false,
    };
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("cat", ColumnType::I64).not_null(),
            Column::new("v", ColumnType::Vector(ty)),
        ],
        &["id"],
    )
}

/// Inserts `count` rows of ids from `first`, one per transaction, with vectors of `base`, and returns the p50 and p95 of
/// one insert with its commit (durable when `durable`).
fn timed_inserts(
    store: &Store,
    base: &Vectors,
    first: i64,
    count: usize,
    durable: bool,
) -> Result<(f64, f64), String> {
    let mut times = Vec::with_capacity(count);
    for i in 0..count {
        let id = first + i as i64;
        let row = [
            ("id", Value::I64(id)),
            ("cat", Value::I64(id % CATEGORIES)),
            ("v", vector_value(base.row(i % base.len()))),
        ];
        let start = Instant::now();
        let mut write = store.write().map_err(fail)?;
        let table = write.open_table("docs").map_err(fail)?;
        write.insert_row(&table, &row).map_err(fail)?;
        if durable {
            write.commit().map_err(fail)?;
        } else {
            write.commit_deferred().map_err(fail)?;
        }
        times.push(start.elapsed());
    }
    Ok((percentile(&mut times, 50.0), percentile(&mut times, 95.0)))
}

/// Deletes the rows of ids `first..first + count`.
fn delete_rows(store: &Store, first: i64, count: usize) -> Result<(), String> {
    let mut write = store.write().map_err(fail)?;
    let table = write.open_table("docs").map_err(fail)?;
    write
        .delete_range(
            &table,
            &[],
            Value::I64(first)..Value::I64(first + count as i64),
        )
        .map_err(fail)?;
    write.commit().map_err(fail)
}

fn wait_ready(store: &Store) -> Result<(), String> {
    loop {
        store.wait_vector_idle();
        let read = store.read().map_err(fail)?;
        let table = read.open_table("docs").map_err(fail)?;
        let info = read.vector_index_info(&table, "near").map_err(fail)?;
        if info.state == VectorIndexState::Ready && info.pending == 0 {
            return Ok(());
        }
        drop(read);
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The step `cargar`: loads the base, times the inserts without the index, creates the index and waits until it is
/// ready, then times the inserts with it.
pub fn load_step(options: &Options, report: &Path) -> Result<(), String> {
    let (base, _) = load(&options.folder, Set::Cohere, options.rows)?;
    let _ = fs::remove_dir_all(options.folder.join("motor"));
    let store = open(&options.folder)?;
    let mut write = store.write().map_err(fail)?;
    write
        .open_table_with("docs", TableMode::CreateNew(&schema(base.dim())))
        .map_err(fail)?;
    write.commit().map_err(fail)?;
    let started = Instant::now();
    for first in (0..base.len()).step_by(LOAD_BATCH) {
        let mut write = store.write().map_err(fail)?;
        let table = write.open_table("docs").map_err(fail)?;
        for id in first..(first + LOAD_BATCH).min(base.len()) {
            let row = [
                ("id", Value::I64(id as i64)),
                ("cat", Value::I64(id as i64 % CATEGORIES)),
                ("v", vector_value(base.row(id))),
            ];
            write.insert_row(&table, &row).map_err(fail)?;
        }
        write.commit().map_err(fail)?;
    }
    let load_time = started.elapsed();
    let rows = base.len() as i64;
    let first = rows + 1_000_000;
    let without_durable = timed_inserts(&store, &base, first, INSERTS, true)?;
    delete_rows(&store, first, INSERTS)?;
    let without_deferred = timed_inserts(&store, &base, first, INSERTS, false)?;
    delete_rows(&store, first, INSERTS)?;
    store.make_durable().map_err(fail)?;
    // The index, built while nothing else writes.
    store.take_vector_turns();
    let started = Instant::now();
    let mut write = store.write().map_err(fail)?;
    let table = write.open_table("docs").map_err(fail)?;
    let definition = VectorIndexDefinition::new("near", "v", VectorMetric::L2);
    write
        .create_vector_index(&table, &definition, IndexMode::CreateNew)
        .map_err(fail)?;
    write.commit().map_err(fail)?;
    wait_ready(&store)?;
    let build_time = started.elapsed();
    let (build_turn, build_batches) = store.take_vector_turns();
    // With the worker paused, the cost of the insert itself (VI15: the worker without work); then with it running.
    store.pause_vector_work(true);
    let with_durable = timed_inserts(&store, &base, first, INSERTS, true)?;
    let with_deferred = timed_inserts(&store, &base, first + INSERTS as i64, INSERTS, false)?;
    store.pause_vector_work(false);
    wait_ready(&store)?;
    store.take_vector_turns();
    let running = timed_inserts(&store, &base, first + 2 * INSERTS as i64, INSERTS, true)?;
    wait_ready(&store)?;
    let (insert_turn, insert_batches) = store.take_vector_turns();
    delete_rows(&store, first, 3 * INSERTS)?;
    wait_ready(&store)?;
    let (info_lists, info) = {
        let read = store.read().map_err(fail)?;
        let table = read.open_table("docs").map_err(fail)?;
        let info = read.vector_index_info(&table, "near").map_err(fail)?;
        (info.lists, info)
    };
    let text = format!(
        "## Carga, inserción e índice ({} filas de {} dimensiones)\n\n\
         | Medida | Valor |\n|---|---|\n\
         | Carga, transacciones de {LOAD_BATCH} filas | {:.1} s |\n\
         | Insertar una fila sin el índice, confirmación durable (p50 / p95) | {:.3} / {:.3} ms |\n\
         | Insertar una fila con el índice, confirmación durable, trabajador en pausa (p50 / p95) | {:.3} / {:.3} ms |\n\
         | Razón del p95 durable, con / sin | {:.3} |\n\
         | Insertar una fila con el índice, confirmación durable, trabajador activo (p50 / p95) | {:.3} / {:.3} ms |\n\
         | Insertar una fila sin el índice, confirmación diferida (p50 / p95) | {:.3} / {:.3} ms |\n\
         | Insertar una fila con el índice, confirmación diferida, trabajador en pausa (p50 / p95) | {:.3} / {:.3} ms |\n\
         | Razón del p95 diferido, con / sin | {:.3} |\n\
         | Entrenar y construir (crear hasta listo) | {:.1} s |\n\
         | Lotes de la construcción; el que más retuvo el turno | {build_batches}; {:.2} ms |\n\
         | Lotes de las inserciones con el trabajador activo; el que más retuvo el turno | {insert_batches}; {:.2} ms |\n\
         | Listas; indicadores | {:?}; saturados {:?}, lista mayor {:?} |\n\n",
        base.len(),
        base.dim(),
        load_time.as_secs_f64(),
        without_durable.0,
        without_durable.1,
        with_durable.0,
        with_durable.1,
        with_durable.1 / without_durable.1,
        running.0,
        running.1,
        without_deferred.0,
        without_deferred.1,
        with_deferred.0,
        with_deferred.1,
        with_deferred.1 / without_deferred.1,
        build_time.as_secs_f64(),
        build_turn.as_secs_f64() * 1000.0,
        insert_turn.as_secs_f64() * 1000.0,
        info_lists,
        info.saturated,
        info.largest_list,
    );
    append(report, &text)?;
    store.close().map_err(fail)
}

/// The ids of the 10 nearest of each query in the exact ground truth.
fn truth(options: &Options, rows: usize) -> Result<Vec<Vec<u32>>, String> {
    let path = truth_path(&options.folder, Set::Cohere, Metric::L2Squared, rows);
    let lists = read_ivecs(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(lists.into_iter().map(|l| l[..10].to_vec()).collect())
}

fn ids(found: &krummi::Neighbours) -> Vec<u32> {
    (found.rows.iter())
        .map(|n| match n.key[..] {
            [Value::I64(id)] => id as u32,
            _ => u32::MAX,
        })
        .collect()
}

/// recall@10, p50 and p95 of `query` over `queries` with `filter`, after one warm-up pass; `expected` gives each
/// query's 10 true neighbours.
fn sweep(
    store: &Store,
    queries: &Vectors,
    count: usize,
    probes: Option<usize>,
    filter: &[FilterCondition<'_>],
    expected: &[Vec<u32>],
) -> Result<(f64, f64, f64, bool), String> {
    let read = store.read().map_err(fail)?;
    let table = read.open_table("docs").map_err(fail)?;
    let query = |q: usize| {
        let mut nearest =
            Nearest::new(vector_value(queries.row(q)), 10).with_filter(filter.to_vec());
        if let Some(probes) = probes {
            nearest = nearest.with_probes(probes);
        }
        nearest
    };
    for q in 0..count {
        read.nearest(&table, "near", &query(q)).map_err(fail)?;
    }
    let mut times = Vec::with_capacity(count);
    let mut hits = 0usize;
    let mut exact = true;
    for (q, expected) in expected.iter().enumerate().take(count) {
        let nearest = query(q);
        let start = Instant::now();
        let found = read.nearest(&table, "near", &nearest).map_err(fail)?;
        times.push(start.elapsed());
        exact &= found.exact;
        hits += ids(&found)
            .iter()
            .filter(|id| expected.contains(id))
            .count();
    }
    let recall = hits as f64 / (10 * count) as f64;
    Ok((
        recall,
        percentile(&mut times, 50.0),
        percentile(&mut times, 95.0),
        exact,
    ))
}

/// The exact 10 nearest of the first `count` queries with `filter`, on every core.
fn filtered_truth(
    store: &Store,
    queries: &Vectors,
    count: usize,
    filter: &[FilterCondition<'_>],
) -> Result<Vec<Vec<u32>>, String> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let read = store.read().map_err(fail)?;
    let table = read.open_table("docs").map_err(fail)?;
    let mut out = vec![Vec::new(); count];
    let chunk = count.div_ceil(threads);
    std::thread::scope(|scope| -> Result<(), String> {
        let mut handles = Vec::new();
        for (part, slots) in out.chunks_mut(chunk).enumerate() {
            let (read, table) = (&read, &table);
            handles.push(scope.spawn(move || -> Result<(), String> {
                for (i, slot) in slots.iter_mut().enumerate() {
                    let q = part * chunk + i;
                    let nearest =
                        Nearest::new(vector_value(queries.row(q)), 10).with_filter(filter.to_vec());
                    let found = read
                        .nearest_exact(table, "v", VectorMetric::L2, &nearest)
                        .map_err(fail)?;
                    *slot = ids(&found);
                }
                Ok(())
            }));
        }
        for handle in handles {
            handle.join().expect("a thread of the truth")?;
        }
        Ok(())
    })?;
    Ok(out)
}

/// The step `buscar`: recall@10 and latency without a filter for a sweep of probes, and with a filter that passes few
/// rows (1 %) and one that passes many (50 %).
pub fn search_step(options: &Options, report: &Path) -> Result<(), String> {
    let (base, queries) = load(&options.folder, Set::Cohere, options.rows)?;
    let expected = truth(options, base.len())?;
    let store = open(&options.folder)?;
    wait_ready(&store)?;
    let lists = {
        let read = store.read().map_err(fail)?;
        let table = read.open_table("docs").map_err(fail)?;
        read.vector_index_info(&table, "near")
            .map_err(fail)?
            .lists
            .unwrap_or(1) as usize
    };
    let mut text = format!(
        "## Búsquedas sin filtro ({} consultas, k = 10, {lists} listas)\n\n\
         | Sondeos | % listas | recall@10 | p50 ms | p95 ms |\n|---|---|---|---|---|\n",
        queries.len()
    );
    let sweep_probes = options
        .nprobes
        .clone()
        .unwrap_or_else(|| vec![20, 30, 40, 50, 60, 80]);
    for probes in sweep_probes.iter().copied().map(Some).chain([None]) {
        let (recall, p50, p95, _) = sweep(&store, &queries, queries.len(), probes, &[], &expected)?;
        let label = probes.map_or("por defecto".to_string(), |p| p.to_string());
        let share = probes.map_or(100.0 * krummi::table::DEFAULT_PROBE_SHARE, |p| {
            100.0 * p as f64 / lists as f64
        });
        text.push_str(&format!(
            "| {label} | {share:.1} | {recall:.4} | {p50:.2} | {p95:.2} |\n"
        ));
    }
    append(report, &format!("{text}\n"))?;
    let few = vec![FilterCondition::Equal {
        column: "cat",
        value: Value::I64(7),
    }];
    let many = vec![FilterCondition::Range {
        column: "cat",
        lower: std::ops::Bound::Included(Value::I64(0)),
        upper: std::ops::Bound::Excluded(Value::I64(50)),
    }];
    let mut text = format!(
        "## Búsquedas con filtro ({FILTERED_QUERIES} consultas, k = 10)\n\n\
         | Filtro | Filas que pasan | Sondeos | Camino | recall@10 | p50 ms | p95 ms |\n|---|---|---|---|---|---|---|\n"
    );
    let indexed = {
        let read = store.read().map_err(fail)?;
        !read
            .describe_table("docs")
            .map_err(fail)?
            .indexes()
            .is_empty()
    };
    let note = if indexed {
        "con el índice secundario `by_cat`"
    } else {
        "sin índice sobre `cat`"
    };
    text = text.replacen("k = 10)", &format!("k = 10, {note})"), 1);
    for (label, filter, passing) in [
        ("cat = 7", &few, base.len() / 100),
        ("cat < 50", &many, base.len() / 2),
    ] {
        let truth = filtered_truth(&store, &queries, FILTERED_QUERIES, filter)?;
        for probes in sweep_probes.iter().copied().map(Some).chain([None]) {
            let (recall, p50, p95, exact) =
                sweep(&store, &queries, FILTERED_QUERIES, probes, filter, &truth)?;
            let path = if exact { "exacto" } else { "aproximado" };
            let probes = probes.map_or("por defecto".to_string(), |p| p.to_string());
            text.push_str(&format!(
                "| {label} | {passing} | {probes} | {path} | {recall:.4} | {p50:.2} | {p95:.2} |\n"
            ));
        }
    }
    append(report, &format!("{text}\n"))?;
    store.close().map_err(fail)
}

/// The step `indice`: creates the secondary index `by_cat` over `cat`, through which the filters find their rows.
pub fn index_step(options: &Options, report: &Path) -> Result<(), String> {
    let store = open(&options.folder)?;
    let started = Instant::now();
    let mut write = store.write().map_err(fail)?;
    let table = write.open_table("docs").map_err(fail)?;
    let by_cat = krummi::IndexDefinition::new("by_cat", vec![krummi::IndexKey::new("cat")]);
    write
        .create_index(&table, &by_cat, IndexMode::CreateIfNotExists)
        .map_err(fail)?;
    write.commit().map_err(fail)?;
    append(
        report,
        &format!(
            "Índice secundario `by_cat` creado en {:.1} s.\n\n",
            started.elapsed().as_secs_f64()
        ),
    )?;
    store.close().map_err(fail)
}

/// The step `sin-indice`: drops the secondary index `by_cat`, so that the filters read the table.
pub fn drop_index_step(options: &Options, report: &Path) -> Result<(), String> {
    let store = open(&options.folder)?;
    let mut write = store.write().map_err(fail)?;
    let table = write.open_table("docs").map_err(fail)?;
    write.drop_index(&table, "by_cat").map_err(fail)?;
    write.commit().map_err(fail)?;
    append(report, "Índice secundario `by_cat` borrado.\n\n")?;
    store.close().map_err(fail)
}

/// The step `umbral`: for filters that pass from 0.5 % to 20 % of the rows, the p50 of the exact path (threshold above
/// the rows) against the index's (threshold zero), to place the threshold of VI6 where the index starts to win.
pub fn threshold_step(options: &Options, report: &Path) -> Result<(), String> {
    let (base, queries) = load(&options.folder, Set::Cohere, options.rows)?;
    let store = open(&options.folder)?;
    wait_ready(&store)?;
    let mut text = "## Umbral de VI6 (p50 de 100 consultas, k = 10, sondeos por defecto)\n\n\
                    | Filtro | Filas que pasan | Exacto p50 ms | Índice p50 ms | recall@10 del índice |\n|---|---|---|---|---|\n"
        .to_string();
    for top in [1i64, 2, 3, 5, 10, 20] {
        // `cat` is the id modulo 100: `cat < top` passes `top` percent of the rows.
        let filter = vec![FilterCondition::Range {
            column: "cat",
            lower: std::ops::Bound::Included(Value::I64(0)),
            upper: std::ops::Bound::Excluded(Value::I64(top)),
        }];
        let passing = base.len() as i64 * top / CATEGORIES;
        let truth = filtered_truth(&store, &queries, 100, &filter)?;
        store.set_vector_search_threshold(Some(u64::MAX));
        let (_, exact_p50, _, exact) = sweep(&store, &queries, 100, None, &filter, &truth)?;
        store.set_vector_search_threshold(Some(0));
        let (recall, index_p50, _, approximate) =
            sweep(&store, &queries, 100, None, &filter, &truth)?;
        store.set_vector_search_threshold(None);
        assert!(exact && !approximate, "the hooks choose the paths");
        text.push_str(&format!(
            "| cat < {top} | {passing} | {exact_p50:.2} | {index_p50:.2} | {recall:.4} |\n"
        ));
    }
    append(report, &format!("{text}\n"))?;
    store.close().map_err(fail)
}
