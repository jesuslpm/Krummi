//! SQ8 codes (design document 12, *SQ8 quantizer and drift*): one byte per component, from a range per dimension
//! taken from a sample; a component out of the range saturates to code 0 or 255. The distance to a code is computed
//! in `f32` with the fixed order of document 5 (16 lanes, the fixed tree, the tail in order), so the candidates are the
//! same on every platform; it only chooses candidates, and the rerank computes the exact distances.

use crate::data::Vectors;
use crate::kernels::{LANES, Metric, norm};

/// How the vectors are compared through their codes: L2 on the vectors as given, or the dot product, on the vectors
/// as given (`Dot`) or normalized (`Cosine`, where the order of 1 - cos is the order of -dot of unit vectors).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    L2,
    Dot,
    UnitDot,
}

impl Space {
    pub fn of(metric: Metric) -> Space {
        match metric {
            Metric::L2Squared => Space::L2,
            Metric::Dot => Space::Dot,
            Metric::Cosine => Space::UnitDot,
        }
    }

    /// The vector the codes describe: `v` itself, or `v` divided by its norm for `UnitDot`.
    pub fn transform(self, v: &[f32], v_norm: f64) -> Vec<f32> {
        match self {
            Space::UnitDot => v.iter().map(|&x| (f64::from(x) / v_norm) as f32).collect(),
            _ => v.to_vec(),
        }
    }
}

/// The range of every dimension: a component x gets the code round((x - lo) / step), clamped to 0..=255.
pub struct Quantizer {
    lo: Vec<f32>,
    step: Vec<f32>,
}

/// A query prepared for one quantizer: the distance to a code is then one pass over the code.
pub enum Prepared {
    /// For L2: q - lo, and the step of each dimension.
    L2 { shifted: Vec<f32>, step: Vec<f32> },
    /// For the dot products: the weight q·step of each code, and the constant Σq·lo.
    Dot { weight: Vec<f32>, constant: f32 },
}

impl Quantizer {
    /// The minimum and maximum of every dimension over `sample` (already transformed to the space).
    pub fn train<'a>(dim: usize, sample: impl IntoIterator<Item = &'a [f32]>) -> Quantizer {
        let mut lo = vec![f32::INFINITY; dim];
        let mut hi = vec![f32::NEG_INFINITY; dim];
        for v in sample {
            for (i, &x) in v.iter().enumerate() {
                lo[i] = lo[i].min(x);
                hi[i] = hi[i].max(x);
            }
        }
        let step = lo
            .iter()
            .zip(&hi)
            .map(|(&l, &h)| if h > l { (h - l) / 255.0 } else { 1.0 })
            .collect();
        Quantizer { lo, step }
    }

    pub fn dim(&self) -> usize {
        self.lo.len()
    }

    /// Appends the code of `v` to `codes` and returns how many of its components saturated.
    pub fn encode(&self, v: &[f32], codes: &mut Vec<u8>) -> usize {
        let mut saturated = 0;
        for ((&x, &lo), &step) in v.iter().zip(&self.lo).zip(&self.step) {
            let c = ((x - lo) / step).round();
            if !(0.0..=255.0).contains(&c) {
                saturated += 1;
            }
            codes.push(c.clamp(0.0, 255.0) as u8);
        }
        saturated
    }

    /// Prepares `query` (already transformed to the space) for distances in `space`.
    pub fn prepare(&self, space: Space, query: &[f32]) -> Prepared {
        match space {
            Space::L2 => Prepared::L2 {
                shifted: query.iter().zip(&self.lo).map(|(q, l)| q - l).collect(),
                step: self.step.clone(),
            },
            Space::Dot | Space::UnitDot => {
                let weight = query.iter().zip(&self.step).map(|(q, s)| q * s).collect();
                let constant = fixed_sum(query.iter().zip(&self.lo).map(|(q, l)| q * l));
                Prepared::Dot { weight, constant }
            }
        }
    }
}

/// Adds `terms` in the fixed order: 16 lanes, the tree, then the tail in order.
fn fixed_sum(terms: impl ExactSizeIterator<Item = f32>) -> f32 {
    let full = terms.len() - terms.len() % LANES;
    let mut terms = terms;
    let mut lanes = [0f32; LANES];
    for (i, t) in terms.by_ref().take(full).enumerate() {
        lanes[i % LANES] += t;
    }
    let mut width = LANES / 2;
    while width > 0 {
        for j in 0..width {
            lanes[j] += lanes[j + width];
        }
        width /= 2;
    }
    terms.fold(lanes[0], |sum, t| sum + t)
}

/// One pass of `term(code[i], a[i], b[i])` over `code` in the fixed order. The three slices are walked in blocks of
/// [`LANES`] with no index, so the compiler checks no bound inside a block and keeps the lanes in vector registers.
#[inline]
fn pass(code: &[u8], a: &[f32], b: &[f32], term: impl Fn(f32, f32, f32) -> f32) -> f32 {
    let (code_chunks, code_tail) = code.as_chunks::<LANES>();
    let (a_chunks, a_tail) = a.as_chunks::<LANES>();
    let (b_chunks, b_tail) = b.as_chunks::<LANES>();
    let mut lanes = [0f32; LANES];
    for ((c, x), y) in code_chunks.iter().zip(a_chunks).zip(b_chunks) {
        for j in 0..LANES {
            lanes[j] += term(f32::from(c[j]), x[j], y[j]);
        }
    }
    let mut width = LANES / 2;
    while width > 0 {
        for j in 0..width {
            lanes[j] += lanes[j + width];
        }
        width /= 2;
    }
    code_tail
        .iter()
        .zip(a_tail)
        .zip(b_tail)
        .fold(lanes[0], |sum, ((&c, &x), &y)| {
            sum + term(f32::from(c), x, y)
        })
}

impl Prepared {
    /// The approximate distance to `code`; smaller is nearer.
    pub fn distance(&self, code: &[u8]) -> f32 {
        match self {
            Prepared::L2 { shifted, step } => pass(code, shifted, step, |c, q, s| {
                let d = q - c * s;
                d * d
            }),
            Prepared::Dot { weight, constant } => {
                -(constant + pass(code, weight, weight, |c, w, _| w * c))
            }
        }
    }
}

/// The codes of a whole set of vectors.
pub struct Codes {
    pub quantizer: Quantizer,
    pub space: Space,
    pub codes: Vec<u8>,
    /// Codes with at least one saturated component, and saturated components (the counters of document 12).
    pub saturated_codes: usize,
    pub saturated_components: usize,
}

impl Codes {
    /// Trains the quantizer on the rows `sample` of `base` and encodes every row.
    pub fn build(base: &Vectors, metric: Metric, sample: &[usize]) -> Codes {
        let space = Space::of(metric);
        let transformed: Vec<Vec<f32>> = sample
            .iter()
            .map(|&id| space.transform(base.row(id), base.norm(id)))
            .collect();
        let quantizer = Quantizer::train(base.dim(), transformed.iter().map(Vec::as_slice));
        let mut codes = Vec::with_capacity(base.len() * base.dim());
        let (mut saturated_codes, mut saturated_components) = (0, 0);
        for id in 0..base.len() {
            let s = quantizer.encode(&space.transform(base.row(id), base.norm(id)), &mut codes);
            saturated_codes += usize::from(s > 0);
            saturated_components += s;
        }
        Codes {
            quantizer,
            space,
            codes,
            saturated_codes,
            saturated_components,
        }
    }

    pub fn code(&self, id: usize) -> &[u8] {
        let dim = self.quantizer.dim();
        &self.codes[id * dim..(id + 1) * dim]
    }

    /// Prepares a query given as stored (it is transformed here).
    pub fn prepare(&self, query: &[f32]) -> Prepared {
        let q = self.space.transform(query, norm(query));
        self.quantizer.prepare(self.space, &q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_within_half_a_step_and_saturate_outside() {
        let q = Quantizer::train(2, [[0.0f32, -1.0].as_slice(), &[2.55, 1.0]]);
        let mut codes = Vec::new();
        assert_eq!(q.encode(&[0.0, -1.0], &mut codes), 0);
        assert_eq!(q.encode(&[2.55, 1.0], &mut codes), 0);
        assert_eq!(q.encode(&[1.0, 0.0], &mut codes), 0);
        assert_eq!(q.encode(&[-5.0, 9.0], &mut codes), 2);
        // 1 / (2/255) is 127.4999... in f32, so the middle of the second range rounds down.
        assert_eq!(codes, [0, 0, 255, 255, 100, 127, 0, 255]);
    }

    #[test]
    fn code_distances_approximate_the_exact_ones() {
        let dim = 40;
        let values: Vec<f32> = (0..dim * 50)
            .map(|i| ((i * 7919 % 997) as f32 - 498.0) / 100.0)
            .collect();
        let base = Vectors::new(dim, values);
        let sample: Vec<usize> = (0..base.len()).collect();
        for metric in [Metric::L2Squared, Metric::Dot, Metric::Cosine] {
            let codes = Codes::build(&base, metric, &sample);
            assert_eq!(codes.saturated_codes, 0);
            let query = base.row(3);
            let prepared = codes.prepare(query);
            for id in 0..base.len() {
                let exact = metric.distance(base.row(id), query, base.norm(id), norm(query));
                let approx = f64::from(prepared.distance(codes.code(id)));
                let scale = match metric {
                    Metric::L2Squared | Metric::Dot => exact.abs().max(10.0),
                    Metric::Cosine => 1.0,
                };
                let shift = if metric == Metric::Cosine { 1.0 } else { 0.0 };
                assert!(
                    (approx + shift - exact).abs() / scale < 0.02,
                    "{metric:?} {id}: {approx} against {exact}"
                );
            }
        }
    }
}
