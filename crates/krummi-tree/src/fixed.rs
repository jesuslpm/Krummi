//! Fixed-size trees (D1–D4 of the fixed-size trees execution): ordered maps from keys that are unsigned 64-bit
//! integers, in numeric order (X5), to values that all have the tree's length, 0 to [`MAX_LEN`] bytes, fixed when
//! the tree is created (X1). Their nodes have a dense format of their own, so that an entry takes only its key and
//! its value:
//!
//! | Bytes | Content |
//! |---|---|
//! | 0 | kind: `3` leaf, `4` branch |
//! | 1 | level: `0` for a leaf, one more than its children's for a branch |
//! | 2–3 | number of entries |
//! | 4–5 | a leaf's value length, the tree's; zero in a branch |
//! | 6–7 | zero |
//! | 8–15 | a branch's leftmost child; zero in a leaf |
//! | 16… | the entries, packed in key order |
//!
//! A leaf entry is the key (8 bytes) and the value; a branch entry is the key (8 bytes) and a child page (8 bytes),
//! the child holding the keys from that key up to the next entry's, and the leftmost child the keys before the
//! first. Integers are little-endian (X12). A leaf holds `(PAYLOAD_SIZE − 16) / (8 + len)` entries, 3 at 2,048 bytes
//! and 1,020 at 0, and a branch 510.
//!
//! The rules are those of the slotted trees: a full node splits by count into halves, unless the new entry falls
//! past its last one: then it keeps its entries and a new sibling starts with the new one (a branch moves its last
//! key up), so that keys added in increasing order fill their nodes. A node left under a quarter of its page merges
//! with a sibling when both fit in one; a branch root left with one child is replaced by it; a node left without
//! entries is freed and leaves its parent, and the root keeps its page. Operations that change nothing touch no page
//! (T10), while overwriting a value always changes its leaf (X8).
//!
//! Every node is read through the checked read of the slotted trees ([`is_valid`] keeps every accessor within the
//! page), sharing their remembered check, and a leaf whose length is not the tree's is damage of its page (D2, X14).
//! An interval is two bounds normalised to inclusive integers, empty exactly when no integer lies between them; an
//! empty one reads no page (D4, X7). Memory besides pages is reserved through [`reserve`] (P13, T11).
//!
//! The operations take the tree's length on every call; like the rest of the crate they do not validate sizes (B7):
//! they panic on a length over [`MAX_LEN`] or a value of another length, a misuse of the engine.

use std::ops::Bound::{self, Excluded, Included, Unbounded};

use krummi_tx::{PAYLOAD_SIZE, Result, WriteTransaction};

use crate::node::{UNDERFULL, put_u16, u16_at, u64_at};
use crate::{
    Format, Op, Outcome, PageSource, RangeUpdate, Update, child_level, node_at_with, reserve,
};

/// Longest value of a fixed-size tree (X1).
pub const MAX_LEN: usize = 2048;
/// Bytes of the node header.
const HEADER: usize = 16;
/// Bytes of a key.
const KEY: usize = 8;
/// Bytes of a branch entry: its key and its child.
const BRANCH_ENTRY: usize = KEY + 8;
const LEAF: u8 = 3;
const BRANCH: u8 = 4;

const FIXED: Format = Format {
    is_valid,
    kinds: [LEAF, BRANCH],
};

// A full node splits into two that are not empty, and a node's count fits in its two bytes.
const _: () = assert!(capacity(KEY + MAX_LEN) >= 3);
const _: () = assert!(capacity(KEY) <= u16::MAX as usize);

/// How many entries of `entry` bytes a node holds.
const fn capacity(entry: usize) -> usize {
    (PAYLOAD_SIZE - HEADER) / entry
}

/// Whether `node`, the payload of a page whose checksum is valid, has a consistent layout: a leaf at level `0` with
/// a length of up to [`MAX_LEN`] or a branch at levels `1..=254` without one, zero bytes 6 and 7, and a number of
/// entries within the capacity its own header gives, so that every accessor stays within the page (D2). Key order
/// is not checked, as in the slotted nodes.
pub(crate) fn is_valid(node: &[u8]) -> bool {
    let entry = match (node[0], node[1], value_len(node)) {
        (LEAF, 0, len) if len <= MAX_LEN => KEY + len,
        (BRANCH, 1..=254, 0) => BRANCH_ENTRY,
        _ => return false,
    };
    node[6] == 0 && node[7] == 0 && count(node) <= capacity(entry)
}

pub(crate) fn is_leaf(node: &[u8]) -> bool {
    node[0] == LEAF
}

pub(crate) fn count(node: &[u8]) -> usize {
    u16_at(node, 2)
}

/// The length of a leaf's values.
pub(crate) fn value_len(node: &[u8]) -> usize {
    u16_at(node, 4)
}

fn leftmost(node: &[u8]) -> u64 {
    u64_at(node, 8)
}

fn entry_size(node: &[u8]) -> usize {
    if is_leaf(node) {
        KEY + value_len(node)
    } else {
        BRANCH_ENTRY
    }
}

/// Where entry `index` starts; at the number of entries, where they end.
fn offset(node: &[u8], index: usize) -> usize {
    HEADER + index * entry_size(node)
}

pub(crate) fn key(node: &[u8], index: usize) -> u64 {
    u64_at(node, offset(node, index))
}

fn value(node: &[u8], index: usize) -> &[u8] {
    let at = offset(node, index) + KEY;
    &node[at..at + value_len(node)]
}

/// Child `index` of a branch, from `0` (the leftmost) to its number of entries.
pub(crate) fn child(node: &[u8], index: usize) -> u64 {
    if index == 0 {
        leftmost(node)
    } else {
        u64_at(node, offset(node, index - 1) + KEY)
    }
}

/// How many of the node's first keys satisfy `holds`, a test that is true up to some key and false after it.
fn partition(node: &[u8], holds: impl Fn(u64) -> bool) -> usize {
    let (mut low, mut high) = (0, count(node));
    while low < high {
        let mid = low + (high - low) / 2;
        if holds(key(node, mid)) {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    low
}

/// The index of the entry of `key`, or where it would go.
fn search(node: &[u8], key: u64) -> std::result::Result<usize, usize> {
    let index = partition(node, |k| k < key);
    if index < count(node) && self::key(node, index) == key {
        Ok(index)
    } else {
        Err(index)
    }
}

/// The child of a branch that holds `key`.
fn child_index(node: &[u8], key: u64) -> usize {
    partition(node, |k| k <= key)
}

/// Makes `node` a node of `level`, a leaf at level `0` with values of `len` bytes, holding `entries`, packed.
pub(crate) fn fill(node: &mut [u8], level: u8, len: usize, leftmost: u64, entries: &[u8]) {
    node[..HEADER].fill(0);
    node[0] = if level == 0 { LEAF } else { BRANCH };
    node[1] = level;
    put_u16(node, 4, if level == 0 { len } else { 0 });
    node[8..HEADER].copy_from_slice(&leftmost.to_le_bytes());
    node[HEADER..HEADER + entries.len()].copy_from_slice(entries);
    put_u16(node, 2, entries.len() / entry_size(node));
}

/// Inserts `entry` as entry `index`.
fn insert(node: &mut [u8], index: usize, entry: &[u8]) {
    let n = count(node);
    let (at, end) = (offset(node, index), offset(node, n));
    node.copy_within(at..end, at + entry.len());
    node[at..at + entry.len()].copy_from_slice(entry);
    put_u16(node, 2, n + 1);
}

/// Removes the entries `indexes`, moving the next ones to keep them packed.
fn remove_entries(node: &mut [u8], indexes: std::ops::Range<usize>) {
    let n = count(node);
    let (from, to, end) = (
        offset(node, indexes.start),
        offset(node, indexes.end),
        offset(node, n),
    );
    node.copy_within(to..end, from);
    put_u16(node, 2, n - indexes.len());
}

fn branch_entry(key: u64, child: u64) -> [u8; BRANCH_ENTRY] {
    let mut entry = [0; BRANCH_ENTRY];
    entry[..KEY].copy_from_slice(&key.to_le_bytes());
    entry[KEY..].copy_from_slice(&child.to_le_bytes());
    entry
}

fn underfull_or_done(node: &[u8]) -> Outcome<u64> {
    if offset(node, count(node)) < UNDERFULL {
        Outcome::Underfull
    } else {
        Outcome::Done
    }
}

/// The payload of fixed node `page`, checked as the slotted nodes are (see [`node_at_with`]), and, when it is a
/// leaf, with `len` as its length; otherwise damage of `page` (D2).
fn node_of<S: PageSource + ?Sized>(
    tx: &S,
    page: u64,
    level: Option<u8>,
    len: usize,
) -> Result<&[u8]> {
    let node = node_at_with(tx, page, level, &FIXED)?;
    if is_leaf(node) && value_len(node) != len {
        return Err(tx.damaged(page));
    }
    Ok(node)
}

/// The inclusive bounds of the keys between `lower` and `upper`, or `None` when no integer lies between them (D4):
/// the lower bound above the upper one, the same integer with one excluded, two contiguous integers excluded, an
/// excluded lower bound `u64::MAX` or an excluded upper bound `0` (X7). It is decided from the bounds alone.
pub fn interval(lower: Bound<u64>, upper: Bound<u64>) -> Option<(u64, u64)> {
    let low = match lower {
        Included(key) => key,
        Excluded(key) => key.checked_add(1)?,
        Unbounded => 0,
    };
    let high = match upper {
        Included(key) => key,
        Excluded(key) => key.checked_sub(1)?,
        Unbounded => u64::MAX,
    };
    (low <= high).then_some((low, high))
}

/// Creates an empty fixed-size tree whose values have `len` bytes and returns its root page.
///
/// # Panics
///
/// If `len` is over [`MAX_LEN`].
pub fn create(tx: &mut WriteTransaction<'_>, len: usize) -> Result<u64> {
    assert!(len <= MAX_LEN, "a value has up to {MAX_LEN} bytes");
    let root = tx.allocate()?;
    fill(tx.page_mut(root)?, 0, len, 0, &[]);
    Ok(root)
}

/// The value of `key` in the tree at `root`, of `len` bytes, borrowed from the transaction without copying it (D11);
/// with `len` 0, an empty value.
pub fn get<S: PageSource + ?Sized>(
    tx: &S,
    root: u64,
    len: usize,
    key: u64,
) -> Result<Option<&[u8]>> {
    let (_, node) = descend(tx, root, len, key)?;
    Ok(search(node, key).ok().map(|index| value(node, index)))
}

/// The page of the leaf of the tree at `root` where `key` is or would be (tests use it to damage a known page).
pub fn leaf_of<S: PageSource + ?Sized>(tx: &S, root: u64, len: usize, key: u64) -> Result<u64> {
    Ok(descend(tx, root, len, key)?.0)
}

/// The number of levels of the tree at `root`, one for a lone leaf (tests use it to see a tree gain and lose
/// levels).
pub fn height<S: PageSource + ?Sized>(tx: &S, root: u64, len: usize) -> Result<u8> {
    Ok(node_of(tx, root, None, len)?[1] + 1)
}

/// The page number and the payload of the leaf where `key` is or would be.
fn descend<S: PageSource + ?Sized>(
    tx: &S,
    root: u64,
    len: usize,
    key: u64,
) -> Result<(u64, &[u8])> {
    let (mut page, mut level) = (root, None);
    loop {
        let node = node_of(tx, page, level, len)?;
        if is_leaf(node) {
            return Ok((page, node));
        }
        level = child_level(node);
        page = child(node, child_index(node, key));
    }
}

/// Stores `value` under `key`, creating the entry or overwriting its value; an overwrite with the same value still
/// changes the leaf (X8).
///
/// # Panics
///
/// If `len` is over [`MAX_LEN`] or `value` does not have `len` bytes.
pub fn set(
    tx: &mut WriteTransaction<'_>,
    root: u64,
    len: usize,
    key: u64,
    value: &[u8],
) -> Result<Update> {
    update(tx, root, len, key, Op::Set(value))
}

/// Stores `value` under `key` only if the key is not there; otherwise changes nothing.
///
/// # Panics
///
/// As [`set`].
pub fn try_add(
    tx: &mut WriteTransaction<'_>,
    root: u64,
    len: usize,
    key: u64,
    value: &[u8],
) -> Result<Update> {
    update(tx, root, len, key, Op::Add(value))
}

/// Removes the entry of `key`, if it is there.
///
/// # Panics
///
/// If `len` is over [`MAX_LEN`].
pub fn remove(tx: &mut WriteTransaction<'_>, root: u64, len: usize, key: u64) -> Result<Update> {
    update(tx, root, len, key, Op::Remove)
}

fn update(
    tx: &mut WriteTransaction<'_>,
    root: u64,
    len: usize,
    key: u64,
    op: Op<'_>,
) -> Result<Update> {
    assert!(len <= MAX_LEN, "a value has up to {MAX_LEN} bytes");
    if let Op::Set(value) | Op::Add(value) = op {
        assert_eq!(value.len(), len, "a value has the tree's length");
    }
    let mut count_changed = false;
    // `change` checked the root, which no other node points to.
    let outcome = change(tx, root, None, len, key, op, &mut count_changed)?;
    let root = fix_root(tx, root, len, outcome)?;
    Ok(Update {
        root,
        count_changed,
    })
}

/// The tree's root page after a change left `outcome` in the root at `root`: a new root above a split one, the only
/// child of a branch root left without entries, or the same page.
fn fix_root(
    tx: &mut WriteTransaction<'_>,
    root: u64,
    len: usize,
    outcome: Outcome<u64>,
) -> Result<u64> {
    let mut root = root;
    match outcome {
        Outcome::Done => {}
        Outcome::Split { key, right } => {
            // A checked branch is below level 255.
            let level = tx.page(root)?[1] + 1;
            let new_root = tx.allocate()?;
            fill(
                tx.page_mut(new_root)?,
                level,
                len,
                root,
                &branch_entry(key, right),
            );
            root = new_root;
        }
        Outcome::Underfull => {
            let mut level = None;
            loop {
                let node = node_of(&*tx, root, level, len)?;
                if is_leaf(node) || count(node) > 0 {
                    break;
                }
                level = child_level(node);
                let only = leftmost(node);
                tx.free(root, 1)?;
                root = only;
            }
        }
        // The root keeps its page: an empty leaf stays, and a branch whose only child went away becomes one.
        Outcome::Empty => {
            if !is_leaf(tx.page(root)?) {
                fill(tx.page_mut(root)?, 0, len, 0, &[]);
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
    len: usize,
    key: u64,
    op: Op<'_>,
    count_changed: &mut bool,
) -> Result<Outcome<u64>> {
    let node = node_of(&*tx, page, level, len)?;
    if is_leaf(node) {
        return change_leaf(tx, page, key, op, count_changed);
    }
    let index = child_index(node, key);
    let (below, below_level) = (child(node, index), child_level(node));
    match change(tx, below, below_level, len, key, op, count_changed)? {
        Outcome::Done => Ok(Outcome::Done),
        Outcome::Split { key, right } => insert_entry(tx, page, index, &branch_entry(key, right)),
        Outcome::Underfull => merge_child(tx, page, index, len),
        Outcome::Empty => drop_child(tx, page, index),
    }
}

fn change_leaf(
    tx: &mut WriteTransaction<'_>,
    page: u64,
    key: u64,
    op: Op<'_>,
    count_changed: &mut bool,
) -> Result<Outcome<u64>> {
    match (op, search(tx.page(page)?, key)) {
        (Op::Add(_), Ok(_)) | (Op::Remove, Err(_)) => Ok(Outcome::Done),
        (Op::Remove, Ok(index)) => {
            *count_changed = true;
            let node = tx.page_mut(page)?;
            remove_entries(node, index..index + 1);
            if count(node) == 0 {
                return Ok(Outcome::Empty);
            }
            Ok(underfull_or_done(node))
        }
        (Op::Set(value), Ok(index)) => {
            let node = tx.page_mut(page)?;
            let at = offset(node, index) + KEY;
            node[at..at + value.len()].copy_from_slice(value);
            Ok(Outcome::Done)
        }
        (Op::Set(value) | Op::Add(value), Err(index)) => {
            *count_changed = true;
            let mut entry = [0; KEY + MAX_LEN];
            entry[..KEY].copy_from_slice(&key.to_le_bytes());
            entry[KEY..KEY + value.len()].copy_from_slice(value);
            insert_entry(tx, page, index, &entry[..KEY + value.len()])
        }
    }
}

/// Inserts `entry` as entry `index` of the node at `page`, splitting the node when it is full.
fn insert_entry(
    tx: &mut WriteTransaction<'_>,
    page: u64,
    index: usize,
    entry: &[u8],
) -> Result<Outcome<u64>> {
    if count(tx.page(page)?) == capacity(entry.len()) {
        return split(tx, page, index, entry);
    }
    let node = tx.page_mut(page)?;
    insert(node, index, entry);
    // As in the slotted trees, a leaf may still be under a quarter full, and a branch that gains a child is done.
    Ok(if is_leaf(node) {
        underfull_or_done(node)
    } else {
        Outcome::Done
    })
}

/// Splits the full node at `page`, with `entry` put at `index`, into itself and a new right sibling. A new entry
/// past the node's last one hints that keys arrive in order, so the node keeps its entries and the sibling starts
/// with the new one: a leaf is left untouched, and a branch moves its last key up to the parent, its child becoming
/// the sibling's leftmost, so that the sibling has a key. Any other split divides the entries into halves.
fn split(
    tx: &mut WriteTransaction<'_>,
    page: u64,
    index: usize,
    entry: &[u8],
) -> Result<Outcome<u64>> {
    let node = tx.page(page)?;
    let (n, size, level, len) = (count(node), entry.len(), node[1], value_len(node));
    let leaf = is_leaf(node);
    let append = index == n;
    if leaf && append {
        let right = tx.allocate()?;
        fill(tx.page_mut(right)?, 0, len, 0, entry);
        return Ok(Outcome::Split {
            key: u64_at(entry, 0),
            right,
        });
    }
    let left_leftmost = leftmost(node);
    let mut all = Vec::new();
    reserve(&*tx, &mut all, (n + 1) * size)?;
    let at_index = offset(node, index);
    all.extend_from_slice(&node[HEADER..at_index]);
    all.extend_from_slice(entry);
    all.extend_from_slice(&node[at_index..offset(node, n)]);
    let at = if append { n - 1 } else { n.div_ceil(2) };
    let (key, right_leftmost, right_entries) = if leaf {
        (u64_at(&all, at * size), 0, &all[at * size..])
    } else {
        let up = at * size;
        (u64_at(&all, up), u64_at(&all, up + KEY), &all[up + size..])
    };
    let right = tx.allocate()?;
    fill(
        tx.page_mut(right)?,
        level,
        len,
        right_leftmost,
        right_entries,
    );
    fill(
        tx.page_mut(page)?,
        level,
        len,
        left_leftmost,
        &all[..at * size],
    );
    Ok(Outcome::Split { key, right })
}

/// Merges child `index` of the branch at `page`, which is under a quarter full, with a sibling when both fit in
/// one node.
fn merge_child(
    tx: &mut WriteTransaction<'_>,
    page: u64,
    index: usize,
    len: usize,
) -> Result<Outcome<u64>> {
    let parent = tx.page(page)?;
    let n = count(parent);
    if n == 0 {
        return Ok(Outcome::Underfull);
    }
    // The pair is children `pair` and `pair + 1`, separated by entry `pair`.
    let pair = index.min(n - 1);
    let (left, right) = (child(parent, pair), child(parent, pair + 1));
    let separator = key(parent, pair);
    let below = child_level(parent);
    let left_count = count(node_of(&*tx, left, below, len)?);
    let right_node = node_of(&*tx, right, below, len)?;
    let leaf = is_leaf(right_node);
    // A branch also takes the separator, with the right node's leftmost child.
    let total = left_count + count(right_node) + usize::from(!leaf);
    if total > capacity(entry_size(right_node)) {
        return Ok(Outcome::Done);
    }
    let mut moved = Vec::new();
    reserve(
        &*tx,
        &mut moved,
        (count(right_node) + 1) * entry_size(right_node),
    )?;
    if !leaf {
        moved.extend_from_slice(&branch_entry(separator, leftmost(right_node)));
    }
    moved.extend_from_slice(&right_node[HEADER..offset(right_node, count(right_node))]);
    let target = tx.page_mut(left)?;
    let end = offset(target, left_count);
    target[end..end + moved.len()].copy_from_slice(&moved);
    put_u16(target, 2, total);
    tx.free(right, 1)?;
    let parent = tx.page_mut(page)?;
    remove_entries(parent, pair..pair + 1);
    Ok(underfull_or_done(parent))
}

/// Frees child `index` of the branch at `page`, which has no entries left, and removes it with its separator; the
/// branch is then empty itself when that was its only child.
fn drop_child(tx: &mut WriteTransaction<'_>, page: u64, index: usize) -> Result<Outcome<u64>> {
    let parent = tx.page(page)?;
    let (n, gone) = (count(parent), child(parent, index));
    tx.free(gone, 1)?;
    if n == 0 {
        return Ok(Outcome::Empty);
    }
    let parent = tx.page_mut(page)?;
    if index == 0 {
        let next = child(parent, 1);
        parent[8..HEADER].copy_from_slice(&next.to_le_bytes());
        remove_entries(parent, 0..1);
    } else {
        remove_entries(parent, index - 1..index);
    }
    Ok(underfull_or_done(parent))
}

/// Frees every page of the tree at `root`, reading every node.
pub fn free_all(tx: &mut WriteTransaction<'_>, root: u64, len: usize) -> Result<()> {
    free_subtree(tx, root, None, len).map(|_| ())
}

/// Frees every page of the subtree at `page`, a node of `level` when given, and returns how many entries it had.
fn free_subtree(
    tx: &mut WriteTransaction<'_>,
    page: u64,
    level: Option<u8>,
    len: usize,
) -> Result<u64> {
    let node = node_of(&*tx, page, level, len)?;
    let n = count(node);
    if is_leaf(node) {
        tx.free(page, 1)?;
        return Ok(n as u64);
    }
    let mut children = Vec::new();
    reserve(&*tx, &mut children, n + 1)?;
    children.extend((0..=n).map(|i| child(node, i)));
    let below = child_level(node);
    // A node is freed before its children, so a child that names the node itself, an ancestor or a page freed
    // before it no longer may hold data and is reported as damage instead of being freed twice.
    tx.free(page, 1)?;
    let mut entries = 0;
    for page in children {
        entries += free_subtree(tx, page, below, len)?;
    }
    Ok(entries)
}

/// Removes every entry between `lower` and `upper` from the tree at `root`, as the slotted trees do: a leaf loses
/// its run of entries in the interval, a branch frees the children wholly inside it, reading their nodes to count
/// their entries, and the children that hold the bounds are fixed like after a key removal. An empty interval (D4)
/// reads nothing, and a removal that finds no entry changes no page (T10).
///
/// # Panics
///
/// If `len` is over [`MAX_LEN`].
pub fn remove_range(
    tx: &mut WriteTransaction<'_>,
    root: u64,
    len: usize,
    lower: Bound<u64>,
    upper: Bound<u64>,
) -> Result<RangeUpdate> {
    assert!(len <= MAX_LEN, "a value has up to {MAX_LEN} bytes");
    let Some((low, high)) = interval(lower, upper) else {
        return Ok(RangeUpdate { root, removed: 0 });
    };
    let mut removed = 0;
    // `remove_in` checked the root, which no other node points to.
    let outcome = remove_in(tx, root, None, len, (low, high), &mut removed)?;
    let root = fix_root(tx, root, len, outcome)?;
    Ok(RangeUpdate { root, removed })
}

/// Removes the entries from `low` to `high`, both included, from the subtree at `page`, a node of `level` when
/// given, adding their number to `removed`. A subtree where it removes nothing reports [`Outcome::Done`] and changes
/// no page.
fn remove_in(
    tx: &mut WriteTransaction<'_>,
    page: u64,
    level: Option<u8>,
    len: usize,
    (low, high): (u64, u64),
    removed: &mut u64,
) -> Result<Outcome<u64>> {
    let node = node_of(&*tx, page, level, len)?;
    if is_leaf(node) {
        let (first, last) = (partition(node, |k| k < low), partition(node, |k| k <= high));
        if first == last {
            return Ok(Outcome::Done);
        }
        let node = tx.page_mut(page)?;
        remove_entries(node, first..last);
        *removed += (last - first) as u64;
        if count(node) == 0 {
            return Ok(Outcome::Empty);
        }
        return Ok(underfull_or_done(node));
    }
    let below = child_level(node);
    // The children that hold the first and the last keys of the interval.
    let (first, last) = (child_index(node, low), child_index(node, high));
    if first == last {
        let only = child(node, first);
        return match remove_in(tx, only, below, len, (low, high), removed)? {
            Outcome::Done => Ok(Outcome::Done),
            Outcome::Underfull => merge_child(tx, page, first, len),
            Outcome::Empty => drop_child(tx, page, first),
            Outcome::Split { .. } => unreachable!("a removal does not split"),
        };
    }
    // The children wholly inside the interval, between the two that hold its bounds.
    let (left, right) = (child(node, first), child(node, last));
    let mut inner = Vec::new();
    reserve(&*tx, &mut inner, last - first - 1)?;
    inner.extend((first + 1..last).map(|i| child(node, i)));
    let before = *removed;
    let left_outcome = remove_in(tx, left, below, len, (low, high), removed)?;
    let right_outcome = remove_in(tx, right, below, len, (low, high), removed)?;
    for &inside in &inner {
        *removed += free_subtree(tx, inside, below, len)?;
    }
    if inner.is_empty() && *removed == before {
        return Ok(Outcome::Done);
    }
    if !inner.is_empty() {
        // Entry `first` separates child `first` from the next one: removing the entries drops the inner children.
        remove_entries(tx.page_mut(page)?, first..first + inner.len());
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
        merge_child(tx, page, first, len)?;
    }
    if !right_gone && matches!(right_outcome, Outcome::Underfull) {
        // The right child may have been merged into the left one just above.
        let at = if left_gone { first } else { first + 1 };
        let parent = tx.page(page)?;
        if at <= count(parent) && child(parent, at) == right {
            merge_child(tx, page, at, len)?;
        }
    }
    Ok(underfull_or_done(tx.page(page)?))
}

/// One end of a [`FixedRange`]: the path from the root to its leaf, each node with the index of the child it went
/// down to and, in the leaf, the front's next entry or the back's entry after its next one.
struct End<'t> {
    path: Vec<(&'t [u8], usize)>,
    started: bool,
    finished: bool,
    /// The last key this end returned.
    last: Option<u64>,
}

impl End<'_> {
    const fn new() -> Self {
        End {
            path: Vec::new(),
            started: false,
            finished: false,
            last: None,
        }
    }
}

/// A double-ended scan of the entries between two bounds of a fixed-size tree, in numeric order, borrowed from the
/// transaction without copying them (D11), built as the slotted trees' [`Range`](crate::Range): it reads no page
/// until it is used, each end keeps its path and stops at the bound or before the other end's last entry, so that
/// each entry comes out once, and after any failure both ends stop.
pub struct FixedRange<'t, S: PageSource + ?Sized> {
    tx: &'t S,
    root: u64,
    len: usize,
    low: u64,
    high: u64,
    front: End<'t>,
    back: End<'t>,
}

impl<'t, S: PageSource + ?Sized> FixedRange<'t, S> {
    /// A scan of the keys between `lower` and `upper` in the tree at `root`, whose values have `len` bytes. It reads
    /// no page; an empty interval (D4) gives an empty scan.
    pub fn new(tx: &'t S, root: u64, len: usize, lower: Bound<u64>, upper: Bound<u64>) -> Self {
        let bounds = interval(lower, upper);
        let (low, high) = bounds.unwrap_or((1, 0));
        let mut range = FixedRange {
            tx,
            root,
            len,
            low,
            high,
            front: End::new(),
            back: End::new(),
        };
        if bounds.is_none() {
            range.stop();
        }
        range
    }

    /// Ends both ends: after a failure, or once either end is exhausted, nothing more comes out.
    fn stop(&mut self) {
        self.front.finished = true;
        self.back.finished = true;
    }

    /// The path from the root to the leaf where an end starts: at each level, the child that holds `low` and, in
    /// the leaf, the index of the first key from it, for the front; the child that holds `high` and, in the leaf,
    /// the index after the last key up to it, for the back.
    fn seek(&mut self, front: bool) -> Result<()> {
        let (low, high, len) = (self.low, self.high, self.len);
        let path = if front {
            &mut self.front.path
        } else {
            &mut self.back.path
        };
        let mut node = node_of(self.tx, self.root, None, len)?;
        reserve(self.tx, path, usize::from(node[1]) + 1)?;
        loop {
            let index = match (front, is_leaf(node)) {
                (true, true) => partition(node, |k| k < low),
                (true, false) => child_index(node, low),
                (false, _) => partition(node, |k| k <= high),
            };
            path.push((node, index));
            if is_leaf(node) {
                return Ok(());
            }
            node = node_of(self.tx, child(node, index), child_level(node), len)?;
        }
    }

    /// The front's next entry, or `None` at its end.
    fn step_front(&mut self) -> Result<Option<(u64, &'t [u8])>> {
        if !self.front.started {
            self.front.started = true;
            self.seek(true)?;
        }
        loop {
            let Some(&(leaf, index)) = self.front.path.last() else {
                return Ok(None);
            };
            if index < count(leaf) {
                let found = key(leaf, index);
                if found > self.high || self.back.last.is_some_and(|last| found >= last) {
                    return Ok(None);
                }
                self.front.path.last_mut().expect("a leaf").1 += 1;
                self.front.last = Some(found);
                return Ok(Some((found, value(leaf, index))));
            }
            // The next leaf: up to the first branch with a child to the right, then down its leftmost children.
            self.front.path.pop();
            loop {
                let Some((branch, next)) = self.front.path.last_mut() else {
                    return Ok(None);
                };
                if *next < count(branch) {
                    // The next child's keys start at this separator: if it already ends the scan, they all do, and
                    // the scan stops without reading that child.
                    let separator = key(branch, *next);
                    if separator > self.high || self.back.last.is_some_and(|last| separator >= last)
                    {
                        return Ok(None);
                    }
                    *next += 1;
                    let (branch, next) = (*branch, *next);
                    self.descend(true, branch, next)?;
                    break;
                }
                self.front.path.pop();
            }
        }
    }

    /// The back's next entry, or `None` at its end.
    fn step_back(&mut self) -> Result<Option<(u64, &'t [u8])>> {
        if !self.back.started {
            self.back.started = true;
            self.seek(false)?;
        }
        loop {
            let Some(&(leaf, end)) = self.back.path.last() else {
                return Ok(None);
            };
            if end > 0 {
                let index = end - 1;
                let found = key(leaf, index);
                if found < self.low || self.front.last.is_some_and(|last| found <= last) {
                    return Ok(None);
                }
                self.back.path.last_mut().expect("a leaf").1 = index;
                self.back.last = Some(found);
                return Ok(Some((found, value(leaf, index))));
            }
            // The previous leaf: up to the first branch with a child to the left, then down its rightmost children.
            self.back.path.pop();
            loop {
                let Some((branch, next)) = self.back.path.last_mut() else {
                    return Ok(None);
                };
                if *next > 0 {
                    // The previous child's keys are before this separator: if they are all before the interval or
                    // the front's last key, the scan stops without reading that child.
                    let separator = key(branch, *next - 1);
                    if separator <= self.low
                        || self.front.last.is_some_and(|last| separator <= last)
                    {
                        return Ok(None);
                    }
                    *next -= 1;
                    let (branch, next) = (*branch, *next);
                    self.descend(false, branch, next)?;
                    break;
                }
                self.back.path.pop();
            }
        }
    }

    /// Goes down from child `index` of `branch` to a leaf along the leftmost children for the front, or the
    /// rightmost ones for the back, and adds the nodes to that end's path.
    fn descend(&mut self, front: bool, branch: &'t [u8], index: usize) -> Result<()> {
        let path = if front {
            &mut self.front.path
        } else {
            &mut self.back.path
        };
        let mut node = node_of(self.tx, child(branch, index), child_level(branch), self.len)?;
        loop {
            // A branch's rightmost child and a leaf's end are both at its number of entries.
            let index = if front { 0 } else { count(node) };
            path.push((node, index));
            if is_leaf(node) {
                return Ok(());
            }
            node = node_of(self.tx, child(node, index), child_level(node), self.len)?;
        }
    }

    /// Runs a step of one end, stopping both ends after a failure.
    fn guard(&mut self, front: bool) -> Option<Result<(u64, &'t [u8])>> {
        let finished = if front {
            self.front.finished
        } else {
            self.back.finished
        };
        if finished {
            return None;
        }
        let step = if front {
            self.step_front()
        } else {
            self.step_back()
        };
        match step {
            Ok(Some(entry)) => Some(Ok(entry)),
            Ok(None) => {
                // An end stops only when no entry is left between it and the other end's last one, or the bound,
                // so the other end must not read or reserve anything more either.
                self.stop();
                None
            }
            Err(error) => {
                self.stop();
                Some(Err(error))
            }
        }
    }
}

impl<'t, S: PageSource + ?Sized> Iterator for FixedRange<'t, S> {
    type Item = Result<(u64, &'t [u8])>;

    fn next(&mut self) -> Option<Self::Item> {
        self.guard(true)
    }
}

impl<S: PageSource + ?Sized> DoubleEndedIterator for FixedRange<'_, S> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.guard(false)
    }
}

impl<S: PageSource + ?Sized> std::iter::FusedIterator for FixedRange<'_, S> {}
