//! Time and space of fixed-size trees against normal trees with the same keys (decision X13 of the fixed-size trees
//! PRD, checked by criterion AC-X21).
//!
//! ```text
//! cargo run --release -p krummi --example fixed_timing [options]
//! ```
//!
//! Options:
//!
//! - `--folder <path>`: where the stores live, each in its own temporary folder (the system's temporary folder by
//!   default).
//! - `--entries <count>`: entries per load (1,000,000 by default, as X13 sets).
//! - `--lengths <n,...>`: the value lengths in bytes (`0,8,100` by default, as X13 sets).
//! - `--quick`: a smoke test, the same as `--entries 10000`.
//!
//! The keys are distinct random integers from `0` to `2^64−1`, and each value's bytes are random; both come from fixed
//! seeds, so every load stores the same entries. The fixed-size tree stores the integer keys; the normal tree stores
//! each key as its 8 big-endian bytes, which sort in the same numeric order, and converting a key to its bytes when
//! storing it and back to an integer when reading it is part of the measured time. Every store is on disk and
//! commits durably. For each value length and each kind of tree it measures (X13):
//!
//! - storing the entries in key order in one new store, and at random in another, both in writes of 1,000 entries,
//!   and the store's size (P12) and pages in use (its pages but the free ones) after each load;
//! - in the store loaded at random, closed and reopened without dropping the system's cache: reading every entry at
//!   random in a single read transaction, then scanning them all forward in another, reading the key and every byte
//!   of each value returned in both; and removing them all at random, in writes of 1,000 entries.
//!
//! It prints a header with the operating system and the architecture, and a Markdown table per value length with a
//! row per kind of tree; times are in microseconds per entry. The processor and the disk go in the report by hand.
//! The measurement is run by hand: neither the tests nor the CI run it. No time is a pass mark (D5 of the root PRD).

// The measurement shares the tree measurements' helpers, of which it uses only some.
#[allow(dead_code)]
mod timing;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use krummi::{OpenMode, Store, StoreInfo, StoreOptions};
use timing::{BATCH, TREE, shuffled, splitmix};

/// Entries per load, as X13 sets them.
const ENTRIES: usize = 1_000_000;
/// Entries per load with `--quick`.
const QUICK: usize = 10_000;
/// The value lengths of X13.
const LENGTHS: [usize; 3] = [0, 8, 100];

/// A kind of tree measured.
#[derive(Clone, Copy)]
enum Kind {
    /// A fixed-size tree with integer keys.
    Fixed,
    /// A normal tree with the keys as 8 big-endian bytes.
    Normal,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Fixed => "fixed-size",
            Kind::Normal => "normal, 8-byte big-endian keys",
        }
    }
}

/// The entries of every load: distinct random keys and the values, `len` random bytes each, in one buffer.
struct Data {
    keys: Vec<u64>,
    len: usize,
    values: Vec<u8>,
}

impl Data {
    fn new(entries: usize, len: usize) -> Self {
        // SplitMix64 is a bijection, so distinct indexes give distinct keys.
        let keys = (0..entries as u64)
            .map(|i| splitmix(i ^ 0xF1ED_0001))
            .collect();
        let values = (0..(entries * len) as u64)
            .map(|i| splitmix(i ^ 0x7A1E_0002) as u8)
            .collect();
        Data { keys, len, values }
    }

    fn value(&self, index: usize) -> &[u8] {
        &self.values[index * self.len..(index + 1) * self.len]
    }
}

/// What one kind of tree measured with one value length.
struct Measured {
    in_order: Duration,
    in_order_info: StoreInfo,
    at_random: Duration,
    at_random_info: StoreInfo,
    read: Duration,
    scan: Duration,
    remove: Duration,
}

/// Stores the entries `order` in a new store in `folder`, in writes of [`BATCH`] entries, and returns the time and
/// the store's information after the last commit; the store stays open for the caller.
fn load(folder: &Path, kind: Kind, data: &Data, order: &[usize]) -> (Store, Duration, StoreInfo) {
    let store = Store::open(folder, &StoreOptions::default()).expect("open");
    let start = Instant::now();
    for chunk in order.chunks(BATCH) {
        let mut write = store.write().expect("write");
        match kind {
            Kind::Fixed => {
                let tree = write
                    .open_fixed_tree_with(TREE, data.len, OpenMode::OpenOrCreate)
                    .expect("open or create the tree");
                for &i in chunk {
                    write
                        .set_fixed(&tree, data.keys[i], data.value(i))
                        .expect("set");
                }
            }
            Kind::Normal => {
                let tree = write
                    .open_tree_with(TREE, OpenMode::OpenOrCreate)
                    .expect("open or create the tree");
                for &i in chunk {
                    write
                        .set(&tree, &data.keys[i].to_be_bytes(), data.value(i))
                        .expect("set");
                }
            }
        }
        write.commit().expect("commit");
    }
    let elapsed = start.elapsed();
    let info = store.info();
    (store, elapsed, info)
}

/// A checksum of a key and a value, so that reading them is not optimised away.
fn fold(sum: u64, key: u64, value: &[u8]) -> u64 {
    value
        .iter()
        .fold(sum ^ key, |s, &b| s.rotate_left(5) ^ u64::from(b))
}

/// Reads every entry of `order` in one read transaction and returns the time and a checksum.
fn read_all(store: &Store, kind: Kind, data: &Data, order: &[usize]) -> (Duration, u64) {
    let read = store.read().expect("read");
    let start = Instant::now();
    let mut sum = 0;
    match kind {
        Kind::Fixed => {
            let tree = read.open_fixed_tree(TREE, data.len).expect("open");
            for &i in order {
                let value = read
                    .get_fixed(&tree, data.keys[i])
                    .expect("get")
                    .expect("there");
                sum = fold(sum, data.keys[i], value);
            }
        }
        Kind::Normal => {
            let tree = read.open_tree(TREE).expect("open");
            for &i in order {
                let value = read
                    .get(&tree, &data.keys[i].to_be_bytes())
                    .expect("get")
                    .expect("there");
                sum = fold(sum, data.keys[i], value);
            }
        }
    }
    (start.elapsed(), sum)
}

/// Scans every entry forward in one read transaction, converting each normal key back to its integer, and returns
/// the time, the number of entries and a checksum.
fn scan_all(store: &Store, kind: Kind, data: &Data) -> (Duration, usize, u64) {
    let read = store.read().expect("read");
    let start = Instant::now();
    let (mut count, mut sum) = (0, 0);
    match kind {
        Kind::Fixed => {
            let tree = read.open_fixed_tree(TREE, data.len).expect("open");
            for entry in read.range_fixed(&tree, ..).expect("scan") {
                let (key, value) = entry.expect("entry");
                sum = fold(sum, key, value);
                count += 1;
            }
        }
        Kind::Normal => {
            let tree = read.open_tree(TREE).expect("open");
            for entry in read.range(&tree, ..).expect("scan") {
                let (key, value) = entry.expect("entry");
                let key = u64::from_be_bytes(key.try_into().expect("8 bytes"));
                sum = fold(sum, key, value);
                count += 1;
            }
        }
    }
    (start.elapsed(), count, sum)
}

/// Removes every entry of `order` in writes of [`BATCH`] entries and returns the time.
fn remove_all(store: &Store, kind: Kind, data: &Data, order: &[usize]) -> Duration {
    let start = Instant::now();
    for chunk in order.chunks(BATCH) {
        let mut write = store.write().expect("write");
        match kind {
            Kind::Fixed => {
                let tree = write.open_fixed_tree(TREE, data.len).expect("open");
                for &i in chunk {
                    assert!(write.remove_fixed(&tree, data.keys[i]).expect("remove"));
                }
            }
            Kind::Normal => {
                let tree = write.open_tree(TREE).expect("open");
                for &i in chunk {
                    assert!(
                        write
                            .remove(&tree, &data.keys[i].to_be_bytes())
                            .expect("remove")
                    );
                }
            }
        }
        write.commit().expect("commit");
    }
    start.elapsed()
}

/// Runs every measurement of `kind` with `data` in new folders under `parent`.
fn measure(parent: &Path, kind: Kind, data: &Data) -> Measured {
    let entries = data.keys.len();
    let mut in_key_order: Vec<usize> = (0..entries).collect();
    in_key_order.sort_unstable_by_key(|&i| data.keys[i]);
    let random = shuffled(entries, 0x5EED_0005);

    let folder = tempfile::tempdir_in(parent).expect("folder");
    let (store, in_order, in_order_info) = load(folder.path(), kind, data, &in_key_order);
    store.close().expect("close");

    let folder = tempfile::tempdir_in(parent).expect("folder");
    let (store, at_random, at_random_info) = load(folder.path(), kind, data, &random);
    store.close().expect("close");
    let store = Store::open(folder.path(), &StoreOptions::default()).expect("reopen");
    let reads = shuffled(entries, 0x5EED_0006);
    let (read, read_sum) = read_all(&store, kind, data, &reads);
    let (scan, scanned, scan_sum) = scan_all(&store, kind, data);
    assert_eq!(scanned, entries, "the scan returns every entry");
    std::hint::black_box((read_sum, scan_sum));
    let remove = remove_all(&store, kind, data, &shuffled(entries, 0x5EED_0007));
    store.close().expect("close");
    Measured {
        in_order,
        in_order_info,
        at_random,
        at_random_info,
        read,
        scan,
        remove,
    }
}

struct Options {
    folder: PathBuf,
    entries: usize,
    lengths: Vec<usize>,
}

fn options() -> Options {
    let mut options = Options {
        folder: std::env::temp_dir(),
        entries: ENTRIES,
        lengths: LENGTHS.to_vec(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{arg} needs a value"));
        match arg.as_str() {
            "--folder" => options.folder = PathBuf::from(value()),
            "--entries" => options.entries = value().parse().expect("a count"),
            "--lengths" => {
                options.lengths = value()
                    .split(',')
                    .map(|n| n.parse().expect("a length in bytes"))
                    .collect();
            }
            "--quick" => options.entries = QUICK,
            other => panic!("unknown option {other}"),
        }
    }
    options
}

fn micros(elapsed: Duration, entries: usize) -> String {
    format!("{:.2}", elapsed.as_secs_f64() * 1e6 / entries as f64)
}

fn main() {
    let options = options();
    println!("# Fixed-size trees against normal trees (X13)");
    println!();
    println!(
        "- OS: {}, architecture: {}; processor and disk: fill in by hand.",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!(
        "- Entries per load: {}; writes of {BATCH} entries.",
        options.entries
    );
    for &len in &options.lengths {
        let data = Data::new(options.entries, len);
        println!();
        println!("## Values of {len} bytes");
        println!();
        println!(
            "| Tree | Store in order (µs) | Size in order (bytes) | Pages in use in order | Store at random (µs) | \
             Size at random (bytes) | Pages in use at random | Read at random (µs) | Scan forward (µs) | \
             Remove at random (µs) |"
        );
        println!("|---|---|---|---|---|---|---|---|---|---|");
        for kind in [Kind::Fixed, Kind::Normal] {
            let m = measure(&options.folder, kind, &data);
            let n = options.entries;
            let in_use = |info: &StoreInfo| info.page_count - info.free_pages;
            println!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                kind.name(),
                micros(m.in_order, n),
                m.in_order_info.size_bytes,
                in_use(&m.in_order_info),
                micros(m.at_random, n),
                m.at_random_info.size_bytes,
                in_use(&m.at_random_info),
                micros(m.read, n),
                micros(m.scan, n),
                micros(m.remove, n),
            );
        }
    }
}
