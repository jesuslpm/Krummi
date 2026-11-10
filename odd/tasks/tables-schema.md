# Tables with schema (7.1) — ODD execution document

Objective: implement `docs/prd/krummi/tables-schema.prd.md` (decisions E1–E21, criteria AC1–AC19), the authoritative
specification; its ancestry is `tables.prd.md` (module 7, N1–N11), `krummi.prd.md` (root) and the inherited trees,
transactions, journal and benchmark PRDs. Technical starting state: the table design documents (*Design documents*).
Branch: `feat/tables-schema` (from `docs/tables`). Delivery: `ask-on-risk` (single branch, left unmerged for the user).
Test runner: `cargo test --workspace` (Windows; Linux in WSL 2 Ubuntu). Estimate: 16 h – 26 h. Real start:
2026-11-10 21:32:39 +0000.

## Specs

Each spec quotes an acceptance criterion verbatim; the decisions it names (E#, N#, A#, B#, T#, G#, K#) are part of
it and are read in the PRDs, never paraphrased here. The PRD's *Validaciones y mensajes*, *Salida*, *Fuera de alcance*
and *Dejado a la implementación* bound every task; nothing outside *Fuera de alcance* gets a task. AC-N1 to AC-N6,
AC-N9 and AC-N10 of the parent are met with what the 7.1 criteria fix (PRD preamble of its criteria).

- **S1** — **AC1** — Dados los modos de A1 sobre tablas, cuando se abre, se crea nuevo, se abre o crea, se crea o vacía y se vacía una tabla que existe y una que no, entonces cada modo hace lo de E1, y en una lectura solo existe Abrir; cuando se abre o crea con un esquema compatible al que le faltan columnas que la tabla tiene, entonces la abre y sus filas se leen con todas sus columnas; cuando se abre o crea, o se crea o vacía, con un esquema que difiere en el tipo, la nulabilidad o el valor por defecto de una columna, nombra una columna que no existe o cambia la clave primaria, entonces falla sin efecto con esquema distinto y sus datos; y cuando se abre o crea con cualquier esquema un nombre que es de un árbol o de una secuencia, entonces falla con tipo distinto.
- **S2** — **AC2** — Dadas una tabla con filas y otra tabla, cuando se renombra una a la otra sin sobrescritura, entonces falla con árbol ya existente; con sobrescritura, la de destino desaparece con sus filas y la renombrada conserva sus filas, su identidad y sus aperturas; Describir devuelve el esquema de E2 en una lectura y en una escritura, la escritura con sus propios cambios, con la misma identidad de una columna antes y después de renombrarla y con otro estado del esquema después de cada cambio que da versión nueva; Describir un nombre que no existe falla con árbol no encontrado, y el de un árbol o una secuencia, con tipo distinto; Existe dice que existen los nombres de las tres clases; y Listar muestra las tablas, los árboles y las secuencias con su clase, en el orden de B5.
- **S3** — **AC3** — Dada una tabla con filas y otra apertura de ella, cuando se vacía y se confirma, entonces no tiene filas, conserva su esquema y su identidad, la otra apertura la ve vacía, y una secuencia que usaba no cambia.
- **S4** — **AC4** — Dado cada esquema no válido de E5, cuando se crea la tabla con él o se pide el cambio que lo produciría, entonces falla sin efecto con esquema no válido, con la tabla, la columna y la regla, y la transacción puede confirmar sus otros cambios; cuando una tabla creada en la versión 1 recibe 65.534 cambios y se pide otro, entonces falla sin efecto con esquema no válido; un cambio que no cambia nada no da versión nueva; otra apertura de la tabla en la misma escritura ve el esquema nuevo; y cada cambio de esquema escribe las mismas páginas con 10 que con 100.000 filas, según su gancho de pruebas (AC-N2, AC-N6).
- **S5** — **AC5** — Dada una tabla con valores por defecto, cuando se inserta una fila sin esas columnas, entonces se leen sus valores por defecto, y nulo en las que no tienen; con un nulo explícito, nulo; y sin valor ni nulo en una columna que no los admite, falla con valor no válido; cuando se actualiza una fila dando solo algunas columnas, entonces las demás conservan su valor, también una columna añadida después de que la aplicación definió su esquema; cuando se guarda una fila con una columna que la tabla no tiene, entonces falla con esquema distinto; y cuando le falta una columna de la clave o repite una columna, con valor no válido.
- **S6** — **AC6** — Dados valores en los extremos de cada dominio de E8 y uno más allá de cada extremo, cuando se guardan, entonces los de dentro se leen iguales y los de fuera fallan con valor no válido; dados un `decimal` con más decimales que su escala en cada caso de empate y de signo, y un instante con fracción de 100 ns, cuando se guardan, entonces se leen redondeados como dice E8; 0001-01-01T00:00:00.0000005Z se lee 0001-01-01T00:00:00.000001Z; y 99,995 en un `decimal(4, 2)` y 9999-12-31T23:59:59.9999995Z fallan con valor no válido.
- **S7** — **AC7** — Dados vectores `bit` con un solo bit puesto en cada posición de un byte, un `f16` conocido, un vector con `-0` y `string` con U+0000 y vacío, cuando se guardan y se leen, entonces los bits están donde dice E7, el `-0` se lee `+0` y los textos se leen iguales; y el texto vacío y los bytes vacíos sirven como clave primaria.
- **S8** — **AC8** — Dados `ulid` y `uuid` de cada generador, cuando se convierten a texto y se vuelven a leer, entonces dan el mismo valor en la forma de E9; y cada forma no admitida de E9 falla con valor no válido.
- **S9** — **AC9** — Dada una tabla con una clave primaria de dos columnas, cuando se recorren intervalos con cada prefijo y cada combinación de límites incluidos, excluidos y ausentes, hacia delante y hacia atrás, entonces vuelven las filas de E12 (AC-N1); un intervalo invertido no devuelve ninguna; cada intervalo mal formado de E12 falla con valor no válido, y un límite que no cabe en la clave, con tamaño no válido aunque el intervalo esté vacío; cuando se borra un intervalo, entonces devuelve cuántas filas borró y quedan las de fuera (AC-N3); leer una fila que no existe devuelve que no está, y borrarla devuelve falso; y pedir el número de filas lee las mismas páginas con 10 que con 100.000 filas, según su gancho de pruebas (AC-N3).
- **S10** — **AC10** — Dada una tabla con filas, cuando una carga masiva tiene, antes de confirmar, una fila con clave duplicada, una no válida, una que falla siendo la última, una fuente que falla, una cancelación o un pánico, entonces la tabla, el número de filas y las secuencias quedan como antes y otra escritura obtiene después su turno (7.2 añade el caso de una tabla con índices); cuando se cargan 100.000 filas fuera de orden, entonces la tabla tiene las mismas filas que si se hubieran insertado una a una e informa de 100.000; una carga vacía no cambia nada; cuando un hilo con el turno de escritura empieza una carga, falla con uso incorrecto; y una carga que no obtiene su turno antes de su tiempo límite falla con tiempo de espera agotado (AC-N3).
- **S11** — **AC11** — Dada una secuencia, cuando una escritura pide valores y se confirma, otra pide valores y aborta, se vacía una tabla, se borran filas y se simula una caída del proceso y se reabre, entonces los valores siguientes continúan desde el último confirmado y ninguno repite uno confirmado; una secuencia creada con primer valor 1.000 da 1.000, y Abrir o crear con otro primer valor no la cambia; una en el máximo falla sin efecto con valor no válido; pedir un valor en una lectura no es posible; las operaciones de secuencia sobre una tabla o un árbol, y al revés, fallan con tipo distinto; y renombrar sin sobrescritura una tabla al nombre de una secuencia falla con árbol ya existente (AC-N4, AC-N5).
- **S12** — **AC12** — Dados 100.000 valores de cada generador de E15 desde un solo hilo, con el reloj de prueba que se detiene, retrocede y avanza, y con la parte aleatoria de prueba cerca de agotarse dentro de un milisegundo, cuando se ordenan con el orden de su columna (el de SQL Server, comprobado con `SqlGuid`, para el `uuid` secuencial), entonces quedan en el orden en que se generaron; cuando varios hilos generan a la vez, entonces todos los valores son distintos, los de cada hilo crecen, y un valor generado después de que otro hilo terminó los suyos es mayor que todos ellos; dado un fallo simulado del generador del sistema, la función falla con entrada y salida; y dados vectores de prueba guardados en el repositorio, cada uno con un milisegundo y unos bits aleatorios fijos y los bytes esperados, cuando cada generador los produce con el reloj y los bits de prueba, entonces da exactamente esos bytes: los del `uuid` secuencial, generados una vez con la disposición del generador .NET del usuario y ordenados con `SqlGuid`, también en el orden de los vectores; los del `uuid` versión 7, con sus bits de versión y de variante intactos tras un acarreo que los cruza; y los de cada regla de retroceso y de desbordamiento; y con el reloj de prueba en el último milisegundo de 48 bits y el incremento agotado, la función falla con valor no válido (AC-N4).
- **S13** — **AC13** — Dadas las operaciones de E16, cuando se ejecutan en una escritura que no hace nada más, entonces son cambios para T10 exactamente las que E16 dice; una referencia anterior a cualquier operación que puede modificar deja de valer; y listar el catálogo no muestra ningún árbol interno de una tabla, que ningún nombre alcanza como árbol según su gancho de pruebas (AC-N5).
- **S14** — **AC14** — Dados dos niveles consecutivos de la precedencia de E17, y sus dos excepciones, cuando una operación provoca a la vez un fallo de cada uno, entonces gana el que dice E17; cuando una fila o un esquema incumplen dos reglas, en dos columnas o en la misma, entonces se informa la que dicen E5 y E17: por ejemplo, con el esquema `id, a, b`, `a` y `b` sin nulos, la fila `[(b, 2), (b, 3), (id, 1), (a, nulo)]` da columna repetida en `b`, la fila `[(n, nulo), (n, 5)]` con `n` sin nulos da columna repetida en `n`, y una fila sin `id` y con `a` nula da columna de la clave que falta en `id`; y los errores llevan los datos de E17.
- **S15** — **AC15** — Dados una clave primaria de exactamente 2.048 bytes y otra de 2.049, una fila de exactamente 256 MiB y otra de un byte más, en la forma de E18, y nombres de columna de 0, 1, 1.024 y 1.025 bytes, cuando se usan, entonces los que están dentro funcionan y los que no fallan sin efecto con tamaño no válido y sus datos.
- **S16** — **AC16** — Dadas las cargas W23–W29, cuando se ejecuta el banco completo, entonces el informe muestra 29 umbrales, `no aplica` en los motores que no son Voron, la tabla de tamaños las cuatro cargas de E21, y las sumas de comprobación de W26 y W27 coinciden en los dos motores; y la ejecución reducida usa las cantidades de su columna (AC-N9). El informe de cierre incluye las mediciones de una sola carga de AC-N10.
- **S17** — **AC17** — Dados vectores de prueba del orden de SQL Server guardados en el repositorio, generados una vez con `SqlGuid` de .NET como referencia independiente de Krummi, cuando se ordenan las filas de una columna con ese orden, entonces quedan en el orden de los vectores (AC-N1). Prueban la semántica de .NET, no resultados de una instancia de SQL Server.
- **S18** — **AC18** — Dado un almacén con tablas, cuando lo abre un motor con el formato 10, entonces falla con la versión del formato (B12); y la información del almacén no cambia de forma.
- **S19** — **AC19** — Dada una tabla y una secuencia con una página propia dañada, cuando se piden Abrir, Crear nuevo, Abrir o crear, Existe, Describir, Listar, Renombrar sin sobrescritura o a su mismo nombre y el número de filas, entonces ninguna falla con daño; y leer, guardar, recorrer o pedir un valor que necesitan esa página fallan con la clase de daño y su número de página (E19).
- **S20** — **AC-N1** — Dada una tabla con una columna de cada tipo de N4 que se puede ordenar, como clave primaria de una columna y como parte de una de varias, cuando se insertan filas y se recorren por la clave primaria hacia delante y hacia atrás, entonces vuelven en el orden de N5 para cada tipo, incluidos `ulid`, enteros negativos, `-0` y NaN, y `uuid` en los dos órdenes: el de RFC 9562 es el de sus 16 bytes, y el de SQL Server es el que da `SqlGuid` de .NET a los mismos valores, sobre vectores de prueba que fija 7.1 ordenados con `SqlGuid`, y `bytes` vacíos y con el byte cero en cualquier posición (y `string` vacío y con el carácter nulo si 7.1 los admite) delante de otras columnas de la clave; cuando se recorre un intervalo de la clave primaria, sobre una clave entera y sobre un prefijo de una compuesta, con cada combinación de extremos incluidos, excluidos y sin extremo, entonces vuelven exactamente las filas del intervalo según la semántica que fije 7.1; cuando una prueba de propiedades guarda filas al azar en tablas con claves primarias al azar, de una a cuatro columnas de todos los tipos que se pueden ordenar, con sus valores extremos (`-0` y `+0`, NaN con distintos bits, `uuid` en los dos órdenes, los tipos de fecha y hora y `ulid`) y claves en el límite de tamaño, entonces el orden de los recorridos, las filas de cada intervalo y las claves duplicadas son los de un modelo de referencia de N5 que compara los valores por su tipo, sin usar su codificación; y cada fila se lee con los valores guardados, `-0` como `+0` y todo NaN como el único NaN (7.1).
- **S21** — **AC-N2** — Dada una tabla, cuando se inserta una fila con una clave primaria que ya existe, un valor que no cumple las reglas de su columna (uno por cada regla de N4) o un nulo en una columna que no los admite, entonces la operación falla con su clase (N10), la tabla y la transacción quedan como antes, y la transacción puede confirmar sus otros cambios; insertar `+0` con una clave `-0` guardada, o un NaN con otro, da clave duplicada (N5); cuando se guardan valores en los límites de su tipo (vectores de dimensión 1, u 8 para `bit`, y 4.096, de cada tipo de elemento, y un `decimal` de 38 dígitos), entonces se leen iguales; cuando se crea una tabla con cada uno de los esquemas no válidos que enumera 7.1, entonces falla sin efecto con la clase que fije 7.1; y cuando se abre una tabla en cada caso de esquema distinto que fije 7.1, entonces falla con esquema distinto (7.1).
- **S22** — **AC-N3** — Dada una tabla, cuando se actualiza una fila que no existe, entonces la tabla no cambia y la operación informa de que no la encontró; cuando se inserta o actualiza una fila que no existe y después otra vez con otros valores, entonces la primera vez la inserta y la segunda la reemplaza; leer y borrar una fila que no existe dan lo que fije 7.1; el número de filas que informa una apertura es, tras cada operación de la prueba, el de filas que ve su transacción, leyendo las mismas páginas con 10 que con 100.000 filas según el gancho de pruebas de 7.1, también en una lectura concurrente que no ve la escritura; cuando se borra un intervalo de la clave primaria, entonces quedan exactamente las filas de fuera del intervalo; y cuando se cargan en masa 100.000 filas en orden de clave primaria, entonces la tabla tiene las mismas filas que si se hubieran insertado una a una; y las cargas con filas fuera de orden, repetidas o no válidas dan lo que fije 7.1, también cuando la que falla es la última, cuando una clave generada choca con una explícita o con una fila que ya existía y cuando se agota el contador autonumérico: si 7.1 fija que la carga falla entera, entonces la tabla, sus índices y sus contadores quedan como antes de la carga, también cuando la fuente de las filas falla o la aplicación cancela la carga a mitad, y otra escritura obtiene después su turno (7.1).
- **S23** — **AC-N4** — Dados 100.000 valores de cada generador de N6, generados con el reloj de prueba que fije 7.1, tantos en el mismo milisegundo como permita el incremento que fije 7.1 y el resto en milisegundos sucesivos, desde un solo hilo, cuando se ordenan con el orden de su columna, entonces quedan en el orden en que se generaron; y dada una tabla con una columna autonumérica, cuando se confirman inserciones, se borra la fila de mayor valor y se confirma, se vacía la tabla y se confirma, y se simula una caída del proceso y se reabre, entonces las inserciones siguientes en esa tabla nunca repiten un valor de una fila confirmada (7.1).
- **S24** — **AC-N5** — Dados un árbol y una tabla en un almacén, cuando se crea una tabla con el nombre de la tabla, entonces falla con la clase de ya existente que fije 7.1; cuando se crea una tabla con el nombre del árbol, o un árbol con el de la tabla, entonces falla con tipo distinto, que gana a ya existente (B3); cuando se abren, borran, renombran o vacían como árbol la tabla y como tabla el árbol, o se renombra uno al nombre del otro con sobrescritura, entonces cada operación falla sin efecto con tipo distinto; cuando se renombra uno al nombre del otro sin sobrescritura, entonces falla sin efecto con ya existente; Existe dice que los dos nombres existen; y listar el catálogo muestra la tabla con su tipo y ninguno de sus árboles internos, que, según el gancho de pruebas que fije 7.1, ningún nombre alcanza como árbol (7.1).
- **S25** — **AC-N6** — Dada una tabla con filas, cuando se añade una columna que admite nulos, una con valor por defecto, se quita una columna, se renombra otra y se permiten nulos en otra, entonces las filas guardadas antes se leen con el esquema nuevo, con nulo o con el valor por defecto en las columnas añadidas, una columna añadida con el nombre de una quitada se lee nula, y la escritura de cada cambio escribe las mismas páginas con 10 que con 100.000 filas, según el gancho de pruebas que fije 7.1; y cuando se pide cada cambio no admitido de N3, o quitar una columna de la clave primaria, entonces falla sin efecto con la clase que fije 7.1 (7.1).
- **S26** — **AC-N9** — Dadas las cargas de N11, cuando se comparan con las tablas de Voron en una ejecución completa del banco, entonces Krummi llega al umbral de K7 en cada una, en Windows y en Linux; un umbral incumplido sigue K13: no impide cerrar el PRD hijo y se convierte en un cambio aparte (7.1, 7.2).
- **S27** — **AC-N10** — Dado el cierre de cada PRD hijo, cuando se revisa su informe, entonces incluye las mediciones de una sola carga de N11, en Windows y en Linux: las cargas del banco que ya existían, antes y después de su cambio, y sus cargas nuevas después (7.1, 7.2, 7.3).

## Design documents

The technical starting state. Every task names the sections it follows; a deviation is logged in `## Log` with its
reason and annotated, dated, in the design document.

| Document | What applies to 7.1 |
|---|---|
| `docs/design/tables/README.md` | Decisions index; generators and sequences notes (P20, sequences instead of identity columns). |
| `01-row-format-and-types.md` | Column types, row layout "row v1" (header, null bitmap, fixed section, end offsets, variable section), layout order, primary key columns in the row (only `string` and `bytes`), schema evolution, vector payload, compression hook (flags bit 2). |
| `02-row-storage.md` | Per-table trees (`rows`; `meta` as schema history), the two phases of a row write (check, then write), bulk load as its own transaction, operations (insert, update, delete, scans, truncate, drop). Index and statistics trees are 7.2 and 7.3. |
| `03-keys-and-indexes.md` | *Key encoding, per type* (tuple codes, integers, floats, decimal `40`, uuid and ulid `30`, string and bytes escaping), the SQL Server `uniqueidentifier` permutation, composite keys and range bounds (the `‖ FF` table and the 2,048-byte bound rule), size limit. Ascending only (primary key). |
| `05-vectors.md` | Vector payload (elements, `i8` scale, cosine norm) as stored in a row; no distance kernels in 7.1. |
| `07-dictionary-compression.md` | Only the reserved hook in the row header (N9); nothing is compressed. |
| `11-query-contract-reconciliation.md` | §2.2 catalog identities (random 64-bit table and column ids, `schema_state`, redraw on a known value, test source); P19 (schema version exhaustion fails typed before any change); §2.8 and P20 (pager `fill_random` through `rustix` `rand` and `windows-sys` BCrypt; OS bits at each new millisecond). |

Out of 7.1: documents 04, 06, 08, 09, 10 and 12 (later modules, 7.2 and later).

## Execution design

Decisions within *Dejado a la implementación*; workers follow them and log any deviation.

- **D1 — Code location.** Table code lives in the `krummi` crate, module `table` (`crates/krummi/src/table/`):
  `value.rs` (column types, values, validation, normalization, domains, rounding, `ulid` and `uuid` text), `key.rs`
  (document 3 key encoding, decoding, range bounds), `row.rs` (document 1 row v1), `schema.rs` (schemas, E5
  validation, changes, E6 compatibility, catalog encoding), and the operations. Generators are public functions in
  `crates/krummi/src/generate.rs`. The OS random source is a safe pager function (`krummi-pager`, P20), the only new
  `unsafe`. No new crate.
- **D2 — Catalog classes.** `TreeKind` (non-exhaustive) gains `Table` and `Sequence`, so `list_trees`, `tree_kind`
  and `DifferentTreeType` (tipo distinto) carry the class with no new listing API. The catalog adds kind bytes
  `TABLE = 4` and `SEQUENCE = 5`.
- **D3 — Table storage.** The table's catalog entry holds its random identity, the root and row count of its `rows`
  tree, its current schema (columns with ids, names, types, nullability, defaults, primary key, version,
  `schema_state`) and the root of its schema-history tree (one entry per version, key the big-endian `u16` version,
  value that version's column list, to decode old rows). Open, Describe, Exists, List and the row count read only
  the catalog (E19, the count hook of AC9). Internal trees are reachable only through the table entry (AC13).
- **D4 — Sequence storage.** The catalog entry holds its identity and the root of its own one-entry tree with the
  next value (E14), so asking for a value reads its own page (E19, AC19) and Open, Exists and List do not.
- **D5 — Errors.** New `ErrorKind`s `InvalidValue`, `DuplicateKey`, `DifferentSchema` (esquema distinto) and
  `InvalidSchema`, with typed rule enums in the order of E5 and E17; `SizeSubject` gains `ColumnName`, `PrimaryKey`
  and `Row`. Values in error data (a duplicate primary key, an expected and a found schema element) are carried as
  their text form, because errors live in `krummi-pager` and the value types in `krummi`.
- **D6 — Identities.** Table ids, column ids and `schema_state` come from `fill_random` (P12); a value equal to a
  known one (the catalog's table ids, the table's history) is drawn again; a test hook injects the source.
- **D7 — Generators.** One process-wide state per generator behind a lock; test hooks inject the clock and the
  random bits (AC12). The sequential `uuid` vectors are produced once with the user's .NET generator layout and
  ordered with `SqlGuid` by a C# program kept in the repository; the SQL Server order vectors (AC17) the same way.
- **D8 — Format.** `FORMAT_VERSION` becomes 11 (E20); a format-10 fixture store backs AC18.

## Tasks

| ID | Specs | Design | Route | Status | Commit |
|---|---|---|---|---|---|
| T1 | S14, S15, S18 (error classes, sizes, format) | D2, D5, D8 | inline | done (RED/GREEN: format asserts and the `bytes_tests` pin failed at 10, pass at 11; suite 594 passed) | `17d8a82` |
| T2 | S8 (text forms and generators), S12, S23 | README P20; 11 §2.8; D7 | delegated: parallel with T3 and T4 (Writer trigger a: three independent units, disjoint surfaces) | done (RED/GREEN: 3 pager and 10 generator tests failed against stubs, pass) | `56c5a6b` |
| T3 | S6, S7, S17, S20 (encoding half), S21 (value rules) | 01; 03 key encoding; 05 payload; 07 hook | delegated: parallel with T2 and T4 | done (RED/GREEN: 18 key and row tests against `todo!()` stubs; value tests written after the code, no RED; 3 mutants caught) | `cab64a6` |
| T4 | S16, S26 (Voron side and shared data) | benchmark PRD K4; PRD *Cargas del banco* | delegated: parallel with T2 and T3 | done (RED/GREEN: `tests/table_data.rs` failed to compile, then 10 passed; C# `selftest` equals the Rust answers) | `0edd736` |
| T5 | S1, S2, S3, S4, S24, S25 | 01 schema evolution; 02 truncate and drop; 11 §2.2; D2, D3, D6 | to decide | pending | |
| T6 | S5, S13, S14, S15, S21, S22 (row half) | 02 two phases and operations; 01; 03 | to decide | pending | |
| T7 | S9, S20, S22 (range and count half) | 03 composite keys and range bounds; 02 scans | to decide | pending | |
| T8 | S10, S22 (bulk half) | 02 bulk load | to decide | pending | |
| T9 | S11, S23 (sequence half) | D4; README sequences | to decide | pending | |
| T10 | S19, S13 (hooks) | D3, D4 | to decide | pending | |
| T11 | S16, S26 (Krummi side, report) | benchmark PRD | to decide | pending | |
| T12 | S16, S26, S27 (runs, measurements), docs | — | to decide | pending | |

## Log

- **L1** (2026-11-10, the user's request, verbatim): «Implementa 7.1 con ODD». Then: «Quisiera que ODD tuviera en
  cuenta los documentos de diseño de las tablas.» Estimate given: 16 h – 26 h; the user answered «Sí», start
  2026-11-10 21:32:39 +0000.
- **L2** (2026-11-10): exploration done (explorer map of the catalog, errors, format, bench and test layout). Branch
  `feat/tables-schema` created from `docs/tables`, which is 50 commits ahead of `main` and holds the PRD and the
  design documents. RDD: on (global).
- **L3** (2026-11-10): T1 committed (`17d8a82`). Default taken: the display of `DifferentTreeType` now names the
  classes as nouns ("a table, not a normal tree"); the kind display pinned by the fixed-size trees *Salida* test
  (`fixed-size of 8 bytes`) is unchanged. `DifferentSchema.expected/found` are `Box<str>` to keep `Error` under
  clippy's `result_large_err` limit. The `bytes_tests` data-file pin changed only through the header page (format
  version). `ulid`/`uuid` text forms (E9) moved to T2 with the `Ulid`/`Uuid` types in `crates/krummi/src/ids.rs`
  (D1: generators and the two types in `ids.rs`, not `generate.rs`). T2, T3 and T4 launched in parallel at 21:51.
- **L4** (2026-11-10): RDD review of T1 (`d487cde..17d8a82`, escalated as high-risk item 1, format bump): lenses
  risk and readability, lineage `review-6baa1e8b06e51441`, **approved** and acknowledged. Advisory findings (non
  blocking): R1-001 and R2-different-tree-type-message (WARNING, `error.rs:530-533`, the tree-versus-tree text of
  `DifferentTreeType` lost its leading "tree"); R2-debug-format-in-display and R2-invalid-value-dangling-comma
  (SUGGESTION, `error.rs:548-589`). Resolution: fixed in T5 (restore the tree-versus-tree wording, tidy the
  `InvalidValue` text). Engram mirror pending: `mem_save` fails with "host session already ended".
- **L5** (2026-11-10): T2 done (`56c5a6b`): `krummi_pager::fill_random` (rustix `rand`, `BCryptGenRandom`, the only
  new `unsafe`), fault `Op::Random`; `Ulid`/`Uuid` text forms; `ids::ulid`, `ids::uuid_v7`, `ids::sequential_uuid`
  with a hidden `ids::testing::Generator` hook; vectors from `tools/guid-vectors` (3 generators × 9 scenarios).
  Defaults: a random-source `Io` error has an empty `folder`; a draw is 10 bytes big-endian (v7 keeps the low 74
  bits); a clock before 1970 reads as millisecond 0; the hook is a separate generator instance. Deviation from
  design 11 §2.8: `fill_random` returns the engine's `Result` (mapped to `Io`), in `random.rs` instead of `sys.rs`.
- **L6** (2026-11-10): RDD review of T2 (`17d8a82..56c5a6b`, assessed medium/under budget, escalated as item 2:
  new `unsafe` FFI in the pager): lenses risk and reliability, lineage `review-518dbc901dd2906a`, **approved** and
  acknowledged, no findings.
- **L7** (2026-11-10): T4 done (`0edd736`): `krummi-bench/src/table_data.rs` (the data specification of W23–W29,
  checksums and digests), `seed_of` in `data.rs` (existing data byte-identical), the C# port in `Data.cs`, W23–W29 on
  Voron's `Table` API in `voron/Tables.cs`; reduced runs passed with matching digests and checksums. Defaults: W24's
  random key is a version 4 uuid; the ulid millisecond is 2026-01-01 + row. Found for T11: `orchestrator/plan.rs`
  deletes a `Start::After` store after its first dependent load, but W26–W29 all start from W23's store. The harness
  must be built against `work/ravendb-7.2.7` (`C:\projects\ravendb` is on v7.1); the csproj header now uses
  double-dash dotnet options.
- **L8** (2026-11-10): T3 done (`cab64a6`): `table::{value, key, row}` and `tests/table_encoding.rs` (33 tests with
  the N5 property test), SQL Server order vectors from `tools/sqlguid-vectors` (1,266 uuids). Defaults: temporal
  `Value`s hold nanoseconds (stored as microseconds); `Value::Vector` holds elements and the `i8` scale, the encoder
  computes the norm; a row with the compressed bit or a reserved bit set reads as damaged; no integer widening; a
  decimal that overflows its precision after rescaling or rounding reports `DecimalPrecision`; `key_range` checks
  value rules, then `InvalidRange`, then size. Seam check after T2–T4: fmt, clippy clean, 650 tests passed.
  RDD review of T3 (`0edd736..cab64a6`, escalated item 1: persisted key and row formats): lenses risk, readability
  and reliability, lineage `review-197a225f96da64f8`, **approved** and acknowledged, advisory findings:
  R2-nanos-doc-inverted (WARNING, `key.rs:36`), R2-nanos-duplicated, R2-decode-fallthrough (SUGGESTION),
  R3-row-encode-silent-type-mismatch, R3-row-decode-temporal-domain, R3-decode-key-decimal-precision (WARNING),
  R3-key-len-zip-truncation (SUGGESTION). Resolution: fixed at the start of T5.
