// AC-X7 (S7), X4, B1, R9: a fixed-size tree opened in a read transaction can be sent to other threads and shared
// between them, like the transaction, and its scans can be sent to another thread.
// Compile-only: `main` does not run the scenario.
use krummi::{ReadFixedRange, ReadFixedTree, Result, Store};

fn send_and_sync<T: Send + Sync>() {}
fn send<T: Send>() {}

fn scenario(store: &Store) -> Result<()> {
    let read = &store.read()?;
    let shared = read.open_fixed_tree("a", 8)?;
    let sent = read.open_fixed_tree("b", 0)?;
    let scan = read.range_fixed(&shared, 5..)?;
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| read.get_fixed(&shared, 1));
        }
        s.spawn(move || read.len(&sent));
        s.spawn(move || scan.count());
    });
    Ok(())
}

fn main() {
    send_and_sync::<ReadFixedTree<'static>>();
    send::<ReadFixedRange<'static>>();
    let _ = scenario;
}
