//! Multi-value trees (D1–D4, D6 and D7 of the multi-value trees execution): ordered maps from keys of 1 to
//! [`MAX_KEY`] bytes to sets of distinct values of 0 to [`MAX_KEY`] bytes, ordered byte by byte (M1). A multi-value
//! tree is a normal tree, the parent, whose leaf value for a key is a blob that holds the key's values in one of two
//! representations (M2):
//!
//! | Representation | Blob |
//! |---|---|
//! | Inline | tag `0`, number of values (u16), then each value as its length (u16) and its bytes, in order |
//! | Subtree | tag `1`, flags (bit 0: the key has the empty value), number of values (u64), inline bytes (u64), root (u64) |
//!
//! The empty value is a zero-length element inline, and a flag in a subtree, whose keys are the key's non-empty
//! values, each with an empty value: a subtree never holds an empty key. The inline bytes of a subtree are the length
//! its inline blob would have, so that demotion is decided without reading the subtree. Integers are little-endian
//! (M14).
//!
//! The volume of a key is its length plus the length of its inline blob (D2). An addition that makes it exceed the
//! promotion threshold moves the values to a new subtree in that operation; a removal that leaves it below the
//! demotion threshold moves them back to the entry, reading and then freeing the subtree, in that operation; between
//! the two a key keeps its representation. The promotion threshold is at most [`MAX_PROMOTION`], so an inline entry
//! always fits in its leaf, never off it (M1).
//!
//! The number of values of a key is in its blob, so counting them and scanning keys read the parent only, never a
//! subtree (M11, D3). The tree's number of entries is its number of keys: each change reports whether a key appeared
//! or went away. Freeing a key, a range of keys or the whole tree frees the subtrees of its keys before the parent's
//! pages (D6). A blob that is not consistent, in a page whose checksum is valid, is damage of the parent leaf that
//! holds it (G6); every subtree page is read with the checks of the tree code.
//!
//! Like the rest of the crate, the operations do not validate sizes (B7): they panic on a key or a value out of
//! range, a misuse of the engine.

use std::ops::Bound::{Excluded, Included, Unbounded};
use std::sync::atomic::{AtomicU64, Ordering};

use krummi_tx::{Result, WriteTransaction};

use crate::node::{self, LEAF_CELL_HEADER, MAX_LEAF_CELL, Value};
use crate::range::OwnedInterval;
use crate::{Interval, MAX_KEY, PageSource, Range, RangeUpdate, copy, descend, leaves, reserve};

/// Highest promotion threshold: an inline entry of this volume fills the longest leaf cell kept in its leaf (D2).
pub const MAX_PROMOTION: usize = MAX_LEAF_CELL - LEAF_CELL_HEADER;
/// Provisional promotion threshold, until the measurement of M15 fixes it (M3).
const DEFAULT_PROMOTION: usize = 2048;
/// Provisional demotion threshold, until the measurement of M15 fixes it (M3).
const DEFAULT_DEMOTION: usize = 1024;

const INLINE: u8 = 0;
const SUBTREE: u8 = 1;
/// Bytes of an inline blob before its values: the tag and the number of values.
const INLINE_HEADER: usize = 3;
/// Bytes before each value of an inline blob: its length.
const ELEMENT_HEADER: usize = 2;
/// Bytes of a subtree blob.
const SUBTREE_BLOB: usize = 26;
/// The flag of a subtree blob for a key with the empty value.
const HAS_EMPTY: u8 = 1;

// The entry of a key of the most bytes with its values in a subtree always fits in its leaf.
const _: () = assert!(LEAF_CELL_HEADER + MAX_KEY + SUBTREE_BLOB <= MAX_LEAF_CELL);
const _: () = assert!(DEFAULT_DEMOTION < DEFAULT_PROMOTION && DEFAULT_PROMOTION <= MAX_PROMOTION);

/// The promotion threshold in the high half and the demotion threshold in the low one, so that both change at once.
static THRESHOLDS: AtomicU64 = AtomicU64::new(pack(DEFAULT_PROMOTION, DEFAULT_DEMOTION));

const fn pack(promotion: usize, demotion: usize) -> u64 {
    ((promotion as u64) << 32) | demotion as u64
}

/// A measurement hook, not part of the engine's interface (D7): sets the promotion and the demotion thresholds of
/// every multi-value tree of the process. Only the measurement of M15 calls it; tests use the defaults.
///
/// # Panics
///
/// Unless `demotion < promotion <= MAX_PROMOTION`.
#[doc(hidden)]
pub fn set_multi_thresholds(promotion: usize, demotion: usize) {
    assert!(
        demotion < promotion && promotion <= MAX_PROMOTION,
        "the thresholds satisfy demotion < promotion <= {MAX_PROMOTION}"
    );
    THRESHOLDS.store(pack(promotion, demotion), Ordering::Relaxed);
}

/// A test hook, not part of the engine's interface: the promotion and the demotion thresholds in force (D2).
#[doc(hidden)]
pub fn multi_thresholds() -> (usize, usize) {
    let packed = THRESHOLDS.load(Ordering::Relaxed);
    (
        (packed >> 32) as usize,
        (packed & u64::from(u32::MAX)) as usize,
    )
}

/// A test hook, not part of the engine's interface: the volume of a key of `key_len` bytes whose values have the
/// lengths `value_lens` (D2), the quantity compared with the thresholds.
#[doc(hidden)]
pub fn volume(key_len: usize, value_lens: impl IntoIterator<Item = usize>) -> usize {
    key_len
        + INLINE_HEADER
        + value_lens
            .into_iter()
            .map(|len| ELEMENT_HEADER + len)
            .sum::<usize>()
}

/// A test hook, not part of the engine's interface: the root page of the subtree of `key` in the multi-value tree at
/// `root`, or `None` when the key is not there or its values are in its entry.
#[doc(hidden)]
pub fn subtree_of<S: PageSource + ?Sized>(tx: &S, root: u64, key: &[u8]) -> Result<Option<u64>> {
    Ok(match entry(tx, root, key)? {
        Some((_, Entry::Subtree(subtree))) => Some(subtree.root),
        _ => None,
    })
}

/// What an operation on the values of a key did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiUpdate {
    /// The parent's root page afterwards.
    pub root: u64,
    /// How many values it added or removed.
    pub values: u64,
    /// Whether the key appeared or went away, so the tree's number of entries (its keys) changed by one.
    pub key_changed: bool,
}

impl MultiUpdate {
    const fn unchanged(root: u64) -> Self {
        MultiUpdate {
            root,
            values: 0,
            key_changed: false,
        }
    }
}

/// The values of a key, as its blob holds them.
#[derive(Clone, Copy)]
enum Entry<'t> {
    /// The values in the entry: their number and their encoding after the blob's header, checked.
    Inline {
        count: u64,
        elements: &'t [u8],
    },
    Subtree(Subtree),
}

/// The values of a key in a subtree.
#[derive(Clone, Copy)]
struct Subtree {
    has_empty: bool,
    /// The number of values, the empty one included.
    count: u64,
    /// The length of the inline blob of these values.
    inline_len: u64,
    root: u64,
}

impl Subtree {
    fn encode(&self) -> [u8; SUBTREE_BLOB] {
        let mut blob = [0; SUBTREE_BLOB];
        blob[0] = SUBTREE;
        blob[1] = if self.has_empty { HAS_EMPTY } else { 0 };
        blob[2..10].copy_from_slice(&self.count.to_le_bytes());
        blob[10..18].copy_from_slice(&self.inline_len.to_le_bytes());
        blob[18..26].copy_from_slice(&self.root.to_le_bytes());
        blob
    }
}

impl<'t> Entry<'t> {
    /// Decodes a blob, or `None` if it is not consistent: an unknown tag, a wrong length, no values, values out of
    /// order or too long, or inline bytes that cannot be those of its values.
    fn decode(blob: &'t [u8]) -> Option<Self> {
        match *blob.first()? {
            INLINE if blob.len() >= INLINE_HEADER => {
                let count = u16::from_le_bytes([blob[1], blob[2]]);
                let elements = &blob[INLINE_HEADER..];
                let mut rest = elements;
                let mut previous: Option<&[u8]> = None;
                for _ in 0..count {
                    let (value, after) = split_element(rest)?;
                    if previous.is_some_and(|previous| previous >= value) {
                        return None;
                    }
                    (previous, rest) = (Some(value), after);
                }
                (count > 0 && rest.is_empty()).then_some(Entry::Inline {
                    count: u64::from(count),
                    elements,
                })
            }
            SUBTREE if blob.len() == SUBTREE_BLOB && blob[1] & !HAS_EMPTY == 0 => {
                let field =
                    |at: usize| u64::from_le_bytes(blob[at..at + 8].try_into().expect("8 bytes"));
                let subtree = Subtree {
                    has_empty: blob[1] == HAS_EMPTY,
                    count: field(2),
                    inline_len: field(10),
                    root: field(18),
                };
                let least = subtree.count.checked_mul(ELEMENT_HEADER as u64)?;
                let most = subtree
                    .count
                    .saturating_mul((ELEMENT_HEADER + MAX_KEY) as u64);
                let values = subtree.inline_len.checked_sub(INLINE_HEADER as u64)?;
                (subtree.count > 0 && (least..=most).contains(&values))
                    .then_some(Entry::Subtree(subtree))
            }
            _ => None,
        }
    }

    /// The number of values, read from the blob alone (M11).
    fn count(&self) -> u64 {
        match self {
            Entry::Inline { count, .. } => *count,
            Entry::Subtree(subtree) => subtree.count,
        }
    }
}

/// The first value of an inline encoding and the rest, or `None` if it does not hold one.
fn split_element(elements: &[u8]) -> Option<(&[u8], &[u8])> {
    let (len, rest) = elements.split_first_chunk::<ELEMENT_HEADER>()?;
    let len = usize::from(u16::from_le_bytes(*len));
    (len <= MAX_KEY && len <= rest.len()).then(|| rest.split_at(len))
}

/// The values of a checked inline encoding, in order.
#[derive(Clone)]
struct Elements<'t>(&'t [u8]);

impl<'t> Iterator for Elements<'t> {
    type Item = &'t [u8];

    fn next(&mut self) -> Option<&'t [u8]> {
        let (value, rest) = split_element(self.0)?;
        self.0 = rest;
        Some(value)
    }
}

/// The page of the parent leaf with the entry of `key` and its values, or `None` if the key is not there. An entry
/// that is not a consistent blob kept in its leaf is damage of that leaf (G6).
fn entry<'t, S: PageSource + ?Sized>(
    tx: &'t S,
    root: u64,
    key: &[u8],
) -> Result<Option<(u64, Entry<'t>)>> {
    let (page, leaf) = descend(tx, root, key)?;
    let Ok(index) = node::search(leaf, key) else {
        return Ok(None);
    };
    match node::value(leaf, index) {
        Value::Inline(blob) => Entry::decode(blob),
        Value::Run { .. } => None,
    }
    .map(|entry| Some((page, entry)))
    .ok_or_else(|| tx.damaged(page))
}

/// Decodes the blob of `key` that a scan of the parent at `root` returned; one that is not consistent is damage of
/// the leaf that holds it, found again.
fn decode_scanned<'t, S: PageSource + ?Sized>(
    tx: &'t S,
    root: u64,
    key: &[u8],
    blob: &'t [u8],
) -> Result<Entry<'t>> {
    Entry::decode(blob).ok_or_else(|| match descend(tx, root, key) {
        Ok((page, _)) => tx.damaged(page),
        Err(error) => error,
    })
}

fn check_key(key: &[u8]) {
    assert!(
        (1..=MAX_KEY).contains(&key.len()),
        "a key has 1 to {MAX_KEY} bytes"
    );
}

fn check_value(value: &[u8]) {
    assert!(value.len() <= MAX_KEY, "a value has up to {MAX_KEY} bytes");
}

fn check_value_interval(interval: &Interval<'_>) {
    for bound in [interval.lower, interval.upper] {
        if let Included(value) | Excluded(value) = bound {
            check_value(value);
        }
    }
    assert!(
        interval.prefix.len() <= MAX_KEY,
        "a prefix has up to {MAX_KEY} bytes"
    );
}

/// The inline blob of `count` values, `len` bytes long, reserved as [`reserve`] does.
fn encode_inline<'a, S: PageSource + ?Sized>(
    tx: &S,
    count: u64,
    len: usize,
    values: impl Iterator<Item = &'a [u8]>,
) -> Result<Vec<u8>> {
    let mut blob = Vec::new();
    reserve(tx, &mut blob, len)?;
    blob.push(INLINE);
    let count = u16::try_from(count).expect("an inline blob holds fewer than 2^16 values");
    blob.extend_from_slice(&count.to_le_bytes());
    for value in values {
        let value_len = u16::try_from(value.len()).expect("a value has up to 2,048 bytes");
        blob.extend_from_slice(&value_len.to_le_bytes());
        blob.extend_from_slice(value);
    }
    debug_assert_eq!(blob.len(), len);
    Ok(blob)
}

/// The part of a value interval made of non-empty values, as an interval of the keys of a subtree, or `None` when
/// it holds no non-empty value because its upper bound is the empty value (D4).
fn non_empty_part<'a>(interval: &Interval<'a>) -> Option<Interval<'a>> {
    let lower = match interval.lower {
        // Every non-empty value is after the empty one.
        Included(b"") | Excluded(b"") => Unbounded,
        bound => bound,
    };
    if let Included(b"") | Excluded(b"") = interval.upper {
        return None;
    }
    Some(Interval {
        prefix: interval.prefix,
        lower,
        upper: interval.upper,
    })
}

/// Stores `blob` as the entry of `key` in the parent at `root` and returns the parent's root.
fn store(tx: &mut WriteTransaction<'_>, root: u64, key: &[u8], blob: &[u8]) -> Result<u64> {
    Ok(crate::set(tx, root, key, blob)?.root)
}

/// Moves the values of an inline blob to a new subtree and returns its blob (M2).
fn promote(tx: &mut WriteTransaction<'_>, blob: &[u8]) -> Result<[u8; SUBTREE_BLOB]> {
    let mut subtree = Subtree {
        has_empty: false,
        count: u64::from(u16::from_le_bytes([blob[1], blob[2]])),
        inline_len: blob.len() as u64,
        root: crate::create(tx)?,
    };
    for value in Elements(&blob[INLINE_HEADER..]) {
        if value.is_empty() {
            subtree.has_empty = true;
        } else {
            subtree.root = crate::try_add(tx, subtree.root, value, b"")?.root;
        }
    }
    Ok(subtree.encode())
}

/// The inline blob of the values of `subtree`, the entry of a key in the parent leaf at `page`, reading every leaf
/// of the subtree. A subtree whose values do not match its blob is damage of that leaf.
fn demote<S: PageSource + ?Sized>(tx: &S, page: u64, subtree: &Subtree) -> Result<Vec<u8>> {
    let len = usize::try_from(subtree.inline_len).map_err(|_| tx.damaged(page))?;
    let mut blob = Vec::new();
    reserve(tx, &mut blob, len)?;
    blob.push(INLINE);
    // The volume is below the demotion threshold, so the count fits; checked anyway, against damage.
    let count = u16::try_from(subtree.count).map_err(|_| tx.damaged(page))?;
    blob.extend_from_slice(&count.to_le_bytes());
    let mut seen = 0;
    if subtree.has_empty {
        blob.extend_from_slice(&[0, 0]);
        seen += 1;
    }
    leaves(tx, subtree.root, None, &mut |_, leaf| {
        for index in 0..node::count(leaf) {
            let value = node::key(leaf, index);
            if blob.len() + ELEMENT_HEADER + value.len() > len {
                return Err(tx.damaged(page));
            }
            // A checked leaf has keys of up to `MAX_KEY` bytes.
            blob.extend_from_slice(&(value.len() as u16).to_le_bytes());
            blob.extend_from_slice(value);
            seen += 1;
        }
        Ok(())
    })?;
    if blob.len() != len || seen != subtree.count {
        return Err(tx.damaged(page));
    }
    Ok(blob)
}

/// Stores the entry of `key` after a removal from its `subtree`, in the parent leaf at `page`: the key goes away
/// with its last value; it moves back to its entry when its volume falls below the demotion threshold; otherwise it
/// keeps its subtree (M2, D6). Returns the parent's root and whether the key went away.
fn settle(
    tx: &mut WriteTransaction<'_>,
    root: u64,
    key: &[u8],
    page: u64,
    subtree: &Subtree,
) -> Result<(u64, bool)> {
    if subtree.count == 0 {
        crate::free_all(tx, subtree.root)?;
        return Ok((crate::remove(tx, root, key)?.root, true));
    }
    let (_, demotion) = multi_thresholds();
    if (key.len() as u64).saturating_add(subtree.inline_len) < demotion as u64 {
        let blob = demote(&*tx, page, subtree)?;
        crate::free_all(tx, subtree.root)?;
        return Ok((store(tx, root, key, &blob)?, false));
    }
    Ok((store(tx, root, key, &subtree.encode())?, false))
}

/// `subtree` after `values` values of `bytes` inline bytes in all went away, or damage of the parent leaf at `page`
/// when its blob had fewer.
fn shrink<S: PageSource + ?Sized>(
    tx: &S,
    page: u64,
    subtree: Subtree,
    values: u64,
    bytes: u64,
) -> Result<Subtree> {
    match (
        subtree.count.checked_sub(values),
        subtree.inline_len.checked_sub(bytes),
    ) {
        (Some(count), Some(inline_len)) => Ok(Subtree {
            count,
            inline_len,
            ..subtree
        }),
        _ => Err(tx.damaged(page)),
    }
}

/// Adds `value` to the values of `key` in the multi-value tree at `root`, creating the key if it is not there; a
/// value that is already there changes nothing and touches no page (M5, T10). An addition that makes the key's
/// volume exceed the promotion threshold moves its values to a subtree (M2).
///
/// # Panics
///
/// If `key` is empty or longer than [`MAX_KEY`], or `value` is longer than [`MAX_KEY`].
pub fn add(
    tx: &mut WriteTransaction<'_>,
    root: u64,
    key: &[u8],
    value: &[u8],
) -> Result<MultiUpdate> {
    check_key(key);
    check_value(value);
    let len = ELEMENT_HEADER + value.len();
    let (blob, key_changed) = match entry(&*tx, root, key)? {
        None => (
            encode_inline(&*tx, 1, INLINE_HEADER + len, std::iter::once(value))?,
            true,
        ),
        Some((_, Entry::Inline { count, elements })) => {
            if Elements(elements).any(|v| v == value) {
                return Ok(MultiUpdate::unchanged(root));
            }
            let before = Elements(elements).take_while(|v| *v < value);
            let after = Elements(elements).skip_while(|v| *v < value);
            let values = before.chain(std::iter::once(value)).chain(after);
            let blob_len = INLINE_HEADER + elements.len() + len;
            (encode_inline(&*tx, count + 1, blob_len, values)?, false)
        }
        Some((_, Entry::Subtree(mut subtree))) => {
            if value.is_empty() {
                if subtree.has_empty {
                    return Ok(MultiUpdate::unchanged(root));
                }
                subtree.has_empty = true;
            } else {
                let update = crate::try_add(tx, subtree.root, value, b"")?;
                if !update.count_changed {
                    return Ok(MultiUpdate::unchanged(root));
                }
                subtree.root = update.root;
            }
            subtree.count += 1;
            subtree.inline_len += len as u64;
            return Ok(MultiUpdate {
                root: store(tx, root, key, &subtree.encode())?,
                values: 1,
                key_changed: false,
            });
        }
    };
    let (promotion, _) = multi_thresholds();
    let root = if key.len() + blob.len() > promotion {
        let subtree = promote(tx, &blob)?;
        store(tx, root, key, &subtree)?
    } else {
        store(tx, root, key, &blob)?
    };
    Ok(MultiUpdate {
        root,
        values: 1,
        key_changed,
    })
}

/// Removes `value` from the values of `key` in the multi-value tree at `root`; the key goes away with its last value
/// (M6). A value that is not there changes nothing and touches no page (T10). A removal that leaves the key's volume
/// below the demotion threshold moves its values back to its entry (M2).
///
/// # Panics
///
/// As [`add`].
pub fn remove_value(
    tx: &mut WriteTransaction<'_>,
    root: u64,
    key: &[u8],
    value: &[u8],
) -> Result<MultiUpdate> {
    check_key(key);
    check_value(value);
    let Some((page, found)) = entry(&*tx, root, key)? else {
        return Ok(MultiUpdate::unchanged(root));
    };
    let (root, key_changed) = match found {
        Entry::Inline { count, elements } => {
            if !Elements(elements).any(|v| v == value) {
                return Ok(MultiUpdate::unchanged(root));
            }
            if count == 1 {
                (crate::remove(tx, root, key)?.root, true)
            } else {
                let blob_len = INLINE_HEADER + elements.len() - ELEMENT_HEADER - value.len();
                let rest = Elements(elements).filter(|v| *v != value);
                let blob = encode_inline(&*tx, count - 1, blob_len, rest)?;
                (store(tx, root, key, &blob)?, false)
            }
        }
        Entry::Subtree(mut subtree) => {
            if value.is_empty() {
                if !subtree.has_empty {
                    return Ok(MultiUpdate::unchanged(root));
                }
                subtree.has_empty = false;
            } else {
                let update = crate::remove(tx, subtree.root, value)?;
                if !update.count_changed {
                    return Ok(MultiUpdate::unchanged(root));
                }
                subtree.root = update.root;
            }
            let bytes = (ELEMENT_HEADER + value.len()) as u64;
            let subtree = shrink(&*tx, page, subtree, 1, bytes)?;
            settle(tx, root, key, page, &subtree)?
        }
    };
    Ok(MultiUpdate {
        root,
        values: 1,
        key_changed,
    })
}

/// Removes `key` with all its values from the multi-value tree at `root`, freeing its subtree first (D6), and reports
/// how many values it had; a key that is not there changes nothing (M6).
///
/// # Panics
///
/// If `key` is empty or longer than [`MAX_KEY`].
pub fn remove_key(tx: &mut WriteTransaction<'_>, root: u64, key: &[u8]) -> Result<MultiUpdate> {
    check_key(key);
    let Some((_, found)) = entry(&*tx, root, key)? else {
        return Ok(MultiUpdate::unchanged(root));
    };
    let values = found.count();
    if let Entry::Subtree(subtree) = found {
        crate::free_all(tx, subtree.root)?;
    }
    Ok(MultiUpdate {
        root: crate::remove(tx, root, key)?.root,
        values,
        key_changed: true,
    })
}

/// Removes every key of `interval` with all its values from the multi-value tree at `root`, freeing their subtrees
/// before the parent's pages (D6), and reports how many keys it removed (M8). An empty interval (R3) reads nothing.
///
/// # Panics
///
/// As [`crate::remove_range`].
pub fn remove_key_range(
    tx: &mut WriteTransaction<'_>,
    root: u64,
    interval: Interval<'_>,
) -> Result<RangeUpdate> {
    if interval.is_empty() {
        return Ok(RangeUpdate { root, removed: 0 });
    }
    let mut subtrees = Vec::new();
    for item in Range::new(&*tx, root, interval)? {
        let (key, blob) = item?;
        if let Entry::Subtree(subtree) = decode_scanned(&*tx, root, key, blob)? {
            reserve(&*tx, &mut subtrees, 1)?;
            subtrees.push(subtree.root);
        }
    }
    for subtree in subtrees {
        crate::free_all(tx, subtree)?;
    }
    crate::remove_range(tx, root, interval)
}

/// Removes the values of `interval` from the values of `key` in the multi-value tree at `root` and reports how many
/// it removed; the key goes away with its last value, and moves back to its entry when its volume falls below the
/// demotion threshold (M8, M2). An empty interval (R3) reads nothing, not even the key's entry (M7).
///
/// # Panics
///
/// If `key` is empty or longer than [`MAX_KEY`], or a bound or the prefix is longer than [`MAX_KEY`].
pub fn remove_value_range(
    tx: &mut WriteTransaction<'_>,
    root: u64,
    key: &[u8],
    interval: Interval<'_>,
) -> Result<MultiUpdate> {
    check_key(key);
    check_value_interval(&interval);
    if interval.is_empty_of_values() {
        return Ok(MultiUpdate::unchanged(root));
    }
    let Some((page, found)) = entry(&*tx, root, key)? else {
        return Ok(MultiUpdate::unchanged(root));
    };
    match found {
        Entry::Inline { count, elements } => {
            let (removed, bytes) = Elements(elements)
                .filter(|v| interval.contains(v))
                .fold((0, 0), |(n, b), v| (n + 1, b + ELEMENT_HEADER + v.len()));
            if removed == 0 {
                return Ok(MultiUpdate::unchanged(root));
            }
            let (root, key_changed) = if removed == count {
                (crate::remove(tx, root, key)?.root, true)
            } else {
                let rest = Elements(elements).filter(|v| !interval.contains(v));
                let blob_len = INLINE_HEADER + elements.len() - bytes;
                let blob = encode_inline(&*tx, count - removed, blob_len, rest)?;
                (store(tx, root, key, &blob)?, false)
            };
            Ok(MultiUpdate {
                root,
                values: removed,
                key_changed,
            })
        }
        Entry::Subtree(mut subtree) => {
            let empty = subtree.has_empty && interval.contains(b"");
            let (mut removed, mut bytes) =
                (u64::from(empty), ELEMENT_HEADER as u64 * u64::from(empty));
            if let Some(keys) = non_empty_part(&interval) {
                // The inline bytes of the values to remove, for the volume (D2), before removing them.
                for item in Range::new(&*tx, subtree.root, keys)? {
                    bytes += (ELEMENT_HEADER + item?.0.len()) as u64;
                }
                let update = crate::remove_range(tx, subtree.root, keys)?;
                subtree.root = update.root;
                removed += update.removed;
            }
            if removed == 0 {
                return Ok(MultiUpdate::unchanged(root));
            }
            subtree.has_empty &= !empty;
            let subtree = shrink(&*tx, page, subtree, removed, bytes)?;
            let (root, key_changed) = settle(tx, root, key, page, &subtree)?;
            Ok(MultiUpdate {
                root,
                values: removed,
                key_changed,
            })
        }
    }
}

/// Whether `key` has `value` in the multi-value tree at `root` (M5).
///
/// # Panics
///
/// As [`add`].
pub fn contains<S: PageSource + ?Sized>(
    tx: &S,
    root: u64,
    key: &[u8],
    value: &[u8],
) -> Result<bool> {
    check_key(key);
    check_value(value);
    Ok(match entry(tx, root, key)? {
        None => false,
        Some((_, Entry::Inline { elements, .. })) => Elements(elements).any(|v| v == value),
        Some((_, Entry::Subtree(subtree))) if value.is_empty() => subtree.has_empty,
        Some((_, Entry::Subtree(subtree))) => crate::get(tx, subtree.root, value)?.is_some(),
    })
}

/// The number of values of `key` in the multi-value tree at `root`, `0` when it is not there, read from the parent
/// alone, never from a subtree (M6, M11).
///
/// # Panics
///
/// If `key` is empty or longer than [`MAX_KEY`].
pub fn value_count<S: PageSource + ?Sized>(tx: &S, root: u64, key: &[u8]) -> Result<u64> {
    check_key(key);
    Ok(entry(tx, root, key)?.map_or(0, |(_, found)| found.count()))
}

/// Frees every page of the multi-value tree at `root`: the subtrees of its keys, then the parent (D6). It reads the
/// parent's nodes and every node of the subtrees.
pub fn free_all(tx: &mut WriteTransaction<'_>, root: u64) -> Result<()> {
    let reader = &*tx;
    let mut subtrees = Vec::new();
    leaves(reader, root, None, &mut |page, leaf| {
        for index in 0..node::count(leaf) {
            let found = match node::value(leaf, index) {
                Value::Inline(blob) => Entry::decode(blob),
                Value::Run { .. } => None,
            };
            match found {
                Some(Entry::Subtree(subtree)) => {
                    reserve(reader, &mut subtrees, 1)?;
                    subtrees.push(subtree.root);
                }
                Some(Entry::Inline { .. }) => {}
                None => return Err(reader.damaged(page)),
            }
        }
        Ok(())
    })?;
    for subtree in subtrees {
        crate::free_all(tx, subtree)?;
    }
    crate::free_all(tx, root)
}

/// A double-ended scan of the keys of an interval of a multi-value tree, each with its number of values, read from
/// the parent alone (M7, M11). Keys are borrowed from the transaction without copying them.
pub struct Keys<'t, S: PageSource + ?Sized> {
    tx: &'t S,
    root: u64,
    range: Range<'t, S>,
    failed: bool,
}

/// A scan of the keys of `interval` in the multi-value tree at `root`, with the rules of [`Range`]: it reads no page
/// until it is used, and an empty interval (R3) gives an empty scan.
pub fn keys<'t, S: PageSource + ?Sized>(
    tx: &'t S,
    root: u64,
    interval: Interval<'_>,
) -> Result<Keys<'t, S>> {
    Ok(Keys {
        tx,
        root,
        range: Range::new(tx, root, interval)?,
        failed: false,
    })
}

impl<'t, S: PageSource + ?Sized> Keys<'t, S> {
    /// The key and the number of values of a parent entry from the scan; after a failure, nothing more comes out.
    fn counted(
        &mut self,
        item: Option<Result<(&'t [u8], &'t [u8])>>,
    ) -> Option<Result<(&'t [u8], u64)>> {
        let result = item?.and_then(|(key, blob)| {
            Ok((key, decode_scanned(self.tx, self.root, key, blob)?.count()))
        });
        self.failed = result.is_err();
        Some(result)
    }
}

impl<'t, S: PageSource + ?Sized> Iterator for Keys<'t, S> {
    type Item = Result<(&'t [u8], u64)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let item = self.range.next();
        self.counted(item)
    }
}

impl<S: PageSource + ?Sized> DoubleEndedIterator for Keys<'_, S> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let item = self.range.next_back();
        self.counted(item)
    }
}

impl<S: PageSource + ?Sized> std::iter::FusedIterator for Keys<'_, S> {}

/// Where a scan of values is.
enum State<'t, S: PageSource + ?Sized> {
    /// No page read yet.
    Unread,
    /// The values of the interval in the entry, the front's next one at `front` and the back's at `back - 1`.
    Inline {
        values: Vec<&'t [u8]>,
        front: usize,
        back: usize,
    },
    /// The values in a subtree: the empty value while it is still to come, and the scan of the non-empty values of
    /// the interval, if it holds any.
    Subtree {
        empty: bool,
        range: Option<Range<'t, S>>,
    },
    /// Nothing more comes out: the end was reached, the interval or the key holds nothing, or a step failed.
    Done,
}

/// A double-ended scan of the values of an interval of one key of a multi-value tree, in order, the empty value
/// first (M7). Values are borrowed from the transaction without copying them: those in the entry are part of the
/// parent's leaf, those in a subtree are its keys.
pub struct Values<'t, S: PageSource + ?Sized> {
    tx: &'t S,
    root: u64,
    key: Vec<u8>,
    interval: OwnedInterval,
    state: State<'t, S>,
}

/// A scan of the values of `interval` of `key` in the multi-value tree at `root`. It copies the key and the
/// interval, which may fail for lack of memory, and reads no page until it is used; an empty interval (R3) gives an
/// empty scan that never reads the key's entry, and so does a key that is not there.
///
/// # Panics
///
/// As [`remove_value_range`].
pub fn values<'t, S: PageSource + ?Sized>(
    tx: &'t S,
    root: u64,
    key: &[u8],
    interval: Interval<'_>,
) -> Result<Values<'t, S>> {
    check_key(key);
    check_value_interval(&interval);
    let state = if interval.is_empty_of_values() {
        State::Done
    } else {
        State::Unread
    };
    Ok(Values {
        tx,
        root,
        key: copy(tx, key)?,
        interval: OwnedInterval::new(tx, interval)?,
        state,
    })
}

impl<'t, S: PageSource + ?Sized> Values<'t, S> {
    /// Reads the key's entry and sets where the scan starts.
    fn start(&mut self) -> Result<State<'t, S>> {
        let interval = self.interval.borrow();
        Ok(match entry(self.tx, self.root, &self.key)? {
            None => State::Done,
            Some((_, Entry::Inline { count, elements })) => {
                let mut values = Vec::new();
                reserve(self.tx, &mut values, count as usize)?;
                values.extend(Elements(elements).filter(|v| interval.contains(v)));
                State::Inline {
                    front: 0,
                    back: values.len(),
                    values,
                }
            }
            Some((_, Entry::Subtree(subtree))) => State::Subtree {
                empty: subtree.has_empty && interval.contains(b""),
                range: non_empty_part(&interval)
                    .map(|keys| Range::new(self.tx, subtree.root, keys))
                    .transpose()?,
            },
        })
    }

    /// The next value from the front or from the back; after the end or a failure, nothing more comes out (R8).
    fn step(&mut self, front: bool) -> Option<Result<&'t [u8]>> {
        if let State::Unread = self.state {
            match self.start() {
                Ok(state) => self.state = state,
                Err(error) => {
                    self.state = State::Done;
                    return Some(Err(error));
                }
            }
        }
        let empty: &'t [u8] = &[];
        let item = match &mut self.state {
            State::Unread | State::Done => None,
            State::Inline {
                values,
                front: next,
                back,
            } => {
                if next == back {
                    None
                } else if front {
                    *next += 1;
                    Some(Ok(values[*next - 1]))
                } else {
                    *back -= 1;
                    Some(Ok(values[*back]))
                }
            }
            State::Subtree {
                empty: pending,
                range,
            } => {
                if front && *pending {
                    *pending = false;
                    Some(Ok(empty))
                } else {
                    let step = match range {
                        Some(range) if front => range.next(),
                        Some(range) => range.next_back(),
                        None => None,
                    };
                    match step {
                        Some(result) => Some(result.map(|(value, _)| value)),
                        // The back reaches the empty value after every other one.
                        None if *pending => {
                            *pending = false;
                            Some(Ok(empty))
                        }
                        None => None,
                    }
                }
            }
        };
        if !matches!(item, Some(Ok(_))) {
            self.state = State::Done;
        }
        item
    }
}

impl<'t, S: PageSource + ?Sized> Iterator for Values<'t, S> {
    type Item = Result<&'t [u8]>;

    fn next(&mut self) -> Option<Self::Item> {
        self.step(true)
    }
}

impl<S: PageSource + ?Sized> DoubleEndedIterator for Values<'_, S> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.step(false)
    }
}

impl<S: PageSource + ?Sized> std::iter::FusedIterator for Values<'_, S> {}
