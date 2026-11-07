//! Candidate 4 of the writes diagnosis (performance plan): a commit keeps its page payloads for the copies of the
//! next transactions, instead of freeing them and zeroing new ones.

use crate::{Engine, OpenOptions};

#[test]
fn a_commit_keeps_its_page_payloads_for_the_next_copies() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Engine::open(tmp.path(), &OpenOptions::default()).unwrap();
    let mut tx = engine.write(None).unwrap();
    let pages: Vec<u64> = (0..3).map(|_| tx.allocate().unwrap()).collect();
    tx.commit().unwrap();
    let kept = engine.spare_payloads();
    assert!(kept >= pages.len(), "{kept} payloads kept");
    let mut tx = engine.write(None).unwrap();
    tx.page_mut(pages[0]).unwrap()[0] = 9;
    assert!(
        engine.spare_payloads() < kept,
        "the copy took a kept payload"
    );
    assert_eq!(tx.page(pages[1]).unwrap()[0], 0);
    tx.commit().unwrap();
    assert!(engine.spare_payloads() >= kept);
    let tx = engine.write(None).unwrap();
    assert_eq!(tx.page(pages[0]).unwrap()[0], 9);
    drop(tx);
    engine.close().unwrap();
}

/// A temporary store keeps none: all its memory counts against its maximum size (P10, T14).
#[test]
fn a_temporary_store_keeps_no_page_payloads() {
    let engine = Engine::open_temporary(&OpenOptions::default()).unwrap();
    let mut tx = engine.write(None).unwrap();
    tx.allocate().unwrap();
    tx.commit().unwrap();
    assert_eq!(engine.spare_payloads(), 0);
}
