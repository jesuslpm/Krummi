//! What the tree measurements share: the random keys, their random orders, loading a store and the rows of the
//! Markdown table. Not an example itself: Cargo only builds `examples/*.rs` and `examples/*/main.rs`.

use std::path::Path;
use std::time::{Duration, Instant};

use krummi::{OpenMode, Store, StoreOptions, TreeKind};

/// Entries per write.
pub const BATCH: usize = 1_000;
/// Bytes of every value.
pub const VALUE: usize = 100;
/// The measured tree.
pub const TREE: &str = "timing";
/// Bytes of every key.
pub const KEY: usize = 16;

/// Stores every key, in the given order, in a new store in `folder`, in writes of [`BATCH`] entries that commit
/// durably, and returns the time and the store's size.
pub fn load(folder: &Path, keys: &[[u8; KEY]]) -> (Duration, u64) {
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
pub fn keys(count: usize) -> Vec<[u8; KEY]> {
    (0..count as u64)
        .map(|i| {
            let mut key = [0u8; KEY];
            key[..8].copy_from_slice(&splitmix(i ^ 0x5EED_0001).to_be_bytes());
            key[8..].copy_from_slice(&splitmix(!i).to_be_bytes());
            key
        })
        .collect()
}

/// The indexes `0..count` in a random order fixed by `seed` (Fisher–Yates).
pub fn shuffled(count: usize, seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..count).collect();
    let mut state = seed;
    for i in (1..count).rev() {
        state = splitmix(state);
        order.swap(i, (state % (i as u64 + 1)) as usize);
    }
    order
}

/// The SplitMix64 mixing function, a bijection of `u64`.
pub fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Prints the table's header.
pub fn header() {
    println!("| Operation | Entries | Seconds | µs per entry | Store size (bytes) |");
    println!("|---|---|---|---|---|");
}

/// Prints a row of the table.
pub fn row(operation: &str, entries: usize, elapsed: Duration, size: Option<u64>) {
    let size = size.map_or_else(|| "—".to_string(), |s| s.to_string());
    println!(
        "| {operation} | {entries} | {:.2} | {:.2} | {size} |",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1e6 / entries as f64
    );
}
