//! The Pager: opening, creating and closing a store, and reading, writing and growing its pages.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use memmap2::Mmap;

use crate::error::{Error, Result};
use crate::format::{
    self, FORMAT_VERSION, HEADER_PAGES, Header, HeaderRead, PAGE_HEADER_SIZE, PAGE_SIZE,
    PAGE_SIZE_U64,
};
use crate::sys;

/// Name of the data file of a store (P3).
pub const DATA_FILE: &str = "muninn.data";
/// Name of the file a store is prepared in before it becomes [`DATA_FILE`] (P8).
pub const NEW_DATA_FILE: &str = "muninn.data.new";
/// Smallest maximum size accepted when opening a store (P10).
pub const MIN_MAX_SIZE: u64 = 1 << 20;
/// Size of a new store (at most 1 MiB, P11).
pub const INITIAL_SIZE: u64 = 64 * 1024;
/// Largest growth step: below it the store doubles on each growth (P11).
pub const MAX_GROWTH_STEP: u64 = 1 << 30;

/// Options for opening a store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenOptions {
    /// Maximum size of the store in bytes, for as long as it stays open (P10). At least [`MIN_MAX_SIZE`]; a
    /// value that is not a multiple of the page size is rounded down.
    pub max_size: Option<u64>,
}

/// Information about an open store (P12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// Format version of the store.
    pub format_version: u32,
    /// Page size in bytes.
    pub page_size: u32,
    /// Size of the store in bytes: of `muninn.data` without an incomplete final page, or of the memory its
    /// pages take in temporary mode.
    pub size_bytes: u64,
    /// Number of pages, header pages included.
    pub page_count: u64,
    /// Effective maximum size, after rounding down to the page size, or `None`.
    pub max_size: Option<u64>,
    /// Whether the store lives only in memory.
    pub temporary: bool,
}

/// An open store.
///
/// References returned by [`Pager::page`] borrow the Pager, so the compiler keeps them from outliving it (P15).
/// Growing the store takes `&self` and keeps every reference valid. Writing a page takes `&mut self`, so no
/// reference can be alive while a page changes, except through the scratch area (module 2), which writes one page
/// while others are borrowed and guarantees that nobody holds a reference to the page it writes.
pub struct Pager {
    folder: Option<PathBuf>,
    storage: Storage,
    max_size: Option<u64>,
    writes_disabled: AtomicBool,
    /// The current header and the copy the next header write goes to (J6 of the Journal PRD).
    header: Mutex<HeaderState>,
}

#[derive(Clone, Copy)]
struct HeaderState {
    header: Header,
    /// Index in [`format::HEADER_COPY_OFFSETS`] of the copy the next write replaces: never the current one.
    next_copy: usize,
    /// What the other copy records as applied: `0` when it is damaged.
    other_applied: u64,
}

impl HeaderState {
    fn new(header: Header, current_copy: usize, other: Option<Header>) -> HeaderState {
        HeaderState {
            header,
            next_copy: 1 - current_copy,
            other_applied: other.map_or(0, |h| h.applied_txid.min(header.applied_txid)),
        }
    }
}

enum Storage {
    File(FileStorage),
    Memory(Mutex<MemoryState>),
}

struct FileStorage {
    file: sys::LockedFile,
    path: PathBuf,
    state: Mutex<FileState>,
}

struct FileState {
    /// Size in bytes, a multiple of the page size.
    len: u64,
    /// Every mapping made since the store opened; the last one covers `len`. Older mappings stay alive until
    /// the store closes, so references into them remain valid after a growth.
    maps: Vec<Mmap>,
}

struct MemoryState {
    /// Size in bytes, a multiple of the page size.
    len: u64,
    /// Chunks of memory, by the byte offset each one starts at. A growth adds a chunk and never moves the
    /// existing ones.
    chunks: Vec<Chunk>,
}

/// A zeroed allocation of a temporary store, owned through a raw pointer: after it is allocated no `&mut` to the
/// whole chunk is ever created, so one page can be written while other pages of the chunk are borrowed.
struct Chunk {
    start: u64,
    ptr: NonNull<u8>,
    len: usize,
}

// SAFETY: a Chunk owns its allocation like a `Box<[u8]>` does. Its bytes are only read through `Pager::raw_page`
// and written through `Pager::write_sealed_at`, whose rules keep a page from being written while it is borrowed.
unsafe impl Send for Chunk {}
// SAFETY: see `Send`; shared access only reads bytes, under the same rules.
unsafe impl Sync for Chunk {}

impl Chunk {
    fn new(start: u64, bytes: Box<[u8]>) -> Chunk {
        let len = bytes.len();
        let ptr =
            NonNull::new(Box::into_raw(bytes).cast::<u8>()).expect("Box pointers are never null");
        Chunk { start, ptr, len }
    }

    /// Pointer to byte `offset` of the chunk.
    fn at(&self, offset: usize) -> *mut u8 {
        assert!(offset < self.len);
        // SAFETY: `offset` is inside the allocation, as just checked.
        unsafe { self.ptr.as_ptr().add(offset) }
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        let bytes = std::ptr::slice_from_raw_parts_mut(self.ptr.as_ptr(), self.len);
        // SAFETY: `ptr` and `len` come from `Box::into_raw` of a `Box<[u8]>` of `len` bytes in `Chunk::new`, and
        // the allocation is freed only here, once.
        drop(unsafe { Box::from_raw(bytes) });
    }
}

impl Pager {
    /// Opens the store in `folder`, creating the folder and the store when needed (flow of the Pager PRD).
    pub fn open(folder: impl AsRef<Path>, options: &OpenOptions) -> Result<Pager> {
        let max_size = effective_max_size(options)?;
        let folder = folder.as_ref();
        prepare_folder(folder)?;
        loop {
            let entries = list_folder(folder)?;
            if entries.iter().any(|e| is_named(e, DATA_FILE)) {
                return open_existing(folder, max_size);
            }
            let only_leftover = entries.len() == 1 && is_named(&entries[0], NEW_DATA_FILE);
            if only_leftover {
                match fs::symlink_metadata(folder.join(NEW_DATA_FILE)) {
                    Ok(meta) if meta.is_file() => {}
                    // A folder named muninn.data.new is not a leftover of ours.
                    Ok(_) => {
                        return Err(Error::NotAStore {
                            folder: folder.to_path_buf(),
                        });
                    }
                    // Another opener renamed or discarded it since the folder was listed: look again.
                    Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(access(folder, e)),
                }
            }
            if !entries.is_empty() && !only_leftover {
                return Err(Error::NotAStore {
                    folder: folder.to_path_buf(),
                });
            }
            if only_leftover {
                discard_leftover(folder)?;
            }
            match create(folder, max_size)? {
                Created::Store(pager) => return Ok(pager),
                Created::Retry => continue,
            }
        }
    }

    /// Opens a store that lives only in memory (G9).
    pub fn open_temporary(options: &OpenOptions) -> Result<Pager> {
        let max_size = effective_max_size(options)?;
        let chunk = sys::alloc_zeroed(INITIAL_SIZE as usize).map_err(|_| Error::OutOfMemory {
            requested: INITIAL_SIZE,
        })?;
        Ok(Pager {
            folder: None,
            storage: Storage::Memory(Mutex::new(MemoryState {
                len: INITIAL_SIZE,
                chunks: vec![Chunk::new(0, chunk)],
            })),
            max_size,
            writes_disabled: AtomicBool::new(false),
            header: Mutex::new(HeaderState::new(
                Header::new_store(),
                0,
                Some(Header::new_store()),
            )),
        })
    }

    /// The folder of the store, or `None` for a temporary store.
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    /// The current header.
    pub fn header(&self) -> Header {
        self.header.lock().unwrap().header
    }

    /// Writes `header` with the next revision into the header copy that is not the current one, and flushes it
    /// (J6 of the Journal PRD), so that a torn write leaves the other copy intact (P6). A temporary store only
    /// keeps it in memory. A failure makes the store refuse writes like any other (G12).
    pub fn write_header(&self, header: Header) -> Result<()> {
        self.check_writable()?;
        let mut state = self.header.lock().unwrap();
        let header = Header {
            revision: state.header.revision + 1,
            ..header
        };
        if let Storage::File(f) = &self.storage {
            let offset = format::HEADER_COPY_OFFSETS[state.next_copy];
            sys::write_at(&f.file, &f.path, offset, &header.encode())
                .and_then(|()| sys::sync(&f.file, &f.path))
                .map_err(|e| self.write_failed(e, 0))?;
        }
        state.other_applied = state.header.applied_txid;
        state.header = header;
        state.next_copy = 1 - state.next_copy;
        Ok(())
    }

    /// The last commit both header copies record as applied: journal files up to it are needed by neither (J8).
    pub(crate) fn applied_by_both_copies(&self) -> u64 {
        let state = self.header.lock().unwrap();
        state.header.applied_txid.min(state.other_applied)
    }
    pub fn info(&self) -> Info {
        let len = self.len();
        Info {
            format_version: FORMAT_VERSION,
            page_size: PAGE_SIZE as u32,
            size_bytes: len,
            page_count: len / PAGE_SIZE_U64,
            max_size: self.max_size,
            temporary: self.folder.is_none(),
        }
    }

    /// Number of pages of the store, header pages included.
    pub fn page_count(&self) -> u64 {
        self.len() / PAGE_SIZE_U64
    }

    fn len(&self) -> u64 {
        match &self.storage {
            Storage::File(f) => f.state.lock().unwrap().len,
            Storage::Memory(m) => m.lock().unwrap().len,
        }
    }

    /// The payload of page `page_number`, without copying it (P15), after checking its integrity (G6).
    ///
    /// # Panics
    ///
    /// If `page_number` is a header page.
    pub fn page(&self, page_number: u64) -> Result<&[u8]> {
        assert!(
            page_number >= HEADER_PAGES,
            "header pages are not read through Pager::page"
        );
        let Some(page) = self.raw_page(page_number) else {
            return Err(self.damaged(page_number));
        };
        if !format::verify_page(page_number, page) {
            return Err(self.damaged(page_number));
        }
        Ok(&page[PAGE_HEADER_SIZE..])
    }

    /// Whether every byte of page `page_number` is zero: a page reserved by a growth and never written. `false`
    /// for a page beyond the end of the store.
    pub fn page_is_blank(&self, page_number: u64) -> bool {
        self.raw_page(page_number)
            .is_some_and(|page| page.iter().all(|&b| b == 0))
    }

    /// The bytes of page `page_number`, without checking them, or `None` beyond the end of the store.
    fn raw_page(&self, page_number: u64) -> Option<&[u8]> {
        match &self.storage {
            Storage::File(f) => {
                let state = f.state.lock().unwrap();
                if page_number >= state.len / PAGE_SIZE_U64 {
                    return None;
                }
                let map = state
                    .maps
                    .last()
                    .expect("a file store always has a mapping");
                let offset = (page_number * PAGE_SIZE_U64) as usize;
                let ptr = map[offset..offset + PAGE_SIZE].as_ptr();
                drop(state);
                // SAFETY: `ptr` points to PAGE_SIZE bytes inside a mapping owned by `self`. Mappings are only
                // dropped with the Pager, and pushing new ones moves the `Mmap` handles but not the mapped memory,
                // so the bytes stay valid for the lifetime of `&self`. They are not modified while the returned
                // reference lives: page writes take `&mut self`, or go through `write_sealed_shared`, whose caller
                // guarantees that no reference to that page is alive; other programs modifying the file while the
                // store is open is outside the scope of the PRD (P16).
                Some(unsafe { std::slice::from_raw_parts(ptr, PAGE_SIZE) })
            }
            Storage::Memory(m) => {
                let state = m.lock().unwrap();
                if page_number >= state.len / PAGE_SIZE_U64 {
                    return None;
                }
                let (chunk, offset) = locate(&state.chunks, page_number);
                let ptr = state.chunks[chunk].at(offset);
                drop(state);
                // SAFETY: `ptr` points to PAGE_SIZE bytes of a chunk owned by `self` (pages never straddle chunks,
                // which are multiples of the page size). Chunks are only freed with the Pager and never move, so
                // the bytes stay valid for the lifetime of `&self`. They are only written through `&mut self` or
                // `write_sealed_shared`, whose caller guarantees that no reference to that page is alive.
                Some(unsafe { std::slice::from_raw_parts(ptr, PAGE_SIZE) })
            }
        }
    }

    /// Writes `payload` as the content of page `page_number`; the rest of the page is zeroed.
    ///
    /// A failure other than a lack of space makes the store refuse writes until it is reopened (G12).
    ///
    /// # Panics
    ///
    /// If `page_number` is a header page or beyond the end of the store (call [`Pager::ensure_pages`] first),
    /// or if `payload` is longer than [`format::PAGE_PAYLOAD_SIZE`].
    pub fn write_page(&mut self, page_number: u64, payload: &[u8]) -> Result<()> {
        assert!(
            page_number >= HEADER_PAGES,
            "header pages are not written through Pager::write_page"
        );
        let count = self.page_count();
        assert!(
            page_number < count,
            "page {page_number} is beyond the end of the store ({count} pages)"
        );
        self.check_writable()?;
        let mut page = Box::new([0u8; PAGE_SIZE]);
        format::seal_page(page_number, payload, &mut page);
        // SAFETY: `&mut self` rules out any reference into the store's pages and any other write.
        let result = unsafe { self.write_sealed_at(page_number, &page) };
        result.map_err(|e| self.write_failed(e, PAGE_SIZE_U64))
    }

    /// Writes `page`, already sealed for `page_number` (recovery, J8 of the Journal PRD). A failure other than a
    /// lack of space makes the store refuse writes (G12).
    ///
    /// # Panics
    ///
    /// If `page_number` is a header page or beyond the end of the store.
    pub(crate) fn write_sealed(&mut self, page_number: u64, page: &[u8; PAGE_SIZE]) -> Result<()> {
        // SAFETY: `&mut self` rules out any reference into the store's pages and any other write.
        unsafe { self.write_sealed_shared(page_number, page) }
    }

    /// Writes `page`, already sealed for `page_number`, while other pages of the store may be borrowed. A failure
    /// other than a lack of space makes the store refuse writes until it is reopened (G12).
    ///
    /// # Safety
    ///
    /// No reference returned by [`Pager::page`] for `page_number` may be alive, and no other write to the same
    /// page may run at the same time.
    ///
    /// # Panics
    ///
    /// If `page_number` is a header page or beyond the end of the store.
    pub(crate) unsafe fn write_sealed_shared(
        &self,
        page_number: u64,
        page: &[u8; PAGE_SIZE],
    ) -> Result<()> {
        assert!(
            page_number >= HEADER_PAGES && page_number < self.page_count(),
            "page {page_number} cannot be written"
        );
        self.check_writable()?;
        // SAFETY: the caller guarantees what `write_sealed_at` requires.
        let result = unsafe { self.write_sealed_at(page_number, page) };
        result.map_err(|e| self.write_failed(e, PAGE_SIZE_U64))
    }

    /// Stores the sealed bytes of a page.
    ///
    /// # Safety
    ///
    /// As [`Pager::write_sealed_shared`].
    unsafe fn write_sealed_at(&self, page_number: u64, page: &[u8; PAGE_SIZE]) -> io::Result<()> {
        match &self.storage {
            Storage::File(f) => {
                sys::write_at(&f.file, &f.path, page_number * PAGE_SIZE_U64, &page[..])
            }
            Storage::Memory(m) => {
                let state = m.lock().unwrap();
                let (chunk, offset) = locate(&state.chunks, page_number);
                let target = state.chunks[chunk].at(offset);
                // SAFETY: `target` points to PAGE_SIZE bytes of a live chunk (pages never straddle chunks), and
                // the caller guarantees that nobody reads or writes those bytes meanwhile; the chunk is reached
                // through its raw pointer, so no reference to other pages is invalidated.
                unsafe { std::ptr::copy_nonoverlapping(page.as_ptr(), target, PAGE_SIZE) };
                Ok(())
            }
        }
    }

    /// Whether the store accepts writes: `false` after a failed write until it is reopened (G12).
    pub fn accepts_writes(&self) -> bool {
        !self.writes_disabled.load(Ordering::Acquire)
    }

    /// Lets a store that refused writes try them again; only before closing, to retry the last flush (T12).
    pub(crate) fn allow_writes_again(&mut self) {
        self.writes_disabled.store(false, Ordering::Release);
    }

    /// Flushes `muninn.data` to the disk while pages may be borrowed (J6 of the Journal PRD). A failure makes the
    /// store refuse writes until it is reopened.
    pub(crate) fn flush_data(&self) -> Result<()> {
        self.check_writable()?;
        let result = match &self.storage {
            Storage::File(f) => sys::sync(&f.file, &f.path),
            Storage::Memory(_) => Ok(()),
        };
        result.map_err(|e| self.write_failed(e, 0))
    }

    /// Flushes the written pages to the disk. A failure makes the store refuse writes until it is reopened.
    pub fn sync(&mut self) -> Result<()> {
        self.check_writable()?;
        let result = match &self.storage {
            Storage::File(f) => sys::sync(&f.file, &f.path),
            Storage::Memory(_) => Ok(()),
        };
        result.map_err(|e| self.write_failed(e, 0))
    }

    /// Grows the store, if needed, so that it has at least `page_count` pages (P10, P11). References already
    /// returned by [`Pager::page`] stay valid.
    pub fn ensure_pages(&self, page_count: u64) -> Result<()> {
        self.ensure_pages_within(page_count, None)
    }

    /// Like [`Pager::ensure_pages`], but a growth never takes the store beyond `limit` bytes either: what the
    /// maximum size leaves once the other memory of the store is counted (T14 of the Transactions PRD).
    pub fn ensure_pages_within(&self, page_count: u64, limit: Option<u64>) -> Result<()> {
        self.grow(page_count, limit, true)
    }

    /// Like [`Pager::ensure_pages`], but ignoring the maximum size: recovery restores the pages of the commits it
    /// applies even beyond it (J8 of the Journal PRD).
    pub(crate) fn ensure_pages_unbounded(&self, page_count: u64) -> Result<()> {
        self.grow(page_count, None, false)
    }

    fn grow(&self, page_count: u64, limit: Option<u64>, bounded: bool) -> Result<()> {
        self.check_writable()?;
        let needed = page_count.saturating_mul(PAGE_SIZE_U64);
        match &self.storage {
            Storage::File(f) => {
                let mut state = f.state.lock().unwrap();
                if needed <= state.len {
                    return Ok(());
                }
                let new_len = self.next_size(state.len, needed, limit, bounded)?;
                sys::reserve(&f.file, &f.path, state.len, new_len)
                    .map_err(|e| self.write_failed(e, new_len - state.len))?;
                match sys::map(&f.file, &f.path, new_len) {
                    Ok(map) => state.maps.push(map),
                    Err(e) => {
                        // Keep the size the store had (P11) and give the reserved space back. Windows refuses to
                        // shrink a mapped file, so there the space stays in the file, unused.
                        let _ = f.file.set_len(state.len);
                        return Err(self.map_failed(e, new_len));
                    }
                }
                state.len = new_len;
            }
            Storage::Memory(m) => {
                let mut state = m.lock().unwrap();
                if needed <= state.len {
                    return Ok(());
                }
                let new_len = self.next_size(state.len, needed, limit, bounded)?;
                let requested = new_len - state.len;
                let chunk = sys::alloc_zeroed(requested as usize)
                    .map_err(|_| Error::OutOfMemory { requested })?;
                let start = state.len;
                state.chunks.push(Chunk::new(start, chunk));
                state.len = new_len;
            }
        }
        Ok(())
    }

    /// Closes the store: flushes it to the disk and releases its lock, which is released even if the flush
    /// fails (P14). Dropping a Pager without calling `close` also releases the lock.
    pub fn close(self) -> Result<()> {
        match &self.storage {
            Storage::File(f) => {
                let result = sys::sync(&f.file, &f.path);
                // Windows releases a handle's locks when it closes, but asks for an explicit unlock first. On
                // Unix the lock goes with the descriptor, which `LockedFile` closes under its table.
                #[cfg(windows)]
                let _ = f.file.unlock();
                result.map_err(|source| Error::Io {
                    folder: self.folder.clone().unwrap_or_default(),
                    source,
                })
            }
            Storage::Memory(_) => Ok(()),
        }
    }

    /// The size the store grows to when it needs `needed` bytes and has `len` (P10, P11).
    fn next_size(&self, len: u64, needed: u64, limit: Option<u64>, bounded: bool) -> Result<u64> {
        let Some(needed) = needed.checked_next_multiple_of(PAGE_SIZE_U64) else {
            return Err(Error::MaxSize {
                max_size: self.max_size.unwrap_or(u64::MAX - u64::MAX % PAGE_SIZE_U64),
                needed,
            });
        };
        if !bounded {
            return Ok(needed);
        }
        let cap = match (self.max_size, limit) {
            (Some(max), Some(limit)) => Some(max.min(limit)),
            (max, limit) => max.or(limit),
        };
        if let Some(cap) = cap
            && needed > cap
        {
            // What the store needs in all, counting the memory that `limit` leaves out.
            let others = match (self.max_size, limit) {
                (Some(max), Some(limit)) => max.saturating_sub(limit),
                _ => 0,
            };
            return Err(Error::MaxSize {
                max_size: self.max_size.unwrap_or(cap),
                needed: needed.saturating_add(others),
            });
        }
        let step = len.min(MAX_GROWTH_STEP);
        let target = (len + step).max(needed);
        Ok(cap.map_or(target, |cap| target.min(cap)))
    }

    pub(crate) fn check_writable(&self) -> Result<()> {
        if self.writes_disabled.load(Ordering::Acquire) {
            return Err(Error::Io {
                folder: self.folder.clone().unwrap_or_default(),
                source: io::Error::other(
                    "writes are refused after a failed write; reopen the store",
                ),
            });
        }
        Ok(())
    }

    /// Classifies a failed write of something that belongs to the store outside `muninn.data` (the journal, the
    /// temporary area), with the same consequence as a failed page write: an I/O failure makes the store refuse
    /// writes until it is reopened (G12).
    pub(crate) fn fail_write(&self, source: io::Error, requested: u64) -> Error {
        self.write_failed(source, requested)
    }

    fn write_failed(&self, source: io::Error, requested: u64) -> Error {
        let error = Error::from_write(
            self.folder.as_deref().unwrap_or(Path::new("")),
            source,
            requested,
        );
        if matches!(error, Error::Io { .. }) {
            self.writes_disabled.store(true, Ordering::Release);
        }
        error
    }

    fn map_failed(&self, source: io::Error, requested: u64) -> Error {
        match source.kind() {
            io::ErrorKind::OutOfMemory => Error::OutOfMemory { requested },
            _ => Error::Io {
                folder: self.folder.clone().unwrap_or_default(),
                source,
            },
        }
    }

    fn damaged(&self, page: u64) -> Error {
        Error::Damaged {
            folder: self.folder.clone(),
            page: Some(page),
        }
    }
}

/// Chunk index and byte offset inside it of `page_number`.
fn locate(chunks: &[Chunk], page_number: u64) -> (usize, usize) {
    let byte = page_number * PAGE_SIZE_U64;
    let chunk = chunks.partition_point(|c| c.start <= byte) - 1;
    (chunk, (byte - chunks[chunk].start) as usize)
}

fn effective_max_size(options: &OpenOptions) -> Result<Option<u64>> {
    match options.max_size {
        Some(max) if max < MIN_MAX_SIZE => Err(Error::InvalidOptions {
            reason: "the maximum size must be at least 1 MiB",
        }),
        Some(max) => Ok(Some(max - max % PAGE_SIZE_U64)),
        None => Ok(None),
    }
}

fn access(folder: &Path, source: io::Error) -> Error {
    Error::Access {
        folder: folder.to_path_buf(),
        source,
    }
}

/// Step 1 of the order of checks: the folder can be used; it is created with its parents when missing.
fn prepare_folder(folder: &Path) -> Result<()> {
    match fs::metadata(folder) {
        Ok(meta) if meta.is_dir() => Ok(()),
        Ok(_) => Err(access(
            folder,
            io::Error::new(io::ErrorKind::NotADirectory, "the path is a file"),
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let mut missing = Vec::new();
            let mut level = Some(folder);
            while let Some(path) = level.filter(|p| !p.as_os_str().is_empty() && !p.exists()) {
                missing.push(path);
                level = path.parent();
            }
            fs::create_dir_all(folder).map_err(|e| access(folder, e))?;
            // A store that survives a power cut (P8) needs the folders that hold it to survive too.
            for created in missing {
                let parent = created
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                sys::sync_dir(parent).map_err(|e| create_error(folder, e))?;
            }
            Ok(())
        }
        Err(e) => Err(access(folder, e)),
    }
}

/// Whether a folder entry has the reserved name `name`: case-insensitively on Windows, where the file system
/// treats both spellings as the same file.
fn is_named(entry: &OsString, name: &str) -> bool {
    if cfg!(windows) {
        entry.to_str().is_some_and(|e| e.eq_ignore_ascii_case(name))
    } else {
        entry == name
    }
}

fn list_folder(folder: &Path) -> Result<Vec<OsString>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(folder).map_err(|e| access(folder, e))? {
        names.push(entry.map_err(|e| access(folder, e))?.file_name());
    }
    Ok(names)
}

fn open_existing(folder: &Path, max_size: Option<u64>) -> Result<Pager> {
    let path = folder.join(DATA_FILE);
    let meta = fs::metadata(&path).map_err(|e| access(folder, e))?;
    if meta.is_dir() {
        return Err(Error::NotAStore {
            folder: folder.to_path_buf(),
        });
    }
    // Step 2 of the order of checks: the store's exclusive lock (D6, P9).
    let file = match sys::open_locked(&path) {
        Ok(file) => file,
        Err(sys::LockError::InUse) => {
            return Err(Error::StoreInUse {
                folder: folder.to_path_buf(),
            });
        }
        Err(sys::LockError::Io(e)) => return Err(access(folder, e)),
    };
    let mut start = vec![0u8; HEADER_PAGES as usize * PAGE_SIZE];
    let read = sys::read_start(&file, &mut start).map_err(|source| Error::Io {
        folder: folder.to_path_buf(),
        source,
    })?;
    let header = match format::read_header_pages(&start[..read]) {
        HeaderRead::NotAStore => {
            return Err(Error::NotAStore {
                folder: folder.to_path_buf(),
            });
        }
        HeaderRead::Damaged => {
            return Err(Error::Damaged {
                folder: Some(folder.to_path_buf()),
                page: None,
            });
        }
        HeaderRead::Found(header) if header.format_version != FORMAT_VERSION => {
            return Err(Error::DifferentFormat {
                folder: folder.to_path_buf(),
                found: header.format_version,
                expected: FORMAT_VERSION,
            });
        }
        HeaderRead::Found(header) => header,
    };
    // The copy the header was read from is the current one; with two equal copies, either.
    let copies = format::header_copies(&start[..read]);
    let current_copy = usize::from(copies[0] != Some(header));
    let header = HeaderState::new(header, current_copy, copies[1 - current_copy]);
    let file_len = file
        .metadata()
        .map_err(|source| Error::Io {
            folder: folder.to_path_buf(),
            source,
        })?
        .len();
    // An incomplete final page, left by an interrupted growth, is ignored (P11).
    let len = file_len - file_len % PAGE_SIZE_U64;
    let map = map_new(&file, &path, len, folder)?;
    Ok(file_pager(folder, file, path, len, map, max_size, header))
}

fn map_new(file: &File, path: &Path, len: u64, folder: &Path) -> Result<Mmap> {
    sys::map(file, path, len).map_err(|source| match source.kind() {
        io::ErrorKind::OutOfMemory => Error::OutOfMemory { requested: len },
        _ => Error::Io {
            folder: folder.to_path_buf(),
            source,
        },
    })
}

fn file_pager(
    folder: &Path,
    file: sys::LockedFile,
    path: PathBuf,
    len: u64,
    map: Mmap,
    max_size: Option<u64>,
    header: HeaderState,
) -> Pager {
    Pager {
        header: Mutex::new(header),
        folder: Some(folder.to_path_buf()),
        storage: Storage::File(FileStorage {
            file,
            path,
            state: Mutex::new(FileState {
                len,
                maps: vec![map],
            }),
        }),
        max_size,
        writes_disabled: AtomicBool::new(false),
    }
}

/// Removes the `muninn.data.new` left by an interrupted creation, unless another opener holds it (P8).
fn discard_leftover(folder: &Path) -> Result<()> {
    let path = folder.join(NEW_DATA_FILE);
    let in_use = || Error::StoreInUse {
        folder: folder.to_path_buf(),
    };
    let file = match sys::open_locked(&path) {
        Ok(file) => file,
        Err(sys::LockError::InUse) => return Err(in_use()),
        Err(sys::LockError::Io(e)) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(sys::LockError::Io(e)) if sys::is_sharing_violation(&e) => return Err(in_use()),
        Err(sys::LockError::Io(e)) => return Err(access(folder, e)),
    };
    drop(file);
    match sys::remove(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        // Another opener is preparing its store in it (it is opened without delete sharing on Windows).
        Err(e) if sys::is_sharing_violation(&e) => Err(in_use()),
        Err(e) => Err(access(folder, e)),
    }
}

fn create_error(folder: &Path, source: io::Error) -> Error {
    match source.kind() {
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
            access(folder, source)
        }
        _ => Error::from_write(folder, source, INITIAL_SIZE),
    }
}

/// What a creation attempt produced.
enum Created {
    Store(Pager),
    /// The folder changed under this opener (another opener created the store or discarded this opener's
    /// `muninn.data.new`): the caller looks at the folder again.
    Retry,
}

/// Creates a store in an empty folder: prepares it in `muninn.data.new` under the store lock, flushes it and
/// renames it to `muninn.data` without replacing an existing one, keeping the file open and locked from start
/// to end (P8, P9).
///
/// Another opener can only discard this opener's `muninn.data.new` in the instant between its creation and its
/// lock (and only on Unix: on Windows it is opened without delete sharing). The rename then finds that the file
/// at `muninn.data.new` is no longer this one, and the caller looks at the folder again.
fn create(folder: &Path, max_size: Option<u64>) -> Result<Created> {
    let new_path = folder.join(NEW_DATA_FILE);
    let data_path = folder.join(DATA_FILE);
    let file = match sys::create_locked(&new_path) {
        Ok(file) => file,
        Err(sys::LockError::InUse) => {
            return Err(Error::StoreInUse {
                folder: folder.to_path_buf(),
            });
        }
        Err(sys::LockError::Io(e)) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(Error::StoreInUse {
                folder: folder.to_path_buf(),
            });
        }
        Err(sys::LockError::Io(e)) => return Err(create_error(folder, e)),
    };
    let ours = sys::FileIdentity::of(&file);
    let discard = |file: sys::LockedFile| {
        drop(file);
        ours.remove_if_same(&new_path);
    };
    let prepared = (|| {
        let mut pages = vec![0u8; HEADER_PAGES as usize * PAGE_SIZE];
        format::write_header_pages(&mut pages, &Header::new_store());
        sys::write_at(&file, &new_path, 0, &pages).map_err(|e| create_error(folder, e))?;
        sys::reserve(&file, &new_path, pages.len() as u64, INITIAL_SIZE)
            .map_err(|e| create_error(folder, e))?;
        sys::sync(&file, &new_path).map_err(|e| create_error(folder, e))
    })();
    if let Err(error) = prepared {
        discard(file);
        return Err(error);
    }
    match sys::rename_open_no_replace(&file, &new_path, &data_path) {
        Ok(()) => {}
        // Another opener created the store first: discard what this one prepared.
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            discard(file);
            return Ok(Created::Retry);
        }
        // Another opener discarded this file as a leftover before it was locked.
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Created::Retry),
        Err(e) => {
            discard(file);
            return Err(create_error(folder, e));
        }
    }
    sys::sync_dir(folder).map_err(|e| create_error(folder, e))?;
    let map = map_new(&file, &data_path, INITIAL_SIZE, folder)?;
    Ok(Created::Store(file_pager(
        folder,
        file,
        data_path,
        INITIAL_SIZE,
        map,
        max_size,
        HeaderState::new(Header::new_store(), 0, Some(Header::new_store())),
    )))
}

#[cfg(test)]
#[path = "pager_tests.rs"]
mod tests;
