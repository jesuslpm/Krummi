//! What the journal keeps of each page (the journal page diffs PRD, AC-Y1), through the public API and the
//! journal's test hook that records the size of every entry it encodes on this thread.

use krummi_pager::testing::entry_sizes;

use super::check_tests::{leaf, long_key, root_of};
use super::*;

/// A value of 100 bytes for entry `i`, `round` telling apart the versions of the same entry.
fn value(i: usize, round: u8) -> Vec<u8> {
    (0..100).map(|b| (i as u8) ^ (b as u8) ^ round).collect()
}

/// The bytes the journal kept of page `page` in the commit `commit` makes, before compression.
fn kept_of(page: u64, commit: impl FnOnce()) -> usize {
    entry_sizes::start();
    commit();
    let sizes = entry_sizes::take();
    let of_page: Vec<usize> = sizes
        .iter()
        .filter(|&&(number, _)| number == page)
        .map(|&(_, len)| len)
        .collect();
    assert_eq!(of_page.len(), 1, "one entry of page {page} in {sizes:?}");
    of_page[0]
}

/// AC-Y1 (Y1): of a leaf half full of entries with keys of 16 bytes and values of 100 bytes, the journal keeps less
/// than a quarter of a page when a commit changes a value for one of the same size, when one stores a new entry
/// without splitting the leaf, and when one removes an entry from the middle; and of a page a commit creates with a
/// single entry, too.
#[test]
fn ac_y1_the_journal_keeps_little_of_a_page_a_commit_changes_little() {
    const QUARTER: usize = krummi_pager::format::PAGE_SIZE / 4;
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let key = |i: usize| long_key(i * 2, 16);
    let mut write = store.write().unwrap();
    let tree = write.open_tree_with("n", OpenMode::CreateNew).unwrap();
    for i in 0..30 {
        write.set(&tree, &key(i), &value(i, 0)).unwrap();
    }
    write.commit().unwrap();
    let page = root_of(&store, "n");
    assert_eq!(leaf(&store, page, &key(0)), page, "a tree of one leaf");

    let changed = kept_of(page, || {
        let mut write = store.write().unwrap();
        let tree = write.open_tree("n").unwrap();
        write.set(&tree, &key(10), &value(10, 1)).unwrap();
        write.commit().unwrap();
    });
    let stored = kept_of(page, || {
        let mut write = store.write().unwrap();
        let tree = write.open_tree("n").unwrap();
        write.set(&tree, &long_key(21, 16), &value(99, 0)).unwrap();
        write.commit().unwrap();
    });
    let removed = kept_of(page, || {
        let mut write = store.write().unwrap();
        let tree = write.open_tree("n").unwrap();
        write.remove(&tree, &key(3)).unwrap();
        write.commit().unwrap();
    });
    let created = {
        entry_sizes::start();
        let mut write = store.write().unwrap();
        let tree = write.open_tree_with("x", OpenMode::CreateNew).unwrap();
        write.set(&tree, &key(0), &value(0, 0)).unwrap();
        write.commit().unwrap();
        let root = root_of(&store, "x");
        let sizes = entry_sizes::take();
        sizes
            .iter()
            .find(|&&(number, _)| number == root)
            .expect("the new tree's root")
            .1
    };
    for (what, kept) in [
        ("a value changed", changed),
        ("an entry stored", stored),
        ("an entry removed", removed),
        ("a page created", created),
    ] {
        assert!(kept < QUARTER, "{what}: {kept} bytes");
    }
    // A removal leaves a hole: it changes the offsets after it and the header (102 bytes here), where moving the
    // cells changed 1,018.
    assert!(removed < 256, "an entry removed: {removed} bytes");
    let read = store.read().unwrap();
    let tree = read.open_tree("n").unwrap();
    assert_eq!(read.get(&tree, &key(10)).unwrap(), Some(&value(10, 1)[..]));
    assert_eq!(read.get(&tree, &key(3)).unwrap(), None);
    drop(read);
    store.close().unwrap();
}
