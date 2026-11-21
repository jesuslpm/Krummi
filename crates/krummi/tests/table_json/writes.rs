//! AC-JS3 at table level (JS3, JS8): each fault of the table of AC-JS3 through insert, update, insert or update and the
//! bulk load, with its table, column, rule and JSON fault, and nothing changed; the wrong type and the null not
//! allowed. AC-JS9 (JS5, E10): the identical update. AC-JS10 (JS4, E18): the longest document.

use std::convert::Infallible;

use krummi::table::value::{ColumnType, Value};
use krummi::{BulkLoadError, JsonCause, SizeSubject, Upserted};

use super::*;

/// The rows of the table of AC-JS3: the text, its cause, its position and its pointer.
fn faults() -> Vec<(Vec<u8>, JsonCause, u64, String)> {
    let syntax = |text: &str, position| {
        (
            text.as_bytes().to_vec(),
            JsonCause::Syntax,
            position,
            String::new(),
        )
    };
    let repeated = |name: &str| JsonCause::RepeatedProperty(name.into());
    vec![
        syntax("", 0),
        syntax("   ", 3),
        syntax("1 2", 2),
        (
            b"\xEF\xBB\xBF1".to_vec(),
            JsonCause::Syntax,
            0,
            String::new(),
        ),
        syntax("NaN", 0),
        syntax("+1", 0),
        syntax(".5", 0),
        syntax("'a'", 0),
        syntax("01", 1),
        syntax("1.", 2),
        syntax("[1,]", 3),
        syntax(r#"{"a":1,}"#, 7),
        syntax(r#"{"a":1}/*c*/"#, 7),
        syntax("[\"a\t\"]", 3),
        syntax(r#""\x""#, 2),
        syntax(r#""\u12""#, 5),
        (
            br#""\ud800""#.to_vec(),
            JsonCause::LoneSurrogate,
            1,
            String::new(),
        ),
        (
            br#""\udc00""#.to_vec(),
            JsonCause::LoneSurrogate,
            1,
            String::new(),
        ),
        (
            br#""\ud800A""#.to_vec(),
            JsonCause::LoneSurrogate,
            1,
            String::new(),
        ),
        (b"\xFF".to_vec(), JsonCause::InvalidUtf8, 0, String::new()),
        (
            b"[\"\xE2\x28\"]".to_vec(),
            JsonCause::InvalidUtf8,
            2,
            String::new(),
        ),
        (
            b"\"\xE2\x82".to_vec(),
            JsonCause::InvalidUtf8,
            1,
            String::new(),
        ),
        (
            format!("{}{}", "[".repeat(129), "]".repeat(129)).into_bytes(),
            JsonCause::TooDeep,
            128,
            "/0".repeat(127),
        ),
        (
            br#"{"a":1,"a":2}"#.to_vec(),
            repeated("a"),
            7,
            String::new(),
        ),
        (
            br#"{"a":1,"b":{"a":2,"a":3}}"#.to_vec(),
            repeated("a"),
            18,
            "/b".to_owned(),
        ),
        (
            br#"{"x":{"a/~":[{"k":1,"k":2}]}}"#.to_vec(),
            repeated("k"),
            20,
            "/x/a~1~0/0".to_owned(),
        ),
        (
            br#"{"\ud800":1,"\ud800":2}"#.to_vec(),
            JsonCause::LoneSurrogate,
            2,
            String::new(),
        ),
    ]
}

/// The ways of giving the bytes of a row of [`faults`]: as bytes and, when they are UTF-8, as text (JS3).
fn given(bytes: &[u8]) -> Vec<Value<'_>> {
    let mut ways = vec![doc_bytes(bytes)];
    if let Ok(text) = std::str::from_utf8(bytes) {
        ways.push(doc(text));
    }
    ways
}

/// The table `t` as `store` has it: its rows, its row count and its modification counter.
fn state(
    write: &WriteTransaction<'_>,
    table: &WriteTable<'_>,
) -> (Vec<(i64, Option<String>)>, u64, u64) {
    let rows = write
        .range_rows(table, &[], ..)
        .unwrap()
        .map(|row| {
            let row = row.unwrap();
            (
                row.i64(0).unwrap(),
                row.json(1).map(|j| j.text().unwrap().to_owned()),
            )
        })
        .collect();
    (
        rows,
        write.row_count(table).unwrap(),
        write.modification_counter(table).unwrap(),
    )
}

/// AC-JS3: each text of the table fails to insert, update and insert or update with its fault, the table, the
/// counter and the transaction as before; the transaction commits its other changes; and a bulk load whose last row
/// brings one aborts whole with that error.
#[test]
fn ac_js3_each_fault_fails_every_write_with_its_data() {
    let faults = faults();
    stores(|store, folder| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &docs());
        write
            .insert_row(
                &table,
                &[("id", Value::I64(1)), ("doc", doc(r#"{"ok":1}"#))],
            )
            .unwrap();
        write.commit().unwrap();
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        write
            .insert_row(&table, &[("id", Value::I64(2)), ("doc", doc("[2]"))])
            .unwrap();
        let before = state(&write, &table);
        for (bytes, cause, position, pointer) in &faults {
            let expected = (
                Some("t".to_owned()),
                Some("doc".to_owned()),
                ValueRule::InvalidJson,
                Some(JsonFault {
                    cause: cause.clone(),
                    position: *position,
                    pointer: pointer.as_str().into(),
                }),
            );
            let what = String::from_utf8_lossy(bytes);
            // A row that exists and one that does not.
            for (id, value) in [1, 3]
                .into_iter()
                .flat_map(|id| given(bytes).into_iter().map(move |v| (id, v)))
            {
                let row = [("id", Value::I64(id)), ("doc", value)];
                let error = write.insert_row(&table, &row).unwrap_err();
                assert_eq!(invalid_value(error, folder), expected, "insert {what}");
                let error = write.update_row(&table, &row).unwrap_err();
                assert_eq!(invalid_value(error, folder), expected, "update {what}");
                let error = write.upsert_row(&table, &row).unwrap_err();
                assert_eq!(invalid_value(error, folder), expected, "upsert {what}");
            }
            assert_eq!(state(&write, &table), before, "{what}");
        }
        write.commit().unwrap();
        let read = store.read().unwrap();
        let table = read.open_table("t").unwrap();
        assert_eq!(
            read.row_count(&table).unwrap(),
            2,
            "the other change committed"
        );
        drop(read);
        assert_eq!(read_text(store, 2).as_deref(), Some("[2]"));
        for (bytes, cause, position, pointer) in &faults {
            let rows = [
                Ok::<_, Infallible>(vec![("id", Value::I64(10)), ("doc", doc("{}"))]),
                Ok(vec![("id", Value::I64(11)), ("doc", doc_bytes(bytes))]),
            ];
            match store.bulk_load("t", rows).unwrap_err() {
                BulkLoadError::Store(error) => assert_eq!(
                    invalid_value(error, folder).3,
                    Some(JsonFault {
                        cause: cause.clone(),
                        position: *position,
                        pointer: pointer.as_str().into(),
                    })
                ),
                BulkLoadError::Source(never) => match never {},
            }
            let read = store.read().unwrap();
            let table = read.open_table("t").unwrap();
            assert_eq!(read.row_count(&table).unwrap(), 2, "the load aborted whole");
        }
    });
}

/// AC-JS3: a `string` or `bytes` value in a JSON column, and a JSON value in another column, fail with the wrong type;
/// a null in a JSON column without nulls with null not allowed, also when its default is missing.
#[test]
fn ac_js3_the_wrong_type_and_the_null_not_allowed() {
    stores(|store, folder| {
        let schema = Schema::new(
            vec![
                Column::new("id", ColumnType::I64).not_null(),
                Column::new("doc", ColumnType::Json).not_null(),
                Column::new("s", ColumnType::String),
            ],
            &["id"],
        );
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &schema);
        let fail = |write: &mut WriteTransaction<'_>, row: &[(&str, Value<'_>)]| {
            let (table_name, column, rule, json) =
                invalid_value(write.insert_row(&table, row).unwrap_err(), folder);
            assert_eq!(table_name.as_deref(), Some("t"));
            assert_eq!(json.is_some(), rule == ValueRule::InvalidJson);
            (column.unwrap(), rule)
        };
        let id = || ("id", Value::I64(1));
        let wrong = |column: &str| (column.to_owned(), ValueRule::WrongType);
        assert_eq!(
            fail(&mut write, &[id(), ("doc", Value::String("{}".into()))]),
            wrong("doc")
        );
        assert_eq!(
            fail(&mut write, &[id(), ("doc", Value::Bytes(b"{}".into()))]),
            wrong("doc")
        );
        assert_eq!(
            fail(&mut write, &[id(), ("doc", doc("{}")), ("s", doc("{}"))]),
            wrong("s")
        );
        let null = ("doc".to_owned(), ValueRule::NullNotAllowed);
        assert_eq!(fail(&mut write, &[id(), ("doc", Value::Null)]), null);
        assert_eq!(fail(&mut write, &[id()]), null);
        // In schema order, the invalid document comes before the wrong type of a later column (E17).
        let invalid = ("doc".to_owned(), ValueRule::InvalidJson);
        assert_eq!(
            fail(&mut write, &[("s", doc("{}")), id(), ("doc", doc("{"))]),
            invalid
        );
        write
            .insert_row(&table, &[id(), ("doc", doc("{}"))])
            .unwrap();
        assert_eq!(write.row_count(&table).unwrap(), 1);
    });
}

/// AC-JS9 (JS5, E10, ST4): an update with the same document written with other whitespace and escapes is identical and
/// adds nothing to the modification counter; with another number text or another member order the row changes.
#[test]
fn ac_js9_an_update_with_the_same_normalized_text_is_identical() {
    stores(|store, _| {
        let mut write = store.write().unwrap();
        let table = create(&mut write, "t", &docs());
        write
            .insert_row(
                &table,
                &[("id", Value::I64(1)), ("doc", doc(r#"{"a":1,"s":"é"}"#))],
            )
            .unwrap();
        write.commit().unwrap();
        let mut write = store.write().unwrap();
        let table = write.open_table("t").unwrap();
        let added = |write: &mut WriteTransaction<'_>, text: &str| {
            let before = write.modification_counter(&table).unwrap();
            let row = [("id", Value::I64(1)), ("doc", doc(text))];
            assert!(write.update_row(&table, &row).unwrap());
            assert_eq!(write.upsert_row(&table, &row).unwrap(), Upserted::Updated);
            write.modification_counter(&table).unwrap() - before
        };
        assert_eq!(added(&mut write, r#"{ "a" : 1 , "s" : "é" }"#), 0);
        assert_eq!(added(&mut write, "{\"a\":1,\"s\":\"\\u00e9\"}\n"), 0);
        assert_eq!(added(&mut write, r#"{"a":1.0,"s":"é"}"#), 1);
        assert_eq!(added(&mut write, r#"{"s":"é","a":1}"#), 1);
        // Read back and written again, the document is the same.
        let row = write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
        let value = row.get("doc").unwrap().into_owned();
        drop(row);
        let before = write.modification_counter(&table).unwrap();
        write
            .update_row(&table, &[("id", Value::I64(1)), ("doc", value)])
            .unwrap();
        assert_eq!(write.modification_counter(&table).unwrap(), before);
        write.commit().unwrap();
        assert_eq!(read_text(store, 1).as_deref(), Some(r#"{"s":"é","a":1}"#));
    });
}

/// The longest value of `n` bytes, `value(n)`, that a row of `table`, an `i64` key and one nullable column, admits:
/// the largest `n` whose row's stored size is within the maximum (E18), found by a binary search.
fn longest(
    write: &WriteTransaction<'_>,
    table: &WriteTable<'_>,
    value: impl Fn(usize) -> Value<'static>,
) -> usize {
    let layout = write.table_schema(table).unwrap().layout();
    let fits = |n: usize| layout.encoded_len(&[Value::I64(0), value(n)]) <= MAX_VALUE;
    let (mut low, mut high) = (0, MAX_VALUE + 1);
    while high - low > 1 {
        let mid = low + (high - low) / 2;
        if fits(mid) { low = mid } else { high = mid }
    }
    low
}

/// A document of `n` bytes, `n` >= 2: a string of `a`s.
fn string_document(n: usize) -> String {
    let mut text = String::with_capacity(n);
    text.push('"');
    text.extend(std::iter::repeat_n('a', n - 2));
    text.push('"');
    text
}

/// AC-JS10 (JS4, E18): the longest normalized text a JSON column admits, N, is stored and read back as it is; N + 1
/// fails with invalid size; N is at most 8 bytes less than the longest text of a `string` column in the same table;
/// and the document of N bytes given indented, its text longer than the row's maximum, is stored.
#[test]
fn ac_js10_the_longest_document() {
    let store = Store::open_temporary(&krummi::StoreOptions::default()).unwrap();
    let mut write = store.write().unwrap();
    let table = create(&mut write, "t", &docs());
    let strings = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("s", ColumnType::String),
        ],
        &["id"],
    );
    let texts = create(&mut write, "s", &strings);
    // The search measures stored sizes only: the values borrow one buffer, which need not be a document.
    let buffer: &'static [u8] = vec![b'a'; MAX_VALUE + 1].leak();
    let n = longest(&write, &table, |n| {
        Value::Json(Json::from_bytes(&buffer[..n]))
    });
    let s = longest(&write, &texts, |n| {
        Value::String(std::str::from_utf8(&buffer[..n]).unwrap().into())
    });
    assert!(n <= s && s - n <= 8, "{n} and {s}");
    {
        let document = string_document(n);
        write
            .insert_row(&table, &[("id", Value::I64(1)), ("doc", doc(&document))])
            .unwrap();
        let row = write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
        assert!(row.json(1).unwrap().text() == Some(document.as_str()));
    }
    {
        let over = string_document(n + 1);
        let error = write
            .insert_row(&table, &[("id", Value::I64(2)), ("doc", doc(&over))])
            .unwrap_err();
        assert_eq!(
            invalid_size(error, None),
            (SizeSubject::Row, MAX_VALUE as u64 + 1, 0, MAX_VALUE as u64)
        );
    }
    {
        // `["aaa…"]` of N bytes, given with whitespace that takes it past the maximum.
        let inner = string_document(n - 2);
        let indented = format!("[\n{}{inner}\n]\n", " ".repeat(16));
        assert!(indented.len() > MAX_VALUE);
        write
            .upsert_row(&table, &[("id", Value::I64(1)), ("doc", doc(&indented))])
            .unwrap();
        let row = write.get_row(&table, &[Value::I64(1)]).unwrap().unwrap();
        let text = row.json(1).unwrap();
        let text = text.text().unwrap();
        assert_eq!(text.len(), n);
        assert!(text.starts_with("[\"a") && text.ends_with("a\"]"));
    }
    assert_eq!(write.row_count(&table).unwrap(), 1);
}
