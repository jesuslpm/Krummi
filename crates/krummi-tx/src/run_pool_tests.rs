//! M1 of the 2026-11-15 performance diagnostic: a commit keeps the buffers of its value runs for the runs of the next
//! transactions, up to 32 MiB per store on disk, zeroed when reused, instead of freeing them and zeroing new ones.

use std::io;

use krummi_pager::ErrorKind;
use krummi_pager::format::{PAGE_HEADER_SIZE, PAGE_SIZE};
use krummi_pager::testing::{self, Action, Op};

use crate::{Engine, OpenOptions};

/// Commits a value run of `pages` pages filled with `byte`, and returns its first page.
fn commit_run(engine: &Engine, pages: u64, byte: u8) -> u64 {
    let mut tx = engine.write(None).unwrap();
    let (first, content) = tx.allocate_value_run(pages).unwrap();
    content.fill(byte);
    tx.commit().unwrap();
    first
}

#[test]
fn a_commit_keeps_its_run_buffers_and_the_next_run_gets_one_zeroed() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Engine::open(tmp.path(), &OpenOptions::default()).unwrap();
    let old = commit_run(&engine, 4, 7);
    assert_eq!(engine.spare_run_bytes(), 4 * PAGE_SIZE);
    // A read that sees the committed run keeps seeing its bytes while the buffer serves the next run.
    let read = engine.read().unwrap();
    let mut tx = engine.write(None).unwrap();
    let (new, content) = tx.allocate_value_run(4).unwrap();
    assert_eq!(engine.spare_run_bytes(), 0, "the run took the kept buffer");
    assert!(content.iter().all(|&b| b == 0), "a reused buffer is zeroed");
    content.fill(9);
    // A run of another length does not take a kept buffer.
    tx.allocate_value_run(2).unwrap();
    tx.commit().unwrap();
    assert!(read.run(old, 4).unwrap().iter().all(|&b| b == 7));
    drop(read);
    let read = engine.read().unwrap();
    assert!(read.run(old, 4).unwrap().iter().all(|&b| b == 7));
    assert!(read.run(new, 4).unwrap().iter().all(|&b| b == 9));
    assert_eq!(
        read.run(new, 4).unwrap().len(),
        4 * PAGE_SIZE - PAGE_HEADER_SIZE
    );
    drop(read);
    assert_eq!(engine.spare_run_bytes(), 6 * PAGE_SIZE);
    engine.close().unwrap();
}

/// At most 32 MiB of buffers: a run that does not fit in what is left is freed, and so is a larger one.
#[test]
fn a_store_keeps_at_most_32_mib_of_run_buffers() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Engine::open(tmp.path(), &OpenOptions::default()).unwrap();
    let pages_of = |mib: usize| (mib << 20).div_ceil(PAGE_SIZE) as u64;
    let mut tx = engine.write(None).unwrap();
    for mib in [20, 20, 40] {
        tx.allocate_value_run(pages_of(mib)).unwrap();
    }
    tx.allocate_value_run(pages_of(10)).unwrap();
    tx.commit().unwrap();
    // The runs are committed by first page: what fits of 20, 20, 40 and 10 MiB in that order is kept.
    let kept = engine.spare_run_bytes();
    assert!(kept <= 32 << 20, "{kept} bytes kept");
    assert_eq!(kept, (pages_of(20) + pages_of(10)) as usize * PAGE_SIZE);
    engine.close().unwrap();
}

/// A temporary store keeps none: all its memory counts against its maximum size (P10, T14).
#[test]
fn a_temporary_store_keeps_no_run_buffers() {
    let engine = Engine::open_temporary(&OpenOptions::default()).unwrap();
    commit_run(&engine, 3, 1);
    assert_eq!(engine.spare_run_bytes(), 0);
}

/// Reusing a buffer is the allocation's fault point (P13): it fails with out of memory where a new run would, frees
/// the buffer, and the next write works.
#[test]
fn reusing_a_run_buffer_fails_where_a_new_run_would() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Engine::open(tmp.path(), &OpenOptions::default()).unwrap();
    // Each allocation of reserving the run fails in turn, until the reservation passes them all.
    let mut reuse_failed = false;
    for failures in 0.. {
        if engine.spare_run_bytes() == 0 {
            commit_run(&engine, 2, 5);
        }
        let mut tx = engine.write(None).unwrap();
        testing::reset();
        testing::inject(
            Op::Alloc,
            testing::count(Op::Alloc) + failures,
            Action::Fail(io::ErrorKind::OutOfMemory),
        );
        let reserved = tx
            .allocate_value_run(2)
            .map(|(_, content)| content.to_vec());
        testing::reset();
        match reserved {
            Ok(content) => {
                assert!(content.iter().all(|&b| b == 0));
                break;
            }
            Err(error) => {
                assert_eq!(error.kind(), ErrorKind::OutOfMemory);
                drop(tx);
                // The failure at the reuse took the kept buffer and freed it.
                reuse_failed |= engine.spare_run_bytes() == 0;
            }
        }
    }
    assert!(reuse_failed, "reusing the buffer is a fault point");
    let first = commit_run(&engine, 2, 6);
    let read = engine.read().unwrap();
    assert!(read.run(first, 2).unwrap().iter().all(|&b| b == 6));
}
