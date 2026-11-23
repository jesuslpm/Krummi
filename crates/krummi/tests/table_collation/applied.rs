//! The collation applies where Krummi compares texts (IC3, IC6, IC8): the primary key, a unique index, an index scan
//! and an estimate compare the folded forms, whatever the spelling given.

use std::borrow::Cow;

use krummi::table::value::Value;
use krummi::{Collation, Condition, Error, IndexDefinition, IndexKey, IndexMode, Schema};

use super::{create, text};
use crate::support::*;

fn s(text: &str) -> Value<'_> {
    Value::String(Cow::Borrowed(text))
}

#[test]
fn keys_unique_indexes_scans_and_estimates_fold() {
    stores(|store, _| {
        let schema = Schema::new(
            vec![
                text("id").not_null().with_collation(Collation::Ci),
                text("name").with_collation(Collation::CiAi),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &schema);
        let unique = IndexDefinition::new("ix", vec![IndexKey::new("name")]).unique();
        (write.create_index(&table, &unique, IndexMode::CreateNew)).unwrap();
        let row = [("id", s("Ana")), ("name", s("José"))];
        write.insert_row(&table, &row).unwrap();
        let primary = write.insert_row(&table, &[("id", s("ANA"))]).err();
        assert!(matches!(
            primary,
            Some(Error::DuplicateKey { index: None, .. })
        ));
        let index = write.insert_row(&table, &[("id", s("b")), ("name", s("JOSE"))]);
        assert!(matches!(
            index.err(),
            Some(Error::DuplicateKey { index: Some(_), .. })
        ));
        write.commit().unwrap();

        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        let row = read.get_row(&table, &[s("aNa")]).unwrap().unwrap();
        assert_eq!(row.get("id"), Some(s("Ana")));
        assert_eq!(
            read.range_rows(&table, &[], s("ANA")..=s("ana"))
                .unwrap()
                .count(),
            1
        );
        let scan = read.index_rows(&table, "ix", &[s("jose")], ..).unwrap();
        let ids: Vec<_> = scan
            .map(|row| row.unwrap().get("id").unwrap().into_owned())
            .collect();
        assert_eq!(ids, [s("Ana")]);
        let estimate = |name: &str| {
            let condition = Condition::Equal {
                column: "name",
                value: s(name),
            };
            read.estimate(&table, &condition).unwrap().rows
        };
        assert_eq!((estimate("JOSE"), estimate("josé")), (1.0, 1.0));
    });
}
