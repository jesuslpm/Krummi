# Trees and key operations (5.1) — ODD execution document

Objective: implement `docs/prd/krummi/trees-keys.prd.md` (decisions A1–A15, criteria AC-A1–AC-A20), the
authoritative specification; its ancestry is `trees.prd.md` (module 5, B1–B12) and `krummi.prd.md` (root), with the
pager, transactions, journal, journal-group-commit and free-space PRDs inherited.
Branch: `feat/trees-keys`. Delivery: `ask-on-risk` (single branch, merged into `main` after verification). Test runner:
`cargo test --workspace` (Windows; Linux in WSL 2 Ubuntu). Estimate: 3 h – 5 h. Real start: 2026-10-25 06:31:17 +0000.

## Specs

Each spec quotes an acceptance criterion verbatim; the decisions it names (A#, B#, and the inherited ones) are part
of it. The parent's criteria are those it assigns to 5.1, in the parts the 5.1 criteria preamble names (AC-B3 with the
provisional maximum, AC-B4 and AC-B6 in their 5.1 parts, AC-B14 in its same-type part); AC-B18 includes its
2026-10-25 annotation. The PRD's *Validaciones y mensajes*, *Fuera de alcance* and *Dejado a la implementación* bound
every task. The parent's "Antes de implementar 5.1" gate (A14) is task T1.

- **S0** — **Annotation (B1 exception)** — una operación a través de una apertura de otra transacción, también de una ya terminada o de una lectura usada en una escritura, falla sin efecto con la clase de uso incorrecto y la transacción sigue utilizable (excepción a B1, anotada en el padre). Su precedencia es la de una apertura de un árbol borrado (A7): después del tamaño no válido, gana a todo lo demás.
- **S1** — **AC-A1** — Dada una escritura, cuando abre en modo Abrir un árbol que no existe, entonces falla con la clase de árbol no encontrado con su nombre; en modo Crear nuevo, lo crea vacío, y otra vez en ese modo falla con árbol ya existente con su nombre; en modo Abrir o crear, lo crea si no existe y, si existe, lo abre con sus entradas; y dada una lectura, cuando abre un árbol que no existe, entonces falla con árbol no encontrado (A1, A6).
- **S2** — **AC-A2** — Dado un árbol con entradas, cuando se abre en modo Crear o vaciar o Vaciar, entonces queda sin entradas, con número de entradas `0`, y una apertura anterior del mismo árbol lo ve vacío y sigue funcionando; dado un árbol que no existe, Crear o vaciar lo crea y Vaciar falla con árbol no encontrado; y dado un árbol creado y llenado en la misma escritura, Vaciar lo deja vacío y la confirmación lo guarda vacío (A1, A2).
- **S3** — **AC-A3** — Dada una transacción de lectura, cuando abre un árbol, comprueba si existe, lista, lee entradas y consulta el número de entradas, entonces funciona; y abrirlo en cualquier otro modo, borrarlo, renombrarlo o guardar, añadir o quitar entradas no compila (A1, A3, A4).
- **S4** — **AC-A4** — Dado un árbol existente, cuando se pregunta si existe, entonces sí, y tras borrarlo, no; dada una escritura en disco que solo borra un árbol que no existe, cuando se confirma, entonces los archivos del almacén quedan idénticos byte a byte; y Existe y Borrar con un nombre vacío o de 1.025 bytes fallan con tamaño no válido (A3, A10, B4).
- **S5** — **AC-A5** — Dados los árboles `a` y `b`, cuando se renombra `a` a `c`, entonces `c` tiene las entradas de `a`, `a` no existe y una apertura anterior de `a` sigue funcionando; renombrar `c` a `b` falla con árbol ya existente con el nombre `b`, y con sobrescritura deja `b` con las entradas de `c`, sigue valiendo la apertura de `c` y hace fallar con árbol no encontrado, con el nombre `b`, una apertura anterior del `b` original; renombrar un árbol que no existe falla con árbol no encontrado con su nombre, también a su mismo nombre; renombrar `b` a `b` no cambia nada; y dado un árbol `t` abierto, renombrado a `d` y borrado, cuando se usa la apertura de `t`, entonces falla con árbol no encontrado con el nombre `d` (A3, A6).
- **S6** — **AC-A6** — Dado un árbol `a` renombrado a `c` y otra vez a `a`, cuando se usa su apertura original, entonces sigue funcionando; y dado `a` renombrado a `c`, un árbol nuevo creado como `a` y `c` renombrado a `a` con sobrescritura, cuando se usa la apertura del `a` nuevo, entonces falla con árbol no encontrado (A3, A8).
- **S7** — **AC-A7** — Dados árboles creados en desorden, incluidos nombres que solo difieren en mayúsculas, cuando se listan en una lectura, entonces salen con su tipo en orden de bytes; en un almacén sin árboles, la lista está vacía; en una escritura, la lista incluye los creados, renombrados y borrados por ella; y la lista obtenida sigue disponible después de borrar un árbol y de terminar la transacción (A3).
- **S8** — **AC-A8** — Dado un árbol, cuando se guarda una clave nueva y luego la misma clave con otro valor, entonces Leer devuelve el segundo valor; Añadir con esa clave devuelve que no añadió y el valor no cambia; Añadir con una clave nueva devuelve que añadió; Leer una clave que no está devuelve que no está, y Leer una clave guardada con valor vacío devuelve un valor vacío; Quitar devuelve que estaba la primera vez y que no estaba la segunda; y Añadir con una clave que ya está y un valor de 1.025 bytes falla con tamaño no válido (A4).
- **S9** — **AC-A9** — Dada una escritura en disco que solo hace Añadir sobre claves existentes, Quitar sobre claves que no están, Abrir o crear sobre un árbol existente y operaciones que fallan con árbol no encontrado o árbol ya existente, cuando se confirma, entonces termina sin error y los archivos del almacén quedan idénticos byte a byte; y una escritura en disco que solo vacía un árbol ya vacío sí escribe en el almacén (A10, T10).
- **S10** — **AC-A10** — Dado un árbol con entradas suficientes para que se dividan y se fusionen páginas, de modo que el árbol gane y pierda niveles, cuando se guardan claves nuevas, se sobrescriben, se añaden claves nuevas y existentes, se quitan claves presentes y ausentes, se quitan todas hasta la última y se vacía, entonces tras cada operación el número de entradas coincide con las claves que Leer encuentra entre todas las que la prueba usó; una lectura abierta antes ve el número de su instantánea; tras abortar una escritura, el número es el de antes; tras confirmar y reabrir, el número se conserva; y con una página propia del árbol dañada, consultarlo no falla (A5, A9).
- **S11** — **AC-A11** — Dada una escritura con un árbol `t`, cuando repite diez veces, alternando, estos dos pasos: (a) abrir `t`, borrarlo y crearlo otra vez en modo Crear nuevo; (b) abrir `t`, crear un árbol `u` en modo Crear nuevo y renombrar `u` a `t` con sobrescritura; entonces cada apertura de `t` anterior a la última sustitución falla con árbol no encontrado, y solo una apertura de `t` hecha después de la última sustitución ve el árbol vigente (A6, A8).
- **S12** — **AC-A12** — Dado un árbol con una de sus páginas dañada, cuando se abre, se pregunta si existe, se lista o se renombra sin sobrescritura, entonces no falla; cuando se lee una entrada de esa página, entonces falla con la clase de daño y el número de la página, y vuelve a fallar igual al repetirlo; y cuando una escritura lo borra, lo vacía, lo sobrescribe con Renombrar o guarda en esa página y falla con daño, entonces confirmarla falla con la clase de daño y no aplica nada (A9, T11).
- **S13** — **AC-A13** — Dadas operaciones que cumplen varias situaciones a la vez, cuando se ejecutan, entonces fallan con la que gana según A7: renombrar con un origen de 1.025 bytes a un destino existente da tamaño no válido; renombrar un origen que no existe a un destino de 1.025 bytes da tamaño no válido; renombrar un origen que no existe a un destino existente da árbol no encontrado, también en un catálogo de varias páginas con la del destino dañada y la página donde estaría el origen intacta; guardar una clave de 2.049 bytes a través de una apertura de un árbol borrado da tamaño no válido; y una apertura de un árbol borrado cuya página está dañada da árbol no encontrado (A7).
- **S14** — **AC-A14** — Dado un almacén nuevo, cuando se consulta su información antes de crear ningún árbol, entonces su tamaño es como mucho 1 MiB, las páginas libres son las páginas menos las propias del almacén (AC-F15) y la lista de árboles está vacía (A12).
- **S15** — **AC-A15** — Dado un almacén con un árbol ya creado, que se mantiene dentro de una sola región del espacio libre, y tomando las páginas en uso (las páginas menos las páginas libres que da la información) cuando cada confirmación es durable (en modo temporal, cuando devuelve): cuando dos veces seguidas se crea un árbol `x`, se llena con entradas suficientes para ocupar varias páginas y varios niveles, se confirma, se borra y se confirma, entonces las páginas en uso tras el segundo borrado son las mismas que tras el primero, y las de antes del primer ciclo cuando la entrada de `x` no hizo crecer el catálogo (las pruebas lo saben por la implementación); y cuando se crea un árbol vacío, se confirma, se toma el valor, se llena igual, se confirma, se vacía y se confirma, entonces el valor final es el tomado con el árbol vacío (A2, A3, F10, F12).
- **S16** — **AC-A16** — Dada una escritura que crea, renombra, vacía y borra árboles, y guarda y quita entradas con divisiones y fusiones de páginas, cuando se provoca una caída del proceso y simulada de corriente en cada punto en que su confirmación escribe o lleva datos al disco, entonces al reabrir la lista de árboles, cada Leer de las claves que la prueba usó y el número de entradas de cada árbol están todos como antes de la escritura o todos como después, nunca a medias, y el número de entradas coincide con las claves que Leer encuentra (G1, G5, A5).
- **S17** — **AC-A17** — Dada una escritura, cuando guarda un valor de 1.024 bytes con una clave de 2.048 bytes, entonces se guarda; y uno de 1.025 bytes falla con tamaño no válido, con mínimo `0` y máximo `1024` (A11, B6).
- **S18** — **AC-A18** — Dado un almacén de formato `5`, aunque esté vacío, cuando se abre, entonces falla con la clase de formato distinto como formato anterior (A13, AC-B12).
- **S19** — **AC-A19** — Dada la medición de A15, cuando se revisa el informe de cierre, entonces contiene sus tiempos, los tamaños del almacén y los datos del equipo en Windows y en Linux (A15).
- **S20** — **AC-A20** — Dado el código de una aplicación de prueba con una referencia leída de un árbol, cuando la usa después de abrir otro árbol en modo Abrir, de Existe, de Listar o de Leer, entonces compila; cuando la usa después de abrir un árbol en cualquier otro modo, de Borrar o de Renombrar, aunque fallen o no cambien nada, entonces no compila; y una apertura de una transacción de lectura se puede pasar a otro hilo, mientras que pasar a otro hilo una de una escritura no compila (A10, B1).
- **S21** — **AC-B1** — Dada una escritura, cuando guarda entradas con claves de 1, de 1.025 y de 2.048 bytes, cada una con un valor del máximo vigente, entonces se guardan y se leen idénticas; y cuando intenta guardar, leer o borrar con una clave vacía o de 2.049 bytes, entonces falla con la clase de tamaño no válido sin efecto, con los datos de B8: clave, tamaño recibido `0` o `2049`, mínimo `1` y máximo `2048` (5.1).
- **S22** — **AC-B2** — Dada una escritura, cuando crea árboles con nombres de 1 byte y de 1.024 bytes, incluido uno de 1.024 bytes formado por caracteres de varios bytes, entonces se crean; y cuando intenta crear uno con un nombre vacío, de 1.025 bytes o de 600 caracteres de dos bytes (1.200 bytes), entonces falla con la clase de tamaño no válido sin crear nada, con el tamaño recibido en bytes (5.1).
- **S23** — **AC-B3** — Dada una escritura, cuando guarda un valor vacío y otro del máximo vigente, los dos con una clave de 2.048 bytes, entonces se leen idénticos tras confirmar y, en disco, también tras reabrir; y cuando intenta guardar uno de un byte más que el máximo, entonces falla con la clase de tamaño no válido sin guardar nada, con mínimo `0` y el máximo vigente (5.1 con su máximo provisional; 5.3 con 256 MiB).
- **S24** — **AC-B4** — Dada una escritura con cambios ya hechos, cuando una operación falla por tamaño no válido y otra por tipo de árbol distinto, entonces la escritura se confirma sin error y aplica todos sus cambios anteriores y posteriores a esos fallos; dada una transacción de lectura, cuando una operación falla por tamaño no válido, entonces sigue leyendo; dada una escritura en disco cuyas únicas operaciones fallaron con estas clases, cuando se confirma, entonces los archivos del almacén quedan idénticos byte a byte (T10); y dado un almacén que no acepta escrituras (T12), cuando una operación falla por tamaño no válido, entonces falla con esa clase y no con la de entrada y salida, y confirmar esa escritura sin cambios termina sin error (5.1 el tamaño; 5.4 el tipo).
- **S25** — **AC-B5** — Dada una escritura que ya tuvo un fallo de tamaño máximo (P10), cuando una operación posterior falla por tamaño no válido y después se confirma, entonces la confirmación falla con la clase de tamaño máximo y no aplica nada; y dada una escritura que primero tuvo un fallo de tamaño no válido y después uno de tamaño máximo, cuando se confirma, entonces falla igual con la clase de tamaño máximo (B7) (5.1).
- **S26** — **AC-B6** — Dados los árboles `pedidos` y `Pedidos`, y dos árboles cuyos nombres son `é` precompuesta y `e` seguida del acento combinante, cuando se guardan entradas en cada uno, entonces son cuatro árboles distintos y cada uno tiene solo sus entradas (5.1); y dado un árbol normal que se borra, cuando se crea otro con su nombre como multivalor, entonces se crea (5.4).
- **S27** — **AC-B9** — Dado un almacén en disco cuyo catálogo cabe en una página y esa página está dañada, cuando se abre un árbol con un nombre de 1.025 bytes, entonces falla con la clase de tamaño no válido; y con un nombre válido, con la clase de daño (B7) (5.1).
- **S28** — **AC-B10** — Dada una escritura con dos árboles abiertos, y uno de ellos abierto dos veces, cuando guarda entradas en cada uno, entonces cada apertura ve los cambios hechos a través de las otras, y la confirmación las aplica todas (B9) (5.1).
- **S29** — **AC-B11** — Dado el código de una aplicación de prueba, cuando modifica un árbol abierto en una transacción de lectura, usa un árbol después de terminar su transacción o usa una referencia leída después de una operación de modificación que falló sin efecto, entonces no compila (B1, B9) (5.1).
- **S30** — **AC-B12** — Dado un PRD hijo que declara su cambio de formato incompatible, cuando se abre un almacén de la versión anterior, aunque esté vacío, entonces falla con la clase de formato distinto como formato anterior; y dado uno que lo declara compatible, cuando se abre un almacén creado antes de ese cambio, entonces se abre y se usa sin error (B12) (cada PRD hijo que cambie lo que se guarda en disco, desde 5.1).
- **S31** — **AC-B13** — Dada una operación con un nombre y una clave fuera de sus tamaños, cuando se ejecuta, entonces el error de tamaño no válido es del nombre; y con una clave y un valor fuera de sus tamaños, de la clave (B7) (5.1).
- **S32** — **AC-B14** — Dada una escritura con un árbol abierto dos veces, cuando lo borra a través de una apertura y después usa la otra, también tras crear otro árbol con el mismo nombre, entonces la operación falla sin efecto con el error que fije 5.1, el árbol nuevo no cambia y la escritura se confirma sin error; repitiendo tres veces borrar y crear el árbol, con el mismo tipo y con otro, cada apertura anterior falla igual y solo la última ve el árbol vigente; y una transacción de lectura abierta antes de esa escritura sigue viendo el árbol original a través de su apertura, también después de confirmarla (B9, G3) (5.1).
- **S33** — **AC-B17** — Dada una transacción de lectura abierta y una escritura que, con inserciones y borrados, divide y fusiona páginas de un árbol hasta cambiar su altura, cuando la lectura lee todas las entradas del árbol antes y después de que la escritura se confirme, entonces ve exactamente las de su instantánea; y una lectura nueva ve las de la escritura (G3, T14) (5.1, y cada PRD hijo cuyas operaciones cambian la estructura de un árbol).
- **S34** — **AC-B18** — Dada una escritura que divide y fusiona páginas de un árbol, cuando se provoca una caída del proceso y simulada de corriente en cada punto en que su confirmación escribe o lleva datos al disco, entonces al reabrir el árbol tiene todas sus entradas de antes o todas las de después, y se puede leer y modificar sin error (G1, G5, AC4 del padre); y cuando una operación falla a mitad de una división por disco lleno o tamaño máximo, entonces la escritura ya no puede confirmarse y, tras abortarla, el árbol queda como estaba (T11) (5.1, y cada PRD hijo cuyas operaciones cambian la estructura de un árbol).
- **S35** — **AC10 (root)** — Dado un almacén pequeño de prueba, cuando se altera cada uno de sus bytes por separado y se abre y se lee todo el almacén, entonces cada alteración produce un error de la clase de daño o ninguna diferencia observable en los datos leídos, salvo las de la marca de identificación, que producen el error de que no es un almacén; nunca datos distintos de los escritos (G6, D10). Cada PRD hijo lo repite con las estructuras que añade.
- **S36** — **AC20 (root)** — Dada una lectura de cualquier estructura de datos, cuando devuelve un valor, entonces la referencia apunta a memoria del motor (la proyección del archivo, o la memoria de la transacción si la página está cifrada o comprimida) y no a una copia hecha para esa lectura; y usar la referencia después de cerrar la transacción no compila (D11). Cada PRD hijo con lecturas lo comprueba.

## Design

Decisions of the execution, within *Dejado a la implementación*; workers follow them and record any deviation in the
Log.

- **D1 — Crate.** A new crate `krummi-tree` (no `unsafe`, AC21 applies) holds the page layout of tree nodes, the
  B+ tree algorithms and the catalog. It builds on the page API of `krummi-tx` (`page`, `page_mut`, `allocate`,
  `free`), which keeps page numbers on change (copy into the transaction, T14), so a modified node keeps its number
  and no path copying is needed. The public `krummi` crate wraps it, as it wraps `krummi-tx` today.
- **D2 — Roots in the meta page.** `krummi-tx` gains a small set of `u64` slots in its meta page for the layers above
  (read and set inside transactions, carried by commits like the frontier). Trees use two: the catalog root (`0` =
  no catalog yet, A12) and the next tree identity (A8, never reused). Format version `6` (A13).
- **D3 — Catalog.** The catalog is itself a B+ tree keyed by the tree name's UTF-8 bytes; each entry holds the tree
  identity, its kind, its root page and its number of entries (A5).
- **D4 — Nodes.** Slotted pages: a header (leaf or branch, cell count, free space), a sorted array of cell offsets and
  the cells (leaf: key length, value length, key, value; branch: key and child page, plus a leftmost child). With
  8,176 useful bytes, a leaf holds at least two entries with a 2,048-byte key and a 1,024-byte value (A11), and a
  branch at least three keys. Split by bytes into halves; merge a node with a sibling when an operation leaves it
  under a quarter full and the two fit in one page; the root branch with one child is replaced by it (height
  shrinks). Emptying or deleting a tree reads only its branch pages to free every page (A2, A3, A9).
- **D5 — Handles and transactions.** Every transaction gets a serial number unique within the open store. A handle
  carries the store lifetime (it cannot outlive the store), its transaction's serial, the tree identity and kind;
  using it in another transaction fails with misuse without effect (S0). Read handles also carry the tree's root and
  count (a snapshot never changes) and are `Send + Sync`; write handles are `!Send` and their state (root, count,
  name, deleted) lives in a per-transaction table keyed by identity, so every handle of the same tree sees the same
  state (B9), the count is read without touching pages (A5), a deleted tree's handles fail with tree not found
  (A6), and changed entries are written into the catalog at commit.
- **D6 — Public API** (names are proposals; English, Rust conventions). Read transaction: `open_tree(&self, name,
  kind)` (mode Open), `tree_exists(&self, name)`, `list_trees(&self) -> Vec<(String, TreeKind)>` (owned copy, A3),
  `get(&self, &tree, key) -> Option<&[u8]>`, `len(&self, &tree)`. Write transaction: the same reads (all `&self`, so
  they do not end references, A10) plus, all `&mut self`: `open_tree_with(name, kind, mode)` with `OpenMode` =
  `CreateNew | OpenOrCreate | CreateOrEmpty | Empty` (Open is `open_tree`), `delete_tree`, `rename_tree(from, to,
  overwrite)`, `set` (Guardar), `try_add` (Añadir, returns `bool`), `remove` (Quitar, returns `bool`). `TreeKind`
  is `#[non_exhaustive]` with `Normal`.
- **D7 — Errors.** Four new classes in `krummi-pager`'s `ErrorKind`/`Error`, with the data of T18 plus their own:
  `InvalidSize { what: name|key|value, received, min, max }`, `DifferentTreeType { name, found, requested }`,
  `TreeNotFound { name }`, `TreeAlreadyExists { name }`. They are public validation errors: they never become the
  write's recorded failure (T11), unlike misuse of the page API, which keeps its current behavior; the cross-
  transaction misuse of S0 does not record a failure either.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T0 | B1 annotation (S0), execution document | inline | done | (this commit) |
| T1 | A14 gate: public API skeleton (D5–D7) and compile tests (S20, S29) | delegated (context backstop) | pending | |
| T2 | Format 6 and meta slots (D2), nodes and B+ tree algorithms with internal tests (D4): S18, S30 | delegated (context backstop) | pending | |
| T3 | Catalog (D3), handles (D5), public API wiring (D6), errors (D7) and every functional criterion: S0–S11, S13, S14, S17, S21–S28, S31, S32, S36 | delegated (context backstop) | pending | |
| T4 | Damage, crashes, faults, snapshots and pages: S12, S15, S16, S33–S35 | delegated (context backstop) | pending | |
| T5 | Measurement (A15): S19 | inline | pending | |
| T6 | Docs, checks, verify, RDD, report | inline | pending | |

Route evidence: no parallel units (each task builds on the previous one); the writer trigger that fires is the
long-session backstop (the parent context already holds both PRD sessions), so each task goes to one bounded ODD
Worker in sequence. Risk: item 1 (persisted format change, format `6`) and item 3 (new public API and error classes)
→ one independent verifier at the feature's end.

## Log

- **L1** (2026-10-25, user): "Adelante con la ejecución ODD"
- **L2** (2026-10-25, user, before the start): the B1 conflict (transactions as values, several handles, compile-time
  handle expiry) is resolved by "Comprobarlo al ejecutar": a handle used in another transaction fails with misuse
  without effect (S0), annotated in `trees.prd.md` and `trees-keys.prd.md`.
- **L3** (06:31:17): estimate 3 h – 5 h confirmed; branch `feat/trees-keys`.
