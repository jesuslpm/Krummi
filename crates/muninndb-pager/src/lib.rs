//! The Pager of MuninnDB (module 1): the paged data file every other module reads and writes through.
//!
//! This crate is the only part of the engine allowed to use unsafe code (D12 of the root PRD). Its API is
//! internal to the engine: applications use the `muninndb` crate.

mod error;
pub mod format;
mod pager;
mod sys;

pub use error::{Error, ErrorKind, Result};
pub use pager::{
    DATA_FILE, INITIAL_SIZE, Info, MAX_GROWTH_STEP, MIN_MAX_SIZE, NEW_DATA_FILE, OpenOptions, Pager,
};
