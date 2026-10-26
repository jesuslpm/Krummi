//! B+ trees of Krummi (module 5, D1 and D4 of the trees and key operations execution): ordered maps from keys of
//! 1 to [`MAX_KEY`] bytes to values of 0 to [`MAX_VALUE`] bytes, stored in slotted node pages through the page API
//! of `krummi-tx`. A tree is known by its root page; the layers above keep it (the catalog in a meta slot, each
//! tree in the catalog, see [`catalog`]).
//!
//! A changed node keeps its page number, because the transaction copies a page on its first change (T14), so no
//! path is copied. An insertion that overflows a node splits it by bytes into halves, up to a new root; a removal
//! or an overwrite that leaves a node under a quarter full merges it with a sibling when both fit in one page, and
//! a branch root left with one child is replaced by it. A node left without entries (a leaf, or a branch whose only
//! child went away) is freed and leaves its parent, so no leaf but the root is ever empty. Operations that end up
//! changing nothing (adding a key that is there, removing one that is not) touch no page, so they are not changes
//! for the commit (T10).
//!
//! Every node is checked when the tree code reads it: a page whose checksum is valid but whose layout is not
//! consistent, or a child that is not a data page or not one level below its parent, fails with the damage class
//! and the page number (G6, A9), and in a write it is the transaction's failure (T11). The node accessors then stay
//! within the page without checking again.
//!
//! The memory an operation needs besides pages (the copy of a node that splits or merges, its cells, a separator
//! key, the children of a node freed, the list of trees) is reserved through [`reserve`]: running out of it fails
//! the operation with the out-of-memory class instead of aborting the process (P13), and in a write it is the
//! transaction's failure (T11). Reading an entry reserves nothing.
//!
//! The crate does not validate sizes: the public API does (B7). Its operations panic on a key or a value out of
//! range, a misuse of the engine.

pub mod catalog;
mod node;
mod range;
#[cfg(test)]
mod tests;

use krummi_tx::{Error, PAYLOAD_SIZE, ReadTransaction, Result, WriteTransaction};
use node::{MAX_BRANCH_CELL, MAX_LEAF_CELL, UNDERFULL};
pub use node::{MAX_KEY, MAX_VALUE};
pub use range::{Interval, Range};

/// Read access to pages and meta slots, shared by read and write transactions.
pub trait PageSource {
    /// The payload of page `page_number` as the transaction sees it, without copying it (D11).
    fn page(&self, page_number: u64) -> Result<&[u8]>;

    /// Meta slot `slot` as the transaction sees it.
    fn slot(&self, slot: usize) -> Result<u64>;

    /// Whether page `page_number` may hold a tree's data, checked before following a page number read from a page.
    fn may_hold_data(&self, page_number: u64) -> bool;

    /// The damage error of page `page_number`, inconsistent although its checksum is valid; in a write it is the
    /// transaction's failure (T11).
    fn damaged(&self, page_number: u64) -> Error;

    /// `error`, the failure of an operation of the tree code; in a write it becomes the transaction's failure (T11).
    fn fail(&self, error: Error) -> Error;
}

impl PageSource for ReadTransaction<'_> {
    fn page(&self, page_number: u64) -> Result<&[u8]> {
        ReadTransaction::page(self, page_number)
    }

    fn slot(&self, slot: usize) -> Result<u64> {
        ReadTransaction::slot(self, slot)
    }

    fn may_hold_data(&self, page_number: u64) -> bool {
        ReadTransaction::may_hold_data(self, page_number)
    }

    fn damaged(&self, page_number: u64) -> Error {
        ReadTransaction::damaged(self, page_number)
    }

    fn fail(&self, error: Error) -> Error {
        error
    }
}

impl PageSource for WriteTransaction<'_> {
    fn page(&self, page_number: u64) -> Result<&[u8]> {
        WriteTransaction::page(self, page_number)
    }

    fn slot(&self, slot: usize) -> Result<u64> {
        WriteTransaction::slot(self, slot)
    }

    fn may_hold_data(&self, page_number: u64) -> bool {
        WriteTransaction::may_hold_data(self, page_number)
    }

    fn damaged(&self, page_number: u64) -> Error {
        WriteTransaction::damaged(self, page_number)
    }

    fn fail(&self, error: Error) -> Error {
        WriteTransaction::fail(self, error)
    }
}

/// Reserves room for `additional` more elements in `vec` for an operation in `tx`: running out of memory fails it
/// with the out-of-memory class instead of aborting the process (P13), the transaction's failure in a write (T11).
pub fn reserve<S: PageSource + ?Sized, T>(
    tx: &S,
    vec: &mut Vec<T>,
    additional: usize,
) -> Result<()> {
    krummi_tx::try_reserve(vec, additional).map_err(|error| tx.fail(error))
}

/// A copy of `bytes`, reserved as [`reserve`] does.
pub fn copy<S: PageSource + ?Sized>(tx: &S, bytes: &[u8]) -> Result<Vec<u8>> {
    let mut copy = Vec::new();
    reserve(tx, &mut copy, bytes.len())?;
    copy.extend_from_slice(bytes);
    Ok(copy)
}

/// A copy of `text`, reserved as [`reserve`] does.
pub fn copy_str<S: PageSource + ?Sized>(tx: &S, text: &str) -> Result<String> {
    Ok(String::from_utf8(copy(tx, text.as_bytes())?).expect("a copy of a str is UTF-8"))
}

/// The payload of node `page`, checked: a data page with a consistent layout (see [`node::is_valid`]) and, when
/// `level` is given, at that level. A child is read one level below its parent, so no path loops. Otherwise it
/// fails with the damage class and the number of the page it was asked for, as when its checksum fails (G6).
fn node_at<S: PageSource + ?Sized>(tx: &S, page: u64, level: Option<u8>) -> Result<&[u8]> {
    if !tx.may_hold_data(page) {
        return Err(tx.damaged(page));
    }
    let node = tx.page(page)?;
    if node::is_valid(node) && level.is_none_or(|level| node::level(node) == level) {
        Ok(node)
    } else {
        Err(tx.damaged(page))
    }
}

/// The level of the children of a checked branch.
fn child_level(branch: &[u8]) -> Option<u8> {
    Some(node::level(branch) - 1)
}

/// What an operation that may change a tree did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Update {
    /// The tree's root page afterwards: it changes when the root splits or collapses.
    pub root: u64,
    /// Whether the number of entries changed: [`set`] stored a new key, [`try_add`] added it, [`remove`] removed it.
    pub count_changed: bool,
}

/// Creates an empty tree and returns its root page.
pub fn create(tx: &mut WriteTransaction<'_>) -> Result<u64> {
    let root = tx.allocate()?;
    node::init(tx.page_mut(root)?, 0, 0);
    Ok(root)
}

/// The value of `key` in the tree at `root`, borrowed from the transaction without copying it (D11).
pub fn get<'t, S: PageSource + ?Sized>(
    tx: &'t S,
    root: u64,
    key: &[u8],
) -> Result<Option<&'t [u8]>> {
    let (_, node) = descend(tx, root, key)?;
    Ok(node::search(node, key).ok().map(|i| node::value(node, i)))
}

/// The page of the leaf of the tree at `root` where `key` is or would be (tests use it to damage a known page).
pub fn leaf_of<S: PageSource + ?Sized>(tx: &S, root: u64, key: &[u8]) -> Result<u64> {
    Ok(descend(tx, root, key)?.0)
}

/// The page number and the payload of the leaf where `key` is or would be.
pub(crate) fn descend<'t, S: PageSource + ?Sized>(
    tx: &'t S,
    root: u64,
    key: &[u8],
) -> Result<(u64, &'t [u8])> {
    let (mut page, mut level) = (root, None);
    loop {
        let node = node_at(tx, page, level)?;
        if node::is_leaf(node) {
            return Ok((page, node));
        }
        level = child_level(node);
        page = node::child(node, node::child_index(node, key));
    }
}

/// Calls `visit` with every entry of the tree at `root`, in key order.
pub fn for_each<S: PageSource + ?Sized>(
    tx: &S,
    root: u64,
    visit: &mut impl FnMut(&[u8], &[u8]),
) -> Result<()> {
    leaves(tx, root, None, &mut |_, leaf| {
        for index in 0..node::count(leaf) {
            visit(node::key(leaf, index), node::value(leaf, index));
        }
        Ok(())
    })
}

/// Calls `visit` with the page number and the payload of every leaf of the subtree at `page`, a node of `level`
/// when given, in key order.
pub(crate) fn leaves<'t, S: PageSource + ?Sized>(
    tx: &'t S,
    page: u64,
    level: Option<u8>,
    visit: &mut impl FnMut(u64, &'t [u8]) -> Result<()>,
) -> Result<()> {
    let node = node_at(tx, page, level)?;
    if node::is_leaf(node) {
        return visit(page, node);
    }
    for index in 0..=node::count(node) {
        leaves(tx, node::child(node, index), child_level(node), visit)?;
    }
    Ok(())
}

/// Stores `value` under `key`, creating the entry or overwriting its value.
///
/// # Panics
///
/// If `key` is empty or longer than [`MAX_KEY`], or `value` is longer than [`MAX_VALUE`].
pub fn set(tx: &mut WriteTransaction<'_>, root: u64, key: &[u8], value: &[u8]) -> Result<Update> {
    update(tx, root, key, Op::Set(value))
}

/// Stores `value` under `key` only if the key is not there; otherwise changes nothing.
///
/// # Panics
///
/// As [`set`].
pub fn try_add(
    tx: &mut WriteTransaction<'_>,
    root: u64,
    key: &[u8],
    value: &[u8],
) -> Result<Update> {
    update(tx, root, key, Op::Add(value))
}

/// Removes the entry of `key`, if it is there.
///
/// # Panics
///
/// If `key` is empty or longer than [`MAX_KEY`].
pub fn remove(tx: &mut WriteTransaction<'_>, root: u64, key: &[u8]) -> Result<Update> {
    update(tx, root, key, Op::Remove)
}

/// Frees every page of the tree at `root`, reading only its branch pages and the root (D4).
pub fn free_all(tx: &mut WriteTransaction<'_>, root: u64) -> Result<()> {
    free_subtree(tx, root, None)
}

fn free_subtree(tx: &mut WriteTransaction<'_>, page: u64, level: Option<u8>) -> Result<()> {
    let node = node_at(&*tx, page, level)?;
    let mut children = Vec::new();
    let below = if node::is_leaf(node) {
        None
    } else {
        reserve(&*tx, &mut children, node::count(node) + 1)?;
        children.extend((0..=node::count(node)).map(|i| node::child(node, i)));
        child_level(node)
    };
    // A node is freed before its children, so a child that names the node itself, an ancestor or a page freed
    // before it no longer may hold data and is reported as damage instead of being freed twice.
    tx.free(page, 1)?;
    for child in children {
        if below != Some(0) {
            free_subtree(tx, child, below)?;
        } else if tx.may_hold_data(child) {
            tx.free(child, 1)?;
        } else {
            return Err(tx.damaged(child));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Op<'a> {
    Set(&'a [u8]),
    Add(&'a [u8]),
    Remove,
}

/// What a change left in a node, for its parent.
enum Outcome {
    Done,
    /// The node split: `right` is its new sibling, whose keys start at `key`.
    Split {
        key: Vec<u8>,
        right: u64,
    },
    /// The node is under a quarter full.
    Underfull,
    /// The node has no entries left: a leaf, or a branch whose only child went away. Its parent frees it.
    Empty,
}

fn update(tx: &mut WriteTransaction<'_>, root: u64, key: &[u8], op: Op<'_>) -> Result<Update> {
    assert!(
        (1..=MAX_KEY).contains(&key.len()),
        "a key has 1 to {MAX_KEY} bytes"
    );
    if let Op::Set(value) | Op::Add(value) = op {
        assert!(
            value.len() <= MAX_VALUE,
            "a value has up to {MAX_VALUE} bytes"
        );
    }
    let mut count_changed = false;
    let mut root = root;
    // `change` checked the root, which no other node points to.
    match change(tx, root, None, key, op, &mut count_changed)? {
        Outcome::Done => {}
        Outcome::Split { key, right } => {
            // A checked branch is below level 255.
            let level = node::level(tx.page(root)?) + 1;
            let new_root = tx.allocate()?;
            let mut buf = [0; MAX_BRANCH_CELL];
            let len = node::encode_branch(&mut buf, &key, right);
            node::fill(tx.page_mut(new_root)?, level, root, &[&buf[..len]]);
            root = new_root;
        }
        Outcome::Underfull => {
            let mut level = None;
            loop {
                let node = node_at(&*tx, root, level)?;
                if node::is_leaf(node) || node::count(node) > 0 {
                    break;
                }
                level = child_level(node);
                let child = node::leftmost(node);
                tx.free(root, 1)?;
                root = child;
            }
        }
        // The root keeps its page: an empty leaf stays, and a branch whose only child went away becomes one.
        Outcome::Empty => {
            if !node::is_leaf(tx.page(root)?) {
                node::init(tx.page_mut(root)?, 0, 0);
            }
        }
    }
    Ok(Update {
        root,
        count_changed,
    })
}

/// Applies `op` to the subtree at `page`, a node of `level` when given; `count_changed` reports whether the number
/// of entries changed. It checks `page`, which the functions it calls for it read again without checking.
fn change(
    tx: &mut WriteTransaction<'_>,
    page: u64,
    level: Option<u8>,
    key: &[u8],
    op: Op<'_>,
    count_changed: &mut bool,
) -> Result<Outcome> {
    let node = node_at(&*tx, page, level)?;
    if node::is_leaf(node) {
        return change_leaf(tx, page, key, op, count_changed);
    }
    let index = node::child_index(node, key);
    let child = node::child(node, index);
    match change(tx, child, child_level(node), key, op, count_changed)? {
        Outcome::Done => Ok(Outcome::Done),
        Outcome::Split { key, right } => {
            let mut buf = [0; MAX_BRANCH_CELL];
            let len = node::encode_branch(&mut buf, &key, right);
            insert_cell(tx, page, index, &buf[..len])
        }
        Outcome::Underfull => merge_child(tx, page, index),
        Outcome::Empty => drop_child(tx, page, index),
    }
}

fn change_leaf(
    tx: &mut WriteTransaction<'_>,
    page: u64,
    key: &[u8],
    op: Op<'_>,
    count_changed: &mut bool,
) -> Result<Outcome> {
    let node = tx.page(page)?;
    let found = node::search(node, key);
    let value = match (op, found) {
        (Op::Add(_), Ok(_)) | (Op::Remove, Err(_)) => return Ok(Outcome::Done),
        (Op::Remove, Ok(index)) => {
            *count_changed = true;
            let node = tx.page_mut(page)?;
            node::remove(node, index);
            if node::count(node) == 0 {
                return Ok(Outcome::Empty);
            }
            return Ok(underfull_or_done(node));
        }
        (Op::Set(value) | Op::Add(value), _) => value,
    };
    *count_changed = found.is_err();
    if let Ok(index) = found
        && node::value(node, index).len() == value.len()
    {
        node::value_mut(tx.page_mut(page)?, index).copy_from_slice(value);
        return Ok(Outcome::Done);
    }
    let mut buf = [0; MAX_LEAF_CELL];
    let len = node::encode_leaf(&mut buf, key, value);
    let index = found.unwrap_or_else(|i| i);
    let replaced = found.map_or(0, |i| node::cell(node, i).len() + 2);
    if node::free_space(node) + replaced < len + 2 {
        return split(tx, page, index, found.is_ok(), &buf[..len]);
    }
    let node = tx.page_mut(page)?;
    if found.is_ok() {
        node::remove(node, index);
    }
    node::insert(node, index, &buf[..len]);
    Ok(underfull_or_done(node))
}

fn underfull_or_done(node: &[u8]) -> Outcome {
    if node::used(node) < UNDERFULL {
        Outcome::Underfull
    } else {
        Outcome::Done
    }
}

/// Inserts a branch cell, splitting the branch when it does not fit.
fn insert_cell(
    tx: &mut WriteTransaction<'_>,
    page: u64,
    index: usize,
    cell: &[u8],
) -> Result<Outcome> {
    if node::free_space(tx.page(page)?) < cell.len() + 2 {
        return split(tx, page, index, false, cell);
    }
    node::insert(tx.page_mut(page)?, index, cell);
    Ok(Outcome::Done)
}

/// Splits the node at `page`, with `cell` put at `index` (replacing the cell there when `replace`), into itself
/// and a new right sibling, by bytes into halves.
fn split(
    tx: &mut WriteTransaction<'_>,
    page: u64,
    index: usize,
    replace: bool,
    cell: &[u8],
) -> Result<Outcome> {
    let old = copy(&*tx, tx.page(page)?)?;
    let leaf = node::is_leaf(&old);
    let level = node::level(&old);
    let count = node::count(&old);
    let mut cells: Vec<&[u8]> = Vec::new();
    reserve(&*tx, &mut cells, count + 1)?;
    cells.extend((0..index).map(|i| node::cell(&old, i)));
    cells.push(cell);
    let rest = index + usize::from(replace);
    cells.extend((rest..count).map(|i| node::cell(&old, i)));
    let at = node::split_point(&cells, leaf);
    let (key, right_leftmost, right_cells) = if leaf {
        (node::cell_key(cells[at], true), 0, &cells[at..])
    } else {
        let up = cells[at];
        (
            node::cell_key(up, false),
            node::cell_child(up),
            &cells[at + 1..],
        )
    };
    let key = copy(&*tx, key)?;
    let right = tx.allocate()?;
    node::fill(tx.page_mut(right)?, level, right_leftmost, right_cells);
    node::fill(
        tx.page_mut(page)?,
        level,
        node::leftmost(&old),
        &cells[..at],
    );
    Ok(Outcome::Split { key, right })
}

/// Merges child `index` of the branch at `page`, which is under a quarter full, with a sibling when both fit in
/// one page (D4).
fn merge_child(tx: &mut WriteTransaction<'_>, page: u64, index: usize) -> Result<Outcome> {
    let parent = tx.page(page)?;
    let count = node::count(parent);
    if count == 0 {
        return Ok(Outcome::Underfull);
    }
    // The pair is children `pair` and `pair + 1`, separated by cell `pair`.
    let pair = index.min(count - 1);
    let (left, right) = (node::child(parent, pair), node::child(parent, pair + 1));
    let below = child_level(parent);
    node_at(&*tx, left, below)?;
    let mut separator = [0; MAX_BRANCH_CELL];
    let right_node = node_at(&*tx, right, below)?;
    let leaf = node::is_leaf(right_node);
    let extra = if leaf {
        0
    } else {
        let parent = tx.page(page)?;
        node::encode_branch(
            &mut separator,
            node::key(parent, pair),
            node::leftmost(right_node),
        ) + 2
    };
    let fits =
        node::used(tx.page(left)?) + node::used(right_node) - node::HEADER + extra <= PAYLOAD_SIZE;
    if !fits {
        return Ok(Outcome::Done);
    }
    let moved = copy(&*tx, right_node)?;
    let target = tx.page_mut(left)?;
    if !leaf {
        node::insert(target, node::count(target), &separator[..extra - 2]);
    }
    for i in 0..node::count(&moved) {
        node::insert(target, node::count(target), node::cell(&moved, i));
    }
    tx.free(right, 1)?;
    let parent = tx.page_mut(page)?;
    node::remove(parent, pair);
    Ok(underfull_or_done(parent))
}

/// Frees child `index` of the branch at `page`, which has no entries left, and removes it with its separator; the
/// branch is then empty itself when that was its only child.
fn drop_child(tx: &mut WriteTransaction<'_>, page: u64, index: usize) -> Result<Outcome> {
    let parent = tx.page(page)?;
    let count = node::count(parent);
    tx.free(node::child(parent, index), 1)?;
    if count == 0 {
        return Ok(Outcome::Empty);
    }
    let parent = tx.page_mut(page)?;
    if index == 0 {
        node::set_leftmost(parent, node::child(parent, 1));
        node::remove(parent, 0);
    } else {
        node::remove(parent, index - 1);
    }
    Ok(underfull_or_done(parent))
}
