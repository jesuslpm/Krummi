// AC-A20 (S20), A10: a reference read from a tree stays usable after opening a tree in mode Open, tree_exists,
// list_trees, get and len, in a write transaction and in a read transaction.
// Compile-only: `main` does not run the scenario.
use krummi::{Result, Store};

fn scenario(store: &Store) -> Result<()> {
    let write = store.write()?;
    let a = write.open_tree("a")?;
    let value = write.get(&a, b"k")?;
    let b = write.open_tree("b")?;
    let exists = write.tree_exists("c")?;
    let trees = write.list_trees()?;
    let other = write.get(&b, b"k")?;
    let count = write.len(&a)?;
    println!("{value:?} {exists} {trees:?} {other:?} {count}");
    write.commit()?;

    let read = store.read()?;
    let a = read.open_tree("a")?;
    let value = read.get(&a, b"k")?;
    let b = read.open_tree("b")?;
    let exists = read.tree_exists("c")?;
    let trees = read.list_trees()?;
    let other = read.get(&b, b"k")?;
    let count = read.len(&a)?;
    println!("{value:?} {exists} {trees:?} {other:?} {count}");
    Ok(())
}

fn main() {
    let _ = scenario;
}
