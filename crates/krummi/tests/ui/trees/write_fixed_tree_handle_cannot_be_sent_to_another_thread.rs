// AC-X7 (S7), X4, B1: a fixed-size tree opened in a write transaction stays on its thread, like the transaction.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let write = store.write().unwrap();
    let tree = write.open_fixed_tree("f", 8).unwrap();
    std::thread::scope(|s| {
        s.spawn(move || drop(tree));
    });
}
