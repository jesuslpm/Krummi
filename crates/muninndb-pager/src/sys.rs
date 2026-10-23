//! Operating-system operations of the Pager.
//!
//! Every file operation the Pager performs goes through this module, so tests can inject failures (disk full,
//! I/O errors, out of memory), kill the process at a chosen point, or record the operations to simulate a power
//! cut (see `faults`, compiled only for tests).

use std::fs::{self, File};
use std::io;
use std::path::Path;

use memmap2::{Mmap, MmapMut, MmapOptions};

#[cfg(any(test, feature = "test-hooks"))]
pub(crate) use faults::hooks;

/// The operations tests can target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Op {
    CreateFile,
    Write,
    Reserve,
    Sync,
    Rename,
    Remove,
    SyncDir,
    Alloc,
    Map,
    Lock,
    /// Registering a snapshot (module 2), which can run out of memory.
    Snapshot,
    /// Reading a journal file (module 3).
    Read,
    /// Creating, growing or removing the files and the folder of the temporary area (module 3).
    Temp,
}

#[inline]
fn before(op: Op, path: &Path) -> io::Result<()> {
    #[cfg(any(test, feature = "test-hooks"))]
    {
        hooks::before(op, path)
    }
    #[cfg(not(any(test, feature = "test-hooks")))]
    {
        let _ = (op, path);
        Ok(())
    }
}

/// Creates `path`, failing if it already exists, opened for reading and writing.
pub(crate) fn create_new(path: &Path) -> io::Result<File> {
    before(Op::CreateFile, path)?;
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create_new(true);
    // While the store is prepared, nobody else may delete the file (see `is_sharing_violation`), and the
    // handle itself can rename it (see `rename_open_no_replace`).
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const GENERIC_READ: u32 = 0x8000_0000;
        const GENERIC_WRITE: u32 = 0x4000_0000;
        const DELETE: u32 = 0x0001_0000;
        const FILE_SHARE_READ_WRITE: u32 = 0x1 | 0x2;
        options.access_mode(GENERIC_READ | GENERIC_WRITE | DELETE);
        options.share_mode(FILE_SHARE_READ_WRITE);
    }
    let file = options.open(path)?;
    #[cfg(any(test, feature = "test-hooks"))]
    hooks::record(faults::Recorded::Create(path.to_path_buf()));
    Ok(file)
}

/// Whether `error` means another handle keeps the file from being opened or deleted (Windows only).
pub(crate) fn is_sharing_violation(error: &io::Error) -> bool {
    cfg!(windows) && error.raw_os_error() == Some(32) // ERROR_SHARING_VIOLATION
}

/// Identity of an open file, to avoid deleting a file another opener put under the same name.
pub(crate) struct FileIdentity(#[cfg_attr(not(unix), allow(dead_code))] Option<(u64, u64)>);

impl FileIdentity {
    pub(crate) fn of(file: &File) -> FileIdentity {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            FileIdentity(file.metadata().ok().map(|m| (m.dev(), m.ino())))
        }
        #[cfg(not(unix))]
        {
            let _ = file;
            FileIdentity(None)
        }
    }

    /// Whether `path` is this file. Always true on Windows, where another opener's file cannot replace this
    /// one while it is open (it is opened without delete sharing).
    pub(crate) fn is_at(&self, path: &Path) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let at_path = fs::metadata(path).ok().map(|m| (m.dev(), m.ino()));
            at_path.is_some() && at_path == self.0
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            true
        }
    }

    /// Removes `path` if it is still this file.
    pub(crate) fn remove_if_same(&self, path: &Path) {
        if self.is_at(path) {
            let _ = remove(path);
        }
    }
}

/// Why a file could not be opened and locked.
#[derive(Debug)]
pub(crate) enum LockError {
    /// Another opener, in this process or another one, holds the lock.
    InUse,
    /// The file could not be opened, created or locked.
    Io(io::Error),
}

/// A file opened and exclusively locked for a store (D6, P9). The lock lasts until the value is dropped.
///
/// On Unix the lock is a POSIX record lock (`fcntl F_SETLK`). Unlike `flock`, it belongs to the process and is
/// not inherited by child processes, so a child created while the store is open never keeps it locked. POSIX
/// locks do not conflict inside one process, and closing *any* descriptor of the file drops the process's lock,
/// so a table of the files this process has locked (by device and inode) detects a second open in the same
/// process before a second descriptor exists, and every descriptor of a locked file is closed only under the
/// table's mutex, by the holder. On Windows the lock is `LockFileEx` on the handle, which handles are never
/// inherited by child processes and which already conflicts inside one process.
pub(crate) struct LockedFile {
    file: Option<File>,
    #[cfg(unix)]
    id: (u64, u64),
}

impl std::ops::Deref for LockedFile {
    type Target = File;
    fn deref(&self) -> &File {
        self.file.as_ref().expect("the file is only taken on drop")
    }
}

#[cfg(unix)]
mod lock_table {
    use std::collections::HashMap;
    use std::fs::File;
    use std::sync::{Mutex, MutexGuard};

    /// Files this process has locked, by device and inode, with descriptors of them opened by mistake (a path
    /// that changed between the check and the open), which are closed only when the holder closes its own.
    pub(super) type Table = Option<HashMap<(u64, u64), Vec<File>>>;

    static LOCKED: Mutex<Table> = Mutex::new(None);

    pub(super) fn table() -> MutexGuard<'static, Table> {
        LOCKED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(unix)]
fn identity(meta: &fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (meta.dev(), meta.ino())
}

/// Opens the existing file at `path` for reading and writing and locks it.
pub(crate) fn open_locked(path: &Path) -> Result<LockedFile, LockError> {
    #[cfg(unix)]
    {
        let mut guard = lock_table::table();
        let table = guard.get_or_insert_with(Default::default);
        if let Ok(meta) = fs::metadata(path)
            && table.contains_key(&identity(&meta))
        {
            return Err(LockError::InUse);
        }
        let file = open_rw(path).map_err(LockError::Io)?;
        let id = identity(&file.metadata().map_err(LockError::Io)?);
        if let Some(extra) = table.get_mut(&id) {
            // The path now leads to a file this process holds: closing this descriptor would drop that lock.
            extra.push(file);
            return Err(LockError::InUse);
        }
        posix_lock(&file)?;
        table.insert(id, Vec::new());
        Ok(LockedFile {
            file: Some(file),
            id,
        })
    }
    #[cfg(not(unix))]
    {
        let file = open_rw(path).map_err(LockError::Io)?;
        handle_lock(&file)?;
        Ok(LockedFile { file: Some(file) })
    }
}

/// Creates the file at `path` (see [`create_new`]) and locks it.
pub(crate) fn create_locked(path: &Path) -> Result<LockedFile, LockError> {
    #[cfg(unix)]
    {
        let mut guard = lock_table::table();
        let table = guard.get_or_insert_with(Default::default);
        let file = create_new(path).map_err(LockError::Io)?;
        let locked = file
            .metadata()
            .map_err(LockError::Io)
            .and_then(|meta| posix_lock(&file).map(|()| identity(&meta)));
        match locked {
            Ok(id) => {
                table.insert(id, Vec::new());
                Ok(LockedFile {
                    file: Some(file),
                    id,
                })
            }
            Err(error) => {
                drop(file);
                discard_created(path, &error);
                Err(error)
            }
        }
    }
    #[cfg(not(unix))]
    {
        let file = create_new(path).map_err(LockError::Io)?;
        match handle_lock(&file) {
            Ok(()) => Ok(LockedFile { file: Some(file) }),
            Err(error) => {
                drop(file);
                discard_created(path, &error);
                Err(error)
            }
        }
    }
}

/// An injected fault of the lock step (tests only): `WouldBlock` stands for another opener holding the lock.
fn lock_fault(error: io::Error) -> LockError {
    if error.kind() == io::ErrorKind::WouldBlock {
        LockError::InUse
    } else {
        LockError::Io(error)
    }
}

/// Removes a file [`create_locked`] just created but could not lock. When another opener holds it (it took it
/// for a leftover in the instant before the lock), that opener removes it.
fn discard_created(path: &Path, error: &LockError) {
    if matches!(error, LockError::Io(_)) {
        let _ = remove(path);
    }
}

#[cfg(unix)]
fn posix_lock(file: &File) -> Result<(), LockError> {
    use rustix::fs::{FlockOperation, fcntl_lock};
    before(Op::Lock, Path::new("")).map_err(lock_fault)?;
    match fcntl_lock(file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(()),
        Err(e) if e == rustix::io::Errno::AGAIN || e == rustix::io::Errno::ACCESS => {
            Err(LockError::InUse)
        }
        Err(e) => Err(LockError::Io(e.into())),
    }
}

#[cfg(not(unix))]
fn handle_lock(file: &File) -> Result<(), LockError> {
    before(Op::Lock, Path::new("")).map_err(lock_fault)?;
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(fs::TryLockError::WouldBlock) => Err(LockError::InUse),
        Err(fs::TryLockError::Error(e)) => Err(LockError::Io(e)),
    }
}

impl Drop for LockedFile {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let mut guard = lock_table::table();
            // Closing the descriptor releases the POSIX lock; it happens under the table's mutex so no other
            // opener of this process can lock the file in between and then lose that lock to this close.
            drop(self.file.take());
            if let Some(table) = guard.as_mut() {
                table.remove(&self.id);
            }
        }
        #[cfg(not(unix))]
        drop(self.file.take());
    }
}

/// Opens an existing file for reading and writing.
pub(crate) fn open_rw(path: &Path) -> io::Result<File> {
    fs::OpenOptions::new().read(true).write(true).open(path)
}

/// Writes all of `buf` at `offset`.
pub(crate) fn write_at(file: &File, path: &Path, offset: u64, buf: &[u8]) -> io::Result<()> {
    before(Op::Write, path)?;
    #[cfg(unix)]
    std::os::unix::fs::FileExt::write_all_at(file, buf, offset)?;
    #[cfg(windows)]
    {
        let mut done = 0;
        while done < buf.len() {
            let n = std::os::windows::fs::FileExt::seek_write(
                file,
                &buf[done..],
                offset + done as u64,
            )?;
            if n == 0 {
                return Err(io::Error::from(io::ErrorKind::WriteZero));
            }
            done += n;
        }
    }
    #[cfg(any(test, feature = "test-hooks"))]
    hooks::record(faults::Recorded::Write(
        path.to_path_buf(),
        offset,
        buf.to_vec(),
    ));
    Ok(())
}

/// Reads up to `buf.len()` bytes from the start of the file; returns how many were read.
pub(crate) fn read_start(file: &File, buf: &mut [u8]) -> io::Result<usize> {
    read_from(file, buf, 0)
}

/// Reads up to `buf.len()` bytes from `offset` of a journal file; returns how many were read (fewer at its end).
pub(crate) fn read_at(file: &File, path: &Path, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
    before(Op::Read, path)?;
    read_from(file, buf, offset)
}

fn read_from(file: &File, buf: &mut [u8], offset: u64) -> io::Result<usize> {
    let mut done = 0;
    while done < buf.len() {
        let at = offset + done as u64;
        #[cfg(unix)]
        let n = std::os::unix::fs::FileExt::read_at(file, &mut buf[done..], at)?;
        #[cfg(windows)]
        let n = std::os::windows::fs::FileExt::seek_read(file, &mut buf[done..], at)?;
        if n == 0 {
            break;
        }
        done += n;
    }
    Ok(done)
}

/// Opens an existing journal file for reading.
pub(crate) fn open_read(path: &Path) -> io::Result<File> {
    before(Op::Read, path)?;
    File::open(path)
}

/// Creates the folder of the temporary area.
pub(crate) fn create_temp_dir(path: &Path) -> io::Result<()> {
    before(Op::Temp, path)?;
    fs::create_dir(path)
}

/// Creates a file of the temporary area with `len` bytes reserved on disk and maps it for reading and writing.
/// Its operations are not recorded: the temporary area never takes part in recovery.
pub(crate) fn create_temp_file(path: &Path, len: u64) -> io::Result<(File, MmapMut)> {
    before(Op::Temp, path)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)?;
    let mapped = (|| {
        reserve_space(&file, 0, len)?;
        let len = usize::try_from(len).map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
        // SAFETY: the file was just created by this process inside the store's folder, which the store's lock
        // (D6, P9) keeps other MuninnDB instances out of; the temporary area writes and reads it only through this
        // mapping, under the rules of `crate::temp`. Another program modifying it is outside the scope of the PRD
        // (P16).
        unsafe { MmapOptions::new().len(len).map_mut(&file) }
    })();
    match mapped {
        Ok(map) => Ok((file, map)),
        Err(e) => {
            let _ = fs::remove_file(path);
            Err(e)
        }
    }
}

/// Removes the folder of the temporary area with everything in it; a missing folder is not an error.
pub(crate) fn remove_temp_dir(path: &Path) -> io::Result<()> {
    before(Op::Temp, path)?;
    match fs::remove_dir_all(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Grows the file from `old_len` to `new_len` bytes, reserving the space on disk so that a lack of space shows
/// up here and not on a later write (P11). On failure the file keeps `old_len`.
pub(crate) fn reserve(file: &File, path: &Path, old_len: u64, new_len: u64) -> io::Result<()> {
    debug_assert!(new_len > old_len);
    before(Op::Reserve, path)?;
    let result = reserve_space(file, old_len, new_len);
    if result.is_err() {
        // Best effort: give the space back. The size the Pager uses does not depend on it (it keeps `old_len`).
        let _ = file.set_len(old_len);
    }
    result?;
    #[cfg(any(test, feature = "test-hooks"))]
    hooks::record(faults::Recorded::SetLen(path.to_path_buf(), new_len));
    Ok(())
}

#[cfg(target_os = "linux")]
fn reserve_space(file: &File, old_len: u64, new_len: u64) -> io::Result<()> {
    use rustix::fs::{FallocateFlags, fallocate};
    match fallocate(file, FallocateFlags::empty(), old_len, new_len - old_len) {
        Ok(()) => Ok(()),
        Err(e) if e == rustix::io::Errno::OPNOTSUPP || e == rustix::io::Errno::NOSYS => {
            write_zeros(file, old_len, new_len)
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(windows)]
fn reserve_space(file: &File, _old_len: u64, new_len: u64) -> io::Result<()> {
    // NTFS allocates the clusters of a non-sparse file when its end moves, and reports a lack of space here.
    file.set_len(new_len)
}

#[cfg(not(any(target_os = "linux", windows)))]
fn reserve_space(file: &File, old_len: u64, new_len: u64) -> io::Result<()> {
    write_zeros(file, old_len, new_len)
}

#[cfg(not(windows))]
fn write_zeros(file: &File, old_len: u64, new_len: u64) -> io::Result<()> {
    const CHUNK: usize = 1 << 20;
    let zeros = vec![0u8; CHUNK];
    let mut offset = old_len;
    while offset < new_len {
        let n = CHUNK.min((new_len - offset) as usize);
        std::os::unix::fs::FileExt::write_all_at(file, &zeros[..n], offset)?;
        offset += n as u64;
    }
    Ok(())
}

/// Flushes the file's data and size to the disk.
pub(crate) fn sync(file: &File, path: &Path) -> io::Result<()> {
    before(Op::Sync, path)?;
    #[cfg(any(test, feature = "test-hooks"))]
    let skipped = faults::syncs_skipped();
    #[cfg(not(any(test, feature = "test-hooks")))]
    let skipped = false;
    if !skipped {
        file.sync_all()?;
    }
    #[cfg(any(test, feature = "test-hooks"))]
    hooks::record(faults::Recorded::Sync(path.to_path_buf()));
    Ok(())
}

/// Renames the open file `file`, created at `from` by [`create_new`], to `to`, failing with `AlreadyExists`
/// instead of replacing `to`, and with `NotFound` when `from` is no longer `file` (another opener discarded it).
/// The file stays open, so its lock is never released. On Windows the rename goes through the handle and is
/// flushed before returning.
pub(crate) fn rename_open_no_replace(file: &File, from: &Path, to: &Path) -> io::Result<()> {
    before(Op::Rename, from)?;
    rename_open_impl(file, from, to)?;
    #[cfg(any(test, feature = "test-hooks"))]
    hooks::record(faults::Recorded::Rename(
        from.to_path_buf(),
        to.to_path_buf(),
    ));
    Ok(())
}

#[cfg(windows)]
fn rename_open_impl(file: &File, _from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{FileRenameInfo, SetFileInformationByHandle};
    // `from` is always this handle's file: nobody can delete it while it is open without delete sharing.
    let (buffer, size) = rename_info(to);
    // SAFETY: `buffer` holds a FILE_RENAME_INFO of `size` bytes built by `rename_info`, and the handle is valid
    // for the call, which reads at most `size` bytes from it.
    let ok = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileRenameInfo,
            buffer.as_ptr().cast(),
            size as u32,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    file.sync_all()
}

/// Builds a FILE_RENAME_INFO for renaming to `to` without replacing it, and returns it with its size in bytes.
///
/// The name is NUL-terminated even though FileNameLength gives its length (without the terminator): kernel32
/// converts it to an NT path as a NUL-terminated string, and without the terminator it reads past the name.
#[cfg(windows)]
fn rename_info(to: &Path) -> (Vec<u64>, usize) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_RENAME_INFO;
    let name: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    let size = std::mem::offset_of!(FILE_RENAME_INFO, FileName) + name.len() * size_of::<u16>();
    // A u64 buffer gives the alignment FILE_RENAME_INFO needs (it holds a handle).
    let mut buffer = vec![0u64; size.div_ceil(size_of::<u64>())];
    let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    // SAFETY: `buffer` is zeroed, aligned for FILE_RENAME_INFO and at least `size` bytes long, so the header
    // fields and the `name.len()` UTF-16 units (terminator included) written from `FileName` on stay inside it.
    // Zero means ReplaceIfExists = false and no root directory (`to` is an absolute path).
    unsafe {
        (*info).FileNameLength = ((name.len() - 1) * size_of::<u16>()) as u32;
        let target = std::ptr::addr_of_mut!((*info).FileName).cast::<u16>();
        std::ptr::copy_nonoverlapping(name.as_ptr(), target, name.len());
    }
    (buffer, size)
}

#[cfg(all(test, windows))]
mod rename_info_tests {
    use super::rename_info;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use windows_sys::Win32::Storage::FileSystem::FILE_RENAME_INFO;

    /// The rename once read past the target name when no zero followed it (R3-001 of the CI review).
    #[test]
    fn the_target_name_is_nul_terminated_inside_the_buffer() {
        for length in 1..=12 {
            let to = Path::new(r"C:\stores")
                .join("s".repeat(length))
                .join("muninn.data");
            let units: Vec<u16> = to.as_os_str().encode_wide().collect();
            let (buffer, size) = rename_info(&to);
            let bytes: Vec<u8> = buffer.iter().flat_map(|w| w.to_ne_bytes()).collect();
            let length_at = std::mem::offset_of!(FILE_RENAME_INFO, FileNameLength);
            let name_at = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
            let declared =
                u32::from_ne_bytes(bytes[length_at..length_at + 4].try_into().unwrap()) as usize;
            assert_eq!(
                declared,
                units.len() * 2,
                "FileNameLength excludes the terminator"
            );
            assert_eq!(
                size,
                name_at + units.len() * 2 + 2,
                "the size covers the terminator"
            );
            let written: Vec<u16> = bytes[name_at..size]
                .chunks(2)
                .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                .collect();
            assert_eq!(&written[..units.len()], &units[..]);
            assert_eq!(written[units.len()], 0, "the name is followed by a NUL");
            assert!(
                bytes[..4].iter().all(|b| *b == 0),
                "ReplaceIfExists is false"
            );
        }
    }
}

#[cfg(unix)]
fn rename_open_impl(file: &File, from: &Path, to: &Path) -> io::Result<()> {
    // The caller holds the file's lock, so another opener cannot discard it between this check and the rename.
    if !FileIdentity::of(file).is_at(from) {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }
    rename_no_replace_impl(from, to)
}

#[cfg(target_os = "linux")]
fn rename_no_replace_impl(from: &Path, to: &Path) -> io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};
    match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(()),
        // File systems without RENAME_NOREPLACE: a hard link never replaces either.
        Err(e) if e == rustix::io::Errno::INVAL || e == rustix::io::Errno::NOSYS => {
            fs::hard_link(from, to)?;
            fs::remove_file(from)
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn rename_no_replace_impl(from: &Path, to: &Path) -> io::Result<()> {
    fs::hard_link(from, to)?;
    fs::remove_file(from)
}

/// Removes a file.
pub(crate) fn remove(path: &Path) -> io::Result<()> {
    before(Op::Remove, path)?;
    fs::remove_file(path)?;
    #[cfg(any(test, feature = "test-hooks"))]
    hooks::record(faults::Recorded::Remove(path.to_path_buf()));
    Ok(())
}

/// Makes the entries of `dir` (creations and renames) durable. Windows has no directory flush: renames are
/// flushed through the renamed file instead (see [`rename_open_no_replace`]).
pub(crate) fn sync_dir(dir: &Path) -> io::Result<()> {
    before(Op::SyncDir, dir)?;
    #[cfg(any(test, feature = "test-hooks"))]
    let skipped = faults::syncs_skipped();
    #[cfg(not(any(test, feature = "test-hooks")))]
    let skipped = false;
    #[cfg(unix)]
    if !skipped {
        File::open(dir)?.sync_all()?;
    }
    #[cfg(not(unix))]
    let _ = skipped;
    #[cfg(any(test, feature = "test-hooks"))]
    hooks::record(faults::Recorded::SyncDir(dir.to_path_buf()));
    Ok(())
}

/// Allocates `len` zeroed bytes, or reports that the system has no memory for them.
pub(crate) fn alloc_zeroed(len: usize) -> io::Result<Box<[u8]>> {
    before(Op::Alloc, Path::new(""))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(len)
        .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
    bytes.resize(len, 0);
    Ok(bytes.into_boxed_slice())
}

/// The point where registering a snapshot can fail for lack of memory; only tests make it fail.
pub(crate) fn snapshot_point() -> io::Result<()> {
    before(Op::Snapshot, Path::new(""))
}

/// Maps the first `len` bytes of `file` into memory, read-only.
pub(crate) fn map(file: &File, path: &Path, len: u64) -> io::Result<Mmap> {
    before(Op::Map, path)?;
    let len = usize::try_from(len).map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
    // SAFETY: the mapping is read-only and the store's lock (D6, P9) keeps other MuninnDB instances from writing
    // the file. The Pager itself only writes pages through positioned writes while no reference into the
    // mapping is alive (page writes take `&mut self`). Another program modifying the file while the store is
    // open is outside the scope of the PRD (P16).
    unsafe { MmapOptions::new().len(len).map(file) }
}

#[cfg(any(test, feature = "test-hooks"))]
pub(crate) mod faults;
