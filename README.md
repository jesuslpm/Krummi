# MuninnDB

An embedded, transactional storage engine in Rust, inspired by [Voron](https://github.com/ravendb/ravendb),
the storage engine of RavenDB. The name follows Voron's: *voron* is Russian for raven, and Muninn is the raven
of memory in Norse mythology.

Implemented so far: module 1, the **Pager** ([PRD](docs/prd/muninndb/pager.prd.md)). An application can create,
open, inspect and close stores, on disk or in memory; storing data arrives with the transactions of module 2. The
product and its split into modules are in the [root PRD](docs/prd/muninndb/muninndb.prd.md). Every product change
starts with a PRD in `docs/prd/muninndb/` (see `CLAUDE.md`).

```rust
use muninndb::{Store, StoreOptions};

let store = Store::open("data/orders", &StoreOptions::default())?;
println!("{:?}", store.info()); // format version, page size, size, pages, maximum size, temporary
store.close()?;
```

A store is a folder with one file, `muninn.data`, made of 8 KiB pages. Only one process can have it open at a
time. The `muninndb` crate is the public API; `muninndb-pager` is internal to the engine and is the only crate
allowed to use `unsafe` code (decision D12 of the root PRD).

## Layout

| Path | Contents |
|---|---|
| `crates/muninndb/` | Public API of the engine |
| `crates/muninndb-pager/` | The Pager: paged data file, memory mapping, locking, growth; the only crate with `unsafe` |
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
an acceptance step; run it on Linux, where an antivirus does not scan every modified file:

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
