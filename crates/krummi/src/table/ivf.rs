//! IVF-SQ8, the arithmetic of a vector index's generation (VI1, VI4 of the vector indexes PRD; document 12 of the table
//! design, *Training without long snapshots*, *SQ8 quantizer and drift*): the number of lists, k-means with a fixed seed
//! on one thread, the SQ8 quantizer, the assignment of a vector to its list and its code, and the distance of a query to
//! a code. Ported from the experiment's `krummi-ann-lab`.
//!
//! Everything works on the `f32` work vectors of `distance::work_vector` with the fixed-order `f32` kernels, so the list
//! and the code of a vector are the same on every platform for the same generation; the training is not reproduced (its
//! result is stored).

use super::distance::{LANES, code_pass, work_l2, work_sum};

/// The k-means iterations of the training (document 12).
pub(crate) const KMEANS_ITERATIONS: usize = 10;
/// The most sample vectors per list (VI4).
pub(crate) const SAMPLE_PER_LIST: usize = 256;
/// The rows with a vector a table needs before an index trains (VI4).
pub(crate) const MIN_TRAINING_ROWS: u64 = 10_000;
/// The fraction of each dimension's sampled range the quantizer adds on each side, so that a sample's extremes leave
/// room for nearby values before they saturate (*Dejado a la implementación*).
const QUANTIZER_MARGIN: f32 = 0.02;

/// The lists of a generation trained on `rows` rows with a vector (VI4): the rows divided by 1,000 and rounded down up
/// to 1 M rows, their square root rounded down above; at least one.
pub(crate) fn lists_for(rows: u64) -> u32 {
    let lists = if rows <= 1_000_000 {
        rows / 1000
    } else {
        rows.isqrt()
    };
    u32::try_from(lists.max(1)).unwrap_or(u32::MAX)
}

/// A small deterministic generator (splitmix64), for the sample and the k-means seeds.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub(crate) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value of `0..n`, `n` at least 1.
    pub(crate) fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// How the codes are compared: L2 on the work vectors, or the dot product (the cosine's work vectors are unit vectors,
/// whose dot product orders as one minus the cosine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code, reason = "used by the approximate search (T7)")]
pub(crate) enum Space {
    L2,
    Dot,
}

/// A trained generation: its lists' centroids and its quantizer (per dimension, the code of `x` is
/// `round((x - lo) / step)` clamped to 0..=255), with the rows it was trained on.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Generation {
    pub(crate) lists: u32,
    pub(crate) dim: usize,
    pub(crate) trained_rows: u64,
    pub(crate) lo: Vec<f32>,
    pub(crate) step: Vec<f32>,
    pub(crate) centroids: Vec<f32>,
}

impl Generation {
    /// Trains `lists` lists and the quantizer on `sample`, work vectors of `dim` elements, with `seed`; `stop` ends it
    /// early, returning `None`, when the store closes (VI12).
    pub(crate) fn train(
        sample: &[Vec<f32>],
        dim: usize,
        lists: u32,
        trained_rows: u64,
        seed: u64,
        stop: &dyn Fn() -> bool,
    ) -> Option<Generation> {
        let centroids = kmeans(sample, dim, lists as usize, seed, stop)?;
        let (lo, step) = quantizer(sample, dim);
        Some(Generation {
            lists,
            dim,
            trained_rows,
            lo,
            step,
            centroids,
        })
    }

    #[allow(dead_code, reason = "used by the approximate search (T7)")]
    pub(crate) fn centroid(&self, list: usize) -> &[f32] {
        &self.centroids[list * self.dim..(list + 1) * self.dim]
    }

    /// The list of `v`: its nearest centroid by L2, ties to the lowest list.
    pub(crate) fn assign(&self, v: &[f32]) -> u32 {
        nearest(&self.centroids, self.dim, v)
    }

    /// The SQ8 code of `v` into `code` and whether a component saturated.
    pub(crate) fn encode(&self, v: &[f32], code: &mut Vec<u8>) -> bool {
        code.clear();
        let mut saturated = false;
        for ((&x, &lo), &step) in v.iter().zip(&self.lo).zip(&self.step) {
            let c = ((x - lo) / step).round();
            saturated |= !(0.0..=255.0).contains(&c);
            code.push(c.clamp(0.0, 255.0) as u8);
        }
        saturated
    }

    /// The lists in order of the distance of their centroids to `query`, nearest first, ties to the lowest list.
    #[allow(dead_code, reason = "used by the approximate search (T7)")]
    pub(crate) fn probe_order(&self, query: &[f32]) -> Vec<u32> {
        let mut order: Vec<(f32, u32)> = (0..self.lists as usize)
            .map(|list| (work_l2(self.centroid(list), query), list as u32))
            .collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        order.into_iter().map(|(_, list)| list).collect()
    }

    /// `query` prepared for distances to codes in `space`.
    #[allow(dead_code, reason = "used by the approximate search (T7)")]
    pub(crate) fn prepare(&self, space: Space, query: &[f32]) -> Prepared {
        match space {
            Space::L2 => Prepared::L2 {
                shifted: query.iter().zip(&self.lo).map(|(q, l)| q - l).collect(),
                step: self.step.clone(),
            },
            Space::Dot => {
                let weight: Vec<f32> = query.iter().zip(&self.step).map(|(q, s)| q * s).collect();
                let products: Vec<f32> = query.iter().zip(&self.lo).map(|(q, l)| q * l).collect();
                Prepared::Dot {
                    weight,
                    constant: work_sum(&products),
                }
            }
        }
    }

    /// The bytes of the generation in its header value: lists, dimension, rows at training, then `lo`, `step` and the
    /// centroids, as little-endian `f32`s.
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let floats = self.lo.len() + self.step.len() + self.centroids.len();
        let mut out = Vec::with_capacity(14 + 4 * floats);
        out.extend(self.lists.to_le_bytes());
        out.extend(
            u16::try_from(self.dim)
                .expect("at most 4,096")
                .to_le_bytes(),
        );
        out.extend(self.trained_rows.to_le_bytes());
        for x in self.lo.iter().chain(&self.step).chain(&self.centroids) {
            out.extend(x.to_le_bytes());
        }
        out
    }

    /// The generation of `bytes`, `None` when they are not what [`to_bytes`](Self::to_bytes) writes for `dim`.
    pub(crate) fn from_bytes(bytes: &[u8], dim: usize) -> Option<Generation> {
        if bytes.len() < 14 {
            return None;
        }
        let lists = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
        let stored_dim = usize::from(u16::from_le_bytes(bytes[4..6].try_into().ok()?));
        let trained_rows = u64::from_le_bytes(bytes[6..14].try_into().ok()?);
        let floats = 2 * dim + (lists as usize).checked_mul(dim)?;
        if stored_dim != dim || lists == 0 || bytes.len() != 14 + 4 * floats {
            return None;
        }
        let mut values = bytes[14..]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c));
        let lo: Vec<f32> = values.by_ref().take(dim).collect();
        let step: Vec<f32> = values.by_ref().take(dim).collect();
        let centroids: Vec<f32> = values.collect();
        let valid = (lo.iter().chain(&centroids)).all(|x| x.is_finite())
            && step.iter().all(|s| s.is_finite() && *s > 0.0);
        valid.then_some(Generation {
            lists,
            dim,
            trained_rows,
            lo,
            step,
            centroids,
        })
    }
}

/// A query prepared for one generation: the distance to a code is then one pass over the code.
#[allow(dead_code, reason = "used by the approximate search (T7)")]
pub(crate) enum Prepared {
    /// For L2: q - lo, and the step of each dimension.
    L2 { shifted: Vec<f32>, step: Vec<f32> },
    /// For the dot product: the weight q·step of each code, and the constant Σq·lo.
    Dot { weight: Vec<f32>, constant: f32 },
}

#[allow(dead_code, reason = "used by the approximate search (T7)")]
impl Prepared {
    /// The approximate distance to `code`; smaller is nearer. It only chooses candidates.
    pub(crate) fn distance(&self, code: &[u8]) -> f32 {
        match self {
            Prepared::L2 { shifted, step } => code_pass(code, shifted, step, |c, q, s| {
                let d = q - c * s;
                d * d
            }),
            Prepared::Dot { weight, constant } => {
                -(constant + code_pass(code, weight, weight, |c, w, _| w * c))
            }
        }
    }
}

/// The nearest of the `dim`-element `centroids` to `v` by L2, ties to the lowest.
fn nearest(centroids: &[f32], dim: usize, v: &[f32]) -> u32 {
    let mut best = (f32::INFINITY, 0u32);
    for (list, centroid) in centroids.chunks_exact(dim).enumerate() {
        let d = work_l2(centroid, v);
        if d < best.0 {
            best = (d, list as u32);
        }
    }
    best.1
}

/// Lloyd's k-means over `points`: `lists` initial centroids chosen with `seed`, then [`KMEANS_ITERATIONS`] rounds of
/// assigning every point and moving every centroid to the mean of its points (sums in `f64`, in point order). An empty
/// list takes a point chosen with the same generator. `None` when `stop` says so between points.
fn kmeans(
    points: &[Vec<f32>],
    dim: usize,
    lists: usize,
    seed: u64,
    stop: &dyn Fn() -> bool,
) -> Option<Vec<f32>> {
    let mut rng = Rng::new(seed);
    let n = points.len() as u64;
    let mut centroids: Vec<f32> = Vec::with_capacity(lists * dim);
    let mut order: Vec<usize> = (0..points.len()).collect();
    for i in 0..lists.min(points.len()) {
        let j = i + rng.below(n - i as u64) as usize;
        order.swap(i, j);
        centroids.extend_from_slice(&points[order[i]]);
    }
    while centroids.len() < lists * dim {
        centroids.extend_from_slice(&points[rng.below(n) as usize]);
    }
    let mut sums = vec![0f64; lists * dim];
    let mut counts = vec![0usize; lists];
    for _ in 0..KMEANS_ITERATIONS {
        sums.fill(0.0);
        counts.fill(0);
        for (at, point) in points.iter().enumerate() {
            if at % 1024 == 0 && stop() {
                return None;
            }
            let list = nearest(&centroids, dim, point) as usize;
            counts[list] += 1;
            for (s, &x) in sums[list * dim..(list + 1) * dim].iter_mut().zip(point) {
                *s += f64::from(x);
            }
        }
        for list in 0..lists {
            let centroid = &mut centroids[list * dim..(list + 1) * dim];
            if counts[list] == 0 {
                centroid.copy_from_slice(&points[rng.below(n) as usize]);
            } else {
                let count = counts[list] as f64;
                for (c, s) in centroid.iter_mut().zip(&sums[list * dim..(list + 1) * dim]) {
                    *c = (s / count) as f32;
                }
            }
        }
    }
    Some(centroids)
}

/// The quantizer of `sample`: per dimension, the range of the sample widened by [`QUANTIZER_MARGIN`] on each side, in
/// 255 steps; a dimension with one value takes a step of 1.
fn quantizer(sample: &[Vec<f32>], dim: usize) -> (Vec<f32>, Vec<f32>) {
    let mut lo = vec![f32::INFINITY; dim];
    let mut hi = vec![f32::NEG_INFINITY; dim];
    for v in sample {
        for (i, &x) in v.iter().enumerate() {
            lo[i] = lo[i].min(x);
            hi[i] = hi[i].max(x);
        }
    }
    let mut step = vec![1.0f32; dim];
    for i in 0..dim {
        let range = hi[i] - lo[i];
        if range > 0.0 && range.is_finite() {
            lo[i] -= range * QUANTIZER_MARGIN;
            step[i] = range * (1.0 + 2.0 * QUANTIZER_MARGIN) / 255.0;
        }
        if !lo[i].is_finite() {
            lo[i] = 0.0;
        }
    }
    (lo, step)
}

const _: () = assert!(LANES == 16);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lists_of_vi4() {
        assert_eq!(lists_for(10_000), 10);
        assert_eq!(lists_for(19_999), 19);
        assert_eq!(lists_for(1_000_000), 1000);
        assert_eq!(lists_for(1_000_001), 1000);
        assert_eq!(lists_for(4_000_000), 2000);
        assert_eq!(lists_for(500), 1);
    }

    fn sample(n: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
        let mut rng = Rng::new(seed);
        (0..n)
            .map(|i| {
                (0..dim)
                    .map(|_| (rng.below(1000) as f32 / 100.0) + (i % 4) as f32 * 20.0)
                    .collect()
            })
            .collect()
    }

    #[test]
    fn training_is_deterministic_and_round_trips() {
        let points = sample(400, 8, 1);
        let a = Generation::train(&points, 8, 4, 400, 7, &|| false).unwrap();
        let b = Generation::train(&points, 8, 4, 400, 7, &|| false).unwrap();
        assert_eq!(a, b);
        assert_eq!(Generation::from_bytes(&a.to_bytes(), 8), Some(a.clone()));
        assert_eq!(Generation::from_bytes(&a.to_bytes(), 9), None);
        assert_eq!(Generation::from_bytes(&a.to_bytes()[..20], 8), None);
        // The four clusters 20 apart get their own lists.
        let lists: std::collections::HashSet<u32> =
            points.iter().take(4).map(|p| a.assign(p)).collect();
        assert_eq!(lists.len(), 4);
        assert!(Generation::train(&points, 8, 4, 400, 7, &|| true).is_none());
    }

    #[test]
    fn codes_saturate_outside_the_range_and_distances_approximate() {
        let points = sample(300, 8, 3);
        let generation = Generation::train(&points, 8, 3, 300, 1, &|| false).unwrap();
        let mut code = Vec::new();
        assert!(!generation.encode(&points[0], &mut code));
        assert!(generation.encode(&[1e6; 8], &mut code));
        assert_eq!(code, [255; 8]);
        let q = &points[5];
        let prepared = generation.prepare(Space::L2, q);
        for p in &points[..50] {
            generation.encode(p, &mut code);
            let exact: f32 = p.iter().zip(q).map(|(a, b)| (a - b) * (a - b)).sum();
            let approx = prepared.distance(&code);
            assert!(
                (approx - exact).abs() <= exact.max(2.0) * 0.05,
                "{approx} {exact}"
            );
        }
        let prepared = generation.prepare(Space::Dot, q);
        generation.encode(&points[7], &mut code);
        let exact: f32 = -points[7].iter().zip(q).map(|(a, b)| a * b).sum::<f32>();
        assert!((prepared.distance(&code) - exact).abs() < exact.abs() * 0.02);
        let order = generation.probe_order(q);
        assert_eq!(order.len(), 3);
        assert_eq!(order[0], generation.assign(q));
    }
}
