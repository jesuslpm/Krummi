//! The scratch area (module 2): committed page versions that are not in the data file yet, and the snapshots that
//! read through them (T2, T14, T16 of the Transactions PRD).
//!
//! As in Voron, every page keeps its number. A write transaction changes copies of its pages; committing publishes
//! them as the newest version of each page. A version stays here while an open snapshot still needs the version
//! before it, which may be the one in the data file; once no snapshot is older than it, it is written in place and
//! leaves the scratch area. A version that leaves stays allocated until every snapshot that may hold a reference
//! to it has ended, so references returned by [`Snapshot::page`] stay valid for the whole snapshot (D11).
//!
//! Why the shared page writes are sound: a flush writes page `n` only when every open snapshot sees the newest
//! version of `n` (none is older than it), and that version stays in the scratch area until the write ends, so
//! every snapshot reads `n` from here and nobody holds a reference into the data file's copy of `n`. Snapshots
//! that start afterwards read the newest version too. Only one [`ScratchWriter`] exists at a time, so no two
//! writes of a page overlap.

use std::cell::Cell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem::size_of;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::error::{Error, Result};
use crate::format::{HEADER_PAGES, PAGE_HEADER_SIZE, PAGE_PAYLOAD_SIZE, PAGE_SIZE, PAGE_SIZE_U64};
use crate::pager::{Info, Pager};
use crate::sys;

/// The payload of a page being changed by a write transaction.
pub type PayloadBuf = Box<[u8; PAGE_PAYLOAD_SIZE]>;
/// A whole page, sealed with its header and checksum, ready to be published.
pub type SealedPage = Box<[u8; PAGE_SIZE]>;

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
    /// `None` only after [`Scratch::close`] took it.
    pager: Option<Pager>,
    state: Mutex<State>,
    /// Held by the one [`ScratchWriter`] that may exist at a time.
    writer: Mutex<()>,
}

struct State {
    /// Number of the newest committed version; `0` is what the data file had when the store opened.
    last_committed: u64,
    /// Versions of each page that are only here, oldest first.
    versions: HashMap<u64, Vec<Version>>,
    /// Bytes taken by `versions`.
    bytes: u64,
    /// Open snapshots by ascending sequence number; their versions never decrease along the list.
    live: Vec<Live>,
    next_seq: u64,
    /// Versions that left `versions`, with the newest sequence number issued when they left: they are freed
    /// once no open snapshot has a sequence number up to that one.
    retired: Vec<(u64, Arc<SealedPage>)>,
}

struct Version {
    txid: u64,
    page: Arc<SealedPage>,
}

#[derive(Clone, Copy)]
struct Live {
    seq: u64,
    txid: u64,
}

impl Scratch {
    /// Puts a scratch area in front of an open store.
    pub fn new(pager: Pager) -> Scratch {
        Scratch {
            pager: Some(pager),
            state: Mutex::new(State {
                last_committed: 0,
                versions: HashMap::new(),
                bytes: 0,
                live: Vec::new(),
                next_seq: 0,
                retired: Vec::new(),
            }),
            writer: Mutex::new(()),
        }
    }

    /// The store under the scratch area.
    pub fn pager(&self) -> &Pager {
        self.pager.as_ref().expect("the store is open")
    }

    /// Information about the store (P12).
    pub fn info(&self) -> Info {
        self.pager().info()
    }

    /// Bytes of page versions held only in the scratch area (T14).
    pub fn scratch_bytes(&self) -> u64 {
        self.lock().bytes
    }

    /// Opens a snapshot of everything committed so far (T2, G3).
    pub fn snapshot(&self) -> Result<Snapshot<'_>> {
        let mut state = self.lock();
        self.register(&mut state)
    }

    /// Waits until no other writer exists and returns the writer, with a snapshot of everything committed so far.
    pub fn begin_write(&self) -> Result<ScratchWriter<'_>> {
        let guard = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        let snapshot = self.snapshot()?;
        Ok(ScratchWriter {
            snapshot,
            _guard: guard,
            _not_shared: PhantomData,
        })
    }

    /// Writes every committed version to the data file and closes the store, which releases its lock even if a
    /// write fails (P14). A store that refused writes after a failure tries them again here (T12).
    pub fn close(mut self) -> Result<()> {
        let flushed = self.flush_all();
        let pager = self.pager.take().expect("the store is open");
        let closed = pager.close();
        flushed.and(closed)
    }

    /// Writes the newest version of every page to the data file and empties the scratch area.
    fn flush_all(&mut self) -> Result<()> {
        let pager = self.pager.as_mut().expect("the store is open");
        pager.allow_writes_again();
        let state = self.state.get_mut().unwrap_or_else(PoisonError::into_inner);
        let mut pages: Vec<_> = state.versions.keys().copied().collect();
        pages.sort_unstable();
        for page_number in pages {
            let page = &state.versions[&page_number]
                .last()
                .expect("a page in the scratch area has a version")
                .page;
            // SAFETY: `&mut self` rules out open snapshots, and with them any reference into the store's pages
            // and any other write.
            unsafe { pager.write_sealed_shared(page_number, page)? };
            let versions = state.versions.remove(&page_number).unwrap_or_default();
            state.bytes -= versions.len() as u64 * PAGE_SIZE_U64;
        }
        state.retired.clear();
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn register(&self, state: &mut State) -> Result<Snapshot<'_>> {
        let out_of_memory = || Error::OutOfMemory {
            requested: size_of::<Live>() as u64,
        };
        sys::snapshot_point().map_err(|_| out_of_memory())?;
        state.live.try_reserve(1).map_err(|_| out_of_memory())?;
        let live = Live {
            seq: state.next_seq,
            txid: state.last_committed,
        };
        state.next_seq += 1;
        state.live.push(live);
        Ok(Snapshot {
            scratch: self,
            seq: live.seq,
            txid: live.txid,
        })
    }
}

impl Drop for Scratch {
    /// A store dropped without closing still writes what was committed, without reporting a failure (T9).
    fn drop(&mut self) {
        if self.pager.is_some() {
            let _ = self.flush_all();
        }
    }
}

impl State {
    /// Moves page versions out of `versions`; they are freed once the snapshots that may use them end.
    fn retire(&mut self, versions: impl IntoIterator<Item = Version>) {
        let bound = self.next_seq.saturating_sub(1);
        for version in versions {
            self.bytes -= PAGE_SIZE_U64;
            self.retired.push((bound, version.page));
        }
    }

    /// Takes out the versions that no open snapshot can read any more, for the caller to free. A version that has a
    /// newer one is read only by the snapshots at least as new as it and older than the newer one; with none
    /// open, nobody needs it, and nobody can hold a reference to it either: a snapshot that read it would be one
    /// of those. So it is freed at once, not retired, and a long snapshot does not make them pile up (T14).
    fn collect_garbage(&mut self) -> Vec<Version> {
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

    /// Frees the retired versions that no open snapshot can hold a reference to.
    fn free_retired(&mut self) -> Vec<Arc<SealedPage>> {
        let oldest = self.live.first().map_or(u64::MAX, |l| l.seq);
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
            let ptr = version.page.as_ptr();
            drop(state);
            // SAFETY: `ptr` points to the PAGE_SIZE bytes of a boxed page owned by an `Arc` in the scratch area.
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
        let free = {
            let mut state = self.scratch.lock();
            if let Ok(index) = state.live.binary_search_by_key(&self.seq, |l| l.seq) {
                state.live.remove(index);
            }
            state.free_retired()
        };
        drop(free);
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

    /// Writes in place every newest version that no open snapshot is older than, after retiring the versions
    /// that no snapshot can read any more. Versions written before a failure leave the scratch area; the rest
    /// stay for a later flush.
    pub fn flush(&mut self) -> Result<()> {
        let scratch = self.snapshot.scratch;
        let garbage;
        let mut pending: Vec<(u64, u64, Arc<SealedPage>)> = {
            let mut state = scratch.lock();
            garbage = state.collect_garbage();
            let oldest = state.live.first().map(|l| l.txid);
            state
                .versions
                .iter()
                .filter_map(|(&page_number, versions)| {
                    let newest = versions.last()?;
                    oldest
                        .is_none_or(|oldest| oldest >= newest.txid)
                        .then(|| (page_number, newest.txid, Arc::clone(&newest.page)))
                })
                .collect()
        };
        drop(garbage);
        pending.sort_unstable_by_key(|(page_number, _, _)| *page_number);
        for (page_number, txid, page) in pending {
            // SAFETY: see the module documentation: every open snapshot reads this page from the scratch area,
            // where its newest version stays until the write ends, and this writer is the only one.
            unsafe { scratch.pager().write_sealed_shared(page_number, &page)? };
            let mut state = scratch.lock();
            if state.versions[&page_number].last().map(|v| v.txid) == Some(txid) {
                let versions = state.versions.remove(&page_number).unwrap_or_default();
                state.retire(versions);
            }
        }
        Ok(())
    }

    /// Publishes `pages`, each sealed for its number, as the newest versions, for every snapshot opened from now
    /// on and none opened before (T2). Returns the number of the new version.
    ///
    /// # Panics
    ///
    /// If a page is a header page or beyond the end of the store.
    pub fn publish(&mut self, pages: Vec<(u64, SealedPage)>) -> u64 {
        let scratch = self.snapshot.scratch;
        let page_count = scratch.pager().page_count();
        let mut state = scratch.lock();
        let txid = state.last_committed + 1;
        for (page_number, page) in pages {
            assert!(
                (HEADER_PAGES..page_count).contains(&page_number),
                "page {page_number} cannot be published"
            );
            state.bytes += PAGE_SIZE_U64;
            state
                .versions
                .entry(page_number)
                .or_default()
                .push(Version {
                    txid,
                    page: Arc::new(page),
                });
        }
        state.last_committed = txid;
        txid
    }
}

#[cfg(test)]
#[path = "scratch_tests.rs"]
mod tests;
