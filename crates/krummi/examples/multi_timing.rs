//! Time and space of multi-value trees with each candidate pair of thresholds (decision M15 of the multi-value trees
//! PRD, whose data feeds the choice of the thresholds, M3, and is checked by criteria AC-M19 and AC-M20).
//!
//! ```text
//! cargo run --release -p krummi --example multi_timing [options]
//! ```
//!
//! Options:
//!
//! - `--folder <path>`: where the stores live, each in its own temporary folder (the system's temporary folder by
//!   default).
//! - `--pairs <p:d,...>`: the threshold pairs, promotion and demotion in bytes of volume, `max` standing for
//!   [`MAX_PROMOTION`] (the six of M3 below by default).
//! - `--loads <name,...>`: the loads to run, of `n1`, `n10`, `n100`, `n1000`, `n10000`, `n100000`, `n1000000`,
//!   `zipf` and `oscillating` (all by default), so that a run can be split.
//! - `--values <count>`: values per load (1,000,000 by default, as M15 sets); the skewed load spreads them over a
//!   tenth as many keys and the oscillating load makes a tenth as many operations, so the default is M15's load.
//!   A set size above the count is skipped.
//! - `--quick`: a smoke test, the same as `--values 10000`.
//!
//! Every store is on disk and commits durably; keys and values are random 16-byte strings with fixed seeds. Before
//! each load the thresholds are set with the measurement hook of the engine (D7). The loads (M15):
//!
//! - **By set size**: the values spread over keys of N values each, for N = 1, 10, 100, 1,000, 10,000, 100,000 and
//!   1,000,000.
//! - **Skewed**: each value goes to a key chosen at random among a tenth as many keys (100,000) with a Zipf
//!   distribution of exponent 1.
//!
//!   For each of them, in a new store: add every value in a random order that interleaves the keys, in writes of
//!   1,000 values; the store's size (P12) after the last commit; then, closed and reopened without dropping the
//!   system's cache, check every value at random with Contains in a single read transaction, scan the keys of the
//!   whole tree, and scan the values of every key, reading every byte, each in its own read transaction; and remove
//!   every value at random with Remove value, in writes of 1,000. It also prints the store's size after the
//!   removals.
//! - **Oscillating**: in a new store, one key with the most values whose volume does not exceed the promotion
//!   threshold, so that the next Add exceeds it; then a tenth as many operations as values (100,000), alternating,
//!   from an Add, an Add of a new value and a Remove value of that same value, in writes of 1,000 operations: the
//!   time, the store's size after the last commit, and whether the key ends in a subtree.
//!
//! The candidate pairs (M3), in bytes of volume (the key's length plus its inline blob: 3 bytes and, per value, 2
//! bytes and its bytes, D2), so that with 16-byte keys and values a key of `n` values has a volume of `19 + 18n`:
//!
//! | Promotion | Values inline at most | Demotion close (7/8) | Demotion about half |
//! |---|---|---|---|
//! | 512, low | 27 | 448 | 256 |
//! | 2,048, intermediate (the provisional default) | 112 | 1,792 | 1,024 |
//! | [`MAX_PROMOTION`] (4,074), high: an inline entry fills the longest leaf cell | 225 | 3,564 | 2,037 |
//!
//! The low promotion moves keys of 100 values to subtrees and keeps keys of 10 inline; the intermediate and the high
//! keep keys of 100 inline too, with a quarter and a half of a leaf. The close demotion is 64 bytes, three and a half
//! values, below the low promotion and proportionally below the others.
//!
//! It prints a header with the operating system and the architecture, and a Markdown table per load with a row per
//! pair; durations are in milliseconds. The processor and the disk go in the report by hand. The results go in the
//! closing report of the multi-value trees execution, with the evaluation of M3. No time is a pass mark (D5 of the
//! root PRD).

// The measurement shares the tree measurements' helpers, of which it uses only some.
#[allow(dead_code)]
mod timing;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use krummi::{OpenMode, Store, StoreOptions};
use krummi_tree::multi::{MAX_PROMOTION, multi_thresholds, set_multi_thresholds, volume};
use timing::{BATCH, KEY, TREE, shuffled, splitmix};

/// Values per load, as M15 sets them.
const VALUES: usize = 1_000_000;
/// Values per load with `--quick`.
const QUICK: usize = 10_000;
/// The set sizes of the loads by set size.
const SET_SIZES: [usize; 7] = [1, 10, 100, 1_000, 10_000, 100_000, 1_000_000];
/// The candidate promotion thresholds of M3: low, intermediate and high.
const PROMOTIONS: [usize; 3] = [512, 2048, MAX_PROMOTION];

/// A load of M15.
#[derive(Clone, Copy, PartialEq)]
enum Load {
    /// Keys of this many values each.
    SetSize(usize),
    /// Values spread over keys with a Zipf distribution of exponent 1.
    Zipf,
    /// One key at the promotion threshold, with Add and Remove value alternating.
    Oscillating,
}

impl Load {
    fn name(self) -> String {
        match self {
            Load::SetSize(n) => format!("n{n}"),
            Load::Zipf => "zipf".into(),
            Load::Oscillating => "oscillating".into(),
        }
    }
}

/// The values of a load and the key of each.
struct Data {
    keys: Vec<[u8; KEY]>,
    values: Vec<[u8; KEY]>,
    /// The index in `keys` of the key of each value.
    owner: Vec<u32>,
    /// The indexes of the keys with values, in the keys' byte order, and their number of values.
    used: Vec<(usize, u64)>,
}

/// What a load by set size or the skewed load measures with one pair.
struct Measured {
    add: Duration,
    size: u64,
    contains: Duration,
    keys: Duration,
    values: Duration,
    remove: Duration,
    emptied: u64,
}

fn main() {
    let options = Options::parse();
    println!("# Multi-value trees measurement (M15)");
    println!();
    println!(
        "- OS: {}, architecture: {}; processor and disk: fill in by hand.",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!(
        "- Values per load: {}; writes of {BATCH}; keys and values of {KEY} bytes; MAX_PROMOTION = {MAX_PROMOTION}.",
        options.values
    );
    println!(
        "- Pairs (promotion:demotion): {}.",
        pairs_label(&options.pairs)
    );

    for &load in &options.loads {
        println!();
        match load {
            Load::SetSize(n) if n > options.values => {
                println!(
                    "## Load {}: skipped, {n} values per key exceed the {} values of the load",
                    load.name(),
                    options.values
                );
            }
            Load::SetSize(_) | Load::Zipf => {
                let data = data(load, options.values);
                println!(
                    "## Load {}: {} values, keys with values: {}",
                    load.name(),
                    data.values.len(),
                    data.used.len()
                );
                println!();
                println!(
                    "| Promotion:demotion | Add (ms) | µs per add | Size after add (bytes) | Contains (ms) | Scan keys (ms) | Scan values (ms) | Remove (ms) | Size after remove (bytes) |"
                );
                println!("|---|---|---|---|---|---|---|---|---|");
                for &(promotion, demotion) in &options.pairs {
                    set_multi_thresholds(promotion, demotion);
                    let m = measure(&options.folder, &data);
                    println!(
                        "| {promotion}:{demotion} | {} | {:.2} | {} | {} | {} | {} | {} | {} |",
                        ms(m.add),
                        m.add.as_secs_f64() * 1e6 / data.values.len() as f64,
                        m.size,
                        ms(m.contains),
                        ms(m.keys),
                        ms(m.values),
                        ms(m.remove),
                        m.emptied,
                    );
                }
            }
            Load::Oscillating => {
                let operations = options.values / 10;
                println!(
                    "## Load oscillating: {operations} operations on one key at the promotion threshold"
                );
                println!();
                println!(
                    "| Promotion:demotion | Values at the threshold | Time (ms) | µs per operation | Size (bytes) | Ends in a subtree |"
                );
                println!("|---|---|---|---|---|---|");
                for &(promotion, demotion) in &options.pairs {
                    set_multi_thresholds(promotion, demotion);
                    let (at_threshold, elapsed, size, subtree) =
                        oscillate(&options.folder, operations);
                    println!(
                        "| {promotion}:{demotion} | {at_threshold} | {} | {:.2} | {size} | {} |",
                        ms(elapsed),
                        elapsed.as_secs_f64() * 1e6 / operations.max(1) as f64,
                        if subtree { "yes" } else { "no" },
                    );
                }
            }
        }
    }
}

/// The options of the command line.
struct Options {
    folder: PathBuf,
    pairs: Vec<(usize, usize)>,
    loads: Vec<Load>,
    values: usize,
}

impl Options {
    fn parse() -> Self {
        let mut options = Options {
            folder: std::env::temp_dir(),
            pairs: PROMOTIONS
                .iter()
                .flat_map(|&p| [(p, p * 7 / 8), (p, p / 2)])
                .collect(),
            loads: all_loads(),
            values: VALUES,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            let mut value = || {
                args.next()
                    .unwrap_or_else(|| usage(&format!("{arg} needs a value")))
            };
            match arg.as_str() {
                "--folder" => options.folder = value().into(),
                "--pairs" => options.pairs = parse_pairs(&value()),
                "--loads" => options.loads = parse_loads(&value()),
                "--values" => {
                    options.values = value()
                        .parse()
                        .ok()
                        .filter(|&v| v > 0)
                        .unwrap_or_else(|| usage("--values takes a positive count"));
                }
                "--quick" => options.values = QUICK,
                "--help" | "-h" => usage(""),
                other => usage(&format!("unknown option {other}")),
            }
        }
        options
    }
}

fn all_loads() -> Vec<Load> {
    SET_SIZES
        .iter()
        .map(|&n| Load::SetSize(n))
        .chain([Load::Zipf, Load::Oscillating])
        .collect()
}

fn parse_loads(list: &str) -> Vec<Load> {
    list.split(',')
        .map(|name| {
            all_loads()
                .into_iter()
                .find(|load| load.name() == name.trim())
                .unwrap_or_else(|| usage(&format!("unknown load {name}")))
        })
        .collect()
}

/// Pairs `promotion:demotion`, each satisfying `demotion < promotion <= MAX_PROMOTION`.
fn parse_pairs(list: &str) -> Vec<(usize, usize)> {
    let threshold = |s: &str| match s.trim() {
        "max" => Some(MAX_PROMOTION),
        s => s.parse().ok(),
    };
    list.split(',')
        .map(|pair| {
            pair.split_once(':')
                .and_then(|(p, d)| Some((threshold(p)?, threshold(d)?)))
                .filter(|&(p, d)| d < p && p <= MAX_PROMOTION)
                .unwrap_or_else(|| {
                    usage(&format!(
                        "pair {pair} is not promotion:demotion with demotion < promotion <= {MAX_PROMOTION}"
                    ))
                })
        })
        .collect()
}

fn pairs_label(pairs: &[(usize, usize)]) -> String {
    pairs
        .iter()
        .map(|(p, d)| format!("{p}:{d}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn usage(error: &str) -> ! {
    if !error.is_empty() {
        eprintln!("error: {error}");
    }
    eprintln!(
        "usage: multi_timing [--folder <path>] [--pairs <p:d,...>] [--loads <name,...>] [--values <count>] [--quick]"
    );
    eprintln!(
        "loads: {}",
        all_loads()
            .iter()
            .map(|l| l.name())
            .collect::<Vec<_>>()
            .join(", ")
    );
    std::process::exit(if error.is_empty() { 0 } else { 2 })
}

/// The keys and values of a load by set size or of the skewed load, with `count` values.
fn data(load: Load, count: usize) -> Data {
    let values = timing::keys(count);
    let (key_count, owner): (usize, Vec<u32>) = match load {
        Load::SetSize(n) => (count / n, (0..count).map(|j| (j / n) as u32).collect()),
        Load::Zipf => {
            let key_count = (count / 10).max(1);
            (key_count, zipf(key_count, count, 0x5EED_0102))
        }
        Load::Oscillating => unreachable!("the oscillating load has its own data"),
    };
    let keys = random_keys(key_count);
    let mut counts = vec![0u64; key_count];
    for &k in &owner {
        counts[k as usize] += 1;
    }
    let mut used: Vec<(usize, u64)> = counts
        .into_iter()
        .enumerate()
        .filter(|&(_, n)| n > 0)
        .collect();
    used.sort_unstable_by_key(|&(k, _)| keys[k]);
    Data {
        keys,
        values,
        owner,
        used,
    }
}

/// `count` distinct random keys, other than the values that [`timing::keys`] gives: the first 8 bytes are a
/// bijection of the index.
fn random_keys(count: usize) -> Vec<[u8; KEY]> {
    (0..count as u64)
        .map(|i| {
            let mut key = [0u8; KEY];
            key[..8].copy_from_slice(&splitmix(i ^ 0x5EED_0101).to_be_bytes());
            key[8..].copy_from_slice(&splitmix(i.wrapping_add(0x5EED_0103)).to_be_bytes());
            key
        })
        .collect()
}

/// `samples` key indexes among `keys`, index `k` drawn with a weight of `1 / (k + 1)` (Zipf, exponent 1), from a
/// fixed `seed`: inverse transform over the cumulative weights.
fn zipf(keys: usize, samples: usize, seed: u64) -> Vec<u32> {
    let cumulative: Vec<f64> = (1..=keys)
        .scan(0.0, |sum, k| {
            *sum += 1.0 / k as f64;
            Some(*sum)
        })
        .collect();
    let total = cumulative[keys - 1];
    let mut state = seed;
    (0..samples)
        .map(|_| {
            state = splitmix(state);
            // 53 random bits, a uniform number in [0, 1).
            let u = (state >> 11) as f64 / (1u64 << 53) as f64 * total;
            cumulative.partition_point(|&c| c <= u).min(keys - 1) as u32
        })
        .collect()
}

/// Runs a load by set size or the skewed load in a new store inside `base`, with the thresholds in force.
fn measure(base: &Path, data: &Data) -> Measured {
    let folder = tempfile::tempdir_in(base).expect("a temporary folder");
    let count = data.values.len();
    let key_of = |j: usize| &data.keys[data.owner[j] as usize];

    let store = Store::open(folder.path(), &StoreOptions::default()).expect("open");
    let order = shuffled(count, 0x5EED_0104);
    let start = Instant::now();
    for chunk in order.chunks(BATCH) {
        let mut write = store.write().expect("write");
        let tree = write
            .open_multi_tree_with(TREE, OpenMode::OpenOrCreate)
            .expect("open or create the tree");
        for &j in chunk {
            let added = write.add(&tree, key_of(j), &data.values[j]).expect("add");
            assert!(added, "value {j} is new");
        }
        write.commit().expect("commit");
    }
    let add = start.elapsed();
    let size = store.info().size_bytes;
    store.close().expect("close");

    let store = Store::open(folder.path(), &StoreOptions::default()).expect("reopen");
    let contains = {
        let read = store.read().expect("read");
        let tree = read.open_multi_tree(TREE).expect("open the tree");
        let order = shuffled(count, 0x5EED_0105);
        let start = Instant::now();
        for &j in &order {
            let found = read
                .contains(&tree, key_of(j), &data.values[j])
                .expect("contains");
            assert!(found, "value {j} is there");
        }
        start.elapsed()
    };
    let keys = {
        let read = store.read().expect("read");
        let tree = read.open_multi_tree(TREE).expect("open the tree");
        let start = Instant::now();
        let (mut scanned, mut values, mut sum) = (0, 0, 0u64);
        for entry in read.keys(&tree, ..).expect("scan the keys") {
            let (key, n) = entry.expect("a key");
            scanned += 1;
            values += n;
            sum = checksum(sum, key);
        }
        let elapsed = start.elapsed();
        std::hint::black_box(sum);
        assert_eq!((scanned, values), (data.used.len(), count as u64));
        elapsed
    };
    let values = {
        let read = store.read().expect("read");
        let tree = read.open_multi_tree(TREE).expect("open the tree");
        let start = Instant::now();
        let mut sum = 0u64;
        for &(k, expected) in &data.used {
            let mut scanned = 0;
            for value in read
                .values(&tree, &data.keys[k], ..)
                .expect("scan the values")
            {
                sum = checksum(sum, value.expect("a value"));
                scanned += 1;
            }
            assert_eq!(scanned, expected, "the values of key {k}");
        }
        let elapsed = start.elapsed();
        std::hint::black_box(sum);
        elapsed
    };

    let order = shuffled(count, 0x5EED_0106);
    let start = Instant::now();
    for chunk in order.chunks(BATCH) {
        let mut write = store.write().expect("write");
        let tree = write.open_multi_tree(TREE).expect("open the tree");
        for &j in chunk {
            let removed = write
                .remove_value(&tree, key_of(j), &data.values[j])
                .expect("remove value");
            assert!(removed, "value {j} was there");
        }
        write.commit().expect("commit");
    }
    let remove = start.elapsed();
    let emptied = store.info().size_bytes;
    store.close().expect("close");

    Measured {
        add,
        size,
        contains,
        keys,
        values,
        remove,
        emptied,
    }
}

/// Runs the oscillating load in a new store inside `base`, with the thresholds in force: returns the values of the
/// key at the threshold, the time of the `operations`, the store's size after the last commit and whether the key
/// ends in a subtree.
fn oscillate(base: &Path, operations: usize) -> (usize, Duration, u64, bool) {
    let (promotion, _) = multi_thresholds();
    let mut at_threshold = 0;
    while volume(KEY, std::iter::repeat_n(KEY, at_threshold + 1)) <= promotion {
        at_threshold += 1;
    }
    let key = random_keys(1)[0];
    let values = timing::keys(at_threshold + operations.div_ceil(2));

    let folder = tempfile::tempdir_in(base).expect("a temporary folder");
    let store = Store::open(folder.path(), &StoreOptions::default()).expect("open");
    for chunk in values[..at_threshold].chunks(BATCH) {
        let mut write = store.write().expect("write");
        let tree = write
            .open_multi_tree_with(TREE, OpenMode::OpenOrCreate)
            .expect("open or create the tree");
        for value in chunk {
            assert!(write.add(&tree, &key, value).expect("add"));
        }
        write.commit().expect("commit");
    }

    let start = Instant::now();
    for first in (0..operations).step_by(BATCH) {
        let mut write = store.write().expect("write");
        let tree = write.open_multi_tree(TREE).expect("open the tree");
        for i in first..operations.min(first + BATCH) {
            let value = &values[at_threshold + i / 2];
            let changed = if i % 2 == 0 {
                write.add(&tree, &key, value).expect("add")
            } else {
                write
                    .remove_value(&tree, &key, value)
                    .expect("remove value")
            };
            assert!(changed, "operation {i} changes the key");
        }
        write.commit().expect("commit");
    }
    let elapsed = start.elapsed();
    let size = store.info().size_bytes;
    let subtree = {
        let read = store.read().expect("read");
        let tree = read.open_multi_tree(TREE).expect("open the tree");
        read.subtree_of(&tree, &key).expect("subtree of").is_some()
    };
    store.close().expect("close");
    (at_threshold, elapsed, size, subtree)
}

fn checksum(sum: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(sum, |s, &b| s.wrapping_add(u64::from(b)))
}

/// A duration in milliseconds, for the tables.
fn ms(elapsed: Duration) -> String {
    format!("{:.1}", elapsed.as_secs_f64() * 1e3)
}
