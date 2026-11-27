//! The approximate SQ8 scan with rerank (design document 12, *Benchmarks*, step 1), and the rerank that every
//! approximate search shares: the candidates' rows are read again and their exact distances decide the result.

use crate::data::Vectors;
use crate::kernels::{Metric, norm};
use crate::sq8::{Codes, Prepared};
use crate::topk::{Neighbour, TopK};

/// The `k` nearest of `candidates` by their exact distance to `query` (document 5's kernels), ties by id.
pub fn rerank(
    base: &Vectors,
    query: &[f32],
    metric: Metric,
    candidates: impl IntoIterator<Item = u32>,
    k: usize,
) -> Vec<Neighbour> {
    let query_norm = norm(query);
    let mut top = TopK::new(k);
    for id in candidates {
        let row = id as usize;
        top.push(
            metric.distance(base.row(row), query, base.norm(row), query_norm),
            id,
        );
    }
    top.into_sorted()
}

/// Pushes every `(id, code)` of `entries` into `top` by its SQ8 distance to `prepared`; `top` keeps the best of them
/// up to its capacity, and the caller may keep filling it.
pub fn push_codes<'a>(
    top: &mut TopK,
    prepared: &Prepared,
    entries: impl IntoIterator<Item = (u32, &'a [u8])>,
) {
    for (id, code) in entries {
        top.push(f64::from(prepared.distance(code)), id);
    }
}

/// The approximate SQ8 scan: every code of the base, the best `candidates` of them reranked, `k` kept.
pub fn sq8_scan(
    base: &Vectors,
    codes: &Codes,
    metric: Metric,
    query: &[f32],
    k: usize,
    candidates: usize,
) -> Vec<Neighbour> {
    let prepared = codes.prepare(query);
    let mut top = TopK::new(candidates.max(k));
    push_codes(
        &mut top,
        &prepared,
        (0..base.len()).map(|id| (id as u32, codes.code(id))),
    );
    rerank(
        base,
        query,
        metric,
        top.into_sorted().into_iter().map(|n| n.id),
        k,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topk::exact;

    #[test]
    fn a_scan_that_reranks_every_row_is_the_exact_search() {
        let dim = 24;
        let values: Vec<f32> = (0..dim * 120)
            .map(|i| ((i * 6007 % 499) as f32 - 250.0) / 50.0)
            .collect();
        let base = Vectors::new(dim, values);
        let sample: Vec<usize> = (0..base.len()).step_by(3).collect();
        for metric in [Metric::L2Squared, Metric::Dot, Metric::Cosine] {
            let codes = Codes::build(&base, metric, &sample);
            let query = base.row(11).to_vec();
            let all = sq8_scan(&base, &codes, metric, &query, 10, base.len());
            assert_eq!(all, exact(&base, &query, metric, 10), "{metric:?}");
            let few = sq8_scan(&base, &codes, metric, &query, 10, 40);
            assert_eq!(few.len(), 10);
            assert!(few.windows(2).all(|w| w[0].order(&w[1]).is_lt()));
        }
    }
}
