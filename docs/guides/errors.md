# Errors

Every fallible operation of Krummi returns `krummi::Result<T>`, whose error is `krummi::Error`. This guide explains
how to tell errors apart by their kind, what every kind means and what to do about it, and the public types that
some errors carry to say exactly what went wrong: `SizeSubject`, `ValueRule`, `JsonFault` and `JsonCause`,
`SchemaRule`, `SchemaDifference`, and `BulkLoadError` for bulk loads. Read it when you write the error handling of
an application; each operation's guide says which errors it returns.

## Steps

1. Propagate errors with `?`, or keep the `krummi::Error` to inspect it.
2. Branch on `error.kind()`, an `ErrorKind`, when you only need the class.
3. Match on the `Error` variant when you need its data (the folder, the name, the rule).
4. Always add a `_` arm: both enums are `#[non_exhaustive]` and may gain variants.
5. Show the error with `Display` (`{error}`) when you report it; the text is English.

```rust
use krummi::{ErrorKind, OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    write.open_tree_with("orders", OpenMode::CreateNew)?;

    // 2. Branch on the kind.
    match write.open_tree_with("orders", OpenMode::CreateNew) {
        Ok(_) => unreachable!("it already exists"),
        Err(error) => match error.kind() {
            ErrorKind::TreeAlreadyExists => println!("already there: {error}"),
            ErrorKind::InvalidSize => println!("bad name: {error}"),
            _ => return Err(error.into()), // 4. anything else goes up
        },
    }

    // 3. Match on the variant for its data.
    let error = write.open_tree("missing").err().unwrap();
    match &error {
        krummi::Error::TreeNotFound { folder, name } => {
            assert_eq!(folder, &None); // a temporary store has no folder
            assert_eq!(name, "missing");
        }
        other => panic!("unexpected: {other}"),
    }

    // 5. The English text.
    assert_eq!(error.to_string(), "tree \"missing\" not found in temporary store");

    write.commit()?; // these failures did not prevent the commit
    store.close()?;
    Ok(())
}
```

## The `Error` type

- `error.kind()` gives its `ErrorKind`. Each `Error` variant has the kind of the same name, except
  `Error::DifferentIndex`, which is of kind `ErrorKind::DifferentSchema`.
- `Error` implements `std::error::Error` and `Display`, so `?` turns it into `Box<dyn std::error::Error>`.
  `source()` gives the operating system's `std::io::Error` for `Access` and `Io`, and `None` for the others.
- `error.duplicate()` makes a copy of the same kind and data (an operating-system cause is copied as its kind and
  message), since `Error` is not `Clone`.
- `folder` is the store's folder, and `None` in the errors that a temporary store can give.
- `ErrorKind` is `Copy`, `Eq` and `Hash`, so you can compare it, match on it and keep it in sets.

## Every kind

| `ErrorKind` | What it means | What to do | See |
|---|---|---|---|
| `Access` | The folder cannot be used: it is a file, permission is missing, the file system is read-only or the path is not valid; or a store file cannot be read or deleted because of permissions. | Fix the path or the permissions. | [Stores](stores.md) |
| `StoreInUse` | The store is already open, by another process or by this one. | Close the other `Store`, or share the one you have between threads. | [Stores](stores.md#the-lock) |
| `NotAStore` | The folder has entries but no Krummi store. | Open an empty or a new folder, or the right one. | [Stores](stores.md) |
| `Damaged` | Part of the store is damaged. `page` says which page, or `None` when what is damaged is what the store needs to open. | Krummi never returns damaged data as valid. Restore the store from a copy. | [Stores](stores.md#when-the-system-fails) |
| `DifferentFormat` | The store was written with another format version (`found`, `expected`). | Open it with the version of Krummi that wrote it; stores are not migrated before 1.0. | [Stores](stores.md) |
| `DiskFull` | No space left on the disk. The transaction applies nothing; the store stays open and readable. | Free space and retry; no reopening is needed. | [Stores](stores.md#size-and-growth) |
| `Io` | The operating system failed to read or write. After a failed write or flush the store refuses writes until it is reopened; reads keep working. | Close the store and open it again: the opening recovers every commit. | [Stores](stores.md#when-the-system-fails) |
| `InvalidOptions` | The options given to open the store are not valid (`reason`): a `max_size` under 1 MiB. | Fix the options. | [Stores](stores.md#options) |
| `MaxSize` | Growing would pass the maximum size set when opening (`max_size`, `needed`, in bytes). The store stays usable. | Free space in the store, or open it with a larger `max_size`. | [Stores](stores.md#options) |
| `OutOfMemory` | The system did not give the memory the store needs (`requested` bytes). An open store stays usable. | Free memory and retry. | [Stores](stores.md) |
| `Timeout` | A request for the write transaction waited its whole `limit`. It had no effect. | Retry, or wait longer. | [Transactions](transactions.md#waiting-with-a-limit) |
| `Misuse` | The engine was called in an order or with values it does not accept (`reason`): a second write transaction on the same thread, a wait limit too large to count, a tree, table or sequence handle used in another transaction. | A programming error: fix the code. | [Transactions](transactions.md) |
| `InvalidSize` | A name, key, value, prefix, row or index entry is outside its allowed sizes. | Check the size against the table of `SizeSubject` below. | [Trees](trees.md), [Tables](tables.md) |
| `DifferentTreeType` | The name exists with another kind (`found`) than the one requested (`requested`): a normal tree opened as a multi-value tree, a fixed-size tree with another value length, a table opened as a tree, and so on. | Use the opening of its kind; `tree_kind` tells it. | [Trees](trees.md) |
| `TreeNotFound` | The tree, table or sequence does not exist, or a handle refers to one its write transaction deleted. | Create it, or open it with a mode that creates it. | [Trees](trees.md) |
| `TreeAlreadyExists` | A tree, table or sequence with that name exists (with `CreateNew`, or a rename without overwrite), or an index with that name in its table (`index`). | Open it instead, or choose another name. | [Trees](trees.md), [Indexes](indexes.md) |
| `InvalidValue` | A value breaks a rule of its column or of its operation (`rule`, a `ValueRule`). | Fix the value; see `ValueRule` below. | [Rows](rows.md), [Data types](data-types.md) |
| `DuplicateKey` | A row with that primary key already exists, or a row repeats the values of a unique index (`index`, `index_key`). | Use `update_row` or `upsert_row`, or another key. | [Rows](rows.md), [Indexes](indexes.md) |
| `DifferentSchema` | A schema or index definition given to open or create does not match what the table has, or a row names a column the table does not have (`difference`, `expected`, `found`). | Compare with `describe_table`, and change the schema with its operations. | [Tables](tables.md) |
| `InvalidSchema` | A schema, a change to it, or an index definition breaks a rule (`rule`, a `SchemaRule`). | Fix the schema; see `SchemaRule` below. | [Tables](tables.md), [Indexes](indexes.md) |

## Errors when opening a store

`Store::open` can fail with `InvalidOptions`, `Access`, `StoreInUse`, `NotAStore`, `Damaged`, `DifferentFormat`,
and, while creating or recovering the store, `DiskFull`, `OutOfMemory` or `Io`. `Store::open_temporary` fails
with `InvalidOptions` or `OutOfMemory`. [Stores](stores.md#opening-a-store-on-disk) gives the order in which
they are checked.

```rust
use krummi::{Error, ErrorKind, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = std::env::temp_dir().join(format!(
        "krummi-guide-errors-open-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
    ));
    let store = Store::open(&folder, &StoreOptions::default())?;

    match Store::open(&folder, &StoreOptions::default()) {
        Err(Error::StoreInUse { folder: in_use }) => assert_eq!(in_use, folder),
        Err(other) => panic!("unexpected: {other}"),
        Ok(_) => panic!("a store opens once"),
    }

    let error = Store::open_temporary(&StoreOptions { max_size: Some(4096) }).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::InvalidOptions);
    assert_eq!(error.to_string(), "invalid options: the maximum size must be at least 1 MiB");

    store.close()?;
    std::fs::remove_dir_all(&folder)?;
    Ok(())
}
```

## Which failures stop a write transaction

A failure inside a write transaction is one of two sorts ([Transactions](transactions.md#when-a-commit-fails)):

| Sort | Kinds | Effect |
|---|---|---|
| Validation | `InvalidSize`, `DifferentTreeType`, `TreeNotFound`, `TreeAlreadyExists`, `InvalidValue`, `DuplicateKey`, `DifferentSchema`, `InvalidSchema`, and `Misuse` for a handle of another transaction | No effect. The transaction goes on and can commit. |
| Failure of the store | `DiskFull`, `MaxSize`, `OutOfMemory`, `Damaged`, `Io` | The transaction can no longer commit: `commit` fails with the class of the first such failure and applies nothing. |

After an `Io` failure when writing, the store also refuses every write until it is reopened.

## Sizes: `SizeSubject`

`Error::InvalidSize` says what is out of size (`what`), the size it received and the allowed range, `min` to `max`
inclusive, in bytes. For an index entry or a prefix of an index scan, `table` and `index` name the index.

| `SizeSubject` | What it is | Allowed |
|---|---|---|
| `TreeName` | The name of a tree, table or sequence, in bytes of UTF-8 | 1 to 1,024 |
| `Key` | A key of a tree | 1 to 2,048 |
| `Value` | A value of a tree | 0 to 268,435,456 (256 MiB) in a normal tree; 0 to 2,048 in a multi-value tree; exactly the tree's length in a fixed-size tree |
| `Prefix` | The prefix of a scan or a range removal, or of an interval of an index | 0 to 2,048 |
| `ColumnName` | A column name, in bytes of UTF-8 | 1 to 1,024 |
| `PrimaryKey` | The primary key of a row, or a bound of a range of it, in its stored form | 1 to 2,048 |
| `Row` | A row in its stored form, or the included columns of an index entry | up to 256 MiB |
| `IndexName` | An index name, in bytes of UTF-8 | 1 to 1,024 |
| `IndexEntry` | An entry of a secondary index: its key columns and the row's primary key, in their stored form | 1 to 2,048 |

```rust
use krummi::{Error, OpenMode, SizeSubject, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let tree = write.open_tree_with("t", OpenMode::OpenOrCreate)?;
    match write.set(&tree, &[1u8; 3000], b"v").unwrap_err() {
        Error::InvalidSize { what, received, min, max, .. } => {
            assert_eq!(what, SizeSubject::Key);
            assert_eq!((received, min, max), (3000, 1, 2048));
        }
        other => panic!("unexpected: {other}"),
    }
    write.commit()?;
    store.close()?;
    Ok(())
}
```

## Values: `ValueRule`

`Error::InvalidValue` gives the `rule` a value breaks, and `table` and `column` (or `sequence`) say where. When a
row breaks several rules, the first one in this order is reported.

| `ValueRule` | Meaning |
|---|---|
| `RepeatedColumn` | A row gives a column more than once. |
| `MissingKeyColumn` | A primary key column is missing from the row or the key. |
| `NullNotAllowed` | A null, or no value, in a column that does not allow nulls. |
| `WrongType` | A value of another type than its column's. |
| `MalformedText` | A `ulid` or `uuid` given as malformed text. |
| `InvalidJson` | A text that is not a valid JSON document; `json` says why and where (see below). |
| `MalformedJsonPointer` | A malformed JSON pointer given to navigate a document. |
| `InvalidReadScale` | A `decimal(p, s)` read from a JSON value with `p` or `s` out of 1 ≤ p ≤ 38, 0 ≤ s ≤ p. |
| `OutOfDomain` | A value outside its type's domain, also after rounding. |
| `DecimalPrecision` | A `decimal` that does not fit its precision. |
| `VectorDimension` | A vector of another dimension than its column's. |
| `VectorNotFinite` | A NaN or an infinity in a vector. |
| `ZeroVectorCosine` | A zero vector in a column with the cosine metric. |
| `InvalidRange` | A malformed range of keys. |
| `SequenceExhausted` | A sequence at the maximum of the signed 64-bit integers. |
| `MillisecondOutOfRange` | A generator millisecond that does not fit in 48 bits. |
| `EmptyProjection` | A projection of no columns. |

[Data types](data-types.md) gives each type's domain, [Rows](rows.md) the rules of each row operation, and
[Sequences and identifiers](sequences-and-ids.md) those of sequences and generators.

## JSON faults: `JsonFault` and `JsonCause`

With `ValueRule::InvalidJson`, the error's `json` field holds a `JsonFault`:

| Field | Meaning |
|---|---|
| `cause` | Why the text is not a valid document, a `JsonCause`. |
| `position` | The number of bytes of the text before the fault. |
| `pointer` | The RFC 6901 pointer of the innermost object or array open at the fault; `""` for the root or none. |

| `JsonCause` | Meaning, and where `position` points |
|---|---|
| `InvalidUtf8` | A byte sequence that is not valid UTF-8; its first byte. |
| `Syntax` | A fault of the RFC 8259 grammar; the first character with which the text stops being the start of a JSON text, or the text's length when it ends too soon. |
| `LoneSurrogate` | An escaped UTF-16 surrogate that does not form a pair; the `\` of its escape. |
| `RepeatedProperty(name)` | An object with two properties of this (decoded) name; the opening quote of the later one. |
| `TooDeep` | More than 128 nested objects or arrays; the `{` or `[` that opens level 129. |

At the same position, the causes are reported in the order of this table. See [JSON columns](json.md).

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
    let docs = write.open_table_with("docs", TableMode::OpenOrCreate(&schema))?;

    let text = r#"{"a":1,"a":2}"#;
    let error = write
        .insert_row(&docs, &[("id", Value::I64(1)), ("doc", Value::Json(Json::new(text)))])
        .unwrap_err();
    match error {
        Error::InvalidValue { column, rule, json: Some(fault), .. } => {
            assert_eq!(column.as_deref(), Some("doc"));
            assert_eq!(rule, ValueRule::InvalidJson);
            assert_eq!(fault.cause, JsonCause::RepeatedProperty("a".into()));
            assert_eq!(fault.position, 7); // the quote of the second "a"
            assert_eq!(&*fault.pointer, "");
        }
        other => panic!("unexpected: {other}"),
    }
    write.commit()?;
    store.close()?;
    Ok(())
}
```

## Schemas: `SchemaRule` and `SchemaDifference`

`Error::InvalidSchema` gives the `rule` a schema, a change to it or an index definition breaks, with `table`, and
`column` or `index` when the rule is about one. When a schema breaks several rules, the first in this order is
reported, and within it the first column in schema order.

| `SchemaRule` | Meaning |
|---|---|
| `NoPrimaryKey` | No primary key. |
| `InvalidKeyColumn` | A primary key column that allows nulls, is a vector or JSON, or has a default value. |
| `InvalidKeyReference` | The primary key names a column that does not exist, or repeats one. |
| `DuplicateColumnName` | Two columns with the same name. |
| `TooManyColumns` | More than 1,024 live columns. |
| `VectorOutOfLimits` | A vector type outside its limits. |
| `DecimalOutOfLimits` | A `decimal(p, s)` outside 1 ≤ p ≤ 38 and 0 ≤ s ≤ p. |
| `CosineOnBit` | The cosine metric on `bit` elements. |
| `InvalidDefault` | A default value that breaks the rules of its column. |
| `NotNullWithoutDefault` | An added column that does not allow nulls and has no default value. |
| `UnsupportedChange` | A change that is not allowed: changing a type, changing the primary key, forbidding nulls. |
| `ColumnNotFound` | Dropping, renaming or modifying a column that does not exist, or an index naming one. |
| `DropKeyColumn` | Dropping a primary key column, or a column of an index (`index` names it). |
| `VersionExhausted` | A schema change past version 65,535. |
| `NoIndexKey` | An index definition without key columns. |
| `VectorIndexKey` | A vector or JSON column in the key of an index. |
| `RepeatedIndexColumn` | A column given more than once in an index definition. |
| `TooManyIndexKeyColumns` | An index key of more than 32 columns. |
| `RedundantIndexKey` | An index key that is the whole primary key or, in an index that is not unique, a prefix of it, in the same order and all ascending. |
| `SameIndexKey` | An index key equal to another index's of the table. |
| `TooManyIndexes` | More than 64 indexes in a table. |
| `VectorColumn` | A vector column named by an estimate, a read of statistics or analyze: vectors have no statistics. |
| `JsonColumn` | A JSON column named by an estimate, a read of statistics or analyze: JSON columns have no statistics. |
| `SchemaTooLarge` | A schema whose stored form, with its names, defaults and index definitions, passes 256 MiB. |

`Error::DifferentSchema` (for a table) and `Error::DifferentIndex` (for an index, same kind) give the `difference`,
with `expected` (what you gave) and `found` (what the table has) as text.

| `SchemaDifference` | What differs |
|---|---|
| `Type` | A column's type. |
| `Nullability` | Whether a column allows nulls. |
| `Default` | A column's default value. |
| `MissingColumn` | A column named by the schema or by a row that the table does not have. |
| `PrimaryKey` | The primary key columns or their order. |
| `Projection` | A row bound to a projection of another table, another store or another schema of its table. |
| `IndexUniqueness` | Whether an index is unique. |
| `IndexKey` | A key column of an index at a position (its column, direction and null order), or the number of key columns. |
| `IndexIncluded` | An included column of an index at a position, or the number of included columns. |
| `MissingIndex` | An index that the table does not have, named by a scan of an index. |

See [Tables](tables.md) and [Secondary indexes](indexes.md).

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{
    Column, Error, ErrorKind, Schema, SchemaDifference, SchemaRule, Store, StoreOptions, TableMode, ValueRule,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;

    // A schema without a primary key.
    let no_key = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &[]);
    match write.open_table_with("t", TableMode::CreateNew(&no_key)).err().unwrap() {
        Error::InvalidSchema { rule, .. } => assert_eq!(rule, SchemaRule::NoPrimaryKey),
        other => panic!("unexpected: {other}"),
    }

    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("name", ColumnType::String)],
        &["id"],
    );
    let table = write.open_table_with("t", TableMode::CreateNew(&schema))?;

    // Opening it with another type for a column.
    let other = Schema::new(
        vec![Column::new("id", ColumnType::I32).not_null(), Column::new("name", ColumnType::String)],
        &["id"],
    );
    match write.open_table_with("t", TableMode::OpenOrCreate(&other)).err().unwrap() {
        Error::DifferentSchema { column, difference, .. } => {
            assert_eq!(column.as_deref(), Some("id"));
            assert_eq!(difference, SchemaDifference::Type);
        }
        other => panic!("unexpected: {other}"),
    }

    // A value of the wrong type, then a repeated primary key.
    match write.insert_row(&table, &[("id", Value::String("one".into()))]).unwrap_err() {
        Error::InvalidValue { column, rule, .. } => {
            assert_eq!(column.as_deref(), Some("id"));
            assert_eq!(rule, ValueRule::WrongType);
        }
        other => panic!("unexpected: {other}"),
    }
    write.insert_row(&table, &[("id", Value::I64(1))])?;
    let error = write.insert_row(&table, &[("id", Value::I64(1))]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::DuplicateKey);

    write.commit()?; // none of those failures prevents the commit
    store.close()?;
    Ok(())
}
```

## Bulk loads: `BulkLoadError`

`Store::bulk_load` and `Store::bulk_load_with_timeout` read rows from a source you give, an iterator of
`Result<row, E>`, so they fail with `BulkLoadError<E>`:

| Variant | Meaning |
|---|---|
| `BulkLoadError::Store(Error)` | The engine failed: a misuse or a timeout getting the write turn, the table (not found, another class), the first row that breaks the rules of insert, or a failure of a write or of the commit. |
| `BulkLoadError::Source(E)` | Your source returned this error. Returning an error is also how you cancel a load. |

Either way the whole load is aborted: the table, its row count and its sequences are as before. `BulkLoadError`
converts from `Error`, implements `Display` (the engine's text, or `the source of the bulk load failed: ` followed
by yours) and implements `std::error::Error` when `E` does. See [Rows](rows.md).

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{BulkLoadError, Column, ErrorKind, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let schema = Schema::new(vec![Column::new("id", ColumnType::I64).not_null()], &["id"]);
    write.open_table_with("t", TableMode::CreateNew(&schema))?;
    write.commit()?;

    // The source fails on its second row: nothing is loaded.
    let rows = vec![Ok(vec![("id", Value::I64(1))]), Err("the file ended early")];
    match store.bulk_load("t", rows) {
        Err(BulkLoadError::Source(message)) => assert_eq!(message, "the file ended early"),
        other => panic!("unexpected: {other:?}"),
    }
    let read = store.read()?;
    assert_eq!(read.row_count(&read.open_table("t")?)?, 0);
    drop(read);

    // The engine fails: the table does not exist.
    let rows: Vec<Result<Vec<(&str, Value)>, String>> = vec![Ok(vec![("id", Value::I64(1))])];
    match store.bulk_load("missing", rows) {
        Err(BulkLoadError::Store(error)) => assert_eq!(error.kind(), ErrorKind::TreeNotFound),
        other => panic!("unexpected: {other:?}"),
    }
    store.close()?;
    Ok(())
}
```

## See also

[Getting started](getting-started.md) · [Stores](stores.md) · [Transactions](transactions.md) · [Trees](trees.md) ·
[Tables](tables.md) · [Rows](rows.md) · [Data types](data-types.md) · [JSON columns](json.md) ·
[Secondary indexes](indexes.md) · [Sequences and identifiers](sequences-and-ids.md)
