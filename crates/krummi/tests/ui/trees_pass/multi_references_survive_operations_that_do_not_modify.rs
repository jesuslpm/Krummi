// AC-M9 (S9), M9, R6: in a write transaction, a key and a value returned by the scans of a multi-value tree, and the
// scans themselves, stay usable after contains, value_count, len, new scans and opening a tree in mode Open; and in a
// read transaction too.
// Compile-only: `main` does not run the scenario.
use krummi::{Result, Store};

fn scenario(store: &Store) -> Result<()> {
    let write = store.write()?;
    let a = write.open_multi_tree("a")?;
    let mut keys = write.keys(&a, ..)?;
    let mut values = write.values(&a, b"k", ..)?;
    let value = values.next();
    let key = keys.next();
    let found = write.contains(&a, b"k", b"v")?;
    let count = write.value_count(&a, b"k")?;
    let entries = write.len(&a)?;
    let b = write.open_multi_tree("b")?;
    let other = write.keys_prefix(&b, b"p", ..)?.next();
    let more = write.values_prefix(&b, b"k", b"p", ..)?.next_back();
    let last = values.next_back();
    let next = keys.next_back();
    println!("{value:?} {key:?} {found} {count} {entries} {other:?} {more:?} {last:?} {next:?}");
    drop((keys, values));
    write.commit()?;

    let read = store.read()?;
    let a = read.open_multi_tree("a")?;
    let value = read.values(&a, b"k", ..)?.next();
    let found = read.contains(&a, b"k", b"v")?;
    let count = read.value_count(&a, b"k")?;
    let key = read.keys(&a, ..)?.next();
    println!("{value:?} {found} {count} {key:?}");
    Ok(())
}

fn main() {
    let _ = scenario;
}
