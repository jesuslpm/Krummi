// AC-M9 (S9), M4: the handle of a multi-value tree takes none of the operations of normal trees: set, try_add, get
// and remove of 5.1, and the scans and range removals of 5.2, in a write transaction and in a read transaction.
use krummi::{OpenMode, Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let tree = write.open_multi_tree_with("m", OpenMode::OpenOrCreate).unwrap();
    let _ = write.set(&tree, b"k", b"v");
    let _ = write.try_add(&tree, b"k", b"v");
    let _ = write.get(&tree, b"k");
    let _ = write.remove(&tree, b"k");
    let _ = write.range(&tree, ..);
    let _ = write.range_prefix(&tree, b"p", ..);
    let _ = write.remove_range(&tree, ..);
    let _ = write.remove_range_prefix(&tree, b"p", ..);
    write.commit().unwrap();

    let read = store.read().unwrap();
    let tree = read.open_multi_tree("m").unwrap();
    let _ = read.get(&tree, b"k");
    let _ = read.range(&tree, ..);
    let _ = read.range_prefix(&tree, b"p", ..);
}
