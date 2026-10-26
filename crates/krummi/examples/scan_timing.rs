//! Time to scan and to remove by intervals the store of the 5.1 measurement (decision R11 of the in-order scan
//! and prefix search PRD, checked by criterion AC-R15).
//!
//! ```text
//! cargo run --release -p krummi --example scan_timing [folder] [entries]
//! ```
//!
//! It loads, in a temporary folder inside `folder` (the system's temporary folder by default), the store that
//! `tree_timing` loads at random: `entries` random 16-byte keys (1,000,000 by default) with the same seed and
//! values of 100 bytes, in writes of 1,000 that commit durably. Each part then works on its own copy of that
//! store, closed and reopened. In a single read transaction, reading the bytes of every key and value returned:
//! a whole scan forward and then backward, and 10,000 forward scans of 100 entries, each from a random existing key
//! with at least 100 entries from it to the end.
//! Then every entry is removed with 1,000 contiguous intervals between random cuts, the first with its lower bound
//! open and the last with its upper bound open, one per write, each committing durably. It prints a Markdown
//! table. There is no threshold (D5 of the root PRD).

mod timing;

use std::fs;
use std::ops::Bound::{Excluded, Included, Unbounded};
use std::path::Path;
use std::time::Instant;

use krummi::{Result, Store, StoreOptions, TreeKind};
use timing::{KEY, TREE, VALUE, header, keys, load, row, shuffled};

const SCANS: usize = 10_000;
const SCAN_LEN: usize = 100;
const INTERVALS: usize = 1_000;

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
    let loaded = tempfile::tempdir_in(&base).expect("a temporary folder");
    let (elapsed, size) = load(loaded.path(), &keys);
    row("store at random", entries, elapsed, Some(size));

    let copy = copy_of(loaded.path(), &base);
    let store = Store::open(copy.path(), &StoreOptions::default()).expect("reopen");
    {
        let read = store.read().expect("read");
        let tree = read
            .open_tree(TREE, TreeKind::Normal)
            .expect("open the tree");

        let start = Instant::now();
        let (count, bytes) = consume(read.range(&tree, ..).expect("scan"));
        assert_eq!((count, bytes), (entries, entries * (KEY + VALUE)));
        row("scan forward", entries, start.elapsed(), None);

        let start = Instant::now();
        let (count, _) = consume(read.range(&tree, ..).expect("scan").rev());
        assert_eq!(count, entries);
        row("scan backward", entries, start.elapsed(), None);

        // Random starts with at least SCAN_LEN entries from them to the end, so every scan returns SCAN_LEN.
        let length = SCAN_LEN.min(entries);
        let order = shuffled(entries - length + 1, 0x5EED_0003);
        let start = Instant::now();
        let mut returned = 0;
        for &rank in order.iter().cycle().take(SCANS) {
            let scan = read.range(&tree, &sorted[rank][..]..).expect("scan");
            let count = consume(scan.take(length)).0;
            assert_eq!(count, length, "a scan from rank {rank}");
            returned += count;
        }
        row(
            &format!("{SCANS} scans of {SCAN_LEN} from random keys"),
            returned,
            start.elapsed(),
            None,
        );
    }
    store.close().expect("close");

    let copy = copy_of(loaded.path(), &base);
    let store = Store::open(copy.path(), &StoreOptions::default()).expect("reopen");
    // As many intervals as entries at most, so that small runs work too.
    let intervals = INTERVALS.min(entries);
    let mut cuts: Vec<usize> = shuffled(entries, 0x5EED_0004)
        .into_iter()
        .filter(|&i| i > 0)
        .take(intervals - 1)
        .collect();
    cuts.sort_unstable();
    let start = Instant::now();
    let mut removed = 0;
    for i in 0..intervals {
        let lower = if i == 0 {
            Unbounded
        } else {
            Included(&sorted[cuts[i - 1]][..])
        };
        let upper = if i == intervals - 1 {
            Unbounded
        } else {
            Excluded(&sorted[cuts[i]][..])
        };
        let mut write = store.write().expect("write");
        let tree = write
            .open_tree(TREE, TreeKind::Normal)
            .expect("open the tree");
        removed += write
            .remove_range(&tree, (lower, upper))
            .expect("remove range");
        write.commit().expect("commit");
    }
    assert_eq!(removed, entries as u64);
    row(
        &format!("remove with {intervals} intervals"),
        entries,
        start.elapsed(),
        Some(store.info().size_bytes),
    );
    store.close().expect("close");
}

/// A copy of the closed store in `folder`, in a new temporary folder inside `base`.
fn copy_of(folder: &Path, base: &Path) -> tempfile::TempDir {
    let copy = tempfile::tempdir_in(base).expect("a temporary folder");
    for entry in fs::read_dir(folder).expect("the store's folder") {
        let path = entry.expect("an entry").path();
        fs::copy(&path, copy.path().join(path.file_name().expect("a name"))).expect("copy");
    }
    copy
}

/// The number of entries a scan returns and their bytes, reading every byte of each key and value.
fn consume<'t>(scan: impl Iterator<Item = Result<(&'t [u8], &'t [u8])>>) -> (usize, usize) {
    let (mut count, mut bytes, mut sum) = (0, 0, 0u64);
    for entry in scan {
        let (key, value) = entry.expect("an entry");
        count += 1;
        bytes += key.len() + value.len();
        sum = key
            .iter()
            .chain(value)
            .fold(sum, |s, &b| s.wrapping_add(u64::from(b)));
    }
    std::hint::black_box(sum);
    (count, bytes)
}
