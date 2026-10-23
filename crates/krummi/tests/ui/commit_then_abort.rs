// T8 / AC-T11: a committed transaction cannot be used again, for instance to abort it.
use krummi::{Store, StoreOptions};

fn main() {
    let store = Store::open_temporary(&StoreOptions::default()).unwrap();
    let write = store.write().unwrap();
    write.commit().unwrap();
    write.abort();
}
