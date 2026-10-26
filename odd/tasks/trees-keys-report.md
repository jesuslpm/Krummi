# Árboles y operaciones por clave (5.1) terminado: verificado, en su rama sin fusionar

Informe de cierre de `trees-keys` (2026-10-25). Enfoque de ejecución: ODD.
Fuente de producto: [trees-keys.prd.md](../../docs/prd/krummi/trees-keys.prd.md) (padre:
[trees.prd.md](../../docs/prd/krummi/trees.prd.md)). Documento de ejecución: [trees-keys.md](trees-keys.md).

Rama `feat/trees-keys`, sin fusionar en `main` por indicación del usuario (L11), que la revisará con Codex. Nada se ha
subido. Último commit de código: `06ec986` (corrección de la revisión de Codex).

## Resultado

- **Especificación:** las 37 especificaciones S0–S36 tienen prueba y pasan, salvo las partes que no aplican en 5.1
  (la mitad «compatible» de S30 y la mitad «nombre antes que clave» de S31) y S19, que se cumple con este informe.
  `cargo test --workspace`: 293 pasan, 0 fallan, 9 ignoradas (Windows, HEAD, con la corrección R1; la intermitencia
  conocida de `ac_f18` apareció en una corrida y pasa aislada); en Linux (WSL 2 Ubuntu, copia de
  `1138169`), 283 pasan, 0 fallan, 9 ignoradas.
- **Aceptación:** barrido de cada byte de un almacén de 128 KiB con árboles (AC10), 6.477,5 s en Windows, sin
  fallos; caídas en cada punto de la confirmación y del cierre (AC-A16, AC-B18), 32,3 s; un fallo en cada operación
  de página de divisiones y fusiones (AC-B18), 25,8 s; medición de A15 en Windows y Linux (sección Medición).
- **Revisiones y verificación:** cuatro revisiones RDD aprobadas (sección Revisiones RDD); verificador independiente
  sin bloqueantes ni hallazgos de seguridad de memoria (L16), con 16 casos de compilación, sondas de referencias
  retenidas mientras otras escrituras dividen, fusionan, liberan y reutilizan páginas, y una comprobación diferencial
  con un almacén de formato `5` construido con `main`.
- **Revisión externa (Codex):** [2026-10-25-code.md](../../docs/reviews/trees-keys/2026-10-25-code.md), dos
  hallazgos major sin bloqueantes, resueltos según
  [2026-10-25-code-resolutions.md](../../docs/reviews/trees-keys/2026-10-25-code-resolutions.md) y pendientes de una
  revisión de seguimiento: R1 (memoria agotada, `06ec986`) y R2 (datos del equipo, `62b1813`).
- **Hallazgos corregidos por el camino:** una página con checksum válido pero estructura incoherente hacía entrar en
  pánico en lugar de dar daño (nodos y entradas del catálogo, `4735a47`); una hoja vacía no raíz podía quedar en el
  árbol (`4735a47`); un nodo que se nombra a sí mismo o a un antecesor como hija se liberaba dos veces (`4ab9216`);
  una asignación de memoria fallida en las operaciones de árboles abortaba el proceso en lugar de dar
  `OutOfMemory` (Codex R1, `06ec986`: todas pasan por un helper falible con el punto de fallo `Op::Alloc` del Pager,
  con 9 pruebas que barren cada reserva).

## Cobertura de los criterios

La traza criterio → prueba está en el Log del documento de ejecución (L5, L6, L8, L12) y en el veredicto del
verificador (L16). Resumen:

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| S0 (excepción a B1) | `s0_a_handle_of_another_transaction_fails_with_misuse_without_effect` | PASS |
| AC-A1–AC-A11, AC-A13, AC-A14, AC-A17, AC-A18 | `ac_a1_…` a `ac_a18_…` en `crates/krummi/tests/trees.rs` y `crates/krummi-pager/tests/open.rs` | PASS |
| AC-A12, AC-A15 | `ac_a12_a_damaged_page_of_a_tree`, `ac_a15_deleting_or_emptying_a_tree_gives_its_pages_back` | PASS |
| AC-A16 | `ac_a16_crashes_during_a_commit_of_catalog_and_key_operations` (y la carrera ignorada a lo largo del cierre) | PASS |
| AC-A19 | este informe (sección Medición) | PASS |
| AC-A20, AC-B11 | `ac_a20_ac_b11_tree_handles_and_references_at_compile_time` (trybuild) | PASS |
| AC-B1–AC-B6, AC-B9, AC-B10, AC-B13, AC-B14 | `ac_b1_…` a `ac_b14_…` en `trees.rs`; la parte T12 de AC-B4 en `crates/krummi/src/tests.rs` | PASS |
| AC-B12 | `ac_a18_format_5_is_a_previous_format_even_when_empty`, `ac9_…`, `ac_f17_…` | PASS (mitad incompatible) |
| AC-B17 | `ac_b17_a_read_keeps_its_snapshot_while_a_write_changes_the_height` | PASS |
| AC-B18 | `ac_b18_crashes_during_a_commit_with_splits_and_merges`, `ac_b18_a_failure_at_…` | PASS |
| AC10 (raíz) | muestra `ac10_sample_of_altered_bytes_with_trees` (paso 397); carrera completa ignorada, 6.477,5 s | PASS |
| AC20 (raíz) | `ac20_values_read_from_a_tree_point_into_the_engine_s_memory` | PASS |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `2702406..862cff7` (T0–T2) | risk, reliability | aprobada | WARNING R1-001: los nodos confiaban en su cabecera → validación y daño en `4735a47`. WARNING R1-002 y R3-todo-public-api: métodos en `todo!()` → implementados en T3 (`9e3ad1b`). WARNING R3-empty-nonroot-leaf: hoja vacía no raíz → `4735a47` | `review-3cec8f5748714163` |
| `862cff7..9e3ad1b` (T3) | risk, readability, reliability | aprobada | WARNING R1-001, R2-001 y R3: `unimplemented!` en `encode` para otros tipos → inalcanzable en 5.1 (solo `Normal`), pasa a `unreachable!` en `4735a47`. WARNING R1-002: tipo desconocido en el catálogo → daño en `4735a47`. SUGGESTION R2-002 y R2-003 → comentarios en `4735a47`. WARNING R3-unresolved-workspace-test-failures → la intermitencia se reproduce en `main` (L9) | `review-c3a949b0f2248dff` |
| `9e3ad1b..32d4d04` (T4, medición, documentación y los dos commits de Codex del usuario) | risk, reliability | aprobada | WARNING R3-001: doble liberación ante un hijo que se nombra a sí mismo → `4ab9216`. SUGGESTION R3-002: el costo de la validación de nodos no se midió antes y después → pendiente | `review-52ac120ffe927c92` |
| `32d4d04..5ffa33a` (último tramo) | risk, reliability | aprobada | SUGGESTION R3-ci-filter-zero-match: el paso de CI que filtra por `ac_b18` pasaría si la prueba cambiara de nombre → pendiente | `review-74e5af866c16bc0b` |
| `5ffa33a..06ec986` (informe, revisión de Codex, R2 y R1) | resilience, reliability | aprobada | WARNING R3-damaged-return-changed → refutado: `track()` devuelve el mismo resultado, así que `fail()` devuelve el mismo error. SUGGESTION R3-global-fault-injection → refutado: el registro de fallos es `thread_local!`. WARNING R3-weak-sweep-thresholds: umbrales mínimos débiles en algunos barridos de memoria → pendiente. SUGGESTION R4-001: la tabla ordenada de árboles cuesta O(N²) al abrir miles de árboles en una escritura → pendiente | `review-99c3098713ed944a` |

Las evaluaciones de `2702406..af8e299` y `2702406..862cff7` dieron riesgo medio y `under_budget`; las cuatro
revisiones se elevaron a riesgo alto con el ítem 1 (formato persistente o lectura de páginas persistidas).

## Medición (A15)

Release, 1.000.000 de claves aleatorias de 16 bytes con semilla fija, valores de 100 bytes, confirmación durable,
escrituras de 1.000 entradas; `cargo run --release -p krummi --example tree_timing`.

| Operación | Windows (s) | Windows (µs por entrada) | Linux, WSL 2 ext4 (s) | Linux (µs por entrada) |
|---|---|---|---|---|
| Guardar en orden de clave | 3,52 | 3,52 | 13,11 | 13,11 |
| Guardar al azar | 26,79 | 26,79 | 37,50 | 37,50 |
| Leer al azar (tras reabrir, una lectura) | 3,69 | 3,69 | 3,61 | 3,61 |
| Quitar al azar | 26,82 | 26,82 | 36,68 | 36,68 |

Tamaño del almacén tras cada carga, en los dos sistemas: 268.435.456 bytes (el archivo crece doblando, P11).

Equipo (los dos entornos en la misma máquina; consultado el 2026-10-26 con `Get-CimInstance`, `Get-PhysicalDisk`,
`lscpu` y `df`, tras la revisión de Codex, R2):

| Entorno | Sistema operativo | Procesador | Disco |
|---|---|---|---|
| Windows | Windows 11 Pro 10.0.26200, x64 | 13th Gen Intel Core i9-13980HX, 24 núcleos, 32 hilos | NVMe Samsung MZVL22T0HBLB-00B00 (SSD, 2 TB), NTFS |
| Linux | WSL 2 Ubuntu, ext4 | el mismo (`lscpu`: 32 CPU) | disco virtual ext4 de WSL 2 (`/dev/sde`, 1.007 GB), guardado como `.vhdx` en el mismo NVMe |

## Tiempo

- Estimación: 3 h – 5 h. Arranque real: 2026-10-25 06:31:17 +0000. Fin: 2026-10-26 04:53:54 +0000 (pausa entre
  sesiones de 17 h 21 min, fuera del activo).
- **Duración activa desde el arranque: 5 h 01 min**, dentro de la estimación: 4 h 26 min de la ejecución y 35 min de
  las correcciones de la revisión de Codex, en una segunda sesión (2026-10-26). El 89 % fue código y pruebas, y una
  parte grande de T4 fue la carrera de aceptación de AC10 de cada byte dentro del worker (6.477,5 s, unos 1 h 48 min):
  abrir un archivo recién escrito cuesta unos 35 ms en esta máquina.
- Esperas por el usuario: ninguna. Los mensajes del usuario durante la ejecución (L4, L11 y el cambio de la regla de
  merge) no detuvieron el trabajo.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 3 min 09 s | 1 % | 1 |
| Código y tests | 3 h 56 min | 79 % | 4 |
| Checks (fmt, clippy, suite de tests) | 12 min 40 s | 4 % | 2 |
| Verificación independiente | 10 min 26 s | 3 % | 1 |
| Revisión RDD | 15 min 15 s | 5 % | 5 |
| Correcciones de hallazgos | 22 min 44 s | 8 % | 1 |
| Cierre (informe y merge) | 0 min 01 s | 0 % | 2 |
| **Total activo** | **5 h 01 min** | **100 %** | **16** |

- Inicio: 2026-10-25 06:31:36 +00:00. Fin: 2026-10-26 04:53:54 +00:00.
- Arranque confirmado: 06:31:36. Activo desde el arranque: **5 h 01 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.
- Pausas entre sesiones (fuera del activo): 17 h 21 min.
  - 2026-10-25 10:58:15 +00:00 – 2026-10-26 04:19:29 +00:00 (17 h 21 min)

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD review 1 lenses | 07:23:23 | 07:24:50 | 1 min 27 s |
| Revisión RDD | RDD review 2 lenses | 07:59:49 | 08:00:57 | 1 min 08 s |
| Aceptación | A15 measurement Windows | 10:38:25 | 10:39:38 | 1 min 13 s |
| Aceptación | A15 measurement and suite Linux | 10:40:00 | 10:40:05 | 0 min 05 s |
| Revisión RDD | RDD review 3 lenses | 10:38:25 | 10:40:20 | 1 min 55 s |
| Aceptación | A15 measurement and suite Linux | 10:40:29 | 10:44:43 | 4 min 14 s |
| Revisión RDD | RDD review 4 lenses | 10:54:15 | 10:54:39 | 0 min 24 s |
| Revisión RDD | RDD review 5 lenses | 04:42:33 | 04:43:44 | 1 min 11 s |
| **Aceptación (total)** | | | | **5 min 32 s** |
| **Revisión RDD (total)** | | | | **6 min 05 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `a0b996d` | docs: annotate the B1 handle exception and add the 5.1 execution document | T0 | 2 min 32 s | Plan (documento de ejecución) 2 min 32 s |
| `0b7ef36` | docs: separate static and run-time handle checks and add their criterion | T0, T1 | 2 min 33 s | Plan (documento de ejecución) 0 min 37 s, Código y tests 1 min 56 s |
| `af8e299` | feat(trees): add the tree API skeleton, its error classes and compile tests | T1 | 8 min 49 s | Código y tests 8 min 49 s |
| `c23822a` | feat(pager): bump the format version to 6 for the tree catalog | T1, T2 | 36 min 04 s | Código y tests 36 min 04 s |
| `d3e4a14` | feat(tx): add meta page slots for the layers above |  | 0 min 00 s |  |
| `862cff7` | feat(tree): add the krummi-tree crate with slotted B+ tree nodes |  | 0 min 00 s |  |
| `e33d9aa` | docs: record the first RDD review of the trees branch | T2 | 4 min 24 s | Código y tests 0 min 23 s, Revisión RDD 4 min 01 s |
| `c0f4019` | feat(tree): add the tree catalog | T3 | 32 min 51 s | Código y tests 32 min 51 s |
| `9e3ad1b` | feat(trees): implement the public tree API with handles, validation and tests | T3 | 0 min 16 s | Código y tests 0 min 16 s |
| `36b4c37` | docs: record the second RDD review of the trees branch | T3 | 2 min 50 s | Código y tests 0 min 23 s, Revisión RDD 2 min 27 s |
| `5c0ea31` | feat(trees): add the tree timing measurement of decision A15 |  | 0 min 00 s |  |
| `78007fa` | docs: add review-only instructions and skills for Codex | T4 | 56 min 12 s | Código y tests 56 min 12 s |
| `f1bc05c` | chore(review): configure delegated reviews and report policy | T4 | 24 min 28 s | Código y tests 24 min 28 s |
| `7d38767` | docs: never merge a run automatically; the user merges after their review | T4 | 8 min 28 s | Código y tests 8 min 28 s |
| `4735a47` | fix(tree): report inconsistent nodes and catalog entries as damage and free empty leaves | T4 | 1 h 06 min | Código y tests 1 h 06 min |
| `32d4d04` | test(trees): add damage, page-use, snapshot, crash, fault and AC10 acceptance tests |  | 0 min 00 s |  |
| `1138169` | docs: describe trees in the README and sweep them in CI | T4 | 1 min 53 s | Código y tests 0 min 17 s, Revisión RDD 1 min 36 s |
| `4ab9216` | fix(tree): free a node before its children so a looping child is damage | T4 | 3 min 32 s | Revisión RDD 3 min 32 s |
| `5ffa33a` | docs: record the Linux run and the independent verification of 5.1 | T4, T6 | 10 min 50 s | Verificación independiente 10 min 26 s, Revisión RDD 0 min 24 s |
| `af4116c` | docs: add the trees and key operations closing report | T6 | 4 min 15 s | Checks (fmt, clippy, suite de tests) 3 min 16 s, Revisión RDD 0 min 58 s, Cierre (informe y merge) 0 min 01 s |
| `4786f5a` | docs: add the Codex code review of the trees branch |  | 0 min 00 s |  |
| `62b1813` | docs: record the measuring equipment of A15 (Codex review R2) | R1 | 1 min 36 s | Correcciones de hallazgos 1 min 36 s |
| `06ec986` | fix(tree): reserve tree memory fallibly so running out of it fails with OutOfMemory | R1 | 20 min 54 s | Correcciones de hallazgos 20 min 54 s |
| — | después del último commit | R1 | 11 min 55 s | Checks (fmt, clippy, suite de tests) 9 min 24 s, Revisión RDD 2 min 17 s, Correcciones de hallazgos 0 min 14 s |
| **Total** | | | **5 h 01 min** | |

Las tareas T1–T4 fueron a ODD Workers en secuencia (el respaldo de contexto largo); su tiempo cuenta como código y
pruebas. Los 56 min y 24 min atribuidos a `78007fa` y `f1bc05c` son tiempo de T4 en curso: esos commits los hizo el
usuario en la rama mientras el worker de T4 trabajaba.

## Informe de cambios (`2702406..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 3.471 | 16 | 2.745 | 314 | 33 |
| `crates/krummi-tree` | 1.724 | 0 | 1.382 | 208 | 5 |
| `odd` | 495 | 0 | — | — | 3 |
| `crates/krummi-tx` | 354 | 18 | 264 | 64 | 5 |
| `crates/krummi-pager` | 197 | 14 | 161 | 29 | 7 |
| `AGENTS.md` | 200 | 0 | — | — | 1 |
| `docs` | 196 | 2 | — | — | 7 |
| `.agents` | 160 | 0 | — | — | 2 |
| `README.md` | 23 | 6 | — | — | 1 |
| `.codex` | 27 | 0 | 25 | 2 | 3 |
| `CLAUDE.md` | 13 | 4 | — | — | 1 |
| `.github` | 7 | 0 | 5 | 2 | 1 |
| `.claude` | 2 | 1 | — | — | 1 |
| `Cargo.toml` | 1 | 1 | 1 | 0 | 1 |
| **Total** | **6.870** | **62** | **4.583** | **619** | **71** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+9 −0)

`AGENTS.md`, `.agents`, `.codex`, 8 líneas de `CLAUDE.md` y 2 de `README.md` vienen de los commits del usuario
`78007fa` y `f1bc05c` (configuración de revisión con Codex, +418 −21), fuera del alcance de 5.1; `docs/reviews/` es
el informe de Codex y sus resoluciones.

## Resumen

Krummi guarda por primera vez datos de la aplicación: dentro de una transacción se abren árboles con nombre con los
modos de `FileMode` de .NET (abrir, crear nuevo, abrir o crear, crear o vaciar, vaciar), se comprueba si existen, se
listan, renombran y borran, y en cada árbol se guardan, añaden, leen y quitan entradas como en un `Dictionary`, con
el número de entradas al instante y lecturas sin copia. Por dentro, un crate nuevo `krummi-tree` implementa árboles
B+ de páginas con celdas sobre la API de páginas de las transacciones (las páginas conservan su número, así que no
hay copia de caminos) y un catálogo que es a su vez un árbol B+; la página de metadatos gana huecos para las capas
superiores y el formato pasa a `6`. Las aperturas son datos enteros: el compilador garantiza que las referencias
caducan en el siguiente cambio y que una apertura no vive más que su almacén, y en ejecución se comprueban solo la
apertura de otra transacción (uso incorrecto) y la de un árbol borrado (árbol no encontrado). Una página con
estructura incoherente se informa como daño.

## Decisiones tomadas sin preguntar

- Tipos de apertura estrictos: usar una apertura de lectura en una escritura, o al revés, no compila (estrecha la
  excepción de B1; L5).
- Política de solo fusiones de D4, con la corrección de la hoja vacía; separadores con la clave completa.
- Los cambios estructurales del catálogo se escriben al momento (Existe y Listar los ven); la raíz y el número de
  entradas de las operaciones por clave, al confirmar.
- Las cuatro revisiones RDD se elevaron a riesgo alto (ítem 1).
- Muestras de la suite: AC10 cada 397 bytes; fallos inyectados cada 13 puntos; las carreras completas, ignoradas y
  en un paso propio de la CI con 60 min de límite.
- El tamaño del almacén en la medición es el tamaño del archivo que da la información (P12).

## Pendiente (no bloqueante)

- `krummi-tx` `ac_f18_the_maximum_size_in_a_temporary_store` (y una vez `ac12_ac_t20_…`) falla de forma intermitente
  en ejecuciones completas; también en `main` (L9), así que no es de esta rama.
- El costo de validar cada nodo no se midió antes y después (RDD 3, R3-002); la lectura al azar cuesta 3,6–3,7 µs por
  entrada con la validación.
- El paso de CI que filtra por `ac_b18` pasaría en silencio si la prueba cambiara de nombre (RDD 4).
- Una carga en orden de clave deja las hojas medio llenas (división por la mitad); en una prueba de 5.000 entradas en
  debug el almacén cargado en orden ocupaba el doble. Una división al final para inserciones crecientes sería un
  cambio aparte, medido.
- Un puntero de una hoja hacia páginas de otro árbol, en una página falsificada con checksum válido, no se detecta sin
  leer las hojas (D4); corresponde a la verificación (módulo 12).
- La medición en Linux se tomó sobre `1138169`, antes del arreglo de `free_subtree` (`4ab9216`), que no afecta a
  las operaciones medidas.
- Las instantáneas `.stderr` de trybuild dependen de la versión del compilador, como las de módulos anteriores.
- Algunos barridos de memoria agotada tienen umbrales mínimos débiles, y la división no puede mostrar el rojo con la
  inyección, porque el Pager reserva antes su página (RDD 5, R3-weak-sweep-thresholds).
- La tabla de árboles de cada escritura es un `Vec` ordenado (no hay inserción falible en `BTreeMap`): abrir miles de
  árboles distintos en una escritura cuesta O(N²) (RDD 5, R4-001).
- Las inserciones en `BTreeMap`/`BTreeSet` de páginas modificadas y liberadas de `krummi-tx`, anteriores a esta rama,
  siguen siendo infalibles; fuera del alcance de R1.

**Riesgo:** ítem 1 (formato persistente `6`: huecos de metadatos, páginas de árbol y catálogo) e ítem 3 (API pública
y clases de error nuevas); cubiertos por las revisiones RDD y el verificador independiente.

## Limpieza manual

- La copia de WSL usada para Linux: `~/krummi-trees` en la distribución Ubuntu (1,3 GiB, con su `target`).
