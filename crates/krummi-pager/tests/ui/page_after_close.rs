// AC20: a page reference cannot be used after the store is closed.
use krummi_pager::{OpenOptions, Pager};

fn main() {
    let pager = Pager::open_temporary(&OpenOptions::default()).unwrap();
    let page = pager.page(2).unwrap();
    pager.close().unwrap();
    println!("{}", page.len());
}
