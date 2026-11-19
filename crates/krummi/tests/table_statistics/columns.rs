//! Statistics of index prefixes and the equality in several columns (AC-ST13).

use krummi::table::value::{ColumnType, Value};
use krummi::{
    Column, Condition, Error, IndexDefinition, IndexKey, IndexMode, Schema, Store, TableMode,
    ValueRule,
};

use super::support::*;

const ROWS: i64 = 20_000;

/// A table `g` of 20,000 rows: `ciudad` takes 200 values and determines `pais` (20 values); `n` takes 100 values
/// independently of both; an index on (`pais`, `ciudad`, `n`).
fn commit_places(store: &Store) {
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("pais", ColumnType::I64).not_null(),
            Column::new("ciudad", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64).not_null(),
        ],
        &["id"],
    );
    let mut write = store.write().unwrap();
    let table = write
        .open_table_with("g", TableMode::CreateNew(&schema))
        .unwrap();
    for id in 0..ROWS {
        let ciudad = id % 200;
        write
            .insert_row(
                &table,
                &[
                    ("id", Value::I64(id)),
                    ("pais", Value::I64(ciudad % 20)),
                    ("ciudad", Value::I64(ciudad)),
                    ("n", Value::I64((id / 200) % 100)),
                ],
            )
            .unwrap();
    }
    write
        .create_index(
            &table,
            &IndexDefinition::new(
                "ix",
                vec![
                    IndexKey::new("pais"),
                    IndexKey::new("ciudad"),
                    IndexKey::new("n"),
                ],
            ),
            IndexMode::CreateNew,
        )
        .unwrap();
    write.commit().unwrap();
}

fn equal_columns(store: &Store, pairs: &[(&str, i64)]) -> krummi::Estimate {
    let read = store.read().unwrap();
    let table = read.open_table("g").unwrap();
    let pairs = pairs.iter().map(|(c, v)| (*c, Value::I64(*v))).collect();
    read.estimate(&table, &Condition::EqualColumns(pairs))
        .unwrap()
}

fn fraction(store: &Store, column: &str, value: i64) -> f64 {
    let read = store.read().unwrap();
    let table = read.open_table("g").unwrap();
    let condition = Condition::Equal {
        column,
        value: Value::I64(value),
    };
    read.estimate(&table, &condition).unwrap().rows / ROWS as f64
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

/// AC-ST13 (ST17, ST18): an index prefix gives the joint estimate in any order of the columns; without it the
/// columns combine with the backoff biased to overestimate; the exact rules and the errors hold.
#[test]
fn ac_st13_index_prefixes_and_the_biased_backoff() {
    stores(|store, _| {
        commit_places(store);
        store.analyze("g", None).unwrap();
        let joint = equal_columns(store, &[("ciudad", 5), ("pais", 5)]);
        assert!(joint.joint);
        assert!(same(joint.rows, 100.0), "{}", joint.rows);
        let three = equal_columns(store, &[("ciudad", 5), ("pais", 5), ("n", 0)]);
        assert!(three.joint);
        assert!(three.rows <= 2.0, "{}", three.rows);
        // The statistics of the index: two prefixes.
        let read = store.read().unwrap();
        let table = read.open_table("g").unwrap();
        let prefixes = read.index_statistics(&table, "ix").unwrap().unwrap();
        assert_eq!(
            prefixes.iter().map(|p| p.columns).collect::<Vec<_>>(),
            [2, 3]
        );
        assert!(
            same(prefixes[0].distinct, 200.0),
            "{}",
            prefixes[0].distinct
        );
        assert!(
            same(prefixes[1].distinct, 20_000.0),
            "{}",
            prefixes[1].distinct
        );
        assert_eq!(prefixes[0].frequent[0].0.len(), 2);
        assert!(matches!(
            read.index_statistics(&table, "nope").unwrap_err(),
            Error::DifferentIndex { .. }
        ));
        drop(read);

        // Without the index: f1 × f2^(1/4), and with three columns f1 × f2^(1/4) × f3^(1/16).
        let mut write = store.write().unwrap();
        let table = write.open_table("g").unwrap();
        write.drop_index(&table, "ix").unwrap();
        write.commit().unwrap();
        let mut f = [fraction(store, "ciudad", 5), fraction(store, "pais", 5)];
        f.sort_by(f64::total_cmp);
        let two = equal_columns(store, &[("ciudad", 5), ("pais", 5)]);
        assert!(!two.joint);
        assert!(
            same(two.rows, f[0] * f[1].powf(0.25) * ROWS as f64),
            "{}",
            two.rows
        );
        let mut f = [
            fraction(store, "ciudad", 5),
            fraction(store, "pais", 5),
            fraction(store, "n", 0),
        ];
        f.sort_by(f64::total_cmp);
        let three = equal_columns(store, &[("ciudad", 5), ("pais", 5), ("n", 0)]);
        let expected = f[0] * f[1].powf(0.25) * f[2].powf(1.0 / 16.0) * ROWS as f64;
        assert!(same(three.rows, expected), "{} for {expected}", three.rows);
        assert!(
            two.rows >= f[0] * f[1] * ROWS as f64,
            "never below independence"
        );
        // The primary key given estimates at most one row.
        assert!(equal_columns(store, &[("id", 3), ("pais", 3)]).rows <= 1.0);
        // A new index has no statistics: an estimate by its columns samples.
        let mut write = store.write().unwrap();
        let table = write.open_table("g").unwrap();
        write
            .create_index(
                &table,
                &IndexDefinition::new("iy", vec![IndexKey::new("pais"), IndexKey::new("ciudad")]),
                IndexMode::CreateNew,
            )
            .unwrap();
        write.commit().unwrap();
        let samples = store.statistics_samples();
        let again = equal_columns(store, &[("pais", 5), ("ciudad", 5)]);
        assert!(again.joint && same(again.rows, 100.0), "{}", again.rows);
        assert_eq!(store.statistics_samples(), samples + 1);

        // The errors: an empty list, a repeated column.
        let read = store.read().unwrap();
        let table = read.open_table("g").unwrap();
        let rule = |pairs: Vec<(&str, Value<'_>)>| match read
            .estimate(&table, &Condition::EqualColumns(pairs))
            .unwrap_err()
        {
            Error::InvalidValue { rule, .. } => rule,
            other => panic!("{other:?}"),
        };
        assert_eq!(rule(vec![]), ValueRule::InvalidRange);
        assert_eq!(
            rule(vec![("pais", Value::I64(1)), ("pais", Value::I64(2))]),
            ValueRule::RepeatedColumn
        );
    });
}
