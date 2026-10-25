// B1 and its 2026-10-25 exception, T8: a tree handle cannot be used after its store is closed, even when its
// transaction has ended.
use krummi::{Store, StoreOptions, TreeKind};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    let tree = read.open_tree("t", TreeKind::Normal).unwrap();
    drop(read);
    store.close().unwrap();
    drop(tree);
}
