// AC-B11 (S29), B9, A10: a reference read from a tree stops being usable at the next set, try_add or remove, in
// that tree or in another one, even when the operation fails without effect or changes nothing.
use krummi::{Store, StoreOptions, TreeKind};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let a = write.open_tree("a", TreeKind::Normal).unwrap();
    let b = write.open_tree("b", TreeKind::Normal).unwrap();

    // A set that fails: the key is empty.
    let value = write.get(&a, b"k").unwrap();
    let _ = write.set(&a, b"", b"v");
    println!("{value:?}");

    // A try_add that does not add, in another tree.
    let value = write.get(&a, b"k").unwrap();
    let _ = write.try_add(&b, b"present", b"v");
    println!("{value:?}");

    // A remove that does not remove.
    let value = write.get(&a, b"k").unwrap();
    let _ = write.remove(&a, b"absent");
    println!("{value:?}");
}
