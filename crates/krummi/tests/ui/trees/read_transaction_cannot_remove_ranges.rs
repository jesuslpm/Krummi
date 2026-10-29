// AC-R8 (S8), R7: a read transaction has no range removal.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let a = read.open_tree("a").unwrap();
    let _ = read.remove_range(&a, ..);
    let _ = read.remove_range_prefix(&a, b"p", ..);
}
