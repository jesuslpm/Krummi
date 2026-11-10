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
    }
}

#[test]
fn a_valid_schema_decodes_as_it_was_encoded() {
    let schema = valid();
    assert_eq!(TableSchema::decode(1, &schema.encode(5)), Some((schema, 5)));
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
        assert_eq!(TableSchema::decode(1, &schema.encode(5)), None, "{label}");
    }
}

/// G6: bytes that are not a schema, whatever they are, decode as `None` and never panic.
#[test]
fn garbage_never_panics() {
    let encoded = valid().encode(5);
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
        if let Some((schema, _)) = TableSchema::decode(1, &bytes) {
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
