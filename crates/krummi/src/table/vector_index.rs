//! Vector indexes of a table (VI1, VI2, VI4, VI10 and VI14 of the vector indexes PRD; document 12 of the table design,
//! *Structures*): their definitions, creating and dropping them in a write, Describe's list of them, and the trees
//! that hold them.
//!
//! A vector index is not a secondary index (N7): it has a column and a metric, no key, no included columns, and is
//! never scanned. Its definition is in the table's catalog entry, after the statistics part, with the roots of its two
//! trees, which no catalog name reaches (N1): the **pending** tree, keyed by the primary key of each row that changed
//! and that the index does not reflect yet (VI3), with its number of keys; and the **data** tree, which holds
//! everything else under a leading tag byte: the state record, and each generation's header, counters, centroids, list
//! sizes, postings (`generation ‖ list ‖ primary key` → SQ8 code) and locator (`generation ‖ primary key` → list). The
//! entry also holds the index's instance, a counter that emptying the table raises, so that the worker drops a batch
//! prepared for an index that is no longer the same (VI3).
//!
//! Creating a vector index is a short write that saves its definition and an index in training (VI4); the store's
//! worker trains and builds it afterwards.
//!
//! ```
//! use krummi::table::value::{ColumnType, VectorElement, VectorType};
//! use krummi::{Column, IndexMode, Schema, Store, StoreOptions, TableMode, VectorIndexDefinition, VectorMetric};
//!
//! let store = Store::open_temporary(&StoreOptions::default())?;
//! let mut write = store.write()?;
//! let ty = VectorType { element: VectorElement::F32, dimension: 4, cosine: false };
//! let schema = Schema::new(
//!     vec![Column::new("id", ColumnType::I64).not_null(), Column::new("embedding", ColumnType::Vector(ty))],
//!     &["id"],
//! );
//! let table = write.open_table_with("docs", TableMode::CreateNew(&schema))?;
//! let definition = VectorIndexDefinition::new("by_embedding", "embedding", VectorMetric::L2);
//! write.create_vector_index(&table, &definition, IndexMode::CreateNew)?;
//! let described = write.describe_table("docs")?;
//! assert_eq!(described.vector_indexes()[0].column.name, "embedding");
//! write.drop_index(&table, "by_embedding")?;
//! write.commit()?;
//! # Ok::<(), krummi::Error>(())
//! ```

use krummi_pager::{Error, Result, SchemaDifference, SchemaRule};
use krummi_tree::catalog::MAX_EXTRA;

use super::catalog::WriteTable;
use super::distance::VectorMetric;
use super::index::{IndexMode, MAX_INDEXES, SchemaIndexColumn};
use super::schema::{Different, Invalid, TableSchema};
use super::value::{ColumnType, VectorElement};
use crate::WriteTransaction;

/// The definition of a vector index, as the application gives it to [`WriteTransaction::create_vector_index`] (VI1):
/// a name of 1 to 1,024 bytes, in the same name space as the table's secondary indexes; a vector column of `f32`,
/// `f16` or `i8` elements; and a metric the column admits: L2 or the dot product, or the cosine on a column that
/// declares it (VI2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorIndexDefinition {
    pub name: String,
    pub column: String,
    pub metric: VectorMetric,
}

impl VectorIndexDefinition {
    /// A vector index `name` over `column` with `metric`.
    pub fn new(
        name: impl Into<String>,
        column: impl Into<String>,
        metric: VectorMetric,
    ) -> VectorIndexDefinition {
        VectorIndexDefinition {
            name: name.into(),
            column: column.into(),
            metric,
        }
    }
}

/// A vector index of a table, as Describe gives it (VI1): its identity, a random 64-bit value drawn when it is created
/// (P12), its name, its column (identity and name, as the schema the transaction sees names it) and its metric.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaVectorIndex {
    pub id: u64,
    pub name: String,
    pub column: SchemaIndexColumn,
    pub metric: VectorMetric,
}

impl SchemaVectorIndex {
    /// The bytes that [`TableSchema::encode`] writes for the index: identity, name, column, metric and roots.
    pub(crate) fn stored_len(&self) -> usize {
        8 + 4 + self.name.len() + 8 + 1 + 4 * 8
    }
}

/// What a table's catalog entry keeps of a vector index besides its definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VectorRoots {
    /// Raised each time the table is emptied: a batch of another instance is dropped (VI3).
    pub instance: u64,
    /// The root of its pending tree and its number of keys (VI3, VI11).
    pub pending: u64,
    pub pending_count: u64,
    /// The root of its data tree.
    pub data: u64,
}

/// The tags that lead the keys of a data tree.
#[allow(dead_code, reason = "used by the worker and the searches (T5–T8)")]
pub(crate) mod tag {
    /// The state record: one key, the tag alone.
    pub(crate) const STATE: u8 = 0;
    /// `generation` → its header: lists, rows at training, quantizer.
    pub(crate) const HEADER: u8 = 1;
    /// `generation` → its counters: entries and codes with a saturated component.
    pub(crate) const COUNTERS: u8 = 2;
    /// `generation ‖ list` → the list's centroid.
    pub(crate) const CENTROID: u8 = 3;
    /// `generation ‖ list` → the list's number of entries.
    pub(crate) const LIST_SIZE: u8 = 4;
    /// `generation ‖ list ‖ primary key` → the row's SQ8 code.
    pub(crate) const POSTING: u8 = 5;
    /// `generation ‖ primary key` → the row's list.
    pub(crate) const LOCATOR: u8 = 6;
    /// `generation` → nothing: a generation a publication retired, whose keys later batches delete.
    pub(crate) const RETIRED: u8 = 7;
}

/// The state record of a vector index (VI4, VI9): its active generation (0 while none is ready), the generation it
/// builds (0 while none), whether a rebuild is in progress, the cursor of the build (the last primary key it reached,
/// none before the first) and the next generation to number.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct IndexState {
    pub(crate) active: u32,
    pub(crate) building: u32,
    pub(crate) rebuilding: bool,
    pub(crate) cursor: Option<Vec<u8>>,
    pub(crate) next: u32,
}

/// The state of a vector index as its information reports it (VI4, VI11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VectorIndexState {
    /// No generation trained yet: since it was created or its table emptied, or with fewer than 10,000 rows with a
    /// vector.
    Training,
    /// Its first generation trained and being built.
    Building,
    /// A generation serves the searches.
    Ready,
    /// A generation serves the searches while a new one is trained and built.
    ReadyRebuilding,
}

impl IndexState {
    /// The state of an index just created or emptied: training, its first generation numbered 1.
    pub(crate) fn new() -> IndexState {
        IndexState {
            next: 1,
            ..IndexState::default()
        }
    }

    #[allow(dead_code, reason = "used by the worker and the searches (T5–T8)")]
    pub(crate) fn public(&self) -> VectorIndexState {
        match (self.active, self.building, self.rebuilding) {
            (0, 0, _) => VectorIndexState::Training,
            (0, _, _) => VectorIndexState::Building,
            (_, 0, false) => VectorIndexState::Ready,
            _ => VectorIndexState::ReadyRebuilding,
        }
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16);
        out.extend(self.active.to_be_bytes());
        out.extend(self.building.to_be_bytes());
        out.extend(self.next.to_be_bytes());
        out.push(u8::from(self.rebuilding));
        if let Some(cursor) = &self.cursor {
            out.extend(cursor);
        }
        out
    }

    #[allow(dead_code, reason = "used by the worker and the searches (T5–T8)")]
    pub(crate) fn decode(bytes: &[u8]) -> Option<IndexState> {
        if bytes.len() < 13 || bytes[12] > 1 {
            return None;
        }
        let u32_at = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
        Some(IndexState {
            active: u32_at(0),
            building: u32_at(4),
            next: u32_at(8),
            rebuilding: bytes[12] == 1,
            cursor: (bytes.len() > 13).then(|| bytes[13..].to_vec()),
        })
    }
}

/// The vector index that `definition` adds to `schema`, with a placeholder identity, or the first rule it breaks (VI1,
/// VI2, *Validaciones*): its column does not exist; it is not a vector, or its elements are `bit`; the metric does not
/// fit the column; another vector index has its column and metric; the table would pass 64 indexes; the schema would
/// pass its size. `adding` is false for Create if not exists over the index of its name with the same definition.
pub(crate) fn check_vector_definition(
    schema: &TableSchema,
    definition: &VectorIndexDefinition,
    adding: bool,
) -> std::result::Result<SchemaVectorIndex, Invalid> {
    let fail = |rule, column: Option<&str>| {
        Err(Invalid {
            rule,
            column: column.map(str::to_owned),
            index: Some(definition.name.clone()),
        })
    };
    let Some(column) = schema.column(&definition.column) else {
        return fail(SchemaRule::ColumnNotFound, Some(&definition.column));
    };
    let ColumnType::Vector(vector) = column.ty else {
        return fail(SchemaRule::VectorIndexColumn, Some(&column.name));
    };
    if vector.element == VectorElement::Bit {
        return fail(SchemaRule::VectorIndexColumn, Some(&column.name));
    }
    if definition.metric == VectorMetric::Hamming || !definition.metric.fits(&vector) {
        return fail(SchemaRule::VectorIndexMetric, Some(&column.name));
    }
    let same = (schema.vector_indexes().iter()).any(|other| {
        other.name != definition.name
            && other.column.id == column.id
            && other.metric == definition.metric
    });
    if same {
        return fail(SchemaRule::SameVectorIndex, Some(&column.name));
    }
    if adding && schema.index_count() >= MAX_INDEXES {
        return fail(SchemaRule::TooManyIndexes, None);
    }
    let index = SchemaVectorIndex {
        id: 0,
        name: definition.name.clone(),
        column: SchemaIndexColumn {
            id: column.id,
            name: column.name.clone(),
        },
        metric: definition.metric,
    };
    if adding && schema.stored_len() + index.stored_len() > MAX_EXTRA {
        return fail(SchemaRule::SchemaTooLarge, None);
    }
    Ok(index)
}

/// Whether `definition` is the definition of `index`, the table's vector index of its name, or its first difference:
/// the column, then the metric (IX2).
fn compare_vector_definition(
    index: &SchemaVectorIndex,
    definition: &VectorIndexDefinition,
) -> std::result::Result<(), Different> {
    let different = |difference, expected: String, found: String| {
        Err(Different {
            column: None,
            difference,
            expected,
            found,
            index: Some(index.name.clone()),
        })
    };
    if definition.column != index.column.name {
        return different(
            SchemaDifference::VectorIndexColumn,
            format!("{:?}", definition.column),
            format!("{:?}", index.column.name),
        );
    }
    if definition.metric != index.metric {
        return different(
            SchemaDifference::VectorIndexMetric,
            definition.metric.to_string(),
            index.metric.to_string(),
        );
    }
    Ok(())
}

/// The difference of an index of the other kind with the name an index definition gives (IX2, VI1).
pub(crate) fn other_kind(name: &str, given_vector: bool) -> Different {
    let kind = |vector: bool| {
        if vector {
            "a vector index"
        } else {
            "a secondary index"
        }
        .to_owned()
    };
    Different {
        column: None,
        difference: SchemaDifference::IndexKind,
        expected: kind(given_vector),
        found: kind(!given_vector),
        index: Some(name.to_owned()),
    }
}

impl WriteTransaction<'_> {
    /// Creates the vector index `definition` over `table` in `mode` (VI1, VI4): a short write that saves its definition
    /// and an index in training, with a new identity and a new index state (IX6), without reading the rows; the store's
    /// worker trains and builds it after the write commits, while other writes go on. Create if not exists leaves an
    /// index of its name with the same definition as it is. Fails, in this order, with
    /// [`ErrorKind::InvalidSize`](crate::ErrorKind::InvalidSize) for a name or a column name without 1 to 1,024 bytes;
    /// with [`ErrorKind::Misuse`](crate::ErrorKind::Misuse) for a handle of another transaction and
    /// [`ErrorKind::TreeNotFound`](crate::ErrorKind::TreeNotFound) for a table this write deleted; with
    /// [`ErrorKind::TreeAlreadyExists`](crate::ErrorKind::TreeAlreadyExists) when the table has an index, secondary or
    /// vector, of the name and the mode is Create new, and with
    /// [`ErrorKind::DifferentSchema`](crate::ErrorKind::DifferentSchema) when the mode is Create if not exists and that
    /// index is of the other kind or has another column or metric; and with
    /// [`ErrorKind::InvalidSchema`](crate::ErrorKind::InvalidSchema) for the rules of the definition: a column that
    /// does not exist, one that is not a vector or whose elements are `bit`, a metric it does not admit, the column and
    /// metric of another vector index, more than 64 indexes, or a schema too large. A failure has no effect.
    pub fn create_vector_index(
        &mut self,
        table: &WriteTable<'_>,
        definition: &VectorIndexDefinition,
        mode: IndexMode,
    ) -> Result<()> {
        self.folder.check_index_name(&definition.name)?;
        self.folder.check_column_name(&definition.column)?;
        let (index, history) = {
            let state = self.live_table(table)?;
            let invalid = |invalid| self.folder.invalid_schema(&state.name, invalid);
            let different = |different| self.folder.different_schema(&state.name, different);
            let existing = state.schema.vector_index(&definition.name);
            let secondary = state.schema.index(&definition.name).is_some();
            match (mode, existing, secondary) {
                (IndexMode::CreateNew, Some(_), _) | (IndexMode::CreateNew, _, true) => {
                    return Err(self
                        .folder
                        .index_exists_error(&state.name, &definition.name));
                }
                (IndexMode::CreateIfNotExists, _, true) => {
                    return Err(different(other_kind(&definition.name, true)));
                }
                (IndexMode::CreateIfNotExists, Some(existing), _) => {
                    compare_vector_definition(existing, definition).map_err(different)?;
                    check_vector_definition(&state.schema, definition, false).map_err(invalid)?;
                    return Ok(());
                }
                _ => {}
            }
            let index =
                check_vector_definition(&state.schema, definition, true).map_err(invalid)?;
            (index, state.history)
        };
        let known =
            move |tx: &Self, id: u64| Ok(tx.known_identity(history, id)? || tx.store_knows(id)?);
        let id = self.draw_identity(known)?;
        let index_state = self.draw_identity(|tx, state| Ok(state == id || known(tx, state)?))?;
        let roots = self.new_vector_trees(0)?;
        let history = self.record_identities(history, [id, index_state])?;
        let state = self
            .tables
            .get_mut()
            .get_mut(table.id)
            .expect("a live handle");
        let at = state
            .schema
            .add_vector_index(SchemaVectorIndex { id, ..index }, index_state);
        state.vector_roots.insert(at, roots);
        state.maintained = None;
        state.history = history;
        state.vector_changed = true;
        self.write_table_entry(table.id)
    }

    /// The empty trees of a vector index of instance `instance`: its pending tree, and its data tree with the state of
    /// an index in training.
    pub(crate) fn new_vector_trees(&mut self, instance: u64) -> Result<VectorRoots> {
        let pending = krummi_tree::create(&mut self.inner)?;
        let data = krummi_tree::create(&mut self.inner)?;
        let data = krummi_tree::set(
            &mut self.inner,
            data,
            &[tag::STATE],
            &IndexState::new().encode(),
        )?
        .root;
        Ok(VectorRoots {
            instance,
            pending,
            pending_count: 0,
            data,
        })
    }

    /// Frees every page of the trees of a vector index (VI10).
    pub(crate) fn free_vector_trees(&mut self, roots: &VectorRoots) -> Result<()> {
        krummi_tree::free_all(&mut self.inner, roots.pending)?;
        krummi_tree::free_all(&mut self.inner, roots.data)
    }

    /// Drops the vector index at `at` of `table`, freeing its trees, with a new index state (VI1, IX4).
    pub(crate) fn drop_vector_index(&mut self, table: &WriteTable<'_>, at: usize) -> Result<()> {
        let (roots, history) = {
            let state = self.live_table(table)?;
            (state.vector_roots[at], state.history)
        };
        let index_state = self.draw_identity(|tx, state| {
            Ok(tx.known_identity(history, state)? || tx.store_knows(state)?)
        })?;
        self.free_vector_trees(&roots)?;
        let history = self.record_identities(history, [index_state])?;
        let state = self
            .tables
            .get_mut()
            .get_mut(table.id)
            .expect("a live handle");
        state.schema.remove_vector_index(at, index_state);
        state.vector_roots.remove(at);
        state.maintained = None;
        state.history = history;
        state.vector_changed = true;
        self.write_table_entry(table.id)
    }
}

impl crate::Folder<'_> {
    /// The error of an index name the table already has (IX2).
    pub(crate) fn index_exists_error(self, table: &str, index: &str) -> Error {
        Error::TreeAlreadyExists {
            folder: self.owned(),
            name: table.to_owned(),
            index: Some(index.into()),
        }
    }
}
