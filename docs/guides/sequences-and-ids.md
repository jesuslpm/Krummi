# Sequences and identifiers

Krummi has two ways to make values for primary keys: **sequences**, named objects of a store that hand out
increasing 64-bit integers inside write transactions, and the **generators** of `krummi::ids`, functions that make
`ulid` and `uuid` values with no store at all. Use a sequence when you want small, dense numbers that are never handed
out twice by the store; use a generator when you want identifiers that can be made anywhere, before any transaction,
and that sort by creation time. Neither is a column default: the application calls them and puts the value in the row.

## Quick path

1. In a write transaction, open a sequence with `open_sequence_with(name, SequenceMode::OpenOrCreate(None))`.
2. Ask for values with `next_value(&sequence)`: 1, 2, 3, ...
3. Use each value in a row, and commit.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, Schema, SequenceMode, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("name", ColumnType::String)],
        &["id"],
    );
    let mut write = store.write()?;
    let orders = write.open_table_with("orders", TableMode::OpenOrCreate(&schema))?;
    let ids = write.open_sequence_with("order_ids", SequenceMode::OpenOrCreate(None))?;
    for name in ["first", "second"] {
        let id = write.next_value(&ids)?;
        write.insert_row(&orders, &[("id", Value::I64(id)), ("name", Value::String(name.into()))])?;
    }
    assert!(write.get_row(&orders, &[Value::I64(2)])?.is_some());
    write.commit()?;
    Ok(())
}
```

## Opening a sequence

A sequence lives in the catalog with the trees and the tables, and shares their names: a name belongs to one object
only. `SequenceMode` gives the first value of a sequence it creates, 1 when it is `None`; any `i64` is accepted.

| Opening | Sequence missing | Sequence exists |
|---|---|---|
| `open_sequence_with(name, SequenceMode::CreateNew(first))` | Creates it. | Fails with `ErrorKind::TreeAlreadyExists`. |
| `open_sequence_with(name, SequenceMode::OpenOrCreate(first))` | Creates it. | Opens it, keeping its next value; `first` is ignored. |
| `WriteTransaction::open_sequence(name)` | Fails with `ErrorKind::TreeNotFound`. | Opens it. |
| `ReadTransaction::open_sequence(name)` | Fails with `ErrorKind::TreeNotFound`. | Opens it, without a value (see below). |

A name outside 1 to 1,024 bytes fails with `ErrorKind::InvalidSize`, and the name of a tree or a table with
`ErrorKind::DifferentTreeType`. Sequences have no mode that empties them. A `WriteSequence` stays on its thread and
fails with `ErrorKind::Misuse` in another transaction.

```rust
use krummi::{ErrorKind, SequenceMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let invoices = write.open_sequence_with("invoices", SequenceMode::CreateNew(Some(1_000)))?;
    assert_eq!(write.next_value(&invoices)?, 1_000);

    let error = write.open_sequence_with("invoices", SequenceMode::CreateNew(None)).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::TreeAlreadyExists);
    let again = write.open_sequence_with("invoices", SequenceMode::OpenOrCreate(Some(1)))?;
    assert_eq!(write.next_value(&again)?, 1_001); // the first value is ignored when it exists

    let below_zero = write.open_sequence_with("offsets", SequenceMode::CreateNew(Some(-2)))?;
    assert_eq!(write.next_value(&below_zero)?, -2);
    assert_eq!(write.next_value(&below_zero)?, -1);

    assert_eq!(write.open_sequence("missing").err().unwrap().kind(), ErrorKind::TreeNotFound);
    write.commit()?;
    Ok(())
}
```

## Asking for values

`next_value(&sequence)` returns the first value, then one more each time. It is a write, so:

- it exists only in a write transaction, through a `WriteSequence`; a `ReadSequence` from a read transaction only
  proves the sequence exists, and the compiler rejects asking it for a value;
- it takes `&mut self`, so it ends the rows and values read before, like any write;
- its values belong to the transaction: if the transaction aborts, they are handed out again; a value of a committed
  transaction is never handed out again, even when its rows are deleted, a table is emptied or the process crashes.

A sequence is not tied to any table. After it hands out `i64::MAX`, `next_value` fails with `Error::InvalidValue`,
rule `ValueRule::SequenceExhausted`, naming the sequence in its `sequence` field, without effect.

```rust
use krummi::{Error, SequenceMode, Store, StoreOptions, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let ids = write.open_sequence_with("ids", SequenceMode::CreateNew(None))?;
    assert_eq!(write.next_value(&ids)?, 1);
    write.commit()?;

    // An aborted write gives its values back.
    let mut write = store.write()?;
    let ids = write.open_sequence("ids")?;
    assert_eq!(write.next_value(&ids)?, 2);
    assert_eq!(write.next_value(&ids)?, 3);
    write.abort();

    let mut write = store.write()?;
    let ids = write.open_sequence("ids")?;
    assert_eq!(write.next_value(&ids)?, 2);
    write.commit()?;

    // A read transaction can open the sequence, but not ask it for a value.
    let read = store.read()?;
    let _proof = read.open_sequence("ids")?;
    drop(read);

    // At the end of the i64 range.
    let mut write = store.write()?;
    let last = write.open_sequence_with("last", SequenceMode::CreateNew(Some(i64::MAX)))?;
    assert_eq!(write.next_value(&last)?, i64::MAX);
    let error = write.next_value(&last).unwrap_err();
    assert!(matches!(
        error,
        Error::InvalidValue { rule: ValueRule::SequenceExhausted, sequence: Some(ref s), .. } if s == "last"
    ));
    write.commit()?;
    Ok(())
}
```

## Renaming and deleting sequences

| Operation | Behavior |
|---|---|
| `rename_sequence(from, to, overwrite)` | Keeps the next value, the identity and the handles. Renaming to its own name does nothing. |
| `delete_sequence(name)` | Deletes the sequence and frees its page. A missing name does nothing. |
| `tree_exists(name)`, `tree_kind(name)`, `list_trees()` | Include sequences, as `TreeKind::Sequence`. |

`rename_sequence` fails with `ErrorKind::TreeNotFound` if `from` does not exist, and with
`ErrorKind::TreeAlreadyExists` if `to` exists, unless `overwrite` is `true`: then a sequence `to` is deleted first,
and a tree or a table `to` fails with `ErrorKind::DifferentTreeType`. After `delete_sequence`, its handles fail with
`ErrorKind::TreeNotFound`.

```rust
use krummi::{ErrorKind, SequenceMode, Store, StoreOptions, TreeKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let a = write.open_sequence_with("a", SequenceMode::CreateNew(Some(10)))?;
    write.open_sequence_with("b", SequenceMode::CreateNew(None))?;
    assert_eq!(write.tree_kind("a")?, Some(TreeKind::Sequence));

    assert_eq!(write.rename_sequence("a", "b", false).unwrap_err().kind(), ErrorKind::TreeAlreadyExists);
    write.rename_sequence("a", "b", true)?;
    assert_eq!(write.next_value(&a)?, 10); // the handle follows the rename

    write.delete_sequence("b")?;
    write.delete_sequence("b")?; // a missing sequence: nothing happens
    assert_eq!(write.next_value(&a).unwrap_err().kind(), ErrorKind::TreeNotFound);
    write.commit()?;
    Ok(())
}
```

## Identifier generators

`krummi::ids` has three generators. Each is a function that needs no store and returns `Result`.

| Function | Returns | For a column of type | Layout |
|---|---|---|---|
| `ids::ulid()` | `Ulid` | `ColumnType::Ulid` | 48 bits of Unix milliseconds, then 80 random bits, big-endian. |
| `ids::uuid_v7()` | `Uuid` | `ColumnType::Uuid(UuidOrder::Rfc9562)` | RFC 9562 version 7: 48 bits of milliseconds, the version and variant bits, 74 random bits. |
| `ids::sequential_uuid()` | `Uuid` | `ColumnType::Uuid(UuidOrder::SqlServer)` | 48 bits of milliseconds in the bytes SQL Server compares first, 80 random bits in the others, as the .NET sequential generator lays them out. |

Ordering guarantees:

- Each generator is **strictly increasing within the process**, also across threads. In a new millisecond it draws
  new random bits; in the same millisecond, or when the clock goes backwards, it keeps the last millisecond and adds
  1 to the random part; if that overflows, it moves to the next millisecond and draws again.
- The random bits come from the operating system and cannot be predicted from other values.
- There is **no order promise between processes or across restarts**.
- Each value sorts by creation in its own column type: a `ulid` by its bytes, a version 7 `uuid` in a column with
  the RFC 9562 order, and a sequential `uuid` in a column with the SQL Server order. A sequential `uuid` in a column
  with the RFC 9562 order does not sort by time.

They fail with `Error::InvalidValue`, rule `ValueRule::MillisecondOutOfRange`, when the clock passes what 48 bits of
milliseconds hold (after the year 10889), and with `Error::Io` when the operating system gives no random bits.

`Ulid` and `Uuid` are 16-byte newtypes with public bytes (`.0`). A `Uuid` is always given in RFC 9562 byte order, also
for a column with the SQL Server order: the order is a property of the column, not of the value, so `Uuid` has no
`Ord`. `Ulid` has `Ord`, which is its creation order.

```rust
use krummi::ids::{self, Ulid};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = ids::ulid()?;
    let b = ids::ulid()?;
    assert!(a < b); // strictly increasing in the process, also within one millisecond

    let v7 = ids::uuid_v7()?;
    assert_eq!(v7.0[6] >> 4, 7); // version 7
    assert_eq!(v7.0[8] >> 6, 0b10); // the RFC 9562 variant

    // The millisecond: the first 6 bytes of a ulid and of a version 7 uuid,
    // bytes 10 to 15 of a sequential uuid (RFC 9562 byte order).
    let millis = |bytes: &[u8]| bytes.iter().fold(0u64, |n, b| n << 8 | u64::from(*b));
    let seq = ids::sequential_uuid()?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis() as u64;
    for ms in [millis(&a.0[..6]), millis(&v7.0[..6]), millis(&seq.0[10..])] {
        assert!(ms <= now && now - ms < 60_000);
    }

    // Text forms: 26 characters of Crockford's Base32, and lowercase 8-4-4-4-12.
    let text = a.to_string();
    assert_eq!(text.len(), 26);
    assert_eq!(text.parse::<Ulid>()?, a);
    let uuid_text = v7.to_string();
    assert_eq!(uuid_text.len(), 36);
    assert_eq!(uuid_text.parse::<ids::Uuid>()?, v7);
    Ok(())
}
```

### Text forms

| Type | `Display` | `FromStr` accepts |
|---|---|---|
| `Ulid` | 26 characters of Crockford's Base32 in uppercase, the first at most `7` | Only that form. |
| `Uuid` | RFC 9562 text in lowercase with hyphens (`8-4-4-4-12`), the bytes in their order | Only that form: no uppercase, braces or missing hyphens. |

Any other text fails with `Error::InvalidValue`, rule `ValueRule::MalformedText`. The same rule applies to a
`Value::String` written to a `ulid` or `uuid` column (see [Data types](data-types.md)).

```rust
use krummi::ids::{Ulid, Uuid};
use krummi::{Error, ValueRule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let uuid: Uuid = "0192f1c4-8b7a-7cde-8f00-123456789abc".parse()?;
    assert_eq!(uuid.0[0], 0x01);
    let malformed = "0192F1C4-8B7A-7CDE-8F00-123456789ABC".parse::<Uuid>().unwrap_err();
    assert!(matches!(malformed, Error::InvalidValue { rule: ValueRule::MalformedText, .. }));
    assert!("01ARZ3NDEKTSV4RRFFQ69G5FAV".parse::<Ulid>().is_ok());
    assert!("01arz3ndektsv4rrffq69g5fav".parse::<Ulid>().is_err()); // lowercase
    Ok(())
}
```

## Identifiers as primary keys

A generated identifier is a good single-column primary key: the value is known before the insert, so the application
can use it in other rows of the same write, and new rows go to the end of the key order, which keeps inserts and
[bulk loads](rows.md#bulk-load) in key order. Match the generator to the column type:

| Column | Generator | Row value |
|---|---|---|
| `Column::new("id", ColumnType::I64).not_null()` | a sequence's `next_value` | `Value::I64(id)` |
| `Column::new("id", ColumnType::Ulid).not_null()` | `ids::ulid()` | `Value::Ulid(id)` |
| `Column::new("id", ColumnType::Uuid(UuidOrder::Rfc9562)).not_null()` | `ids::uuid_v7()` | `Value::Uuid(id)` |
| `Column::new("id", ColumnType::Uuid(UuidOrder::SqlServer)).not_null()` | `ids::sequential_uuid()` | `Value::Uuid(id)` |

```rust
use krummi::ids;
use krummi::table::value::{ColumnType, UuidOrder, Value};
use krummi::{Column, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let tables = [
        ("by_ulid", ColumnType::Ulid),
        ("by_uuid_v7", ColumnType::Uuid(UuidOrder::Rfc9562)),
        ("by_sequential_uuid", ColumnType::Uuid(UuidOrder::SqlServer)),
    ];
    for (name, ty) in tables {
        let schema = Schema::new(
            vec![Column::new("id", ty).not_null(), Column::new("n", ColumnType::I32).not_null()],
            &["id"],
        );
        let table = write.open_table_with(name, TableMode::CreateNew(&schema))?;
        for n in 0..100 {
            let id = match ty {
                ColumnType::Ulid => Value::Ulid(ids::ulid()?),
                ColumnType::Uuid(UuidOrder::Rfc9562) => Value::Uuid(ids::uuid_v7()?),
                _ => Value::Uuid(ids::sequential_uuid()?),
            };
            write.insert_row(&table, &[("id", id), ("n", Value::I32(n))])?;
        }
        // A scan in key order gives the rows in the order they were created.
        let order: Vec<i32> = write
            .range_rows(&table, &[], ..)?
            .map(|row| row.map(|row| row.i32(1).unwrap()))
            .collect::<krummi::Result<_>>()?;
        assert_eq!(order, (0..100).collect::<Vec<_>>());
    }
    write.commit()?;
    Ok(())
}
```

See also: [Tables and schemas](tables.md), [Rows](rows.md), [Data types](data-types.md),
[Transactions](transactions.md), [Errors](errors.md).
