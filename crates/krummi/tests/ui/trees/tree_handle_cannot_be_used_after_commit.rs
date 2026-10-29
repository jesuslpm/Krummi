// AC-B11 (S29), B1, T8: after a commit, its transaction cannot be used, so neither can a tree through it.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let tree = write.open_tree("t").unwrap();
    write.commit().unwrap();
    write.set(&tree, b"k", b"v").unwrap();
}
