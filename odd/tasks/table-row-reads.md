# Table row reads without materializing values — ODD execution document

Objective: make the table loads that miss their K7 threshold (W27 `tabla-intervalos` on Windows and Linux, W26
`tabla-lecturas` and W28 `tabla-actualizacion` on Windows) meet it, as an optimization with no observable product
change (no PRD: annotation of 2026-11-04 to K13 of `docs/prd/krummi/benchmark.prd.md`; E11, E19 and the *Salida* of
`docs/prd/krummi/tables-schema.prd.md` unchanged). Branch: `feat/table-row-reads` (from `feat/tables-schema`).
Delivery: `ask-on-risk` (single branch, left unmerged). Test runner: `cargo test --workspace` (Windows; Linux in WSL 2
Ubuntu). Estimate: 6 h – 10 h. Real start: 2026-11-11 12:05:51 +0000.

## Specs

- **S1** — Acceptance, the annotation of 2026-11-04 to K13 (benchmark PRD), verbatim: «Una optimización que no cambia
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
| T3 | S1 (complete runs on Windows and Linux) | inline (background runs) | pending | |

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
