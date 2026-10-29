// AC-R8 (S8), R6, R7, A10: a scan of a write transaction, and a key it returned, stop being usable at
// remove_range and remove_range_prefix, even when they fail without effect or remove nothing.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let a = write.open_tree("a").unwrap();

    // A removal that removes nothing: the interval is empty.
    let mut scan = write.range(&a, ..).unwrap();
    let _ = write.remove_range(&a, &b"z"[..]..&b"a"[..]);
    let _ = scan.next();

    // A removal that fails: the prefix is too long.
    let (key, _) = write.range(&a, ..).unwrap().next().unwrap().unwrap();
    let _ = write.remove_range_prefix(&a, &[0; 2049], ..);
    println!("{key:?}");
}
