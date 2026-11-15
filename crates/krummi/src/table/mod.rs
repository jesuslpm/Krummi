//! Tables with schema (7.1): column types and values, the key encoding of document 3 and the row format of
//! document 1 of the table design, schemas and their changes, and the tables of the catalog.

mod bulk;
pub mod catalog;
mod identity;
pub mod key;
#[cfg(test)]
mod malformed_tests;
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
#[cfg(test)]
mod update_tests;
pub mod value;

pub use bulk::BulkLoadError;
pub use catalog::{ReadTable, TableMode, WriteTable};
pub(crate) use projection::Projections;
pub use projection::{ProjectedColumn, ProjectedRow, Projection};
pub use ranges::{ReadRows, WriteRows};
pub(crate) use rows::Readers;
pub use rows::{TableRow, Upserted};
pub use schema::{Column, Schema, SchemaColumn, TableSchema};
pub(crate) use sequence::Sequences;
pub use sequence::{ReadSequence, SequenceMode, WriteSequence};

/// Test hooks, not part of the interface.
#[doc(hidden)]
pub mod testing {
    pub use super::catalog::TableTrees;
    pub use super::identity::testing::with_identity_source;
}
