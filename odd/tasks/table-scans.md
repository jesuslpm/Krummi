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
| T3a | S4, S6 | inline | pending | per-page: larger table of checked nodes |
| T3b | S4, S6 | inline | pending | per-row: `TableRow::value` of a fixed column of a current row without `RowLayout::row` |
| T3c | S4, S6 | inline | pending | per-row: `TableRow::get` by name without hashing for few columns |
| T3d | S4, S6 | inline | pending | filter: fixed-size values compared as order-preserving integers |
| T3e | S4, S6 | inline | pending | `passing`: whatever the profile after T3a–T3d still shows |
| T4 | S6 | inline | pending | closing (checks, RDD, Codex, report) |

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
