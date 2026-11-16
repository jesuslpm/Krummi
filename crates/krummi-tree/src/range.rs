//! Intervals of keys and double-ended scans over them (D2 and D3 of the in-order scan execution).
//!
//! An [`Interval`] is a lower and an upper bound, each included, excluded or open, and a prefix (empty for none).
//! Its keys are the valid keys between the bounds that start with the prefix. It is tested on keys with two
//! predicates that are monotonic in key order: [`Interval::after_lower`] is false and then true, and
//! [`Interval::before_upper`] true and then false. The prefix folds into them: a key at or after the prefix is
//! after it, and a key before the prefix or starting with it is before its end. So a descent can find where an
//! interval starts or ends with a binary search, and no key needs a separate prefix test.
//!
//! A [`Range`] scans an interval from both ends. Each end keeps the path from the root to its leaf, the page and
//! the child or entry index at every level, and finds its first entry on its first step, so a range reads no page
//! until it is used. An end stops at the bound, or before the other end's last entry, so that each entry comes out
//! once. Every node is read with the checks of the tree code (damage with its page number, G6), and a value off its
//! leaf is read from its run when its entry comes out (V2, V7). After any failure both ends stop.

use std::ops::Bound::{self, Excluded, Included, Unbounded};

use krummi_tx::Result;

use crate::node::Value;
use crate::{MAX_KEY, PageSource, child_level, copy, node, node_at, reserve, resolve};

/// An interval of keys, borrowed: the bounds and the prefix of a scan or a removal.
#[derive(Debug, Clone, Copy)]
pub struct Interval<'a> {
    /// The keys must start with it; empty for no prefix.
    pub prefix: &'a [u8],
    pub lower: Bound<&'a [u8]>,
    pub upper: Bound<&'a [u8]>,
}

impl Interval<'_> {
    /// Whether `key` is not before the lower bound and not before the prefix.
    pub fn after_lower(&self, key: &[u8]) -> bool {
        let bound = match self.lower {
            Included(lower) => key >= lower,
            Excluded(lower) => key > lower,
            Unbounded => true,
        };
        // Every key is at or after the empty prefix; testing it first saves a comparison per step of a scan.
        bound && (self.prefix.is_empty() || key >= self.prefix)
    }

    /// Whether `key` is not after the upper bound and not after every key that starts with the prefix.
    pub fn before_upper(&self, key: &[u8]) -> bool {
        let bound = match self.upper {
            Included(upper) => key <= upper,
            Excluded(upper) => key < upper,
            Unbounded => true,
        };
        bound && (self.prefix.is_empty() || key < self.prefix || key.starts_with(self.prefix))
    }

    /// Whether every key before `key` is before the lower bound or before the prefix. It may say no when no key
    /// in between exists, which only costs a page read.
    pub fn all_before_lower(&self, key: &[u8]) -> bool {
        let bound = match self.lower {
            Included(lower) | Excluded(lower) => lower >= key,
            Unbounded => false,
        };
        bound || self.prefix >= key
    }

    /// Whether `item` is in the interval: between the bounds and starting with the prefix.
    pub fn contains(&self, item: &[u8]) -> bool {
        self.after_lower(item) && self.before_upper(item)
    }

    /// Whether no valid key (1 to [`MAX_KEY`] bytes) that starts with the prefix lies between the bounds (R3). It
    /// is decided from the arguments alone: the smallest such key after the lower bound is compared with the upper
    /// bound. The bounds and the prefix must be valid sizes.
    pub fn is_empty(&self) -> bool {
        self.is_empty_from(&[0])
    }

    /// Whether no valid value of a multi-value tree (0 to [`MAX_KEY`] bytes, the empty value the least, D4 of the
    /// multi-value trees execution) that starts with the prefix lies between the bounds (R3, M7), decided as
    /// [`Interval::is_empty`] does. The bounds may be the empty value.
    pub fn is_empty_of_values(&self) -> bool {
        self.is_empty_from(&[])
    }

    /// Whether the interval is empty among the items from `least`, the smallest valid one, up to [`MAX_KEY`]
    /// bytes, ordered byte by byte.
    fn is_empty_from(&self, least: &[u8]) -> bool {
        /// The smallest candidate key.
        enum Least<'k> {
            /// This key.
            Key(&'k [u8]),
            /// The key right after this one among the valid keys.
            After(&'k [u8]),
        }
        let least = match self.lower {
            Unbounded if self.prefix.is_empty() => Least::Key(least),
            Unbounded => Least::Key(self.prefix),
            Included(lower) | Excluded(lower) if lower.starts_with(self.prefix) => {
                if matches!(self.lower, Included(_)) {
                    Least::Key(lower)
                } else if lower.len() < MAX_KEY
                    || lower[self.prefix.len()..].iter().any(|&b| b != 0xFF)
                {
                    // Its successor still starts with the prefix.
                    Least::After(lower)
                } else {
                    return true;
                }
            }
            Included(lower) | Excluded(lower) if lower < self.prefix => Least::Key(self.prefix),
            Included(_) | Excluded(_) => return true,
        };
        match (least, self.upper) {
            (_, Unbounded) => false,
            (Least::Key(key), Included(upper)) => key > upper,
            (Least::Key(key), Excluded(upper)) => key >= upper,
            // No valid key lies between a key and its successor.
            (Least::After(key), Included(upper)) => upper <= key,
            (Least::After(key), Excluded(upper)) => upper <= key || is_successor(key, upper),
        }
    }
}

/// Whether `next` is the valid key right after `key`: `key` with a zero byte added, or, when `key` has the most
/// bytes a key may have, `key` without its trailing `0xFF` bytes and its last byte incremented.
fn is_successor(key: &[u8], next: &[u8]) -> bool {
    if key.len() < MAX_KEY {
        return next.len() == key.len() + 1 && next.starts_with(key) && next[key.len()] == 0;
    }
    match key.iter().rposition(|&b| b != 0xFF) {
        Some(at) => next.len() == at + 1 && next[..at] == key[..at] && next[at] == key[at] + 1,
        None => false,
    }
}

/// An [`Interval`] copied, so that a scan does not borrow its caller's bounds.
pub(crate) struct OwnedInterval {
    prefix: Vec<u8>,
    lower: Bound<Vec<u8>>,
    upper: Bound<Vec<u8>>,
}

impl OwnedInterval {
    /// A copy of `interval`, reserved as [`reserve`] does.
    pub(crate) fn new<S: PageSource + ?Sized>(tx: &S, interval: Interval<'_>) -> Result<Self> {
        let owned = |bound: Bound<&[u8]>| -> Result<Bound<Vec<u8>>> {
            Ok(match bound {
                Included(key) => Included(copy(tx, key)?),
                Excluded(key) => Excluded(copy(tx, key)?),
                Unbounded => Unbounded,
            })
        };
        Ok(OwnedInterval {
            prefix: copy(tx, interval.prefix)?,
            lower: owned(interval.lower)?,
            upper: owned(interval.upper)?,
        })
    }

    pub(crate) fn borrow(&self) -> Interval<'_> {
        Interval {
            prefix: &self.prefix,
            lower: self.lower.as_ref().map(Vec::as_slice),
            upper: self.upper.as_ref().map(Vec::as_slice),
        }
    }
}

/// One end of a [`Range`]: its leaf, where it is in it, and the branches from the root down to that leaf.
struct End<'t> {
    /// The branches from the root down to the leaf, each with the index of the child the end went down to.
    path: Vec<(&'t [u8], usize)>,
    /// The leaf the end reads; empty until its first step.
    leaf: &'t [u8],
    /// In the leaf, the front's next entry or the back's entry after its next one.
    index: usize,
    /// Where the quick steps of [`Range::next`] and [`Range::next_back`] stop, which test nothing but the index:
    /// the front returns the entries before it, the back those from it. It is the whole leaf when the leaf is clear
    /// and the other end has returned no entry, and no entry otherwise ([`FRONT_SHUT`] and [`BACK_SHUT`]), so that
    /// every other step goes through the checks of [`Range::guard`].
    quick: usize,
    started: bool,
    finished: bool,
    /// Whether this end has returned an entry. Its last one is then next to its index: before it for the front, at
    /// it for the back (see [`Range::front_last`]), so that the quick steps need not record it.
    returned: bool,
    /// Whether every key of the current leaf is within the interval's bound on this end's side (the upper one for
    /// the front, the lower one for the back), so that its steps need not compare each key with it.
    clear: bool,
}

/// The front's [`End::quick`] that leaves it no quick step.
const FRONT_SHUT: usize = 0;
/// The back's [`End::quick`] that leaves it no quick step.
const BACK_SHUT: usize = usize::MAX;

impl End<'_> {
    const fn new(quick: usize) -> Self {
        End {
            path: Vec::new(),
            leaf: &[],
            index: 0,
            quick,
            started: false,
            finished: false,
            returned: false,
            clear: false,
        }
    }
}

/// Whether every key of `leaf` is not after the upper bound and not after the keys with the prefix. The keys that
/// pass form a prefix of the key order, so the leaf's last key decides for all of them.
fn leaf_before_upper(interval: &Interval<'_>, leaf: &[u8]) -> bool {
    let count = node::count(leaf);
    count > 0 && interval.before_upper(node::key(leaf, count - 1))
}

/// Whether every key of `leaf` is not before the lower bound and not before the prefix. The keys that pass form a
/// suffix of the key order, so the leaf's first key decides for all of them.
fn leaf_after_lower(interval: &Interval<'_>, leaf: &[u8]) -> bool {
    node::count(leaf) > 0 && interval.after_lower(node::key(leaf, 0))
}

/// A double-ended scan of the entries of an interval of a tree, in key order, borrowed from the transaction
/// without copying them (D11).
pub struct Range<'t, S: PageSource + ?Sized> {
    tx: &'t S,
    root: u64,
    interval: OwnedInterval,
    front: End<'t>,
    back: End<'t>,
}

impl<'t, S: PageSource + ?Sized> Range<'t, S> {
    /// A scan of `interval` in the tree at `root`. It copies the interval, which may fail for lack of memory, and
    /// reads no page; an empty interval (R3) gives an empty scan.
    pub fn new(tx: &'t S, root: u64, interval: Interval<'_>) -> Result<Self> {
        let empty = interval.is_empty();
        let interval = OwnedInterval::new(tx, interval)?;
        let mut range = Range {
            tx,
            root,
            interval,
            front: End::new(FRONT_SHUT),
            back: End::new(BACK_SHUT),
        };
        if empty {
            range.stop();
        }
        Ok(range)
    }

    /// Ends both ends: after a failure, or once either end is exhausted, nothing more comes out.
    fn stop(&mut self) {
        self.front.finished = true;
        self.back.finished = true;
        self.front.quick = FRONT_SHUT;
        self.back.quick = BACK_SHUT;
    }

    /// The path from the root to the leaf where the front starts: at each level, the child that may hold the first
    /// key after the lower bound and, in the leaf, its index.
    fn seek_front(&mut self) -> Result<()> {
        let interval = self.interval.borrow();
        let path = &mut self.front.path;
        let mut node = node_at(self.tx, self.root, None)?;
        reserve(self.tx, path, usize::from(node::level(node)))?;
        loop {
            let index = node::partition(node, |key| !interval.after_lower(key));
            if node::is_leaf(node) {
                self.enter(true, node, index);
                return Ok(());
            }
            path.push((node, index));
            node = node_at(self.tx, node::child(node, index), child_level(node))?;
        }
    }

    /// The path from the root to the leaf where the back starts: at each level, the child that may hold the last
    /// key before the upper bound and, in the leaf, the index after it.
    fn seek_back(&mut self) -> Result<()> {
        let interval = self.interval.borrow();
        let path = &mut self.back.path;
        let mut node = node_at(self.tx, self.root, None)?;
        reserve(self.tx, path, usize::from(node::level(node)))?;
        loop {
            let index = node::partition(node, |key| interval.before_upper(key));
            if node::is_leaf(node) {
                self.enter(false, node, index);
                return Ok(());
            }
            path.push((node, index));
            node = node_at(self.tx, node::child(node, index), child_level(node))?;
        }
    }

    /// Makes `leaf` the leaf of the front (`front`) or the back, at `index`: whether its steps compare the keys
    /// with the bound, whether they may be quick, and the prefetch of the leaf after it.
    fn enter(&mut self, front: bool, leaf: &'t [u8], index: usize) {
        let interval = self.interval.borrow();
        if front {
            let clear = leaf_before_upper(&interval, leaf);
            self.front.quick = if clear && !self.back.returned {
                node::count(leaf)
            } else {
                FRONT_SHUT
            };
            self.front.clear = clear;
            self.front.leaf = leaf;
            self.front.index = index;
        } else {
            let clear = leaf_after_lower(&interval, leaf);
            self.back.quick = if clear && !self.front.returned {
                0
            } else {
                BACK_SHUT
            };
            self.back.clear = clear;
            self.back.leaf = leaf;
            self.back.index = index;
        }
        self.prefetch_neighbour(front);
    }

    /// The last key the front returned. Between steps, and until the scan stops, the front's leaf and index are
    /// those of the step that returned it, as every step that moves to another leaf returns its first entry or stops.
    fn front_last(&self) -> Option<&'t [u8]> {
        let front = &self.front;
        front
            .returned
            .then(|| node::key(front.leaf, front.index - 1))
    }

    /// The last key the back returned, as [`Range::front_last`].
    fn back_last(&self) -> Option<&'t [u8]> {
        let back = &self.back;
        back.returned.then(|| node::key(back.leaf, back.index))
    }

    /// The front's next entry, or `None` at its end.
    fn step_front(&mut self) -> Result<Option<(&'t [u8], &'t [u8])>> {
        if !self.front.started {
            self.front.started = true;
            self.seek_front()?;
        }
        loop {
            let (leaf, index) = (self.front.leaf, self.front.index);
            if index < node::count(leaf) {
                let (key, value) = node::entry(leaf, index);
                if (!self.front.clear && !self.interval.borrow().before_upper(key))
                    || self.back_last().is_some_and(|last| key >= last)
                {
                    return Ok(None);
                }
                let value = resolve(self.tx, value)?;
                self.front.index = index + 1;
                self.front.returned = true;
                // From now on the back compares each of its keys with this one.
                self.back.quick = BACK_SHUT;
                return Ok(Some((key, value)));
            }
            // The next leaf: up to the first branch with a child to the right, then down its leftmost children.
            let back_last = self.back_last();
            loop {
                let Some((branch, child)) = self.front.path.last_mut() else {
                    return Ok(None);
                };
                if *child < node::count(branch) {
                    // The next child's keys start at this separator: if it already ends the scan, they all do, and
                    // the scan stops without reading that child.
                    let separator = node::key(branch, *child);
                    let interval = self.interval.borrow();
                    if !interval.before_upper(separator)
                        || back_last.is_some_and(|last| separator >= last)
                    {
                        return Ok(None);
                    }
                    *child += 1;
                    let (branch, child) = (*branch, *child);
                    self.descend(true, branch, child)?;
                    break;
                }
                self.front.path.pop();
            }
        }
    }

    /// The back's next entry, or `None` at its end.
    fn step_back(&mut self) -> Result<Option<(&'t [u8], &'t [u8])>> {
        if !self.back.started {
            self.back.started = true;
            self.seek_back()?;
        }
        loop {
            let (leaf, end) = (self.back.leaf, self.back.index);
            if end > 0 {
                let index = end - 1;
                let (key, value) = node::entry(leaf, index);
                if (!self.back.clear && !self.interval.borrow().after_lower(key))
                    || self.front_last().is_some_and(|last| key <= last)
                {
                    return Ok(None);
                }
                let value = resolve(self.tx, value)?;
                self.back.index = index;
                self.back.returned = true;
                // From now on the front compares each of its keys with this one.
                self.front.quick = FRONT_SHUT;
                return Ok(Some((key, value)));
            }
            // The previous leaf: up to the first branch with a child to the left, then down its rightmost children.
            let front_last = self.front_last();
            loop {
                let Some((branch, child)) = self.back.path.last_mut() else {
                    return Ok(None);
                };
                if *child > 0 {
                    // The previous child's keys are before this separator: if every key before it is before the
                    // interval or the front's last key, the scan stops without reading that child.
                    let separator = node::key(branch, *child - 1);
                    let interval = self.interval.borrow();
                    if interval.all_before_lower(separator)
                        || front_last.is_some_and(|last| separator <= last)
                    {
                        return Ok(None);
                    }
                    *child -= 1;
                    let (branch, child) = (*branch, *child);
                    self.descend(false, branch, child)?;
                    break;
                }
                self.back.path.pop();
            }
        }
    }

    /// Goes down from child `child` of `branch` to a leaf along the leftmost children for the front, or the
    /// rightmost ones for the back, adds the branches to that end's path and makes the leaf its leaf.
    fn descend(&mut self, front: bool, branch: &'t [u8], child: usize) -> Result<()> {
        let path = if front {
            &mut self.front.path
        } else {
            &mut self.back.path
        };
        let mut node = node_at(self.tx, node::child(branch, child), child_level(branch))?;
        loop {
            // A branch's rightmost child and a leaf's end are both at its number of cells.
            let index = if front { 0 } else { node::count(node) };
            if node::is_leaf(node) {
                self.enter(front, node, index);
                return Ok(());
            }
            path.push((node, index));
            node = node_at(self.tx, node::child(node, index), child_level(node))?;
        }
    }

    /// Asks for the leaf an end reads after its current one, the next child of the same branch for the front and the
    /// previous one for the back, to be brought into the caches while it reads the current one. Only when every key
    /// of the current leaf is within the end's bound, as otherwise the scan stops in it.
    fn prefetch_neighbour(&self, front: bool) {
        let end = if front { &self.front } else { &self.back };
        if !end.clear {
            return;
        }
        let Some(&(branch, child)) = end.path.last() else {
            return;
        };
        let neighbour = if front {
            (child < node::count(branch)).then_some(child + 1)
        } else {
            child.checked_sub(1)
        };
        if let Some(neighbour) = neighbour {
            self.tx.prefetch(node::child(branch, neighbour));
        }
    }

    /// Runs a step of one end with every check, stopping both ends after a failure. It is kept out of the quick
    /// steps of [`Range::next`] and [`Range::next_back`], so that they stay small enough to inline.
    #[inline(never)]
    fn guard(&mut self, front: bool) -> Option<Result<(&'t [u8], &'t [u8])>> {
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
                // An end stops only when no entry is left between it and the other end's last one, or the bound:
                // the other end has nothing left either, so it must not read or reserve anything more (Codex R1).
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

impl<'t, S: PageSource + ?Sized> Iterator for Range<'t, S> {
    type Item = Result<(&'t [u8], &'t [u8])>;

    /// The quick step: an entry of a clear leaf with its value in the leaf, while the back has returned none, needs
    /// no key comparison and cannot fail. Every other step goes through [`Range::guard`]. Always inlined, so that the
    /// scans that wrap this one do not call it.
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        let front = &mut self.front;
        if front.index < front.quick
            && let (key, Value::Inline(value)) = node::entry(front.leaf, front.index)
        {
            front.index += 1;
            return Some(Ok((key, value)));
        }
        self.guard(true)
    }
}

impl<S: PageSource + ?Sized> DoubleEndedIterator for Range<'_, S> {
    /// The quick step of the back, as [`Range::next`]'s.
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        let back = &mut self.back;
        if back.index > back.quick
            && let (key, Value::Inline(value)) = node::entry(back.leaf, back.index - 1)
        {
            back.index -= 1;
            return Some(Ok((key, value)));
        }
        self.guard(false)
    }
}

impl<S: PageSource + ?Sized> std::iter::FusedIterator for Range<'_, S> {}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty(prefix: &[u8], lower: Bound<&[u8]>, upper: Bound<&[u8]>) -> bool {
        Interval {
            prefix,
            lower,
            upper,
        }
        .is_empty()
    }

    #[test]
    fn inverted_and_equal_excluded_bounds_are_empty() {
        assert!(empty(b"", Included(b"c"), Included(b"a")));
        assert!(empty(b"", Included(b"a"), Excluded(b"a")));
        assert!(empty(b"", Excluded(b"a"), Included(b"a")));
        assert!(!empty(b"", Included(b"a"), Included(b"a")));
        assert!(!empty(b"", Unbounded, Unbounded));
        // No key is before the smallest one.
        assert!(empty(b"", Unbounded, Excluded(&[0])));
        assert!(!empty(b"", Unbounded, Included(&[0])));
    }

    #[test]
    fn no_key_lies_between_a_key_and_its_successor() {
        assert!(empty(b"", Excluded(b"a"), Excluded(b"a\0")));
        assert!(!empty(b"", Excluded(b"a"), Included(b"a\0")));
        assert!(!empty(b"", Excluded(b"a"), Excluded(b"a\0\0")));
        // A key of the most bytes a key may have: its successor drops the trailing 0xFF bytes.
        let mut longest = vec![0xFF; MAX_KEY];
        longest[0] = b'a';
        assert!(empty(b"", Excluded(&longest), Excluded(b"b")));
        assert!(!empty(b"", Excluded(&longest), Included(b"b")));
        assert!(empty(b"", Excluded(&vec![0xFF; MAX_KEY]), Unbounded));
    }

    #[test]
    fn prefixes_that_no_key_between_the_bounds_starts_with() {
        assert!(empty(b"a", Included(b"c"), Unbounded));
        assert!(!empty(b"a", Included(b"0"), Unbounded));
        assert!(empty(b"b", Unbounded, Excluded(b"b")));
        assert!(!empty(b"b", Unbounded, Included(b"b")));
        assert!(empty(b"a", Excluded(b"a"), Excluded(b"a\0")));
        // A prefix of the most bytes a key may have is a single key.
        let full = vec![b'p'; MAX_KEY];
        assert!(!empty(&full, Included(&full), Unbounded));
        assert!(empty(&full, Excluded(&full), Unbounded));
        // The successor of a longest key with the prefix leaves the prefix.
        let mut longest = vec![0xFF; MAX_KEY];
        longest[0] = b'a';
        assert!(empty(b"a", Excluded(&longest), Unbounded));
        longest[5] = 0;
        assert!(!empty(b"a", Excluded(&longest), Unbounded));
    }

    fn empty_of_values(prefix: &[u8], lower: Bound<&[u8]>, upper: Bound<&[u8]>) -> bool {
        Interval {
            prefix,
            lower,
            upper,
        }
        .is_empty_of_values()
    }

    /// D4 of the multi-value trees execution: among values the empty value is the least, so it is a valid bound and
    /// the interval up to it holds it, while the same intervals of keys keep their meaning.
    #[test]
    fn the_empty_value_is_the_least_value() {
        assert!(!empty_of_values(b"", Unbounded, Excluded(&[0])));
        assert!(empty(b"", Unbounded, Excluded(&[0])));
        assert!(!empty_of_values(b"", Unbounded, Included(b"")));
        assert!(empty_of_values(b"", Unbounded, Excluded(b"")));
        assert!(!empty_of_values(b"", Included(b""), Included(b"")));
        assert!(empty_of_values(b"", Excluded(b""), Included(b"")));
        assert!(empty_of_values(b"", Excluded(b""), Excluded(&[0])));
        assert!(!empty_of_values(b"", Excluded(b""), Included(&[0])));
        assert!(empty_of_values(b"a", Unbounded, Included(b"")));
        assert!(!empty_of_values(b"a", Included(b""), Unbounded));
        assert!(empty_of_values(b"", Included(b"b"), Included(b"a")));
        let all = Interval {
            prefix: b"",
            lower: Unbounded,
            upper: Unbounded,
        };
        assert!(all.contains(b"") && all.contains(&[0xFF]));
        let after = Interval {
            prefix: b"",
            lower: Excluded(b""),
            upper: Unbounded,
        };
        assert!(!after.contains(b"") && after.contains(&[0]));
        let prefixed = Interval {
            prefix: b"a",
            lower: Unbounded,
            upper: Unbounded,
        };
        assert!(!prefixed.contains(b"") && prefixed.contains(b"ab") && !prefixed.contains(b"b"));
    }

    #[test]
    fn the_prefix_folds_into_the_bounds() {
        let interval = Interval {
            prefix: &[0xFF, 0xFF],
            lower: Unbounded,
            upper: Unbounded,
        };
        assert!(!interval.after_lower(&[0xFF]) && interval.after_lower(&[0xFF, 0xFF]));
        assert!(interval.before_upper(&[0xFF, 0xFF, 0xFF]) && interval.before_upper(&[0x01]));
        let b = Interval {
            prefix: b"b",
            lower: Unbounded,
            upper: Unbounded,
        };
        assert!(b.before_upper(b"a") && b.before_upper(b"b") && b.before_upper(b"ba"));
        assert!(!b.before_upper(b"c") && !b.after_lower(b"a") && b.after_lower(b"b"));
    }

    /// A measurement of the step of a scan, not a check: scans of a tree of 1,000,000 entries with W11's sizes (16-byte
    /// keys, 100-byte values, stored in a scrambled order), whole from each end without bounds, with both bounds and
    /// with a prefix, and 100,000 scans of 100 entries from scattered keys, as W11's. It prints the best of five
    /// times per entry of each. Run it in a release build, on an otherwise idle machine:
    ///
    /// `cargo test -p krummi-tree --release --lib -- --ignored --nocapture range_step_measurement`
    #[test]
    #[ignore = "a measurement; run it in a release build"]
    fn range_step_measurement() {
        use std::hint::black_box;
        use std::time::{Duration, Instant};

        use krummi_tx::{Engine, OpenOptions};

        const ENTRIES: u64 = 1_000_000;
        // Every key starts with five zero bytes, so that a scan with that prefix reads the whole tree.
        let key = |i: u64| -> [u8; 16] {
            let mut key = [0; 16];
            key[..8].copy_from_slice(&i.to_be_bytes());
            key[8..].copy_from_slice(&i.wrapping_mul(0x9E37_79B9_7F4A_7C15).to_le_bytes());
            key
        };
        let value =
            |i: u64| -> [u8; 100] { std::array::from_fn(|b| (i as u8).wrapping_add(b as u8)) };
        let engine = Engine::open_temporary(&OpenOptions::default()).unwrap();
        let mut tx = engine.write(None).unwrap();
        let mut root = crate::create(&mut tx).unwrap();
        // 7,919 is prime to 1,000,000, so `i * 7919 % ENTRIES` visits every key once.
        for i in 0..ENTRIES {
            let n = i * 7919 % ENTRIES;
            root = crate::set(&mut tx, root, &key(n), &value(n)).unwrap().root;
        }
        tx.commit().unwrap();
        let read = engine.read().unwrap();

        let (lower, upper) = (key(1), key(ENTRIES - 1));
        let all = Interval {
            prefix: b"",
            lower: Unbounded,
            upper: Unbounded,
        };
        let bounded = Interval {
            prefix: b"",
            lower: Included(&lower),
            upper: Excluded(&upper),
        };
        let prefixed = Interval {
            prefix: &[0; 5],
            lower: Unbounded,
            upper: Unbounded,
        };
        let starts: Vec<[u8; 16]> = (0..100_000u64).map(|i| key(i * 7919 % ENTRIES)).collect();
        // The keys' and values' first bytes, so that the scan reads each entry.
        let sum = |(key, value): (&[u8], &[u8])| u64::from(key[0]) + u64::from(value[0]);
        let measure = |name: &str, run: &dyn Fn() -> (u64, u64)| {
            let mut best = Duration::MAX;
            let mut entries = 0;
            for _ in 0..5 {
                let start = Instant::now();
                let (count, total) = run();
                best = best.min(start.elapsed());
                black_box(total);
                entries = count;
            }
            println!(
                "{name:<24} {entries:>9} entries {:>7.2} ns/entry",
                best.as_secs_f64() * 1e9 / entries as f64
            );
        };
        let whole = |interval: Interval<'_>, back: bool| {
            let scan = Range::new(&read, root, interval).unwrap();
            let fold = |(count, total): (u64, u64), entry: Result<(&[u8], &[u8])>| {
                (count + 1, total + sum(entry.unwrap()))
            };
            if back {
                scan.rev().fold((0, 0), fold)
            } else {
                scan.fold((0, 0), fold)
            }
        };
        measure("forward", &|| whole(all, false));
        measure("backward", &|| whole(all, true));
        measure("forward, bounds", &|| whole(bounded, false));
        measure("backward, bounds", &|| whole(bounded, true));
        measure("forward, prefix", &|| whole(prefixed, false));
        measure("backward, prefix", &|| whole(prefixed, true));
        let short = |back: bool| {
            let (mut count, mut total) = (0, 0);
            for start in &starts {
                let mut add = |entry: Result<(&[u8], &[u8])>| {
                    count += 1;
                    total += sum(entry.unwrap());
                };
                if back {
                    let to = Interval {
                        prefix: b"",
                        lower: Unbounded,
                        upper: Included(start),
                    };
                    let scan = Range::new(&read, root, to).unwrap();
                    scan.rev().take(100).for_each(&mut add);
                } else {
                    let from = Interval {
                        prefix: b"",
                        lower: Included(start),
                        upper: Unbounded,
                    };
                    let scan = Range::new(&read, root, from).unwrap();
                    scan.take(100).for_each(&mut add);
                }
            }
            (count, total)
        };
        measure("100,000 x 100 forward", &|| short(false));
        measure("100,000 x 100 backward", &|| short(true));
    }
}
