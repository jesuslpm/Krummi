// T8 / AC-T11: inside a write, a page reference cannot be used after another change of the same transaction.
use muninndb_tx::{Engine, OpenOptions};

fn main() {
    let engine = Engine::open_temporary(&OpenOptions::default()).unwrap();
    let mut write = engine.write(None).unwrap();
    let page = write.allocate().unwrap();
    let before = write.page(page).unwrap();
    write.page_mut(page).unwrap()[0] = 1;
    println!("{}", before[0]);
}
