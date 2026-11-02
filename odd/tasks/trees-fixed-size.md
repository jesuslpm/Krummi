# Fixed-size trees (5.5) — ODD execution document

Objective: implement [trees-fixed-size.prd.md](../../docs/prd/krummi/trees-fixed-size.prd.md) (decisions X1–X15,
criteria AC-X1–AC-X21), with its ancestry (trees.prd.md, krummi.prd.md) and siblings 5.1–5.4 and the remembered node
check. Branch: `feat/trees-fixed-size` (PRD commit `a97017e`, branch point `2125833`). Delivery: work-unit commits on
the branch, unmerged. Test runner: `cargo test --workspace` (Windows; Linux in WSL 2 Ubuntu). Estimate: 6 h – 9 h.
Real start: 2026-11-02 09:10:20 +0000.

## Specs

Each S# quotes one acceptance criterion of the PRD verbatim (Spanish, authoritative). The decisions X1–X15 of the
PRD are part of every spec; *Fuera de alcance* limits them and *Dejado a la implementación* is the only room for
the design choices below.

- **S1** — **AC-X1** — Dado un árbol de tamaño fijo, cuando se guardan en orden aleatorio las claves `0`, `1`, `2^63−1`, `2^63`, `2^64−2` y `2^64−1` y otras al azar, entonces un recorrido completo las devuelve en orden numérico hacia delante y en el inverso hacia atrás, en la misma escritura, en una lectura posterior y, en disco, tras reabrir (X5, AC-B7).
- **S2** — **AC-X2** — Dada una escritura, cuando crea árboles de tamaño fijo de longitudes `0`, `1`, `8` y `2048`, entonces se crean, y en cada uno se guardan y se leen idénticos valores de su longitud; en el de longitud `0`, Leer distingue una clave guardada, con su valor vacío, de una que no está; y cuando abre o crea uno de longitud `2049`, en cada modo de A1 y también en Abrir en una transacción de lectura, con y sin un árbol de ese nombre, entonces falla sin efecto con la clase de tamaño no válido, como valor, con el tamaño recibido `2049`, mínimo `0` y máximo `2048`, y no crea nada (X1, X2).
- **S3** — **AC-X3** — Dado un árbol de tamaño fijo de longitud 8, cuando se guarda o se añade un valor de 0, 7 o 9 bytes, también con una clave que ya está, entonces falla sin efecto con la clase de tamaño no válido, como valor, con el tamaño recibido, mínimo `8` y máximo `8`, y la transacción sigue utilizable; y con un árbol de longitud 0 y un valor de 1 byte, igual, con mínimo `0` y máximo `0` (X1, X6, AC-B16).
- **S4** — **AC-X4** — Dado un árbol de tamaño fijo de longitud 8, uno normal y uno multivalor, cuando se abre o se crea, en cada modo de A1, el de tamaño fijo como tamaño fijo de 16 bytes, como normal y como multivalor, y los otros dos como tamaño fijo de 8 bytes, entonces falla con la clase de tipo de árbol distinto, con el nombre, el tipo que tiene y el tipo pedido, con sus longitudes, y el árbol queda intacto; falla igual con una página propia del árbol dañada y el catálogo intacto; borrarlo sin pedir tipo nunca falla con esa clase; Listar informa cada árbol con su tipo y, en el de tamaño fijo, su longitud; renombrarlo y vaciarlo conservan su longitud; Renombrar con sobrescritura borra un árbol de tamaño fijo de destino; y tras borrar el de tamaño fijo, crear otro con su nombre y otra longitud lo crea (X3, X4, AC-B8).
- **S5** — **AC-X5** — Dado un árbol de tamaño fijo vacío, cuando se guardan, se añaden, se leen y se quitan entradas, con las claves `0` y `2^64−1` entre ellas, entonces Guardar crea o sobrescribe, Añadir devuelve que añadió la primera vez y que no añadió la segunda sin cambiar nada, Leer devuelve el valor guardado o que no está, Quitar devuelve que estaba la primera vez y que no estaba la segunda, y el número de entradas coincide en cada paso con las claves guardadas y no quitadas (X6, A4, A5).
- **S6** — **AC-X6** — Dado un árbol de tamaño fijo con entradas, cuando se recorre con límites incluidos, excluidos y abiertos, con `0` y `2^64−1` como límites y con límites que no son claves del árbol, alternando los dos extremos, entonces cada recorrido devuelve exactamente lo que dicen R1 y R2 en orden numérico; con cada caso de intervalo vacío de X7 (el límite inferior mayor que el superior, los dos iguales y alguno excluido, `5` y `6` excluidos, desde `2^64−1` excluido y hasta `0` excluido), el recorrido no devuelve nada y Quitar intervalo quita `0`, también con la página raíz del árbol dañada (en disco); y cuando se quitan intervalos, también todo el árbol, entonces Quitar intervalo devuelve cuántas quitó, el número de entradas baja en esa cantidad y un recorrido devuelve las demás (X7, R1–R3, R7).
- **S7** — **AC-X7** — Dado el código de una aplicación de prueba, cuando usa con la apertura de un árbol de tamaño fijo una operación de los árboles normales o multivalor, o una operación de este PRD con la apertura de un árbol normal o multivalor, o Guardar, Añadir, Quitar o Quitar intervalo en una transacción de lectura, o usa una referencia o un recorrido después de una de las operaciones de X8 que falló sin efecto, o pasa a otro hilo la apertura de un árbol de tamaño fijo de una escritura o un recorrido suyo, entonces no compila; pasar a otro hilo la de una lectura y sus recorridos compila; y Leer, el número de entradas y los recorridos no terminan las referencias ni los recorridos anteriores (X4, X8, B1, R9, AC-B11).
- **S8** — **AC-X8** — Dado un almacén con un árbol de tamaño fijo ya creado, que se mantiene dentro de una sola región del espacio libre, tomando las páginas en uso como en AC-A15: cuando se crea un árbol de tamaño fijo vacío, se confirma, se toma el valor, se le guardan entradas suficientes para varios niveles y se confirma, y después se le quitan todas con cada una de estas formas en pruebas separadas (Quitar, Quitar intervalo, una combinación de los dos y vaciarlo), entonces el valor final es el tomado con el árbol vacío; y cuando el árbol se borra o se sobrescribe con Renombrar, el valor final es el de antes de crearlo cuando su entrada no hizo crecer el catálogo, como en AC-A15 (X9, A2, A3).
- **S9** — **AC-X9** — Dada una escritura con un árbol de tamaño fijo abierto dos veces, y otra transacción del mismo almacén, cuando lo borra a través de una apertura y usa la otra con cada operación de este PRD, también tras crear otro árbol con el mismo nombre, entonces falla sin efecto con árbol no encontrado y el último nombre del árbol; cuando usa la apertura en la otra transacción, entonces falla sin efecto con uso incorrecto; y con además un valor de otra longitud, en los dos casos, el error es de tamaño no válido (X4, X11, A6, A7, AC-B14).
- **S10** — **AC-X10** — Dado un árbol de tamaño fijo en disco de al menos dos niveles, con una página dañada en la parte que lleva a unas claves, cuando se leen, se guardan, se añaden o se quitan esas claves, también una que ya está o que no está, o se recorre o se quita un intervalo que las contiene, entonces cada operación falla con la clase de daño y el número de la página, y vuelve a fallar igual al repetirla (salvo el riesgo residual de C7, que la prueba no provoca); el recorrido termina tras el fallo y sus entradas anteriores son correctas; las claves cuyo camino desde la raíz no pasa por esa página se leen sin error; los modos Abrir, Crear nuevo y Abrir o crear, Existe, Tipo, Listar, Renombrar sin sobrescritura y el número de entradas nunca fallan con daño en las páginas del propio árbol, y dan su resultado o su error de siempre (por ejemplo, Crear nuevo sobre ese árbol falla con árbol ya existente); y cuando una escritura vacía, borra o sobrescribe con Renombrar ese árbol, entonces puede fallar con la clase de daño y la escritura sigue T11 (X10, A9, R8, G6, F9).
- **S11** — **AC-X11** — Dada una transacción de lectura abierta y una escritura que, con inserciones y borrados, divide y fusiona páginas de un árbol de tamaño fijo hasta cambiar su altura, cuando la lectura lee todas las entradas y el número de entradas antes y después de que la escritura se confirme, entonces ve exactamente las de su instantánea; y una lectura nueva ve las de la escritura (G3, T14, AC-B17).
- **S12** — **AC-X12** — Dada una escritura que divide y fusiona páginas de un árbol de tamaño fijo, cuando se provoca una caída del proceso y simulada de corriente en cada punto en que su confirmación escribe o lleva datos al disco, entonces al reabrir el árbol tiene todas sus entradas de antes o todas las de después, con su número de entradas; y cuando se simula un fallo de T11 distinto de entrada y salida en cada punto en que una operación reserva, lee o libera páginas, entonces confirmar falla con la clase del primer fallo y no aplica nada, y tras abortar la escritura el árbol y su número de entradas están como antes y una escritura nueva funciona sin error (G1, G5, T11, AC-B18).
- **S13** — **AC-X13** — Dado un almacén pequeño con un árbol de tamaño fijo de al menos dos niveles, cuando se altera cada uno de sus bytes por separado, se abre y se leen todas sus entradas, entonces cada alteración se detecta como daño o no tiene efecto, con las reglas de AC10 de la raíz y sus anotaciones; nunca se devuelve una clave o un valor con un byte distinto (X10, G6, AC10).
- **S14** — **AC-X14** — Dados los valores que devuelven Leer y los recorridos de un árbol de tamaño fijo de longitud 8, cuando se leen, entonces las referencias están dentro de la memoria del motor que fija T16 y no en una copia, en disco y en modo temporal, y usar una referencia después de terminar la transacción no compila (X6, X7, AC20).
- **S15** — **AC-X15** — Dada una escritura en disco que solo hace Abrir o crear sobre un árbol de tamaño fijo existente, Añadir de claves que ya están, Quitar de claves que no están, Quitar intervalo que no quita nada, Leer, el número de entradas, recorridos, y operaciones que fallan por tamaño no válido o por tipo de árbol distinto, cuando se confirma, entonces termina sin error y los archivos del almacén quedan idénticos byte a byte (X8, T10).
- **S16** — **AC-X16** — Dado un almacén de formato `8` que la ejecución crea con la versión anterior a este cambio, compilada aparte, y conserva como dato de prueba, y los de formato `6` y `7` ya conservados, cuando se abren, entonces fallan con la clase de formato distinto como formato anterior y no se modifican; un almacén nuevo informa la versión de formato `9`, con el mismo tamaño y las mismas páginas libres que un almacén nuevo de formato `8` mientras no tiene árboles; y leyendo en disco los bytes de una hoja de un árbol de tamaño fijo, con el conocimiento de la implementación, sus claves, tal como la hoja las guarda, y la longitud del árbol están en little endian (X12, A12, AC-B12).
- **S17** — **AC-X17** — Dada una operación que abre un árbol de tamaño fijo con un nombre de 1.025 bytes y una longitud de 2.049, cuando se ejecuta, entonces el error de tamaño no válido es del nombre; y dada una apertura con un nombre válido y una longitud de 2.049, cuando se ejecuta sobre un árbol existente de tamaño fijo de otra longitud, sobre uno normal, sobre uno que no existe en el modo Abrir, sobre uno existente en el modo Crear nuevo y, en disco, con la página del catálogo dañada, entonces falla en todos los casos con tamaño no válido, sin efecto; en un almacén que no acepta escrituras (T12), falla con esa clase y no con la de entrada y salida; y una escritura con cambios antes y después de fallos de X1 y X2 se confirma sin error y aplica todos esos cambios (X2, X11, AC-B4, AC-B9, AC-B13).
- **S18** — **AC-X18** — Dada una escritura en disco con un árbol de tamaño fijo, cuando hace solo uno de estos cambios en pruebas separadas (Guardar con el mismo valor que ya tenía la clave, vaciar un árbol ya vacío, Añadir de una clave nueva, Quitar de una clave que está, o Quitar intervalo que quita al menos una entrada) y se confirma, entonces los archivos del almacén cambian, como en cualquier escritura con cambios (X8, T10).
- **S19** — **AC-X19** — Dado un árbol de tamaño fijo con sus páginas en el archivo de datos y sitio para recordar todos los nodos que lee la prueba, cuando una transacción de lectura lee la misma clave varias veces y otra la lee después, entonces todas devuelven lo mismo, y la organización de cada nodo del camino se comprueba solo en la primera lectura de ese nodo y su integridad en todas; y dada una página de una hoja suya en el archivo de datos con la organización dañada y el código de integridad recalculado para que sea correcto, cuando se lee dos veces en la misma transacción de lectura y una vez en otra, entonces cada lectura falla con la clase de daño y el número de esa página, también cuando antes se había recordado otra versión correcta de esa página; esa misma página dañada en el área temporal (confirmada y sin trasladar) falla igual las dos veces; un byte alterado en la página de un nodo recordado sin recalcular su código hace fallar la lectura siguiente con daño; y tras cerrar y reabrir el almacén, la primera lectura de cada nodo comprueba su organización otra vez. La prueba usa nodos a los que la implementación aplica lo recordado (X14, C1–C4, C7, AC-C1, AC-C4, AC-C5, AC-C7, AC-C9).
- **S20** — **AC-X20** — Dados un árbol normal, uno multivalor y dos de tamaño fijo de longitudes `0` y `8`, cuando se pide su Tipo por su nombre en una transacción de lectura y en una de escritura, entonces devuelve el tipo de cada uno, con su longitud en los de tamaño fijo, igual que Listar; con un nombre que no tiene ningún árbol, también uno borrado en la misma escritura, devuelve que no existe sin fallar; tras renombrar un árbol y tras borrarlo y recrearlo con otro tipo en la misma escritura, devuelve el tipo vigente con su nombre nuevo; con un nombre vacío o de 1.025 bytes, falla sin efecto con tamaño no válido, como nombre; con una página de uno de esos árboles dañada y el catálogo intacto (en disco), no falla; en disco, con la página del catálogo dañada, falla con la clase de daño, y con un nombre de 1.025 bytes falla con tamaño no válido; en un almacén que no acepta escrituras (T12), funciona igual; no termina las referencias ni los recorridos anteriores, y una escritura en disco que solo pide tipos se confirma dejando los archivos idénticos byte a byte (X15, X8, T10).
- **S21** — **AC-X21** — Dado el informe de cierre, cuando se revisa, entonces contiene la medición de X13 con sus tiempos, por fase, y los tamaños del almacén, para cada tipo de árbol y cada longitud de valor, y los datos del equipo, en Windows y en Linux (X13).

## Design

Decisions within *Dejado a la implementación*:

- **D1 — Node format.** Fixed-size trees get their own dense node format in `krummi_tree::fixed`, not the slotted
  node, so an entry costs only its 8-byte key and its value. Header, 16 bytes: byte 0 kind (`3` fixed leaf, `4`
  fixed branch), byte 1 level (`0` leaf), bytes 2–3 count (u16 LE), bytes 4–5 the value length (u16 LE, leaves; `0`
  in branches), bytes 6–7 zero, bytes 8–15 a branch's leftmost child (zero in a leaf). Then `count` entries packed
  in key order: a leaf entry is the key (u64 LE) and the value (the tree's length); a branch entry is the key (u64
  LE) and the child (u64 LE), the child holding the keys from that key up to the next entry's key, the leftmost
  child the keys before the first. Capacities: leaf `(PAYLOAD_SIZE − 16) / (8 + len)` (3 at 2,048 bytes, 1,020 at
  0), branch 510. Integers little-endian (X12).
- **D2 — Layout check (X14).** `fixed::is_valid`: kind `3` at level `0` or kind `4` at levels `1..=254`, count
  within the capacity of the node's own length (≤ 2,048), so every accessor stays within the page. It shares the
  remembered check of `node_at` (same cache, same page number and integrity code). Because the cache does not know
  the format, every read also checks the kind family after a cache hit (normal nodes `1`/`2`, fixed `3`/`4`), and a
  fixed leaf's header length must equal the tree's length; otherwise damage of that page.
- **D3 — Splits and merges.** A full node splits by count into halves, or, when the new entry falls past its last
  one, keeps its entries and a new sibling starts with the new entry (a branch moves its last key up), as the slotted
  trees do. A node left under a quarter of its page merges with a sibling when both fit; a branch root with one child
  is replaced by it; an empty node is freed and leaves its parent.
- **D4 — Intervals.** A fixed interval is two `Bound<u64>` normalised to inclusive `[low, high]`: an excluded lower
  bound `2^64−1` or an excluded upper `0` has no integer, and the interval is empty when `low > high`. This is exactly
  X7's list (inverted, equal with one excluded, contiguous excluded, the two extremes) and reads no page.
- **D5 — Public API.** `TreeKind::FixedSize(u16)` (catalog kind byte `3`, entry of 27 bytes: the 25 of the other
  kinds plus the length, u16 LE); Display "fixed-size of N bytes". Openings `open_fixed_tree(name, value_len)` and
  `open_fixed_tree_with(name, value_len, mode)` returning `ReadFixedTree` / `WriteFixedTree`, which carry the
  length. Operations on the transactions: `get_fixed`, `set_fixed`, `try_add_fixed`, `remove_fixed`, `range_fixed`
  (`impl RangeBounds<u64>`, items `(u64, &[u8])`), `remove_range_fixed`; `len` through the sealed handle traits;
  catalog operation Tipo as `tree_kind(name) -> Result<Option<TreeKind>>` on both transactions. The requested
  length is a `usize`, checked after the name and before the catalog (X2, X11); a value's length is checked against
  the handle's length before the handle itself (X1, X11).
- **D6 — Format `9`.** `FORMAT_VERSION` 9; a format-8 store is kept as fixture `tests/data/store-5.4`, created with
  the binary of `2125833` built apart.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T0 | — | inline | done: exploration, design D1–D6 | — |
| T1 | S1, S5, S6 (engine part) | delegated (context backstop) | done | `d51b613` |
| T2 | S1–S9, S14–S17, S20 | delegated (context backstop) | done | `e279b64`, `21f7cbd` |
| T3 | S10–S13, S18, S19 | delegated (context backstop) | done | `90b7bb1` |
| T4 | S21 | inline + background runs | done | `edfe769`, `c3e9a23`, `457d135` |
| T5 | all | inline: docs, checks, verification, RDD, Codex, closing | done except Codex (L19) | `f7acb34`–`d5a378e`, report |

T1: engine module `krummi_tree::fixed` (node format, layout check through the shared remembered check, get, set,
try_add, remove, range removal, freeing, double-ended scan) with a seeded model test against a `BTreeMap`. T2: public
API, `TreeKind::FixedSize`, catalog kind `3`, Tipo, format `9`, format-8 fixture, integration and compile tests. T3:
damage, snapshot, crash, T11 sweeps, byte alteration, positive T10, remembered check tests. T4: measurement example
and runs on Windows and WSL.

Route evidence: the units are sequential (T2 needs T1, T3 needs T2), so no parallel writers. They are delegated for
the long-session backstop: the run is about 5,000 forecast lines plus RDD, Codex and the measurement, which the
parent context must still hold. Forecast: T1 about 1,500 lines, T2 about 2,000, T3 about 1,200, T4 about 400.

## Log

- **L1** (2026-11-02, user): "Adelante" (implement PRD 5.5 after it was committed in `a97017e`).
- **L2** (2026-11-02): estimate 6 h – 9 h confirmed by the user ("Sí"); real start 09:10:20 +0000; phase log
  `odd/tasks/trees-fixed-size-phases.tsv`. RDD on (global).
- **L3** (T0): exploration mapped the node format, the write path, the scans, the catalog, the handles and the
  tests; design D1–D6.
- **L4** (T2 preparation, inline): format-8 fixture `crates/krummi/tests/data/store-5.4/krummi.data` (65,536 bytes),
  made by the code of `2125833` (main before this change) compiled apart from a `git archive` copy in a temporary
  directory, with a throwaway example: normal tree `t` with keys `0..20` as 8-byte big-endian and values of 8 bytes
  `[i; 8]`, and multi-value tree `m` with key `key` and values `0..5` (one byte each). It printed `format 8`. Scratch
  removed.
- **L5** (T1, ODD Worker `opus`/`high`): engine module `krummi_tree::fixed` (fixed.rs 980 lines, fixed_tests.rs
  1,004). Shared: `node_at` became `node_at_with(tx, page, level, &Format)` (validator and kind family; the kind is
  checked after a cache hit too), `Op`, `Outcome<K>`, `Update`, `RangeUpdate`, `reserve`. Written apart: change,
  split, merge, drop, fix_root, range removal, freeing and the scan (byte keys, slotted cells and value runs make
  sharing them need a node-format trait). Deviations: `is_valid` also requires bytes 6–7 zero and a branch length
  field zero; underfull counts bytes as in slotted trees (a one-entry leaf of 2,048 bytes is never underfull); scans
  descend into the exact child of `low`/`high`. RED: E0583 (module missing). GREEN: `cargo test -p krummi-tree` 41
  passed (62.5 s); `cargo test --workspace --no-fail-fast` 33 suites, 436 passed, 0 failed; fmt and clippy clean.
  Parent spot check: `cargo test -p krummi-tree` 41 passed (62.9 s). Commit `d51b613`. Risk: item 3 (the shared
  `node_at` read path of every tree gains the kind check). RDD assess: medium, `under_budget` (2,057 lines);
  escalated with item 3 → high; review started, lenses risk and reliability, lineage `review-c1d9c0da0a7812cb`.
- **L6** (RDD review 1): `a97017e..d51b613` (T1), lenses risk and reliability, escalated with item 3: **approved**,
  lineage `review-c1d9c0da0a7812cb`, acknowledged (authority burned). Risk: no findings (evidence: every fixed node
  read goes through `node_at_with` with the kind check after a cache hit; `is_valid` bounds counts; freeing frees a
  node before its children). Reliability, two suggestions, non-blocking: R3-001 (a root split at level 254 would
  build an unreadable level-255 root; not addressed: the slotted `fix_root` has the same limit and a 510-way fanout
  never reaches it) and R3-002 (no test proves a scan stops at a separator without reading a damaged child past its
  bound; folded into T3). Reviewed boundary: `d51b613`.
- **L7** (T2, ODD Worker `opus`/`high`): public API (`TreeKind::FixedSize(u16)`, `ReadFixedTree`/`WriteFixedTree`,
  `open_fixed_tree`, `open_fixed_tree_with`, `get_fixed`, `set_fixed`, `try_add_fixed`, `remove_fixed`,
  `range_fixed`, `remove_range_fixed`, `tree_kind`), catalog kind `3` (27-byte entry, decode by kind, length over
  2,048 rejected as damage), format `9`, kind dispatch in delete/empty/rename-overwrite, README. Tests:
  `tests/fixed_trees.rs` (13 criteria tests), `src/fixed_tests.rs` (AC-X14 and the T12 parts of AC-X17/AC-X20, which
  need engine hooks), 7 failing and 2 passing trybuild cases (no existing `.stderr` changed), AC-X16 with the
  fixtures of formats 6, 7 and 8. Defaults taken: the "same size and free pages as a new format-8 store" is pinned as
  literals measured on `d51b613` (65,536 bytes, 8 pages, 4 free); shared test helpers moved from `multi_trees.rs`
  to `tests/support`. RED: 284 compile errors (E0432/E0599). GREEN: `cargo test --workspace --no-fail-fast` 34
  suites, 451 passed, 0 failed; fmt and clippy clean (one scoped `allow(clippy::reversed_empty_ranges)` for the
  PRD's `8..=5`). Parent spot check: `cargo test -p krummi --test fixed_trees` 13 passed; `e279b64` alone builds
  with its tests (`cargo test --workspace --no-run` on a `git archive` copy). Commits split for review: `e279b64`
  (implementation, literals, support helpers) and `21f7cbd` (new tests, fixture, compile cases). Risk: items 1 and 3
  (persisted catalog entry and format, public `TreeKind`), requested by X3, X12 and X15. RDD assess
  `d51b613..HEAD`: high (`process_boundary`), 3,265 lines; review started with lenses risk, reliability and
  readability.
- **L8** (RDD, T2): the review of `d51b613..21f7cbd` (3,265 lines) stopped at START with `lens_context_budget_exceeded`
  after consent was granted; no authority was created. Next: review `e279b64` and `21f7cbd` as separate candidates
  (each with HEAD at that commit) once the T3 worker no longer edits the working tree.
- **L9** (T4, inline): `crates/krummi/examples/fixed_timing.rs` runs X13 (`--entries`, `--lengths`, `--folder`,
  `--quick`): for lengths 0, 8 and 100, a fixed-size tree against a normal tree with 8-byte big-endian keys (the
  key conversions inside the timed loops), 1,000,000 distinct SplitMix64 keys and random values with fixed seeds,
  durable commits of 1,000; loads in key order and at random with size and pages in use, then in the random store,
  reopened: reads at random, a forward scan (key and every value byte folded into a checksum) and removals at random.
  Smoke run `--quick` on Windows passed (for example, 0-byte values in order: 16 pages in use against 24). fmt and
  clippy clean. Commit `edfe769`. The full runs wait for T3, so its test runs do not skew the times.
- **L10** (T3, ODD Worker `opus`/`high`): tests only, no engine defect. S10
  `ac_x10_a_damaged_leaf_on_the_way_to_some_keys` (also R3-002: scans and range removals whose bounds stop at the
  separators around the damaged leaf do not read it; a temporary mutation disabling the separator checks made it
  fail); S11 `ac_x11_a_read_keeps_its_snapshot_while_a_write_splits_and_merges`; S12 crash sweep
  `ac_x12_crashes_during_a_commit_with_splits_and_merges_of_a_fixed_size_tree` (+ ignored through the close) and T11
  sweep `ac_x12_a_failure_at_a_sample_of_page_operations_of_a_fixed_size_tree_applies_nothing` (+ ignored every
  point, 777 points, floor 760); S13 `ac_x13_sample_of_altered_bytes_with_a_fixed_size_tree` (+ ignored every byte,
  3,389.85 s in release on Windows); S18 `ac_x18_a_write_with_one_change_changes_the_files`; S19 five `ac_x19_*` in
  `check_tests.rs`. First-run failures were test mistakes (a read after a failed range removal, whose leaves T11
  leaves unspecified; a bound inside the damaged leaf's part; arithmetic). Defaults taken: AC-X10 damages a leaf;
  AC-X11 changes height 1 → 2 with length 1,000 (one-entry leaves of 2,048 never merge). Worker checks: fmt and
  clippy clean; `cargo test --workspace --no-fail-fast` 34 suites, 462 passed, 0 failed, 20 ignored; the three
  ignored sweeps pass in release. Parent: removed the duplicate `scan_fails_at` left in `multi_trees.rs` (moved to
  `support`) and its unused `Result` import; `multi_trees` 15 and `fixed_trees` 16 passed; fmt and workspace clippy
  clean. Commit `90b7bb1`. Risk: none (tests only).
- **L11** (2026-11-02, process): a parent script error truncated this document (untracked) while appending L10; it
  was rebuilt from its generator and the entries appended during the run, with the same content.
- **L12** (T4 runs): X13 measured on Windows (11:31–11:36 UTC) and on Linux in WSL 2 (11:37–11:47 UTC), one after the
  other; results and machine in `odd/tasks/trees-fixed-size-measurements.md` (commit `c3e9a23`). Fixed-size trees use
  43 %/40 % fewer pages with 0-byte values, 27–31 % with 8 and 5 % with 100; Windows loads in key order ×6.8, ×3.2
  and ×1.6 faster; on Linux the durable commit dominates (×1.8, ×1.5, none).
- **L13** (RDD reviews 2 and 3, T2, each with HEAD detached at its commit): review of `d51b613..e279b64`
  (implementation, 861 lines), lenses risk, reliability and readability: **approved**, lineage
  `review-11144303a4987ddc`, acknowledged. Advisory findings: R2-dangling-acx16-reference, R3-fixed-public-coverage,
  R2-unused-fixed-helpers and R3-catalog-fixed-decode-untested (warnings that the tests are missing: they are in the
  next commit `21f7cbd`, including `catalog_entries_of_fixed_size_trees_are_checked_by_kind` and AC-X16);
  R3-fixed-module-outside-candidate (suggestion: the engine was reviewed in review 1); R2-tree-kind-doc-wording and
  R2-mixed-language-op-names (suggestions, fixed in `541325e`). Review of `e279b64..21f7cbd` (tests, 2,404 lines),
  lenses reliability and readability: **approved**, lineage `review-b629f1b1fc81bb2d`, acknowledged; three
  suggestions on the trybuild `.stderr` snapshots pinning private fields and rustc help texts (R2-001,
  R3-brittle-send-diagnostics, R3-toolchain-help-text): not addressed, the existing compile suite uses the same
  snapshots by design. Reviewed boundary: `21f7cbd`.
- **L14** (RDD review 4, last slice): `21f7cbd..541325e` (T3 tests, the measurement example and documents, 1,614
  lines; assess high, `process_boundary`), lenses reliability and readability, started. Independent verifier (ODD
  Verify `sonnet`/`high`) launched for the whole feature, because T1 and T2 named high-risk items.
- **L15** (RDD review 4): `21f7cbd..541325e`, lenses reliability and readability: **approved**, lineage
  `review-eabfe89bce05b27f`, acknowledged. Advisory findings: R2-001 (warning: the example's usage line lacked `--`)
  and R3-001 (suggestion: the measurement's checksums were not compared with the stored entries), both fixed in
  `457d135` (the checksums are compared outside the timed loops; `--quick` passes; the full runs of L12 used the
  same timed code without that check); R2-002 (suggestion: test doc comments name operations by their PRD names,
  Leer, Guardar, Tipo): not addressed, the existing tests do the same (for example `tree_integrity.rs:32`, from
  2026-10-25); R2-003 (suggestion: T4 status and log stale at review time): already updated in L12–L14 and the task
  table. Reviewed boundary: `541325e`; `457d135` (the example fix, 7 lines) is left for the closing slice.
- **L16** (independent verifier, ODD Verify `sonnet`/`high`, `2125833..541325e`): S1–S20 pass, S21 pending the
  closing report; no blockers. Focused suites pass on a `git archive` copy (`fixed_trees` 16, lib `fixed` 8 + 1
  ignored, `tree_integrity ac_x` 2 + 2 ignored, `krummi-tree fixed` 11, `trees_compile` 1 with every new case).
  Probes: the PRD *Salida* example exactly; X2 precedence in every mode (also `usize::MAX` and 65,544, no
  truncation); X1 before Misuse and TreeNotFound; X7 against a `BTreeMap` with 1,521 bound pairs from both ends and
  every empty case (0 mismatches); a random model at lengths 0–2,048 with reopen; a 1,500-tree catalog switching
  entries between 25 and 27 bytes; normal and multi-value trees at `2125833` and HEAD give identical output and a
  byte-identical data file after the 16 KB header (only the format version differs); rejected and no-op writes leave
  the files identical. Advisories: the ignored sweeps were not run by it (the T3 worker ran them); the level-254
  root split (R3-001); an emptied catalog keeps its root page (shared catalog code, unchanged). Scratch removed by
  the parent.
- **L17** (checks at `457d135`, 12:02–12:06 UTC): `cargo fmt --all --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean; `cargo test --workspace --no-fail-fast` 34 suites, 462 passed, 0 failed, 20
  ignored. Codex code review round 1 over `a97017e..HEAD` started.
- **L18** (CI, `9198c8b`): the CI's tree sweep step did not run the new every-point T11 sweep (its filter lists
  `ac_b18 ac_r12 ac_v8 ac_m14`) and would have run the every-byte AC-X13 sweep (3,389.85 s in release on Windows)
  in debug within the step's 120 minutes. Default taken (run rule: no stop to ask): `ac_x12` added to the lib filter;
  `tree_integrity -- --ignored --skip ac_x13` in that step; a new step runs `ac_x13` alone in release with 120
  minutes. The filters were checked with `--list`; the YAML parses. Risk: item 5 (CI). The CI run itself is not
  observable before a push.
- **L19** (Codex code review round 1): two attempts over `a97017e..f7acb34` (12:06 UTC) and `a97017e..9198c8b`
  (12:35 UTC) ended with wrapper status `error`, "You’ve hit your usage limit", no report written (threads
  `01a19c27-b8b0-7c23-a359-6b2dec7e6602` and `01a19c41-8bfa-7d42-b0e5-19d7fdb65682`; logs
  `%TEMP%\codex-review\20261102-120650-trees-fixed-size-code` and `…\20261102-123502-trees-fixed-size-code`). Codex
  can run again on 2026-11-04 06:48. Not an approval: it stays pending for the user.
- **L20** (RDD review 5): `541325e..c88ff58` (the example fix, the CI change and the log, 88 lines; assess high,
  `shell_source`), lenses resilience and reliability: **approved**, lineage `review-8b8eba8c9f33c8d1`, acknowledged.
  Suggestions: R4-acx13-release-loses-debug-asserts (the release sweep lost `debug_assert!` and overflow checks):
  fixed in the next commit with `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS` and `CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS`
  set to `true` for that step (Cargo's profile environment variables; not run locally, to keep the release build of
  the running sweeps untouched); R3-002 (the checksum assertions depend on the fold order): a comment says so.
  R4-acx13-timeout-headroom and R3-001 (the 120-minute budget of the new step is not measured on CI runners): left
  pending, observable only after a push.
- **L21** (CI's ignored sweeps in release at `9198c8b`'s filters, 12:13–13:03 UTC): `cargo test --release -p krummi
  --lib -- --ignored ac_b18 ac_r12 ac_v8 ac_m14 ac_x12` 5 passed (1.88 s); `cargo test --release -p krummi --test
  tree_integrity -- --ignored --skip ac_x13` 8 passed (2,989.40 s). The AC-X13 every-byte sweep passed in T3
  (3,389.85 s, L10).
- **L22** (RDD review 6): `c88ff58..d5a378e` (9 lines), lens resilience: **approved**, lineage
  `review-135e2bafabd287fe`, acknowledged. R4-ac-x13-timeout-headroom (suggestion: the 56-minute figure was measured
  without the debug checks): left pending with the CI timing of L20. Reviewed boundary: `d5a378e`; the commits after
  it are documents only.
- **L23** (2026-11-02 15:13 UTC, user decision on L19): "Fusionamos sin ella". The Codex code review is waived for
  this run; the branch is fast-forwarded into `main`, not pushed.

## Closing

Closing report: [trees-fixed-size-report.md](trees-fixed-size-report.md). Measurement:
[trees-fixed-size-measurements.md](trees-fixed-size-measurements.md). Codex code review not run (L19); the user chose to merge without it (L23).
