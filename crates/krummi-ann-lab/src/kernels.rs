//! The exact distance kernels of design document 5 (*Distance functions*, *Arithmetic in `f64`*, *Determinism*), for
//! `f32` vectors held in memory.
//!
//! Every element is converted to `f64` (exactly) and accumulated in [`LANES`] lanes: lane j takes the elements j,
//! j + 16, j + 32, ...; the lanes are added in a fixed tree (8, 4, 2, 1); then the tail elements are added in order.
//! Rust never reorders nor fuses floating-point operations, so every platform computes the same bits. This is the
//! order of the engine's `vector_norm` (`crates/krummi/src/table/value.rs`), which 7.6 will share with these kernels.

/// The lanes of the fixed-order kernels (document 5, *Determinism*).
pub const LANES: usize = 16;

/// A distance of document 5. Smaller is nearer for all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Metric {
    /// Σ(aᵢ - bᵢ)².
    L2Squared,
    /// -Σaᵢbᵢ, negated so that smaller is nearer.
    Dot,
    /// 1 - Σaᵢbᵢ / (‖a‖‖b‖), with the norms computed by [`norm`].
    Cosine,
}

impl Metric {
    /// The name the reports use.
    pub fn name(self) -> &'static str {
        match self {
            Metric::L2Squared => "l2",
            Metric::Dot => "dot",
            Metric::Cosine => "coseno",
        }
    }

    /// The distance from `a` to `b`. The norms are used only by [`Metric::Cosine`]; the others ignore them.
    pub fn distance(self, a: &[f32], b: &[f32], a_norm: f64, b_norm: f64) -> f64 {
        match self {
            Metric::L2Squared => l2_squared(a, b),
            Metric::Dot => -sum_products(a, b),
            Metric::Cosine => 1.0 - sum_products(a, b) / (a_norm * b_norm),
        }
    }
}

/// Adds the lanes in the fixed tree: lane j += lane j + 8, then j + 4, j + 2, j + 1. Lane 0 holds the sum.
fn reduce(mut lanes: [f64; LANES]) -> f64 {
    let mut width = LANES / 2;
    while width > 0 {
        for j in 0..width {
            lanes[j] += lanes[j + width];
        }
        width /= 2;
    }
    lanes[0]
}

/// Σ(aᵢ - bᵢ)² in the fixed order. `a` and `b` have the same length.
pub fn l2_squared(a: &[f32], b: &[f32]) -> f64 {
    assert_eq!(a.len(), b.len(), "vectors of different dimension");
    let (a_chunks, a_tail) = a.as_chunks::<LANES>();
    let (b_chunks, b_tail) = b.as_chunks::<LANES>();
    let mut lanes = [0f64; LANES];
    for (x, y) in a_chunks.iter().zip(b_chunks) {
        for j in 0..LANES {
            let d = f64::from(x[j]) - f64::from(y[j]);
            lanes[j] += d * d;
        }
    }
    a_tail
        .iter()
        .zip(b_tail)
        .fold(reduce(lanes), |sum, (x, y)| {
            let d = f64::from(*x) - f64::from(*y);
            sum + d * d
        })
}

/// Σaᵢbᵢ in the fixed order. Each product of two `f32` values is exact in `f64`.
pub fn sum_products(a: &[f32], b: &[f32]) -> f64 {
    assert_eq!(a.len(), b.len(), "vectors of different dimension");
    let (a_chunks, a_tail) = a.as_chunks::<LANES>();
    let (b_chunks, b_tail) = b.as_chunks::<LANES>();
    let mut lanes = [0f64; LANES];
    for (x, y) in a_chunks.iter().zip(b_chunks) {
        for j in 0..LANES {
            lanes[j] += f64::from(x[j]) * f64::from(y[j]);
        }
    }
    a_tail
        .iter()
        .zip(b_tail)
        .fold(reduce(lanes), |sum, (x, y)| {
            sum + f64::from(*x) * f64::from(*y)
        })
}

/// The L2 norm: the square root of Σaᵢ² in the fixed order, as a cosine column stores it.
pub fn norm(a: &[f32]) -> f64 {
    sum_products(a, a).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same order written as plain scalar loops: lane j over j, j + 16, ...; the tree; the tail.
    fn reference(a: &[f32], b: &[f32], term: impl Fn(f64, f64) -> f64) -> f64 {
        let full = a.len() - a.len() % LANES;
        let mut lanes = [0f64; LANES];
        for i in 0..full {
            lanes[i % LANES] += term(f64::from(a[i]), f64::from(b[i]));
        }
        let mut sum = reduce(lanes);
        for i in full..a.len() {
            sum += term(f64::from(a[i]), f64::from(b[i]));
        }
        sum
    }

    /// Deterministic vectors of `len` elements with a mix of magnitudes, signs, subnormals and values near the
    /// `f32` limits.
    fn vector(len: usize, seed: u32) -> Vec<f32> {
        varied(len, seed, true)
    }

    /// As [`vector`], with ordinary values only when `extremes` is false.
    fn varied(len: usize, seed: u32, extremes: bool) -> Vec<f32> {
        let special = [
            f32::MAX,
            -f32::MAX,
            f32::MIN_POSITIVE,
            1.0e-45,
            -1.0e-40,
            0.0,
            1.0e20,
            -3.5,
        ];
        let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
        (0..len)
            .map(|i| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                if extremes && i % 7 == 3 {
                    special[(state % 8) as usize]
                } else {
                    (state as f32 / u32::MAX as f32 - 0.5) * 8.0
                }
            })
            .collect()
    }

    #[test]
    fn kernels_follow_the_fixed_order_reference() {
        for len in [1, 7, 15, 16, 17, 33, 100, 128, 768, 4096] {
            let a = vector(len, len as u32);
            let b = vector(len, len as u32 + 1000);
            let l2 = reference(&a, &b, |x, y| (x - y) * (x - y));
            let dot = reference(&a, &b, |x, y| x * y);
            assert_eq!(
                l2_squared(&a, &b).to_bits(),
                l2.to_bits(),
                "l2, length {len}"
            );
            assert_eq!(
                sum_products(&a, &b).to_bits(),
                dot.to_bits(),
                "dot, length {len}"
            );
        }
    }

    /// The golden test of document 5: the exact `u64` bits of every distance and norm, so that a platform or a
    /// compiler that computes other bits fails. The bits were taken from win-x64 and must match on linux-x64.
    #[test]
    fn distances_have_the_pinned_bits() {
        let a = varied(37, 1, false);
        let b = varied(37, 2, false);
        let long_a = varied(4096, 9, false);
        let long_b = varied(4096, 10, false);
        let extreme_a = vector(100, 3);
        let extreme_b = vector(100, 4);
        let mut bits = Vec::new();
        for metric in [Metric::L2Squared, Metric::Dot, Metric::Cosine] {
            for (x, y) in [(&a, &b), (&long_a, &long_b), (&extreme_a, &extreme_b)] {
                let d = metric.distance(x, y, norm(x), norm(y));
                assert!(d.is_finite(), "{metric:?}");
                bits.push(d.to_bits());
            }
        }
        bits.push(norm(&a).to_bits());
        bits.push(norm(&long_a).to_bits());
        bits.push(norm(&extreme_a).to_bits());
        assert_eq!(bits, GOLDEN, "pinned bits: {bits:#018x?}");
    }

    /// l2, dot and cosine of the three pairs, then the three norms.
    const GOLDEN: [u64; 12] = [
        0x4073f2cfe3d99aa5,
        0x40e5847359a9c054,
        0x500fffffc0000020,
        0xc024a751cc42b8e0,
        0x4056a29be2491254,
        0x4442f939d0000000,
        0x3fee0d9b29d5727b,
        0x3ff010e6b27436a8,
        0x3ff0000000000000,
        0x402aa7cf1c1aa1b7,
        0x406296f698506074,
        0x47efffffe0000000,
    ];

    /// The cases of R3 in document 5 that `f32` arithmetic gets wrong.
    #[test]
    fn extreme_values_give_finite_distances() {
        let big = [1.0e20f32];
        let tiny = [1.0e-30f32];
        let cosine_big = Metric::Cosine.distance(&big, &big, norm(&big), norm(&big));
        let cosine_tiny = Metric::Cosine.distance(&tiny, &tiny, norm(&tiny), norm(&tiny));
        assert!(cosine_big.abs() < 1e-15, "{cosine_big}");
        assert!(cosine_tiny.abs() < 1e-15, "{cosine_tiny}");
        let dot = Metric::Dot.distance(&[1.0e20, 1.0e20], &[1.0e20, -1.0e20], 0.0, 0.0);
        assert_eq!(dot, 0.0);
        let max = [f32::MAX; 4096];
        assert!(l2_squared(&max, &max.map(|x| -x)).is_finite());
        assert!(norm(&max).is_finite());
    }
}
