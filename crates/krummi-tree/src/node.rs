//! The slotted page of a tree node (D4). A node fills a page's payload:
//!
//! | Bytes | Content |
//! |---|---|
//! | 0 | kind: `1` leaf, `2` branch |
//! | 1 | level: `0` for a leaf, one more than its children's for a branch |
//! | 2–3 | number of cells |
//! | 4–5 | start of the cells: the cells and the holes fill the end of the payload, from here to its end |
//! | 6–7 | bytes of the holes: what removed cells left among the others |
//! | 8–15 | a branch's leftmost child; zero in a leaf |
//! | 16… | the offsets of the cells, 2 bytes each, in key order |
//!
//! Removing a cell, or replacing it with one of another size, leaves its bytes as a hole, without moving the other
//! cells, so that a commit changes few bytes of the page (Y4 of the journal page diffs PRD). The free space is the gap
//! between the offsets and the cells and the holes; a cell that does not fit in the gap moves the cells against the
//! end of the payload first, which joins the holes to the gap. A leaf cell is the key length (2 bytes), the value length (2 bytes), the key and the value. A value whose
//! cell would be longer than the leaf's threshold (half a node, D7 of the large values execution) lives off its leaf,
//! in a value run of its own pages: its cell has `0xFFFF` as the value length, and the run's first page and the
//! value's length (8 bytes each) in place of the value (D2). A branch cell is the key length (2 bytes), the key
//! and the child page (8 bytes): that child holds the keys from its key up to the next cell's key, and the leftmost
//! child the keys before the first cell's. Integers are little-endian (V3); keys compare byte by byte as unsigned
//! numbers, a prefix before its extensions (B5).

use std::cmp::Ordering;

use krummi_tx::PAYLOAD_SIZE;

/// Bytes of the node header.
pub(crate) const HEADER: usize = 16;
/// Longest key a tree holds (A11, B5).
pub const MAX_KEY: usize = 2048;
/// Longest value a tree holds (V1 of the large values PRD).
pub const MAX_VALUE: usize = 268_435_456;
/// Longest leaf cell: half the room of a node, so that a leaf holds at least two (D4). It is the leaf's threshold
/// (D7), the one the evaluation of V4 of the large values PRD chose over a full page: a value that would make its
/// cell longer lives off its leaf.
pub(crate) const MAX_LEAF_CELL: usize = (PAYLOAD_SIZE - HEADER) / 2 - 2;
/// Bytes of a leaf cell before its key: the key length and the value length, 2 bytes each.
pub(crate) const LEAF_CELL_HEADER: usize = 4;
/// The value length of a leaf cell whose value lives off its leaf (D2).
const OFF_LEAF: usize = 0xFFFF;
/// Bytes that stand for a value off its leaf in its cell: the run's first page and the value's length.
const RUN_REF: usize = 16;
/// Longest branch cell.
pub(crate) const MAX_BRANCH_CELL: usize = 2 + MAX_KEY + 8;
/// Bytes a node uses under which it is merged with a sibling when they fit in one page (a quarter).
pub(crate) const UNDERFULL: usize = PAYLOAD_SIZE / 4;

pub(crate) const LEAF: u8 = 1;
pub(crate) const BRANCH: u8 = 2;

// At least two entries fit per leaf and three keys per branch (D4); an entry with the longest key always fits with
// its value off its leaf, and no value kept in its leaf has the length that marks one off it.
const _: () = assert!(HEADER + 2 * (MAX_LEAF_CELL + 2) <= PAYLOAD_SIZE);
const _: () = assert!(HEADER + 3 * (MAX_BRANCH_CELL + 2) <= PAYLOAD_SIZE);
const _: () = assert!(LEAF_CELL_HEADER + MAX_KEY + RUN_REF <= MAX_LEAF_CELL);
const _: () = assert!(MAX_LEAF_CELL < OFF_LEAF);
const _: () = assert!(PAYLOAD_SIZE <= u16::MAX as usize);

/// Whether an entry with a key of `key` bytes keeps a value of `value` bytes in its leaf (D7).
pub(crate) fn fits_inline(key: usize, value: usize) -> bool {
    LEAF_CELL_HEADER + key + value <= MAX_LEAF_CELL
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

pub(crate) fn u16_at(node: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes([node[at], node[at + 1]]))
}

pub(crate) fn put_u16(node: &mut [u8], at: usize, value: usize) {
    let value = u16::try_from(value).expect("offsets fit in a page");
    node[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

pub(crate) fn u64_at(node: &[u8], at: usize) -> u64 {
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
/// values off their leaf of 1 to [`MAX_VALUE`] bytes, that fill the end of the payload exactly, without gaps or
/// overlaps. The accessors and the algorithms of this crate stay within the page for such a node, so the tree code
/// checks every node it reads (G6, A9).
pub(crate) fn is_valid(node: &[u8]) -> bool {
    let node: &[u8; PAYLOAD_SIZE] = node[..PAYLOAD_SIZE].try_into().expect("a payload");
    match (node[0], node[1]) {
        (LEAF, 0) => cells_are_valid::<true>(node),
        (BRANCH, 1..=254) => cells_are_valid::<false>(node),
        _ => false,
    }
}

/// The checks of [`is_valid`] after the kind and the level, for a leaf when `LEAF`. Each cell lies after the start
/// of the cells and within its sizes, no two share a byte, and the cells and the holes the header counts fill the
/// end of the payload exactly (Y4 of the journal page diffs PRD). Without holes, that is cells that tile the end of the
/// payload, which one walk from `top` checks; only a node with holes pays for a map of the bytes its cells cover.
fn cells_are_valid<const LEAF: bool>(node: &[u8; PAYLOAD_SIZE]) -> bool {
    let u16_at = |at: usize| usize::from(u16::from_le_bytes([node[at], node[at + 1]]));
    let (count, top, holes) = (u16_at(2), u16_at(4), u16_at(6));
    // A node without cells has no holes either.
    if HEADER + 2 * count > top || top > PAYLOAD_SIZE || count == 0 && holes != 0 {
        return false;
    }
    if holes == 0 {
        cells_tile::<LEAF>(node, count, top)
    } else {
        cells_cover::<LEAF>(node, count, top, holes)
    }
}

/// The length of the cell at `at` when it lies within the payload and its sizes, a leaf's when `LEAF`.
fn checked_cell_len<const LEAF: bool>(node: &[u8; PAYLOAD_SIZE], at: usize) -> Option<usize> {
    let u16_at = |at: usize| usize::from(u16::from_le_bytes([node[at], node[at + 1]]));
    let (lengths, max) = if LEAF {
        (LEAF_CELL_HEADER, MAX_LEAF_CELL)
    } else {
        (2, MAX_BRANCH_CELL)
    };
    if at + lengths > PAYLOAD_SIZE {
        return None;
    }
    let key = u16_at(at);
    let len = if LEAF {
        LEAF_CELL_HEADER + key + value_bytes(u16_at(at + 2))
    } else {
        2 + key + 8
    };
    let fits = at + len <= PAYLOAD_SIZE
        && key <= MAX_KEY
        && len <= max
        && !(LEAF
            && u16_at(at + 2) == OFF_LEAF
            && !(1..=MAX_VALUE as u64).contains(&u64_at(node, at + len - 8)));
    fits.then_some(len)
}

/// A node without holes: the cells, followed from `top`, end exactly at the end of the payload, `count` of them, and
/// the offsets are their starts, each once.
fn cells_tile<const LEAF: bool>(node: &[u8; PAYLOAD_SIZE], count: usize, top: usize) -> bool {
    let u16_at = |at: usize| usize::from(u16::from_le_bytes([node[at], node[at + 1]]));
    // Bit `at` is set when a cell starts at byte `at`.
    let mut starts = [0u64; PAYLOAD_SIZE.div_ceil(64)];
    let (mut at, mut seen) = (top, 0);
    while at < PAYLOAD_SIZE {
        let Some(len) = checked_cell_len::<LEAF>(node, at).filter(|_| seen < count) else {
            return false;
        };
        starts[at / 64] |= 1 << (at % 64);
        at += len;
        seen += 1;
    }
    if seen != count {
        return false;
    }
    // An offset that is no start, or a repeated one, finds its bit clear.
    for index in 0..count {
        let at = u16_at(HEADER + 2 * index);
        if at >= PAYLOAD_SIZE || starts[at / 64] & (1 << (at % 64)) == 0 {
            return false;
        }
        starts[at / 64] &= !(1 << (at % 64));
    }
    true
}

/// A node with holes: each cell lies after `top` and within its sizes, no two share a byte, and the cells and the
/// `holes` fill the end of the payload exactly.
fn cells_cover<const LEAF: bool>(
    node: &[u8; PAYLOAD_SIZE],
    count: usize,
    top: usize,
    holes: usize,
) -> bool {
    let u16_at = |at: usize| usize::from(u16::from_le_bytes([node[at], node[at + 1]]));
    // Bit `at` is set when a cell holds byte `at`.
    let mut covered = [0u64; PAYLOAD_SIZE.div_ceil(64)];
    let mut cells = 0;
    for index in 0..count {
        let at = u16_at(HEADER + 2 * index);
        let Some(len) = checked_cell_len::<LEAF>(node, at).filter(|_| at >= top) else {
            return false;
        };
        let mut from = at;
        while from < at + len {
            let (word, bit) = (from / 64, from % 64);
            let bits = (64 - bit).min(at + len - from);
            let mask = (u64::MAX >> (64 - bits)) << bit;
            if covered[word] & mask != 0 {
                return false;
            }
            covered[word] |= mask;
            from += bits;
        }
        cells += len;
    }
    cells + holes == PAYLOAD_SIZE - top
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

/// Bytes free for new cells and their offsets: the gap between the offsets and the cells, and the holes.
pub(crate) fn free_space(node: &[u8]) -> usize {
    gap(node) + holes(node)
}

/// Bytes between the offsets and the cells.
fn gap(node: &[u8]) -> usize {
    top(node) - HEADER - 2 * count(node)
}

/// Bytes of the holes among the cells.
fn holes(node: &[u8]) -> usize {
    u16_at(node, 6)
}

/// Bytes in use, header included.
pub(crate) fn used(node: &[u8]) -> usize {
    PAYLOAD_SIZE - free_space(node)
}

/// Length of the cell at byte `at`.
fn cell_len_at(node: &[u8], at: usize) -> usize {
    let key = u16_at(node, at);
    if is_leaf(node) {
        LEAF_CELL_HEADER + key + value_bytes(u16_at(node, at + 2))
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
    let start = at + if is_leaf(node) { LEAF_CELL_HEADER } else { 2 };
    &node[start..start + u16_at(node, at)]
}

/// The value of entry `index` of a leaf.
pub(crate) fn value(node: &[u8], index: usize) -> Value<'_> {
    let at = offset(node, index);
    let start = at + LEAF_CELL_HEADER + u16_at(node, at);
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
    let start = at + LEAF_CELL_HEADER + u16_at(node, at);
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
    let start = if leaf { LEAF_CELL_HEADER } else { 2 };
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
    let start = LEAF_CELL_HEADER + key.len();
    buf[LEAF_CELL_HEADER..start].copy_from_slice(key);
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
    if gap(node) < cell.len() + 2 {
        compact(node);
    }
    let count = count(node);
    let top = top(node) - cell.len();
    node[top..top + cell.len()].copy_from_slice(cell);
    let slot = HEADER + 2 * index;
    node.copy_within(slot..HEADER + 2 * count, slot + 2);
    put_u16(node, slot, top);
    put_u16(node, 2, count + 1);
    put_u16(node, 4, top);
}

/// Removes cell `index`, leaving its bytes as a hole: the lowest cell joins the gap instead, and a node left
/// without cells has no holes (Y4 of the journal page diffs PRD).
pub(crate) fn remove(node: &mut [u8], index: usize) {
    let count = count(node);
    let at = offset(node, index);
    let len = cell_len_at(node, at);
    let slot = HEADER + 2 * index;
    node.copy_within(slot + 2..HEADER + 2 * count, slot);
    put_u16(node, 2, count - 1);
    if count == 1 {
        put_u16(node, 4, PAYLOAD_SIZE);
        put_u16(node, 6, 0);
    } else if at == top(node) {
        put_u16(node, 4, at + len);
    } else {
        put_u16(node, 6, holes(node) + len);
    }
    #[cfg(test)]
    if testing::COMPACT_ON_REMOVE.get() {
        compact(node);
    }
}

/// What the tests of holes switch and count (AC-Y5 of the journal page diffs PRD).
#[cfg(test)]
pub(crate) mod testing {
    use std::cell::Cell;

    thread_local! {
        /// Moves the cells against the end of the payload after every removal, as before holes existed.
        pub(crate) static COMPACT_ON_REMOVE: Cell<bool> = const { Cell::new(false) };
        /// Times the cells were moved against the end of the payload on this thread.
        pub(crate) static COMPACTIONS: Cell<usize> = const { Cell::new(0) };
    }
}

/// Moves the cells against the end of the payload, in key order, so that the holes join the gap.
fn compact(node: &mut [u8]) {
    #[cfg(test)]
    testing::COMPACTIONS.set(testing::COMPACTIONS.get() + 1);
    let old: [u8; PAYLOAD_SIZE] = node[..PAYLOAD_SIZE].try_into().expect("a payload");
    let mut top = PAYLOAD_SIZE;
    for index in 0..count(node) {
        let at = offset(&old, index);
        let len = cell_len_at(&old, at);
        top -= len;
        node[top..top + len].copy_from_slice(&old[at..at + len]);
        put_u16(node, HEADER + 2 * index, top);
    }
    put_u16(node, 4, top);
    put_u16(node, 6, 0);
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
    fn cells_are_found_in_key_order_and_removal_leaves_a_hole() {
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
        // The first cell, the highest in the page, left a hole; the others did not move.
        assert_eq!(
            top(&node),
            PAYLOAD_SIZE - (4 + 1 + 1) - (4 + 2) - (4 + 1 + 3)
        );
        assert_eq!(holes(&node), 4 + 1 + 1);
        assert!(is_valid(&node));
    }

    /// The check of [`is_valid`] as it was before it followed the cells first (W1, performance plan): the
    /// reference that the faster one must agree with on every node.
    fn is_valid_reference(node: &[u8]) -> bool {
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
            (LEAF_CELL_HEADER, MAX_LEAF_CELL)
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
                    && !(1..=MAX_VALUE as u64).contains(&u64_at(node, at + len - 8))
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

    /// [`is_valid`] through the map of covered bytes of a node with holes, for a node whose header counts none.
    fn is_valid_by_coverage(node: &[u8]) -> bool {
        let node: &[u8; PAYLOAD_SIZE] = node.try_into().expect("a payload");
        let (count, top) = (count(node), top(node));
        if HEADER + 2 * count > top || top > PAYLOAD_SIZE {
            return false;
        }
        match (node[0], node[1]) {
            (LEAF, 0) => cells_cover::<true>(node, count, top, 0),
            (BRANCH, 1..=254) => cells_cover::<false>(node, count, top, 0),
            _ => false,
        }
    }

    /// A node built from valid cells, then damaged at random in its header, its offsets or its cells, is accepted
    /// by [`is_valid`] exactly when the reference accepts it.
    #[test]
    fn the_layout_check_agrees_with_its_reference_on_damaged_nodes() {
        let mut state = 0x6B72_756D_6D69_0001u64;
        let mut next = move |bound: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % bound as u64) as usize
        };
        let mut bases = Vec::new();
        for (key_len, value_len) in [(16, 100), (2, 0), (40, 300)] {
            let mut node = vec![0; PAYLOAD_SIZE];
            init(&mut node, 0, 0);
            let mut buf = [0; MAX_LEAF_CELL];
            for i in 0u64..100_000 {
                let key: Vec<u8> = (0..key_len)
                    .map(|b| (i >> (8 * (b % 8))) as u8 ^ b as u8)
                    .collect();
                let value = if i % 5 == 4 {
                    Value::Run {
                        first: 100 + i,
                        len: 5_000,
                    }
                } else {
                    Value::Inline(&[7; 300][..value_len])
                };
                let len = encode_leaf(&mut buf, &key, value);
                if free_space(&node) < len + 2 {
                    break;
                }
                let index = match search(&node, &key) {
                    Ok(_) => continue,
                    Err(index) => index,
                };
                insert(&mut node, index, &buf[..len]);
            }
            bases.push(node);
        }
        let mut branch = vec![0; PAYLOAD_SIZE];
        init(&mut branch, 3, 9);
        let mut buf = [0; MAX_BRANCH_CELL];
        for i in 0..200u64 {
            let len = encode_branch(&mut buf, &i.to_be_bytes(), 1_000 + i);
            insert(&mut branch, i as usize, &buf[..len]);
        }
        bases.push(branch);
        // Any 16 bits, unlike `put_u16`, which only takes the values a valid node holds.
        let put = |node: &mut [u8], at: usize, value: usize| {
            node[at..at + 2].copy_from_slice(&(value as u16).to_le_bytes());
        };
        let (mut accepted, mut rejected) = (0, 0);
        for round in 0..60_000 {
            let mut node = bases[round % bases.len()].clone();
            assert!(is_valid(&node) && is_valid_reference(&node));
            for _ in 0..1 + next(3) {
                // A damaged count may point past the payload; the offsets damaged are within it.
                let count = count(&node).min((PAYLOAD_SIZE - HEADER) / 2);
                match next(7) {
                    // A byte of the header: kind, level, count, top.
                    0 => node[next(HEADER)] = next(256) as u8,
                    // An offset set to another one, or to any value.
                    1 if count > 1 => {
                        let other = offset(&node, next(count));
                        put(&mut node, HEADER + 2 * next(count), other);
                    }
                    2 if count > 0 => {
                        put(&mut node, HEADER + 2 * next(count), next(PAYLOAD_SIZE + 64))
                    }
                    // A byte of a cell: its lengths, its key, its value or its run.
                    3 | 4 => {
                        let at = top(&node).min(PAYLOAD_SIZE - 1);
                        node[at + next(PAYLOAD_SIZE - at)] = next(256) as u8;
                    }
                    // The count or the top moved by a little.
                    5 => put(&mut node, 2, (count + next(3)).saturating_sub(1)),
                    _ => {
                        let top = (top(&node) + next(9)).saturating_sub(4);
                        put(&mut node, 4, top);
                    }
                }
            }
            // The reference knew no holes: a node whose header counts some is checked by the tests of holes.
            if holes(&node) != 0 {
                continue;
            }
            let expected = is_valid_reference(&node);
            assert_eq!(is_valid(&node), expected, "round {round}");
            // The map of covered bytes, which only nodes with holes take, agrees on nodes without them too.
            assert_eq!(is_valid_by_coverage(&node), expected, "round {round}");
            if expected {
                accepted += 1
            } else {
                rejected += 1
            }
        }
        assert!(
            accepted > 1_000 && rejected > 1_000,
            "{accepted} accepted, {rejected} rejected"
        );
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

    /// Y4: a leaf that takes inserts, removals and replacements with cells of another size keeps its holes, joins
    /// them to the gap only when a cell does not fit without them, always passes the layout check, holds what a model
    /// holds, and uses exactly the bytes of its cells and offsets.
    #[test]
    fn holes_follow_a_model_of_inserts_removals_and_replacements() {
        let mut state = 0x6B72_756D_6D69_0004u64;
        let mut next = move |bound: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % bound as u64) as usize
        };
        let mut node = vec![0; PAYLOAD_SIZE];
        init(&mut node, 0, 0);
        let mut model = std::collections::BTreeMap::<Vec<u8>, Vec<u8>>::new();
        let mut buf = [0; MAX_LEAF_CELL];
        let (mut compactions, mut with_holes) = (0, 0);
        for step in 0..20_000 {
            let key = format!("{:05}", next(200)).into_bytes();
            let value = vec![step as u8; next(300)];
            let len = encode_leaf(&mut buf, &key, Value::Inline(&value));
            match (search(&node, &key), next(3)) {
                (Ok(index), 0) => {
                    remove(&mut node, index);
                    model.remove(&key);
                }
                (Ok(index), _) => {
                    // A replacement of another size, as the tree does it: remove, then insert if it fits.
                    remove(&mut node, index);
                    model.remove(&key);
                    if free_space(&node) >= len + 2 {
                        let had_holes = holes(&node) > 0 && gap(&node) < len + 2;
                        insert(&mut node, index, &buf[..len]);
                        model.insert(key, value);
                        compactions += usize::from(had_holes);
                    }
                }
                (Err(index), _) if free_space(&node) >= len + 2 => {
                    let had_holes = holes(&node) > 0 && gap(&node) < len + 2;
                    insert(&mut node, index, &buf[..len]);
                    model.insert(key, value);
                    compactions += usize::from(had_holes);
                }
                (Err(_), _) => {}
            }
            with_holes += usize::from(holes(&node) > 0);
            assert!(is_valid(&node), "step {step}");
            assert_eq!(count(&node), model.len(), "step {step}");
            let cells: usize = (0..count(&node)).map(|i| cell(&node, i).len() + 2).sum();
            assert_eq!(used(&node), HEADER + cells, "step {step}");
            for (index, (key, value)) in model.iter().enumerate() {
                assert_eq!(super::key(&node, index), &key[..], "step {step}");
                assert_eq!(
                    super::value(&node, index),
                    Value::Inline(value),
                    "step {step}"
                );
            }
        }
        assert!(
            compactions > 10 && with_holes > 1_000,
            "{compactions} compactions, {with_holes} steps with holes"
        );
    }

    /// AC-Y6 (Y4): a node with holes passes the layout check; cells that overlap, a cell out of the page or over its
    /// sizes, and a count of holes that does not match its cells, do not.
    #[test]
    fn the_layout_check_accepts_holes_and_rejects_overlaps() {
        let mut node = leaf_with(&[(b"a", b"11"), (b"b", b"22"), (b"c", b"33"), (b"d", b"44")]);
        remove(&mut node, 1);
        assert!(holes(&node) > 0 && is_valid(&node), "a hole");
        let damaged = |change: &dyn Fn(&mut Vec<u8>)| {
            let mut copy = node.clone();
            change(&mut copy);
            is_valid(&copy)
        };
        // The second cell made to start inside the first one.
        assert!(!damaged(&|n| {
            let first = offset(n, 0);
            put_u16(n, HEADER + 2, first + 1);
        }));
        // Two offsets of the same cell.
        assert!(!damaged(&|n| {
            let first = offset(n, 0);
            put_u16(n, HEADER + 2, first);
        }));
        // A cell that runs past the end of the payload.
        assert!(!damaged(&|n| put_u16(n, HEADER, PAYLOAD_SIZE - 3)));
        // A key over its maximum size.
        assert!(!damaged(&|n| {
            let at = offset(n, 0);
            put_u16(n, at, MAX_KEY + 1);
        }));
        // A node without cells that counts holes.
        let mut empty = vec![0; PAYLOAD_SIZE];
        init(&mut empty, 0, 0);
        put_u16(&mut empty, 4, PAYLOAD_SIZE - 10);
        put_u16(&mut empty, 6, 10);
        assert!(!is_valid(&empty));
        // A count of holes one byte off, either way.
        assert!(!damaged(&|n| {
            let holes = holes(n);
            put_u16(n, 6, holes + 1);
        }));
        assert!(!damaged(&|n| {
            let holes = holes(n);
            put_u16(n, 6, holes - 1);
        }));
    }
}
