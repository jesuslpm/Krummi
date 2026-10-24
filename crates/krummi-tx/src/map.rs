//! The free-space map page of a region (F2 of the free-space PRD): one bit per page of the region, set when the
//! page is free. Bit `i` is bit `i % 8` of byte `i / 8`, so the payload reads as little-endian 64-bit words.

/// What a region's map says, kept in memory to skip regions that cannot serve a reservation (F9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Summary {
    /// Free pages in the region.
    pub free: u64,
    /// Longest run of free pages in a row.
    pub longest: u64,
}

/// Whether bit `bit` is set (the page is free).
pub(crate) fn is_free(map: &[u8], bit: u64) -> bool {
    map[(bit / 8) as usize] & (1 << (bit % 8)) != 0
}

/// Sets `count` bits from `first` to free or in use.
pub(crate) fn set(map: &mut [u8], first: u64, count: u64, free: bool) {
    for bit in first..first + count {
        let byte = &mut map[(bit / 8) as usize];
        let mask = 1 << (bit % 8);
        if free {
            *byte |= mask;
        } else {
            *byte &= !mask;
        }
    }
}

/// The first bit of the first run of at least `count` free pages, or `None`.
pub(crate) fn find_run(map: &[u8], count: u64) -> Option<u64> {
    let mut found = None;
    for_each_run(map, |start, len| {
        if len >= count {
            found = Some(start);
        }
        found.is_some()
    });
    found
}

/// Free pages and the longest run of them.
pub(crate) fn summary(map: &[u8]) -> Summary {
    let mut summary = Summary {
        free: 0,
        longest: 0,
    };
    for_each_run(map, |_, len| {
        summary.free += len;
        summary.longest = summary.longest.max(len);
        false
    });
    summary
}

/// Calls `visit(start, len)` for each maximal run of set bits, in order, until it returns `true`. Whole words
/// are skipped or counted at once: a run grows by the trailing ones of the remaining bits of a word.
fn for_each_run(map: &[u8], mut visit: impl FnMut(u64, u64) -> bool) {
    let (mut start, mut len) = (0, 0);
    for (index, chunk) in map.as_chunks::<8>().0.iter().enumerate() {
        let word = u64::from_le_bytes(*chunk);
        let base = index as u64 * 64;
        let mut bit = 0;
        while bit < 64 {
            let rest = word >> bit;
            if rest & 1 == 0 {
                if len > 0 {
                    if visit(start, len) {
                        return;
                    }
                    len = 0;
                }
                if rest == 0 {
                    break;
                }
                bit += rest.trailing_zeros();
                continue;
            }
            if len == 0 {
                start = base + u64::from(bit);
            }
            let ones = rest.trailing_ones();
            len += u64::from(ones);
            bit += ones;
        }
    }
    if len > 0 {
        visit(start, len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BYTES: usize = crate::PAYLOAD_SIZE;

    fn map_with(runs: &[(u64, u64)]) -> Vec<u8> {
        let mut map = vec![0; BYTES];
        for &(first, count) in runs {
            set(&mut map, first, count, true);
        }
        map
    }

    #[test]
    fn set_and_read_bits() {
        let mut map = map_with(&[(3, 5)]);
        assert!((3..8).all(|b| is_free(&map, b)));
        assert!(!is_free(&map, 2) && !is_free(&map, 8));
        set(&mut map, 4, 2, false);
        assert_eq!(
            (0..10).filter(|&b| is_free(&map, b)).collect::<Vec<_>>(),
            [3, 6, 7]
        );
    }

    #[test]
    fn runs_inside_and_across_words() {
        let map = map_with(&[(1, 2), (60, 10), (200, 300)]);
        assert_eq!(find_run(&map, 1), Some(1));
        assert_eq!(find_run(&map, 3), Some(60), "a run across a word boundary");
        assert_eq!(find_run(&map, 11), Some(200), "whole words in the middle");
        assert_eq!(find_run(&map, 300), Some(200));
        assert_eq!(find_run(&map, 301), None);
        assert_eq!(
            summary(&map),
            Summary {
                free: 312,
                longest: 300
            }
        );
    }

    #[test]
    fn a_run_up_to_the_last_bit() {
        let last = BYTES as u64 * 8;
        let map = map_with(&[(last - 70, 70)]);
        assert_eq!(find_run(&map, 70), Some(last - 70));
        assert_eq!(summary(&map).longest, 70);
    }

    #[test]
    fn an_empty_and_a_full_map() {
        assert_eq!(find_run(&map_with(&[]), 1), None);
        assert_eq!(summary(&map_with(&[])).free, 0);
        let bits = BYTES as u64 * 8;
        let full = map_with(&[(0, bits)]);
        assert_eq!(find_run(&full, bits), Some(0));
        assert_eq!(
            summary(&full),
            Summary {
                free: bits,
                longest: bits
            }
        );
    }
}
