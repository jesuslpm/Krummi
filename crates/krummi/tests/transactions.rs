//! The public API of module 2: read and write transactions (Transactions PRD: T1, T4, T5, T6, T8, T10, T18;
//! AC-T6, AC-T8, AC-T9, AC-T10, AC-T11, AC-T22).

use std::thread;
use std::time::Duration;

use krummi::{Error, ErrorKind, ReadTransaction, Store, StoreOptions, WriteTransaction};

/// Compiles only when `T` is not `Send`: otherwise the call below is ambiguous between the two impls.
trait NotSend<A> {
    fn check() {}
}
impl<T: ?Sized> NotSend<()> for T {}
impl<T: ?Sized + Send> NotSend<u8> for T {}

/// Compiles only when `T` is not `Sync`.
trait NotSync<A> {
    fn check() {}
}
impl<T: ?Sized> NotSync<()> for T {}
impl<T: ?Sized + Sync> NotSync<u8> for T {}

fn stores(test: impl Fn(&Store)) {
    let tmp = tempfile::tempdir().unwrap();
    let on_disk = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    test(&on_disk);
    on_disk.close().unwrap();
    let in_memory = Store::open_temporary(&StoreOptions::default()).unwrap();
    test(&in_memory);
    in_memory.close().unwrap();
}

#[test]
fn ac_t11_transactions_open_commit_and_abort_and_the_store_closes_after() {
    stores(|store| {
        let read = store.read().unwrap();
        store.write().unwrap().commit().unwrap();
        store.write().unwrap().abort();
        drop(store.write().unwrap());
        store
            .write_with_timeout(Duration::ZERO)
            .unwrap()
            .commit()
            .unwrap();
        drop(read);
    });
}

#[test]
fn ac_t6_ac_t8_ac_t9_ac_t22_the_wait_errors_through_the_public_api() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    let write = store.write().unwrap();
    match store
        .write_with_timeout(Duration::from_secs(1))
        .err()
        .unwrap()
    {
        Error::Misuse { folder, .. } => assert_eq!(folder.as_deref(), Some(tmp.path())),
        other => panic!("expected misuse, got {other:?}"),
    }
    let store = &store;
    thread::scope(|s| {
        let limit = Duration::from_millis(20);
        let error = s
            .spawn(move || store.write_with_timeout(limit).err().unwrap())
            .join()
            .unwrap();
        match error {
            Error::Timeout { folder, limit: l } => {
                assert_eq!((folder.as_deref(), l), (Some(tmp.path()), limit));
            }
            other => panic!("expected a timeout, got {other:?}"),
        }
    });
    write.commit().unwrap();
    let error = store.write_with_timeout(Duration::MAX).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::Misuse);
    let day = Duration::from_secs(24 * 60 * 60);
    store.write_with_timeout(day).unwrap().commit().unwrap();
}

#[test]
fn ac_t10_read_transactions_cross_threads_and_write_transactions_do_not() {
    fn send_and_sync<T: Send + Sync>() {}
    send_and_sync::<ReadTransaction<'static>>();
    send_and_sync::<Store>();
    stores(|store| {
        let read = store.read().unwrap();
        thread::scope(|s| {
            for _ in 0..4 {
                s.spawn(|| store.read().map(drop).unwrap());
            }
            s.spawn(move || drop(read));
        });
    });
    <WriteTransaction<'static> as NotSend<_>>::check();
    <WriteTransaction<'static> as NotSync<_>>::check();
}

#[test]
fn ac_t10_ac_t11_what_must_not_compile() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}

/// J1 and J17 of the Journal PRD: a deferred commit and making what is committed durable, on disk and in a
/// temporary store; a clean close leaves only `krummi.data`.
#[test]
fn j1_deferred_commits_and_making_them_durable() {
    stores(|store| {
        store.write().unwrap().commit_deferred().unwrap();
        store.make_durable().unwrap();
        store.write().unwrap().commit().unwrap();
        store.make_durable().unwrap();
    });
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path(), &StoreOptions::default()).unwrap();
    store.write().unwrap().commit_deferred().unwrap();
    store.close().unwrap();
    let names: Vec<String> = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["krummi.data"]);
}
