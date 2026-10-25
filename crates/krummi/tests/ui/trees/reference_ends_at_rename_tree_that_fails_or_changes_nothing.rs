// AC-A20 (S20), AC-B11 (S29), A10: a reference read from a tree stops being usable when a tree is renamed, even
// when the rename fails without effect or renames a tree to its own name.
use krummi::{Store, StoreOptions, TreeKind};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let tree = write.open_tree("t", TreeKind::Normal).unwrap();

    let value = write.get(&tree, b"k").unwrap();
    let _ = write.rename_tree("missing", "other", false);
    println!("{value:?}");

    let value = write.get(&tree, b"k").unwrap();
    let _ = write.rename_tree("t", "t", true);
    println!("{value:?}");
}
