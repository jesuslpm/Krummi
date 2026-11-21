//! The statistics of a table's columns (statistics PRD 7.3): what a build of one snapshot keeps per column (ST1), how
//! it is stored in the table's statistics tree, and how stale it is (ST5).
//!
//! A build reads the rows of a snapshot and keeps, per column that is not a vector: the null fraction, the fraction of
//! large values (`string` or `bytes` of more than [`LARGE`] bytes, never read), an estimate of the distinct values,
//! the most frequent values and an equi-depth histogram of the others. Values are kept as the elements of the key
//! encoding of their column (document 3), so they compare as bytes in the order of N5. Fractions are of the rows the
//! build read.

use std::borrow::Cow;

use super::key::KeyColumn;
use super::value::{ColumnType, Value};

/// Tables of up to this many rows are read whole; larger ones give about this many rows by whole leaves (ST1).
pub(crate) const WHOLE: u64 = 30_000;
/// A `string` or `bytes` value of more than this many bytes is large: never read nor kept (ST1).
pub(crate) const LARGE: usize = 2_048;
/// The most frequent values a column keeps.
const FREQUENT: usize = 10;
/// The buckets of a column's histogram.
const BUCKETS: usize = 100;
/// A histogram bound of `string` or `bytes` is shortened to at most this many bytes (ST16).
const BOUND: usize = 64;

/// The kind byte of a column's entry in the statistics tree.
const COLUMN: u8 = 0;
/// The kind byte of an index prefix's entry in the statistics tree (ST17).
const PREFIX: u8 = 1;

/// The statistics of one column, from one snapshot (ST1).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColumnStats {
    /// The table's rows in the snapshot and the rows read.
    pub rows: u64,
    pub read: u64,
    /// The table's modification counter in the snapshot (ST4).
    pub counter: u64,
    pub nulls: f64,
    pub large: f64,
    /// The estimated distinct values that are not null nor large, in the whole table.
    pub distinct: f64,
    /// Elements and their fractions, from the most frequent.
    pub frequent: Vec<(Vec<u8>, f64)>,
    pub histogram: Vec<Bucket>,
    /// The id of the last commit its snapshot saw, while in memory; 0 once read from the tree, which does not keep it.
    pub snapshot: u64,
}

/// A bucket of a histogram: the values up to `upper`, after the previous bucket's.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Bucket {
    pub upper: Vec<u8>,
    pub fraction: f64,
    pub shortened: bool,
}

/// The key of a column's entry in the statistics tree.
pub(crate) fn column_key(column: u64) -> [u8; 9] {
    let mut key = [COLUMN; 9];
    key[1..].copy_from_slice(&column.to_be_bytes());
    key
}

/// The key of the entry of the prefix of `columns` key columns of the index `index` in the statistics tree (ST17).
pub(crate) fn prefix_key(index: u64, columns: u8) -> [u8; 10] {
    let mut key = [PREFIX; 10];
    key[1..9].copy_from_slice(&index.to_be_bytes());
    key[9] = columns;
    key
}

/// The modifications after which statistics of `rows` rows are stale: SQL Server's threshold (N8).
pub(crate) fn threshold(rows: u64) -> f64 {
    let rows = rows as f64;
    if rows <= 500.0 {
        500.0
    } else {
        (500.0 + 0.2 * rows).min((1000.0 * rows).sqrt())
    }
}

/// Whether statistics of `rows` rows and counter `origin` are stale for a transaction that sees the counter `seen`
/// (ST5); never when they are of a later snapshot.
pub(crate) fn is_stale(rows: u64, origin: u64, seen: u64) -> bool {
    seen.saturating_sub(origin) as f64 > threshold(rows)
}

/// The key column that encodes the values of a column of type `ty`.
pub(crate) fn element_column(ty: ColumnType) -> KeyColumn {
    KeyColumn::primary(ty)
}

/// Whether a column of type `ty` has statistics: every type but vectors (ST1) and JSON (JS10 of the JSON columns
/// PRD).
pub(crate) fn has_stats(ty: &ColumnType) -> bool {
    !matches!(ty, ColumnType::Vector(_) | ColumnType::Json)
}

/// Whether `value` is large (ST1).
pub(crate) fn is_large(value: &Value<'_>) -> bool {
    match value {
        Value::String(text) => text.len() > LARGE,
        Value::Bytes(bytes) => bytes.len() > LARGE,
        _ => false,
    }
}

/// What a build gathers for one column while it reads rows.
pub(crate) struct Gather {
    column: KeyColumn,
    nulls: u64,
    large: u64,
    values: Vec<Vec<u8>>,
}

impl Gather {
    pub(crate) fn new(ty: ColumnType) -> Gather {
        Gather {
            column: element_column(ty),
            nulls: 0,
            large: 0,
            values: Vec::new(),
        }
    }

    /// Counts the combination of one row read, the key elements of an index prefix (ST17).
    pub(crate) fn add_element(&mut self, element: Vec<u8>) {
        self.values.push(element);
    }

    /// The statistics of an index prefix: the distinct combinations and the most frequent ones, without a
    /// histogram (ST17).
    pub(crate) fn finish_combinations(
        self,
        rows: u64,
        read: u64,
        counter: u64,
        cancel: &dyn Fn() -> bool,
    ) -> Option<ColumnStats> {
        let mut stats = self.finish(rows, read, counter, cancel)?;
        stats.histogram.clear();
        Some(stats)
    }

    /// Counts a large value of one row read, which was not read (ST1).
    pub(crate) fn add_large(&mut self) {
        self.large += 1;
    }

    /// Counts the value of one row read.
    pub(crate) fn add(&mut self, value: &Value<'_>) {
        if let Value::Null = value {
            self.nulls += 1;
        } else if is_large(value) {
            self.large += 1;
        } else {
            let mut element = Vec::new();
            // A stored value always encodes in its column's key encoding.
            if self.column.encode(value, &mut element).is_ok() {
                self.values.push(element);
            }
        }
    }

    /// The statistics of the column, from `read` rows of a snapshot of `rows` rows and counter `counter`.
    /// The statistics of the column, from `read` rows of a snapshot of `rows` rows and counter `counter`, or `None`
    /// once `cancel` says so, between its steps.
    pub(crate) fn finish(
        mut self,
        rows: u64,
        read: u64,
        counter: u64,
        cancel: &dyn Fn() -> bool,
    ) -> Option<ColumnStats> {
        if cancel() {
            return None;
        }
        let of = |n: u64| {
            if read == 0 {
                0.0
            } else {
                n as f64 / read as f64
            }
        };
        self.values.sort_unstable();
        if cancel() {
            return None;
        }
        // Runs of equal elements: (element, count), in order.
        let mut runs: Vec<(Vec<u8>, u64)> = Vec::new();
        for value in self.values {
            match runs.last_mut() {
                Some((last, count)) if *last == value => *count += 1,
                _ => runs.push((value, 1)),
            }
        }
        let sampled = runs.len() as u64;
        let singles = runs.iter().filter(|(_, count)| *count == 1).count() as u64;
        let kept: u64 = runs.iter().map(|(_, count)| count).sum();
        let distinct = if read >= rows || kept == 0 {
            sampled as f64
        } else {
            // Duj1 (Haas and Stokes): n d / (n - f1 + f1 n / N), with N the table's rows that are kept values.
            let n = kept as f64;
            let total = (n * rows as f64 / read as f64).max(n);
            let estimate = n * sampled as f64 / (n - singles as f64 + singles as f64 * n / total);
            estimate.clamp(sampled as f64, total)
        };
        // The most frequent values: the greatest counts, ties in value order; a value seen once is frequent only
        // when the table was read whole.
        let mut order: Vec<usize> = (0..runs.len()).collect();
        order.sort_by(|&a, &b| runs[b].1.cmp(&runs[a].1).then(a.cmp(&b)));
        let whole = read >= rows;
        let frequent_at: Vec<usize> = order
            .into_iter()
            .filter(|&at| whole || runs[at].1 > 1)
            .take(FREQUENT)
            .collect();
        let frequent = frequent_at
            .iter()
            .map(|&at| (runs[at].0.clone(), of(runs[at].1)))
            .collect();
        // The equi-depth histogram of the other values.
        let others: Vec<&(Vec<u8>, u64)> = (runs.iter().enumerate())
            .filter(|(at, _)| !frequent_at.contains(at))
            .map(|(_, run)| run)
            .collect();
        let total: u64 = others.iter().map(|(_, count)| count).sum();
        let per_bucket = total.div_ceil(BUCKETS as u64).max(1);
        let mut histogram: Vec<Bucket> = Vec::new();
        let mut in_bucket = 0;
        // The previous bucket's real greatest value: a shortened bound lies after it (R4 of the code review).
        let mut previous: Option<&[u8]> = None;
        for (at, (element, count)) in others.iter().enumerate() {
            in_bucket += count;
            if in_bucket >= per_bucket || at + 1 == others.len() {
                let (upper, shortened) = shorten(&self.column, element, previous);
                previous = Some(element);
                histogram.push(Bucket {
                    upper,
                    fraction: of(in_bucket),
                    shortened,
                });
                in_bucket = 0;
            }
        }
        Some(ColumnStats {
            rows,
            read,
            counter,
            nulls: of(self.nulls),
            large: of(self.large),
            distinct,
            frequent,
            histogram,
            snapshot: 0,
        })
    }
}

/// The element `element` of `column`, shortened when its text or bytes pass [`BOUND`] bytes: to the shortest prefix of
/// at least [`BOUND`] bytes (the text at a character boundary) whose element is still greater than `previous`, the
/// previous bucket's bound, so that the bounds keep their order; re-encoded; and whether it was.
fn shorten(column: &KeyColumn, element: &[u8], previous: Option<&[u8]>) -> (Vec<u8>, bool) {
    let encode = |value: &Value<'_>| {
        let mut upper = Vec::new();
        column
            .encode(value, &mut upper)
            .expect("a prefix of a value encodes");
        upper
    };
    let after = |upper: &[u8]| previous.is_none_or(|p| upper > p);
    match column.decode(&mut &element[..]) {
        Some(Value::String(text)) if text.len() > BOUND => {
            let mut end = BOUND;
            while end < text.len() {
                if text.is_char_boundary(end) {
                    let upper = encode(&Value::String(Cow::Borrowed(&text[..end])));
                    if after(&upper) {
                        return (upper, true);
                    }
                }
                end += 1;
            }
            (element.to_vec(), false)
        }
        Some(Value::Bytes(bytes)) if bytes.len() > BOUND => {
            for end in BOUND..bytes.len() {
                let upper = encode(&Value::Bytes(Cow::Borrowed(&bytes[..end])));
                if after(&upper) {
                    return (upper, true);
                }
            }
            (element.to_vec(), false)
        }
        _ => (element.to_vec(), false),
    }
}

impl ColumnStats {
    /// Its value in the statistics tree.
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for n in [self.rows, self.read, self.counter] {
            out.extend(n.to_le_bytes());
        }
        for f in [self.nulls, self.large, self.distinct] {
            out.extend(f.to_le_bytes());
        }
        out.extend(
            u16::try_from(self.frequent.len())
                .expect("few")
                .to_le_bytes(),
        );
        for (element, fraction) in &self.frequent {
            put_bytes(&mut out, element);
            out.extend(fraction.to_le_bytes());
        }
        out.extend(
            u16::try_from(self.histogram.len())
                .expect("few")
                .to_le_bytes(),
        );
        for bucket in &self.histogram {
            put_bytes(&mut out, &bucket.upper);
            out.extend(bucket.fraction.to_le_bytes());
            out.push(u8::from(bucket.shortened));
        }
        out
    }

    /// The statistics that [`encode`](Self::encode) wrote in `bytes`, or `None` when they are not its layout.
    pub(crate) fn decode(bytes: &[u8]) -> Option<ColumnStats> {
        let mut r = bytes;
        let rows = take_u64(&mut r)?;
        let read = take_u64(&mut r)?;
        let counter = take_u64(&mut r)?;
        let nulls = f64::from_bits(take_u64(&mut r)?);
        let large = f64::from_bits(take_u64(&mut r)?);
        let distinct = f64::from_bits(take_u64(&mut r)?);
        let mut frequent = Vec::new();
        for _ in 0..take_u16(&mut r)? {
            let element = take_bytes(&mut r)?;
            frequent.push((element, f64::from_bits(take_u64(&mut r)?)));
        }
        let mut histogram = Vec::new();
        for _ in 0..take_u16(&mut r)? {
            let upper = take_bytes(&mut r)?;
            let fraction = f64::from_bits(take_u64(&mut r)?);
            let shortened = match take(&mut r, 1)?[0] {
                0 => false,
                1 => true,
                _ => return None,
            };
            histogram.push(Bucket {
                upper,
                fraction,
                shortened,
            });
        }
        r.is_empty().then_some(ColumnStats {
            rows,
            read,
            counter,
            nulls,
            large,
            distinct,
            frequent,
            histogram,
            snapshot: 0,
        })
    }
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend(u32::try_from(bytes.len()).expect("short").to_le_bytes());
    out.extend(bytes);
}

fn take<'b>(r: &mut &'b [u8], n: usize) -> Option<&'b [u8]> {
    let (head, rest) = r.split_at_checked(n)?;
    *r = rest;
    Some(head)
}

fn take_u64(r: &mut &[u8]) -> Option<u64> {
    Some(u64::from_le_bytes(take(r, 8)?.try_into().ok()?))
}

fn take_u16(r: &mut &[u8]) -> Option<u16> {
    Some(u16::from_le_bytes(take(r, 2)?.try_into().ok()?))
}

fn take_bytes(r: &mut &[u8]) -> Option<Vec<u8>> {
    let len = u32::from_le_bytes(take(r, 4)?.try_into().ok()?);
    Some(take(r, usize::try_from(len).ok()?)?.to_vec())
}
