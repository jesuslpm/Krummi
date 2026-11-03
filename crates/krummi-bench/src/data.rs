//! The data of every load (K4, design D5 of the execution document). The .NET program for Voron implements the same
//! functions; the orchestrator compares the digest of what each engine received, so a difference is caught on every
//! run (AC-K7).
//!
//! Every key comes from an **index**: `key(i)` is a bijection of `i`, so distinct indices give distinct keys, and
//! the keys of consecutive indices are in random key order. Each load takes its indices from its own range
//! (`index_base`): the base store uses `0..N`, so the new keys of a load are distinct from the base and from those
//! of any other load. Visiting indices in increasing order visits keys at random.

use crate::workload::{KEY_LEN, Load};

/// The SplitMix64 mixing function, a bijection of `u64` (the same as the timing examples of `krummi`).
pub fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The first index of the new keys of `load` (bits 40 and up hold the load's number). The base store uses
/// `0..N` with `N` at most 1,000,000, far below `2^40`.
pub fn index_base(load: Load) -> u64 {
    load.number() << 40
}

/// The 16-byte key of index `i`: bytes 0..8 are `splitmix(i ^ 0x5EED_0001)` and 8..16 `splitmix(!i)`, both
/// big-endian.
pub fn key(i: u64) -> [u8; KEY_LEN] {
    let mut k = [0u8; KEY_LEN];
    k[..8].copy_from_slice(&splitmix(i ^ 0x5EED_0001).to_be_bytes());
    k[8..].copy_from_slice(&splitmix(!i).to_be_bytes());
    k
}

/// Fills `out` with the value of index `i`: the big-endian bytes of `splitmix((i * 0x1_0000 + j) ^
/// 0xBA5E_0000_0000_0000)` for `j = 0, 1, …`, the last word truncated.
pub fn value_into(i: u64, out: &mut [u8]) {
    for (j, chunk) in out.chunks_mut(8).enumerate() {
        let word =
            splitmix((i.wrapping_mul(0x1_0000).wrapping_add(j as u64)) ^ 0xBA5E_0000_0000_0000);
        chunk.copy_from_slice(&word.to_be_bytes()[..chunk.len()]);
    }
}

/// The value of index `i` with `len` bytes.
pub fn value(i: u64, len: usize) -> Vec<u8> {
    let mut v = vec![0u8; len];
    value_into(i, &mut v);
    v
}

/// The integer key of index `i` of a fixed-size tree (W21, W22): `splitmix(i ^ 0xF1DE_0000)`, a bijection.
pub fn fixed_key(i: u64) -> u64 {
    splitmix(i ^ 0xF1DE_0000)
}

/// The seed of the random order of `thread` in `load`.
pub fn seed(load: Load, thread: u64) -> u64 {
    splitmix((load.number() << 32) | thread)
}

/// The indices `0..n` in the random order of `seed` (Fisher–Yates from the last position down, as the timing
/// examples): for `i` from `n − 1` down to 1, `state = splitmix(state)` and positions `i` and `state % (i + 1)`
/// swap.
pub fn permutation(n: u64, seed: u64) -> Vec<u32> {
    let mut order: Vec<u32> = (0..n as u32).collect();
    let mut state = seed;
    for i in (1..n as usize).rev() {
        state = splitmix(state);
        order.swap(i, (state % (i as u64 + 1)) as usize);
    }
    order
}

/// The order in which `thread` of a reading load visits the `n` base indices: thread 0 in increasing order (keys
/// at random), the others in their own `permutation(n, seed(load, thread))`.
pub fn read_order(load: Load, thread: u64, n: u64) -> Vec<u32> {
    if thread == 0 {
        (0..n as u32).collect()
    } else {
        permutation(n, seed(load, thread))
    }
}

/// The sum of the bytes of `bytes`, the checksum of a read (K8). Wrapping, so order does not matter.
pub fn byte_sum(bytes: &[u8]) -> u64 {
    bytes.iter().map(|&b| b as u64).sum()
}

/// The digest of the data an engine received for a load (AC-K7). Items are fed in the order the load issues them:
/// for every item, each 8-byte chunk (the last padded with zeros) is read little-endian and mixed as
/// `h = splitmix(h ^ word)`, then `h = splitmix(h ^ len)`. Integer keys are fed as their 8 little-endian bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Digest(u64);

impl Default for Digest {
    fn default() -> Self {
        Digest(0x6B72_756D_6D69_0006)
    }
}

impl Digest {
    pub fn feed(&mut self, item: &[u8]) {
        for chunk in item.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.0 = splitmix(self.0 ^ u64::from_le_bytes(word));
        }
        self.0 = splitmix(self.0 ^ item.len() as u64);
    }

    pub fn feed_u64(&mut self, value: u64) {
        self.feed(&value.to_le_bytes());
    }

    pub fn value(self) -> u64 {
        self.0
    }

    /// The 16 lowercase hexadecimal digits of the protocol.
    pub fn hex(self) -> String {
        format!("{:016x}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known answers: the .NET program must produce the same, and the orchestrator compares every load's digest.
    #[test]
    fn splitmix_and_keys_have_their_known_values() {
        assert_eq!(splitmix(0), 0xE220_A839_7B1D_CDAF);
        assert_eq!(splitmix(1), 0x910A_2DEC_8902_5CC1);
        let k = key(0);
        assert_eq!(&k[..8], &splitmix(0x5EED_0001).to_be_bytes());
        assert_eq!(&k[8..], &splitmix(u64::MAX).to_be_bytes());
    }

    #[test]
    fn values_are_the_words_of_their_index() {
        let v = value(3, 100);
        assert_eq!(v.len(), 100);
        assert_eq!(
            &v[..8],
            &splitmix((3 * 0x1_0000) ^ 0xBA5E_0000_0000_0000).to_be_bytes()
        );
        assert_eq!(
            &v[96..],
            &splitmix((3 * 0x1_0000 + 12) ^ 0xBA5E_0000_0000_0000).to_be_bytes()[..4]
        );
        assert_ne!(value(3, 16), value(4, 16));
    }

    #[test]
    fn keys_of_different_ranges_never_collide() {
        let mut all = std::collections::HashSet::new();
        for i in 0..10_000 {
            assert!(all.insert(key(i)));
            assert!(all.insert(key(index_base(Load::W4) + i)));
            assert!(all.insert(key(index_base(Load::W15) + i)));
        }
    }

    #[test]
    fn permutations_are_fixed_by_their_seed_and_cover_every_index() {
        let p = permutation(1_000, seed(Load::W8, 1));
        assert_eq!(p, permutation(1_000, seed(Load::W8, 1)));
        assert_ne!(p, permutation(1_000, seed(Load::W8, 2)));
        let mut sorted = p.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..1_000).collect::<Vec<u32>>());
        assert_eq!(read_order(Load::W7, 0, 5), [0, 1, 2, 3, 4]);
        // Pinned so that a change of the algorithm, which the .NET program would not follow, is noticed here too.
        assert_eq!(&permutation(8, 42)[..], &[0, 1, 4, 7, 3, 6, 2, 5]);
    }

    #[test]
    fn the_digest_depends_on_bytes_lengths_and_order() {
        let digest = |items: &[&[u8]]| {
            let mut d = Digest::default();
            items.iter().for_each(|i| d.feed(i));
            d
        };
        assert_ne!(digest(&[b"ab", b"c"]), digest(&[b"a", b"bc"]));
        assert_ne!(digest(&[b"ab", b"cd"]), digest(&[b"cd", b"ab"]));
        assert_ne!(digest(&[b"a"]), digest(&[b"a\0"]));
        assert_eq!(digest(&[b"x"]).hex().len(), 16);
    }
}
