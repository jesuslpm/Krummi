// AC-X7 (S7), AC-X20 (S20), X8: in a write transaction, a value read from a fixed-size tree and a scan of it stay
// usable after get_fixed, len, new scans, opening a tree in mode Open, tree_exists, tree_kind and list_trees; and in a
// read transaction too. A reference read from a normal tree stays usable after tree_kind as well.
// Compile-only: `main` does not run the scenario.
use krummi::{Result, Store};

fn scenario(store: &Store) -> Result<()> {
    let write = store.write()?;
    let a = write.open_fixed_tree("a", 8)?;
    let mut scan = write.range_fixed(&a, ..)?;
    let value = write.get_fixed(&a, 1)?;
    let first = scan.next();
    let other = write.get_fixed(&a, 2)?;
    let entries = write.len(&a)?;
    let b = write.open_fixed_tree("b", 0)?;
    let more = write.range_fixed(&b, ..=7)?.next_back();
    let normal = write.open_tree("n")?;
    let bytes = write.get(&normal, b"k")?;
    let kind = write.tree_kind("a")?;
    let exists = write.tree_exists("c")?;
    let trees = write.list_trees()?;
    let last = scan.next_back();
    println!("{value:?} {first:?} {other:?} {entries} {more:?} {bytes:?} {kind:?} {exists} {trees:?} {last:?}");
    drop(scan);
    write.commit()?;

    let read = store.read()?;
    let a = read.open_fixed_tree("a", 8)?;
    let mut scan = read.range_fixed(&a, 3..)?;
    let value = read.get_fixed(&a, 1)?;
    let kind = read.tree_kind("a")?;
    let entries = read.len(&a)?;
    let first = scan.next();
    println!("{value:?} {kind:?} {entries} {first:?}");
    Ok(())
}

fn main() {
    let _ = scenario;
}
