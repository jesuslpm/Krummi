// AC-A20 (S20), AC-B11 (S29), A10: a reference read from a tree stops being usable when a tree is deleted, even
// one that does not exist, so nothing changes.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let tree = write.open_tree("t").unwrap();
    let value = write.get(&tree, b"k").unwrap();
    write.delete_tree("missing").unwrap();
    println!("{value:?}");
}
