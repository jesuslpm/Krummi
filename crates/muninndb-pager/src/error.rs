//! Error classes of the engine (D8 of the root PRD).
//!
//! Each variant is one error class; it carries as separate data what the command-line tool needs to build its
//! Spanish message. The `Display` text is English.

use std::fmt;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

/// The class of an [`Error`], for callers that only need to branch on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The folder cannot be used: it is a file, permission is missing, the file system is read-only, or the path
    /// is not valid.
    Access,
    /// The store is already open, by another process or by this one.
    StoreInUse,
    /// The folder does not contain a MuninnDB store.
    NotAStore,
    /// Part of the store is damaged.
    Damaged,
    /// The store uses a different format version.
    DifferentFormat,
    /// There is no space left on the disk.
    DiskFull,
    /// The operating system failed to read or write for a reason other than lack of space.
    Io,
    /// The options passed when opening are not valid.
    InvalidOptions,
    /// Growing the store would exceed the maximum size set when opening it.
    MaxSize,
    /// The system did not provide the memory the store needs.
    OutOfMemory,
    /// A request for a write transaction waited for its whole limit without getting its turn (T4).
    Timeout,
    /// The engine was called in an order or with values it does not accept, such as a second write transaction
    /// on the same thread (T5). Distinct from [`ErrorKind::InvalidOptions`], which is about opening a store.
    Misuse,
}

/// An engine error.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// See [`ErrorKind::Access`].
    Access { folder: PathBuf, source: io::Error },
    /// See [`ErrorKind::StoreInUse`].
    StoreInUse { folder: PathBuf },
    /// See [`ErrorKind::NotAStore`].
    NotAStore { folder: PathBuf },
    /// See [`ErrorKind::Damaged`]. `page` is `None` when what is damaged is what the store needs to open.
    Damaged {
        folder: Option<PathBuf>,
        page: Option<u64>,
    },
    /// See [`ErrorKind::DifferentFormat`].
    DifferentFormat {
        folder: PathBuf,
        found: u32,
        expected: u32,
    },
    /// See [`ErrorKind::DiskFull`].
    DiskFull { folder: PathBuf },
    /// See [`ErrorKind::Io`].
    Io { folder: PathBuf, source: io::Error },
    /// See [`ErrorKind::InvalidOptions`].
    InvalidOptions { reason: &'static str },
    /// See [`ErrorKind::MaxSize`]. Sizes are in bytes.
    MaxSize { max_size: u64, needed: u64 },
    /// See [`ErrorKind::OutOfMemory`]. `requested` is in bytes.
    OutOfMemory { requested: u64 },
    /// See [`ErrorKind::Timeout`]. `folder` is `None` for a temporary store; `limit` is the limit requested.
    Timeout {
        folder: Option<PathBuf>,
        limit: Duration,
    },
    /// See [`ErrorKind::Misuse`]. `folder` is `None` for a temporary store.
    Misuse {
        folder: Option<PathBuf>,
        reason: &'static str,
    },
}

impl Error {
    /// The class of this error.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Access { .. } => ErrorKind::Access,
            Error::StoreInUse { .. } => ErrorKind::StoreInUse,
            Error::NotAStore { .. } => ErrorKind::NotAStore,
            Error::Damaged { .. } => ErrorKind::Damaged,
            Error::DifferentFormat { .. } => ErrorKind::DifferentFormat,
            Error::DiskFull { .. } => ErrorKind::DiskFull,
            Error::Io { .. } => ErrorKind::Io,
            Error::InvalidOptions { .. } => ErrorKind::InvalidOptions,
            Error::MaxSize { .. } => ErrorKind::MaxSize,
            Error::OutOfMemory { .. } => ErrorKind::OutOfMemory,
            Error::Timeout { .. } => ErrorKind::Timeout,
            Error::Misuse { .. } => ErrorKind::Misuse,
        }
    }

    /// Classifies a failed write, size change or flush: lack of space is [`Error::DiskFull`], memory exhaustion
    /// is [`Error::OutOfMemory`], anything else is [`Error::Io`].
    pub(crate) fn from_write(folder: &std::path::Path, source: io::Error, requested: u64) -> Error {
        match source.kind() {
            io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => Error::DiskFull {
                folder: folder.to_path_buf(),
            },
            io::ErrorKind::OutOfMemory => Error::OutOfMemory { requested },
            _ => Error::Io {
                folder: folder.to_path_buf(),
                source,
            },
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Access { folder, source } => {
                write!(f, "cannot use folder {}: {source}", folder.display())
            }
            Error::StoreInUse { folder } => write!(f, "store {} is already open", folder.display()),
            Error::NotAStore { folder } => write!(
                f,
                "folder {} does not contain a MuninnDB store",
                folder.display()
            ),
            Error::Damaged { folder, page } => {
                let store = store_name(folder);
                match page {
                    Some(page) => write!(f, "page {page} of {store} is damaged"),
                    None => write!(f, "{store} is damaged and cannot be opened"),
                }
            }
            Error::DifferentFormat {
                folder,
                found,
                expected,
            } => write!(
                f,
                "store {} has format {found}, but this version of MuninnDB uses format {expected}",
                folder.display()
            ),
            Error::DiskFull { folder } => {
                write!(f, "no space left on disk for store {}", folder.display())
            }
            Error::Io { folder, source } => write!(
                f,
                "input/output error in store {}: {source}",
                folder.display()
            ),
            Error::InvalidOptions { reason } => write!(f, "invalid options: {reason}"),
            Error::MaxSize { max_size, needed } => {
                write!(
                    f,
                    "the store needs {needed} bytes but its maximum size is {max_size} bytes"
                )
            }
            Error::OutOfMemory { requested } => {
                write!(f, "out of memory while reserving {requested} bytes")
            }
            Error::Timeout { folder, limit } => write!(
                f,
                "no write transaction was available on {} within {limit:?}",
                store_name(folder)
            ),
            Error::Misuse { folder, reason } => {
                write!(f, "misuse of {}: {reason}", store_name(folder))
            }
        }
    }
}

/// "store <folder>", or "temporary store" when there is no folder.
fn store_name(folder: &Option<PathBuf>) -> String {
    folder.as_ref().map_or_else(
        || "temporary store".to_owned(),
        |p| format!("store {}", p.display()),
    )
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Access { source, .. } | Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Result type of the engine.
pub type Result<T> = std::result::Result<T, Error>;
