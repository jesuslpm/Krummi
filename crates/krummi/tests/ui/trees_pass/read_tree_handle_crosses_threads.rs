// AC-A20 (S20), B1, T6: a tree opened in a read transaction can be sent to other threads and shared between
// them, like the transaction.
// Compile-only: `main` does not run the scenario.
use krummi::{ReadTree, Result, Store};

fn send_and_sync<T: Send + Sync>() {}

fn scenario(store: &Store) -> Result<()> {
    let read = &store.read()?;
    let shared = read.open_tree("a")?;
    let sent = read.open_tree("b")?;
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| read.get(&shared, b"k").map(|v| v.map(<[u8]>::len)));
        }
        s.spawn(move || read.len(&sent));
    });
    Ok(())
}

fn main() {
    send_and_sync::<ReadTree<'static>>();
    let _ = scenario;
}
