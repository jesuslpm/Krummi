//! The temporary area (J7 of the Journal PRD): where the committed page versions that are not in `krummi.data` yet
//! live, outside the process memory for stores on disk.
//!
//! The area is a set of chunks of page-sized slots. For a store on disk each chunk is a file of the `krummi.temp`
//! subfolder, mapped for reading and writing; for a temporary store it is plain memory. A chunk is never moved, and
//! never unmapped or freed while one of its slots is held, so a reference to a slot stays valid as long as the slot
//! is held.
//!
//! A [`Slot`] holds one sealed page, or a value run in contiguous slots of one chunk, so that the run reads as one
//! sequence (D5 of the large values execution). Shared chunks hold any slots and stay until the area is removed.
//! They grow as they did before value runs: only when the free slots are fewer than the pages to store, by twice
//! the last shared chunk up to [`MAX_CHUNK_PAGES`] (8 MiB), or by the run's size when it is larger. A run larger
//! than [`MAX_CHUNK_PAGES`], or one that the free slots could hold page by page but no free range fits, gets a chunk
//! of its own, of exactly its size, released (unmapped and its file removed) as soon as the run is. In a store on
//! disk without a maximum size, released own chunks of one size are kept instead, up to [`IDLE_PAGES`], for the
//! next runs of that size, which take them without creating a file (candidate 5 of the writes diagnosis,
//! performance plan); a chunk of another size releases them. So what the area reserves so that a run is contiguous
//! never piles up: besides the slots held, it holds at most what the page-by-page policy would, plus one shared
//! chunk, and without a maximum size [`IDLE_PAGES`] of released own chunks (P10, T14; R1 of the large values code
//! review). A run's slots are held whole while any of its pages keeps a version, so the scratch area counts them
//! whole for the maximum size until its last version leaves, not page by page (`State::hold` in the `scratch`
//! module).
//!
//! A slot is written once, by [`TempArea::store_with`] or [`TempArea::store_run`], before anyone else can see it, and
//! only read afterwards. Dropping it gives all its pages back to the area at once, merged with the free ranges next
//! to them, and the area may hand them out again; the scratch area drops a version only when no snapshot can hold a
//! reference to it, so a slot is never rewritten or released while it is borrowed.

use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex, PoisonError};

use memmap2::MmapMut;

use crate::error::{Error, Result};
use crate::format::PAGE_SIZE;
use crate::pager::Pager;
use crate::sys;

/// Name of the subfolder of the temporary area in the store's folder (J2).
pub const TEMP_DIR: &str = "krummi.temp";
/// Slots of the first shared chunk; each new one doubles the previous one up to [`MAX_CHUNK_PAGES`] (8 MiB), so a
/// store near a full disk never reserves much more than it needs.
const FIRST_CHUNK_PAGES: usize = 16;
const MAX_CHUNK_PAGES: usize = 1024;
/// The most pages of released own chunks the area keeps for later runs of their size (candidate 5 of the writes
/// diagnosis, performance plan): 32 MiB.
const IDLE_PAGES: usize = (32 << 20) / PAGE_SIZE;

/// The temporary area of a store.
pub(crate) struct TempArea {
    /// `krummi.temp` for a store on disk, `None` for a temporary store.
    dir: Option<PathBuf>,
    /// Whether released own chunks are kept for later runs: only in a store on disk without a maximum size, since
    /// with one the files of `krummi.temp` count against it (P10, T14).
    keeps_released: bool,
    state: Mutex<AreaState>,
}

struct AreaState {
    /// Chunks by number; `None` where a run's own chunk was released, a number the next new chunk takes again.
    chunks: Vec<Option<Chunk>>,
    /// Numbers of the released chunks.
    vacant: Vec<usize>,
    /// Released own chunks of a store on disk, all of one size and at most [`IDLE_PAGES`] in all, kept for the
    /// next runs of that size.
    idle: Vec<Chunk>,
    /// Free ranges of slots of the shared chunks: the number of free slots from (chunk, index) on. Two ranges are
    /// never next to each other in the same chunk: giving slots back merges them.
    free: BTreeMap<(usize, usize), usize>,
    /// Slots handed out and not dropped yet, each of one or more pages.
    held: usize,
    /// Slots of the last shared chunk, which the next one doubles; 0 before the first.
    shared_pages: usize,
    /// Files created in `krummi.temp` since the area began or was removed; they name the next one, so a name is
    /// never taken again, not even after a file that could not be removed.
    files: usize,
}

/// Memory of one chunk: `pages` slots from `ptr`.
struct Chunk {
    ptr: NonNull<u8>,
    pages: usize,
    keep: Keep,
    /// Whether the chunk holds one run only and is released with it.
    own: bool,
}

/// What owns the memory of a chunk.
enum Keep {
    /// A mapped file of `krummi.temp`, with its path; the file stays open while it is mapped.
    File(
        #[allow(dead_code)] File,
        #[allow(dead_code)] MmapMut,
        PathBuf,
    ),
    /// A heap allocation owned through `ptr` (see `Drop`).
    Heap,
}

// SAFETY: the chunks own their memory like a `Box<[u8]>` or an `MmapMut` do. Slots are written only by
// `TempArea::store_with` before anyone else can reach them, and read through shared references afterwards (see the
// module documentation), so sharing the area between threads is sound.
unsafe impl Send for TempArea {}
// SAFETY: see `Send`.
unsafe impl Sync for TempArea {}

impl Drop for Chunk {
    fn drop(&mut self) {
        if let Keep::Heap = self.keep {
            let bytes =
                std::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr(), self.pages * PAGE_SIZE);
            // SAFETY: a heap chunk's `ptr` comes from `Box::into_raw` of a `Box<[u8]>` of `pages * PAGE_SIZE`
            // bytes in `TempArea::grow`, and it is freed only here, once.
            drop(unsafe { Box::from_raw(bytes) });
        }
    }
}

impl Chunk {
    /// Frees the chunk's memory: unmaps its file and only then removes it, since a mapped file cannot be removed on
    /// Windows.
    fn release(mut self) {
        let path = match &mut self.keep {
            Keep::File(_, _, path) => Some(std::mem::take(path)),
            Keep::Heap => None,
        };
        drop(self);
        if let Some(path) = path {
            sys::remove_temp_file(&path);
        }
    }
}

impl TempArea {
    /// The temporary area of the store in `folder`, or of a temporary store, with the store's maximum size. It
    /// creates nothing until a version is stored.
    pub(crate) fn new(folder: Option<&Path>, max_size: Option<u64>) -> TempArea {
        TempArea {
            dir: folder.map(|f| f.join(TEMP_DIR)),
            keeps_released: folder.is_some() && max_size.is_none(),
            state: Mutex::new(AreaState {
                chunks: Vec::new(),
                vacant: Vec::new(),
                idle: Vec::new(),
                free: BTreeMap::new(),
                held: 0,
                shared_pages: 0,
                files: 0,
            }),
        }
    }

    /// Copies `page` into a free slot and returns it. A failure to grow the area is classified like a write of the
    /// store, with `pager` refusing writes after an I/O failure (G12, T12).
    #[cfg(test)]
    pub(crate) fn store(
        self: &Arc<TempArea>,
        page: &[u8; PAGE_SIZE],
        pager: &Pager,
    ) -> Result<Slot> {
        self.store_run(page, pager)
    }

    /// Copies `run`, a whole number of pages, into contiguous free slots of one chunk and returns them, growing the
    /// area when no free range fits it (D5 of the large values execution; see the module documentation). A failure
    /// to grow the area is classified like a write of the store, with `pager` refusing writes after an I/O failure
    /// (G12, T12).
    ///
    /// # Panics
    ///
    /// If `run` is empty or not a whole number of pages.
    pub(crate) fn store_run(self: &Arc<TempArea>, run: &[u8], pager: &Pager) -> Result<Slot> {
        assert!(
            !run.is_empty() && run.len().is_multiple_of(PAGE_SIZE),
            "a run of {} bytes is not a whole number of pages",
            run.len()
        );
        self.store_with(run.len() / PAGE_SIZE, pager, |slots| {
            slots.copy_from_slice(run);
        })
    }

    /// Takes `pages` contiguous free slots of one chunk as [`TempArea::store_run`] does, has `fill` write every byte
    /// of them, and returns them: the commit seals its pages straight into their slots this way, without a sealed
    /// copy first (O5 of the performance plan). The slots are nobody else's until this returns, so no reader sees
    /// them half written. If `fill` panics, the slots go back to the area.
    ///
    /// # Panics
    ///
    /// If `pages` is 0.
    pub(crate) fn store_with(
        self: &Arc<TempArea>,
        pages: usize,
        pager: &Pager,
        fill: impl FnOnce(&mut [u8]),
    ) -> Result<Slot> {
        assert!(pages > 0, "a slot holds at least one page");
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let (chunk, index) = match state.take(pages) {
            Some(slots) => slots,
            None => self.grow(&mut state, pager, pages)?,
        };
        let ptr = slot_ptr(state.chunk(chunk), index, pages);
        state.held += 1;
        drop(state);
        // Built first, so that a panic in `fill` gives the slots back.
        let slot = Slot {
            area: Arc::clone(self),
            ptr,
            chunk,
            index,
            pages,
        };
        // SAFETY: `ptr` points to `pages` slots of a live chunk, taken under the lock from its free ranges or from the
        // chunk `grow` just added: nobody holds a reference to them (a slot is free only once its `Slot` is dropped,
        // and a new chunk was never handed out, nor is `slot` yet) and nobody else writes them (nobody else can take
        // them now). Their bytes are initialized: a heap chunk is allocated zeroed and a file chunk is mapped. The
        // reference ends before `slot` is returned, after which they are only read.
        let bytes = unsafe { std::slice::from_raw_parts_mut(ptr.as_ptr(), pages * PAGE_SIZE) };
        fill(bytes);
        Ok(slot)
    }

    /// Adds a chunk for `pages` slots that no free range fits and takes them from it, returning its number and the
    /// first slot: a chunk of their own, of exactly `pages` slots, when they are more than [`MAX_CHUNK_PAGES`] or the
    /// free slots could hold them page by page, so that only contiguity is missing; otherwise a shared chunk, twice
    /// the last one up to [`MAX_CHUNK_PAGES`] and at least `pages` slots.
    fn grow(&self, state: &mut AreaState, pager: &Pager, pages: usize) -> Result<(usize, usize)> {
        let own = pages > MAX_CHUNK_PAGES || state.free.values().sum::<usize>() >= pages;
        let size = if own {
            pages
        } else {
            match state.shared_pages {
                0 => FIRST_CHUNK_PAGES,
                last => (last * 2).min(MAX_CHUNK_PAGES),
            }
            .max(pages)
        };
        let bytes = size * PAGE_SIZE;
        let kept = own
            .then(|| state.idle.pop_if(|chunk| chunk.pages == size))
            .flatten();
        let chunk = match (kept, &self.dir) {
            (Some(chunk), _) => chunk,
            (None, None) => {
                let memory = sys::alloc_zeroed(bytes).map_err(|_| Error::OutOfMemory {
                    requested: bytes as u64,
                })?;
                let ptr = NonNull::new(Box::into_raw(memory).cast::<u8>())
                    .expect("Box pointers are never null");
                Chunk {
                    ptr,
                    pages: size,
                    keep: Keep::Heap,
                    own,
                }
            }
            (None, Some(dir)) => {
                if state.files == 0 {
                    // A leftover of an earlier run of this store goes first (J7).
                    sys::remove_temp_dir(dir)
                        .and_then(|()| sys::create_temp_dir(dir))
                        .map_err(|e| temp_error(pager, e, bytes as u64))?;
                }
                let path = dir.join(format!("scratch.{:010}", state.files + 1));
                let (file, mut map) = sys::create_temp_file(&path, bytes as u64)
                    .map_err(|e| temp_error(pager, e, bytes as u64))?;
                state.files += 1;
                let ptr = NonNull::new(map.as_mut_ptr()).expect("mappings are never null");
                Chunk {
                    ptr,
                    pages: size,
                    keep: Keep::File(file, map, path),
                    own,
                }
            }
        };
        let id = match state.vacant.pop() {
            Some(id) => id,
            None => {
                state.chunks.push(None);
                state.chunks.len() - 1
            }
        };
        state.chunks[id] = Some(chunk);
        if !own {
            state.shared_pages = size;
            if size > pages {
                state.free.insert((id, pages), size - pages);
            }
        }
        Ok((id, 0))
    }

    /// Unmaps every chunk and removes `krummi.temp` (J7). While a slot is still held nothing is unmapped, so its
    /// reference stays valid, and the folder may stay behind.
    pub(crate) fn remove(&self) -> io::Result<()> {
        let chunks = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if state.held > 0 {
                return Ok(());
            }
            state.free.clear();
            state.vacant.clear();
            drop(std::mem::take(&mut state.idle));
            state.shared_pages = 0;
            state.files = 0;
            std::mem::take(&mut state.chunks)
        };
        drop(chunks);
        match &self.dir {
            Some(dir) => sys::remove_temp_dir(dir),
            None => Ok(()),
        }
    }

    /// Whether `bytes` are inside one of the area's chunks (tests: AC-V3).
    pub(crate) fn holds(&self, bytes: &[u8]) -> bool {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.chunks.iter().flatten().any(|chunk| {
            let start = chunk.ptr.as_ptr() as usize;
            let at = bytes.as_ptr() as usize;
            at >= start && at + bytes.len() <= start + chunk.pages * PAGE_SIZE
        })
    }

    /// Bytes of every chunk the area holds, free slots included (tests).
    pub(crate) fn capacity(&self) -> u64 {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state
            .chunks
            .iter()
            .flatten()
            .chain(&state.idle)
            .map(|chunk| (chunk.pages * PAGE_SIZE) as u64)
            .sum()
    }

    /// Gives back the slots of a dropped [`Slot`]: to the free ranges of a shared chunk, or by keeping or releasing
    /// the run's own chunk. Both happen under the lock and before `held` drops, so [`TempArea::remove`] never finds a
    /// chunk being released.
    fn release(&self, chunk: usize, index: usize, pages: usize) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.chunk(chunk).own {
            let own = state.chunks[chunk].take().expect("a live chunk");
            state.vacant.push(chunk);
            if self.keeps_released {
                state.keep_or_release(own);
            } else {
                own.release();
            }
        } else {
            state.give(chunk, index, pages);
        }
        state.held -= 1;
    }
}

impl AreaState {
    /// Chunk number `chunk`, which a held slot keeps alive.
    fn chunk(&self, chunk: usize) -> &Chunk {
        self.chunks[chunk].as_ref().expect("a live chunk")
    }

    /// Keeps a released own chunk for the next run of its size, after releasing the kept chunks of another size,
    /// while the kept ones hold at most [`IDLE_PAGES`] pages; otherwise releases it.
    fn keep_or_release(&mut self, own: Chunk) {
        if self
            .idle
            .first()
            .is_some_and(|kept| kept.pages != own.pages)
        {
            self.idle.drain(..).for_each(Chunk::release);
        }
        if (self.idle.len() + 1) * own.pages <= IDLE_PAGES {
            self.idle.push(own);
        } else {
            own.release();
        }
    }

    /// Takes `pages` contiguous free slots from the first free range that has them: their chunk and first index. A
    /// single page takes the first range at once, since every free range has at least one slot.
    fn take(&mut self, pages: usize) -> Option<(usize, usize)> {
        let (&(chunk, index), &len) = self.free.iter().find(|&(_, &len)| len >= pages)?;
        self.free.remove(&(chunk, index));
        if len > pages {
            self.free.insert((chunk, index + pages), len - pages);
        }
        Some((chunk, index))
    }

    /// Gives back `pages` slots from (chunk, index), merged with the free ranges right before and after them.
    fn give(&mut self, chunk: usize, index: usize, pages: usize) {
        let (mut start, mut len) = (index, pages);
        if let Some(next) = self.free.remove(&(chunk, index + pages)) {
            len += next;
        }
        if let Some((&(c, i), &before)) = self.free.range(..(chunk, index)).next_back()
            && c == chunk
            && i + before == index
        {
            self.free.remove(&(c, i));
            start = i;
            len += before;
        }
        self.free.insert((chunk, start), len);
    }
}

/// Pointer to slot `index` of `chunk`, the first of `pages` slots.
fn slot_ptr(chunk: &Chunk, index: usize, pages: usize) -> NonNull<u8> {
    assert!(index + pages <= chunk.pages);
    // SAFETY: the slots are inside the chunk, as just checked, so the offset stays inside its allocation.
    unsafe { chunk.ptr.add(index * PAGE_SIZE) }
}

/// Classifies a failure of the temporary area: a missing permission is the access class (J7); the rest like a
/// failed write of the store.
fn temp_error(pager: &Pager, source: io::Error, requested: u64) -> Error {
    match source.kind() {
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => Error::Access {
            folder: pager.folder().map(Path::to_path_buf).unwrap_or_default(),
            source,
        },
        _ => pager.fail_write(source, requested),
    }
}

/// Contiguous slots of the temporary area holding one sealed page or a value run. It gives them all back when
/// dropped.
pub struct Slot {
    area: Arc<TempArea>,
    ptr: NonNull<u8>,
    chunk: usize,
    index: usize,
    pages: usize,
}

// SAFETY: a Slot only reads the bytes it points to, which nobody writes while it lives (see the module
// documentation), and its area is `Send + Sync`.
unsafe impl Send for Slot {}
// SAFETY: see `Send`.
unsafe impl Sync for Slot {}

impl Slot {
    /// The sealed page in the slot, or the first page of its run.
    pub fn page(&self) -> &[u8; PAGE_SIZE] {
        self.page_at(0)
    }

    /// Page `index` of the run in the slots.
    ///
    /// # Panics
    ///
    /// If the slots hold `index` pages or fewer.
    pub fn page_at(&self, index: usize) -> &[u8; PAGE_SIZE] {
        self.bytes()[index * PAGE_SIZE..(index + 1) * PAGE_SIZE]
            .try_into()
            .expect("a whole page")
    }

    /// Number of pages in the slots.
    pub fn pages(&self) -> usize {
        self.pages
    }

    /// Every page in the slots, as one contiguous sequence.
    pub fn bytes(&self) -> &[u8] {
        // SAFETY: `ptr` points to the `pages` contiguous slots of this `Slot`, inside a chunk the area keeps while the
        // slot lives: a shared chunk until the area is removed, which happens only once every slot is dropped, and a
        // run's own chunk until this slot is dropped. The bytes were written before the slot was returned and are
        // never written again while it lives.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.pages * PAGE_SIZE) }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.area.release(self.chunk, self.index, self.pages);
    }
}

#[cfg(test)]
mod tests {
    use std::panic::AssertUnwindSafe;

    use super::*;
    use crate::pager::OpenOptions;
    use crate::sys::Op;
    use crate::sys::faults::{self, Action};

    /// `pages` pages whose bytes tell where they come from.
    fn run(pages: usize, seed: u8) -> Vec<u8> {
        (0..pages * PAGE_SIZE)
            .map(|i| (i % 253) as u8 ^ seed)
            .collect()
    }

    /// Runs `test` with the temporary area of a store on disk and of a temporary store.
    fn both_modes(test: impl Fn(&Arc<TempArea>, &Pager)) {
        let tmp = tempfile::tempdir().unwrap();
        let on_disk = Pager::open(tmp.path(), &OpenOptions::default()).unwrap();
        test(&Arc::new(TempArea::new(on_disk.folder(), None)), &on_disk);
        let temporary = Pager::open_temporary(&OpenOptions::default()).unwrap();
        test(&Arc::new(TempArea::new(None, None)), &temporary);
    }

    fn chunks(area: &TempArea) -> Vec<usize> {
        area.state
            .lock()
            .unwrap()
            .chunks
            .iter()
            .flatten()
            .map(|c| c.pages)
            .collect()
    }

    fn assert_holds(slot: &Slot, bytes: &[u8]) {
        assert_eq!(slot.bytes(), bytes);
        for (i, page) in bytes.chunks(PAGE_SIZE).enumerate() {
            assert_eq!(&slot.page_at(i)[..], page, "page {i}");
            assert_eq!(
                slot.page_at(i).as_ptr(),
                slot.bytes()[i * PAGE_SIZE..].as_ptr(),
                "page {i} is a view into the run"
            );
        }
    }

    /// D5: a run is stored in contiguous slots and read back as one sequence and page by page; one larger than any
    /// chunk gets a chunk of its own size, next to the single-page slots, which work as before.
    #[test]
    fn a_run_is_stored_in_contiguous_slots_and_read_whole_and_page_by_page() {
        both_modes(|area, pager| {
            let single = area.store(&[9; PAGE_SIZE], pager).unwrap();
            let small = run(3, 1);
            let large = run(MAX_CHUNK_PAGES + 5, 2);
            let small_slot = area.store_run(&small, pager).unwrap();
            let large_slot = area.store_run(&large, pager).unwrap();
            assert_holds(&small_slot, &small);
            assert_holds(&large_slot, &large);
            assert_eq!(single.page(), &[9; PAGE_SIZE]);
            assert_eq!(chunks(area), vec![FIRST_CHUNK_PAGES, MAX_CHUNK_PAGES + 5]);
            if let Some(folder) = pager.folder() {
                let file = folder.join(TEMP_DIR).join("scratch.0000000002");
                assert_eq!(
                    std::fs::metadata(file).unwrap().len(),
                    ((MAX_CHUNK_PAGES + 5) * PAGE_SIZE) as u64
                );
            }
        });
    }

    /// R1 of the code review: a run's own chunk, of exactly its size, goes with the run's last holder, and its number
    /// is taken again, so runs a page larger each time never pile up chunks. In a store on disk the last one is kept
    /// for a run of its size (candidate 5 of the writes diagnosis), and goes with the next one of another size, its
    /// file too.
    #[test]
    fn a_run_s_own_chunk_is_released_with_it() {
        both_modes(|area, pager| {
            let single = area.store(&[1; PAGE_SIZE], pager).unwrap();
            for extra in 1..4 {
                let bytes = run(MAX_CHUNK_PAGES + extra, extra as u8);
                let slot = Arc::new(area.store_run(&bytes, pager).unwrap());
                assert_holds(&slot, &bytes);
                let holder = Arc::clone(&slot);
                drop(slot);
                assert_eq!(
                    chunks(area),
                    vec![FIRST_CHUNK_PAGES, MAX_CHUNK_PAGES + extra],
                    "still held"
                );
                drop(holder);
                assert_eq!(chunks(area), vec![FIRST_CHUNK_PAGES]);
                let kept = match pager.folder() {
                    Some(_) => MAX_CHUNK_PAGES + extra,
                    None => 0,
                };
                assert_eq!(
                    area.capacity(),
                    ((FIRST_CHUNK_PAGES + kept) * PAGE_SIZE) as u64
                );
                if let Some(folder) = pager.folder() {
                    let own = format!("scratch.{:010}", extra + 1);
                    assert_eq!(temp_files(folder), ["scratch.0000000001", own.as_str()]);
                }
            }
            assert_eq!(area.state.lock().unwrap().chunks.len(), 2, "number reused");
            assert_eq!(single.page(), &[1; PAGE_SIZE]);
            drop(single);
            area.remove().unwrap();
        });
    }

    /// Files of `krummi.temp`, by name.
    fn temp_files(folder: &Path) -> Vec<String> {
        let mut files: Vec<String> = std::fs::read_dir(folder.join(TEMP_DIR))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        files.sort();
        files
    }

    /// Candidate 5 of the writes diagnosis (performance plan): in a store on disk, a run's released own chunk is
    /// kept and a later run of its size takes it, without creating a file; a temporary store frees it, since all its
    /// memory counts against its maximum size (P10, T14).
    #[test]
    fn a_released_own_chunk_serves_the_next_run_of_its_size() {
        both_modes(|area, pager| {
            let pages = MAX_CHUNK_PAGES + 1;
            drop(area.store_run(&run(pages, 1), pager).unwrap());
            let again = run(pages, 2);
            let slot = area.store_run(&again, pager).unwrap();
            assert_holds(&slot, &again);
            let files = area.state.lock().unwrap().files;
            match pager.folder() {
                Some(folder) => {
                    assert_eq!(files, 1, "the second run took the first one's file");
                    assert_eq!(temp_files(folder), ["scratch.0000000001"]);
                }
                None => assert_eq!(area.capacity(), (pages * PAGE_SIZE) as u64),
            }
            drop(slot);
            area.remove().unwrap();
        });
    }

    /// Candidate 5 of the writes diagnosis: with a maximum size, a store on disk releases its own chunks as before,
    /// since the files of `krummi.temp` count against it (P10, T14).
    #[test]
    fn with_a_maximum_size_no_released_own_chunk_is_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let pager = Pager::open(tmp.path(), &OpenOptions::default()).unwrap();
        let area = Arc::new(TempArea::new(pager.folder(), Some(1 << 30)));
        drop(
            area.store_run(&run(MAX_CHUNK_PAGES + 1, 1), &pager)
                .unwrap(),
        );
        assert_eq!(area.capacity(), 0);
        assert!(temp_files(tmp.path()).is_empty());
        area.remove().unwrap();
    }

    /// Candidate 5 of the writes diagnosis: the released own chunks kept are of one size and hold at most
    /// [`IDLE_PAGES`] pages; the rest are released as before.
    #[test]
    fn the_released_own_chunks_kept_are_bounded() {
        let tmp = tempfile::tempdir().unwrap();
        let pager = Pager::open(tmp.path(), &OpenOptions::default()).unwrap();
        let area = Arc::new(TempArea::new(pager.folder(), None));
        let pages = MAX_CHUNK_PAGES + 1;
        let held: Vec<Slot> = (0..5)
            .map(|i| area.store_run(&run(pages, i), &pager).unwrap())
            .collect();
        drop(held);
        let kept = IDLE_PAGES / pages;
        assert!((1..5).contains(&kept));
        assert_eq!(temp_files(tmp.path()).len(), kept);
        assert_eq!(area.capacity(), (kept * pages * PAGE_SIZE) as u64);
        // A run of another size releases them when its own chunk is released.
        drop(area.store_run(&run(pages + 1, 9), &pager).unwrap());
        assert_eq!(temp_files(tmp.path()), ["scratch.0000000006"]);
        area.remove().unwrap();
    }

    /// R1 of the code review: a run that the free slots could hold page by page, but that no free range fits, gets a
    /// chunk of its own instead of a shared one; a run that needs more slots than are free makes the shared chunks
    /// grow as single pages do, and that chunk stays.
    #[test]
    fn only_a_lack_of_free_slots_grows_the_shared_chunks() {
        both_modes(|area, pager| {
            // The first shared chunk with every other slot free: half of it, in single slots.
            let all: Vec<Slot> = (0..FIRST_CHUNK_PAGES)
                .map(|i| area.store(&[i as u8; PAGE_SIZE], pager).unwrap())
                .collect();
            let kept: Vec<Slot> = all
                .into_iter()
                .enumerate()
                .filter_map(|(i, slot)| (i % 2 == 0).then_some(slot))
                .collect();
            let pair = area.store_run(&run(2, 7), pager).unwrap();
            assert_eq!(chunks(area), vec![FIRST_CHUNK_PAGES, 2], "contiguity only");
            drop(pair);
            assert_eq!(chunks(area), vec![FIRST_CHUNK_PAGES]);
            let larger = run(FIRST_CHUNK_PAGES / 2 + 1, 8);
            let slot = area.store_run(&larger, pager).unwrap();
            assert_holds(&slot, &larger);
            let grown = vec![FIRST_CHUNK_PAGES, 2 * FIRST_CHUNK_PAGES];
            assert_eq!(chunks(area), grown, "more pages than free slots");
            drop(slot);
            assert_eq!(chunks(area), grown, "a shared chunk stays");
            drop(kept);
            area.remove().unwrap();
        });
    }

    /// D5: a run's slots go back to the area as a unit when its last holder drops it, and are handed out again, to
    /// runs and to single pages, without growing the area.
    #[test]
    fn a_run_is_freed_as_a_unit_and_its_slots_are_reused() {
        both_modes(|area, pager| {
            let first = Arc::new(area.store_run(&run(FIRST_CHUNK_PAGES, 3), pager).unwrap());
            let holder = Arc::clone(&first);
            drop(first);
            assert_eq!(area.state.lock().unwrap().held, 1, "still held");
            drop(holder);
            assert_eq!(area.state.lock().unwrap().held, 0);
            let singles: Vec<Slot> = (0..4)
                .map(|i| area.store(&[i; PAGE_SIZE], pager).unwrap())
                .collect();
            let again = run(FIRST_CHUNK_PAGES - 4, 4);
            let reused = area.store_run(&again, pager).unwrap();
            assert_holds(&reused, &again);
            assert_eq!(chunks(area), vec![FIRST_CHUNK_PAGES], "nothing grew");
            drop(singles);
            drop(reused);
            // The freed slots merge back into one range that a whole-chunk run takes again.
            let whole = area.store_run(&run(FIRST_CHUNK_PAGES, 5), pager).unwrap();
            assert_eq!(chunks(area), vec![FIRST_CHUNK_PAGES]);
            drop(whole);
            area.remove().unwrap();
        });
    }

    /// O5: slots filled in place hold what the filler wrote over whatever a freed slot held before, and a filler
    /// that panics gives its slots back.
    #[test]
    fn slots_filled_in_place_hold_what_was_written_and_a_panic_holds_nothing() {
        both_modes(|area, pager| {
            drop(area.store_run(&run(2, 1), pager).unwrap());
            let filled = run(3, 9);
            let slot = area
                .store_with(3, pager, |slots| {
                    assert_eq!(slots.len(), 3 * PAGE_SIZE);
                    slots.copy_from_slice(&filled);
                })
                .unwrap();
            assert_holds(&slot, &filled);
            let panicked = std::panic::catch_unwind(AssertUnwindSafe(|| {
                area.store_with(1, pager, |_| panic!("filler"))
            }));
            assert!(panicked.is_err());
            assert_eq!(area.state.lock().unwrap().held, 1, "only the filled slot");
            drop(slot);
            assert_eq!(area.state.lock().unwrap().held, 0);
            area.remove().unwrap();
        });
    }

    /// A run that does not fit in a free range makes the area grow; when the growth fails, nothing is held and the
    /// area works afterwards.
    #[test]
    fn a_failed_growth_for_a_run_holds_nothing() {
        let pager = Pager::open_temporary(&OpenOptions::default()).unwrap();
        let area = Arc::new(TempArea::new(None, None));
        faults::reset();
        faults::inject(Op::Alloc, 0, Action::Fail(io::ErrorKind::OutOfMemory));
        let error = area.store_run(&run(2, 6), &pager).err().unwrap();
        faults::reset();
        assert_eq!(error.kind(), crate::ErrorKind::OutOfMemory);
        assert_eq!(area.state.lock().unwrap().held, 0);
        assert_holds(&area.store_run(&run(2, 6), &pager).unwrap(), &run(2, 6));
    }
}
