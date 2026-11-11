# Table row reads without materializing values — ODD execution document

Objective: make the table loads that miss their K7 threshold (W27 `tabla-intervalos` on Windows and Linux, W26
`tabla-lecturas` and W28 `tabla-actualizacion` on Windows) meet it, as an optimization with no observable product
change (no PRD: annotation of 2026-11-04 to K13 of `docs/prd/krummi/benchmark.prd.md`; E11, E19 and the *Salida* of
`docs/prd/krummi/tables-schema.prd.md` unchanged). Branch: `feat/table-row-reads` (from `feat/tables-schema`).
Delivery: `ask-on-risk` (single branch, left unmerged). Test runner: `cargo test --workspace` (Windows; Linux in WSL 2
Ubuntu). Estimate: 6 h – 10 h. Real start: 2026-11-11 12:05:51 +0000.

## Specs

- **S1** — Acceptance (W27 included since L10), the annotation of 2026-11-04 to K13 (benchmark PRD), verbatim: «Una optimización que no cambia
  nada de eso no necesita PRD: se ejecuta directamente y su criterio de aceptación es que las cargas que ataca cumplen
  su umbral en las ejecuciones completas de Windows y de Linux, sin que empeoren las demás cargas (decisión del
  usuario).» The loads it attacks: W26, W27 and W28 (K7: Krummi / Voron ≥ 0,80).
- **S2** — E11 of 7.1 unchanged, verbatim: «Una fila leída o recorrida tiene todas sus columnas vivas, en el orden del
  esquema, cada una accesible por su nombre o por su posición entre las columnas vivas, con nulo o con su valor por
  defecto en las añadidas después de guardarla (N3); una columna quitada nunca aparece.»
- **S3** — E19 of 7.1 unchanged, verbatim: «leer, guardar, borrar, recorrer, cargar y pedir un valor fallan con daño
  en una página que necesitan». A damaged row still fails the read or the scan step that returns it (the scan stays
  fused after it); reading a column of a returned row cannot fail.
- **S4** — *Salida* of the root (D11) and of the tables PRD unchanged: `string`, `bytes` and vector values are returned
  without copying, and they stay valid while the transaction is open (not only while the row lives); the other values
  are decoded copies.

## Execution design

- **D1** — A read row keeps the stored row bytes, borrowed from the page, and the encoded key, and checks the whole row
  when it is read, as today, without building its values. Each access to a column decodes it from the checked bytes.
  An older version's layout and the defaults borrowed from the catalog are shared by the rows of a read through
  reference counts. A row made owned (`into_owned`) holds its decoded values.
- **D2** — The accessors return values instead of references (`value`, `get` return `Option<Value<'t>>`; `values` and
  `columns` iterate). Allowed by 7.1's *Dejado a la implementación* (names and types, D8); the branch is unmerged, so
  the interface has no users yet.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S2, S3, S4 | inline | done (RED: `d11_values_outlive_their_row` does not compile against the reference accessors; `always_decodes_names_the_types_whose_every_value_decodes` pins the skipped checks; suite 785 passed) | `a840f02` |
| T2 | S1 (single-load measurements of W26–W28 and fixes) | inline | done (L3, L4: W26 and W28 over their threshold in single loads; W27 0.43, left by the user's decision) | `a840f02` |
| T4 | S1 (W27): typed accessors that read a checked column without a `Value`; an access plan if needed; a remembered row check if still short | inline | typed reads done (L11); the rest waits for the user | `5409362` |
| T3 | S1 (complete runs on Windows and Linux) | inline (background runs) | pending (the first attempt was stopped, L10) | |

## Log

- **L1** (2026-11-11, the user's requests, verbatim): «No vas a corregir tabla intervalos?»; to writing a PRD, «Sí»;
  to doing it as an optimization without PRD, with ODD, keeping E11 and E19, «Sí»; to the estimate of 6 h – 10 h,
  «Sí». Start 2026-11-11 12:05:51 +0000.
- **L2** (2026-11-11): starting state: diagnosis of W27 in `tables-schema.md` L19 (`Reader::decode` 70 % of the load;
  two allocations per row; 48-byte `Value` moves through `Result<Option<Value>>`). Branch created from
  `feat/tables-schema` at `bb8acca`. Another session commits JSON design documents in this copy; they may land on this
  branch and are not part of this run.
- **L3** (2026-11-11): T1 committed (`a840f02`); RDD assess `bb8acca..a840f02` medium, `under_budget` (pending in the
  slice). Single-load measurements on Windows (`work/t12/measure.py` from T12's base store, median op/s; Voron from the
  T12 Windows run): W26 872,063 (Voron 739,806, 1.18), W27 4,580,639 (Voron 10,657,201, 0.43), W28 107,095 (Voron
  127,190, 0.84); before, 559,000, 1,670,000 and 96,000. Steps of W27: stored row 2.92 M; check and checked decode
  without `Result<Option<Value>>` 3.74 M; inlining and an `Infallible` error 4.22 M; skipping the always-valid
  fixed-width checks and the bench's 64-bit division 4.56 M. Keeping the parsed header in the row gave nothing
  (4.52 M) and was reverted. Bound: with the column checks removed (an experiment, reverted), W27 reaches 5.66 M,
  0.53; the rest is the tree step (about 32 ns per row), the bench's own work and building a `Value` per column
  (about 15 ns each), while Voron reads raw fields without checks. So W27 cannot reach 0.80 with checked rows and
  `Value` reads: a product decision for the user. Wait starts.
- **L4** (2026-11-11, the user's answer, verbatim): «Cerrar con W26 y W28 (Recomendado)»: run the complete runs to
  confirm W26 and W28 without worsening the other loads; W27 stays a failed threshold (K13), documented with these
  measurements, for a later change. Wait ends. Next: RDD of the slice and the Codex review before the complete runs,
  so that the runs measure the final code.
- **L5** (2026-11-11): RDD of the slice `bb8acca..e036803` (forced: assessed medium, `under_budget`, 593 lines), lenses
  risk, readability and reliability, lineage `review-7d3a174b3d64e617`, **approved** and acknowledged. Advisories:
  R2-001 (`into_owned` cloned an owned row's values), R2-002 (`value` did not say an owned row copies its values),
  R2-003 (the bench's bare `6`), R3-key-skip-equivalence (WARNING: no test that `is_key` accepts the keys `decode_key`
  decodes), R3-checked-value-panic-contract (no direct test that every checked column reads): all fixed in the next
  commit; the `is_key` test fails when the UTF-8 check of `skip_element` is removed (mutation run, reverted).
- **L6** (2026-11-11): Codex round 1 ([report](../../docs/reviews/table-row-reads/2026-11-11-code.md),
  [resolutions](../../docs/reviews/table-row-reads/2026-11-11-code-resolutions.md)): `findings`, R1 (major: reading
  every column of a compound key walked its prefix again for each column, quadratic in the key columns; an identical
  update read the old values twice) → fixed in `9941b62` (the row keeps where each column of a compound key starts;
  the update decodes the old values once). Reproduction after the fix: linear, near the base. Single loads after it:
  W26 997,950, W27 5,043,318, W28 116,014. Checks: fmt, clippy clean, 787 passed.
- **L7** (2026-11-11): Codex round 2 ([report](../../docs/reviews/table-row-reads/2026-11-11-code-02.md)): `approved`,
  "Approved: R1 verified resolved; no open findings in the requested follow-up scope." Next: RDD of the last slice,
  then the complete runs (T3).
- **L8** (2026-11-11): RDD of the slice `e036803..ae39594` (forced: medium, `under_budget`, 462 lines), lenses
  readability and reliability, lineage `review-22562fd92a7cefd8`, **approved** and acknowledged. Advisories, all
  SUGGESTION and fixed in the next commit: R2-001 (the `key_value` contract was unclear), R2-002 (the key test's name
  covered only `is_key`), R3-key-starts-u16-panic (`key_starts` panicked past `u16::MAX` instead of returning `None`).
  Checks: fmt, clippy clean, 787 passed.
- **L9** (2026-11-11): RDD of `a02838b`'s slice `ae39594..a02838b` (forced: medium, `under_budget`, 23 lines), lenses
  readability and reliability, lineage `review-d0fe117898c0870c`, **approved**
  and acknowledged. Advisories: R2-001 (SUGGESTION: "or 0 for a key of one column" reads as a special case) dismissed:
  it is one, since a key of one column does not go through `key_starts` and the row reads it from 0
  (`Reader::decode`); R3-key-starts-overflow-untested (SUGGESTION) accepted as is: no key of at most 2,048 bytes
  reaches the branch, which only replaces a panic. T3 started on `a02838b` (`work/rr/complete.sh`).
- **L10** (2026-11-11, the user's messages, verbatim): «Pues habrá que hacer lo necesario par cumplir. Ya se hace
  comprobación de checksum, no? no haría falta otra comprobación. ¿Cómo podemos reducir el coste de construir los
  valores?»; to the plan (typed accessors and an access plan, then a remembered row check like the node check cache if
  W27 is still short, stopping the complete runs and repeating them at the end), «Sí». This replaces L4: W27 must meet
  its threshold too. Verified before answering: the integrity check (G6) covers only pages read from the data file,
  and the trees check a node's organisation "venga de donde venga" (`trees-node-check-cache.prd.md`), so the row
  check stays; without it W27 only reached 0.53 (L3). The complete runs started at 14:10 were stopped at 14:15.
- **L11** (2026-11-11): correction of L3: the single loads of L3, L6 and L9 ran without the bench power plan, while the
  T12 runs (Voron's figures and the thresholds) ran with it, so their ratios were too high. Under the plan, alternating
  binaries (median op/s, two rounds): W26 `a02838b` 629,952 / 616,782, typed 622,797 / 610,087 (Voron 739,806: 0.84);
  W27 `a02838b` 3,228,770 / 3,196,773, typed 3,666,620 / 3,636,663 (Voron 10,657,201: 0.34); W28 `a02838b` 93,529 /
  93,000, typed 92,659 / 92,678 (Voron 127,190: 0.73; T12's head gave 96,609 / 95,773, so its cost is in the write
  path). Typed reads committed in `5409362` (+14 % in W27); tests `typed_reads_give_the_values_of_their_columns` and
  `a_typed_read_of_another_type_panics`; suite 789 passed. A first full run failed once in
  `writer_check_tests::ac_h1_a_write_never_checks_the_nodes_it_changed` (it counts scratch version checks, which depend
  on the transfer thread's timing; trees code this run does not touch), then passed alone and in a full rerun.
  Profile of W27 under the plan with typed reads: the row check 30 % (UTF-8 included), the tree step 16 %, the column
  reads 37 % (the second UTF-8 validation included), the `ulid` key 11 %. Removing the whole check, the second UTF-8
  validation and half the key cost would give about 170 ns per row, about 0.55: W27 does not reach 0.80 with these
  steps. Wait for the user starts.
- **L12** (2026-11-11, the user's answers, verbatim): «Seguir con W27»; then, while T4 went on, «Tienes que encontrar
  otras optimizaciones.» Wait ends. Plan from the profile, measured one by one under the power plan against the
  previous binary: an access plan per column of the current version, the `ulid` key read from its element, no
  `Arc<Reader>` clone per row, the tree step, and the remembered row check.
- **L13** (2026-11-11): measurement fix: from about 14:15, repetitions of the same binary fell to a quarter mid-run
  (W27 8.8 M, 8.8 M, 2.6 M, 1.6 M without the plan), Voron too, with the disk idle and 25 GB free: Windows moved the
  worker to the efficiency cores. Pinned with `start /affinity FFFF` (logical processors 0–15, the performance cores
  of the i9-13980HX) W27 gave 6.7–8.8 M, pinned to `FFFF0000` (efficiency cores) 2.1 M. The complete runs of the
  benchmark do not pin their workers; this may affect them (a benchmark matter, recorded for the closing report).
  The WSL complete run, which went on after the stop of L10 (the first `pkill` came before it started), was killed at
  15:47; measurements between 14:17 and 15:47 shared the machine with it. From here, every single load runs pinned to
  the performance cores and under the bench power plan. W27, median op/s, two rounds: Voron 10,531,831–10,653,665;
  `a02838b` 3,285,776 / 3,267,569 (0.31); `963d126` 5,156,107 / 5,187,309 (0.49). With the column and key checks
  removed (experiment, reverted), 6,702,691 / 6,850,678 (0.64): the check costs about 48 ns per row.
  Committed `963d126`: a per-column access plan and check plan, `uuid`/`ulid` key elements checked by code and
  length, the transaction's kept readers (one `Arc` clone per row cost about 26 ns: measured by adding one), the `ulid`
  key read from its element and `text_bytes`. Suite 789 passed. Profiling with samply now fails ("samply requires
  Administrator privileges"): the elevated helper was refused.
- **L14** (2026-11-11): further steps, each measured pinned and under the plan against the previous binary (W27 median
  op/s): lean check without building values (`9e8fdaa`) 5.41 M; leaves a read transaction found valid, checked once
  per leaf (`2802e07`) 5.96 M, 0.56; per-table leaves by exact address (`82b2918`) equal. A direct-mapped table of
  16,384 leaves was slower (5.45 M: W27's 20,000 leaves thrash it) and a product hash of page-aligned addresses left
  hashbrown's bucket bits equal (5.37 M); both reverted. New test `a_malformed_row_fails_only_the_scans_that_return_it`
  (fails when every leaf passes, mutation run). Suite 791 passed. Profiling now works in WSL (samply as the user,
  `perf_event_paranoid` 1): WSL under the plan, W27 Krummi 5.95–5.99 M against Voron 9.30–10.16 M (0.61). Profile
  shares inside the load: typed reads (bench `checksum`, `checked_bytes`, `id_of`) about 31 %, the tree step 24 %,
  `ReadRows::next` and `decode` 21 %, the leaf memory 4.5 %, the bench's own sums about 6 %. The hottest instruction
  is the pointer chain row → kept reader → `Arc` → `Reader` that each typed read walks.
