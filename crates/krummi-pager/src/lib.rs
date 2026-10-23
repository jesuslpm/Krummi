//! The Pager of Krummi (module 1): the paged data file every other module reads and writes through.
//!
//! This crate is the only part of the engine allowed to use unsafe code (D12 of the root PRD). Its API is
//! internal to the engine: applications use the `krummi` crate.

mod error;
pub mod format;
mod journal;
mod pager;
mod recovery;
mod scratch;
mod sys;
mod temp;

pub use error::{Error, ErrorKind, Result};
pub use journal::{JOURNAL_PREFIX, RELEASE_FILE_LIMIT};
pub use pager::{
    DATA_FILE, INITIAL_SIZE, Info, MAX_GROWTH_STEP, MIN_MAX_SIZE, NEW_DATA_FILE, OpenOptions, Pager,
};
pub use scratch::{
    PayloadBuf, Scratch, ScratchWriter, SealedPage, Snapshot, Staged, alloc_payload, alloc_sealed,
};
pub use temp::TEMP_DIR;

/// Fault injection for the tests of the engine crates (feature `test-hooks`). Faults are per thread.
#[cfg(feature = "test-hooks")]
pub mod testing {
    pub use crate::sys::Op;
    pub use crate::sys::faults::{
        Action, Model, Recorded, abort_at, count, inject, materialize, points, recorded_len, reset,
        skip_syncs, start_recording, take_recording,
    };
}
