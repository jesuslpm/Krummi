//! The scratch area (module 2): committed page versions that are not in the data file yet, and the snapshots that
//! read through them (T2, T14, T16 of the Transactions PRD).
//!
//! As in Voron, every page keeps its number. A write transaction changes copies of its pages; committing publishes
//! them as the newest version of each page. A version stays here while an open snapshot still needs the version
//! before it, which may be the one in the data file; once no snapshot is older than it, it is written in place and
//! leaves the scratch area. A version that leaves stays allocated until every snapshot that may hold a reference
//! to it has ended, so references returned by [`Snapshot::page`] stay valid for the whole snapshot (D11).
//!
//! With the journal's queue (E2, E3 of the grouped journal PRD), a commit is published for the next writer when it
//! passes its point of no return, and read snapshots see it only once it is durable: a read snapshot sees up to the
//! last durable commit, the writer's snapshot up to the last published one. The writer's snapshot sees the newest
//! version of every page, so it never keeps an older one alive; read snapshots opened later may need the newest
//! durable version of a page while a newer one is not durable yet, so that one is kept too.
//!
//! Why the shared page writes are sound: a flush writes page `n` only when every open snapshot sees the newest
//! version of `n` (none is older than it), and that version stays in the scratch area until the write ends, so
//! every snapshot reads `n` from here and nobody holds a reference into the data file's copy of `n`. Snapshots
//! that start afterwards read the newest version too. While the store is open only the transfer thread writes
//! versions in place, holding `flushing` (see the `transfer` module), so no two writes of a page overlap; the
//! close writes the rest once that thread has ended. A scratch area without the thread ([`Scratch::new`], for its
//! own tests) writes them on the caller's thread, under the same lock.

use std::cell::Cell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem::size_of;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::format::Header;
use crate::format::{HEADER_PAGES, PAGE_HEADER_SIZE, PAGE_PAYLOAD_SIZE, PAGE_SIZE, PAGE_SIZE_U64};
use crate::journal::Journal;
use crate::pager::{Info, Pager};
use crate::recovery;
use crate::sys;
use crate::temp::{Slot, TempArea};
use crate::transfer::{self, Demand, Pacing, Signal};

/// The payload of a page being changed by a write transaction.
pub type PayloadBuf = Box<[u8; PAGE_PAYLOAD_SIZE]>;
/// A whole page, sealed with its header and checksum, ready to be published.
pub type SealedPage = Box<[u8; PAGE_SIZE]>;
/// Pages of a commit copied into the temporary area, ready to be published (J7 of the Journal PRD).
pub struct Staged(Vec<(u64, Arc<Slot>)>);

/// A zeroed page payload, or the out-of-memory class if the system does not provide it (P13).
pub fn alloc_payload() -> Result<PayloadBuf> {
    alloc::<PAGE_PAYLOAD_SIZE>()
}

/// A zeroed whole page, or the out-of-memory class if the system does not provide it (P13).
pub fn alloc_sealed() -> Result<SealedPage> {
    alloc::<PAGE_SIZE>()
}

fn alloc<const N: usize>() -> Result<Box<[u8; N]>> {
    let bytes = sys::alloc_zeroed(N).map_err(|_| Error::OutOfMemory {
        requested: N as u64,
    })?;
    Ok(bytes.try_into().expect("exactly N bytes were allocated"))
}

/// A store with its scratch area.
pub struct Scratch {
    /// What the transfer thread shares with the store.
    shared: Arc<Shared>,
    /// Held by the one [`ScratchWriter`] that may exist at a time.
    writer: Mutex<()>,
    /// The transfer thread, which every store opened with [`Scratch::open`] has (J5).
    transfers: Option<JoinHandle<()>>,
}

/// The part of a store the transfer thread uses too.
pub(crate) struct Shared {
    /// `None` only after [`Scratch::close`] took it.
    pager: Option<Pager>,
    /// Where the versions live (J7 of the Journal PRD).
    area: Arc<TempArea>,
    /// The journal of a store on disk; `None` for a temporary store (J17).
    journal: Option<Journal>,
    state: Mutex<State>,
    /// Held while versions are written in place, so no two writes of a page overlap (J5).
    flushing: Mutex<Flushing>,
    /// Wakes the transfer thread.
    signal: Arc<Signal>,
    /// Called by a transfer while it holds `flushing`, once it knows what it writes (tests).
    transfer_hook: Mutex<Option<Hook>>,
    /// Called when a transfer fails outside any operation, so the transaction layer wakes its waiting requests
    /// (J13).
    failure_hook: Mutex<Option<Hook>>,
    /// Transfer passes so far (tests).
    passes: AtomicU64,
}

/// What `flushing` guards besides the writes themselves.
struct Flushing {
    /// When a transfer last flushed `krummi.data` to release journal files.
    last_data_sync: Option<Instant>,
}

type Hook = Arc<dyn Fn() + Send + Sync>;

struct State {
    /// Number of the newest committed version: commits are numbered on from what the header records (J6).
    last_committed: u64,
    /// Versions of each page that are only here, oldest first.
    versions: HashMap<u64, Vec<Version>>,
    /// Bytes taken by `versions`.
    bytes: u64,
    /// Open read snapshots by ascending sequence number; their versions never decrease along the list.
    live: Vec<Live>,
    /// The writer's snapshot, which sees commits that are not durable yet (E3).
    writer: Option<Live>,
    next_seq: u64,
    /// Versions that left `versions`, with the newest sequence number issued when they left: they are freed
    /// once no open snapshot has a sequence number up to that one.
    retired: Vec<(u64, Arc<Slot>)>,
}

struct Version {
    txid: u64,
    page: Arc<Slot>,
}

#[derive(Clone, Copy)]
struct Live {
    seq: u64,
    txid: u64,
}

impl Scratch {
    /// Recovers the store if it closed in a crash (J8 of the Journal PRD) and puts a scratch area and an empty
    /// journal in front of it, with the transfer thread (J5). Recovery removes the journal and the temporary area
    /// a crash left.
    pub fn open(mut pager: Pager) -> Result<Scratch> {
        let signal = Arc::new(Signal::new());
        let (journal, (transfers, sender)) = match pager.folder().map(Path::to_path_buf) {
            Some(folder) => {
                // Both threads start first: failing to start one leaves the store untouched (E13).
                let journal = Journal::start(&folder, &pager)?;
                let thread = transfer::start(&signal)?;
                let recovered = recovery::recover(&mut pager, &folder)?;
                journal.begin(recovered.last_txid, recovered.next_number);
                let durable = Arc::clone(&signal);
                journal.set_durable_hook(Some(Arc::new(move || durable.raise(0))));
                (Some(journal), thread)
            }
            None => (None, transfer::start(&signal)?),
        };
        let scratch = Scratch::with_journal(pager, journal, signal, Some(transfers));
        // The thread ends at once if this fails, and the store is closed without it.
        let _ = sender.send(Arc::clone(&scratch.shared));
        Ok(scratch)
    }

    /// Puts a scratch area in front of a store, with no journal and no transfer thread (tests of the scratch
    /// area).
    pub fn new(pager: Pager) -> Scratch {
        Scratch::with_journal(pager, None, Arc::new(Signal::new()), None)
    }

    fn with_journal(
        pager: Pager,
        journal: Option<Journal>,
        signal: Arc<Signal>,
        transfers: Option<JoinHandle<()>>,
    ) -> Scratch {
        let header = pager.header();
        let shared = Shared {
            area: Arc::new(TempArea::new(pager.folder())),
            journal,
            pager: Some(pager),
            flushing: Mutex::new(Flushing {
                last_data_sync: None,
            }),
            signal,
            transfer_hook: Mutex::new(None),
            failure_hook: Mutex::new(None),
            passes: AtomicU64::new(0),
            state: Mutex::new(State {
                last_committed: header.applied_txid,
                versions: HashMap::new(),
                bytes: 0,
                live: Vec::new(),
                writer: None,
                next_seq: 0,
                retired: Vec::new(),
            }),
        };
        Scratch {
            shared: Arc::new(shared),
            writer: Mutex::new(()),
            transfers,
        }
    }

    /// The store under the scratch area.
    pub fn pager(&self) -> &Pager {
        self.shared.pager()
    }

    /// Information about the store (P12).
    pub fn info(&self) -> Info {
        self.pager().info()
    }

    /// Bytes of page versions held only in the scratch area (T14).
    pub fn scratch_bytes(&self) -> u64 {
        self.lock().bytes
    }

    /// Opens a snapshot of everything committed and durable so far (T2, G3, E3).
    pub fn snapshot(&self) -> Result<Snapshot<'_>> {
        let mut state = self.lock();
        self.register(&mut state, false)
    }

    /// Waits until no other writer exists and returns the writer, with a snapshot of everything committed so far,
    /// durable or not (E3).
    pub fn begin_write(&self) -> Result<ScratchWriter<'_>> {
        let guard = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        let snapshot = {
            let mut state = self.lock();
            self.register(&mut state, true)?
        };
        Ok(ScratchWriter {
            snapshot,
            _guard: guard,
            _not_shared: PhantomData,
        })
    }

    /// Waits until every commit that passed its point of no return is durable (J11, E10). Fails at once with the I/O
    /// class when the store refuses writes (T12). A temporary store has nothing to do (J17).
    pub fn make_durable(&self) -> Result<()> {
        let pager = self.pager();
        pager.check_writable()?;
        match &self.shared.journal {
            Some(journal) => journal.make_durable(pager),
            None => Ok(()),
        }
    }

    /// Waits until commit `txid` is durable, or fails with the I/O class if its group failed (J4, E8). A temporary
    /// store has nothing to wait for.
    pub fn wait_durable(&self, txid: u64) -> Result<()> {
        match &self.shared.journal {
            Some(journal) => journal.wait_durable(txid),
            None => Ok(()),
        }
    }

    /// Wakes the commits waiting for room in the journal's queue, after the store stopped accepting writes (T12).
    pub fn wake(&self) {
        self.shared.wake();
    }

    /// Sets the function called when the journal's writer fails a group or the transfer thread fails a transfer,
    /// or none (T12, J13).
    pub fn set_failure_hook(&self, hook: Option<Arc<dyn Fn() + Send + Sync>>) {
        if let Some(journal) = &self.shared.journal {
            journal.set_failure_hook(hook.clone());
        }
        *self
            .shared
            .failure_hook
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = hook;
    }

    /// Stops the journal's writer from taking queued commits, or lets it go on (tests).
    pub fn set_journal_paused(&self, paused: bool) {
        if let Some(journal) = &self.shared.journal {
            journal.set_paused(paused);
        }
    }

    /// Number of commits waiting in the journal's queue (tests).
    pub fn journal_queued(&self) -> usize {
        self.shared.journal.as_ref().map_or(0, Journal::queued)
    }

    /// Writes of groups so far, one per journal file a group touches (tests).
    pub fn journal_writes(&self) -> u64 {
        self.shared.journal.as_ref().map_or(0, Journal::writes)
    }

    /// Last durable commit (tests).
    pub fn journal_durable(&self) -> u64 {
        self.shared
            .journal
            .as_ref()
            .map_or(self.lock().last_committed, Journal::durable)
    }

    /// Sets the bytes the journal's queue holds before a commit waits for room (tests).
    pub fn set_journal_queue_limit(&self, bytes: u64) {
        if let Some(journal) = &self.shared.journal {
            journal.set_queue_limit(bytes);
        }
    }

    /// Has the transfer thread run a pass now and waits for it; the pass releases the journal files it can however
    /// recently `krummi.data` was flushed (tests: AC-E17).
    pub fn transfer(&self) -> Result<()> {
        self.demand(Demand::Pass)
    }

    /// Transfer passes so far, by the transfer thread or [`Scratch::transfer`] (tests).
    pub fn transfer_passes(&self) -> u64 {
        self.shared.passes.load(Ordering::Relaxed)
    }

    /// Sets the minimum time between passes, the scratch bytes from which a pass starts without waiting for it,
    /// and the minimum time between flushes of `krummi.data` that release journal files (tests).
    pub fn set_transfer_pacing(
        &self,
        interval: Duration,
        bytes: u64,
        data_sync_interval: Duration,
    ) {
        self.shared.signal.set_pacing(Pacing {
            interval,
            bytes,
            data_sync_interval,
        });
    }

    /// Stops the transfer thread from starting passes, waiting for the one it may be running, or lets it go on
    /// (tests that need every transfer to run on their own thread).
    pub fn set_transfer_paused(&self, paused: bool) {
        self.shared.signal.set_paused(paused);
        if paused {
            drop(
                self.shared
                    .flushing
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner),
            );
        }
    }

    /// A flag that stays set while the transfer thread runs (tests).
    pub fn transfer_thread_alive(&self) -> Arc<AtomicBool> {
        self.shared.signal.alive()
    }

    /// Sets a function a transfer calls while it holds its lock, after it chose the versions it writes and before
    /// it writes them, or none (tests: AC-J17).
    pub fn set_transfer_hook(&self, hook: Option<Arc<dyn Fn() + Send + Sync>>) {
        *self
            .shared
            .transfer_hook
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = hook;
    }

    /// Sets the function the journal's writer calls between writing a group and making it durable (tests).
    pub fn set_journal_sync_hook(&self, hook: Option<Arc<dyn Fn() + Send + Sync>>) {
        if let Some(journal) = &self.shared.journal {
            journal.set_sync_hook(hook);
        }
    }

    /// Sets the size at which the journal goes on in a new file (tests).
    pub fn set_journal_file_limit(&self, bytes: u64) {
        if let Some(journal) = &self.shared.journal {
            journal.set_file_limit(bytes);
        }
    }

    /// Has the transfer thread transfer everything no read needs and record it in both header copies, so that the
    /// journal keeps only what later commits need, and waits for it (tests: AC-J18).
    pub fn checkpoint(&self) -> Result<()> {
        self.demand(Demand::Checkpoint)
    }

    /// Has the transfer thread do `demand` and waits for it, so that no other thread transfers (J5); a store
    /// without the thread does it here.
    fn demand(&self, demand: Demand) -> Result<()> {
        match self.shared.signal.demand(demand) {
            Some(outcome) => outcome,
            None => self.shared.serve(demand, Duration::ZERO).map(|_| ()),
        }
    }

    /// Writes every committed version to the data file and closes the store, which releases its lock even if a
    /// write fails (P14). A store that refused writes after a failure tries them again here (T12); if the close
    /// fails, the journal stays for the next opening to recover (J14).
    pub fn close(mut self) -> Result<()> {
        self.stop_transfers();
        let shared = Arc::get_mut(&mut self.shared).expect("the transfer thread ended");
        let flushed = shared.flush_all();
        let pager = shared.pager.take().expect("the store is open");
        let closed = pager.close();
        flushed.and(closed)
    }

    /// Ends the transfer thread, after the pass it may be running.
    fn stop_transfers(&mut self) {
        self.shared.signal.stop();
        if let Some(handle) = self.transfers.take() {
            // A thread that panicked left nothing half done that the close cannot redo.
            let _ = handle.join();
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.shared.lock()
    }

    /// Registers a read snapshot of what is durable, or the writer's snapshot of everything published (E3).
    fn register(&self, state: &mut State, writer: bool) -> Result<Snapshot<'_>> {
        let out_of_memory = || Error::OutOfMemory {
            requested: size_of::<Live>() as u64,
        };
        sys::snapshot_point().map_err(|_| out_of_memory())?;
        let txid = match (&self.shared.journal, writer) {
            (Some(journal), false) => state.last_committed.min(journal.durable()),
            _ => state.last_committed,
        };
        let live = Live {
            seq: state.next_seq,
            txid,
        };
        if writer {
            state.writer = Some(live);
        } else {
            state.live.try_reserve(1).map_err(|_| out_of_memory())?;
            state.live.push(live);
        }
        state.next_seq += 1;
        Ok(Snapshot {
            scratch: self,
            seq: live.seq,
            txid: live.txid,
            writer,
        })
    }
}

impl Drop for Scratch {
    /// A store dropped without closing still writes what was committed, without reporting a failure (T9).
    fn drop(&mut self) {
        self.stop_transfers();
        if let Some(shared) = Arc::get_mut(&mut self.shared)
            && shared.pager.is_some()
        {
            let _ = shared.flush_all();
        }
    }
}

impl Shared {
    fn pager(&self) -> &Pager {
        self.pager.as_ref().expect("the store is open")
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn wake(&self) {
        if let Some(journal) = &self.journal {
            journal.wake();
        }
    }

    /// One transfer pass (J5): writes in place the durable versions no read needs any more, then releases the
    /// journal files `krummi.data` no longer needs, unless it was flushed less than `data_sync_interval` ago, and
    /// then returns when to try that release again.
    pub(crate) fn pass(&self, data_sync_interval: Duration) -> Result<Option<Instant>> {
        let mut flushing = self.flushing.lock().unwrap_or_else(PoisonError::into_inner);
        self.passes.fetch_add(1, Ordering::Relaxed);
        self.flush_locked(&flushing, false)?;
        self.release_files(&mut flushing, data_sync_interval)
    }

    /// Does what a caller asked the transfer thread for, releasing journal files no sooner than
    /// `data_sync_interval` after the last flush of `krummi.data` unless the demand says otherwise, and returns
    /// when to try a release put off again.
    pub(crate) fn serve(
        &self,
        demand: Demand,
        data_sync_interval: Duration,
    ) -> Result<Option<Instant>> {
        if let Demand::Pass = demand {
            return self.pass(Duration::ZERO);
        }
        let mut flushing = self.flushing.lock().unwrap_or_else(PoisonError::into_inner);
        self.flush_locked(&flushing, true)?;
        if let Demand::Flush = demand {
            return self.release_files(&mut flushing, data_sync_interval);
        }
        if let Some(journal) = &self.journal {
            journal.start_new_file(self.pager())?;
        }
        self.record_applied(&mut flushing)?;
        Ok(None)
    }

    /// Lets the transaction layer know that a transfer failed outside any operation: the store already refuses
    /// writes (J13), and the commits waiting for room in the journal's queue fail at once (T12).
    pub(crate) fn report_failure(&self) {
        let hook = self
            .failure_hook
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(hook) = hook {
            hook();
        }
        self.wake();
    }

    /// Writes in place every newest version that no open snapshot is older than and that is durable, after
    /// retiring the versions that no snapshot can read any more; with `force`, it first waits until every queued
    /// commit is durable, so every such version goes (J5). It calls the test hook once it knows what it writes.
    fn flush_locked(&self, _flushing: &MutexGuard<'_, Flushing>, force: bool) -> Result<()> {
        let pager = self.pager();
        if force && let Some(journal) = &self.journal {
            journal.make_durable(pager)?;
        }
        let durable = self.journal.as_ref().map_or(u64::MAX, Journal::durable);
        let garbage;
        let mut pending: Vec<(u64, u64, Arc<Slot>)> = {
            let mut state = self.lock();
            let visible = state.last_committed.min(durable);
            garbage = state.collect_garbage(visible);
            let oldest = state.live.first().map(|l| l.txid);
            state
                .versions
                .iter()
                .filter_map(|(&page_number, versions)| {
                    let newest = versions.last()?;
                    (newest.txid <= durable && oldest.is_none_or(|oldest| oldest >= newest.txid))
                        .then(|| (page_number, newest.txid, Arc::clone(&newest.page)))
                })
                .collect()
        };
        drop(garbage);
        pending.sort_unstable_by_key(|(page_number, _, _)| *page_number);
        let hook = self
            .transfer_hook
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(hook) = hook {
            hook();
        }
        for (page_number, txid, page) in pending {
            // SAFETY: see the module documentation: every open snapshot reads this page from the scratch area,
            // where its newest version stays until the write ends, and `flushing` makes this the only write.
            unsafe { pager.write_sealed_shared(page_number, page.page())? };
            let mut state = self.lock();
            if state.versions[&page_number].last().map(|v| v.txid) == Some(txid) {
                let versions = state.versions.remove(&page_number).unwrap_or_default();
                state.retire(versions);
            }
        }
        Ok(())
    }

    /// Releases the journal files that `krummi.data` no longer needs (J6): only when the oldest one can go, since
    /// otherwise flushing `krummi.data` frees nothing, and no sooner than `data_sync_interval` after the last
    /// flush; otherwise returns when to try again.
    fn release_files(
        &self,
        flushing: &mut MutexGuard<'_, Flushing>,
        data_sync_interval: Duration,
    ) -> Result<Option<Instant>> {
        let Some(journal) = &self.journal else {
            return Ok(None);
        };
        if !journal.can_remove_files(self.applied(journal)) {
            return Ok(None);
        }
        if let Some(last) = flushing.last_data_sync {
            let due = last + data_sync_interval;
            if Instant::now() < due {
                return Ok(Some(due));
            }
        }
        self.record_applied(flushing)?;
        Ok(None)
    }

    /// Flushes `krummi.data`, records in both header copies the last commit it holds entirely, and removes the
    /// journal files neither copy needs any more (J6).
    fn record_applied(&self, flushing: &mut MutexGuard<'_, Flushing>) -> Result<()> {
        let Some(journal) = &self.journal else {
            return Ok(());
        };
        let pager = self.pager();
        pager.flush_data()?;
        flushing.last_data_sync = Some(Instant::now());
        let applied = self.applied(journal);
        // A file goes only once both copies record it, so both are written now (J8).
        for _ in 0..2 {
            pager.write_header(Header {
                applied_txid: applied,
                journal_first: journal.first_needed(applied),
                ..pager.header()
            })?;
        }
        journal.remove_files(pager, pager.applied_by_both_copies(), false)
    }

    /// The last commit `krummi.data` holds entirely once the versions written in place are flushed. Versions newer
    /// than the last durable commit are never written in place, and the ones they supersede are kept, so it never
    /// goes past it (J4, E3).
    fn applied(&self, journal: &Journal) -> u64 {
        self.lock().applied().min(journal.durable())
    }

    /// Makes the journal durable, writes the newest version of every page to the data file, records it in both
    /// header copies and removes the journal and the temporary area (J14).
    fn flush_all(&mut self) -> Result<()> {
        let pager = self.pager.as_mut().expect("the store is open");
        pager.allow_writes_again();
        if let Some(journal) = &self.journal {
            // Writes what is queued, retrying a group that failed, and stops the journal's writer (E12).
            journal.close(pager)?;
        }
        let state = self.state.get_mut().unwrap_or_else(PoisonError::into_inner);
        let mut pages: Vec<_> = state.versions.keys().copied().collect();
        pages.sort_unstable();
        let pages_written = pages.len();
        for page_number in pages {
            let page = &state.versions[&page_number]
                .last()
                .expect("a page in the scratch area has a version")
                .page;
            // SAFETY: `&mut self` rules out open snapshots, and with them any reference into the store's pages
            // and any other write.
            unsafe { pager.write_sealed_shared(page_number, page.page())? };
            let versions = state.versions.remove(&page_number).unwrap_or_default();
            state.bytes -= versions.len() as u64 * PAGE_SIZE_U64;
        }
        state.retired.clear();
        // Nothing was committed since the store opened: `krummi.data` and its header are already what a close
        // would leave, so a close writes nothing, unless a header copy is damaged and the close can repair it (P6).
        let unchanged = pages_written == 0
            && self.journal.as_ref().is_none_or(Journal::has_no_files)
            && !pager.has_damaged_header_copy();
        if let Some(journal) = self.journal.as_ref().filter(|_| !unchanged) {
            pager.flush_data()?;
            let applied = state.last_committed;
            for _ in 0..2 {
                pager.write_header(Header {
                    applied_txid: applied,
                    journal_first: journal.first_needed(applied),
                    ..pager.header()
                })?;
            }
            journal.remove_files(pager, applied, true)?;
        }
        self.area.remove().map_err(|e| pager.fail_write(e, 0))
    }
}

impl State {
    /// The last commit whose changes are all in the data file: every version left here is newer.
    fn applied(&self) -> u64 {
        self.versions
            .values()
            .filter_map(|versions| versions.first())
            .map(|v| v.txid - 1)
            .min()
            .unwrap_or(self.last_committed)
    }

    /// Moves page versions out of `versions`; they are freed once the snapshots that may use them end.
    fn retire(&mut self, versions: impl IntoIterator<Item = Version>) {
        let bound = self.next_seq.saturating_sub(1);
        for version in versions {
            self.bytes -= PAGE_SIZE_U64;
            self.retired.push((bound, version.page));
        }
    }

    /// Takes out the versions that no snapshot can read any more, for the caller to free. A version that has a
    /// newer one is read only by the snapshots at least as new as it and older than the newer one; with none
    /// open, nobody needs it, and nobody can hold a reference to it either: a snapshot that read it would be one
    /// of those. So it is freed at once, not retired, and a long snapshot does not make them pile up (T14). A
    /// version whose newer one is beyond `visible`, the newest commit read snapshots see, is kept for the read
    /// snapshots still to open (E3).
    fn collect_garbage(&mut self, visible: u64) -> Vec<Version> {
        let mut dropped = Vec::new();
        for versions in self.versions.values_mut() {
            if versions.len() < 2 {
                continue;
            }
            let mut kept = Vec::with_capacity(versions.len());
            let mut iter = std::mem::take(versions).into_iter().peekable();
            while let Some(version) = iter.next() {
                let needed = match iter.peek() {
                    None => true,
                    Some(next) if next.txid > visible => true,
                    Some(next) => {
                        let first = self.live.partition_point(|l| l.txid < version.txid);
                        self.live.get(first).is_some_and(|l| l.txid < next.txid)
                    }
                };
                if needed {
                    kept.push(version);
                } else {
                    dropped.push(version);
                }
            }
            *versions = kept;
        }
        self.bytes -= dropped.len() as u64 * PAGE_SIZE_U64;
        dropped
    }

    /// Frees the retired versions that no open snapshot, the writer's included, can hold a reference to.
    fn free_retired(&mut self) -> Vec<Arc<Slot>> {
        let oldest = self
            .live
            .first()
            .map(|l| l.seq)
            .into_iter()
            .chain(self.writer.map(|w| w.seq))
            .min()
            .unwrap_or(u64::MAX);
        let (free, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.retired)
            .into_iter()
            .partition(|(bound, _)| *bound < oldest);
        self.retired = keep;
        free.into_iter().map(|(_, page)| page).collect()
    }
}

/// A view of the store as it was when the snapshot opened (G3). It can be sent to and shared between threads
/// (T6), and the references it returns live as long as it does (D11).
pub struct Snapshot<'a> {
    scratch: &'a Scratch,
    seq: u64,
    txid: u64,
    /// Whether it is the writer's snapshot (E3).
    writer: bool,
}

impl<'a> Snapshot<'a> {
    /// Number of the newest committed version this snapshot sees.
    pub fn txid(&self) -> u64 {
        self.txid
    }

    /// The store this snapshot reads.
    pub fn scratch(&self) -> &'a Scratch {
        self.scratch
    }

    /// The payload of page `page_number` as this snapshot sees it, without copying it (D11, T16): the newest
    /// version not newer than the snapshot from the scratch area, or else the page in the store.
    ///
    /// # Panics
    ///
    /// If `page_number` is a header page.
    pub fn page(&self, page_number: u64) -> Result<&[u8]> {
        let state = self.scratch.lock();
        let version = state
            .versions
            .get(&page_number)
            .and_then(|versions| versions.iter().rev().find(|v| v.txid <= self.txid));
        if let Some(version) = version {
            let ptr = version.page.page().as_ptr();
            drop(state);
            // SAFETY: `ptr` points to the PAGE_SIZE bytes of a slot of the temporary area owned by an `Arc` in the
            // scratch area; the area keeps a slot's memory while the slot is held.
            // The scratch area keeps that `Arc` while this snapshot is open: a version leaves `versions` only by
            // being retired with a bound at least this snapshot's sequence number, and retired versions are freed
            // only once no open snapshot has a sequence number up to their bound. Published pages are never
            // written again.
            let page = unsafe { std::slice::from_raw_parts(ptr, PAGE_SIZE) };
            return Ok(&page[PAGE_HEADER_SIZE..]);
        }
        drop(state);
        // No version up to this snapshot is in the scratch area: the store has the one it sees, and no flush
        // writes this page while the snapshot is open, because any newer version is newer than the snapshot.
        self.scratch.pager().page(page_number)
    }
}

impl Drop for Snapshot<'_> {
    fn drop(&mut self) {
        let (free, waiting) = {
            let mut state = self.scratch.lock();
            if self.writer {
                state.writer = None;
            } else if let Ok(index) = state.live.binary_search_by_key(&self.seq, |l| l.seq) {
                state.live.remove(index);
            }
            (state.free_retired(), state.bytes)
        };
        drop(free);
        // The end of a read may let versions go to `krummi.data` (J5); the writer's snapshot ends with its commit,
        // which already asked for a transfer.
        if !self.writer && waiting > 0 {
            self.scratch.shared.signal.raise(waiting);
        }
    }
}

/// The one writer of the scratch area. It stays on the thread that created it and cannot be shared between
/// threads, so no two flushes or publications ever overlap.
pub struct ScratchWriter<'a> {
    snapshot: Snapshot<'a>,
    /// Not `Send`: the writer stays on its thread.
    _guard: MutexGuard<'a, ()>,
    /// Not `Sync`: the writer is never shared.
    _not_shared: PhantomData<Cell<()>>,
}

impl<'a> ScratchWriter<'a> {
    /// The snapshot the writer started from: everything committed before it got its turn (T2).
    pub fn snapshot(&self) -> &Snapshot<'a> {
        &self.snapshot
    }

    /// Has the transfer thread write in place every newest version that no open snapshot is older than, making
    /// the journal durable first (J5), and waits for it. Versions written before a failure leave the scratch area;
    /// the rest stay for a later flush.
    pub fn flush(&mut self) -> Result<()> {
        self.snapshot.scratch.demand(Demand::Flush)
    }

    /// Commits `pages`, each sealed for its number: copies them into the temporary area, puts them in the
    /// journal's queue and publishes them for the next writer (E2). Returns the number of the commit, which read
    /// snapshots see once it is durable (E3; [`Scratch::wait_durable`]). On failure nothing is published (J12).
    pub fn commit(&mut self, pages: &[(u64, SealedPage)]) -> Result<u64> {
        let staged = self.stage(pages)?;
        let scratch = self.snapshot.scratch;
        if let Some(journal) = &scratch.shared.journal {
            let txid = scratch.lock().last_committed + 1;
            let refs: Vec<(u64, &[u8; PAGE_SIZE])> =
                pages.iter().map(|(n, p)| (*n, &**p)).collect();
            journal.enqueue(scratch.pager(), txid, &refs)?;
        }
        Ok(self.publish(staged))
    }

    /// Copies `pages`, each sealed for its number, into the temporary area, ready to be published (J7). Nothing
    /// is visible yet; dropping the result gives the slots back.
    ///
    /// # Panics
    ///
    /// If a page is a header page or beyond the end of the store.
    pub fn stage(&self, pages: &[(u64, SealedPage)]) -> Result<Staged> {
        let scratch = self.snapshot.scratch;
        let pager = scratch.pager();
        let page_count = pager.page_count();
        let mut staged = Vec::new();
        staged
            .try_reserve_exact(pages.len())
            .map_err(|_| Error::OutOfMemory {
                requested: (pages.len() * size_of::<(u64, Arc<Slot>)>()) as u64,
            })?;
        for (page_number, page) in pages {
            assert!(
                (HEADER_PAGES..page_count).contains(page_number),
                "page {page_number} cannot be published"
            );
            let slot = scratch.shared.area.store(page, pager)?;
            staged.push((*page_number, Arc::new(slot)));
        }
        Ok(Staged(staged))
    }

    /// Publishes the staged pages as the newest versions, for every writer from now on and for the read snapshots
    /// opened once the commit is durable (T2, E3), and asks the transfer thread to move them to `krummi.data`
    /// (J5). Returns the number of the new version.
    pub fn publish(&mut self, staged: Staged) -> u64 {
        let scratch = self.snapshot.scratch;
        let mut state = scratch.lock();
        let txid = state.last_committed + 1;
        for (page_number, page) in staged.0 {
            state.bytes += PAGE_SIZE_U64;
            state
                .versions
                .entry(page_number)
                .or_default()
                .push(Version { txid, page });
        }
        state.last_committed = txid;
        let bytes = state.bytes;
        drop(state);
        scratch.shared.signal.raise(bytes);
        txid
    }
}

#[cfg(test)]
#[path = "scratch_tests.rs"]
mod tests;
