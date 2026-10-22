//! The Pager of MuninnDB (module 1): the paged data file every other module reads and writes through.
//!
//! This crate is the only part of the engine allowed to use unsafe code (D12 of the root PRD). Its API is
//! internal to the engine: applications use the `muninndb` crate.

mod error;
pub mod format;
mod pager;
mod scratch;
mod sys;

pub use error::{Error, ErrorKind, Result};
pub use pager::{
    DATA_FILE, INITIAL_SIZE, Info, MAX_GROWTH_STEP, MIN_MAX_SIZE, NEW_DATA_FILE, OpenOptions, Pager,
};
pub use scratch::{
    PayloadBuf, Scratch, ScratchWriter, SealedPage, Snapshot, alloc_payload, alloc_sealed,
};

/// Fault injection for the tests of the engine crates (feature `test-hooks`). Faults are per thread.
#[cfg(feature = "test-hooks")]
pub mod testing {
    pub use crate::sys::Op;
    pub use crate::sys::faults::{Action, inject, reset};
}
