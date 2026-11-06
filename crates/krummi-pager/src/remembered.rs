//! The pages whose integrity a read already verified while the store is open (I1–I7 of the remembered page
//! integrity PRD).
//!
//! One bit per page of the store, set when a read verifies the page and cleared before the engine writes it in
//! place (I4). A value stored off its leaf is remembered and forgotten with every page of its range, and is only
//! remembered while every one of them is (I1). The bits live in a reserved address range that never moves and is committed, zeroed, as the
//! store grows: readers find a page's bit without a lock, and a growth only commits more of the range before the
//! store publishes its new size, so every page a reader can reach already has its bit. One bit per page is 16 KiB
//! per GiB of store (I6); what the system commits is rounded up to its memory pages.

use std::io;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::format::PAGE_SIZE;
use crate::sys;

/// Pages a store on disk can have before a growth finds no room for their bits, which is the out-of-memory class
/// (I6): 64 TiB of store, whose bits take 1 GiB of address space and no memory until the store reaches that size,
/// less than the reservation of a temporary store. A store with a smaller maximum size could reserve less, but
/// recovery grows a store beyond its maximum (P10), so every store on disk reserves this.
pub(crate) const MAX_FILE_PAGES: u64 = (64 << 40) / PAGE_SIZE as u64;

/// What the reservation is rounded up to: a multiple of the page size of every system the engine runs on.
const RESERVE_ALIGN: usize = 64 << 10;

#[cfg(feature = "test-hooks")]
thread_local! {
    /// Whether committing bits fails on this thread ([`crate::testing::with_remembered_commits_failing`]).
    static FAIL_COMMITS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs `f` with every commit of remembered bits made on this thread failing as a lack of memory (tests: AC-I11).
#[cfg(feature = "test-hooks")]
pub fn with_remembered_commits_failing<T>(f: impl FnOnce() -> T) -> T {
    /// Puts back the setting from before, also when `f` panics.
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            FAIL_COMMITS.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(FAIL_COMMITS.with(|cell| cell.replace(true)));
    f()
}

/// The remembered pages of a store.
pub(crate) struct Remembered {
    /// The start of the reserved range, as words of 64 bits; it never moves.
    words: *const AtomicU64,
    /// Words committed so far: a reader only touches a word below it. Raised only while `reservation` is locked,
    /// and after the commit, so a reader that sees it sees the committed words.
    committed: AtomicU64,
    /// The range itself and the pages whose bits are committed, locked by a growth while it commits.
    reservation: Mutex<(sys::Reservation, u64)>,
}

// SAFETY: the words are atomics in memory the reservation owns until `Remembered` drops; sharing them between threads
// is what they are for, and the reservation's own `Send`/`Sync` cover the rest.
unsafe impl Send for Remembered {}
// SAFETY: see `Send`.
unsafe impl Sync for Remembered {}

impl Remembered {
    /// Room for the bits of `max_pages` pages, with those of the first `pages` committed and clear. Fails with the
    /// bytes it asked for when the system gives neither the address space nor the memory: the reservation's, or the
    /// committed bits'.
    pub(crate) fn new(max_pages: u64, pages: u64) -> Result<Remembered, u64> {
        let reserve = bytes_for(max_pages.max(pages))
            .and_then(|bytes| bytes.checked_next_multiple_of(RESERVE_ALIGN))
            .ok_or(u64::MAX)?;
        let reservation = sys::Reservation::new(reserve).map_err(|_| reserve as u64)?;
        let remembered = Remembered {
            words: reservation.ptr().as_ptr().cast::<AtomicU64>(),
            committed: AtomicU64::new(0),
            reservation: Mutex::new((reservation, 0)),
        };
        remembered
            .ensure(pages)
            .map_err(|_| bytes_for(pages).map_or(u64::MAX, |bytes| bytes as u64))?;
        Ok(remembered)
    }

    /// Commits the bits of the first `pages` pages, clear, when they are not committed yet. On failure nothing more
    /// is committed: the system has no memory for them, or the store goes beyond what was reserved.
    pub(crate) fn ensure(&self, pages: u64) -> io::Result<()> {
        let out_of_memory = || io::Error::from(io::ErrorKind::OutOfMemory);
        let mut guard = self.reservation.lock().unwrap();
        let (reservation, covered) = &mut *guard;
        if pages <= *covered {
            return Ok(());
        }
        // Every growth fails under the test hook, also one whose bits fit in the words already committed.
        #[cfg(feature = "test-hooks")]
        if FAIL_COMMITS.with(std::cell::Cell::get) {
            return Err(out_of_memory());
        }
        let bytes = bytes_for(pages).ok_or_else(out_of_memory)?;
        if bytes > reservation.committed() {
            if bytes > reservation.len() {
                return Err(out_of_memory());
            }
            reservation.commit(bytes)?;
            self.committed
                .store((bytes / size_of::<u64>()) as u64, Ordering::Release);
        }
        *covered = pages;
        Ok(())
    }

    /// Whether all the `count` pages from page `first` are remembered.
    pub(crate) fn contains(&self, first: u64, count: u64) -> bool {
        count > 0
            && words(first, count).all(|(page, mask)| {
                self.word(page)
                    .is_some_and(|word| word.load(Ordering::Acquire) & mask == mask)
            })
    }

    /// Remembers the `count` pages from page `first`.
    pub(crate) fn remember(&self, first: u64, count: u64) {
        for (page, mask) in words(first, count) {
            if let Some(word) = self.word(page) {
                word.fetch_or(mask, Ordering::AcqRel);
            }
        }
    }

    /// Forgets the `count` pages from page `first`. Never fails and needs no memory (I4).
    pub(crate) fn forget(&self, first: u64, count: u64) {
        for (page, mask) in words(first, count) {
            if let Some(word) = self.word(page) {
                word.fetch_and(!mask, Ordering::AcqRel);
            }
        }
    }

    /// The word that holds page `page`'s bit, when it is committed.
    fn word(&self, page: u64) -> Option<&AtomicU64> {
        let index = page / 64;
        (index < self.committed.load(Ordering::Acquire)).then(|| {
            // SAFETY: the word is inside the committed part of the reservation, which `self` owns and which never
            // moves or shrinks while `self` lives; committed memory is zeroed, a valid `AtomicU64`, and only ever
            // accessed atomically.
            unsafe { &*self.words.add(index as usize) }
        })
    }
}

/// Bytes of the words that hold the bits of `pages` pages.
fn bytes_for(pages: u64) -> Option<usize> {
    let words = pages.div_ceil(64).max(1);
    usize::try_from(words.checked_mul(size_of::<u64>() as u64)?).ok()
}

/// The words that hold the bits of the `count` pages from page `first`: for each, a page in it and the mask of
/// those pages' bits.
fn words(first: u64, count: u64) -> impl Iterator<Item = (u64, u64)> {
    let end = first.saturating_add(count);
    let mut page = first;
    std::iter::from_fn(move || {
        (page < end).then(|| {
            let in_word = (end - page).min(64 - page % 64);
            let mask = if in_word == 64 {
                u64::MAX
            } else {
                ((1u64 << in_word) - 1) << (page % 64)
            };
            let at = page;
            page += in_word;
            (at, mask)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_remembered_until_it_is_forgotten() {
        let remembered = Remembered::new(1 << 20, 200).unwrap();
        assert!(!remembered.contains(70, 1));
        remembered.remember(70, 1);
        assert!(remembered.contains(70, 1));
        assert!(!remembered.contains(69, 1) && !remembered.contains(71, 1));
        remembered.forget(70, 1);
        assert!(!remembered.contains(70, 1));
    }

    #[test]
    fn forgetting_a_range_clears_every_page_of_it_and_nothing_else() {
        let remembered = Remembered::new(1 << 20, 400).unwrap();
        remembered.remember(0, 400);
        remembered.forget(10, 300);
        for page in 0..400 {
            assert_eq!(
                remembered.contains(page, 1),
                !(10..310).contains(&page),
                "page {page}"
            );
        }
    }

    #[test]
    fn pages_beyond_what_is_committed_are_never_remembered() {
        let remembered = Remembered::new(1 << 20, 64).unwrap();
        // A system page of bits is committed at least, so go well beyond it.
        let far = 1 << 18;
        remembered.remember(far, 1);
        assert!(!remembered.contains(far, 1));
        remembered.ensure(far + 1).unwrap();
        assert!(!remembered.contains(far, 1));
        remembered.remember(far, 1);
        assert!(remembered.contains(far, 1));
    }

    #[test]
    fn a_store_beyond_the_reservation_has_no_room_for_its_bits() {
        let remembered = Remembered::new(1 << 10, 64).unwrap();
        let error = remembered.ensure(1 << 30).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::OutOfMemory);
    }

    #[test]
    fn a_run_is_remembered_only_while_every_page_of_it_is() {
        let remembered = Remembered::new(1 << 20, 1 << 16).unwrap();
        remembered.remember(100, 200);
        assert!(remembered.contains(100, 200));
        assert!(!remembered.contains(100, 201), "a longer run");
        remembered.forget(250, 1);
        assert!(!remembered.contains(100, 200), "a page inside it rewritten");
        assert!(remembered.contains(100, 150) && !remembered.contains(0, 0));
    }
}
