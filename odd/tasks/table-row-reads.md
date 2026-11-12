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
- **S5** — Since L20, the PRD `docs/prd/krummi/tables-row-reads.prd.md` (decisions L1–L4; it replaces S3 for reads by
  key and scans, and S1's acceptance of W26 and W27 with AC-L9), its acceptance criteria verbatim:

  1. **AC-L1** — Dada una fila guardada con un texto que no es UTF-8 en una página con la integridad correcta, cuando se
     lee por su clave y cuando se recorre hacia delante y hacia atrás un intervalo que la incluye, en una transacción de
     lectura y en una de escritura, entonces la lectura y el paso del recorrido no fallan, sus demás columnas se leen con
     sus valores, leer ese texto como texto termina con un pánico y leerlo como bytes los devuelve tal cual; las pruebas
     pasan igual compiladas en depuración y en optimizado.
  2. **AC-L2** — Dadas una fila con un `bool` que no es 0 ni 1, una con un elemento de la clave mal formado y una con un
     desplazamiento de un valor variable fuera de la fila, en páginas con la integridad correcta, cuando se leen por su
     clave o se recorren, entonces la lectura y el paso del recorrido no fallan, y el recorrido devuelve las filas del
     intervalo en el orden de los bytes de sus claves (lo que da leer la columna mal formada queda a la implementación,
     L2).
  3. **AC-L3** — Dadas una fila de una versión de esquema que la tabla no tuvo, una con una cabecera que Krummi no
     escribe y una que no llega al final de su parte fija, en páginas con la integridad correcta, cuando se leen por su
     clave o se recorren, entonces fallan con la clase de daño y el número de la página de la fila; y una fila de una
     versión que falta en el historial de la tabla falla con la clase de daño y el número de la página del historial
     que debería tenerla. Todo como hoy.
  4. **AC-L4** — Dada una fila existente mal formada en una página con la integridad correcta, cuando se actualiza o se
     inserta o actualiza, también después de leerla en la misma transacción, entonces falla con daño de su página, sin
     cambiar la fila, y la transacción sigue T11, como hoy (L3).
  5. **AC-L5** — Dado el pánico de leer un texto que no es UTF-8: cuando la aplicación lo captura alrededor de esa
     lectura, entonces la transacción sigue intacta (en una de lectura, las demás lecturas funcionan; en una de
     escritura, puede seguir escribiendo y confirmarse, y se aplica lo que hizo); y cuando lo captura fuera de la
     transacción, que el pánico atraviesa, entonces una de lectura termina, una de escritura se descarta sin aplicar
     nada y su turno queda libre (T10), y en los dos casos otras lecturas, la siguiente escritura y el cierre del almacén
     funcionan.
  6. **AC-L6** — Dada una página de filas alterada con el almacén cerrado, cuando se lee o se recorre una fila suya,
     entonces falla con daño (AC19 de 7.1 y AC10 de la raíz siguen valiendo).
  7. **AC-L7** — Dado un almacén escrito por 7.1, cuando se abre y se leen sus tablas, entonces se leen las mismas filas
     con los mismos valores, sin migrar nada.
  8. **AC-L8** — Dados E19 de 7.1, G6 de la raíz y K7 del banco, en el original y en su traducción, cuando se leen,
     entonces llevan la anotación de este PRD.
  9. **AC-L9** — Dadas las ejecuciones completas del banco en Windows y en Linux, cuando se comparan con las del cierre de
     7.1, entonces W26 y W27 cumplen su umbral de K7 en las dos, y ninguna otra carga baja más de 0,05 su razón con
     Voron, salvo que la mediana de una medición de una sola carga, con sus repeticiones, la iguale a la del cierre de
     7.1 dentro de ese margen. Un umbral de W26 o W27 que siga incumplido no es un fallo de este PRD: sigue K13 y se
     informa en el cierre.
- **S6** — Since L33, W28 on Windows (0.75, the only load under K7): the user's words, verbatim, to proposals 1 and 2
  of Codex's investigation (`work/codex/table-perf.md`: patch the validated fixed-width columns of an update instead of
  rebuilding the row; keep the located entry across the update's validation and write): «Sí». An optimization with no
  observable change (S1's annotation to K13): the stored rows, E10's identical updates, E17's error order and L3's
  check of the replaced row stay as they are; acceptance is W28 meeting its threshold without worsening the other
  table loads.

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
| T5 | S5 (AC-L1–AC-L7): reads by key and scans without row content checks, update and upsert still checking | inline | done (RED: 6 of the 8 new tests in `table::malformed_tests` failed with damage; GREEN: 8 passed in debug and in release; suite 799 passed) | `5dcfa29` |
| T3 | S1, S5 (AC-L9: complete runs on Windows and Linux) | inline (background runs) | done for the table loads only (L30, L31); W1–W22 not run by the user's decision | `d6a88b7` |
| T6 | S6: an update that gives only fixed-width columns of a current-version row rewrites their slots in the stored row (`Reader::patch`, `Slot::write`) | inline | done (L35) (guard test `an_update_stores_the_bytes_of_the_whole_merged_row` passes before and after; fails with `Slot::write` emptied, mutation run) || `7b645e5`, `c873418`, `bb1408b` |
| T7 | S6: the update writes a row of the same length where `get_located` found it (`krummi_tree::overwrite`), without a second descent | inline | done (L35; RED: `a_located_value_of_the_same_length_is_overwritten_in_place` did not compile) || `7b645e5` |
| T8 | S6: the journal entry of a page skips unchanged 64-byte blocks before comparing word by word (`journal::encode_page`), the same entry byte for byte | inline | reverted (L35) (equivalence test `entries_skip_unchanged_blocks_and_keep_the_runs_of_every_word` against the word-by-word encoder; fails when a changed block is skipped, mutation run) || `c76626e`, reverted in `5bd0f92` |
| T9 | Investigate the W23 drop of L31 (about −3 % from `bb8acca` to `d6a88b7`) | inline (A/B measurements) | done (L42): no drop | — |
| T10 | W25 (Codex proposal 3): a row that gives every column in schema order skips the name map and its staging; a one-column key is encoded without a staged vector | inline | done (L43) (equivalence test `a_row_in_schema_order_fails_and_is_stored_as_in_any_other_order`) | `9027c66` |
| T11 | W25 (Codex proposal 5): an insert beyond the table's greatest key adds at the end of the rows tree's last leaf (`krummi_tree::append_last`) when the tree would change only there | inline | done (L44) (tree equivalence tests byte for byte, table model test, mutations) | (this commit) |

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
- **L15** (2026-11-11, the user's answer, verbatim): «Seguir optimizando» (flatten the typed reads with direct
  references and a compact per-column plan, slim the row and the scan step, measuring each step). Wait ends.
- **L16** (2026-11-11): more steps, pinned and under the plan (W27 median op/s): compact per-column record and a
  direct reader reference (`ed61733`) +3.5 %; `#[inline]` scan step (`7852933`) 6.25 M; `next_row`, which lends the
  row instead of moving a 120-byte `Result<TableRow>` (the size of `Error`) several times per row (`e7dd8d2`): 8.15 M,
  0.77. The check now costs about 7 %: without it, 8.72 M, 0.83 (experiment, reverted). W26 0.89–0.96 (passes); W28
  about 96,000, 0.76, with or without checks: its cost is the write path. The user's messages, verbatim: «Y además
  ecribe un PRD que anote K7 para las cargas de tablas: Voron no comprueba sus filas y Krummi sí (E19). Deja de
  comprobar las filas.»; «Como dije antes, las páginas ya se comprueban con xxhash, así que ¿por qué más
  comprobaciones?»; «Y qué otras optimizaciones tenemos? debe de haber alguna manera de cumplir el umbral, tienes el
  código de Voron y herramientas de perfilado.»; «Y qué tal sería hacer un árbol especializado con clave de tamaño
  fijo para claves de una sóla columna de tipo i64, u64, ulid y uuid?» (answered: about 3–6 % for days of work and a
  format change; not now); «No entiendo qué hay que decodificar cuando tienes un ulid como clave, los bytes ya están
  directamente en la memoria.» (right: a `ulid` key element is the type code and its 16 bytes, read as they are);
  to the malformed row question, «Pánico del proceso (Recomendado)». PRD drafted:
  `docs/prd/krummi/tables-row-reads.prd.md` (L1–L3, AC-L1–AC-L5).
- **L17** (2026-11-11): PRD reviewer (two rounds, no blocker in the second); the user's answers, verbatim: to writes,
  «Solo lecturas (Recomendado)». Codex PRD round 1 ([report](../../docs/reviews/tables-row-reads/2026-11-11-prd.md),
  [resolutions](../../docs/reviews/tables-row-reads/2026-11-11-prd-resolutions.md)): `findings`, R1 and R2 (major,
  product questions), R3 and R4 (minor, fixed). The user's answers, verbatim: R1 «Pánico normal del hilo
  (Recomendado)»; R2 «Sigue intacta (Recomendado)».
- **L18** (2026-11-11): Codex PRD round 2 ([report](../../docs/reviews/tables-row-reads/2026-11-11-prd-02.md)):
  R1, R2, R4 verified, R3 reopened (L1 and AC-L3 named different pages for a version missing from the history), fixed
  in `77325f6`; round 3 ([report](../../docs/reviews/tables-row-reads/2026-11-11-prd-03.md)): `approved`. Annotations
  of E19 (7.1), G6 (root) and K7 (benchmark) applied; translations in progress.
- **L19** (2026-11-11): translations done (`tables-row-reads.prd.en.md` and the three English annotations; same
  headings, decision and criterion IDs). The elaboration time is left out: `prd_time.py` measures a whole session and
  this one holds several features since 2026-11-03 (it gave 94 h 36 min active), so it cannot isolate this PRD. The PRD
  waits for the user's authorization to implement it.
- **L20** (2026-11-11, the user's answer, verbatim): to implementing the PRD with an estimate of 1–2 h plus about 4 h of
  complete runs, «Sí». Wait ends; T5 starts 2026-11-11 20:23:30 +0000. Route: inline (one understood change; no
  writer trigger). Design: `Reader::decode` takes `check`, false for reads by key and scans (version, header and fixed
  part only), true for the row an update or upsert replaces (key and every stored column); `key_starts` gives one
  start per column also for a malformed key; the checked leaves (`2802e07`, `82b2918`) and the tree's leaf access
  added for them are removed, so `krummi-tree` is back to the base. AC-L3 passed before the change (as today); AC-L6
  holds through the AC19 tests of `table_damage.rs`, unchanged and passing; AC-L7: no line of the row or key encoding
  changed since `bb8acca` (`git diff bb8acca -- crates/krummi/src/table/{row,key}.rs` touches only reading), so a
  store written by 7.1 reads the same. Checks: fmt, clippy clean, 799 passed; the new tests also pass with
  `--release`.
- **L21** (2026-11-11): RDD of the slice `a02838b..5dcfa29` (assessed medium, `under_budget`, 2,366 lines; requested as
  the last code slice), lenses risk, resilience, readability and reliability, lineage `review-9378929f5e8f62c8`,
  **approved** and acknowledged. Advisories: R1-001 and R4-reader-cache-history-key (WARNING: a read transaction keys
  its kept readers by the table's history root, assumed unique) dismissed: every table gets its own history tree,
  created by `krummi_tree::create` when the table is created (`catalog.rs:566`), so two tables of one snapshot never
  share a root; the new test `tables_read_in_one_read_transaction_keep_their_own_schemas` pins it.
  R4-next-row-fast-path-version and R3-lent-fast-path-version-transition-untested (WARNING: the lent fast path might
  read another version with the current layout) dismissed for the behaviour, since `RowLayout::row` rejects a row of
  another version (`row.rs:287`), and their missing tests added: the AC-L3 tests also scan with `next_row`, and
  `a_lent_scan_reads_an_older_row_after_current_ones` reads an older row after current ones; removing the fast path's
  `layout.row` check fails three tests (mutation run, reverted). R2-annotation-merged-into-list (WARNING: the E19
  annotation continued the E8 list item) fixed with a blank line in `tables-schema.prd.md` and its translation.
  R2-duplicated-array-helper (SUGGESTION) fixed: one `slot_array` in `row.rs` for the decoder, the check and the typed
  reads. R2-slot-name-collision (SUGGESTION) fixed: `decode_into` takes `current`, as `Scan::current`.
  R3-readers-linear-growth (SUGGESTION) accepted as is: the list holds one reader per table a read transaction reads,
  or per schema a table had in a write transaction.
- **L22** (2026-11-11): RDD fixes committed in `3ecc294`; checks: fmt, clippy clean, 801 passed. Single loads of
  `3ecc294` against `e7dd8d2`, alternating, pinned to the performance cores and under the bench power plan (median
  op/s, two rounds; `work/rr/nochk-single.log`): W27 `e7dd8d2` 8,440,716 / 8,303,025, `3ecc294` 9,196,898 / 9,102,685
  (Voron 10,531,831–10,653,665: 0.86); W26 `e7dd8d2` 713,113 / 709,047, `3ecc294` 741,938 / 743,036 (Voron 739,806:
  1.00). Both meet K7's 0.80 in single loads; AC-L9 is decided by the complete runs (T3).
- **L23** (2026-11-11): Codex code review of the whole run, round 1
  ([report](../../docs/reviews/table-row-reads/2026-11-11-code-03.md),
  [resolutions](../../docs/reviews/table-row-reads/2026-11-11-code-03-resolutions.md)): `findings`, R2 (major: a typed
  read of an older row decoded its text, so `text_bytes` panicked) fixed in `2bc4fe4`; R3 (major: `Readers::keep`
  walked a list, linear in the tables read) fixed in `bb496a1` with a hash trie (`get_row` on the last of 1,000 tables
  196 ns, was 4,066). Checks: fmt, clippy clean, 803 passed; the malformed-row tests pass in release.
- **L24** (2026-11-11): Codex follow-up, round 2 ([report](../../docs/reviews/table-row-reads/2026-11-11-code-04.md)):
  R2 and R3 verified resolved; R4 (minor: the R3 test failed Clippy's `needless_borrows_for_generic_args`) fixed in
  `6167d9e`. The L23 claim of a clean Clippy was wrong: the check command piped Clippy into `tail`, which hid its exit
  code; the checks now run with `pipefail`.
- **L25** (2026-11-11): Codex follow-up, round 3 ([report](../../docs/reviews/table-row-reads/2026-11-11-code-05.md)):
  `approved`, "Approved for the requested follow-up scope: R4 verified resolved; no open findings."
- **L26** (2026-11-11): RDD of the last code slice `5dcfa29..9ea1b59` (lenses risk, readability and reliability),
  lineage `review-cb691290c004b232`, **approved** and acknowledged. Advisories, all SUGGESTION and fixed in the next
  commit: R2-001 (the trie's digit width was not tied to `FANOUT`: now derived from it, with a const assertion),
  R2-002 (an unnamed column index in the lent-scan test: now found from the schema), R2-003 (two resolutions
  headings `## 2026-11-11`: the R4 one now says so), R3-older-text-bytes-write-tx-uncovered (the older-row text test
  now also reads in a write transaction), R3-trie-concurrent-publication-uncommitted (new test
  `threads_of_one_read_transaction_read_each_table_with_its_own_schema`: 8 threads of one read transaction open and read 64
  tables in different orders).
- **L27** (2026-11-11): RDD of the slice `9ea1b59..5174433` (lenses reliability and readability), lineage
  `review-056aee61ef36b0bd`, **approved** and acknowledged. Advisories: R2-test-name-overclaims and
  R3-concurrent-test-does-not-observe-once (WARNING: the concurrency test's name promised one kept reader per table,
  which it does not observe) fixed by renaming it to what it checks; R2-digit-guard-incomplete and
  R3-digit-guard-incomplete (SUGGESTION) fixed: the const assertion also bounds `DIGIT`; R2-repeated-magic-modulus
  (SUGGESTION) fixed with a named constant and a comment on the visit order; R2-missing-resolution-context (WARNING:
  the test names an RDD finding that no review document records) dismissed: RDD findings are recorded in this log
  (L26), not in the Codex resolutions.
- **L28** (2026-11-11): advisories of L27 fixed in `d6a88b7`; checks: fmt, clippy clean (with `pipefail`),
  `table_rows` 33 passed, the crate's unit tests 75 passed. RDD of the slice `5174433..d6a88b7` (lenses readability
  and reliability), lineage `review-389fd4a1315c29ca`, **approved** and acknowledged. Advisory R2-001 (SUGGESTION: a
  `names` vector shadows the `names` closure in the concurrency test) accepted as is: test-only naming. Every code
  commit of the run has been through RDD; the code is final for the complete runs (T3).
- **L29** (2026-11-12): T3's complete runs of `d6a88b7` started 2026-11-11 21:44:25 +0000 and stopped when Windows
  restarted during the night without asking: the last output, `work/rr/windows.err`, is from 22:28:52 (redb's loads of
  the Windows run); no report was written and the WSL run never started. The phase log closes the background run and
  starts a wait at that time. The bench power plan stayed active through the restart and was restored on 2026-11-12
  06:31.
- **L30** (2026-11-12, the user's answer, verbatim): to relaunching the complete runs, «Lanza sólo las ejecuciones de
  las tablas». Wait ends. The orchestrator has no option to pick loads, so `work/rr/tables.py` runs W23–W29 for Krummi
  and Voron as it does (Krummi first; W23–W25 from an empty store, W26–W29 from W23's last store; 1 warm-up and 3
  measured repetitions, the median; Voron's data digests checked against Krummi's), with `head.exe` and the
  `VoronBench.dll` built from `d6a88b7` by the interrupted run, under the bench power plan, not pinned, as the
  orchestrator runs; `work/rr/wsl-tables.sh` does the same on WSL 2. AC-L9's other condition (no other load drops more
  than 0.05) is not measured by this decision: the change touches only the table code, and the closing report says so.
- **L31** (2026-11-12): T3, table loads of `d6a88b7` (`work/rr/tables.py`, under the bench power plan):
  - Windows, unpinned (06:35–06:42): Voron came out at half its usual figures (W27 5,360,405, W26 466,998, W28 65,061;
    7.1's close and the single loads gave about 10.5 M, 740k and 127k) while Krummi matched its single loads (W27
    9,270,580, W26 756,592): Voron's process ran on the efficiency cores (L13), so this run is discarded.
  - Windows, both engines pinned to the performance cores (06:53–06:59, `work/rr/windows-tables-pinned.tsv`; 7.1's
    close in brackets): W23 367,219 / 332,339 = 1.10 (1.17); W24 96,620 / 105,388 = 0.92 (0.91); W25 362,140 /
    429,253 = 0.84 (0.88); W26 728,545 / 760,213 = **0.96** (0.75); W27 9,221,262 / 10,387,076 = **0.89** (0.15);
    W28 96,793 / 129,084 = 0.75 (0.73); W29 118,826 / 52,987 = 2.24 (2.20). Data digests match in every load.
  - WSL 2, unpinned (06:42–06:51, `work/rr/linux-tables.tsv`): W23 1.11 (1.09); W24 1.03 (1.01); W25 1.18 (1.06);
    W26 **1.13** (0.88); W27 **0.97** (0.19); W28 0.91 (0.87); W29 1.80 (1.91). Data digests match.
  - AC-L9's drops over 0.05, checked with single loads: W29 on WSL 2, two more runs of W23 and W29 for both engines,
    1.89 and 1.93 against 7.1's 1.91 (`work/rr/w29-1.tsv`, `w29-2.tsv`). W23 on Windows (−0.07): Krummi alone, pinned,
    alternating `bb8acca` and `d6a88b7` (`work/rr/w23-single.log`): 359,784 / 360,341 against 351,154 / 344,787, about
    −3 %, about −0.03 of ratio against Voron's 332k; the rest of the drop is Voron measuring higher pinned (332k
    against 317k at 7.1's close). W25 the same in both (353,866 / 363,423 against 357,648 / 358,824).
  - W26 and W27 meet K7 on both systems. W28 still misses it on Windows (0.75, 0.73 at 7.1's close): out of this PRD's
    scope, K13. The loads W1–W22 were not run (L30).
- **L32** (2026-11-12): closing report: [table-row-reads-report.md](table-row-reads-report.md). The branch stays
  unmerged and unpushed for the user's review.
- **L33** (2026-11-12, the user's messages, verbatim): on Codex's report, to starting proposals 1 and 2 in a new
  branch, «¿Para que necesitamos otra rama? esta no está mergeada con main y aún no hemos terminado 7.1»; to the
  estimate of 3 h – 5 h, «Sí». Start 2026-11-12 08:24:26 +0000, on this branch. Route: inline (one understood change
  in `write_row` and one tree operation; no writer trigger).
- **L34** (2026-11-12): T6 and T7 committed together in `7b645e5` (both change the same lines of `write_row`); checks:
  fmt, clippy clean, 806 passed. The user's message, verbatim, to proposal 4 (the journal diff by blocks) after the
  answer on vector instructions: «Inclúyelo en esta ronda, al fin y al cabo las mediciones es lo que más tiempo se
  lleva.» T8 added: blocks of 64 bytes compared as arrays, which the compiler vectorizes with SSE2, without `unsafe`;
  AVX2 by runtime detection only if a measurement shows it is worth it (the i9-13980HX has no AVX-512).
- **L35** (2026-11-12): T8 committed in `c76626e`; checks: fmt, clippy clean, 807 passed; `page_diffs`'s ignored
  acceptance test passed in release (184 s). The ignored `recovery` sweep of every byte (about 73 min, from its
  1-in-997 sample's 4.4 s) was stopped to measure, by the user's answer, verbatim, «Sí»; it runs again during the
  reviews. Single loads, Krummi, pinned, under the plan (median op/s; `work/rr/t678-single.log`, `t8-w28.log`):
  W28 `d6a88b7` 97,788 / 95,300, T6–T7 `7b645e5` 112,696 / 110,430 / 109,459 / 110,271 (+15 %; Voron pinned
  129,084: 0.86), T6–T8 `c76626e` 108,195 / 109,565 / 107,878 / 108,752 (1–4 % under T6–T7 in each round); W23,
  W24, W25 and W29, `d6a88b7` against T6–T8, within ±3 % (W23 367,181 / 365,012 against 371,539 / 369,760; W24
  91,791 / 92,063 against 91,843 / 94,651; W25 361,188 / 353,152 against 356,363 / 355,840; W29 117,475 / 118,455
  against 116,550 / 119,198). Voron's `DiffPages` (`Sparrow.Server/Utils/DiffPages.cs`) is a scalar loop over four
  `long`s per step, without vector instructions, with 32-byte runs. The user's answer, verbatim, to reverting T8:
  «Sí. ¿Y las otras optimizaciones que recomendó Codex?». Reverted in `5bd0f92`; the pager's tests pass (150).
- **L36** (2026-11-12): the user's answer, verbatim, to closing this round (final table runs, RDD, Codex, the
  recovery sweep and the report, leaving proposals 3 and 5 for a later change and not 6): «Sí». RDD of the slice
  `d6a88b7..5bd0f92` (T6–T7, T8 and its revert; lenses risk, reliability and readability), lineage
  `review-28c95d242b64ea75`, **approved** and acknowledged. Advisories:
  - R1-patch-null-not-null-panic, R2-002, R3-patch-path-invalid-value-untested (WARNING: could a null in a column
    without nulls reach `Slot::write`'s panic): dismissed for the behaviour, since `Reader::check` normalizes every
    given value and fails a null there (`value.rs:380`) before the row is read; stated in `Reader::patch`'s
    documentation and pinned by the new test `an_update_with_an_invalid_fixed_width_value_fails_before_the_patch`
    (null and another type, through update and upsert).
  - R3-overwrite-skips-apply-rows (WARNING) and R2-003: `apply_rows` only records the root and the count, which an
    in-place write keeps; said in the comment, and the update test now reads every row back from a new read
    transaction after its commit.
  - R2-001 (WARNING: T6 and T7 missing from the committed execution document): fixed by committing the document.
  - R1-overwrite-trusts-located, R2-006, R3-stale-located-untested (SUGGESTION): `overwrite` also checks that the page
    is a leaf, and its documentation says what its checks guard.
  - R2-004 (SUGGESTION): `FIXED_BYTES` names the width of `fixed_bytes`. R2-005, R3-identical-assertion-weak
    (SUGGESTION): the message says what the assertion observes; the update test now also goes through `upsert_row`.
    R3-tree-test-root-shadowed (SUGGESTION): the tree test keeps the first root and names the grown one.
- **L37** (2026-11-12): RDD fixes committed in `33f7f7b`; suite 807 passed. Codex review of `d6a88b7..33f7f7b`
  ([report](../../docs/reviews/table-row-reads/2026-11-12-code.md),
  [resolutions](../../docs/reviews/table-row-reads/2026-11-12-code-resolutions.md)): `findings`, R1 (major:
  `Reader::patch` searched the key columns' list for each given column, quadratic in a wide compound key; 2.32 times
  the full merge with 1,022 key columns) fixed in `c873418` (patch / full merge after it: 0.76 at 1,022 keys).
- **L38** (2026-11-12): Codex follow-up ([report](../../docs/reviews/table-row-reads/2026-11-12-code-02.md)):
  `approved`, "Approved for the requested follow-up scope: R1 verified resolved; no open findings."
- **L39** (2026-11-12): RDD of the slice `33f7f7b..` the follow-up's record (lenses reliability and readability),
  lineage `review-f239fe4be7f70170`, **approved** and acknowledged. Advisories, fixed in the next commit:
  R2-fastcolumn-key-vs-in-key-naming (WARNING) and R2-shadowed-key-binding: the field is `primary_key`, the vector
  `is_primary_key`, one doc line per field; R2-test-doc-overclaims-constant-time and R3-test-doc-claims-constant-time:
  the test's documentation says it is a functional check; R3-wide-key-test-no-variable-key: the test's key now starts
  with a text column. A functional test cannot tell a patch that skips the text key column from the full merge, which
  stores the same row, so the constant-time search is pinned only by the review's measurement. Accepted as is:
  R2-review-report-line-citation-off-by-one (a line number in Codex's report, which is not edited).
- **L40** (2026-11-12): RDD of `bb1408b` (lens readability), lineage `review-4a114bbe6859cbf3`, **approved** without
  findings, acknowledged. The ignored `recovery` sweep of every altered byte passed (4,075.66 s, 10:45–11:53, alongside
  the reviews). Final table loads of `bb1408b` (`work/rr/tables.py`, under the bench power plan; data digests match in
  every load), against `d6a88b7`'s (L31) in brackets:
  - Windows, both engines pinned (`work/rr/final-windows.tsv`): W23 368,033 / 339,593 = 1.08 (1.10); W24 96,524 /
    107,427 = 0.90 (0.92); W25 366,869 / 445,392 = 0.82 (0.84); W26 745,173 / 753,134 = 0.99 (0.96); W27 9,247,732 /
    10,986,147 = 0.84 (0.89: Krummi 9,221,262 then, Voron 10,387,076); W28 114,865 / 129,932 = **0.88** (0.75); W29
    122,357 / 53,512 = 2.29 (2.24).
  - WSL 2 (`work/rr/final-linux.tsv`): W23 1.17 (1.11); W24 1.03 (1.03); W25 1.11 (1.18); W26 1.14 (1.13); W27 0.97
    (0.97); W28 70,442 / 70,502 = **1.00** (0.91); W29 1.82 (1.80).
  - Every table load meets K7 on both systems. The changes against L31 other than W28 are within the runs' variance:
    Krummi's figures are the same, and Voron's move by up to 6 %.
- **L41** (2026-11-12): final checks on `bb1408b`: fmt, clippy clean, 808 passed, 0 failed, 23 ignored. Closing report
  updated with the second round: [table-row-reads-report.md](table-row-reads-report.md). The branch stays unmerged and
  unpushed for the user's review.
- **L42** (2026-11-12) — Third round, user request «Hazlo todo» (after the Krummi and Voron only benchmark, see
  `odd/tasks/benchmark-krummi-voron.md`; estimate for the whole batch 6 h – 10 h). T9: Krummi-only W23 on Windows,
  pinned to the performance cores, bench power plan, nine release builds alternated in three rounds
  (`work/bkv/ab.py`, `work/bkv/w23-bisect.tsv`), medians of the three rounds: `bb8acca` 372,381; `a840f02` 369,281;
  `963d126` 369,026; `ed61733` 370,803; `e7dd8d2` 370,108; `5dcfa29` 372,066; `bb496a1` 371,052; `d6a88b7` 371,925;
  HEAD (`1531bd0`, same Krummi as `bb1408b`) 368,866. Every build's rounds span 361k–375k: there is no drop; L31's −3 %
  came from two alternating pairs within this spread.
- **L43** (2026-11-12) — T10: `Reader::check` takes a row that gives every column once in schema order without the
  name map, the `at` and `given` vectors (it can break none of the name rules, and its values are checked in the
  same schema order); `encode_key_of` encodes a one-column key without a staged vector. Guard test
  `a_row_in_schema_order_fails_and_is_stored_as_in_any_other_order` (first rule in schema order, same stored row,
  duplicate key). Krummi-only A/B against HEAD, pinned, power plan, three alternating rounds (`work/bkv/e1.tsv`),
  medians: W25 360,048 → **385,142 (+7.0 %)**; W23 373,629 → 387,893 (+3.8 %); W24 95,859 → 97,333 (+1.5 %). Every
  round of the candidate is above every round of HEAD in W25 and W23. `cargo test -p krummi`: 373 passed.
- **L44** (2026-11-12) — T11 (Codex proposal 5): `krummi_tree::last_leaf` finds the rightmost leaf (with its left
  sibling) and the greatest key; `krummi_tree::append_last` adds at the end of that leaf only when `try_add` would
  change that leaf and nothing else in the same way (the key beyond the leaf's last, the value in the leaf, no
  split, and a leaf left under a quarter full only when it is the root or a merge with its left sibling would not
  fit, as `merge_child` decides). The table state keeps the last leaf and the greatest key: `apply_rows` (every other
  change of the rows tree) and emptying the table drop the leaf, a removal forgets the greatest key, and an insert
  that walked the tree beyond the greatest key finds the last leaf again, once per leaf in an ordered run. Tests:
  `appending_to_the_last_leaf_changes_the_tree_as_adding_does` and `appending_after_removals_changes_the_tree_as_adding_does`
  (every node byte for byte against `try_add` alone), `appending_declines_where_adding_would_merge_the_last_leaf`
  (built state; a mutation that skips the merge check fails it, and the other two did not catch it), and the table
  model test `ordered_inserts_mixed_with_other_changes_keep_the_rows_in_order` (inserts in and out of order,
  deletes, ranges, updates in place and resized, upserts, emptying; mutations that keep the leaf after `apply_rows`
  or after emptying fail it). Krummi-only A/B against T10, three rounds (`work/bkv/e2.tsv`), medians: W25 381,525 →
  **469,456 (+23.0 %)**; W23 383,866 → **470,030 (+22.4 %)**; W24 +0.6 %; W26 +0.8 %; W27 −1.2 %; W28 −0.4 %; W29
  −1.1 % (the last four within their rounds' spread). `cargo test -p krummi -p krummi-tree`: 427 passed.
- **L45** (2026-11-12) — RDD of `18e38f7..a71297b` (T10, T11; escalated, item 1), lenses risk, reliability and
  readability: approved, lineage `review-c0f10da564cef53f`, acknowledged. R2-001 (WARNING, `append_last` repeated the
  merge rule of `merge_child` by hand): both call `merge_fits` now. R1-001 (SUGGESTION, tie the kept leaf to the
  root): the state keeps the root with the leaf and uses it only for that root. R3 (SUGGESTION, the table test did
  not show the fast path ran): unit test `ordered_inserts_add_at_the_end_of_the_last_leaf` (over 2,900 of 3,000
  ordered inserts add at the end, a test-only count; the same rows, root and count with the leaf dropped before each
  insert). R2-002 (SUGGESTION): `_folder`. Checks: clippy clean, `cargo test -p krummi -p krummi-tree` 428 passed.
