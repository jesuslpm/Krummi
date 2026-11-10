//! Tables with schema (7.1): column types and values, the key encoding of document 3 and the row format of
//! document 1 of the table design, schemas and their changes, and the tables of the catalog.

pub mod catalog;
mod identity;
pub mod key;
pub mod row;
pub mod schema;
pub mod value;

pub use catalog::{ReadTable, TableMode, WriteTable};
pub use schema::{Column, Schema, SchemaColumn, TableSchema};

/// Test hooks, not part of the interface.
#[doc(hidden)]
pub mod testing {
    pub use super::catalog::TableTrees;
    pub use super::identity::testing::with_identity_source;
}
