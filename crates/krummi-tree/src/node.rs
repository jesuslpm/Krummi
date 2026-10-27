//! The slotted page of a tree node (D4). A node fills a page's payload:
//!
//! | Bytes | Content |
//! |---|---|
//! | 0 | kind: `1` leaf, `2` branch |
//! | 1 | level: `0` for a leaf, one more than its children's for a branch |
//! | 2–3 | number of cells |
//! | 4–5 | start of the cells: they fill the end of the payload, from here to its end |
//! | 6–7 | zero |
//! | 8–15 | a branch's leftmost child; zero in a leaf |
//! | 16… | the offsets of the cells, 2 bytes each, in key order |
//!
//! The cells stay packed against the end of the payload, so the free space is the gap between the offsets and the
//! cells. A leaf cell is the key length (2 bytes), the value length (2 bytes), the key and the value. A value whose
//! cell would be longer than the leaf's threshold (half a node, D7 of the large values execution) lives off its leaf,
//! in a value run of its own pages: its cell has `0xFFFF` as the value length, and the run's first page and the
//! value's length (8 bytes each) in place of the value (D2). A branch cell is the key length (2 bytes), the key
//! and the child page (8 bytes): that child holds the keys from its key up to the next cell's key, and the leftmost
//! child the keys before the first cell's. Integers are little-endian (V3); keys compare byte by byte as unsigned
//! numbers, a prefix before its extensions (B5).

use std::cell::Cell;
use std::cmp::Ordering;

use krummi_tx::PAYLOAD_SIZE;

/// Bytes of the node header.
pub(crate) const HEADER: usize = 16;
/// Longest key a tree holds (A11, B5).
pub const MAX_KEY: usize = 2048;
/// Longest value a tree holds (V1 of the large values PRD).
pub const MAX_VALUE: usize = 268_435_456;
/// Longest leaf cell: half the room of a node, so that a leaf holds at least two (D4). It is the leaf's threshold
/// (D7): a value that would make its cell longer lives off its leaf.
pub(crate) const MAX_LEAF_CELL: usize = (PAYLOAD_SIZE - HEADER) / 2 - 2;
/// The value length of a leaf cell whose value lives off its leaf (D2).
const OFF_LEAF: usize = 0xFFFF;
/// Bytes that stand for a value off its leaf in its cell: the run's first page and the value's length.
const RUN_REF: usize = 16;
/// Longest branch cell.
pub(crate) const MAX_BRANCH_CELL: usize = 2 + MAX_KEY + 8;
/// Bytes a node uses under which it is merged with a sibling when they fit in one page (a quarter).
pub(crate) const UNDERFULL: usize = PAYLOAD_SIZE / 4;

const LEAF: u8 = 1;
const BRANCH: u8 = 2;

// At least two entries fit per leaf and three keys per branch (D4); an entry with the longest key always fits with
// its value off its leaf, and no value kept in its leaf has the length that marks one off it.
const _: () = assert!(HEADER + 2 * (MAX_LEAF_CELL + 2) <= PAYLOAD_SIZE);
const _: () = assert!(HEADER + 3 * (MAX_BRANCH_CELL + 2) <= PAYLOAD_SIZE);
const _: () = assert!(4 + MAX_KEY + RUN_REF <= MAX_LEAF_CELL);
const _: () = assert!(MAX_LEAF_CELL < OFF_LEAF);
const _: () = assert!(PAYLOAD_SIZE <= u16::MAX as usize);

thread_local! {
    /// The leaf's threshold for the operations of this thread (see [`set_inline_limit`]).
    static INLINE_LIMIT: Cell<usize> = const { Cell::new(MAX_LEAF_CELL) };
}

/// Sets, for the operations of this thread, the longest leaf cell that keeps its value in its leaf, or restores the
/// default, half a node, with `None`. A knob to evaluate other thresholds (V4 of the large values PRD, D7 of its
/// execution), not part of the engine's interface. Reading never depends on it: a cell of either kind is always
/// readable.
///
/// # Panics
///
/// If `limit` is longer than the default: a leaf must hold two of the longest cells (D4).
#[doc(hidden)]
pub fn set_inline_limit(limit: Option<usize>) {
    let limit = limit.unwrap_or(MAX_LEAF_CELL);
    assert!(
        limit <= MAX_LEAF_CELL,
        "an inline cell has up to {MAX_LEAF_CELL} bytes"
    );
    INLINE_LIMIT.with(|cell| cell.set(limit));
}

/// The longest leaf cell that keeps its value in its leaf, for the operations of this thread.
pub(crate) fn inline_limit() -> usize {
    INLINE_LIMIT.with(Cell::get)
}

/// Whether an entry with a key of `key` bytes keeps a value of `value` bytes in its leaf (D7).
pub(crate) fn fits_inline(key: usize, value: usize) -> bool {
    4 + key + value <= inline_limit()
}

/// The value of a leaf entry: its bytes, in the leaf, or where it lives off its leaf (D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Value<'a> {
    Inline(&'a [u8]),
    /// A value run from page `first` that holds the value's `len` bytes.
    Run {
        first: u64,
        len: u64,
    },
}

fn u16_at(node: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes([node[at], node[at + 1]]))
}

fn put_u16(node: &mut [u8], at: usize, value: usize) {
    let value = u16::try_from(value).expect("offsets fit in a page");
    node[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn u64_at(node: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(node[at..at + 8].try_into().expect("8 bytes"))
}

/// Makes `node` an empty node of `level`, a leaf at level `0`, with `leftmost` as a branch's first child.
pub(crate) fn init(node: &mut [u8], level: u8, leftmost: u64) {
    node[..HEADER].fill(0);
    node[0] = if level == 0 { LEAF } else { BRANCH };
    node[1] = level;
    put_u16(node, 4, PAYLOAD_SIZE);
    set_leftmost(node, leftmost);
}

/// Makes `node` a node of `level` holding `cells`, in order.
pub(crate) fn fill(node: &mut [u8], level: u8, leftmost: u64, cells: &[&[u8]]) {
    init(node, level, leftmost);
    for (index, cell) in cells.iter().enumerate() {
        insert(node, index, cell);
    }
}

/// Whether `node`, the payload of a page whose checksum is valid, has a consistent layout: a known kind, a leaf at
/// level `0` and a branch above it, the offsets within the header's count, and cells within their maximum sizes,
/// values off their leaf no longer than [`MAX_VALUE`], that fill the end of the payload exactly, without gaps or
/// overlaps. The accessors and the algorithms of this
/// crate stay within the page for such a node, so the tree code checks every node it reads (G6, A9).
pub(crate) fn is_valid(node: &[u8]) -> bool {
    let leaf = match (node[0], node[1]) {
        (LEAF, 0) => true,
        (BRANCH, 1..=254) => false,
        _ => return false,
    };
    let (count, top) = (count(node), top(node));
    if HEADER + 2 * count > top || top > PAYLOAD_SIZE {
        return false;
    }
    let (lengths, max) = if leaf {
        (4, MAX_LEAF_CELL)
    } else {
        (2, MAX_BRANCH_CELL)
    };
    // Bit `at` is set when a cell starts at byte `at`.
    let mut starts = [0u64; PAYLOAD_SIZE.div_ceil(64)];
    for index in 0..count {
        let at = offset(node, index);
        if at < top || at + lengths > PAYLOAD_SIZE {
            return false;
        }
        let len = cell_len_at(node, at);
        let key = u16_at(node, at);
        if at + len > PAYLOAD_SIZE
            || key > MAX_KEY
            || len > max
            || starts[at / 64] & (1 << (at % 64)) != 0
            || leaf
                && u16_at(node, at + 2) == OFF_LEAF
                && u64_at(node, at + len - 8) > MAX_VALUE as u64
        {
            return false;
        }
        starts[at / 64] |= 1 << (at % 64);
    }
    // The cells, followed from the first byte after the free space, end exactly at the end of the payload.
    let (mut at, mut seen) = (top, 0);
    while at < PAYLOAD_SIZE {
        if starts[at / 64] & (1 << (at % 64)) == 0 {
            return false;
        }
        at += cell_len_at(node, at);
        seen += 1;
    }
    seen == count
}

pub(crate) fn is_leaf(node: &[u8]) -> bool {
    node[0] == LEAF
}

pub(crate) fn level(node: &[u8]) -> u8 {
    node[1]
}

pub(crate) fn count(node: &[u8]) -> usize {
    u16_at(node, 2)
}

fn top(node: &[u8]) -> usize {
    u16_at(node, 4)
}

pub(crate) fn leftmost(node: &[u8]) -> u64 {
    u64_at(node, 8)
}

pub(crate) fn set_leftmost(node: &mut [u8], child: u64) {
    node[8..16].copy_from_slice(&child.to_le_bytes());
}

fn offset(node: &[u8], index: usize) -> usize {
    u16_at(node, HEADER + 2 * index)
}

/// Bytes free for new cells and their offsets.
pub(crate) fn free_space(node: &[u8]) -> usize {
    top(node) - HEADER - 2 * count(node)
}

/// Bytes in use, header included.
pub(crate) fn used(node: &[u8]) -> usize {
    PAYLOAD_SIZE - free_space(node)
}

/// Length of the cell at byte `at`.
fn cell_len_at(node: &[u8], at: usize) -> usize {
    let key = u16_at(node, at);
    if is_leaf(node) {
        4 + key + value_bytes(u16_at(node, at + 2))
    } else {
        2 + key + 8
    }
}

/// Bytes a leaf cell takes after its key, from its value length.
fn value_bytes(len: usize) -> usize {
    if len == OFF_LEAF { RUN_REF } else { len }
}

/// The bytes of cell `index`.
pub(crate) fn cell(node: &[u8], index: usize) -> &[u8] {
    let at = offset(node, index);
    &node[at..at + cell_len_at(node, at)]
}

/// The key of cell `index`.
pub(crate) fn key(node: &[u8], index: usize) -> &[u8] {
    let at = offset(node, index);
    let start = at + if is_leaf(node) { 4 } else { 2 };
    &node[start..start + u16_at(node, at)]
}

/// The value of entry `index` of a leaf.
pub(crate) fn value(node: &[u8], index: usize) -> Value<'_> {
    let at = offset(node, index);
    let start = at + 4 + u16_at(node, at);
    match u16_at(node, at + 2) {
        OFF_LEAF => Value::Run {
            first: u64_at(node, start),
            len: u64_at(node, start + 8),
        },
        len => Value::Inline(&node[start..start + len]),
    }
}

/// The value of entry `index` of a leaf, kept in its leaf, to change it in place.
pub(crate) fn value_mut(node: &mut [u8], index: usize) -> &mut [u8] {
    let at = offset(node, index);
    let start = at + 4 + u16_at(node, at);
    let len = u16_at(node, at + 2);
    debug_assert_ne!(len, OFF_LEAF, "a value in its leaf");
    &mut node[start..start + len]
}

/// The first of the entries `indexes` of a leaf whose value lives off its leaf: its index, its run's first page and
/// its length.
pub(crate) fn next_run(
    node: &[u8],
    mut indexes: std::ops::Range<usize>,
) -> Option<(usize, u64, u64)> {
    indexes.find_map(|index| match value(node, index) {
        Value::Run { first, len } => Some((index, first, len)),
        Value::Inline(_) => None,
    })
}

/// Child `index` of a branch, from `0` (the leftmost) to its number of cells.
pub(crate) fn child(node: &[u8], index: usize) -> u64 {
    if index == 0 {
        return leftmost(node);
    }
    let at = offset(node, index - 1);
    u64_at(node, at + 2 + u16_at(node, at))
}

/// The key of a leaf cell (`leaf`) or of a branch cell, from its bytes.
pub(crate) fn cell_key(cell: &[u8], leaf: bool) -> &[u8] {
    let start = if leaf { 4 } else { 2 };
    &cell[start..start + u16_at(cell, 0)]
}

/// The child of a branch cell, from its bytes.
pub(crate) fn cell_child(cell: &[u8]) -> u64 {
    u64_at(cell, cell.len() - 8)
}

/// The index of the cell with `key`, or where it would go.
pub(crate) fn search(node: &[u8], key: &[u8]) -> Result<usize, usize> {
    let (mut low, mut high) = (0, count(node));
    while low < high {
        let mid = low + (high - low) / 2;
        match self::key(node, mid).cmp(key) {
            Ordering::Less => low = mid + 1,
            Ordering::Greater => high = mid,
            Ordering::Equal => return Ok(mid),
        }
    }
    Err(low)
}

/// How many of the node's first keys satisfy `holds`, a test that is true up to some key and false after it.
pub(crate) fn partition(node: &[u8], mut holds: impl FnMut(&[u8]) -> bool) -> usize {
    let (mut low, mut high) = (0, count(node));
    while low < high {
        let mid = low + (high - low) / 2;
        if holds(self::key(node, mid)) {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    low
}

/// The child of a branch that holds `key`.
pub(crate) fn child_index(node: &[u8], key: &[u8]) -> usize {
    match search(node, key) {
        Ok(index) => index + 1,
        Err(index) => index,
    }
}

/// Writes a leaf cell into `buf` and returns its length.
pub(crate) fn encode_leaf(buf: &mut [u8], key: &[u8], value: Value<'_>) -> usize {
    put_u16(buf, 0, key.len());
    buf[4..4 + key.len()].copy_from_slice(key);
    let start = 4 + key.len();
    match value {
        Value::Inline(value) => {
            put_u16(buf, 2, value.len());
            buf[start..start + value.len()].copy_from_slice(value);
            start + value.len()
        }
        Value::Run { first, len } => {
            put_u16(buf, 2, OFF_LEAF);
            buf[start..start + 8].copy_from_slice(&first.to_le_bytes());
            buf[start + 8..start + RUN_REF].copy_from_slice(&len.to_le_bytes());
            start + RUN_REF
        }
    }
}

/// Writes a branch cell into `buf` and returns its length.
pub(crate) fn encode_branch(buf: &mut [u8], key: &[u8], child: u64) -> usize {
    put_u16(buf, 0, key.len());
    buf[2..2 + key.len()].copy_from_slice(key);
    let end = 2 + key.len() + 8;
    buf[2 + key.len()..end].copy_from_slice(&child.to_le_bytes());
    end
}

/// Inserts `cell` as cell `index`.
///
/// # Panics
///
/// If it does not fit.
pub(crate) fn insert(node: &mut [u8], index: usize, cell: &[u8]) {
    assert!(free_space(node) >= cell.len() + 2, "the cell fits");
    let count = count(node);
    let top = top(node) - cell.len();
    node[top..top + cell.len()].copy_from_slice(cell);
    let slot = HEADER + 2 * index;
    node.copy_within(slot..HEADER + 2 * count, slot + 2);
    put_u16(node, slot, top);
    put_u16(node, 2, count + 1);
    put_u16(node, 4, top);
}

/// Removes cell `index`, moving the cells below it to keep them packed.
pub(crate) fn remove(node: &mut [u8], index: usize) {
    let count = count(node);
    let at = offset(node, index);
    let len = cell_len_at(node, at);
    let top = top(node);
    node.copy_within(top..at, top + len);
    for other in 0..count {
        let slot = HEADER + 2 * other;
        let moved = u16_at(node, slot);
        if moved < at {
            put_u16(node, slot, moved + len);
        }
    }
    let slot = HEADER + 2 * index;
    node.copy_within(slot + 2..HEADER + 2 * count, slot);
    put_u16(node, 2, count - 1);
    put_u16(node, 4, top + len);
}

/// Where to split `cells`, whose sizes with their offsets add up to more than a page, into two nodes with as
/// equal a number of bytes as possible: a leaf keeps the cells before the index and its new sibling the rest; a
/// branch also sends the cell at the index up to its parent, with its child as the sibling's leftmost.
pub(crate) fn split_point(cells: &[&[u8]], leaf: bool) -> usize {
    let total: usize = cells.iter().map(|c| c.len() + 2).sum();
    let (range, up) = if leaf {
        (1..cells.len(), false)
    } else {
        (1..cells.len() - 1, true)
    };
    let mut best = (usize::MAX, 0);
    let mut before: usize = cells[..range.start].iter().map(|c| c.len() + 2).sum();
    for index in range {
        let moved = if up { cells[index].len() + 2 } else { 0 };
        let larger = before.max(total - before - moved);
        if larger < best.0 {
            best = (larger, index);
        }
        before += cells[index].len() + 2;
    }
    assert!(best.0 <= PAYLOAD_SIZE - HEADER, "both halves fit");
    best.1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf_with(entries: &[(&[u8], &[u8])]) -> Vec<u8> {
        let mut node = vec![0; PAYLOAD_SIZE];
        init(&mut node, 0, 0);
        let mut buf = [0; MAX_LEAF_CELL];
        for (index, (key, value)) in entries.iter().enumerate() {
            let len = encode_leaf(&mut buf, key, Value::Inline(value));
            insert(&mut node, index, &buf[..len]);
        }
        node
    }

    #[test]
    fn cells_are_found_in_key_order_and_removal_keeps_them_packed() {
        let mut node = leaf_with(&[(b"a", b"1"), (b"ab", b""), (b"b", b"333")]);
        assert_eq!(search(&node, b"ab"), Ok(1));
        assert_eq!(search(&node, b"aa"), Err(1));
        assert_eq!(search(&node, b"c"), Err(3));
        assert_eq!(
            (value(&node, 0), value(&node, 1)),
            (Value::Inline(b"1"), Value::Inline(b""))
        );
        let used_before = used(&node);
        remove(&mut node, 0);
        assert_eq!(used(&node), used_before - (4 + 1 + 1) - 2);
        assert_eq!((key(&node, 0), key(&node, 1)), (&b"ab"[..], &b"b"[..]));
        assert_eq!(value(&node, 1), Value::Inline(b"333"));
        assert_eq!(top(&node), PAYLOAD_SIZE - (4 + 2) - (4 + 1 + 3));
    }

    #[test]
    fn branch_children_follow_their_keys() {
        let mut node = vec![0; PAYLOAD_SIZE];
        init(&mut node, 1, 10);
        let mut buf = [0; MAX_BRANCH_CELL];
        let len = encode_branch(&mut buf, b"m", 20);
        insert(&mut node, 0, &buf[..len]);
        assert_eq!(
            [b"a", b"m", b"z"].map(|k| child(&node, child_index(&node, k))),
            [10, 20, 20]
        );
        assert_eq!(cell_child(cell(&node, 0)), 20);
        assert_eq!(cell_key(cell(&node, 0), false), b"m");
    }

    #[test]
    fn split_points_balance_bytes() {
        let small = [1u8; 10];
        let big = [1u8; 3000];
        let cells: Vec<&[u8]> = vec![&small, &small, &big, &small];
        assert_eq!(split_point(&cells, true), 2);
        assert_eq!(split_point(&cells, false), 2);
    }
}
