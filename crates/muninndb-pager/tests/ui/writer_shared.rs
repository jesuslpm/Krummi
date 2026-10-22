// T6: the scratch writer stays on its thread and cannot be shared, so no two flushes overlap.
use muninndb_pager::{OpenOptions, Pager, Scratch};

fn main() {
    let scratch = Scratch::new(Pager::open_temporary(&OpenOptions::default()).unwrap());
    let writer = scratch.begin_write().unwrap();
    std::thread::scope(|s| {
        s.spawn(|| writer.snapshot().txid());
    });
}
