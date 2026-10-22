//! Transactions and threads (Transactions PRD: T6, T8; AC-T10, AC-T11, AC20).

mod common;

use std::thread;

use common::*;

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

#[test]
fn ac_t10_read_transactions_cross_threads_and_write_transactions_do_not() {
    fn send_and_sync<T: Send + Sync>() {}
    send_and_sync::<muninndb_tx::ReadTransaction<'static>>();
    send_and_sync::<muninndb_tx::Engine>();
    <muninndb_tx::WriteTransaction<'static> as NotSend<_>>::check();
    <muninndb_tx::WriteTransaction<'static> as NotSync<_>>::check();
}

#[test]
fn ac_t10_a_shared_read_shows_every_thread_the_same_snapshot() {
    both(|engine| {
        let page = new_page(engine, "a");
        let read = engine.read().unwrap();
        thread::scope(|s| {
            let read = &read;
            for i in 0..4 {
                s.spawn(move || {
                    for _ in 0..20 {
                        assert_eq!(text(read.page(page).unwrap()), "a");
                    }
                    i
                });
            }
            for text in ["b", "c", "d"] {
                set(engine, page, text);
            }
        });
        thread::scope(|s| {
            s.spawn(move || assert_eq!(text(read.page(page).unwrap()), "a"))
                .join()
                .unwrap();
        });
        assert_eq!(read_text(engine, page), "d");
    });
}

#[test]
fn ac20_ac_t11_page_references_that_must_not_compile() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
