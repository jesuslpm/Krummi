# JSON columns

This guide covers JSON columns: declaring one, giving a document, what Krummi validates and normalizes when it writes
it and the errors it gives, reading a document back without copying it, navigating it with JSON pointers, reading its
values as Krummi types, how documents compare, and what JSON columns cannot do. The types are in
`krummi::table::json` (`Json`, `JsonValue`, `JsonKind`, `Members`, `Elements`); the errors' data, `JsonFault` and
`JsonCause`, are at the crate root.

## Steps

1. Declare the column with `ColumnType::Json`.
2. Give each document as `Value::Json(Json::new(text))`, or `Json::from_bytes(bytes)` for bytes that may not be
   UTF-8. Krummi validates and normalizes it when the row is written.
3. Read it back with `row.json(position)`: a `Json` holding the normalized text, borrowed from the page.
4. Navigate it with `json.document()`, which gives the root `JsonValue`, and its `get`, `element` and `pointer`.
5. Read a value as a Krummi type with `as_text`, `as_bool`, `as_i64`, `as_u64`, `as_decimal`, `as_f64`, `as_date` or
   `as_timestamp`.

```rust
use krummi::table::json::{Json, JsonKind};
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;

    // 1. Declare the column.
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("doc", ColumnType::Json)],
        &["id"],
    );
    let orders = write.open_table_with("orders", TableMode::CreateNew(&schema))?;

    // 2. Give the document as text; the write validates and normalizes it.
    let text = r#" { "customer" : "Ana" , "lines" : [ { "sku" : "A1" , "qty" : 2 } ] , "paid" : true } "#;
    write.insert_row(&orders, &[("id", Value::I64(1)), ("doc", Value::Json(Json::new(text)))])?;
    write.commit()?;

    // 3. Read it back: the normalized text, borrowed from the page.
    let read = store.read()?;
    let orders = read.open_table("orders")?;
    let row = read.get_row(&orders, &[Value::I64(1)])?.expect("inserted");
    let json = row.json(1).expect("not null");
    assert_eq!(
        json.text(),
        Some(r#"{"customer":"Ana","lines":[{"sku":"A1","qty":2}],"paid":true}"#)
    );

    // 4. Navigate it.
    let document = json.document().expect("read back");
    assert_eq!(document.kind(), JsonKind::Object);
    let qty = document.pointer("/lines/0/qty")?.expect("present");

    // 5. Read values as Krummi types.
    assert_eq!(qty.as_i64(), Some(2));
    assert_eq!(document.get("customer").and_then(|v| v.as_text()).as_deref(), Some("Ana"));
    assert_eq!(document.get("paid").and_then(|v| v.as_bool()), Some(true));
    Ok(())
}
```

## Declaring a JSON column

`ColumnType::Json` has no parameters, and describes itself as `json`. A JSON column is like any other column: it
allows nulls unless it is `.not_null()`, it may have a default value (see
[Nulls and default values](#nulls-and-default-values)), and it can be added, dropped, renamed and allowed nulls
([tables](tables.md)). No column can change its type to or from `json`.

## Giving a document

A document is a `Json`, made with one of two constructors:

- `Json::new(text)` takes a `&str` or a `String` (anything that is `Into<Cow<str>>`), borrowed or owned.
- `Json::from_bytes(bytes)` takes a `&[u8]` or a `Vec<u8>`, which may not be valid UTF-8; a write rejects it then.

Making a `Json` checks nothing. The document is validated when a row is written (insert, update, insert or update,
bulk load), so the error names the table and the column. Krummi has no builder of objects and arrays: build the text
with your JSON library and give it to `Json::new`. To store a document you read, give its text.

`json.as_bytes()` returns the bytes as given. `json.text()` and `json.document()` return `None` for a document given by
the application: only a document read back is known to be normalized.

A `Value::String` or `Value::Bytes` in a JSON column, or a `Value::Json` in a column of another type, fails with
`ValueRule::WrongType`.

```rust
use krummi::table::json::Json;
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, Schema, Store, StoreOptions, TableMode, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("doc", ColumnType::Json)],
        &["id"],
    );
    let t = write.open_table_with("docs", TableMode::CreateNew(&schema))?;

    let borrowed = Json::new(r#"{"a":1}"#);
    let owned = Json::new(String::from("[1, 2, 3]"));
    let bytes = Json::from_bytes(&b"\"bytes\""[..]);
    assert_eq!(owned.as_bytes(), b"[1, 2, 3]");
    assert_eq!(owned.text(), None); // not normalized until it is written

    for (id, json) in [(1, borrowed), (2, owned), (3, bytes)] {
        write.insert_row(&t, &[("id", Value::I64(id)), ("doc", Value::Json(json))])?;
    }

    // A text is not a document.
    let error = write
        .insert_row(&t, &[("id", Value::I64(4)), ("doc", Value::String(r#"{"a":1}"#.into()))])
        .unwrap_err();
    assert!(matches!(error, Error::InvalidValue { rule: ValueRule::WrongType, .. }));
    Ok(())
}
```

## What a valid document is

A valid document is a UTF-8 text that is exactly one RFC 8259 value, with optional whitespace around and between its
parts. A bare value at the root is a document too: `42`, `"x"`, `true`, `null`.

| Valid | Not valid |
|---|---|
| any RFC 8259 value at the root | an empty text, or only whitespace |
| whitespace: space, tab, line feed, carriage return | data after the value; a BOM; a comment; any other whitespace |
| numbers of any length or exponent (`1e400`, `123456789012345678901234567890`) | `NaN`, `Infinity`, `+1`, `01`, `1.`, `.5`, `0x10` |
| escaped U+0000 (`\u0000`) and Unicode noncharacters | an unescaped control character U+0000 to U+001F in a string |
| an escaped surrogate pair (`😀`) | a lone surrogate (`"\ud800"`, `"\udc00"`) |
| | an escape RFC 8259 does not define (`"\x"`, `"\u12"`) |
| | single quotes; a trailing comma (`[1,]`, `{"a":1,}`) |
| properties whose names differ after decoding | two properties of one object with the same decoded name (`{"a":1,"a":2}`) |
| up to 128 nested objects and arrays | 129 or more |

Names are compared by their characters after decoding their escapes, with no Unicode normalization: `"é"` as one
character and `e` followed by a combining accent are different names.

## The normalized text

Krummi stores and returns every document as its normalized text, which keeps everything the document means:

1. No whitespace outside strings.
2. `true`, `false` and `null` as they are.
3. Each number with the characters it was written with: `1.50e+3` stays `1.50e+3`, `-0` stays `-0`.
4. Each string and property name with its escapes decoded and written again: every character as it is in UTF-8,
   except `"` as `\"`, `\` as `\\`, the control characters U+0008, U+000C, U+000A, U+000D and U+0009 as `\b`, `\f`,
   `\n`, `\r` and `\t`, and the other control characters as `\u00` and two lowercase hexadecimal digits (`\u001f`). So
   `\/` becomes `/`, `é` becomes `é`, and `😀` becomes `😀`.
5. The properties of each object in the order they came.

Normalizing a normalized text gives the same text, and the normalized text is never longer than the given one. It is
not canonical: `{"a":1,"b":2}` and `{"b":2,"a":1}`, or `{"n":1}` and `{"n":1.0}`, are different documents. The size of
a row (at most 256 MiB) counts the normalized text, so an indented document that is too large as given is admitted
when its normalized text fits.

```rust
use krummi::table::json::Json;
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("doc", ColumnType::Json)],
        &["id"],
    );
    let t = write.open_table_with("docs", TableMode::CreateNew(&schema))?;

    let cases = [
        (r#" { "b" : 1.50 , "a" : [ true , "é\/" ] } "#, r#"{"b":1.50,"a":[true,"é/"]}"#),
        (r#"[1.50e+3, -0, 1e400]"#, r#"[1.50e+3,-0,1e400]"#),
        (r#""é 😀 \u000A \u0001 \"""#, r#""é 😀 \n \u0001 \"""#),
        ("\t42\r\n", "42"),
        (r#"{"z":1, "a":2}"#, r#"{"z":1,"a":2}"#), // the order is kept
    ];
    for (id, (given, _)) in (0..).zip(cases) {
        write.insert_row(&t, &[("id", Value::I64(id)), ("doc", Value::Json(Json::new(given)))])?;
    }
    for (id, (_, normalized)) in (0..).zip(cases) {
        let row = write.get_row(&t, &[Value::I64(id)])?.expect("inserted");
        assert_eq!(row.json(1).expect("not null").text(), Some(normalized));
    }
    Ok(())
}
```

## Validation errors

A document that is not valid fails the write with `Error::InvalidValue`, with the table, the column, the rule
`ValueRule::InvalidJson` and a `json` field, a `JsonFault` that says why and where:

- `cause`, a `JsonCause`;
- `position`, the number of bytes of the given text before the fault, from 0;
- `pointer`, the RFC 6901 pointer of the innermost object or array open at the fault, `""` for the root or none.

| `JsonCause` | Position |
|---|---|
| `InvalidUtf8` | the first byte of the first sequence that is not UTF-8, also an incomplete one at the end |
| `Syntax` | the first character with which the text stops being the start of a JSON text, or the text's length when it ends too soon (0 for an empty text) |
| `LoneSurrogate` | the `\` of the escape that does not form a pair |
| `RepeatedProperty(name)` | the opening quote of the later name; `name` is the decoded name |
| `TooDeep` | the `{` or `[` that opens level 129 |

When a text has several faults, the one at the lowest position is reported and, at the same position, the first cause
in this table's order. A null in a `not_null` column, and a value of another type, are reported before an invalid
document ([data types](data-types.md#errors)). A valid document that makes the row larger than its maximum fails after
that, with `Error::InvalidSize`. In a bulk load, the first row that fails aborts the whole load.

```rust
use krummi::table::json::Json;
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, JsonCause, Schema, Store, StoreOptions, TableMode, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("doc", ColumnType::Json)],
        &["id"],
    );
    let t = write.open_table_with("docs", TableMode::CreateNew(&schema))?;

    let deep = format!("{}{}", "[".repeat(129), "]".repeat(129));
    let deepest = "/0".repeat(127); // the array open at level 128
    let cases: Vec<(Json, JsonCause, u64, &str)> = vec![
        (Json::new(r#"{"a":1,"b":{"a":2,"a":3}}"#), JsonCause::RepeatedProperty("a".into()), 18, "/b"),
        (Json::new("[1,]"), JsonCause::Syntax, 3, ""),
        (Json::new(""), JsonCause::Syntax, 0, ""),
        (Json::new(r#"{"a":{"s":"\ud800"}}"#), JsonCause::LoneSurrogate, 11, "/a"),
        (Json::from_bytes(&b"[\"\xFF\"]"[..]), JsonCause::InvalidUtf8, 2, ""),
        (Json::new(deep), JsonCause::TooDeep, 128, &deepest),
    ];
    for (json, cause, position, pointer) in cases {
        let error = write.insert_row(&t, &[("id", Value::I64(1)), ("doc", Value::Json(json))]).unwrap_err();
        match error {
            Error::InvalidValue { column, rule, json: Some(fault), .. } => {
                let fault = *fault;
                assert_eq!(column.as_deref(), Some("doc"));
                assert_eq!(rule, ValueRule::InvalidJson);
                assert_eq!((fault.cause, fault.position, &*fault.pointer), (cause, position, pointer));
            }
            other => return Err(other.into()),
        }
    }

    // The error's text carries the fault too.
    let error = write
        .insert_row(&t, &[("id", Value::I64(1)), ("doc", Value::Json(Json::new("[1,]")))])
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        r#"invalid value in column "doc" of table "docs" of temporary store: not a valid JSON document: syntax error at byte 3 in """#
    );
    Ok(())
}
```

## Reading a document back

A JSON column reads like any column: `row.json(position)` returns `Option<Json>`, `None` for a null; `row.get(name)`
and `row.value(position)` return `Value::Json`. It also reads through a projection with the `JsonDocument` type, and
as an included column of an index entry, without reading the row ([rows](rows.md), [indexes](indexes.md)).

The `Json` of a row borrows the document's normalized text from the page, without a copy, and lives as long as the
row's other borrowed values: until the next operation that may change the transaction. `json.text()` gives that text
as `&str`, and `json.document()` gives the root `JsonValue` to navigate it. A row copied with `row.into_owned()`, or a
document with `json.into_owned()`, owns its text and is read the same way.

Reading does not check the stored text again: it was checked when it was written.

## Navigating a document

A `JsonValue` is the document or any value inside it. Its text is the fragment of the normalized text that forms it,
borrowed, and is itself a normalized JSON text: in `{"a":[1,2],"s":"x"}`, the value of `a` has the text `[1,2]` and
that of `s` the text `"x"`, with its quotes.

| Method | Returns | `None` when |
|---|---|---|
| `kind()` | `JsonKind`: `Object`, `Array`, `String`, `Number`, `Bool` or `Null` | never |
| `text()` | `&str`, the value's normalized text | never |
| `len()`, `is_empty()` | the number of properties or elements | not an object or array |
| `members()` | `Members`, an iterator of `(Cow<str>, JsonValue)` in stored order | not an object |
| `get(name)` | the value of the property `name`, compared with the decoded names | absent, or not an object |
| `elements()` | `Elements`, an iterator of `JsonValue` in order | not an array |
| `element(index)` | the element at `index`, from 0 | absent, or not an array |
| `pointer(p)` | `Result<Option<JsonValue>>`: the value of the RFC 6901 pointer `p` | absent |

A property name from `members()` is decoded as `as_text` decodes a string: borrowed unless it has escapes.

**JSON pointers** are relative to the value they are asked of. The empty pointer `""` is the value itself; otherwise
each step is `/` and a name, in which `~1` stands for `/` and `~0` for `~`. A step on an object takes its property of
that name; on an array, the element at that index when the step is `0` or digits without a leading zero (`10`, not
`010` or `+1`; `-` is absent); on any other value, absent. A malformed pointer, one that is not empty and does not
start with `/`, or with a `~` not followed by `0` or `1`, fails with `Error::InvalidValue` and
`ValueRule::MalformedJsonPointer`, with no table or column, before navigating.

```rust
use krummi::table::json::{Json, JsonKind};
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, Schema, Store, StoreOptions, TableMode, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("doc", ColumnType::Json)],
        &["id"],
    );
    let t = write.open_table_with("docs", TableMode::CreateNew(&schema))?;
    let text = r#"{"a":[1,2],"s":"x","a/b":{"~":[10,20]},"e":{}}"#;
    write.insert_row(&t, &[("id", Value::I64(1)), ("doc", Value::Json(Json::new(text)))])?;

    let row = write.get_row(&t, &[Value::I64(1)])?.expect("inserted");
    let json = row.json(1).expect("not null");
    let doc = json.document().expect("read back");

    // Classes, texts and sizes.
    assert_eq!(doc.len(), Some(4));
    let a = doc.get("a").expect("present");
    assert_eq!((a.kind(), a.text(), a.len()), (JsonKind::Array, "[1,2]", Some(2)));
    assert_eq!(doc.get("s").map(|s| s.text()), Some(r#""x""#));
    assert_eq!(doc.get("e").and_then(|e| e.is_empty()), Some(true));
    assert_eq!(a.get("x"), None); // an array has no properties
    assert_eq!(a.element(2), None); // past the last element

    // Properties in stored order, elements in order.
    let names: Vec<String> = doc.members().expect("an object").map(|(name, _)| name.into_owned()).collect();
    assert_eq!(names, ["a", "s", "a/b", "e"]);
    let numbers: Vec<&str> = a.elements().expect("an array").map(|v| v.text()).collect();
    assert_eq!(numbers, ["1", "2"]);

    // Pointers.
    assert_eq!(doc.pointer("/a~1b/~0/1")?.map(|v| v.text()), Some("20"));
    assert_eq!(doc.pointer("")?, Some(doc));
    assert_eq!(a.pointer("/0")?.map(|v| v.text()), Some("1")); // relative to `a`
    assert_eq!(doc.pointer("/a/01")?, None);
    assert_eq!(doc.pointer("/a/-")?, None);
    assert_eq!(doc.pointer("/missing/x")?, None);
    let error = doc.pointer("a").unwrap_err();
    assert!(matches!(
        error,
        Error::InvalidValue { rule: ValueRule::MalformedJsonPointer, table: None, column: None, .. }
    ));
    Ok(())
}
```

## Reading values as Krummi types

The typed reads of a `JsonValue` return the value or `None`, never an error for the value itself: a value of another
class, `null`, or a value outside the type all give `None`.

| Method | Reads | Gives `None` for |
|---|---|---|
| `as_text()` → `Cow<str>` | a string without its quotes; borrowed, or decoded into a copy when it has escapes (`"a\nb"`) | not a string |
| `as_bool()` → `bool` | `true`, `false` | anything else |
| `as_i64()`, `as_u64()` | a number whose exact value is an integer in range, in any notation: `100`, `100.0`, `1e2`, `1.00e2` give 100; `-0` gives 0 | `100.6`, `1e-1`, `9223372036854775808` in `i64`, `-1` in `u64` |
| `as_decimal(p, s)` → `Result<Option<Decimal>>` | a number rounded to scale `s` with ties away from zero (`1.235` gives 1.24 at scale 2) | more than `p` digits once rounded (`99.995` at `(4, 2)`) |
| `as_f64()` | a number rounded to the nearest `f64`, `-0` as `+0`; `1e-400` gives `+0` | outside the finite range (`1e400`) |
| `as_date()` → `i32` | a string exactly `YYYY-MM-DD` that exists, in the `date` domain, as days since 1970-01-01 | `"2026-02-30"`, a year that is not four digits or is `0000` |
| `as_timestamp()` → `i128` | a string `YYYY-MM-DDTHH:MM:SS[.fraction]` with an offset `Z`, `+HH:MM` or `-HH:MM`, converted to UTC and rounded to the microsecond, in the `timestamp` domain, as nanoseconds since the Unix epoch | no offset, lowercase `t` or `z`, a space for `T`, a leap second `:60`, a point without digits, outside the domain |

`as_decimal` checks `p` and `s` first: outside 1 ≤ p ≤ 38 and 0 ≤ s ≤ p it fails with `Error::InvalidValue` and
`ValueRule::InvalidReadScale`, with no table or column. A number is read without expanding its exponent, so the cost
of a read follows the length of its text, not its value: `1e999999999999` gives `None` in `i64` at once.

There are no reads as `time`, `duration`, `uuid` or `ulid`: read the string with `as_text` and convert it yourself
(`as_text()?.parse::<Uuid>()` for a `uuid`).

```rust
use krummi::table::json::Json;
use krummi::table::value::{ColumnType, Decimal, Value};
use krummi::{Column, Error, Schema, Store, StoreOptions, TableMode, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("doc", ColumnType::Json)],
        &["id"],
    );
    let t = write.open_table_with("docs", TableMode::CreateNew(&schema))?;
    let text = r#"{"plain":"abc","escaped":"a\nb","ok":true,"n":[100,100.0,1e2,100.6,-1,1.235,99.995,1e400,-1e-400],
        "day":"2026-11-11","bad_day":"2026-02-30","at":"2026-11-11T00:30:00+01:00",
        "fine":"2026-11-11T10:23:00.1234567Z","local":"2026-11-11T10:23:00"}"#;
    write.insert_row(&t, &[("id", Value::I64(1)), ("doc", Value::Json(Json::new(text)))])?;

    let row = write.get_row(&t, &[Value::I64(1)])?.expect("inserted");
    let json = row.json(1).expect("not null");
    let doc = json.document().expect("read back");
    let at = |pointer: &str| doc.pointer(pointer).ok().flatten().expect("present");

    // Text: borrowed without escapes, a copy with them.
    assert!(matches!(at("/plain").as_text(), Some(std::borrow::Cow::Borrowed("abc"))));
    assert!(matches!(at("/escaped").as_text(), Some(std::borrow::Cow::Owned(s)) if s == "a\nb"));
    assert_eq!(at("/ok").as_bool(), Some(true));
    assert_eq!(at("/plain").as_i64(), None); // a string is not a number

    // Integers in any notation, when they are exact.
    let ints: Vec<Option<i64>> = (0..5).map(|i| at(&format!("/n/{i}")).as_i64()).collect();
    assert_eq!(ints, [Some(100), Some(100), Some(100), None, Some(-1)]);
    assert_eq!(at("/n/4").as_u64(), None);

    // Decimals rounded to the scale, floats to the nearest.
    assert_eq!(at("/n/5").as_decimal(10, 2)?, Some(Decimal { mantissa: 124, scale: 2 }));
    assert_eq!(at("/n/6").as_decimal(4, 2)?, None);
    let error = at("/n/5").as_decimal(39, 0).unwrap_err();
    assert!(matches!(error, Error::InvalidValue { rule: ValueRule::InvalidReadScale, .. }));
    assert_eq!(at("/n/7").as_f64(), None);
    assert!(at("/n/8").as_f64().is_some_and(|x| x == 0.0 && x.is_sign_positive()));

    // Dates and instants.
    assert_eq!(at("/day").as_date(), Some(20_768));
    assert_eq!(at("/bad_day").as_date(), None);
    let instant = at("/at").as_timestamp().expect("an instant");
    assert_eq!(Value::Timestamp(instant).to_string(), "2026-11-10T23:30:00Z");
    let fine = at("/fine").as_timestamp().expect("an instant");
    assert_eq!(Value::Timestamp(fine).to_string(), "2026-11-11T10:23:00.123457Z");
    assert_eq!(at("/local").as_timestamp(), None); // no offset
    Ok(())
}
```

## Nulls and default values

A JSON column can hold a null, which is not the document `null`: `row.json(position)` returns `None` for the first and
a `Json` whose text is `null` for the second.

A default value is given as a document, `Column::new(…).with_default(Value::Json(Json::new(text)))`. It is stored
normalized, and `describe_table` returns it so. Unlike in other columns, the document `null` is a default value, not
the lack of one. A default that is not a valid document fails with `Error::InvalidSchema` and
`SchemaRule::InvalidDefault`, without the cause, position or pointer of a write. When a table is opened with a schema,
two JSON defaults are the same when their normalized texts are: `{ "a" : 1 }` matches `{"a":1}`.

```rust
use krummi::table::json::{Json, JsonKind};
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, Schema, SchemaRule, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let settings = Value::Json(Json::new(r#"{ "theme" : "dark" }"#));
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("maybe", ColumnType::Json),
            Column::new("settings", ColumnType::Json).with_default(settings),
            Column::new("extra", ColumnType::Json).with_default(Value::Json(Json::new("null"))),
        ],
        &["id"],
    );
    let t = write.open_table_with("users", TableMode::CreateNew(&schema))?;
    write.insert_row(&t, &[("id", Value::I64(1))])?;
    {
        let row = write.get_row(&t, &[Value::I64(1)])?.expect("inserted");
        assert!(row.json(1).is_none()); // the column's null
        assert_eq!(row.json(2).expect("its default").text(), Some(r#"{"theme":"dark"}"#));
        let extra = row.json(3).expect("the document null");
        assert_eq!(extra.document().map(|d| d.kind()), Some(JsonKind::Null));
    }

    // The default is stored normalized.
    let described = write.describe_table("users")?;
    let default = described.column("settings").and_then(|c| c.default.clone());
    assert_eq!(default, Some(Value::Json(Json::new(r#"{"theme":"dark"}"#))));

    // An invalid default is an invalid schema.
    let bad = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("doc", ColumnType::Json).with_default(Value::Json(Json::new("{"))),
        ],
        &["id"],
    );
    let error = write.open_table_with("bad", TableMode::CreateNew(&bad)).err().expect("invalid");
    assert!(matches!(error, Error::InvalidSchema { rule: SchemaRule::InvalidDefault, .. }));
    Ok(())
}
```

## Equality of documents

Two `Json` values are equal when their bytes are (`==`, and `Hash` agrees). A document read back holds its normalized
text, so it equals a `Json` given with that same text, but not one given with another spacing or escape form: compare
read-back documents with each other, or with normalized texts.

The same rule decides when an update changes a row: an update whose document has the same normalized text as the
stored one is identical and changes nothing, so a document that differs only in whitespace or escapes does not change
the row, while `{"n":1}` against `{"n":1.0}` does.

```rust
use krummi::table::json::Json;
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("doc", ColumnType::Json)],
        &["id"],
    );
    let t = write.open_table_with("docs", TableMode::CreateNew(&schema))?;
    write.insert_row(&t, &[("id", Value::I64(1)), ("doc", Value::Json(Json::new(r#"[ 1, "a" ]"#)))])?;
    write.insert_row(&t, &[("id", Value::I64(2)), ("doc", Value::Json(Json::new(r#"[1,"a"]"#)))])?;

    let read = |id| -> Result<Json<'static>, krummi::Error> {
        let row = write.get_row(&t, &[Value::I64(id)])?.expect("inserted");
        Ok(row.json(1).expect("not null").into_owned())
    };
    let (first, second) = (read(1)?, read(2)?);
    assert_eq!(first, second); // the same normalized text
    assert_eq!(first, Json::new(r#"[1,"a"]"#));
    assert_ne!(first, Json::new(r#"[ 1, "a" ]"#)); // another text
    Ok(())
}
```

## What JSON columns cannot do

| Not allowed | Error |
|---|---|
| a JSON column in the primary key | `Error::InvalidSchema`, `SchemaRule::InvalidKeyColumn` |
| a JSON column in the key of an index | `Error::InvalidSchema`, `SchemaRule::VectorIndexKey` |
| statistics of a JSON column: estimating, reading statistics or analyzing it | `Error::InvalidSchema`, `SchemaRule::JsonColumn` ([statistics](statistics.md)) |
| changing a column's type to or from `json` | `Error::InvalidSchema`, `SchemaRule::UnsupportedChange` |

A JSON column can be an included column of an index, whose entries then return the document without reading the row
([indexes](indexes.md)). Krummi has no indexes on paths inside a document and no queries over them: read the
documents and navigate them.

```rust
use krummi::table::value::ColumnType;
use krummi::{
    Column, Error, IndexDefinition, IndexKey, IndexMode, Schema, SchemaRule, Store, StoreOptions, TableMode,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;

    let keyed = Schema::new(vec![Column::new("doc", ColumnType::Json).not_null()], &["doc"]);
    let error = write.open_table_with("keyed", TableMode::CreateNew(&keyed)).err().expect("invalid");
    assert!(matches!(error, Error::InvalidSchema { rule: SchemaRule::InvalidKeyColumn, .. }));

    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String),
            Column::new("doc", ColumnType::Json),
        ],
        &["id"],
    );
    let t = write.open_table_with("docs", TableMode::CreateNew(&schema))?;
    let by_doc = IndexDefinition::new("by_doc", vec![IndexKey::new("doc")]);
    let error = write.create_index(&t, &by_doc, IndexMode::CreateNew).unwrap_err();
    assert!(matches!(error, Error::InvalidSchema { rule: SchemaRule::VectorIndexKey, .. }));

    // As an included column it is fine.
    let by_name = IndexDefinition::new("by_name", vec![IndexKey::new("name")]).with_included(&["doc"]);
    write.create_index(&t, &by_name, IndexMode::CreateNew)?;
    Ok(())
}
```

## Costs

- **Writing** validates and normalizes the document in one pass over its text, plus sorting each object's property
  names to find repeats: O(n log n) at worst.
- **Reading** the whole document costs no copy: `json.text()` borrows the page.
- **Navigating walks the text.** A document is stored as its normalized text, with no index of its properties, so
  `get`, `element`, `pointer` and `len` scan the text from the start of the object or array: reading a late property,
  or an element late in a large array, walks every value before it. To read several properties of one object, iterate
  `members()` once instead of calling `get` for each.
- **Typed reads** borrow their text when they can: `as_text` and the names of `members()` copy only a string with
  escapes; numbers are read without expanding their exponents.

See also: [data types](data-types.md), [tables](tables.md), [rows](rows.md), [indexes](indexes.md),
[statistics](statistics.md), [errors](errors.md).
