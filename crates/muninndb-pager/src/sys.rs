//! Operating-system operations of the Pager.
//!
//! Every file operation the Pager performs goes through this module, so tests can inject failures (disk full,
//! I/O errors, out of memory), kill the process at a chosen point, or record the operations to simulate a power
//! cut (see `faults`, compiled only for tests).

use std::fs::{self, File};
use std::io;
use std::path::Path;

use memmap2::{Mmap, MmapOptions};

#[cfg(test)]
pub(crate) use faults::hooks;

/// The operations tests can target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Op {
    CreateFile,
    Write,
    Reserve,
    Sync,
    Rename,
    Remove,
    SyncDir,
    Alloc,
    Map,
}

#[inline]
fn before(op: Op, path: &Path) -> io::Result<()> {
    #[cfg(test)]
    {
        hooks::before(op, path)
    }
    #[cfg(not(test))]
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
    // While the store is prepared, nobody else may delete the file (see `is_sharing_violation`).
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::share_mode(&mut options, 0x1 | 0x2); // FILE_SHARE_READ | WRITE
    let file = options.open(path)?;
    #[cfg(test)]
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

    /// Removes `path` if it is still this file. On Windows another opener's file cannot be deleted while it
    /// prepares it (it is opened without delete sharing), so the removal is attempted directly.
    pub(crate) fn remove_if_same(&self, path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let same = fs::metadata(path).ok().map(|m| (m.dev(), m.ino()));
            if same.is_none() || same != self.0 {
                return;
            }
        }
        let _ = remove(path);
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
    #[cfg(test)]
    hooks::record(faults::Recorded::Write(
        path.to_path_buf(),
        offset,
        buf.to_vec(),
    ));
    Ok(())
}

/// Reads up to `buf.len()` bytes from the start of the file; returns how many were read.
pub(crate) fn read_start(file: &File, buf: &mut [u8]) -> io::Result<usize> {
    let mut done = 0;
    while done < buf.len() {
        #[cfg(unix)]
        let n = std::os::unix::fs::FileExt::read_at(file, &mut buf[done..], done as u64)?;
        #[cfg(windows)]
        let n = std::os::windows::fs::FileExt::seek_read(file, &mut buf[done..], done as u64)?;
        if n == 0 {
            break;
        }
        done += n;
    }
    Ok(done)
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
    #[cfg(test)]
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
    file.sync_all()?;
    #[cfg(test)]
    hooks::record(faults::Recorded::Sync(path.to_path_buf()));
    Ok(())
}

/// Renames `from` to `to`, failing with `AlreadyExists` instead of replacing `to`. On Windows the rename is
/// written through to the disk before returning.
pub(crate) fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    before(Op::Rename, from)?;
    rename_no_replace_impl(from, to)?;
    #[cfg(test)]
    hooks::record(faults::Recorded::Rename(
        from.to_path_buf(),
        to.to_path_buf(),
    ));
    Ok(())
}

#[cfg(windows)]
fn rename_no_replace_impl(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};
    let wide = |p: &Path| {
        p.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<u16>>()
    };
    let (from, to) = (wide(from), wide(to));
    // SAFETY: both arguments are valid, NUL-terminated UTF-16 strings that outlive the call; without
    // MOVEFILE_REPLACE_EXISTING the call fails when `to` exists.
    let ok = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
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

#[cfg(not(any(target_os = "linux", windows)))]
fn rename_no_replace_impl(from: &Path, to: &Path) -> io::Result<()> {
    fs::hard_link(from, to)?;
    fs::remove_file(from)
}

/// Removes a file.
pub(crate) fn remove(path: &Path) -> io::Result<()> {
    before(Op::Remove, path)?;
    fs::remove_file(path)?;
    #[cfg(test)]
    hooks::record(faults::Recorded::Remove(path.to_path_buf()));
    Ok(())
}

/// Makes the entries of `dir` (creations and renames) durable. Windows has no directory flush: renames are
/// written through instead (see [`rename_no_replace`]).
pub(crate) fn sync_dir(dir: &Path) -> io::Result<()> {
    before(Op::SyncDir, dir)?;
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(test)]
    hooks::record(faults::Recorded::SyncDir);
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

#[cfg(test)]
pub(crate) mod faults;
