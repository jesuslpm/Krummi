# Multi-value trees (5.4) — ODD execution document

Objective: implement [trees-multivalue.prd.md](../../docs/prd/krummi/trees-multivalue.prd.md) (decisions M1–M15,
criteria AC-M1–AC-M20), with its ancestry (trees.prd.md, krummi.prd.md) and siblings 5.1–5.3.
Branch: `feat/trees-multivalue` (PRD commit `9a50bd3`). Delivery: work-unit commits on the branch, unmerged.
Test runner: `cargo test --workspace` (Windows; Linux in WSL 2 Ubuntu). Estimate: 5 h – 8 h. Real start:
2026-10-29 10:20:06 +0000.

## Specs

Each S# quotes one acceptance criterion of the PRD verbatim (Spanish, authoritative). The decisions M1–M15 of the
PRD are part of every spec; *Fuera de alcance* limits them and *Dejado a la implementación* is the only room for
the design choices below.

- **S1** — **AC-M1** — Dado un árbol multivalor vacío, cuando se añaden a claves de 1 y de 2.048 bytes, en orden aleatorio, valores vacíos, de 1 byte y de 2.048 bytes, con bytes de `0x00` a `0xFF` y valores que son prefijo de otros, entonces cada Añadir devuelve que añadió la primera vez y que no añadió la segunda, sin cambiar nada; Contiene los encuentra y no encuentra los que no se añadieron; el número de valores de cada clave coincide con los añadidos, y es `0` para una clave que no está; el número de entradas es el número de claves; y el recorrido de valores de cada clave los devuelve ordenados byte a byte sin signo, con el vacío primero y cada valor que es prefijo de otro antes que él, en la misma escritura, en una lectura posterior y, en disco, tras reabrir (M1, M5, M6, AC-B7).
- **S2** — **AC-M2** — Dado un árbol multivalor, cuando se añade, se quita, se comprueba, o se usa como límite de un recorrido de valores o de Quitar intervalo de valores, un valor de 2.049 bytes, entonces falla sin efecto con la clase de tamaño no válido, como valor, con el tamaño recibido `2049`, mínimo `0` y máximo `2048`, y la transacción sigue utilizable; cuando cada operación de este PRD que recibe una clave la recibe vacía o de 2.049 bytes, también el número de valores y Quitar clave, entonces falla igual como clave, con mínimo `1` y máximo `2048`; con una clave y un valor de 2.049 bytes, el error es de la clave; con un límite inferior y un prefijo fuera de tamaño, del límite inferior; y con un prefijo de 2.049 bytes, del prefijo, con mínimo `0` y máximo `2048` (M1, M6, M7, M12, AC-B15).
- **S3** — **AC-M3** — Dada una clave con valores, cuando se le quitan uno a uno con Quitar valor, entonces cada uno devuelve que estaba la primera vez y que no estaba la segunda; al quitar el último, la clave desaparece: Contiene, el número de valores (`0`), el recorrido de claves y el número de entradas lo reflejan; y cuando se le vuelve a añadir un valor, la clave reaparece con un solo valor (M5, M6).
- **S4** — **AC-M4** — Dadas una clave con un valor, otra con varios valores en su entrada y otra con valores en un subárbol, cuando se quita cada una con Quitar clave, entonces devuelve su número de valores (`1` para la primera) y la clave desaparece; y con una clave que no está, devuelve `0` sin fallar (M6, M2).
- **S5** — **AC-M5** — Dada una clave, con los dos umbrales que la prueba toma de la implementación, cuando se le añaden valores, uno a uno, hasta pasar el umbral de subida, y después se le quitan hasta quedar por debajo del de bajada, con el valor vacío entre ellos y sin él, en escrituras separadas y en la misma escritura, entonces tras cada paso Contiene, el número de valores y el recorrido de valores, hacia delante y hacia atrás, coinciden con los valores añadidos y no quitados, en la misma escritura, en una lectura posterior y, en disco, tras reabrir; los valores pasan al subárbol justo al superar el umbral de subida, siguen en él mientras están entre los dos umbrales y vuelven a la entrada justo al quedar por debajo del de bajada; con Quitar intervalo de valores que salta de una vez de por encima del de subida a por debajo del de bajada, vuelven a la entrada en esa operación; y el valor vacío conserva su lugar al pasar de una representación a la otra (M2, M8, M1).
- **S6** — **AC-M6** — Dado un árbol multivalor con claves con valores en su entrada y en un subárbol, cuando se recorren sus claves y los valores de cada clave con límites incluidos, excluidos y abiertos, con el valor vacío como límite incluido y excluido, con prefijos (también vacío y acabado en bytes `0xFF`), y alternando los dos extremos, entonces cada recorrido devuelve exactamente lo que dicen R1 a R4 aplicados a las claves o a los valores, cada clave con su número de valores; el recorrido de valores de una clave que no está, y el de claves de un árbol vacío o que se quedó sin claves, no devuelven nada; con límites invertidos, el intervalo está vacío, también con la página de la entrada de la clave dañada (en disco); y cuando una escritura modifica el árbol a través de otra apertura, seguir usando un recorrido anterior no compila (M7, R1–R4, R6).
- **S7** — **AC-M7** — Dado un árbol multivalor, cuando se quitan intervalos de valores de una clave en su entrada y en un subárbol, también todos los de la clave, entonces cada Quitar intervalo de valores devuelve cuántos quitó, y la clave que se queda sin valores desaparece; y cuando se quitan intervalos de claves, también con claves con subárbol, entonces devuelve cuántas claves quitó, el número de entradas baja en esa cantidad y un recorrido de claves devuelve las demás con su número de valores (M8, M6).
- **S8** — **AC-M8** — Dado un almacén con un árbol multivalor ya creado, que se mantiene dentro de una sola región del espacio libre, tomando las páginas en uso como en AC-A15: cuando se crea un árbol multivalor vacío, se confirma, se toma el valor, se le añaden claves con valores en su entrada y en subárboles y se confirma, y después se le quitan todas las claves con cada una de estas formas en pruebas separadas (Quitar valor, Quitar clave, Quitar intervalo de valores, Quitar intervalo de claves y vaciarlo), entonces el valor final es el tomado con el árbol vacío; cuando a una clave con subárbol se le quitan valores hasta volver a su entrada y se confirma, entonces las páginas en uso son las mismas que con un árbol al que se le añadieron esos mismos valores sin que la clave llegara nunca al subárbol (las pruebas lo preparan sabiendo la implementación); y cuando el árbol se borra o se sobrescribe con Renombrar, el valor final es el de antes de crearlo cuando su entrada no hizo crecer el catálogo, como en AC-A15 (M10, A2, A3).
- **S9** — **AC-M9** — Dado el código de una aplicación de prueba, cuando usa Guardar, Añadir, Leer o Quitar de 5.1, un recorrido o Quitar intervalo de 5.2 con la apertura de un árbol multivalor, o una operación de este PRD con la de un árbol normal, o Añadir, Quitar valor, Quitar clave o Quitar intervalo en una transacción de lectura, o usa una referencia o un recorrido después de una de las operaciones de M9 que falló sin efecto, o pasa a otro hilo la apertura de un árbol multivalor de una escritura o un recorrido suyo, entonces no compila; y pasar a otro hilo la de una lectura y sus recorridos compila; y Contiene, el número de valores y los recorridos no terminan las referencias ni los recorridos anteriores (M4, M9, B1, R9, AC-B11).
- **S10** — **AC-M10** — Dado un árbol normal y otro multivalor, cuando se abren o se crean con su nombre como el otro tipo, en cada modo de A1, entonces falla con la clase de tipo de árbol distinto, con el nombre, el tipo que tiene y el tipo pedido, y el árbol queda intacto; falla igual con una página propia del árbol dañada y el catálogo intacto; y borrarlo sin pedir tipo nunca falla con esa clase; dada una escritura con cambios hechos antes y después de una operación que falla por tipo de árbol distinto, cuando se confirma, entonces termina sin error y aplica todos esos cambios; dada una transacción de lectura, cuando una apertura falla por tipo de árbol distinto, entonces sigue leyendo; dado un almacén que no acepta escrituras (T12), cuando una apertura falla por tipo de árbol distinto, entonces falla con esa clase y no con la de entrada y salida, y confirmar esa escritura sin cambios termina sin error; dado un árbol normal que se borra, cuando se crea otro con su nombre como multivalor, entonces se crea; Listar informa cada árbol con su tipo; y Renombrar con sobrescritura borra el árbol de destino sea del tipo que sea, también un multivalor con subárboles (M4, M12, AC-B4, AC-B6, AC-B8, A3).
- **S11** — **AC-M11** — Dada una escritura con un árbol multivalor abierto dos veces, y otra transacción del mismo almacén, cuando lo borra a través de una apertura y usa la otra con cada operación de este PRD, también tras crear otro árbol con el mismo nombre, entonces falla sin efecto con árbol no encontrado y el último nombre del árbol; cuando usa la apertura en la otra transacción, entonces falla sin efecto con uso incorrecto; y con además un argumento fuera de su tamaño, en los dos casos, el error es de tamaño no válido (M4, M12, A6, A7, AC-B14).
- **S12** — **AC-M12** — Dado un árbol multivalor en disco, en pruebas separadas, con la página raíz dañada del subárbol de una clave (un subárbol de al menos dos niveles) y con una página dañada en la parte del árbol que lleva a otra clave, cuando se añade, se quita o se comprueba un valor, también uno que ya está o que no está, o se recorren los valores de esas claves, entonces cada operación que necesita la página falla con la clase de daño y su número, y vuelve a fallar igual al repetirla; el recorrido de valores termina tras el fallo, sus valores anteriores son correctos y en una lectura un recorrido nuevo vuelve a fallar igual; las demás claves se leen y se modifican sin error; el número de valores de la clave con el subárbol dañado y el recorrido de claves no fallan; y cuando una escritura quita la clave con el subárbol dañado con Quitar clave, le quita valores hasta que vuelve a su entrada, quita un intervalo de claves que la contiene, vacía el árbol, lo borra o lo sobrescribe con Renombrar, entonces falla con la clase de daño y el número de una página del subárbol, y la escritura sigue T11 (M11, A9, R8, G6, F9).
- **S13** — **AC-M13** — Dada una transacción de lectura abierta y una escritura que, con Añadir y Quitar valor, hace pasar claves de su entrada a un subárbol y de vuelta, y divide y fusiona páginas del árbol y de los subárboles hasta cambiar su altura, cuando la lectura lee todas las claves, sus valores, su número de valores y el número de entradas antes y después de que la escritura se confirme, entonces ve exactamente los de su instantánea; y una lectura nueva ve los de la escritura (G3, T14, AC-B17).
- **S14** — **AC-M14** — Dada una escritura que añade y quita valores pasando claves de una representación a otra y que divide y fusiona páginas, cuando se provoca una caída del proceso y simulada de corriente en cada punto en que su confirmación escribe o lleva datos al disco, entonces al reabrir el árbol está entero como antes de la escritura o entero como después: cada clave con sus valores y su número de valores, y el número de entradas; y cuando se simula un fallo de T11 distinto de entrada y salida en cada punto en que una operación reserva, lee o libera páginas, también al pasar los valores de una clave a un subárbol y de vuelta, entonces confirmar falla con la clase del primer fallo y no aplica nada, y tras abortar la escritura el árbol, sus claves, sus valores y el número de entradas están como antes y una escritura nueva funciona sin error (G1, G5, T11, AC-B18).
- **S15** — **AC-M15** — Dado un almacén pequeño con un árbol multivalor con claves con valores en su entrada y en un subárbol, cuando se altera cada uno de sus bytes por separado, se abre y se leen todos sus valores, entonces cada alteración se detecta como daño o no tiene efecto, con las reglas de AC10 de la raíz y sus anotaciones; nunca se devuelve un valor con un byte distinto (M11, G6, AC10).
- **S16** — **AC-M16** — Dadas las claves y los valores que devuelven el recorrido de claves y el recorrido de valores, de claves con valores en su entrada y en un subárbol, cuando se leen, entonces las referencias están dentro de la memoria del motor que fija T16 y no en una copia, en disco y en modo temporal, y usar una referencia después de terminar la transacción no compila (M7, AC20).
- **S17** — **AC-M17** — Dada una escritura en disco que solo hace Añadir de valores que ya están, Quitar valor de valores que no están, Quitar clave de claves que no están, Quitar intervalo de claves y de valores que no quitan nada, Contiene, el número de valores, recorridos, y operaciones que fallan por tamaño no válido o por tipo de árbol distinto, cuando se confirma, entonces termina sin error y los archivos del almacén quedan idénticos byte a byte (M9, T10).
- **S18** — **AC-M18** — Dado un almacén de formato `7` que la ejecución crea con la versión de 5.3, compilada aparte, y conserva como dato de prueba, y el de formato `6` de 5.1, cuando se abren, entonces fallan con la clase de formato distinto como formato anterior y no se modifican; un almacén nuevo informa la versión de formato `8`; y leyendo en disco los bytes de la entrada de una clave con subárbol, con el conocimiento de la implementación, su número de valores y la página de su subárbol están en little endian (M14, AC-B12).
- **S19** — **AC-M19** — Dado el informe de cierre, cuando se revisa, entonces contiene la medición de M15 con sus tiempos, por fase, y los tamaños del almacén para cada carga y cada pareja de umbrales candidata, y los datos del equipo, en Windows y en Linux (M15).
- **S20** — **AC-M20** — Dado el informe de cierre, cuando se revisa, entonces contiene la evaluación de M3 con al menos las seis parejas de umbrales candidatas que nombra, los datos de la medición de cada una, la complejidad que añade cada una, la elegida y por qué, justificada con esos datos (M3).

## Design

Decisions within *Dejado a la implementación*:

- **D1 — Parent tree and entry blob.** A multi-value tree is a normal B+ tree (the parent) whose leaf value for a
  key is a blob. Inline blob: tag `0`, value count (u16 LE), then each value as length (u16 LE) and bytes, sorted
  byte-wise, the empty value as a zero-length element. Subtree blob: tag `1`, flags (bit 0: has the empty value),
  value count (u64 LE), inline-encoded bytes of all values (u64 LE), subtree root (u64 LE). The subtree is a normal
  tree whose keys are the non-empty values, each with an empty value; it never holds an empty key (user
  preference recorded in the PRD).
- **D2 — Threshold unit.** The volume of a key is its key length plus the length of its inline blob. Promotion
  when an Add makes the volume exceed the promotion threshold; demotion when a removal leaves it below the
  demotion threshold. The promotion threshold is at most 4,074 bytes (`MAX_LEAF_CELL` − cell header), so an inline
  entry always fits its leaf and never goes off-leaf (M1). Candidates for M3 are set by T5's measurement; the
  defaults are fixed there.
- **D3 — Counts.** The tree's entry count is its number of keys (catalog count). The per-key value count is read
  from the blob, never from the subtree (M11).
- **D4 — Value intervals.** Value bounds and prefixes are evaluated with value semantics (the empty value is the
  least). In a subtree the interval is translated to non-empty keys and the empty value is handled from the flag.
  The empty-interval check for values does not assume a least key of `[0]`.
- **D5 — Public API.** Each type has its own handles and openings: `open_tree(name)` / `open_tree_with(name,
  mode)` for normal trees (the kind argument goes away) and `open_multi_tree` / `open_multi_tree_with` for
  multi-value trees, returning `ReadMultiTree` / `WriteMultiTree`. `TreeKind` gains `MultiValue` (catalog kind
  byte `2`) for Listar and the type error. Operation names follow Rust conventions (padre, *Dejado*).
- **D6 — Freeing.** Empty, delete, rename-overwrite, remove key and remove key range free every subtree of the
  affected keys before freeing the parent pages.
- **D7 — Thresholds for measurement.** The thresholds live in the engine with a hidden setter used only by the
  measurement example, so the tests always use the defaults.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T0 | — | inline | done: exploration (explorer `opus`/`medium`), design D1–D7 | — |
| T1 | S1, S3–S7 (engine part) | delegated (context backstop) | done | `8b5a7c8` |
| T2 | S1–S10, S16–S18 | delegated (context backstop) | done | `cd7b815`, `182e461` |
| T3 | S11–S15 | delegated (context backstop) | done | `2f7b39c` |
| T4 | S19–S20 | inline + background runs | done | `5a6df2b`, `c44fe75`, T4 close below |
| T5 | all | inline: docs, checks, verification, RDD, Codex, closing | done | `d605cc3`–`4f1e083`, report |

T1: engine module in `krummi-tree` (blob encoding, both representations, transitions with hysteresis, value and
key scans, removals, freeing) with a model test against an in-memory map. T2: public API, catalog kind, format `8`,
fixture of format `7`, and the integration and compile tests. T3: damage, snapshot, crash, fault and byte-alteration
tests for multi-value trees. T4: measurement example, runs on Windows and WSL, evaluation and default thresholds.

Route evidence: the units are sequential (T2 needs T1, T3 needs T2), so no parallel writers. They are delegated
for the long-session backstop: the parent context already holds the PRD session and the exploration.

## Log

- **L1** (2026-10-29, user): "Haz el commit del los archivos que has tocado y empieza la implementación con ODD"
- **L2** (2026-10-29): estimate 5 h – 8 h confirmed by the user ("Sí"); real start 10:20:06 +0000; phase log
  `odd/tasks/trees-multivalue-phases.tsv`. RDD on (global).
- **L3** (T0): exploration mapped the tree engine, catalog, public API, tests and measurements; design D1–D7.
- **L4** (T1, ODD Worker `opus`/`high`): engine module `krummi_tree::multi` (multi.rs, multi_tests.rs, range.rs
  `is_empty_from` / `contains`). RED: E0432/E0425 with the tests and an empty module. GREEN: 25 krummi-tree tests,
  seeded model test of 4,320 ops with reopen, exact-threshold tests (mutants `<=`/`>=` killed). Checks fmt, clippy,
  workspace tests: pass. Parent spot check: `cargo test -p krummi-tree` 25 passed. Deviations: `inline_len` is the
  whole inline blob length (header included); a key with only the empty value may stay in a subtree with an empty
  root; value-range removal pre-scans the removed bytes. Provisional thresholds 2048/1024. Commit `8b5a7c8`.
  Risk: item 3 (shared `Interval` helpers), low. RDD assess on `9a50bd3..8b5a7c8`: medium, `under_budget`.
- **L5** (T2, ODD Worker `opus`/`high`): public API (D5: `open_tree(name)`, `open_multi_tree*`, `ReadMultiTree` /
  `WriteMultiTree`, contains, value_count, keys/values scans, add, remove_value, remove_key, key/value range
  removals; `len` over sealed handle traits), catalog kind byte 2, `TreeKind::MultiValue`, format 8, kind-aware
  freeing, format-7 fixture `tests/data/store-5.3` built from `cd50b90` via `git archive` (scratch removed).
  RED: 405 compile errors on `tests/multi_trees.rs`; GREEN: 12/12; mutation (always normal `free_all`) caught by
  S8. Parent fixed the last hard-coded format 7 in `krummi-pager/tests/pages.rs:171`; parent spot check
  `cargo test --workspace --no-fail-fast`: 33 suites ok. Commit split for review: `cd7b815` (implementation and
  mechanical call sites, verified alone: clippy clean, 32 suites ok) and `182e461` (new tests and fixture); final
  tree identical to the original single commit. Risk: items 1 and 3 (persisted format, public API), requested by
  M14 and D5.
- **L6** (RDD): review 1, `9a50bd3..8b5a7c8` (T1), lenses risk/readability/reliability: approved, lineage
  `review-65f9aede008e0181`; advisory: R1-001 global threshold setter (suggestion), R2-free-all-name-collision
  (warning), R2-hardcoded-max-in-message, R2-subtree-blob-offsets (warning), R3-global-thresholds (warning),
  R3-untested-blob-damage (warning). The slice `9a50bd3..4835dd0` and then `8b5a7c8..cd7b815` stopped with
  `lens_context_budget_exceeded` (no authority created). Default taken (run rule: no stop to ask): `cd7b815` stays
  without RDD; it is about 800 lines of mechanical call-site edits across 42 large test files plus the API, and
  splitting further would need non-compiling intermediate commits. It is covered by the Codex code review of the
  whole range and the independent verifier at the end. Review 2, `cd7b815..182e461` (T2 tests), lenses
  reliability/readability: approved, lineage `review-443b1e1d58a73985`; advisory: R3-value-scan-send-unproved
  (warning: the compile-fail case does not prove the write value scan is not Send), R2-tautological-threshold-assert,
  R2-hardcoded-entry-layout-constants, R2-duplicated-size-locals, R3-read-prefix-removals-unchecked,
  R3-normal-handle-write-prefix-ops-unchecked (suggestions). Folded into T3.
- **L7** (T4 example, ODD Worker `opus`/`high`): `crates/krummi/examples/multi_timing.rs` (M15 loads, 6 pairs
  512:448, 512:256, 2048:1792, 2048:1024, 4074:3564, 4074:2037; `--pairs`, `--loads`, `--values`, `--quick`).
  Default taken: the oscillating load counts 100,000 operations (50,000 Add/Remove pairs). Quick smoke run
  (10,000 values, Windows) only: Contains about 12–15 µs per value on subtree keys against 2.6 µs on one-value keys.
  Commit `7d8c973`, recommitted as `5a6df2b` without a T3 deletion it had picked up.
- **L8** (2026-10-29, user): "Pues si cuesta tanto 12–15 µs por valor frente a unos 2,6 µs con claves de un solo
  valor, no sé que estamos haciendo mal ni si merece la pena usar árboles multivalor." Decision: the measurement
  adds a baseline (the same pairs in a normal tree with composite keys) and a count of pages read per Contains for
  each representation; the full run waits for T3 so builds do not skew the times.
- **L9** (2026-10-29, user): "Yo no creo que sea necesario comprobar el blob, se supone que está ordenado, ya lo
  verifican los tests. Igual que un árbol, no se verifica que está ordenado, se supone." Verified: `node::is_valid`
  checks structure only, not key order. The blob decode drops its order and duplicate check and keeps the
  structural checks (G6); sent to the T3 worker. That check only affected inline keys, not the subtree case of L8.
- **L10** (T3, ODD Worker `opus`/`high`): S11 size cases completed for every operation; S12 damaged subtree root
  and damaged leaf on the way to a key (fail with the page, other keys keep working, commit fails with T11 and
  applies nothing); S13 snapshot across promotions, demotions and height changes, on disk and temporary; S14 crash
  sweep (`crash_sweep_with`) and T11 sweep (`t11_sweep_with`, about 920 points, floor 900; ignored every-point
  variants); S15 byte-alteration sample plus ignored every-byte sweep (`each_altered_byte`, `check_items`). Every new
  test passed on its first run: no defect found. RDD follow-ups applied (offset constants, `{MAX_KEY}` message,
  `multi::free_tree`, structural blob-damage tests, Send cases split in three, test cleanups); L9 applied. Default
  taken: CI tree-sweep timeout raised from 60 to 120 min (every-byte multi sweep about 44 min on Windows debug).
  Ignored runs: crashes through the close 13.9 s, every page operation 53.7 s, every byte 2,637 s. Parent spot check:
  fmt, clippy clean; `cargo test --workspace` 408 passed, 0 failed, 17 ignored. Risk: item 5 (CI) and item 3, low
  (shared test helpers refactored, existing tests pass).
- **L11** (RDD): review 3, `182e461..2f7b39c` (T4 example `5a6df2b` and T3), assessed high (process and CI
  signals), lenses resilience/readability/reliability: approved, lineage `review-296029a3f7646019`, acknowledged
  (authority burned); the reviewed boundary is now `2f7b39c`. Advisory findings: R4-blob-order-check-dropped
  (warning; the user's decision L9, kept), R2-duplicated-multi-seen (warning; the lib tests and the integration
  tests are separate crates with no shared test module, kept), R3-off-leaf-case-confounded (warning; to fix in T5:
  the off-leaf case also has an over-long element), R2-duplicated-scan-checker, R2-test-blob-layout-literals
  (suggestions, to look at in T5), R2-placeholder-commit-ref (fixed here).
- **L12** (T4, measurement): full M15 run, Windows then Linux, 2026-10-29 14:04–18:55. Linux consistent: Add
  about the same in both representations (multi 40–48 µs, composite 38–44 µs per value), Contains 3.1–5.6 µs
  against 3.7–4.0 µs, multi 28–52% fewer pages in use except n1 (+11%); the 12–15 µs of L7 was not reproduced.
  Windows discarded: the composite rows took 85–108 µs per random add against 27 µs measured by 5.2 on this machine,
  and the last loads came back to about 28–30 µs, so something outside the engine slowed it down; rerun at once.
  Evaluation in `trees-multivalue-measurements.md`: 2048:1024 stays.
- **L13** (T5): R3-off-leaf-case-confounded rebuilt with a consistent 5,000-byte blob; the key scan then returned the
  off-leaf entry's count instead of damage (RED: `inconsistent_blobs_are_damage_of_their_leaf`). Fixed in
  `decode_scanned` (a scanned blob that does not fit in a leaf is damage, M1, which also covers the key range
  removal); R2-test-blob-layout-literals fixed (the tests use `INLINE`/`SUBTREE`). Commit `d605cc3`; README
  `003a703`. Checks: fmt, clippy clean; `cargo test --workspace` 408 passed, 0 failed. Risk: item 6 not applicable
  (pinned by the test); item 1, low (reads of persisted pages: a stricter damage check, nothing written changes).
- **L14** (T4 close): Windows rerun 19:02–20:04, consistent (composite 26–34 µs per random add, as 5.2's 27 µs). Both
  systems agree: 512 wastes space (n100 ×2.9 pages), 4,074 saves 5% of pages in zipf only (and 4–6% of Add time there on Windows) and
  costs up to 70% more per operation at the threshold; the two demotion thresholds do not separate. 2048:1024 stays
  as the default (S20); the measurement with both systems and the machine data is in
  `trees-multivalue-measurements.md` (S19).
- **L15** (RDD): review 4, last slice `2f7b39c..fe52cc9` (composite baseline `c44fe75`, PRD draft `ab0d944`, fix
  `d605cc3`, README `003a703`, thresholds `fe52cc9`), assessed medium and `under_budget`, reviewed as the last slice;
  lenses resilience/readability/reliability: approved, lineage `review-e827cc5980447c65`, acknowledged (authority
  burned); reviewed boundary now `fe52cc9`. Advisory findings: R2-zipf-add-claim-contradicts-linux-table (warning)
  and R3-zipf-add-gain-overstated (suggestion): fixed, the 4–6% Add gain of 4,074 in zipf is Windows only;
  R2-tree-open-inside-timed-values-loop (warning): kept and documented in the measurement (the values-scan column
  includes one tree opening per key in both representations; a rerun would take 2.5 h and not change the comparison).
- **L16** (T5, ODD Verify `sonnet`/`high`, on `fe52cc9` in a scratch clone): no blockers; S1–S18 met with test
  evidence; S19–S20 partially met until the closing report carries the measurement. Probes: an empty-value-only
  subtree of a 2,048-byte key, a random model of the public API against a `BTreeMap` (3 seeds), and a normal-tree
  workload compared with the baseline `97887ef`, identical except the format-version header bytes. Checks in the clone:
  fmt, clippy clean; `cargo test --workspace` 408 passed, 0 failed, 17 ignored. Advisories: S14/S15 every-point and
  every-byte sweeps taken from L10, not rerun; `set_multi_thresholds` is process-global (only the example calls it);
  the `open_tree(name)` API change (D5); `cd7b815` had no RDD review (L6), covered by this verification and Codex.
  Scratch removed by the parent.
- **L17** (T5, Codex code review R1 of `97887ef..c455c3e`): `findings`, report
  `docs/reviews/trees-multivalue/2026-10-29-code.md`. R1 (blocker): the public bounds `ReadHandle`/`WriteHandle`
  exposed `as_tree()` of their sealed supertraits, so a generic function reached the normal handle of a multi-value
  tree and normal `set` persisted a blob later read as damage; fixed in `4ecf488` (the method takes a crate-only
  token; compile-fail regression, RED observed). R2 (minor): `--values` not divisible by a set size panicked in the
  example; fixed in `dc317ec` (whole sets only). Resolutions in `2026-10-29-code-resolutions.md`. Checks: 408 passed,
  0 failed.
- **L18** (RDD): review 5, `fe52cc9..` the Codex fixes (`4ecf488`, `dc317ec`, the report and its resolutions),
  assessed medium and `under_budget`, raised to high with item 2 (the type-level guard that keeps normal operations
  away from multi-value entries); lenses risk/reliability: approved, lineage `review-b2683ba3440bc7f9`,
  acknowledged (authority burned). Advisory: R3-set-size-truncation-no-automated-regression (suggestion): kept; the
  example has no test target that `cargo test --workspace` runs, and the reproduction command is in the resolutions.
- **L19** (T5, Codex follow-up of `c455c3e..d5dc52a`): `approved`, report
  `docs/reviews/trees-multivalue/2026-10-29-code-02.md`: R1 and R2 verified resolved, no regressions. The loop ends
  after two rounds.
- **L20** (T5 close): closing report [trees-multivalue-report.md](trees-multivalue-report.md). T5 done. The branch
  stays unmerged and unpushed.
- **L21** (2026-10-29/30, user, after the closing report): "Con los números delante me parece que los árboles
  multivalor no merecen la pena, los árboles normales van muy bien y no compensa la complejidad introducida." Then,
  on key sizes: "¿Pero cuanto de mejora se tendría con 32, 64, 128 y 256 bytes en las claves?", and on values: "En un
  índice secundario "normal" pondríamos la clave principal de la tabla en el valor, pero para uno "de cobertura" el
  valor podría ser bastante más largo. ¿Cómo influiría esto?", with "Sí, pero no incluyas estas mediciones ni en la
  CI y las pruebas normales." Done: `--key-size` (`1a61e0a`) and `--value-size` (`eee6166`) in the measurement
  example, run by hand only; results in `trees-multivalue-measurements.md`, *Key and value sizes*. With 8–16-byte
  values and keys of 64 bytes or more, 67–93% fewer pages; with 256-byte values 9–20% fewer (70% more in n10); with
  1,024-byte values no gain. Contains in a subtree is 17–69% slower even with as many pages read: a code-path cost to
  look at. Two runs slowed down from outside (32-byte keys, rerun; the n1 multi-value row with 64-byte keys).
- **L22** (2026-10-30, user): "Los números que diste antes sugieren que el umbral de subida podría estar demasiado
  bajo. Quizá con umbrales de 2048 y 4096 fuera mejor." 4,096 exceeds `MAX_PROMOTION` (4,074, M1); measured 2048:1024
  against 4074:2037 at larger sizes (*Thresholds at larger sizes* in the measurement): 4,074 is never worse and takes
  59% fewer pages where 2,048 moved keys to small subtrees. Investigation of Contains in subtrees (user: "Sí,
  investiga."): in a scratch clone with `node::is_valid` short-circuited, Contains in a subtree went from 6.4–7.0 to
  2.8–3.1 µs and in the composite tree from 3.7–4.1 to 3.3–3.4 µs (key 64, value 8): whole-node validation on every
  page read costs about 3.5 µs on the dense leaves of subtrees; every read also checks the page's xxh3. Engine-wide
  follow-up, outside this branch.
- **L23** (2026-10-30, user): "Sí", to changing the default thresholds to 4074:2037 (`DEFAULT_PROMOTION` =
  `MAX_PROMOTION`, `DEFAULT_DEMOTION` = half). The thresholds are not persisted, so existing stores keep working: a
  subtree below the new demotion threshold moves back to its entry at its next removal. Twelve tests had fixtures sized
  for 2,048:1,024 (150 values of 16 bytes to promote, exact counts at the thresholds); their counts were scaled to the
  new thresholds. The random model test saw 37 promotions and 2 demotions: its keys hold 5–30 values and crossed the
  thresholds through 2,048-byte values, one of which no longer promotes alone and already blocks demotion. It now
  draws values of 993–1,000 bytes too, runs 180 operations per batch and adds 40% of the time while shrinking (121
  promotions, 14 demotions, fixed seed). User question: "Por qué no podían ser los umbrales 4096:2048?": an inline
  entry above 4,074 bytes does not fit in the longest leaf cell (half a page) and would go off its leaf (M1).
- **L24** (2026-10-30): RDD review of `ab18419` (base `dbfd9e1`, committed only; assessed medium, `under_budget`,
  reviewed as the last slice), lens reliability: approved, lineage `review-32be4f2fe4c84d3b`, acknowledged. Advisory
  R3-compat-unproved (WARNING): no test opens a store written with 2048:1024 under the new defaults. Kept as an
  advisory: format 8, the first with multi-value trees, comes with this unmerged branch, so no such store exists
  outside local tests; the engine compares a subtree's volume with the demotion threshold only when removing
  (`multi.rs`, `remove_value`), and only the test helper `check` asserts a subtree is at or above it. A test would
  need `set_multi_thresholds`, a process-wide value that would disturb the tests running in parallel.
- **L25** (2026-10-30): Codex code review of `d5dc52a..c6097f8`
  ([report](../../docs/reviews/trees-multivalue/2026-10-30-code.md),
  [resolutions](../../docs/reviews/trees-multivalue/2026-10-30-code-resolutions.md)): status `findings`, R1 (minor,
  the oscillating load panics when one value already passes the threshold) → fixed. A first attempt failed with the
  model at capacity.
