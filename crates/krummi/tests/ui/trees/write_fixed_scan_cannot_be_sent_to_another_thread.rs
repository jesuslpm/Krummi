// AC-X7 (S7), X7, R9, B1: a scan of a fixed-size tree of a write transaction stays on its transaction's thread.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let write = store.write().unwrap();
    let f = write.open_fixed_tree("f", 8).unwrap();
    let scan = write.range_fixed(&f, ..).unwrap();
    std::thread::scope(|s| {
        s.spawn(move || scan.count());
    });
}
