//! The page contents that the layers above have already checked, remembered while the store is open (C1–C6 of the
//! remembered node layout check PRD).
//!
//! A content is identified by its page number and by the integrity code that a read has just verified (C2). The code
//! covers the page number, so a content of another page, or a changed content, has another code: a hit needs the code
//! to equal the one remembered. The table has a fixed number of slots, one atomic word each, chosen by the page
//! number, so a lookup never pairs one page's number with another's code (C5); remembering a content replaces
//! whatever its slot held. Code `0` marks an empty slot and is never remembered.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::error::Result;
use crate::scratch::try_reserve;

/// Slots of the table of a store: 512 KiB.
pub const CHECKED_SLOTS: usize = 1 << 16;

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

/// The table of a store.
pub(crate) struct Checked {
    /// Allocated once when the store opens and never resized.
    slots: Vec<AtomicU64>,
    /// Contents remembered so far: one per successful check of a verified page (tests count them).
    remembered: AtomicU64,
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
        let mut table = Vec::new();
        try_reserve(&mut table, slots)?;
        table.resize_with(slots, || AtomicU64::new(0));
        Ok(Checked {
            slots: table,
            remembered: AtomicU64::new(0),
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

    /// Slots of the table.
    pub(crate) fn slots(&self) -> usize {
        self.slots.len()
    }

    /// Contents remembered so far.
    pub(crate) fn remembered(&self) -> u64 {
        self.remembered.load(Ordering::Relaxed)
    }

    fn slot(&self, page: u64) -> &AtomicU64 {
        // The remainder is below the slot count, a `usize`.
        &self.slots[(page % self.slots.len() as u64) as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let checked = Checked {
            slots: (0..4).map(|_| AtomicU64::new(0)).collect(),
            remembered: AtomicU64::new(0),
        };
        checked.remember(1, 10);
        checked.remember(5, 50);
        assert!(!checked.contains(1, 10), "replaced");
        assert!(checked.contains(5, 50));
        assert_eq!(checked.remembered(), 2);
    }
}
