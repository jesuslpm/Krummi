//! An external in-memory HNSW (design document 12, *Benchmarks*, step 3), in this tool only, to measure the gap with
//! IVF-SQ8. It is the `hnsw_rs` crate with document 5's kernels as its distance, and its candidates are reranked with
//! the exact kernels like every other search here. Its build inserts on every core in an order that depends on the
//! scheduling, so the graph, and thus its results, may differ between runs: its sections carry no digest.

use std::fmt::Write as _;
use std::time::Instant;

use hnsw_rs::prelude::{Distance, Hnsw};

use crate::data::Vectors;
use crate::kernels::{Metric, l2_squared, norm, sum_products};
use crate::measure::{mib, parallel, recall_of, timed};
use crate::search::rerank;
use crate::sq8::Space;
use crate::topk::Neighbour;

/// Links per node above layer 0; `hnsw_rs` keeps twice as many in layer 0 (the M of hnswlib and pgvector, 16).
pub const M: usize = 16;

/// Candidates of each insert (the usual setting of ann-benchmarks for hnswlib).
pub const EF_CONSTRUCTION: usize = 200;

/// Candidates of each search.
pub const EF_SEARCHES: [usize; 7] = [10, 20, 40, 80, 160, 320, 640];

/// The most layers `hnsw_rs` builds.
const LAYERS: usize = 16;

/// Document 5's kernels as the graph's distance, which `hnsw_rs` needs non-negative: L2 as it is; the dot product
/// shifted by `offset`, a bound on every |a·b| of the data; for cosine, 1 - a·b of the normalized vectors.
struct Kernel {
    metric: Metric,
    offset: f64,
}

impl Distance<f32> for Kernel {
    fn eval(&self, a: &[f32], b: &[f32]) -> f32 {
        let d = match self.metric {
            Metric::L2Squared => l2_squared(a, b),
            Metric::Dot => self.offset - sum_products(a, b),
            Metric::Cosine => 1.0 - sum_products(a, b),
        };
        d.max(0.0) as f32
    }
}

/// Builds the graph over `base` and measures it under `metric` over an `ef` sweep; returns its section of the report.
pub fn measure(
    title: &str,
    base: &Vectors,
    queries: &Vectors,
    metric: Metric,
    truth: &[Vec<u32>],
) -> String {
    let space = Space::of(metric);
    let points: Vec<Vec<f32>> = (0..base.len())
        .map(|id| space.transform(base.row(id), base.norm(id)))
        .collect();
    let largest = (0..base.len())
        .map(|id| base.norm(id))
        .chain((0..queries.len()).map(|q| queries.norm(q)))
        .fold(0.0, f64::max);
    let kernel = Kernel {
        metric,
        offset: largest * largest,
    };
    let started = Instant::now();
    let graph = Hnsw::new(M, base.len(), LAYERS, EF_CONSTRUCTION, kernel);
    let items: Vec<(&[f32], usize)> = points
        .iter()
        .enumerate()
        .map(|(id, p)| (p.as_slice(), id))
        .collect();
    graph.parallel_insert_slice(&items);
    let build_time = started.elapsed().as_secs_f64();
    let search = |query: &[f32], k: usize, ef: usize| -> Vec<Neighbour> {
        let q = space.transform(query, norm(query));
        let found = graph.search(&q, k, ef.max(k));
        rerank(base, query, metric, found.iter().map(|n| n.d_id as u32), k)
    };

    let mut out = String::new();
    let _ = writeln!(out, "### {title}\n");
    let _ = writeln!(
        out,
        "Paso 3, HNSW externo (`hnsw_rs`): M {M} (2·M en la capa 0), ef_construction {EF_CONSTRUCTION}, construido \
         en {build_time:.1} s en todos los núcleos; los enlaces de la capa 0 ocupan {:.1} MiB (2·M identificadores \
         de 4 bytes por vector), más los vectores, {:.1} MiB. Sin resumen: la construcción en paralelo no es \
         determinista.\n",
        mib(base.len() * 2 * M * 4),
        mib(base.values().len() * 4)
    );
    let _ = writeln!(
        out,
        "| ef | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|---|");
    for ef in EF_SEARCHES {
        let (found, latency) = timed(queries, |q| search(q, 10, ef));
        let found100 = parallel(queries.len(), |i| search(queries.row(i), 100, ef));
        let _ = writeln!(
            out,
            "| {ef} | {:.4} | {:.4} | {:.2} | {:.2} | {:.2} | {:.0} |",
            recall_of(&found, truth, 10),
            recall_of(&found100, truth, 100),
            latency.p50,
            latency.p95,
            latency.p99,
            1e3 / latency.mean
        );
    }
    let _ = writeln!(out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_distances_are_never_negative_and_keep_the_order() {
        let a = [3.0f32, 4.0];
        let b = [3.0f32, 4.0];
        let c = [-3.0f32, -4.0];
        let dot = Kernel {
            metric: Metric::Dot,
            offset: 25.0,
        };
        assert_eq!(dot.eval(&a, &b), 0.0);
        assert_eq!(dot.eval(&a, &c), 50.0);
        let l2 = Kernel {
            metric: Metric::L2Squared,
            offset: 0.0,
        };
        assert_eq!(l2.eval(&a, &c), 100.0);
        let unit = [0.6f32, 0.8];
        let cosine = Kernel {
            metric: Metric::Cosine,
            offset: 0.0,
        };
        assert!(cosine.eval(&unit, &unit) >= 0.0);
        assert!(cosine.eval(&unit, &[-0.6, -0.8]) > 1.99);
    }
}
