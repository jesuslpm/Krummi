// AC-M16 (S16), M7, R6: a key or a value returned by a scan of a multi-value tree cannot be used after the
// transaction ends.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let a = read.open_multi_tree("a").unwrap();
    let (key, _) = read.keys(&a, ..).unwrap().next().unwrap().unwrap();
    drop(read);
    println!("{key:?}");

    let read = store.read().unwrap();
    let a = read.open_multi_tree("a").unwrap();
    let value = read.values(&a, b"k", ..).unwrap().next().unwrap().unwrap();
    drop(read);
    println!("{value:?}");
}
