//! The searches of the k nearest neighbours of a vector (VI5–VI8 and VI16 of the vector indexes PRD): the exact one,
//! which computes the distance of every row that passes its filter, and the one through a vector index.
//!
//! The rows that **pass** are those that meet the filter ([`FilterCondition`], joined with AND) and whose vector is not
//! null (VI6). A search returns up to k of them, each with the values of its primary key, decoded into copies of their
//! own, and its distance in the units of VI8, from the nearest; rows at the same compared distance (the square, for
//! L2) come in the order of their primary keys. The rows are read afterwards by their primary key.
//!
//! ```
//! use krummi::table::value::{ColumnType, Value, VectorElement, VectorType};
//! use krummi::{Column, FilterCondition, Nearest, Schema, Store, StoreOptions, TableMode, VectorMetric};
//!
//! let store = Store::open_temporary(&StoreOptions::default())?;
//! let mut write = store.write()?;
//! let ty = VectorType { element: VectorElement::F32, dimension: 2, cosine: false };
//! let schema = Schema::new(
//!     vec![
//!         Column::new("id", ColumnType::I64).not_null(),
//!         Column::new("color", ColumnType::String),
//!         Column::new("embedding", ColumnType::Vector(ty)),
//!     ],
//!     &["id"],
//! );
//! let table = write.open_table_with("items", TableMode::CreateNew(&schema))?;
//! let vector = |x: f32, y: f32| Value::Vector([x.to_le_bytes(), y.to_le_bytes()].concat().into());
//! for (id, color, x) in [(1, "red", 0.0), (2, "blue", 1.0), (3, "red", 3.0)] {
//!     let row = [("id", Value::I64(id)), ("color", Value::String(color.into())), ("embedding", vector(x, 0.0))];
//!     write.insert_row(&table, &row)?;
//! }
//! write.commit()?;
//!
//! let read = store.read()?;
//! let table = read.open_table("items")?;
//! let red = vec![FilterCondition::Equal { column: "color", value: Value::String("red".into()) }];
//! let query = Nearest::new(vector(2.0, 0.0), 2).with_filter(red);
//! let found = read.nearest_exact(&table, "embedding", VectorMetric::L2, &query)?;
//! assert!(found.exact);
//! let ids: Vec<(Value<'static>, f64)> = found.rows.iter().map(|n| (n.key[0].clone(), n.distance)).collect();
//! assert_eq!(ids, [(Value::I64(3), 1.0), (Value::I64(1), 2.0)]);
//! # Ok::<(), krummi::Error>(())
//! ```

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::ops::Bound::Unbounded;
use std::sync::Arc;

use krummi_pager::{Error, Result, ValueRule};
use krummi_tree::{Interval, PageSource};

use super::catalog::{ReadTable, WriteTable};
use super::distance::{Exact, VectorMetric};
use super::filter::{Candidates, Filter, FilterCondition, index_keys};
use super::rows::{At, Reader, TableRow, Versions};
use super::schema::{SchemaColumn, TableSchema};
use super::value::{ColumnType, Value, VectorType, normalize};
use super::vector_index::VectorRoots;
use crate::{Folder, ReadTransaction, WriteTransaction};

/// The most neighbours a search returns (document 5, question 9).
pub(crate) const MAX_K: usize = 10_000;

/// A search of the k nearest neighbours of a vector (VI5, VI16): the query vector, a value of the searched column's
/// type; k, from 1 to 10,000; the filter, conditions joined with AND (none by default: every row with a vector
/// passes); and, for a search through an index, how many lists to probe first (by default, a number the index
/// chooses).
#[derive(Debug, Clone, PartialEq)]
pub struct Nearest<'a> {
    pub vector: Value<'a>,
    pub k: usize,
    pub filter: Vec<FilterCondition<'a>>,
    pub probes: Option<usize>,
}

impl<'a> Nearest<'a> {
    /// The `k` nearest neighbours of `vector`, without a filter.
    pub fn new(vector: Value<'a>, k: usize) -> Nearest<'a> {
        Nearest {
            vector,
            k,
            filter: Vec::new(),
            probes: None,
        }
    }

    /// The search with the filter `filter`.
    pub fn with_filter(self, filter: Vec<FilterCondition<'a>>) -> Nearest<'a> {
        Nearest { filter, ..self }
    }

    /// The search probing `probes` lists first, when it goes through an index (VI6).
    pub fn with_probes(self, probes: usize) -> Nearest<'a> {
        Nearest {
            probes: Some(probes),
            ..self
        }
    }
}

/// What a search finds (VI5, VI6, *Salida*): up to k rows, the nearest first, and whether the result is exact or
/// approximate. An exact search, and a search through an index while the index is not ready, are always exact.
#[derive(Debug, Clone, PartialEq)]
pub struct Neighbours {
    pub exact: bool,
    pub rows: Vec<Neighbour>,
}

/// A row a search found: the values of its primary key, in key order, and its distance in the units of VI8.
#[derive(Debug, Clone, PartialEq)]
pub struct Neighbour {
    pub key: Vec<Value<'static>>,
    pub distance: f64,
}

/// A row kept by [`TopK`]: its compared key, its encoded primary key and the values of its primary key.
struct Kept {
    key: f64,
    primary: Vec<u8>,
    values: Vec<Value<'static>>,
}

impl Kept {
    fn order(&self, other: &Kept) -> Ordering {
        (self.key.total_cmp(&other.key)).then_with(|| self.primary.cmp(&other.primary))
    }
}

impl PartialEq for Kept {
    fn eq(&self, other: &Self) -> bool {
        self.order(other) == Ordering::Equal
    }
}

impl Eq for Kept {}

impl PartialOrd for Kept {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Kept {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order(other)
    }
}

/// The best k rows by (compared key, primary key): a max-heap whose top is the worst kept.
pub(crate) struct TopK {
    k: usize,
    heap: BinaryHeap<Kept>,
}

impl TopK {
    pub(crate) fn new(k: usize) -> TopK {
        TopK {
            k,
            heap: BinaryHeap::with_capacity(k.min(MAX_K) + 1),
        }
    }

    /// Whether a row with `key` and `primary` would be kept.
    pub(crate) fn admits(&self, key: f64, primary: &[u8]) -> bool {
        if self.heap.len() < self.k {
            return true;
        }
        let worst = self.heap.peek().expect("k is at least 1");
        match key.total_cmp(&worst.key) {
            Ordering::Less => true,
            Ordering::Equal => primary < worst.primary.as_slice(),
            Ordering::Greater => false,
        }
    }

    /// Keeps the row unless k better ones are kept; `values` gives its primary key's values when it is kept.
    pub(crate) fn push(
        &mut self,
        key: f64,
        primary: &[u8],
        values: impl FnOnce() -> Vec<Value<'static>>,
    ) {
        if !self.admits(key, primary) {
            return;
        }
        self.heap.push(Kept {
            key,
            primary: primary.to_vec(),
            values: values(),
        });
        if self.heap.len() > self.k {
            self.heap.pop();
        }
    }

    /// Keeps the best of `other`'s rows too.
    pub(crate) fn merge(&mut self, other: TopK) {
        for kept in other.heap {
            if self.admits(kept.key, &kept.primary) {
                self.heap.push(kept);
                if self.heap.len() > self.k {
                    self.heap.pop();
                }
            }
        }
    }

    /// The rows kept, the nearest first, with their distances in `metric`'s units.
    pub(crate) fn into_neighbours(self, metric: VectorMetric) -> Vec<Neighbour> {
        (self.heap.into_sorted_vec().into_iter())
            .map(|kept| Neighbour {
                key: kept.values,
                distance: metric.distance(kept.key),
            })
            .collect()
    }
}

/// A table as a search reads it, in a read or a write transaction.
pub(crate) struct Searched<'a> {
    pub(crate) folder: Folder<'a>,
    pub(crate) name: &'a str,
    pub(crate) rows_root: u64,
    pub(crate) history: u64,
    pub(crate) reader: &'a Arc<Reader>,
    pub(crate) schema: &'a TableSchema,
    pub(crate) index_roots: &'a [u64],
    pub(crate) vector_roots: &'a [VectorRoots],
}

impl Searched<'_> {
    fn at(&self) -> At<'_> {
        At {
            name: self.name,
            rows_root: self.rows_root,
            history: self.history,
        }
    }

    fn invalid(&self, column: Option<&str>, rule: ValueRule) -> Error {
        Error::InvalidValue {
            json: None,
            folder: self.folder.owned(),
            table: Some(self.name.to_owned()),
            column: column.map(str::to_owned),
            sequence: None,
            rule,
        }
    }

    /// The vector column `name` and its type, with the failures of the exact search: a column the table does not have
    /// (a different schema); one that is not a vector, a metric it does not admit (VI16), invalid values.
    pub(crate) fn vector_column(&self, name: &str) -> Result<(&SchemaColumn, VectorType)> {
        self.folder.check_column_name(name)?;
        let Some(column) = self.schema.column(name) else {
            return Err(Error::DifferentSchema {
                folder: self.folder.owned(),
                table: self.name.to_owned(),
                column: Some(name.to_owned()),
                difference: krummi_pager::SchemaDifference::MissingColumn,
                expected: "a column".into(),
                found: "no column".into(),
            });
        };
        match column.ty {
            ColumnType::Vector(vector) => Ok((column, vector)),
            _ => Err(self.invalid(Some(name), ValueRule::WrongType)),
        }
    }

    /// The query vector of `query` for `column` of type `vector`, normalized (N4), and the checks of k and of the
    /// probes (*Validaciones*).
    pub(crate) fn check_query<'q>(
        &self,
        column: &SchemaColumn,
        vector: VectorType,
        query: &'q Nearest<'_>,
    ) -> Result<Cow<'q, [u8]>> {
        let bytes = match &query.vector {
            Value::Null => return Err(self.invalid(Some(&column.name), ValueRule::NullNotAllowed)),
            value => match normalize(&column.ty, false, value.reborrow()) {
                Ok(Value::Vector(bytes)) => bytes,
                Ok(_) => unreachable!("a vector column normalizes to a vector"),
                Err(rule) => return Err(self.invalid(Some(&column.name), rule)),
            },
        };
        debug_assert_eq!(bytes.len(), vector.value_len());
        if !(1..=MAX_K).contains(&query.k) || query.probes == Some(0) {
            return Err(self.invalid(None, ValueRule::InvalidRange));
        }
        Ok(bytes)
    }

    /// Calls `visit` with the encoded primary key, the row and the vector of each row that passes `filter` (VI6), in
    /// the order its candidates come; stops when `visit` returns false.
    pub(crate) fn passing<'t, S: PageSource + ?Sized>(
        &'t self,
        tx: &'t S,
        column: usize,
        filter: &Filter,
        mut visit: impl FnMut(&[u8], &TableRow<'t>, &[u8]) -> Result<bool>,
    ) -> Result<()> {
        let mut versions = Versions::default();
        let mut scratch = Vec::new();
        let mut one = |key: &'t [u8],
                       bytes: &'t [u8],
                       versions: &mut Versions<'t>|
         -> Result<bool> {
            let row =
                self.reader
                    .decode(tx, self.at(), versions, Cow::Borrowed(key), bytes, false)?;
            let Some(Value::Vector(vector)) = row.value(column) else {
                return Ok(true);
            };
            if !filter.meets(|i| row.value(i).expect("a live column"), &mut scratch) {
                return Ok(true);
            }
            visit(key, &row, &vector)
        };
        match filter.candidates(self.schema) {
            Candidates::None => Ok(()),
            Candidates::Table => {
                let every_row = Interval {
                    prefix: &[],
                    lower: Unbounded,
                    upper: Unbounded,
                };
                for entry in krummi_tree::Range::new(tx, self.rows_root, every_row)? {
                    let (key, bytes) = entry?;
                    if !one(key, bytes, &mut versions)? {
                        break;
                    }
                }
                Ok(())
            }
            Candidates::Index { at, key, ranges } => {
                let root = self.index_roots[at];
                index_keys(tx, root, &key, &ranges, |primary| {
                    let Some(bytes) = krummi_tree::get(tx, self.rows_root, primary)? else {
                        // An entry whose row is not in the table is damage of the index (IX12).
                        return Err(match krummi_tree::leaf_of(tx, root, primary) {
                            Ok(page) => tx.damaged(page),
                            Err(error) => error,
                        });
                    };
                    one(primary, bytes, &mut versions)
                })
            }
        }
    }

    /// The values of the primary key of `row`, in key order, in copies of their own.
    pub(crate) fn key_values(&self, row: &TableRow<'_>) -> Vec<Value<'static>> {
        (self.schema.key_ids().iter())
            .map(|id| {
                let place = self.schema.column_position(*id).expect("a key column");
                row.value(place).expect("a live column").into_owned()
            })
            .collect()
    }

    /// The exact search (VI16) of the column `column` with `metric`.
    pub(crate) fn exact<S: PageSource + ?Sized>(
        &self,
        tx: &S,
        column: &str,
        metric: VectorMetric,
        query: &Nearest<'_>,
    ) -> Result<Neighbours> {
        let (schema_column, vector) = self.vector_column(column)?;
        if !metric.fits(&vector) {
            return Err(self.invalid(Some(column), ValueRule::WrongType));
        }
        let bytes = self.check_query(schema_column, vector, query)?;
        let filter = Filter::check(self.folder, self.name, self.schema, &query.filter)?;
        let place = self
            .schema
            .column_position(schema_column.id)
            .expect("a live column");
        let exact = Exact::new(vector, metric, &bytes);
        let mut top = TopK::new(query.k);
        self.passing(tx, place, &filter, |primary, row, value| {
            top.push(exact.key(value), primary, || self.key_values(row));
            Ok(true)
        })?;
        Ok(Neighbours {
            exact: true,
            rows: top.into_neighbours(metric),
        })
    }
}

impl ReadTransaction<'_> {
    /// The k nearest neighbours of `query`'s vector among the rows of `table` that pass its filter, by the exact
    /// distance `metric` on the vector column `column` (VI16): it computes the distance of every row that passes and
    /// never uses a vector index. The result is always exact. Fails with
    /// [`ErrorKind::Misuse`](crate::ErrorKind::Misuse) for a table opened in another transaction; with
    /// [`ErrorKind::InvalidSize`](crate::ErrorKind::InvalidSize) for a column name without 1 to 1,024 bytes; with
    /// [`ErrorKind::DifferentSchema`](crate::ErrorKind::DifferentSchema) for a column the table does not have; with
    /// [`ErrorKind::InvalidValue`](crate::ErrorKind::InvalidValue) for a column that is not a vector, a metric it does
    /// not admit (Hamming on `bit`, the cosine on a cosine column, L2 and the dot product on the others), a missing
    /// query vector or one that breaks the column's rules (N4), k outside 1 to 10,000 or zero lists to probe; then
    /// with the failures of the filter (see [`FilterCondition`]); and with damage of a page it reads.
    pub fn nearest_exact(
        &self,
        table: &ReadTable<'_>,
        column: &str,
        metric: VectorMetric,
        query: &Nearest<'_>,
    ) -> Result<Neighbours> {
        self.folder.check_column_name(column)?;
        self.check_table(table)?;
        table
            .searched(self.folder)
            .exact(&self.inner, column, metric, query)
    }
}

impl ReadTable<'_> {
    pub(crate) fn searched<'a>(&'a self, folder: Folder<'a>) -> Searched<'a> {
        Searched {
            folder,
            name: &self.name,
            rows_root: self.rows_root,
            history: self.history,
            reader: &self.reader,
            schema: &self.schema,
            index_roots: &self.index_roots,
            vector_roots: &self.vector_roots,
        }
    }
}

/// What a search in a write transaction reads of a table: an owned copy of its state, so that the search borrows no
/// state of the transaction while it reads.
pub(crate) struct OwnedSearched {
    pub(crate) name: String,
    pub(crate) rows_root: u64,
    pub(crate) history: u64,
    pub(crate) reader: Arc<Reader>,
    pub(crate) schema: TableSchema,
    pub(crate) index_roots: Vec<u64>,
    pub(crate) vector_roots: Vec<VectorRoots>,
}

impl OwnedSearched {
    pub(crate) fn searched<'a>(&'a self, folder: Folder<'a>) -> Searched<'a> {
        Searched {
            folder,
            name: &self.name,
            rows_root: self.rows_root,
            history: self.history,
            reader: &self.reader,
            schema: &self.schema,
            index_roots: &self.index_roots,
            vector_roots: &self.vector_roots,
        }
    }
}

impl WriteTransaction<'_> {
    /// The table of `table` as this write sees it, for a search.
    pub(crate) fn owned_searched(&self, table: &WriteTable<'_>) -> Result<OwnedSearched> {
        let state = self.live_table(table)?;
        Ok(OwnedSearched {
            name: state.name.clone(),
            rows_root: state.rows_root,
            history: state.history,
            reader: Arc::clone(&state.reader),
            schema: state.schema.clone(),
            index_roots: state.index_roots.clone(),
            vector_roots: state.vector_roots.clone(),
        })
    }

    /// [`ReadTransaction::nearest_exact`] over the rows this write sees, with its failures, and with
    /// [`ErrorKind::TreeNotFound`](crate::ErrorKind::TreeNotFound) for a handle of a table the write deleted. It is not a
    /// change for T10 and ends no reference.
    pub fn nearest_exact(
        &self,
        table: &WriteTable<'_>,
        column: &str,
        metric: VectorMetric,
        query: &Nearest<'_>,
    ) -> Result<Neighbours> {
        self.folder.check_column_name(column)?;
        let owned = self.owned_searched(table)?;
        owned
            .searched(self.folder)
            .exact(&self.inner, column, metric, query)
    }
}

/// The rows that pass at most which a search through an index computes every exact distance instead (VI6), measured
/// (*Dejado a la implementación*).
pub(crate) const EXACT_THRESHOLD: u64 = 20_000;
/// The share of the lists a search probes first when it does not say (VI6).
const DEFAULT_PROBE_SHARE: f64 = 0.04;
/// The candidates a search reranks per neighbour, and at least (VI6, *Dejado a la implementación*).
const RERANK_PER_NEIGHBOUR: usize = 2;
const RERANK_AT_LEAST: usize = 20;

impl Searched<'_> {
    /// The search of VI5 and VI6 through the vector index `index` (filter first, then distance).
    pub(crate) fn approximate<S: PageSource + ?Sized>(
        &self,
        shared: &crate::Shared,
        tx: &S,
        index: &str,
        query: &Nearest<'_>,
    ) -> Result<Neighbours> {
        let at = (self.schema.vector_index_position(index))
            .ok_or_else(|| self.folder.missing_vector_index(self.name, index))?;
        let definition = &self.schema.vector_indexes()[at];
        let (id, vector) = super::vector_index::vector_of(self.schema, at);
        let column = self
            .schema
            .column_by_id(definition.column.id)
            .expect("a live column of the index");
        let metric = definition.metric;
        let bytes = self.check_query(column, vector, query)?;
        let filter = Filter::check(self.folder, self.name, self.schema, &query.filter)?;
        let place = self
            .schema
            .column_position(column.id)
            .expect("a live column");
        let roots = self.vector_roots[at];
        let state = super::vector_index::read_state(tx, &roots)?;
        let exact = Exact::new(vector, metric, &bytes);
        let hooks = shared.vectors.search_hooks();
        let threshold = hooks.threshold.unwrap_or(EXACT_THRESHOLD);
        // No generation ready: the exact distance of every row that passes (VI5).
        let exact_all = |top: &mut TopK| {
            self.passing(tx, place, &filter, |primary, row, value| {
                top.push(exact.key(value), primary, || self.key_values(row));
                Ok(true)
            })
        };
        if state.active == 0 {
            let mut top = TopK::new(query.k);
            exact_all(&mut top)?;
            return Ok(Neighbours {
                exact: true,
                rows: top.into_neighbours(metric),
            });
        }
        // The rows the index does not reflect yet (VI5): evaluated exactly, those that pass with their distances.
        let mut pending: HashSet<Vec<u8>> = HashSet::new();
        let all = Interval {
            prefix: &[],
            lower: Unbounded,
            upper: Unbounded,
        };
        for entry in krummi_tree::Range::new(tx, roots.pending, all)? {
            pending.insert(entry?.0.to_vec());
        }
        if filter.has_conditions() {
            // Filter first (VI6): the rows that pass, with their exact distances, as long as they stay under the
            // threshold; past it, the count is all that decides, so the scan stops.
            let mut top = TopK::new(query.k);
            let mut count = 0u64;
            self.passing(tx, place, &filter, |primary, row, value| {
                count += 1;
                top.push(exact.key(value), primary, || self.key_values(row));
                Ok(count <= threshold)
            })?;
            if count <= threshold {
                return Ok(Neighbours {
                    exact: true,
                    rows: top.into_neighbours(metric),
                });
            }
        } else {
            // Without a filter every row with a vector passes: those the generation holds outside the pending rows,
            // and the pending rows that have one.
            let (entries, _) = super::vector_worker::read_counters(tx, roots.data, state.active)?;
            let mut held = 0u64;
            for key in &pending {
                let locator = super::vector_worker::locator_key(state.active, key);
                held += u64::from(krummi_tree::get(tx, roots.data, &locator)?.is_some());
            }
            let (_, with_vector) = self.pending_top(tx, place, 1, &exact, &pending, &filter)?;
            if entries - held.min(entries) + with_vector as u64 <= threshold {
                let mut top = TopK::new(query.k);
                exact_all(&mut top)?;
                return Ok(Neighbours {
                    exact: true,
                    rows: top.into_neighbours(metric),
                });
            }
        }
        let (mut top, pending_passed) =
            self.pending_top(tx, place, query.k, &exact, &pending, &filter)?;
        let generation =
            super::vector_worker::load_generation(shared, tx, id, &roots, &vector, state.active)?;
        let mut work = Vec::new();
        super::distance::work_vector(&vector, &bytes, &mut work);
        let space = match metric {
            VectorMetric::L2 => super::ivf::Space::L2,
            _ => super::ivf::Space::Dot,
        };
        let prepared = generation.prepare(space, &work);
        let order = generation.probe_order(&work);
        let probes = query.probes.unwrap_or_else(|| {
            ((f64::from(generation.lists) * DEFAULT_PROBE_SHARE).ceil() as usize).max(1)
        });
        let wanted = if hooks.rerank_all {
            usize::MAX
        } else {
            (query.k * RERANK_PER_NEIGHBOUR).max(RERANK_AT_LEAST)
        };
        // The postings of the probed lists by their SQ8 distance; in that order, each is checked against the filter and,
        // when it passes, gets its exact distance, until `wanted` passed. That is the best `wanted` of the rows that
        // pass in those lists, as if the rows that pass were found first and only those considered (VI6). While fewer
        // than k pass in the probed lists, the next lists in order join them.
        let mut pool: Vec<(f32, Vec<u8>)> = Vec::new();
        let mut checked: HashMap<Vec<u8>, bool> = HashMap::new();
        let mut versions = Versions::default();
        let mut scratch = Vec::new();
        let mut probed = 0;
        loop {
            let reach = if probed == 0 { probes } else { probed + 1 };
            for list in &order[probed.min(order.len())..reach.min(order.len())] {
                let prefix = super::vector_worker::list_key(
                    super::vector_index::tag::POSTING,
                    state.active,
                    *list,
                );
                let list_entries = Interval {
                    prefix: &prefix,
                    lower: Unbounded,
                    upper: Unbounded,
                };
                for entry in krummi_tree::Range::new(tx, roots.data, list_entries)? {
                    let (key, code) = entry?;
                    let primary = &key[prefix.len()..];
                    if !pending.contains(primary) {
                        pool.push((prepared.distance(code), primary.to_vec()));
                    }
                }
            }
            probed = reach.min(order.len());
            pool.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            let mut kept = TopK::new(query.k);
            let mut passed = 0usize;
            for (_, primary) in &pool {
                if passed >= wanted {
                    break;
                }
                if checked.get(primary) == Some(&false) {
                    continue;
                }
                let Some(stored) = krummi_tree::get(tx, self.rows_root, primary)? else {
                    // A posting of a row that is not pending names a row of the snapshot (I-ready).
                    return Err(match krummi_tree::leaf_of(tx, roots.data, primary) {
                        Ok(page) => tx.damaged(page),
                        Err(error) => error,
                    });
                };
                let row = self.reader.decode(
                    tx,
                    self.at(),
                    &mut versions,
                    Cow::Owned(primary.clone()),
                    stored,
                    false,
                )?;
                let meets = filter.meets(|i| row.value(i).expect("a live column"), &mut scratch);
                checked.insert(primary.clone(), meets);
                if !meets {
                    continue;
                }
                if let Some(Value::Vector(value)) = row.value(place) {
                    passed += 1;
                    kept.push(exact.key(&value), primary, || self.key_values(&row));
                }
            }
            if passed + pending_passed >= query.k || probed >= order.len() {
                top.merge(kept);
                break;
            }
        }
        Ok(Neighbours {
            exact: false,
            rows: top.into_neighbours(metric),
        })
    }

    /// The best `k` of the `pending` rows that exist with a vector and meet `filter`, by their exact distances, and how
    /// many of them there are.
    fn pending_top<S: PageSource + ?Sized>(
        &self,
        tx: &S,
        place: usize,
        k: usize,
        exact: &Exact<'_>,
        pending: &HashSet<Vec<u8>>,
        filter: &Filter,
    ) -> Result<(TopK, usize)> {
        let mut top = TopK::new(k);
        let mut count = 0;
        let mut versions = Versions::default();
        let mut scratch = Vec::new();
        for key in pending {
            let Some(stored) = krummi_tree::get(tx, self.rows_root, key)? else {
                continue;
            };
            let row = self.reader.decode(
                tx,
                self.at(),
                &mut versions,
                Cow::Borrowed(key),
                stored,
                false,
            )?;
            if !filter.meets(|i| row.value(i).expect("a live column"), &mut scratch) {
                continue;
            }
            if let Some(Value::Vector(value)) = row.value(place) {
                count += 1;
                top.push(exact.key(&value), key, || self.key_values(&row));
            }
        }
        Ok((top, count))
    }
}

impl ReadTransaction<'_> {
    /// The k nearest neighbours of `query`'s vector among the rows of `table` that pass its filter, through the vector
    /// index `index`, which decides the column and the metric (VI5, VI6). The filter is evaluated first: when at most a
    /// threshold of rows pass, it computes the exact distance of each of them and the result is exact; otherwise it
    /// probes the lists of the index nearest to the query, considering only the rows that passed, goes on to the next
    /// lists while fewer than k of them were found, and reranks the candidates with their exact distances, so it returns
    /// k rows whenever at least k pass; the result is then approximate. The rows the index does not reflect yet (its
    /// pending rows) are evaluated exactly, so the worker's lag hides none; while no generation is ready, the search is
    /// exact. Fails as [`nearest_exact`](Self::nearest_exact), with
    /// [`ErrorKind::InvalidSize`](crate::ErrorKind::InvalidSize) for an index name without 1 to 1,024 bytes and
    /// [`ErrorKind::DifferentSchema`](crate::ErrorKind::DifferentSchema) for a vector index the table does not have (a
    /// secondary index's name included), instead of the failures of the column and the metric.
    pub fn nearest(
        &self,
        table: &ReadTable<'_>,
        index: &str,
        query: &Nearest<'_>,
    ) -> Result<Neighbours> {
        self.folder.check_index_name(index)?;
        self.check_table(table)?;
        table
            .searched(self.folder)
            .approximate(self.store, &self.inner, index, query)
    }
}

impl WriteTransaction<'_> {
    /// [`ReadTransaction::nearest`] over the rows this write sees, with its failures, and with
    /// [`ErrorKind::TreeNotFound`](crate::ErrorKind::TreeNotFound) for a handle of a table the write deleted. It is not a
    /// change for T10 and ends no reference.
    pub fn nearest(
        &self,
        table: &WriteTable<'_>,
        index: &str,
        query: &Nearest<'_>,
    ) -> Result<Neighbours> {
        self.folder.check_index_name(index)?;
        let owned = self.owned_searched(table)?;
        owned
            .searched(self.folder)
            .approximate(self.store, &self.inner, index, query)
    }
}
