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

mod timing;

use std::time::Instant;

use krummi::{Store, StoreOptions, TreeKind};
use timing::{BATCH, TREE, VALUE, header, keys, load, row, shuffled};

fn main() {
    let mut args = std::env::args().skip(1);
    let base = args.next().map_or_else(std::env::temp_dir, Into::into);
    let entries: usize = args
        .next()
        .map_or(1_000_000, |s| s.parse().expect("entries"));
    let keys = keys(entries);
    let mut sorted = keys.clone();
    sorted.sort_unstable();

    header();

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
