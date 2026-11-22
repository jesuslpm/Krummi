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
| T0 | S3 | stubs and the doctest module | inline | done | `7f405ae` |
| T1 | S1–S3 | getting-started, stores, transactions, errors | delegated (parallel writers) | done | `14a9381`, `a172447` |
| T2 | S1–S3 | trees, multi-value-trees, fixed-size-trees | delegated (parallel writers) | done | `e56c581` |
| T3 | S1–S3 | tables, rows, sequences-and-ids | delegated (parallel writers) | done | `47d9ec2`, `3cdd6db` |
| T4 | S1–S3 | data-types, json | delegated (parallel writers) | done | `bdc6ddb`, `49d18d3`, `0bb936e` |
| T5 | S1–S3 | indexes, statistics | delegated (parallel writers) | done | `2fa37df` |
| T6 | S1, S2 | guide index `docs/guides/README.md` and `README.md` links | inline | done | `2e81b0f` |
| T7 | S4 | checks, registry test, RDD, Codex review, closing report | inline | done | `62e544d`, `c93259f`, `1bb6cd2`, report commit |

Route evidence: T1–T5 are five independent units with disjoint edit surfaces, each several files of reading and
hundreds of lines of writing (Writer trigger (a), parallelism). The parent owns git and `lib.rs`.

## Log

- **L1** (2026-11-22, user): «Crea guías de uso de Krummi paso a paso con todos los casos y operaciones y tipos de
  datos bien explicados y actualiza el README.md con enlaces a las guias.»
- **L2** (2026-11-22, user): base «En feat/tables-json misma»; estimate 3 h – 5 h confirmed («Sí, empieza»).
- **L3** — T0: 14 stub guides and `#[cfg(doctest)] mod guides` in `crates/krummi/src/lib.rs`, each guide included with
  `include_str!`; a trial example ran as a doctest (`cargo test -p krummi --doc guides`: 1 passed).
- **L4** — T1–T5: five parallel writers wrote the 14 guides; each guide was committed after its doctests passed
  (`cargo test -p krummi --doc -- guides::`: 112 passed and 3 compile-fail passed with all of them). T6 inline.
- **L5** (2026-11-22, user): «No entiendo que los tipos de datos la unidad del valor de time, timespan y duration sean
  nanosegundos y lo almacenado sea microsegundos. Esto despista mucho y no se explica.» Fixed in `49d18d3` (section
  *Nanoseconds in your code, microseconds in the store*). The user then asked for a PRD that unifies the unit in
  microseconds: `docs/prd/krummi/tables/tables-temporal-micros.prd.md`, outside this run.
- **L6** — Checks on `2e81b0f`: fmt and clippy pass; `cargo test --workspace` 61 binaries, 1,212 passed, 0 failed,
  35 ignored.
- **L7** — RDD: the whole range stopped with `lens_context_budget_exceeded`; reviewed by parts. `fb95203..7f405ae`
  (reliability) approved, `review-641192694d67d165`, one SUGGESTION R3-guide-registry-drift, fixed by the registry
  test `62e544d` (RED with a guide unregistered, then GREEN). `7f405ae..49d18d3` passive. `49d18d3..62e544d`
  (reliability) approved, `review-1da10a831654401e`, five informational advisories not acted on.
- **L8** — Codex R1 ([report](../../docs/reviews/usage-guides/2026-11-22-code.md)): findings R1, R2 (minor) and one
  pre-existing advisory, all fixed (`3cdd6db`, `0bb936e`, `a172447`); R2 follow-up
  ([report](../../docs/reviews/usage-guides/2026-11-22-code-02.md)) approved
  ([resolutions](../../docs/reviews/usage-guides/2026-11-22-code-resolutions.md)).
- **L9** — Closing report: [usage-guides-report.md](usage-guides-report.md).
