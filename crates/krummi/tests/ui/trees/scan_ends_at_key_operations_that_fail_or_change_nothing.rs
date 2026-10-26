// AC-R8 (S8), R6, A10: a scan of a write transaction stops being usable at the next set, try_add or remove, in
// that tree or in another one, even when the operation fails without effect or changes nothing.
use krummi::{Store, StoreOptions, TreeKind};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let a = write.open_tree("a", TreeKind::Normal).unwrap();
    let b = write.open_tree("b", TreeKind::Normal).unwrap();

    // A set that fails: the key is empty.
    let mut scan = write.range(&a, ..).unwrap();
    let _ = write.set(&a, b"", b"v");
    let _ = scan.next();

    // A try_add that does not add, in another tree.
    let mut scan = write.range(&a, ..).unwrap();
    let _ = write.try_add(&b, b"present", b"v");
    let _ = scan.next_back();

    // A remove that does not remove.
    let mut scan = write.range_prefix(&a, b"p", ..).unwrap();
    let _ = write.remove(&a, b"absent");
    let _ = scan.next();
}
