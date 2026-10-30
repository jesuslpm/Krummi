# Krummi

An embedded, transactional storage engine in Rust, inspired by [Voron](https://github.com/ravendb/ravendb),
the storage engine of RavenDB. The name follows Voron's: *voron* is Russian for raven, and *krummi* is the
Icelandic nickname for the raven.

Implemented so far: module 1, the **Pager** ([PRD](docs/prd/krummi/pager.prd.en.md)), module 2, the
**transactions** ([PRD](docs/prd/krummi/transactions.prd.en.md)), and module 3, the **journal and recovery**
([PRD](docs/prd/krummi/journal.prd.en.md)), with grouped and direct journal writes
([PRD](docs/prd/krummi/journal-group-commit.prd.en.md)), module 4, **free space**
([PRD](docs/prd/krummi/free-space.prd.en.md)), and the first four parts of module 5, **trees and key operations**,
**in-order scan and prefix search**, **large values** and **multi-value trees**
([module PRD](docs/prd/krummi/trees.prd.en.md), [PRD](docs/prd/krummi/trees-keys.prd.en.md),
[PRD](docs/prd/krummi/trees-scan.prd.en.md), [PRD](docs/prd/krummi/trees-large-values.prd.en.md),
[PRD](docs/prd/krummi/trees-multivalue.prd.en.md)). An application can create, open, inspect and close stores, on disk
or in memory, and open read transactions from any number of threads and one write transaction at a time, which the
others wait for in order of arrival. A commit that returns survives a crash or a power cut, and opening a store after
a crash recovers it. Inside a transaction it opens named trees and stores, adds, reads and removes entries by key,
with values of up to 256 MiB that reads return as one sequence of bytes without copying them, scans them in key order
from both ends, by interval and by prefix, and removes whole intervals. It also opens multi-value trees, where each
key holds a sorted set of values of up to 2,048 bytes, the empty value included, with their own operations and scans.
The product and its split into modules are in
the [root PRD](docs/prd/krummi/krummi.prd.en.md).
Every product change starts with a PRD in `docs/prd/krummi/` (see `CLAUDE.md`).

```rust
use std::time::Duration;
use krummi::{OpenMode, Store, StoreOptions};

let store = Store::open("data/orders", &StoreOptions::default())?;
println!("{:?}", store.info()); // format version, page size, size, pages, maximum size, temporary, free pages
let read = store.read()?; // a snapshot: sees every write committed before it opened, none after
let write = store.write_with_timeout(Duration::from_secs(5))?; // or store.write() to wait without limit
write.commit()?; // durable when it returns, and visible to the reads opened from now on
store.write()?.commit_deferred()?; // returns at once; durable, and visible to reads, a disk write later
store.make_durable()?; // waits until every commit made so far is durable
let mut write = store.write()?;
let tree = write.open_tree_with("orders", OpenMode::OpenOrCreate)?; // like .NET's FileMode
write.set(&tree, b"order-1", b"two ravens")?; // like a Dictionary: set, try_add, get, remove, len
write.set(&tree, b"order-2", &vec![7u8; 8 << 20])?; // values up to 256 MiB, kept in contiguous pages
let big = write.get(&tree, b"order-2")?.unwrap(); // the whole 8 MiB as one slice, not copied
assert_eq!(big.len(), 8 << 20);
for entry in write.range_prefix(&tree, b"order-", ..)?.rev() { // like BTreeMap::range, from either end
    let (key, value) = entry?; // borrowed from the engine, not copied
    println!("{key:?} {value:?}");
}
write.remove_range(&tree, &b"order-0"[..]..&b"order-5"[..])?; // returns how many entries it removed
let tags = write.open_multi_tree_with("tags", OpenMode::OpenOrCreate)?; // each key holds a sorted set of values
assert!(write.add(&tags, b"order-1", b"urgent")?); // whether it added; contains, value_count, remove_value, remove_key
for value in write.values(&tags, b"order-1", ..)? { // a key's values in order; keys(..) scans keys with their counts
    println!("{:?}", value?);
}
write.commit()?;
drop(read);
store.close()?; // moves everything into krummi.data and removes the journal
```

A store is a folder with one file, `krummi.data`, made of 8 KiB pages. While it is open, the folder may also hold
the journal files `krummi.journal.<number>` and the `krummi.temp` subfolder; a clean close removes them. Only one
process can have it open at a time. The `krummi` crate is the public API; `krummi-pager`, `krummi-tx` and `krummi-tree` are internal to the engine, and
`krummi-pager` is the only crate allowed to use `unsafe` code (decision D12 of the root PRD).

As in Voron, a write transaction changes copies of its pages and every page keeps its number. A commit enters the
queue of the write-ahead journal (LZ4-compressed records with checksums) and releases the write turn; the journal's
own thread writes every queued commit in one write, directly to the disk when the file system allows it, so commits
from several threads share one wait for the disk (group commit). The next writer sees a queued commit at once; reads
see it once it is durable, which is when a durable commit returns. `cargo run --release -p krummi-tx --example
commit_rate` measures durable commits per second.
The committed version lives in a scratch area, in mapped files of `krummi.temp`, until no open read needs the
version before it; then the store's transfer thread writes it in its place in `krummi.data`. Commits, the end of a
read and the journal making a group durable only wake that thread, which gathers them into one pass at most every
200 ms (at once when the scratch area grew by 32 MiB since the last pass), so a page many commits rewrite is
written once per pass. It flushes `krummi.data` and records in the header how far the journal has been applied only
when that releases a journal file. Opening a store after a crash replays the journal from that point.

Free space is a map of bits, one page of it per region of about 65,400 pages (511 MiB), at a fixed place in the
region, so the map never needs pages reserved through itself. Pages after the frontier were never used; the map
records the free ones before it. A write reserves single pages or exact ranges of up to a region, from the map
first and at the frontier otherwise, and frees pages for the writes that follow its commit: the readers that still
see a freed page keep its earlier version in the scratch area, so a long read never makes `krummi.data` grow. The
map pages travel through the journal like any other page; the engine reads them only when a reservation needs them,
and the store information reports the free pages without reading them. `cargo run --release -p krummi-tx --example
free_space_timing` measures reservations on a store of 1 million pages.

Trees are B+ trees of slotted 8 KiB pages, with keys of 1 to 2,048 bytes in byte order and values of up to 256 MiB.
A value whose entry would not fit twice in a leaf lives off it, in a run of contiguous pages of its own that is never
changed: overwriting it writes a new run and frees the old one, and removing it, or emptying or deleting its tree,
frees the run without reading it, so a damaged value can always be removed. Reads and scans return every value as
one contiguous sequence without copying it, also in a temporary store. A catalog, itself a B+ tree, maps each tree's name to its identity, kind, root page and number of
entries; it takes no page until the first tree is created. A changed node keeps its page number, so a write needs no
path copying, and reads return values without copying them. Opening and creating follow .NET's `FileMode` (open,
create new, open or create, create or empty, empty), and the key operations follow a .NET `Dictionary`. A page whose
layout is inconsistent is reported as damage, never trusted; every read from the data file verifies the page's
checksum, and a node whose layout already passed with that same checksum is not checked again while the store stays
open, as `cargo run --release -p krummi --example node_check_timing` measures. Scans follow Rust's `BTreeMap::range`: two bounds,
each included, excluded or open, and an optional prefix, consumed from the front or the back; a scan keeps the
path from the root at each end and reads no page until it is used, and a range removal frees the subtrees wholly
inside the interval in one descent. `cargo run --release -p krummi --example tree_timing` measures storing, reading
and removing 1 million entries, `--example scan_timing` scanning them and removing them by intervals, and
`--example value_timing` storing and reading values from 1 KiB to 8 MiB and a mixed load.

A multi-value tree is a tree whose leaf entries hold each key's values. While a key's values are few, they live
sorted in its entry; when an Add takes the key past a promotion threshold, they move to a tree of their own for that
key, outside the catalog, and they come back to the entry only when removals take it below a lower demotion
threshold, so a key at the edge does not move back and forth. With its values in a tree of their own, a key's empty
value is a flag of its entry, since tree keys are never empty. Emptying, deleting or overwriting the tree, and removing a key, free those trees too.
`cargo run --release -p krummi --example multi_timing` measures them with several pairs of thresholds and loads,
against a normal tree whose keys are the key followed by the value.

## Layout

| Path | Contents |
|---|---|
| `crates/krummi/` | Public API of the engine |
| `crates/krummi-pager/` | The Pager: paged data file, memory mapping, locking, growth, scratch area and snapshots, journal and recovery; the only crate with `unsafe` |
| `crates/krummi-tx/` | Transactions: the write turn and its queue, read and write transactions, the internal page API, free space |
| `crates/krummi-tree/` | B+ trees over the page API: slotted nodes, splits and merges, multi-value trees, and the catalog of named trees |
| `odd/tasks/` | ODD execution documents, phase logs and closing reports |
| `.github/workflows/` | Continuous integration: the engine checks on Linux and Windows |
| `CLAUDE.md` | Agent instructions: workflow, checks, closing and worktree rules |
| `.claude/skills/` | Project skills: `prd`, `vibe-coding`, `closing-report`, `worktree` |
| `.claude/hooks/` | Guard that keeps a worktree session inside its own copy |
| `AGENTS.md` | Instructions for review-only agents such as Codex: their role and a map of how Claude works here |
| `.agents/skills/` | Review skills for those agents: `review-prd`, `review-code` |
| `docs/methodology/` | The working method (PRD, vibe coding, ODD, SDD) and its slides |
| `docs/prd/krummi/` | Product PRDs (Spanish authoritative, `*.prd.en.md` translations) |
| `docs/worktrees.md` | Guide to working on several changes at once in worktree copies |
| `scripts/worktree/` | Scripts behind the `worktree` skill, with their tests |
| `scripts/slides/` | Speaker-notes audio for the methodology slides |

## Checks of the engine

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

GitHub Actions runs these checks on Linux and Windows on every push and pull request
([`.github/workflows/ci.yml`](.github/workflows/ci.yml)), plus the full AC10 sweep.

The suite runs a sample of the every-byte damage sweep (AC10), for the Pager's pages and for a store written through
transactions, with and without free space, and for a store with trees. The full sweeps reopen a store 65,536 times each and are an acceptance step that CI runs on both
systems. Locally, an antivirus that scans every modified file can make them impractically slow:

```bash
cargo test -p krummi-pager --test pages -- --ignored ac10
cargo test -p krummi-tx --test commit -- --ignored ac10
cargo test -p krummi-tx --test free_space -- --ignored ac10
cargo test -p krummi --test tree_integrity -- --ignored ac10
```

On Windows, an administrator can exclude one folder from Microsoft Defender while measuring, and point the tests'
temporary folder at it. While the exclusion exists, nothing written to that folder is scanned, so remove it as soon
as the measurement ends. The tests must reach the folder by its long path: `%TEMP%` is often a short (8.3) path, and
the exclusion did not apply to files opened through it.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/antivirus/Add-DefenderExclusion.ps1     # %TEMP%\krummi-av-bench
$oldTmp = $env:TMP; $env:TMP = "$env:LOCALAPPDATA\Temp\krummi-av-bench"
cargo test -p krummi-pager --test pages -- --ignored ac10
$env:TMP = $oldTmp
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/antivirus/Remove-DefenderExclusion.ps1
```

## Tests of the tooling

```powershell
python -m unittest discover -s .claude/hooks -p "test_*.py"
python -m unittest discover -s .claude/skills/closing-report/scripts -p "test_*.py"
python -m unittest discover -s .claude/skills/prd/scripts -p "test_*.py"
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/tests/Test-Worktree.ps1
```
