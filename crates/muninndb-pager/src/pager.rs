//! The Pager: opening, creating and closing a store, and reading, writing and growing its pages.

use std::ffi::OsString;
use std::fs::{self, File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
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
/// Growing the store takes `&self` and keeps every reference valid; writing a page takes `&mut self`, so no
/// reference can be alive while a page changes.
pub struct Pager {
    folder: Option<PathBuf>,
    storage: Storage,
    max_size: Option<u64>,
    writes_disabled: AtomicBool,
}

enum Storage {
    File(FileStorage),
    Memory(Mutex<MemoryState>),
}

struct FileStorage {
    file: File,
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
    /// Chunks of memory and the byte offset each one starts at. A growth adds a chunk and never moves the
    /// existing ones.
    chunks: Vec<(u64, Box<[u8]>)>,
}

impl Pager {
    /// Opens the store in `folder`, creating the folder and the store when needed (flow of the Pager PRD).
    pub fn open(folder: impl AsRef<Path>, options: &OpenOptions) -> Result<Pager> {
        let max_size = effective_max_size(options)?;
        let folder = folder.as_ref();
        prepare_folder(folder)?;
        loop {
            let entries = list_folder(folder)?;
            if entries.iter().any(|e| e == DATA_FILE) {
                return open_existing(folder, max_size);
            }
            let only_leftover = entries.len() == 1 && entries[0] == NEW_DATA_FILE;
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
                chunks: vec![(0, chunk)],
            })),
            max_size,
            writes_disabled: AtomicBool::new(false),
        })
    }

    /// Information about the store.
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
        let page: &[u8] = match &self.storage {
            Storage::File(f) => {
                let state = f.state.lock().unwrap();
                if page_number >= state.len / PAGE_SIZE_U64 {
                    return Err(self.damaged(page_number));
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
                // reference lives: page writes take `&mut self`, and other programs modifying the file while the
                // store is open is outside the scope of the PRD (P16).
                unsafe { std::slice::from_raw_parts(ptr, PAGE_SIZE) }
            }
            Storage::Memory(m) => {
                let state = m.lock().unwrap();
                if page_number >= state.len / PAGE_SIZE_U64 {
                    return Err(self.damaged(page_number));
                }
                let (chunk, offset) = locate(&state.chunks, page_number);
                let ptr = state.chunks[chunk].1[offset..offset + PAGE_SIZE].as_ptr();
                drop(state);
                // SAFETY: `ptr` points to PAGE_SIZE bytes of a boxed chunk owned by `self`. Chunks are only freed
                // with the Pager, and pushing new ones moves the boxes but not their heap memory, so the bytes stay
                // valid for the lifetime of `&self`. They are only written through `&mut self`, which cannot
                // coexist with the returned reference.
                unsafe { std::slice::from_raw_parts(ptr, PAGE_SIZE) }
            }
        };
        if !format::verify_page(page_number, page) {
            return Err(self.damaged(page_number));
        }
        Ok(&page[PAGE_HEADER_SIZE..])
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
        let result = match &mut self.storage {
            Storage::File(f) => {
                let mut page = Box::new([0u8; PAGE_SIZE]);
                format::seal_page(page_number, payload, &mut page);
                sys::write_at(&f.file, &f.path, page_number * PAGE_SIZE_U64, &page[..])
            }
            Storage::Memory(m) => {
                let state = m.get_mut().unwrap();
                let (chunk, offset) = locate(&state.chunks, page_number);
                let page: &mut [u8; PAGE_SIZE] = (&mut state.chunks[chunk].1
                    [offset..offset + PAGE_SIZE])
                    .try_into()
                    .unwrap();
                format::seal_page(page_number, payload, page);
                Ok(())
            }
        };
        result.map_err(|e| self.write_failed(e, PAGE_SIZE_U64))
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
        self.check_writable()?;
        let needed = page_count.saturating_mul(PAGE_SIZE_U64);
        match &self.storage {
            Storage::File(f) => {
                let mut state = f.state.lock().unwrap();
                if needed <= state.len {
                    return Ok(());
                }
                let new_len = self.next_size(state.len, needed)?;
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
                let new_len = self.next_size(state.len, needed)?;
                let requested = new_len - state.len;
                let chunk = sys::alloc_zeroed(requested as usize)
                    .map_err(|_| Error::OutOfMemory { requested })?;
                let start = state.len;
                state.chunks.push((start, chunk));
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
    fn next_size(&self, len: u64, needed: u64) -> Result<u64> {
        let needed = needed.div_ceil(PAGE_SIZE_U64) * PAGE_SIZE_U64;
        if let Some(max_size) = self.max_size
            && needed > max_size
        {
            return Err(Error::MaxSize { max_size, needed });
        }
        let step = len.min(MAX_GROWTH_STEP);
        let target = (len + step).max(needed);
        Ok(self.max_size.map_or(target, |max| target.min(max)))
    }

    fn check_writable(&self) -> Result<()> {
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
fn locate(chunks: &[(u64, Box<[u8]>)], page_number: u64) -> (usize, usize) {
    let byte = page_number * PAGE_SIZE_U64;
    let chunk = chunks.partition_point(|(start, _)| *start <= byte) - 1;
    (chunk, (byte - chunks[chunk].0) as usize)
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
            fs::create_dir_all(folder).map_err(|e| access(folder, e))
        }
        Err(e) => Err(access(folder, e)),
    }
}

fn list_folder(folder: &Path) -> Result<Vec<OsString>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(folder).map_err(|e| access(folder, e))? {
        names.push(entry.map_err(|e| access(folder, e))?.file_name());
    }
    Ok(names)
}

/// Step 2 of the order of checks: the store's exclusive lock (D6, P9).
fn lock(file: &File, folder: &Path) -> Result<()> {
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(TryLockError::WouldBlock) => Err(Error::StoreInUse {
            folder: folder.to_path_buf(),
        }),
        Err(TryLockError::Error(e)) => Err(access(folder, e)),
    }
}

fn open_existing(folder: &Path, max_size: Option<u64>) -> Result<Pager> {
    let path = folder.join(DATA_FILE);
    let meta = fs::metadata(&path).map_err(|e| access(folder, e))?;
    if meta.is_dir() {
        return Err(Error::NotAStore {
            folder: folder.to_path_buf(),
        });
    }
    let file = sys::open_rw(&path).map_err(|e| access(folder, e))?;
    lock(&file, folder)?;
    let mut start = vec![0u8; HEADER_PAGES as usize * PAGE_SIZE];
    let read = sys::read_start(&file, &mut start).map_err(|source| Error::Io {
        folder: folder.to_path_buf(),
        source,
    })?;
    match format::read_header_pages(&start[..read]) {
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
        HeaderRead::Found(_) => {}
    }
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
    Ok(file_pager(folder, file, path, len, map, max_size))
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
    file: File,
    path: PathBuf,
    len: u64,
    map: Mmap,
    max_size: Option<u64>,
) -> Pager {
    Pager {
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
    let file = match sys::open_rw(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) if sys::is_sharing_violation(&e) => return Err(in_use()),
        Err(e) => return Err(access(folder, e)),
    };
    lock(&file, folder)?;
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
    let file = match sys::create_new(&new_path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(Error::StoreInUse {
                folder: folder.to_path_buf(),
            });
        }
        Err(e) => return Err(create_error(folder, e)),
    };
    let ours = sys::FileIdentity::of(&file);
    let discard = |file: File| {
        drop(file);
        ours.remove_if_same(&new_path);
    };
    let prepared = (|| {
        lock(&file, folder)?;
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
    )))
}

#[cfg(test)]
#[path = "pager_tests.rs"]
mod tests;
