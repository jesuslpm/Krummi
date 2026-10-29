// AC-R8 (S8), R6: a key returned by a scan of a write transaction stays usable after the scan is dropped, but not
// after the next operation that may modify the transaction.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let a = write.open_tree("a").unwrap();
    let (key, _) = write.range(&a, ..).unwrap().next().unwrap().unwrap();
    println!("{key:?}");
    let _ = write.remove(&a, b"absent");
    println!("{key:?}");
}
