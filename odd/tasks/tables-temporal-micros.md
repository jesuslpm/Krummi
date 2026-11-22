# Temporal values in microseconds — ODD execution document

Objective: implement `docs/prd/krummi/tables/tables-temporal-micros.prd.md` (approved by the Codex PRD review on
2026-11-22). Branch: `feat/tables-json` (same branch as the PRD; base `c813083`). Delivery: `ask-on-risk` (single
branch, left unmerged). Test runner: `cargo test --workspace` (Windows; Linux in WSL 2 Ubuntu). Estimate: 6 h – 10 h.
Real start: 2026-11-22 07:08:13 +00:00.

## Specs

- **S1** — The PRD `docs/prd/krummi/tables/tables-temporal-micros.prd.md` and its ancestry up to the root: decisions
  TU1–TU8, *Validaciones y mensajes*, *Salida*, *Fuera de alcance* and *Dejado a la implementación*, verbatim there.
  Its acceptance criteria AC-TU1–AC-TU6 and AC-TU8 are tests; AC-TU4 also has an execution check (a store written by
  the base opened by the head); AC-TU7 is the measurement.
- **S2** — TU7 and AC-TU7: N11 with its 2026-11-20 annotation: a one-round pass of every bench load before and after,
  and 10 to 20 rounds of W23–W32 and of any load that moves more than 3 %, on Windows and Linux, with the bench
  data checks (K4) unchanged.
- **S3** — The project's checks (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`), RDD on each work-unit commit, the Codex code review over the whole range, and the closing
  report.
- **S4** — The user's instruction (L1): «Implementa el PRD de microsegundos con ODD».

## Tasks

| ID | Specs | Task | Route | Status | Commit |
|---|---|---|---|---|---|
| T1 | S1 (AC-TU4) | pin the stored bytes of temporal rows, keys and index entries with the base code | inline | in progress | |
| T2 | S1 (TU1–TU6, AC-TU1–AC-TU6) | values in microseconds as `i64` everywhere, no rounding of given values, JSON instant in µs, tests | inline | pending | |
| T3 | S1 (TU8, AC-TU8) | guides and API docs with one unit | inline | pending | |
| T4 | S2 | measurements, Windows and Linux | inline, background runs | pending | |
| T5 | S3 | checks, RDD, Codex review, closing report | inline | pending | |

Route evidence: one behavior (the unit of temporal values) with its tests; no parallel units and no writer reason, so
the parent works inline.

## Log

- **L1** (2026-11-22, user): «Implementa el PRD de microsegundos con ODD»; estimate 6 h – 10 h confirmed («Sí,
  empieza»).
- **L2** — Branch: `feat/tables-json`, where the PRD is (default chosen without asking).
- **L3** — T1: golden bytes captured from the base code (`c813083`): `crates/krummi/tests/table_temporal.rs` (rows and
  primary keys) and `temporal_entries_store_the_bytes_stored_before_the_change` in
  `crates/krummi/src/table/index_key_tests.rs` (index entries, four shapes). Their inputs are given through helpers
  that are the only lines T2 changes. A store written by the base (rows, a temporal default, an index over
  `ts DESC, d`, analyzed statistics) and its readout are kept in the scratchpad for the AC-TU4 execution check.
