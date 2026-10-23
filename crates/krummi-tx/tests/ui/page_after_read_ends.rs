// T8 / AC20: a page reference cannot outlive its read transaction.
use krummi_tx::{Engine, OpenOptions};

fn main() {
    let engine = Engine::open_temporary(&OpenOptions::default()).unwrap();
    let read = engine.read().unwrap();
    let page = read.page(3).unwrap();
    drop(read);
    println!("{}", page.len());
}
