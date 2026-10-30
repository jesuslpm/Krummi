//! Time of reading tree nodes without and with remembering their checked layout (decision C9 of the remembered node
//! layout check PRD, criterion AC-C10). It is run by hand: neither the tests nor CI run it.
//!
//! ```text
//! cargo run --release -p krummi --example node_check_timing -- [options]
//! ```
//!
//! Options:
//!
//! - `--folder <path>`: where the stores live, each in its own temporary folder (the system's temporary folder by
//!   default).
//! - `--reads <count>`: Gets per round of the leaf load (1,000,000 by default).
//! - `--values <count>`: values of the Contains load (200,000 by default), in keys of 1,000 values.
//! - `--rounds <count>`: rounds of each case, alternating without and with remembering (3 by default); each row
//!   reports the fastest round of each.
//!
//! Loads, on stores on disk with durable commits, closed and reopened before each round without dropping the OS
//! cache, so each round starts with nothing remembered:
//!
//! - **Leaf**: a normal tree whose only node, its root leaf, holds 1, 100 or 370 entries with empty values: keys of 16
//!   bytes for 1 and 370 entries (370 fill the leaf), of 72 bytes for 100 (a key of 64 bytes followed by a value of
//!   8, as in a composite key). Each round opens the tree in one read transaction and Gets its keys at random,
//!   `--reads` times: each Get reads that one node.
//! - **Contains**: a multi-value tree with keys of 64 bytes and 1,000 values of 8 bytes each, so that every key has its
//!   values in a subtree, and the same values in a normal tree whose keys are the key followed by the value (the
//!   composite baseline of the multi-value measurement). Each round checks every value at random in one read
//!   transaction: Contains in the multi-value tree, Get in the composite one.
//!
//! The process-wide switch [`krummi_pager::set_remember_checked`] turns remembering off for the rounds without it.
//! It prints a header with the operating system and the architecture and a Markdown table per load; times are in
//! nanoseconds per operation. No time is a pass mark (D5 of the root PRD).

// The measurement shares the tree measurements' helpers, of which it uses only some.
#[allow(dead_code)]
mod timing;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use krummi::{OpenMode, Store, StoreOptions};
use timing::{BATCH, shuffled, splitmix};

/// The tree of each load.
const TREE: &str = "timing";
/// The composite tree of the Contains load.
const COMPOSITE: &str = "composite";
/// Values of each key of the Contains load.
const PER_KEY: usize = 1_000;
/// Key and value lengths of the Contains load.
const KEY: usize = 64;
const VALUE: usize = 8;

struct Options {
    folder: PathBuf,
    reads: usize,
    values: usize,
    rounds: usize,
}

impl Options {
    fn parse() -> Options {
        let mut options = Options {
            folder: std::env::temp_dir(),
            reads: 1_000_000,
            values: 200_000,
            rounds: 3,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            let value = args.next().unwrap_or_else(|| panic!("{arg} needs a value"));
            let count = || value.parse::<usize>().ok().filter(|&n| n > 0);
            match arg.as_str() {
                "--folder" => options.folder = PathBuf::from(&value),
                "--reads" => options.reads = count().expect("--reads takes a positive count"),
                "--values" => options.values = count().expect("--values takes a positive count"),
                "--rounds" => options.rounds = count().expect("--rounds takes a positive count"),
                _ => panic!("unknown option {arg}"),
            }
        }
        options.values = options.values.div_ceil(PER_KEY) * PER_KEY;
        options
    }
}

/// `len` pseudo-random bytes fixed by `seed`.
fn bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed;
    (0..len.div_ceil(8))
        .flat_map(|_| {
            state = splitmix(state);
            state.to_le_bytes()
        })
        .take(len)
        .collect()
}

/// The fastest round of `run` without remembering and the fastest with it, alternating, each in a new opening.
fn compare(
    folder: &Path,
    rounds: usize,
    mut run: impl FnMut(&Store) -> Duration,
) -> (Duration, Duration) {
    let (mut off, mut on) = (Duration::MAX, Duration::MAX);
    for _ in 0..rounds {
        for remember in [false, true] {
            krummi_pager::set_remember_checked(remember);
            let store = Store::open(folder, &StoreOptions::default()).expect("open");
            let elapsed = run(&store);
            store.close().expect("close");
            let best = if remember { &mut on } else { &mut off };
            *best = (*best).min(elapsed);
        }
    }
    krummi_pager::set_remember_checked(true);
    (off, on)
}

fn ns(elapsed: Duration, operations: usize) -> f64 {
    elapsed.as_secs_f64() * 1e9 / operations as f64
}

fn print_row(case: &str, operations: usize, (off, on): (Duration, Duration)) {
    let (off, on) = (ns(off, operations), ns(on, operations));
    println!(
        "| {case} | {off:.0} | {on:.0} | {:+.0}% |",
        (on - off) / off * 100.0
    );
}

/// A store in a new temporary folder under `base` with a normal tree whose root leaf holds `keys`.
fn leaf_store(base: &Path, keys: &[Vec<u8>]) -> tempfile::TempDir {
    let folder = tempfile::tempdir_in(base).expect("a temporary folder");
    let store = Store::open(folder.path(), &StoreOptions::default()).expect("open");
    let mut write = store.write().expect("write");
    let tree = write
        .open_tree_with(TREE, OpenMode::CreateNew)
        .expect("create");
    for key in keys {
        write.set(&tree, key, b"").expect("set");
    }
    write.commit().expect("commit");
    store.close().expect("close");
    folder
}

fn leaf_load(options: &Options) {
    println!("## Leaf: Get of the only node of a tree");
    println!();
    println!("| Entries in the leaf | ns per Get, without | ns per Get, with | Change |");
    println!("|---|---|---|---|");
    for (entries, len) in [(1, 16), (100, 72), (370, 16)] {
        let keys: Vec<Vec<u8>> = (0..entries)
            .map(|i| {
                let mut key = format!("{i:08}").into_bytes();
                key.resize(len, b'.');
                key
            })
            .collect();
        let folder = leaf_store(&options.folder, &keys);
        let order: Vec<usize> = (0..options.reads)
            .map(|i| (splitmix(i as u64) % entries as u64) as usize)
            .collect();
        let times = compare(folder.path(), options.rounds, |store| {
            let read = store.read().expect("read");
            let tree = read.open_tree(TREE).expect("open the tree");
            let start = Instant::now();
            for &i in &order {
                assert!(read.get(&tree, &keys[i]).expect("get").is_some());
            }
            start.elapsed()
        });
        print_row(&entries.to_string(), options.reads, times);
    }
}

fn contains_load(options: &Options) {
    let keys: Vec<Vec<u8>> = (0..options.values / PER_KEY)
        .map(|k| bytes(1 + k as u64, KEY))
        .collect();
    let values: Vec<(usize, Vec<u8>)> = (0..options.values)
        .map(|j| (j / PER_KEY, bytes(0x5eed_0000 + j as u64, VALUE)))
        .collect();
    let composite: Vec<Vec<u8>> = values
        .iter()
        .map(|(k, value)| [&keys[*k][..], value].concat())
        .collect();
    let folder = tempfile::tempdir_in(&options.folder).expect("a temporary folder");
    let store = Store::open(folder.path(), &StoreOptions::default()).expect("open");
    for chunk in shuffled(values.len(), 7).chunks(BATCH) {
        let mut write = store.write().expect("write");
        let multi = write
            .open_multi_tree_with(TREE, OpenMode::OpenOrCreate)
            .expect("open or create");
        let normal = write
            .open_tree_with(COMPOSITE, OpenMode::OpenOrCreate)
            .expect("open or create");
        for &j in chunk {
            let (k, value) = &values[j];
            write.add(&multi, &keys[*k], value).expect("add");
            write.set(&normal, &composite[j], b"").expect("set");
        }
        write.commit().expect("commit");
    }
    store.close().expect("close");

    println!("## Contains: every value at random, keys of {PER_KEY} values in subtrees");
    println!();
    println!("| Tree | ns per check, without | ns per check, with | Change |");
    println!("|---|---|---|---|");
    let order = shuffled(values.len(), 11);
    let times = compare(folder.path(), options.rounds, |store| {
        let read = store.read().expect("read");
        let tree = read.open_multi_tree(TREE).expect("open the tree");
        let start = Instant::now();
        for &j in &order {
            let (k, value) = &values[j];
            assert!(read.contains(&tree, &keys[*k], value).expect("contains"));
        }
        start.elapsed()
    });
    print_row("multi-value", values.len(), times);
    let times = compare(folder.path(), options.rounds, |store| {
        let read = store.read().expect("read");
        let tree = read.open_tree(COMPOSITE).expect("open the tree");
        let start = Instant::now();
        for &j in &order {
            assert!(read.get(&tree, &composite[j]).expect("get").is_some());
        }
        start.elapsed()
    });
    print_row("composite", values.len(), times);
}

fn main() {
    let options = Options::parse();
    println!("# Remembered node layout check measurement (C9)");
    println!();
    println!(
        "- OS: {}, architecture: {}; processor and disk: fill in by hand.",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!(
        "- Leaf Gets per round: {}; Contains values: {}; rounds: {}, the fastest of each reported; slots per store: {}.",
        options.reads,
        options.values,
        options.rounds,
        krummi_pager::CHECKED_SLOTS
    );
    println!();
    leaf_load(&options);
    println!();
    contains_load(&options);
}
