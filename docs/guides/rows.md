# Rows

This guide covers the rows of a [table](tables.md): inserting, updating, inserting or updating, reading and deleting
them by primary key, reading their values, scanning and deleting ranges of the primary key, loading many rows at once,
and reading columns faster with projections. Use it once you have a table; the values of each column type are in
[Data types](data-types.md) and JSON documents in [JSON](json.md).

## Quick path

1. Open the table in a write transaction.
2. Write rows as slices of `(column name, Value)` pairs with `insert_row`, `update_row` or `upsert_row`.
3. Read a row with `get_row(&table, &[key values])`, then its columns with `get(name)` or a typed accessor.
4. Scan rows in key order with `range_rows(&table, &prefix, bounds)`.
5. Commit.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode, Upserted};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String),
            Column::new("stock", ColumnType::I32).not_null().with_default(Value::I32(0)),
        ],
        &["id"],
    );
    let mut write = store.write()?;
    let items = write.open_table_with("items", TableMode::CreateNew(&schema))?;
    write.insert_row(&items, &[("id", Value::I64(1)), ("name", Value::String("pen".into()))])?;
    assert!(write.update_row(&items, &[("id", Value::I64(1)), ("stock", Value::I32(5))])?);
    let done = write.upsert_row(&items, &[("id", Value::I64(2)), ("name", Value::String("ink".into()))])?;
    assert_eq!(done, Upserted::Inserted);

    let row = write.get_row(&items, &[Value::I64(1)])?.expect("inserted");
    assert_eq!(row.text(1).as_deref(), Some("pen"));
    assert_eq!(row.i32(2), Some(5));
    drop(row);

    let names: Vec<String> = write
        .range_rows(&items, &[], ..)?
        .map(|row| row.map(|row| row.text(1).unwrap().into_owned()))
        .collect::<krummi::Result<_>>()?;
    assert_eq!(names, ["pen", "ink"]);
    write.commit()?;
    Ok(())
}
```

## Writing rows

Every write receives the row as `&[(&str, Value)]` pairs, with every primary key column. The order of the pairs does
not matter.

| Operation | Returns | Behavior |
|---|---|---|
| `insert_row(&table, row)` | `()` | Adds the row. A column the row does not give takes its default or, without one, null. Fails with `ErrorKind::DuplicateKey` if the key exists, even when the row is identical. |
| `update_row(&table, row)` | `bool` | Changes only the columns the row gives; the others keep their values. Returns `false`, changing nothing, when no row has that key. |
| `upsert_row(&table, row)` | `Upserted` | Updates the row if it is there (`Upserted::Updated`), otherwise inserts it (`Upserted::Inserted`), with the same rules. |

More rules:

- An explicit `Value::Null` stores null, never the default.
- An update that leaves the row as it reads is identical: it returns `true` and writes nothing.
- A primary key never changes. To change it, delete the row and insert it with the new key in the same write.
- Rows are written in the current schema version; see [Tables and schemas](tables.md#changing-a-schema).

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, ErrorKind, Schema, Store, StoreOptions, TableMode, Upserted};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("city", ColumnType::String).with_default(Value::String("Oslo".into())),
            Column::new("visits", ColumnType::U32),
        ],
        &["id"],
    );
    let mut write = store.write()?;
    let t = write.open_table_with("t", TableMode::CreateNew(&schema))?;

    // Missing columns: the default, or null.
    write.insert_row(&t, &[("id", Value::I64(1))])?;
    // An explicit null stores null, never the default.
    write.insert_row(&t, &[("id", Value::I64(2)), ("city", Value::Null)])?;
    let row = write.get_row(&t, &[Value::I64(1)])?.unwrap();
    assert_eq!((row.get("city"), row.get("visits")), (Some(Value::String("Oslo".into())), Some(Value::Null)));
    drop(row);
    assert_eq!(write.get_row(&t, &[Value::I64(2)])?.unwrap().get("city"), Some(Value::Null));

    // The same key again fails, and changes nothing.
    let error = write.insert_row(&t, &[("id", Value::I64(1))]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::DuplicateKey);

    // An update changes only what it gives; a missing row is reported, not created.
    assert!(write.update_row(&t, &[("id", Value::I64(1)), ("visits", Value::U32(3))])?);
    assert!(!write.update_row(&t, &[("id", Value::I64(9)), ("visits", Value::U32(3))])?);
    assert_eq!(write.row_count(&t)?, 2);

    // Insert or update.
    assert_eq!(write.upsert_row(&t, &[("id", Value::I64(1)), ("visits", Value::U32(4))])?, Upserted::Updated);
    assert_eq!(write.upsert_row(&t, &[("id", Value::I64(3))])?, Upserted::Inserted);

    // Changing a key: delete and insert in the same write.
    assert!(write.delete_row(&t, &[Value::I64(3)])?);
    write.insert_row(&t, &[("id", Value::I64(4))])?;
    write.commit()?;
    Ok(())
}
```

### Errors of a write

A row that breaks a rule fails without changing the table and without preventing the commit (see
[Errors](errors.md)). `Error::InvalidValue` names the table, the column and a `ValueRule`; when a row breaks several
rules, one is reported: first a repeated column, then a missing key column (the first in key order), then the first
column in schema order that breaks any other rule.

| Problem | Error |
|---|---|
| A column given twice | `InvalidValue`, `ValueRule::RepeatedColumn` |
| A primary key column missing | `InvalidValue`, `ValueRule::MissingKeyColumn` |
| A null, or no value, in a column without nulls | `InvalidValue`, `ValueRule::NullNotAllowed` |
| A value of another type than its column's | `InvalidValue`, `ValueRule::WrongType` |
| A value outside its type's domain (see [Data types](data-types.md)) | `InvalidValue`, `ValueRule::OutOfDomain` and the other value rules |
| A column the table does not have | `DifferentSchema`, `SchemaDifference::MissingColumn` |
| A column name outside 1 to 1,024 bytes | `InvalidSize`, `SizeSubject::ColumnName` |
| A primary key over 2,048 bytes in its stored form | `InvalidSize`, `SizeSubject::PrimaryKey` |
| A row over 256 MiB in its stored form | `InvalidSize`, `SizeSubject::Row` |
| A key that exists (`insert_row`) | `DuplicateKey`, with the table and the key as text |

The size of the row that an update would give is checked after reading the stored row. An `upsert_row` that inserts
fails with `NullNotAllowed` when it leaves a column without nulls nor a default empty; when it updates, that column
keeps its stored value.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, ErrorKind, Schema, SchemaDifference, SizeSubject, Store, StoreOptions, TableMode, ValueRule};

fn value_rule(error: Error) -> ValueRule {
    match error {
        Error::InvalidValue { rule, .. } => rule,
        other => panic!("not an invalid value: {other}"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![
            Column::new("code", ColumnType::String).not_null(),
            Column::new("qty", ColumnType::I32).not_null(),
        ],
        &["code"],
    );
    let mut write = store.write()?;
    let t = write.open_table_with("t", TableMode::CreateNew(&schema))?;
    let code = || ("code", Value::String("a".into()));

    let error = write.insert_row(&t, &[code(), ("qty", Value::I32(1)), ("qty", Value::I32(2))]).unwrap_err();
    assert_eq!(value_rule(error), ValueRule::RepeatedColumn);
    let error = write.insert_row(&t, &[("qty", Value::I32(1))]).unwrap_err();
    assert_eq!(value_rule(error), ValueRule::MissingKeyColumn);
    let error = write.insert_row(&t, &[code()]).unwrap_err();
    assert_eq!(value_rule(error), ValueRule::NullNotAllowed);
    let error = write.insert_row(&t, &[code(), ("qty", Value::I64(1))]).unwrap_err();
    assert_eq!(value_rule(error), ValueRule::WrongType);
    let error = write.insert_row(&t, &[code(), ("qty", Value::I32(1)), ("color", Value::Null)]).unwrap_err();
    assert!(matches!(error, Error::DifferentSchema { difference: SchemaDifference::MissingColumn, .. }));
    let long = Value::String("k".repeat(3_000).into());
    let error = write.insert_row(&t, &[("code", long), ("qty", Value::I32(1))]).unwrap_err();
    assert!(matches!(error, Error::InvalidSize { what: SizeSubject::PrimaryKey, max: 2_048, .. }));

    // An insert or update that would insert needs every column without nulls nor a default.
    let error = write.upsert_row(&t, &[code()]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidValue);
    write.insert_row(&t, &[code(), ("qty", Value::I32(1))])?;
    write.upsert_row(&t, &[code()])?; // it updates: "qty" keeps 1

    assert_eq!(write.row_count(&t)?, 1);
    write.commit()?; // none of the failures prevented it
    Ok(())
}
```

## Reading and deleting by key

`get_row(&table, &key)` returns `Some(TableRow)` or `None` when no row has that key. `delete_row(&table, &key)`
removes the row and returns whether it was there. The key is a slice with one value per primary key column, in key
order (the order of the schema's primary key list, not the schema order).

| Key | Result |
|---|---|
| Every key column, valid values | The row, or `None` / `false` |
| Fewer or more values than the key, or a null | `ErrorKind::InvalidValue` |
| A value that breaks its column's rules | `ErrorKind::InvalidValue` |
| More than 2,048 bytes in its stored form | `ErrorKind::InvalidSize` |

A read transaction has `get_row` too; it sees its snapshot. A write transaction sees its own changes.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, ErrorKind, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![
            Column::new("user", ColumnType::I32).not_null(),
            Column::new("day", ColumnType::Date).not_null(),
            Column::new("steps", ColumnType::U32),
        ],
        &["user", "day"],
    );
    let mut write = store.write()?;
    let t = write.open_table_with("steps", TableMode::CreateNew(&schema))?;
    write.insert_row(&t, &[("day", Value::Date(100)), ("user", Value::I32(7)), ("steps", Value::U32(9_000))])?;
    write.commit()?;

    let read = store.read()?;
    let t = read.open_table("steps")?;
    let key = [Value::I32(7), Value::Date(100)]; // key order: user, then day
    assert_eq!(read.get_row(&t, &key)?.unwrap().u32(2), Some(9_000));
    assert!(read.get_row(&t, &[Value::I32(7), Value::Date(101)])?.is_none());
    let error = read.get_row(&t, &[Value::I32(7)]).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::InvalidValue);
    drop(read);

    let mut write = store.write()?;
    let t = write.open_table("steps")?;
    assert!(write.delete_row(&t, &key)?);
    assert!(!write.delete_row(&t, &key)?);
    write.commit()?;
    Ok(())
}
```

## Reading a row's values

A `TableRow` has every live column in schema order, each by name or by position. A row written before a column was
added reads that column's default or null; a dropped column never appears.

| Method | Returns |
|---|---|
| `get(name)` | `Option<Value>`: `None` for a name that is not a live column, `Some(Value::Null)` for a null. |
| `value(position)` | The same by position; `None` past the last column. |
| `name(position)` | The column's name. |
| `len()` | The number of live columns. |
| `values()` | The values in schema order. |
| `columns()` | `(name, value)` pairs in schema order. |
| `into_owned()` | A `TableRow<'static>` with copies of its values. |

The typed accessors read a column by position as its Rust type, without building a `Value`, which is cheaper in a
scan. Each returns `None` for a null or a position past the last column, and panics when the column is of another
type (a fault of the program); `value` reads any column.

| Accessor | Column type | Returns |
|---|---|---|
| `bool`, `i8` … `i64`, `u8` … `u64`, `f32`, `f64` | the same | the number |
| `date` | `date` | `i32`, days since 1970-01-01 |
| `time` | `time` | `i64`, microseconds since midnight |
| `timestamp` | `timestamp` | `i64`, microseconds since the Unix epoch, UTC |
| `duration` | `duration` | `i64`, microseconds |
| `uuid`, `ulid` | `uuid`, `ulid` | `krummi::ids::Uuid`, `krummi::ids::Ulid` |
| `text` | `string` | `Cow<str>` |
| `text_bytes` | `string` | `Cow<[u8]>`, the UTF-8 without checking it |
| `bytes` | `bytes` | `Cow<[u8]>` |
| `json` | `json` | `krummi::table::json::Json` (see [JSON](json.md)) |

The three temporal accessors return microseconds, the unit Krummi stores and the one you give: see
[Dates, times, instants and durations](data-types.md#dates-times-instants-and-durations).

A `decimal` or vector column is read with `value`/`get`, or with a [projection](#projections).

Reads do not check a row's content: a row that is malformed in a page whose checksum is valid can read a value other
than the written one, or panic.

```rust
use krummi::table::json::Json;
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String),
            Column::new("photo", ColumnType::Bytes),
            Column::new("meta", ColumnType::Json),
        ],
        &["id"],
    );
    let mut write = store.write()?;
    let t = write.open_table_with("t", TableMode::CreateNew(&schema))?;
    write.insert_row(
        &t,
        &[
            ("id", Value::I64(1)),
            ("name", Value::String("Muninn".into())),
            ("meta", Value::Json(Json::new(r#"{"tags":["raven"]}"#))),
        ],
    )?;

    let row = write.get_row(&t, &[Value::I64(1)])?.expect("inserted");
    assert_eq!(row.len(), 4);
    assert_eq!(row.name(1), Some("name"));
    assert_eq!(row.get("name"), Some(Value::String("Muninn".into())));
    assert_eq!(row.value(2), Some(Value::Null));
    assert_eq!(row.get("nope"), None);
    assert_eq!(row.i64(0), Some(1));
    assert_eq!(row.text(1).as_deref(), Some("Muninn"));
    assert_eq!(row.bytes(2), None); // null
    assert_eq!(row.json(3).unwrap().text(), Some(r#"{"tags":["raven"]}"#));
    let names: Vec<&str> = row.columns().map(|(name, _)| name).collect();
    assert_eq!(names, ["id", "name", "photo", "meta"]);
    drop(row);
    write.commit()?;
    Ok(())
}
```

### Borrowed values and their lifetime

Text, bytes, vectors and JSON values of a row borrow the transaction's pages, without a copy. So a row, and every
value read from it, lives until the next operation that may modify its transaction: any write, schema change or
catalog change that takes `&mut self` (also when it fails or changes nothing). The compiler enforces it. In a read
transaction, rows live as long as the transaction. To keep a row across a write, copy it with `into_owned()`, or copy
the values you need with `Value::into_owned`.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("name", ColumnType::String)],
        &["id"],
    );
    let mut write = store.write()?;
    let t = write.open_table_with("t", TableMode::CreateNew(&schema))?;
    write.insert_row(&t, &[("id", Value::I64(1)), ("name", Value::String("a".into()))])?;

    let borrowed = write.get_row(&t, &[Value::I64(1)])?.expect("inserted");
    let owned = borrowed.into_owned(); // a copy: it outlives the next write
    let name = owned.get("name").unwrap().into_owned();
    write.insert_row(&t, &[("id", Value::I64(2)), ("name", name)])?;
    assert_eq!(owned.text(1).as_deref(), Some("a"));
    write.commit()?;
    Ok(())
}
```

## Scanning ranges of the primary key

`range_rows(&table, prefix, bounds)` scans rows in primary key order. A range is:

- a **prefix**: equal values for the first k key columns, in key order, from none (`&[]`) to all of them;
- **bounds** on the next key column, as any Rust range of `Value`s: `..`, `a..`, `a..b`, `a..=b`, `..b`, `..=b`, or
  `(Bound, Bound)` for an excluded lower bound.

With a prefix of every key column, the range is that one row and the bounds must be `..`. The scan is a
double-ended iterator of `Result<TableRow>`: `.rev()` scans backward, and `next()` and `next_back()` can be mixed to
consume it from both ends. An empty or inverted range gives no rows and no error. `delete_range` takes the same range
and returns how many rows it deleted, freeing their pages.

| Scan | Returns |
|---|---|
| `ReadTransaction::range_rows` | `ReadRows`, which can be sent to another thread. |
| `WriteTransaction::range_rows` | `WriteRows`, with the write's changes; it ends at the next operation that may modify the write. |
| `WriteTransaction::delete_range` | `u64`, the rows deleted. |

| Range | Error |
|---|---|
| A null, or a value that breaks its column's rules | `InvalidValue` with the column |
| A prefix with more values than the key, or bounds after a full prefix | `InvalidValue`, `ValueRule::InvalidRange` |
| A prefix or bound over 2,048 bytes in its stored form, even if the range is empty | `InvalidSize` |

NaN is a valid bound. Bounds apply to one column only; for a condition on several columns, filter the rows you scan.

```rust
use std::ops::Bound;
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, Schema, Store, StoreOptions, TableMode, ValueRule, WriteRows};

/// The "at" column of each row of a scan.
fn at(rows: WriteRows<'_>) -> krummi::Result<Vec<i64>> {
    rows.map(|row| row.map(|row| row.i64(1).unwrap())).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![
            Column::new("tenant", ColumnType::I32).not_null(),
            Column::new("at", ColumnType::I64).not_null(),
        ],
        &["tenant", "at"],
    );
    let mut write = store.write()?;
    let t = write.open_table_with("events", TableMode::CreateNew(&schema))?;
    for (tenant, at) in [(1, 10), (1, 20), (1, 30), (1, 40), (2, 10)] {
        write.insert_row(&t, &[("tenant", Value::I32(tenant)), ("at", Value::I64(at))])?;
    }
    let one = [Value::I32(1)];
    assert_eq!(at(write.range_rows(&t, &one, ..)?)?, [10, 20, 30, 40]);
    assert_eq!(at(write.range_rows(&t, &one, Value::I64(20)..Value::I64(40))?)?, [20, 30]);
    assert_eq!(at(write.range_rows(&t, &one, ..=Value::I64(20))?)?, [10, 20]);
    let after_20 = (Bound::Excluded(Value::I64(20)), Bound::Unbounded);
    assert_eq!(at(write.range_rows(&t, &one, after_20)?)?, [30, 40]);
    assert_eq!(at(write.range_rows(&t, &[], ..)?)?.len(), 5); // the whole table
    assert_eq!(at(write.range_rows(&t, &one, Value::I64(40)..Value::I64(10))?)?, []); // inverted

    // Backward, and from both ends at once.
    let backward: Vec<i64> = at(write.range_rows(&t, &one, ..)?)?.into_iter().rev().collect();
    assert_eq!(backward, [40, 30, 20, 10]);
    let mut rows = write.range_rows(&t, &one, ..)?;
    assert_eq!(rows.next().unwrap()?.i64(1), Some(10));
    assert_eq!(rows.next_back().unwrap()?.i64(1), Some(40));
    assert_eq!(rows.count(), 2);

    // A full key with bounds is a malformed range.
    let full = [Value::I32(1), Value::I64(10)];
    let error = write.range_rows(&t, &full, Value::I64(0)..).err().unwrap();
    assert!(matches!(error, Error::InvalidValue { rule: ValueRule::InvalidRange, .. }));

    assert_eq!(write.delete_range(&t, &one, Value::I64(30)..)?, 2);
    assert_eq!(write.row_count(&t)?, 3);
    write.commit()?;
    Ok(())
}
```

### Lending scans with `next_row`

`next_row()` returns the next row from the front lent until the next call (`Ok(None)` at the end), instead of moving
a new `TableRow` out at each step. A loop that only reads columns costs less this way. After a failure, a scan
returns nothing more from either end.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("amount", ColumnType::I64)],
        &["id"],
    );
    let mut write = store.write()?;
    let t = write.open_table_with("t", TableMode::CreateNew(&schema))?;
    for id in 1..=100 {
        write.insert_row(&t, &[("id", Value::I64(id)), ("amount", Value::I64(id * 2))])?;
    }
    write.commit()?;

    let read = store.read()?;
    let t = read.open_table("t")?;
    let mut rows = read.range_rows(&t, &[], ..)?;
    let mut total = 0;
    while let Some(row) = rows.next_row()? {
        total += row.i64(1).unwrap_or(0);
    }
    assert_eq!(total, 10_100);
    Ok(())
}
```

### `Direction`, `Nulls` and `Condition`

A primary key is always ascending without nulls, so its scans take no direction: scan backward with `.rev()`.
`Direction` and `Nulls` describe the columns of a secondary index ([Secondary indexes](indexes.md)), and `Condition`
describes a condition whose rows a [statistics](statistics.md) estimate counts; neither filters a primary key scan.

## Bulk load

`Store::bulk_load(table, rows)` loads many rows as one write transaction of its own and returns how many it loaded.
It is atomic: either every row is committed, or nothing changes. Each row is any `AsRef<[(&str, Value)]>` wrapped in
a `Result<_, E>`, so the source can fail; the rows follow the rules of `insert_row`. Rows may come in any order, and
primary key order is faster. The table may already have rows and indexes.

| Outcome | Result |
|---|---|
| The source ends | Commits and returns the number of rows; an empty source changes nothing and returns 0. |
| The source yields `Err(e)` | `BulkLoadError::Source(e)`: the load is aborted. This is also how the application cancels a load. |
| A row breaks a rule of insert (a duplicate key included) | `BulkLoadError::Store(error)` with the first such row's error; the load is aborted. |
| The table is missing, or is a tree or a sequence | `BulkLoadError::Store` with `TreeNotFound` or `DifferentTreeType`. |
| The thread already holds a write transaction | `BulkLoadError::Store` with `Misuse`, at once (an open read does not prevent it). |
| A panic in the source or in the load | The load is aborted and the panic continues. |

`bulk_load` waits for the write turn without limit; `bulk_load_with_timeout(table, rows, limit)` waits at most
`limit` and fails with `ErrorKind::Timeout` before looking at the table or the source. After a load aborted before its
commit, the table, its row count and the sequences are as they were, and the write turn passes to the next writer. A
failure of the commit's write to the disk has an unknown outcome, never partial ([Errors](errors.md)).

```rust
use std::convert::Infallible;
use std::num::ParseIntError;
use std::time::Duration;
use krummi::table::value::{ColumnType, Value};
use krummi::{BulkLoadError, Column, ErrorKind, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("square", ColumnType::I64)],
        &["id"],
    );
    let mut write = store.write()?;
    write.open_table_with("t", TableMode::CreateNew(&schema))?;
    write.commit()?;

    let rows = (0..1_000).map(|id| Ok::<_, Infallible>([("id", Value::I64(id)), ("square", Value::I64(id * id))]));
    assert_eq!(store.bulk_load("t", rows)?, 1_000);

    // A source that fails: parsing text, the third line is not a number.
    let lines = ["2000", "2001", "x", "2003"];
    let rows = lines.iter().map(|line| line.parse::<i64>().map(|id| [("id", Value::I64(id))]));
    let error: BulkLoadError<ParseIntError> = store.bulk_load("t", rows).unwrap_err();
    assert!(matches!(error, BulkLoadError::Source(_)));

    // A duplicate key aborts the whole load.
    let rows = [5_000, 5_001, 7].map(|id| Ok::<_, Infallible>([("id", Value::I64(id))]));
    match store.bulk_load_with_timeout("t", rows, Duration::from_secs(5)) {
        Err(BulkLoadError::Store(error)) => assert_eq!(error.kind(), ErrorKind::DuplicateKey),
        other => panic!("{other:?}"),
    }

    let read = store.read()?;
    let t = read.open_table("t")?;
    assert_eq!(read.row_count(&t)?, 1_000); // neither failed load left a row
    Ok(())
}
```

## Projections

A projection names once the columns you will read from a table's rows. Krummi prepares once what depends on the
schema (where each column is and how it is stored), and every row is then read without repeating that work. Use one
in a loop over many rows that reads the same columns; for a single row, `get` and the typed accessors are enough.
The values are the same as reading the column by position.

1. `projection(&table, &["col", ...])` prepares a `Projection` over the schema the handle sees. The list's order
   gives the projection's positions; key columns may be in it.
2. `projection.column::<T>(position)` gives a `ProjectedColumn<T>` for a position, checking its type once.
3. For each row, `projection.bind(&row)` gives a `ProjectedRow`; `projected.get(&column)` reads one column, `None`
   for a null.

| Read as `T` (`krummi::table::projection::...`) | Column type | `get` returns |
|---|---|---|
| `bool`, `i8` … `u64`, `f32`, `f64` | the same | the number |
| `Decimal` (`krummi::table::value::Decimal`) | `decimal(p, s)` of any precision and scale | `Decimal` |
| `Text`, `TextBytes` | `string` | `Cow<str>`, or its unchecked bytes `Cow<[u8]>` |
| `Bytes` | `bytes` | `Cow<[u8]>` |
| `Date`, `Time`, `Timestamp`, `Duration` | the same | `i32` days, or `i64` microseconds, as the typed accessors |
| `Uuid`, `Ulid` (`krummi::ids::...`) | `uuid` of either order, `ulid` | the value |
| `Vector` | any vector | `Cow<[u8]>`, its elements as `Value::Vector` holds them |
| `JsonDocument` | `json` | `Json` |

Rules and errors:

| Case | Result |
|---|---|
| A name outside 1 to 1,024 bytes | `InvalidSize` |
| A name that is not a live column of that schema | `DifferentSchema`, `MissingColumn` |
| A repeated column, or an empty list | `InvalidValue` (`ValueRule::EmptyProjection` for the empty list) |
| `column::<T>` with a type that is not the column's | `DifferentSchema`, with the type asked and the column's |
| `column` with a position past the list | Panics: a fault of the program. |
| `bind` of a row of another table, store or schema of the table | `DifferentSchema`, `SchemaDifference::Projection` |
| `get` with a column of another projection or list order | Panics. |

A projection is tied to the schema it was prepared with. After a schema change, prepare a new one: rows of the new
schema do not bind to the old projection. Preparing reads no page and does not end borrowed rows. The store keeps the
first 1,024 projections it prepares (`krummi::table::projection::MAX_PROJECTIONS`) and returns the kept one when you
prepare the same list over the same schema again, from any transaction or thread; later ones work but are not kept.
A projection cannot outlive its store.

```rust
use krummi::table::projection::Text;
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, ErrorKind, Schema, SchemaDifference, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String),
            Column::new("score", ColumnType::F64),
        ],
        &["id"],
    );
    let mut write = store.write()?;
    let t = write.open_table_with("players", TableMode::CreateNew(&schema))?;
    for (id, name, score) in [(1, "ada", 9.5), (2, "bob", 7.0), (3, "cy", 8.25)] {
        write.insert_row(
            &t,
            &[("id", Value::I64(id)), ("name", Value::String(name.into())), ("score", Value::F64(score))],
        )?;
    }
    write.commit()?;

    let read = store.read()?;
    let t = read.open_table("players")?;
    let projection = read.projection(&t, &["score", "name"])?;
    let score = projection.column::<f64>(0)?;
    let name = projection.column::<Text>(1)?;
    let mut best = (0.0, String::new());
    let mut rows = read.range_rows(&t, &[], ..)?;
    while let Some(row) = rows.next_row()? {
        let row = projection.bind(row)?;
        if let (Some(s), Some(n)) = (row.get(&score), row.get(&name))
            && s > best.0
        {
            best = (s, n.into_owned());
        }
    }
    assert_eq!(best, (9.5, "ada".to_string()));

    // The type is checked once, when the column is prepared.
    let error = projection.column::<i64>(0).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::DifferentSchema);
    assert_eq!(read.projection(&t, &[]).err().unwrap().kind(), ErrorKind::InvalidValue);
    drop(rows);
    drop(read);

    // After a schema change, rows of the new schema need a new projection.
    let mut write = store.write()?;
    let t = write.open_table("players")?;
    let before = write.projection(&t, &["name"])?;
    write.add_column(&t, &Column::new("team", ColumnType::String))?;
    let row = write.get_row(&t, &[Value::I64(2)])?.expect("inserted");
    let error = before.bind(&row).err().unwrap();
    assert!(matches!(error, Error::DifferentSchema { difference: SchemaDifference::Projection, .. }));
    let after = write.projection(&t, &["name"])?;
    let name = after.column::<Text>(0)?;
    assert_eq!(after.bind(&row)?.get(&name).as_deref(), Some("bob"));
    drop(row);
    write.commit()?;
    Ok(())
}
```

See also: [Tables and schemas](tables.md), [Data types](data-types.md), [JSON](json.md),
[Secondary indexes](indexes.md), [Statistics](statistics.md), [Sequences and identifiers](sequences-and-ids.md),
[Transactions](transactions.md), [Errors](errors.md).
