//! Read and write transactions (Transactions PRD: T1, T2, T8, T10, T11, T14), and the reservations and frees of
//! free space inside a write (free-space PRD: F3–F9).
//!
//! Value runs (large values execution, D4 and D5): a layer above stores a large value in a run of contiguous pages,
//! reserved like any range (F5) but kept by the write as one contiguous buffer, which it fills once, when it reserves
//! it; runs are never changed afterwards. Every transaction reads a run as one sequence without copying it: a write
//! from its own buffer, or else, like any reader, from the run's version in the temporary area or from the store.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::Ordering::Relaxed;

use krummi_pager::format::{PAGE_HEADER_SIZE, PAGE_SIZE, PAGE_SIZE_U64, seal_run};
use krummi_pager::{
    PageChange, PayloadBuf, RunBuf, ScratchWriter, Snapshot, alloc_payload, growth_target,
};

#[cfg(feature = "test-hooks")]
use crate::LayoutEvent;
use crate::map;
use crate::space::Layout;
use crate::{
    Engine, Error, FIRST_PAGE, MAP_PAGE, META_PAGE, PageCheck, Point, Result, meta_field,
    meta_is_blank, slot_offset,
};

/// A read transaction: a snapshot of the store as it was when it opened (T2). It can be sent to and shared
/// between threads (T6); it ends when it is dropped.
pub struct ReadTransaction<'e> {
    snapshot: Snapshot<'e>,
    /// The pages, runs and meta slots read, while tests count them.
    #[cfg(feature = "test-hooks")]
    pages_read: std::sync::atomic::AtomicU64,
}

impl<'e> ReadTransaction<'e> {
    pub(crate) fn new(snapshot: Snapshot<'e>) -> ReadTransaction<'e> {
        ReadTransaction {
            snapshot,
            #[cfg(feature = "test-hooks")]
            pages_read: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// How many pages, value runs and meta slots this transaction read so far (tests of the tables with schema PRD:
    /// an operation reads the same pages whatever the size of a table, AC9).
    #[cfg(feature = "test-hooks")]
    pub fn pages_read(&self) -> u64 {
        self.pages_read.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn count_read(&self) {
        #[cfg(feature = "test-hooks")]
        self.pages_read
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// The payload of page `page_number` as this transaction sees it, without copying it (D11, T16). The
    /// reference lives as long as the transaction (T8).
    pub fn page(&self, page_number: u64) -> Result<&[u8]> {
        self.count_read();
        self.snapshot.page(page_number)
    }

    /// The payload of page `page_number`, as [`page`](Self::page), with what its read tells about its layout check:
    /// the integrity code its read verified when it comes from the store (C2 of the remembered node layout check
    /// PRD), or whether a version of the scratch area already passed the check (U1 of the version checks PRD).
    pub fn page_and_check(&self, page_number: u64) -> Result<(&[u8], PageCheck<'_>)> {
        self.count_read();
        let (page, origin) = self.snapshot.page_and_code(page_number)?;
        Ok((page, origin.into()))
    }

    /// Asks the processor to bring page `page_number`, as this transaction sees it, into its caches without reading
    /// it, for a read soon after: a hint that checks and counts nothing.
    pub fn prefetch(&self, page_number: u64) {
        self.snapshot.prefetch(page_number);
    }

    /// Records that a layer above checked the layout of a version of the scratch area that
    /// [`page_and_check`](Self::page_and_check) returned with `mark`, and whether it `passed`: a pass holds for every
    /// later read of that version (U1 of the version checks PRD).
    pub fn version_checked(&self, mark: krummi_pager::VersionMark<'_>, passed: bool) {
        self.snapshot.version_checked(mark, passed);
    }

    /// Whether the content of page `page_number` with integrity code `code` already passed the check of the layers
    /// above while the store is open (C1, C2).
    pub fn is_checked(&self, page_number: u64, code: u64) -> bool {
        self.snapshot
            .scratch()
            .pager()
            .is_checked(page_number, code)
    }

    /// Remembers that the content of page `page_number` with integrity code `code` passed the check of the layers
    /// above (C1, C2).
    pub fn remember_checked(&self, page_number: u64, code: u64) {
        self.snapshot
            .scratch()
            .pager()
            .remember_checked(page_number, code);
    }

    /// Whether the content of page `page_number` with integrity code `code` is remembered as checked because it kept
    /// the check of the page it was copied from (Q3 of the written pages PRD); only debug builds tell, for Q4.
    pub fn layout_kept(&self, page_number: u64, code: u64) -> bool {
        self.snapshot
            .scratch()
            .pager()
            .layout_kept(page_number, code)
    }

    /// The content of the value run of `count` pages from page `first` as this transaction sees it, after the run's
    /// header, as one contiguous sequence without copying it (V2 of the large values PRD, T16). The reference lives
    /// as long as the transaction (T8). A damaged run, or pages that are not such a run, are the damage class with
    /// page `first` (G6, V7).
    ///
    /// # Panics
    ///
    /// If `first` is one of the store's header pages.
    pub fn run(&self, first: u64, count: u64) -> Result<&[u8]> {
        self.count_read();
        Ok(&self.snapshot.run(first, count)?[PAGE_HEADER_SIZE..])
    }

    /// Whether a value run of `count` pages from page `first` may hold data of the layers above, as
    /// [`may_hold_data`](Self::may_hold_data) for each of its pages: the whole run lies within the store's pages.
    pub fn may_hold_run(&self, first: u64, count: u64) -> bool {
        let page_count = self.snapshot.scratch().pager().page_count();
        first
            .checked_add(count)
            .is_some_and(|end| count > 0 && self.may_hold_data(first) && end <= page_count)
    }

    /// Slot `slot` of the meta page as this transaction sees it; `0` if it was never set.
    ///
    /// # Panics
    ///
    /// If `slot` is not below [`META_SLOTS`](crate::META_SLOTS).
    pub fn slot(&self, slot: usize) -> Result<u64> {
        self.count_read();
        let at = slot_offset(slot);
        match self.snapshot.page(META_PAGE) {
            Ok(meta) => Ok(meta_field(meta, at)),
            // Without a version in the scratch area that this snapshot sees, a blank meta page in the store means
            // nothing was committed before it: no transfer writes the page while this snapshot is open.
            Err(_) if meta_is_blank(self.snapshot.scratch().pager()) => Ok(0),
            Err(error) => Err(error),
        }
    }

    /// Whether page `page_number` may hold data of the layers above: a page after the store's first own pages. A
    /// layer that reads a page number from a page checks it before reading it, so that a damaged number is damage
    /// (G6) and not a misuse.
    pub fn may_hold_data(&self, page_number: u64) -> bool {
        page_number >= FIRST_PAGE
    }

    /// The damage error of page `page_number`, for a layer above that finds the page inconsistent although its
    /// checksum is valid (G6).
    pub fn damaged(&self, page_number: u64) -> Error {
        Error::Damaged {
            folder: self
                .snapshot
                .scratch()
                .pager()
                .folder()
                .map(Path::to_path_buf),
            page: Some(page_number),
        }
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
    dirty: BTreeMap<u64, Changed>,
    /// Value runs this transaction reserved, by first page, each a buffer of whole pages with room for its header
    /// (D4); their pages are not in `dirty`.
    runs: BTreeMap<u64, RunBuf>,
    /// Pages of `runs`, which count as changed pages (T14).
    run_pages: u64,
    /// The frontier when the transaction got its turn, and as this transaction moved it (F3).
    start_frontier: u64,
    frontier: u64,
    /// Free pages of the map this transaction may still reserve: the committed ones it did not take, plus the ones
    /// it skipped at the frontier (F5). Its own frees join them only at commit (F6).
    free: u64,
    /// Pages this transaction freed: they are set free in the map at commit (F6, F8).
    freed: BTreeSet<u64>,
    /// Regions where this transaction made pages free, whose longest free run the engine must forget.
    gained: BTreeSet<u64>,
    /// The leaf a tree layer above last found on a tree's right edge, for [`recent_leaf`](Self::recent_leaf); a
    /// mutex, never contended, so that it adds no type that cannot be shared between threads to those the compile-fail
    /// tests list.
    recent_leaf: std::sync::Mutex<RecentLeaf>,
    /// Whether a leaf is remembered, read without the lock.
    recent_on: std::sync::atomic::AtomicBool,
    /// The first operation that failed: the transaction can no longer be committed (T11).
    failure: RefCell<Option<Error>>,
    /// The changes, reads and layout checks of changed pages, while a test records them; a mutex, so that the tests'
    /// build adds no type that cannot be shared between threads.
    #[cfg(feature = "test-hooks")]
    layout_log: std::sync::Mutex<Option<Vec<LayoutEvent>>>,
    /// The pages, runs and meta slots read, while tests count them.
    #[cfg(feature = "test-hooks")]
    pages_read: std::sync::atomic::AtomicU64,
}

/// A page this transaction changed, and how. Its layout is never checked by this transaction (H1 of the commit checks
/// PRD).
struct Changed {
    payload: PayloadBuf,
    /// The transaction created the page, zeroed, rather than copying its newest version: the journal keeps what
    /// differs from zeros, not from that version (Y1 of the journal page diffs PRD).
    created: bool,
    /// The page keeps the layout check of the page or version it was copied from: that one's layout was checked or
    /// remembered when the copy was made, and every change since only overwrote values of entries in the leaf with
    /// others of the same length (Q3 of the written pages PRD). Any other change clears it for good.
    keeps_check: bool,
}

/// The leaf on a tree's right edge that a layer above found in this transaction, and the first key it held (S1 of the
/// ordered-write cache execution, as Voron's recently found pages, for keys that come in order).
#[derive(Default)]
struct RecentLeaf {
    /// The tree's root and the leaf's page, `None` when nothing is remembered.
    at: Option<(u64, u64)>,
    /// The first key the leaf held: it holds every key from it on.
    lower: Vec<u8>,
}

impl Changed {
    /// A page just created, zeroed, or copied for this transaction; `keeps_check` for a copy of a checked page that
    /// is about to change only by same-length value overwrites (Q3).
    fn new(payload: PayloadBuf, created: bool, keeps_check: bool) -> Changed {
        Changed {
            payload,
            created,
            keeps_check,
        }
    }

    /// The payload to change it in any way: the page no longer keeps a layout check (Q3).
    fn payload_mut(&mut self) -> &mut [u8] {
        self.keeps_check = false;
        &mut self.payload[..]
    }

    /// The payload to overwrite values in it with others of the same length, which keeps its layout check (Q3).
    fn payload_same_layout(&mut self) -> &mut [u8] {
        &mut self.payload[..]
    }
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
            runs: BTreeMap::new(),
            run_pages: 0,
            freed: BTreeSet::new(),
            gained: BTreeSet::new(),
            failure: RefCell::new(None),
            recent_leaf: std::sync::Mutex::default(),
            recent_on: std::sync::atomic::AtomicBool::new(false),
            #[cfg(feature = "test-hooks")]
            layout_log: std::sync::Mutex::new(None),
            #[cfg(feature = "test-hooks")]
            pages_read: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// The payload of page `page_number` as this transaction sees it: its own change, or what was committed
    /// before it got its turn (T2). The reference lives until the next change of this transaction (T8). Reading a
    /// page this transaction freed, or a page of a value run it reserved, fails with the misuse class (F8, T11).
    ///
    /// # Panics
    ///
    /// If the page was never reserved.
    pub fn page(&self, page_number: u64) -> Result<&[u8]> {
        self.page_with(page_number).map(|(page, _)| page)
    }

    /// The payload of page `page_number`, as [`page`](Self::page), with what a layer above needs to check its
    /// layout: the integrity code stored in it when it comes from the store, which its read verified or a read of
    /// this opening already did (S2 of the writer checks PRD), that this transaction changed it, whose layout it
    /// never checks (H1 of the commit checks PRD), or whether a version of the scratch area already passed the check
    /// (C2, C3 of the remembered node layout check PRD, U1 of the version checks PRD).
    ///
    /// # Panics
    ///
    /// If the page was never reserved.
    pub fn page_and_check(&self, page_number: u64) -> Result<(&[u8], PageCheck<'_>)> {
        let read = self.page_with(page_number);
        #[cfg(feature = "test-hooks")]
        if let Ok((_, PageCheck::Changed)) = read {
            self.log(LayoutEvent::Read(page_number));
        }
        read
    }

    fn page_with(&self, page_number: u64) -> Result<(&[u8], PageCheck<'_>)> {
        self.count_read();
        self.track(self.engine.injected_fault())?;
        self.check_reserved(page_number, 1);
        if let Err(error) = self.check_page(page_number) {
            return self.track(Err(error));
        }
        if let Some(changed) = self.dirty.get(&page_number) {
            return Ok((&changed.payload[..], PageCheck::Changed));
        }
        let result = self.writer.snapshot().page_and_code(page_number);
        let (page, origin) = self.track(result)?;
        Ok((page, origin.into()))
    }

    /// Records that a layer above checked the layout of page `page_number`, read with
    /// [`page_and_check`](Self::page_and_check) as `check` says, and whether it `passed`. A version of the scratch area
    /// that passes is not checked again by any transaction while it stays there (U1 of the version checks PRD).
    pub fn layout_checked(&self, page_number: u64, check: PageCheck<'_>, passed: bool) {
        #[cfg(feature = "test-hooks")]
        self.log(LayoutEvent::Check(page_number));
        #[cfg(not(feature = "test-hooks"))]
        let _ = page_number;
        if let PageCheck::Version { mark, .. } = check {
            self.writer.snapshot().version_checked(mark, passed);
        }
    }

    /// Whether the content of page `page_number` with integrity code `code` already passed the check of the layers
    /// above while the store is open (C1, C2).
    pub fn is_checked(&self, page_number: u64, code: u64) -> bool {
        self.writer
            .snapshot()
            .scratch()
            .pager()
            .is_checked(page_number, code)
    }

    /// Remembers that the content of page `page_number` with integrity code `code` passed the check of the layers
    /// above (C1, C2).
    pub fn remember_checked(&self, page_number: u64, code: u64) {
        self.writer
            .snapshot()
            .scratch()
            .pager()
            .remember_checked(page_number, code);
    }

    /// Whether the content of page `page_number` with integrity code `code` is remembered as checked because it kept
    /// the check of the page it was copied from (Q3 of the written pages PRD); only debug builds tell, for Q4.
    pub fn layout_kept(&self, page_number: u64, code: u64) -> bool {
        self.writer
            .snapshot()
            .scratch()
            .pager()
            .layout_kept(page_number, code)
    }

    /// The payload of page `page_number` to change it. The first change copies the page into this transaction
    /// (T14). Changing one of the store's own pages, a page this transaction freed, or a page of a value run it
    /// reserved, fails with the misuse class (F8, T11).
    ///
    /// # Panics
    ///
    /// If the page was never reserved.
    pub fn page_mut(&mut self, page_number: u64) -> Result<&mut [u8]> {
        let result = self
            .engine
            .injected_fault()
            .and_then(|()| self.writable(page_number));
        self.track(result)?;
        #[cfg(feature = "test-hooks")]
        self.log(LayoutEvent::Change(page_number));
        Ok(self
            .dirty
            .get_mut(&page_number)
            .expect("copied above")
            .payload_mut())
    }

    /// The payload of page `page_number`, as [`page_mut`](Self::page_mut), for a layer above that only overwrites the
    /// value of entries the leaf already holds, stored in the leaf, with others of the same length, which leaves the
    /// node's layout as it was. A copy made here of a page or version whose layout was checked or remembered (C2, U1 of
    /// the version checks PRD, or kept itself), and changed only through this method until the commit, keeps that
    /// layout check: its version is published as checked (Q3 of the written pages PRD). Any change through
    /// [`page_mut`](Self::page_mut) ends that for the rest of the transaction. The caller vouches that it changes
    /// nothing else; debug builds check the layout of such a version on every read by another transaction (Q4).
    ///
    /// # Panics
    ///
    /// If the page was never reserved.
    pub fn page_mut_same_layout(&mut self, page_number: u64) -> Result<&mut [u8]> {
        let result = self
            .engine
            .injected_fault()
            .and_then(|()| self.writable_with(page_number, true));
        self.track(result)?;
        #[cfg(feature = "test-hooks")]
        self.log(LayoutEvent::Change(page_number));
        Ok(self
            .dirty
            .get_mut(&page_number)
            .expect("copied above")
            .payload_same_layout())
    }

    /// The leaf on the right edge of the tree at `root` that holds `key`, as [`remember_leaf`](Self::remember_leaf)
    /// last named it, when `key` is not below the first key it gave. A key of that tree below it forgets the leaf:
    /// keys are not coming in order, and remembering would cost a lock on every change for nothing (W3 of the write
    /// optimizations execution). The transaction also forgets that leaf whenever it reserves or frees pages, or an
    /// operation fails: a tree layer above changes the shape of a tree (splits, merges, a new root, a tree freed) only
    /// by reserving or freeing pages, and freed pages are not reserved again in the same transaction (F6), so until
    /// then the leaf is the same tree's last and holds every key from that first one on. A change of a remembered leaf
    /// skips the walk from the root (S1 of the ordered-write cache execution). A transaction that failed (T11)
    /// remembers nothing: a failed operation can leave a tree half changed, where only a walk names the leaf the other
    /// operations of the transaction would.
    #[inline]
    pub fn recent_leaf(&self, root: u64, key: &[u8]) -> Option<u64> {
        if !self.recent_on.load(Relaxed) || self.has_failed() {
            return None;
        }
        let mut recent = self.hint();
        let (tree, leaf) = recent.at?;
        if tree != root {
            return None;
        }
        if key < recent.lower.as_slice() {
            recent.at = None;
            self.recent_on.store(false, Relaxed);
            return None;
        }
        Some(leaf)
    }

    /// Remembers `leaf` as the last leaf of the tree at `root`, holding every key from `lower` on, in place of the leaf
    /// remembered before. The caller vouches for the leaf as the tree is now; see [`recent_leaf`](Self::recent_leaf).
    /// Without memory for the key it remembers nothing. Meant only for the tree layer, which names a leaf it has just
    /// reached by walking the tree's right edge (R1-001 of the RDD review of T4 of the write optimizations
    /// execution): a wrong leaf would send later changes to it.
    #[doc(hidden)]
    pub fn remember_leaf(&self, root: u64, leaf: u64, lower: &[u8]) {
        let mut recent = self.hint();
        let recent = &mut *recent;
        recent.at = None;
        self.recent_on.store(false, Relaxed);
        recent.lower.clear();
        // A memory to skip a walk is not worth failing for, nor an injected allocation failure of the tests.
        if recent.lower.try_reserve(lower.len()).is_err() {
            return;
        }
        recent.lower.extend_from_slice(lower);
        recent.at = Some((root, leaf));
        self.recent_on.store(true, Relaxed);
    }

    /// Forgets the leaf of [`recent_leaf`](Self::recent_leaf), before a change that may reshape a tree.
    fn forget_leaf(&self) {
        if self.recent_on.load(Relaxed) {
            self.recent_on.store(false, Relaxed);
            self.hint().at = None;
        }
    }

    /// The leaf of [`recent_leaf`](Self::recent_leaf), locked; nothing panics while it is.
    fn hint(&self) -> std::sync::MutexGuard<'_, RecentLeaf> {
        self.recent_leaf
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
        self.forget_leaf();
        let result = self
            .engine
            .injected_fault()
            .and_then(|()| self.reserve_run(count, false));
        self.track(result)
    }

    /// Reserves a value run of exactly `count` contiguous pages as [`allocate_run`](Self::allocate_run) does (F5),
    /// kept as one contiguous buffer (D4), and returns its first page and its content after the run's header,
    /// zeroed, for the caller to fill now: a run is never changed afterwards. Its pages are not pages of their own:
    /// it is read with [`run`](Self::run) and freed whole with [`free`](Self::free).
    pub fn allocate_value_run(&mut self, count: u64) -> Result<(u64, &mut [u8])> {
        self.forget_leaf();
        let result = self
            .engine
            .injected_fault()
            .and_then(|()| self.reserve_run(count, true));
        let first = self.track(result)?;
        let run = self.runs.get_mut(&first).expect("reserved above");
        Ok((first, &mut run[PAGE_HEADER_SIZE..]))
    }

    /// The content of the value run of `count` pages from page `first` as this transaction sees it, after the run's
    /// header, as one contiguous sequence without copying it (V2 of the large values PRD): its own buffer for a run
    /// it reserved, or else what was committed before it got its turn (T2). The reference lives until the next
    /// change of this transaction (T8). Pages that are not such a run are the damage class with page `first` (G6,
    /// V7); reading pages this transaction freed or changed as pages fails with the misuse class (F8, T11).
    ///
    /// # Panics
    ///
    /// If the pages were never reserved.
    pub fn run(&self, first: u64, count: u64) -> Result<&[u8]> {
        self.count_read();
        self.track(self.engine.injected_fault())?;
        self.check_reserved(first, count);
        let end = first + count;
        let result = if self.freed.range(first..end).next().is_some() {
            Err(self.engine.misuse("the page was freed by this transaction"))
        } else if self.dirty.range(first..end).next().is_some() {
            Err(self
                .engine
                .misuse("the page was changed by this transaction"))
        } else {
            match self.run_at(first) {
                Some((at, run)) if at == first && run_count(run) == count => {
                    return Ok(&run[PAGE_HEADER_SIZE..]);
                }
                None if self.runs.range(first..end).next().is_none() => {
                    self.writer.snapshot().run(first, count)
                }
                // Not exactly one of its own runs: what the store would report (D3).
                _ => Err(self.damage(first)),
            }
        };
        self.track(result).map(|run| &run[PAGE_HEADER_SIZE..])
    }

    /// Frees `count` pages from `first`, which must be in use as this transaction sees them; their changes in this
    /// transaction are dropped, and so is a value run it reserved there, which is freed whole. They become free for
    /// the writes that follow this commit, not for this transaction (F6, F8). Freeing the store's own pages, pages
    /// from the frontier on, pages already free, a page twice, or part of a value run this transaction reserved
    /// fails with the misuse class (T11).
    pub fn free(&mut self, first: u64, count: u64) -> Result<()> {
        self.forget_leaf();
        let result = self
            .engine
            .injected_fault()
            .and_then(|()| self.release(first, count));
        self.track(result)
    }

    /// Slot `slot` of the meta page as this transaction sees it, with its own changes; `0` if it was never set.
    /// A failure follows T11, like [`page`](Self::page).
    ///
    /// # Panics
    ///
    /// If `slot` is not below [`META_SLOTS`](crate::META_SLOTS).
    pub fn slot(&self, slot: usize) -> Result<u64> {
        self.count_read();
        let at = slot_offset(slot);
        if let Some(meta) = self.dirty.get(&META_PAGE) {
            return Ok(meta_field(&meta.payload[..], at));
        }
        if !self.engine.space.meta_written() {
            return Ok(0);
        }
        let meta = self.writer.snapshot().page(META_PAGE);
        Ok(meta_field(self.track(meta)?, at))
    }

    /// Sets slot `slot` of the meta page; the commit carries it. Setting a slot to the value it has changes nothing
    /// (T10).
    ///
    /// # Panics
    ///
    /// If `slot` is not below [`META_SLOTS`](crate::META_SLOTS).
    pub fn set_slot(&mut self, slot: usize, value: u64) -> Result<()> {
        let at = slot_offset(slot);
        if self.slot(slot)? == value {
            return Ok(());
        }
        let ensured = self.ensure_meta();
        self.track(ensured)?;
        let meta = self
            .dirty
            .get_mut(&META_PAGE)
            .expect("ensured above")
            .payload_mut();
        meta[at..at + 8].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    /// Whether page `page_number` may hold data of the layers above as this transaction sees it: a page it may
    /// read, reserved and not freed by it. A layer that reads a page number from a page checks it before reading
    /// it, so that a damaged number is damage (G6) and not a misuse.
    pub fn may_hold_data(&self, page_number: u64) -> bool {
        (FIRST_PAGE..self.frontier).contains(&page_number) && !self.freed.contains(&page_number)
    }

    /// Whether a value run of `count` pages from page `first` may hold data of the layers above as this transaction
    /// sees it, as [`may_hold_data`](Self::may_hold_data) for each of its pages.
    pub fn may_hold_run(&self, first: u64, count: u64) -> bool {
        first
            .checked_add(count)
            .is_some_and(|end| count > 0 && first >= FIRST_PAGE && end <= self.frontier)
            && self.freed.range(first..first + count).next().is_none()
    }

    /// The damage error of page `page_number`, for a layer above that finds the page inconsistent although its
    /// checksum is valid (G6). It becomes the transaction's failure, as any damage it meets (T11).
    pub fn damaged(&self, page_number: u64) -> Error {
        self.fail(self.damage(page_number))
    }

    fn damage(&self, page_number: u64) -> Error {
        Error::Damaged {
            folder: self.engine.folder.clone(),
            page: Some(page_number),
        }
    }

    /// Makes `error`, the failure of an operation of a layer above (such as running out of memory, P13), the
    /// transaction's failure, as the failures of its own operations are (T11), and returns it.
    pub fn fail(&self, error: Error) -> Error {
        self.track::<()>(Err(error)).unwrap_err()
    }

    /// Whether an operation of this transaction failed, so that it can no longer be committed (T11).
    pub fn has_failed(&self) -> bool {
        self.failure.borrow().is_some()
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
        if self.dirty.is_empty() && self.runs.is_empty() {
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
        self.track(finished)?;
        let meta_written = self.dirty.contains_key(&META_PAGE);
        // The pager seals the pages straight into the temporary area (O5 of the performance plan).
        let dirty = std::mem::take(&mut self.dirty);
        // One small value per page carries both flags, in the one collection besides the pages (Q6 of the written
        // pages PRD).
        let (pages, changes): (Vec<(u64, PayloadBuf)>, Vec<PageChange>) = dirty
            .into_iter()
            .map(|(page, changed)| {
                let change = PageChange::new(changed.created, changed.keeps_check);
                ((page, changed.payload), change)
            })
            .unzip();
        let runs = self.seal_runs();
        self.engine.hook(Point::CommitPublish);
        let committed = self.writer.commit(&pages, &changes, &runs);
        // The pages and the runs are copied into the temporary area now, or the commit failed: their buffers serve
        // the next copies and runs.
        self.engine.keep_payloads(pages);
        self.engine.keep_runs(runs);
        let txid = self.track(committed)?;
        // The point of no return (E2): the commit is in the journal's queue and the next writer sees it, and the
        // pages it freed (F6).
        self.engine.space.publish(
            txid,
            self.frontier,
            self.free,
            &self.gained,
            meta_written,
            self.engine.scratch.visible_txid(),
        );
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

    fn check_reserved(&self, first: u64, count: u64) {
        assert!(
            first >= FIRST_PAGE
                && first
                    .checked_add(count)
                    .is_some_and(|end| count > 0 && end <= self.frontier),
            "pages {first} to {first} + {count} were never reserved"
        );
    }

    /// Whether page `page_number` can be used as a page of its own: not freed by this transaction, and not in a value
    /// run it reserved (F8, D4).
    fn check_page(&self, page_number: u64) -> Result<()> {
        if self.freed.contains(&page_number) {
            Err(self.engine.misuse("the page was freed by this transaction"))
        } else if self.run_at(page_number).is_some() {
            Err(self.engine.misuse("the page is in a value run"))
        } else {
            Ok(())
        }
    }

    /// The value run this transaction reserved that holds page `page_number`, with its first page.
    fn run_at(&self, page_number: u64) -> Option<(u64, &RunBuf)> {
        let (&first, run) = self.runs.range(..=page_number).next_back()?;
        (page_number < first + run_count(run)).then_some((first, run))
    }

    fn writable(&mut self, page_number: u64) -> Result<()> {
        self.writable_with(page_number, false)
    }

    /// As [`writable`](Self::writable); a copy made for `same_layout` changes keeps the layout check of its source when
    /// that was checked or remembered (Q3 of the written pages PRD).
    fn writable_with(&mut self, page_number: u64, same_layout: bool) -> Result<()> {
        if self.layout.is_own(page_number) {
            return Err(self
                .engine
                .misuse("the store's own pages cannot be changed"));
        }
        self.check_reserved(page_number, 1);
        self.check_page(page_number)?;
        if !self.dirty.contains_key(&page_number) {
            self.copy_for_write(page_number, same_layout)?;
        }
        Ok(())
    }

    /// Copies page `page_number` into this transaction. With `same_layout`, the copy keeps the layout check of what it
    /// copies when that passed the check or kept it: a content of the store remembered in the table of checked contents
    /// (C2), or a version of the scratch area that passed or kept it (U1, Q3 of the written pages PRD).
    fn copy_for_write(&mut self, page_number: u64, same_layout: bool) -> Result<()> {
        self.reserve(1, 0)?;
        let mut payload = self.engine.payload_for_copy()?;
        let snapshot = self.writer.snapshot();
        let (page, origin) = snapshot.page_and_code(page_number)?;
        payload.copy_from_slice(page);
        let keeps_check = same_layout
            && match origin {
                krummi_pager::Origin::Store(code) => {
                    snapshot.scratch().pager().is_checked(page_number, code)
                }
                krummi_pager::Origin::Version { checked, .. } => checked,
            };
        self.dirty
            .insert(page_number, Changed::new(payload, false, keeps_check));
        Ok(())
    }

    /// Adds zeroed pages to this transaction.
    fn add_zeroed(&mut self, pages: impl IntoIterator<Item = u64>) -> Result<()> {
        for page in pages {
            self.dirty
                .insert(page, Changed::new(alloc_payload()?, true, false));
            #[cfg(feature = "test-hooks")]
            self.log(LayoutEvent::Change(page));
        }
        Ok(())
    }

    /// Adds the `count` pages from `first` that this transaction just reserved, zeroed: as pages, or as one value
    /// run (D4).
    fn add_reserved(&mut self, first: u64, count: u64, value_run: bool) -> Result<()> {
        if !value_run {
            return self.add_zeroed(first..first + count);
        }
        self.runs.insert(first, self.engine.run_for(count)?);
        self.run_pages += count;
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
        self.copy_for_write(META_PAGE, false)
    }

    /// The map page of `region` in this transaction, copied on first use.
    fn ensure_map(&mut self, region: u64) -> Result<&mut [u8]> {
        self.ensure_meta()?;
        let page = self.layout.map_page(region);
        if !self.dirty.contains_key(&page) {
            self.copy_for_write(page, false)?;
        }
        Ok(self
            .dirty
            .get_mut(&page)
            .expect("copied above")
            .payload_mut())
    }

    /// Reserves `count` contiguous pages, as pages or as one value run (`value_run`), and returns the first.
    fn reserve_run(&mut self, count: u64, value_run: bool) -> Result<u64> {
        if count == 0 || count > self.layout.cap() {
            return Err(self
                .engine
                .misuse("a range must have at least one page and fit in a region"));
        }
        if let Some(first) = self.take_from_map(count, value_run)? {
            return Ok(first);
        }
        self.take_at_frontier(count, value_run)
    }

    /// Takes the first run of `count` free pages of the map, reading the regions it needs (F4, F5, F9).
    fn take_from_map(&mut self, count: u64, value_run: bool) -> Result<Option<u64>> {
        if self.free < count {
            return Ok(None);
        }
        let engine = self.engine;
        let space = &engine.space;
        for region in 0..=self.layout.region(self.frontier - 1) {
            let map_page = self.layout.map_page(region);
            let found = match self.dirty.get(&map_page) {
                Some(map) => map::find_run(&map.payload[..], count).ok(),
                None if !space.may_hold(region, count) => None,
                None => {
                    let map = self.writer.snapshot().page(map_page)?;
                    map::find_run(map, count)
                        .inspect_err(|&longest| space.learn(region, longest))
                        .ok()
                }
            };
            let Some(bit) = found else { continue };
            let first = region * self.layout.region_pages() + bit;
            self.ensure_map(region)?;
            self.reserve(count, first + count)?;
            map::set(self.ensure_map(region)?, bit, count, false);
            self.add_reserved(first, count, value_run)?;
            self.free -= count;
            return Ok(Some(first));
        }
        Ok(None)
    }

    /// Takes `count` pages at the frontier; when they do not fit in its region, the rest of the region is left
    /// free and they start after the next region's map page, which this transaction writes (F3, F5).
    fn take_at_frontier(&mut self, count: u64, value_run: bool) -> Result<u64> {
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
            self.gained.insert(region);
        }
        if let Some(region) = enter {
            self.add_zeroed([layout.map_page(region)])?;
        }
        self.add_reserved(first, count, value_run)?;
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
        // A value run this transaction reserved goes whole: its own range, or none of it.
        let own_run = match self.run_at(first) {
            Some((at, run)) if at == first && run_count(run) == count => true,
            None if self.runs.range(first..end).next().is_none() => false,
            _ => return Err(self.engine.misuse("a value run is freed whole")),
        };
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
        if own_run {
            self.runs.remove(&first);
            self.run_pages -= count;
        }
        for page in first..end {
            self.dirty.remove(&page);
            self.freed.insert(page);
        }
        Ok(())
    }

    /// Before sealing: sets this transaction's frees in the map, writes the frontier and the free pages in the
    /// meta page, and makes sure the store reaches the frontier after a recovery (F3).
    fn finish_space(&mut self) -> Result<()> {
        let layout = self.layout;
        for &page in &self.freed {
            let region = layout.region(page);
            let map = self
                .dirty
                .get_mut(&layout.map_page(region))
                .expect("freeing copied the map page")
                .payload_mut();
            map::set(map, layout.bit(page), 1, true);
            self.gained.insert(region);
        }
        self.free += self.freed.len() as u64;
        if let Some(meta) = self.dirty.get_mut(&META_PAGE).map(Changed::payload_mut) {
            meta[..8].copy_from_slice(&self.frontier.to_le_bytes());
            meta[8..16].copy_from_slice(&self.free.to_le_bytes());
        }
        let last = self.frontier - 1;
        if self.frontier > self.start_frontier
            && !self.dirty.contains_key(&last)
            && self.run_at(last).is_none()
        {
            self.reserve(1, 0)?;
            self.add_zeroed([last])?;
        }
        Ok(())
    }

    /// Makes room for `new_pages` more changed pages and for a store of at least `file_pages` pages. With a
    /// maximum size, the file, the scratch area and this transaction's changes count together (T14); before a
    /// growth the waiting versions would limit, and when they do not fit, the versions no read needs any more are
    /// written in place first, and only then the operation fails with the maximum-size class (J7).
    fn reserve(&mut self, new_pages: u64, file_pages: u64) -> Result<()> {
        let pager = self.engine.scratch.pager();
        let Some(max_size) = self.engine.max_size else {
            if file_pages > pager.page_count() {
                pager.ensure_pages(file_pages)?;
            }
            return Ok(());
        };
        let mut flushed = false;
        // A growth is capped by what the scratch area holds at that moment. When that cap would cut it, the
        // versions no read needs are written in place first, so the store's size does not depend on when the
        // background transfer last ran (J7); otherwise the write does not wait for the transfer (J5).
        if file_pages > pager.page_count() && self.engine.scratch.scratch_bytes() > 0 {
            let growth =
                growth_target(pager.info().size_bytes, file_pages * PAGE_SIZE_U64).min(max_size);
            if growth.saturating_add(self.memory(new_pages)) > max_size {
                self.writer.flush()?;
                flushed = true;
            }
        }
        loop {
            let file = pager.info().size_bytes.max(file_pages * PAGE_SIZE_U64);
            let memory = self.memory(new_pages);
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

    /// The memory that counts against the maximum size with `new_pages` more changed pages: the scratch area and
    /// this transaction's changes, the pages of its value runs included (T14).
    fn memory(&self, new_pages: u64) -> u64 {
        let pages = self.dirty.len() as u64 + self.run_pages + new_pages;
        self.engine.scratch.scratch_bytes() + pages * PAGE_SIZE_U64
    }

    /// Seals the value runs in place, each as a whole (D1, D5).
    fn seal_runs(&mut self) -> Vec<(u64, RunBuf)> {
        self.run_pages = 0;
        std::mem::take(&mut self.runs)
            .into_iter()
            .map(|(first, mut run)| {
                seal_run(first, &mut run);
                (first, run)
            })
            .collect()
    }

    /// How many pages, value runs and meta slots this transaction read so far, its own changes included (tests of the
    /// tables with schema PRD: an operation reads the same pages whatever the size of a table, AC9).
    #[cfg(feature = "test-hooks")]
    pub fn pages_read(&self) -> u64 {
        self.pages_read.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn count_read(&self) {
        #[cfg(feature = "test-hooks")]
        self.pages_read
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Starts recording the changes of pages, the reads of changed pages and the layout checks the layers above
    /// report, dropping what it recorded so far (tests of the writer checks PRD).
    #[cfg(feature = "test-hooks")]
    pub fn record_layout(&self) {
        *self.layout_log.lock().unwrap() = Some(Vec::new());
    }

    /// What this transaction recorded since [`record_layout`](Self::record_layout), or since the last call; it goes
    /// on recording (tests of the writer checks PRD).
    #[cfg(feature = "test-hooks")]
    pub fn take_layout_log(&self) -> Vec<LayoutEvent> {
        self.layout_log
            .lock()
            .unwrap()
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    #[cfg(feature = "test-hooks")]
    fn log(&self, event: LayoutEvent) {
        if let Some(log) = self.layout_log.lock().unwrap().as_mut() {
            log.push(event);
        }
    }

    /// Records the first failure of the transaction (T11) and whether the store stopped accepting writes (T12).
    fn track<T>(&self, result: Result<T>) -> Result<T> {
        if let Err(error) = &result {
            // A change that failed half way may have left a tree's shape anywhere.
            self.forget_leaf();
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

/// Number of pages of a value run's buffer.
fn run_count(run: &RunBuf) -> u64 {
    (run.len() / PAGE_SIZE) as u64
}

impl Drop for WriteTransaction<'_> {
    fn drop(&mut self) {
        self.engine.release_turn();
    }
}
