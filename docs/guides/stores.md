# Stores

A store is where Krummi keeps your data: a folder on disk, or a block of memory for tests. This guide covers every
way to open one, its options, the information it reports, the files it keeps in its folder, its lock, how it closes
and makes commits durable, what happens after a crash, how it grows and how it reuses free space. Read it when you
decide where your data lives and how your application opens and closes it.

## Steps

1. Choose a folder (or use a temporary store).
2. Open the store with `Store::open` and the `StoreOptions` you need.
3. Use it: open transactions, inspect it with `info`.
4. End every transaction.
5. Close it with `close`.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. A folder of its own.
    let folder = std::env::temp_dir().join(format!(
        "krummi-guide-stores-steps-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
    ));

    // 2. Open it: the folder does not exist, so it is created with a new store.
    let store = Store::open(&folder, &StoreOptions::default())?;

    // 3. Use it.
    let mut write = store.write()?;
    let tree = write.open_tree_with("events", OpenMode::OpenOrCreate)?;
    write.set(&tree, b"first", b"opened")?;
    write.commit()?;
    let info = store.info();
    assert!(!info.temporary);
    assert_eq!(info.page_size, 8192);

    // 4 and 5. No transaction is open, so it can close.
    store.close()?;

    // After a clean close the folder holds only the data file.
    let names: Vec<String> = std::fs::read_dir(&folder)?
        .map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()?;
    assert_eq!(names, ["krummi.data"]);

    std::fs::remove_dir_all(&folder)?;
    Ok(())
}
```

## Opening a store on disk

`Store::open(folder, &options)` opens the store in `folder`, which is anything that converts to a path (`&str`,
`String`, `&Path`, `PathBuf`).

| What is at `folder` | What `open` does |
|---|---|
| Nothing | Creates the folder, with its missing parents, and a new store in it. |
| An empty folder | Creates a new store in it. |
| A folder with a Krummi store | Opens it, recovering it first if it closed in a crash. |
| A folder with other entries and no store (hidden entries count) | Fails with `ErrorKind::NotAStore` and touches nothing. |
| A file, a folder without permission, a read-only file system, an invalid path | Fails with `ErrorKind::Access`. |

Creating a store is atomic: it is prepared in `krummi.data.new`, flushed to disk and only then renamed to
`krummi.data`, so when `open` returns, the new store survives a power cut. Folders that `open` created are kept even
if the creation fails.

When several problems hold at once, `open` reports the first of this list:

1. The options are invalid (`InvalidOptions`); nothing is touched.
2. The folder cannot be used (`Access`).
3. The store is already open (`StoreInUse`).
4. The folder is not a store and is not empty (`NotAStore`).
5. The header is damaged in both of its copies (`Damaged`).
6. The store has another format version (`DifferentFormat`). A store of another format is never recovered.
7. The rest of what it needs to open is damaged (`Damaged`).
8. Recovery after a crash fails (the class of its failure: `Damaged`, `Access`, `DiskFull`, `OutOfMemory`, `Io`).

```rust
use krummi::{ErrorKind, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = std::env::temp_dir().join(format!(
        "krummi-guide-stores-open-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
    ));

    // A folder with a file that is not part of a store is not a store.
    std::fs::create_dir_all(&folder)?;
    std::fs::write(folder.join("notes.txt"), b"not a store")?;
    let error = Store::open(&folder, &StoreOptions::default()).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::NotAStore);

    // A path that is a file cannot be used as a folder.
    let error = Store::open(folder.join("notes.txt"), &StoreOptions::default()).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::Access);

    // A missing folder, with missing parents, gets a new store.
    let nested = folder.join("a").join("b");
    let store = Store::open(&nested, &StoreOptions::default())?;
    store.close()?;
    assert!(nested.join("krummi.data").is_file());

    std::fs::remove_dir_all(&folder)?;
    Ok(())
}
```

## Opening a temporary store

`Store::open_temporary(&options)` opens a store that lives only in memory. It creates no file or folder, has no
journal and no recovery, and keeps nothing when it closes. Transactions, trees and tables behave as on disk, so it
is the usual choice for tests. Deferred commits and `make_durable` are accepted and have nothing more to do.

A temporary store reserves address space for its pages (not memory) of 64 GiB, or of its maximum size when that is
smaller; growing past 64 GiB fails with `ErrorKind::OutOfMemory`, and past the maximum size with
`ErrorKind::MaxSize`.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    assert!(store.info().temporary);

    let mut write = store.write()?;
    let tree = write.open_tree_with("t", OpenMode::OpenOrCreate)?;
    write.set(&tree, b"k", b"v")?;
    write.commit_deferred()?; // accepted, as a durable commit
    store.make_durable()?; // nothing to do

    store.close()?; // everything is gone
    Ok(())
}
```

There is no other way to open a store: `open` and `open_temporary` are the two.

## Options

`StoreOptions` has one field. `StoreOptions::default()` leaves it unset.

| Field | Type | Default | What it does |
|---|---|---|---|
| `max_size` | `Option<u64>` | `None` | The maximum size of the store in bytes while it stays open. |

Rules of `max_size`:

- It must be at least 1 MiB (1,048,576 bytes), on disk and in memory; a smaller value fails with
  `ErrorKind::InvalidOptions` before anything is touched.
- A value that is not a multiple of the page size (8,192 bytes) is rounded down. `StoreInfo::max_size` gives the
  value after rounding.
- It is not saved in the store: it holds while this opening lasts, and the next opening sets its own.
- On disk it counts the data file and the committed page versions not yet moved into it (the `krummi.temp`
  subfolder), but not the journal. In a temporary store it counts all the memory of the store.
- A growth that would pass it is cut to the maximum, and fails with `ErrorKind::MaxSize` only when the pages it needs
  do not fit. The operation fails, its write transaction can no longer commit, and the store stays usable.
- A store already larger than the maximum opens and reads normally, but cannot grow.
- Recovery after a crash restores every durable commit even if the store passes the maximum.
- Without it, a store on disk can grow up to 64 TiB, or until the disk is full. Past 64 TiB a growth fails with
  `ErrorKind::OutOfMemory`.

```rust
use krummi::{ErrorKind, OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Less than 1 MiB is not accepted.
    let error = Store::open_temporary(&StoreOptions { max_size: Some(1000) }).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::InvalidOptions);

    // Rounded down to a multiple of 8,192 bytes.
    let store = Store::open_temporary(&StoreOptions { max_size: Some((1 << 20) + 100) })?;
    assert_eq!(store.info().max_size, Some(1 << 20));

    // A value that does not fit fails with MaxSize, and so does the commit of its transaction.
    let mut write = store.write()?;
    let tree = write.open_tree_with("big", OpenMode::OpenOrCreate)?;
    let error = write.set(&tree, b"k", &vec![0u8; 2 << 20]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::MaxSize);
    assert_eq!(write.commit().unwrap_err().kind(), ErrorKind::MaxSize);

    // The store is still usable.
    let mut write = store.write()?;
    let tree = write.open_tree_with("small", OpenMode::OpenOrCreate)?;
    write.set(&tree, b"k", &[0u8; 1000])?;
    write.commit()?;
    store.close()?;
    Ok(())
}
```

## Store information

`store.info()` returns a `StoreInfo` with a snapshot of the store's state. It always works while the store is open,
also when the store refuses writes after a failed write.

| Field | Type | Meaning |
|---|---|---|
| `format_version` | `u32` | The format version of the store. A store of another version does not open (`DifferentFormat`). |
| `page_size` | `u32` | The page size in bytes: always 8,192. |
| `size_bytes` | `u64` | The size of `krummi.data` in bytes, without an incomplete final page; in a temporary store, the memory its pages take. |
| `page_count` | `u64` | The number of pages, header pages included: `size_bytes / page_size`. |
| `max_size` | `Option<u64>` | The maximum size after rounding, or `None`. |
| `temporary` | `bool` | Whether the store lives only in memory. |
| `free_pages` | `u64` | Pages a write can use without making the store grow, as of what is committed and durable. |

`StoreInfo` implements `Debug`, `Clone`, `PartialEq` and `Eq`.

```rust
use krummi::{Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let info = store.info();
    assert_eq!(info.page_size, 8192);
    assert_eq!(info.page_count * u64::from(info.page_size), info.size_bytes);
    assert!(info.size_bytes <= 1 << 20); // a new store takes at most 1 MiB
    assert_eq!(info.max_size, None);
    assert!(info.temporary);
    println!("{info:?}");
    store.close()?;
    Ok(())
}
```

## Files in the store's folder

| Name | What it is | When it is there |
|---|---|---|
| `krummi.data` | The data file: the header (in two copies), the pages of every tree and table, and the free-space map. | Always. |
| `krummi.journal.<number>` | The journal: one or more files, the number of 19 decimal digits, such as `krummi.journal.0000000000000000001`. | While the store is open and has commits; after a crash, until the next opening recovers them. |
| `krummi.temp` | A subfolder with the committed page versions not yet moved into `krummi.data`. | While the store is open and has commits; after a crash. |
| `krummi.sort` | A subfolder with the temporary sort files of an index being created. | While an index is created; after a crash. |
| `krummi.data.new` | A store being created, before it becomes `krummi.data`. | During the creation; it may stay after an interrupted one. |

These names belong to the store, which creates and deletes them. Any other file or folder in a store's folder is
foreign: Krummi ignores it and never changes or deletes it. A clean close, and the next opening after a crash,
leave only `krummi.data` (and the foreign entries). Do not copy or change these files while the store is open.

```rust
use krummi::{Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = std::env::temp_dir().join(format!(
        "krummi-guide-stores-files-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
    ));
    Store::open(&folder, &StoreOptions::default())?.close()?;

    // A foreign file in a store's folder is ignored and kept.
    std::fs::write(folder.join("README.txt"), b"my notes")?;
    let store = Store::open(&folder, &StoreOptions::default())?;
    store.close()?;
    assert_eq!(std::fs::read(folder.join("README.txt"))?, b"my notes");

    std::fs::remove_dir_all(&folder)?;
    Ok(())
}
```

## The lock

Only one `Store` can have a folder open at a time, in any process. A second `open` of the same folder fails with
`ErrorKind::StoreInUse` without touching the store, whether it comes from another process or from the same one,
and also through another path to the same folder (relative, with other capitalization on Windows, through a link).
Of several openings of the same empty or missing folder at the same time, one creates the store and the others
fail with `StoreInUse`.

The lock creates no file. Closing the store releases it, and so do dropping the `Store` and the end of the process,
even in a crash: a crash never leaves a store locked.

```rust
use krummi::{ErrorKind, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = std::env::temp_dir().join(format!(
        "krummi-guide-stores-lock-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
    ));
    let store = Store::open(&folder, &StoreOptions::default())?;
    let error = Store::open(&folder, &StoreOptions::default()).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::StoreInUse);

    drop(store); // releases the lock, like close, but without reporting a failure
    let store = Store::open(&folder, &StoreOptions::default())?;
    store.close()?;

    std::fs::remove_dir_all(&folder)?;
    Ok(())
}
```

## Closing

`store.close()` stops the store's background work, moves everything committed into `krummi.data`, flushes it to
disk, removes the journal and the temporary files, and releases the lock. After a close without error, the next
opening has nothing to recover.

- `close` takes the store by value, and every transaction borrows the store, so closing while a transaction is open
  does not compile. End every transaction first (drop the reads; commit, abort or drop the write).
- If writing fails while closing, `close` returns the error (`ErrorKind::Io`, for example) and still releases the
  lock. The journal stays, and the next opening recovers every commit.
- Dropping a `Store` does the same as `close`, but cannot report a failure. Prefer `close` when you want to know.
- Background statistics work in progress, such as a sample, is dropped; see [Statistics](statistics.md).

## Making commits durable

A durable commit (`commit`) returns only when it survives a crash. A deferred commit (`commit_deferred`) returns
before that; see [Transactions](transactions.md#deferred-commits). `store.make_durable()` returns when every commit
that returned before it was called is durable, deferred ones included.

- It does not wait for the open write transaction, does not take the write turn and has no time limit.
- With nothing pending it returns at once without writing.
- In a store that refuses writes after a failed write it fails with `ErrorKind::Io`, even with nothing pending.
- If flushing the journal fails, it fails with `ErrorKind::Io` and the store refuses writes until it is reopened.
- In a temporary store it has nothing to do.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = std::env::temp_dir().join(format!(
        "krummi-guide-stores-durable-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
    ));
    let store = Store::open(&folder, &StoreOptions::default())?;
    for i in 0u32..10 {
        let mut write = store.write()?;
        let tree = write.open_tree_with("log", OpenMode::OpenOrCreate)?;
        write.set(&tree, &i.to_be_bytes(), b"entry")?;
        write.commit_deferred()?; // returns without waiting for the disk
    }
    store.make_durable()?; // now all ten survive a crash or a power cut

    let read = store.read()?; // and reads see them
    let tree = read.open_tree("log")?;
    assert_eq!(read.len(&tree)?, 10);
    drop(read);
    store.close()?;
    std::fs::remove_dir_all(&folder)?;
    Ok(())
}
```

## Crash recovery

You never call recovery: `Store::open` does it when the store closed in a crash (of the process or of the system,
or a power cut).

- Every durable commit is there: every `commit` that returned, and every deferred commit that became durable
  through `make_durable`, a later durable commit or a close.
- Deferred commits that were not durable yet may be lost, also to a crash of the process alone. What survives is
  always a prefix of the commits: never part of a commit, and never a commit without the ones before it. A deferred
  commit usually becomes durable within one write of the disk.
- No part of a transaction that did not commit is ever there.
- A commit that was being written when the crash came is either whole or absent.
- Recovery can make `krummi.data` pass the maximum size of the opening.
- If recovery fails (lack of space or memory, a system failure), the opening fails with that class and the next
  opening repeats the whole recovery. A damaged journal fails with `ErrorKind::Damaged` without touching the store.

When opening finishes, the journal files and `krummi.temp` are gone and the store is as a clean close would leave
it.

## Size and growth

- Pages are 8 KiB (8,192 bytes), for every store; it is not configurable.
- A new store takes at most 1 MiB.
- `krummi.data` is always a multiple of 8 KiB. It grows in jumps that double its size until the jump reaches 1 GiB,
  and by at least what a write needs.
- Growing really reserves the space on disk, so a full disk is reported when growing (`ErrorKind::DiskFull`), not
  later. A growth that fails leaves the file as it was, and the store stays usable; when space is free again, writes
  work without reopening.
- The file never shrinks by itself.
- A store on disk can grow up to 64 TiB; a temporary one up to 64 GiB. `max_size` lowers either limit.

## Free space

The pages that a write frees (by removing entries, deleting or emptying trees and tables, overwriting large values)
go to a free-space map. Every write that gets the turn after that commit can reuse them, before the store grows.
Read transactions that still see the freed pages keep their old content, so a long read never makes `krummi.data`
grow, though it keeps older page versions in `krummi.temp` while it lasts.

`StoreInfo::free_pages` counts the pages a write can use without growing the store, as of what is committed and
durable: the free pages of the map plus the never-used pages at the end of the file. What open transactions, or
deferred commits that are not durable yet, free or reserve is not counted.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;

    let mut write = store.write()?;
    let tree = write.open_tree_with("blobs", OpenMode::OpenOrCreate)?;
    write.set(&tree, b"a", &vec![7u8; 1 << 20])?; // 1 MiB, in pages of its own
    write.commit()?;
    let grown = store.info();

    let mut write = store.write()?;
    let tree = write.open_tree("blobs")?;
    assert!(write.remove(&tree, b"a")?);
    write.commit()?;
    let freed = store.info();
    assert!(freed.free_pages >= 128); // the value's pages are free again
    assert_eq!(freed.size_bytes, grown.size_bytes); // the store does not shrink

    // A new value of the same size reuses them: the store does not grow.
    let mut write = store.write()?;
    let tree = write.open_tree("blobs")?;
    write.set(&tree, b"b", &vec![8u8; 1 << 20])?;
    write.commit()?;
    assert_eq!(store.info().size_bytes, grown.size_bytes);

    store.close()?;
    Ok(())
}
```

## Using a store from several threads

`Store` is `Send` and `Sync`: share it by reference (for example with `std::thread::scope`) or in an `Arc`. Read
transactions can move between threads; a write transaction stays on the thread that opened it. See
[Transactions](transactions.md#sharing-a-store-between-threads).

## When the system fails

- A full disk fails the operation with `ErrorKind::DiskFull`; the transaction applies nothing and the store stays
  open and readable.
- Any other failure to write or to flush fails with `ErrorKind::Io`, and from then on the store refuses writes:
  every write request and `make_durable` fail with `Io`, while reads keep working. Close it and open it again;
  the opening recovers every commit.
- A failure to read fails the operation with `ErrorKind::Io`; the store stays usable.
- A physical failure while reading through the memory-mapped file (an unreadable sector, a disk that goes away) ends
  the process instead of returning an error, as in Voron. If it persists, the next opening or read of that page
  reports it.
- Damage found in a page fails the operation with `ErrorKind::Damaged`; Krummi never returns damaged data as valid.

See [Errors](errors.md) for every kind.

## See also

[Getting started](getting-started.md) · [Transactions](transactions.md) · [Errors](errors.md) ·
[Trees](trees.md) · [Tables](tables.md) · [Statistics](statistics.md)
