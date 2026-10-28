# Large values (5.3) — ODD execution document

Objective: implement `docs/prd/krummi/trees-large-values.prd.md` (decisions V1–V11, criteria AC-V1–AC-V14), the
authoritative specification; its ancestry is `trees.prd.md` (module 5, B1–B12) and `krummi.prd.md` (root), and it
reuses its siblings `trees-keys.prd.md` (5.1, A1–A15) and `trees-scan.prd.md` (5.2, R1–R11), with the pager,
transactions, journal, journal-group-commit and free-space PRDs inherited.
Branch: `feat/trees-large-values`. Delivery: single feature branch, left unmerged for the user's review (default
taken without asking, CLAUDE.md *Questions first*). Test runner: `cargo test --workspace` (Windows; Linux in WSL 2
Ubuntu). Estimate: 6 h – 9 h. Real start: 2026-10-27 18:39:54 +0000.

## Specs

Each spec quotes an acceptance criterion verbatim; the decisions it names (V#, and the inherited ones) are part of
it. The PRD's *Validaciones y mensajes*, *Salida*, *Fuera de alcance* and *Dejado a la implementación* bound every
task. The parent and root criteria assigned to 5.3 are inside them: AC-B3 in S1, AC20 in S3, AC-B17 in S6, AC-B18 in
S8, AC10 in S11, AC-B12 in S12. The PRD and its annotations to earlier PRDs are committed (`27a5e5b`).

- **S1** — **AC-V1** — Dada una escritura, cuando guarda con una clave de 2.048 bytes un valor vacío y otro de 268.435.456 bytes, entonces se leen idénticos tras confirmar y, en disco, también tras reabrir, y un recorrido del árbol los devuelve; cuando el de 268.435.456 bytes se sobrescribe con uno de 1 byte y después se quita, entonces Leer devuelve el valor nuevo y después que la clave no está; y cuando intenta guardar o añadir uno de 268.435.457 bytes, entonces falla con la clase de tamaño no válido sin guardar nada, como valor, con el tamaño recibido, mínimo `0` y máximo `268435456`, y la escritura sigue utilizable (V1, AC-B3).
- **S2** — **AC-V2** — Dado un árbol, cuando se guardan, con claves de 1 y de 2.048 bytes, valores de 1.024 y 1.025 bytes, de 1 MiB y 8 MiB, y de un byte menos, igual y uno más que cada uno de estos límites, que las pruebas toman de la implementación: el umbral de la hoja y el mayor valor que cabe en 1, en 2 y en 3 páginas fuera de la hoja, entonces Leer los devuelve idénticos en la misma escritura, en una lectura posterior y, en disco, tras reabrir; y cuando se sobrescriben con valores mayores, menores y vacíos, se añaden sobre claves existentes y se quitan, entonces Leer devuelve siempre el último valor guardado o que la clave no está, y el número de entradas coincide con las claves que Leer encuentra (V1, V2, A4, A5).
- **S3** — **AC-V3** — Dado un valor fuera de su hoja, cuando se lee, entonces la referencia está dentro de la memoria del motor que fija T16 y no en una copia, en cada uno de estos casos: en disco, una versión ya escrita en su sitio en `krummi.data`, dentro de la proyección de `krummi.data`; en disco, una versión confirmada que todavía no se ha escrito en `krummi.data`, y una versión anterior que una lectura sigue viendo después de que una escritura posterior sobrescribiera el valor, dentro del área de versiones de `krummi.temp` donde el motor la conserva (J7, T14, T16); en modo temporal, esos mismos casos dentro de la memoria del almacén o de la memoria donde el motor conserva las versiones; en modo temporal, un valor guardado cuando el almacén tiene que crecer y su rango, con páginas consecutivas, atravesaría el límite entre la memoria que el almacén ya tenía y la que añade al crecer; y en una escritura que lee el valor que acaba de guardar, dentro de su propia copia. En todos los casos el valor se devuelve como una sola secuencia contigua, con los bytes idénticos a los guardados, y usar la referencia después de terminar la transacción no compila (V2, AC20).
- **S4** — **AC-V4** — Dado un árbol con valores dentro y fuera de su hoja, cuando se recorre hacia delante, hacia atrás y con prefijo, entonces cada entrada sale con su valor completo e idéntico, como una sola secuencia sin copia; y cuando se quitan intervalos que contienen valores fuera de su hoja, entonces Quitar intervalo devuelve cuántas entradas quitó y un recorrido del árbol entero devuelve las demás con sus valores (V2, R1, R7).
- **S5** — **AC-V5** — Dado un almacén con un árbol ya creado, que se mantiene dentro de una sola región del espacio libre (con valores cuyo tamaño no obliga a saltar de región, F5), tomando las páginas en uso como en AC-A15: cuando se crea un árbol vacío, se confirma, se toma el valor, se le guardan valores dentro y fuera de su hoja, se sobrescriben con valores mayores y menores, se confirma y después se vacía, entonces el valor final es el tomado con el árbol vacío; lo mismo cuando se le quitan todas las entradas con Quitar intervalo; cuando el árbol se borra o se sobrescribe con Renombrar, el valor final es el de antes de crearlo cuando su entrada no hizo crecer el catálogo (las pruebas lo saben por la implementación), como en AC-A15; y cuando se guarda un valor fuera de su hoja de 1 MiB, se confirma, se sobrescribe con uno de 64 KiB que sigue fuera de su hoja y se confirma, entonces cada vez las páginas en uso crecen, respecto del árbol sin ese valor, como mucho en las páginas que llenarían sus bytes, redondeadas hacia arriba, más una, y las que crecen por la entrada de la hoja (V5, A2, A3, R7, F10).
- **S6** — **AC-V6** — Dada una transacción de lectura abierta con referencias a valores fuera de su hoja, y una escritura que sobrescribe y quita esos valores, y divide y fusiona hojas con valores dentro y fuera de ellas hasta cambiar la altura del árbol, cuando se confirma y otra escritura reutiliza las páginas liberadas (F6), entonces los bytes de las referencias de la lectura no cambian, la lectura sigue leyendo y recorriendo exactamente los valores de su instantánea, y una lectura nueva ve los de la escritura (V2, G3, T14, T16, AC-B17).
- **S7** — **AC-V7** — Dado un valor fuera de su hoja con un byte alterado en su primera, en una intermedia y en su última página, en pruebas separadas, cuando se lee o un recorrido necesita su entrada, entonces falla con la clase de daño y el número de una página del valor, y vuelve a fallar igual al repetirlo; las entradas que el recorrido devolvió antes son correctas y las demás claves se leen sin error; cuando una escritura guarda sobre esa clave, la quita o quita un intervalo que la contiene, entonces no falla, se confirma y Leer devuelve el valor nuevo o que la clave no está; cuando añade sobre esa clave, entonces devuelve que no la añadió sin fallar, y Leer sigue fallando con la clase de daño; y cuando una escritura vacía el árbol, lo borra o lo sobrescribe con Renombrar, entonces no falla y se confirma (V7, A9, G6, R8).
- **S8** — **AC-V8** — Dada una escritura que guarda, sobrescribe y quita valores fuera de su hoja, cuando se provoca una caída del proceso y simulada de corriente en cada punto en que su confirmación escribe o lleva datos al disco, entonces al reabrir cada clave tiene su valor de antes o el de después, todas a la vez, y el número de entradas coincide; cuando se simula un fallo de T11 distinto de entrada y salida al escribir en cada punto en que una operación reserva, lee o libera páginas, también al reservar el rango de un valor, entonces confirmar falla con la clase del primer fallo y no aplica nada, y tras abortar la escritura el árbol y su número de entradas están como antes y una escritura nueva funciona sin error; y cuando se simula disco lleno o memoria agotada durante la propia confirmación, entonces falla con esa clase, termina la transacción sin aplicar nada y una escritura nueva funciona sin error (V3, V8, G1, G5, T11, AC-B18).
- **S9** — **AC-V9** — Dado un almacén con páginas libres sueltas suficientes para un valor pero sin un hueco contiguo de su tamaño, y cuyo final está en la frontera del espacio libre (la prueba lo prepara sabiendo la implementación), cuando se guarda ese valor y se confirma, entonces se guarda, el número de páginas del almacén (P12) crece al menos en las que ocupa el valor y las páginas libres sueltas siguen libres; y con un tamaño máximo que no deja crecer lo suficiente, entonces la operación, o a más tardar la confirmación, falla con la clase de tamaño máximo, no aplica nada y una escritura nueva funciona sin error (V6, F5, T11).
- **S10** — **AC-V10** — Dada una escritura en disco que solo hace Añadir con un valor de 8 MiB sobre una clave existente, Quitar sobre claves que no están y operaciones que fallan por tamaño no válido con un valor de 268.435.457 bytes, cuando se confirma, entonces termina sin error y los archivos del almacén quedan idénticos byte a byte; Añadir con un valor de 268.435.456 bytes sobre una clave existente, con un tamaño máximo que no le deja sitio, devuelve que no la añadió sin fallar; y un valor de 268.435.457 bytes a través de una apertura de un árbol borrado, o en una escritura que ya tuvo un fallo de T11, falla con tamaño no válido (V1, A7, A10, B7, T10).
- **S11** — **AC-V11** — Dado un almacén pequeño con un valor fuera de su hoja, cuando se altera cada uno de sus bytes por separado y se abre y se lee el valor, entonces cada alteración se detecta como daño o no tiene efecto, con las reglas de AC10 de la raíz y sus anotaciones; nunca se devuelve el valor con un byte distinto (V7, G6, AC10).
- **S12** — **AC-V12** — Dado el almacén de formato `6` conservado como dato de prueba, cuando se abre, entonces falla con la clase de formato distinto como formato anterior y no se modifica; un almacén nuevo informa la versión de formato `7`; y leyendo en disco los bytes de la entrada de un valor fuera de su hoja, con el conocimiento de la implementación, su página y su longitud están en little endian (V3, V9, AC-B12).
- **S13** — **AC-V13** — Dado el informe de cierre, cuando se revisa, entonces contiene la medición de V11 con sus tiempos y tamaños del almacén por tamaño de valor y de la carga mixta, y los datos del equipo, en Windows y en Linux (V11).
- **S14** — **AC-V14** — Dado el informe de cierre, cuando se revisa, entonces contiene la evaluación de V4 con al menos los dos umbrales que nombra, los datos de la medición de cada uno, la complejidad que añade cada uno, el elegido y por qué (V4).

## Design

Decisions within *Dejado a la implementación*:

- **D1 — Value run (format 7).** An off-leaf value of `len` bytes occupies a run of `N = ceil((len + 16) / 8192)`
  contiguous pages (so at most `ceil(len / 8192) + 1`, V5). The run's first 16 bytes are its only header: bytes
  `0..8` the xxh3-64 of bytes `8..N*8192` of the whole run, bytes `8..16` the first page number (LE). The value
  follows at byte 16; the rest is zero. Interior pages carry no header. The value length lives in the leaf, not in
  the run (V7: freeing a run never reads it). A damaged run reports its first page number.
- **D2 — Leaf cell.** A value-length field of `0xFFFF` marks an off-leaf value; the cell body is then the key plus
  16 bytes: first page (u64 LE) and value length (u64 LE) (V3: little endian).
- **D3 — Pager.** `Pager::run(first, count) -> Result<&[u8]>` returns the run's bytes contiguously after checking
  its checksum. On disk it slices the newest mapping (already contiguous). In temporary mode the store's memory
  becomes one reserved address range, committed as the store grows, so any range is contiguous and no page moves;
  the reservation is the maximum size when there is one and a large fixed default otherwise (the default is
  recorded as a limitation; exceeding it fails with memory exhausted, P13).
- **D4 — Write transaction.** A value run is reserved with the existing range reservation (F5) and kept as one
  contiguous dirty buffer. Runs are immutable: overwriting a value writes a new run and frees the old one (the
  PRD leaves reuse open; F6 means the old pages are not reusable in the same write). Reading a run resolves dirty
  buffer → retained version → pager.
- **D5 — Commit, journal, retained versions, transfer and recovery.** Runs are sealed as a whole. In the journal
  their pages travel as raw entries (a flag bit in the page number) that recovery writes without the per-page check;
  the record checksum covers them. The temporary area stores a run in contiguous slots and keeps one version entry
  per page pointing into it, so `Snapshot::run` returns it contiguously; transfer writes and retires a run as a unit.
- **D6 — Tree.** Insert, overwrite, add, remove, remove range, empty, delete and rename-overwrite allocate and free
  runs; freeing reads leaves but never value pages (V7). Get and scans resolve a run to the value slice.
- **D7 — Leaf threshold (V4).** Half page: an entry stays inline when `4 + key + value` is at most 4,078 bytes, so
  every leaf holds at least two entries and the 2-way split holds; otherwise its value goes to a run. T5 evaluated
  the full page (a cell up to 8,158 bytes, with a split into three) with a temporary knob and the V11 measurement,
  kept the half page and removed the knob and the prototype (`odd/tasks/trees-large-values-measurements.md`).
- **D8 — Public API.** `MAX_VALUE` = 268,435,456; signatures and lifetimes unchanged.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T0 | — | inline | done | — |
| T1 | S12 (format), pager part of S3, S8, S11 | delegated (context backstop) | done | `656acdf` |
| T2 | transaction part of S3, S6, S8 | delegated (context backstop) | done | `244548d` |
| T3 | S1, S2, S3, S4, S5, S7, S9, S10, S12 (leaf part) | delegated (context backstop) | done | `9a596b2` |
| T4 | S6, S8, S11 (tree level), L7 fixes | delegated (context backstop) | done | `d0be3f0` |
| T5 | S13, S14, L9 fixes | delegated (context backstop) | done | `899244a` |
| T6 | docs, checks, verification, RDD review, Codex review, closing report | inline | done (Codex round 3 pending, L18) | `46fdd12`, `7b4286b`, `3a528ec`, `c0b8d1a`, `e1cc4ab`, `71fde57` |

Route evidence: the tasks depend on each other (pager → transaction → tree → acceptance → measurement), so there are
no parallel units; the parent context is past the long-session backstop (the PRD was written in this same session),
so each task goes to one bounded ODD Worker (`opus`, `high`), one after another. D1 changes the persisted format
(high-risk item 1): the independent verifier runs at the end of the feature.

## Log

- **L1** (2026-10-27, user): "Da igual, empecemos la implementación del trees-large-values", then "Sí" to the
  estimate "6 h – 9 h".
- **L2** (18:40–, exploration): today every layer works page by page: each page has its own 16-byte header
  (`krummi-pager/src/format.rs`), the write keeps one boxed payload per page, the temporary area stores single-page
  slots, the journal and recovery verify each page, and temporary-mode memory grows in separate chunks that a range
  can straddle. `unsafe` is allowed only in `krummi-pager` (workspace lint and `unsafe_policy.rs`). Tests that
  hard-code format 6 and the 5.1 fixture (`crates/krummi/tests/data/store-5.1`) must change.
- **L3** (18:55): Engram mirror `odd/trees-large-values/tasks` pending: Engram refuses writes in this session
  (the host session was ended when VS Code closed and the resumed session cannot re-register). The task file is
  the only copy until a new session resynchronizes it.
- **L4** (T1, ODD Worker `opus`/`high`): `seal_run`/`verify_run` (a one-page run is a sealed page), `Pager::run`,
  `write_run`/`write_run_shared`, format 7, journal entries flagged `RAW_ENTRY` (bit 63) that recovery writes
  without the per-page check, temporary area runs in contiguous slots (`store_run`, `Slot::bytes`/`page_at`), and
  temporary stores in one reserved address range (`sys::Reservation`, 64 GiB without a maximum size). RED observed
  per item (compile failures, then Damaged in recovery before the change). `rustix` gains `mm` and `windows-sys`
  gains `Win32_System_Memory`; `Cargo.lock` unchanged. The parent fixed the format assertion of
  `krummi-tx/tests/recovery.rs` (outside the worker's surface). Checks reported by the worker: fmt, clippy and
  `cargo test --workspace` green on Windows and Linux; parent spot check: fmt and the recovery test pass.
- **L5** (RDD review 1): commit `656acdf` (base `27a5e5b`), assessed `high`, raised with `--escalate-item 1`
  (persisted format). Lenses risk, resilience, reliability; lineage `review-6ccf3988157feb31`; **approved**,
  acknowledged, authority burned. Advisory findings, all non-blocking:
  - R1-001, R4-fixed-reservation, R3-temporary-reservation-open-failure (WARNING): a temporary store reserves
    64 GiB (or its maximum size) at open, which fails under an address-space limit or a very large maximum. Fix in
    T2: reserve the smaller of the maximum size and the default, so a large maximum no longer fails at open.
  - R4-unaligned-commit (WARNING): committing from a length that is a multiple of 8 KiB is unaligned on hosts with
    16 or 64 KiB pages. Not a supported platform (D3 of the root: x64), but cheap: fix in T2 by committing from the
    OS-page-aligned start.
  - R4-first-fit-scan (SUGGESTION): one-page stores now scan the free ranges. Fix in T2 with a one-page fast path.
- **L6** (T2, ODD Worker `opus`/`high`): `allocate_value_run`, `run`, `may_hold_run` on both transactions, runs as
  one contiguous dirty buffer (`runs: BTreeMap<first, RunBuf>`, counted like pages in `memory()`), commit seals each
  run, journals its pages as `RAW_ENTRY` references and stages it with `store_run` (one version per page with its
  index), `Snapshot::run` returns a run only when it is whole in one slot or wholly in the store (else damage).
  Transfer writes versions in commit order (an older run written whole could overwrite a newer run that reused its
  pages) and retires a run as a unit. L5 fixes: (a) reservation `min(max_size, default)`, RED on Windows; (b) commit
  from a 64 KiB-aligned start, RED on Linux (`mprotect` EINVAL); (c) dismissed with evidence: the first-fit lookup
  already stops at the first free range for one page (every range has at least one slot), so no test can fail
  before a fast path; a doc sentence explains it. New `krummi-tx/tests/runs.rs` (9 tests: read-back in all places,
  references, AC-V6 variants, transfer ordering, crash/power-cut sweep, T11 at 4 points × 4 classes × 2 modes,
  commit-time disk full and memory). The new tests were written before the API but not run RED; behavioral RED was
  observed by reverting the ordering and whole-run retirement choices (`Damaged{page:4}`). Worker checks: fmt,
  clippy, `cargo test --workspace` (352 passed) on Windows, crate tests on Linux; parent spot check: `runs.rs`
  9 passed. Risk: items 1 and 4.
- **L7** (RDD review 2): commit `244548d` (base `656acdf`), assessed `high`, raised with `--escalate-item 4`
  (transfer ordering and shared writes). Lenses risk, resilience, reliability; lineage `review-033cd994e42cec40`;
  **approved**, acknowledged, authority burned (a first attempt never started: the driver called WSL's `bash`).
  Advisory findings, all non-blocking, fixed in T4:
  - R3-read-may-hold-run-checks-first-page-only (WARNING): the read side checks only the first page, not that the
    run ends within the used pages.
  - R3-snapshot-run-damage-paths-untested (WARNING): wrong count, a middle page, and a range partly in the
    temporary area and partly in the store reach the damage branches of `Snapshot::run` without a test.
  - R3-stage-run-length-not-asserted (SUGGESTION): `stage` documents a panic for a partial page that it never
    checks.
- **L8** (T3, ODD Worker `opus`/`high`): leaf cells with value length `0xFFFF` hold the run's first page and the
  value length (LE); `node::value` returns `Value::Inline` or `Value::Run`; `MAX_VALUE` 268,435,456; half-page
  threshold (`MAX_LEAF_CELL` 4,078: an entry stays inline when `4 + key + value` fits); `set_inline_limit` knob
  (thread-local, can only lower the limit, reads ignore it) for T5. Overwrite writes a new run and frees the old one;
  Add on an existing key reserves nothing; Remove, Remove range, `free_subtree` (now always reads leaves) and
  `free_all` free runs without reading value pages. RED: compile failures, then with stubs every new test failed at
  the first value over 1,024 bytes. GREEN: `tree_values.rs` (S1, S2, S4, S5, S7, S10, S12 leaf part) and the unit
  tests `crates/krummi/src/value_tests.rs` (S3, S9: they need engine hooks). Existing tests changed: the maximum
  (AC-A17 now stores 1,024 and 1,025 bytes), and two precedence tests (`ac_a13`, `ac_r11`) damage a value page
  instead of a leaf, which deleting now reads. Worker checks: fmt, clippy, `cargo test --workspace` on Windows,
  crate tests on Linux; parent spot check: `tree_values` 7 passed. Note for T4: `WriteTransaction::run` reports
  misuse, not damage, for a checksum-valid leaf that names pages the write changed or freed (only reachable with a
  crafted leaf).
- **L9** (RDD review 3): commit `9a596b2` (base `244548d`), assessed `high`, raised with `--escalate-item 1`
  (stored values written and freed). Lenses risk, reliability, readability; lineage `review-27cf815bcee22c3d`;
  **approved**, acknowledged, authority burned. Advisory findings, all non-blocking:
  - R1-free-subtree-now-reads-leaves and R3-free-subtree-now-reads-every-leaf (WARNING): emptying, deleting,
    removing an inner subtree or rename-overwriting a tree now reads every leaf, so a damaged leaf makes them fail
    where the baseline read only branches. Accepted: A9 lets those operations fail with damage on any page of the
    tree and V7 protects only value pages; a per-tree "has off-leaf values" marker would be a format addition for a
    damaged-store case, out of this PRD. Recorded for the closing report.
  - R2-change-leaf-len-shadowing (WARNING), R2-duplicated-cell-header-constant, R2-test-hooks-in-public-api and
    R2-free-runs-split-protocol (SUGGESTION): readability. Fix in T5, which changes the same code.
  - R3-crafted-run-reference-edge-cases-untested (SUGGESTION): a checksum-valid leaf naming a run of length 0 or
    overlapping a tree node is only reachable with crafted bytes. Fix in T5: `is_valid` rejects an off-leaf length
    of 0, with a test that such references end as damage.
- **L10** (T4, ODD Worker `opus`/`high`): AC-V6 with heights 2 → ≥4 → 3 and page reuse (`value_tests.rs`), AC-V8
  crash sweep (`tree_integrity.rs`), T11 sweep over off-leaf values with four classes (`src/tests.rs`, sampled step
  13; full sweep `#[ignore]`, run once, passed) and disk full / memory exhausted during the commit; AC10 sample with
  a value run (23.2 s, was 18.3 s; the full `#[ignore]` sweep, about 2 h, not run). L7 fixes: (a) read-side
  `may_hold_run` bounds the run by the store's pages, RED observed; (b) `Snapshot::run` damage branches tested; (c)
  `stage` asserts whole pages, RED observed (the panic came from `temp.rs`). Item 5 left: a write reports misuse
  only for a crafted checksum-valid leaf naming pages it changed; no sweep reaches it. Worker checks: fmt, clippy,
  `cargo test --workspace` on Windows (about 4 min), `-p krummi -p krummi-tx` on Linux; parent spot check: the
  AC-V8 unit tests pass. RDD: assessed `medium`, `under_budget` (pending in the slice after `9a596b2`).
- **L11** (T5, ODD Worker `opus`/`high`): measurement example `value_timing` (V11; 3 and 6 KiB writes hold the whole
  entries that fit in 16 MiB: 5,461 and 2,730), 70–78 s per run on each OS; results and the threshold evaluation in
  `odd/tasks/trees-large-values-measurements.md`. Full-page prototype (+84/−38 lines and +81 test lines, 3-way split;
  RED at `split_point`'s assertion, GREEN with 23 three-way splits in the model test) against half page: storing
  4–6 KiB values about 80% slower on both OS, reads within noise, mixed load +3.5% pages in use. Half page kept;
  prototype and the `set_inline_limit` knob removed. Values of exactly 8 KiB and 64 KiB take one extra page (the
  16-byte run header), within V5. L9 fixes: names in `change_leaf`, `node::LEAF_CELL_HEADER`, test hooks
  `#[doc(hidden)]`, `free_runs(tx, page, indexes)`, and `is_valid` rejects an off-leaf length of 0 (RED: damage
  reported on a value page instead of the leaf). Worker checks: fmt, clippy, `cargo test --workspace` on Windows
  (308 passed, 14 ignored), tree and krummi tests on Linux. The parent added `value_timing` to the README.
- **L12** (RDD review 4, last slice): commits `d0be3f0`..`fa3f291` (base `9a596b2`, last assessment `medium`,
  `under_budget`; reviewed by the CLAUDE.md closing rule). Lenses risk, reliability, readability; lineage
  `review-6e7966cf3bbe5d18`; **approved**, acknowledged, authority burned. Seven SUGGESTION findings. Fixed in
  `3a0b336`: R1-max-inline-value-unchecked-subtraction (`saturating_sub` again),
  R2-value-timing-hardcoded-write-count (derived from `TOTAL / PER_WRITE`), R3-commit-failure-loop-accepts-swallowed-
  injection (the `Ok` branch asserts the failure did not fire). Accepted without change: R2-commit-sweep-unexplained-
  value-size (the doc comment names why 48 pages and `failures > 0` guards it), R2-cross-coupled-test-modules and
  R2-duplicated-pattern-helper (integration tests cannot share a crate-private unit-test helper; one 4-line
  generator per side), R3-read-may-hold-run-uses-live-page-count (a weaker guard reachable only with a crafted
  checksum-valid leaf; wrong bytes still fail as damage).
- **L13** (final checks): the first run failed one test on Windows, `runs.rs::a_read_keeps_a_run_that_later_writes_free_and_reuse`
  at its pointer comparison, and could not run on Linux (`cargo` missing from the non-login shell of the parent's
  script). The test raced the background transfer: it paused the transfer after the first commit, so the run could
  already be in `krummi.data`; the bytes were right, only the pointer it compared had moved. It passed 8 of 8 alone.
  Fixed in `46fdd12` (pause before the first commit; the other pointer tests already did). Rerun: `cargo fmt --all
  --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test --workspace` 369 passed, 0
  failed, 13 ignored on Windows and on Linux (WSL 2 Ubuntu).
- **L14** (Codex code review, round 1): `docs/reviews/trees-large-values/2026-10-27-code.md` over `27a5e5b..46fdd12`;
  wrapper status `findings`, `threadId` `01a17f4a-20f3-75d1-a3c4-669c5f80a8fe`, coordinator `gpt-6.1-sol`/`high`,
  reviewer `krummi-review-critical` `gpt-6-astra`/`xhigh`. One open finding, R1 (major): a chunk of the temporary
  area made for one oversized run is kept until close and its capacity is not counted against the maximum size, so
  repeated slightly larger overwrites grow `krummi.temp` past `max_size`. Verified against the code (`temp.rs` grows
  a chunk of the run's size and releases chunks only when the area is removed): a demonstrated defect against the
  PRD's *Dejado a la implementación* (that memory counts for the maximum size). Fix delegated (ODD Worker).
- **L15** (independent verifier, `sonnet`/`high`, on `4eb4807` in scratch clones): no blockers; S1–S12 met, S13 and
  S14 partial until the closing report. `cargo test --workspace` 369 passed; the full AC-V8 T11 sweep passed (40 s).
  Differential against `27a5e5b`: a 15,480-line transcript of random operations with inline values (set, add,
  remove, three range removals, scans both ways and by prefix, count, rename, delete, empty, abort, error kinds and
  store info), on disk and in temporary mode, byte-identical. Probes kept as regression tests in `3a528ec`
  (`crates/krummi/tests/tree_value_model.rs`; the altered-byte probe `#[ignore]`, about 300 s); its probe of
  rejected operations hashing every file of the folder was not kept: on Windows it read the locked `krummi.data` as
  empty, it failed after the R1 fix because a background transfer removed a released `krummi.temp` chunk (temporary
  area, J7, not a write of the rejected operations), and AC-V10 is already `tree_values.rs::ac_v10_…`.
- **L16** (Codex R1 fix, ODD Worker `opus`/`high`, `7b4286b`): own chunks for runs over a normal chunk or without a
  contiguous free range, released with the run's last holder; transfer passes free retired versions. Resolutions in
  `docs/reviews/trees-large-values/2026-10-27-code-resolutions.md`. RDD review 5 (`62f7571..3a528ec`, escalated item
  4, lenses risk, resilience, reliability; lineage `review-695b76ad34d5b00d`): **approved**, acknowledged, authority
  burned (a first attempt never started: an untracked file needed a selection). Advisory findings: R3-002 fixed in
  `c0b8d1a` (a comment without code); accepted as follow-up work for the closing report: R4-own-chunk-churn
  (WARNING: with fragmented shared chunks, every small run that finds no contiguous range gets its own file while a
  snapshot holds it; a resource and latency cost, not a correctness one, and growing shared chunks instead reopens
  R1), R4-release-syscalls-under-lock (SUGGESTION: unmap and delete outside the area lock), R3-001 (WARNING: the
  regression test's exact file-bytes equality could fail if Windows keeps a released file open, e.g. an antivirus
  scan) and R3-003 (SUGGESTION: the probes hard-code the threshold sizes).
- **L17** (Codex follow-up, round 2): `docs/reviews/trees-large-values/2026-10-27-code-02.md`, wrapper status
  `findings`, `threadId` `01a17f6b-d5fe-7253-84c4-532b728e477f`. R1 "Still open; original scenario verified fixed,
  overall finding unresolved": a run reused for a slightly smaller one left the whole old allocation alive and
  uncounted under an old read. Fixed in `e1cc4ab` (a run is charged whole while any version references it; RED at
  the second run: 17,162,240 bytes held, 8,536,064 counted). RDD review 6 (`3a528ec..e1cc4ab`, lineage
  `review-713a431a84a3734d`): **approved**, acknowledged; advisory R3-retired-runs-uncounted (WARNING) recorded in
  the resolutions and not changed (retired versions are in `krummi.data`; retired single pages were never counted).
- **L18** (Codex follow-up, round 3): wrapper status `error` (usage limit until 2026-10-28 02:35), `threadId`
  `01a17f7f-413f-75b1-b21d-21c23a84c773`, no report. Not an approval: the round-2 fix of R1 is unverified by Codex
  and the loop's last round is still owed. Closing report: [trees-large-values-report.md](trees-large-values-report.md).
- **L19** (2026-10-28, Codex follow-up, round 3, after the user restored the CLI login): two attempts without a
  report (the ChatGPT login did not offer `gpt-6.1-sol`; content flagged as a possible cybersecurity risk), then
  `docs/reviews/trees-large-values/2026-10-28-code.md` with wrapper status `violation` caused by the user deleting
  `codex-models.json` (user, 2026-10-28: "Lo borré yo. Adelante y nueva revisión codex."). R1 still open: retired
  runs kept by a newer read were uncounted. Fixed in `098e621` (RED: 51,126,272 bytes held, 0 counted). RDD review 7
  (`373208f..098e621`, lineage `review-aba90cc61b80a4bb`): **approved**, acknowledged; R3-001 dismissed with
  evidence, R3-002 accepted. The user authorized a fourth Codex round.
- **L20** (2026-10-28, Codex follow-up, round 4, authorized by the user beyond the three-round limit):
  `docs/reviews/trees-large-values/2026-10-28-code-02.md`, wrapper status `findings`. R1 verified resolved. New R2
  (major), introduced by `098e621`: a write's own flush kept runs only its snapshot retained, so a write failed with
  the maximum size without any read (against J7). Fixed in `7584790` (RED: `MaxSize { max_size: 25165824, needed:
  25608192 }`). RDD review 8 (`45c1bf2..7584790`, lineage `review-48af2c09d934d928`): **approved**, acknowledged;
  its two WARNING on the borrow argument and its SUGGESTION dismissed with evidence. Verifying R2 needs a fifth
  Codex round, which only the user can authorize.
