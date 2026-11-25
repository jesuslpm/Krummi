//! The exact search (VI16) and the filters of every search (VI7): the order, the ties and the units of the distances
//! (VI8, AC-VI12), the conditions with nulls as in SQL (AC-VI13), the filters through a secondary index and without
//! one, and the errors of *Validaciones*.

use std::ops::Bound;

use krummi::table::value::{ColumnType, Value, VectorElement, VectorType};
use krummi::{
    Collation, Column, ErrorKind, FilterCondition, IndexDefinition, IndexKey, IndexMode, Nearest,
    Schema, SchemaRule, TableMode, ValueRule, VectorMetric,
};

use super::support::stores;
use super::{f32_vector, ids, insert, schema, vector};

/// A row of the filter tests: `(id, n, tag, v)`.
type Row = (i64, Option<i64>, Option<&'static str>, Option<[f32; 2]>);

/// A filter and the rows it passes by the model of SQL.
type Case<'a> = (
    Vec<FilterCondition<'a>>,
    Box<dyn Fn(Option<i64>, Option<&str>) -> bool>,
);

/// The rows of the filter tests, with nulls in `n`, `tag` and `v`.
fn rows() -> Vec<Row> {
    let mut rows = Vec::new();
    for id in 0..40i64 {
        let n = (id % 5 != 0).then_some(id % 7);
        let tag = match id % 3 {
            0 => None,
            1 => Some("Red"),
            _ => Some("blue"),
        };
        let v = (id % 11 != 4).then_some([id as f32 * 0.5, (id % 4) as f32]);
        rows.push((id, n, tag, v));
    }
    rows
}

/// The ids that a filter passes, by the model of SQL, the nearest to `(0, 0)` first, ties by id.
fn expected(rows: &[Row], pass: impl Fn(Option<i64>, Option<&str>) -> bool, k: usize) -> Vec<i64> {
    let mut found: Vec<(f64, i64)> = (rows.iter())
        .filter_map(|&(id, n, tag, v)| {
            let v = v?;
            pass(n, tag).then(|| (f64::from(v[0]).powi(2) + f64::from(v[1]).powi(2), id))
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    found.into_iter().take(k).map(|(_, id)| id).collect()
}

fn fill(store: &krummi::Store, with_indexes: bool) {
    let mut write = store.write().unwrap();
    let ty = f32_vector(2, false);
    let mut schema = schema(ty);
    schema.columns[2] = Column::new("tag", ColumnType::String).with_collation(Collation::Ci);
    let table = write
        .open_table_with("t", TableMode::CreateNew(&schema))
        .unwrap();
    for (id, n, tag, v) in rows() {
        insert(&mut write, &table, id, n, tag, v.map(|v| vector(&v)));
    }
    if with_indexes {
        for (name, column) in [("by_n", "n"), ("by_tag", "tag")] {
            let definition = IndexDefinition::new(name, vec![IndexKey::new(column).descending()]);
            write
                .create_index(&table, &definition, IndexMode::CreateNew)
                .unwrap();
        }
    }
    write.commit().unwrap();
}

fn search(store: &krummi::Store, filter: Vec<FilterCondition<'_>>, k: usize) -> Vec<i64> {
    let read = store.read().unwrap();
    let table = read.open_table("t").unwrap();
    let query = Nearest::new(vector(&[0.0, 0.0]), k).with_filter(filter);
    let found = read
        .nearest_exact(&table, "v", VectorMetric::L2, &query)
        .unwrap();
    assert!(found.exact);
    // The same search in a write transaction finds the same rows.
    let write = store.write().unwrap();
    let table = write.open_table("t").unwrap();
    let in_write = write
        .nearest_exact(&table, "v", VectorMetric::L2, &query)
        .unwrap();
    assert_eq!(in_write, found);
    ids(&found)
}

/// AC-VI13 (the nulls) and VI7: every class of condition, on columns with nulls, with and without a secondary index
/// over the column, passes exactly the rows SQL passes; a row with a null vector never comes.
#[test]
fn ac_vi13_the_filters_pass_the_rows_of_sql() {
    for with_indexes in [false, true] {
        stores(|store, _| {
            fill(store, with_indexes);
            let rows = rows();
            let k = 100;
            let int = Value::I64;
            let text = |t: &str| Value::String(t.to_owned().into());
            let cases: Vec<Case<'_>> = vec![
                (vec![], Box::new(|_, _| true)),
                (
                    vec![FilterCondition::Equal {
                        column: "n",
                        value: int(3),
                    }],
                    Box::new(|n, _| n == Some(3)),
                ),
                // An equality with a null passes nothing (not the null rows, unlike ST2).
                (
                    vec![FilterCondition::Equal {
                        column: "n",
                        value: Value::Null,
                    }],
                    Box::new(|_, _| false),
                ),
                (
                    vec![FilterCondition::Range {
                        column: "n",
                        lower: Bound::Included(int(2)),
                        upper: Bound::Excluded(int(5)),
                    }],
                    Box::new(|n, _| n.is_some_and(|n| (2..5).contains(&n))),
                ),
                (
                    vec![FilterCondition::Range {
                        column: "n",
                        lower: Bound::Unbounded,
                        upper: Bound::Unbounded,
                    }],
                    Box::new(|n, _| n.is_some()),
                ),
                (
                    vec![FilterCondition::Range {
                        column: "n",
                        lower: Bound::Included(Value::Null),
                        upper: Bound::Unbounded,
                    }],
                    Box::new(|_, _| false),
                ),
                (
                    vec![FilterCondition::In {
                        column: "n",
                        values: vec![int(1), int(6), int(1), Value::Null],
                    }],
                    Box::new(|n, _| n == Some(1) || n == Some(6)),
                ),
                (
                    vec![FilterCondition::In {
                        column: "n",
                        values: vec![Value::Null],
                    }],
                    Box::new(|_, _| false),
                ),
                (
                    vec![FilterCondition::NotIn {
                        column: "n",
                        values: vec![int(1), int(2)],
                    }],
                    Box::new(|n, _| n.is_some_and(|n| n != 1 && n != 2)),
                ),
                (
                    vec![FilterCondition::NotIn {
                        column: "n",
                        values: vec![int(1), Value::Null],
                    }],
                    Box::new(|_, _| false),
                ),
                // The `ci` collation of `tag`: "RED" finds "Red".
                (
                    vec![FilterCondition::EqualColumns(vec![
                        ("tag", text("RED")),
                        ("n", int(4)),
                    ])],
                    Box::new(|n, tag| tag == Some("Red") && n == Some(4)),
                ),
                (
                    vec![FilterCondition::EqualColumns(vec![
                        ("tag", text("blue")),
                        ("n", Value::Null),
                    ])],
                    Box::new(|_, _| false),
                ),
                // Several conditions on the same column all hold.
                (
                    vec![
                        FilterCondition::In {
                            column: "n",
                            values: vec![int(1), int(2), int(3)],
                        },
                        FilterCondition::NotIn {
                            column: "n",
                            values: vec![int(2)],
                        },
                        FilterCondition::In {
                            column: "tag",
                            values: vec![text("BLUE"), text("red")],
                        },
                    ],
                    Box::new(|n, tag| matches!(n, Some(1 | 3)) && tag.is_some()),
                ),
            ];
            for (at, (filter, pass)) in cases.into_iter().enumerate() {
                assert_eq!(
                    search(store, filter, k),
                    expected(&rows, &pass, k),
                    "case {at}, indexes {with_indexes}"
                );
            }
            // k bounds the result.
            assert_eq!(search(store, vec![], 3), expected(&rows, |_, _| true, 3));
        });
    }
}

/// A NaN in an IN list finds the rows with a NaN (VI7, N5).
#[test]
fn a_nan_in_a_list_finds_the_nan_rows() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("x", ColumnType::F64),
                Column::new("v", ColumnType::Vector(f32_vector(1, false))),
            ],
            &["id"],
        );
        let table = write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .unwrap();
        for (id, x) in [(1, f64::NAN), (2, 1.0), (3, -f64::NAN)] {
            let row = [
                ("id", Value::I64(id)),
                ("x", Value::F64(x)),
                ("v", vector(&[id as f32])),
            ];
            write.insert_row(&table, &row).unwrap();
        }
        let query = Nearest::new(vector(&[0.0]), 10).with_filter(vec![FilterCondition::In {
            column: "x",
            values: vec![Value::F64(f64::NAN)],
        }]);
        let found = write
            .nearest_exact(&table, "v", VectorMetric::L2, &query)
            .unwrap();
        assert_eq!(ids(&found), [1, 3]);
        write.commit().unwrap();
    });
}

/// AC-VI12 and VI8: L2 gives the Euclidean distance, the dot product its negation, the cosine one minus the cosine,
/// as references computed apart; and two rows whose Euclidean distances are equal once rounded come in the order of
/// their squares, not of their keys.
#[test]
fn ac_vi12_the_distances_and_their_order() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("plain", ColumnType::Vector(f32_vector(2, false))),
                Column::new("unit", ColumnType::Vector(f32_vector(2, true))),
            ],
            &["id"],
        );
        let table = write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .unwrap();
        let points: [(i64, [f32; 2]); 4] = [
            (1, [1.0, 2.0]),
            (2, [-3.0, 0.5]),
            (3, [0.25, -4.0]),
            (4, [2.0, 2.0]),
        ];
        for (id, p) in points {
            let row = [
                ("id", Value::I64(id)),
                ("plain", vector(&p)),
                ("unit", vector(&p)),
            ];
            write.insert_row(&table, &row).unwrap();
        }
        let q = [0.5f32, 1.5];
        let reference = |metric: VectorMetric, p: [f32; 2]| -> f64 {
            let (a, b) = (q.map(f64::from), p.map(f64::from));
            let dot = a[0] * b[0] + a[1] * b[1];
            match metric {
                VectorMetric::L2 => ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt(),
                VectorMetric::Dot => -dot,
                _ => {
                    1.0 - dot
                        / ((a[0] * a[0] + a[1] * a[1]).sqrt() * (b[0] * b[0] + b[1] * b[1]).sqrt())
                }
            }
        };
        for (column, metric) in [
            ("plain", VectorMetric::L2),
            ("plain", VectorMetric::Dot),
            ("unit", VectorMetric::Cosine),
        ] {
            let found = write
                .nearest_exact(&table, column, metric, &Nearest::new(vector(&q), 4))
                .unwrap();
            let mut expected: Vec<(f64, i64)> = points
                .iter()
                .map(|&(id, p)| (reference(metric, p), id))
                .collect();
            expected.sort_by(|a, b| a.0.total_cmp(&b.0));
            assert_eq!(
                ids(&found),
                expected.iter().map(|e| e.1).collect::<Vec<_>>(),
                "{metric}"
            );
            for (row, (distance, _)) in found.rows.iter().zip(&expected) {
                assert!(
                    (row.distance - distance).abs() < 1e-12,
                    "{metric}: {} {distance}",
                    row.distance
                );
            }
        }
        // 2⁵² and 2⁵² + 1 have the same square root in `f64`: 2²⁶. The row of the larger square has the smaller key.
        let big = 67_108_864.0f32; // 2²⁶
        write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(10)),
                    ("plain", vector(&[big, 1.0])),
                    ("unit", vector(&[1.0, 0.0])),
                ],
            )
            .unwrap();
        write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(11)),
                    ("plain", vector(&[big, 0.0])),
                    ("unit", vector(&[1.0, 0.0])),
                ],
            )
            .unwrap();
        let found = write
            .nearest_exact(
                &table,
                "plain",
                VectorMetric::L2,
                &Nearest::new(vector(&[0.0, 0.0]), 6),
            )
            .unwrap();
        let last: Vec<(i64, f64)> = ids(&found)
            .into_iter()
            .zip(found.rows.iter().map(|n| n.distance))
            .skip(4)
            .collect();
        assert_eq!(last, [(11, f64::from(big)), (10, f64::from(big))]);
        write.commit().unwrap();
    });
}

/// Rows at the same distance come in the order of their primary keys; f16, i8 and bit columns search too (VI16).
#[test]
fn ties_by_key_and_every_element() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let f16 = VectorType {
            element: VectorElement::F16,
            dimension: 2,
            cosine: false,
        };
        let i8 = VectorType {
            element: VectorElement::I8,
            dimension: 2,
            cosine: false,
        };
        let bit = VectorType {
            element: VectorElement::Bit,
            dimension: 16,
            cosine: false,
        };
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("h", ColumnType::Vector(f16)),
                Column::new("q", ColumnType::Vector(i8)),
                Column::new("b", ColumnType::Vector(bit)),
            ],
            &["id"],
        );
        let table = write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .unwrap();
        let half =
            |x: u16, y: u16| Value::Vector([x.to_le_bytes(), y.to_le_bytes()].concat().into());
        let int8 =
            |x: i8, y: i8| Value::Vector([x as u8, y as u8, 0, 0, 0x80, 0x3F].to_vec().into());
        let bits = |x: u8, y: u8| Value::Vector(vec![x, y].into());
        // f16 1.0 is 0x3C00, 2.0 is 0x4000; i8 with scale 1.0.
        let data = [
            (5, half(0x3C00, 0), int8(1, 0), bits(0b1, 0)),
            (3, half(0, 0x3C00), int8(0, 1), bits(0b10, 0)),
            (4, half(0x4000, 0), int8(2, 0), bits(0b11, 0)),
        ];
        for (id, h, q, b) in data {
            write
                .insert_row(
                    &table,
                    &[("id", Value::I64(id)), ("h", h), ("q", q), ("b", b)],
                )
                .unwrap();
        }
        let origin_h = half(0, 0);
        let found = write
            .nearest_exact(&table, "h", VectorMetric::L2, &Nearest::new(origin_h, 3))
            .unwrap();
        assert_eq!(ids(&found), [3, 5, 4]);
        assert_eq!(
            found.rows.iter().map(|n| n.distance).collect::<Vec<_>>(),
            [1.0, 1.0, 2.0]
        );
        let found = write
            .nearest_exact(&table, "q", VectorMetric::Dot, &Nearest::new(int8(1, 1), 3))
            .unwrap();
        assert_eq!(ids(&found), [4, 3, 5]);
        assert_eq!(found.rows[0].distance, -2.0);
        let found = write
            .nearest_exact(
                &table,
                "b",
                VectorMetric::Hamming,
                &Nearest::new(bits(0, 0), 3),
            )
            .unwrap();
        assert_eq!(ids(&found), [3, 5, 4]);
        assert_eq!(
            found.rows.iter().map(|n| n.distance).collect::<Vec<_>>(),
            [1.0, 1.0, 2.0]
        );
        write.commit().unwrap();
    });
}

/// An empty table, a filter that passes nothing and the rows a write changed: no error, and the write's own view.
#[test]
fn empty_results_and_the_writes_own_rows() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = write
            .open_table_with("t", TableMode::CreateNew(&schema(f32_vector(2, false))))
            .unwrap();
        let query = Nearest::new(vector(&[0.0, 0.0]), 5);
        assert!(
            write
                .nearest_exact(&table, "v", VectorMetric::L2, &query)
                .unwrap()
                .rows
                .is_empty()
        );
        insert(
            &mut write,
            &table,
            1,
            Some(1),
            None,
            Some(vector(&[1.0, 0.0])),
        );
        insert(&mut write, &table, 2, Some(2), None, None);
        assert_eq!(
            ids(&write
                .nearest_exact(&table, "v", VectorMetric::L2, &query)
                .unwrap()),
            [1]
        );
        let nothing = query.clone().with_filter(vec![FilterCondition::Equal {
            column: "n",
            value: Value::I64(9),
        }]);
        assert!(
            write
                .nearest_exact(&table, "v", VectorMetric::L2, &nothing)
                .unwrap()
                .rows
                .is_empty()
        );
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert_eq!(
            ids(&read
                .nearest_exact(&table, "v", VectorMetric::L2, &query)
                .unwrap()),
            [1]
        );
    });
}

/// The failures of *Validaciones* for the exact search and the filters, each without effect.
#[test]
fn the_errors_of_a_search() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let mut schema = schema(f32_vector(2, false));
        schema
            .columns
            .push(Column::new("unit", ColumnType::Vector(f32_vector(2, true))));
        schema.columns.push(Column::new("doc", ColumnType::Json));
        let table = write
            .open_table_with("t", TableMode::CreateNew(&schema))
            .unwrap();
        insert(
            &mut write,
            &table,
            1,
            Some(1),
            None,
            Some(vector(&[1.0, 0.0])),
        );
        let q = || Nearest::new(vector(&[1.0, 0.0]), 1);
        let kind = |column: &str, metric, query: &Nearest<'_>| {
            write
                .nearest_exact(&table, column, metric, query)
                .unwrap_err()
                .kind()
        };
        assert_eq!(kind("", VectorMetric::L2, &q()), ErrorKind::InvalidSize);
        assert_eq!(
            kind("missing", VectorMetric::L2, &q()),
            ErrorKind::DifferentSchema
        );
        assert_eq!(kind("n", VectorMetric::L2, &q()), ErrorKind::InvalidValue);
        for metric in [VectorMetric::Cosine, VectorMetric::Hamming] {
            assert_eq!(kind("v", metric, &q()), ErrorKind::InvalidValue, "{metric}");
        }
        assert_eq!(
            kind("unit", VectorMetric::L2, &q()),
            ErrorKind::InvalidValue
        );
        let rule = |column: &str, query: &Nearest<'_>| match write.nearest_exact(
            &table,
            column,
            VectorMetric::L2,
            query,
        ) {
            Err(krummi::Error::InvalidValue { rule, .. }) => rule,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            rule("v", &Nearest::new(Value::Null, 1)),
            ValueRule::NullNotAllowed
        );
        assert_eq!(
            rule("v", &Nearest::new(vector(&[1.0]), 1)),
            ValueRule::VectorDimension
        );
        assert_eq!(
            rule("v", &Nearest::new(vector(&[f32::NAN, 0.0]), 1)),
            ValueRule::VectorNotFinite
        );
        assert_eq!(
            rule("v", &Nearest::new(Value::I64(1), 1)),
            ValueRule::WrongType
        );
        match write.nearest_exact(
            &table,
            "unit",
            VectorMetric::Cosine,
            &Nearest::new(vector(&[0.0, 0.0]), 1),
        ) {
            Err(krummi::Error::InvalidValue { rule, .. }) => {
                assert_eq!(rule, ValueRule::ZeroVectorCosine)
            }
            other => panic!("{other:?}"),
        }
        for k in [0, 10_001] {
            assert_eq!(
                rule("v", &Nearest::new(vector(&[1.0, 0.0]), k)),
                ValueRule::InvalidRange
            );
        }
        assert_eq!(rule("v", &q().with_probes(0)), ValueRule::InvalidRange);
        assert!(
            write
                .nearest_exact(
                    &table,
                    "v",
                    VectorMetric::L2,
                    &Nearest::new(vector(&[1.0, 0.0]), 10_000)
                )
                .is_ok()
        );
        let filtered =
            |filter| write.nearest_exact(&table, "v", VectorMetric::L2, &q().with_filter(filter));
        let missing = filtered(vec![FilterCondition::Equal {
            column: "nope",
            value: Value::I64(1),
        }]);
        assert_eq!(missing.unwrap_err().kind(), ErrorKind::DifferentSchema);
        for (column, expected) in [
            ("v", SchemaRule::VectorColumn),
            ("doc", SchemaRule::JsonColumn),
        ] {
            match filtered(vec![FilterCondition::Equal {
                column,
                value: Value::I64(1),
            }]) {
                Err(krummi::Error::InvalidSchema { rule, .. }) => assert_eq!(rule, expected),
                other => panic!("{other:?}"),
            }
        }
        let invalid = |filter| filtered(filter).unwrap_err().kind();
        assert_eq!(
            invalid(vec![FilterCondition::In {
                column: "n",
                values: vec![]
            }]),
            ErrorKind::InvalidValue
        );
        assert_eq!(
            invalid(vec![FilterCondition::NotIn {
                column: "n",
                values: vec![]
            }]),
            ErrorKind::InvalidValue
        );
        assert_eq!(
            invalid(vec![FilterCondition::EqualColumns(vec![])]),
            ErrorKind::InvalidValue
        );
        assert_eq!(
            invalid(vec![FilterCondition::EqualColumns(vec![
                ("n", Value::I64(1)),
                ("n", Value::I64(2))
            ])]),
            ErrorKind::InvalidValue
        );
        assert_eq!(
            invalid(vec![FilterCondition::Equal {
                column: "n",
                value: Value::I32(1)
            }]),
            ErrorKind::InvalidValue
        );
        assert_eq!(
            invalid(vec![FilterCondition::In {
                column: "n",
                values: vec![Value::I64(1), Value::Bool(true)]
            }]),
            ErrorKind::InvalidValue
        );
        // The write is still usable, and a handle of a table it deleted fails with not found.
        write.delete_table("t").unwrap();
        assert_eq!(
            write
                .nearest_exact(&table, "v", VectorMetric::L2, &q())
                .unwrap_err()
                .kind(),
            ErrorKind::TreeNotFound
        );
        write.commit().unwrap();
    });
}
