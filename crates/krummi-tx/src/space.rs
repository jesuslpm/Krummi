//! Free space (free-space PRD, F1–F14): the regions of the store, the frontier and the total of free pages of the
//! map as the writer sees them, what each region's map page says, and what is durable for the store information.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use crate::map::Summary;
use crate::{FIRST_PAGE, MAP_PAGE, PAYLOAD_SIZE};

/// Pages of a region: as many as bits in a map page (F2).
pub const REGION_PAGES: u64 = PAYLOAD_SIZE as u64 * 8;

/// Where the regions and their map pages are. The region size is fixed for a store; tests may use a smaller one
/// (criteria preamble of the free-space PRD).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Layout {
    region_pages: u64,
}

impl Layout {
    /// The longest range a reservation may ask for: a region without its map page (F5).
    pub(crate) fn cap(self) -> u64 {
        self.region_pages - 1
    }

    pub(crate) fn region_pages(self) -> u64 {
        self.region_pages
    }

    pub(crate) fn region(self, page: u64) -> u64 {
        page / self.region_pages
    }

    /// Position of `page` in its region, which is its bit in the region's map.
    pub(crate) fn bit(self, page: u64) -> u64 {
        page % self.region_pages
    }

    /// The map page of `region`: its first page, except in the first region, where it follows the header and the
    /// meta page (F2).
    pub(crate) fn map_page(self, region: u64) -> u64 {
        if region == 0 {
            MAP_PAGE
        } else {
            region * self.region_pages
        }
    }

    /// Whether `page` is a map page.
    pub(crate) fn is_map(self, page: u64) -> bool {
        page == MAP_PAGE || (page >= self.region_pages && self.bit(page) == 0)
    }

    /// Whether `page` belongs to the store itself: the header, the meta page and the map pages.
    pub(crate) fn is_own(self, page: u64) -> bool {
        page < FIRST_PAGE || self.is_map(page)
    }

    /// Pages from `frontier` to the end of a store of `page_count` pages, without the map positions of the
    /// regions the frontier has not reached yet (F10); zero when the store ends before the frontier.
    pub(crate) fn pages_after(self, frontier: u64, page_count: u64) -> u64 {
        if page_count <= frontier {
            return 0;
        }
        let maps = (page_count - 1) / self.region_pages - (frontier - 1) / self.region_pages;
        page_count - frontier - maps
    }
}

/// The frontier and the free pages of the map as of a commit.
#[derive(Debug, Clone, Copy)]
struct Published {
    txid: u64,
    frontier: u64,
    free: u64,
}

/// What the writer and the store information know about free space.
pub(crate) struct Space {
    region_pages: AtomicU64,
    /// First page never used, as of the newest commit that passed its point of no return (F3).
    frontier: AtomicU64,
    /// Free pages of the map, as of that commit.
    free: AtomicU64,
    /// Whether the meta page was ever committed.
    meta_written: AtomicBool,
    /// What each region's map says, as of the newest commit, for the regions read since opening; `None` for
    /// the unknown ones, which are never taken for full (F9).
    regions: Mutex<Vec<Option<Summary>>>,
    /// The commits since the last one a read can see, the oldest first, for the store information (F10).
    published: Mutex<VecDeque<Published>>,
}

impl Space {
    pub(crate) fn new(frontier: u64, free: u64, meta_written: bool) -> Space {
        Space {
            region_pages: AtomicU64::new(REGION_PAGES),
            frontier: AtomicU64::new(frontier),
            free: AtomicU64::new(free),
            meta_written: AtomicBool::new(meta_written),
            regions: Mutex::new(Vec::new()),
            published: Mutex::new(VecDeque::from([Published {
                txid: 0,
                frontier,
                free,
            }])),
        }
    }

    pub(crate) fn layout(&self) -> Layout {
        Layout {
            region_pages: self.region_pages.load(Ordering::Relaxed),
        }
    }

    #[cfg(feature = "test-hooks")]
    pub(crate) fn set_region_pages(&self, pages: u64) {
        assert!(
            (FIRST_PAGE + 2..=REGION_PAGES).contains(&pages),
            "a region holds the store's own pages and fits in a map page"
        );
        self.region_pages.store(pages, Ordering::Relaxed);
    }

    pub(crate) fn frontier(&self) -> u64 {
        self.frontier.load(Ordering::Acquire)
    }

    pub(crate) fn free(&self) -> u64 {
        self.free.load(Ordering::Acquire)
    }

    pub(crate) fn meta_written(&self) -> bool {
        self.meta_written.load(Ordering::Acquire)
    }

    /// Whether `region` may hold a run of `count` free pages: unknown regions may.
    pub(crate) fn may_hold(&self, region: u64, count: u64) -> bool {
        let regions = self.regions.lock().unwrap_or_else(PoisonError::into_inner);
        match regions.get(region as usize) {
            Some(Some(summary)) => summary.longest >= count,
            _ => true,
        }
    }

    #[cfg(feature = "test-hooks")]
    pub(crate) fn regions_read(&self) -> usize {
        let regions = self.regions.lock().unwrap_or_else(PoisonError::into_inner);
        regions.iter().filter(|r| r.is_some()).count()
    }

    /// Records what the map of `region` says as of the newest commit.
    pub(crate) fn learn(&self, region: u64, summary: Summary) {
        let mut regions = self.regions.lock().unwrap_or_else(PoisonError::into_inner);
        let index = region as usize;
        if regions.len() <= index {
            regions.resize(index + 1, None);
        }
        regions[index] = Some(summary);
    }

    /// Records commit `txid` at its point of no return: from then on the next writer works from it (F6).
    pub(crate) fn publish(
        &self,
        txid: u64,
        frontier: u64,
        free: u64,
        maps: &[(u64, Summary)],
        meta_written: bool,
    ) {
        self.frontier.store(frontier, Ordering::Release);
        self.free.store(free, Ordering::Release);
        if meta_written {
            self.meta_written.store(true, Ordering::Release);
        }
        for &(region, summary) in maps {
            self.learn(region, summary);
        }
        self.published
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push_back(Published {
                txid,
                frontier,
                free,
            });
    }

    /// Free pages for the store information (F10): the free pages of the map as of the newest commit a read can
    /// see (`visible`), plus the pages between its frontier and the end of a store of `page_count` pages.
    pub(crate) fn free_pages(&self, visible: u64, page_count: u64) -> u64 {
        let mut published = self
            .published
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        while published.get(1).is_some_and(|p| p.txid <= visible) {
            published.pop_front();
        }
        let seen = published[0];
        seen.free + self.layout().pages_after(seen.frontier, page_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_and_their_map_pages() {
        let layout = Layout { region_pages: 10 };
        assert_eq!(layout.cap(), 9);
        assert_eq!(
            (layout.map_page(0), layout.map_page(1), layout.map_page(3)),
            (MAP_PAGE, 10, 30)
        );
        let own: Vec<u64> = (0..31).filter(|&p| layout.is_own(p)).collect();
        assert_eq!(own, [0, 1, 2, 3, 10, 20, 30]);
        assert_eq!((layout.region(19), layout.bit(19)), (1, 9));
    }

    #[test]
    fn pages_after_the_frontier_skip_future_map_pages() {
        let layout = Layout { region_pages: 10 };
        assert_eq!(layout.pages_after(4, 8), 4, "a new store");
        assert_eq!(layout.pages_after(4, 25), 19, "the map pages at 10 and 20");
        assert_eq!(
            layout.pages_after(10, 25),
            13,
            "the map page at the frontier"
        );
        assert_eq!(layout.pages_after(11, 25), 13);
        assert_eq!(layout.pages_after(30, 25), 0, "a store that ends before it");
    }

    #[test]
    fn the_information_follows_what_reads_see() {
        let space = Space::new(FIRST_PAGE, 0, false);
        space.publish(5, 20, 3, &[], true);
        space.publish(6, 30, 7, &[], true);
        assert_eq!(space.free_pages(4, 40), 36, "nothing visible yet");
        assert_eq!(space.free_pages(5, 40), 3 + 20);
        assert_eq!(space.free_pages(6, 40), 7 + 10);
    }
}
