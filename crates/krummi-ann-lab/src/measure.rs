//! The measurements of S6 for one data set, one scale and one metric: the approximate SQ8 scan with rerank (step 1)
//! and IVF-Flat and IVF-SQ8 over an nprobe sweep (step 2), against the exact ground truth. Latencies are taken on one
//! thread, query after query, with the data in memory (warm); recall@100 searches run on every core, untimed.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use crate::data::Vectors;
use crate::ivf::{Ivf, lists_for, sample_ids};
use crate::kernels::Metric;
use crate::search::{push_codes, rerank};
use crate::sq8::Codes;
use crate::topk::{self, Neighbour, TopK};

/// The seeds of the sample and of k-means.
const SAMPLE_SEED: u64 = 21;
const KMEANS_SEED: u64 = 33;

/// Rerank factors r of step 1: `k·r` candidates are reranked.
const SCAN_FACTORS: [usize; 5] = [1, 2, 4, 8, 16];

/// Rerank factors of IVF-SQ8 measured with their latency.
const IVF_FACTORS: [usize; 3] = [2, 4, 8];

/// The default nprobe sweep, cut at the number of lists.
const NPROBES: [usize; 8] = [1, 2, 4, 8, 16, 32, 64, 128];

/// Queries run untimed before each timed configuration.
const WARMUP: usize = 50;

/// Queries of the exact scan's latency: a full scan each, so fewer than the others.
const EXACT_QUERIES: usize = 100;

/// Latency percentiles of one configuration, in milliseconds.
#[derive(Clone, Copy, Debug)]
pub struct Latency {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub mean: f64,
}

impl Latency {
    pub fn of(mut times: Vec<Duration>) -> Latency {
        times.sort();
        let at = |p: f64| {
            let i = ((times.len() as f64 * p).ceil() as usize).clamp(1, times.len()) - 1;
            times[i].as_secs_f64() * 1e3
        };
        let mean = times.iter().map(Duration::as_secs_f64).sum::<f64>() / times.len() as f64 * 1e3;
        Latency {
            p50: at(0.50),
            p95: at(0.95),
            p99: at(0.99),
            mean,
        }
    }
}

/// A running FNV-1a hash of results, to compare the same run on two platforms.
#[derive(Clone, Copy)]
pub struct Digest(u64);

impl Default for Digest {
    fn default() -> Digest {
        Digest(0xcbf2_9ce4_8422_2325)
    }
}

impl Digest {
    pub fn add(&mut self, results: &[Neighbour]) {
        for n in results {
            for byte in
                n.id.to_le_bytes()
                    .into_iter()
                    .chain(n.distance.to_bits().to_le_bytes())
            {
                self.0 ^= u64::from(byte);
                self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
            }
        }
    }

    pub fn hex(self) -> String {
        format!("{:016x}", self.0)
    }
}

fn ids(results: &[Neighbour]) -> Vec<u32> {
    results.iter().map(|n| n.id).collect()
}

/// Runs `search` over every query on this thread, timed, after a warm-up; returns the results and the latency.
pub(crate) fn timed(
    queries: &Vectors,
    search: impl Fn(&[f32]) -> Vec<Neighbour>,
) -> (Vec<Vec<Neighbour>>, Latency) {
    for q in 0..WARMUP.min(queries.len()) {
        std::hint::black_box(search(queries.row(q)));
    }
    let mut times = Vec::with_capacity(queries.len());
    let mut results = Vec::with_capacity(queries.len());
    for q in 0..queries.len() {
        let started = Instant::now();
        let r = search(queries.row(q));
        times.push(started.elapsed());
        results.push(r);
    }
    (results, Latency::of(times))
}

/// Runs `work(i)` for every query index `i` on every core, untimed.
pub(crate) fn parallel<T: Clone + Default + Send>(
    count: usize,
    work: impl Fn(usize) -> T + Sync,
) -> Vec<T> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = count.div_ceil(threads).max(1);
    let mut out = vec![T::default(); count];
    std::thread::scope(|scope| {
        for (part, slots) in out.chunks_mut(chunk).enumerate() {
            let work = &work;
            scope.spawn(move || {
                for (i, slot) in slots.iter_mut().enumerate() {
                    *slot = work(part * chunk + i);
                }
            });
        }
    });
    out
}

pub(crate) fn recall_of(results: &[Vec<Neighbour>], truth: &[Vec<u32>], k: usize) -> f64 {
    let found: Vec<Vec<u32>> = results.iter().map(|r| ids(r)).collect();
    crate::run::recall(&found, truth, k)
}

pub(crate) fn mib(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

/// Measures one data set at one scale under one metric and returns its section of the report, in Markdown. `nprobes`
/// replaces the default sweep.
pub fn measure(
    title: &str,
    base: &Vectors,
    queries: &Vectors,
    metric: Metric,
    truth: &[Vec<u32>],
    nprobes: Option<&[usize]>,
) -> String {
    let rows = base.len();
    let lists = lists_for(rows);
    let sample = sample_ids(rows, lists, SAMPLE_SEED);
    let mut out = String::new();
    let mut digest = Digest::default();
    let _ = writeln!(out, "### {title}\n");

    // The exact scan, the baseline of the latency (document 12, *Search latency*).
    let exact_queries = queries.prefix(EXACT_QUERIES);
    let (exact, exact_latency) = timed(&exact_queries, |q| topk::exact(base, q, metric, 10));
    exact.iter().for_each(|r| digest.add(r));
    let _ = writeln!(
        out,
        "Búsqueda exacta ({EXACT_QUERIES} consultas): p50 {:.2} ms, p95 {:.2} ms, p99 {:.2} ms.\n",
        exact_latency.p50, exact_latency.p95, exact_latency.p99
    );

    // Step 1: the approximate SQ8 scan with rerank.
    let started = Instant::now();
    let codes = Codes::build(base, metric, &sample);
    let codes_time = started.elapsed().as_secs_f64();
    let most = 100 * SCAN_FACTORS[SCAN_FACTORS.len() - 1];
    let candidates = |q: &[f32]| -> Vec<u32> {
        let prepared = codes.prepare(q);
        let mut top = TopK::new(most);
        push_codes(
            &mut top,
            &prepared,
            (0..rows).map(|id| (id as u32, codes.code(id))),
        );
        ids(&top.into_sorted())
    };
    let (scan, scan_latency) = timed(queries, |q| {
        let prepared = codes.prepare(q);
        let mut top = TopK::new(40);
        push_codes(
            &mut top,
            &prepared,
            (0..rows).map(|id| (id as u32, codes.code(id))),
        );
        rerank(
            base,
            q,
            metric,
            top.into_sorted().into_iter().map(|n| n.id),
            10,
        )
    });
    scan.iter().for_each(|r| digest.add(r));
    let ordered: Vec<Vec<u32>> = parallel(queries.len(), |i| candidates(queries.row(i)));
    let _ = writeln!(
        out,
        "Paso 1, escaneo SQ8 aproximado con reordenación: códigos de {:.1} MiB en {codes_time:.1} s; \
         {} códigos con algún componente saturado ({} componentes).\n",
        mib(codes.codes.len()),
        codes.saturated_codes,
        codes.saturated_components
    );
    let _ = writeln!(
        out,
        "| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |"
    );
    let _ = writeln!(out, "|---|---|---|");
    for r in SCAN_FACTORS {
        let mut at = |k: usize| -> f64 {
            let results = parallel(queries.len(), |i| {
                let q = queries.row(i);
                rerank(base, q, metric, ordered[i][..k * r].iter().copied(), k)
            });
            results.iter().for_each(|res| digest.add(res));
            recall_of(&results, truth, k)
        };
        let _ = writeln!(out, "| {r} | {:.4} | {:.4} |", at(10), at(100));
    }
    let _ = writeln!(
        out,
        "\nLatencia del escaneo con r = 4 y k = 10: p50 {:.2} ms, p95 {:.2} ms, p99 {:.2} ms.\n",
        scan_latency.p50, scan_latency.p95, scan_latency.p99
    );

    // Step 2: IVF-Flat and IVF-SQ8.
    let started = Instant::now();
    let ivf = Ivf::build(base, metric, lists, &sample, KMEANS_SEED);
    let build_time = started.elapsed().as_secs_f64();
    let (sq8_bytes, flat_bytes) = ivf.bytes();
    let _ = writeln!(
        out,
        "Paso 2, IVF: {lists} listas, muestra de {} vectores, construido en {build_time:.1} s (k-means de 10 \
         iteraciones, cuantizador y asignación de todas las filas); lista mayor {:.2} veces la media; IVF-SQ8 \
         {:.1} MiB, IVF-Flat {:.1} MiB; {} códigos con algún componente saturado.\n",
        sample.len(),
        ivf.imbalance(),
        mib(sq8_bytes),
        mib(flat_bytes),
        ivf.saturated_codes
    );
    let _ = writeln!(
        out,
        "| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|---|---|---|---|");
    let nprobes = nprobes.unwrap_or(&NPROBES);
    for nprobe in nprobes.iter().copied().filter(|&n| n <= lists) {
        let share = 100.0 * nprobe as f64 / lists as f64;
        let row = |out: &mut String,
                   index: &str,
                   r: &str,
                   r10: f64,
                   r100: Option<f64>,
                   l: Latency| {
            let r100 = r100.map_or("—".to_string(), |v| format!("{v:.4}"));
            let _ = writeln!(
                out,
                "| {nprobe} | {share:.1} | {index} | {r} | {r10:.4} | {r100} | {:.2} | {:.2} | {:.2} | {:.0} |",
                l.p50,
                l.p95,
                l.p99,
                1e3 / l.mean
            );
        };
        let (flat, flat_latency) = timed(queries, |q| ivf.search_flat(q, 10, nprobe));
        let flat100 = parallel(queries.len(), |i| {
            ivf.search_flat(queries.row(i), 100, nprobe)
        });
        flat.iter().chain(&flat100).for_each(|r| digest.add(r));
        row(
            &mut out,
            "IVF-Flat",
            "—",
            recall_of(&flat, truth, 10),
            Some(recall_of(&flat100, truth, 100)),
            flat_latency,
        );
        for r in IVF_FACTORS {
            let (sq8, latency) = timed(queries, |q| ivf.search_sq8(base, q, 10, nprobe, 10 * r));
            sq8.iter().for_each(|res| digest.add(res));
            let r100 = (r == 4).then(|| {
                let results = parallel(queries.len(), |i| {
                    ivf.search_sq8(base, queries.row(i), 100, nprobe, 100 * r)
                });
                results.iter().for_each(|res| digest.add(res));
                recall_of(&results, truth, 100)
            });
            row(
                &mut out,
                "IVF-SQ8",
                &r.to_string(),
                recall_of(&sq8, truth, 10),
                r100,
                latency,
            );
        }
    }
    let _ = writeln!(
        out,
        "\nResumen de los resultados (FNV-1a de ids y distancias): `{}`.\n",
        digest.hex()
    );
    out
}
