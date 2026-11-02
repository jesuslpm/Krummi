// AC-X7 (S7), X8: a value read from a fixed-size tree in a write transaction, and a scan of it, stop being usable at
// the next operation that may modify, through another handle of the tree too, even when it fails without effect or
// changes nothing: set and add of a value of another length, an add of a key that is there, a remove of a key that is
// not there, a range removal that removes nothing, and opening with a length over 2,048 in a creating mode.
use krummi::{OpenMode, Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let f = write.open_fixed_tree("f", 8).unwrap();
    let other = write.open_fixed_tree("f", 8).unwrap();

    // A set that fails: the value has another length.
    let value = write.get_fixed(&f, 1).unwrap();
    let _ = write.set_fixed(&f, 1, b"short");
    println!("{value:?}");

    // An add that does not add, through another handle.
    let mut scan = write.range_fixed(&f, ..).unwrap();
    let _ = write.try_add_fixed(&other, 1, b"value 08");
    let _ = scan.next();

    // An add that fails.
    let (_, value) = write.range_fixed(&f, ..).unwrap().next().unwrap().unwrap();
    let _ = write.try_add_fixed(&f, 1, b"");
    println!("{value:?}");

    // A remove that does not remove.
    let mut scan = write.range_fixed(&f, 5..).unwrap();
    let _ = write.remove_fixed(&other, 99);
    let _ = scan.next_back();

    // A range removal of an empty interval.
    let value = write.get_fixed(&f, 1).unwrap();
    let _ = write.remove_range_fixed(&f, 8..5);
    println!("{value:?}");

    // An opening with a length over 2,048, which fails before reading anything.
    let mut scan = write.range_fixed(&f, ..).unwrap();
    let _ = write.open_fixed_tree_with("f", 2049, OpenMode::OpenOrCreate);
    let _ = scan.next();
}
