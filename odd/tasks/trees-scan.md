# In-order scan and prefix search (5.2) — ODD execution document

Objective: implement `docs/prd/krummi/trees-scan.prd.md` (decisions R1–R11, criteria AC-R1–AC-R15), the
authoritative specification; its ancestry is `trees.prd.md` (module 5, B1–B12) and `krummi.prd.md` (root), and it
reuses its sibling `trees-keys.prd.md` (5.1, A1–A15), with the pager, transactions, journal, journal-group-commit and
free-space PRDs inherited.
Branch: `feat/trees-scan`. Delivery: `ask-on-risk` (single branch, left unmerged for the user's review). Test
runner: `cargo test --workspace` (Windows; Linux in WSL 2 Ubuntu). Estimate: 3 h – 4 h 30 min. Real start:
2026-10-26 09:38:44 +0000.

## Specs

Each spec quotes an acceptance criterion verbatim; the decisions it names (R#, A#, B#, and the inherited ones) are
part of it. The PRD's *Validaciones y mensajes*, *Salida*, *Fuera de alcance* and *Dejado a la implementación* bound
every task. The parent criteria assigned to 5.2 are S16–S18, and the root criteria every child repeats are S19–S20.
The PRD's annotations to earlier PRDs are already committed (`c6fa485`).

- **S1** — **AC-R1** — Dado el árbol del ejemplo de *Salida*, cuando se piden los recorridos de su tabla, entonces cada uno devuelve exactamente las claves indicadas, en ese orden, con sus valores; sacando alternativamente por los dos extremos un recorrido de todo el árbol, empezando por el delantero, salen `a`, `c`, `ab`, `ba` y `b`, y después ninguna más por ningún extremo; y los dos Quitar intervalo del ejemplo, cada uno sobre el árbol original, devuelven `2` y `1` y quitan esas claves (R1–R4, R7).
- **S2** — **AC-R2** — Dado un árbol con entradas suficientes para tener varios niveles, cuando se recorre entero hacia delante, hacia atrás y alternando extremos al azar, entonces salen todas sus claves, en orden, en orden inverso y una vez cada una, y su número coincide con el número de entradas (A5); y cuando se piden recorridos cuyos límites son una clave existente, una clave entre dos claves vecinas (también donde cambian de página) o una clave antes de la primera o después de la última, cada uno incluido y excluido, entonces cada extremo empieza donde dice R2 (R1, R2).
- **S3** — **AC-R3** — Dadas claves con bytes de `0x00` a `0xFF`, incluidas `FF`, `FF FF`, `FF FF 00` y `FF FF FF`, cuando se recorre el prefijo `FF FF`, entonces salen `FF FF`, `FF FF 00` y `FF FF FF` hacia delante, y en orden inverso hacia atrás; con el prefijo vacío, todas las claves; con un prefijo de 2.048 bytes igual a una clave, esa clave; y con un prefijo que no tiene ninguna clave, ninguna (R4).
- **S4** — **AC-R4** — Dados límites invertidos, límites con la misma clave y alguno excluido, y el prefijo `a` con el límite inferior `c`, cuando se recorre y se quita el intervalo, entonces no sale ninguna entrada, Quitar intervalo devuelve `0` y no fallan, también con la raíz del árbol dañada; a través de una apertura de un árbol borrado, fallan igual con árbol no encontrado; con la misma clave incluida en los dos límites, sale esa entrada si existe, una sola vez; dada una escritura en disco que solo pide recorridos, quita intervalos vacíos o sin claves en el árbol y operaciones que fallan por tamaño no válido, cuando se confirma, entonces termina sin error y los archivos del almacén quedan idénticos byte a byte; y una escritura en disco que quita una sola entrada con Quitar intervalo sí escribe en el almacén (R3, R7, T10).
- **S5** — **AC-R5** — Dada una transacción de lectura y una de escritura, cuando recorren con un límite inferior o superior vacío o de 2.049 bytes, y la escritura además quita un intervalo con ellos, entonces falla con tamaño no válido sin efecto, como clave, con el tamaño recibido, mínimo `1` y máximo `2048`; con un prefijo de 2.049 bytes, como prefijo, con mínimo `0` y máximo `2048`; con límites de 1 y de 2.048 bytes y prefijos de 0 y de 2.048 bytes, no falla; con los dos límites fuera de tamaño, el error es del inferior, y con el superior y el prefijo, del superior; con un límite fuera de tamaño en un árbol sin entradas o en un intervalo invertido, falla igual; y la transacción sigue utilizable (R5, B7).
- **S6** — **AC-R6** — Dada una escritura que guarda y quita entradas y después pide un recorrido, cuando lo consume, entonces ve esos cambios; y dada una escritura con un árbol abierto dos veces, cuando quita un intervalo a través de una apertura, entonces la otra ve el árbol sin las entradas quitadas y el número de entradas nuevo (R6, R7).
- **S7** — **AC-R7** — Dada una transacción de lectura con un recorrido a medias y otro sin empezar, cuando una escritura quita intervalos y guarda entradas hasta cambiar la altura del árbol y se confirma, entonces los dos recorridos devuelven exactamente las entradas de su instantánea, y una lectura nueva ve las de la escritura (R6, G3, AC-B17).
- **S8** — **AC-R8** — Dado el código de una aplicación de prueba con un recorrido de una escritura, cuando lo usa después de guardar, añadir, quitar, quitar un intervalo, abrir un árbol en un modo distinto de Abrir, borrar o renombrar un árbol, aunque fallen o no cambien nada, entonces no compila; cuando lo usa después de Leer, de abrir un árbol en modo Abrir, de Existe, de Listar, del número de entradas o de pedir otro recorrido, entonces compila; una clave devuelta por un recorrido de una lectura se usa después de soltar el recorrido, y una de una escritura se usa después de soltarlo pero no después de la siguiente operación que puede modificar; Quitar intervalo en una transacción de lectura no compila; y pasar a otro hilo un recorrido de una lectura compila, y uno de una escritura no (R6, R7, R9, AC-B11).
- **S9** — **AC-R9** — Dado un árbol con varios niveles, cuando se quitan intervalos que caen dentro de una página, que cruzan varias páginas y niveles, con prefijo y sin él, entonces cada uno devuelve cuántas entradas quitó, solo esas claves desaparecen, el número de entradas baja en esa cantidad y un recorrido del árbol entero devuelve las demás; y tras confirmar y reabrir, se conserva (R7, A5).
- **S10** — **AC-R10** — Dado un almacén con un árbol ya creado, que se mantiene dentro de una sola región del espacio libre, tomando las páginas en uso como en AC-A15, cuando se crea un árbol vacío, se confirma, se toma el valor, se llena con entradas suficientes para ocupar varias páginas y varios niveles, se confirma, se le quitan todas sus entradas con un intervalo de límites abiertos y se confirma, entonces el valor final es el tomado con el árbol vacío; y lo mismo cuando se le quitan con diez intervalos contiguos, en escrituras distintas (R7, A2, F10).
- **S11** — **AC-R11** — Dada una escritura con un árbol abierto dos veces, cuando lo borra a través de una apertura y después pide un recorrido o quita un intervalo con la otra, entonces falla con árbol no encontrado, con el último nombre del árbol, sin efecto; dada una apertura de una transacción ya terminada, cuando se usa en otra para recorrer o quitar un intervalo, entonces falla con uso incorrecto sin efecto; en cada uno de esos dos casos, con además un límite de 2.049 bytes, gana tamaño no válido, y con además la raíz del árbol dañada, gana su clase (uso incorrecto o árbol no encontrado) sobre el daño; y en todos los casos la transacción sigue utilizable (R8).
- **S12** — **AC-R12** — Dada una escritura que quita intervalos con fusiones de páginas que cambian la altura del árbol, cuando se provoca una caída del proceso y simulada de corriente en cada punto en que su confirmación escribe o lleva datos al disco, entonces al reabrir el árbol tiene todas sus entradas de antes o todas las de después y el número de entradas coincide; y cuando se simula un fallo de T11 en cada punto en que Quitar intervalo reserva, lee o libera una página, entonces confirmar falla con la clase del primer fallo y no aplica nada, y tras abortar el árbol y su número de entradas están como antes (R7, G1, G5, AC-B18).
- **S13** — **AC-R13** — Dado un árbol con una página hoja dañada, y otro con una página intermedia dañada, cuando una lectura los recorre por los dos extremos hasta necesitar esa página, entonces falla con la clase de daño y el número de la página, las entradas devueltas antes son las correctas, después no devuelve más entradas por ningún extremo, la transacción sigue utilizable y un recorrido nuevo que la necesita falla igual; un recorrido y un Quitar intervalo cuyas claves están en otra parte del árbol, ni en esa página ni en sus vecinas ni en el camino hacia ellas, no fallan; cuando una escritura recorre o quita un intervalo que la necesita y falla con daño, entonces confirmarla falla con la clase de daño y no aplica nada; y cuando se simula un fallo de memoria agotada o de entrada y salida a mitad de un recorrido, entonces falla con esa clase y termina igual (R8, A9, T11).
- **S14** — **AC-R14** — Dado un almacén creado y llenado con la versión de 5.1, conservado como dato de prueba, cuando se abre con la de 5.2, entonces su versión de formato es `6` y se recorre sin error; y cuando se le quitan intervalos con fusiones que cambian la altura de un árbol, se confirma y se reabre, entonces un recorrido entero y la lectura de cada clave que queda no fallan con daño; y cuando ese almacén se abre con la versión de 5.1, entonces la lista de árboles, cada clave que queda y el número de entradas se leen sin error, y guardar y quitar entradas hasta provocar divisiones y fusiones, confirmar y reabrir funcionan sin error. La parte de la versión de 5.1 es una comprobación de la ejecución, con esa versión compilada aparte y su resultado en el informe de cierre, no una prueba que se guarda en el repositorio (R10).
- **S15** — **AC-R15** — Dada la medición de R11, cuando se revisa el informe de cierre, entonces contiene sus tiempos y los datos del equipo en Windows y en Linux (R11).
- **S16** — **AC-B7** (5.2 part) — Dadas claves guardadas en orden aleatorio, incluidas claves con bytes de `0x00` a `0xFF` y claves que son prefijo de otras, cuando se recorre el árbol en orden, entonces salen ordenadas byte a byte sin signo, y cada clave que es prefijo de otra va antes que ella (5.2).
- **S17** — **AC-B17** with Quitar intervalo, checked by AC-R7 (S7).
- **S18** — **AC-B18** with Quitar intervalo, including its 2026-10-25 annotation, checked by AC-R12 (S12).
- **S19** — **AC10** (root) with scans — Dado un almacén pequeño de prueba, cuando se altera cada uno de sus bytes por separado y se abre y se lee todo el almacén, entonces cada alteración produce un error de la clase de daño o ninguna diferencia observable en los datos leídos, salvo las de la marca de identificación, que producen el error de que no es un almacén; nunca datos distintos de los escritos (G6, D10). Cada PRD hijo lo repite con las estructuras que añade.
- **S20** — **AC20** (root) with scans — Dada una lectura de cualquier estructura de datos, cuando devuelve un valor, entonces la referencia apunta a memoria del motor (la proyección del archivo, o la memoria de la transacción si la página está cifrada o comprimida) y no a una copia hecha para esa lectura; y usar la referencia después de cerrar la transacción no compila (D11). Cada PRD hijo con lecturas lo comprueba.

## Design

Decisions within *Dejado a la implementación*:

- **D1 — Public API.** Both transactions get `range(&tree, bounds)` and `range_prefix(&tree, prefix, bounds)`,
  with `bounds: impl RangeBounds<&[u8]>` (so `..`, `a..b`, `a..=b` and `(Bound, Bound)` of slices work; `RangeBounds<[u8]>` would not take `a..b`, which std implements only for sized types), returning a
  double-ended, fused iterator of `Result<(&[u8], &[u8])>`: `ReadRange` (Send, as its transaction is Sync) and
  `WriteRange` (not Send). The write transaction gets `remove_range(&tree, bounds) -> Result<u64>` and
  `remove_range_prefix(&tree, prefix, bounds) -> Result<u64>`. Checks run in the R8 order: sizes (lower, upper,
  prefix), the handle (misuse, deleted tree), the R3 emptiness test, then pages; a scan reads no page until its first
  step from each end.
- **D2 — R3 emptiness.** An interval is empty without reading pages when no valid key (1 to 2,048 bytes) that starts
  with the prefix lies between the bounds; with the empty prefix this includes inverted bounds and equal bounds with
  one excluded. It is decided exactly: the smallest valid key with the prefix that satisfies the lower bound is
  compared with the upper bound. Otherwise the prefix folds into the two monotonic predicates that test a key
  against the bounds (a key at or after the prefix is after it; a key before it or starting with it is before its
  end), so a descent finds both ends with a binary search and no key needs a separate prefix test.
- **D3 — Scan.** A cursor per end holds the path from the root (page and child index per level, reserved once with
  the tree's height) and positions itself on its first step; the ends stop when the next key would pass the other
  end's last returned key or the bound. Every node is read with the 5.1 checks (damage with its page number); after
  any failure both ends stop.
- **D4 — Remove range.** One descent: a leaf removes its contiguous run of cells in the interval; a branch recurses
  into the children holding the bounds, frees the children fully inside the interval (reading their leaves to count
  their entries) and drops their separators, then fixes the boundary children with the 5.1 drop and merge steps.
  The root is fixed as 5.1 fixes it after a key operation (shared code). A removal that finds no entry touches no
  page (T10).

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T0 | — | inline | done | — |
| T1 | S1 (scans), S2, S3, S5, S6 (scan part), S8, S11 (scan part), S13 (scan damage), S16, S20 | inline | done | `51be749`, fix `b07f248` |
| T2 | S1 (removes), S4, S6 (remove part), S8 (removal cases), S9, S10, S11 (remove part), S13 (removes) | inline | done | `1ddd9ca` |
| T3 | S7, S12, S13 (out-of-memory and I/O failures mid-scan), S17, S18, S19 | inline | done | `f67d59b` |
| T4 | S14 | inline | done | `d6f2263` |
| T5 | S15 | inline | code done, runs pending | T5COMMIT |
| T6 | docs, checks, verification, RDD review, closing report | inline | pending | — |

Route evidence: every task is one behavior of one crate pair (krummi-tree and krummi) built on the previous one, so
there are no independent parallel units and no writer trigger; the parent writes inline following this document.
T2 changes stored data on removal (high-risk item 1), so the independent verifier runs at the end of the feature.

## Log

- **L1** (2026-10-26, user): "Empieza con la implementación ODD", then "Ya puedes empezar con la ejecución ODD"
  after the estimate "3 h – 4 h 30 min" and a wait for another session (not active time).
- **L2** (09:38–, exploration): the 5.1 tree code has no partial or descending traversal; nodes have no sibling links,
  so a cursor keeps the path. `SizeSubject` is `#[non_exhaustive]`, so adding `Prefix` is not breaking. Test
  infrastructure mapped: `trees.rs` helpers, `tree_integrity.rs` crash sweep, `tree_memory.rs` OOM sweep, the T11
  sweep in `crates/krummi/src/tests.rs`, trybuild cases in `tests/ui/trees*`, the `tree_timing` example.
- **L3** (T1): RED: `tree_scans.rs` and the trybuild cases failed to compile (39 errors: no `range`, `range_prefix`,
  `ReadRange`, `WriteRange`, `SizeSubject::Prefix`). GREEN: 10 tests, 4 unit tests of the interval, 5 compile-fail
  and 1 compile-pass cases. `a..b` of slices does not implement `RangeBounds<[u8]>` (std implements it only for
  sized types), so the API takes `RangeBounds<&[u8]>` (D1 updated). Tree test helpers moved to
  `tests/support/mod.rs` (the 28 tests of `trees.rs` unchanged and passing). The S13 out-of-memory and I/O part
  moves to T3, where the failure injection lives.
- **L4** (RDD review 1, 09:57–10:08): commit `51be749` (base `0e204da`), assessed `high` (`process_boundary`
  on `tests/support/mod.rs`); lenses resilience, readability, reliability; lineage `review-b3f0715ef79299e4`;
  **approved**, acknowledged, authority burned. Advisory findings, all non-blocking:
  - R4-eager-next-leaf-read (WARNING): a bounded scan read the neighbour leaf before seeing that the bound ended
    it, so a damaged neighbour failed a scan that had returned everything. Fixed: both ends stop at the parent's
    separator before reading the next child (`Interval::all_before_lower` for the back); RED observed (damage on
    page 163 without the fix), GREEN with it; covered in `ac_r13_scans_and_damaged_pages`.
  - R2-misplaced-partition-doc (WARNING) and R3-002 (SUGGESTION): `partition` took `child_index`'s doc comment.
    Fixed.
  - R2-design-doc-drift (SUGGESTION): D1 and D2 described `RangeBounds<[u8]>` and a prefix turned into bounds.
    Fixed: D1 and D2 describe the code.
  - R2-shadowed-open-helper (SUGGESTION): the scan tests' `open` shadowed the support one. Renamed `tree_t`.
  - R3-001 (WARNING): S13's out-of-memory and I/O failures mid-scan had no test. Accepted: moved to T3.
  - R2-mirrored-cursor-logic (SUGGESTION): the front and back step functions mirror each other. Kept: each is
    short and reads directly; a direction abstraction would hide the index arithmetic that differs.
- **L5** (T2): RED: the removal tests failed to compile (30 errors: no `remove_range`, `remove_range_prefix`). GREEN:
  19 scan and removal tests, including `range_removals_match_a_model` (40 rounds of random intervals with keys
  stored back in between, checked after each round by a whole scan in both directions, the count and a read of
  every key), and 2 more compile-fail cases. The root fix after a change moved into `fix_root`, shared by key
  operations and range removal; freeing a subtree can count its entries (`free_subtree(.., count)`). Every test of
  `krummi` and `krummi-tree` passes, so the 5.1 behavior that shares that code holds.
- **L6** (RDD review 2, 10:12–10:30): commits `b07f248` and `1ddd9ca` (base `51be749`), assessed `medium`
  (`under_budget`) and raised to `high` with `--escalate-item 1` (range removal deletes stored entries); the
  assessment always covers the range up to `HEAD`, so the fix commit was reviewed with T2 instead of alone. Lenses
  risk, reliability, resilience; lineage `review-7dbcd8773193aef6`; **approved**, acknowledged, authority burned.
  Two attempts before it failed in the harness, not in the review: the consent invocation ran through `cmd.exe`
  and then WSL's `bash` (both `not_started`, retry safe). Advisory findings:
  - R4-partial-range-removal-on-error (WARNING): after a failure of T11 inside a range removal the write's tree is
    partly changed. Accepted as specified (PRD *Dejado a la implementación*: what the write sees after a T11
    failure; it cannot commit), and hardened: the count update saturates, so further operations in that failed
    write cannot overflow it.
  - R3-underfull-after-boundary-merge (WARNING): merging the two bound children can leave a node under a quarter.
    Dismissed with evidence: 5.1's merge policy keeps no minimum fill either (a merge only happens when both fit
    in one page) and the parent PRD leaves the fill policy to the implementation; AC-R10 checks that pages come
    back when a tree is emptied.
  - R3-no-structural-check-after-range-removal (SUGGESTION): whole scans read every node with the layout and level
    checks of 5.1, so a malformed node fails them; occupancy is not an invariant (above). No change.
- **L7** (T3): the T11 sweep of 5.1 now takes the write and the failure classes (`t11_sweep_of`); its range
  scenario alternates disk full, maximum size and input and output at 598 points (sample: every 13th), covering
  range removals and scans in a write (the I/O failure in the middle of a scan; a read transaction maps its pages
  and has no read that fails with I/O). Out of memory: during a range removal, in the middle of a write's and a
  read's scan, and when a scan copies its bounds. Crash sweep of a write with range removals that lowers the tree
  (`ac_r12_crashes_during_a_commit_with_range_removals`), and root AC10 now scans every tree from both ends. AC-R7
  checks the height drop with a new test helper `krummi_tree::height`.
- **L8** (T4, 10:15–10:22): the 5.1 code (`0e204da`) exported with `git archive` to the scratch folder, with an
  example `compat` (`make`, `check`), built in release. `make` created the fixture `tests/data/store-5.1/krummi.data`
  (512 KiB: tree `t` with keys 0–399 of 300 bytes, tree `small`), committed with `*.data binary` in
  `.gitattributes`. Committed test `ac_r14_a_store_of_5_1_scans_and_keeps_its_format_after_range_removals`: format
  `6`, whole scan, removals that lower the tree, reopening, no damage. Execution check (not a committed test): the
  5.2 code (`HEAD`, exported the same way) removed `[190, 180, 10]` entries from a copy of the fixture, leaving 20;
  the 5.1 `check` then printed "20 keys before, 153 after storing 400 and removing 267 with the 5.1 engine": it read
  the list, every key, its value and the count, split and merged pages, committed and reopened without error.
