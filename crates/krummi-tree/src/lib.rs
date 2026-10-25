//! B+ trees of Krummi (module 5, D1 and D4 of the trees and key operations execution): ordered maps from keys of
//! 1 to [`MAX_KEY`] bytes to values of 0 to [`MAX_VALUE`] bytes, stored in slotted node pages through the page API
//! of `krummi-tx`. A tree is known by its root page; the layers above keep it (the catalog in a meta slot, each
//! tree in the catalog).
//!
//! A changed node keeps its page number, because the transaction copies a page on its first change (T14), so no
//! path is copied. An insertion that overflows a node splits it by bytes into halves, up to a new root; a removal
//! or an overwrite that leaves a node under a quarter full merges it with a sibling when both fit in one page, and
//! a branch root left with one child is replaced by it. Operations that end up changing nothing (adding a key that
//! is there, removing one that is not) touch no page, so they are not changes for the commit (T10).
//!
//! The crate does not validate sizes: the public API does (B7). Its operations panic on a key or a value out of
//! range, a misuse of the engine.

mod node;
#[cfg(test)]
mod tests;

use krummi_tx::{PAYLOAD_SIZE, ReadTransaction, Result, WriteTransaction};
use node::{MAX_BRANCH_CELL, MAX_LEAF_CELL, UNDERFULL};
pub use node::{MAX_KEY, MAX_VALUE};

/// Read access to pages, shared by read and write transactions.
pub trait PageSource {
    /// The payload of page `page_number` as the transaction sees it, without copying it (D11).
    fn page(&self, page_number: u64) -> Result<&[u8]>;
}

impl PageSource for ReadTransaction<'_> {
    fn page(&self, page_number: u64) -> Result<&[u8]> {
        ReadTransaction::page(self, page_number)
    }
}

impl PageSource for WriteTransaction<'_> {
    fn page(&self, page_number: u64) -> Result<&[u8]> {
        WriteTransaction::page(self, page_number)
    }
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
    let mut page = root;
    loop {
        let node = tx.page(page)?;
        if node::is_leaf(node) {
            return Ok(node::search(node, key).ok().map(|i| node::value(node, i)));
        }
        page = node::child(node, node::child_index(node, key));
    }
}

/// Calls `visit` with every entry of the tree at `root`, in key order.
pub fn for_each<S: PageSource + ?Sized>(
    tx: &S,
    root: u64,
    visit: &mut impl FnMut(&[u8], &[u8]),
) -> Result<()> {
    let node = tx.page(root)?;
    if node::is_leaf(node) {
        for index in 0..node::count(node) {
            visit(node::key(node, index), node::value(node, index));
        }
        return Ok(());
    }
    for index in 0..=node::count(node) {
        for_each(tx, node::child(node, index), visit)?;
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
    let node = tx.page(root)?;
    if !node::is_leaf(node) {
        let leaves = node::level(node) == 1;
        let children: Vec<u64> = (0..=node::count(node))
            .map(|i| node::child(node, i))
            .collect();
        for child in children {
            if leaves {
                tx.free(child, 1)?;
            } else {
                free_all(tx, child)?;
            }
        }
    }
    tx.free(root, 1)
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
    match change(tx, root, key, op, &mut count_changed)? {
        Outcome::Done => {}
        Outcome::Split { key, right } => {
            let level = node::level(tx.page(root)?)
                .checked_add(1)
                .expect("a tree has fewer than 256 levels");
            let new_root = tx.allocate()?;
            let mut buf = [0; MAX_BRANCH_CELL];
            let len = node::encode_branch(&mut buf, &key, right);
            node::fill(tx.page_mut(new_root)?, level, root, &[&buf[..len]]);
            root = new_root;
        }
        Outcome::Underfull => loop {
            let node = tx.page(root)?;
            if node::is_leaf(node) || node::count(node) > 0 {
                break;
            }
            let child = node::leftmost(node);
            tx.free(root, 1)?;
            root = child;
        },
    }
    Ok(Update {
        root,
        count_changed,
    })
}

/// Applies `op` to the subtree at `page`; `count_changed` reports whether the number of entries changed.
fn change(
    tx: &mut WriteTransaction<'_>,
    page: u64,
    key: &[u8],
    op: Op<'_>,
    count_changed: &mut bool,
) -> Result<Outcome> {
    let node = tx.page(page)?;
    if node::is_leaf(node) {
        return change_leaf(tx, page, key, op, count_changed);
    }
    let index = node::child_index(node, key);
    let child = node::child(node, index);
    match change(tx, child, key, op, count_changed)? {
        Outcome::Done => Ok(Outcome::Done),
        Outcome::Split { key, right } => {
            let mut buf = [0; MAX_BRANCH_CELL];
            let len = node::encode_branch(&mut buf, &key, right);
            insert_cell(tx, page, index, &buf[..len])
        }
        Outcome::Underfull => merge_child(tx, page, index),
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
    let old = tx.page(page)?.to_vec();
    let leaf = node::is_leaf(&old);
    let level = node::level(&old);
    let count = node::count(&old);
    let mut cells: Vec<&[u8]> = Vec::with_capacity(count + 1);
    cells.extend((0..index).map(|i| node::cell(&old, i)));
    cells.push(cell);
    let rest = index + usize::from(replace);
    cells.extend((rest..count).map(|i| node::cell(&old, i)));
    let at = node::split_point(&cells, leaf);
    let right = tx.allocate()?;
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
    node::fill(tx.page_mut(right)?, level, right_leftmost, right_cells);
    node::fill(
        tx.page_mut(page)?,
        level,
        node::leftmost(&old),
        &cells[..at],
    );
    Ok(Outcome::Split {
        key: key.to_vec(),
        right,
    })
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
    let mut separator = [0; MAX_BRANCH_CELL];
    let right_node = tx.page(right)?;
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
    let moved = right_node.to_vec();
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
