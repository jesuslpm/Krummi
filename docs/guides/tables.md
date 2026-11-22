# Tables and schemas

A table is a named object of the store's catalog with a schema: typed columns and a primary key. This guide covers
how to declare a schema, how to create, open, describe, rename, empty and delete tables, and how to change a schema
later. Use a table when your data has columns; use a [tree](trees.md) when you only need keys and values of bytes.
Reading and writing rows is in [Rows](rows.md); the column types and their values are in [Data types](data-types.md).

## Quick path

1. Declare the columns with `Column::new(name, type)`, adding `.not_null()` or `.with_default(value)` as needed.
2. Build a `Schema` from the columns and the names of the primary key columns.
3. In a write transaction, open the table with `open_table_with(name, TableMode::OpenOrCreate(&schema))`.
4. Insert rows and commit.
5. Later, open it with `open_table(name)` in any transaction and read its schema with `describe_table`.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;

    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String),
            Column::new("active", ColumnType::Bool).not_null().with_default(Value::Bool(true)),
        ],
        &["id"],
    );
    let mut write = store.write()?;
    let people = write.open_table_with("people", TableMode::OpenOrCreate(&schema))?;
    write.insert_row(&people, &[("id", Value::I64(1)), ("name", Value::String("Huginn".into()))])?;
    write.commit()?;

    let read = store.read()?;
    let people = read.open_table("people")?;
    assert_eq!(read.row_count(&people)?, 1);
    let described = read.describe_table("people")?;
    assert_eq!(described.primary_key(), ["id"]);
    assert_eq!(described.version(), 1);
    Ok(())
}
```

## Columns

`Column` is how the application declares a column, to create a table or to add a column. Its fields are public, and
three builders cover the usual cases.

| Builder | Result |
|---|---|
| `Column::new(name, ty)` | A column that allows nulls, without a default value. |
| `.not_null()` | The same column without nulls. |
| `.with_default(value)` | The same column with a default value: a constant of its type, which an insert uses when the row does not give the column. |

A default is a constant, never an expression: to fill a column with a generated value, call a
[generator or a sequence](sequences-and-ids.md) and put the value in the row. A `Value::Null` default is the same as
no default.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I32).not_null(),
            Column::new("note", ColumnType::String),                       // nullable, no default
            Column::new("score", ColumnType::F64).with_default(Value::F64(0.5)), // nullable, with default
            Column::new("tag", ColumnType::String).with_default(Value::Null), // a null default is no default
        ],
        &["id"],
    );
    let mut write = store.write()?;
    let table = write.open_table_with("t", TableMode::CreateNew(&schema))?;
    write.insert_row(&table, &[("id", Value::I32(1))])?;
    let row = write.get_row(&table, &[Value::I32(1)])?.expect("inserted");
    assert_eq!(row.get("note"), Some(Value::Null));
    assert_eq!(row.get("score"), Some(Value::F64(0.5)));
    assert_eq!(row.get("tag"), Some(Value::Null));
    drop(row);
    let tag = write.table_schema(&table)?;
    assert_eq!(tag.column("tag").unwrap().default, None);
    write.commit()?;
    Ok(())
}
```

Rules of the columns of a schema:

| Rule | Limit or behavior |
|---|---|
| Number of columns | 1 to 1,024 live columns (dropped columns do not count). |
| Column name | UTF-8, 1 to 1,024 bytes, compared by its bytes, unique in the table. |
| Schema order | The order of the columns when the table was created; each added column goes at the end. |
| Default value | A constant that follows the rules of its column; it is normalized as a stored value is. |
| Whole schema | Its stored form, with names, defaults and index definitions, fits in 256 MiB. |

A name outside 1 to 1,024 bytes fails with `ErrorKind::InvalidSize`. Every other broken rule fails with
`Error::InvalidSchema`, whose `rule` field (a `SchemaRule`) says which rule and whose `column` field names the column.

## The primary key

The primary key is one or more distinct columns, given by name in `Schema::new(columns, &[...])`. Its order is the
order of that list, not the schema order: rows are sorted by the first key column, then by the second, and so on,
each in its type's order and always ascending. Key columns must:

- not allow nulls (`.not_null()`);
- have no default value;
- be of a type that can be ordered: any type except vectors and JSON.

| Schema | Error (`SchemaRule`) |
|---|---|
| An empty primary key | `NoPrimaryKey` |
| A key column that allows nulls, is a vector or JSON, or has a default | `InvalidKeyColumn` |
| A key that names a missing column or repeats one | `InvalidKeyReference` |
| Two columns with the same name | `DuplicateColumnName` |
| More than 1,024 columns | `TooManyColumns` |
| A `decimal` or vector outside its limits, cosine on `bit` | `DecimalOutOfLimits`, `VectorOutOfLimits`, `CosineOnBit` |
| A default that breaks its column's rules | `InvalidDefault` |
| A stored schema past 256 MiB | `SchemaTooLarge` |

When a schema breaks several rules, the error reports the first one in the order of this table and, within it, the
first column in schema order. The encoded primary key of a row is at most 2,048 bytes (see [Rows](rows.md)).

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, Schema, SchemaRule, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;

    // A composite key: "day" first, then "seq", whatever the schema order.
    let schema = Schema::new(
        vec![
            Column::new("seq", ColumnType::I32).not_null(),
            Column::new("day", ColumnType::Date).not_null(),
            Column::new("text", ColumnType::String),
        ],
        &["day", "seq"],
    );
    let log = write.open_table_with("log", TableMode::CreateNew(&schema))?;
    for (day, seq) in [(2, 1), (1, 2), (1, 1)] {
        write.insert_row(&log, &[("day", Value::Date(day)), ("seq", Value::I32(seq))])?;
    }
    let keys: Vec<(i32, i32)> = write
        .range_rows(&log, &[], ..)?
        .map(|row| row.map(|row| (row.date(1).unwrap(), row.i32(0).unwrap())))
        .collect::<krummi::Result<_>>()?;
    assert_eq!(keys, [(1, 1), (1, 2), (2, 1)]);

    // A nullable key column is rejected, and nothing is created.
    let bad = Schema::new(vec![Column::new("id", ColumnType::I64)], &["id"]);
    let error = write.open_table_with("bad", TableMode::CreateNew(&bad)).err().unwrap();
    assert!(matches!(
        error,
        Error::InvalidSchema { rule: SchemaRule::InvalidKeyColumn, column: Some(ref c), .. } if c == "id"
    ));
    assert!(!write.tree_exists("bad")?);
    write.commit()?;
    Ok(())
}
```

To change a row's primary key, delete the row and insert it with the new key in the same write.

## Opening a table

`open_table(name)` only opens: it never compares a schema, and it exists in read and write transactions.
`open_table_with(name, mode)` is only in a write transaction and takes a `TableMode`, like .NET's `FileMode`.

| Mode | Table missing | Table exists |
|---|---|---|
| `TableMode::CreateNew(&schema)` | Creates it empty. | Fails with `ErrorKind::TreeAlreadyExists`. |
| `TableMode::OpenOrCreate(&schema)` | Creates it empty. | Opens it if the schema is compatible. |
| `TableMode::CreateOrEmpty(&schema)` | Creates it empty. | Removes all its rows if the schema is compatible. |
| `TableMode::Empty` | Fails with `ErrorKind::TreeNotFound`. | Removes all its rows. |
| `open_table(name)` | Fails with `ErrorKind::TreeNotFound`. | Opens it. |

The modes with a schema validate it first (the rules above), whether or not the table exists. A given schema is
**compatible** with an existing table when each column it names exists with the same type, nullability and default,
and its primary key has the same columns in the same order. The table may have more columns than the schema names,
for example columns that a newer version of the application added; they stay and are read with the others. An
incompatible schema fails with `Error::DifferentSchema`, whose `difference` field (a `SchemaDifference`) is `Type`,
`Nullability`, `Default`, `MissingColumn` or `PrimaryKey`, with `expected` and `found` as text.

Emptying a table, with `CreateOrEmpty` or `Empty`, frees the pages of its rows and of its indexes' entries. It keeps
its schema, its index definitions, its identity and its open handles, which see it empty.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, ErrorKind, Schema, SchemaDifference, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let full = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String),
            Column::new("email", ColumnType::String),
        ],
        &["id"],
    );
    let mut write = store.write()?;
    let users = write.open_table_with("users", TableMode::CreateNew(&full))?;
    write.insert_row(&users, &[("id", Value::I64(1))])?;

    // CreateNew on an existing table fails.
    let error = write.open_table_with("users", TableMode::CreateNew(&full)).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::TreeAlreadyExists);

    // A schema that names only some of the columns is compatible.
    let older = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("name", ColumnType::String)],
        &["id"],
    );
    let same = write.open_table_with("users", TableMode::OpenOrCreate(&older))?;
    assert_eq!(write.row_count(&same)?, 1);

    // A column with another type is not.
    let other = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("name", ColumnType::Bytes)],
        &["id"],
    );
    let error = write.open_table_with("users", TableMode::OpenOrCreate(&other)).err().unwrap();
    assert!(matches!(
        error,
        Error::DifferentSchema { difference: SchemaDifference::Type, column: Some(ref c), .. } if c == "name"
    ));

    // CreateOrEmpty and Empty remove the rows; every handle sees the table empty.
    write.open_table_with("users", TableMode::CreateOrEmpty(&older))?;
    assert_eq!(write.row_count(&users)?, 0);
    write.insert_row(&users, &[("id", Value::I64(2))])?;
    write.open_table_with("users", TableMode::Empty)?;
    assert_eq!(write.row_count(&same)?, 0);

    // Empty and open_table never create a table.
    assert_eq!(write.open_table_with("nope", TableMode::Empty).err().unwrap().kind(), ErrorKind::TreeNotFound);
    assert_eq!(write.open_table("nope").err().unwrap().kind(), ErrorKind::TreeNotFound);
    write.commit()?;

    // A read transaction opens with open_table only.
    let read = store.read()?;
    let users = read.open_table("users")?;
    assert_eq!(read.row_count(&users)?, 0);
    Ok(())
}
```

Handles follow the rules of tree handles (see [Transactions](transactions.md)):

- A `ReadTable` sees its transaction's snapshot and can be sent to and shared between threads.
- A `WriteTable` stays on its thread. Every handle of the same table in one write sees the same rows and schema.
- A handle used in another transaction fails with `ErrorKind::Misuse`, without effect.
- `row_count(&table)` reads the count from the catalog, without reading the table.

## Describing a table

`describe_table(name)` returns the table's current schema as a `TableSchema`, an owned copy; a write transaction sees
its own changes. `table_schema(&table)` does the same from a handle. Both read only the catalog.

| `TableSchema` method | Returns |
|---|---|
| `columns()` | The live columns in schema order, as `SchemaColumn`s. |
| `column(name)` | The live column `name`, or `None`. |
| `primary_key()` | The names of the primary key columns, in key order. |
| `version()` | 1 at creation, one more for each schema change that changes something. |
| `table_id()` | The table's identity: a random 64-bit value. |
| `schema_state()` | A random 64-bit value drawn again at each new version. |
| `indexes()`, `index(name)`, `index_state()` | The secondary indexes (see [Secondary indexes](indexes.md)). |

A `SchemaColumn` has the public fields `id` (its identity, kept when it is renamed), `name`, `ty`, `nullable` and
`default`. Its `Display` form is the name in quotes, the type, `not null` and the default.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("qty", ColumnType::I32).not_null().with_default(Value::I32(1)),
        ],
        &["id"],
    );
    let mut write = store.write()?;
    let orders = write.open_table_with("orders", TableMode::CreateNew(&schema))?;
    write.commit()?;

    let read = store.read()?;
    let described = read.describe_table("orders")?;
    let names: Vec<&str> = described.columns().iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["id", "qty"]);
    let qty = described.column("qty").unwrap();
    assert_eq!(qty.ty, ColumnType::I32);
    assert!(!qty.nullable);
    assert_eq!(qty.default, Some(Value::I32(1)));
    assert_eq!(qty.to_string(), "\"qty\" i32 not null default 1");
    assert_eq!(described.version(), 1);
    assert!(described.indexes().is_empty());
    let table = read.open_table("orders")?;
    assert_eq!(read.table_schema(&table)?, described);
    drop(orders);
    Ok(())
}
```

## Renaming, emptying and deleting

| Operation | Behavior |
|---|---|
| `rename_table(from, to, overwrite)` | Keeps the schema, rows, indexes, identity and handles. Renaming to its own name does nothing. |
| `open_table_with(name, TableMode::Empty)` | Removes every row (see *Opening a table*). |
| `delete_table(name)` | Deletes the table with its rows and indexes and frees their pages. A missing name does nothing. |

`rename_table` fails with `ErrorKind::TreeNotFound` if `from` does not exist, and with
`ErrorKind::TreeAlreadyExists` if `to` exists, unless `overwrite` is `true`: then a table `to` is deleted first.
Overwriting a tree or a sequence fails with `ErrorKind::DifferentTreeType`. After `delete_table`, the table's
handles fail with `ErrorKind::TreeNotFound`.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, ErrorKind, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
    let mut write = store.write()?;
    let a = write.open_table_with("a", TableMode::CreateNew(&schema))?;
    write.insert_row(&a, &[("id", Value::I64(1))])?;
    write.open_table_with("b", TableMode::CreateNew(&schema))?;

    // Without overwrite, an existing destination fails.
    let error = write.rename_table("a", "b", false).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::TreeAlreadyExists);

    // With overwrite, "b" is deleted and "a" takes its name; the handle follows it.
    write.rename_table("a", "b", true)?;
    assert_eq!(write.row_count(&a)?, 1);
    assert_eq!(write.open_table("a").err().unwrap().kind(), ErrorKind::TreeNotFound);

    write.delete_table("b")?;
    write.delete_table("b")?; // a missing table: nothing happens
    assert_eq!(write.row_count(&a).unwrap_err().kind(), ErrorKind::TreeNotFound);
    write.commit()?;
    Ok(())
}
```

## Tables, trees and sequences share names

Trees, tables and sequences live in one catalog with one set of names: a name belongs to one object only.

- `tree_exists(name)` is `true` for a tree, a table or a sequence, and `list_trees()` lists all three, a table with
  `TreeKind::Table` and a sequence with `TreeKind::Sequence`; so does `tree_kind(name)`.
- Opening, emptying, deleting or renaming a table as a tree, or a tree as a table, fails with
  `ErrorKind::DifferentTreeType`, without effect.
- Creating a table with the name of a tree fails with `ErrorKind::DifferentTreeType` too.

```rust
use krummi::table::value::ColumnType;
use krummi::{Column, ErrorKind, OpenMode, Schema, Store, StoreOptions, TableMode, TreeKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
    let mut write = store.write()?;
    write.open_table_with("people", TableMode::CreateNew(&schema))?;
    write.open_tree_with("blobs", OpenMode::CreateNew)?;

    assert!(write.tree_exists("people")?);
    assert_eq!(write.tree_kind("people")?, Some(TreeKind::Table));
    assert_eq!(
        write.list_trees()?,
        [("blobs".to_string(), TreeKind::Normal), ("people".to_string(), TreeKind::Table)]
    );

    assert_eq!(write.open_tree("people").err().unwrap().kind(), ErrorKind::DifferentTreeType);
    assert_eq!(write.open_table("blobs").err().unwrap().kind(), ErrorKind::DifferentTreeType);
    let error = write.open_table_with("blobs", TableMode::OpenOrCreate(&schema)).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::DifferentTreeType);
    assert_eq!(write.delete_table("blobs").unwrap_err().kind(), ErrorKind::DifferentTreeType);
    write.commit()?;
    Ok(())
}
```

## Changing a schema

A write transaction changes a table's schema with four operations. None of them rewrites the rows: each writes the
same few pages whatever the number of rows, and the rows written before the change are read through the table's
schema history, as the current schema says.

| Operation | What existing rows read afterwards |
|---|---|
| `add_column(&table, &column)` | The new column at the end, with its default or null. |
| `drop_column(&table, name)` | The column no longer appears. |
| `rename_column(&table, from, to)` | The same values under the new name; the column keeps its identity. |
| `modify_column(&table, name, ty, nullable)` | Only `nullable = true` on a column outside the key is supported: the column now allows nulls. |

Each change that changes something adds 1 to the schema version, also when there are several in one write. A change
that changes nothing gives no new version: renaming a column to its own name, or modifying a column with its own type
and nullability. Every handle of the table in that write sees the new schema at once. An update rewrites its row in
the current version.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String).not_null(),
            Column::new("legacy", ColumnType::Bool),
        ],
        &["id"],
    );
    let mut write = store.write()?;
    let table = write.open_table_with("items", TableMode::CreateNew(&schema))?;
    write.insert_row(&table, &[("id", Value::I64(1)), ("name", Value::String("old".into()))])?;
    write.commit()?;

    let mut write = store.write()?;
    let table = write.open_table("items")?;
    let id_before = write.table_schema(&table)?.column("name").unwrap().id;
    write.add_column(&table, &Column::new("price", ColumnType::I32).not_null().with_default(Value::I32(10)))?;
    write.add_column(&table, &Column::new("note", ColumnType::String))?;
    write.drop_column(&table, "legacy")?;
    write.rename_column(&table, "name", "title")?;
    write.modify_column(&table, "title", ColumnType::String, true)?;
    write.rename_column(&table, "title", "title")?; // changes nothing

    let schema = write.table_schema(&table)?;
    assert_eq!(schema.version(), 6);
    assert_eq!(schema.column("title").unwrap().id, id_before);
    assert!(schema.column("title").unwrap().nullable);

    // The row written before the changes reads with the current schema.
    let row = write.get_row(&table, &[Value::I64(1)])?.expect("inserted");
    let columns: Vec<(&str, Value<'_>)> = row.columns().collect();
    assert_eq!(
        columns,
        [
            ("id", Value::I64(1)),
            ("title", Value::String("old".into())),
            ("price", Value::I32(10)),
            ("note", Value::Null),
        ]
    );
    drop(row);
    write.commit()?;
    Ok(())
}
```

### Rules and errors of schema changes

Every broken rule fails with `Error::InvalidSchema` (rule in `SchemaRule`) without changing anything, and, like every
public validation error, it does not prevent the commit. A column name outside 1 to 1,024 bytes fails with
`ErrorKind::InvalidSize`.

| Change | Error (`SchemaRule`) |
|---|---|
| Adding a column whose name exists, or renaming to one | `DuplicateColumnName` |
| Adding a column without nulls and without a default, also to an empty table | `NotNullWithoutDefault` |
| Adding a column past 1,024, or with a type or default outside its rules | The schema rules above |
| Dropping, renaming or modifying a column that does not exist | `ColumnNotFound` |
| Dropping a primary key column, or a column of an index (the error's `index` names it) | `DropKeyColumn` |
| Changing a type, forbidding nulls, or modifying a primary key column | `UnsupportedChange` (`InvalidKeyColumn` when a key column would allow nulls) |
| A change past schema version 65,535 | `VersionExhausted` |

A column added with the name of a dropped one is a new column: it has a new identity, goes at the end, and never
shows the old column's values.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, Schema, SchemaRule, Store, StoreOptions, TableMode};

fn rule(error: Error) -> SchemaRule {
    match error {
        Error::InvalidSchema { rule, .. } => rule,
        other => panic!("not an invalid schema: {other}"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("color", ColumnType::String),
        ],
        &["id"],
    );
    let mut write = store.write()?;
    let table = write.open_table_with("t", TableMode::CreateNew(&schema))?;
    write.insert_row(&table, &[("id", Value::I64(1)), ("color", Value::String("red".into()))])?;

    let not_null = Column::new("size", ColumnType::I32).not_null();
    assert_eq!(rule(write.add_column(&table, &not_null).unwrap_err()), SchemaRule::NotNullWithoutDefault);
    assert_eq!(rule(write.drop_column(&table, "id").unwrap_err()), SchemaRule::DropKeyColumn);
    assert_eq!(rule(write.drop_column(&table, "nope").unwrap_err()), SchemaRule::ColumnNotFound);
    assert_eq!(rule(write.rename_column(&table, "color", "id").unwrap_err()), SchemaRule::DuplicateColumnName);
    let error = write.modify_column(&table, "color", ColumnType::Bytes, true).unwrap_err();
    assert_eq!(rule(error), SchemaRule::UnsupportedChange);
    let error = write.modify_column(&table, "color", ColumnType::String, false).unwrap_err();
    assert_eq!(rule(error), SchemaRule::UnsupportedChange);
    assert_eq!(write.table_schema(&table)?.version(), 1); // nothing changed

    // Dropping a column and adding one with its name gives a new, empty column.
    write.drop_column(&table, "color")?;
    write.add_column(&table, &Column::new("color", ColumnType::String))?;
    let row = write.get_row(&table, &[Value::I64(1)])?.expect("inserted");
    assert_eq!(row.get("color"), Some(Value::Null));
    drop(row);
    write.commit()?; // the failed changes did not prevent the commit
    Ok(())
}
```

See also: [Rows](rows.md), [Data types](data-types.md), [JSON](json.md), [Secondary indexes](indexes.md),
[Statistics](statistics.md), [Sequences and identifiers](sequences-and-ids.md), [Trees](trees.md),
[Transactions](transactions.md), [Errors](errors.md).
