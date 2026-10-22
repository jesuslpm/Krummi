// T8 / AC-T11: the store cannot be closed while a read transaction is open.
use muninndb::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let read = store.read().unwrap();
    store.close().unwrap();
    drop(read);
}
