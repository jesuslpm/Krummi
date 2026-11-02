// AC-X7 (S7), X6, X7, B1: a read transaction has none of the operations that modify a fixed-size tree: opening in
// another mode than Open, set, add, remove and range removal.
use krummi::{OpenMode, Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let tree = read.open_fixed_tree("f", 8).unwrap();
    let _ = read.open_fixed_tree_with("f", 8, OpenMode::OpenOrCreate);
    let _ = read.set_fixed(&tree, 1, b"value 08");
    let _ = read.try_add_fixed(&tree, 1, b"value 08");
    let _ = read.remove_fixed(&tree, 1);
    let _ = read.remove_range_fixed(&tree, ..);
}
