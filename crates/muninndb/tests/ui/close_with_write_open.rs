// T8 / AC-T11: the store cannot be closed while the write transaction is open.
use muninndb::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let write = store.write().unwrap();
    store.close().unwrap();
    write.commit().unwrap();
}
