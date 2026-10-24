//! Read and write transactions (Transactions PRD: T1, T2, T8, T10, T11, T14), and the reservations and frees of
//! free space inside a write (free-space PRD: F3–F9).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use krummi_pager::format::{PAGE_SIZE_U64, seal_page};
use krummi_pager::{PayloadBuf, ScratchWriter, SealedPage, Snapshot, alloc_payload, alloc_sealed};

use crate::map::{self, Summary};
use crate::space::Layout;
use crate::{Engine, Error, FIRST_PAGE, MAP_PAGE, META_PAGE, Point, Result};

/// A read transaction: a snapshot of the store as it was when it opened (T2). It can be sent to and shared
/// between threads (T6); it ends when it is dropped.
pub struct ReadTransaction<'e> {
    snapshot: Snapshot<'e>,
}

impl<'e> ReadTransaction<'e> {
    pub(crate) fn new(snapshot: Snapshot<'e>) -> ReadTransaction<'e> {
        ReadTransaction { snapshot }
    }

    /// The payload of page `page_number` as this transaction sees it, without copying it (D11, T16). The
    /// reference lives as long as the transaction (T8).
    pub fn page(&self, page_number: u64) -> Result<&[u8]> {
        self.snapshot.page(page_number)
    }
}

/// The write transaction. It stays on the thread that opened it (T6). Dropping it without committing aborts it
/// (T10).
pub struct WriteTransaction<'e> {
    engine: &'e Engine,
    writer: ScratchWriter<'e>,
    layout: Layout,
    /// Pages this transaction changed, by number: the meta page and map pages among them when it reserved or
    /// freed pages.
    dirty: BTreeMap<u64, PayloadBuf>,
    /// The frontier when the transaction got its turn, and as this transaction moved it (F3).
    start_frontier: u64,
    frontier: u64,
    /// Free pages of the map this transaction may still reserve: the committed ones it did not take, plus the ones
    /// it skipped at the frontier (F5). Its own frees join them only at commit (F6).
    free: u64,
    /// Pages this transaction freed: they are set free in the map at commit (F6, F8).
    freed: BTreeSet<u64>,
    /// The first operation that failed: the transaction can no longer be committed (T11).
    failure: RefCell<Option<Error>>,
}

impl<'e> WriteTransaction<'e> {
    pub(crate) fn new(engine: &'e Engine, writer: ScratchWriter<'e>) -> WriteTransaction<'e> {
        let frontier = engine.space.frontier();
        WriteTransaction {
            layout: engine.space.layout(),
            start_frontier: frontier,
            frontier,
            free: engine.space.free(),
            engine,
            writer,
            dirty: BTreeMap::new(),
            freed: BTreeSet::new(),
            failure: RefCell::new(None),
        }
    }

    /// The payload of page `page_number` as this transaction sees it: its own change, or what was committed
    /// before it got its turn (T2). The reference lives until the next change of this transaction (T8). Reading a
    /// page this transaction freed fails with the misuse class (F8, T11).
    ///
    /// # Panics
    ///
    /// If the page was never reserved.
    pub fn page(&self, page_number: u64) -> Result<&[u8]> {
        self.check_reserved(page_number);
        if self.freed.contains(&page_number) {
            return self.track(Err(self
                .engine
                .misuse("the page was freed by this transaction")));
        }
        if let Some(payload) = self.dirty.get(&page_number) {
            return Ok(&payload[..]);
        }
        let result = self.writer.snapshot().page(page_number);
        self.track(result)
    }

    /// The payload of page `page_number` to change it. The first change copies the page into this transaction
    /// (T14). Changing one of the store's own pages, or a page this transaction freed, fails with the misuse
    /// class (F8, T11).
    ///
    /// # Panics
    ///
    /// If the page was never reserved.
    pub fn page_mut(&mut self, page_number: u64) -> Result<&mut [u8]> {
        let result = self.writable(page_number);
        self.track(result)?;
        Ok(&mut self.dirty.get_mut(&page_number).expect("copied above")[..])
    }

    /// Reserves a page, zeroed, and returns its number: a free page of the map if there is one, otherwise the
    /// page at the frontier (F4).
    pub fn allocate(&mut self) -> Result<u64> {
        self.allocate_run(1)
    }

    /// Reserves exactly `count` contiguous pages, zeroed, and returns the first: a run of free pages of the map if
    /// there is one, otherwise at the frontier, in the next region when they do not fit in the frontier's (F5).
    /// Asking for none, or for more than a region holds, fails with the misuse class.
    pub fn allocate_run(&mut self, count: u64) -> Result<u64> {
        let result = self.reserve_run(count);
        self.track(result)
    }

    /// Frees `count` pages from `first`, which must be in use as this transaction sees them; their changes in this
    /// transaction are dropped. They become free for the writes that follow this commit, not for this transaction
    /// (F6, F8). Freeing the store's own pages, pages from the frontier on, pages already free, or a page twice
    /// fails with the misuse class (T11).
    pub fn free(&mut self, first: u64, count: u64) -> Result<()> {
        let result = self.release(first, count);
        self.track(result)
    }

    /// Commits the transaction durably (J4 of the Journal PRD): once nothing can make it fail without the store
    /// refusing writes, the commit enters the journal's queue and releases the turn, its point of no return (E2 of
    /// the grouped journal PRD); the next writer sees it at once, and it returns once it survives a crash, visible
    /// to every transaction opened after this returns (T2, E3). If an operation failed, the commit fails with the
    /// class of the first failure and applies nothing (T11); after the point of no return it can only fail with the
    /// I/O class, when its group fails (E8), and the close may still write it, entire (E12, J12). A transaction
    /// without changes writes nothing, and waits until the commits it saw are durable (E11). The transaction ends
    /// either way.
    pub fn commit(self) -> Result<()> {
        self.commit_with(true)
    }

    /// Commits the transaction in deferred mode (J10, E5 of the grouped journal PRD): it returns once the commit is
    /// in the journal's queue, without waiting for it to be durable; it becomes durable with its group, like a
    /// durable commit in its place, and read transactions see it from then on (E3). Until then any crash may lose
    /// it, with every later commit. It waits, keeping the turn, while the journal's queue is full (E2).
    pub fn commit_deferred(self) -> Result<()> {
        self.commit_with(false)
    }

    fn commit_with(mut self, durable: bool) -> Result<()> {
        if let Some(error) = self.failure.get_mut().take() {
            return Err(error);
        }
        if self.dirty.is_empty() {
            // E11: what it saw may not be durable yet; aborting, or a deferred commit, never waits.
            let seen = self.writer.snapshot().txid();
            let engine = self.engine;
            drop(self);
            return if durable {
                engine.wait_durable(seen)
            } else {
                Ok(())
            };
        }
        self.engine.hook(Point::CommitFlush);
        self.engine.hook(Point::CommitSeal);
        let finished = self.finish_space();
        let maps = self.track(finished)?;
        let meta_written = self.dirty.contains_key(&META_PAGE);
        let sealed = self.seal();
        let pages = self.track(sealed)?;
        self.engine.hook(Point::CommitPublish);
        let committed = self.writer.commit(&pages);
        let txid = self.track(committed)?;
        // The point of no return (E2): the commit is in the journal's queue and the next writer sees it, and the
        // pages it freed (F6).
        self.engine
            .space
            .publish(txid, self.frontier, self.free, &maps, meta_written);
        self.engine.hook(Point::CommitQueued);
        // The turn goes to the next writer before waiting for the disk. The transfer thread moves the commit to
        // `krummi.data` once it is durable, without blocking anyone (E2, J5).
        let engine = self.engine;
        drop(self);
        if durable {
            engine.hook(Point::CommitWait);
            engine.wait_durable(txid)?;
        }
        Ok(())
    }

    /// Aborts the transaction: none of its changes apply (T10).
    pub fn abort(self) {}

    fn check_reserved(&self, page_number: u64) {
        assert!(
            (FIRST_PAGE..self.frontier).contains(&page_number),
            "page {page_number} was never reserved"
        );
    }

    fn writable(&mut self, page_number: u64) -> Result<()> {
        if self.layout.is_own(page_number) {
            return Err(self
                .engine
                .misuse("the store's own pages cannot be changed"));
        }
        self.check_reserved(page_number);
        if self.freed.contains(&page_number) {
            return Err(self.engine.misuse("the page was freed by this transaction"));
        }
        if !self.dirty.contains_key(&page_number) {
            self.copy_for_write(page_number)?;
        }
        Ok(())
    }

    fn copy_for_write(&mut self, page_number: u64) -> Result<()> {
        self.reserve(1, 0)?;
        let mut payload = alloc_payload()?;
        payload.copy_from_slice(self.writer.snapshot().page(page_number)?);
        self.dirty.insert(page_number, payload);
        Ok(())
    }

    /// Adds zeroed pages to this transaction.
    fn add_zeroed(&mut self, pages: impl IntoIterator<Item = u64>) -> Result<()> {
        for page in pages {
            self.dirty.insert(page, alloc_payload()?);
        }
        Ok(())
    }

    /// The meta page in this transaction; the first one ever also writes the first region's map page (F2).
    fn ensure_meta(&mut self) -> Result<()> {
        if self.dirty.contains_key(&META_PAGE) {
            return Ok(());
        }
        if !self.engine.space.meta_written() {
            self.reserve(2, 0)?;
            return self.add_zeroed([META_PAGE, MAP_PAGE]);
        }
        self.copy_for_write(META_PAGE)
    }

    /// The map page of `region` in this transaction, copied on first use.
    fn ensure_map(&mut self, region: u64) -> Result<&mut [u8]> {
        self.ensure_meta()?;
        let page = self.layout.map_page(region);
        if !self.dirty.contains_key(&page) {
            self.copy_for_write(page)?;
        }
        Ok(&mut self.dirty.get_mut(&page).expect("copied above")[..])
    }

    fn reserve_run(&mut self, count: u64) -> Result<u64> {
        if count == 0 || count > self.layout.cap() {
            return Err(self
                .engine
                .misuse("a range must have at least one page and fit in a region"));
        }
        if let Some(first) = self.take_from_map(count)? {
            return Ok(first);
        }
        self.take_at_frontier(count)
    }

    /// Takes the first run of `count` free pages of the map, reading the regions it needs (F4, F5, F9).
    fn take_from_map(&mut self, count: u64) -> Result<Option<u64>> {
        if self.free < count {
            return Ok(None);
        }
        let engine = self.engine;
        let space = &engine.space;
        for region in 0..=self.layout.region(self.frontier - 1) {
            let map_page = self.layout.map_page(region);
            let found = match self.dirty.get(&map_page) {
                Some(map) => map::find_run(&map[..], count),
                None if !space.may_hold(region, count) => None,
                None => {
                    let map = self.writer.snapshot().page(map_page)?;
                    let summary = map::summary(map);
                    space.learn(region, summary);
                    (summary.longest >= count)
                        .then(|| map::find_run(map, count))
                        .flatten()
                }
            };
            let Some(bit) = found else { continue };
            let first = region * self.layout.region_pages() + bit;
            self.ensure_map(region)?;
            self.reserve(count, first + count)?;
            map::set(self.ensure_map(region)?, bit, count, false);
            self.add_zeroed(first..first + count)?;
            self.free -= count;
            return Ok(Some(first));
        }
        Ok(None)
    }

    /// Takes `count` pages at the frontier; when they do not fit in its region, the rest of the region is left
    /// free and they start after the next region's map page, which this transaction writes (F3, F5).
    fn take_at_frontier(&mut self, count: u64) -> Result<u64> {
        let layout = self.layout;
        let region = layout.region(self.frontier);
        let offset = layout.bit(self.frontier);
        let (skip, enter) = if region > 0 && offset == 0 {
            (false, Some(region))
        } else if offset + count > layout.region_pages() {
            (true, Some(region + 1))
        } else {
            (false, None)
        };
        let first = enter.map_or(self.frontier, |r| layout.map_page(r) + 1);
        self.ensure_meta()?;
        if skip {
            self.ensure_map(region)?;
        }
        self.reserve(count + u64::from(enter.is_some()), first + count)?;
        if skip {
            let skipped = layout.region_pages() - offset;
            map::set(self.ensure_map(region)?, offset, skipped, true);
            self.free += skipped;
        }
        if let Some(region) = enter {
            self.add_zeroed([layout.map_page(region)])?;
        }
        self.add_zeroed(first..first + count)?;
        self.frontier = first + count;
        Ok(first)
    }

    fn release(&mut self, first: u64, count: u64) -> Result<()> {
        let end = first.checked_add(count).filter(|_| count > 0);
        let Some(end) = end else {
            return Err(self.engine.misuse("a range must have at least one page"));
        };
        for page in first..end {
            let reason = if self.layout.is_own(page) {
                "the store's own pages cannot be freed"
            } else if page >= self.frontier {
                "the page was never in use"
            } else if self.freed.contains(&page) {
                "the page was already freed by this transaction"
            } else {
                continue;
            };
            return Err(self.engine.misuse(reason));
        }
        let layout = self.layout;
        for region in layout.region(first)..=layout.region(end - 1) {
            let start = region * layout.region_pages();
            let pages = first.max(start)..end.min(start + layout.region_pages());
            let map = self.ensure_map(region)?;
            if pages
                .clone()
                .any(|page| map::is_free(map, layout.bit(page)))
            {
                return Err(self.engine.misuse("the page is already free"));
            }
        }
        for page in first..end {
            self.dirty.remove(&page);
            self.freed.insert(page);
        }
        Ok(())
    }

    /// Before sealing: sets this transaction's frees in the map, writes the frontier and the free pages in the
    /// meta page, makes sure the store reaches the frontier after a recovery (F3), and returns what each changed
    /// map page now says.
    fn finish_space(&mut self) -> Result<Vec<(u64, Summary)>> {
        let layout = self.layout;
        for &page in &self.freed {
            let map = self
                .dirty
                .get_mut(&layout.map_page(layout.region(page)))
                .expect("freeing copied the map page");
            map::set(&mut map[..], layout.bit(page), 1, true);
        }
        self.free += self.freed.len() as u64;
        if let Some(meta) = self.dirty.get_mut(&META_PAGE) {
            meta[..8].copy_from_slice(&self.frontier.to_le_bytes());
            meta[8..16].copy_from_slice(&self.free.to_le_bytes());
        }
        let last = self.frontier - 1;
        if self.frontier > self.start_frontier && !self.dirty.contains_key(&last) {
            self.add_zeroed([last])?;
        }
        Ok(self
            .dirty
            .iter()
            .filter(|(page, _)| layout.is_map(**page))
            .map(|(&page, map)| (layout.region(page), map::summary(&map[..])))
            .collect())
    }

    /// Makes room for `new_pages` more changed pages and for a store of at least `file_pages` pages. With a
    /// maximum size, the file, the scratch area and this transaction's changes count together (T14); when they
    /// do not fit, the versions no read needs any more are written in place first, and only then the operation
    /// fails with the maximum-size class.
    fn reserve(&mut self, new_pages: u64, file_pages: u64) -> Result<()> {
        let pager = self.engine.scratch.pager();
        let Some(max_size) = self.engine.max_size else {
            if file_pages > pager.page_count() {
                pager.ensure_pages(file_pages)?;
            }
            return Ok(());
        };
        let mut flushed = false;
        loop {
            let file = pager.info().size_bytes.max(file_pages * PAGE_SIZE_U64);
            let memory = self.engine.scratch.scratch_bytes()
                + (self.dirty.len() as u64 + new_pages) * PAGE_SIZE_U64;
            let needed = file.saturating_add(memory);
            if needed <= max_size {
                if file_pages > pager.page_count() {
                    pager.ensure_pages_within(file_pages, Some(max_size - memory))?;
                }
                return Ok(());
            }
            if flushed {
                return Err(Error::MaxSize { max_size, needed });
            }
            self.writer.flush()?;
            flushed = true;
        }
    }

    fn seal(&mut self) -> Result<Vec<(u64, SealedPage)>> {
        let mut pages = Vec::with_capacity(self.dirty.len());
        for (page_number, payload) in std::mem::take(&mut self.dirty) {
            let mut page = alloc_sealed()?;
            seal_page(page_number, &payload[..], &mut page);
            pages.push((page_number, page));
        }
        Ok(pages)
    }

    /// Records the first failure of the transaction (T11) and whether the store stopped accepting writes (T12).
    fn track<T>(&self, result: Result<T>) -> Result<T> {
        if let Err(error) = &result {
            let mut failure = self.failure.borrow_mut();
            if failure.is_none() {
                *failure = Some(error.duplicate());
            }
            drop(failure);
            self.engine.note_failure();
        }
        result
    }
}

impl Drop for WriteTransaction<'_> {
    fn drop(&mut self) {
        self.engine.release_turn();
    }
}
