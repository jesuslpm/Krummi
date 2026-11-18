//! The data of the table loads W23–W32 (E21 and *Cargas del banco* of tables-schema.prd.md; IX15 and *Cargas del
//! banco* of tables-indexes.prd.md; K4 of the benchmark PRD).
//! The .NET program for Voron implements the same functions (`voron/Data.cs`, class `TableData`); the orchestrator
//! compares each load's digest with Krummi's, and both sides pin the same known answers (`tests/table_data.rs`, the
//! Voron `selftest`).
//!
//! Every table has the primary key `id` (16 bytes) and the columns `n` (`i64`), `x` (`f64`), `ts` (`timestamp`, in
//! microseconds), `flag` (`bool`) and `name` (`string` of 75 ASCII letters and digits). Row `r` takes its values from
//! the words `word(r, j) = splitmix((r · 0x1_0000 + j) ^ 0x7AB1_E000_0000_0000)`, as `data::value_into` with another
//! salt:
//!
//! | Column | Words | Value |
//! |---|---|---|
//! | `n` | 0 | the word as `i64`: uniform over every `i64` |
//! | `x` | 1 | `(word >> 11) · 2⁻⁵³`: uniform in [0, 1), never NaN, infinite or −0 |
//! | `ts` | 2 | `TS_MIN + hi(word · (TS_MAX − TS_MIN))`: uniform in [2000-01-01T00:00:00Z, 2030-01-01T00:00:00Z) |
//! | `flag` | 3 | bit 0 of the word |
//! | `name` | 4..79 | character `k` is `NAME_ALPHABET[hi(word(r, 4 + k) · 62)]` |
//! | `id`, `ulid` | 79, 80 | E15's `ulid` layout: millisecond `ULID_MS0 + r` big-endian in bytes 0..6, then the low 16 bits of word 79 and word 80, big-endian |
//! | `id`, `uuid` | 81, 82 | words 81 and 82 big-endian, with RFC 9562's version 4 (byte 6 high nibble) and variant `10` (byte 8 high bits) |
//! | new `n` of W28 | 83 | the word as `i64` |
//!
//! `hi(a · b)` is the high 64 bits of the 128-bit product. The `ulid` keys grow with `r`, so the rows of W23 and
//! W25 are in key order; the `uuid` keys of W24 are at random. W26–W29 work on W23's rows: W26 reads, W28 updates
//! and W29 deletes them in `order(load, N)`, and W27 starts its ranges at the rows of the first entries of
//! `order(W27, N)`.
//!
//! W30 inserts W23's rows into a table with two indexes, `ix_n` on `n` and `ix_ts` on `ts`; W31 and W32 work on its
//! rows: W31 looks up the `n` of every row in `order(W31, N)`, and W32 starts its ranges of `ix_ts` at the `ts` of the
//! rows of the first entries of `order(W32, N)`. The order of `ix_ts` is that of `ts`, rows with the same `ts` in the
//! order of their key, which is row order for the `ulid` keys ([`ts_order`]).
//!
//! **Checksum** of a row read (W26, W27, W31, W32), the same in both engines: the wrapping sum of the bytes of `id`,
//! `n` as `u64`, the bits of `x`, `ts` as `u64`, `flag` as 0 or 1 and the bytes of `name`. It is a sum, so the order
//! of the rows does not change it.

use crate::data::{Digest, byte_sum, permutation, seed_of, splitmix};

/// The load numbers (`Load::number`) of the table loads.
pub const W23: u64 = 23;
pub const W24: u64 = 24;
pub const W25: u64 = 25;
pub const W26: u64 = 26;
pub const W27: u64 = 27;
pub const W28: u64 = 28;
pub const W29: u64 = 29;
pub const W30: u64 = 30;
pub const W31: u64 = 31;
pub const W32: u64 = 32;

/// Bytes of `id`.
pub const ID_LEN: usize = 16;
/// Characters of `name`.
pub const NAME_LEN: usize = 75;
/// The characters of `name`, in ASCII order.
pub const NAME_ALPHABET: &[u8; 62] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
/// 2000-01-01T00:00:00Z in microseconds since the Unix epoch, the first `ts`.
pub const TS_MIN: i64 = 946_684_800_000_000;
/// 2030-01-01T00:00:00Z in microseconds, the end of `ts` (excluded).
pub const TS_MAX: i64 = 1_893_456_000_000_000;
/// The millisecond of the `ulid` of row 0: 2026-01-01T00:00:00Z.
pub const ULID_MS0: u64 = 1_767_225_600_000;

const SALT: u64 = 0x7AB1_E000_0000_0000;
const NAME_WORD: u64 = 4;
const ULID_WORD: u64 = 79;
const UUID_WORD: u64 = 81;
const NEW_N_WORD: u64 = 83;

/// The key of a table: `ulid` in order (W23, W25 and the loads on W23's table) or `uuid` at random (W24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    Ulid,
    Uuid,
}

/// One row of a table load.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: [u8; ID_LEN],
    pub n: i64,
    pub x: f64,
    pub ts: i64,
    pub flag: bool,
    pub name: [u8; NAME_LEN],
}

impl Row {
    /// Row `r` with its key of `kind`.
    pub fn new(r: u64, kind: KeyKind) -> Row {
        let mut name = [0u8; NAME_LEN];
        for (k, c) in name.iter_mut().enumerate() {
            *c = NAME_ALPHABET[hi(word(r, NAME_WORD + k as u64), 62) as usize];
        }
        Row {
            id: key(r, kind),
            n: n(r),
            x: (word(r, 1) >> 11) as f64 * (1.0 / (1u64 << 53) as f64),
            ts: ts(r),
            flag: word(r, 3) & 1 == 1,
            name,
        }
    }

    /// `name` as text.
    pub fn name_str(&self) -> &str {
        std::str::from_utf8(&self.name).expect("names are ASCII")
    }

    /// The checksum of this row (see the module).
    pub fn checksum(&self) -> u64 {
        checksum(&self.id, self.n, self.x, self.ts, self.flag, &self.name)
    }

    /// Feeds the row to a data digest: `id`, then `n`, the bits of `x` and `ts` as 8 little-endian bytes each,
    /// `flag` as one byte (0 or 1) and `name`, each as its own item.
    pub fn feed(&self, digest: &mut Digest) {
        digest.feed(&self.id);
        digest.feed_u64(self.n as u64);
        digest.feed_u64(self.x.to_bits());
        digest.feed_u64(self.ts as u64);
        digest.feed(&[u8::from(self.flag)]);
        digest.feed(&self.name);
    }
}

/// The checksum of a row read with these column values (see the module).
pub fn checksum(id: &[u8], n: i64, x: f64, ts: i64, flag: bool, name: &[u8]) -> u64 {
    byte_sum(id)
        .wrapping_add(n as u64)
        .wrapping_add(x.to_bits())
        .wrapping_add(ts as u64)
        .wrapping_add(u64::from(flag))
        .wrapping_add(byte_sum(name))
}

/// The `n` of row `r`.
pub fn n(r: u64) -> i64 {
    word(r, 0) as i64
}

/// The `ts` of row `r`, in microseconds.
pub fn ts(r: u64) -> i64 {
    TS_MIN + hi(word(r, 2), (TS_MAX - TS_MIN) as u64) as i64
}

/// The key of row `r`.
pub fn key(r: u64, kind: KeyKind) -> [u8; ID_LEN] {
    let mut id = [0u8; ID_LEN];
    match kind {
        KeyKind::Ulid => {
            id[..6].copy_from_slice(&(ULID_MS0 + r).to_be_bytes()[2..]);
            id[6..8].copy_from_slice(&(word(r, ULID_WORD) as u16).to_be_bytes());
            id[8..].copy_from_slice(&word(r, ULID_WORD + 1).to_be_bytes());
        }
        KeyKind::Uuid => {
            id[..8].copy_from_slice(&word(r, UUID_WORD).to_be_bytes());
            id[8..].copy_from_slice(&word(r, UUID_WORD + 1).to_be_bytes());
            id[6] = (id[6] & 0x0F) | 0x40;
            id[8] = (id[8] & 0x3F) | 0x80;
        }
    }
    id
}

/// The new `n` that W28 gives row `r`; never the old one, because `splitmix` is a bijection.
pub fn new_n(r: u64) -> i64 {
    word(r, NEW_N_WORD) as i64
}

/// Rows `0..n` with keys of `kind`, in the order W23, W24 and W25 insert them.
pub fn rows(n: u64, kind: KeyKind) -> Vec<Row> {
    (0..n).map(|r| Row::new(r, kind)).collect()
}

/// The rows `0..n` of W23's table in the random order of `load` (W26–W29, W31 or W32): `permutation(n,
/// seed_of(load, 0))`.
pub fn order(load: u64, n: u64) -> Vec<u32> {
    permutation(n, seed_of(load, 0))
}

/// The rows where W27's `ranges` ranges start: the first `ranges` of `order(W27, n)`.
pub fn range_starts(n: u64, ranges: u64) -> Vec<u32> {
    let mut starts = order(W27, n);
    starts.truncate(ranges as usize);
    starts
}

/// The checksum W26 expects: that of every row of W23's table.
pub fn expected_read_sum(n: u64) -> u64 {
    (0..n).fold(0, |sum, r| {
        sum.wrapping_add(Row::new(r, KeyKind::Ulid).checksum())
    })
}

/// The rows W27 reads and their checksum: from each start, up to `range_len` rows forward, fewer at the end of the
/// table.
pub fn expected_ranges(n: u64, ranges: u64, range_len: u64) -> (u64, u64) {
    let mut prefix = Vec::with_capacity(n as usize + 1);
    prefix.push(0u64);
    for r in 0..n {
        let last = prefix[r as usize];
        prefix.push(last.wrapping_add(Row::new(r, KeyKind::Ulid).checksum()));
    }
    let (mut count, mut sum) = (0, 0u64);
    for s in range_starts(n, ranges) {
        let (s, e) = (u64::from(s), (u64::from(s) + range_len).min(n));
        count += e - s;
        sum = sum.wrapping_add(prefix[e as usize].wrapping_sub(prefix[s as usize]));
    }
    (count, sum)
}

/// The rows `0..n` of W30's table in the order of its index `ix_ts`: by `ts`, rows with the same `ts` in the order of
/// their `ulid` key, which is row order.
pub fn ts_order(n: u64) -> Vec<u32> {
    let mut rows: Vec<(i64, u32)> = (0..n as u32).map(|r| (ts(u64::from(r)), r)).collect();
    rows.sort_unstable();
    rows.into_iter().map(|(_, r)| r).collect()
}

/// The rows whose `ts` starts W32's `ranges` ranges: the first `ranges` of `order(W32, n)`.
pub fn index_range_starts(n: u64, ranges: u64) -> Vec<u32> {
    let mut starts = order(W32, n);
    starts.truncate(ranges as usize);
    starts
}

/// The rows W32 reads and their checksum: from the first entry of `ix_ts` with each start's `ts`, up to `range_len`
/// rows forward in the order of [`ts_order`], fewer at the end of the index.
pub fn expected_index_ranges(n: u64, ranges: u64, range_len: u64) -> (u64, u64) {
    let sorted = ts_order(n);
    let sorted_ts: Vec<i64> = sorted.iter().map(|&r| ts(u64::from(r))).collect();
    let mut prefix = Vec::with_capacity(sorted.len() + 1);
    prefix.push(0u64);
    for &r in &sorted {
        let last = *prefix.last().expect("starts with 0");
        prefix.push(last.wrapping_add(Row::new(u64::from(r), KeyKind::Ulid).checksum()));
    }
    let (mut count, mut sum) = (0, 0u64);
    for s in index_range_starts(n, ranges) {
        let start = ts(u64::from(s));
        let from = sorted_ts.partition_point(|&t| t < start);
        let to = (from + range_len as usize).min(sorted.len());
        count += (to - from) as u64;
        sum = sum.wrapping_add(prefix[to].wrapping_sub(prefix[from]));
    }
    (count, sum)
}

/// The digest of the data `load` receives with `n` rows (AC-K7), fed in the order the load issues it: the rows of
/// W23, W24, W25 and W30 (`Row::feed`); the keys W26 reads, then its expected checksum; the start keys of W27's
/// `ranges` ranges of `range_len`, then its expected row count and checksum; each key W28 updates followed by its new
/// `n` (8 little-endian bytes); the keys W29 deletes; the values of `n` W31 looks up (8 little-endian bytes each), then
/// its expected checksum; and the start values of `ts` of W32's ranges (8 little-endian bytes each, in microseconds),
/// then its expected row count and checksum. `ranges` and `range_len` are used only by W27 and W32.
pub fn digest(load: u64, n: u64, ranges: u64, range_len: u64) -> Digest {
    let mut d = Digest::default();
    let ulid = |r: u32| key(u64::from(r), KeyKind::Ulid);
    match load {
        W23 | W25 | W30 => (0..n).for_each(|r| Row::new(r, KeyKind::Ulid).feed(&mut d)),
        W24 => (0..n).for_each(|r| Row::new(r, KeyKind::Uuid).feed(&mut d)),
        W26 => {
            order(W26, n).into_iter().for_each(|r| d.feed(&ulid(r)));
            d.feed_u64(expected_read_sum(n));
        }
        W27 => {
            range_starts(n, ranges)
                .into_iter()
                .for_each(|r| d.feed(&ulid(r)));
            let (count, sum) = expected_ranges(n, ranges, range_len);
            d.feed_u64(count);
            d.feed_u64(sum);
        }
        W28 => {
            for r in order(W28, n) {
                d.feed(&ulid(r));
                d.feed_u64(new_n(u64::from(r)) as u64);
            }
        }
        W29 => order(W29, n).into_iter().for_each(|r| d.feed(&ulid(r))),
        W31 => {
            order(W31, n)
                .into_iter()
                .for_each(|r| d.feed_u64(self::n(u64::from(r)) as u64));
            d.feed_u64(expected_read_sum(n));
        }
        W32 => {
            index_range_starts(n, ranges)
                .into_iter()
                .for_each(|r| d.feed_u64(ts(u64::from(r)) as u64));
            let (count, sum) = expected_index_ranges(n, ranges, range_len);
            d.feed_u64(count);
            d.feed_u64(sum);
        }
        _ => panic!("load {load} is not a table load"),
    }
    d
}

fn word(r: u64, j: u64) -> u64 {
    splitmix(r.wrapping_mul(0x1_0000).wrapping_add(j) ^ SALT)
}

/// The high 64 bits of `a · b`: a value uniform in `0..b` when `a` is uniform over every `u64`.
fn hi(a: u64, b: u64) -> u64 {
    ((u128::from(a) * u128::from(b)) >> 64) as u64
}
