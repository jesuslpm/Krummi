//! Internal tests of the schema encodings: a decoded schema keeps the invariants of E5 or is damage (G6), garbage never
//! panics, and the identities of a change are given as its plan says.

use super::*;

/// `id i64 not null` (key), `name string`, `n i32 not null default 7`.
fn valid() -> TableSchema {
    TableSchema {
        table_id: 1,
        columns: vec![
            SchemaColumn {
                id: 10,
                name: "id".into(),
                ty: ColumnType::I64,
                nullable: false,
                default: None,
            },
            SchemaColumn {
                id: 11,
                name: "name".into(),
                ty: ColumnType::String,
                nullable: true,
                default: None,
            },
            SchemaColumn {
                id: 12,
                name: "n".into(),
                ty: ColumnType::I32,
                nullable: false,
                default: Some(Value::I32(7)),
            },
        ],
        key: vec![10],
        version: 3,
        schema_state: 99,
        indexes: Vec::new(),
        index_state: 98,
    }
}

#[test]
fn a_valid_schema_decodes_as_it_was_encoded() {
    let schema = valid();
    assert_eq!(
        TableSchema::decode(1, &schema.encode(5, &[], TableStats::default())),
        Some((schema.clone(), 5, vec![], TableStats::default()))
    );
    // ST13 of the statistics PRD: the counter, the statistics tree and the last emptying come back.
    let stats = TableStats {
        counter: 1_234,
        root: 77,
        emptied: 1_000,
    };
    assert_eq!(
        TableSchema::decode(1, &schema.encode(5, &[], stats)),
        Some((schema, 5, vec![], stats))
    );
}

/// G6 (E5): a schema that E5 would refuse is never in a catalog Krummi wrote, so decoding one is damage.
#[test]
fn a_decoded_schema_that_breaks_e5_is_damage() {
    type Break = fn(&mut TableSchema);
    let broken: [(&str, Break); 7] = [
        ("duplicate column ids", |s| s.columns[2].id = 11),
        ("duplicate column names", |s| {
            s.columns[2].name = "name".into()
        }),
        ("version 0", |s| s.version = 0),
        ("nullable key column", |s| s.columns[0].nullable = true),
        ("vector key column", |s| {
            s.columns[0].ty = ColumnType::Vector(VectorType {
                element: VectorElement::F32,
                dimension: 1,
                cosine: false,
            })
        }),
        ("key column with a default", |s| {
            s.columns[0].default = Some(Value::I64(1))
        }),
        ("key naming a missing column", |s| s.key = vec![13]),
    ];
    for (label, break_it) in broken {
        let mut schema = valid();
        break_it(&mut schema);
        assert_eq!(
            TableSchema::decode(1, &schema.encode(5, &[], TableStats::default())),
            None,
            "{label}"
        );
    }
}

/// G6: bytes that are not a schema, whatever they are, decode as `None` and never panic.
#[test]
fn garbage_never_panics() {
    let encoded = valid().encode(5, &[], TableStats::default());
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for round in 0..20_000 {
        let mut bytes = encoded.clone();
        match round % 3 {
            // Flip a few bytes of a valid encoding.
            0 => {
                for _ in 0..1 + next() % 4 {
                    let at = (next() as usize) % bytes.len();
                    bytes[at] = next() as u8;
                }
            }
            // Cut it short.
            1 => bytes.truncate((next() as usize) % bytes.len()),
            // Random bytes of any length up to twice its.
            _ => {
                bytes = (0..(next() as usize) % (2 * encoded.len()))
                    .map(|_| next() as u8)
                    .collect()
            }
        }
        if let Some((schema, _, _, _)) = TableSchema::decode(1, &bytes) {
            // What decodes keeps E5's invariants.
            assert!(schema.version >= 1);
            assert!(!schema.key.is_empty());
        }
    }
}

/// `Next::identify` takes a column identity exactly when the change adds a column, so that no column keeps the
/// placeholder identity.
#[test]
#[should_panic(expected = "an added column needs its identity")]
fn an_added_column_needs_its_identity() {
    let column = Column::new("x", ColumnType::Bool);
    let next = valid().plan(&Change::Add(&column)).unwrap().unwrap();
    let _ = next.identify(1, None);
}

#[test]
#[should_panic(expected = "only an added column takes an identity")]
fn only_an_added_column_takes_an_identity() {
    let next = valid().plan(&Change::Drop("name")).unwrap().unwrap();
    let _ = next.identify(1, Some(2));
}

/// [`valid`] with two indexes: `by_name` (unique, `name` descending with its nulls last, including `n`) and `by_n`
/// (`n`, then the primary key column `id`).
fn with_indexes() -> TableSchema {
    let key = |id: u64, name: &str, direction, nulls| SchemaIndexKey {
        id,
        name: name.into(),
        direction,
        nulls,
    };
    let mut schema = valid();
    schema.indexes = vec![
        SchemaIndex {
            id: 20,
            name: "by_n".into(),
            unique: false,
            key: vec![
                key(12, "n", Direction::Ascending, Nulls::First),
                key(10, "id", Direction::Ascending, Nulls::First),
            ],
            included: vec![],
        },
        SchemaIndex {
            id: 21,
            name: "by_name".into(),
            unique: true,
            key: vec![key(11, "name", Direction::Descending, Nulls::Last)],
            included: vec![SchemaIndexColumn {
                id: 12,
                name: "n".into(),
            }],
        },
    ];
    schema
}

/// IX14: the indexes, their identities, the roots of their trees and the index state decode as they were encoded,
/// and the encoding has the length that the size check counts (IX3).
#[test]
fn indexes_decode_as_they_were_encoded() {
    let schema = with_indexes();
    let bytes = schema.encode(5, &[30, 31], TableStats::default());
    assert_eq!(bytes.len(), schema.stored_len());
    assert_eq!(
        TableSchema::decode(1, &bytes),
        Some((schema, 5, vec![30, 31], TableStats::default()))
    );
}

/// G6 (IX1, IX3, IX14): an index that Krummi would never write is damage of the catalog page.
#[test]
fn a_decoded_index_that_breaks_its_rules_is_damage() {
    type Break = fn(&mut TableSchema);
    let broken: [(&str, Break); 9] = [
        ("names out of order", |s| s.indexes.swap(0, 1)),
        ("repeated name", |s| s.indexes[1].name = "by_n".into()),
        ("repeated identity", |s| s.indexes[1].id = 20),
        ("no key columns", |s| s.indexes[0].key.clear()),
        ("missing key column", |s| s.indexes[0].key[0].id = 99),
        ("repeated key column", |s| s.indexes[0].key[1].id = 12),
        ("missing included column", |s| {
            s.indexes[1].included[0].id = 99
        }),
        ("included key column", |s| s.indexes[1].included[0].id = 11),
        ("included primary key column", |s| {
            s.indexes[1].included[0].id = 10
        }),
    ];
    for (label, break_it) in broken {
        let mut schema = with_indexes();
        break_it(&mut schema);
        assert_eq!(
            TableSchema::decode(1, &schema.encode(5, &[30, 31], TableStats::default())),
            None,
            "{label}"
        );
    }
    // The condition of a partial index is reserved: an index with one is not Krummi's.
    let mut bytes = with_indexes().encode(5, &[30, 31], TableStats::default());
    // The condition's length is the last thing before the statistics part.
    let at = bytes.len() - 24 - 4;
    bytes[at..at + 4].copy_from_slice(&1u32.to_le_bytes());
    bytes.insert(at + 4, 0);
    assert_eq!(TableSchema::decode(1, &bytes), None, "a condition");
    // A vector key column.
    let mut schema = with_indexes();
    schema.columns[1].ty = ColumnType::Vector(VectorType {
        element: VectorElement::F32,
        dimension: 1,
        cosine: false,
    });
    assert_eq!(
        TableSchema::decode(1, &schema.encode(5, &[30, 31], TableStats::default())),
        None,
        "a vector key column"
    );
}

/// IX5: renaming a column renames it in its indexes, and dropping a column of an index, not of the primary key, fails
/// naming the first index in the order of their names that has it.
#[test]
fn schema_changes_keep_the_indexes_in_step() {
    let schema = with_indexes();
    let next = schema
        .plan(&Change::Rename {
            from: "n",
            to: "count",
        })
        .unwrap()
        .unwrap()
        .identify(7, None);
    assert_eq!(next.indexes()[0].key[0].name, "count");
    assert_eq!(next.indexes()[1].included[0].name, "count");
    let error = schema.plan(&Change::Drop("n")).err().unwrap();
    assert_eq!(
        error,
        Invalid {
            rule: SchemaRule::DropKeyColumn,
            column: Some("n".into()),
            index: Some("by_n".into()),
        }
    );
    let error = schema.plan(&Change::Drop("id")).err().unwrap();
    assert_eq!((error.rule, error.index), (SchemaRule::DropKeyColumn, None));
}

/// IX6: the walk over a stored schema gives the root of its history tree and every identity that
/// [`TableSchema::identities`] gives, without decoding a default; bytes that are not a schema's layout give `None`.
#[test]
fn the_stored_identities_are_the_schemas() {
    let mut schema = with_indexes();
    schema.columns[1].default = Some(Value::String("x".repeat(1 << 16).into()));
    let bytes = schema.encode(5, &[30, 31], TableStats::default());
    let mut walked = Vec::new();
    assert_eq!(stored_identities(&bytes, |id| walked.push(id)), Some(5));
    let mut expected: Vec<u64> = schema.identities().collect();
    walked.sort_unstable();
    expected.sort_unstable();
    assert_eq!(walked, expected);
    for len in 0..bytes.len() {
        assert_eq!(stored_identities(&bytes[..len], |_| {}), None, "{len}");
    }
    let mut longer = bytes.clone();
    longer.push(0);
    assert_eq!(stored_identities(&longer, |_| {}), None);
}
