// AC-M9 (S9), M7, R9, B1: a key scan of a multi-value tree of a write transaction stays on its transaction's thread.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let write = store.write().unwrap();
    let a = write.open_multi_tree("a").unwrap();
    let keys = write.keys(&a, ..).unwrap();
    std::thread::scope(|s| {
        s.spawn(move || keys.count());
    });
}
