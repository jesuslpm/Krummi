//! Adapters of the five Rust engines (task T2). The loads of the worker are generic over [`Store`], so every engine
//! is called through the same statically dispatched methods and pays the same cost per call.
//!
//! Every adapter applies exactly the options of [`crate::workload::Engine::changed_options`] and design D8, and
//! every write transaction it commits is durable (K3).

pub mod canopydb;
pub mod fjall;
pub mod krummi;
pub mod lmdb;
pub mod redb;

use std::fmt::Display;
use std::path::Path;

/// A failure, as the line the worker prints on standard error: the engine's error text or the benchmark's cause.
pub type Result<T> = std::result::Result<T, String>;

/// Converts an engine error into its text.
pub fn cause(error: impl Display) -> String {
    error.to_string()
}

/// The tree a store is opened for. A store of the benchmark holds one tree: `bench`, `multi` or `fixed` (D10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tree {
    /// The normal tree `bench`: 16-byte keys.
    Normal,
    /// The multi-value tree `multi` (W19, W20).
    Multi,
    /// The fixed-size tree `fixed`: `u64` keys and 8-byte values (W21, W22).
    Fixed,
}

/// The name of every tree, for the engines that name them.
pub const NORMAL_NAME: &str = "bench";
pub const MULTI_NAME: &str = "multi";
pub const FIXED_NAME: &str = "fixed";
/// Bytes of every value of the fixed-size tree (D10).
pub const FIXED_VALUE_LEN: usize = 8;

/// An open store of one engine. It is shared by the threads of a load, so it must be `Sync`; every thread opens its
/// own transactions.
pub trait Store: Sized + Sync {
    type Reader<'t>: Reader;
    type Writer<'t>: Writer;

    /// Opens the store in the folder `path`, which exists and may be empty, for `tree`.
    fn open(path: &Path, tree: Tree) -> Result<Self>;

    /// Closes the store, and waits until the engine released it.
    fn close(self) -> Result<()>;

    /// Runs `f` in a new read transaction.
    fn read<R>(&self, f: impl FnOnce(&Self::Reader<'_>) -> Result<R>) -> Result<R>;

    /// Runs `f` in a new write transaction and commits it durably, if `f` succeeds.
    fn write<R>(&self, f: impl FnOnce(&mut Self::Writer<'_>) -> Result<R>) -> Result<R>;
}

/// The reads of a read transaction. Reading a value adds all its bytes (K8); the methods return those sums.
pub trait Reader {
    /// The byte sum of the value of `key` in the normal tree, or `None` if the key is not there.
    fn get(&self, key: &[u8]) -> Result<Option<u64>>;

    /// Scans the normal tree forward from `start` included (from its first key if `None`), reading up to `limit`
    /// entries; returns how many it read and the byte sum of their keys and values.
    fn scan(&self, start: Option<&[u8]>, limit: u64) -> Result<(u64, u64)>;

    /// The number of values of `key` in the multi-value tree, and their byte sum.
    fn values(&self, key: &[u8]) -> Result<(u64, u64)>;

    /// The byte sum of the value of `key` in the fixed-size tree, or `None` if the key is not there.
    fn get_fixed(&self, key: u64) -> Result<Option<u64>>;
}

/// The changes of a write transaction.
pub trait Writer {
    /// Stores `value` under `key` in the normal tree.
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<()>;

    /// Removes `key` from the normal tree.
    fn remove(&mut self, key: &[u8]) -> Result<()>;

    /// Adds `value` to the values of `key` in the multi-value tree.
    fn add(&mut self, key: &[u8], value: &[u8]) -> Result<()>;

    /// Stores `value` under `key` in the fixed-size tree.
    fn put_fixed(&mut self, key: u64, value: &[u8]) -> Result<()>;
}

/// The cause of using a structure an engine does not offer natively (K6). The worker refuses those loads before
/// opening a store, so this is never reached.
pub fn unsupported(engine: &str, tree: Tree) -> String {
    format!("{engine} no ofrece el árbol {tree:?}")
}

/// The cause of using a store opened for another tree.
pub fn wrong_tree(tree: Tree) -> String {
    format!("el almacén no está abierto para el árbol {tree:?}")
}

/// Sums the bytes of `bytes` ([`crate::data::byte_sum`]).
pub use crate::data::byte_sum;
