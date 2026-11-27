//! The distances of vector columns (VI2, VI8 of the vector indexes PRD; document 5 of the table design, *Distance
//! functions*, *Arithmetic in `f64`*, *Determinism*): the exact kernels over the stored bytes of a vector value, and
//! the `f32` work kernels that train, assign and quantize an index.
//!
//! An exact kernel converts every element to `f64` (exactly for `f32` and `f16`) and accumulates in [`LANES`] lanes:
//! lane j takes the elements j, j + 16, j + 32, ...; the lanes are added in a fixed tree (8, 4, 2, 1); then the tail
//! elements are added in order. Rust never reorders nor fuses floating-point operations, so every platform computes
//! the same bits; this is the order of [`vector_norm`]. The `i8` dot product is the exact `i32` sum of the products,
//! times the two scales in `f64`; Hamming counts the differing bits.
//!
//! A distance has two forms: the **key** a search compares (Σ(aᵢ - bᵢ)² for L2, -Σaᵢbᵢ for the dot product,
//! 1 - cos for the cosine and the count of differing bits for Hamming), and the **distance** it reports (VI8): the key
//! itself, except for L2, whose distance is the square root of its key, the Euclidean distance.

use super::value::{VectorElement, VectorType, vector_norm};

/// The lanes of the fixed-order kernels (document 5, *Determinism*).
pub(crate) const LANES: usize = 16;

/// The metric of a vector index or of an exact search (VI2, VI8, VI16 of the vector indexes PRD).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VectorMetric {
    /// The Euclidean distance; smaller is nearer.
    L2,
    /// The dot product negated, so that smaller is nearer.
    Dot,
    /// One minus the cosine; only on a column that declares the cosine metric (N4).
    Cosine,
    /// The number of differing bits; only on a `bit` column, and only in the exact search (VI16).
    Hamming,
}

impl std::fmt::Display for VectorMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            VectorMetric::L2 => "l2",
            VectorMetric::Dot => "dot",
            VectorMetric::Cosine => "cosine",
            VectorMetric::Hamming => "hamming",
        })
    }
}

impl VectorMetric {
    /// Whether a column of type `vector` admits the metric: Hamming on `bit`, cosine on a cosine column, L2 and the dot
    /// product on the other columns (VI2, VI16).
    pub(crate) fn fits(self, vector: &VectorType) -> bool {
        match self {
            VectorMetric::Hamming => vector.element == VectorElement::Bit,
            VectorMetric::Cosine => vector.cosine,
            VectorMetric::L2 | VectorMetric::Dot => {
                !vector.cosine && vector.element != VectorElement::Bit
            }
        }
    }

    /// The distance reported for a compared `key` (VI8): the square root for L2, the key itself otherwise.
    pub(crate) fn distance(self, key: f64) -> f64 {
        match self {
            VectorMetric::L2 => key.sqrt(),
            _ => key,
        }
    }

    /// Its byte in the catalog.
    pub(crate) fn code(self) -> u8 {
        match self {
            VectorMetric::L2 => 0,
            VectorMetric::Dot => 1,
            VectorMetric::Cosine => 2,
            VectorMetric::Hamming => 3,
        }
    }

    /// The metric of a catalog byte, `None` for an unknown one.
    pub(crate) fn of_code(code: u8) -> Option<VectorMetric> {
        Some(match code {
            0 => VectorMetric::L2,
            1 => VectorMetric::Dot,
            2 => VectorMetric::Cosine,
            3 => VectorMetric::Hamming,
            _ => return None,
        })
    }
}

/// A query vector prepared for exact distances to the values of one column: its bytes and, for the cosine, its norm.
pub(crate) struct Exact<'q> {
    vector: VectorType,
    metric: VectorMetric,
    query: &'q [u8],
    norm: f64,
}

impl<'q> Exact<'q> {
    /// Prepares `query`, a normalized value of a column of type `vector` that admits `metric`.
    pub(crate) fn new(vector: VectorType, metric: VectorMetric, query: &'q [u8]) -> Exact<'q> {
        debug_assert!(metric.fits(&vector) && query.len() == vector.value_len());
        let norm = if metric == VectorMetric::Cosine {
            vector_norm(&vector, query)
        } else {
            0.0
        };
        Exact {
            vector,
            metric,
            query,
            norm,
        }
    }

    /// The compared key from the query to `value`, a vector value of the column as a row gives it (without the norm
    /// a cosine column stores, which is the same [`vector_norm`] computes here).
    pub(crate) fn key(&self, value: &[u8]) -> f64 {
        match self.metric {
            VectorMetric::L2 => l2_squared(&self.vector, self.query, value),
            VectorMetric::Dot => -dot(&self.vector, self.query, value),
            VectorMetric::Cosine => {
                let norm = vector_norm(&self.vector, value);
                1.0 - dot(&self.vector, self.query, value) / (self.norm * norm)
            }
            VectorMetric::Hamming => f64::from(hamming(self.query, value)),
        }
    }
}

/// Adds the lanes in the fixed tree: lane j += lane j + 8, then j + 4, j + 2, j + 1. Lane 0 holds the sum.
fn reduce<T: Copy + std::ops::AddAssign>(mut lanes: [T; LANES]) -> T {
    let mut width = LANES / 2;
    while width > 0 {
        for j in 0..width {
            let add = lanes[j + width];
            lanes[j] += add;
        }
        width /= 2;
    }
    lanes[0]
}

/// An IEEE binary16, finite, as a `f32`, exactly: a subnormal is its fraction times 2⁻²⁴, a normal number gets its
/// exponent rebased and its fraction widened.
#[inline]
pub(crate) fn f16_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = u32::from((bits >> 10) & 0x1F);
    let fraction = u32::from(bits & 0x3FF);
    if exponent == 0 {
        let magnitude = fraction as f32 * f32::from_bits(0x3380_0000); // 2⁻²⁴
        f32::from_bits(magnitude.to_bits() | sign)
    } else {
        f32::from_bits(sign | ((exponent + 112) << 23) | (fraction << 13))
    }
}

/// One pass of `term(aᵢ, bᵢ)` over two vectors of `W`-byte elements that `convert` reads, in the fixed order.
macro_rules! fixed_pass {
    ($a:expr, $b:expr, $width:literal, $convert:expr, $term:expr) => {{
        let (a, b): (&[u8], &[u8]) = ($a, $b);
        let convert = $convert;
        let term = $term;
        let (a_chunks, a_tail) = a.as_chunks::<{ $width * LANES }>();
        let (b_chunks, b_tail) = b.as_chunks::<{ $width * LANES }>();
        let mut lanes = [0f64; LANES];
        for (x, y) in a_chunks.iter().zip(b_chunks) {
            for j in 0..LANES {
                let xa = convert(
                    x[$width * j..$width * (j + 1)]
                        .try_into()
                        .expect("an element"),
                );
                let yb = convert(
                    y[$width * j..$width * (j + 1)]
                        .try_into()
                        .expect("an element"),
                );
                lanes[j] += term(xa, yb);
            }
        }
        let tail = a_tail
            .as_chunks::<$width>()
            .0
            .iter()
            .zip(b_tail.as_chunks::<$width>().0);
        tail.fold(reduce(lanes), |sum, (x, y)| {
            sum + term(convert(*x), convert(*y))
        })
    }};
}

fn f32_at(bytes: [u8; 4]) -> f64 {
    f64::from(f32::from_le_bytes(bytes))
}

fn f16_at(bytes: [u8; 2]) -> f64 {
    f64::from(f16_to_f32(u16::from_le_bytes(bytes)))
}

/// The `f32` scale of an `i8` value, after its elements.
fn scale(vector: &VectorType, value: &[u8]) -> f64 {
    let dim = usize::from(vector.dimension);
    f64::from(f32::from_le_bytes(
        value[dim..dim + 4].try_into().expect("a scale"),
    ))
}

/// Σaᵢbᵢ of two values of `vector`.
pub(crate) fn dot(vector: &VectorType, a: &[u8], b: &[u8]) -> f64 {
    match vector.element {
        VectorElement::F32 => fixed_pass!(a, b, 4, f32_at, |x: f64, y: f64| x * y),
        VectorElement::F16 => fixed_pass!(a, b, 2, f16_at, |x: f64, y: f64| x * y),
        VectorElement::I8 => {
            let dim = usize::from(vector.dimension);
            // At most 128² × 4,096 in magnitude: exact in an `i32`, whatever the order.
            let sum: i32 = (a[..dim].iter().zip(&b[..dim]))
                .map(|(&x, &y)| i32::from(x as i8) * i32::from(y as i8))
                .sum();
            f64::from(sum) * (scale(vector, a) * scale(vector, b))
        }
        VectorElement::Bit => {
            let common: u32 = (a.iter().zip(b)).map(|(x, y)| (x & y).count_ones()).sum();
            f64::from(common)
        }
    }
}

/// Σ(aᵢ - bᵢ)² of two values of `vector`; an `i8` element is its byte times its vector's scale, in `f64`.
pub(crate) fn l2_squared(vector: &VectorType, a: &[u8], b: &[u8]) -> f64 {
    let square = |x: f64, y: f64| {
        let d = x - y;
        d * d
    };
    match vector.element {
        VectorElement::F32 => fixed_pass!(a, b, 4, f32_at, square),
        VectorElement::F16 => fixed_pass!(a, b, 2, f16_at, square),
        VectorElement::I8 => {
            let dim = usize::from(vector.dimension);
            let (sa, sb) = (scale(vector, a), scale(vector, b));
            let convert_a = |x: [u8; 1]| f64::from(x[0] as i8) * sa;
            let convert_b = |x: [u8; 1]| f64::from(x[0] as i8) * sb;
            let (a_chunks, a_tail) = a[..dim].as_chunks::<LANES>();
            let (b_chunks, b_tail) = b[..dim].as_chunks::<LANES>();
            let mut lanes = [0f64; LANES];
            for (x, y) in a_chunks.iter().zip(b_chunks) {
                for j in 0..LANES {
                    lanes[j] += square(convert_a([x[j]]), convert_b([y[j]]));
                }
            }
            (a_tail.iter().zip(b_tail)).fold(reduce(lanes), |sum, (x, y)| {
                sum + square(convert_a([*x]), convert_b([*y]))
            })
        }
        VectorElement::Bit => f64::from(hamming(a, b)),
    }
}

/// The number of bits that differ between two `bit` values.
pub(crate) fn hamming(a: &[u8], b: &[u8]) -> u32 {
    let (a_words, a_tail) = a.as_chunks::<8>();
    let (b_words, b_tail) = b.as_chunks::<8>();
    let words: u32 = (a_words.iter().zip(b_words))
        .map(|(x, y)| (u64::from_le_bytes(*x) ^ u64::from_le_bytes(*y)).count_ones())
        .sum();
    words
        + (a_tail.iter().zip(b_tail))
            .map(|(x, y)| (x ^ y).count_ones())
            .sum::<u32>()
}

/// The `f32` work vector of a value of `vector` (not `bit`), as an index trains, assigns and quantizes it: `f32`
/// elements as they are, `f16` ones exactly, `i8` ones times the scale in `f32`; for the cosine, divided by the norm
/// (in `f64`, then rounded), so that the order of 1 - cos is the order of the negated dot product of unit vectors.
pub(crate) fn work_vector(vector: &VectorType, value: &[u8], out: &mut Vec<f32>) {
    out.clear();
    let dim = usize::from(vector.dimension);
    match vector.element {
        VectorElement::F32 => {
            out.extend((value[..dim * 4].as_chunks::<4>().0.iter()).map(|c| f32::from_le_bytes(*c)))
        }
        VectorElement::F16 => out.extend(
            (value[..dim * 2].as_chunks::<2>().0.iter())
                .map(|c| f16_to_f32(u16::from_le_bytes(*c))),
        ),
        VectorElement::I8 => {
            let scale = f32::from_le_bytes(value[dim..dim + 4].try_into().expect("a scale"));
            out.extend(value[..dim].iter().map(|&b| f32::from(b as i8) * scale));
        }
        VectorElement::Bit => unreachable!("no index over a bit column"),
    }
    if vector.cosine {
        let norm = vector_norm(vector, &value[..vector.value_len()]);
        for x in out.iter_mut() {
            *x = (f64::from(*x) / norm) as f32;
        }
    }
}

/// Σ(aᵢ - bᵢ)² of two `f32` work vectors in `f32`, in the fixed order: the assignment's distance to a centroid.
pub(crate) fn work_l2(a: &[f32], b: &[f32]) -> f32 {
    let (a_chunks, a_tail) = a.as_chunks::<LANES>();
    let (b_chunks, b_tail) = b.as_chunks::<LANES>();
    let mut lanes = [0f32; LANES];
    for (x, y) in a_chunks.iter().zip(b_chunks) {
        for j in 0..LANES {
            let d = x[j] - y[j];
            lanes[j] += d * d;
        }
    }
    (a_tail.iter().zip(b_tail)).fold(reduce(lanes), |sum, (x, y)| {
        let d = x - y;
        sum + d * d
    })
}

/// Σ terms in `f32` in the fixed order: 16 lanes, the tree, then the tail in order.
pub(crate) fn work_sum(terms: &[f32]) -> f32 {
    let (chunks, tail) = terms.as_chunks::<LANES>();
    let mut lanes = [0f32; LANES];
    for x in chunks {
        for j in 0..LANES {
            lanes[j] += x[j];
        }
    }
    tail.iter().fold(reduce(lanes), |sum, x| sum + x)
}

/// One pass of `term(codeᵢ, aᵢ, bᵢ)` over an SQ8 code in `f32`, in the fixed order.
#[inline]
pub(crate) fn code_pass(
    code: &[u8],
    a: &[f32],
    b: &[f32],
    term: impl Fn(f32, f32, f32) -> f32,
) -> f32 {
    let (code_chunks, code_tail) = code.as_chunks::<LANES>();
    let (a_chunks, a_tail) = a.as_chunks::<LANES>();
    let (b_chunks, b_tail) = b.as_chunks::<LANES>();
    let mut lanes = [0f32; LANES];
    for ((c, x), y) in code_chunks.iter().zip(a_chunks).zip(b_chunks) {
        for j in 0..LANES {
            lanes[j] += term(f32::from(c[j]), x[j], y[j]);
        }
    }
    (code_tail.iter().zip(a_tail).zip(b_tail)).fold(reduce(lanes), |sum, ((&c, &x), &y)| {
        sum + term(f32::from(c), x, y)
    })
}
