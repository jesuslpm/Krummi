// AC-X14 (S14), X6, X7, AC20: a value read from a fixed-size tree, by Leer or by a scan, cannot be used after its
// transaction ends.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let f = read.open_fixed_tree("f", 8).unwrap();
    let value = read.get_fixed(&f, 1).unwrap();
    drop(read);
    println!("{value:?}");

    let read = store.read().unwrap();
    let f = read.open_fixed_tree("f", 8).unwrap();
    let (_, value) = read.range_fixed(&f, ..).unwrap().next().unwrap().unwrap();
    drop(read);
    println!("{value:?}");
}
