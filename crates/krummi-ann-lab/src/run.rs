//! The command's three steps: prepare the data sets (S3), compute the exact ground truth (S5) and measure (S6).

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Instant;

use crate::data::Vectors;
use crate::files::{read_fvecs, read_ivecs, write_fvecs, write_ivecs};
use crate::ivf::lists_for;
use crate::kernels::Metric;
use crate::prepare;
use crate::topk;

/// Queries per data set: the latency percentiles and recall of every configuration are taken over them.
pub const QUERIES: usize = 1_000;

/// Neighbours kept in the ground truth: enough for recall@100.
pub const TRUTH_K: usize = 100;

/// The seed that chooses the GloVe queries.
const GLOVE_SEED: u64 = 12;

#[derive(Debug, Default)]
pub struct Options {
    pub folder: PathBuf,
    pub set: Option<Set>,
    pub rows: Option<usize>,
    pub report: Option<PathBuf>,
    /// Only this metric of the set (`--metrica`).
    pub metric: Option<Metric>,
    /// The nprobe sweep instead of the default one (`--sondeos`).
    pub nprobes: Option<Vec<usize>>,
}

/// A data set of S3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Set {
    Sift,
    Glove,
    Cohere,
}

impl Set {
    pub const ALL: [Set; 3] = [Set::Sift, Set::Glove, Set::Cohere];

    pub fn name(self) -> &'static str {
        match self {
            Set::Sift => "sift",
            Set::Glove => "glove",
            Set::Cohere => "cohere",
        }
    }

    /// The metrics measured on this set (defaults of the execution document).
    pub fn metrics(self) -> &'static [Metric] {
        match self {
            Set::Sift => &[Metric::L2Squared],
            Set::Glove => &[Metric::Cosine],
            Set::Cohere => &[Metric::L2Squared, Metric::Dot],
        }
    }

    /// The metric of the ground truth the source publishes, if it publishes one.
    pub fn source_metric(self) -> Option<Metric> {
        match self {
            Set::Sift => Some(Metric::L2Squared),
            Set::Glove => None,
            Set::Cohere => Some(Metric::Cosine),
        }
    }
}

impl fmt::Display for Set {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Set {
    type Err = String;

    fn from_str(text: &str) -> Result<Set, String> {
        Set::ALL
            .into_iter()
            .find(|s| s.name() == text)
            .ok_or_else(|| format!("Conjunto no válido: {text}. Son sift, glove y cohere."))
    }
}

fn data(folder: &Path, file: &str) -> PathBuf {
    folder.join("data").join(file)
}

fn failed(path: &Path, e: impl fmt::Display) -> String {
    format!("No se pudo leer o escribir {}: {e}", path.display())
}

fn sets(options: &Options) -> Vec<Set> {
    options.set.map_or_else(|| Set::ALL.to_vec(), |s| vec![s])
}

/// Converts the downloaded files under `raw/` into `data/<set>-base.fvecs`, `data/<set>-query.fvecs` and, when the
/// source has one, `data/<set>-source.ivecs`. A set whose files already exist is left as it is.
pub fn prepare(options: &Options) -> Result<(), String> {
    let raw = options.folder.join("raw");
    fs::create_dir_all(options.folder.join("data")).map_err(|e| failed(&options.folder, e))?;
    for set in sets(options) {
        let base_path = data(&options.folder, &format!("{set}-base.fvecs"));
        if base_path.exists() {
            println!("{set}: ya preparado.");
            continue;
        }
        let started = Instant::now();
        let query_path = data(&options.folder, &format!("{set}-query.fvecs"));
        let source_path = data(&options.folder, &format!("{set}-source.ivecs"));
        let (base, queries, source) = match set {
            Set::Sift => {
                let dir = raw.join("sift");
                let read = |file: &str| {
                    let path = dir.join(file);
                    read_fvecs(&path).map_err(|e| failed(&path, e))
                };
                let truth = dir.join("sift_groundtruth.ivecs");
                let source = read_ivecs(&truth).map_err(|e| failed(&truth, e))?;
                (
                    read("sift_base.fvecs")?,
                    read("sift_query.fvecs")?,
                    Some(source),
                )
            }
            Set::Glove => {
                let path = raw.join("glove.twitter.27B.100d.txt");
                let glove = prepare::read_glove(&path, 100).map_err(|e| failed(&path, e))?;
                println!("glove: {} filas de ceros descartadas.", glove.zero_rows);
                let (base, queries) = prepare::hold_out(&glove.vectors, QUERIES, GLOVE_SEED);
                (base, queries, None)
            }
            Set::Cohere => {
                let path = |file: &str| raw.join(format!("cohere_{file}.parquet"));
                let vectors = |file: &str| {
                    let p = path(file);
                    prepare::read_parquet_vectors(&p).map_err(|e| failed(&p, e))
                };
                let truth = path("neighbors");
                let source =
                    prepare::read_parquet_neighbours(&truth).map_err(|e| failed(&truth, e))?;
                (vectors("train")?, vectors("test")?, Some(source))
            }
        };
        let queries = queries.prefix(QUERIES);
        write_fvecs(&query_path, &queries).map_err(|e| failed(&query_path, e))?;
        if let Some(source) = source {
            write_ivecs(&source_path, &source[..QUERIES.min(source.len())])
                .map_err(|e| failed(&source_path, e))?;
        }
        write_fvecs(&base_path, &base).map_err(|e| failed(&base_path, e))?;
        println!(
            "{set}: {} vectores de {} dimensiones y {} consultas en {:.1} s.",
            base.len(),
            base.dim(),
            queries.len(),
            started.elapsed().as_secs_f64()
        );
    }
    Ok(())
}

/// The base and the queries of a prepared set, the base cut to `rows` vectors.
pub fn load(folder: &Path, set: Set, rows: Option<usize>) -> Result<(Vectors, Vectors), String> {
    let read = |file: String| {
        let path = data(folder, &file);
        read_fvecs(&path).map_err(|e| failed(&path, e))
    };
    let base = read(format!("{set}-base.fvecs"))?;
    let base = match rows {
        Some(rows) if rows < base.len() => base.prefix(rows),
        _ => base,
    };
    Ok((base, read(format!("{set}-query.fvecs"))?))
}

/// The file of the exact ground truth of `set` under `metric` with a base of `rows` vectors.
pub fn truth_path(folder: &Path, set: Set, metric: Metric, rows: usize) -> PathBuf {
    data(
        folder,
        &format!("{set}-{}-{rows}-exact.ivecs", metric.name()),
    )
}

/// The exact `k` nearest neighbours of every query, on every core: the queries are split among the threads, and
/// each search is the same whichever thread runs it.
pub fn exact_all(base: &Vectors, queries: &Vectors, metric: Metric, k: usize) -> Vec<Vec<u32>> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = queries.len().div_ceil(threads).max(1);
    let mut lists = vec![Vec::new(); queries.len()];
    std::thread::scope(|scope| {
        for (part, slots) in lists.chunks_mut(chunk).enumerate() {
            // Each thread walks the base once for its whole block of queries, so a base row is read from memory
            // once per block instead of once per query; every query still sees the rows in id order, as in
            // `topk::exact`, so the result is the same.
            scope.spawn(move || {
                let first = part * chunk;
                let block: Vec<(&[f32], f64)> = (first..first + slots.len())
                    .map(|q| (queries.row(q), queries.norm(q)))
                    .collect();
                let mut tops: Vec<topk::TopK> = block.iter().map(|_| topk::TopK::new(k)).collect();
                for id in 0..base.len() {
                    let (row, row_norm) = (base.row(id), base.norm(id));
                    for ((query, query_norm), top) in block.iter().zip(&mut tops) {
                        top.push(
                            metric.distance(row, query, row_norm, *query_norm),
                            id as u32,
                        );
                    }
                }
                for (slot, top) in slots.iter_mut().zip(tops) {
                    *slot = top.into_sorted().iter().map(|n| n.id).collect();
                }
            });
        }
    });
    lists
}

/// The share of the first `k` ids of `expected` that are among the first `k` of `found`, over all the queries. A
/// base of fewer than `k` rows has fewer than `k` neighbours to find, so the share is of those it has.
pub fn recall(found: &[Vec<u32>], expected: &[Vec<u32>], k: usize) -> f64 {
    let (mut hits, mut wanted_total) = (0usize, 0usize);
    for (f, e) in found.iter().zip(expected) {
        let wanted = &e[..k.min(e.len())];
        hits += f.iter().take(k).filter(|id| wanted.contains(id)).count();
        wanted_total += wanted.len();
    }
    hits as f64 / wanted_total.max(1) as f64
}

/// How many queries have first `k` neighbours at other distances in `found` than in `expected`. Two lists that
/// differ only in which of several vectors at the same distance they keep (ties) count as equal.
pub fn differing_distances(
    base: &Vectors,
    queries: &Vectors,
    metric: Metric,
    found: &[Vec<u32>],
    expected: &[Vec<u32>],
    k: usize,
) -> usize {
    let distances = |q: usize, ids: &[u32]| {
        let mut d: Vec<f64> = ids[..k.min(ids.len())]
            .iter()
            .map(|&id| {
                let id = id as usize;
                metric.distance(base.row(id), queries.row(q), base.norm(id), queries.norm(q))
            })
            .collect();
        d.sort_by(f64::total_cmp);
        d
    };
    (0..found.len().min(expected.len()))
        .filter(|&q| distances(q, &found[q]) != distances(q, &expected[q]))
        .count()
}

/// Computes and saves the exact ground truth of every metric of the set at `--filas` (all the vectors by default);
/// with the whole base, compares it with the source's own ground truth.
pub fn ground_truth(options: &Options) -> Result<(), String> {
    for set in sets(options) {
        let (base, queries) = load(&options.folder, set, options.rows)?;
        let mut metrics = set.metrics().to_vec();
        if let Some(m) = set.source_metric()
            && !metrics.contains(&m)
        {
            metrics.push(m);
        }
        for metric in metrics {
            let path = truth_path(&options.folder, set, metric, base.len());
            let lists = if path.exists() {
                let lists = read_ivecs(&path).map_err(|e| failed(&path, e))?;
                check_truth(&path, &lists, queries.len(), base.len())?;
                lists
            } else {
                let started = Instant::now();
                let lists = exact_all(&base, &queries, metric, TRUTH_K);
                write_ivecs(&path, &lists).map_err(|e| failed(&path, e))?;
                println!(
                    "{set} {} {}: verdad exacta en {:.1} s.",
                    metric.name(),
                    base.len(),
                    started.elapsed().as_secs_f64()
                );
                lists
            };
            let source = data(&options.folder, &format!("{set}-source.ivecs"));
            if set.source_metric() == Some(metric) && options.rows.is_none() && source.exists() {
                let theirs = read_ivecs(&source).map_err(|e| failed(&source, e))?;
                println!(
                    "{set} {}: coincidencia con la verdad de la fuente, recall@10 {:.4}, recall@100 {:.4}; \
                     consultas cuyas 10 primeras distancias difieren: {}.",
                    metric.name(),
                    recall(&lists, &theirs, 10),
                    recall(&lists, &theirs, TRUTH_K),
                    differing_distances(&base, &queries, metric, &lists, &theirs, 10)
                );
            }
        }
    }
    Ok(())
}

/// Measures the approximate searches of S5 (steps 1 and 2) on every metric of the set at `--filas` (all the vectors
/// by default), prints each section and appends it to `--informe` when given. The exact ground truth must exist
/// (`verdad`).
pub fn measure(options: &Options) -> Result<(), String> {
    sections(options, |title, base, queries, metric, truth| {
        crate::measure::measure(
            title,
            base,
            queries,
            metric,
            truth,
            options.nprobes.as_deref(),
        )
    })
}

/// Measures the external HNSW (step 3) as [`measure`] measures the other searches.
pub fn graph(options: &Options) -> Result<(), String> {
    sections(options, crate::hnsw::measure)
}

/// Runs `section` on every set and metric chosen by `options`, prints each section and appends it to `--informe`.
fn sections(
    options: &Options,
    section: impl Fn(&str, &Vectors, &Vectors, Metric, &[Vec<u32>]) -> String,
) -> Result<(), String> {
    for (set, metrics) in chosen(options)? {
        let (base, queries) = load(&options.folder, set, options.rows)?;
        if let Some(nprobes) = &options.nprobes {
            check_nprobes(nprobes, base.len())?;
        }
        for metric in metrics {
            let path = truth_path(&options.folder, set, metric, base.len());
            if !path.exists() {
                return Err(format!(
                    "Falta la verdad exacta {}: ejecute antes la orden verdad con las mismas opciones.",
                    path.display()
                ));
            }
            let truth = read_ivecs(&path).map_err(|e| failed(&path, e))?;
            check_truth(&path, &truth, queries.len(), base.len())?;
            let title = format!(
                "{set}, {} filas de {} dimensiones, {}",
                base.len(),
                base.dim(),
                metric.name()
            );
            let started = Instant::now();
            let section = section(&title, &base, &queries, metric, &truth);
            println!(
                "{section}(medido en {:.1} s)
",
                started.elapsed().as_secs_f64()
            );
            if let Some(report) = &options.report {
                use std::io::Write;
                let mut file = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(report)
                    .map_err(|e| failed(report, e))?;
                file.write_all(section.as_bytes())
                    .map_err(|e| failed(report, e))?;
            }
        }
    }
    Ok(())
}

/// The sets and metrics a measurement walks: every metric of each set, or only `--metrica`. Without `--conjunto`, a
/// set that lacks that metric is skipped; with it, the set must have the metric.
fn chosen(options: &Options) -> Result<Vec<(Set, Vec<Metric>)>, String> {
    let chosen: Vec<(Set, Vec<Metric>)> = sets(options)
        .into_iter()
        .map(|set| {
            let metrics = set
                .metrics()
                .iter()
                .copied()
                .filter(|&m| options.metric.is_none_or(|only| only == m))
                .collect();
            (set, metrics)
        })
        .filter(|(_, metrics): &(Set, Vec<Metric>)| !metrics.is_empty())
        .collect();
    match (options.set, options.metric) {
        (Some(set), Some(metric)) if chosen.is_empty() => Err(format!(
            "El conjunto {set} no se mide con la métrica {}.",
            metric.name()
        )),
        _ => Ok(chosen),
    }
}

/// Fails when a cached exact ground truth does not belong to these queries and this base: one list per query, each of
/// `TRUTH_K` ids (or all the rows of a smaller base). Without the check, recall would be computed on the shorter side.
fn check_truth(path: &Path, truth: &[Vec<u32>], queries: usize, rows: usize) -> Result<(), String> {
    let k = TRUTH_K.min(rows);
    if truth.len() == queries && truth.iter().all(|list| list.len() == k) {
        Ok(())
    } else {
        Err(format!(
            "La verdad exacta {} no corresponde a las {queries} consultas ni a las {rows} filas: bórrela y ejecute \
             de nuevo la orden verdad.",
            path.display()
        ))
    }
}

/// Fails when an explicit nprobe exceeds the lists of a base of `rows` vectors, so no requested row is dropped.
fn check_nprobes(nprobes: &[usize], rows: usize) -> Result<(), String> {
    let lists = lists_for(rows);
    match nprobes.iter().find(|&&n| n > lists) {
        Some(n) => Err(format!(
            "El sondeo {n} supera las {lists} listas de {rows} filas."
        )),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recall_counts_the_ids_found_among_the_first_k() {
        let expected = vec![vec![1, 2, 3, 4], vec![5, 6, 7, 8]];
        let found = vec![vec![2, 1, 9, 4], vec![5, 0, 0, 0]];
        assert_eq!(recall(&found, &expected, 2), 0.75);
        assert_eq!(recall(&found, &expected, 4), 0.5);
        // A base of fewer than k rows: finding all of its neighbours is a recall of 1.
        let small = vec![vec![0, 1, 2, 3, 4]];
        assert_eq!(recall(&small, &small, 10), 1.0);
        assert_eq!(recall(&[vec![0, 1, 2, 3, 9]], &small, 10), 0.8);
    }

    #[test]
    fn exact_all_is_the_exact_search_of_each_query() {
        let base = Vectors::new(4, (0..400).map(|i| ((i * 37) % 101) as f32).collect());
        let queries = base.prefix(13);
        let lists = exact_all(&base, &queries, Metric::L2Squared, 5);
        for (q, list) in lists.iter().enumerate() {
            let one: Vec<u32> = topk::exact(&base, queries.row(q), Metric::L2Squared, 5)
                .iter()
                .map(|n| n.id)
                .collect();
            assert_eq!(list, &one);
        }
    }

    #[test]
    fn explicit_nprobes_above_the_lists_are_rejected() {
        assert_eq!(check_nprobes(&[4, 10], 10_000), Ok(()));
        assert_eq!(
            check_nprobes(&[4, 11], 10_000),
            Err("El sondeo 11 supera las 10 listas de 10000 filas.".to_string())
        );
    }

    #[test]
    fn a_set_without_the_chosen_metric_is_skipped_unless_named() {
        let options = |set: Option<Set>| Options {
            set,
            metric: Some(Metric::Cosine),
            ..Options::default()
        };
        assert_eq!(
            chosen(&options(None)).unwrap(),
            vec![(Set::Glove, vec![Metric::Cosine])]
        );
        assert_eq!(
            chosen(&options(Some(Set::Sift))).unwrap_err(),
            "El conjunto sift no se mide con la métrica coseno."
        );
        assert_eq!(chosen(&Options::default()).unwrap().len(), Set::ALL.len());
    }

    #[test]
    fn a_truth_of_other_queries_or_another_k_is_rejected() {
        let path = Path::new("t.ivecs");
        let lists = |n: usize, k: usize| vec![vec![0u32; k]; n];
        assert_eq!(check_truth(path, &lists(3, TRUTH_K), 3, 1000), Ok(()));
        assert_eq!(check_truth(path, &lists(3, 50), 3, 50), Ok(()));
        let message = "La verdad exacta t.ivecs no corresponde a las 3 consultas ni a las 1000 filas: bórrela y \
                       ejecute de nuevo la orden verdad.";
        assert_eq!(
            check_truth(path, &lists(2, TRUTH_K), 3, 1000).unwrap_err(),
            message
        );
        assert_eq!(
            check_truth(path, &lists(3, 10), 3, 1000).unwrap_err(),
            message
        );
    }

    #[test]
    fn differing_distances_counts_only_real_differences_not_ties() {
        // Rows 1 and 2 are at the same distance from the query; row 3 is farther.
        let base = Vectors::new(1, vec![0.0, 1.0, -1.0, 2.0]);
        let queries = Vectors::new(1, vec![0.0]);
        let expected = vec![vec![0, 1]];
        let count = |found: Vec<u32>| {
            differing_distances(&base, &queries, Metric::L2Squared, &[found], &expected, 2)
        };
        assert_eq!(count(vec![0, 2]), 0);
        assert_eq!(count(vec![0, 3]), 1);
    }
}
