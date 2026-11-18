//! The temporary files of the sort that creates an index (IX2 of the secondary indexes PRD): the sorted runs that do
//! not fit in the memory the sort reserves, in the subfolder `krummi.sort` of the store's folder.
//!
//! `krummi.sort` follows the rules of `krummi.temp` (J2, J8 and J14 of the Journal PRD): it is a reserved name (P3), so
//! what it holds belongs to the store; it is created only when the first file is, and [`SortFiles`] removes it when
//! dropped, also when a panic unwinds through the sort, leaving it behind only when it cannot be removed; the close
//! of the store removes one left so, and the next open removes one a crash left, in its step 8, also when it is a
//! file (`recovery`). Its files never count for the maximum size (P10) nor in the store's information (P12), and are
//! never read for recovery. A temporary store has no folder and so no files: its sort stays in memory (G9).
//!
//! A failure creating a file or the folder is the access class for a missing permission and otherwise the class of
//! a failed write (disk full, out of memory or I/O); one writing a file, the same, and an I/O failure then makes the
//! store refuse writes until it is reopened (T12, as for the temporary area, J7); one reading a file is the access
//! class for a missing permission, out of memory, or I/O. A run knows how many bytes were written into its file, and is
//! read up to exactly that many: a file that ends before them, a read that would pass them, and a record whose header
//! names more bytes than the run has left ([`SortReader::malformed`]) are the I/O class, so that a short or damaged file
//! never reads as a run that ended early.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::pager::Pager;
use crate::sys;

/// Name of the subfolder of the sort's files in the store's folder (IX2 of the secondary indexes PRD).
pub const SORT_DIR: &str = "krummi.sort";

/// Bytes a file of the sort is written and read in.
const BUFFER: usize = 64 << 10;

/// The files of one sort, in `krummi.sort` of a store on disk. Dropping it removes the folder, as best effort, when
/// it created it.
pub struct SortFiles<'p> {
    pager: &'p Pager,
    dir: PathBuf,
    /// Files created; they name the next one.
    files: u64,
    /// Whether this sort created the folder, which it then removes.
    created: bool,
}

impl<'p> SortFiles<'p> {
    /// The files of a sort of the store `pager`, or `None` for a temporary store, which sorts in memory only (G9). It
    /// creates nothing until the first file.
    pub fn new(pager: &'p Pager) -> Option<SortFiles<'p>> {
        let dir = pager.folder()?.join(SORT_DIR);
        Some(SortFiles {
            pager,
            dir,
            files: 0,
            created: false,
        })
    }

    /// Creates a new file to write a run into, creating `krummi.sort` before the first one, after removing one an
    /// earlier sort could not remove.
    pub fn create(&mut self) -> Result<SortWriter<'p>> {
        if !self.created {
            sys::remove_sort(&self.dir).map_err(|e| self.create_error(e))?;
            sys::create_sort_dir(&self.dir).map_err(|e| self.create_error(e))?;
            self.created = true;
        }
        self.files += 1;
        let path = self.dir.join(format!("run.{:010}", self.files));
        let file = sys::create_sort_file(&path).map_err(|e| self.create_error(e))?;
        Ok(SortWriter {
            pager: self.pager,
            file,
            path,
            buf: buffer()?,
            len: 0,
        })
    }

    /// Opens the file of `run` to read it from its start.
    pub fn open(&self, run: &SortRun) -> Result<SortReader> {
        let file = sys::open_sort(&run.path).map_err(|e| read_error(self.pager, e))?;
        Ok(SortReader {
            file,
            path: run.path.clone(),
            folder: self
                .pager
                .folder()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            buf: buffer()?,
            at: 0,
            left: run.len,
        })
    }

    /// Removes the file of `run`, which the sort no longer needs, as best effort: one left behind goes with the folder.
    pub fn remove(&self, run: SortRun) {
        let _ = std::fs::remove_file(run.path);
    }

    fn create_error(&self, source: io::Error) -> Error {
        match access(self.pager, source) {
            Ok(error) => error,
            Err(source) => Error::from_write(folder(self.pager), source, BUFFER as u64),
        }
    }
}

impl Drop for SortFiles<'_> {
    fn drop(&mut self) {
        if self.created {
            let _ = sys::remove_sort(&self.dir);
        }
    }
}

/// A file of the sort being written.
pub struct SortWriter<'p> {
    pager: &'p Pager,
    file: File,
    path: PathBuf,
    buf: Vec<u8>,
    /// Bytes appended so far, buffered or written.
    len: u64,
}

impl SortWriter<'_> {
    /// Appends `bytes` to the file.
    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        if self.buf.len() + bytes.len() > self.buf.capacity() {
            self.flush()?;
        }
        if bytes.len() > self.buf.capacity() {
            write_out(self.pager, &mut self.file, &self.path, bytes)?;
        } else {
            self.buf.extend_from_slice(bytes);
        }
        self.len += bytes.len() as u64;
        Ok(())
    }

    /// Writes what is still buffered and closes the file, whose run can then be read.
    pub fn finish(mut self) -> Result<SortRun> {
        self.flush()?;
        Ok(SortRun {
            path: std::mem::take(&mut self.path),
            len: self.len,
        })
    }

    fn flush(&mut self) -> Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let written = write_out(self.pager, &mut self.file, &self.path, &self.buf);
        self.buf.clear();
        written
    }
}

/// Writes `bytes` at the end of `file`, a file of the sort at `path`, classifying a failure (T12 for I/O).
fn write_out(pager: &Pager, file: &mut File, path: &Path, bytes: &[u8]) -> Result<()> {
    sys::write_sort(file, path, bytes)
        .map_err(|e| access(pager, e).unwrap_or_else(|e| pager.fail_write(e, bytes.len() as u64)))
}

/// A file of the sort that was written whole, with the number of bytes written into it.
pub struct SortRun {
    path: PathBuf,
    len: u64,
}

/// A file of the sort being read from its start.
pub struct SortReader {
    file: File,
    path: PathBuf,
    folder: PathBuf,
    /// What was read from the file and not taken yet: `buf[at..]`.
    buf: Vec<u8>,
    at: usize,
    /// Bytes of the run not taken yet.
    left: u64,
}

impl SortReader {
    /// Fills `out` with the next bytes of the run and returns `true`, or returns `false` when the run ended before
    /// them. A run that ends inside them, or a file that ends before its run, fails with the I/O class.
    pub fn read(&mut self, out: &mut [u8]) -> Result<bool> {
        if self.left == 0 && !out.is_empty() {
            return Ok(false);
        }
        if out.len() as u64 > self.left {
            return Err(self.malformed());
        }
        let mut filled = 0;
        while filled < out.len() {
            if self.at == self.buf.len() {
                self.buf.resize(self.buf.capacity(), 0);
                let read = sys::read_sort(&mut self.file, &self.path, &mut self.buf);
                let read = read.map_err(|e| self.read_error(e))?;
                self.buf.truncate(read);
                self.at = 0;
                if read == 0 {
                    return Err(self.read_error(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "a file of the sort ended before its run",
                    )));
                }
            }
            let take = (out.len() - filled).min(self.buf.len() - self.at);
            out[filled..filled + take].copy_from_slice(&self.buf[self.at..self.at + take]);
            filled += take;
            self.at += take;
        }
        self.left -= out.len() as u64;
        Ok(true)
    }

    /// Fills `out` with the next bytes of the run, which must have them: a run that ends before fails with the I/O
    /// class.
    pub fn read_rest(&mut self, out: &mut [u8]) -> Result<()> {
        match self.read(out)? {
            true => Ok(()),
            false => Err(self.malformed()),
        }
    }

    /// Bytes of the run not read yet.
    pub fn left(&self) -> u64 {
        self.left
    }

    /// The I/O class for a run whose content does not read as written: a record that passes the run's end, or whose
    /// header names parts that do not fit together.
    pub fn malformed(&self) -> Error {
        self.read_error(io::Error::new(
            io::ErrorKind::InvalidData,
            "a run of the sort is malformed",
        ))
    }

    fn read_error(&self, source: io::Error) -> Error {
        classify_read(&self.folder, source)
    }
}

/// The buffer a file of the sort is written or read through; the system refusing its memory is the out-of-memory
/// class (P13).
fn buffer() -> Result<Vec<u8>> {
    let out_of_memory = |_| Error::OutOfMemory {
        requested: BUFFER as u64,
    };
    sys::alloc_point().map_err(out_of_memory)?;
    let mut buf = Vec::new();
    buf.try_reserve_exact(BUFFER)
        .map_err(|_| Error::OutOfMemory {
            requested: BUFFER as u64,
        })?;
    Ok(buf)
}

fn folder(pager: &Pager) -> &Path {
    pager.folder().unwrap_or(Path::new(""))
}

/// The access class for a missing permission, or `source` back.
fn access(pager: &Pager, source: io::Error) -> std::result::Result<Error, io::Error> {
    match source.kind() {
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => Ok(Error::Access {
            folder: folder(pager).to_path_buf(),
            source,
        }),
        _ => Err(source),
    }
}

fn read_error(pager: &Pager, source: io::Error) -> Error {
    classify_read(folder(pager), source)
}

/// Classifies a failed read of a file of the sort: access for a missing permission, out of memory, or I/O.
fn classify_read(folder: &Path, source: io::Error) -> Error {
    match source.kind() {
        io::ErrorKind::PermissionDenied => Error::Access {
            folder: folder.to_path_buf(),
            source,
        },
        io::ErrorKind::OutOfMemory => Error::OutOfMemory {
            requested: BUFFER as u64,
        },
        _ => Error::Io {
            folder: folder.to_path_buf(),
            source,
        },
    }
}
