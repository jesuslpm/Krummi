# Getting started

This guide takes you from an empty project to a program that stores data in Krummi and reads it back. It opens a
store, writes a key and a value in a tree and a row in a table inside one write transaction, commits, reads both
back in a read transaction, and closes the store. Read it first; every step links to the guide that covers it in
detail.

## Steps

1. Add the `krummi` crate to your project.
2. Open a store.
3. Open the write transaction and make your changes.
4. Commit.
5. Open a read transaction and read the data back.
6. Close the store.

## 1. Add the crate

Krummi is not published on crates.io (its workspace sets `publish = false`), so you add it from a copy of the
repository or from its Git URL. The public API is the `krummi` crate, in `crates/krummi`; the other crates of the
workspace are internal to the engine.

```text
# Cargo.toml of your application

[dependencies]
# From a local copy of the repository:
krummi = { path = "../Krummi/crates/krummi" }

# Or from the Git repository (pick one of the two lines):
# krummi = { git = "<URL of the Krummi repository>" }
```

Krummi needs Rust 1.89 or later (the workspace's `rust-version`).

## 2 to 6. A complete program

The program below does every step. It keeps its store in a new folder under the system's temporary folder and
removes that folder at the end.

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, OpenMode, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A folder of its own for the store. A missing folder is created; an empty one gets a new store.
    let folder = std::env::temp_dir().join(format!(
        "krummi-guide-getting-started-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
    ));

    // 2. Open the store.
    let store = Store::open(&folder, &StoreOptions::default())?;

    // 3. The write transaction: one at a time per store.
    let mut write = store.write()?;

    // A tree: byte keys, each with one value.
    let notes = write.open_tree_with("notes", OpenMode::OpenOrCreate)?;
    write.set(&notes, b"greeting", b"hello, raven")?;

    // A table: typed columns and a primary key.
    let schema = Schema::new(
        vec![
            Column::new("id", ColumnType::I64).not_null(),
            Column::new("name", ColumnType::String),
        ],
        &["id"],
    );
    let birds = write.open_table_with("birds", TableMode::OpenOrCreate(&schema))?;
    write.insert_row(&birds, &[("id", Value::I64(1)), ("name", Value::String("Huginn".into()))])?;

    // 4. Commit: durable when it returns, and visible to the transactions opened after it.
    write.commit()?;

    // 5. A read transaction sees the store as it is when it opens.
    let read = store.read()?;
    let notes = read.open_tree("notes")?;
    assert_eq!(read.get(&notes, b"greeting")?, Some(&b"hello, raven"[..]));

    let birds = read.open_table("birds")?;
    let row = read.get_row(&birds, &[Value::I64(1)])?.expect("the row we inserted");
    assert_eq!(row.get("name"), Some(Value::String("Huginn".into())));
    assert_eq!(row.text(1).as_deref(), Some("Huginn")); // by position, as its Rust type
    drop(row);
    drop(read); // a store cannot close while a transaction is open

    // 6. Close the store: everything committed is in its data file, and the lock is released.
    store.close()?;

    // Opening it again finds the data.
    let store = Store::open(&folder, &StoreOptions::default())?;
    let read = store.read()?;
    let notes = read.open_tree("notes")?;
    assert_eq!(read.get(&notes, b"greeting")?, Some(&b"hello, raven"[..]));
    drop(read);
    store.close()?;

    std::fs::remove_dir_all(&folder)?;
    Ok(())
}
```

## Try it without files

A temporary store lives only in memory and keeps nothing when it closes. It behaves like a store on disk, so it is
the quickest way to try things out and the usual choice for tests.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let tree = write.open_tree_with("counters", OpenMode::OpenOrCreate)?;
    write.set(&tree, b"visits", &1u64.to_le_bytes())?;
    write.commit()?;

    let read = store.read()?;
    let tree = read.open_tree("counters")?;
    let visits = read.get(&tree, b"visits")?.map(|v| u64::from_le_bytes(v.try_into().unwrap()));
    assert_eq!(visits, Some(1));
    drop(read);
    store.close()?;
    Ok(())
}
```

## Rules to keep in mind

| Rule | Why | Detail in |
|---|---|---|
| Only one process, and one `Store`, can have a folder open at a time. | The store takes a lock; a second opening fails with `ErrorKind::StoreInUse`. | [Stores](stores.md) |
| There is one write transaction at a time; the others wait in order of arrival. | Krummi has a single writer per store. | [Transactions](transactions.md) |
| A thread cannot open a second write transaction while it has one open. | It would wait for itself; it fails with `ErrorKind::Misuse`. | [Transactions](transactions.md) |
| A value or row read inside a write transaction must be dropped before the next change and before `commit`. | Reads borrow the engine's memory without copying it. | [Transactions](transactions.md) |
| Dropping a write transaction without committing it rolls it back. | Nothing it changed applies. | [Transactions](transactions.md) |
| Close every transaction before `close`. | `close` takes the store by value; the compiler refuses it while a transaction borrows the store. | [Stores](stores.md) |
| Every operation returns `krummi::Result`; branch on `error.kind()`. | Errors are grouped into kinds. | [Errors](errors.md) |

## See also

[Stores](stores.md) · [Transactions](transactions.md) · [Errors](errors.md) · [Trees](trees.md) ·
[Tables](tables.md) · [Rows](rows.md) · [Data types](data-types.md)
