//! The bytes a commit writes stay those of the stored format (O5 of the performance plan reworked how the commit
//! builds them): the journal's writes and the data file after the close, hashed, recorded before the change.

use krummi_pager::testing::{self, Recorded};

use crate::{Engine, OpenOptions, PAYLOAD_SIZE};

/// FNV-1a of `bytes`, folded into `hash`: a hash fixed here, independent of the engine's own.
fn fnv(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

const FNV_START: u64 = 0xcbf2_9ce4_8422_2325;

/// Payload bytes that compress fully, half or not at all, the same on every target.
fn payload(kind: usize, seed: u64) -> Vec<u8> {
    let mut x = 0x9E37_79B9_7F4A_7C15u64 ^ seed;
    let mut noise = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x as u8
    };
    (0..PAYLOAD_SIZE)
        .map(|i| match kind {
            0 => (i % 7) as u8,
            1 if i % 2 == 0 => noise(),
            1 => 0,
            _ => noise(),
        })
        .collect()
}

/// Single pages of every kind, a value run, a page changed again and a group of three deferred commits written
/// together: the journal's writes and the data file keep their bytes.
#[test]
fn commits_keep_the_bytes_of_the_journal_and_the_data_file() {
    let tmp = tempfile::tempdir().unwrap();
    testing::reset();
    testing::start_recording();
    let engine = Engine::open(tmp.path(), &OpenOptions::default()).unwrap();
    let mut tx = engine.write(None).unwrap();
    let mut pages = Vec::new();
    for kind in 0..3 {
        let page = tx.allocate().unwrap();
        tx.page_mut(page)
            .unwrap()
            .copy_from_slice(&payload(kind, page));
        pages.push(page);
    }
    tx.commit().unwrap();
    let mut tx = engine.write(None).unwrap();
    let (_, run) = tx.allocate_value_run(3).unwrap();
    for (i, b) in run.iter_mut().enumerate() {
        *b = (i / 300) as u8 ^ (i % 11) as u8;
    }
    tx.page_mut(pages[0]).unwrap()[..5].copy_from_slice(b"hello");
    tx.commit().unwrap();
    engine.set_journal_paused(true);
    for (i, &page) in pages.iter().enumerate() {
        let mut tx = engine.write(None).unwrap();
        tx.page_mut(page).unwrap()[100..108].copy_from_slice(&(i as u64).to_le_bytes());
        tx.commit_deferred().unwrap();
    }
    engine.set_journal_paused(false);
    engine.make_durable().unwrap();
    engine.close().unwrap();
    let writes: Vec<(u64, Vec<u8>)> = testing::take_recording()
        .into_iter()
        .filter_map(|op| match op {
            Recorded::Write(path, offset, bytes)
                if path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("krummi.journal.")) =>
            {
                Some((offset, bytes))
            }
            _ => None,
        })
        .collect();
    let journal = writes.iter().fold(FNV_START, |hash, (offset, bytes)| {
        fnv(fnv(hash, &offset.to_le_bytes()), bytes)
    });
    // Pages 0 and 2 vary by a few bytes from run to run, also before O5, so every page but those two is pinned.
    let file = std::fs::read(tmp.path().join("krummi.data")).unwrap();
    let page = krummi_pager::format::PAGE_SIZE;
    let data = fnv(fnv(FNV_START, &file[page..2 * page]), &file[3 * page..]);
    testing::reset();
    // One write per group: the two durable commits, then the three deferred ones together.
    let sizes: Vec<usize> = writes.iter().map(|(_, bytes)| bytes.len()).collect();
    assert_eq!(sizes, [20480, 4096, 12288]);
    // The pages after the header and the meta page are those of format 9: the journal page diffs changed the journal
    // and the format version in the header, not what commits leave in `krummi.data` (Y1 of their PRD).
    assert_eq!(fnv(FNV_START, &file[3 * page..]), 0xef70_dc69_e134_e0ee);
    assert_eq!(
        (journal, data),
        (0x426c_c654_e287_875f, 0x98f6_6bda_6608_030b)
    );
}
