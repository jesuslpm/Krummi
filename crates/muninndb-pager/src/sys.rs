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

/// Renames the open file `file`, created at `from` by [`create_new`], to `to`, failing with `AlreadyExists`
/// instead of replacing `to`, and with `NotFound` when `from` is no longer `file` (another opener discarded it).
/// The file stays open, so its lock is never released. On Windows the rename goes through the handle and is
/// flushed before returning.
pub(crate) fn rename_open_no_replace(file: &File, from: &Path, to: &Path) -> io::Result<()> {
    before(Op::Rename, from)?;
    rename_open_impl(file, from, to)?;
    #[cfg(test)]
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
    #[cfg(test)]
    hooks::record(faults::Recorded::Remove(path.to_path_buf()));
    Ok(())
}

/// Makes the entries of `dir` (creations and renames) durable. Windows has no directory flush: renames are
/// flushed through the renamed file instead (see [`rename_open_no_replace`]).
pub(crate) fn sync_dir(dir: &Path) -> io::Result<()> {
    before(Op::SyncDir, dir)?;
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(test)]
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
