// AC-M9 (S9), M5, M6, M8: a read transaction has none of the operations that modify a multi-value tree: opening in
// another mode than Open, add, remove value, remove key and the range removals, with or without a prefix.
use krummi::{OpenMode, Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let tree = read.open_multi_tree("m").unwrap();
    let _ = read.open_multi_tree_with("m", OpenMode::OpenOrCreate);
    let _ = read.add(&tree, b"k", b"v");
    let _ = read.remove_value(&tree, b"k", b"v");
    let _ = read.remove_key(&tree, b"k");
    let _ = read.remove_key_range(&tree, ..);
    let _ = read.remove_value_range(&tree, b"k", ..);
    let _ = read.remove_key_range_prefix(&tree, b"p", ..);
    let _ = read.remove_value_range_prefix(&tree, b"k", b"p", ..);
}
