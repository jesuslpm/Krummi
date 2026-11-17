//! A write transaction changes a page in a free slot of the temporary area, which the commit seals in place, instead
//! of a payload in memory that the commit copies into a slot (S2 of the night optimizations: one copy less per changed
//! page). The stored bytes, the errors and what every other transaction sees stay as they were; without a free slot
//! the page is changed in memory as before.

mod common;

use std::fs;
use std::path::Path;

use common::{disk, put, read_text, text};
use krummi_pager::format::{HEADER_PAGES, PAGE_PAYLOAD_SIZE, PAGE_SIZE};
use krummi_pager::testing::{self, Action, Op};
use krummi_pager::{DATA_FILE, JOURNAL_PREFIX, Memory};
use krummi_tx::{Engine, ErrorKind};

/// SplitMix-like xorshift: a small, deterministic generator.
struct Rng(u64);

impl Rng {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % bound as u64) as usize
    }
}

/// Commits `count` new pages holding their own texts, and bytes that are not zeros at their end, transfers them and
/// frees their versions, so that the temporary area has free slots that held them, and returns the pages.
fn with_free_slots(engine: &Engine, count: usize) -> Vec<u64> {
    let mut tx = engine.write(None).unwrap();
    let pages: Vec<u64> = (0..count).map(|_| tx.allocate().unwrap()).collect();
    for (i, &page) in pages.iter().enumerate() {
        put(&mut tx, page, &format!("old {i}"));
        tx.page_mut(page).unwrap()[PAGE_PAYLOAD_SIZE - 100..].fill(0xaa);
    }
    tx.commit().unwrap();
    assert!(engine.transfer_now());
    pages
}

/// A seeded workload of copied, created, freed and reused pages, value runs and meta slots, with transfers between
/// commits that free slots of the temporary area. Returns how many pages it changed in a slot of the temporary area.
fn workload(engine: &Engine, seed: u64) -> usize {
    let mut rng = Rng(seed);
    let mut pages: Vec<u64> = Vec::new();
    let mut in_slots = 0;
    for commit in 0..40 {
        let mut tx = engine.write(None).unwrap();
        let mut changed = Vec::new();
        for _ in 0..1 + rng.below(12) {
            match rng.below(10) {
                0..=2 => {
                    let page = tx.allocate().unwrap();
                    let payload = tx.page_mut(page).unwrap();
                    if rng.below(4) != 0 {
                        let at = rng.below(PAGE_PAYLOAD_SIZE - 64);
                        payload[at..at + 1 + rng.below(63)].fill(1 + rng.below(255) as u8);
                    }
                    pages.push(page);
                    changed.push(page);
                }
                3..=6 if !pages.is_empty() => {
                    let page = pages[rng.below(pages.len())];
                    let payload = tx.page_mut(page).unwrap();
                    let at = rng.below(PAGE_PAYLOAD_SIZE - 300);
                    payload[at..at + 1 + rng.below(299)].fill(1 + rng.below(255) as u8);
                    changed.push(page);
                }
                7 if pages.len() > 2 => {
                    let page = pages.swap_remove(rng.below(pages.len()));
                    tx.free(page, 1).unwrap();
                    changed.retain(|&p| p != page);
                }
                8 => {
                    let count = 1 + rng.below(3) as u64;
                    let (_, content) = tx.allocate_value_run(count).unwrap();
                    content[..8].copy_from_slice(&(commit as u64).to_le_bytes());
                }
                _ => tx.set_slot(rng.below(4), rng.below(1000) as u64).unwrap(),
            }
        }
        changed.sort_unstable();
        changed.dedup();
        in_slots += changed
            .iter()
            .filter(|&&page| engine.memory_of(tx.page(page).unwrap()) == Some(Memory::Versions))
            .count();
        tx.commit().unwrap();
        if rng.below(3) == 0 {
            assert!(engine.transfer_now());
        }
    }
    in_slots
}

/// Files by name, with their bytes.
type Files = Vec<(String, Vec<u8>)>;

/// The journal files of the store in `folder`.
fn journal_files(folder: &Path) -> Files {
    let mut files: Files = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(JOURNAL_PREFIX)
        })
        .map(|entry| {
            let name = entry.file_name().into_string().unwrap();
            (name, fs::read(entry.path()).unwrap())
        })
        .collect();
    files.sort();
    files
}

/// Runs the workload with seed `seed`, page slots on or off, and returns the journal files once every commit is
/// durable, the store's pages after the close, and how many pages it changed in slots.
fn stored_bytes(seed: u64, slots: bool) -> (Files, Vec<u8>, usize) {
    let tmp = tempfile::tempdir().unwrap();
    testing::set_page_slots(slots);
    let engine = disk(tmp.path());
    let in_slots = workload(&engine, seed);
    let journal = journal_files(tmp.path());
    engine.close().unwrap();
    testing::set_page_slots(true);
    let data = fs::read(tmp.path().join(DATA_FILE)).unwrap();
    (
        journal,
        data[HEADER_PAGES as usize * PAGE_SIZE..].to_vec(),
        in_slots,
    )
}

/// S2: the journal records and the store's pages are the same bytes whether the pages were changed in slots of the
/// temporary area or in memory.
#[test]
fn pages_changed_in_slots_store_the_same_bytes() {
    for seed in [0x9e37_79b9_7f4a_7c15, 0x2545_f491_4f6c_dd1d, 7] {
        let (journal, data, none) = stored_bytes(seed, false);
        let (slot_journal, slot_data, in_slots) = stored_bytes(seed, true);
        assert_eq!(
            none, 0,
            "seed {seed:#x}: no page in a slot when they are off"
        );
        assert!(in_slots > 0, "seed {seed:#x}: some pages changed in slots");
        assert!(!journal.is_empty());
        assert_eq!(
            journal.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            slot_journal
                .iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>(),
            "seed {seed:#x}"
        );
        for ((name, bytes), (_, slot_bytes)) in journal.iter().zip(&slot_journal) {
            assert!(
                bytes == slot_bytes,
                "seed {seed:#x}: journal file {name} differs"
            );
        }
        assert!(
            data == slot_data,
            "seed {seed:#x}: the store's pages differ"
        );
    }
}

/// S2: with a free slot in the temporary area, the first change of a page copies it there; the commit publishes that
/// slot, sealed, as the page's new version, without another copy.
#[test]
fn a_copied_page_is_changed_in_a_free_slot_and_published_from_it() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let pages = with_free_slots(&engine, 4);
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, pages[1], "new 1");
    let own = tx.page(pages[1]).unwrap();
    assert_eq!(engine.memory_of(own), Some(Memory::Versions));
    assert_eq!(text(own), "new 1");
    let at = own.as_ptr();
    tx.commit().unwrap();
    let read = engine.read().unwrap();
    assert_eq!(text(read.page(pages[1]).unwrap()), "new 1");
    assert_eq!(
        read.page(pages[1]).unwrap().as_ptr(),
        at,
        "published from its slot"
    );
    assert_eq!(text(read.page(pages[0]).unwrap()), "old 0");
    drop(read);
    engine.close().unwrap();
    let engine = disk(tmp.path());
    assert_eq!(read_text(&engine, pages[1]), "new 1");
    engine.close().unwrap();
}

/// S2: a page the transaction creates in a slot that held an older version reads as zeros, as a new payload does.
#[test]
fn a_page_created_in_a_reused_slot_reads_as_zeros() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let pages = with_free_slots(&engine, 4);
    let mut tx = engine.write(None).unwrap();
    // The slots are taken in order, and the first two held the meta and map pages: these changes take them, and the
    // meta page's copy takes a third, so the new page takes one that held a page with bytes that are not zeros.
    put(&mut tx, pages[0], "x");
    put(&mut tx, pages[1], "y");
    let page = tx.allocate().unwrap();
    let created = tx.page(page).unwrap();
    assert_eq!(engine.memory_of(created), Some(Memory::Versions));
    assert!(created.iter().all(|&b| b == 0));
    tx.commit().unwrap();
    assert!(
        engine
            .read()
            .unwrap()
            .page(page)
            .unwrap()
            .iter()
            .all(|&b| b == 0)
    );
    engine.close().unwrap();
}

/// S2: without a free slot (a new store's temporary area has none) a page is changed in memory, as before.
#[test]
fn without_a_free_slot_a_page_is_changed_in_memory() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let mut tx = engine.write(None).unwrap();
    let page = tx.allocate().unwrap();
    put(&mut tx, page, "first");
    assert_eq!(engine.memory_of(tx.page(page).unwrap()), None);
    tx.commit().unwrap();
    assert_eq!(read_text(&engine, page), "first");
    engine.close().unwrap();
}

/// S2: a failure of memory at the first change of a page fails it with the same error whether a free slot was there
/// or not, and the transaction then fails its commit with it (T11).
#[test]
fn a_memory_failure_fails_a_change_the_same_way_with_or_without_a_slot() {
    let mut errors = Vec::new();
    for slots in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        testing::set_page_slots(slots);
        let engine = disk(tmp.path());
        let pages = with_free_slots(&engine, 4);
        let mut tx = engine.write(None).unwrap();
        testing::reset();
        testing::inject(Op::Alloc, 0, Action::Fail(std::io::ErrorKind::OutOfMemory));
        let error = tx.page_mut(pages[2]).err().unwrap();
        testing::reset();
        assert_eq!(error.kind(), ErrorKind::OutOfMemory);
        let commit = tx.commit().err().unwrap();
        errors.push((error.to_string(), commit.to_string()));
        assert_eq!(read_text(&engine, pages[2]), "old 2");
        engine.close().unwrap();
        testing::set_page_slots(true);
    }
    assert_eq!(errors[0], errors[1]);
}

/// S2: readers that opened before the change, while the page was being changed in its slot, and before the commit
/// was durable, never see the unpublished slot's bytes.
#[test]
fn readers_never_see_a_page_being_changed_in_a_slot() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let pages = with_free_slots(&engine, 4);
    let before = engine.read().unwrap();
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, pages[0], "changing");
    assert_eq!(
        engine.memory_of(tx.page(pages[0]).unwrap()),
        Some(Memory::Versions)
    );
    let during = engine.read().unwrap();
    assert_eq!(text(during.page(pages[0]).unwrap()), "old 0");
    assert_eq!(text(before.page(pages[0]).unwrap()), "old 0");
    tx.commit().unwrap();
    assert_eq!(text(before.page(pages[0]).unwrap()), "old 0");
    assert_eq!(text(during.page(pages[0]).unwrap()), "old 0");
    assert_eq!(read_text(&engine, pages[0]), "changing");
    drop((before, during));
    engine.close().unwrap();
}

/// S2: an aborted transaction leaves nothing of what it changed in slots, and gives its slots back to the
/// temporary area, so the next transaction takes them without growing it.
#[test]
fn an_aborted_transaction_gives_its_slots_back() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = disk(tmp.path());
    let pages = with_free_slots(&engine, 4);
    engine.set_transfer_paused(true);
    let area = engine.area_bytes();
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, pages[3], "aborted");
    let created = tx.allocate().unwrap();
    let slots = [pages[3], created].map(|page| tx.page(page).unwrap().as_ptr());
    assert_eq!(
        engine.memory_of(tx.page(created).unwrap()),
        Some(Memory::Versions)
    );
    tx.abort();
    assert_eq!(read_text(&engine, pages[3]), "old 3");
    let mut tx = engine.write(None).unwrap();
    put(&mut tx, pages[3], "again");
    assert_eq!(
        tx.allocate().unwrap(),
        created,
        "the abort reserved nothing"
    );
    let again = [pages[3], created].map(|page| tx.page(page).unwrap().as_ptr());
    assert_eq!(again, slots, "the same slots, given back");
    tx.commit().unwrap();
    assert_eq!(engine.area_bytes(), area, "the area did not grow");
    assert_eq!(read_text(&engine, pages[3]), "again");
    engine.set_transfer_paused(false);
    engine.close().unwrap();
}
