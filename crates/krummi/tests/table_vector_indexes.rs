//! Approximate vector indexes and the nearest-neighbour searches through the public API (vector indexes PRD 7.6):
//! the exact search and its filters (AC-VI12, AC-VI13), and the indexes, their worker and the searches through them.
//! Each criterion runs on a store on disk and on a temporary one, except the parts that reopen the store.

#[path = "../../krummi-tx/tests/common/mod.rs"]
mod common;
mod support;

#[path = "table_vector_indexes/approximate.rs"]
mod approximate;
#[path = "table_vector_indexes/codex.rs"]
mod codex;
#[path = "table_vector_indexes/crash.rs"]
mod crash;
#[path = "table_vector_indexes/definitions.rs"]
mod definitions;
#[path = "table_vector_indexes/exact.rs"]
mod exact;
#[path = "table_vector_indexes/pending.rs"]
mod pending;
#[path = "table_vector_indexes/rebuild.rs"]
mod rebuild;
#[path = "table_vector_indexes/worker.rs"]
mod worker;

use krummi::table::value::{ColumnType, Value, VectorElement, VectorType};
use krummi::{Column, Schema, WriteTable, WriteTransaction};

/// A vector type of `f32` elements.
pub fn f32_vector(dimension: u16, cosine: bool) -> VectorType {
    VectorType {
        element: VectorElement::F32,
        dimension,
        cosine,
    }
}

/// The value of an `f32` vector.
pub fn vector(values: &[f32]) -> Value<'static> {
    Value::Vector(
        values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>()
            .into(),
    )
}

/// A table of an `i64` key `id`, a nullable `i64` column `n`, a nullable `string` column `tag` and a nullable vector
/// column `v` of type `ty`.
pub fn schema(ty: VectorType) -> Schema {
    Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64),
            Column::new("tag", ColumnType::String),
            Column::new("v", ColumnType::Vector(ty)),
        ],
        &["id"],
    )
}

/// Inserts the row `id` with `n`, `tag` and `v` (each `None` for a null).
pub fn insert(
    write: &mut WriteTransaction<'_>,
    table: &WriteTable<'_>,
    id: i64,
    n: Option<i64>,
    tag: Option<&str>,
    v: Option<Value<'static>>,
) {
    let row = [
        ("id", Value::I64(id)),
        ("n", n.map_or(Value::Null, Value::I64)),
        (
            "tag",
            tag.map_or(Value::Null, |t| Value::String(t.to_owned().into())),
        ),
        ("v", v.unwrap_or(Value::Null)),
    ];
    write.insert_row(table, &row).unwrap();
}

/// The `i64` keys of `found`.
pub fn ids(found: &krummi::Neighbours) -> Vec<i64> {
    (found.rows.iter())
        .map(|n| match n.key[..] {
            [Value::I64(id)] => id,
            _ => panic!("an i64 key"),
        })
        .collect()
}
