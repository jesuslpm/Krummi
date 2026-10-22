// T8 / AC20: a page reference cannot outlive its write transaction.
use muninndb_tx::{Engine, OpenOptions};

fn main() {
    let engine = Engine::open_temporary(&OpenOptions::default()).unwrap();
    let mut write = engine.write(None).unwrap();
    let page = write.allocate().unwrap();
    let payload = write.page(page).unwrap();
    write.commit().unwrap();
    println!("{}", payload[0]);
}
