// AC-X7 (S7), X4: the handle of a fixed-size tree takes none of the operations of normal trees (5.1, 5.2) nor of
// multi-value trees (5.4), in a write transaction and in a read transaction.
use krummi::{OpenMode, Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let tree = write.open_fixed_tree_with("f", 8, OpenMode::OpenOrCreate).unwrap();
    let _ = write.set(&tree, b"k", b"v");
    let _ = write.try_add(&tree, b"k", b"v");
    let _ = write.get(&tree, b"k");
    let _ = write.remove(&tree, b"k");
    let _ = write.range(&tree, ..);
    let _ = write.range_prefix(&tree, b"p", ..);
    let _ = write.remove_range(&tree, ..);
    let _ = write.remove_range_prefix(&tree, b"p", ..);
    let _ = write.add(&tree, b"k", b"v");
    let _ = write.contains(&tree, b"k", b"v");
    let _ = write.value_count(&tree, b"k");
    let _ = write.keys(&tree, ..);
    let _ = write.values(&tree, b"k", ..);
    let _ = write.remove_value(&tree, b"k", b"v");
    let _ = write.remove_key(&tree, b"k");
    let _ = write.remove_key_range(&tree, ..);
    let _ = write.remove_value_range(&tree, b"k", ..);
    write.commit().unwrap();

    let read = store.read().unwrap();
    let tree = read.open_fixed_tree("f", 8).unwrap();
    let _ = read.get(&tree, b"k");
    let _ = read.range(&tree, ..);
    let _ = read.range_prefix(&tree, b"p", ..);
    let _ = read.contains(&tree, b"k", b"v");
    let _ = read.value_count(&tree, b"k");
    let _ = read.keys(&tree, ..);
    let _ = read.values(&tree, b"k", ..);
}
