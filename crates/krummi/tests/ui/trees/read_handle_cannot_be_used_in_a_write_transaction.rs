// AC-B11 (S29), B1: a tree opened in a read transaction cannot be modified, also through a write transaction:
// the write transaction takes only its own kind of handle.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let tree = read.open_tree("t").unwrap();
    let mut write = store.write().unwrap();
    write.set(&tree, b"k", b"v").unwrap();
}
