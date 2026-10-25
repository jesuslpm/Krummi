//! Time to store, read and remove 1 million entries in a tree, and the size of the store after each load
//! (decision A15 of the trees and key operations PRD, checked by criterion AC-A19).
//!
//! ```text
//! cargo run --release -p krummi --example tree_timing [folder] [entries]
//! ```
//!
//! Each store lives in a temporary folder inside `folder` (the system's temporary folder by default) and commits
//! durably. The keys are `entries` random 16-byte keys (1,000,000 by default), the same in every load for a fixed
//! seed, and every value has 100 bytes. One store is loaded in key order and another at random, in writes of
//! 1,000 entries; the one loaded at random is closed and reopened, then every key is read at random in a single
//! read transaction, and every key is removed at random in writes of 1,000. It prints a Markdown table. There is
//! no threshold (D5 of the root PRD).

use std::path::Path;
use std::time::{Duration, Instant};

use krummi::{OpenMode, Store, StoreOptions, TreeKind};

const BATCH: usize = 1_000;
const VALUE: usize = 100;
const TREE: &str = "timing";

fn main() {
    let mut args = std::env::args().skip(1);
    let base = args.next().map_or_else(std::env::temp_dir, Into::into);
    let entries: usize = args
        .next()
        .map_or(1_000_000, |s| s.parse().expect("entries"));
    let keys = keys(entries);
    let mut sorted = keys.clone();
    sorted.sort_unstable();

    println!("| Operation | Entries | Seconds | µs per entry | Store size (bytes) |");
    println!("|---|---|---|---|---|");

    let ordered = tempfile::tempdir_in(&base).expect("a temporary folder");
    let (elapsed, size) = load(ordered.path(), &sorted);
    row("store in key order", entries, elapsed, Some(size));

    let random = tempfile::tempdir_in(&base).expect("a temporary folder");
    let (elapsed, size) = load(random.path(), &keys);
    row("store at random", entries, elapsed, Some(size));

    let order = shuffled(entries, 0x5EED_0002);
    let store = Store::open(random.path(), &StoreOptions::default()).expect("reopen");
    let start = Instant::now();
    {
        let read = store.read().expect("read");
        let tree = read
            .open_tree(TREE, TreeKind::Normal)
            .expect("open the tree");
        for &i in &order {
            let value = read
                .get(&tree, &keys[i])
                .expect("get")
                .expect("a stored key");
            assert_eq!(value.len(), VALUE);
        }
    }
    row("read at random", entries, start.elapsed(), None);

    let start = Instant::now();
    for batch in order.chunks(BATCH) {
        let mut write = store.write().expect("write");
        let tree = write
            .open_tree(TREE, TreeKind::Normal)
            .expect("open the tree");
        for &i in batch {
            assert!(write.remove(&tree, &keys[i]).expect("remove"));
        }
        write.commit().expect("commit");
    }
    row(
        "remove at random",
        entries,
        start.elapsed(),
        Some(store.info().size_bytes),
    );
    store.close().expect("close");
}

/// Stores every key, in the given order, in a new store in `folder`, and returns the time and the store's size.
fn load(folder: &Path, keys: &[[u8; 16]]) -> (Duration, u64) {
    let store = Store::open(folder, &StoreOptions::default()).expect("open");
    let value = [0x5Au8; VALUE];
    let start = Instant::now();
    for batch in keys.chunks(BATCH) {
        let mut write = store.write().expect("write");
        let tree = write
            .open_tree_with(TREE, TreeKind::Normal, OpenMode::OpenOrCreate)
            .expect("open or create the tree");
        for key in batch {
            write.set(&tree, key, &value).expect("set");
        }
        write.commit().expect("commit");
    }
    let elapsed = start.elapsed();
    let size = store.info().size_bytes;
    store.close().expect("close");
    (elapsed, size)
}

/// `count` distinct random keys: the first 8 bytes are a bijection of the index, so no two keys are equal.
fn keys(count: usize) -> Vec<[u8; 16]> {
    (0..count as u64)
        .map(|i| {
            let mut key = [0u8; 16];
            key[..8].copy_from_slice(&splitmix(i ^ 0x5EED_0001).to_be_bytes());
            key[8..].copy_from_slice(&splitmix(!i).to_be_bytes());
            key
        })
        .collect()
}

/// The indexes `0..count` in a random order fixed by `seed` (Fisher–Yates).
fn shuffled(count: usize, seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..count).collect();
    let mut state = seed;
    for i in (1..count).rev() {
        state = splitmix(state);
        order.swap(i, (state % (i as u64 + 1)) as usize);
    }
    order
}

/// The SplitMix64 mixing function, a bijection of `u64`.
fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn row(operation: &str, entries: usize, elapsed: Duration, size: Option<u64>) {
    let size = size.map_or_else(|| "—".to_string(), |s| s.to_string());
    println!(
        "| {operation} | {entries} | {:.2} | {:.2} | {size} |",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1e6 / entries as f64
    );
}
