//! Tables with schema (7.1): column types and values, the key encoding of document 3 and the row format of
//! document 1 of the table design, schemas and their changes, and the tables of the catalog; and their secondary
//! indexes (7.2).

mod bulk;
pub mod catalog;
pub mod collation;
mod collation_tables;
mod distance;
#[cfg(test)]
mod distance_tests;
mod estimate;
mod filter;
mod identity;
mod index;
mod index_key;
#[cfg(test)]
mod index_key_tests;
mod index_scan;
#[cfg(test)]
mod index_tests;
mod ivf;
pub mod json;
pub mod key;
mod maintain;
#[cfg(test)]
mod malformed_tests;
mod nearest;
pub mod projection;
#[cfg(test)]
mod projection_tests;
mod ranges;
pub mod row;
#[cfg(test)]
mod row_tests;
mod rows;
pub mod schema;
mod sequence;
mod sort;
mod statistics;
mod stats;
#[cfg(test)]
mod stats_fault_tests;
#[cfg(test)]
mod update_tests;
pub mod value;
mod vector_index;
mod vector_worker;
mod worker;

pub use bulk::BulkLoadError;
pub use catalog::{ReadTable, TableMode, WriteTable};
pub use collation::Collation;
pub use distance::VectorMetric;
pub use estimate::{Condition, Estimate};
pub(crate) use estimate::{PrivateStatistics, Retired, StatsMemory};
pub use filter::FilterCondition;
pub use index::{
    IndexDefinition, IndexKey, IndexMode, SchemaIndex, SchemaIndexColumn, SchemaIndexKey,
};
pub use index_scan::{IndexEntry, ReadIndexEntries, WriteIndexEntries};
pub use key::{Direction, Nulls};
pub use nearest::{Nearest, Neighbour, Neighbours};
pub(crate) use projection::Projections;
pub use projection::{ProjectedColumn, ProjectedRow, Projection};
pub use ranges::{ReadIndexRows, ReadRows, WriteIndexRows, WriteRows};
pub(crate) use rows::Readers;
pub use rows::{TableRow, Upserted};
pub use schema::{Column, Schema, SchemaColumn, TableSchema};
pub(crate) use sequence::Sequences;
pub use sequence::{ReadSequence, SequenceMode, WriteSequence};
pub use statistics::{ColumnStatistics, HistogramBucket, PrefixStatistics};
#[doc(hidden)]
pub use vector_index::VectorRoots;
pub use vector_index::{
    SchemaVectorIndex, VectorIndexDefinition, VectorIndexInfo, VectorIndexState,
};
pub(crate) use vector_worker::VectorWork;
#[doc(hidden)]
pub use vector_worker::{RebuildThresholds, VectorPoint};
#[doc(hidden)]
pub use worker::StatisticsPoint;
pub(crate) use worker::Work;

/// Test hooks, not part of the interface.
#[doc(hidden)]
pub mod testing {
    pub use super::catalog::TableTrees;
    pub use super::identity::testing::with_identity_source;
    pub use super::index::StoredIndexEntry;
    pub use super::sort::testing::{with_sort_memory, with_sort_pause};
}
