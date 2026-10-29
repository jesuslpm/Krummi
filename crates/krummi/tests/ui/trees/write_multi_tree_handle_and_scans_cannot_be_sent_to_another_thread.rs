// AC-M9 (S9), M4, B1, R9: the handle of a multi-value tree of a write transaction, and its key and value scans, stay
// on their transaction's thread.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let write = store.write().unwrap();
    let a = write.open_multi_tree("a").unwrap();
    let keys = write.keys(&a, ..).unwrap();
    let values = write.values(&a, b"k", ..).unwrap();
    std::thread::scope(|s| {
        s.spawn(move || keys.count());
        s.spawn(move || values.count());
        s.spawn(move || drop(a));
    });
}
