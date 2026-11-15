# Table column projections, run pool and short checksum — ODD execution document

Objective: implement the prepared column projection of `docs/prd/krummi/tables/tables-projection.prd.md` (v1.0), and
two optimizations from the 2026-11-15 performance diagnostic with no product change (no PRD): M1, a pool of value run
buffers in the store, and M2, the benchmark's short checksum in both engines. Branch: `feat/table-projection` (from
`main`). Delivery: `ask-on-risk` (single branch, left unmerged). Test runner: `cargo test --workspace` (Windows; Linux
in WSL 2 Ubuntu). Estimate: 7 h – 11 h. Real start: 2026-11-15 17:51:45 +0000.

## Specs

- **S1** — The projection PRD, `docs/prd/krummi/tables/tables-projection.prd.md` v1.0, and its ancestry up to the
  root: decisions PY1–PY7, *Validaciones y mensajes*, *Salida*, *Fuera de alcance* and *Dejado a la implementación*,
  verbatim there. Its acceptance criteria AC-PY1–AC-PY10 and AC-PY12 are the tests of the library; AC-PY13 (the
  annotations) is done in `a7043fc`.
- **S2** — PY7 and AC-PY11 of the same PRD: W26 and W27 read with a projection, prepared inside each repetition's
  measured time, with a test hook that shows it and a way to run W27 without it for the measurement of AC-PY11; full
  benchmark runs on Windows and Linux compared with the last ones (Windows 2026-11-13, Linux 2026-11-12).
- **S3** — M1, decided by the user on 2026-11-15 from the diagnostic (`docs/performance/2026-11-15-diagnostic/`,
  follow-up section 4): the private buffers of value runs are kept after a commit for the runs of the next writes;
  «Sí en el almacén»: the pool lives in the store, not in a thread, and closing the store frees it; at most 32 MiB of
  idle buffers per store, a fixed constant (the user's «sí» to the recommended 32 MiB); a reused buffer is zeroed and
  passes the allocation fault point as a new one would; the pool's own bookkeeping reserves its memory fallibly, and
  keeping a buffer never fails nor aborts the operation: a buffer that cannot be kept is freed.
- **S4** — M2, decided by the user on 2026-11-15 (follow-up section 1): the benchmark's byte sum takes a short path
  for up to 16 bytes in both engines, Rust a scalar sum and C# SSE2 for exactly 16 bytes and a scalar sum below; the
  same checksum as the current one for every input, proved by equivalence tests.
- **S5** — The project's checks (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`), RDD on each work-unit commit, the Codex code review over the whole range, and the closing
  report.

## Execution design

- **Projection identity (PRD *Dejado a la implementación*).** A table's schema is identified by the store instance
  (a serial unique in the process, drawn when the store opens), the table's identity and its schema state (P12). Every
  table reader carries them; binding a row compares the three numbers. A copy of the files opened as another store has
  another serial; an abandoned write's schema state is random and never equals another's.
- **Cache.** The store owns a cache of at most 1,024 projections keyed by table, schema state and the list's column
  positions. A projection holds the reader that prepared it (columns, layout, defaults), a plan per listed column for
  rows of the current version, and plans for each older version built on the first row of that version.
- **Prepared columns.** `ProjectedColumn<T>` carries the projection's key (shared by every preparation of the same
  list) and its position; reading compares the key by address first, then by content, and panics when it differs.
  `T` is the column's Rust type where it is unique and a marker otherwise; decimals, uuids and vectors accept any of
  their parameters (PRD *Dejado a la implementación*).

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S4 (M2): short byte sum in Rust and C#, equivalence tests | inline | done (equivalence: `the_short_byte_sum_equals_the_sum_of_every_byte` and the Voron selftest `byteSum(0..33 bytes) ok`) | `0175298` |
| T2 | S3 (M1): run pool in the store | inline | done (RED: `run_pool_tests` did not compile without the pool; GREEN 4 of 4; mutation without zeroing fails `a_commit_keeps_its_run_buffers_and_the_next_run_gets_one_zeroed`; krummi-tx and krummi-pager suites pass) | `bee2dcb` |
| T3 | S1: projections in the library (prepare, cache, prepared columns, bind, read) with AC-PY1–AC-PY10, AC-PY12 | inline | done (RED: the tests did not compile without the API; GREEN: `table_projections` 10 of 10 and `projection_tests` 2 of 2 in debug and release; mutations without the schema check and with the current plan on older rows fail AC-PY5, AC-PY10 and AC-PY2; workspace suite passed) | `9c9cba6`, `2277110` |
| T4 | S2: W26 and W27 read with a projection, test hook, W27 without it | inline | done (`w26_and_w27_read_with_a_projection`) | `5b17ff0`, `2277110`, `c00f2dc` |
| T5 | S2 (AC-PY11): full runs on Windows and Linux | inline (background runs) | pending | |
| T6 | S5: final checks, RDD of the last slice, Codex review, closing report | inline | pending | |

## Log

- **L1** (2026-11-15) — The user's request, verbatim: «Escribamos el PRD y luego arranquemos todo.» — the projection
  PRD, then one run with the projection, M1 and M2. The estimate went to the user as «Para la ejecución completa (la
  proyección, M1 y M2) estimo **entre 7 h y 11 h**. ¿Empiezo?», and the user answered «Sí» (start 17:51:45 +0000).
- **L2** (2026-11-15) — `a7043fc` commits the PRD, its translation, its Codex review, the annotations (AC-PY13) and the
  copy of the diagnostic. Exploration: rows read by position through `TableRow::typed` (`rows.rs:1011`); readers are
  built per transaction (`catalog.rs:178`, `:269`, `:508`); the engine already keeps page payloads for the next copies
  up to 32 MiB per store (`krummi-tx/src/lib.rs:136`, `:258`), the precedent M1 follows.
- **L3** (2026-11-15) — T1: `byte_sum` takes a scalar `u32` sum up to 16 bytes; `Data.ByteSum` one SSE2 step at 16
  bytes and a scalar sum below; the Voron `selftest` gained `byteSum(0..33 bytes)`, which printed `ok` (run with
  `dotnet run -c Release -p:RavenDbRoot=work/ravendb-7.2.7 -- selftest`). RDD assess (base `main`, committed only):
  `medium`, `under_budget`.
- **L4** (2026-11-15) — T2: the engine keeps up to 32 MiB of value run buffers per store on disk after
  `writer.commit` copied them (`Engine::keep_runs`), handed zeroed through `reuse_run`, the allocation fault point of
  `alloc_run`; the list grows with `try_reserve` and a buffer that cannot be kept is freed. A temporary store keeps
  none, as for the page payloads (P10). RDD assess: `medium`, `under_budget`.
- **L5** (2026-11-15) — T3 decisions within *Dejado a la implementación*: names `ReadTransaction::projection`,
  `WriteTransaction::projection`, `Projection::column::<T>`, `Projection::bind`, `ProjectedRow::get`; `T` is the Rust
  type of the column or a marker of `krummi::table::projection` (`Text`, `TextBytes`, `Bytes`, `Date`, `Time`,
  `Timestamp`, `Duration`, `Vector`); `Decimal`, `Uuid` and `Vector` accept any precision and scale, key order,
  element and dimension. Binding fails with the new `SchemaDifference::Projection`; an empty list with the new
  `ValueRule::EmptyProjection`. The typed reads of `TableRow` now use the same `ReadAs` decoders (DRY). Test hooks:
  `Projection::is_kept`, `Projection::same_as`, `Store::kept_projections`.
- **L6** (2026-11-15) — T4: W26 and W27 read through `read_by_key` and `read_ranges`, which prepare the projection
  inside the measured time; `KRUMMI_BENCH_PROJECTION=off` reads by position (diagnostic hook for AC-PY11). Risk of
  T3: item 3 (the typed reads of `TableRow`, which every existing column read uses, now go through the projection
  decoders) and item 4 (a store-wide cache behind a mutex, older plans in `OnceLock`).
- **L7** (2026-11-15) — RDD of `a7043fc..4774ba4` (M2, M1, the projection, the benchmark; assessed `high` by the item 3
  escalation), lenses risk, readability and reliability: approved, lineage `review-4976a6e6cdfe7836`, acknowledged
  (authority burned). Advisory findings: R2-001 (WARNING, this document listed T3 and T4 as pending): updated here.
  R2-002 (SUGGESTION, `key` meant both the list and the row key in `ProjectedRow::get`): renamed `list`. R2-003
  (SUGGESTION, the benchmark's prepared columns by literal index): taken by the position of their names. R3-001
  (WARNING, a kept projection prepared again after a rename named the old table in its errors): RED in
  `ac_py5_rows_of_another_table_store_or_schema_do_not_bind` (`left: "t"`, `right: "renamed"`), fixed by keeping the
  name in each handle. All in `2277110`. Risk lens found nothing.
- **L8** (2026-11-15) — A/B of W26 and W27, both engines, two rounds pinned to the performance cores under the bench
  power plan (`work/tp/ab.sh`, `work/tp/ab-*.tsv`): with the first plan W27 read **slower** with the projection,
  8.89 M rows/s against 9.20 M by position in both rounds (−3.4 %); W26 even. Cause: each read followed the list to the
  reader's columns for the type and matched the slot's place again. `c00f2dc` puts the type, the live position and,
  for a fixed-width column, its byte range and null bit in each column's plan. A/B of Krummi alone after it, three
  alternated rounds (`work/tp/ab2.py`, `work/tp/ab2-opt.out`): W27 95.2 ns per row with the projection against
  106.1 by position (−10.3 %, all three rounds); W26 1,351 against 1,372 ns (−1.6 %). Tests: `table_projections` 10 of
  10, `projection_tests` 2 of 2; clippy clean.
