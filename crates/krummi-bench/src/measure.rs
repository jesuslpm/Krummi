//! Metrics, medians and thresholds (K7, K8).

/// One repetition of a load: its operations and its time in nanoseconds. Index 0 is the warm-up in a complete run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repetition {
    pub index: u32,
    pub operations: u64,
    pub nanos: u64,
}

impl Repetition {
    /// Operations per second (K8).
    pub fn metric(self) -> f64 {
        self.operations as f64 * 1e9 / self.nanos.max(1) as f64
    }
}

/// The metric of a load: the median of the metrics of its measured repetitions (K8). `warmups` repetitions, the
/// first ones by index, are discarded. Panics if no measured repetition remains.
pub fn load_metric(repetitions: &[Repetition], warmups: u32) -> f64 {
    let mut metrics: Vec<f64> = repetitions
        .iter()
        .filter(|r| r.index >= warmups)
        .map(|r| r.metric())
        .collect();
    assert!(!metrics.is_empty(), "no measured repetition");
    metrics.sort_by(f64::total_cmp);
    let n = metrics.len();
    if n % 2 == 1 {
        metrics[n / 2]
    } else {
        (metrics[n / 2 - 1] + metrics[n / 2]) / 2.0
    }
}

/// Whether Krummi meets the threshold of a load: its metric is at least 0.80 times Voron's, without rounding (K7).
/// Compared as `5 · krummi ≥ 4 · voron`, so `800` against `1000` meets it exactly.
pub fn meets(krummi: f64, voron: f64) -> bool {
    krummi * 5.0 >= voron * 4.0
}

/// The ratio Krummi / Voron as the report shows it: two decimals with a comma, **truncated**, so that a shown
/// `0,80` or more always meets the threshold and a lower one never does.
pub fn ratio_text(krummi: f64, voron: f64) -> String {
    // Without Voron operations there is no ratio to truncate: it is infinite, or 0 when Krummi has none either.
    if voron <= 0.0 || !voron.is_finite() {
        return if krummi > 0.0 {
            "∞".into()
        } else {
            "0,00".into()
        };
    }
    let scaled = krummi * 100.0;
    let mut hundredths = (scaled / voron).floor().max(0.0) as u64;
    // Correct the floating-point division: `hundredths` is the largest whole number with `hundredths · voron ≤
    // 100 · krummi`.
    while (hundredths + 1) as f64 * voron <= scaled {
        hundredths += 1;
    }
    while hundredths > 0 && hundredths as f64 * voron > scaled {
        hundredths -= 1;
    }
    // That comparison and the one of `meets` agree in exact arithmetic but can round differently, so at the
    // threshold the verdict decides: a met threshold never shows less than `0,80`, a missed one never `0,80`.
    if meets(krummi, voron) {
        hundredths = hundredths.max(80);
    } else {
        // A missed threshold is a ratio below 0.80, so only a rounding error can have reached 80.
        hundredths = hundredths.min(79);
    }
    format!("{},{:02}", thousands(hundredths / 100), hundredths % 100)
}

/// `n` with a dot every three digits from the right, also with four digits, whatever the machine's regional settings
/// (annotation of 2026-11-07 to the benchmark PRD's report rules).
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

/// A metric or a size as the report shows it: rounded to the nearest whole number, ties up, then grouped by
/// [`thousands`].
pub fn whole(value: f64) -> String {
    thousands(value.round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(index: u32, operations: u64, nanos: u64) -> Repetition {
        Repetition {
            index,
            operations,
            nanos,
        }
    }

    // AC-K4: the median of the measured repetitions' metrics, the warm-up discarded, also with varying counts.
    #[test]
    fn the_metric_is_the_median_of_the_measured_repetitions() {
        let reps = [
            rep(0, 1_000, 1_000_000_000), // warm-up: 1,000 op/s, discarded
            rep(1, 3_000, 1_000_000_000), // 3,000
            rep(2, 2_000, 2_000_000_000), // 1,000
            rep(3, 5_000, 2_000_000_000), // 2,500
        ];
        assert_eq!(load_metric(&reps, 1), 2_500.0);
        // Without discarding, the warm-up would change it.
        assert_eq!(load_metric(&reps, 0), (1_000.0 + 2_500.0) / 2.0);
        // Reduced run: the only repetition, without warm-up.
        assert_eq!(load_metric(&[rep(0, 42, 1_000_000_000)], 0), 42.0);
    }

    // AC-K3: meets the threshold if and only if Krummi ≥ 0.80 · Voron, without rounding.
    #[test]
    fn the_threshold_is_eighty_percent_of_voron() {
        assert!(meets(800.0, 1_000.0));
        assert!(!meets(799.6, 1_000.0));
        assert!(meets(2_000.0, 1_000.0));
        assert!(!meets(0.0, 1.0));
        assert!(meets(0.8, 1.0));
    }

    // AC-K2: the ratio is truncated, never rounded up past the threshold.
    #[test]
    fn the_ratio_is_truncated_to_two_decimals_with_a_comma() {
        assert_eq!(ratio_text(799.6, 1_000.0), "0,79");
        assert_eq!(ratio_text(800.0, 1_000.0), "0,80");
        assert_eq!(ratio_text(0.8, 1.0), "0,80");
        assert_eq!(ratio_text(1_029.0, 1_000.0), "1,02");
        assert_eq!(ratio_text(29.0, 100.0), "0,29");
        assert_eq!(ratio_text(12_345.0, 1_000.0), "12,34");
        // Voron without operations (W12 with no read finished in a reduced run) must not hang.
        assert_eq!(ratio_text(0.0, 0.0), "0,00");
        assert_eq!(ratio_text(5.0, 0.0), "∞");
        // Below 1,000 every ratio is `0,dd`, so comparing the text compares the shown value.
        for k in 0..1_000 {
            let k = k as f64;
            assert_eq!(meets(k, 1_000.0), k >= 800.0, "{k}");
            assert_eq!(
                ratio_text(k, 1_000.0).as_str() >= "0,80",
                meets(k, 1_000.0),
                "{k}"
            );
        }
    }

    // AC-K2 (annotation of 2026-11-07): the integer part of the truncated ratio carries a dot every three digits.
    #[test]
    fn a_ratio_from_one_thousand_carries_a_thousands_separator() {
        assert_eq!(ratio_text(2_500_009.0, 1_000.0), "2.500,00");
        assert_eq!(ratio_text(999_999.0, 1_000.0), "999,99");
        assert_eq!(ratio_text(79_620.0, 1_000.0), "79,62");
        assert_eq!(ratio_text(1_234_567_890.0, 1.0), "1.234.567.890,00");
    }

    // AC-K2 (annotation of 2026-11-07): a dot every three digits from the right, also with four digits.
    #[test]
    fn whole_numbers_carry_a_dot_every_three_digits() {
        for (n, shown) in [
            (0, "0"),
            (86, "86"),
            (999, "999"),
            (1_000, "1.000"),
            (1_184, "1.184"),
            (26_753_133, "26.753.133"),
            (134_217_728, "134.217.728"),
            (u64::MAX, "18.446.744.073.709.551.615"),
        ] {
            assert_eq!(thousands(n), shown);
        }
    }

    // AC-K2 (annotation of 2026-11-07): a metric is rounded to the nearest whole number, ties up, and then grouped.
    #[test]
    fn a_metric_is_rounded_before_it_is_grouped() {
        assert_eq!(whole(999.6), "1.000");
        assert_eq!(whole(999.4), "999");
        assert_eq!(whole(1_183.5), "1.184");
        assert_eq!(whole(0.0), "0");
    }
}
