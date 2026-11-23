//! AC-IC5: a property test of `ci` and `ci_ai` columns in the primary key and in ascending and descending indexes, with
//! nulls first and last, against a reference model that compares the folded forms of `fold` (checked by AC-IC2) as
//! bytes, never the key encoding: the order of scans forward and backward, the rows of every kind of range, the rows a
//! range deletes and the duplicate keys; and each row reads back with the text it stored.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::ops::Bound::{self, Excluded, Included, Unbounded};

use krummi::table::collation::fold;
use krummi::table::value::Value;
use krummi::{Collation, Error, IndexDefinition, IndexKey, IndexMode, Schema};

use super::{create, text};
use crate::support::*;

/// Pieces that mix case, precomposed and combining accents, `ß`, `ŉ`, Greek final sigma and lone marks.
const PIECES: [&str; 22] = [
    "a", "A", "á", "a\u{301}", "Á", "ß", "SS", "ss", "ŉ", "ʼn", "σ", "ς", "Σ", "é", "E",
    "e\u{301}", "ñ", "N\u{303}", "n", "\u{301}", "b", "Z",
];

fn random_text(rng: &mut Lcg) -> String {
    let pieces = rng.next() % 4;
    (0..pieces)
        .map(|_| PIECES[(rng.next() % PIECES.len() as u64) as usize])
        .collect()
}

fn maybe_text(rng: &mut Lcg) -> Option<String> {
    (!rng.next().is_multiple_of(5)).then(|| random_text(rng))
}

/// A row of the model: the key, the `ci_ai` column `k` and the `ci` column `d`, as stored.
#[derive(Clone, Debug)]
struct Row {
    id: String,
    k: Option<String>,
    d: Option<String>,
}

fn key(collation: Collation, text: &str) -> Vec<u8> {
    fold(collation, text).as_bytes().to_vec()
}

/// The order of an index column of `collation`: nulls first or last, the folded bytes ascending or descending.
fn index_order(
    a: &Option<String>,
    b: &Option<String>,
    collation: Collation,
    desc: bool,
    last: bool,
) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => {
            if last {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        }
        (Some(_), None) => {
            if last {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }
        (Some(a), Some(b)) => {
            let order = key(collation, a).cmp(&key(collation, b));
            if desc { order.reverse() } else { order }
        }
    }
}

fn within(value: &[u8], lower: &Bound<Vec<u8>>, upper: &Bound<Vec<u8>>) -> bool {
    let above = match lower {
        Included(l) => value >= l.as_slice(),
        Excluded(l) => value > l.as_slice(),
        Unbounded => true,
    };
    let below = match upper {
        Included(u) => value <= u.as_slice(),
        Excluded(u) => value < u.as_slice(),
        Unbounded => true,
    };
    above && below
}

fn random_bound(rng: &mut Lcg) -> Bound<String> {
    match rng.next() % 3 {
        0 => Included(random_text(rng)),
        1 => Excluded(random_text(rng)),
        _ => Unbounded,
    }
}

fn as_value(bound: &Bound<String>) -> Bound<Value<'_>> {
    bound
        .as_ref()
        .map(|text| Value::String(Cow::Borrowed(text.as_str())))
}

fn folded_bound(bound: &Bound<String>, collation: Collation) -> Bound<Vec<u8>> {
    bound.as_ref().map(|text| key(collation, text))
}

fn text_of(value: Option<Value<'_>>) -> Option<String> {
    match value? {
        Value::Null => None,
        Value::String(text) => Some(text.into_owned()),
        other => panic!("not a text: {other:?}"),
    }
}

#[test]
fn ac_ic5_scans_ranges_and_duplicates_follow_the_folded_forms() {
    stores(|store, _| {
        let mut rng = Lcg(0x1c5);
        let schema = Schema::new(
            vec![
                text("id").not_null().with_collation(Collation::Ci),
                text("k").with_collation(Collation::CiAi),
                text("d").with_collation(Collation::Ci),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &schema);
        let up = IndexDefinition::new("up", vec![IndexKey::new("k")]);
        let down = IndexDefinition::new("down", vec![IndexKey::new("d").descending().nulls_last()]);
        for index in [&up, &down] {
            write
                .create_index(&table, index, IndexMode::CreateNew)
                .unwrap();
        }
        let mut model: Vec<Row> = Vec::new();
        for _ in 0..400 {
            let row = Row {
                id: random_text(&mut rng),
                k: maybe_text(&mut rng),
                d: maybe_text(&mut rng),
            };
            let value = |text: &Option<String>| match text {
                Some(text) => Value::String(Cow::Owned(text.clone())),
                None => Value::Null,
            };
            let values = [
                ("id", Value::String(Cow::Owned(row.id.clone()))),
                ("k", value(&row.k)),
                ("d", value(&row.d)),
            ];
            let exists = model
                .iter()
                .any(|r| key(Collation::Ci, &r.id) == key(Collation::Ci, &row.id));
            match write.insert_row(&table, &values) {
                Ok(()) => {
                    assert!(!exists, "{:?} inserted twice", row.id);
                    model.push(row);
                }
                Err(Error::DuplicateKey { index: None, .. }) => assert!(exists, "{:?}", row.id),
                Err(other) => panic!("{other}"),
            }
        }
        model.sort_by_key(|row| key(Collation::Ci, &row.id));

        // The table forward and backward, with the texts as stored.
        let ids = |rows: &[Row]| rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
        let scanned: Vec<String> = (write.range_rows(&table, &[], ..).unwrap())
            .map(|row| text_of(row.unwrap().get("id")).unwrap())
            .collect();
        assert_eq!(scanned, ids(&model));
        let backward: Vec<String> = (write.range_rows(&table, &[], ..).unwrap().rev())
            .map(|row| text_of(row.unwrap().get("id")).unwrap())
            .collect();
        assert_eq!(backward, ids(&model).into_iter().rev().collect::<Vec<_>>());
        for row in &model {
            let read = write.get_row(&table, &[Value::String(Cow::Owned(row.id.to_uppercase()))]);
            let read = read.unwrap().unwrap();
            assert_eq!(text_of(read.get("id")).as_ref(), Some(&row.id));
            assert_eq!(text_of(read.get("k")), row.k);
            assert_eq!(text_of(read.get("d")), row.d);
        }

        // Ranges of the key and of both indexes, with bounds of every kind spelled at random.
        let by_index =
            |rows: &[Row], column: fn(&Row) -> &Option<String>, collation, desc, last| {
                let mut rows = rows.to_vec();
                rows.sort_by(|a, b| {
                    index_order(column(a), column(b), collation, desc, last)
                        .then_with(|| key(Collation::Ci, &a.id).cmp(&key(Collation::Ci, &b.id)))
                });
                rows
            };
        for _ in 0..60 {
            let (lower, upper) = (random_bound(&mut rng), random_bound(&mut rng));
            let (l, u) = (
                folded_bound(&lower, Collation::Ci),
                folded_bound(&upper, Collation::Ci),
            );
            let expected: Vec<String> = (model.iter())
                .filter(|r| within(&key(Collation::Ci, &r.id), &l, &u))
                .map(|r| r.id.clone())
                .collect();
            let range = (as_value(&lower), as_value(&upper));
            let got: Vec<String> = (write.range_rows(&table, &[], range).unwrap())
                .map(|row| text_of(row.unwrap().get("id")).unwrap())
                .collect();
            assert_eq!(got, expected, "{lower:?} {upper:?}");

            for (index, column, collation, desc, last) in [
                (
                    "up",
                    (|r: &Row| &r.k) as fn(&Row) -> &Option<String>,
                    Collation::CiAi,
                    false,
                    false,
                ),
                ("down", |r: &Row| &r.d, Collation::Ci, true, true),
            ] {
                let (l, u) = (
                    folded_bound(&lower, collation),
                    folded_bound(&upper, collation),
                );
                let expected: Vec<String> = by_index(&model, column, collation, desc, last)
                    .into_iter()
                    .filter(|r| {
                        let bounded = !matches!((&lower, &upper), (Unbounded, Unbounded));
                        match column(r) {
                            None => !bounded,
                            Some(text) => within(&key(collation, text), &l, &u),
                        }
                    })
                    .map(|r| r.id)
                    .collect();
                let range = (as_value(&lower), as_value(&upper));
                let got: Vec<String> = (write.index_rows(&table, index, &[], range).unwrap())
                    .map(|row| text_of(row.unwrap().get("id")).unwrap())
                    .collect();
                assert_eq!(got, expected, "{index} {lower:?} {upper:?}");
            }
        }

        // Deleting a range removes exactly the rows of its folded interval.
        let (lower, upper) = (Included("b".to_owned()), Excluded("SS".to_owned()));
        let (l, u) = (
            folded_bound(&lower, Collation::Ci),
            folded_bound(&upper, Collation::Ci),
        );
        let removed = model
            .iter()
            .filter(|r| within(&key(Collation::Ci, &r.id), &l, &u))
            .count();
        let deleted = write
            .delete_range(&table, &[], (as_value(&lower), as_value(&upper)))
            .unwrap();
        assert_eq!(deleted, removed as u64);
        model.retain(|r| !within(&key(Collation::Ci, &r.id), &l, &u));
        let left: Vec<String> = (write.range_rows(&table, &[], ..).unwrap())
            .map(|row| text_of(row.unwrap().get("id")).unwrap())
            .collect();
        assert_eq!(left, ids(&model));
        write.commit().unwrap();
    });
}
