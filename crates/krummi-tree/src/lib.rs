//! B+ trees of Krummi (module 5, D1 and D4 of the trees and key operations execution): ordered maps from keys of
//! 1 to [`MAX_KEY`] bytes to values of 0 to [`MAX_VALUE`] bytes, stored in slotted node pages through the page API
//! of `krummi-tx`. A tree is known by its root page; the layers above keep it (the catalog in a meta slot, each
//! tree in the catalog, see [`catalog`]).
//!
//! A value whose entry would not fit twice in a leaf lives off its leaf, in a value run of contiguous pages that the
//! transaction reserves for it (D2, D6 and D7 of the large values execution). Runs are never changed: overwriting
//! such a value stores a new run and frees the old one, in the same write. Reading returns a run's value as one
//! sequence without copying it (V2). Freeing a value reads its leaf, never its run (V7), so a damaged value can
//! always be removed, replaced, or dropped with its tree.
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
pub mod multi;
#[cfg(test)]
mod multi_tests;
mod node;
mod range;
#[cfg(test)]
mod tests;

use std::ops::Bound;

use krummi_tx::{Error, PAYLOAD_SIZE, ReadTransaction, Result, WriteTransaction, run_pages};
use node::{MAX_BRANCH_CELL, MAX_LEAF_CELL, UNDERFULL, Value};
pub use node::{MAX_KEY, MAX_VALUE};
pub use range::{Interval, Range};

/// Read access to pages and meta slots, shared by read and write transactions.
pub trait PageSource {
    /// The payload of page `page_number` as the transaction sees it, without copying it (D11).
    fn page(&self, page_number: u64) -> Result<&[u8]>;

    /// The payload of page `page_number`, as [`page`](Self::page), with the integrity code its read has just
    /// verified, or `None` when no read verifies it, as for a version of the scratch area (C2, C3 of the remembered
    /// node layout check PRD).
    fn page_and_code(&self, page_number: u64) -> Result<(&[u8], Option<u64>)> {
        Ok((self.page(page_number)?, None))
    }

    /// Whether the content of page `page_number` with integrity code `code` already passed the layout check while the
    /// store is open (C1, C2).
    fn is_checked(&self, page_number: u64, code: u64) -> bool {
        let _ = (page_number, code);
        false
    }

    /// Remembers that the content of page `page_number` with integrity code `code` passed the layout check (C1, C2).
    fn remember_checked(&self, page_number: u64, code: u64) {
        let _ = (page_number, code);
    }

    /// The content of the value run of `count` pages from page `first` as the transaction sees it, after the run's
    /// header, as one sequence without copying it (V2).
    fn run(&self, first: u64, count: u64) -> Result<&[u8]>;

    /// Meta slot `slot` as the transaction sees it.
    fn slot(&self, slot: usize) -> Result<u64>;

    /// Whether page `page_number` may hold a tree's data, checked before following a page number read from a page.
    fn may_hold_data(&self, page_number: u64) -> bool;

    /// Whether the value run of `count` pages from page `first` may hold a tree's data, checked before reading or
    /// freeing a run named in a leaf.
    fn may_hold_run(&self, first: u64, count: u64) -> bool;

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

    fn page_and_code(&self, page_number: u64) -> Result<(&[u8], Option<u64>)> {
        ReadTransaction::page_and_code(self, page_number)
    }

    fn is_checked(&self, page_number: u64, code: u64) -> bool {
        ReadTransaction::is_checked(self, page_number, code)
    }

    fn remember_checked(&self, page_number: u64, code: u64) {
        ReadTransaction::remember_checked(self, page_number, code);
    }

    fn run(&self, first: u64, count: u64) -> Result<&[u8]> {
        ReadTransaction::run(self, first, count)
    }

    fn slot(&self, slot: usize) -> Result<u64> {
        ReadTransaction::slot(self, slot)
    }

    fn may_hold_data(&self, page_number: u64) -> bool {
        ReadTransaction::may_hold_data(self, page_number)
    }

    fn may_hold_run(&self, first: u64, count: u64) -> bool {
        ReadTransaction::may_hold_run(self, first, count)
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

    fn page_and_code(&self, page_number: u64) -> Result<(&[u8], Option<u64>)> {
        WriteTransaction::page_and_code(self, page_number)
    }

    fn is_checked(&self, page_number: u64, code: u64) -> bool {
        WriteTransaction::is_checked(self, page_number, code)
    }

    fn remember_checked(&self, page_number: u64, code: u64) {
        WriteTransaction::remember_checked(self, page_number, code);
    }

    fn run(&self, first: u64, count: u64) -> Result<&[u8]> {
        WriteTransaction::run(self, first, count)
    }

    fn slot(&self, slot: usize) -> Result<u64> {
        WriteTransaction::slot(self, slot)
    }

    fn may_hold_data(&self, page_number: u64) -> bool {
        WriteTransaction::may_hold_data(self, page_number)
    }

    fn may_hold_run(&self, first: u64, count: u64) -> bool {
        WriteTransaction::may_hold_run(self, first, count)
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
///
/// The layout of a content whose integrity code its read has just verified is checked once while the store is open:
/// a content that passes is remembered by its page number and code, and a later read of the same content does not
/// check it again (C1–C3 of the remembered node layout check PRD). Every other check runs on every read.
fn node_at<S: PageSource + ?Sized>(tx: &S, page: u64, level: Option<u8>) -> Result<&[u8]> {
    if !tx.may_hold_data(page) {
        return Err(tx.damaged(page));
    }
    let (node, code) = tx.page_and_code(page)?;
    let layout = match code {
        Some(code) if tx.is_checked(page, code) => true,
        Some(code) => {
            let valid = node::is_valid(node);
            if valid {
                tx.remember_checked(page, code);
            }
            valid
        }
        None => node::is_valid(node),
    };
    if layout && level.is_none_or(|level| node::level(node) == level) {
        Ok(node)
    } else {
        Err(tx.damaged(page))
    }
}

/// The level of the children of a checked branch.
fn child_level(branch: &[u8]) -> Option<u8> {
    Some(node::level(branch) - 1)
}

/// The bytes of a leaf entry's value: in its leaf, or its run's, without copying them (V2). A run named in a leaf
/// that cannot be one, or whose pages are damaged, fails with the damage class and the run's first page (G6, V7).
pub(crate) fn resolve<'t, S: PageSource + ?Sized>(tx: &'t S, value: Value<'t>) -> Result<&'t [u8]> {
    match value {
        Value::Inline(bytes) => Ok(bytes),
        Value::Run { first, len } => {
            let count = run_pages(len);
            if !tx.may_hold_run(first, count) {
                return Err(tx.damaged(first));
            }
            // A checked leaf names values of up to `MAX_VALUE` bytes, which their run holds.
            Ok(&tx.run(first, count)?[..len as usize])
        }
    }
}

/// Frees the run of a value off its leaf, from page `first` with `len` bytes, without reading it (V7).
fn free_run(tx: &mut WriteTransaction<'_>, first: u64, len: u64) -> Result<()> {
    let count = run_pages(len);
    if !tx.may_hold_run(first, count) {
        return Err(tx.damaged(first));
    }
    tx.free(first, count)
}

/// Frees the runs of the values off their leaf among the entries `indexes` of the checked leaf at `page`. Freeing
/// a run needs the transaction mutably, so the leaf is looked up again after each run it frees; a leaf without
/// values off it is looked up once.
fn free_runs(
    tx: &mut WriteTransaction<'_>,
    page: u64,
    indexes: std::ops::Range<usize>,
) -> Result<()> {
    let end = indexes.end;
    let mut next = node::next_run(tx.page(page)?, indexes);
    while let Some((index, first, len)) = next {
        free_run(tx, first, len)?;
        next = node::next_run(tx.page(page)?, index + 1..end);
    }
    Ok(())
}

/// Writes the leaf cell of `key` and `value` into `buf` and returns its length: with the value, when the cell fits
/// under the leaf's threshold, or else with the place of a new value run that holds it (D2, D7).
fn encode_entry(
    tx: &mut WriteTransaction<'_>,
    buf: &mut [u8],
    key: &[u8],
    value: &[u8],
) -> Result<usize> {
    if node::fits_inline(key.len(), value.len()) {
        return Ok(node::encode_leaf(buf, key, Value::Inline(value)));
    }
    let len = value.len() as u64;
    let (first, run) = tx.allocate_value_run(run_pages(len))?;
    run[..value.len()].copy_from_slice(value);
    Ok(node::encode_leaf(buf, key, Value::Run { first, len }))
}

/// A test hook, not part of the engine's interface: the longest value that an entry with a key of `key_len` bytes
/// keeps in its leaf, the leaf's threshold (D7); a longer one lives off its leaf (tests take the threshold from it).
#[doc(hidden)]
pub fn max_inline_value(key_len: usize) -> usize {
    MAX_LEAF_CELL.saturating_sub(node::LEAF_CELL_HEADER + key_len)
}

/// A test hook, not part of the engine's interface: the value run of `key` in the tree at `root`, its first page
/// and its number of pages, or `None` when the key is not there or its value is in its leaf (tests use it to find a
/// value's pages).
#[doc(hidden)]
pub fn run_of<S: PageSource + ?Sized>(tx: &S, root: u64, key: &[u8]) -> Result<Option<(u64, u64)>> {
    let (_, node) = descend(tx, root, key)?;
    Ok(
        match node::search(node, key).map(|i| node::value(node, i)) {
            Ok(Value::Run { first, len }) => Some((first, run_pages(len))),
            _ => None,
        },
    )
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
    match node::search(node, key) {
        Ok(index) => resolve(tx, node::value(node, index)).map(Some),
        Err(_) => Ok(None),
    }
}

/// The page of the leaf of the tree at `root` where `key` is or would be (tests use it to damage a known page).
pub fn leaf_of<S: PageSource + ?Sized>(tx: &S, root: u64, key: &[u8]) -> Result<u64> {
    Ok(descend(tx, root, key)?.0)
}

/// The number of levels of the tree at `root`, one for a lone leaf (tests use it to see a tree gain and lose
/// levels).
pub fn height<S: PageSource + ?Sized>(tx: &S, root: u64) -> Result<u8> {
    Ok(node::level(node_at(tx, root, None)?) + 1)
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
            visit(
                node::key(leaf, index),
                resolve(tx, node::value(leaf, index))?,
            );
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

/// Frees every page of the tree at `root` and the runs of its values, reading its nodes but never its runs (V7).
pub fn free_all(tx: &mut WriteTransaction<'_>, root: u64) -> Result<()> {
    free_subtree(tx, root, None).map(|_| ())
}

/// Frees every page of the subtree at `page`, a node of `level` when given, and the runs of its values, and returns
/// how many entries it had. It reads every node of the subtree, the leaves to find their runs, but no run (V7).
fn free_subtree(tx: &mut WriteTransaction<'_>, page: u64, level: Option<u8>) -> Result<u64> {
    let node = node_at(&*tx, page, level)?;
    if node::is_leaf(node) {
        let entries = node::count(node);
        free_runs(tx, page, 0..entries)?;
        tx.free(page, 1)?;
        return Ok(entries as u64);
    }
    let mut children = Vec::new();
    reserve(&*tx, &mut children, node::count(node) + 1)?;
    children.extend((0..=node::count(node)).map(|i| node::child(node, i)));
    let below = child_level(node);
    // A node is freed before its children, so a child that names the node itself, an ancestor or a page freed
    // before it no longer may hold data and is reported as damage instead of being freed twice.
    tx.free(page, 1)?;
    let mut entries = 0;
    for child in children {
        entries += free_subtree(tx, child, below)?;
    }
    Ok(entries)
}

/// What a range removal did to a tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeUpdate {
    /// The tree's root page afterwards: it changes when the root collapses.
    pub root: u64,
    /// How many entries it removed.
    pub removed: u64,
}

/// Removes every entry of `interval` from the tree at `root` (D4): a leaf loses its run of entries in the
/// interval, a branch frees the children wholly inside it, reading their leaves to count their entries, and the
/// children that hold the bounds are fixed like after a key removal. An empty interval (R3) reads nothing, and a
/// removal that finds no entry changes no page (T10).
///
/// # Panics
///
/// If a bound's key is empty or longer than [`MAX_KEY`], or the prefix is longer than [`MAX_KEY`].
pub fn remove_range(
    tx: &mut WriteTransaction<'_>,
    root: u64,
    interval: Interval<'_>,
) -> Result<RangeUpdate> {
    for bound in [interval.lower, interval.upper] {
        if let Bound::Included(key) | Bound::Excluded(key) = bound {
            assert!(
                (1..=MAX_KEY).contains(&key.len()),
                "a key has 1 to {MAX_KEY} bytes"
            );
        }
    }
    assert!(
        interval.prefix.len() <= MAX_KEY,
        "a prefix has up to {MAX_KEY} bytes"
    );
    if interval.is_empty() {
        return Ok(RangeUpdate { root, removed: 0 });
    }
    let mut removed = 0;
    // `remove_in` checked the root, which no other node points to.
    let outcome = remove_in(tx, root, None, &interval, &mut removed)?;
    let root = fix_root(tx, root, outcome)?;
    Ok(RangeUpdate { root, removed })
}

/// Removes the entries of `interval` from the subtree at `page`, a node of `level` when given, adding their number
/// to `removed`. A subtree where it removes nothing reports [`Outcome::Done`] and changes no page.
fn remove_in(
    tx: &mut WriteTransaction<'_>,
    page: u64,
    level: Option<u8>,
    interval: &Interval<'_>,
    removed: &mut u64,
) -> Result<Outcome> {
    let node = node_at(&*tx, page, level)?;
    // In a leaf, the run of entries in the interval; in a branch, the children that hold its first and last keys.
    let first = node::partition(node, |key| !interval.after_lower(key));
    let last = node::partition(node, |key| interval.before_upper(key));
    if node::is_leaf(node) {
        if first >= last {
            return Ok(Outcome::Done);
        }
        free_runs(tx, page, first..last)?;
        let node = tx.page_mut(page)?;
        for _ in first..last {
            node::remove(node, first);
        }
        *removed += (last - first) as u64;
        if node::count(node) == 0 {
            return Ok(Outcome::Empty);
        }
        return Ok(underfull_or_done(node));
    }
    let below = child_level(node);
    if first >= last {
        let child = node::child(node, first.min(last));
        return match remove_in(tx, child, below, interval, removed)? {
            Outcome::Done => Ok(Outcome::Done),
            Outcome::Underfull => merge_child(tx, page, first.min(last)),
            Outcome::Empty => drop_child(tx, page, first.min(last)),
            Outcome::Split { .. } => unreachable!("a removal does not split"),
        };
    }
    // The children wholly inside the interval, between the two that hold its bounds.
    let (left, right) = (node::child(node, first), node::child(node, last));
    let mut inner = Vec::new();
    reserve(&*tx, &mut inner, last - first - 1)?;
    inner.extend((first + 1..last).map(|i| node::child(node, i)));
    let before = *removed;
    let left_outcome = remove_in(tx, left, below, interval, removed)?;
    let right_outcome = remove_in(tx, right, below, interval, removed)?;
    for &child in &inner {
        *removed += free_subtree(tx, child, below)?;
    }
    if *removed == before {
        return Ok(Outcome::Done);
    }
    if !inner.is_empty() {
        // Cell `first` separates child `first` from the next one: removing the cell drops that child.
        let parent = tx.page_mut(page)?;
        for _ in &inner {
            node::remove(parent, first);
        }
    }
    // The two children that hold the bounds are now children `first` and `first + 1`.
    let right_gone = matches!(right_outcome, Outcome::Empty);
    let left_gone = matches!(left_outcome, Outcome::Empty);
    let mut emptied = false;
    if right_gone {
        emptied |= matches!(drop_child(tx, page, first + 1)?, Outcome::Empty);
    }
    if left_gone {
        emptied |= matches!(drop_child(tx, page, first)?, Outcome::Empty);
    }
    if emptied {
        return Ok(Outcome::Empty);
    }
    if !left_gone && matches!(left_outcome, Outcome::Underfull) {
        merge_child(tx, page, first)?;
    }
    if !right_gone && matches!(right_outcome, Outcome::Underfull) {
        // The right child may have been merged into the left one just above.
        let at = if left_gone { first } else { first + 1 };
        let parent = tx.page(page)?;
        if at <= node::count(parent) && node::child(parent, at) == right {
            merge_child(tx, page, at)?;
        }
    }
    Ok(underfull_or_done(tx.page(page)?))
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
    // `change` checked the root, which no other node points to.
    let outcome = change(tx, root, None, key, op, &mut count_changed)?;
    let root = fix_root(tx, root, outcome)?;
    Ok(Update {
        root,
        count_changed,
    })
}

/// The tree's root page after a change left `outcome` in the root at `root`: a new root above a split one, the only
/// child of a branch root left without cells, or the same page.
fn fix_root(tx: &mut WriteTransaction<'_>, root: u64, outcome: Outcome) -> Result<u64> {
    let mut root = root;
    match outcome {
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
    Ok(root)
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
        // Adding over an existing key reserves nothing, whatever the value's size (V1, A10).
        (Op::Add(_), Ok(_)) | (Op::Remove, Err(_)) => return Ok(Outcome::Done),
        (Op::Remove, Ok(index)) => {
            *count_changed = true;
            if let Value::Run { first, len } = node::value(node, index) {
                free_run(tx, first, len)?;
            }
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
    let old_value = found.ok().map(|index| node::value(node, index));
    if let (Ok(index), Some(Value::Inline(old_bytes))) = (found, old_value)
        && old_bytes.len() == value.len()
        && node::fits_inline(key.len(), value.len())
    {
        node::value_mut(tx.page_mut(page)?, index).copy_from_slice(value);
        return Ok(Outcome::Done);
    }
    let index = found.unwrap_or_else(|i| i);
    let room = node::free_space(node) + found.map_or(0, |i| node::cell(node, i).len() + 2);
    // A value off its leaf is replaced by a new run: runs never change (D6).
    let old_run = match old_value {
        Some(Value::Run { first, len }) => Some((first, len)),
        _ => None,
    };
    let mut buf = [0; MAX_LEAF_CELL];
    let cell_len = encode_entry(tx, &mut buf, key, value)?;
    if let Some((old_first, old_len)) = old_run {
        free_run(tx, old_first, old_len)?;
    }
    if room < cell_len + 2 {
        return split(tx, page, index, found.is_ok(), &buf[..cell_len]);
    }
    let node = tx.page_mut(page)?;
    if found.is_ok() {
        node::remove(node, index);
    }
    node::insert(node, index, &buf[..cell_len]);
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
