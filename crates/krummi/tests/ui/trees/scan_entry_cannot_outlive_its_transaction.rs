// Root AC20 (S20), R6, D11: a key returned by a scan of a read transaction cannot be used after the transaction
// ends.
use krummi::{Store, StoreOptions, TreeKind};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let a = read.open_tree("a", TreeKind::Normal).unwrap();
    let (key, _) = read.range(&a, ..).unwrap().next().unwrap().unwrap();
    drop(read);
    println!("{key:?}");
}
