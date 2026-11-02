// AC-X7 (S7), X4: the handles of a normal tree and of a multi-value tree take none of the operations of fixed-size
// trees, in a write transaction and in a read transaction.
use krummi::{OpenMode, Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let normal = write.open_tree_with("n", OpenMode::OpenOrCreate).unwrap();
    let multi = write.open_multi_tree_with("m", OpenMode::OpenOrCreate).unwrap();
    let _ = write.get_fixed(&normal, 1);
    let _ = write.set_fixed(&normal, 1, b"v");
    let _ = write.try_add_fixed(&normal, 1, b"v");
    let _ = write.remove_fixed(&normal, 1);
    let _ = write.range_fixed(&normal, ..);
    let _ = write.remove_range_fixed(&normal, ..);
    let _ = write.get_fixed(&multi, 1);
    let _ = write.set_fixed(&multi, 1, b"v");
    let _ = write.try_add_fixed(&multi, 1, b"v");
    let _ = write.remove_fixed(&multi, 1);
    let _ = write.range_fixed(&multi, ..);
    let _ = write.remove_range_fixed(&multi, ..);
    write.commit().unwrap();

    let read = store.read().unwrap();
    let normal = read.open_tree("n").unwrap();
    let multi = read.open_multi_tree("m").unwrap();
    let _ = read.get_fixed(&normal, 1);
    let _ = read.range_fixed(&normal, ..);
    let _ = read.get_fixed(&multi, 1);
    let _ = read.range_fixed(&multi, ..);
}
