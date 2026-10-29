// AC-B10 (S28), B9, A14: a write transaction keeps several tree handles open at once, two of them of the same
// tree, while it calls operations that modify the transaction, and every handle stays usable.
// Compile-only: `main` does not run the scenario.
use krummi::{OpenMode, Result, Store};

fn scenario(store: &Store) -> Result<()> {
    let mut write = store.write()?;
    let a = write.open_tree_with("a", OpenMode::CreateNew)?;
    let b = write.open_tree_with("b", OpenMode::OpenOrCreate)?;
    let a_again = write.open_tree("a")?;
    write.set(&a, b"k", b"1")?;
    write.set(&a_again, b"k", b"2")?;
    let added = write.try_add(&b, b"k", b"3")?;
    let removed = write.remove(&a_again, b"k")?;
    let c = write.open_tree_with("c", OpenMode::CreateOrEmpty)?;
    write.rename_tree("c", "d", false)?;
    write.delete_tree("d")?;
    let b_emptied = write.open_tree_with("b", OpenMode::Empty)?;
    let counts = [
        write.len(&a)?,
        write.len(&a_again)?,
        write.len(&b)?,
        write.len(&b_emptied)?,
        write.len(&c)?,
    ];
    println!("{added} {removed} {counts:?}");
    write.commit()
}

fn main() {
    let _ = scenario;
}
