# Table scans: cheaper row scans with no behavior change

Objective: make every table row scan cheaper (public and internal), measured before and after. Branch
`perf/table-scans` from `feat/tables-vector-indexes` (`76d8820`). Delivery: `ask-on-risk`. Runner: `cargo test
--workspace`; measurements and profiles only in WSL 2 (user's answer, L2). Closing report: `odd/tasks/table-scans-report.md`.

## Specs

- **S1 Scope.** "Quiero abaratar, en una rama nueva, el recorrido de las filas de una tabla: cualquier recorrido, tanto
  los públicos (`range_rows`, `index_rows`, los recorridos con proyección) como los internos (los que filtran filas en la
  búsqueda vectorial, los de las estadísticas). Es una mejora de rendimiento sin cambio de comportamiento del producto."
- **S2 Baseline first.** "Mide antes de cambiar nada, en esta máquina, y guarda las cifras de partida":
  - "el banco: las cargas de lectura de tablas (W26 lecturas por clave, W27 rangos de la clave primaria, W31
    búsquedas por índice, W32 rangos de un índice) y, para comprobar que no empeoran, las de escritura (W23 a W25,
    W28 a W30)";
  - "un recorrido completo de una tabla de filas pequeñas (por ejemplo, la de W23) y de la tabla con vectores, con
    `range_rows` y con una proyección que lea una sola columna";
  - "el programa de `work\handoff\prof` (instrucciones abajo): modos `nearest`, `scan` y `scanv`".
  - "compara siempre antes y después en la misma máquina"; "Compara alternando antes y después en varias rondas, como
    hace el A/B del banco, y no con una sola ejecución de cada uno."
- **S3 Profile.** "Perfila esos recorridos y saca el desglose que falta, sobre todo a quién llama `Reader::decode` y
  qué pesa cada parte del coste por página."
- **S4 Changes.** "Haz los cambios que el perfil justifique, uno por commit, cada uno con su medida antes y después."
  Candidates:
  - "El filtro: que compare directamente los valores de tamaño fijo, sin codificarlos como clave, con el mismo
    resultado, incluidos los nulos de SQL, `IN` y `NOT IN` (VI7)."
  - "La decodificación por fila: los recorridos internos no usan las proyecciones (`tables-projection.prd.md`, ya
    implementadas), que preparan una vez lo que depende del esquema. Reutilízalas donde sirvan antes de escribir otro
    mecanismo."
  - "El coste por página: busca un camino más corto en un recorrido, por ejemplo cuando el área temporal no tiene
    versiones de esas páginas. Toca el pager y la capa de transacciones: no rompas ninguna garantía de integridad, de
    comprobación de páginas ni de aislamiento. Si no es un cambio pequeño y seguro, documéntalo y no lo hagas."
- **S5 Out of scope.** "guardar las columnas grandes fuera de la fila, al estilo de TOAST. Cambia el formato y necesita
  su propio PRD. No lo implementes; si lo ves útil, deja en el informe de cierre lo que se ganaría, con las medidas que
  lo respalden."
- **S6 Same behavior.** "Los recorridos deben devolver exactamente las mismas filas, en el mismo orden, y las mismas
  comprobaciones de daño. Antes de cada cambio, una prueba que fije el comportamiento actual. Pasa los checks de
  `CLAUDE.md` y la revisión RDD y la de Codex como dice el proyecto. El informe de cierre incluye la tabla de medidas
  antes y después."

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S2 | inline | done | L4 (`work/ts/base-bench.md`, `work/ts/base-prof.tsv`) |
| T2 | S3 | inline | done | L5 |
| T3a | S4, S6 | inline | done | `69a00a8a`, L6: per-page, table of checked nodes 65,536 → 524,288 slots |
| T3b | S4, S6 | inline | dropped | `786cd324` and its revert `ddd950f7` (tests kept), L7 |
| T3c | S4, S6 | inline | done | `2ef2e8ad`, L8: `TableRow::get` compares the names of ≤ 8 columns |
| T3d | S4, S6 | inline | done | `0541b9c2`, L9: filter on fixed-width values by rank |
| T3e | S4, S6 | inline | not done | L10: no small change that keeps S6 |
| T4 | S6 | inline | done | L14–L16, closing report |

Route: every task inline (no Writer trigger: the units depend on each other's measurements and share `rows.rs`).

## Log

- **L1** (2026-11-26, user's request, verbatim): "Lee work\handoff\perf-scan-prompt.md y haz lo que pide". The prompt
  is `work/handoff/perf-scan-prompt.md` (not tracked); S1–S6 quote it.
- **L2** (2026-11-26) Question "¿Dónde perfilo y dónde mido el antes y el después?" → user: "Solo WSL". Estimate
  6–10 h; start confirmed 2026-11-26 08:16:48 +0000.
- **L3** (2026-11-26) Setup: baseline clone at `~/ts/before` (WSL, `76d8820`) with its own target folder; the lab
  store at `~/ann/work/motor`, shared by both binaries (no format change); `perf_event_paranoid` set to 1.
  Bench power plan on (boost off) from 08:34. The program `work/prof` adds the modes `tscan`, `tscanp` (W23's table,
  1 M small rows) and `iscanp` (`index_rows` over all of `ix_ts` of W30) to the handoff's; bench stores in
  `~/ts/stores` (W23, W30, full scale).
- **L4** (2026-11-26) T1 baseline (WSL, `76d8820`). Profiling program, median of each run, 3 rounds:
  nearest 174–178 ms; scan 155–165; scanv 157–164; scanp ~105; tscan ~91; tscanp ~35; iscanp ~1,190 ms. Bench A/A
  (the base binary on both sides, 2 rounds, `ab_loads.py`): every ratio 0.987–1.009, so ~1 % is the noise; base
  ns/op W23 7,114, W24 15,953, W25 1,930, W26 1,319, W27 80.3, W28 13,231, W29 13,857, W30 24,986, W31 3,317,
  W32 1,262.
- **L5** (2026-11-26) T2 profiles (samply, `~/ts/prof-base`, aggregation `agg.py` in the scratchpad):
  - nearest: per-page 34 % (`node_at_with`: `node::is_valid` 13 %, `page_and_check` 11 %, `remember_checked` 3 %);
    `Filter::meets` 29 % (encode 7 %, memcmp 7.5 %); `TableRow::value` 10 %; `Reader::decode` 6.5 % self.
    `Reader::decode` callers here: only `passing`.
  - scanp: `node_at_with` 57 % (`is_valid` 22 %). The table of checked nodes has 65,536 slots (512 MiB of pages) and
    the vector table takes 4.2 GB, so most leaves are checked again on every pass. Experiment (`~/ts/exp`, 1 << 20
    slots, 3 alternated rounds): nearest 175→158 ms, scan 157→145, scanp 105→85; tscanp and iscanp unchanged.
  - tscan: `TableRow::get` 52 % inclusive, of which the name's SipHash 16 %; `row` (iterator decode) 36 %.
  - tscanp: the projection read 29 %, `lend` 25 %; per-page cost small (the table fits the memory).
  - iscanp: a full root-to-leaf `krummi_tree::get` per entry, rows at random: key search in branches 31 %, page
    reads 28 %, memcmp 16 %. Inherent to reading rows in another order; no small change found.
- **L6** (2026-11-26) T3a `69a00a8a`: `CHECKED_SLOTS` 1 << 16 → 1 << 19 (4 MiB per open store, nodes of a 4 GiB working
  set; C5 of the node check cache PRD keeps a fixed, internal size). Default taken without asking: 1 << 19 rather than
  1 << 20, as it gives the same gain at half the memory. Tests: the integrity tests (`integrity_tests`, I1–I11) and
  `checked` pin the behavior and use `with_checked_slots`. A/B base → T3a, 3 alternated rounds (`~/ts/t3a.tsv`):
  nearest 176.6 → 157.4 ms (0.891), scan 162.7 → 140.8 (0.866), scanp 103.8 → 87.2 (0.841), tscan 0.988, tscanp
  0.990, iscanp 0.998.
- **L7** (2026-11-26) T3b: a fixed-width column of a current row read from its slot, without `RowLayout::row`. Tests
  first (`every_fixed_width_column_reads_the_value_written`, `ac_l2_a_fixed_width_column_that_does_not_decode_panics_
  when_read`), passing before and after. A/B T3a → T3b (`~/ts/t3b.tsv`): nearest 1.171, scan 1.081, tscan 1.187 (slower:
  `decode_with` no longer inlined into `TableRow::value`, profile `~/ts/prof-t3b`). With `decode_with` forced inline
  (`~/ts/t3b2.tsv`): nearest 0.986, tscan 0.999. No measured gain, so the code was reverted (`ddd950f7`); the tests stay.
- **L8** (2026-11-26) T3c `2ef2e8ad`: `TableRow::get` compares the names of a table of at most 8 live columns instead of
  hashing them. Test first: `e11_a_column_is_read_by_its_exact_name` (1 to 40 columns, names differing in case, prefix
  and length), passing before and after. A/B T3a → T3c (`~/ts/t3c.tsv`): tscan 91.1 → 66.2 ms (0.727), scan 142.8 →
  120.6 (0.844), nearest 1.007 (does not read by name).
- **L9** (2026-11-26) T3d `0541b9c2`: the filter compares a fixed-width value (not text) by `element_rank`, a `u128`
  that orders as its element, without encoding the row's value. Tests first: `the_filters_order_every_fixed_width_type`
  (every fixed-width type, with nulls, extremes, infinities, NaN, `-0`, both UUID orders, IN with a null, NOT IN with
  and without one, with and without an index), passing before and after; then `element_ranks_order_as_the_elements` and
  `a_value_of_another_type_has_no_rank`. Known environmental failure on this Windows machine, also at the base
  `76d8820`: `definitions::ac_vi9_the_changes_of_the_table` (`os error 33` copying an open store's file).
- **L10** (2026-11-26) T3e: after T3d, `passing` spends ~7.6 % reading the vector of every row before the filter, and
  ~9 % in `page_and_check` (about 25 ns per page). Not done: reading the filter first changes which malformed rows
  panic (a row with a null vector and a malformed filter column would panic where it does not), against S6; the
  per-page path is already the remembered one, and shortening it touches the pager's isolation, not a small change.
  A/B T3c → T3d (`~/ts/t3d.tsv`, 5 rounds): nearest 157.2 → 138.0 ms (0.878).
- **L11** (2026-11-26) RDD `review-a005a2d2888e3ecd`, `76d8820..0541b9c2`, lenses reliability, readability,
  resilience: approved, acknowledged. R2-001 (WARNING, `filter.rs`: the comment of `Test<T>` said the values are what
  rows are checked against) fixed; R2-002 (SUGGESTION, `checked.rs`: a one-off figure in the comment of
  `CHECKED_SLOTS`, and why 1 << 19) fixed; R3-decimal-rank-unchecked (WARNING: the decimal rank does not check
  precision or scale) dismissed: the element does not either (`encode_element`, `key.rs`: the mantissa only), and a
  stored decimal over its precision panics in `decode_with` (`fits_precision`) before the filter reads it.
- **L12** (2026-11-26) Final A/B, base `76d8820` against `0541b9c2` (WSL, bench power plan). Profiling program, 3
  alternated rounds (`work/ts/final-prof.md`): nearest 175.8 → 136.3 ms (0.775), scan 161.3 → 122.1 (0.757), scanv
  162.4 → 117.2 (0.721), scanp 105.1 → 85.3 (0.812), tscan 90.9 → 66.4 (0.731), tscanp 1.003, iscanp 1.012. Bench,
  3 rounds (`work/ts/final-bench.md`, op/s branch / base): W23 1.025, W24 1.031, W25 1.007, W26 0.989 (3 of 3 slower),
  W27 1.001, W28 1.014, W29 0.997, W30 1.013, W31 0.989, W32 1.004. W26 and W31, the point reads, get 10 rounds.
- **L13** (2026-11-26) W26 and W31, 10 alternated rounds (`work/ts/w26-bench.md`): W26 0.988 (4 of 10 rounds slower),
  W31 1.005 (0 of 10). The A/A run gave W26 0.999 and loads down to 0.987: W26's −1.2 % is within the noise. Bench
  power plan restored to Balanced.
- **L14** (2026-11-26) T4 checks on Windows at `3fa7a3a8`: `cargo fmt --all --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean; `cargo test --workspace --no-fail-fast` 1,343 passed, 3 failed, all three in
  `table_vector_indexes`: `definitions::ac_vi9_the_changes_of_the_table` (`os error 33`), `worker::ac_vi3_the_index_
  follows_the_writes` and `worker::ac_vi15_the_lists_of_large_tables` (their 120 s limits, with the binary's other
  tests in parallel). Known environmental failures: the same three fail with the base's code (`76d8820`, whole binary:
  32 passed, 3 failed, 262 s; the branch 33 passed, 3 failed, 213 s); `worker::` alone passes on both (Windows 39.6 s
  and 40.6 s, Linux 46.4 s and 40.6 s), and the whole binary passes `ac_vi9` on Linux.
- **L15** (2026-11-26) Codex R1 (`docs/reviews/table-scans/2026-11-26-code.md`, `76d8820..8ed2a76f`): findings, R1
  blocker: a range without bounds on a text or bytes column took the rank path and passed no row. Fixed in `19f1e784`
  with the regression test `an_unbounded_range_on_text_or_bytes_passes_every_row_that_is_not_null` (RED before the
  fix). Codex R2 (`2026-11-26-code-02.md`, follow-up `8ed2a76f..a36cc6c8`): approved, R1 verified. Resolutions:
  `docs/reviews/table-scans/2026-11-26-code-resolutions.md`.
- **L16** (2026-11-26) RDD `review-1d16c5c961158245`, `0541b9c2..137802c0`, lenses reliability, readability:
  approved, acknowledged; two SUGGESTION findings left as non-blocking follow-ups (a check that every fixed-width
  type has a rank; the unbounded text range through the approximate and pending paths). Closing report:
  [table-scans-report.md](table-scans-report.md).
- **L17** (2026-11-26) Rebase. The base moved: `feat/tables-vector-indexes` is at `d519e9a5`, 13 commits after `76d8820`
  (the vector worker starts when the store opens and reads after durable commits; `check_tests::open_settled`).
  Estimate for the rebase, checks and new measurements: 2 h to 3 h 30 min; start 2026-11-26 12:46:36 +0000. The
  rebase onto `origin/feat/tables-vector-indexes` applied the 15 commits without conflicts once run with
  `merge.directoryRenames=false` (with rename detection, git moved this run's `odd/tasks` files into `odd/done/tables`,
  as the base had archived the only other execution). No new test counts reads after opening a store, so none needs
  `open_settled`. Checks on Windows at `6bc23bd8`: fmt and workspace clippy clean; `cargo test --workspace
  --no-fail-fast` 1,346 passed, 4 failed: the three known environmental failures of L14 and
  `table_statistics::codex::r13_analyze_without_memory_for_its_snapshot_changes_nothing`, which then passed 6 times
  out of 6 with its whole binary. Its cause is the base's: the test injects an out-of-memory failure into the next
  snapshot of the opener's fault context, and the vector worker adopts that context (`vector_worker.rs:166-171`) and,
  since `433843ad` and `02e3ccb2`, takes a snapshot after a durable commit, so it can take the failure meant for
  `analyze`. Bench power plan on for the new measurements.
