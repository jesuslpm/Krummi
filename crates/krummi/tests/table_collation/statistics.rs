//! The statistics and estimates of `ci` and `ci_ai` columns, over folded forms (AC-IC8).

use std::borrow::Cow;
use std::ops::Bound::{Excluded, Included};

use krummi::table::collation::fold;
use krummi::table::value::{ColumnType, Value};
use krummi::{Collation, Column, Condition, IndexDefinition, IndexKey, IndexMode, Schema};

use super::{create, text};
use crate::support::*;

fn s(text: &str) -> Value<'_> {
    Value::String(Cow::Borrowed(text))
}

fn owned(value: &Value<'_>) -> String {
    match value {
        Value::String(text) => text.to_string(),
        other => panic!("not a text: {other:?}"),
    }
}

/// 300 rows, fewer than the 30,000 that statistics read whole (ST1), with texts that differ only in case or
/// accents, and one `ci` text of 2,000 bytes whose folded form has 3,000, which is large (IC8).
const ROWS: i64 = 300;
const CI: [&str; 5] = ["Ana", "ANA", "ana", "Bea", "BEA"];
const CI_AI: [&str; 5] = ["José", "JOSE", "jose", "Ñu", "ñu"];

#[test]
fn ac_ic8_statistics_and_estimates_count_folded_forms() {
    stores(|store, _| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                text("c").with_collation(Collation::Ci),
                text("a").with_collation(Collation::CiAi),
                Column::new("n", ColumnType::I64),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &schema);
        let index = IndexDefinition::new("ix", vec![IndexKey::new("a"), IndexKey::new("n")]);
        (write.create_index(&table, &index, IndexMode::CreateNew)).unwrap();
        for id in 0..ROWS {
            let at = usize::try_from(id).unwrap() % 5;
            let row = [
                ("id", Value::I64(id)),
                ("c", s(CI[at])),
                ("a", s(CI_AI[at])),
                ("n", Value::I64(id % 2)),
            ];
            write.insert_row(&table, &row).unwrap();
        }
        let large = "ŉ".repeat(1_000);
        let row = [
            ("id", Value::I64(ROWS)),
            ("c", s(&large)),
            ("n", Value::I64(0)),
        ];
        write.insert_row(&table, &row).unwrap();
        write.analyze(&table, None).unwrap();

        let rows = (ROWS + 1) as f64;
        let c = write.column_statistics(&table, "c").unwrap().unwrap();
        assert!((c.distinct - 2.0).abs() < 0.5, "{c:?}");
        assert!((c.large_fraction - 1.0 / rows).abs() < 1e-9, "{c:?}");
        let mut seen: Vec<String> = c.frequent.iter().map(|(v, _)| owned(v)).collect();
        seen.extend(c.histogram.iter().map(|bucket| owned(&bucket.upper)));
        seen.sort();
        seen.dedup();
        assert_eq!(seen, ["ana", "bea"]);
        let a = write.column_statistics(&table, "a").unwrap().unwrap();
        let mut seen: Vec<String> = a.frequent.iter().map(|(v, _)| owned(v)).collect();
        seen.extend(a.histogram.iter().map(|bucket| owned(&bucket.upper)));
        seen.sort();
        seen.dedup();
        assert_eq!(seen, ["jose", "ñu"]);
        for value in seen {
            assert_eq!(fold(Collation::CiAi, &value), value.as_str());
        }
        let prefixes = write.index_statistics(&table, "ix").unwrap().unwrap();
        let pair = prefixes.iter().find(|p| p.columns == 2).unwrap();
        assert!((pair.distinct - 5.0).abs() < 0.5, "{pair:?}");
        // Two folded texts by two parities, and the large row's null text (ST17).
        for (combination, _) in &pair.frequent {
            match &combination[0] {
                Value::Null => {}
                value => assert!(["jose", "ñu"].contains(&owned(value).as_str()), "{value:?}"),
            }
        }

        // Every spelling estimates the same.
        let estimate = |condition: Condition<'_>| write.estimate(&table, &condition).unwrap().rows;
        let equal = |column, value| Condition::Equal { column, value };
        let expected = estimate(equal("c", s("ana")));
        assert!(expected > 0.0);
        for spelling in ["ANA", "Ana", "aNa"] {
            assert_eq!(estimate(equal("c", s(spelling))), expected, "{spelling}");
        }
        assert_eq!(
            estimate(equal("a", s("JOSÉ"))),
            estimate(equal("a", s("jose")))
        );
        assert_eq!(estimate(equal("a", s("ÑU"))), estimate(equal("a", s("ñu"))));
        assert_ne!(
            estimate(equal("a", s("nu"))),
            estimate(equal("a", s("ñu"))),
            "ñ is a letter of its own"
        );
        let range = |lower, upper| Condition::Range {
            column: "c",
            lower: Included(s(lower)),
            upper: Excluded(s(upper)),
        };
        assert_eq!(estimate(range("A", "B")), estimate(range("a", "b")));
        let pair = |value| Condition::EqualColumns(vec![("a", s(value)), ("n", Value::I64(0))]);
        assert_eq!(estimate(pair("JOSÉ")), estimate(pair("jose")));
        assert!(
            (estimate(equal("c", s(&large))) - 1.0).abs() < 1e-9,
            "a large value estimates the large fraction"
        );
        write.abort();
    });
}
