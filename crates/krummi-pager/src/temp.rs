//! The temporary area (J7 of the Journal PRD): where the committed page versions that are not in `krummi.data` yet
//! live, outside the process memory for stores on disk.
//!
//! The area is a set of chunks of page-sized slots. For a store on disk each chunk is a file of the `krummi.temp`
//! subfolder, mapped for reading and writing; for a temporary store it is plain memory. Chunks are never moved or
//! unmapped while the area lives, so a reference to a slot stays valid as long as the slot is held.
//!
//! A [`Slot`] holds one sealed page, or a value run in contiguous slots of one chunk, so that the run reads as one
//! sequence (D5 of the large values execution); a run that fits in no free range of the chunks gets a new chunk of
//! at least its size. A slot is written once, by [`TempArea::store`] or [`TempArea::store_run`], before anyone else
//! can see it, and only read afterwards. Dropping it gives all its pages back to the area at once, merged with the
//! free ranges next to them, and the area may hand them out again; the scratch area drops a version only when no
//! snapshot can hold a reference to it, so a slot is never rewritten while it is borrowed.

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
/// Slots of the first chunk; each new chunk doubles the previous one up to [`MAX_CHUNK_PAGES`] (8 MiB), so a store
/// near a full disk never reserves much more than it needs.
const FIRST_CHUNK_PAGES: usize = 16;
const MAX_CHUNK_PAGES: usize = 1024;

/// The temporary area of a store.
pub(crate) struct TempArea {
    /// `krummi.temp` for a store on disk, `None` for a temporary store.
    dir: Option<PathBuf>,
    state: Mutex<AreaState>,
}

struct AreaState {
    chunks: Vec<Chunk>,
    /// Free ranges of slots: the number of free slots from (chunk, index) on. Two ranges are never next to each
    /// other in the same chunk: giving slots back merges them.
    free: BTreeMap<(usize, usize), usize>,
    /// Slots handed out and not dropped yet, each of one or more pages.
    held: usize,
}

/// Memory of one chunk: `pages` slots from `ptr`.
struct Chunk {
    ptr: NonNull<u8>,
    pages: usize,
    keep: Keep,
}

/// What owns the memory of a chunk.
enum Keep {
    /// A mapped file of `krummi.temp`; the file stays open while it is mapped.
    File(#[allow(dead_code)] File, #[allow(dead_code)] MmapMut),
    /// A heap allocation owned through `ptr` (see `Drop`).
    Heap,
}

// SAFETY: the chunks own their memory like a `Box<[u8]>` or an `MmapMut` do. Slots are written only by
// `TempArea::store` before anyone else can reach them, and read through shared references afterwards (see the
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

impl TempArea {
    /// The temporary area of the store in `folder`, or of a temporary store. It creates nothing until a version is
    /// stored.
    pub(crate) fn new(folder: Option<&Path>) -> TempArea {
        TempArea {
            dir: folder.map(|f| f.join(TEMP_DIR)),
            state: Mutex::new(AreaState {
                chunks: Vec::new(),
                free: BTreeMap::new(),
                held: 0,
            }),
        }
    }

    /// Copies `page` into a free slot and returns it. A failure to grow the area is classified like a write of the
    /// store, with `pager` refusing writes after an I/O failure (G12, T12).
    pub(crate) fn store(
        self: &Arc<TempArea>,
        page: &[u8; PAGE_SIZE],
        pager: &Pager,
    ) -> Result<Slot> {
        self.store_run(page, pager)
    }

    /// Copies `run`, a whole number of pages, into contiguous free slots of one chunk and returns them, growing the
    /// area with a chunk of at least its size when no free range fits it (D5 of the large values execution). A
    /// failure to grow the area is classified like a write of the store, with `pager` refusing writes after an I/O
    /// failure (G12, T12).
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
        let pages = run.len() / PAGE_SIZE;
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let (chunk, index) = match state.take(pages) {
            Some(slots) => slots,
            None => {
                self.grow(&mut state, pager, pages)?;
                state.take(pages).expect("the area just grew")
            }
        };
        let ptr = slot_ptr(&state.chunks[chunk], index, pages);
        state.held += 1;
        drop(state);
        // SAFETY: `ptr` points to the `pages` slots of a free range of a live chunk: nobody holds a reference to them
        // (a slot is free only once its `Slot` is dropped) and nobody else writes them (they were just taken out of
        // the free ranges under the lock).
        unsafe { std::ptr::copy_nonoverlapping(run.as_ptr(), ptr.as_ptr(), run.len()) };
        Ok(Slot {
            area: Arc::clone(self),
            ptr,
            chunk,
            index,
            pages,
        })
    }

    /// Adds a chunk: twice the last one, up to [`MAX_CHUNK_PAGES`], and at least `pages` slots.
    fn grow(&self, state: &mut AreaState, pager: &Pager, pages: usize) -> Result<()> {
        let pages = state
            .chunks
            .last()
            .map_or(FIRST_CHUNK_PAGES, |c| (c.pages * 2).min(MAX_CHUNK_PAGES))
            .max(pages);
        let bytes = pages * PAGE_SIZE;
        let chunk = match &self.dir {
            None => {
                let memory = sys::alloc_zeroed(bytes).map_err(|_| Error::OutOfMemory {
                    requested: bytes as u64,
                })?;
                let ptr = NonNull::new(Box::into_raw(memory).cast::<u8>())
                    .expect("Box pointers are never null");
                Chunk {
                    ptr,
                    pages,
                    keep: Keep::Heap,
                }
            }
            Some(dir) => {
                if state.chunks.is_empty() {
                    // A leftover of an earlier run of this store goes first (J7).
                    sys::remove_temp_dir(dir)
                        .and_then(|()| sys::create_temp_dir(dir))
                        .map_err(|e| temp_error(pager, e, bytes as u64))?;
                }
                let path = dir.join(format!("scratch.{:010}", state.chunks.len() + 1));
                let (file, mut map) = sys::create_temp_file(&path, bytes as u64)
                    .map_err(|e| temp_error(pager, e, bytes as u64))?;
                let ptr = NonNull::new(map.as_mut_ptr()).expect("mappings are never null");
                Chunk {
                    ptr,
                    pages,
                    keep: Keep::File(file, map),
                }
            }
        };
        let id = state.chunks.len();
        state.chunks.push(chunk);
        state.free.insert((id, 0), pages);
        Ok(())
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
        state.chunks.iter().any(|chunk| {
            let start = chunk.ptr.as_ptr() as usize;
            let at = bytes.as_ptr() as usize;
            at >= start && at + bytes.len() <= start + chunk.pages * PAGE_SIZE
        })
    }

    fn release(&self, chunk: usize, index: usize, pages: usize) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.held -= 1;
        state.give(chunk, index, pages);
    }
}

impl AreaState {
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
        // SAFETY: `ptr` points to the `pages` contiguous slots of this `Slot`, inside a chunk the area keeps until it
        // is removed, which happens only once every slot is dropped. The bytes were written before the slot was
        // returned and are never written again while it lives.
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
        test(&Arc::new(TempArea::new(on_disk.folder())), &on_disk);
        let temporary = Pager::open_temporary(&OpenOptions::default()).unwrap();
        test(&Arc::new(TempArea::new(None)), &temporary);
    }

    fn chunks(area: &TempArea) -> Vec<usize> {
        area.state
            .lock()
            .unwrap()
            .chunks
            .iter()
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

    /// A run that does not fit in a free range makes the area grow; when the growth fails, nothing is held and the
    /// area works afterwards.
    #[test]
    fn a_failed_growth_for_a_run_holds_nothing() {
        let pager = Pager::open_temporary(&OpenOptions::default()).unwrap();
        let area = Arc::new(TempArea::new(None));
        faults::reset();
        faults::inject(Op::Alloc, 0, Action::Fail(io::ErrorKind::OutOfMemory));
        let error = area.store_run(&run(2, 6), &pager).err().unwrap();
        faults::reset();
        assert_eq!(error.kind(), crate::ErrorKind::OutOfMemory);
        assert_eq!(area.state.lock().unwrap().held, 0);
        assert_holds(&area.store_run(&run(2, 6), &pager).unwrap(), &run(2, 6));
    }
}
