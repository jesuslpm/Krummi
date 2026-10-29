// AC-A20 (S20), A10: a reference read from a tree stops being usable when a tree is opened in any mode other
// than Open, even when the tree exists and nothing changes.
use krummi::{OpenMode, Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let tree = write
        .open_tree_with("t", OpenMode::OpenOrCreate)
        .unwrap();
    for mode in [
        OpenMode::CreateNew,
        OpenMode::OpenOrCreate,
        OpenMode::CreateOrEmpty,
        OpenMode::Empty,
    ] {
        let value = write.get(&tree, b"k").unwrap();
        let _ = write.open_tree_with("t", mode);
        println!("{value:?}");
    }
}
