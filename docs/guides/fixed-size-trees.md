# Fixed-size trees

A fixed-size tree maps unsigned 64-bit integer keys, in numeric order, to values that all have the same length, from
0 to 2,048 bytes, fixed when the tree is created, like a `BTreeMap<u64, [u8; N]>`. This guide covers opening one with
its value length, storing, adding, reading and removing entries, scans between integer bounds from both ends, interval
removal, the limits, and what happens when an opening gives another length. Use it for counters, offsets, timestamps
or any record of a fixed length keyed by an integer: its nodes hold only the 8-byte key and the value, so it is denser
than a [normal tree](trees.md) keyed by big-endian integers. Use a normal tree when keys are not integers
or values vary in length, a [multi-value tree](multi-value-trees.md) when a key has many values, and a
[table](tables.md) with a [sequence](sequences-and-ids.md) when the records have typed columns.

## Step by step

1. Open the tree in a write transaction with `open_fixed_tree_with(name, value_len, OpenMode::OpenOrCreate)`.
2. Store values of exactly `value_len` bytes with `set_fixed`, read them with `get_fixed`.
3. Scan between integer bounds with `range_fixed`.
4. Commit, and read in a read transaction opened with `open_fixed_tree(name, value_len)`.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;

    let mut write = store.write()?;
    let balances = write.open_fixed_tree_with("balances", 8, OpenMode::OpenOrCreate)?;
    write.set_fixed(&balances, 1001, &250i64.to_le_bytes())?;
    write.set_fixed(&balances, 1002, &(-40i64).to_le_bytes())?;
    write.commit()?;

    let read = store.read()?;
    let balances = read.open_fixed_tree("balances", 8)?;
    let value = read.get_fixed(&balances, 1001)?.expect("stored above");
    assert_eq!(i64::from_le_bytes(value.try_into()?), 250);
    for entry in read.range_fixed(&balances, ..)? {
        let (account, value) = entry?;
        println!("{account}: {}", i64::from_le_bytes(value.try_into()?));
    }
    Ok(())
}
```

## Opening and the value length

Every opening gives the length of the values, also a plain open and one in a read transaction. The tree's length is
fixed when it is created and kept while it exists: emptying and renaming keep it. Its handles, `ReadFixedTree` and
`WriteFixedTree`, carry that length and take only the operations of this guide.

| Opening | Transaction | Behavior |
|---|---|---|
| `open_fixed_tree(name, value_len)` | Read and write | Opens an existing fixed-size tree of that length; `ErrorKind::TreeNotFound` if it is missing |
| `open_fixed_tree_with(name, value_len, mode)` | Write | Opens, creates or empties it by `OpenMode` (`CreateNew`, `OpenOrCreate`, `CreateOrEmpty`, `Empty`); a created tree gets `value_len` |

| Case | Result |
|---|---|
| `value_len` over 2,048 | `ErrorKind::InvalidSize` of a value (`SizeSubject::Value`, minimum `0`, maximum `2048`), checked after the name and before reading the store, so it wins over not found or a different kind |
| The tree exists with another length | `ErrorKind::DifferentTreeType`, in every mode: two lengths are two kinds |
| The name is a normal or multi-value tree, a table or a sequence | `ErrorKind::DifferentTreeType` |
| The length is unknown | `tree_kind(name)` returns `Some(TreeKind::FixedSize(len))`, and `list_trees` reports it too |

The names and the catalog operations (`tree_exists`, `tree_kind`, `list_trees`, `rename_tree`, `delete_tree`) are
those of the [normal trees](trees.md#checking-listing-and-naming-trees).

```rust
use krummi::{Error, ErrorKind, OpenMode, SizeSubject, Store, StoreOptions, TreeKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    write.open_fixed_tree_with("offsets", 8, OpenMode::CreateNew)?;

    // Another length is another kind. (Handles have no `Debug`, hence `.err()`.)
    match write.open_fixed_tree_with("offsets", 4, OpenMode::OpenOrCreate).err() {
        Some(Error::DifferentTreeType { found, requested, .. }) => {
            assert_eq!(found, TreeKind::FixedSize(8));
            assert_eq!(requested, TreeKind::FixedSize(4));
        }
        other => panic!("unexpected result: {other:?}"),
    }
    let error = write.open_tree("offsets").err();
    assert_eq!(error.map(|e| e.kind()), Some(ErrorKind::DifferentTreeType));

    // A length over 2,048 is invalid by itself, even for a tree that does not exist.
    match write.open_fixed_tree("missing", 2049).err() {
        Some(Error::InvalidSize { what, received, min, max, .. }) => {
            assert_eq!(what, SizeSubject::Value);
            assert_eq!((received, min, max), (2049, 0, 2048));
        }
        other => panic!("unexpected result: {other:?}"),
    }

    // Find the length of an existing tree by its name, then open it.
    let Some(TreeKind::FixedSize(len)) = write.tree_kind("offsets")? else {
        panic!("not a fixed-size tree");
    };
    let offsets = write.open_fixed_tree("offsets", len.into())?;
    assert_eq!(write.len(&offsets)?, 0);
    write.commit()?;
    Ok(())
}
```

## Operations

| Operation | Transaction | Returns | Behavior |
|---|---|---|---|
| `set_fixed(&tree, key, value)` | Write | `()` | Stores the value, replacing the value of a key that is there |
| `try_add_fixed(&tree, key, value)` | Write | `bool` | Stores the value only if the key is not there; returns whether it did |
| `get_fixed(&tree, key)` | Read and write | `Option<&[u8]>` | The value, or `None`; with length `0`, `Some(&[])` |
| `remove_fixed(&tree, key)` | Write | `bool` | Removes the key; returns whether it was there |
| `len(&tree)` | Read and write | `u64` | The number of entries, counted without walking the tree |
| `range_fixed(&tree, bounds)` | Read and write | scan of `(u64, &[u8])` | The entries within the integer bounds, in numeric order |
| `remove_range_fixed(&tree, bounds)` | Write | `u64` | Removes the entries within the bounds; returns how many |

Keys are passed and returned as `u64`, not as bytes. Every integer from `0` to `u64::MAX` is a valid key and a valid
bound, so no key ever fails with an invalid size. The only size checked is the value's in `set_fixed` and
`try_add_fixed`: it must have exactly the tree's length, or the operation fails with `ErrorKind::InvalidSize` of a
value whose minimum and maximum are both that length, also in `try_add_fixed` when the key is already there.

```rust
use krummi::{Error, OpenMode, SizeSubject, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let counters = write.open_fixed_tree_with("counters", 8, OpenMode::OpenOrCreate)?;

    write.set_fixed(&counters, 0, &1u64.to_le_bytes())?;
    write.set_fixed(&counters, u64::MAX, &2u64.to_le_bytes())?; // every u64 is a key
    write.set_fixed(&counters, 0, &3u64.to_le_bytes())?; // replaces
    assert!(write.try_add_fixed(&counters, 7, &4u64.to_le_bytes())?);
    assert!(!write.try_add_fixed(&counters, 7, &5u64.to_le_bytes())?); // already there: unchanged
    assert_eq!(write.get_fixed(&counters, 7)?, Some(&4u64.to_le_bytes()[..]));
    assert_eq!(write.get_fixed(&counters, 8)?, None);
    assert_eq!(write.len(&counters)?, 3);

    // Increment a counter: copy the value out, then store the new one.
    let current = u64::from_le_bytes(write.get_fixed(&counters, 0)?.unwrap_or(&[0; 8]).try_into()?);
    write.set_fixed(&counters, 0, &(current + 1).to_le_bytes())?;
    assert_eq!(write.get_fixed(&counters, 0)?, Some(&4u64.to_le_bytes()[..]));

    assert!(write.remove_fixed(&counters, u64::MAX)?);
    assert!(!write.remove_fixed(&counters, u64::MAX)?);

    // A value of another length fails, also when the key is there.
    match write.try_add_fixed(&counters, 7, &[0; 4]).unwrap_err() {
        Error::InvalidSize { what, received, min, max, .. } => {
            assert_eq!(what, SizeSubject::Value);
            assert_eq!((received, min, max), (4, 8, 8));
        }
        other => panic!("unexpected error: {other}"),
    }
    write.commit()?; // the failure had no effect
    Ok(())
}
```

### Length zero: a set of integers

A tree of length `0` stores only keys. `get_fixed` still tells a key that is there, `Some(&[])`, from one that is
not, `None`.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let seen = write.open_fixed_tree_with("seen-ids", 0, OpenMode::OpenOrCreate)?;
    for id in [42, 7, 1_000_000] {
        write.set_fixed(&seen, id, &[])?;
    }
    assert_eq!(write.get_fixed(&seen, 7)?, Some(&[][..]));
    assert_eq!(write.get_fixed(&seen, 8)?, None);
    let ids: Vec<u64> = write
        .range_fixed(&seen, ..)?
        .map(|entry| entry.map(|(id, _)| id))
        .collect::<Result<_, _>>()?;
    assert_eq!(ids, [7, 42, 1_000_000]);
    write.commit()?;
    Ok(())
}
```

## Limits and order

| Item | Limits |
|---|---|
| Key | Any `u64`, `0` to `u64::MAX`, in numeric order (so `2` goes before `10`) |
| Value length | `0` to `2,048` bytes, given on every opening and fixed at creation |
| Value | Exactly the tree's length |
| Scan and removal bounds | Any `u64`; none is invalid |
| Entries | No artificial limit |

## Scans

`range_fixed(&tree, bounds)` scans the entries whose keys are within `bounds`, any `RangeBounds<u64>`: `..`, `a..b`,
`a..=b`, `a..`, `..b`, `..=b` or a pair of `std::ops::Bound`. The scan is a double-ended iterator of
`Result<(u64, &[u8])>` in numeric order: `next` takes from the front, `next_back` from the back, `rev()` reverses it,
and each entry comes out once. A bound need not be a key. Bounds that leave no integer between them give an empty
scan that reads nothing, not an error: a lower bound after the upper one, `5..5`, an excluded lower bound
`u64::MAX`, or two excluded contiguous integers such as `5` and `6`. Fixed-size trees have no prefix scan.

```rust
use std::ops::Bound;

use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let readings = write.open_fixed_tree_with("readings", 2, OpenMode::OpenOrCreate)?;
    for second in [2u64, 10, 30, 45, 60, u64::MAX] {
        write.set_fixed(&readings, second, &(second as u16).to_le_bytes())?;
    }
    write.commit()?;

    let read = store.read()?;
    let readings = read.open_fixed_tree("readings", 2)?;
    fn keys<'t>(scan: impl Iterator<Item = krummi::Result<(u64, &'t [u8])>>) -> krummi::Result<Vec<u64>> {
        scan.map(|entry| entry.map(|(key, _)| key)).collect()
    }

    assert_eq!(keys(read.range_fixed(&readings, 10..45)?)?, [10, 30]); // numeric order: 2 before 10
    assert_eq!(keys(read.range_fixed(&readings, 10..=45)?)?, [10, 30, 45]);
    assert_eq!(keys(read.range_fixed(&readings, 31..)?)?, [45, 60, u64::MAX]);
    assert_eq!(keys(read.range_fixed(&readings, ..=10)?.rev())?, [10, 2]);
    let after_30 = (Bound::Excluded(30), Bound::Unbounded);
    assert_eq!(keys(read.range_fixed(&readings, after_30)?)?, [45, 60, u64::MAX]);

    // Both ends at once.
    let mut scan = read.range_fixed(&readings, ..)?;
    assert_eq!(scan.next().transpose()?.map(|(key, _)| key), Some(2));
    assert_eq!(scan.next_back().transpose()?.map(|(key, _)| key), Some(u64::MAX));
    assert_eq!(scan.count(), 4);

    // Empty intervals.
    assert!(keys(read.range_fixed(&readings, 45..10)?)?.is_empty());
    let none = (Bound::Excluded(u64::MAX), Bound::Unbounded);
    assert!(keys(read.range_fixed(&readings, none)?)?.is_empty());
    Ok(())
}
```

## Removing an interval

`remove_range_fixed(&tree, bounds)` removes every entry whose key is within `bounds` and returns how many it removed.
The bounds follow `range_fixed`: bounds that leave no integer between them remove nothing. The tree keeps its
identity, its length and its handles, and the pages it no longer uses are freed. To remove every entry,
`OpenMode::Empty` is simpler.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let events = write.open_fixed_tree_with("events", 4, OpenMode::OpenOrCreate)?;
    for timestamp in 1..=100u64 {
        write.set_fixed(&events, timestamp, &(timestamp as u32).to_le_bytes())?;
    }

    assert_eq!(write.remove_range_fixed(&events, ..=50)?, 50); // everything up to 50
    assert_eq!(write.remove_range_fixed(&events, 91..)?, 10); // 91 to 100
    assert_eq!(write.remove_range_fixed(&events, 80..80)?, 0); // an empty interval
    assert_eq!(write.len(&events)?, 40);

    // Emptying and renaming keep the length.
    write.rename_tree("events", "old-events", false)?;
    let emptied = write.open_fixed_tree_with("old-events", 4, OpenMode::Empty)?;
    assert_eq!(write.len(&emptied)?, 0);
    assert_eq!(write.tree_kind("old-events")?, Some(krummi::TreeKind::FixedSize(4)));
    write.commit()?;
    Ok(())
}
```

## References and changes

`get_fixed` and the scans return values that point into the engine's memory, not copies, with the lifetimes of a
[normal tree](trees.md#what-a-read-borrows-and-for-how-long): in a read transaction they live as long as the
transaction; in a write transaction, until the next operation that may modify it. `set_fixed`, `try_add_fixed`,
`remove_fixed`, `remove_range_fixed` and the openings with a mode take `&mut self` and end them, even when they change
nothing; storing the value a key already has is still a change. `get_fixed`, `len` and `range_fixed` take `&self`.
Decode or copy a value (as the counter example above does) before changing the tree.

See also: [Trees](trees.md) · [Multi-value trees](multi-value-trees.md) · [Transactions](transactions.md) ·
[Errors](errors.md) · [Sequences and IDs](sequences-and-ids.md) · [Tables](tables.md)
