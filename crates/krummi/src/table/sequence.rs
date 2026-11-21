//! Sequences: named catalog objects that hand out increasing signed 64-bit integers (E14 of the tables with schema
//! PRD; N1 of the tables PRD; D2 and D4 of the execution design).
//!
//! A sequence shares the catalog and its names with the trees and the tables: a name is of one class of object only.
//! Its catalog entry, of kind [`TreeKind::Sequence`], holds its identity (A8) and the root of its own one-entry tree,
//! whose value is the next value to hand out, so that opening it, Exists and List read only the catalog and asking for
//! a value reads its own page (E19). A value that is not one, in a page whose checksum is valid, is damage of that
//! page (G6).
//!
//! Asking for a value is a write: only a [`WriteSequence`] has a value, through
//! [`WriteTransaction::next_value`], and a read transaction has no way to ask for one:
//!
//! ```compile_fail
//! # use krummi::{SequenceMode, Store, StoreOptions};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # write.open_sequence_with("ids", SequenceMode::CreateNew(None)).unwrap();
//! # write.commit().unwrap();
//! let read = store.read().unwrap();
//! let ids = read.open_sequence("ids").unwrap();
//! read.next_value(&ids).unwrap();
//! ```
//!
//! nor does a sequence opened in a read get one from a write:
//!
//! ```compile_fail
//! # use krummi::{SequenceMode, Store, StoreOptions};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # write.open_sequence_with("ids", SequenceMode::CreateNew(None)).unwrap();
//! # write.commit().unwrap();
//! let read = store.read().unwrap();
//! let ids = read.open_sequence("ids").unwrap();
//! let mut write = store.write().unwrap();
//! write.next_value(&ids).unwrap();
//! ```
//!
//! Asking for a value may modify the transaction, so it ends the references read before (E16):
//!
//! ```compile_fail
//! # use krummi::{OpenMode, SequenceMode, Store, StoreOptions};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let tree = write.open_tree_with("t", OpenMode::CreateNew).unwrap();
//! # write.set(&tree, b"k", b"v").unwrap();
//! let ids = write.open_sequence_with("ids", SequenceMode::CreateNew(None)).unwrap();
//! let value = write.get(&tree, b"k").unwrap();
//! write.next_value(&ids).unwrap();
//! drop(value);
//! ```
//!
//! and so do the modes that may create a sequence, Delete and Rename:
//!
//! ```compile_fail
//! # use krummi::{OpenMode, SequenceMode, Store, StoreOptions};
//! # let store = Store::open_temporary(&StoreOptions::default()).unwrap();
//! # let mut write = store.write().unwrap();
//! # let tree = write.open_tree_with("t", OpenMode::CreateNew).unwrap();
//! # write.set(&tree, b"k", b"v").unwrap();
//! let value = write.get(&tree, b"k").unwrap();
//! write.open_sequence_with("ids", SequenceMode::OpenOrCreate(None)).unwrap();
//! drop(value);
//! ```
//!
//! while Open keeps them:
//!
//! ```
//! # use krummi::{OpenMode, SequenceMode, Store, StoreOptions};
//! # let store = Store::open_temporary(&StoreOptions::default())?;
//! # let mut write = store.write()?;
//! # let tree = write.open_tree_with("t", OpenMode::CreateNew)?;
//! # write.set(&tree, b"k", b"v")?;
//! # write.open_sequence_with("ids", SequenceMode::CreateNew(Some(1_000)))?;
//! let value = write.get(&tree, b"k")?;
//! let ids = write.open_sequence("ids")?;
//! assert_eq!(value, Some(&b"v"[..]));
//! assert_eq!(write.next_value(&ids)?, 1_000);
//! # Ok::<(), krummi::Error>(())
//! ```

use std::marker::PhantomData;

use krummi_pager::{Error, Result, TreeKind, ValueRule};
use krummi_tree::PageSource;
use krummi_tree::catalog::{self, Entry};

use crate::{Folder, ReadTransaction, Store, WriteTransaction};

/// The key of the one entry of a sequence's tree.
const KEY: &[u8] = &[0];

/// A sequence opened in a read transaction: it proves the sequence is there and of its class, and has no value, since
/// a read cannot ask for one (E14). It can be sent to other threads and cannot outlive its store.
pub struct ReadSequence<'s> {
    _store: PhantomData<&'s Store>,
}

/// A sequence opened in a write transaction, through which [`WriteTransaction::next_value`] asks for its values.
/// Every handle of the sequence in that transaction follows it when it is renamed; it stays on its thread, cannot
/// outlive its store, fails with [`ErrorKind::Misuse`](crate::ErrorKind::Misuse) without effect in another
/// transaction, and with [`ErrorKind::TreeNotFound`](crate::ErrorKind::TreeNotFound) once its sequence was deleted,
/// without reading anything (A6, A7, B9).
pub struct WriteSequence<'s> {
    serial: u64,
    /// The sequence's identity (A8).
    id: u64,
    /// The raw pointer keeps the handle on its thread: it is neither `Send` nor `Sync`.
    _store: PhantomData<(&'s Store, *const ())>,
}

/// How [`WriteTransaction::open_sequence_with`] opens a sequence (E14, the modes of A1 that do not empty);
/// [`WriteTransaction::open_sequence`] only opens. Each mode takes the first value of a sequence it creates, 1 when
/// it is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SequenceMode {
    /// Creates the sequence; fails with [`ErrorKind::TreeAlreadyExists`](crate::ErrorKind) if it exists.
    CreateNew(Option<i64>),
    /// Opens the sequence if it exists, without looking at the first value, otherwise creates it.
    OpenOrCreate(Option<i64>),
}

/// The sequences a write transaction opened, sorted by identity.
#[derive(Default)]
pub(crate) struct Sequences(Vec<SequenceState>);

impl Sequences {
    fn find(&self, id: u64) -> std::result::Result<usize, usize> {
        self.0.binary_search_by_key(&id, |state| state.id)
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut SequenceState> {
        self.find(id).ok().map(|at| &mut self.0[at])
    }
}

/// A sequence as its write transaction sees it. Its catalog entry always has its current root: the rare write that
/// moves the root rewrites the entry at once.
struct SequenceState {
    /// Its identity (A8).
    id: u64,
    /// Its current name or, once deleted, the last one it had (A6).
    name: String,
    /// The root of its one-entry tree.
    root: u64,
    deleted: bool,
}

/// The stored form of the next value, or of no value left after `i64::MAX` was handed out (an empty value).
fn encode(next: Option<i64>) -> ([u8; 8], usize) {
    match next {
        Some(value) => (value.to_le_bytes(), 8),
        None => ([0; 8], 0),
    }
}

/// The next value of a stored form, `Some(None)` when the sequence has none left, `None` when the bytes are not a
/// stored form.
fn decode(bytes: &[u8]) -> Option<Option<i64>> {
    match bytes.len() {
        0 => Some(None),
        8 => Some(Some(i64::from_le_bytes(bytes.try_into().expect("8 bytes")))),
        _ => None,
    }
}

/// The catalog entry of the sequence `name` that `tx` sees, failing with [`Error::DifferentTreeType`] when the name is
/// of another class of object (N1, B3). It reads only the catalog.
fn find_sequence<S: PageSource>(folder: Folder<'_>, tx: &S, name: &str) -> Result<Option<Entry>> {
    match catalog::lookup(tx, name)? {
        Some(entry) if entry.kind != TreeKind::Sequence => {
            Err(folder.different_class(name, entry.kind, TreeKind::Sequence))
        }
        entry => Ok(entry),
    }
}

impl Folder<'_> {
    fn foreign_sequence(self) -> Error {
        Error::Misuse {
            folder: self.owned(),
            reason: "the sequence was opened in another transaction",
        }
    }

    /// The error of asking for a value of the sequence `name` after it handed out `i64::MAX` (E14, E17).
    fn exhausted(self, name: &str) -> Error {
        Error::InvalidValue {
            folder: self.owned(),
            table: None,
            column: None,
            sequence: Some(name.to_owned()),
            rule: ValueRule::SequenceExhausted,
            json: None,
        }
    }
}

impl<'s> ReadTransaction<'s> {
    /// Opens the sequence `name` (E14). Fails with [`Error::InvalidSize`] if the name is empty or longer than 1,024
    /// bytes, with [`Error::DifferentTreeType`] if it is a tree or a table, and with [`Error::TreeNotFound`] if the
    /// snapshot does not have it. It reads only the catalog.
    pub fn open_sequence(&self, name: &str) -> Result<ReadSequence<'s>> {
        self.folder.check_name(name)?;
        find_sequence(self.folder, &self.inner, name)?
            .ok_or_else(|| self.folder.not_found(name))?;
        Ok(ReadSequence {
            _store: PhantomData,
        })
    }
}

/// The operations on sequences of a write transaction, with the rules of its operations on trees and tables: the
/// ones that take `&mut self` may modify it and end the references read before, also when they fail or change
/// nothing (E16). The failures of public validation ([`Error::InvalidSize`], [`Error::DifferentTreeType`],
/// [`Error::TreeNotFound`], [`Error::TreeAlreadyExists`], [`Error::InvalidValue`] of an exhausted sequence and
/// [`Error::Misuse`] for a handle of another transaction) have no effect and do not prevent the commit; any other
/// failure does (T11). The order of E17 holds: names' sizes, a handle of a deleted sequence, the catalog (another
/// class, not found, already exists), the sequence's page, and last an exhausted sequence.
impl<'s> WriteTransaction<'s> {
    /// Opens the sequence `name` (E14), with the failures of [`ReadTransaction::open_sequence`]. It reads only the
    /// catalog.
    pub fn open_sequence(&self, name: &str) -> Result<WriteSequence<'s>> {
        self.folder.check_name(name)?;
        let entry = find_sequence(self.folder, &self.inner, name)?;
        let entry = entry.ok_or_else(|| self.folder.not_found(name))?;
        self.join_sequence(name, &entry)
    }

    /// Opens the sequence `name` in `mode` (E14), with the failures of [`open_sequence`](Self::open_sequence) except
    /// that a missing sequence is created with the mode's first value, 1 when it is `None`; an existing one fails
    /// with [`Error::TreeAlreadyExists`] in [`SequenceMode::CreateNew`] and opens, keeping its next value, in
    /// [`SequenceMode::OpenOrCreate`]. It reads only the catalog.
    pub fn open_sequence_with(
        &mut self,
        name: &str,
        mode: SequenceMode,
    ) -> Result<WriteSequence<'s>> {
        self.folder.check_name(name)?;
        let entry = match (find_sequence(self.folder, &self.inner, name)?, mode) {
            (Some(_), SequenceMode::CreateNew(_)) => return Err(self.folder.already_exists(name)),
            (Some(entry), SequenceMode::OpenOrCreate(_)) => entry,
            (None, SequenceMode::CreateNew(first) | SequenceMode::OpenOrCreate(first)) => {
                self.create_sequence(name, first.unwrap_or(1))?
            }
        };
        self.join_sequence(name, &entry)
    }

    /// Hands out the next value of `sequence`: the first value it was created with, then one more each time (E14).
    /// The value is the transaction's: if it aborts, its values are handed out again, and a value of a committed
    /// transaction is never handed out again. Fails with [`Error::TreeNotFound`] once the sequence was deleted,
    /// without reading anything; with damage of its page (E19, T11); and, after it handed out `i64::MAX`, with
    /// [`Error::InvalidValue`] naming the sequence, without effect, after reading its page (E17).
    pub fn next_value(&mut self, sequence: &WriteSequence<'_>) -> Result<i64> {
        let (name, root) = self.live_sequence(sequence)?;
        let stored = krummi_tree::get(&self.inner, root, KEY)?.and_then(decode);
        let next = match stored {
            Some(next) => next,
            None => {
                let leaf = krummi_tree::leaf_of(&self.inner, root, KEY)?;
                return Err(self.inner.damaged(leaf));
            }
        };
        let Some(value) = next else {
            return Err(self.folder.exhausted(&name));
        };
        let (bytes, len) = encode(value.checked_add(1));
        let update = krummi_tree::set(&mut self.inner, root, KEY, &bytes[..len])?;
        if update.root != root {
            let state = self.sequence_state(sequence.id);
            state.root = update.root;
            let entry = Entry {
                id: sequence.id,
                kind: TreeKind::Sequence,
                root: update.root,
                count: 0,
            };
            catalog::put(&mut self.inner, &name, &entry)?;
        }
        Ok(value)
    }

    /// Deletes the sequence `name` and frees its page (E14, E2); a missing name does nothing. Fails with
    /// [`Error::DifferentTreeType`] if the name is of a tree or a table. The sequence's handles then fail with
    /// [`Error::TreeNotFound`].
    pub fn delete_sequence(&mut self, name: &str) -> Result<()> {
        self.folder.check_name(name)?;
        match find_sequence(self.folder, &self.inner, name)? {
            Some(entry) => self.drop_sequence(name, &entry),
            None => Ok(()),
        }
    }

    /// Renames the sequence `from` to `to`; it keeps its next value, its identity and its handles (E14, E2). Fails
    /// with [`Error::TreeNotFound`] if `from` does not exist, also when `to` is the same name, with
    /// [`Error::DifferentTreeType`] if `from` is a tree or a table, and with [`Error::TreeAlreadyExists`] if `to`
    /// exists, unless `overwrite` is set: then a sequence `to` is deleted first, as
    /// [`delete_sequence`](Self::delete_sequence) does, and a tree or a table fails with
    /// [`Error::DifferentTreeType`] (N1). Renaming a sequence to its own name does nothing.
    pub fn rename_sequence(&mut self, from: &str, to: &str, overwrite: bool) -> Result<()> {
        self.folder.check_name(from)?;
        self.folder.check_name(to)?;
        let source = find_sequence(self.folder, &self.inner, from)?;
        let source = source.ok_or_else(|| self.folder.not_found(from))?;
        if from == to {
            return Ok(());
        }
        let target = catalog::lookup(&self.inner, to)?;
        match target {
            Some(_) if !overwrite => return Err(self.folder.already_exists(to)),
            Some(entry) if entry.kind != TreeKind::Sequence => {
                return Err(self
                    .folder
                    .different_class(to, entry.kind, TreeKind::Sequence));
            }
            _ => {}
        }
        let renamed = krummi_tree::copy_str(&self.inner, to)?;
        self.join_sequence(from, &source)?;
        if let Some(target) = target {
            self.drop_sequence(to, &target)?;
        }
        catalog::put(&mut self.inner, to, &source)?;
        catalog::remove(&mut self.inner, from)?;
        self.sequence_state(source.id).name = renamed;
        Ok(())
    }

    /// A handle of the sequence of `entry`, now named `name`; the sequence's state joins the transaction on its first
    /// opening.
    fn join_sequence(&self, name: &str, entry: &Entry) -> Result<WriteSequence<'s>> {
        let mut sequences = self.sequences.borrow_mut();
        if let Err(at) = sequences.find(entry.id) {
            let name = krummi_tree::copy_str(&self.inner, name)?;
            krummi_tree::reserve(&self.inner, &mut sequences.0, 1)?;
            sequences.0.insert(
                at,
                SequenceState {
                    id: entry.id,
                    name,
                    root: entry.root,
                    deleted: false,
                },
            );
        }
        Ok(WriteSequence {
            serial: self.serial,
            id: entry.id,
            _store: PhantomData,
        })
    }

    /// The state of a sequence that joined the transaction.
    fn sequence_state(&mut self, id: u64) -> &mut SequenceState {
        self.sequences.get_mut().get_mut(id).expect("joined")
    }

    /// The name and the root of the sequence of `sequence`. Fails with misuse for a handle of another transaction and
    /// with not found once the sequence was deleted, without reading anything (A7, E17).
    fn live_sequence(&mut self, sequence: &WriteSequence<'_>) -> Result<(String, u64)> {
        if sequence.serial != self.serial {
            return Err(self.folder.foreign_sequence());
        }
        let state = self.sequence_state(sequence.id);
        if state.deleted {
            let name = state.name.clone();
            return Err(self.folder.not_found(&name));
        }
        let (name, root) = (state.name.clone(), state.root);
        Ok((name, root))
    }

    /// Creates the sequence `name` whose first value is `first`, with a new identity (A8).
    fn create_sequence(&mut self, name: &str, first: i64) -> Result<Entry> {
        let id = catalog::new_identity(&mut self.inner)?;
        let root = krummi_tree::create(&mut self.inner)?;
        let (bytes, len) = encode(Some(first));
        let root = krummi_tree::set(&mut self.inner, root, KEY, &bytes[..len])?.root;
        let entry = Entry {
            id,
            kind: TreeKind::Sequence,
            root,
            count: 0,
        };
        catalog::put(&mut self.inner, name, &entry)?;
        Ok(entry)
    }

    /// Deletes the sequence `name` of `entry`, ending its identity: its page is freed, and its handles fail from now
    /// on (A6, A8).
    fn drop_sequence(&mut self, name: &str, entry: &Entry) -> Result<()> {
        self.join_sequence(name, entry)?;
        krummi_tree::free_all(&mut self.inner, entry.root)?;
        catalog::remove(&mut self.inner, name)?;
        self.sequence_state(entry.id).deleted = true;
        Ok(())
    }
}
