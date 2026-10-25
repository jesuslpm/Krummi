// AC20 of the root (S36), D11, T8: a reference read from a tree cannot be used after its transaction ends.
use krummi::{Store, StoreOptions, TreeKind};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let tree = read.open_tree("t", TreeKind::Normal).unwrap();
    let value = read.get(&tree, b"k").unwrap();
    drop(read);
    println!("{value:?}");
}
