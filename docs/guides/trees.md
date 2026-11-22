# Trees

A tree is a named, ordered map of byte keys to byte values inside a store, like a `BTreeMap<Vec<u8>, Vec<u8>>` that
lives on disk and changes only inside transactions. This guide covers the normal tree: the catalog of names, opening
a tree in every mode, the catalog operations (exists, kind, list, rename, empty, delete), the entry operations
(`set`, `try_add`, `get`, `remove`, `len`), key and value limits, large values, scans and range removals, and how long
the references a tree returns stay valid. Use a normal tree when each key has one value. When a key needs a sorted set
of values, use a [multi-value tree](multi-value-trees.md); when the keys are unsigned 64-bit integers and every value
has the same length, a [fixed-size tree](fixed-size-trees.md) is denser; when the data has typed columns,
a primary key and secondary indexes, use a [table](tables.md).

## Step by step

1. Open a store and start a write transaction with `store.write()`.
2. Open the tree with `open_tree_with(name, OpenMode::OpenOrCreate)`: it creates the tree the first time.
3. Store entries with `set`, read them with `get`, scan them with `range`.
4. Commit with `commit()`: the changes become durable and visible to the reads opened afterwards.
5. Read in a read transaction: `store.read()`, then `open_tree(name)` and `get` or `range`.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;

    let mut write = store.write()?;
    let orders = write.open_tree_with("orders", OpenMode::OpenOrCreate)?;
    write.set(&orders, b"order-1", b"two ravens")?;
    write.set(&orders, b"order-2", b"one raven")?;
    write.commit()?;

    let read = store.read()?;
    let orders = read.open_tree("orders")?;
    assert_eq!(read.get(&orders, b"order-1")?, Some(&b"two ravens"[..]));
    assert_eq!(read.len(&orders)?, 2);
    for entry in read.range(&orders, ..)? {
        let (key, value) = entry?;
        println!("{} = {}", String::from_utf8_lossy(key), String::from_utf8_lossy(value));
    }
    Ok(())
}
```

## The catalog and tree names

Every store has one catalog that maps names to trees. Normal trees, [multi-value trees](multi-value-trees.md),
[fixed-size trees](fixed-size-trees.md), [tables](tables.md) and [sequences](sequences-and-ids.md) all share it, so
two of them never have the same name. The catalog takes no page until the first tree is created.

| Rule | Value |
|---|---|
| Type | `&str`: UTF-8 text, so a name that is not UTF-8 cannot be passed |
| Length | 1 to 1,024 bytes, counted in UTF-8 |
| Comparison | Byte by byte: no case folding or Unicode normalization, so `orders` and `Orders` are two trees |
| Reserved names | None |
| Order | Byte order, as `list_trees` returns them |
| Outside the length | `ErrorKind::InvalidSize`, with `SizeSubject::TreeName`, minimum `1` and maximum `1024` |

Opening a name that belongs to an object of another kind fails with `ErrorKind::DifferentTreeType`, whatever the
mode. `tree_exists` and `tree_kind` see every object of the catalog, tables and sequences included, and
`delete_tree` and `rename_tree` refuse tables and sequences with `ErrorKind::DifferentTreeType` without effect.

```rust
use krummi::{Error, ErrorKind, OpenMode, SequenceMode, SizeSubject, Store, StoreOptions, TreeKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;

    // Names are compared byte by byte: these are two different trees.
    write.open_tree_with("orders", OpenMode::CreateNew)?;
    write.open_tree_with("Orders", OpenMode::CreateNew)?;
    write.open_tree_with("pedidos-señal", OpenMode::CreateNew)?; // any UTF-8 text
    write.open_tree_with(&"n".repeat(1024), OpenMode::CreateNew)?; // the longest name

    // An empty name, or one over 1,024 bytes, fails with invalid size.
    // (Handles have no `Debug`, so the examples read a failed opening with `.err()`.)
    let error = write.open_tree_with("", OpenMode::OpenOrCreate).err();
    assert_eq!(error.map(|e| e.kind()), Some(ErrorKind::InvalidSize));
    match write.open_tree_with(&"n".repeat(1025), OpenMode::OpenOrCreate).err() {
        Some(Error::InvalidSize { what, received, min, max, .. }) => {
            assert_eq!(what, SizeSubject::TreeName);
            assert_eq!((received, min, max), (1025, 1, 1024));
        }
        other => panic!("unexpected result: {other:?}"),
    }

    // A sequence shares the catalog: its name cannot be opened as a tree.
    let _ids = write.open_sequence_with("ids", SequenceMode::OpenOrCreate(None))?;
    assert!(write.tree_exists("ids")?);
    assert_eq!(write.tree_kind("ids")?, Some(TreeKind::Sequence));
    let error = write.open_tree_with("ids", OpenMode::OpenOrCreate).err();
    assert_eq!(error.map(|e| e.kind()), Some(ErrorKind::DifferentTreeType));
    let error = write.delete_tree("ids").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::DifferentTreeType);
    assert!(write.tree_exists("ids")?); // nothing was deleted
    write.commit()?;
    Ok(())
}
```

## Opening a tree

A tree is opened inside a transaction and gives a handle. A read transaction has only `open_tree(name)`, which opens
a tree that exists in its snapshot. A write transaction has `open_tree(name)`, which only opens, and
`open_tree_with(name, mode)`, whose `OpenMode` follows .NET's `FileMode`:

| `OpenMode` | Tree exists | Tree does not exist |
|---|---|---|
| `CreateNew` | Fails with `ErrorKind::TreeAlreadyExists` | Creates it empty |
| `OpenOrCreate` | Opens it | Creates it empty |
| `CreateOrEmpty` | Removes all its entries | Creates it empty |
| `Empty` | Removes all its entries | Fails with `ErrorKind::TreeNotFound` |
| (`open_tree`) | Opens it | Fails with `ErrorKind::TreeNotFound` |

Every opening also fails with `ErrorKind::InvalidSize` for a name outside 1 to 1,024 bytes and with
`ErrorKind::DifferentTreeType` when the name belongs to a multi-value tree, a fixed-size tree, a table or a sequence.
Opening reads only the catalog. Emptying frees the tree's pages; the tree keeps its identity and its handles, which
see it empty.

```rust
use krummi::{ErrorKind, OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;

    // open_tree only opens.
    let error = write.open_tree("log").err();
    assert_eq!(error.map(|e| e.kind()), Some(ErrorKind::TreeNotFound));

    // CreateNew creates, and fails the second time.
    let log = write.open_tree_with("log", OpenMode::CreateNew)?;
    write.set(&log, b"1", b"started")?;
    let error = write.open_tree_with("log", OpenMode::CreateNew).err();
    assert_eq!(error.map(|e| e.kind()), Some(ErrorKind::TreeAlreadyExists));

    // OpenOrCreate opens the existing tree with its entries.
    let same = write.open_tree_with("log", OpenMode::OpenOrCreate)?;
    assert_eq!(write.len(&same)?, 1);

    // CreateOrEmpty and Empty remove every entry; every handle sees the tree empty.
    write.open_tree_with("log", OpenMode::Empty)?;
    assert_eq!(write.len(&log)?, 0);
    let error = write.open_tree_with("missing", OpenMode::Empty).err();
    assert_eq!(error.map(|e| e.kind()), Some(ErrorKind::TreeNotFound));
    write.open_tree_with("fresh", OpenMode::CreateOrEmpty)?; // creates it
    write.commit()?;

    // A read transaction opens with open_tree only.
    let read = store.read()?;
    let log = read.open_tree("log")?;
    assert_eq!(read.len(&log)?, 0);
    Ok(())
}
```

### Read and write handles

`ReadTransaction::open_tree` gives a `ReadTree`; the write openings give a `WriteTree`. The operations that modify
take a `WriteTree`, so modifying through a read handle does not compile, and neither does using a handle after its
store is gone.

| Handle | Sees | Threads | In another transaction |
|---|---|---|---|
| `ReadTree` | The read transaction's snapshot | Can be sent to and shared with other threads, like its transaction | Fails with `ErrorKind::Misuse` without effect |
| `WriteTree` | The write transaction's state, with every change made through any handle of the tree | Stays on the thread that opened it | Fails with `ErrorKind::Misuse` without effect |

A transaction can open as many trees as it wants, and the same tree more than once: every handle of a tree sees the
same state. A handle is bound to the tree it opened, not to its name: it keeps working after a rename, and once its
tree is deleted it fails with `ErrorKind::TreeNotFound`, even if a new tree with the same name is created.

```rust
use krummi::{ErrorKind, OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let first = write.open_tree_with("items", OpenMode::OpenOrCreate)?;
    let second = write.open_tree("items")?;
    write.set(&first, b"a", b"1")?;
    assert_eq!(write.get(&second, b"a")?, Some(&b"1"[..])); // the same tree
    write.commit()?;

    // A handle belongs to the transaction that opened it.
    let one = store.read()?;
    let other = store.read()?;
    let items = one.open_tree("items")?;
    assert_eq!(other.get(&items, b"a").unwrap_err().kind(), ErrorKind::Misuse);
    Ok(())
}
```

## Checking, listing and naming trees

| Operation | Transaction | Returns | Notes |
|---|---|---|---|
| `tree_exists(name)` | Read and write | `bool` | Never fails because the tree is missing; sees tables and sequences too |
| `tree_kind(name)` | Read and write | `Option<TreeKind>` | `TreeKind::Normal`, `MultiValue`, `FixedSize(len)`, `Table` or `Sequence`; `None` if missing |
| `list_trees()` | Read and write | `Vec<(String, TreeKind)>` | Every object of the catalog in byte order of the names; an owned copy |
| `rename_tree(from, to, overwrite)` | Write | `()` | Keeps the tree's kind, entries and handles |
| `open_tree_with(name, OpenMode::Empty)` | Write | handle | Removes every entry; also `CreateOrEmpty` |
| `delete_tree(name)` | Write | `()` | Deletes the tree and frees its pages; a missing tree is not an error |

A write transaction sees its own changes to the catalog; a read transaction sees its snapshot. All of them validate
the name first (`ErrorKind::InvalidSize`).

`rename_tree` fails with `ErrorKind::TreeNotFound` if `from` does not exist (also when `to` is the same name), and
with `ErrorKind::TreeAlreadyExists` if `to` exists, unless `overwrite` is `true`: then the tree `to` is deleted first,
whatever its kind among the trees. Renaming a tree to its own name does nothing.

```rust
use krummi::{ErrorKind, OpenMode, Store, StoreOptions, TreeKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let draft = write.open_tree_with("draft", OpenMode::CreateNew)?;
    write.set(&draft, b"k", b"v")?;
    write.open_tree_with("archive", OpenMode::CreateNew)?;
    write.open_multi_tree_with("tags", OpenMode::CreateNew)?;
    write.open_fixed_tree_with("counters", 8, OpenMode::CreateNew)?;

    assert!(write.tree_exists("draft")?);
    assert!(!write.tree_exists("nothing")?);
    assert_eq!(write.tree_kind("counters")?, Some(TreeKind::FixedSize(8)));
    assert_eq!(write.tree_kind("nothing")?, None);
    assert_eq!(
        write.list_trees()?,
        vec![
            ("archive".to_string(), TreeKind::Normal),
            ("counters".to_string(), TreeKind::FixedSize(8)),
            ("draft".to_string(), TreeKind::Normal),
            ("tags".to_string(), TreeKind::MultiValue),
        ]
    );

    // Renaming keeps the entries and the handles.
    write.rename_tree("draft", "final", false)?;
    assert_eq!(write.get(&draft, b"k")?, Some(&b"v"[..]));
    assert!(!write.tree_exists("draft")?);

    // The destination must not exist, unless overwrite is set.
    let error = write.rename_tree("final", "archive", false).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::TreeAlreadyExists);
    write.rename_tree("final", "archive", true)?; // the old "archive" is deleted first
    let error = write.rename_tree("draft", "x", false).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::TreeNotFound);

    // Deleting ends the tree: its handles fail, and a missing tree is not an error.
    write.delete_tree("archive")?;
    write.delete_tree("archive")?;
    assert_eq!(write.get(&draft, b"k").unwrap_err().kind(), ErrorKind::TreeNotFound);
    let again = write.open_tree_with("archive", OpenMode::CreateNew)?; // a new, empty tree
    assert_eq!(write.len(&again)?, 0);
    assert_eq!(write.len(&draft).unwrap_err().kind(), ErrorKind::TreeNotFound);
    write.commit()?;
    Ok(())
}
```

## Entry operations

The key operations follow a .NET `Dictionary`. None of them fails because the key is there or not; all of them
validate the sizes first.

| Operation | Transaction | Returns | Behavior |
|---|---|---|---|
| `set(&tree, key, value)` | Write | `()` | Stores the value, replacing the value of a key that is there |
| `try_add(&tree, key, value)` | Write | `bool` | Stores the value only if the key is not there; returns whether it did |
| `get(&tree, key)` | Read and write | `Option<&[u8]>` | The value, or `None`; an empty value is `Some(&[])` |
| `remove(&tree, key)` | Write | `bool` | Removes the key; returns whether it was there |
| `len(&tree)` | Read and write | `u64` | The number of entries, counted without walking the tree |

`len` also takes a multi-value or fixed-size handle. In a write transaction every operation sees the changes made so
far by that transaction.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let users = write.open_tree_with("users", OpenMode::OpenOrCreate)?;

    write.set(&users, b"huginn", b"thought")?;
    write.set(&users, b"huginn", b"thought, again")?; // replaces
    assert!(write.try_add(&users, b"muninn", b"memory")?); // added
    assert!(!write.try_add(&users, b"muninn", b"other")?); // already there: unchanged
    assert_eq!(write.get(&users, b"muninn")?, Some(&b"memory"[..]));

    // An empty value is a value: get tells it from a missing key.
    write.set(&users, b"nobody", b"")?;
    assert_eq!(write.get(&users, b"nobody")?, Some(&b""[..]));
    assert_eq!(write.get(&users, b"missing")?, None);
    assert_eq!(write.len(&users)?, 3);

    assert!(write.remove(&users, b"nobody")?);
    assert!(!write.remove(&users, b"nobody")?); // not there any more
    assert_eq!(write.len(&users)?, 2);
    write.commit()?;
    Ok(())
}
```

### Keys, values and their limits

| Item | Minimum | Maximum | Order |
|---|---|---|---|
| Key | 1 byte | 2,048 bytes | Byte by byte as unsigned numbers; a key that is a prefix of another goes first |
| Value | 0 bytes | 268,435,456 bytes (256 MiB) | — |
| Scan bound | 1 byte | 2,048 bytes | Validated as a key |
| Scan prefix | 0 bytes | 2,048 bytes | An empty prefix limits nothing |

A size outside its limits fails with `ErrorKind::InvalidSize`; the error says what was wrong (`SizeSubject::Key`,
`Value` or `Prefix`), the size received and the minimum and maximum. The key is checked before the value, and
`try_add` checks the value even when the key is there. There are no custom comparators: store integers big-endian so
that byte order is numeric order, or use a [fixed-size tree](fixed-size-trees.md).

The failures of validation (`InvalidSize`, `DifferentTreeType`, `TreeNotFound`, `TreeAlreadyExists`, and `Misuse` for
a handle of another transaction) have no effect and leave the write transaction usable: it can still commit. Any
other failure inside a write, such as a damaged page or the store's maximum size, makes its commit fail.

```rust
use krummi::{Error, ErrorKind, OpenMode, SizeSubject, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let tree = write.open_tree_with("limits", OpenMode::OpenOrCreate)?;

    write.set(&tree, &[0xff; 2048], b"the longest key")?;
    assert_eq!(write.set(&tree, b"", b"x").unwrap_err().kind(), ErrorKind::InvalidSize);
    match write.set(&tree, &[0; 2049], b"x").unwrap_err() {
        Error::InvalidSize { what, received, min, max, .. } => {
            assert_eq!(what, SizeSubject::Key);
            assert_eq!((received, min, max), (2049, 1, 2048));
        }
        other => panic!("unexpected error: {other}"),
    }

    // Byte order: upper case before lower case, a prefix before its extensions.
    for key in [&b"b"[..], b"a", b"ab", b"B", b"a\0"] {
        write.set(&tree, key, b"")?;
    }
    let keys = write
        .range(&tree, ..&[0xff][..])?
        .map(|entry| entry.map(|(key, _)| key.to_vec()))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(keys, [&b"B"[..], b"a", b"a\0", b"ab", b"b"]);

    // Integers stored big-endian sort numerically.
    let numbers = write.open_tree_with("numbers", OpenMode::OpenOrCreate)?;
    for n in [300u32, 2, 10] {
        write.set(&numbers, &n.to_be_bytes(), b"")?;
    }
    let sorted = write
        .range(&numbers, ..)?
        .map(|entry| entry.map(|(key, _)| u32::from_be_bytes(key.try_into().unwrap())))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(sorted, [2, 10, 300]);

    // The validation failures above did not spoil the transaction.
    write.commit()?;
    Ok(())
}
```

### Large values

A value can have up to 256 MiB. A value too large to share a page is kept in a run of contiguous pages of its own,
and every read and scan returns it whole as one `&[u8]` that points into the engine's memory, without copying it.
`set` copies a large value into its pages, which the commit writes whole, so it takes memory until the commit and
counts against the store's maximum size. Overwriting or removing a large value frees its pages.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let blobs = write.open_tree_with("blobs", OpenMode::OpenOrCreate)?;
    let photo = vec![7u8; 4 << 20]; // 4 MiB
    write.set(&blobs, b"photo", &photo)?;
    write.commit()?;

    let read = store.read()?;
    let blobs = read.open_tree("blobs")?;
    let value = read.get(&blobs, b"photo")?.expect("stored above"); // one slice, not a copy
    assert_eq!(value.len(), 4 << 20);
    assert!(value.iter().all(|&byte| byte == 7));
    Ok(())
}
```

## Scans

`range(&tree, bounds)` scans the entries whose keys are within `bounds`, in key order. It follows Rust's
`BTreeMap::range`: each bound is included, excluded or open, and `bounds` is any `RangeBounds<&[u8]>`, such as `..`,
`a..b`, `a..=b`, `a..`, `..b` or a pair of `std::ops::Bound`. `range_prefix(&tree, prefix, bounds)` keeps only the
keys that start with `prefix`.

The scan is a double-ended iterator of `Result<(&[u8], &[u8])>`: `next` takes from the front, `next_back` from the
back, and `rev()` reverses it. Each entry comes out once, even when both ends are used, and after a failure the scan
returns nothing more. A scan reads no page until it is used.

| Case | Result |
|---|---|
| A bound's key is not in the tree | Valid: the scan starts or stops where it would be |
| Bounds that leave no key between them (lower after upper, `a..a`) | An empty scan, not an error |
| A bound's key empty or over 2,048 bytes | `ErrorKind::InvalidSize`, also when the tree is empty |
| A prefix over 2,048 bytes | `ErrorKind::InvalidSize`, checked after the bounds |
| An empty prefix | Limits nothing: the same as `range` |

```rust
use std::ops::Bound;

use krummi::{ErrorKind, OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let tree = write.open_tree_with("letters", OpenMode::OpenOrCreate)?;
    for key in ["a", "b", "c", "d", "e", "user:1", "user:2", "user:3"] {
        write.set(&tree, key.as_bytes(), key.to_uppercase().as_bytes())?;
    }
    write.commit()?;

    let read = store.read()?;
    let tree = read.open_tree("letters")?;
    // The keys of a scan, as text.
    fn keys<'t>(
        scan: impl Iterator<Item = krummi::Result<(&'t [u8], &'t [u8])>>,
    ) -> krummi::Result<Vec<String>> {
        scan.map(|entry| entry.map(|(key, _)| String::from_utf8_lossy(key).into_owned()))
            .collect()
    }

    assert_eq!(keys(read.range(&tree, &b"b"[..]..&b"d"[..])?)?, ["b", "c"]);
    assert_eq!(keys(read.range(&tree, &b"b"[..]..=&b"d"[..])?)?, ["b", "c", "d"]);
    assert_eq!(keys(read.range(&tree, ..&b"c"[..])?)?, ["a", "b"]);
    assert_eq!(keys(read.range(&tree, &b"bb"[..]..&b"e"[..])?)?, ["c", "d"]); // "bb" is not a key
    let after_d = (Bound::Excluded(&b"d"[..]), Bound::Included(&b"user:1"[..]));
    assert_eq!(keys(read.range(&tree, after_d)?)?, ["e", "user:1"]);

    // From the back, and from both ends at once.
    assert_eq!(keys(read.range(&tree, ..&b"d"[..])?.rev())?, ["c", "b", "a"]);
    let mut scan = read.range(&tree, ..&b"f"[..])?;
    assert_eq!(scan.next().transpose()?, Some((&b"a"[..], &b"A"[..])));
    assert_eq!(scan.next_back().transpose()?, Some((&b"e"[..], &b"E"[..])));
    assert_eq!(scan.count(), 3); // b, c and d: each entry comes out once

    // A prefix, alone or with bounds.
    assert_eq!(keys(read.range_prefix(&tree, b"user:", ..)?)?, ["user:1", "user:2", "user:3"]);
    assert_eq!(keys(read.range_prefix(&tree, b"user:", &b"user:2"[..]..)?.rev())?, ["user:3", "user:2"]);

    // Empty intervals are empty scans; an empty bound is an invalid size.
    assert!(keys(read.range(&tree, &b"d"[..]..&b"b"[..])?)?.is_empty());
    assert_eq!(read.range(&tree, &b""[..]..).err().map(|e| e.kind()), Some(ErrorKind::InvalidSize));
    Ok(())
}
```

## Removing an interval

`remove_range(&tree, bounds)` removes every entry whose key is within `bounds` and returns how many it removed;
`remove_range_prefix(&tree, prefix, bounds)` limits it to the keys that start with `prefix`. The bounds and sizes
follow the scans: bounds that leave no key between them remove nothing. The tree keeps its identity and its handles,
and the pages it no longer uses are freed. To remove every entry, `OpenMode::Empty` is simpler.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let events = write.open_tree_with("events", OpenMode::OpenOrCreate)?;
    for day in 1..=9u8 {
        write.set(&events, format!("2026-11-0{day}").as_bytes(), b"event")?;
        write.set(&events, format!("draft-{day}").as_bytes(), b"draft")?;
    }
    assert_eq!(write.len(&events)?, 18);

    // Days 1 to 3 inclusive.
    assert_eq!(write.remove_range(&events, &b"2026-11-01"[..]..=&b"2026-11-03"[..])?, 3);
    // Every key that starts with "draft-".
    assert_eq!(write.remove_range_prefix(&events, b"draft-", ..)?, 9);
    // An empty interval removes nothing.
    assert_eq!(write.remove_range(&events, &b"z"[..]..&b"a"[..])?, 0);
    assert_eq!(write.len(&events)?, 6);
    write.commit()?;
    Ok(())
}
```

## What a read borrows, and for how long

`get` and the scans return `&[u8]` slices that point into the engine's memory, not copies. Their lifetime depends on
the transaction:

| Transaction | The references and scans live | A scan can be |
|---|---|---|
| Read | As long as the transaction | Sent to another thread, like the transaction |
| Write | Until the next operation that may modify the transaction, in any tree | Used only on the transaction's thread |

A write transaction's methods that may modify take `&mut self`: `set`, `try_add`, `remove`, `remove_range`, the
openings with a mode, `rename_tree` and `delete_tree`. The compiler ends the earlier references at each of them, even
when the operation fails or changes nothing. `get`, `len`, the scans, `open_tree`, `tree_exists`, `tree_kind` and
`list_trees` take `&self` and keep them. To keep a value across a change, copy it (`to_vec()`); to change entries
while scanning, collect the keys first.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let stock = write.open_tree_with("stock", OpenMode::OpenOrCreate)?;
    write.set(&stock, b"apples", b"3")?;
    write.set(&stock, b"pears", b"0")?;

    // Several reads can be held together: none of them modifies.
    let apples = write.get(&stock, b"apples")?;
    let pears = write.get(&stock, b"pears")?;
    assert_eq!((apples, pears), (Some(&b"3"[..]), Some(&b"0"[..])));

    // Copy a value to keep it across a change.
    let before = write.get(&stock, b"apples")?.map(<[u8]>::to_vec);
    write.set(&stock, b"apples", b"4")?;
    assert_eq!(before.as_deref(), Some(&b"3"[..]));

    // Collect the keys, then change the entries.
    let empty: Vec<Vec<u8>> = write
        .range(&stock, ..)?
        .filter_map(|entry| match entry {
            Ok((key, value)) if value == b"0" => Some(Ok(key.to_vec())),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<_, _>>()?;
    for key in &empty {
        write.remove(&stock, key)?;
    }
    write.commit()?;

    // In a read transaction, references live as long as the transaction.
    let read = store.read()?;
    let stock = read.open_tree("stock")?;
    let all: Vec<(&[u8], &[u8])> = read.range(&stock, ..)?.collect::<Result<_, _>>()?;
    let apples = read.get(&stock, b"apples")?;
    assert_eq!(all, [(&b"apples"[..], &b"4"[..])]);
    assert_eq!(apples, Some(&b"4"[..]));
    Ok(())
}
```

Holding a reference across a change does not compile:

```rust,compile_fail,E0502
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let stock = write.open_tree_with("stock", OpenMode::OpenOrCreate)?;
    write.set(&stock, b"apples", b"3")?;
    let apples = write.get(&stock, b"apples")?;
    write.set(&stock, b"apples", b"4")?; // error: `write` is borrowed by `apples`
    assert_eq!(apples, Some(&b"3"[..]));
    Ok(())
}
```

See also: [Getting started](getting-started.md) · [Stores](stores.md) · [Transactions](transactions.md) ·
[Errors](errors.md) · [Multi-value trees](multi-value-trees.md) · [Fixed-size trees](fixed-size-trees.md) ·
[Tables](tables.md) · [Sequences and IDs](sequences-and-ids.md)
