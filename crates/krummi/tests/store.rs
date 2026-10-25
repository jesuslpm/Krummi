//! The public API of module 1: open, inspect and close stores (Pager PRD, P1 and P12).

use krummi::{ErrorKind, Store, StoreOptions};

#[test]
fn a_store_is_created_inspected_closed_and_reopened() {
    let tmp = tempfile::tempdir().unwrap();
    let folder = tmp.path().join("orders");
    let store = Store::open(&folder, &StoreOptions::default()).unwrap();
    let info = store.info();
    assert_eq!(
        (info.format_version, info.page_size, info.temporary),
        (6, 8192, false)
    );
    assert_eq!(info.page_count * 8192, info.size_bytes);
    assert_eq!(
        Store::open(&folder, &StoreOptions::default())
            .err()
            .unwrap()
            .kind(),
        ErrorKind::StoreInUse
    );
    store.close().unwrap();
    let store = Store::open(&folder, &StoreOptions::default()).unwrap();
    assert_eq!(store.info(), info);
    store.close().unwrap();
}

#[test]
fn a_temporary_store_reports_itself_as_temporary() {
    let store = Store::open_temporary(&StoreOptions {
        max_size: Some(2 << 20),
    })
    .unwrap();
    let info = store.info();
    assert!(info.temporary);
    assert_eq!(
        (info.format_version, info.page_size, info.max_size),
        (6, 8192, Some(2 << 20))
    );
    store.close().unwrap();
}
