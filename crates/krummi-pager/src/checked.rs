//! The page contents that the layers above have already checked, remembered while the store is open (C1–C6 of the
//! remembered node layout check PRD).
//!
//! A content is identified by its page number and by the integrity code that a read has just verified (C2), or that the
//! transfer has just written in place from a version whose layout passed the check or kept it (Q2 of the written pages
//! PRD). The code covers the page number, so a content of another page, or a changed content, has another code: a hit
//! needs the code to equal the one remembered. The table has a fixed number of slots, one atomic word each, chosen by the page
//! number, so a lookup never pairs one page's number with another's code (C5); remembering a content replaces
//! whatever its slot held. Code `0` marks an empty slot and is never remembered.
//!
//! Debug builds keep a second array of atomic words, parallel to the slots: the code of the content its slot got from
//! a version that kept its layout check without a check of its own (Q3 of the written pages PRD), so that their reads
//! check that layout anyway (Q4). Like the slots, it is allocated when the store opens and read without a lock (C6).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::error::Result;
use crate::scratch::try_reserve;

/// Slots of the table of a store: 4 MiB, one per page number modulo this count, enough for the nodes of a working set
/// of 4 GiB. A table larger than the working set it scans checks most of its leaves again on every pass: with 65,536
/// slots (512 MiB of nodes), a scan of a 4.2 GB table took 18 % longer.
pub const CHECKED_SLOTS: usize = 1 << 19;

/// Whether the stores of the process remember checked contents (D4 of the execution).
static ENABLED: AtomicBool = AtomicBool::new(true);

/// A measurement hook, not part of the engine's interface (D4 of the execution): whether the stores of the process
/// remember checked contents. Only the measurement of C9 turns it off, to compare; while it is off, nothing is found
/// or remembered.
pub fn set_remember_checked(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

#[cfg(feature = "test-hooks")]
thread_local! {
    /// The slots of the stores this thread opens, when a test sets them ([`crate::testing::with_checked_slots`]).
    static TEST_SLOTS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Runs `f` with the stores it opens on this thread remembering `slots` contents at most (tests).
///
/// # Panics
///
/// If `slots` is `0`.
#[cfg(feature = "test-hooks")]
pub fn with_checked_slots<T>(slots: usize, f: impl FnOnce() -> T) -> T {
    assert!(
        slots > 0,
        "a table of checked contents has at least one slot"
    );
    /// Puts back the slots set before, also when `f` panics.
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_SLOTS.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(TEST_SLOTS.with(|cell| cell.replace(Some(slots))));
    f()
}

/// The low bits of a value run's mark, which hold its length: up to 65,535 pages, more than the longest value
/// (32,769 pages, V1 of the large values PRD).
const RUN_LENGTH_MASK: u64 = 0xFFFF;

/// The high bits of the mark of a value run with stored code `code`: the code's high 48 bits changed by a constant
/// whose high bits are not zero, so they never equal the high bits of a node's code `code` itself.
fn run_mark_high(code: u64) -> u64 {
    (code ^ 0x5A17_3C4E_9D2B_0000) & !RUN_LENGTH_MASK
}

/// The table of a store.
pub(crate) struct Checked {
    /// Allocated once when the store opens and never resized.
    slots: Vec<AtomicU64>,
    /// Contents remembered so far: one per successful check of a verified page (tests count them).
    remembered: AtomicU64,
    /// Per slot, the code of the content the transfer remembered in it from a version that kept its layout check
    /// (Q3 of the written pages PRD), or `0`: debug builds check that layout on every read anyway (Q4). Allocated once
    /// with `slots`, of the same length, and never resized.
    #[cfg(debug_assertions)]
    kept: Vec<AtomicU64>,
}

impl Checked {
    /// An empty table with the slots of a new store, or the out-of-memory class if the system does not provide it
    /// (P13).
    pub(crate) fn new() -> Result<Checked> {
        #[cfg(feature = "test-hooks")]
        let slots = TEST_SLOTS
            .with(std::cell::Cell::get)
            .unwrap_or(CHECKED_SLOTS);
        #[cfg(not(feature = "test-hooks"))]
        let slots = CHECKED_SLOTS;
        Ok(Checked {
            slots: zeroed_words(slots)?,
            remembered: AtomicU64::new(0),
            #[cfg(debug_assertions)]
            kept: zeroed_words(slots)?,
        })
    }

    /// Whether the content of page `page` with integrity code `code` was remembered and is still in its slot.
    pub(crate) fn contains(&self, page: u64, code: u64) -> bool {
        code != 0
            && ENABLED.load(Ordering::Relaxed)
            && self.slot(page).load(Ordering::Relaxed) == code
    }

    /// Remembers the content of page `page` with integrity code `code`, replacing what its slot held.
    pub(crate) fn remember(&self, page: u64, code: u64) {
        if code != 0 && ENABLED.load(Ordering::Relaxed) {
            self.slot(page).store(code, Ordering::Relaxed);
            self.remembered.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Remembers the content of page `page` with integrity code `code`, replacing what its slot held, as
    /// [`Checked::remember`] does, for a content whose layout passed the check, or kept it, as a version of the scratch
    /// area that the transfer has just written in place (Q2 of the written pages PRD). It is no check of a verified page,
    /// so it does not count as one. `kept` tells that the version kept that check without a check of its own (Q3), which
    /// debug builds record for [`Checked::layout_kept`] (Q4).
    ///
    /// Only the transfer calls it, one write at a time. A read finds the written content only through the scratch
    /// area's lock, taken after both stores, so a read that finds the content also finds whether it was kept. It stores
    /// the content before the record, releasing it, so that a read of an earlier content of the slot that sees the new
    /// record also sees that the slot changed ([`Checked::layout_kept`]).
    pub(crate) fn remember_written(&self, page: u64, code: u64, kept: bool) {
        if code != 0 && ENABLED.load(Ordering::Relaxed) {
            self.slot(page).store(code, Ordering::Relaxed);
            #[cfg(debug_assertions)]
            self.kept[self.index(page)].store(if kept { code } else { 0 }, Ordering::Release);
            #[cfg(not(debug_assertions))]
            let _ = kept;
        }
    }

    /// Whether the content of page `page` with integrity code `code` was remembered by the transfer from a version that
    /// kept its layout check without a check of its own (Q3 of the written pages PRD), for debug builds, which check
    /// such a layout on every read anyway (Q4). It takes no lock (C6).
    ///
    /// Other remembering in the slot leaves the record as it is: a record names one content by its code, which covers
    /// the page number (C5), so it never matches another content; a content that a read checked again after it was
    /// dropped from its slot may keep its record, which only adds a check of that same content in debug builds.
    ///
    /// A read asks it after finding the content in its slot ([`Checked::contains`]). When the slot no longer holds that
    /// content, a write of another page of the slot came in between and its record no longer tells, so the answer is
    /// yes: the read checks a layout that may have kept its check (R5 of the code review of the written pages). The
    /// record is read first, acquiring what the transfer released with it, so a read that sees a later record also
    /// sees the slot change that came before it.
    #[cfg(debug_assertions)]
    pub(crate) fn layout_kept(&self, page: u64, code: u64) -> bool {
        code != 0
            && (self.kept[self.index(page)].load(Ordering::Acquire) == code
                || self.slot(page).load(Ordering::Relaxed) != code)
    }

    /// Remembers that the value run of `length` pages from page `page`, with stored code `code`, passed the integrity
    /// check (I1 of the remembered page integrity PRD), replacing what its slot held. Its mark keeps the code's high
    /// 48 bits, changed by a constant so that a node's code never matches it, and the length in the low 16 bits. It
    /// is no content whose layout passed, so it does not count as one.
    pub(crate) fn remember_run(&self, page: u64, code: u64, length: u64) {
        // A length the mark cannot hold leaves no mark: the run is then remembered by its bits alone.
        if (1..=RUN_LENGTH_MASK).contains(&length) && ENABLED.load(Ordering::Relaxed) {
            self.slot(page)
                .store(run_mark_high(code) | length, Ordering::Relaxed);
        }
    }

    /// The length of the value run from page `page` with stored code `code`, when its slot still holds the run's mark.
    pub(crate) fn run_length(&self, page: u64, code: u64) -> Option<u64> {
        let slot = self.slot(page).load(Ordering::Relaxed);
        let length = slot & RUN_LENGTH_MASK;
        // A mark has a length, so an empty slot is none even when a code's high bits make the mark's zero.
        (ENABLED.load(Ordering::Relaxed)
            && length > 0
            && slot & !RUN_LENGTH_MASK == run_mark_high(code))
        .then_some(length)
    }

    /// Slots of the table.
    pub(crate) fn slots(&self) -> usize {
        self.slots.len()
    }

    /// Contents remembered so far.
    pub(crate) fn remembered(&self) -> u64 {
        self.remembered.load(Ordering::Relaxed)
    }

    fn slot(&self, page: u64) -> &AtomicU64 {
        &self.slots[self.index(page)]
    }

    /// The slot of page `page`.
    fn index(&self, page: u64) -> usize {
        // The remainder is below the slot count, a `usize`.
        (page % self.slots.len() as u64) as usize
    }
}

/// `count` atomic words of value `0`, or the out-of-memory class if the system does not provide them (P13).
fn zeroed_words(count: usize) -> Result<Vec<AtomicU64>> {
    let mut words = Vec::new();
    try_reserve(&mut words, count)?;
    words.resize_with(count, || AtomicU64::new(0));
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_mark_keeps_its_length_and_never_matches_a_node() {
        let checked = Checked::new().unwrap();
        let code = 0x1234_5678_9ABC_DEF0;
        assert_eq!(checked.run_length(7, code), None);
        checked.remember_run(7, code, 3);
        assert_eq!(checked.run_length(7, code), Some(3));
        assert_eq!(
            checked.run_length(7, code ^ (1 << 40)),
            None,
            "another code"
        );
        assert!(
            !checked.contains(7, code),
            "a node with that code is not checked"
        );
        checked.remember_run(7, code, RUN_LENGTH_MASK + 1);
        assert_eq!(
            checked.run_length(7, code),
            Some(3),
            "a length it cannot hold leaves no mark"
        );
        // A code whose mark has no high bits: an empty slot is no mark of length 0.
        let zero = 0x5A17_3C4E_9D2B_0000;
        assert_eq!(run_mark_high(zero), 0);
        assert_eq!(checked.run_length(9, zero), None);
    }

    #[test]
    fn a_content_is_found_only_with_its_page_and_its_code() {
        let checked = Checked::new().unwrap();
        assert!(!checked.contains(7, 42));
        checked.remember(7, 42);
        assert!(checked.contains(7, 42));
        assert!(!checked.contains(7, 43), "another content of the page");
        assert!(!checked.contains(8, 42), "another page");
        assert_eq!(checked.remembered(), 1);
    }

    #[test]
    fn code_zero_is_never_remembered() {
        let checked = Checked::new().unwrap();
        checked.remember(3, 0);
        assert!(!checked.contains(3, 0));
        assert_eq!(checked.remembered(), 0);
    }

    #[test]
    fn pages_that_share_a_slot_replace_each_other() {
        let checked = small_table();
        checked.remember(1, 10);
        checked.remember(5, 50);
        assert!(!checked.contains(1, 10), "replaced");
        assert!(checked.contains(5, 50));
        assert_eq!(checked.remembered(), 2);
    }

    /// A table of four slots.
    fn small_table() -> Checked {
        Checked {
            slots: zeroed_words(4).unwrap(),
            remembered: AtomicU64::new(0),
            #[cfg(debug_assertions)]
            kept: zeroed_words(4).unwrap(),
        }
    }

    /// Q4 of the written pages PRD: the record of a kept check names the content the transfer remembered from a version
    /// that kept it, and nothing else, without a lock (R4 of the code review of the written pages).
    #[cfg(debug_assertions)]
    #[test]
    fn a_kept_check_is_recorded_for_its_content_only() {
        let checked = small_table();
        checked.remember_written(1, 10, true);
        assert!(checked.contains(1, 10));
        assert!(checked.layout_kept(1, 10));
        assert_eq!(checked.remembered(), 0, "no check is counted");
        // A content of another page in the slot is not kept, and the record never matches it.
        checked.remember(5, 50);
        assert!(checked.contains(5, 50));
        assert!(!checked.layout_kept(5, 50));
        // A version that passed a check of its own clears the record of its slot.
        checked.remember_written(1, 10, true);
        checked.remember_written(1, 12, false);
        assert!(checked.contains(1, 12));
        assert!(!checked.layout_kept(1, 12));
        assert!(!checked.layout_kept(2, 0), "code 0 is never kept");
    }

    /// Q4 of the written pages PRD, R5 of the code review of the written pages: a read that found a kept content in its
    /// slot still checks its layout when the transfer writes another page of the slot before the read asks whether it
    /// was kept, because the slot no longer tells.
    #[cfg(debug_assertions)]
    #[test]
    fn a_kept_content_replaced_after_its_hit_is_still_checked() {
        let checked = small_table();
        checked.remember_written(1, 10, true);
        assert!(checked.contains(1, 10), "the read finds the kept content");
        // The transfer writes page 5, of the same slot, between the read's two lookups.
        checked.remember_written(5, 50, false);
        assert!(checked.layout_kept(1, 10), "the read checks it");
        assert!(checked.contains(5, 50));
        assert!(!checked.layout_kept(5, 50), "the new content kept no check");
        // The same after a check of another page of the slot, which leaves the record.
        checked.remember_written(1, 10, true);
        checked.remember(5, 50);
        assert!(checked.layout_kept(1, 10));
    }
}
