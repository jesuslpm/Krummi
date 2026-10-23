//! The temporary area (J7 of the Journal PRD): where the committed page versions that are not in `krummi.data` yet
//! live, outside the process memory for stores on disk.
//!
//! The area is a set of chunks of page-sized slots. For a store on disk each chunk is a file of the `krummi.temp`
//! subfolder, mapped for reading and writing; for a temporary store it is plain memory. Chunks are never moved or
//! unmapped while the area lives, so a reference to a slot stays valid as long as the slot is held.
//!
//! A [`Slot`] is written once, by [`TempArea::store`], before anyone else can see it, and only read afterwards.
//! Dropping it gives it back to the area, which may hand it out again; the scratch area drops a version only when
//! no snapshot can hold a reference to it, so a slot is never rewritten while it is borrowed.

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
    /// Free slots, as (chunk, index).
    free: Vec<(usize, usize)>,
    /// Slots handed out and not dropped yet.
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
                free: Vec::new(),
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
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.free.is_empty() {
            self.grow(&mut state, pager)?;
        }
        let (chunk, index) = state.free.pop().expect("the area just grew");
        let ptr = slot_ptr(&state.chunks[chunk], index);
        state.held += 1;
        drop(state);
        // SAFETY: `ptr` points to the PAGE_SIZE bytes of a free slot of a live chunk: nobody holds a reference to
        // it (a slot is free only once its `Slot` is dropped) and nobody else writes it (it was just taken off the
        // free list under the lock).
        unsafe { std::ptr::copy_nonoverlapping(page.as_ptr(), ptr.as_ptr(), PAGE_SIZE) };
        Ok(Slot {
            area: Arc::clone(self),
            ptr,
            chunk,
            index,
        })
    }

    fn grow(&self, state: &mut AreaState, pager: &Pager) -> Result<()> {
        let pages = state
            .chunks
            .last()
            .map_or(FIRST_CHUNK_PAGES, |c| (c.pages * 2).min(MAX_CHUNK_PAGES));
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
        state.free.extend((0..pages).rev().map(|index| (id, index)));
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

    fn release(&self, chunk: usize, index: usize) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.held -= 1;
        state.free.push((chunk, index));
    }
}

fn slot_ptr(chunk: &Chunk, index: usize) -> NonNull<u8> {
    assert!(index < chunk.pages);
    // SAFETY: `index` is a slot of the chunk, as just checked, so the offset stays inside its allocation.
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

/// A slot of the temporary area holding one sealed page. It gives the slot back when dropped.
pub struct Slot {
    area: Arc<TempArea>,
    ptr: NonNull<u8>,
    chunk: usize,
    index: usize,
}

// SAFETY: a Slot only reads the bytes it points to, which nobody writes while it lives (see the module
// documentation), and its area is `Send + Sync`.
unsafe impl Send for Slot {}
// SAFETY: see `Send`.
unsafe impl Sync for Slot {}

impl Slot {
    /// The sealed page in the slot.
    pub fn page(&self) -> &[u8; PAGE_SIZE] {
        // SAFETY: `ptr` points to the PAGE_SIZE bytes of this slot, inside a chunk the area keeps until it is
        // removed, which happens only once every slot is dropped. The bytes were written before the slot was
        // returned and are never written again while it lives.
        unsafe { &*self.ptr.as_ptr().cast::<[u8; PAGE_SIZE]>() }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.area.release(self.chunk, self.index);
    }
}
