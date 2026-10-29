//! Time to store and read values of 1 KiB to 8 MiB and a mixed load of small and large values, and the size of
//! each store (decision V11 of the large values PRD, checked by criterion AC-V13; its data feeds the evaluation of
//! the leaf's threshold, V4).
//!
//! ```text
//! cargo run --release -p krummi --example value_timing [folder]
//! ```
//!
//! Each store lives in a temporary folder inside `folder` (the system's temporary folder by default), on disk, and
//! commits durably. Keys are random 16-byte keys and values random bytes, both with fixed seeds. For each value size
//! of 1, 2, 3, 4, 6 and 8 KiB, 64 KiB, 1 MiB and 8 MiB, a new store gets 256 MiB of values at random in 16 writes of
//! 16 MiB of values (16,384 entries per write with 1 KiB, 2 with 8 MiB; with 3 and 6 KiB, which do not divide
//! 16 MiB, the whole entries that fit: 5,461 and 2,730); its size is taken after the last commit; it is closed and
//! reopened, without dropping the system's cache, and every value is read at random in a single read transaction,
//! comparing all its bytes. Then, in another new store, a mixed load of 1,000,000 keys, 90% with 100-byte values
//! and 10% with values of 2 to 8 KiB (every byte count equally likely; the large entries spread at random), in
//! writes of 1,000 entries: the time to store them, the store's size, and, reopened, the time to read them all at
//! random and to scan the whole tree forward, reading every byte. Besides the store's size, which grows in steps,
//! it prints the pages in use (the store's pages but the free ones), which tell the space more finely. It prints
//! Markdown tables. There is no threshold (D5 of the root PRD).
//!
//! The evaluation of the leaf's threshold (V4) ran this measurement with the half-page threshold the engine keeps
//! and with a full-page prototype since removed; its data is in `odd/tasks/trees-large-values-measurements.md`.

mod timing;

use std::path::Path;
use std::time::{Duration, Instant};

use krummi::{Store, StoreInfo, StoreOptions};
use timing::{BATCH, KEY, TREE, VALUE, header, keys, load, row, shuffled, splitmix};

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;
/// Bytes of values per store of one size, and per write.
const TOTAL: usize = 256 * MIB;
const PER_WRITE: usize = 16 * MIB;
const SIZES: [usize; 9] = [
    KIB,
    2 * KIB,
    3 * KIB,
    4 * KIB,
    6 * KIB,
    8 * KIB,
    64 * KIB,
    MIB,
    8 * MIB,
];
/// The mixed load: its keys, its share of large values and their sizes; the small values have [`VALUE`] bytes and
/// the writes [`BATCH`] entries, as in the 5.1 measurement.
const MIXED: usize = 1_000_000;
const LARGE: usize = MIXED / 10;
const LARGE_VALUES: std::ops::RangeInclusive<usize> = 2 * KIB..=8 * KIB;

fn main() {
    let mut args = std::env::args().skip(1);
    let base = args.next().map_or_else(std::env::temp_dir, Into::into);
    let pool = random_bytes(TOTAL, 0x5EED_0005);

    println!(
        "| Value size | Entries | Writes | Store (s) | Store (MiB/s) | Read at random (s) | µs per read | Store size (bytes) | Pages in use | Pages in use per value |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|");
    for size in SIZES {
        let folder = tempfile::tempdir_in(&base).expect("a temporary folder");
        let per_write = PER_WRITE / size;
        let entries = TOTAL / PER_WRITE * per_write;
        let keys = keys(entries);
        let value = |i: usize| &pool[i * size..(i + 1) * size];
        let (stored, info) = load(folder.path(), &keys, per_write, value);
        let in_use = in_use(&info);
        let order = shuffled(entries, 0x5EED_0006);
        let read = read_at_random(folder.path(), &keys, &order, value);
        println!(
            "| {} | {entries} | {} | {:.2} | {:.1} | {:.2} | {:.2} | {} | {in_use} | {:.3} |",
            label(size),
            entries.div_ceil(per_write),
            stored.as_secs_f64(),
            (entries * size) as f64 / MIB as f64 / stored.as_secs_f64(),
            read.as_secs_f64(),
            read.as_secs_f64() * 1e6 / entries as f64,
            info.size_bytes,
            in_use as f64 / entries as f64,
        );
    }

    // The mixed load: which keys get a large value, and of how many bytes.
    let keys = keys(MIXED);
    let mut sizes = vec![VALUE; MIXED];
    let span = (LARGE_VALUES.end() - LARGE_VALUES.start() + 1) as u64;
    for &i in &shuffled(MIXED, 0x5EED_0007)[..LARGE] {
        sizes[i] = LARGE_VALUES.start() + (splitmix(i as u64 ^ 0x5EED_0008) % span) as usize;
    }
    let value = |i: usize| {
        let start =
            (splitmix(i as u64 ^ 0x5EED_0009) % (TOTAL - LARGE_VALUES.end()) as u64) as usize;
        &pool[start..start + sizes[i]]
    };
    let total: usize = sizes.iter().sum();
    let folder = tempfile::tempdir_in(&base).expect("a temporary folder");
    let (stored, info) = load(folder.path(), &keys, BATCH, value);
    let order = shuffled(MIXED, 0x5EED_000A);
    let read = read_at_random(folder.path(), &keys, &order, value);
    let scanned = scan(folder.path(), MIXED, MIXED * KEY + total);

    println!();
    println!(
        "Mixed load: {MIXED} entries, {LARGE} of 2 to 8 KiB, {total} bytes of values, in writes of {BATCH}."
    );
    println!();
    header();
    row("store at random", MIXED, stored, Some(info.size_bytes));
    row("read at random", MIXED, read, None);
    row("scan forward", MIXED, scanned, None);
    let in_use = in_use(&info);
    println!();
    println!(
        "Pages in use after the load: {in_use} ({:.4} per entry).",
        in_use as f64 / MIXED as f64
    );
}

/// The pages a store uses after a load: its pages but the free ones, as the trees PRD takes them (AC-A15). The
/// store's size grows in steps, so its pages in use tell the space each threshold takes more finely.
fn in_use(info: &StoreInfo) -> u64 {
    info.page_count - info.free_pages
}

/// `len` random bytes from a fixed `seed`.
fn random_bytes(len: usize, seed: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(len);
    let mut state = seed;
    while bytes.len() < len {
        state = splitmix(state);
        bytes.extend_from_slice(&state.to_le_bytes());
    }
    bytes.truncate(len);
    bytes
}

/// The size of a value, for the table.
fn label(size: usize) -> String {
    if size >= MIB {
        format!("{} MiB", size / MIB)
    } else {
        format!("{} KiB", size / KIB)
    }
}

/// Reopens the store in `folder` and reads every key in `order` in a single read transaction, comparing every byte
/// of its value with `value(i)`; returns the time of the reads.
fn read_at_random<'v>(
    folder: &Path,
    keys: &[[u8; KEY]],
    order: &[usize],
    value: impl Fn(usize) -> &'v [u8],
) -> Duration {
    let store = Store::open(folder, &StoreOptions::default()).expect("reopen");
    let read = store.read().expect("read");
    let tree = read.open_tree(TREE).expect("open the tree");
    let start = Instant::now();
    for &i in order {
        let found = read
            .get(&tree, &keys[i])
            .expect("get")
            .expect("a stored key");
        assert!(found == value(i), "the value of key {i}");
    }
    let elapsed = start.elapsed();
    drop(read);
    store.close().expect("close");
    elapsed
}

/// Reopens the store in `folder` and scans its whole tree forward in a single read transaction, reading every byte
/// of each key and value; checks the number of entries and of bytes and returns the time of the scan.
fn scan(folder: &Path, entries: usize, bytes: usize) -> Duration {
    let store = Store::open(folder, &StoreOptions::default()).expect("reopen");
    let read = store.read().expect("read");
    let tree = read.open_tree(TREE).expect("open the tree");
    let start = Instant::now();
    let (mut count, mut seen, mut sum) = (0, 0, 0u64);
    for entry in read.range(&tree, ..).expect("scan") {
        let (key, value) = entry.expect("an entry");
        count += 1;
        seen += key.len() + value.len();
        sum = key
            .iter()
            .chain(value)
            .fold(sum, |s, &b| s.wrapping_add(u64::from(b)));
    }
    let elapsed = start.elapsed();
    std::hint::black_box(sum);
    assert_eq!((count, seen), (entries, bytes));
    drop(read);
    store.close().expect("close");
    elapsed
}
