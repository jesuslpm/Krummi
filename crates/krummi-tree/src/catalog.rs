//! The catalog of named trees (D3 of the trees and key operations execution): itself a B+ tree keyed by the UTF-8
//! bytes of the tree names, so it lists them in byte order (B5). Each entry holds the tree's identity, kind, root
//! page and number of entries (A5, A8), and a fixed-size tree's entry also the length of its values (X3).
//!
//! Its root lives in the meta slot [`ROOT_SLOT`], `0` while no tree was ever created: the catalog takes no page
//! until then (A12). The meta slot [`IDENTITIES_SLOT`] counts the identities handed out, so an identity is never
//! reused (A8). Callers only write what really changes, so that a write that changes nothing writes nothing (T10).
//! An entry of an unknown kind or of the wrong length for its kind, or a fixed-size tree's length over
//! [`MAX_LEN`](crate::fixed::MAX_LEN), in a page whose checksum is valid, is damage of that page (G6).

use krummi_tx::{Result, TreeKind, WriteTransaction};

use crate::{PageSource, fixed, node};

/// Meta slot with the catalog's root page, `0` before the first tree.
pub const ROOT_SLOT: usize = 0;
/// Meta slot with the number of tree identities handed out so far.
pub const IDENTITIES_SLOT: usize = 1;

/// Bytes of an encoded entry: identity, kind, root and count.
const ENTRY_SIZE: usize = 8 + 1 + 8 + 8;
/// Bytes of an encoded entry of a fixed-size tree: those of [`ENTRY_SIZE`] and the length of its values, little
/// endian (D5 of the fixed-size trees execution).
const FIXED_ENTRY_SIZE: usize = ENTRY_SIZE + 2;
/// The kind byte of a normal tree.
const NORMAL: u8 = 1;
/// The kind byte of a multi-value tree (D5 of the multi-value trees execution).
const MULTI_VALUE: u8 = 2;
/// The kind byte of a fixed-size tree (D5 of the fixed-size trees execution).
const FIXED_SIZE: u8 = 3;

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
    /// The encoded entry, in the first bytes of the array that its length gives.
    fn encode(&self) -> ([u8; FIXED_ENTRY_SIZE], usize) {
        let mut bytes = [0; FIXED_ENTRY_SIZE];
        bytes[..8].copy_from_slice(&self.id.to_le_bytes());
        bytes[9..17].copy_from_slice(&self.root.to_le_bytes());
        bytes[17..25].copy_from_slice(&self.count.to_le_bytes());
        let (kind, len) = match self.kind {
            TreeKind::Normal => (NORMAL, ENTRY_SIZE),
            TreeKind::MultiValue => (MULTI_VALUE, ENTRY_SIZE),
            TreeKind::FixedSize(len) => {
                bytes[25..].copy_from_slice(&len.to_le_bytes());
                (FIXED_SIZE, FIXED_ENTRY_SIZE)
            }
            // `TreeKind` is non-exhaustive for the applications; the catalog knows every kind of its version.
            kind => unreachable!("the catalog has no {kind} trees"),
        };
        bytes[8] = kind;
        (bytes, len)
    }

    /// Decodes an entry, or `None` if it is not one: of an unknown kind, of another length than its kind's, or of a
    /// fixed-size tree whose length is over [`MAX_LEN`](fixed::MAX_LEN).
    fn decode(bytes: &[u8]) -> Option<Entry> {
        let kind = match (bytes.get(8)?, bytes.len()) {
            (&NORMAL, ENTRY_SIZE) => TreeKind::Normal,
            (&MULTI_VALUE, ENTRY_SIZE) => TreeKind::MultiValue,
            (&FIXED_SIZE, FIXED_ENTRY_SIZE) => {
                let len = u16::from_le_bytes([bytes[25], bytes[26]]);
                if usize::from(len) > fixed::MAX_LEN {
                    return None;
                }
                TreeKind::FixedSize(len)
            }
            _ => return None,
        };
        let field = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"));
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
        Ok(index) => {
            Entry::decode_in(tx, page, crate::resolve(tx, node::value(leaf, index))?).map(Some)
        }
        Err(_) => Ok(None),
    }
}

/// The name and kind of every tree, in byte order of the names: an owned copy (A3), reserved as
/// [`reserve`](crate::reserve) does.
pub fn list<S: PageSource + ?Sized>(tx: &S) -> Result<Vec<(String, TreeKind)>> {
    let root = tx.slot(ROOT_SLOT)?;
    let mut trees = Vec::new();
    if root != 0 {
        crate::leaves(tx, root, None, &mut |page, leaf| {
            for index in 0..node::count(leaf) {
                let entry =
                    Entry::decode_in(tx, page, crate::resolve(tx, node::value(leaf, index))?)?;
                // Names are stored from `str`; a name that is not UTF-8, which only foreign bytes can make, is
                // shown with replacement characters, a copy that stays infallible.
                let name = String::from_utf8(crate::copy(tx, node::key(leaf, index))?)
                    .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
                crate::reserve(tx, &mut trees, 1)?;
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
    let (bytes, len) = entry.encode();
    let update = crate::set(tx, root, name.as_bytes(), &bytes[..len])?;
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
