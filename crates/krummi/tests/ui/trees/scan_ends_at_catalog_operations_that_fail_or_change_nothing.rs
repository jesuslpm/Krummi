// AC-R8 (S8), R6, A10: a scan of a write transaction stops being usable at opening a tree in a mode other than
// Open, at delete_tree and at rename_tree, even when they fail or change nothing.
use krummi::{OpenMode, Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let a = write.open_tree("a").unwrap();

    let mut scan = write.range(&a, ..).unwrap();
    let _ = write.open_tree_with("a", OpenMode::OpenOrCreate);
    let _ = scan.next();

    let mut scan = write.range(&a, ..).unwrap();
    let _ = write.delete_tree("missing");
    let _ = scan.next();

    let mut scan = write.range(&a, ..).unwrap();
    let _ = write.rename_tree("a", "a", false);
    let _ = scan.next();
}
