# Krummi

An embedded, transactional storage engine in Rust, inspired by [Voron](https://github.com/ravendb/ravendb),
the storage engine of RavenDB. The name follows Voron's: *voron* is Russian for raven, and *krummi* is the
Icelandic nickname for the raven.

Implemented so far: module 1, the **Pager** ([PRD](docs/prd/krummi/pager.prd.md)), module 2, the
**transactions** ([PRD](docs/prd/krummi/transactions.prd.md)), and module 3, the **journal and recovery**
([PRD](docs/prd/krummi/journal.prd.md)), with grouped and direct journal writes
([PRD](docs/prd/krummi/journal-group-commit.prd.md)). An application can create, open, inspect and close stores, on disk or in
memory, and open read transactions from any number of threads and one write transaction at a time, which the others
wait for in order of arrival. A commit that returns survives a crash or a power cut, and opening a store after a
crash recovers it. Storing data in transactions arrives with the data structures of the next modules. The product and its split into modules are in the [root PRD](docs/prd/krummi/krummi.prd.md).
Every product change starts with a PRD in `docs/prd/krummi/` (see `CLAUDE.md`).

```rust
use std::time::Duration;
use krummi::{Store, StoreOptions};

let store = Store::open("data/orders", &StoreOptions::default())?;
println!("{:?}", store.info()); // format version, page size, size, pages, maximum size, temporary
let read = store.read()?; // a snapshot: sees every write committed before it opened, none after
let write = store.write_with_timeout(Duration::from_secs(5))?; // or store.write() to wait without limit
write.commit()?; // durable when it returns, and visible to the reads opened from now on
store.write()?.commit_deferred()?; // returns at once; durable, and visible to reads, a disk write later
store.make_durable()?; // waits until every commit made so far is durable
drop(read);
store.close()?; // moves everything into krummi.data and removes the journal
```

A store is a folder with one file, `krummi.data`, made of 8 KiB pages. While it is open, the folder may also hold
the journal files `krummi.journal.<number>` and the `krummi.temp` subfolder; a clean close removes them. Only one
process can have it open at a time. The `krummi` crate is the public API; `krummi-pager` and `krummi-tx` are internal to the engine, and
`krummi-pager` is the only crate allowed to use `unsafe` code (decision D12 of the root PRD).

As in Voron, a write transaction changes copies of its pages and every page keeps its number. A commit enters the
queue of the write-ahead journal (LZ4-compressed records with checksums) and releases the write turn; the journal's
own thread writes every queued commit in one write, directly to the disk when the file system allows it, so commits
from several threads share one wait for the disk (group commit). The next writer sees a queued commit at once; reads
see it once it is durable, which is when a durable commit returns. `cargo run --release -p krummi-tx --example
commit_rate` measures durable commits per second.
The committed version lives in a scratch area, in mapped files of `krummi.temp`, until no open read needs the
version before it; then it is written in its place in `krummi.data` and the header records how far the journal has
been applied. Opening a store after a crash replays the journal from that point.

## Layout

| Path | Contents |
|---|---|
| `crates/krummi/` | Public API of the engine |
| `crates/krummi-pager/` | The Pager: paged data file, memory mapping, locking, growth, scratch area and snapshots, journal and recovery; the only crate with `unsafe` |
| `crates/krummi-tx/` | Transactions: the write turn and its queue, read and write transactions, the internal page API |
| `odd/tasks/` | ODD execution documents, phase logs and closing reports |
| `.github/workflows/` | Continuous integration: the engine checks on Linux and Windows |
| `CLAUDE.md` | Agent instructions: workflow, checks, closing and worktree rules |
| `.claude/skills/` | Project skills: `prd`, `vibe-coding`, `closing-report`, `worktree` |
| `.claude/hooks/` | Guard that keeps a worktree session inside its own copy |
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
transactions. The full sweeps reopen a store 65,536 times each and are an acceptance step that CI runs on both
systems. Locally, an antivirus that scans every modified file can make them impractically slow:

```bash
cargo test -p krummi-pager --test pages -- --ignored ac10
cargo test -p krummi-tx --test commit -- --ignored ac10
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
