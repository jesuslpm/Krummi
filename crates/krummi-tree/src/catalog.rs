//! The catalog of named trees (D3 of the trees and key operations execution): itself a B+ tree keyed by the UTF-8
//! bytes of the tree names, so it lists them in byte order (B5). Each entry holds the tree's identity, kind, root
//! page and number of entries (A5, A8).
//!
//! Its root lives in the meta slot [`ROOT_SLOT`], `0` while no tree was ever created: the catalog takes no page
//! until then (A12). The meta slot [`IDENTITIES_SLOT`] counts the identities handed out, so an identity is never
//! reused (A8). Callers only write what really changes, so that a write that changes nothing writes nothing (T10).
//! An entry of an unknown kind or of the wrong length, in a page whose checksum is valid, is damage of that page
//! (G6).

use krummi_tx::{Result, TreeKind, WriteTransaction};

use crate::{PageSource, node};

/// Meta slot with the catalog's root page, `0` before the first tree.
pub const ROOT_SLOT: usize = 0;
/// Meta slot with the number of tree identities handed out so far.
pub const IDENTITIES_SLOT: usize = 1;

/// Bytes of an encoded entry: identity, kind, root and count.
const ENTRY_SIZE: usize = 8 + 1 + 8 + 8;
/// The kind byte of a normal tree.
const NORMAL: u8 = 1;

/// The catalog's entry of a tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// The tree's identity: distinct from every other tree's, before and after (A8).
    pub id: u64,
    /// The tree's kind (B3).
    pub kind: TreeKind,
    /// The tree's root page.
    pub root: u64,
    /// The tree's number of entries (A5).
    pub count: u64,
}

impl Entry {
    fn encode(&self) -> [u8; ENTRY_SIZE] {
        let mut bytes = [0; ENTRY_SIZE];
        bytes[..8].copy_from_slice(&self.id.to_le_bytes());
        bytes[8] = match self.kind {
            TreeKind::Normal => NORMAL,
            // `TreeKind` is non-exhaustive, but its only kind is the normal one until module 5.4 adds the others.
            kind => unreachable!("module 5.1 has no {kind} trees"),
        };
        bytes[9..17].copy_from_slice(&self.root.to_le_bytes());
        bytes[17..].copy_from_slice(&self.count.to_le_bytes());
        bytes
    }

    /// Decodes an entry, or `None` if it is not one: of another length or of an unknown kind.
    fn decode(bytes: &[u8]) -> Option<Entry> {
        if bytes.len() != ENTRY_SIZE {
            return None;
        }
        let field = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"));
        let kind = match bytes[8] {
            NORMAL => TreeKind::Normal,
            _ => return None,
        };
        Some(Entry {
            id: field(0),
            kind,
            root: field(9),
            count: field(17),
        })
    }

    /// Decodes an entry of the catalog's leaf `page`, failing with damage of that page if it is not one.
    fn decode_in<S: PageSource + ?Sized>(tx: &S, page: u64, bytes: &[u8]) -> Result<Entry> {
        Entry::decode(bytes).ok_or_else(|| tx.damaged(page))
    }
}

/// The entry of the tree `name`, or `None` if there is no such tree. It reads only the catalog.
pub fn lookup<S: PageSource + ?Sized>(tx: &S, name: &str) -> Result<Option<Entry>> {
    let root = tx.slot(ROOT_SLOT)?;
    if root == 0 {
        return Ok(None);
    }
    let (page, leaf) = crate::descend(tx, root, name.as_bytes())?;
    match node::search(leaf, name.as_bytes()) {
        Ok(index) => Entry::decode_in(tx, page, node::value(leaf, index)).map(Some),
        Err(_) => Ok(None),
    }
}

/// The name and kind of every tree, in byte order of the names: an owned copy (A3).
pub fn list<S: PageSource + ?Sized>(tx: &S) -> Result<Vec<(String, TreeKind)>> {
    let root = tx.slot(ROOT_SLOT)?;
    let mut trees = Vec::new();
    if root != 0 {
        crate::leaves(tx, root, None, &mut |page, leaf| {
            for index in 0..node::count(leaf) {
                let name = String::from_utf8_lossy(node::key(leaf, index)).into_owned();
                let entry = Entry::decode_in(tx, page, node::value(leaf, index))?;
                trees.push((name, entry.kind));
            }
            Ok(())
        })?;
    }
    Ok(trees)
}

/// Stores `entry` under `name`, creating the catalog with the first tree (A12) or replacing the entry there.
pub fn put(tx: &mut WriteTransaction<'_>, name: &str, entry: &Entry) -> Result<()> {
    let mut root = tx.slot(ROOT_SLOT)?;
    if root == 0 {
        root = crate::create(tx)?;
    }
    let update = crate::set(tx, root, name.as_bytes(), &entry.encode())?;
    tx.set_slot(ROOT_SLOT, update.root)
}

/// Removes the entry of `name`, which must be there. The catalog keeps its root page when it empties.
pub fn remove(tx: &mut WriteTransaction<'_>, name: &str) -> Result<()> {
    let root = tx.slot(ROOT_SLOT)?;
    let update = crate::remove(tx, root, name.as_bytes())?;
    debug_assert!(update.count_changed, "the entry of {name:?} was there");
    tx.set_slot(ROOT_SLOT, update.root)
}

/// Hands out a new tree identity, never handed out before (A8).
pub fn new_identity(tx: &mut WriteTransaction<'_>) -> Result<u64> {
    let id = tx.slot(IDENTITIES_SLOT)?;
    let next = id
        .checked_add(1)
        .expect("fewer than 2^64 trees are ever created");
    tx.set_slot(IDENTITIES_SLOT, next)?;
    Ok(id)
}
