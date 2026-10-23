//! Read and write transactions (Transactions PRD: T1, T2, T8, T10, T11, T14).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;

use muninndb_pager::format::{PAGE_SIZE_U64, seal_page};
use muninndb_pager::{
    PayloadBuf, ScratchWriter, SealedPage, Snapshot, alloc_payload, alloc_sealed,
};

use crate::{Engine, Error, FIRST_PAGE, META_PAGE, Point, Result};

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
    /// Pages this transaction changed, by number; the meta page among them when it reserved pages.
    dirty: BTreeMap<u64, PayloadBuf>,
    /// Next page to reserve.
    next_page: u64,
    /// The first operation that failed: the transaction can no longer be committed (T11).
    failure: RefCell<Option<Error>>,
}

impl<'e> WriteTransaction<'e> {
    pub(crate) fn new(engine: &'e Engine, writer: ScratchWriter<'e>) -> WriteTransaction<'e> {
        WriteTransaction {
            next_page: engine.next_page.load(Ordering::Acquire),
            engine,
            writer,
            dirty: BTreeMap::new(),
            failure: RefCell::new(None),
        }
    }

    /// The payload of page `page_number` as this transaction sees it: its own change, or what was committed
    /// before it got its turn (T2). The reference lives until the next change of this transaction (T8).
    ///
    /// # Panics
    ///
    /// If the page was never reserved.
    pub fn page(&self, page_number: u64) -> Result<&[u8]> {
        self.check_reserved(page_number);
        if let Some(payload) = self.dirty.get(&page_number) {
            return Ok(&payload[..]);
        }
        let result = self.writer.snapshot().page(page_number);
        self.track(result)
    }

    /// The payload of page `page_number` to change it. The first change copies the page into this transaction
    /// (T14).
    ///
    /// # Panics
    ///
    /// If the page was never reserved.
    pub fn page_mut(&mut self, page_number: u64) -> Result<&mut [u8]> {
        self.check_reserved(page_number);
        if !self.dirty.contains_key(&page_number) {
            let result = self.copy_for_write(page_number);
            self.track(result)?;
        }
        Ok(&mut self.dirty.get_mut(&page_number).expect("copied above")[..])
    }

    /// Reserves a new page, zeroed, and returns its number.
    pub fn allocate(&mut self) -> Result<u64> {
        let result = self.allocate_page();
        self.track(result)
    }

    /// Commits the transaction durably (J4 of the Journal PRD): it returns once the commit survives a crash, and
    /// its changes become visible to every transaction opened after this returns (T2). If an operation failed, the
    /// commit fails with the class of the first failure and applies nothing (T11). A transaction without changes
    /// commits without writing anything (T10). The transaction ends either way.
    pub fn commit(self) -> Result<()> {
        self.commit_with(true)
    }

    /// Commits the transaction in deferred mode (J10): it returns once the commit is written to the journal,
    /// without waiting for it to be durable. It survives a crash of the process, and becomes durable with the next
    /// durable commit with changes, [`Engine::make_durable`] or the close. When 256 deferred commits or 1 MiB of
    /// them are pending, the one that reaches that limit makes them durable before it returns (J5).
    pub fn commit_deferred(self) -> Result<()> {
        self.commit_with(false)
    }

    fn commit_with(mut self, durable: bool) -> Result<()> {
        if let Some(error) = self.failure.get_mut().take() {
            return Err(error);
        }
        if self.dirty.is_empty() {
            return Ok(());
        }
        self.engine.hook(Point::CommitFlush);
        self.engine.hook(Point::CommitSeal);
        let reserved = self.dirty.contains_key(&META_PAGE);
        if let Some(meta) = self.dirty.get_mut(&META_PAGE) {
            meta[..8].copy_from_slice(&self.next_page.to_le_bytes());
        }
        let sealed = self.seal();
        let pages = self.track(sealed)?;
        self.engine.hook(Point::CommitPublish);
        let committed = self.writer.commit(&pages, durable);
        self.track(committed)?;
        self.engine
            .next_page
            .store(self.next_page, Ordering::Release);
        if reserved {
            self.engine.meta_written.store(true, Ordering::Release);
        }
        // The turn goes to the next writer before the transfer, which never blocks it (J5).
        let engine = self.engine;
        drop(self);
        engine.transfer();
        Ok(())
    }

    /// Aborts the transaction: none of its changes apply (T10).
    pub fn abort(self) {}

    fn check_reserved(&self, page_number: u64) {
        assert!(
            (FIRST_PAGE..self.next_page).contains(&page_number),
            "page {page_number} was never reserved"
        );
    }

    fn copy_for_write(&mut self, page_number: u64) -> Result<()> {
        self.reserve(1, 0)?;
        let mut payload = alloc_payload()?;
        payload.copy_from_slice(self.writer.snapshot().page(page_number)?);
        self.dirty.insert(page_number, payload);
        Ok(())
    }

    fn allocate_page(&mut self) -> Result<u64> {
        let page_number = self.next_page;
        let meta_copy = !self.dirty.contains_key(&META_PAGE);
        self.reserve(1 + u64::from(meta_copy), page_number + 1)?;
        if meta_copy {
            let mut meta = alloc_payload()?;
            if self.engine.meta_written.load(Ordering::Acquire) {
                meta.copy_from_slice(self.writer.snapshot().page(META_PAGE)?);
            }
            self.dirty.insert(META_PAGE, meta);
        }
        self.dirty.insert(page_number, alloc_payload()?);
        self.next_page += 1;
        Ok(page_number)
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
