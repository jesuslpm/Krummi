// AC-M9 (S9), M4, B1, R9: a multi-value tree opened in a read transaction can be sent to other threads and shared
// between them, like the transaction, and its key and value scans can be sent to another thread.
// Compile-only: `main` does not run the scenario.
use krummi::{ReadKeys, ReadMultiTree, ReadValues, Result, Store};

fn send_and_sync<T: Send + Sync>() {}
fn send<T: Send>() {}

fn scenario(store: &Store) -> Result<()> {
    let read = &store.read()?;
    let shared = read.open_multi_tree("a")?;
    let sent = read.open_multi_tree("b")?;
    let keys = read.keys(&shared, ..)?;
    let values = read.values(&shared, b"k", ..)?;
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| read.contains(&shared, b"k", b"v"));
        }
        s.spawn(move || read.len(&sent));
        s.spawn(move || keys.count());
        s.spawn(move || values.count());
    });
    Ok(())
}

fn main() {
    send_and_sync::<ReadMultiTree<'static>>();
    send::<ReadKeys<'static>>();
    send::<ReadValues<'static>>();
    let _ = scenario;
}
