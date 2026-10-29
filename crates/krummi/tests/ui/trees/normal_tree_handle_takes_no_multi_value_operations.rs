// AC-M9 (S9), M4: the handle of a normal tree takes none of the operations of multi-value trees, in a write
// transaction and in a read transaction.
use krummi::{OpenMode, Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let tree = write.open_tree_with("t", OpenMode::OpenOrCreate).unwrap();
    let _ = write.add(&tree, b"k", b"v");
    let _ = write.remove_value(&tree, b"k", b"v");
    let _ = write.remove_key(&tree, b"k");
    let _ = write.contains(&tree, b"k", b"v");
    let _ = write.value_count(&tree, b"k");
    let _ = write.keys(&tree, ..);
    let _ = write.values(&tree, b"k", ..);
    let _ = write.remove_key_range(&tree, ..);
    let _ = write.remove_value_range(&tree, b"k", ..);
    let _ = write.keys_prefix(&tree, b"p", ..);
    let _ = write.values_prefix(&tree, b"k", b"p", ..);
    let _ = write.remove_key_range_prefix(&tree, b"p", ..);
    let _ = write.remove_value_range_prefix(&tree, b"k", b"p", ..);
    write.commit().unwrap();

    let read = store.read().unwrap();
    let tree = read.open_tree("t").unwrap();
    let _ = read.contains(&tree, b"k", b"v");
    let _ = read.value_count(&tree, b"k");
    let _ = read.keys(&tree, ..);
    let _ = read.values(&tree, b"k", ..);
    let _ = read.keys_prefix(&tree, b"p", ..);
    let _ = read.values_prefix(&tree, b"k", b"p", ..);
}
