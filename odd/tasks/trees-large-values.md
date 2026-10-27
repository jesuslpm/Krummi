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
- **D7 — Leaf threshold (V4).** A test-hooks knob selects the inline limit. Candidates: half page (a cell up to
  4,078 bytes stays inline; the 2-way split holds) and full page (a cell up to 8,158 bytes; needs a split that may
  produce three nodes). T3 ships the half-page threshold; T5 prototypes the full page, measures both and keeps one.
- **D8 — Public API.** `MAX_VALUE` = 268,435,456; signatures and lifetimes unchanged.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T0 | — | inline | done | — |
| T1 | S12 (format), pager part of S3, S8, S11 | delegated (context backstop) | pending | — |
| T2 | transaction part of S3, S6, S8 | delegated (context backstop) | pending | — |
| T3 | S1, S2, S3, S4, S5, S7, S9, S10 | delegated (context backstop) | pending | — |
| T4 | S6, S8, S11 (tree level) | delegated (context backstop) | pending | — |
| T5 | S13, S14 | delegated (context backstop) | pending | — |
| T6 | docs, checks, verification, RDD review, Codex review, closing report | inline | pending | — |

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
