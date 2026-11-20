# Tables statistics — follow-ups

Objective: close the non-blocking RDD warnings and suggestions and the cleanup left by the tables-statistics run
([closing report](../done/tables/tables-statistics-report.md), *Pendiente* and *Limpieza manual*). Branch
`feat/tables-statistics`; delivery: local commits, no merge; tests with `cargo test --workspace`.

## Specs

The user's requests (L1, L2): «Haz lo que queda pendiente», then «haz solo los avisos, las sugerencias y la limpieza».
The slower reads (W7, W9, W20, W18) are out of scope. The items, quoted from the closing report:

- **S1** — «Avisos de RDD que quedan abiertos (tabla de arriba): el recorrido de todas las tablas en
  `find_write_table`, el conjunto de destinos retirados sin podar, `write_unless` sin prueba propia, los caminos sin
  verificar de hojas al azar y de escritura sin prueba de daño, y las guardas de pánico bajo `panic=abort`.»
  - S1a: `find_write_table` opens every table under the write turn when the sampled one is no longer under its name.
  - S1b: the `removed` set of retired targets is never pruned.
  - S1c: `write_unless` has no test of its own.
  - S1d: the unchecked random-leaf and write paths have no damage test.
  - S1e: the panic guards of sampling do nothing under `panic=abort`.
- **S2** — «Sugerencias de RDD sobre el gancho del banco (`review-fb0cd071ffaf8627`): informar los dos errores cuando
  fallan la escritura y el hilo, una prueba de W28 con el hilo y rechazar valores desconocidos de
  `KRUMMI_BENCH_ESTIMATES`.» Also the suggestion of `review-799413ea30a2ac85`: keep the `test-hooks` feature out of
  production builds.
- **S3** — «`work/st` (1,3 GB, ignorado por git): las fuentes y compilaciones de la base y de la rama, los scripts y
  los resultados de las mediciones» and «`~/st` dentro de WSL (602 MB)». Assumption (L2): remove the sources and
  builds, keep the scripts and results.

Each item is addressed by a fix with its test, or closed with evidence when the code already holds it; no product
behavior changes, so no PRD.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1c | inline | done: test of `write_unless` (turn free, arrival order, stop while queued); coverage of existing code, so no RED | `104b9f3` |
| T2 | S1d | inline | done: random-leaf (31,000 rows, seed 7) and write-sampling damage tests; both fail when `resolve_unchecked` checks the run | `9ab22f3` |
| T3 | S1b | inline | done: a deleted table's removed targets are pruned; bound documented. RED `deleting_a_table_prunes_its_removed_targets` (4 entries left, 2 expected), GREEN after | `095d908` |
| T4 | S1a | inline | done: the table's current name is looked up in a read before the write turn; under the turn a scan happens only after a rename or deletion in between, never for a deleted table. No RED: a cost, not observable; rename/delete tests still pass | `17040b3` |
| T5 | S1e, S2 (`test-hooks`) | inline | done: note at the sampling guards; `test-hooks` already only in dev-dependencies (`cargo tree -p krummi-bench -e features -i krummi-pager` and `cargo tree -p krummi -e normal,features -i krummi-pager` show no `test-hooks`) | `e9e2c99` |
| T6 | S2 (bench hook) | inline | done: unknown values fail; both errors reported (test fails on the old `Ok((written?, rounds?))`); W28 test with the thread | `0290241` |
| T7 | S3 | inline | done: `work/st` 1.3 GB → 171 KB, WSL `~/st` 602 MB → 68 KB; scripts and results kept | — (ignored files) |

## Log

- **L1** — 2026-11-20, user: «Haz lo que queda pendiente».
- **L2** — 2026-11-20, user, after being asked whether to include the investigation of the slower reads: «haz solo
  los avisos, las sugerencias y la limpieza». Estimate given: 3–5 h; the user confirmed («Sí») and the run started at
  2026-11-20T18:04:19+00:00 (phase log).
- **L3** — 2026-11-20, checks after T1–T7: `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D
  warnings` ok; `cargo test --workspace`: 59 binaries, 1,030 passed, 0 failed, 35 ignored.
- **L4** — 2026-11-20, RDD review of `134c6f2..3addc0e` (assessment: medium, `under_budget`, so reviewed as the
  last slice), lenses risk, resilience and reliability, consent granted (project rule): approved and acknowledged,
  lineage `review-2dc33303854f3e5d`. Findings, none blocking:
  - R1-001 (SUGGESTION), R4-save-read-before-stop (SUGGESTION), R3-save-read-before-turn (WARNING): the background
    save now opens a read before the write queue, and a read that fails while the store closes returns an error
    instead of `Ok(())`. Not changed: the only callers are the worker's jobs, whose result is discarded (`let _ =
    match job`, "Failures drop the job (ST9)", `crates/krummi/src/table/worker.rs`), so no caller can observe the
    difference.
  - R4-removed-retain-every-retire (SUGGESTION): the retain ran on every retire. Fixed: it runs only when a table
    was deleted.
  - R3-queue-timing-assert (SUGGESTION): the `write_unless` test bounded the stop with a wall-clock assert. Fixed: the
    assert is removed; the join happens while the turn is still held, which already proves that `stop` ended the wait.
  Fix commit `a1a03dc`; checks: queue (10), estimate unit (2) and table_statistics (48) tests pass, clippy clean.
