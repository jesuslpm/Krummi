# Usage guides — ODD execution document

Objective: step-by-step usage guides of Krummi in `docs/guides/`, covering every operation and data type of the public
API, linked from `README.md`. Branch: `feat/tables-json` (the user's choice, so the guides cover the JSON columns).
Delivery: `ask-on-risk` (same branch, left unmerged). Test runner: `cargo test --workspace`; every Rust example of a
guide runs as a doctest of `krummi` (module `guides` in `crates/krummi/src/lib.rs`). Estimate: 3 h – 5 h. Real start:
2026-11-22 05:43:16 +00:00.

## Specs

- **S1** — The user's request (L1): «Crea guías de uso de Krummi paso a paso con todos los casos y operaciones y tipos
  de datos bien explicados y actualiza el README.md con enlaces a las guias.»
- **S2** — Technical docs in English (`CLAUDE.md`, *Repository rules*); CLI messages and PRDs stay in Spanish.
- **S3** — Every Rust example is a complete program that compiles and runs as a doctest, and leaves no file in the
  repository. Docs only: no change to product behavior, so no PRD (`CLAUDE.md`, *Workflow*).
- **S4** — The project's checks (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`), the Codex code review over the run's range, and the closing report.

## Tasks

| ID | Specs | Guides | Route | Status | Commit |
|---|---|---|---|---|---|
| T0 | S3 | stubs and the doctest module | inline | done | |
| T1 | S1–S3 | getting-started, stores, transactions, errors | delegated (parallel writers) | pending | |
| T2 | S1–S3 | trees, multi-value-trees, fixed-size-trees | delegated (parallel writers) | pending | |
| T3 | S1–S3 | tables, rows, sequences-and-ids | delegated (parallel writers) | pending | |
| T4 | S1–S3 | data-types, json | delegated (parallel writers) | pending | |
| T5 | S1–S3 | indexes, statistics | delegated (parallel writers) | pending | |
| T6 | S1, S2 | guide index `docs/guides/README.md` and `README.md` links | inline | pending | |
| T7 | S4 | checks, Codex review, closing report | inline | pending | |

Route evidence: T1–T5 are five independent units with disjoint edit surfaces, each several files of reading and
hundreds of lines of writing (Writer trigger (a), parallelism). The parent owns git and `lib.rs`.

## Log

- **L1** (2026-11-22, user): «Crea guías de uso de Krummi paso a paso con todos los casos y operaciones y tipos de
  datos bien explicados y actualiza el README.md con enlaces a las guias.»
- **L2** (2026-11-22, user): base «En feat/tables-json misma»; estimate 3 h – 5 h confirmed («Sí, empieza»).
- **L3** — T0: 14 stub guides and `#[cfg(doctest)] mod guides` in `crates/krummi/src/lib.rs`, each guide included with
  `include_str!`; a trial example ran as a doctest (`cargo test -p krummi --doc guides`: 1 passed).
