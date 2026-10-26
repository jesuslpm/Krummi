// AC-R8 (S8), R9, B1: a scan of a write transaction stays on its thread.
use krummi::{Store, StoreOptions, TreeKind};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let write = store.write().unwrap();
    let a = write.open_tree("a", TreeKind::Normal).unwrap();
    let scan = write.range(&a, ..).unwrap();
    std::thread::scope(|s| {
        s.spawn(move || scan.count());
    });
}
