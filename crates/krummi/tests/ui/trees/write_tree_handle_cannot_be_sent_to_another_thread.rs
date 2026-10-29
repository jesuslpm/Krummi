// AC-A20 (S20), B1, T6: a tree opened in a write transaction stays on its thread, like the transaction.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let write = store.write().unwrap();
    let tree = write.open_tree("t").unwrap();
    std::thread::scope(|s| {
        s.spawn(move || drop(tree));
    });
}
