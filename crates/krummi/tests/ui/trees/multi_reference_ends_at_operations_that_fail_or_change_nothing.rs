// AC-M9 (S9), AC-M6 (S6), M9: a key or a value returned by a scan of a multi-value tree in a write transaction, and
// the scan itself, stop being usable at the next add, remove value, remove key or range removal, through another
// handle of the tree too, even when the operation fails without effect or changes nothing.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let a = write.open_multi_tree("a").unwrap();
    let other = write.open_multi_tree("a").unwrap();

    // An add that fails: the key is empty.
    let value = write.values(&a, b"k", ..).unwrap().next().unwrap().unwrap();
    let _ = write.add(&a, b"", b"v");
    println!("{value:?}");

    // An add that does not add, through another handle.
    let mut scan = write.keys(&a, ..).unwrap();
    let _ = write.add(&other, b"k", b"present");
    let _ = scan.next();

    // A remove value that does not remove.
    let (key, _) = write.keys(&a, ..).unwrap().next().unwrap().unwrap();
    let _ = write.remove_value(&a, b"k", b"absent");
    println!("{key:?}");

    // A remove key that does not remove, through another handle.
    let mut scan = write.values_prefix(&a, b"k", b"p", ..).unwrap();
    let _ = write.remove_key(&other, b"absent");
    let _ = scan.next_back();

    // Range removals that remove nothing.
    let mut scan = write.values(&a, b"k", ..).unwrap();
    let _ = write.remove_key_range_prefix(&a, b"absent", ..);
    let _ = scan.next();
    let mut scan = write.keys_prefix(&a, b"k", ..).unwrap();
    let _ = write.remove_value_range(&a, b"absent", ..);
    let _ = scan.next();
}
