//! IVF-Flat and IVF-SQ8 (design document 12, *Benchmarks*, step 2): k-means with a fixed seed over a sample, every
//! vector in the list of its nearest centroid (L2 in the space of its codes, ties to the lowest list), and a search
//! that probes the `nprobe` nearest lists.
//!
//! Everything is deterministic for the same data and parameters on every platform: the distances use document 5's
//! fixed-order kernels, the parallel steps only split independent work, and every sum over many vectors is done in
//! vector order on one thread.

use crate::data::Vectors;
use crate::kernels::{Metric, l2_squared, norm};
use crate::prepare::Rng;
use crate::search::{push_codes, rerank};
use crate::sq8::{Prepared, Quantizer, Space};
use crate::topk::{Neighbour, TopK};

/// The k-means iterations of the training (document 12).
pub const KMEANS_ITERATIONS: usize = 10;

/// The lists of pgvector's rule (document 12, *Training and building*): rows / 1000 up to 1 M rows, √rows above.
pub fn lists_for(rows: usize) -> usize {
    if rows <= 1_000_000 {
        (rows / 1000).max(1)
    } else {
        (rows as f64).sqrt() as usize
    }
}

/// Up to 256 sample points per list, chosen with `seed` (document 12: "a sample of 50 to 256 points per list").
pub fn sample_ids(rows: usize, lists: usize, seed: u64) -> Vec<usize> {
    choose(rows, 256 * lists, seed)
}

/// `count` distinct ids of `0..rows` (all of them if fewer), chosen with `seed`, in id order.
pub fn choose(rows: usize, count: usize, seed: u64) -> Vec<usize> {
    let count = count.min(rows);
    let mut order: Vec<usize> = (0..rows).collect();
    let mut rng = Rng::new(seed);
    for i in 0..count {
        let j = i + rng.below(rows - i);
        order.swap(i, j);
    }
    order.truncate(count);
    order.sort_unstable();
    order
}

fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// The nearest centroid of `v` by L2, ties to the lowest list.
fn nearest(centroids: &Vectors, v: &[f32]) -> u32 {
    let mut best = (f64::INFINITY, 0u32);
    for c in 0..centroids.len() {
        let d = l2_squared(centroids.row(c), v);
        if d < best.0 {
            best = (d, c as u32);
        }
    }
    best.1
}

/// The nearest centroid of every vector of `points`, on every core.
pub fn assign_all(centroids: &Vectors, points: &[Vec<f32>]) -> Vec<u32> {
    let mut out = vec![0u32; points.len()];
    let chunk = points.len().div_ceil(threads()).max(1);
    std::thread::scope(|scope| {
        for (part, slots) in out.chunks_mut(chunk).enumerate() {
            scope.spawn(move || {
                for (i, slot) in slots.iter_mut().enumerate() {
                    *slot = nearest(centroids, &points[part * chunk + i]);
                }
            });
        }
    });
    out
}

/// Lloyd's k-means over `points`: `lists` initial centroids chosen with `seed`, then `iterations` rounds of
/// assigning every point and moving every centroid to the mean of its points (sums in `f64`, in point order). An
/// empty list takes a point chosen with the same generator.
pub fn kmeans(points: &[Vec<f32>], lists: usize, iterations: usize, seed: u64) -> Vectors {
    let dim = points[0].len();
    let mut rng = Rng::new(seed);
    let mut centroids: Vec<f32> = Vec::with_capacity(lists * dim);
    for id in choose(points.len(), lists, seed) {
        centroids.extend_from_slice(&points[id]);
    }
    while centroids.len() < lists * dim {
        centroids.extend_from_slice(&points[rng.below(points.len())]);
    }
    let mut current = Vectors::new(dim, centroids);
    for _ in 0..iterations {
        let assignment = assign_all(&current, points);
        let mut sums = vec![0f64; lists * dim];
        let mut counts = vec![0usize; lists];
        for (point, &list) in points.iter().zip(&assignment) {
            let list = list as usize;
            counts[list] += 1;
            for (s, &x) in sums[list * dim..(list + 1) * dim].iter_mut().zip(point) {
                *s += f64::from(x);
            }
        }
        let mut next = Vec::with_capacity(lists * dim);
        for list in 0..lists {
            if counts[list] == 0 {
                next.extend_from_slice(&points[rng.below(points.len())]);
            } else {
                let n = counts[list] as f64;
                next.extend(
                    sums[list * dim..(list + 1) * dim]
                        .iter()
                        .map(|s| (s / n) as f32),
                );
            }
        }
        current = Vectors::new(dim, next);
    }
    current
}

/// One list: the ids of its vectors in id order, their vectors as given (IVF-Flat) and their SQ8 codes (IVF-SQ8).
#[derive(Default)]
pub struct List {
    pub ids: Vec<u32>,
    pub vectors: Vec<f32>,
    pub norms: Vec<f64>,
    pub codes: Vec<u8>,
}

/// An IVF index over a base, with both kinds of list content.
pub struct Ivf {
    pub metric: Metric,
    pub space: Space,
    pub centroids: Vectors,
    pub quantizer: Quantizer,
    pub lists: Vec<List>,
    pub saturated_codes: usize,
}

impl Ivf {
    /// Trains the centroids and the quantizer on the rows `sample` of `base` and puts every row in its list.
    pub fn build(base: &Vectors, metric: Metric, lists: usize, sample: &[usize], seed: u64) -> Ivf {
        let space = Space::of(metric);
        let transformed: Vec<Vec<f32>> = (0..base.len())
            .map(|id| space.transform(base.row(id), base.norm(id)))
            .collect();
        let points: Vec<Vec<f32>> = sample.iter().map(|&id| transformed[id].clone()).collect();
        let centroids = kmeans(&points, lists, KMEANS_ITERATIONS, seed);
        let quantizer = Quantizer::train(base.dim(), points.iter().map(Vec::as_slice));
        let assignment = assign_all(&centroids, &transformed);
        let mut out: Vec<List> = (0..lists).map(|_| List::default()).collect();
        let mut saturated_codes = 0;
        for (id, &list) in assignment.iter().enumerate() {
            let list = &mut out[list as usize];
            list.ids.push(id as u32);
            list.vectors.extend_from_slice(base.row(id));
            list.norms.push(base.norm(id));
            saturated_codes += usize::from(quantizer.encode(&transformed[id], &mut list.codes) > 0);
        }
        Ivf {
            metric,
            space,
            centroids,
            quantizer,
            lists: out,
            saturated_codes,
        }
    }

    /// The `nprobe` lists whose centroids are nearest to `query` (as stored), nearest first, ties to the lowest list.
    pub fn probe(&self, query: &[f32], nprobe: usize) -> Vec<usize> {
        let q = self.space.transform(query, norm(query));
        let mut top = TopK::new(nprobe);
        for c in 0..self.centroids.len() {
            top.push(l2_squared(self.centroids.row(c), &q), c as u32);
        }
        top.into_sorted().iter().map(|n| n.id as usize).collect()
    }

    /// IVF-Flat: the exact distances of every vector of the probed lists; no rerank is needed.
    pub fn search_flat(&self, query: &[f32], k: usize, nprobe: usize) -> Vec<Neighbour> {
        let dim = self.centroids.dim();
        let query_norm = norm(query);
        let mut top = TopK::new(k);
        for list in self.probe(query, nprobe) {
            let list = &self.lists[list];
            for (i, &id) in list.ids.iter().enumerate() {
                let v = &list.vectors[i * dim..(i + 1) * dim];
                top.push(
                    self.metric.distance(v, query, list.norms[i], query_norm),
                    id,
                );
            }
        }
        top.into_sorted()
    }

    /// IVF-SQ8: the best `candidates` codes of the probed lists, reranked with the rows of `base`.
    pub fn search_sq8(
        &self,
        base: &Vectors,
        query: &[f32],
        k: usize,
        nprobe: usize,
        candidates: usize,
    ) -> Vec<Neighbour> {
        let dim = self.centroids.dim();
        let prepared: Prepared = self
            .quantizer
            .prepare(self.space, &self.space.transform(query, norm(query)));
        let mut top = TopK::new(candidates.max(k));
        for list in self.probe(query, nprobe) {
            let list = &self.lists[list];
            push_codes(
                &mut top,
                &prepared,
                list.ids
                    .iter()
                    .enumerate()
                    .map(|(i, &id)| (id, &list.codes[i * dim..(i + 1) * dim])),
            );
        }
        rerank(
            base,
            query,
            self.metric,
            top.into_sorted().into_iter().map(|n| n.id),
            k,
        )
    }

    /// Bytes of the index: centroids, ids and codes (IVF-SQ8), or centroids, ids and vectors (IVF-Flat).
    pub fn bytes(&self) -> (usize, usize) {
        let centroids = self.centroids.values().len() * 4;
        let ids: usize = self.lists.iter().map(|l| l.ids.len() * 4).sum();
        let codes: usize = self.lists.iter().map(|l| l.codes.len()).sum();
        let vectors: usize = self.lists.iter().map(|l| l.vectors.len() * 4).sum();
        (centroids + ids + codes, centroids + ids + vectors)
    }

    /// The bytes the index keeps allocated once built: the capacities of every buffer (ids, codes, vectors and norms
    /// of each list, centroids and their norms, the quantizer) and the list headers. Unlike [`Ivf::bytes`], which
    /// counts lengths, it includes unused capacity; it leaves out only the allocator's own overhead.
    pub fn allocated_bytes(&self) -> usize {
        let lists: usize = self
            .lists
            .iter()
            .map(|l| {
                l.ids.capacity() * 4
                    + l.vectors.capacity() * 4
                    + l.norms.capacity() * 8
                    + l.codes.capacity()
            })
            .sum();
        lists
            + self.lists.capacity() * std::mem::size_of::<List>()
            + self.centroids.allocated_bytes()
            + self.quantizer.allocated_bytes()
    }

    /// The largest list against the mean (the imbalance of document 12's rebuild criteria).
    pub fn imbalance(&self) -> f64 {
        let sizes: Vec<usize> = self.lists.iter().map(|l| l.ids.len()).collect();
        let mean = sizes.iter().sum::<usize>() as f64 / sizes.len() as f64;
        *sizes.iter().max().unwrap_or(&0) as f64 / mean
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topk::exact;

    fn clustered(dim: usize, rows: usize) -> Vectors {
        let mut rng = Rng::new(5);
        let values = (0..rows * dim)
            .map(|i| {
                let cluster = (i / dim) % 7;
                cluster as f32 * 10.0 + (rng.below(1000) as f32 / 1000.0 - 0.5)
            })
            .collect();
        Vectors::new(dim, values)
    }

    #[test]
    fn probing_every_list_is_the_exact_search() {
        let base = clustered(12, 700);
        let sample = sample_ids(base.len(), 7, 1);
        for metric in [Metric::L2Squared, Metric::Dot, Metric::Cosine] {
            let ivf = Ivf::build(&base, metric, 7, &sample, 3);
            assert_eq!(ivf.lists.iter().map(|l| l.ids.len()).sum::<usize>(), 700);
            let query = base.row(42).to_vec();
            let truth = exact(&base, &query, metric, 10);
            assert_eq!(ivf.search_flat(&query, 10, 7), truth, "{metric:?} flat");
            assert_eq!(
                ivf.search_sq8(&base, &query, 10, 7, base.len()),
                truth,
                "{metric:?} sq8"
            );
        }
    }

    #[test]
    fn kmeans_is_repeatable_and_never_worse_than_its_start() {
        let base = clustered(4, 350);
        let points: Vec<Vec<f32>> = (0..base.len()).map(|i| base.row(i).to_vec()).collect();
        let cost = |centroids: &Vectors| -> f64 {
            let assignment = assign_all(centroids, &points);
            points
                .iter()
                .zip(&assignment)
                .map(|(p, &c)| l2_squared(centroids.row(c as usize), p))
                .sum()
        };
        let trained = kmeans(&points, 7, 10, 9);
        assert_eq!(trained.values(), kmeans(&points, 7, 10, 9).values());
        assert!(cost(&trained) <= cost(&kmeans(&points, 7, 0, 9)));
    }

    #[test]
    fn allocated_bytes_count_capacities_and_norms_beyond_the_payload() {
        let base = clustered(12, 700);
        let ivf = Ivf::build(&base, Metric::L2Squared, 7, &sample_ids(700, 7, 1), 3);
        let (sq8, flat) = ivf.bytes();
        let centroids = ivf.centroids.values().len() * 4;
        // The payload of both indexes together, once the shared centroids and ids, plus the norms of the lists.
        let ids = 700 * 4;
        let payload = sq8 + flat - centroids - ids + 700 * 8;
        assert!(ivf.allocated_bytes() >= payload);
    }

    #[test]
    fn lists_follow_pgvector_rule() {
        assert_eq!(lists_for(10_000), 10);
        assert_eq!(lists_for(1_000_000), 1000);
        assert_eq!(lists_for(500), 1);
        assert_eq!(lists_for(4_000_000), 2000);
    }
}
