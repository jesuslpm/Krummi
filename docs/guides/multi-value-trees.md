# Multi-value trees

A multi-value tree keeps, for each key, a sorted set of distinct values, like a `BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>>`
or a .NET dictionary of `SortedSet`. This guide covers opening one, adding, checking, counting and removing values,
removing keys, scanning keys and values from both ends, removing intervals of keys or of values, and the limits that
apply. Use it when one key points to many small values: a secondary index from a color to the orders that have it,
the tags of a document, the members of a group. When each key has one value, use a [normal tree](trees.md); when the
data has typed columns, use a [table](tables.md) with [indexes](indexes.md), which maintain this kind of mapping for
you.

## Step by step

1. Open the tree in a write transaction with `open_multi_tree_with(name, OpenMode::OpenOrCreate)`.
2. Add values to a key with `add`; a key is born with its first value.
3. Check a value with `contains`, count a key's values with `value_count`, scan them with `values`.
4. Remove a value with `remove_value`, or a key with all its values with `remove_key`.
5. Commit, and read in a read transaction opened with `open_multi_tree(name)`.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;

    let mut write = store.write()?;
    let by_color = write.open_multi_tree_with("orders-by-color", OpenMode::OpenOrCreate)?;
    write.add(&by_color, b"red", b"order-1")?;
    write.add(&by_color, b"red", b"order-7")?;
    write.add(&by_color, b"blue", b"order-2")?;
    write.commit()?;

    let read = store.read()?;
    let by_color = read.open_multi_tree("orders-by-color")?;
    assert!(read.contains(&by_color, b"red", b"order-7")?);
    assert_eq!(read.value_count(&by_color, b"red")?, 2);
    let red: Vec<&[u8]> = read.values(&by_color, b"red", ..)?.collect::<Result<_, _>>()?;
    assert_eq!(red, [&b"order-1"[..], b"order-7"]);
    assert_eq!(read.len(&by_color)?, 2); // two keys: blue and red
    Ok(())
}
```

## Opening

A multi-value tree has its own openings and its own handles, `ReadMultiTree` and `WriteMultiTree`, which take only
the operations of this guide: calling `set`, `get` or `range` with them, or `add` with the handle of a normal tree,
does not compile. The modes, the names and the catalog operations are those of the [normal trees](trees.md#opening-a-tree).

| Opening | Transaction | Behavior |
|---|---|---|
| `open_multi_tree(name)` | Read and write | Opens an existing multi-value tree; `ErrorKind::TreeNotFound` if it is missing |
| `open_multi_tree_with(name, mode)` | Write | Opens, creates or empties it by `OpenMode` (`CreateNew`, `OpenOrCreate`, `CreateOrEmpty`, `Empty`) |

Opening a name that belongs to a normal tree, a fixed-size tree, a table or a sequence fails with
`ErrorKind::DifferentTreeType`, and opening a multi-value tree as a normal one does too. `tree_kind` and `list_trees`
report it as `TreeKind::MultiValue`. `tree_exists`, `rename_tree` and `delete_tree` work on it as on any tree, and
emptying or deleting it frees the pages of all its keys.

```rust
use krummi::{ErrorKind, OpenMode, Store, StoreOptions, TreeKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let tags = write.open_multi_tree_with("tags", OpenMode::CreateNew)?;
    write.add(&tags, b"doc-1", b"draft")?;
    assert_eq!(write.tree_kind("tags")?, Some(TreeKind::MultiValue));

    // The kind is checked against the catalog. (Handles have no `Debug`, hence `.err()`.)
    let error = write.open_tree("tags").err();
    assert_eq!(error.map(|e| e.kind()), Some(ErrorKind::DifferentTreeType));
    write.open_tree_with("plain", OpenMode::CreateNew)?;
    let error = write.open_multi_tree("plain").err();
    assert_eq!(error.map(|e| e.kind()), Some(ErrorKind::DifferentTreeType));

    // Emptying keeps the tree and its handles, with no keys.
    write.open_multi_tree_with("tags", OpenMode::Empty)?;
    assert_eq!(write.len(&tags)?, 0);
    write.commit()?;
    Ok(())
}
```

An operation of a normal tree with a multi-value handle is a type error:

```rust,compile_fail,E0308
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let tags = write.open_multi_tree_with("tags", OpenMode::OpenOrCreate)?;
    write.set(&tags, b"doc-1", b"draft")?; // error: expected `&WriteTree`, found `&WriteMultiTree`
    Ok(())
}
```

## Operations

| Operation | Transaction | Returns | Behavior |
|---|---|---|---|
| `add(&tree, key, value)` | Write | `bool` | Adds the value to the key, creating the key; `false` if the value was there (nothing changes) |
| `contains(&tree, key, value)` | Read and write | `bool` | Whether the key has the value; a missing key has no values |
| `value_count(&tree, key)` | Read and write | `u64` | The number of values of the key, `0` if it is missing, read without reading the values |
| `remove_value(&tree, key, value)` | Write | `bool` | Removes the value; returns whether it was there. The key goes away with its last value |
| `remove_key(&tree, key)` | Write | `u64` | Removes the key with all its values; returns how many values it had, `0` if it was missing |
| `len(&tree)` | Read and write | `u64` | The number of keys, counted without walking the tree |
| `keys(&tree, bounds)`, `keys_prefix(&tree, prefix, bounds)` | Read and write | scan of `(&[u8], u64)` | The keys within the bounds, each with its number of values |
| `values(&tree, key, bounds)`, `values_prefix(&tree, key, prefix, bounds)` | Read and write | scan of `&[u8]` | The values of one key within the bounds, in byte order |
| `remove_key_range(&tree, bounds)`, `remove_key_range_prefix(&tree, prefix, bounds)` | Write | `u64` | Removes the keys within the bounds with all their values; returns how many keys |
| `remove_value_range(&tree, key, bounds)`, `remove_value_range_prefix(&tree, key, prefix, bounds)` | Write | `u64` | Removes the values of one key within the bounds; returns how many values |

A key exists only while it has at least one value: there are no keys without values, so `value_count(key) > 0` tells
whether a key exists. None of the operations fails because the key or the value is there or not; all validate the
key first and then the value.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let groups = write.open_multi_tree_with("groups", OpenMode::OpenOrCreate)?;

    assert!(write.add(&groups, b"admins", b"ana")?);
    assert!(write.add(&groups, b"admins", b"luis")?);
    assert!(!write.add(&groups, b"admins", b"ana")?); // already there: nothing changes
    assert!(write.add(&groups, b"guests", b"eva")?);

    assert!(write.contains(&groups, b"admins", b"luis")?);
    assert!(!write.contains(&groups, b"nobody", b"luis")?); // a missing key has no values
    assert_eq!(write.value_count(&groups, b"admins")?, 2);
    assert_eq!(write.len(&groups)?, 2); // keys, not values

    // The key goes away with its last value.
    assert!(write.remove_value(&groups, b"guests", b"eva")?);
    assert!(!write.remove_value(&groups, b"guests", b"eva")?);
    assert_eq!(write.value_count(&groups, b"guests")?, 0);
    assert_eq!(write.len(&groups)?, 1);

    // remove_key returns how many values the key had.
    assert_eq!(write.remove_key(&groups, b"admins")?, 2);
    assert_eq!(write.remove_key(&groups, b"admins")?, 0);
    assert_eq!(write.len(&groups)?, 0);
    write.commit()?;
    Ok(())
}
```

## Limits and order

| Item | Minimum | Maximum | Order |
|---|---|---|---|
| Key | 1 byte | 2,048 bytes | Byte by byte, as in a [normal tree](trees.md#keys-values-and-their-limits) |
| Value | 0 bytes | 2,048 bytes | Byte by byte; the empty value goes before any other |
| Bound of a key scan | 1 byte | 2,048 bytes | Validated as a key |
| Bound of a value scan | 0 bytes | 2,048 bytes | Validated as a value: the empty value is a valid bound |
| Prefix | 0 bytes | 2,048 bytes | An empty prefix limits nothing |
| Values per key | — | No artificial limit | — |

A size outside its limits fails with `ErrorKind::InvalidSize` without effect, and the error says what it was about
(`SizeSubject::Key`, `Value` or `Prefix`): a value has minimum `0` and maximum `2048`. Every value an operation
receives is validated, also the one given to `contains` and `remove_value`. Values larger than 2,048 bytes belong in
a [normal tree](trees.md#large-values), keyed by what the multi-value tree stores.

```rust
use krummi::{Error, ErrorKind, OpenMode, SizeSubject, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let set = write.open_multi_tree_with("set", OpenMode::OpenOrCreate)?;

    // The empty value is valid, and it sorts first.
    write.add(&set, b"k", b"b")?;
    write.add(&set, b"k", b"")?;
    write.add(&set, b"k", b"a")?;
    write.add(&set, b"k", &[0xff; 2048])?; // the longest value
    let values: Vec<Vec<u8>> = write
        .values(&set, b"k", ..)?
        .map(|value| value.map(<[u8]>::to_vec))
        .collect::<Result<_, _>>()?;
    assert_eq!(values, [vec![], b"a".to_vec(), b"b".to_vec(), vec![0xff; 2048]]);

    match write.add(&set, b"k", &[0; 2049]).unwrap_err() {
        Error::InvalidSize { what, received, min, max, .. } => {
            assert_eq!(what, SizeSubject::Value);
            assert_eq!((received, min, max), (2049, 0, 2048));
        }
        other => panic!("unexpected error: {other}"),
    }
    assert_eq!(write.contains(&set, b"", b"a").unwrap_err().kind(), ErrorKind::InvalidSize); // empty key
    write.commit()?; // the failures above had no effect
    Ok(())
}
```

## Scanning keys

`keys(&tree, bounds)` scans the keys within `bounds`, each with its number of values, and `keys_prefix(&tree, prefix,
bounds)` only those that start with `prefix`. They follow the [scans of a normal tree](trees.md#scans): bounds are
any `RangeBounds<&[u8]>`, a bound need not be a key, bounds that leave no key between them give an empty scan, and the
scan is a double-ended iterator, so `rev()` and `next_back()` read it from the back. A key scan returns each key's
number of values, not the values.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let index = write.open_multi_tree_with("city", OpenMode::OpenOrCreate)?;
    for (city, person) in [("lima", "ana"), ("lima", "luis"), ("madrid", "eva"), ("malaga", "leo")] {
        write.add(&index, city.as_bytes(), person.as_bytes())?;
    }
    write.commit()?;

    let read = store.read()?;
    let index = read.open_multi_tree("city")?;
    let all: Vec<(&[u8], u64)> = read.keys(&index, ..)?.collect::<Result<_, _>>()?;
    assert_eq!(all, [(&b"lima"[..], 2), (&b"madrid"[..], 1), (&b"malaga"[..], 1)]);

    let from_the_back: Vec<(&[u8], u64)> = read.keys(&index, ..)?.rev().collect::<Result<_, _>>()?;
    assert_eq!(from_the_back[0], (&b"malaga"[..], 1));

    let starting_with_ma: Vec<(&[u8], u64)> =
        read.keys_prefix(&index, b"ma", ..)?.collect::<Result<_, _>>()?;
    assert_eq!(starting_with_ma, [(&b"madrid"[..], 1), (&b"malaga"[..], 1)]);

    let before_m: Vec<(&[u8], u64)> =
        read.keys(&index, ..&b"m"[..])?.collect::<Result<_, _>>()?;
    assert_eq!(before_m, [(&b"lima"[..], 2)]);
    Ok(())
}
```

## Scanning the values of a key

`values(&tree, key, bounds)` scans the values of one key within `bounds`, in byte order, and
`values_prefix(&tree, key, prefix, bounds)` only those that start with `prefix`. The bounds are values, so the empty
value is a valid bound, included or excluded. A key that is not there has no values: the scan is empty, not an error.
The scan is double-ended, like every other.

```rust
use std::ops::Bound;

use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let log = write.open_multi_tree_with("log", OpenMode::OpenOrCreate)?;
    for entry in ["", "2026-01-05", "2026-02-10", "2026-02-20", "2026-03-01"] {
        write.add(&log, b"server-1", entry.as_bytes())?;
    }
    write.commit()?;

    let read = store.read()?;
    let log = read.open_multi_tree("log")?;
    fn text<'t>(
        scan: impl Iterator<Item = krummi::Result<&'t [u8]>>,
    ) -> krummi::Result<Vec<String>> {
        scan.map(|value| value.map(|v| String::from_utf8_lossy(v).into_owned())).collect()
    }

    // February only, by prefix, and from the back.
    assert_eq!(text(read.values_prefix(&log, b"server-1", b"2026-02", ..)?)?, ["2026-02-10", "2026-02-20"]);
    assert_eq!(text(read.values(&log, b"server-1", ..)?.rev())?[0], "2026-03-01");

    // Bounds are values: excluding the empty value skips it.
    let not_empty = (Bound::Excluded(&b""[..]), Bound::Excluded(&b"2026-02-20"[..]));
    assert_eq!(text(read.values(&log, b"server-1", not_empty)?)?, ["2026-01-05", "2026-02-10"]);

    // A missing key has no values.
    assert!(text(read.values(&log, b"server-9", ..)?)?.is_empty());
    Ok(())
}
```

## Removing intervals

| Operation | Removes | Returns |
|---|---|---|
| `remove_key_range(&tree, bounds)` | Every key within the bounds, with all its values | How many keys |
| `remove_key_range_prefix(&tree, prefix, bounds)` | The same, only keys that start with `prefix` | How many keys |
| `remove_value_range(&tree, key, bounds)` | The values of `key` within the bounds | How many values |
| `remove_value_range_prefix(&tree, key, prefix, bounds)` | The same, only values that start with `prefix` | How many values |

The bounds and their sizes follow the scans: bounds that leave nothing between them remove nothing. A key left with
no values goes away, and a missing key removes nothing. The tree keeps its identity and its handles, and the pages it
no longer uses are freed.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let visits = write.open_multi_tree_with("visits", OpenMode::OpenOrCreate)?;
    for page in ["home", "about", "shop"] {
        for day in ["2026-11-01", "2026-11-02", "2026-11-03"] {
            write.add(&visits, page.as_bytes(), day.as_bytes())?;
        }
    }

    // Drop the visits before November 2nd from one key.
    assert_eq!(write.remove_value_range(&visits, b"home", ..&b"2026-11-02"[..])?, 1);
    assert_eq!(write.value_count(&visits, b"home")?, 2);
    // Drop every visit of "about" on a prefix: the key goes away with its last value.
    assert_eq!(write.remove_value_range_prefix(&visits, b"about", b"2026-11", ..)?, 3);
    assert_eq!(write.value_count(&visits, b"about")?, 0);

    // Drop whole keys by interval or by prefix.
    assert_eq!(write.remove_key_range(&visits, ..&b"i"[..])?, 1); // "home"; "about" is already gone
    assert_eq!(write.remove_key_range_prefix(&visits, b"sh", ..)?, 1); // "shop"
    assert_eq!(write.remove_key_range(&visits, &b"z"[..]..&b"a"[..])?, 0); // an empty interval
    assert_eq!(write.len(&visits)?, 0);
    write.commit()?;
    Ok(())
}
```

## References and changes

The keys and values a read returns point into the engine's memory, not to copies, with the lifetimes of a
[normal tree](trees.md#what-a-read-borrows-and-for-how-long): in a read transaction they live as long as the
transaction, and in a write transaction until the next operation that may modify it. `add`, `remove_value`,
`remove_key`, the two interval removals and the openings with a mode take `&mut self` and end them, even when they
change nothing; `contains`, `value_count`, `len` and the scans take `&self` and keep them. To change a tree while
scanning it, collect what the scan returns first.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let members = write.open_multi_tree_with("members", OpenMode::OpenOrCreate)?;
    for (group, user) in [("a", "ana"), ("a", "old-1"), ("b", "old-2"), ("b", "eva")] {
        write.add(&members, group.as_bytes(), user.as_bytes())?;
    }

    // Collect the keys, then remove the stale values of each one.
    let groups: Vec<Vec<u8>> = write
        .keys(&members, ..)?
        .map(|entry| entry.map(|(key, _)| key.to_vec()))
        .collect::<Result<_, _>>()?;
    let mut removed = 0;
    for group in &groups {
        removed += write.remove_value_range_prefix(&members, group, b"old-", ..)?;
    }
    assert_eq!(removed, 2);
    assert_eq!(write.len(&members)?, 2);
    write.commit()?;
    Ok(())
}
```

See also: [Trees](trees.md) · [Fixed-size trees](fixed-size-trees.md) · [Transactions](transactions.md) ·
[Errors](errors.md) · [Indexes](indexes.md) · [Tables](tables.md)
