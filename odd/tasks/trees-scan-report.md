# Recorrido en orden y búsqueda por prefijo (5.2) terminado: verificado, en su rama sin fusionar

Informe de cierre de `trees-scan` (2026-10-26). Enfoque de ejecución: ODD.
Fuente de producto: [trees-scan.prd.md](../../docs/prd/krummi/trees-scan.prd.md) (padre:
[trees.prd.md](../../docs/prd/krummi/trees.prd.md); hermano: [trees-keys.prd.md](../../docs/prd/krummi/trees-keys.prd.md)).
Documento de ejecución: [trees-scan.md](trees-scan.md).

Rama `feat/trees-scan`, sin fusionar en `main` (el usuario la revisa primero). Nada se ha subido. Último commit de
código: `dcf70fa`.

## Resultado

- **Especificación:** las 20 especificaciones S1–S20 (AC-R1–AC-R15, AC-B7 en su parte de 5.2, AC-B17 y AC-B18 con
  Quitar intervalo, AC10 y AC20 de la raíz) tienen prueba o comprobación y pasan; S15 se cumple con la sección
  Medición de este informe. `cargo test --workspace --no-fail-fast` en Windows sobre `dcf70fa`: 323 pasan, 1 falla,
  11 ignoradas; la que falla es `ac_f18_the_maximum_size_in_a_temporary_store` de `krummi-tx`, código que la rama no
  toca, y falla igual en la base (3 de 5 corridas de su binario en la rama y 3 de 5 en `0e204da`; 0 de 15 aislada;
  L14), la intermitencia conocida que ya anotó el informe de 5.1. En Linux (WSL 2 Ubuntu) pasa todo el workspace
  (L11). `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` sin avisos.
- **Aceptación:** `cargo test -p krummi --test tree_integrity -- --ignored` (cada byte alterado de un almacén con
  árboles, ahora leído también con recorridos desde los dos extremos, y las caídas en cada punto de la confirmación y
  del cierre, también con Quitar intervalo): 4 pasan, 7.639,7 s. Un fallo en cada operación de página: `ac_b18` pasa;
  `ac_r12` falló en una comprobación de la propia prueba tras un fallo inyectado (L9), corregida en `64eaebd` y
  vuelta a correr: pasa en los 598 puntos, 77,6 s. Comprobación cruzada con el motor de 5.1 (L8) y medición de R11
  en Windows y Linux (sección Medición).
- **Revisiones y verificación:** tres revisiones RDD aprobadas (sección Revisiones RDD); verificador independiente
  sin bloqueantes (L13), con una prueba diferencial aleatoria contra `BTreeMap` de siete semillas (hasta 85.000
  entradas quitadas por semilla, con confirmaciones, abortos y reaperturas) y sondas de T10, tamaños y precedencia.
- **Hallazgos corregidos por el camino:**
  - Revisión RDD 1: un recorrido acotado leía la hoja vecina antes de ver que su límite lo terminaba, así que una
    vecina dañada hacía fallar un recorrido que ya lo había devuelto todo; ahora se para en el separador del padre
    (`b07f248`, rojo observado: daño en la página 163 sin el arreglo).
  - Revisión RDD 2: la actualización del número de entradas tras un fallo de T11 podía desbordar; ahora satura, y la
    revisión RDD 3 añadió una aserción de depuración para que un descuadre en una escritura sana siga viéndose
    (`f67d59b`, `dcf70fa`).
  - El barrido completo de `ac_r12` y la corrida en Linux encontraron dos defectos de las pruebas, no del motor: una
    comprobación de orden aplicada a una escritura ya fallida (`64eaebd`) y una prueba de punteros de AC20
    intermitente por el traslado en segundo plano (10 fallos de 60 antes, 0 de 60 después; `1a69262`).
  - `scan_timing` fallaba con menos de 1.000 entradas (revisión RDD 3, `dcf70fa`).

## Cobertura de los criterios de aceptación

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-R1 | `ac_r1_the_output_examples`, `ac_r1_the_output_removals` | PASS |
| AC-R2 | `ac_r2_scans_of_a_tree_of_several_levels` (3.000 claves, límites en cada clave, entre vecinas y en los extremos) | PASS |
| AC-R3 | `ac_r3_prefixes` | PASS |
| AC-R4 | `ac_r4_empty_intervals` | PASS |
| AC-R5 | `ac_r5_sizes_of_scans`, `ac_r5_sizes_of_removals` | PASS |
| AC-R6 | `ac_r6_a_write_scan_sees_its_changes`, `ac_r6_other_handles_see_a_range_removal` | PASS |
| AC-R7 (AC-B17) | `ac_r7_scans_keep_their_snapshot_while_a_write_changes_the_tree` (la altura baja) | PASS |
| AC-R8 | `trees_compile.rs`: 7 casos que no compilan y 1 que compila | PASS |
| AC-R9 | `ac_r9_range_removals_across_pages_and_levels`, `range_removals_match_a_model` (40 rondas) | PASS |
| AC-R10 | `ac_r10_removing_every_entry_gives_the_pages_back` (1 y 10 intervalos) | PASS |
| AC-R11 | `ac_r11_scans_through_deleted_and_foreign_handles`, `ac_r11_handle_failures_win_over_damage`, `ac_r11_removals_through_deleted_and_foreign_handles` | PASS |
| AC-R12 (AC-B18) | `ac_r12_crashes_during_a_commit_with_range_removals` (+ cierre, ignorada), `ac_r12_a_failure_at_*_page_operation_of_range_removals_and_scans_applies_nothing` (muestra y cada punto), `ac_r12_a_range_removal_out_of_memory_follows_t11` | PASS |
| AC-R13 | `ac_r13_scans_and_damaged_pages`, `ac_r13_removals_and_damaged_pages`, `ac_r13_a_write_scan_out_of_memory_stops_and_follows_t11`, `ac_r13_a_read_scan_out_of_memory_stops_and_the_read_stays_usable`; entrada y salida a mitad de un recorrido en una escritura dentro del barrido T11 de `ac_r12` | PASS |
| AC-R14 | `ac_r14_a_store_of_5_1_scans_and_keeps_its_format_after_range_removals` y la comprobación cruzada con el motor de 5.1 (L8) | PASS |
| AC-R15 | sección Medición de este informe | PASS |
| AC-B7 (5.2) | `ac_b7_keys_come_out_in_unsigned_byte_order` | PASS |
| AC10 (raíz) | `ac10_sample_of_altered_bytes_with_trees` y `ac10_every_altered_byte_with_trees` con recorridos | PASS |
| AC20 (raíz) | `ac20_scans_return_references_into_the_engine_s_memory`, `ui/trees/scan_entry_cannot_outlive_its_transaction.rs` | PASS |

Una lectura no puede fallar con entrada y salida a mitad de un recorrido (lee páginas proyectadas en memoria), así
que esa parte de AC-R13 se comprueba en una escritura (L7).

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `0e204da..51be749` | resilience, readability, reliability | aprobada | R4-eager-next-leaf-read (WARNING): lectura de la hoja vecina; corregido en `b07f248`. R2-misplaced-partition-doc (WARNING) y R3-002 (SUGGESTION): comentario de `child_index` desplazado; corregido. R2-design-doc-drift (SUGGESTION): D1 y D2 desactualizados; corregido. R2-shadowed-open-helper (SUGGESTION): renombrado. R3-001 (WARNING): faltaban memoria agotada y E/S a mitad de recorrido; cubierto en T3. R2-mirrored-cursor-logic (SUGGESTION): sin cambio, cada extremo se lee directo. | `review-b3f0715ef79299e4` |
| `51be749..1ddd9ca` | risk, reliability, resilience | aprobada (elevada a alto riesgo con el punto 1) | R4-partial-range-removal-on-error (WARNING): estado parcial tras un fallo de T11; aceptado como especificado y blindado (contador saturado). R3-underfull-after-boundary-merge (WARNING): descartado con evidencia, 5.1 no garantiza llenado mínimo. R3-no-structural-check-after-range-removal (SUGGESTION): sin cambio, los recorridos completos comprueban cada nodo. | `review-7dbcd8773193aef6` |
| `1ddd9ca..1a69262` | reliability, resilience, readability | aprobada | Contador saturado que ocultaría errores (dos WARNING y una SUGGESTION): aserción de depuración en `dcf70fa`. `scan_timing` con menos de 1.000 entradas (dos WARNING): corregido. Longitud de clave literal (SUGGESTION): constante `timing::KEY`. Comprobación de orden debilitada tras un fallo (SUGGESTION): sin cambio, es lo que fija el PRD. | `review-fcafd4847b9889fb` |

Evaluaciones sin revisión: `f67d59b` (T3) y `d6f2263` (T4) salieron `medium`, `under_budget`, y entraron en la
revisión del último tramo. Tras `dcf70fa` no quedan commits de código sin revisar; el de este informe es solo
documentación.

## Reutilización

- Se reutilizaron las comprobaciones de nodos de 5.1 (`node_at`, nivel y formato con daño y número de página), la
  reserva falible de memoria (`reserve`, `copy`), los pasos de fusión y descarte de hijos (`merge_child`,
  `drop_child`) y la liberación de subárboles, que ahora puede contar entradas (`free_subtree(.., count)`).
- Se extrajo `fix_root` de `update`: lo comparten las operaciones por clave y Quitar intervalo. Se añadió
  `node::partition`, la búsqueda por predicado con la que bajan el recorrido y Quitar intervalo.
- Las pruebas reutilizan sin copiarlos los helpers de `trees.rs`, movidos a `tests/support/mod.rs`; el barrido T11 de
  5.1 ahora recibe la escritura y las clases de fallo (`t11_sweep_of`); el barrido de caídas, la inyección de memoria
  agotada y el barrido AC10 se ampliaron en sus propios archivos.
- `tree_timing` y `scan_timing` comparten las claves, los órdenes, la carga y las filas en `examples/timing/mod.rs`.
- Los extremos delantero y trasero del recorrido quedan como funciones espejo a propósito: una abstracción de sentido
  escondería la aritmética de índices, que es justo lo que cambia (revisión RDD 1).

## Medición (R11)

Release, el almacén cargado al azar de A15 (1.000.000 de claves aleatorias de 16 bytes con semilla fija, valores de
100 bytes, escrituras durables de 1.000 entradas); cada parte sobre su propia copia; `cargo run --release -p krummi
--example scan_timing`.

| Operación | Windows (s) | Windows (µs por entrada) | Linux, WSL 2 ext4 (s) | Linux (µs por entrada) |
|---|---|---|---|---|
| Cargar al azar (código de 5.1) | 84,89 | 84,89 | 35,97 | 35,97 |
| Recorrer todo hacia delante (1.000.000) | 0,38 | 0,38 | 0,05 | 0,05 |
| Recorrer todo hacia atrás (1.000.000) | 0,28 | 0,28 | 0,05 | 0,05 |
| 10.000 recorridos de 100 desde claves al azar (999.830 entradas) | 0,56 | 0,56 | 0,09 | 0,09 |
| Quitar todo con 1.000 intervalos, una escritura durable cada uno | 1,94 | 1,94 | 8,29 | 8,29 |

Tamaño del almacén tras la carga y tras quitarlo todo, en los dos sistemas: 268.435.456 bytes (el archivo no
encoge; las páginas quedan libres).

La carga de Windows tardó tres veces los 26,79 s del informe de 5.1. Seguidas en la misma máquina, `tree_timing`
tardó 90,40 s con esta rama y 94,44 s con el código de 5.1 (`0e204da` exportado): es el estado de la máquina ese
día, no una regresión (L10).

Equipo (los dos entornos en la misma máquina; consultado el 2026-10-26 con `Get-CimInstance`, `Get-PhysicalDisk`,
`uname`, `lscpu` y `df`):

| Entorno | Sistema operativo | Procesador | Disco |
|---|---|---|---|
| Windows | Windows 11 Pro 10.0.26200 | 13th Gen Intel Core i9-13980HX (32 hilos) | NVMe Samsung MZVL22T0HBLB-00B00 (SSD) |
| Linux | WSL 2 Ubuntu, kernel 5.15.133.1-microsoft-standard-WSL2, ext4 | el mismo (`lscpu`: 32 CPU) | disco virtual ext4 de WSL 2 (`/dev/sde`, 1.007 GB) en el mismo NVMe |

## Tiempo

- Estimación: 3 h – 4 h 30 min. Arranque real: 2026-10-26 09:38:44 +0000 (`date`). Fin: 2026-10-26 13:01:29 +0000
  (fin del registro de fases).
- **Duración activa desde el arranque: 3 h 22 min** según el registro de fases, dentro de la estimación. Ese total
  incluye la espera a los barridos de aceptación en segundo plano: desde el commit de documentación (`8160de4`,
  10:19:24) hasta el fin de los barridos (12:23:18) no hubo trabajo en primer plano, y el registro lo cuenta como
  «Documentación» porque no se marcó una fase de espera. Sin esas 2 h 04 min, el trabajo fue de 1 h 18 min, el 43 %
  del mínimo estimado; la corrida de 5.1 de los mismos barridos tardó 6.477,5 s, así que su duración era previsible.
- Esperas por el usuario: ninguna dentro de la ejecución (la estimación se confirmó antes del arranque, tras una
  espera a que otra sesión terminara la automatización de las revisiones de Codex, fuera del activo).

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 4 min 11 s | 2 % | 1 |
| Código y tests | 33 min 00 s | 16 % | 5 |
| Checks (fmt, clippy, suite de tests) | 21 min 50 s | 11 % | 2 |
| Aceptación | 1 min 50 s | 1 % | 1 |
| Verificación independiente | 1 min 50 s | 1 % | 1 |
| Revisión RDD | 2 min 30 s | 1 % | 2 |
| Correcciones de hallazgos | 11 min 01 s | 5 % | 2 |
| Documentación | 2 h 05 min | 62 % | 1 |
| Cierre (informe y merge) | 0 min 28 s | 0 % | 1 |
| **Total activo** | **3 h 22 min** | **100 %** | **16** |

- Inicio: 2026-10-26 09:38:58 +00:00. Fin: 2026-10-26 13:01:29 +00:00.
- Arranque confirmado: 09:38:58. Activo desde el arranque: **3 h 22 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD review 1 lenses (T1) | 09:57:05 | 09:59:11 | 2 min 06 s |
| Aceptación | full sweeps: tree_integrity --ignored, lib ac_ --ignored | 10:13:47 | 12:23:18 | 2 h 09 min |
| Revisión RDD | RDD review 3 lenses (last slice) | 12:43:37 | 12:44:33 | 0 min 56 s |
| **Aceptación (total)** | | | | **2 h 09 min** |
| **Revisión RDD (total)** | | | | **3 min 02 s** |

Tarea en segundo plano sin cerrar, no contada: Revisión RDD / RDD review 2 lenses (fix + T2) desde 10:07:37.

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `51be749` | feat(trees): scan a tree in key order from both ends, by interval and prefix | T0 exploration, T1 | 16 min 22 s | Exploración y mapa 4 min 11 s, Código y tests 12 min 11 s |
| `b07f248` | fix(trees): stop a scan at the parent's separator before reading the next leaf | T1, T1 assess, T2 | 6 min 23 s | Código y tests 4 min 46 s, Revisión RDD 1 min 37 s |
| `1ddd9ca` | feat(trees): remove every entry of an interval or prefix at once | T2 | 4 min 59 s | Código y tests 4 min 59 s |
| `f67d59b` | test(trees): crashes, failures and altered bytes with range removals and scans | T2, T2 assess, T3 | 6 min 55 s | Código y tests 6 min 02 s, Revisión RDD 0 min 53 s |
| `d6f2263` | test(trees): a 5.1 store scans and keeps its format after range removals | T3, T4 | 3 min 11 s | Código y tests 3 min 11 s |
| `b120a64` | feat(trees): measure scans and range removals of the 5.1 store (R11) | T4, T5 | 1 min 43 s | Código y tests 1 min 43 s |
| `8160de4` | docs: describe scans and range removals, and sweep them in CI | T5, T6 | 0 min 53 s | Código y tests 0 min 08 s, Documentación 0 min 45 s |
| `64eaebd` | test(trees): check a scan's order only when the write had no earlier failure | T6, ac_r12 full sweep | 2 h 06 min | Correcciones de hallazgos 1 min 39 s, Documentación 2 h 05 min |
| `1a69262` | test(trees): keep the version out of the data file in the scan AC20 test | ac_r12 full sweep, R11 measurement Windows, Linux tests and measurement (WSL) | 16 min 33 s | Checks (fmt, clippy, suite de tests) 14 min 31 s, Aceptación 1 min 50 s, Correcciones de hallazgos 0 min 12 s |
| `dcf70fa` | fix(trees): assert a healthy removal never removes more than the count | Linux tests and measurement (WSL), checks and independent verifier, RDD review 3 advisories | 3 min 14 s | Checks (fmt, clippy, suite de tests) 0 min 29 s, Verificación independiente 1 min 50 s, Correcciones de hallazgos 0 min 55 s |
| — | después del último commit | RDD review 3 advisories, final checks, closing report | 15 min 33 s | Checks (fmt, clippy, suite de tests) 6 min 50 s, Correcciones de hallazgos 8 min 15 s, Cierre (informe y merge) 0 min 28 s |
| **Total** | | | **3 h 22 min** | |

El tramo de «Documentación» es casi todo la espera a la aceptación descrita arriba. El trabajo efectivo se repartió
entre el código y sus pruebas (33 min), los chequeos (incluidas la corrida en Linux y las repeticiones para aislar
dos pruebas intermitentes, 22 min) y las correcciones de hallazgos (11 min). La tarea en segundo plano de la
revisión RDD 2 no tiene fin registrado: sus dos primeros intentos fallaron en el script del agente antes de empezar
(L6), y el tercero se lanzó sin `bg-end`.

## Informe de cambios (`0e204da..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 2.564 | 290 | 1.974 | 244 | 25 |
| `crates/krummi-tree` | 652 | 10 | 527 | 86 | 3 |
| `odd` | 210 | 0 | — | — | 2 |
| `README.md` | 15 | 5 | — | — | 1 |
| `.github` | 4 | 3 | 2 | 2 | 1 |
| `crates/krummi-pager` | 3 | 0 | 2 | 1 | 1 |
| `.gitattributes` | 1 | 0 | — | — | 1 |
| **Total** | **3.449** | **308** | **2.505** | **333** | **34** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `crates/krummi/tests/data/store-5.1/krummi.data` (+- −-)

Previsión: la tarea de cada unidad seguía el criterio orientativo de unas 400 líneas; el total lo engordan las
pruebas (`crates/krummi/tests`) y el almacén de prueba de 5.1, binario y listado aparte.

## Resumen

Una aplicación puede recorrer un árbol en orden de claves, entre dos límites (incluidos, excluidos o abiertos) y con
un prefijo opcional, sacando entradas por el extremo delantero, por el trasero o alternando, sin copiar claves ni
valores. Puede además quitar de una vez todas las entradas de un intervalo o de un prefijo y saber cuántas quitó. La
API sigue a `BTreeMap::range` de Rust (`range`, `range_prefix`, `remove_range`, `remove_range_prefix`, con límites
`RangeBounds<&[u8]>`), salvo que unos límites invertidos dan un intervalo vacío en vez de detener el programa. Cada
extremo del recorrido guarda su camino desde la raíz y no lee ninguna página hasta usarse; el prefijo se integra en
los dos predicados monótonos que prueban una clave contra los límites, y un intervalo en el que ninguna clave válida
puede caer se decide sin leer páginas. Quitar intervalo baja una sola vez: libera los subárboles enteros del
intervalo y arregla los hijos de los límites con los pasos de fusión de 5.1. El formato no cambia (sigue en `6`) y
el motor de 5.1 lee y modifica lo que deja 5.2.

## Valores por defecto elegidos sin preguntar

- La API toma `RangeBounds<&[u8]>` y no `RangeBounds<[u8]>`, porque la biblioteca estándar no implementa la segunda
  para `a..b` con porciones (D1).
- La prueba de intervalo vacío de R3 es exacta: también es vacío, sin leer páginas, un intervalo como `(a, a\0)`
  (entre una clave y su sucesora no hay ninguna), con o sin prefijo (D2).
- Un recorrido en una escritura sigue T11 ante cualquier fallo, como las demás operaciones de un árbol.
- Los extremos delantero y trasero se paran en el separador del padre antes de leer la hoja siguiente (revisión RDD
  1).
- El almacén de prueba de 5.1 se guarda en el repositorio (512 KiB) y la comprobación con el motor de 5.1 es de la
  ejecución, no una prueba guardada (como fija AC-R14).
- El paso de aceptación de CI corre también el barrido T11 completo de `ac_r12`.

## Pendiente (no bloqueante)

- `ac_f18_the_maximum_size_in_a_temporary_store` (`krummi-tx`) falla de forma intermitente al correr su binario
  completo, también en la base `0e204da`; no lo causa este cambio y queda para un arreglo aparte (L14).
- Tras fusionar los dos hijos de los límites, un nodo puede quedar por debajo de un cuarto de llenado, como en 5.1;
  no es un invariante del formato (revisión RDD 2).
- Una revisión de Codex de la rama, si el usuario la quiere antes de fusionar.
- La fusión en `main` y el push son decisiones del usuario.

**Riesgo:** punto 1 (Quitar intervalo borra entradas guardadas y reorganiza páginas; cubierto por la prueba contra un
modelo, los barridos de caídas, de fallos de T11 y de memoria agotada, la comprobación cruzada con 5.1 y el
verificador independiente) y punto 5 (una línea de CI que añade un filtro de pruebas).
