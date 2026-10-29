// AC-R8 (S8), R6, R9, A10: a scan of a write transaction stays usable after get, opening a tree in mode Open,
// tree_exists, list_trees, len and another scan; a key returned by a scan of a read transaction outlives the
// scan; and a scan of a read transaction can be sent to another thread.
// Compile-only: `main` does not run the scenario.
use krummi::{ReadRange, Result, Store};

fn send<T: Send>() {}

fn scenario(store: &Store) -> Result<()> {
    let write = store.write()?;
    let a = write.open_tree("a")?;
    let mut scan = write.range(&a, ..)?;
    let first = scan.next();
    let value = write.get(&a, b"k")?;
    let b = write.open_tree("b")?;
    let exists = write.tree_exists("c")?;
    let trees = write.list_trees()?;
    let count = write.len(&b)?;
    let mut other = write.range_prefix(&b, b"p", ..)?;
    let last = scan.next_back();
    println!("{first:?} {value:?} {exists} {trees:?} {count} {:?} {last:?}", other.next());
    drop(scan);
    write.commit()?;

    let read = store.read()?;
    let a = read.open_tree("a")?;
    let scan = read.range(&a, &b"a"[..]..=&b"z"[..])?;
    let keys: Vec<&[u8]> = scan.map(|entry| entry.map(|(key, _)| key)).collect::<Result<_>>()?;
    std::thread::scope(|s| {
        let sent = read.range(&a, ..).unwrap();
        s.spawn(move || sent.count());
    });
    println!("{keys:?}");
    Ok(())
}

fn main() {
    send::<ReadRange<'static>>();
    let _ = scenario;
}
