# Transactions

Every read and every change in Krummi happens inside a transaction. This guide covers read transactions (snapshots
that any number of threads can share), the single write transaction (how you get it, how long you wait, how you
commit it durably or deferred, how you roll it back), what each transaction sees, how long the values you read
live, and how to share a store between threads. Read it before you write code that reads and changes data at the
same time.

## Steps

1. Open a read transaction with `store.read()` when you only read.
2. Open the write transaction with `store.write()` (or `write_with_timeout`) when you change data.
3. Make the changes through the write transaction.
4. Commit it with `commit()` (durable) or `commit_deferred()`; or drop it to roll it back.
5. Drop the read transactions when you are done with them.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;

    // 1. A read opened now sees the store as it is now: empty.
    let before = store.read()?;

    // 2 and 3. The write transaction.
    let mut write = store.write()?;
    let tree = write.open_tree_with("accounts", OpenMode::OpenOrCreate)?;
    write.set(&tree, b"alice", &100u64.to_le_bytes())?;
    write.set(&tree, b"bob", &50u64.to_le_bytes())?;

    // 4. Commit: both changes apply together.
    write.commit()?;

    // The read opened before keeps its snapshot; a new one sees the commit.
    assert!(!before.tree_exists("accounts")?);
    let after = store.read()?;
    let tree = after.open_tree("accounts")?;
    assert_eq!(after.len(&tree)?, 2);

    // 5. End the reads.
    drop(before);
    drop(after);
    store.close()?;
    Ok(())
}
```

## Read transactions

`store.read()` opens a read transaction: a snapshot of the store with every write committed before it opened and
none after.

- There is no limit on how many are open at once; it fails only when memory runs out (`ErrorKind::OutOfMemory`).
- Reads never wait for the writer, and the writer never waits for reads.
- A read transaction ends when you drop it. Finishing a read is ceasing to use it; there is no commit.
- It is `Send` and `Sync`: you can move it to another thread, or share it between several, which all see the same
  snapshot. The trees and tables you open in it (`ReadTree`, `ReadTable`, and so on) can cross threads too.
- A thread with an open read can open the write transaction of the same store; the read keeps its snapshot.
- While a read is open, the older versions of the pages that later commits change are kept (in `krummi.temp` on
  disk), so a read that stays open a long time on a store that keeps changing makes that space grow. It is freed
  when the read ends. A long read never makes `krummi.data` grow.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let tree = write.open_tree_with("t", OpenMode::OpenOrCreate)?;
    write.set(&tree, b"k", b"first")?;
    write.commit()?;

    let snapshot = store.read()?;

    // A change committed after the snapshot opened.
    let mut write = store.write()?;
    let tree = write.open_tree("t")?;
    write.set(&tree, b"k", b"second")?;
    write.commit()?;

    // Several threads share the snapshot, and all of them see "first".
    let tree = snapshot.open_tree("t")?;
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                assert_eq!(snapshot.get(&tree, b"k").unwrap(), Some(&b"first"[..]));
            });
        }
    });

    let now = store.read()?;
    let tree = now.open_tree("t")?;
    assert_eq!(now.get(&tree, b"k")?, Some(&b"second"[..]));
    drop((snapshot, now));
    store.close()?;
    Ok(())
}
```

## The write transaction

A store has at most one write transaction open at a time. `store.write()` asks for it:

- While another one is open, or other requests are already waiting, the request waits for its turn, without
  limit. Requests are served strictly in order of arrival; none overtakes another.
- If this thread already has a write transaction open on this store, it fails at once with `ErrorKind::Misuse`
  (it would wait for itself). The open one is not affected. A write on another store does not count.
- If the store refuses writes after a failed write, it fails at once with `ErrorKind::Io`.

The write transaction stays on the thread that opened it: it is neither `Send` nor `Sync`, so the compiler refuses
to move or share it. The same holds for the trees and tables opened in it (`WriteTree`, `WriteTable`, and so on).
The store may also run an internal write of its own (it saves table statistics), which takes its turn in the same
queue.

### Waiting with a limit

`store.write_with_timeout(limit)` waits at most `limit`:

| Limit | What it does |
|---|---|
| `Duration::ZERO` | Gets the write transaction only if none is open and nobody is waiting; otherwise fails at once with `ErrorKind::Timeout` without joining the queue. |
| Up to 24 hours | Always accepted. If the turn does not come within the limit, fails with `ErrorKind::Timeout`, leaves the queue and has no effect. |
| Too large to count from now (for example `Duration::MAX`) | Fails at once with `ErrorKind::Misuse`, without joining the queue. |

A request that gives up leaves the queue without changing the order of the others. When several of these
failures apply at once, the first wins: misuse, then the store refusing writes (`Io`), then a zero-limit timeout.
`Error::Timeout` carries the `limit` that was asked for.

```rust
use std::time::Duration;
use krummi::{Error, ErrorKind, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let write = store.write()?; // this thread holds the turn

    // A second request from the same thread is misuse.
    assert_eq!(store.write().err().unwrap().kind(), ErrorKind::Misuse);

    // Another thread asks with a limit and gives up.
    let shared = &store;
    std::thread::scope(|s| {
        let now = s.spawn(|| shared.write_with_timeout(Duration::ZERO).err().unwrap());
        assert_eq!(now.join().unwrap().kind(), ErrorKind::Timeout);

        let limit = Duration::from_millis(20);
        let later = s.spawn(move || shared.write_with_timeout(limit).err().unwrap());
        match later.join().unwrap() {
            Error::Timeout { limit: asked, .. } => assert_eq!(asked, limit),
            other => panic!("expected a timeout, got {other}"),
        }
    });

    write.commit()?; // the turn is free again
    let error = store.write_with_timeout(Duration::MAX).err().unwrap();
    assert_eq!(error.kind(), ErrorKind::Misuse);
    store.write_with_timeout(Duration::from_secs(24 * 60 * 60))?.commit()?;
    store.close()?;
    Ok(())
}
```

## What a transaction sees

| Transaction | Sees |
|---|---|
| A read | Every commit that was durable when it opened, and nothing later. |
| The write | Every commit that passed its point of no return before it got its turn (not when it asked for it), deferred ones included, plus its own changes. |
| Any transaction opened after `commit()` returned, on any thread | That commit. |
| A read opened while a commit is in progress | All of that commit or none of it. |
| A read after `commit_deferred()` returned | That commit only once it is durable, also on the same thread. |

Inside the write transaction, every handle of the same tree or table sees the same state, with the changes made
through any of them.

## Committing

`write.commit()` commits durably:

- It returns once the changes survive a crash of the process or the system and a power cut, and they are then
  visible to every transaction opened after it returns.
- All the changes apply together, or none of them.
- The next write transaction may start (and see them) while this one waits for the disk; commits that wait at the
  same time share one write of the journal.
- A transaction without changes writes nothing; in durable mode it waits until the commits it saw are durable.
- `commit` takes the transaction by value: after it, the transaction is gone, and the compiler refuses to use it,
  commit it or abort it again.

### When a commit fails

Some failures inside a write transaction make it impossible to commit. If an operation fails with
`ErrorKind::DiskFull`, `MaxSize`, `OutOfMemory`, `Damaged` or `Io`, you can still try other operations, but
`commit` fails with the class of the first of those failures, applies nothing and ends the transaction. Except
after an `Io` failure when writing, the store keeps accepting writes.

Validation failures are not like that: they have no effect and do not prevent the commit. They are
`InvalidSize`, `DifferentTreeType`, `TreeNotFound`, `TreeAlreadyExists`, `InvalidValue`, `DuplicateKey`,
`DifferentSchema`, `InvalidSchema`, and `Misuse` for a handle opened in another transaction.

Once a commit waits for the disk it can only fail with `ErrorKind::Io`. Then the store refuses writes, and the
commit may still be there, whole, after the close or the next opening: its outcome is unknown, never partial.

```rust
use krummi::{ErrorKind, OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let tree = write.open_tree_with("t", OpenMode::OpenOrCreate)?;
    write.set(&tree, b"good", b"kept")?;

    // A key of more than 2,048 bytes is a validation failure: no effect, the commit still works.
    let error = write.set(&tree, &[0u8; 3000], b"v").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidSize);
    write.commit()?;

    let read = store.read()?;
    let tree = read.open_tree("t")?;
    assert_eq!(read.get(&tree, b"good")?, Some(&b"kept"[..]));
    drop(read);
    store.close()?;
    Ok(())
}
```

## Deferred commits

`write.commit_deferred()` commits without waiting for the disk:

- It returns at once. The next write transaction sees the changes at once; read transactions see them once they
  are durable, which is usually within one write of the disk.
- It becomes durable together with every commit before it, as soon as a durable commit made at the same point
  would. `store.make_durable()` waits until every commit that returned before it is durable.
- Until it is durable, a crash (even of the process alone) or a power cut may lose it, always together with every
  later commit and never in part.
- It may wait, keeping the write turn, while the journal's queue is full.
- In a temporary store it is the same as `commit`.

Use it when you can afford to lose the last commits in a crash, in exchange for not waiting for the disk on each
one; call `make_durable` at the points where you need them safe.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = std::env::temp_dir().join(format!(
        "krummi-guide-transactions-deferred-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
    ));
    let store = Store::open(&folder, &StoreOptions::default())?;

    let mut write = store.write()?;
    let tree = write.open_tree_with("events", OpenMode::OpenOrCreate)?;
    write.set(&tree, b"e1", b"clicked")?;
    write.commit_deferred()?;

    // The next write sees it at once.
    let mut write = store.write()?;
    let tree = write.open_tree("events")?;
    assert_eq!(write.get(&tree, b"e1")?, Some(&b"clicked"[..]));
    write.set(&tree, b"e2", b"scrolled")?;
    write.commit_deferred()?;

    // Once durable, both survive a crash, and every read opened from now on sees them.
    store.make_durable()?;
    let read = store.read()?;
    let tree = read.open_tree("events")?;
    assert_eq!(read.len(&tree)?, 2);
    drop(read);

    store.close()?;
    std::fs::remove_dir_all(&folder)?;
    Ok(())
}
```

## Rolling back

A write transaction that you abort with `write.abort()`, or simply drop without committing, applies none of its
changes, and the next request in the queue gets its turn. The same happens if the thread that holds it panics
(before the commit's point of no return). The store stays usable; you need to do nothing else. If the transaction
made the store grow, the file keeps its new size.

```rust
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;

    let mut write = store.write()?;
    write.open_tree_with("draft", OpenMode::OpenOrCreate)?;
    write.abort(); // explicit

    {
        let mut write = store.write()?;
        let tree = write.open_tree_with("draft", OpenMode::OpenOrCreate)?;
        write.set(&tree, b"k", b"v")?;
        // dropped here without commit: rolled back
    }

    let read = store.read()?;
    assert!(!read.tree_exists("draft")?);
    drop(read);
    store.close()?;
    Ok(())
}
```

## Borrowed values and their lifetimes

Reads return values without copying them: `get` returns a `&[u8]` that points into the engine's memory (the
memory-mapped data file, the temporary area or the memory of a temporary store), and a row read from a table
(`TableRow`) borrows its text, bytes, vectors and JSON from it in the same way. The compiler enforces how long such
a value lives:

| Where it was read | It lives until |
|---|---|
| A read transaction | The read transaction ends. |
| The write transaction | The next operation of that transaction that may change it: any method that takes `&mut self`, such as `set`, `remove`, `insert_row`, `open_tree_with`, even if it fails or changes nothing. Opening an existing tree with `open_tree` does not end it. |

`commit`, `commit_deferred` and `abort` take the transaction by value, so a value or row borrowed from it must end
before them. When you need it longer, copy it first: `value.to_vec()` for bytes, `row.into_owned()` for a row.

This program does not compile, because `value` still borrows the transaction when `commit` takes it:

```rust,compile_fail,E0505
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let tree = write.open_tree_with("t", OpenMode::OpenOrCreate)?;
    write.set(&tree, b"k", b"v")?;
    let value = write.get(&tree, b"k")?.unwrap();
    write.commit()?; // error: cannot move out of `write` because it is borrowed
    println!("{value:?}");
    Ok(())
}
```

Copy what you need, or end the borrow, and it does:

```rust
use krummi::table::value::{ColumnType, Value};
use krummi::{Column, OpenMode, Schema, Store, StoreOptions, TableMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut write = store.write()?;
    let tree = write.open_tree_with("t", OpenMode::OpenOrCreate)?;
    write.set(&tree, b"k", b"v")?;
    let copied: Vec<u8> = write.get(&tree, b"k")?.unwrap().to_vec(); // owned: outlives the transaction

    let schema = Schema::new(
        vec![Column::new("id", ColumnType::I64).not_null(), Column::new("name", ColumnType::String)],
        &["id"],
    );
    let people = write.open_table_with("people", TableMode::OpenOrCreate(&schema))?;
    write.insert_row(&people, &[("id", Value::I64(1)), ("name", Value::String("Ada".into()))])?;
    let borrowed = write.get_row(&people, &[Value::I64(1)])?.unwrap();
    assert_eq!(borrowed.text(1).as_deref(), Some("Ada"));
    let owned = borrowed.into_owned(); // ends the borrow; the copy lives on

    write.commit()?;
    assert_eq!(copied, b"v");
    assert_eq!(owned.get("name"), Some(Value::String("Ada".into())));
    store.close()?;
    Ok(())
}
```

Scans (`range`, `values`, `range_rows`, ...) follow the same rule: an iterator from a write transaction ends at that
transaction's next operation that may change it.

## Handles belong to their transaction

A tree, table or sequence handle is tied to the transaction that opened it. Using it in another transaction fails
with `ErrorKind::Misuse` and has no effect; open it again in the new transaction. In the write transaction, a
handle of a tree or table that the same transaction deleted fails with `ErrorKind::TreeNotFound`.

```rust
use krummi::{ErrorKind, OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::open_temporary(&StoreOptions::default())?;
    let mut first = store.write()?;
    let tree = first.open_tree_with("t", OpenMode::OpenOrCreate)?;
    first.commit()?;

    let mut second = store.write()?;
    let error = second.set(&tree, b"k", b"v").unwrap_err(); // a handle of `first`
    assert_eq!(error.kind(), ErrorKind::Misuse);
    let tree = second.open_tree("t")?; // open it again
    second.set(&tree, b"k", b"v")?;
    second.commit()?;
    store.close()?;
    Ok(())
}
```

## Sharing a store between threads

`Store` is `Send` and `Sync`. Share it by reference with `std::thread::scope`, or put it in an `Arc` and clone the
`Arc` into each thread. Each thread opens its own transactions:

- Any number of threads read at the same time, each with its own read transaction or sharing one.
- Threads that write each ask for the write transaction; they get it one after another, in order of arrival, and
  commit on their own thread.
- A store cannot close while any transaction is open (it does not compile). With an `Arc`, close it after the other
  threads have dropped their clones, with `Arc::try_unwrap`.

```rust
use std::sync::Arc;
use krummi::{OpenMode, Store, StoreOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Arc::new(Store::open_temporary(&StoreOptions::default())?);

    let workers: Vec<_> = (0u8..4)
        .map(|n| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || -> krummi::Result<()> {
                let mut write = store.write()?; // waits for its turn
                let tree = write.open_tree_with("hits", OpenMode::OpenOrCreate)?;
                write.set(&tree, &[n], b"done")?;
                write.commit()
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap()?;
    }

    let read = store.read()?;
    let tree = read.open_tree("hits")?;
    assert_eq!(read.len(&tree)?, 4);
    drop(read);

    let store = Arc::try_unwrap(store).ok().expect("no other thread holds the store");
    store.close()?;
    Ok(())
}
```

## Rules and limits at a glance

| Rule | Behavior |
|---|---|
| Read transactions open at once | No limit; only memory. |
| Write transactions open at once | One per store; the others wait in order of arrival. |
| Second write on the same thread | `ErrorKind::Misuse`, at once. |
| Waiting limit | None with `write`; `write_with_timeout`: zero to 24 hours always accepted, `Duration::MAX` is misuse. |
| Read transaction across threads | Yes (`Send` and `Sync`). |
| Write transaction across threads | No; the compiler refuses it. |
| Close with a transaction open | Does not compile. |
| Drop a write without committing | Rolled back. |
| Values read in a write | Live until the next operation that may change the transaction. |
| Store refusing writes after an I/O failure | Write requests and `make_durable` fail with `ErrorKind::Io`; reads work; reopen to recover. |

## See also

[Getting started](getting-started.md) · [Stores](stores.md) · [Errors](errors.md) · [Trees](trees.md) ·
[Tables](tables.md) · [Rows](rows.md) · [Statistics](statistics.md)
