# Secondary indexes

A secondary index keeps the rows of a table sorted by other columns than the primary key, so you can look rows up
by those columns or read them in that order. This guide covers how to define an index (its key columns, their
direction and null order, uniqueness and included columns), how to create, describe and drop it, how writes keep it
up to date, and how to scan it for rows or for its entries alone. Use it when a table is read by something other than
its primary key. Indexes work the same in a store on disk and in a temporary store.

## Quick path

1. Create the table (see [tables](tables.md)).
2. Build an `IndexDefinition` with a name and its key columns, each an `IndexKey`.
3. Create it with `WriteTransaction::create_index`, in the same write or a later one; the rows already in the table
   get their entries at once.
4. Insert, update and delete rows as usual: Krummi keeps every index in step with its rows.
5. Scan the index with `index_rows` (the table's rows) or `index_entries` (the index's own values), in a read or a
   write.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("city", ColumnType::String),
        ],
        &["id"],
    );
    let table = write.open_table_with("people", TableMode::CreateNew(&schema))?;
    for (id, city) in [(1, "Oslo"), (2, "Lima"), (3, "Oslo")] {
        write.insert_row(&table, &[("id", Value::I64(id)), ("city", Value::String(city.into()))])?;
    }
    let by_city = IndexDefinition::new("by_city", vec![IndexKey::new("city")]);
    write.create_index(&table, &by_city, IndexMode::CreateIfNotExists)?;
    write.commit()?;

    // Look up the people of Oslo: the prefix gives the value of the first key column.
    let read = store.read()?;
    let table = read.open_table("people")?;
    let mut ids = Vec::new();
    for row in read.index_rows(&table, "by_city", &[Value::String("Oslo".into())], ..)? {
        ids.push(row?.get("id").unwrap().into_owned());
    }
    assert_eq!(ids, [Value::I64(1), Value::I64(3)]);
    Ok(())
}
```

## Define an index

An `IndexDefinition` has four public fields: `name`, `unique`, `key` (a `Vec<IndexKey>`) and `included` (a
`Vec<String>`). The builder methods cover the usual cases:

| Call | Gives |
|---|---|
| `IndexDefinition::new(name, key)` | A non-unique index with the key `key` and no included columns. |
| `.unique()` | The index unique: two rows with the same non-null values in every key column collide. |
| `.with_included(&["a", "b"])` | The index stores these columns too, in this order, so a scan of entries can return them without reading the row. |
| `IndexKey::new(column)` | A key column, ascending, with its nulls first (the defaults of `Direction` and `Nulls`). |
| `.descending()` | The key column sorts its values in reverse (`Direction::Descending`). |
| `.nulls_last()` | The key column's nulls sort after its values (`Nulls::Last`). |

`IndexKey` has the public fields `column`, `direction` (`Direction::Ascending` or `Direction::Descending`) and `nulls`
(`Nulls::First` or `Nulls::Last`), so you can also write it as a struct literal. The null order is part of the
definition even for a column that does not allow nulls. Where nulls go is always stated for the forward order of the
index, whatever the column's direction.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{
    Column, Direction, IndexDefinition, IndexKey, IndexMode, Nulls, Schema, Store, StoreOptions, TableMode,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("email", ColumnType::String),
            Column::new("score", ColumnType::I32),
            Column::new("name", ColumnType::String),
        ],
        &["id"],
    );
    let table = write.open_table_with("users", TableMode::CreateNew(&schema))?;

    // Unique on email.
    let by_email = IndexDefinition::new("by_email", vec![IndexKey::new("email")]).unique();
    // Highest score first, rows without a score at the end, carrying the name.
    let ranking = IndexDefinition::new("ranking", vec![IndexKey::new("score").descending().nulls_last()])
        .with_included(&["name"]);
    // The same kind of key written as a struct literal.
    let by_name = IndexDefinition::new(
        "by_name",
        vec![IndexKey { column: "name".into(), direction: Direction::Ascending, nulls: Nulls::Last }],
    );
    for definition in [&by_email, &ranking, &by_name] {
        write.create_index(&table, definition, IndexMode::CreateNew)?;
    }

    write.insert_row(&table, &[("id", Value::I64(1)), ("score", Value::I32(10))])?;
    write.insert_row(&table, &[("id", Value::I64(2)), ("score", Value::I32(30))])?;
    write.insert_row(&table, &[("id", Value::I64(3))])?;
    let ids: Vec<Value<'static>> = write
        .index_rows(&table, "ranking", &[], ..)?
        .map(|row| row.map(|row| row.get("id").unwrap().into_owned()))
        .collect::<krummi::Result<_>>()?;
    assert_eq!(ids, [Value::I64(2), Value::I64(1), Value::I64(3)]);
    write.commit()?;
    Ok(())
}
```

Rules of a definition:

- The name has 1 to 1,024 bytes of UTF-8 and is unique within its table. It is not a catalog name: two tables may have
  indexes of the same name.
- The key has 1 to 32 distinct columns, in the order the index sorts by.
- Included columns are distinct and not in the key. Any column type can be included, vectors and JSON too.
- Primary key columns may be in the key. Among the included columns they are accepted and do nothing, because every
  entry already holds the whole primary key: the stored definition, and Describe, leave them out.
- An entry of the index holds the values of its key columns, then the row's primary key, and then its included columns.

## Create an index

`WriteTransaction::create_index(&table, &definition, mode)` creates the index and fills it from every row the table
has. It exists only on a write transaction. The `IndexMode` says what happens when the table already has an index of
that name:

| Mode | The table has no index of the name | It has one with the same definition | It has one with another definition |
|---|---|---|---|
| `IndexMode::CreateNew` | Creates it | Fails with `Error::TreeAlreadyExists` | Fails with `Error::TreeAlreadyExists` |
| `IndexMode::CreateIfNotExists` | Creates it | Does nothing | Fails with `Error::DifferentIndex` |

"The same definition" means the same uniqueness, the same key columns with the same direction and null order, and the
same included columns in the same order, primary key columns left out. `Error::DifferentIndex` is of the class
`ErrorKind::DifferentSchema` and names the first difference in its `difference` field:
`SchemaDifference::IndexUniqueness`, then `IndexKey`, then `IndexIncluded`.

What creating an index over existing rows does:

- It reads every row and gives each its entry, with the values a read of the row gives: a column added after the row
  was stored has its default value or null.
- It sorts the entries in memory and, in a store on disk, past the memory it reserves, in temporary files in the
  subfolder `krummi.sort` of the store folder, which it removes when it ends. A temporary store sorts only in memory.
- It checks the size of every entry and, for a unique index, that no two rows repeat its key, before writing any page.
  A failure there leaves the store as it was and the write free to commit its other changes.
- It changes neither the schema version nor the schema state, so prepared projections keep working.
- Every handle of the table in the same write sees the new index at once; other transactions see it after the commit.

To load many rows, you can load the table first and create its indexes after, or create them first and load with them.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{
    Column, Error, ErrorKind, IndexDefinition, IndexKey, IndexMode, Schema, SchemaDifference, Store,
    StoreOptions, TableMode,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("n", ColumnType::I64)],
        &["id"],
    );
    let table = write.open_table_with("t", TableMode::CreateNew(&schema))?;
    write.insert_row(&table, &[("id", Value::I64(1)), ("n", Value::I64(7))])?;

    // A column added later: the existing row's entry takes its default.
    let added = Column::new("tag", ColumnType::String).with_default(Value::String("none".into()));
    write.add_column(&table, &added)?;
    let by_tag = IndexDefinition::new("by_tag", vec![IndexKey::new("tag")]);
    write.create_index(&table, &by_tag, IndexMode::CreateNew)?;
    let entry = write.index_entries(&table, "by_tag", &[], ..)?.next().unwrap()?;
    assert_eq!(entry.get("tag"), Some(Value::String("none".into())));

    // The same definition again: CreateIfNotExists does nothing, CreateNew fails.
    write.create_index(&table, &by_tag, IndexMode::CreateIfNotExists)?;
    let again = write.create_index(&table, &by_tag, IndexMode::CreateNew).unwrap_err();
    assert_eq!(again.kind(), ErrorKind::TreeAlreadyExists);

    // Another definition under the same name.
    let other = IndexDefinition::new("by_tag", vec![IndexKey::new("tag")]).unique();
    match write.create_index(&table, &other, IndexMode::CreateIfNotExists).unwrap_err() {
        Error::DifferentIndex { index, difference, .. } => {
            assert_eq!(index, "by_tag");
            assert_eq!(difference, SchemaDifference::IndexUniqueness);
        }
        error => panic!("unexpected {error}"),
    }

    // A failed creation has no effect: the write still commits.
    write.commit()?;
    Ok(())
}
```

### Invalid definitions

A definition that breaks a rule fails without effect with `Error::InvalidSchema`, naming the table, the index, the
column when there is one, and the `SchemaRule`. When it breaks several, the first in this order is reported, and
within a rule, the first column in the order the definition gives them (key, then included):

| Order | What is wrong | `SchemaRule` |
|---|---|---|
| 1 | No key columns | `NoIndexKey` |
| 2 | A column that does not exist | `ColumnNotFound` |
| 3 | A vector or JSON column in the key | `VectorIndexKey` |
| 4 | A column repeated in the key, in the included columns or in both | `RepeatedIndexColumn` |
| 5 | More than 32 key columns | `TooManyIndexKeyColumns` |
| 6 | A key that is the whole primary key or, in a non-unique index, an ascending prefix of it (a unique index on a strict prefix is a constraint, and is allowed) | `RedundantIndexKey` |
| 7 | The same key as another index of the table (same columns, order, directions and null orders) | `SameIndexKey` |
| 8 | More than 64 indexes in the table | `TooManyIndexes` |
| 9 | A schema too large for its catalog entry (index definitions count) | `SchemaTooLarge` |

Before these, an index or column name that does not have 1 to 1,024 bytes fails with `Error::InvalidSize`, and a
handle of a table this write deleted fails with `Error::TreeNotFound`.

```rust
use krummi::table::value::ColumnType;
use krummi::{
    Column, Error, IndexDefinition, IndexKey, IndexMode, Schema, SchemaRule, Store, StoreOptions, TableMode,
    WriteTable, WriteTransaction,
};

/// The rule a definition breaks.
fn rule(write: &mut WriteTransaction<'_>, table: &WriteTable<'_>, definition: &IndexDefinition) -> SchemaRule {
    match write.create_index(table, definition, IndexMode::CreateNew) {
        Err(Error::InvalidSchema { rule, .. }) => rule,
        other => panic!("unexpected {other:?}"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("n", ColumnType::I64),
            Column::new("doc", ColumnType::Json),
        ],
        &["id"],
    );
    let table = write.open_table_with("t", TableMode::CreateNew(&schema))?;

    let json_key = IndexDefinition::new("ix", vec![IndexKey::new("doc")]);
    assert_eq!(rule(&mut write, &table, &json_key), SchemaRule::VectorIndexKey);
    let primary = IndexDefinition::new("ix", vec![IndexKey::new("id")]);
    assert_eq!(rule(&mut write, &table, &primary), SchemaRule::RedundantIndexKey);
    let repeated = IndexDefinition::new("ix", vec![IndexKey::new("n")]).with_included(&["n"]);
    assert_eq!(rule(&mut write, &table, &repeated), SchemaRule::RepeatedIndexColumn);
    let missing = IndexDefinition::new("ix", vec![]);
    assert_eq!(rule(&mut write, &table, &missing), SchemaRule::NoIndexKey);

    // A JSON column may be included, and the primary key repeated among the included columns is ignored.
    let fine = IndexDefinition::new("ix", vec![IndexKey::new("n")]).with_included(&["doc", "id"]);
    write.create_index(&table, &fine, IndexMode::CreateNew)?;
    let described = write.describe_table("t")?;
    let included: Vec<&str> =
        described.index("ix").unwrap().included.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(included, ["doc"]);
    write.commit()?;
    Ok(())
}
```

## Describe the indexes

`describe_table(name)` and `table_schema(&table)`, in a read or a write, return a `TableSchema` that lists the
indexes as the transaction sees them:

| Method or field | Gives |
|---|---|
| `TableSchema::indexes()` | Every `SchemaIndex` of the table, in the order of their names (by bytes); empty when it has none. |
| `TableSchema::index(name)` | The `SchemaIndex` of that name, or `None`. |
| `TableSchema::index_state()` | A random 64-bit value drawn when the table is created and again each time one of its indexes is created or dropped. Compare it to know whether the set of indexes changed. |
| `SchemaIndex::id` | The index's identity: random, drawn when it is created. An index dropped and created again has a new one. |
| `SchemaIndex::name`, `unique` | Its name and whether it is unique. |
| `SchemaIndex::key` | Its key columns, each a `SchemaIndexKey` with the column's `id`, `name`, `direction` and `nulls`. |
| `SchemaIndex::included` | Its included columns, each a `SchemaIndexColumn` with the column's `id` and `name`; never primary key columns. |

Columns are named as the schema the transaction sees names them, so a renamed column shows its new name. Describe reads
only the catalog. Indexes are not catalog objects: `list` does not show them.

```rust
use krummi::table::value::ColumnType;
use krummi::{
    Column, Direction, IndexDefinition, IndexKey, IndexMode, Nulls, Schema, Store, StoreOptions, TableMode,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("at", ColumnType::Timestamp),
            Column::new("title", ColumnType::String),
        ],
        &["id"],
    );
    let table = write.open_table_with("posts", TableMode::CreateNew(&schema))?;
    let before = write.table_schema(&table)?.index_state();
    let latest = IndexDefinition::new("latest", vec![IndexKey::new("at").descending()]).with_included(&["title"]);
    write.create_index(&table, &latest, IndexMode::CreateNew)?;
    write.commit()?;

    let read = store.read()?;
    let described = read.describe_table("posts")?;
    assert_ne!(described.index_state(), before);
    let index = &described.indexes()[0];
    assert_eq!(index.name, "latest");
    assert!(!index.unique);
    assert_eq!(index.key[0].name, "at");
    assert_eq!(index.key[0].direction, Direction::Descending);
    assert_eq!(index.key[0].nulls, Nulls::First);
    assert_eq!(index.included[0].name, "title");
    assert_eq!(index.key[0].id, described.column("at").unwrap().id);
    Ok(())
}
```

## Drop an index

`WriteTransaction::drop_index(&table, name)` removes the index and frees its pages. A table without an index of that
name changes nothing and returns `Ok(())`. Dropping draws a new index state and changes neither the schema version nor
the schema state. An index cannot be renamed or changed: drop it and create it again, in the same write if you like,
with the same name and another definition.

```rust
use krummi::table::value::ColumnType;
use krummi::{Column, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("n", ColumnType::I64)],
        &["id"],
    );
    let table = write.open_table_with("t", TableMode::CreateNew(&schema))?;
    let first = IndexDefinition::new("by_n", vec![IndexKey::new("n")]);
    write.create_index(&table, &first, IndexMode::CreateNew)?;

    // Change the definition: drop it and create it again.
    write.drop_index(&table, "by_n")?;
    let second = IndexDefinition::new("by_n", vec![IndexKey::new("n").descending()]);
    write.create_index(&table, &second, IndexMode::CreateNew)?;

    // Dropping an index that does not exist is not an error.
    write.drop_index(&table, "missing")?;
    assert_eq!(write.table_schema(&table)?.indexes().len(), 1);
    write.commit()?;
    Ok(())
}
```

How other table operations affect indexes:

| Operation | Its indexes |
|---|---|
| Rename the table | Kept. |
| Empty the table (`TableMode::Empty` or `TableMode::CreateOrEmpty`) | Their entries are removed; definitions, identities and index state are kept. |
| Delete the table, or overwrite it by renaming another onto it | Deleted with it. |
| Rename a column | Kept; Describe shows the new name, with the same identity. |
| Allow nulls in a column of an index | Allowed; its nulls follow the index's null order. |
| Drop a column of an index (key or included) | Fails with `Error::InvalidSchema`, rule `SchemaRule::DropKeyColumn`, naming the first index (by name) that uses it. |

## Writes keep indexes up to date

Every write of a row (insert, update, upsert, delete, delete a range and bulk load) updates every index of the table in
the same transaction, including an index created earlier in that write. All the checks run before anything changes, so
a failed write of a row leaves the row and every index as they were. An identical update changes no index.

In a unique index, two rows collide when every key column has the same value in both, compared after normalization
(so `-0.0` equals `+0.0`), and none of them is null. Rows with a null in any key column never collide. A row that would
collide fails with `Error::DuplicateKey`:

| Field | Holds |
|---|---|
| `table` | The table's name. |
| `key` | The primary key of the row being stored, in text form. |
| `index` | `Some(name)` of the unique index that repeats; `None` when the primary key repeats. |
| `index_key` | `Some` text of the repeated values of the index's key columns; `None` for the primary key. |

When several things repeat, the primary key is reported first, and among indexes the first by name. Creating a unique
index over rows that already repeat a key fails the same way, without effect, naming the second of the first two rows
in the index's order. A bulk load whose rows collide, with the table or with each other, aborts the whole load.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Error, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("email", ColumnType::String)],
        &["id"],
    );
    let table = write.open_table_with("users", TableMode::CreateNew(&schema))?;
    let by_email = IndexDefinition::new("by_email", vec![IndexKey::new("email")]).unique();
    write.create_index(&table, &by_email, IndexMode::CreateNew)?;

    write.insert_row(&table, &[("id", Value::I64(1)), ("email", Value::String("a@x.org".into()))])?;
    let error = write
        .insert_row(&table, &[("id", Value::I64(2)), ("email", Value::String("a@x.org".into()))])
        .unwrap_err();
    match error {
        Error::DuplicateKey { index, index_key, .. } => {
            assert_eq!(index.as_deref(), Some("by_email"));
            assert!(index_key.is_some());
        }
        error => panic!("unexpected {error}"),
    }

    // Nulls never collide in a unique index.
    write.insert_row(&table, &[("id", Value::I64(3))])?;
    write.insert_row(&table, &[("id", Value::I64(4))])?;

    // An update moves the row's entry; the failed insert left nothing behind.
    write.update_row(&table, &[("id", Value::I64(1)), ("email", Value::String("b@x.org".into()))])?;
    let emails: Vec<Value<'static>> = write
        .index_entries(&table, "by_email", &[], ..)?
        .map(|entry| entry.map(|entry| entry.get("email").unwrap().into_owned()))
        .collect::<krummi::Result<_>>()?;
    assert_eq!(emails, [Value::Null, Value::Null, Value::String("b@x.org".into())]);
    write.commit()?;
    Ok(())
}
```

An entry has a maximum size: its key columns and the primary key, in their stored form, may take at most 2,048 bytes
(included columns do not count). A descending column or one with nulls last may take a few more bytes than an
ascending one. A row whose entry is larger fails with `Error::InvalidSize` (`SizeSubject::IndexEntry`), naming the
table and the index, without effect.

## Scan an index

Two operations scan an interval of an index, in a read or a write transaction:

| Operation | Returns | Iterator |
|---|---|---|
| `index_rows(&table, index, prefix, bounds)` | The table's rows, as a primary key scan reads them | `ReadIndexRows` (read) or `WriteIndexRows` (write), of `Result<TableRow>` |
| `index_entries(&table, index, prefix, bounds)` | The index's entries, without reading the rows | `ReadIndexEntries` (read) or `WriteIndexEntries` (write), of `Result<IndexEntry>` |

Both are double-ended iterators: forward, in the order of the index; `.rev()` or `next_back`, in the exact reverse.
Forward, entries come in the order of the key, with each column's direction and null order, and entries with the same
key, nulls included, in the ascending order of their primary key. A scan in a write sees that write's changes. After a
failure a scan returns nothing more.

The interval:

- `prefix` gives values for the first k key columns, from none (`&[]`) to all. A `Value::Null` in the prefix asks for
  the entries whose column is null.
- `bounds`, any `RangeBounds<Value>` (`..`, `a..`, `a..b`, `a..=b`, `..b`, or a pair of `Bound`s), bounds the next key
  column after the prefix. Use `..` when the prefix gives every column, since a bound then is an error.
- Bounds are values in the natural order of the column's type, whatever the column's direction: the lower bound is
  always the smaller value. With a bound, entries whose column is null are outside the interval; without bounds,
  inside.
- Values are normalized and rounded as a row's, so `-0.0` finds `+0.0` and any NaN finds the stored NaN.
- An empty or inverted interval returns nothing and no error.
- Looking up the rows of a value is scanning with that value in every key column. In a unique index without nulls this
  gives at most one row.
- There are no bounds on more than one column at a time, nor on the primary key columns of the entries.

```rust
use std::ops::Bound;

use krummi::table::value::{ColumnType, Value};
use krummi::{Column, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("city", ColumnType::String),
            Column::new("age", ColumnType::I32),
        ],
        &["id"],
    );
    let table = write.open_table_with("people", TableMode::CreateNew(&schema))?;
    let people = [(1, "Oslo", Some(30)), (2, "Lima", Some(41)), (3, "Oslo", Some(25)), (4, "Oslo", None), (5, "Oslo", Some(52))];
    for (id, city, age) in people {
        let age = age.map_or(Value::Null, Value::I32);
        write.insert_row(&table, &[("id", Value::I64(id)), ("city", Value::String(city.into())), ("age", age)])?;
    }
    // City ascending, then age descending; nulls first (the default).
    let by_city = IndexDefinition::new("by_city", vec![IndexKey::new("city"), IndexKey::new("age").descending()]);
    write.create_index(&table, &by_city, IndexMode::CreateNew)?;
    write.commit()?;

    let read = store.read()?;
    let table = read.open_table("people")?;
    let oslo = [Value::String("Oslo".into())];
    let ids = |rows: krummi::ReadIndexRows<'_>| -> krummi::Result<Vec<i64>> {
        rows.map(|row| row.map(|row| match row.get("id") { Some(Value::I64(id)) => id, _ => unreachable!() }))
            .collect()
    };

    // No bounds: every person of Oslo, the null age first, then the oldest first.
    assert_eq!(ids(read.index_rows(&table, "by_city", &oslo, ..)?)?, [4, 5, 1, 3]);
    // Backward: the exact reverse.
    let mut backward = Vec::new();
    for row in read.index_rows(&table, "by_city", &oslo, ..)?.rev() {
        backward.push(row?.get("id").unwrap().into_owned());
    }
    assert_eq!(backward, [Value::I64(3), Value::I64(1), Value::I64(5), Value::I64(4)]);
    // Ages from 26 to 52, excluded: the lower bound is the smaller value, even on a descending column.
    let some = read.index_rows(&table, "by_city", &oslo, (Bound::Included(Value::I32(26)), Bound::Excluded(Value::I32(52))))?;
    assert_eq!(ids(some)?, [1]);
    // The whole key: a lookup.
    let exact = [Value::String("Oslo".into()), Value::I32(25)];
    assert_eq!(ids(read.index_rows(&table, "by_city", &exact, ..)?)?, [3]);
    // A null in the prefix finds the null ages.
    let unknown = [Value::String("Oslo".into()), Value::Null];
    assert_eq!(ids(read.index_rows(&table, "by_city", &unknown, ..)?)?, [4]);
    // No prefix: the whole index; an inverted interval gives nothing.
    assert_eq!(ids(read.index_rows(&table, "by_city", &[], ..)?)?, [2, 4, 5, 1, 3]);
    assert!(read.index_rows(&table, "by_city", &oslo, Value::I32(50)..Value::I32(10))?.next().is_none());
    Ok(())
}
```

The rows of `index_rows` are the table's `TableRow`s and can be bound to a `Projection` like those of a primary key
scan (see [rows](rows.md)). A row's text and bytes borrow the transaction's pages.

## Read entries: covering reads

`index_entries` reads only the index. Each `IndexEntry` has, in this order, the index's key columns, its included
columns, and the primary key columns that are not in its key, each column once. When an index includes every column a
query needs, the query never reads the table's rows: a covering read. A `string` column with the `ci` or `ci_ai`
collation sorts by its folded form, but an entry returns its text as the row stores it, never the folded form; a
unique index over such a column rejects equivalent texts (`Ana` and `ANA`) with `ErrorKind::DuplicateKey`, whose
texts are the row's.

| Method | Gives |
|---|---|
| `len()`, `is_empty()` | The number of columns of the entry (never zero). |
| `name(position)` | The name of the column at `position`. |
| `value(position)`, `get(name)` | The value of a column, or `None` for a position or name the entry does not have. |
| `values()`, `columns()` | Every value, or every `(name, value)` pair, in the entry's order. |

Values of key and primary key columns are decoded into copies of their own, text and bytes included. Values of
included columns are read as a row's: text, bytes, vectors and JSON borrow the transaction's pages. An entry, like a
row, lives until the next operation that may modify its transaction.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, IndexDefinition, IndexKey, IndexMode, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![
            Column::new("region", ColumnType::String).not_null(),
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("total", ColumnType::I64),
            Column::new("customer", ColumnType::String),
            Column::new("notes", ColumnType::String),
        ],
        &["region", "id"],
    );
    let table = write.open_table_with("orders", TableMode::CreateNew(&schema))?;
    for (id, total, customer) in [(1, 120, "Ada"), (2, 80, "Bo"), (3, 300, "Cy")] {
        let row = [
            ("region", Value::String("north".into())),
            ("id", Value::I64(id)),
            ("total", Value::I64(total)),
            ("customer", Value::String(customer.into())),
        ];
        write.insert_row(&table, &row)?;
    }
    let by_total = IndexDefinition::new("by_total", vec![IndexKey::new("total").descending()])
        .with_included(&["customer"]);
    write.create_index(&table, &by_total, IndexMode::CreateNew)?;
    write.commit()?;

    let read = store.read()?;
    let table = read.open_table("orders")?;
    let top = read.index_entries(&table, "by_total", &[], ..)?.next().unwrap()?;
    // Key column, included column, then the primary key columns not in the key.
    let names: Vec<&str> = (0..top.len()).map(|at| top.name(at).unwrap()).collect();
    assert_eq!(names, ["total", "customer", "region", "id"]);
    assert_eq!(top.value(0), Some(Value::I64(300)));
    assert_eq!(top.get("customer"), Some(Value::String("Cy".into())));
    assert_eq!(top.get("id"), Some(Value::I64(3)));
    // A column the index does not store is not in the entry.
    assert_eq!(top.get("notes"), None);
    Ok(())
}
```

## Which columns can be indexed

| Column type | In the key | Included |
|---|---|---|
| `bool`, integers, floats, `decimal`, `string`, `bytes`, `date`, `time`, `timestamp`, `duration`, `uuid` (both byte orders), `ulid` | Yes, in the natural order of the type (see [data types](data-types.md)) | Yes |
| Vector | No: `SchemaRule::VectorIndexKey` | Yes |
| JSON | No: `SchemaRule::VectorIndexKey` | Yes |

A `string` column sorts in the index by its collation (see [collations](data-types.md#collations)), and its key is
measured with its folded form. A `uuid` column declared with SQL Server's order sorts in that order in the index, and its values come back from an
entry in the byte order of RFC 9562, as from a row.

## Read and write scans and references

- A scan in a read transaction (`ReadIndexRows`, `ReadIndexEntries`) can be sent to another thread, like its
  transaction.
- A scan in a write transaction (`WriteIndexRows`, `WriteIndexEntries`) stays on the write's thread and sees its
  changes.
- Scanning takes `&self`: it is not a change and ends no references. The rows and entries it lends end at the next
  operation that may modify the transaction (an insert, a delete, creating or dropping an index...). The compiler
  enforces this.
- `create_index` and `drop_index` take `&mut self`, so they end every reference read before, even when they fail or
  change nothing.
- Creating an index, and dropping one that exists, are changes of the write. `CreateIfNotExists` on an equal index,
  dropping a missing index, scanning and failed operations are not.

## Limits and errors

| Limit | Value |
|---|---|
| Index name | 1 to 1,024 bytes of UTF-8 |
| Indexes per table | 64 |
| Key columns per index | 1 to 32 |
| Entry size (key columns plus primary key, stored form) | 2,048 bytes |
| Scan prefix with its bound, encoded as in an entry | 2,048 bytes |

Errors of the scans, in the order they are checked:

| Situation | Error |
|---|---|
| An index name that does not have 1 to 1,024 bytes | `Error::InvalidSize` |
| A table handle of another transaction | `ErrorKind::Misuse` |
| A table handle of a table the write deleted (write only) | `Error::TreeNotFound` |
| An index the table does not have | `Error::DifferentIndex` with `SchemaDifference::MissingIndex` (class `ErrorKind::DifferentSchema`) |
| A prefix longer than the key, or a bound when the prefix gives every column | `Error::InvalidValue`, rule `ValueRule::InvalidRange` |
| A value of another type, out of its domain, a null in the prefix of a column without nulls, or a null as a bound | `Error::InvalidValue`, with its column and rule |
| A prefix with its bound of more than 2,048 bytes encoded, even when the interval is empty | `Error::InvalidSize`, `SizeSubject::Prefix` |
| A damaged page the scan needs | Damage, as any read |

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{
    Column, Error, ErrorKind, IndexDefinition, IndexKey, IndexMode, Schema, SchemaDifference, Store, StoreOptions,
    TableMode, ValueRule,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("n", ColumnType::I64)],
        &["id"],
    );
    let table = write.open_table_with("t", TableMode::CreateNew(&schema))?;
    write.create_index(&table, &IndexDefinition::new("by_n", vec![IndexKey::new("n")]), IndexMode::CreateNew)?;

    match write.index_rows(&table, "missing", &[], ..) {
        Err(Error::DifferentIndex { difference, .. }) => assert_eq!(difference, SchemaDifference::MissingIndex),
        other => panic!("unexpected {:?}", other.err()),
    }
    let error = write.index_entries(&table, "by_n", &[Value::String("seven".into())], ..).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::InvalidValue);
    match write.index_entries(&table, "by_n", &[Value::I64(7)], Value::I64(1)..) {
        Err(Error::InvalidValue { rule, .. }) => assert_eq!(rule, ValueRule::InvalidRange),
        other => panic!("unexpected {:?}", other.err()),
    }
    match write.index_entries(&table, "by_n", &[], Value::Null..) {
        Err(Error::InvalidValue { rule, .. }) => assert_eq!(rule, ValueRule::InvalidRange),
        other => panic!("unexpected {:?}", other.err()),
    }
    write.commit()?;
    Ok(())
}
```

See also: [tables](tables.md) · [rows](rows.md) · [data types](data-types.md) · [JSON](json.md) ·
[statistics](statistics.md) · [transactions](transactions.md) · [errors](errors.md)
