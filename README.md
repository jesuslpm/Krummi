# MuninnDB

An embedded, transactional storage engine in Rust, inspired by [Voron](https://github.com/ravendb/ravendb),
the storage engine of RavenDB. The name follows Voron's: *voron* is Russian for raven, and Muninn is the raven
of memory in Norse mythology.

Implemented so far: module 1, the **Pager** ([PRD](docs/prd/muninndb/pager.prd.md)), and module 2, the
**transactions** ([PRD](docs/prd/muninndb/transactions.prd.md)). An application can create, open, inspect and close
stores, on disk or in memory, and open read transactions from any number of threads and one write transaction at a
time, which the others wait for in order of arrival. Storing data in them arrives with the data structures of the
next modules. The product and its split into modules are in the [root PRD](docs/prd/muninndb/muninndb.prd.md).
Every product change starts with a PRD in `docs/prd/muninndb/` (see `CLAUDE.md`).

```rust
use std::time::Duration;
use muninndb::{Store, StoreOptions};

let store = Store::open("data/orders", &StoreOptions::default())?;
println!("{:?}", store.info()); // format version, page size, size, pages, maximum size, temporary
let read = store.read()?; // a snapshot: sees every write committed before it opened, none after
let write = store.write_with_timeout(Duration::from_secs(5))?; // or store.write() to wait without limit
write.commit()?; // visible to the reads opened from now on
drop(read);
store.close()?; // writes what was committed; a crash before closing may lose it until module 3
```

A store is a folder with one file, `muninn.data`, made of 8 KiB pages. Only one process can have it open at a
time. The `muninndb` crate is the public API; `muninndb-pager` and `muninndb-tx` are internal to the engine, and
`muninndb-pager` is the only crate allowed to use `unsafe` code (decision D12 of the root PRD).

As in Voron, a write transaction changes copies of its pages and every page keeps its number: a committed version
lives in a scratch area in memory while an open read still needs the version before it, and is then written in its
place in `muninn.data`. Until the journal of module 3, a commit is not durable: what was committed reaches
`muninn.data` at the latest when the store closes or is dropped.

## Layout

| Path | Contents |
|---|---|
| `crates/muninndb/` | Public API of the engine |
| `crates/muninndb-pager/` | The Pager: paged data file, memory mapping, locking, growth, scratch area and snapshots; the only crate with `unsafe` |
| `crates/muninndb-tx/` | Transactions: the write turn and its queue, read and write transactions, the internal page API |
| `odd/tasks/` | ODD execution documents, phase logs and closing reports |
| `.github/workflows/` | Continuous integration: the engine checks on Linux and Windows |
| `CLAUDE.md` | Agent instructions: workflow, checks, closing and worktree rules |
| `.claude/skills/` | Project skills: `prd`, `vibe-coding`, `closing-report`, `worktree` |
| `.claude/hooks/` | Guard that keeps a worktree session inside its own copy |
| `docs/methodology/` | The working method (PRD, vibe coding, ODD, SDD) and its slides |
| `docs/prd/muninndb/` | Product PRDs (Spanish authoritative, `*.prd.en.md` translations) |
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

The suite runs a sample of the every-byte damage sweep (AC10). The full sweep reopens a store 65,536 times and is
an acceptance step that CI runs on both systems (about 1.5 minutes each). Locally, an antivirus that scans every
modified file can make it impractically slow:

```bash
cargo test -p muninndb-pager --test pages -- --ignored ac10
```

## Tests of the tooling

```powershell
python -m unittest discover -s .claude/hooks -p "test_*.py"
python -m unittest discover -s .claude/skills/closing-report/scripts -p "test_*.py"
python -m unittest discover -s .claude/skills/prd/scripts -p "test_*.py"
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/tests/Test-Worktree.ps1
```
