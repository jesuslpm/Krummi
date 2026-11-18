//! The data of the table loads W23–W32 (E21, *Cargas del banco* of tables-schema.prd.md; IX15, *Cargas del banco* of
//! tables-indexes.prd.md; K4 of the benchmark PRD): the distributions of every column, the key layouts, the random
//! orders, the order of the index `ix_ts`, the checksums of W26, W27, W31 and W32 and the data digests. The digests and the checksum pinned here are the known answers the Voron program's `selftest` prints too
//! (`voron/Program.cs`), so a change on one side only fails here or there.

use std::collections::HashSet;

use krummi_bench::data::{self, Digest, byte_sum, permutation, splitmix};
use krummi_bench::table_data::{
    self as table, KeyKind, NAME_ALPHABET, NAME_LEN, Row, TS_MAX, TS_MIN, ULID_MS0, W23, W24, W25,
    W26, W27, W28, W29, W30, W31, W32,
};
use krummi_bench::workload::Load;

const REDUCED: u64 = 10_000;
const FULL: u64 = 1_000_000;

#[test]
fn columns_follow_their_distributions() {
    let rows = table::rows(REDUCED, KeyKind::Ulid);
    assert_eq!(rows.len(), REDUCED as usize);
    let (mut negative, mut positive, mut flags) = (0, 0, [0u32; 2]);
    let (mut x_low, mut x_high) = (false, false);
    for (r, row) in rows.iter().enumerate() {
        assert_eq!(*row, Row::new(r as u64, KeyKind::Ulid), "row {r}");
        // x in [0, 1), never NaN, infinite or -0.
        assert!(
            row.x.is_finite() && (0.0..1.0).contains(&row.x),
            "x {}",
            row.x
        );
        assert!(!row.x.is_sign_negative(), "x {}", row.x);
        x_low |= row.x < 0.01;
        x_high |= row.x > 0.99;
        // ts between 2000-01-01T00:00:00Z and 2030-01-01T00:00:00Z, in microseconds.
        assert!((TS_MIN..TS_MAX).contains(&row.ts), "ts {}", row.ts);
        // name: 75 ASCII letters and digits.
        assert_eq!(row.name.len(), NAME_LEN);
        assert!(row.name.iter().all(u8::is_ascii_alphanumeric));
        assert_eq!(row.name_str().len(), 75);
        if row.n < 0 {
            negative += 1;
        } else {
            positive += 1;
        }
        flags[usize::from(row.flag)] += 1;
    }
    assert_eq!(TS_MIN, 946_684_800_000_000);
    assert_eq!(TS_MAX, 1_893_456_000_000_000);
    assert!(x_low && x_high);
    // Uniform: each half of the range gets about half of the rows.
    assert!((4_500..5_500).contains(&negative), "{negative} negative");
    assert!((4_500..5_500).contains(&positive));
    assert!((4_500..5_500).contains(&flags[0]) && (4_500..5_500).contains(&flags[1]));
    let mut seen = HashSet::new();
    rows.iter()
        .flat_map(|r| r.name.iter())
        .for_each(|&c| _ = seen.insert(c));
    assert_eq!(seen.len(), 62, "every letter and digit appears");
    assert_eq!(NAME_ALPHABET.len(), 62);
    // The columns do not depend on the key kind.
    let uuid_row = Row::new(7, KeyKind::Uuid);
    assert_eq!(
        (
            uuid_row.n,
            uuid_row.x,
            uuid_row.ts,
            uuid_row.flag,
            uuid_row.name
        ),
        (rows[7].n, rows[7].x, rows[7].ts, rows[7].flag, rows[7].name)
    );
}

#[test]
fn ulid_keys_have_the_e15_layout_and_are_in_order() {
    let mut previous = [0u8; 16];
    for r in 0..REDUCED {
        let id = table::key(r, KeyKind::Ulid);
        assert_eq!(id, Row::new(r, KeyKind::Ulid).id);
        // Bytes 0..6: the millisecond, big-endian; 6..16: 80 bits from the seed.
        let mut ms = [0u8; 8];
        ms[2..].copy_from_slice(&id[..6]);
        assert_eq!(u64::from_be_bytes(ms), ULID_MS0 + r);
        assert!(id > previous, "row {r} is not after the previous one");
        previous = id;
    }
    assert_eq!(ULID_MS0, 1_767_225_600_000, "2026-01-01T00:00:00Z");
    // The random part is not constant.
    assert_ne!(
        table::key(0, KeyKind::Ulid)[6..],
        table::key(1, KeyKind::Ulid)[6..]
    );
}

#[test]
fn uuid_keys_are_random_version_4_and_distinct() {
    let mut all = HashSet::new();
    let mut in_order = 0;
    let mut previous = [0u8; 16];
    for r in 0..FULL {
        let id = table::key(r, KeyKind::Uuid);
        assert_eq!(id[6] >> 4, 4, "version of row {r}");
        assert_eq!(id[8] >> 6, 0b10, "variant of row {r}");
        assert!(all.insert(id), "row {r} repeats a key");
        in_order += u32::from(id > previous);
        previous = id;
    }
    // At random: about half of the consecutive pairs are in key order.
    assert!((450_000..550_000).contains(&in_order), "{in_order}");
}

#[test]
fn the_new_n_of_w28_always_differs_from_the_old_one() {
    for r in 0..REDUCED {
        assert_ne!(table::new_n(r), Row::new(r, KeyKind::Ulid).n, "row {r}");
    }
}

#[test]
fn orders_are_permutations_of_their_own_seed() {
    let loads = [W26, W27, W28, W29, W31, W32];
    let orders: Vec<Vec<u32>> = loads.iter().map(|&l| table::order(l, REDUCED)).collect();
    for (o, &load) in orders.iter().zip(&loads) {
        assert_eq!(
            *o,
            permutation(REDUCED, splitmix(load << 32)),
            "load {load}"
        );
        let mut sorted = o.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..REDUCED as u32).collect::<Vec<_>>());
    }
    for i in 0..orders.len() {
        for j in i + 1..orders.len() {
            assert_ne!(orders[i], orders[j]);
        }
    }
    assert_eq!(table::range_starts(REDUCED, 1_000), orders[1][..1_000]);
    assert_eq!(
        table::index_range_starts(REDUCED, 1_000),
        orders[5][..1_000]
    );
    assert_eq!((W23, W24, W25, W29), (23, 24, 25, 29));
    assert_eq!((W30, W31, W32), (30, 31, 32));
}

#[test]
fn the_seed_of_a_load_number_is_that_of_the_load() {
    for load in Load::ALL {
        for thread in 0..3 {
            assert_eq!(
                data::seed(load, thread),
                data::seed_of(load.number(), thread)
            );
        }
    }
}

#[test]
fn the_checksum_adds_every_column() {
    let row = Row::new(3, KeyKind::Ulid);
    let expected = byte_sum(&row.id)
        .wrapping_add(row.n as u64)
        .wrapping_add(row.x.to_bits())
        .wrapping_add(row.ts as u64)
        .wrapping_add(u64::from(row.flag))
        .wrapping_add(byte_sum(&row.name));
    assert_eq!(row.checksum(), expected);
    assert_eq!(
        table::checksum(&row.id, row.n, row.x, row.ts, row.flag, &row.name),
        expected
    );
    // Known answer, printed by the Voron program's selftest too.
    assert_eq!(
        format!("{:016x}", Row::new(0, KeyKind::Ulid).checksum()),
        CHECKSUM_ROW_0
    );
}

#[test]
fn expected_sums_of_w26_and_w27_are_those_of_the_rows_read() {
    let rows = table::rows(REDUCED, KeyKind::Ulid);
    let all = rows.iter().fold(0u64, |a, r| a.wrapping_add(r.checksum()));
    assert_eq!(table::expected_read_sum(REDUCED), all);

    // W27: from each start, the next rows in key order, which is row order for the ulid keys.
    let (ranges, len) = (1_000, 100);
    let mut count = 0u64;
    let mut sum = 0u64;
    for &s in &table::range_starts(REDUCED, ranges) {
        for row in rows.iter().skip(s as usize).take(len as usize) {
            count += 1;
            sum = sum.wrapping_add(row.checksum());
        }
    }
    assert_eq!(table::expected_ranges(REDUCED, ranges, len), (count, sum));
    // A range that starts near the end reads fewer rows.
    assert!(count < ranges * len);
}

/// IX15: every lookup of W31 finds exactly one row, and no two rows share a `ts`, at both scales, so the order of
/// `ix_ts` is that of `ts` alone and both engines return the same rows in the same order whatever they do with ties.
#[test]
fn w30_rows_repeat_neither_n_nor_ts() {
    let mut ns: Vec<i64> = (0..FULL).map(table::n).collect();
    let mut ts: Vec<i64> = (0..FULL).map(table::ts).collect();
    ns.sort_unstable();
    ts.sort_unstable();
    assert!(ns.windows(2).all(|w| w[0] < w[1]), "a repeated n");
    assert!(ts.windows(2).all(|w| w[0] < w[1]), "a repeated ts");
    // The column functions are those of the rows.
    for r in 0..REDUCED {
        let row = Row::new(r, KeyKind::Ulid);
        assert_eq!((table::n(r), table::ts(r)), (row.n, row.ts), "row {r}");
    }
}

/// The order of `ix_ts`: by `ts`, rows with the same `ts` by key, which is row order (IX8).
#[test]
fn ts_order_sorts_the_rows_by_ts_then_by_key() {
    let order = table::ts_order(REDUCED);
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (0..REDUCED as u32).collect::<Vec<_>>());
    for w in order.windows(2) {
        let (a, b) = (
            Row::new(u64::from(w[0]), KeyKind::Ulid),
            Row::new(u64::from(w[1]), KeyKind::Ulid),
        );
        assert!((a.ts, a.id) < (b.ts, b.id), "rows {} and {}", w[0], w[1]);
    }
}

#[test]
fn expected_sums_of_w32_are_those_of_the_rows_read() {
    let rows = table::rows(REDUCED, KeyKind::Ulid);
    let mut by_ts: Vec<&Row> = rows.iter().collect();
    by_ts.sort_by_key(|r| (r.ts, r.id));
    let (ranges, len) = (1_000, 100);
    let mut count = 0u64;
    let mut sum = 0u64;
    for &s in &table::index_range_starts(REDUCED, ranges) {
        // From the first row whose ts is the start's, included.
        let start = rows[s as usize].ts;
        let from = by_ts.iter().position(|r| r.ts >= start).unwrap();
        assert_eq!(by_ts[from].ts, start);
        for row in by_ts.iter().skip(from).take(len as usize) {
            count += 1;
            sum = sum.wrapping_add(row.checksum());
        }
    }
    assert_eq!(
        table::expected_index_ranges(REDUCED, ranges, len),
        (count, sum)
    );
    // A range that starts near the end of the index reads fewer rows.
    assert!(count < ranges * len);
}

#[test]
fn digests_feed_what_each_load_issues() {
    let feed_rows = |kind| {
        let mut d = Digest::default();
        table::rows(100, kind).iter().for_each(|r| r.feed(&mut d));
        d
    };
    assert_eq!(table::digest(W23, 100, 0, 0), feed_rows(KeyKind::Ulid));
    assert_eq!(table::digest(W25, 100, 0, 0), feed_rows(KeyKind::Ulid));
    assert_eq!(table::digest(W24, 100, 0, 0), feed_rows(KeyKind::Uuid));
    let row = Row::new(1, KeyKind::Ulid);
    let mut d = Digest::default();
    row.feed(&mut d);
    let mut e = Digest::default();
    e.feed(&row.id);
    e.feed_u64(row.n as u64);
    e.feed_u64(row.x.to_bits());
    e.feed_u64(row.ts as u64);
    e.feed(&[u8::from(row.flag)]);
    e.feed(&row.name);
    assert_eq!(d, e);

    let keys = |load| {
        let mut d = Digest::default();
        for &r in &table::order(load, 100) {
            d.feed(&table::key(u64::from(r), KeyKind::Ulid));
            if load == W28 {
                d.feed_u64(table::new_n(u64::from(r)) as u64);
            }
        }
        d
    };
    let mut w26 = keys(W26);
    w26.feed_u64(table::expected_read_sum(100));
    assert_eq!(table::digest(W26, 100, 0, 0), w26);
    let mut w27 = Digest::default();
    for &s in &table::range_starts(100, 10) {
        w27.feed(&table::key(u64::from(s), KeyKind::Ulid));
    }
    let (count, sum) = table::expected_ranges(100, 10, 7);
    w27.feed_u64(count);
    w27.feed_u64(sum);
    assert_eq!(table::digest(W27, 100, 10, 7), w27);
    assert_eq!(table::digest(W28, 100, 0, 0), keys(W28));
    assert_eq!(table::digest(W29, 100, 0, 0), keys(W29));

    // W30 inserts W23's rows; W31 looks up the n of each row of its order, then the checksum of every row; W32 starts
    // at the ts of its start rows, then its expected row count and checksum.
    assert_eq!(table::digest(W30, 100, 0, 0), feed_rows(KeyKind::Ulid));
    let mut w31 = Digest::default();
    for &r in &table::order(W31, 100) {
        w31.feed_u64(Row::new(u64::from(r), KeyKind::Ulid).n as u64);
    }
    w31.feed_u64(table::expected_read_sum(100));
    assert_eq!(table::digest(W31, 100, 0, 0), w31);
    let mut w32 = Digest::default();
    for &s in &table::index_range_starts(100, 10) {
        w32.feed_u64(Row::new(u64::from(s), KeyKind::Ulid).ts as u64);
    }
    let (count, sum) = table::expected_index_ranges(100, 10, 7);
    w32.feed_u64(count);
    w32.feed_u64(sum);
    assert_eq!(table::digest(W32, 100, 10, 7), w32);
}

/// Known answers at the reduced quantities (10,000 rows; W27 and W32 with 1,000 ranges of 100), printed by the Voron
/// program's selftest too: the same values on both sides mean both engines receive the same rows, orders and expected
/// sums.
#[test]
fn reduced_digests_have_their_known_values() {
    let got: Vec<String> = [W23, W24, W25, W26, W27, W28, W29, W30, W31, W32]
        .iter()
        .map(|&l| table::digest(l, REDUCED, 1_000, 100).hex())
        .collect();
    assert_eq!(got, REDUCED_DIGESTS);
}

const CHECKSUM_ROW_0: &str = "cea75ae64ee7f42b";
/// W23 to W32 in order. W23, W25 and W30 receive the same rows.
const REDUCED_DIGESTS: [&str; 10] = [
    "04ee91a577139ff6",
    "bf13a9866b014afd",
    "04ee91a577139ff6",
    "f9ff3b9819b073e3",
    "5a536accc3f3dfb3",
    "793fdb33803d5329",
    "1b518456c7d6073c",
    "04ee91a577139ff6",
    "b3118bdd51c932a8",
    "2d301645b54fc739",
];
