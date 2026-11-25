//! Tests of the distance kernels (VI2, VI8 of the vector indexes PRD; document 5, *Tests that pin it*).

use super::distance::{
    Exact, LANES, VectorMetric, code_pass, dot, f16_to_f32, hamming, l2_squared, work_l2, work_sum,
    work_vector,
};
use super::value::{VectorElement, VectorType, f16_to_f64, vector_norm};

fn f32_type(dimension: u16, cosine: bool) -> VectorType {
    VectorType {
        element: VectorElement::F32,
        dimension,
        cosine,
    }
}

fn bytes_of(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// The fixed order written as plain scalar loops: lane j over j, j + 16, ...; the tree; the tail.
fn reference(a: &[f64], b: &[f64], term: impl Fn(f64, f64) -> f64) -> f64 {
    let full = a.len() - a.len() % LANES;
    let mut lanes = [0f64; LANES];
    for i in 0..full {
        lanes[i % LANES] += term(a[i], b[i]);
    }
    let mut width = LANES / 2;
    while width > 0 {
        for j in 0..width {
            lanes[j] += lanes[j + width];
        }
        width /= 2;
    }
    let mut sum = lanes[0];
    for i in full..a.len() {
        sum += term(a[i], b[i]);
    }
    sum
}

/// The generator of the experiment's kernels (`krummi-ann-lab`): a mix of magnitudes, signs, subnormals and values
/// near the `f32` limits when `extremes` is set.
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
fn f32_kernels_follow_the_fixed_order_reference() {
    for len in [1usize, 7, 15, 16, 17, 33, 100, 128, 255, 768, 4096] {
        let ty = f32_type(len as u16, false);
        let (a, b) = (
            varied(len, len as u32, true),
            varied(len, len as u32 + 1000, true),
        );
        let (wide_a, wide_b): (Vec<f64>, Vec<f64>) = (
            a.iter().map(|&x| f64::from(x)).collect(),
            b.iter().map(|&x| f64::from(x)).collect(),
        );
        let (ba, bb) = (bytes_of(&a), bytes_of(&b));
        let l2 = reference(&wide_a, &wide_b, |x, y| (x - y) * (x - y));
        let products = reference(&wide_a, &wide_b, |x, y| x * y);
        assert_eq!(
            l2_squared(&ty, &ba, &bb).to_bits(),
            l2.to_bits(),
            "l2 {len}"
        );
        assert_eq!(
            dot(&ty, &ba, &bb).to_bits(),
            products.to_bits(),
            "dot {len}"
        );
        // At an offset of the buffer, as a row holds it.
        let mut shifted = vec![0u8; 7];
        shifted.extend(&ba);
        assert_eq!(l2_squared(&ty, &shifted[7..], &bb).to_bits(), l2.to_bits());
    }
}

/// The golden bits of document 5, the same as the experiment's kernels, which were taken on win-x64: the engine's
/// kernels compute those bits on Linux (AC-VI10, with its annotation).
#[test]
fn f32_distances_have_the_pinned_bits_of_the_experiment() {
    let a = varied(37, 1, false);
    let b = varied(37, 2, false);
    let long_a = varied(4096, 9, false);
    let long_b = varied(4096, 10, false);
    let extreme_a = varied(100, 3, true);
    let extreme_b = varied(100, 4, true);
    let pairs = [(&a, &b), (&long_a, &long_b), (&extreme_a, &extreme_b)];
    let mut bits = Vec::new();
    for metric in [VectorMetric::L2, VectorMetric::Dot, VectorMetric::Cosine] {
        for (x, y) in pairs {
            let ty = f32_type(x.len() as u16, metric == VectorMetric::Cosine);
            let (bx, by) = (bytes_of(x), bytes_of(y));
            let key = Exact::new(ty, metric, &bx).key(&by);
            bits.push(key.to_bits());
        }
    }
    for x in [&a, &long_a, &extreme_a] {
        bits.push(vector_norm(&f32_type(x.len() as u16, false), &bytes_of(x)).to_bits());
    }
    // l2, dot and cosine of the three pairs, then the three norms (`krummi-ann-lab/src/kernels.rs`).
    let golden: [u64; 12] = [
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
    assert_eq!(bits, golden, "pinned bits: {bits:#018x?}");
}

#[test]
fn f16_converts_exactly() {
    for bits in 0..=u16::MAX {
        if bits & 0x7C00 == 0x7C00 {
            continue;
        }
        let exact = f16_to_f64(bits);
        let converted = f64::from(f16_to_f32(bits));
        assert_eq!(converted.to_bits(), exact.to_bits(), "{bits:#06x}");
    }
}

#[test]
fn f16_and_i8_kernels_follow_the_reference() {
    for len in [1usize, 15, 16, 17, 100, 768] {
        let (ha, hb) = (halves(len, 5), halves(len, 6));
        let ty = VectorType {
            element: VectorElement::F16,
            dimension: len as u16,
            cosine: false,
        };
        let bytes = |v: &[u16]| -> Vec<u8> { v.iter().flat_map(|x| x.to_le_bytes()).collect() };
        let wide = |v: &[u16]| -> Vec<f64> { v.iter().map(|&x| f16_to_f64(x)).collect() };
        let (wa, wb) = (wide(&ha), wide(&hb));
        assert_eq!(
            l2_squared(&ty, &bytes(&ha), &bytes(&hb)).to_bits(),
            reference(&wa, &wb, |x, y| (x - y) * (x - y)).to_bits()
        );
        assert_eq!(
            dot(&ty, &bytes(&ha), &bytes(&hb)).to_bits(),
            reference(&wa, &wb, |x, y| x * y).to_bits()
        );

        let ty = VectorType {
            element: VectorElement::I8,
            dimension: len as u16,
            cosine: false,
        };
        let int = |seed: u32| -> Vec<i8> {
            varied(len, seed, false)
                .iter()
                .map(|x| (x * 31.0) as i8)
                .collect()
        };
        let (ia, ib) = (int(7), int(8));
        let (sa, sb) = (0.25f32, 3.0e-3f32);
        let value = |v: &[i8], s: f32| -> Vec<u8> {
            let mut out: Vec<u8> = v.iter().map(|&x| x as u8).collect();
            out.extend(s.to_le_bytes());
            out
        };
        let exact: i64 = ia
            .iter()
            .zip(&ib)
            .map(|(&x, &y)| i64::from(x) * i64::from(y))
            .sum();
        assert_eq!(
            dot(&ty, &value(&ia, sa), &value(&ib, sb)).to_bits(),
            (exact as f64 * (f64::from(sa) * f64::from(sb))).to_bits()
        );
        let (da, db): (Vec<f64>, Vec<f64>) = (
            ia.iter().map(|&x| f64::from(x) * f64::from(sa)).collect(),
            ib.iter().map(|&x| f64::from(x) * f64::from(sb)).collect(),
        );
        assert_eq!(
            l2_squared(&ty, &value(&ia, sa), &value(&ib, sb)).to_bits(),
            reference(&da, &db, |x, y| (x - y) * (x - y)).to_bits()
        );
    }
}

/// Finite halves of every class (subnormal, normal, both signs), from the bits of [`varied`]'s generator.
fn halves(len: usize, seed: u32) -> Vec<u16> {
    varied(len, seed, false)
        .iter()
        .map(|x| {
            let bits = (x.to_bits() >> 7) as u16;
            if bits & 0x7C00 == 0x7C00 {
                bits & !0x4000
            } else {
                bits
            }
        })
        .collect()
}

#[test]
fn the_i8_dot_product_is_exact_at_the_extremes() {
    let len = 4096usize;
    let ty = VectorType {
        element: VectorElement::I8,
        dimension: len as u16,
        cosine: false,
    };
    let mut value = vec![0x80u8; len];
    value.extend(1.0f32.to_le_bytes());
    assert_eq!(dot(&ty, &value, &value), 128.0 * 128.0 * 4096.0);
}

#[test]
fn hamming_counts_the_differing_bits() {
    let a: Vec<u8> = (0..37).map(|i| (i * 37) as u8).collect();
    let b: Vec<u8> = (0..37).map(|i| (i * 11 + 3) as u8).collect();
    let expected: u32 = a.iter().zip(&b).map(|(x, y)| (x ^ y).count_ones()).sum();
    assert_eq!(hamming(&a, &b), expected);
    let ty = VectorType {
        element: VectorElement::Bit,
        dimension: 296,
        cosine: false,
    };
    let key = Exact::new(ty, VectorMetric::Hamming, &a).key(&b);
    assert_eq!(key, f64::from(expected));
    assert_eq!(VectorMetric::Hamming.distance(key), key);
}

/// VI8: L2 reports the Euclidean distance, the dot product its negation, the cosine one minus the cosine.
#[test]
fn the_units_of_the_distances() {
    let ty = f32_type(2, false);
    let (a, b) = (bytes_of(&[3.0, 0.0]), bytes_of(&[0.0, 4.0]));
    let l2 = Exact::new(ty, VectorMetric::L2, &a);
    assert_eq!(l2.key(&b), 25.0);
    assert_eq!(VectorMetric::L2.distance(l2.key(&b)), 5.0);
    let (c, d) = (bytes_of(&[1.0, 2.0]), bytes_of(&[3.0, -4.0]));
    assert_eq!(Exact::new(ty, VectorMetric::Dot, &c).key(&d), 5.0);
    let ty = f32_type(2, true);
    let (e, f) = (bytes_of(&[1.0, 0.0]), bytes_of(&[1.0, 1.0]));
    let cosine = Exact::new(ty, VectorMetric::Cosine, &e).key(&f);
    assert!((cosine - (1.0 - std::f64::consts::FRAC_1_SQRT_2)).abs() < 1e-15);
}

#[test]
fn metrics_fit_their_columns() {
    let plain = f32_type(4, false);
    let cosine = f32_type(4, true);
    let bit = VectorType {
        element: VectorElement::Bit,
        dimension: 8,
        cosine: false,
    };
    use VectorMetric::*;
    assert!(L2.fits(&plain) && Dot.fits(&plain) && !Cosine.fits(&plain) && !Hamming.fits(&plain));
    assert!(!L2.fits(&cosine) && !Dot.fits(&cosine) && Cosine.fits(&cosine));
    assert!(Hamming.fits(&bit) && !L2.fits(&bit) && !Dot.fits(&bit) && !Cosine.fits(&bit));
    for metric in [L2, Dot, Cosine, Hamming] {
        assert_eq!(VectorMetric::of_code(metric.code()), Some(metric));
    }
    assert_eq!(VectorMetric::of_code(9), None);
}

#[test]
fn work_vectors_and_kernels() {
    let ty = VectorType {
        element: VectorElement::I8,
        dimension: 3,
        cosine: false,
    };
    let mut value = vec![2u8, 0xFE, 0];
    value.extend(0.5f32.to_le_bytes());
    let mut out = Vec::new();
    work_vector(&ty, &value, &mut out);
    assert_eq!(out, [1.0, -1.0, 0.0]);
    let ty = f32_type(2, true);
    work_vector(&ty, &bytes_of(&[3.0, 4.0]), &mut out);
    assert_eq!(out, [0.6, 0.8]);
    let a: Vec<f32> = (0..35).map(|i| i as f32 * 0.5).collect();
    let b: Vec<f32> = (0..35).map(|i| 3.0 - i as f32).collect();
    let wide = |v: &[f32]| -> Vec<f64> { v.iter().map(|&x| f64::from(x)).collect() };
    let expected = reference(&wide(&a), &wide(&b), |x, y| (x - y) * (x - y));
    assert_eq!(f64::from(work_l2(&a, &b)), expected);
    assert_eq!(work_sum(&a), a.iter().sum::<f32>());
    let code: Vec<u8> = (0..35).map(|i| i as u8).collect();
    let pass = code_pass(&code, &a, &b, |c, x, y| c * x + y);
    let check: f32 = (0..35).map(|i| i as f32 * a[i] + b[i]).sum();
    assert!((pass - check).abs() < 1e-2);
}
