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
//! once. Every node is read with the checks of the tree code (damage with its page number, G6). After any failure
//! both ends stop.

use std::ops::Bound::{self, Excluded, Included, Unbounded};

use krummi_tx::Result;

use crate::{MAX_KEY, PageSource, child_level, copy, node, node_at, reserve};

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
        bound && key >= self.prefix
    }

    /// Whether `key` is not after the upper bound and not after every key that starts with the prefix.
    pub fn before_upper(&self, key: &[u8]) -> bool {
        let bound = match self.upper {
            Included(upper) => key <= upper,
            Excluded(upper) => key < upper,
            Unbounded => true,
        };
        bound && (key < self.prefix || key.starts_with(self.prefix))
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

    /// Whether no valid key (1 to [`MAX_KEY`] bytes) that starts with the prefix lies between the bounds (R3). It
    /// is decided from the arguments alone: the smallest such key after the lower bound is compared with the upper
    /// bound. The bounds and the prefix must be valid sizes.
    pub fn is_empty(&self) -> bool {
        /// The smallest candidate key.
        enum Least<'k> {
            /// This key.
            Key(&'k [u8]),
            /// The key right after this one among the valid keys.
            After(&'k [u8]),
        }
        let least = match self.lower {
            Unbounded if self.prefix.is_empty() => Least::Key(&[0]),
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

/// An [`Interval`] copied, so that a [`Range`] does not borrow its caller's bounds.
struct OwnedInterval {
    prefix: Vec<u8>,
    lower: Bound<Vec<u8>>,
    upper: Bound<Vec<u8>>,
}

impl OwnedInterval {
    fn borrow(&self) -> Interval<'_> {
        Interval {
            prefix: &self.prefix,
            lower: self.lower.as_ref().map(Vec::as_slice),
            upper: self.upper.as_ref().map(Vec::as_slice),
        }
    }
}

/// One end of a [`Range`]: the path from the root to its leaf, each node with the index of the child it went down
/// to and, in the leaf, the front's next entry or the back's entry after its next one.
struct End<'t> {
    path: Vec<(&'t [u8], usize)>,
    started: bool,
    finished: bool,
    /// The last key this end returned.
    last: Option<&'t [u8]>,
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
        let owned = |bound: Bound<&[u8]>| -> Result<Bound<Vec<u8>>> {
            Ok(match bound {
                Included(key) => Included(copy(tx, key)?),
                Excluded(key) => Excluded(copy(tx, key)?),
                Unbounded => Unbounded,
            })
        };
        let empty = interval.is_empty();
        let interval = OwnedInterval {
            prefix: copy(tx, interval.prefix)?,
            lower: owned(interval.lower)?,
            upper: owned(interval.upper)?,
        };
        let mut range = Range {
            tx,
            root,
            interval,
            front: End::new(),
            back: End::new(),
        };
        if empty {
            range.stop();
        }
        Ok(range)
    }

    /// Ends both ends: after a failure, nothing more comes out.
    fn stop(&mut self) {
        self.front.finished = true;
        self.back.finished = true;
    }

    /// The path from the root to the leaf where the front starts: at each level, the child that may hold the first
    /// key after the lower bound and, in the leaf, its index.
    fn seek_front(&mut self) -> Result<()> {
        let interval = self.interval.borrow();
        let path = &mut self.front.path;
        let mut node = node_at(self.tx, self.root, None)?;
        reserve(self.tx, path, usize::from(node::level(node)) + 1)?;
        loop {
            let index = node::partition(node, |key| !interval.after_lower(key));
            path.push((node, index));
            if node::is_leaf(node) {
                return Ok(());
            }
            node = node_at(self.tx, node::child(node, index), child_level(node))?;
        }
    }

    /// The path from the root to the leaf where the back starts: at each level, the child that may hold the last
    /// key before the upper bound and, in the leaf, the index after it.
    fn seek_back(&mut self) -> Result<()> {
        let interval = self.interval.borrow();
        let path = &mut self.back.path;
        let mut node = node_at(self.tx, self.root, None)?;
        reserve(self.tx, path, usize::from(node::level(node)) + 1)?;
        loop {
            let index = node::partition(node, |key| interval.before_upper(key));
            path.push((node, index));
            if node::is_leaf(node) {
                return Ok(());
            }
            node = node_at(self.tx, node::child(node, index), child_level(node))?;
        }
    }

    /// The front's next entry, or `None` at its end.
    fn step_front(&mut self) -> Result<Option<(&'t [u8], &'t [u8])>> {
        if !self.front.started {
            self.front.started = true;
            self.seek_front()?;
        }
        loop {
            let Some(&(leaf, index)) = self.front.path.last() else {
                return Ok(None);
            };
            if index < node::count(leaf) {
                let key = node::key(leaf, index);
                let interval = self.interval.borrow();
                if !interval.before_upper(key) || self.back.last.is_some_and(|last| key >= last) {
                    return Ok(None);
                }
                self.front.path.last_mut().expect("a leaf").1 += 1;
                self.front.last = Some(key);
                return Ok(Some((key, node::value(leaf, index))));
            }
            // The next leaf: up to the first branch with a child to the right, then down its leftmost children.
            self.front.path.pop();
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
                        || self.back.last.is_some_and(|last| separator >= last)
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
            let Some(&(leaf, end)) = self.back.path.last() else {
                return Ok(None);
            };
            if end > 0 {
                let index = end - 1;
                let key = node::key(leaf, index);
                let interval = self.interval.borrow();
                if !interval.after_lower(key) || self.front.last.is_some_and(|last| key <= last) {
                    return Ok(None);
                }
                self.back.path.last_mut().expect("a leaf").1 = index;
                self.back.last = Some(key);
                return Ok(Some((key, node::value(leaf, index))));
            }
            // The previous leaf: up to the first branch with a child to the left, then down its rightmost children.
            self.back.path.pop();
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
                        || self.front.last.is_some_and(|last| separator <= last)
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
    /// rightmost ones for the back, and adds the nodes to that end's path.
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
            path.push((node, index));
            if node::is_leaf(node) {
                return Ok(());
            }
            node = node_at(self.tx, node::child(node, index), child_level(node))?;
        }
    }

    /// Runs a step of one end, stopping both ends after a failure.
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
                if front {
                    self.front.finished = true;
                } else {
                    self.back.finished = true;
                }
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

    fn next(&mut self) -> Option<Self::Item> {
        self.guard(true)
    }
}

impl<S: PageSource + ?Sized> DoubleEndedIterator for Range<'_, S> {
    fn next_back(&mut self) -> Option<Self::Item> {
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
}
