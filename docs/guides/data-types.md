# Data types

This guide covers every column type of a Krummi table: how to declare a column of the type (`ColumnType`), how to
give a value (`Value`), how to read it back from a row (the typed accessors of `TableRow`), its valid range, how it
sorts in a primary key or an index, and the error an invalid value gives. It also covers nulls, default values and
the conversions that Krummi does or refuses. Creating tables and storing rows are covered in
[tables](tables.md) and [rows](rows.md); this guide uses them only to show each type.

## Summary

| Type | `ColumnType` | `Value` | Typed accessor | Range | Order in a key |
|---|---|---|---|---|---|
| `bool` | `Bool` | `Bool(bool)` | `bool` | `false`, `true` | `false` before `true` |
| `i8` … `i64` | `I8`, `I16`, `I32`, `I64` | `I8(i8)` … `I64(i64)` | `i8` … `i64` | the Rust type's | numeric |
| `u8` … `u64` | `U8`, `U16`, `U32`, `U64` | `U8(u8)` … `U64(u64)` | `u8` … `u64` | the Rust type's | numeric |
| `f32`, `f64` | `F32`, `F64` | `F32(f32)`, `F64(f64)` | `f32`, `f64` | every value; `-0` stored as `+0`, one NaN | numeric, `-inf` first, NaN after `+inf` |
| `decimal(p, s)` | `Decimal { precision, scale }` | `Decimal(Decimal)` | none (`value`, `get`) | up to `p` digits, 1 ≤ p ≤ 38, 0 ≤ s ≤ p | numeric |
| `string` | `String` | `String(Cow<str>)` | `text`, `text_bytes` | any UTF-8, U+0000 and `""` included | byte by byte, no collation |
| `bytes` | `Bytes` | `Bytes(Cow<[u8]>)` | `bytes` | any bytes, empty included | byte by byte |
| `date` | `Date` | `Date(i32)`, days since 1970-01-01 | `date` | 0001-01-01 to 9999-12-31 | chronological |
| `time` | `Time` | `Time(i64)`, nanoseconds since midnight | `time` | 00:00:00 to 23:59:59.999999 | chronological |
| `timestamp` | `Timestamp` | `Timestamp(i128)`, nanoseconds since the Unix epoch, UTC | `timestamp` | 0001-01-01T00:00:00Z to 9999-12-31T23:59:59.999999Z | chronological |
| `duration` | `Duration` | `Duration(i128)`, nanoseconds | `duration` | ±922,337,203,685,477,580 µs | numeric |
| `uuid` | `Uuid(UuidOrder::Rfc9562)` | `Uuid(Uuid)` | `uuid` | any 16 bytes | the 16 bytes in RFC 9562 order |
| `uuid(sql server)` | `Uuid(UuidOrder::SqlServer)` | `Uuid(Uuid)` | `uuid` | any 16 bytes | SQL Server's `uniqueidentifier` order |
| `ulid` | `Ulid` | `Ulid(Ulid)` | `ulid` | any 16 bytes | its bytes: time, then random part |
| `vector(e, d)` | `Vector(VectorType)` | `Vector(Cow<[u8]>)` | none (`value`, `get`) | dimension 1 to 4,096; finite elements | not ordered: never in a key |
| `json` | `Json` | `Json(Json)` | `json` | a valid JSON document | not ordered: never in a key |

For `time`, `timestamp` and `duration` you always give and read nanoseconds, but Krummi keeps them to the
microsecond: see [Nanoseconds in your code, microseconds in the store](#nanoseconds-in-your-code-microseconds-in-the-store). Every type lives in `krummi::table::value`, except `Uuid` and `Ulid`
(`krummi::ids`) and `Json` (`krummi::table::json`).

## Steps

1. Declare each column with `Column::new(name, ColumnType::…)`, and `.not_null()` or `.with_default(…)` when you
   need them.
2. Give each value as the `Value` variant of its column's type. Krummi never converts a value of one type into
   another.
3. Read the values back with the typed accessor of the column's type (`row.i64(0)`, `row.text(1)`, …), or as a
   `Value` with `row.get(name)` or `row.value(position)`.
4. Handle `Error::InvalidValue`: its `rule` (a `ValueRule`) says which rule the value broke, and `table` and `column`
   say where.

```rust
use krummi::table::value::{ColumnType, Decimal, Value};
use krummi::{Column, Error, Schema, Store, StoreOptions, TableMode, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;

    // 1. Declare the columns.
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String).not_null(),
            Column::new("price", ColumnType::Decimal { precision: 10, scale: 2 }),
            Column::new("in_stock", ColumnType::Bool).with_default(Value::Bool(true)),
        ],
        &["id"],
    );
    let products = write.open_table_with("products", TableMode::CreateNew(&schema))?;

    // 2. Give each value as its column's variant: 19.99 is the mantissa 1999 at scale 2.
    write.insert_row(
        &products,
        &[
            ("id", Value::I64(1)),
            ("name", Value::String("Lamp".into())),
            ("price", Value::Decimal(Decimal { mantissa: 1999, scale: 2 })),
        ],
    )?;

    // 3. Read them back: by position with the typed accessors, or by name as a `Value`.
    {
        let row = write.get_row(&products, &[Value::I64(1)])?.expect("inserted");
        assert_eq!(row.i64(0), Some(1));
        assert_eq!(row.text(1).as_deref(), Some("Lamp"));
        assert_eq!(row.get("price"), Some(Value::Decimal(Decimal { mantissa: 1999, scale: 2 })));
        assert_eq!(row.bool(3), Some(true)); // the default value
    }

    // 4. A value of another type fails, and names the column and the rule.
    let error = write
        .insert_row(&products, &[("id", Value::I32(2)), ("name", Value::String("Desk".into()))])
        .unwrap_err();
    match error {
        Error::InvalidValue { table, column, rule, .. } => {
            assert_eq!(table.as_deref(), Some("products"));
            assert_eq!(column.as_deref(), Some("id"));
            assert_eq!(rule, ValueRule::WrongType);
        }
        other => return Err(other.into()),
    }

    write.commit()?;
    Ok(())
}
```

A failed write of a row changes nothing, and the transaction stays usable: the next insert above could still run.

## Reading values back

Every type has two ways to be read:

- `row.value(position)` and `row.get(name)` return `Option<Value>`: `None` only for a position or name the row does
  not have, and `Some(Value::Null)` for a null. They read any column.
- The typed accessors, named after the type (`bool`, `i8` … `u64`, `f32`, `f64`, `date`, `time`, `timestamp`,
  `duration`, `uuid`, `ulid`, `text`, `text_bytes`, `bytes`, `json`), take the position of the column in schema order
  and return `Option<T>`: `None` for a null or a position past the columns. They skip building a `Value`, so they
  cost less in a scan that reads every column. Each one panics on a column of another type.

`decimal` and vector columns have no typed accessor on `TableRow`; read them with `value` or `get`, or with a
projection, which reads them as `krummi::table::value::Decimal` and `krummi::table::projection::Vector` (see
[rows](rows.md) for projections).

Text, bytes, vectors and JSON documents borrow the transaction's pages without copying them: the row and its values
live until the next operation that may change the transaction. `row.into_owned()` and `value.into_owned()` copy them
when you need them longer.

## Booleans and integers

`ColumnType::Bool` holds `Value::Bool`; the eight integer types `I8`, `I16`, `I32`, `I64`, `U8`, `U16`, `U32` and
`U64` hold the `Value` variant of the same name with the Rust integer of that width. Every value of the Rust type is
valid, so an integer never fails for its range: the only rule it can break is giving another type, including another
width or sign (`Value::I32` in an `i64` column fails with `ValueRule::WrongType`).

In a key, `false` sorts before `true` and integers sort by numeric value, negatives first.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, Schema, Store, StoreOptions, TableMode, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I32).not_null(),
            Column::new("small", ColumnType::I8),
            Column::new("big", ColumnType::U64),
            Column::new("done", ColumnType::Bool),
        ],
        &["id"],
    );
    let t = write.open_table_with("numbers", TableMode::CreateNew(&schema))?;
    for id in [3, -100, 0, -5] {
        write.insert_row(
            &t,
            &[
                ("id", Value::I32(id)),
                ("small", Value::I8(i8::MIN)),
                ("big", Value::U64(u64::MAX)),
                ("done", Value::Bool(id > 0)),
            ],
        )?;
    }

    // The rows come back in the numeric order of the key.
    let mut ids = Vec::new();
    for row in write.range_rows(&t, &[], ..)? {
        let row = row?;
        assert_eq!(row.i8(1), Some(-128));
        assert_eq!(row.u64(2), Some(18_446_744_073_709_551_615));
        ids.push(row.i32(0).expect("a key column is never null"));
    }
    assert_eq!(ids, [-100, -5, 0, 3]);

    // Another width is another type.
    let error = write.insert_row(&t, &[("id", Value::I64(7))]).unwrap_err();
    assert!(matches!(error, Error::InvalidValue { rule: ValueRule::WrongType, .. }));
    Ok(())
}
```

## Floating-point numbers

`ColumnType::F32` and `ColumnType::F64` hold `Value::F32` and `Value::F64`. Every value is valid, infinities and NaN
included, and Krummi normalizes two of them when it writes: `-0` is stored as `+0`, and every NaN as the one positive
quiet NaN. So a row reads back `+0` for a `-0`, and the key and the unique indexes treat `-0` and `+0`, or two NaN,
as the same value.

In a key, floats sort by numeric value: `-inf` first, `+inf` after every finite value, and NaN after `+inf`.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, ErrorKind, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(vec![Column::new("x", ColumnType::F64).not_null()], &["x"]);
    let t = write.open_table_with("floats", TableMode::CreateNew(&schema))?;
    for x in [f64::NAN, 1.5, f64::INFINITY, -0.0, f64::NEG_INFINITY, -2.0] {
        write.insert_row(&t, &[("x", Value::F64(x))])?;
    }

    // `-0` was stored as `+0`, so `+0` repeats the key.
    let error = write.insert_row(&t, &[("x", Value::F64(0.0))]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::DuplicateKey);

    let mut order = Vec::new();
    for row in write.range_rows(&t, &[], ..)? {
        order.push(format!("{:?}", row?.f64(0).expect("not null")));
    }
    assert_eq!(order, ["-inf", "-2.0", "0.0", "1.5", "inf", "NaN"]);
    Ok(())
}
```

## Decimals

`ColumnType::Decimal { precision, scale }` is `decimal(p, s)`: a fixed-point number of at most `precision` digits,
`scale` of them after the point, stored as a 128-bit integer. It holds `Value::Decimal(Decimal { mantissa, scale })`,
whose value is `mantissa` × 10^−`scale`: 19.99 is `Decimal { mantissa: 1999, scale: 2 }`.

- **Declaring**: 1 ≤ `precision` ≤ 38 (`MAX_DECIMAL_PRECISION`) and 0 ≤ `scale` ≤ `precision`; otherwise creating
  the table fails with `Error::InvalidSchema` and `SchemaRule::DecimalOutOfLimits`.
- **Giving**: a value may have any scale. Krummi rounds it to the column's scale, with ties away from zero as SQL
  Server does (1.235 gives 1.24 in a `decimal(10, 2)`, and −1.235 gives −1.24), or adds zeros (7 gives 7.00).
- **Errors**: a value that has more than `precision` digits at the column's scale, also after rounding, fails with
  `ValueRule::DecimalPrecision` (99.995 in a `decimal(4, 2)` rounds to 100.00, five digits).
- **Reading**: a value read back always has the column's scale. Its text form (`to_string()`) is `1.24`.
- **Order**: numeric.

```rust
use krummi::table::value::{ColumnType, Decimal, Value};
use krummi::{Column, Error, Schema, SchemaRule, Store, StoreOptions, TableMode, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("amount", ColumnType::Decimal { precision: 10, scale: 2 }),
            Column::new("rate", ColumnType::Decimal { precision: 4, scale: 2 }),
        ],
        &["id"],
    );
    let t = write.open_table_with("money", TableMode::CreateNew(&schema))?;

    let given = [(1, 1235, 3), (2, -1235, 3), (3, 7, 0)]; // 1.235, -1.235 and 7
    for (id, mantissa, scale) in given {
        write.insert_row(
            &t,
            &[("id", Value::I64(id)), ("amount", Value::Decimal(Decimal { mantissa, scale }))],
        )?;
    }
    let mut read = Vec::new();
    for row in write.range_rows(&t, &[], ..)? {
        let amount = row?.get("amount").expect("a column of the table");
        read.push(amount.to_string());
        assert!(matches!(amount, Value::Decimal(Decimal { scale: 2, .. })));
    }
    assert_eq!(read, ["1.24", "-1.24", "7.00"]);

    // 99.995 rounds to 100.00, which does not fit in decimal(4, 2).
    let error = write
        .insert_row(
            &t,
            &[("id", Value::I64(4)), ("rate", Value::Decimal(Decimal { mantissa: 99_995, scale: 3 }))],
        )
        .unwrap_err();
    assert!(matches!(error, Error::InvalidValue { rule: ValueRule::DecimalPrecision, .. }));

    // A precision outside 1 to 38 is an invalid schema.
    let bad = Schema::new(
        vec![Column::new("d", ColumnType::Decimal { precision: 39, scale: 0 }).not_null()],
        &["d"],
    );
    let error = write.open_table_with("bad", TableMode::CreateNew(&bad)).err().expect("invalid");
    assert!(matches!(error, Error::InvalidSchema { rule: SchemaRule::DecimalOutOfLimits, .. }));
    Ok(())
}
```

## Text and bytes

`ColumnType::String` holds `Value::String`, any valid UTF-8 text, U+0000 included; `ColumnType::Bytes` holds
`Value::Bytes`, any bytes. The empty text and the empty bytes are valid values, also in a primary key. Both are
`Cow`s, so you can give a borrowed `&str` or `&[u8]` (`"text".into()`) or an owned one without copying it.

- **Reading**: `row.text(position)` returns the text as `Cow<str>`, borrowed from the page; `row.text_bytes`
  returns its bytes without checking that they are UTF-8; `row.bytes(position)` returns `Cow<[u8]>`.
- **Order**: byte by byte, with no collation: `"B"` sorts before `"a"`, a text before any longer text that starts
  with it, and `"é"` after `"z"`. Krummi has no collations; to sort without case, store a folded copy of the text in
  another column and key on it.
- **Limits**: no limit of its own beyond the row, at most 256 MiB stored, and the primary key, at most 2,048 bytes
  stored (`krummi::table::key::MAX_KEY`). In a key, a text or bytes value takes its bytes, one more for each zero
  byte, and two bytes of framing. Going past either fails with `Error::InvalidSize`.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, ErrorKind, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("name", ColumnType::String).not_null(), Column::new("raw", ColumnType::Bytes)],
        &["name"],
    );
    let t = write.open_table_with("words", TableMode::CreateNew(&schema))?;
    for name in ["b", "é", "aa", "", "a\0b", "B", "a"] {
        write.insert_row(&t, &[("name", Value::String(name.into())), ("raw", Value::Bytes(name.as_bytes().into()))])?;
    }

    let mut names = Vec::new();
    for row in write.range_rows(&t, &[], ..)? {
        let row = row?;
        assert_eq!(row.bytes(1).as_deref(), row.text_bytes(0).as_deref());
        names.push(row.text(0).expect("not null").into_owned());
    }
    assert_eq!(names, ["", "B", "a", "a\0b", "aa", "b", "é"]);

    // A primary key over 2,048 bytes is an invalid size.
    let long = "x".repeat(3000);
    let error = write.insert_row(&t, &[("name", Value::String(long.into()))]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidSize);
    Ok(())
}
```

## Dates, times, instants and durations

The four temporal types have no time zone and no calendar library behind them: you give and read plain numbers.

| Type | `Value` | Unit of the value | Stored | Domain (constants in `krummi::table::value`) |
|---|---|---|---|---|
| `date` | `Date(i32)` | days since 1970-01-01, proleptic Gregorian | days | `DATE_MIN` (0001-01-01) to `DATE_MAX` (9999-12-31), in days |
| `time` | `Time(i64)` | nanoseconds since midnight | microseconds | 0 to `TIME_MAX` (23:59:59.999999), in microseconds; no leap second |
| `timestamp` | `Timestamp(i128)` | nanoseconds since 1970-01-01T00:00:00Z, UTC | microseconds | `TIMESTAMP_MIN` to `TIMESTAMP_MAX` (0001-01-01T00:00:00Z to 9999-12-31T23:59:59.999999Z), in microseconds |
| `duration` | `Duration(i128)` | nanoseconds, signed | microseconds | `-DURATION_MAX` to `DURATION_MAX` (922,337,203,685,477,580), in microseconds |

- **Rounding**: a `time`, `timestamp` or `duration` with a fraction below the microsecond is rounded to the nearest
  microsecond, with ties toward the greater value (−1.5 µs gives −1 µs). The next section explains why.
- **Time zones**: a `timestamp` is an instant in UTC; Krummi stores no offset or zone. Convert local times to UTC
  before you give them. (A JSON document's instants with an offset are converted by `as_timestamp`, see
  [JSON columns](json.md).)
- **Errors**: a value outside its domain, also when rounding takes it there (9999-12-31T23:59:59.9999995Z), fails with
  `ValueRule::OutOfDomain`.
- **Reading**: `row.date` returns the days (`i32`), `row.time` the nanoseconds since midnight (`i64`), `row.timestamp`
  and `row.duration` the nanoseconds (`i128`).
- **Order**: chronological; durations by their signed value.
- **Text**: `Value::to_string()` writes `date` as `YYYY-MM-DD`, `time` as `HH:MM:SS[.f]`, `timestamp` as
  `YYYY-MM-DDTHH:MM:SS[.f]Z` and `duration` as seconds with an `s`. It is meant for error data; there is no parser.

```rust
use krummi::table::value::{ColumnType, TIMESTAMP_MAX, Value};
use krummi::{Column, Error, Schema, Store, StoreOptions, TableMode, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("day", ColumnType::Date).not_null(),
            Column::new("opens", ColumnType::Time),
            Column::new("seen", ColumnType::Timestamp),
            Column::new("took", ColumnType::Duration),
        ],
        &["day"],
    );
    let t = write.open_table_with("visits", TableMode::CreateNew(&schema))?;

    const SECOND: i64 = 1_000_000_000; // nanoseconds
    write.insert_row(
        &t,
        &[
            ("day", Value::Date(20_779)),                                   // 2026-11-22
            ("opens", Value::Time(9 * 3600 * SECOND + 500)),               // 09:00:00.0000005
            ("seen", Value::Timestamp(1_763_800_000_123_456_789)),          // below the microsecond
            ("took", Value::Duration(-1_500)),                              // -1.5 µs
        ],
    )?;

    {
        let row = write.get_row(&t, &[Value::Date(20_779)])?.expect("inserted");
        assert_eq!(row.date(0), Some(20_779));
        assert_eq!(row.get("day").map(|v| v.to_string()).as_deref(), Some("2026-11-22"));
        assert_eq!(row.time(1), Some(9 * 3600 * SECOND + 1_000)); // the tie went up
        assert_eq!(row.timestamp(2), Some(1_763_800_000_123_457_000));
        assert_eq!(row.duration(3), Some(-1_000)); // the tie went toward the greater value
    }
    assert_eq!(Value::Timestamp(0).to_string(), "1970-01-01T00:00:00Z");

    // Half a microsecond past the last instant rounds outside the domain.
    let past = i128::from(TIMESTAMP_MAX) * 1_000 + 500;
    let error = write
        .insert_row(&t, &[("day", Value::Date(0)), ("seen", Value::Timestamp(past))])
        .unwrap_err();
    assert!(matches!(error, Error::InvalidValue { rule: ValueRule::OutOfDomain, .. }));

    // A whole day is past the last time.
    let error = write
        .insert_row(&t, &[("day", Value::Date(0)), ("opens", Value::Time(86_400 * SECOND))])
        .unwrap_err();
    assert!(matches!(error, Error::InvalidValue { rule: ValueRule::OutOfDomain, .. }));
    Ok(())
}
```

### Nanoseconds in your code, microseconds in the store

`time`, `timestamp` and `duration` have two numbers that are easy to mix up: the **unit** of the value you handle
and the **precision** Krummi keeps.

| | Unit | Where you meet it |
|---|---|---|
| What you give and read | **nanoseconds** | `Value::Time`, `Value::Timestamp`, `Value::Duration`; `row.time`, `row.timestamp`, `row.duration`; range bounds |
| What the store keeps | **microseconds** (6 digits after the second) | the rows and keys on disk; the constants `TIME_MAX`, `TIMESTAMP_MIN`, `TIMESTAMP_MAX`, `DURATION_MAX` |

The unit you handle never changes: you write nanoseconds and you read nanoseconds. What changes is the precision:
Krummi rounds every value to the nearest microsecond when it writes it, so a value read back is always a multiple of
1,000 ns. The precision kept is that of a `datetime2(6)` in SQL Server or a `timestamp` in PostgreSQL.

**Why the value carries nanoseconds.** Rust's clocks and durations count in nanoseconds
(`std::time::Duration::as_nanos`, `SystemTime::duration_since`), and .NET's ticks are 100 ns. With nanoseconds in the
value you pass those numbers as they are, and Krummi rounds them once, with one rule (nearest microsecond, ties
toward the greater value), before it checks the domain. If the value carried microseconds, each application would
have to divide first, and truncating, flooring and rounding would store different values for the same instant.

**What it means for your code:**

1. Convert to nanoseconds, never to microseconds, before you build a `Value`.
2. Expect the value read back to differ from the one given when it had a fraction of a microsecond: compare stored
   values with each other, or round your own value to the microsecond before comparing.
3. The domain constants are in microseconds, the unit of what is kept: multiply them by 1,000 to compare them with a
   `Value` (`i128::from(TIMESTAMP_MAX) * 1_000` is the last instant, in nanoseconds).

```rust
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use krummi::table::value::{ColumnType, TIME_MAX, TIMESTAMP_MAX, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::U64).not_null(),
            Column::new("at", ColumnType::Timestamp),
            Column::new("took", ColumnType::Duration),
        ],
        &["id"],
    );
    let t = write.open_table_with("runs", TableMode::CreateNew(&schema))?;

    // Rust's clock and durations already give nanoseconds: pass them unchanged.
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as i128;
    let took = Duration::from_nanos(1_234_567).as_nanos() as i128; // 1.234567 ms
    write.insert_row(
        &t,
        &[("id", Value::U64(1)), ("at", Value::Timestamp(now)), ("took", Value::Duration(took))],
    )?;

    {
        let row = write.get_row(&t, &[Value::U64(1)])?.expect("inserted");
        // Read back in nanoseconds, rounded to the microsecond: always a multiple of 1,000.
        let at = row.timestamp(1).expect("not null");
        assert_eq!(at % 1_000, 0);
        assert!((at - now).abs() <= 500);
        assert_eq!(row.duration(2), Some(1_235_000)); // 1,234,567 ns rounds to 1,235 µs
    }

    // The constants are microseconds: multiply by 1,000 to get the nanoseconds of a value.
    let last_time = Value::Time(TIME_MAX * 1_000); // 23:59:59.999999
    assert_eq!(last_time.to_string(), "23:59:59.999999");
    let last_instant = i128::from(TIMESTAMP_MAX) * 1_000;
    assert_eq!(Value::Timestamp(last_instant).to_string(), "9999-12-31T23:59:59.999999Z");
    Ok(())
}
```

## UUID and ULID

`ColumnType::Uuid(order)` and `ColumnType::Ulid` hold `Value::Uuid(Uuid)` and `Value::Ulid(Ulid)`, two 16-byte
values of `krummi::ids` (`Uuid(pub [u8; 16])`, `Ulid(pub [u8; 16])`). Every 16 bytes are valid.

- **A `uuid` is always given in the byte order of RFC 9562**, also in a column with the SQL Server order: the order is
  a property of the column's key, not of the value. A `uuid` in the .NET byte order (`Guid.ToByteArray`) cannot be
  told apart and would be stored as another value, so convert it before you give it.
- **Order**: a `UuidOrder::Rfc9562` column sorts by the 16 bytes in order; a `UuidOrder::SqlServer` column, declared as
  `uuid(sql server)`, sorts as SQL Server compares a `uniqueidentifier`: bytes 10 to 15 first, then 8 and 9, 7 and 6,
  5 and 4, and 3, 2, 1, 0 last. A `ulid` sorts by its bytes: its 48-bit millisecond, then its random part.
- **Text**: each has one text form, written by `to_string()` and read by `str::parse`: a `uuid` as lowercase RFC 9562
  text with hyphens (8-4-4-4-12), a `ulid` as 26 uppercase Crockford Base32 characters. Any other form (uppercase or
  braces in a `uuid`, lowercase in a `ulid`) fails with `ValueRule::MalformedText`. Krummi does not convert a
  `Value::String` into a `uuid` or `ulid` column: you parse the text.
- **Reading**: `row.uuid(position)` and `row.ulid(position)`.
- **Generators**: `krummi::ids::ulid`, `uuid_v7` and `sequential_uuid` make increasing values for keys; see
  [sequences and IDs](sequences-and-ids.md).

```rust
use krummi::ids::{Ulid, Uuid};
use krummi::table::value::{ColumnType, UuidOrder, Value};
use krummi::{Column, Error, Schema, Store, StoreOptions, TableMode, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;

    // The same two values in a column of each order.
    let mut first_byte = [0u8; 16];
    first_byte[0] = 1;
    let mut last_byte = [0u8; 16];
    last_byte[15] = 1;
    let mut orders = Vec::new();
    for (name, order) in [("rfc", UuidOrder::Rfc9562), ("sql", UuidOrder::SqlServer)] {
        let schema = Schema::new(vec![Column::new("id", ColumnType::Uuid(order)).not_null()], &["id"]);
        let t = write.open_table_with(name, TableMode::CreateNew(&schema))?;
        write.insert_row(&t, &[("id", Value::Uuid(Uuid(first_byte)))])?;
        write.insert_row(&t, &[("id", Value::Uuid(Uuid(last_byte)))])?;
        let mut firsts = Vec::new();
        for row in write.range_rows(&t, &[], ..)? {
            firsts.push(row?.uuid(0).expect("not null").0[0]);
        }
        orders.push(firsts);
    }
    assert_eq!(orders[0], [0, 1]); // RFC 9562: byte 0 decides first
    assert_eq!(orders[1], [1, 0]); // SQL Server: byte 15 decides before byte 0

    // Text forms, parsed by the application.
    let uuid: Uuid = "0190a3b2-7c4d-7e8f-9a0b-1c2d3e4f5a6b".parse()?;
    assert_eq!(uuid.to_string(), "0190a3b2-7c4d-7e8f-9a0b-1c2d3e4f5a6b");
    let ulid: Ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAV".parse()?;
    assert_eq!(ulid.to_string(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    let error = "01arz3ndektsv4rrffq69g5fav".parse::<Ulid>().unwrap_err();
    assert!(matches!(error, Error::InvalidValue { rule: ValueRule::MalformedText, .. }));

    // A text is not a uuid value.
    let t = write.open_table("rfc")?;
    let error = write.insert_row(&t, &[("id", Value::String(uuid.to_string().into()))]).unwrap_err();
    assert!(matches!(error, Error::InvalidValue { rule: ValueRule::WrongType, .. }));
    write.insert_row(&t, &[("id", Value::Uuid(uuid))])?;
    Ok(())
}
```

## Vectors

`ColumnType::Vector(VectorType { element, dimension, cosine })` declares a vector of `dimension` elements of
`element`, and with `cosine: true` the cosine metric: the column then stores each vector's norm next to it and does
not admit zero vectors. It holds `Value::Vector(bytes)`, the elements in this layout:

| `VectorElement` | Bytes of a value (`VectorType::value_len`) | Layout |
|---|---|---|
| `F32` | 4 × dimension | IEEE binary32, little-endian |
| `F16` | 2 × dimension | IEEE binary16, little-endian |
| `I8` | dimension + 4 | one signed byte per element, then the vector's scale as a little-endian `f32` |
| `Bit` | dimension / 8 | element i is bit i mod 8, from the least significant, of byte i div 8 |

- **Declaring**: `dimension` from 1 to 4,096 (`MAX_VECTOR_DIMENSION`), a multiple of 8 for `Bit`, else
  `SchemaRule::VectorOutOfLimits`; `cosine` with `Bit` fails with `SchemaRule::CosineOnBit`.
- **Errors** of a value, in this order: a value of another length, `ValueRule::VectorDimension`; a NaN or an infinity
  in an `f32` or `f16` element or in an `i8` scale, `ValueRule::VectorNotFinite`; all elements zero (or an `i8` scale
  of zero) in a cosine column, `ValueRule::ZeroVectorCosine`.
- **Normalization**: a `-0` element (or `i8` scale) is stored as `+0`.
- **Reading**: `row.get(name)` returns `Value::Vector` with the value's bytes, without the norm.
- **Order**: vectors have none. A vector column cannot be in the primary key (`SchemaRule::InvalidKeyColumn`) or in
  the key of an index (`SchemaRule::VectorIndexKey`); it can be an included column of an index
  ([indexes](indexes.md)). Vectors have no statistics.

```rust
use krummi::table::value::{ColumnType, Value, VectorElement, VectorType};
use krummi::{Column, Error, Schema, SchemaRule, Store, StoreOptions, TableMode, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let embedding = VectorType { element: VectorElement::F32, dimension: 3, cosine: true };
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("embedding", ColumnType::Vector(embedding)),
        ],
        &["id"],
    );
    let t = write.open_table_with("items", TableMode::CreateNew(&schema))?;
    let f32s = |xs: &[f32]| xs.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();

    let bytes = f32s(&[1.0, 2.0, 2.0]);
    assert_eq!(bytes.len(), embedding.value_len());
    write.insert_row(&t, &[("id", Value::I64(1)), ("embedding", Value::Vector(bytes.clone().into()))])?;
    {
        let row = write.get_row(&t, &[Value::I64(1)])?.expect("inserted");
        assert_eq!(row.get("embedding"), Some(Value::Vector(bytes.into())));
    }

    // The three rules of a value.
    let cases = [
        (f32s(&[1.0, 2.0]), ValueRule::VectorDimension),
        (f32s(&[1.0, f32::NAN, 2.0]), ValueRule::VectorNotFinite),
        (f32s(&[0.0, -0.0, 0.0]), ValueRule::ZeroVectorCosine),
    ];
    for (bytes, expected) in cases {
        let error = write
            .insert_row(&t, &[("id", Value::I64(2)), ("embedding", Value::Vector(bytes.into()))])
            .unwrap_err();
        assert!(matches!(error, Error::InvalidValue { rule, .. } if rule == expected));
    }

    // A bit vector's dimension is a multiple of 8, and a vector is never a key.
    let bits = VectorType { element: VectorElement::Bit, dimension: 12, cosine: false };
    let bad = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("v", ColumnType::Vector(bits))],
        &["id"],
    );
    let error = write.open_table_with("bad", TableMode::CreateNew(&bad)).err().expect("invalid");
    assert!(matches!(error, Error::InvalidSchema { rule: SchemaRule::VectorOutOfLimits, .. }));
    let keyed = Schema::new(vec![Column::new("v", ColumnType::Vector(embedding)).not_null()], &["v"]);
    let error = write.open_table_with("keyed", TableMode::CreateNew(&keyed)).err().expect("invalid");
    assert!(matches!(error, Error::InvalidSchema { rule: SchemaRule::InvalidKeyColumn, .. }));
    Ok(())
}
```

## JSON

`ColumnType::Json` holds `Value::Json(Json)`, a JSON document given as text or bytes and validated and normalized
when it is written; `row.json(position)` reads it back as its normalized text, which you navigate and read as Krummi
types. Like vectors, JSON columns have no order: they cannot be in the primary key or the key of an index, and they
have no statistics. Everything about them is in [JSON columns](json.md).

## Nulls and default values

A column allows nulls unless it is declared with `.not_null()`. A column can also have a default value, a constant of
its type given with `.with_default(value)`.

- **Insert**: a column that the row does not give takes its default value if it has one, and null otherwise.
- **Update** changes only the columns it gives; the others keep their value.
- An explicit `Value::Null` stores null, never the default value.
- A null, or no value, in a `not_null` column fails with `ValueRule::NullNotAllowed`.
- **Reading**: a null reads as `Value::Null` with `get` and `value`, and as `None` with a typed accessor.
- **Primary key**: its columns must be `not_null` and have no default; otherwise creating the table fails with
  `SchemaRule::InvalidKeyColumn`.
- **Defaults**: a default that breaks its column's rules fails with `SchemaRule::InvalidDefault`. A default is stored
  normalized (a `decimal` at the column's scale, a `-0` as `+0`), and `describe_table` shows it so. A null default is
  the same as no default, except in a JSON column, where the document `null` is a default
  ([JSON columns](json.md)). Defaults are constants: a generator such as `ulid()` cannot be one.
- A `not_null` column added to a table that exists needs a default value ([tables](tables.md)).

```rust
use krummi::table::value::{ColumnType, Decimal, Value};
use krummi::{Column, Error, Schema, SchemaRule, Store, StoreOptions, TableMode, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("note", ColumnType::String),
            Column::new("qty", ColumnType::I32).not_null().with_default(Value::I32(1)),
            Column::new("price", ColumnType::Decimal { precision: 6, scale: 2 })
                .with_default(Value::Decimal(Decimal { mantissa: 5, scale: 0 })),
        ],
        &["id"],
    );
    let t = write.open_table_with("lines", TableMode::CreateNew(&schema))?;

    // Missing columns take their default, or null.
    write.insert_row(&t, &[("id", Value::I64(1))])?;
    {
        let row = write.get_row(&t, &[Value::I64(1)])?.expect("inserted");
        assert_eq!(row.get("note"), Some(Value::Null));
        assert_eq!(row.text(1), None);
        assert_eq!(row.i32(2), Some(1));
        assert_eq!(row.get("price"), Some(Value::Decimal(Decimal { mantissa: 500, scale: 2 })));
    }

    // The default is stored normalized: 5 at the column's scale.
    let described = write.describe_table("lines")?;
    let price = described.column("price").expect("a column");
    assert_eq!(price.default, Some(Value::Decimal(Decimal { mantissa: 500, scale: 2 })));
    assert_eq!(price.to_string(), "\"price\" decimal(6, 2) default 5.00");

    // An explicit null is a null, never the default.
    let error = write.insert_row(&t, &[("id", Value::I64(2)), ("qty", Value::Null)]).unwrap_err();
    assert!(matches!(error, Error::InvalidValue { rule: ValueRule::NullNotAllowed, .. }));

    // A key column that allows nulls, and a default of another type, are invalid schemas.
    let nullable_key = Schema::new(vec![Column::new("id", ColumnType::I64)], &["id"]);
    let error = write.open_table_with("a", TableMode::CreateNew(&nullable_key)).err().expect("invalid");
    assert!(matches!(error, Error::InvalidSchema { rule: SchemaRule::InvalidKeyColumn, .. }));
    let wrong_default = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I32).with_default(Value::I64(1)),
        ],
        &["id"],
    );
    let error = write.open_table_with("b", TableMode::CreateNew(&wrong_default)).err().expect("invalid");
    assert!(matches!(error, Error::InvalidSchema { rule: SchemaRule::InvalidDefault, .. }));
    Ok(())
}
```

## Conversions

Krummi converts no value from one type to another. The value of a column is always its type's `Value` variant, and
any other variant fails with `ValueRule::WrongType`. What it does do, when it writes:

| Done when writing | Example |
|---|---|
| `-0` as `+0` and every NaN as one NaN, in `f32` and `f64` | `Value::F64(-0.0)` reads back `0.0` |
| `-0` as `+0` inside vectors, which never hold a NaN or an infinity (`VectorNotFinite`) | an `f32` element `-0.0` reads back `0.0` |
| a `decimal` at the column's scale, ties away from zero | 1.235 in `decimal(10, 2)` reads back 1.24 |
| a `time`, `timestamp` or `duration` to the microsecond, ties up ([why](#nanoseconds-in-your-code-microseconds-in-the-store)) | 500 ns reads back 1,000 ns |
| a JSON document to its normalized text | `{ "a" : 1 }` reads back `{"a":1}` ([JSON columns](json.md)) |

What it refuses:

| Refused | Error | Do instead |
|---|---|---|
| another integer width or sign (`I32` in an `i64` column) | `ValueRule::WrongType` | convert in Rust (`i64::from(x)`) |
| a text in a `uuid`, `ulid`, `date` or number column | `ValueRule::WrongType` | parse it (`"…".parse::<Uuid>()`) |
| a `String` or `Bytes` in a JSON column, a `Json` in a `string` column | `ValueRule::WrongType` | `Value::Json(Json::new(text))` |
| changing a column's type (`modify_column` with another type) | `SchemaRule::UnsupportedChange` | add a new column and copy the values |

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, Schema, SchemaRule, Store, StoreOptions, TableMode, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("count", ColumnType::I32)],
        &["id"],
    );
    let t = write.open_table_with("counts", TableMode::CreateNew(&schema))?;

    let small: i32 = 7;
    let error = write.insert_row(&t, &[("id", Value::I32(small))]).unwrap_err();
    assert!(matches!(error, Error::InvalidValue { rule: ValueRule::WrongType, .. }));
    write.insert_row(&t, &[("id", Value::I64(i64::from(small)))])?;

    let error = write.modify_column(&t, "count", ColumnType::I64, true).unwrap_err();
    assert!(matches!(error, Error::InvalidSchema { rule: SchemaRule::UnsupportedChange, .. }));
    Ok(())
}
```

## Errors

An invalid value fails with `Error::InvalidValue` (`ErrorKind::InvalidValue`), before the operation changes anything.
Its `rule` is one of these `ValueRule`s; when a row breaks several, the first in this order is reported, for the first
column in schema order that breaks one (a repeated or missing key column comes before the others).

| `ValueRule` | When |
|---|---|
| `RepeatedColumn` | a row gives a column twice |
| `MissingKeyColumn` | a row lacks a primary key column |
| `NullNotAllowed` | a null, or no value, in a `not_null` column |
| `WrongType` | a value of another type than its column's |
| `MalformedText` | a `uuid` or `ulid` text not in its one form |
| `InvalidJson` | a JSON text that is not a valid document ([JSON columns](json.md)) |
| `OutOfDomain` | a `date`, `time`, `timestamp` or `duration` outside its domain, also after rounding |
| `DecimalPrecision` | a `decimal` with more digits than its precision, at its scale |
| `VectorDimension` | a vector of another length than its column's |
| `VectorNotFinite` | a NaN or an infinity in a vector |
| `ZeroVectorCosine` | a zero vector in a cosine column |

A type's limits in a schema fail with `Error::InvalidSchema` (`SchemaRule::DecimalOutOfLimits`,
`VectorOutOfLimits`, `CosineOnBit`, `InvalidKeyColumn`, `InvalidDefault`); a row over 256 MiB or a key over
2,048 bytes, with `Error::InvalidSize`. See [errors](errors.md) for every class and its data.

See also: [tables](tables.md), [rows](rows.md), [JSON columns](json.md), [sequences and IDs](sequences-and-ids.md),
[indexes](indexes.md), [statistics](statistics.md), [errors](errors.md).
