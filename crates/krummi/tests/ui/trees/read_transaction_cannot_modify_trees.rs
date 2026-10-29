// AC-A3 (S3), AC-B11 (S29), B1, A1, A3, A4: a read transaction has none of the operations that modify: opening
// in another mode than Open, deleting, renaming, set, try_add and remove do not compile.
use krummi::{OpenMode, Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let tree = read.open_tree("t").unwrap();
    let _ = read.open_tree_with("t", OpenMode::OpenOrCreate);
    let _ = read.delete_tree("t");
    let _ = read.rename_tree("t", "u", false);
    let _ = read.set(&tree, b"k", b"v");
    let _ = read.try_add(&tree, b"k", b"v");
    let _ = read.remove(&tree, b"k");
}
