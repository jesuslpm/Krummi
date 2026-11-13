# Lecturas de filas de las tablas terminadas: Krummi cumple los 29 umbrales en Windows y Linux, rama verificada

Informe de cierre de `table-row-reads` (2026-11-12, actualizado tras la segunda ronda, T6–T8, y la tercera, T9–T13). Enfoque de ejecución: ODD.
Fuente de producto: [tables-row-reads.prd.md](../../../docs/prd/krummi/tables/tables-row-reads.prd.md) (desde L20; antes, una
optimización sin PRD, anotación del 2026-11-04 a K13 de [benchmark.prd.md](../../../docs/prd/krummi/benchmark/benchmark.prd.md)).
Documento de ejecución: [table-row-reads.md](table-row-reads.md).

Rama `feat/table-row-reads`, creada desde `feat/tables-schema` en `bb8acca`; incluye también el banco solo con Krummi
y Voron, con su propio [informe](../benchmark/benchmark-krummi-voron-report.md). Sin publicar; el usuario autorizó fusionarla en
`main` al final de la tercera ronda («Ya está revisada»).

## Resultado

- **Especificación:** las lecturas por la clave y los recorridos ya no comprueban el contenido de la fila (L1); una
  fila mal formada se devuelve y leer su columna da otro valor o un pánico normal del hilo (L2); actualizar e insertar
  o actualizar siguen comprobando la fila que reemplazan (L3). Antes, las optimizaciones sin cambio observable: filas
  perezosas que guardan sus bytes, lecturas tipadas, un plan por columna, los lectores guardados en la transacción
  (ahora en un trie de hash) y `next_row`, que presta la fila. Suite en `bb1408b`: `cargo test --workspace` 808 pasan, 0 fallan,
  23 ignoradas; `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` limpios; los 10
  tests de filas mal formadas pasan también con `--release`.
- **Segunda ronda (T6–T8, L33–L40):** W28, la única carga que seguía fallando en Windows, pasa de 0,75 a **0,88**
  (y de 0,91 a 1,00 en Linux). Lo consiguen dos cambios: una actualización que solo da columnas de ancho fijo
  reescribe sus huecos en la fila guardada en lugar de reconstruirla (`Reader::patch`), y la fila del mismo tamaño se
  escribe donde la encontró la lectura, sin bajar otra vez por el árbol (`krummi_tree::get_located` y `overwrite`).
  La diferencia del journal por bloques (T8) no ganó nada medible y restaba un 1–4 % en W28: se revirtió. Con
  `bb1408b`, las siete cargas de tablas cumplen K7 en los dos sistemas (L40).
- **Tercera ronda (T9–T13, L42–L51):** W25, la carga con menos margen en Windows, pasa de 0,82 a **1,05** (y de 1,11
  a 1,29 en Linux), y W23 de 1,08 a 1,39. Lo consiguen dos cambios: una fila que da todas las columnas en el orden del
  esquema se comprueba sin el mapa de nombres (T10, +7 % en W25), y una inserción más allá de la mayor clave de la
  tabla se añade al final de la última hoja del árbol de filas, que la propia inserción anterior dejó localizada,
  cuando el árbol solo cambiaría ahí (T11, `krummi_tree::append_last`, +23 %). La caída del 3 % de W23 no existe
  (T9: nueve binarios dentro de su dispersión). Ejecuciones completas finales con `--solo-krummi-voron`: **29 de 29
  umbrales** en Windows (fijado a los núcleos de rendimiento) y en Linux (L50). T13 corrigió un test intermitente
  anterior a esta ejecución (L51). Suite final en `d92a88f`: `cargo test --workspace` 829 pasan, 0 fallan, 34
  ignoradas; fmt y clippy limpios. Tres revisiones RDD más, todas aprobadas, y Codex en tres rondas, aprobada (L48,
  L49, L53).
- **Aceptación:** cargas de tablas W23–W29 de Krummi y Voron con `d6a88b7` (decisión del usuario, L30: solo las
  cargas de tablas). W27 pasa de 0,15 a **0,89** en Windows y de 0,19 a **0,97** en Linux; W26 de 0,75 a **0,96** y de
  0,88 a **1,13**. Las sumas de datos de Voron coinciden con las de Krummi en todas las cargas. Detalle en L31.
- **Revisiones y verificación:** once revisiones RDD, todas aprobadas (sección siguiente); en la segunda ronda,
  Codex revisó `d6a88b7..33f7f7b` (dos rondas, aprobada tras corregir R1); pasan las pruebas de aceptación ignoradas
  del journal (`page_diffs`, 184 s) y de la recuperación (`recovery`, cada byte alterado, 4.076 s); revisión cruzada de Codex
  del PRD (tres rondas, aprobada) y del código en dos cadenas: `bb8acca..7cf73ca` (dos rondas, aprobada) y la de toda
  la ejecución, `bb8acca..3ecc294` (tres rondas, aprobada). Informes y resoluciones en
  [docs/reviews/table-row-reads/](../../../docs/reviews/table-row-reads/) y
  [docs/reviews/tables-row-reads/](../../../docs/reviews/tables-row-reads/).
- **Hallazgos corregidos por el camino:**
  - Codex R1 (mayor): leer las columnas de una clave compuesta recorría su prefijo una vez por columna; corregido en
    `9941b62`.
  - Codex R2 (mayor): una lectura tipada de una fila de una versión antigua decodificaba su texto, así que
    `text_bytes` daba pánico con un texto que no es UTF-8; corregido en `2bc4fe4`.
  - Codex R3 (mayor): `Readers::keep` recorría una lista de todas las tablas leídas (4.066 ns por lectura en la última
    de 1.000 tablas); un trie de hash en `bb496a1` lo deja en 196 ns.
  - Codex R4 (menor): un test nuevo no pasaba clippy. El comando de checks lo ocultaba porque pasaba clippy por `tail`;
    desde entonces los checks se ejecutan con `pipefail` (L24).
  - Codex R1 de la segunda ronda (mayor): `Reader::patch` buscaba cada columna dada en la lista de columnas de la
    clave, cuadrático en una clave compuesta ancha (2,32 veces el camino general con 1.022 columnas); corregido en
    `c873418` (0,76 después).
  - Medición: las cargas sueltas sin el plan de energía inflaban las razones (L11), y Windows mueve los procesos a los
    núcleos de eficiencia si no se fijan (L13). La primera tanda de tablas en Windows se descartó por eso (L31).

## Cobertura de los criterios de aceptación

| Criterio | Test o comprobación | Resultado |
|---|---|---|
| AC-L1 | `ac_l1_a_text_that_is_not_utf8_is_returned_and_panics_only_when_read_as_text` y `ac_l1_an_older_row_with_a_text_that_is_not_utf8_reads_it_as_bytes` (`table::malformed_tests`), en depuración y en optimizado | PASS |
| AC-L2 | `ac_l2_a_malformed_value_or_offset_is_returned`, `ac_l2_a_row_under_a_malformed_key_is_returned_in_key_byte_order` | PASS |
| AC-L3 | `ac_l3_an_unknown_version_header_or_fixed_part_is_damage_of_the_row_page`, `ac_l3_a_version_missing_from_the_history_is_damage_of_the_history_page`, también con `next_row` | PASS (pasaban antes del cambio: «como hoy») |
| AC-L4 | `ac_l4_update_and_upsert_still_check_the_row_they_replace` | PASS |
| AC-L5 | `ac_l5_a_panic_captured_around_the_read_leaves_the_transaction_intact`, `ac_l5_a_panic_through_the_transaction_discards_it` | PASS |
| AC-L6 | los tests AC19 de `table_damage.rs`, sin cambios | PASS |
| AC-L7 | ninguna línea de la codificación de filas y claves cambió desde `bb8acca` (L20); Codex lo confirmó estáticamente | PASS (estático: no se abrió un almacén escrito por 7.1) |
| AC-L8 | anotaciones de E19, G6 y K7 en los originales y sus traducciones (`15bf568`; la línea en blanco de E19 en `3ecc294`) | PASS |
| S6 (W28) | `an_update_stores_the_bytes_of_the_whole_merged_row`, `an_update_with_an_invalid_fixed_width_value_fails_before_the_patch`, `an_update_under_a_wide_compound_key_stores_the_merged_row` (`table::update_tests`), `a_located_value_of_the_same_length_is_overwritten_in_place` (`krummi-tree`); tanda final de tablas (L40) | PASS: W28 0,88 en Windows y 1,00 en Linux |
| AC-L9 | cargas de tablas en Windows (fijadas) y en Linux (L31; repetidas con `bb1408b`, L40); ejecuciones completas con `--solo-krummi-voron` (L50) | W26 y W27 cumplen en los dos; las bajadas de más de 0,05 (W23 en Windows, W29 en Linux) quedan dentro del margen con cargas sueltas; las ejecuciones completas finales, W1–W29, cumplen 29 de 29 en los dos sistemas |
| T10, T11 (W25) | `a_row_in_schema_order_fails_and_is_stored_as_in_any_other_order`, `ordered_inserts_mixed_with_other_changes_keep_the_rows_in_order`, `ordered_inserts_add_at_the_end_of_the_last_leaf`, `an_insert_elsewhere_does_not_need_a_damaged_last_leaf` y las equivalencias byte a byte del árbol (`appending_to_the_last_leaf_changes_the_tree_as_adding_does` y siguientes); verificador independiente (L46) | PASS: W25 1,05 en Windows y 1,29 en Linux |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `bb8acca..e036803` | risk, readability, reliability | aprobada | R2-001, R2-002, R2-003, R3-key-skip-equivalence (WARNING), R3-checked-value-panic-contract: corregidos en `ea08579` | `review-7d3a174b3d64e617` |
| `e036803..ae39594` | readability, reliability | aprobada | R2-001, R2-002, R3-key-starts-u16-panic (SUGGESTION): corregidos en `8091626` | `review-22562fd92a7cefd8` |
| `ae39594..a02838b` | readability, reliability | aprobada | R2-001 descartado con evidencia; R3-key-starts-overflow-untested aceptado tal cual | `review-d0fe117898c0870c` |
| `a02838b..5dcfa29` | risk, resilience, readability, reliability | aprobada | R1-001 y R4-reader-cache-history-key (WARNING) descartados (cada tabla crea su historial), con un test nuevo; R4-next-row-fast-path-version y R3-lent-fast-path (WARNING) descartados (`RowLayout::row` rechaza otra versión), con tests nuevos; R2-annotation-merged-into-list (WARNING), R2-duplicated-array-helper y R2-slot-name-collision: corregidos en `3ecc294`; R3-readers-linear-growth aceptado (después lo corrigió Codex R3) | `review-9378929f5e8f62c8` |
| `5dcfa29..9ea1b59` | risk, readability, reliability | aprobada | cinco SUGGESTION (ancho del dígito del trie, índice sin nombre, encabezados repetidos, texto antiguo en escritura, test concurrente): corregidos en `5174433` | `review-cb691290c004b232` |
| `9ea1b59..5174433` | reliability, readability | aprobada | nombre del test concurrente (WARNING, en las dos lentes), guarda de `DIGIT` y constante con nombre: corregidos en `d6a88b7`; R2-missing-resolution-context (WARNING) descartado (los hallazgos RDD van en el registro, L26) | `review-056aee61ef36b0bd` |
| `5174433..d6a88b7` | readability, reliability | aprobada | R2-001 (SUGGESTION: una variable sombrea un cierre en un test) aceptado tal cual | `review-389fd4a1315c29ca` |
| `d6a88b7..5bd0f92` | risk, reliability, readability | aprobada | el nulo en una columna sin nulos ante `Slot::write` (WARNING, tres lentes) descartado: `Reader::check` lo rechaza antes, y lo fija un test nuevo; `apply_rows` omitido tras la escritura en el sitio (WARNING): comentado y probado leyendo tras confirmar; T6–T7 ausentes del documento confirmado (WARNING): confirmado; sugerencias (hoja en `overwrite`, `FIXED_BYTES`, `upsert`, raíz del test del árbol) corregidas en `33f7f7b` | `review-28c95d242b64ea75` |
| `33f7f7b..b5f82bd` | reliability, readability | aprobada | nombres `key` e `in_key` confundibles (WARNING) y sugerencias de documentación y del test: corregidos en `bb1408b`; un número de línea del informe de Codex, aceptado (los informes no se editan) | `review-f239fe4be7f70170` |
| `b5f82bd..bb1408b` | readability | aprobada | ninguno | `review-4a114bbe6859cbf3` |
| `18e38f7..a71297b` | risk, reliability, readability (escalada, item 1) | aprobada | R2-001 (WARNING, `append_last` repetía a mano la regla de fusión de `merge_child`): los dos llaman a `merge_fits`; R1-001 (atar la hoja guardada a la raíz), R3 (el test de la tabla no mostraba que el camino rápido corría) y R2-002 (`_folder`): corregidos en `2c7bb1d` | `review-c0f10da564cef53f` |
| `2c7bb1d..caa86c3` | risk, reliability (escalada, item 1) | aprobada | R3-001 (SUGGESTION, ningún test pasaba por la rama que conserva la hoja cuando la fusión no cabe): test en `29d0694` | `review-8a220532b1fd6830` |
| `caa86c3..3641ace` (último tramo) | readability, reliability | aprobada | R2-001 (tamaños sin explicar en el test del árbol) y R3-001 (ese test no validaba el árbol): corregidos en `abf5555`; R2-002 (etiqueta de la fase de checks): corregida; R3-002 (pausar la transferencia en el test e11): no aplicada, los tests de integración no llegan a los ganchos del motor (L52) | `review-8123cb28685cc534` |

Las evaluaciones de cada tramo dijeron `medium`, `under_budget`; todos los tramos se revisaron igualmente. En la
tercera ronda se escalaron al item 1 (T11 cambia cómo se escriben las páginas del árbol de filas). Además, un
verificador independiente (L46) encontró un defecto de T11 (`follow_last` leía el borde derecho del árbol, contra E19),
corregido en `caa86c3` con un test RED/GREEN, y Codex revisó `18e38f7..29d0694` en dos rondas: un hallazgo menor de
documentación, corregido en `ce11e62`, y aprobada
([informe](../../../docs/reviews/table-row-reads/2026-11-12-code-03.md),
[seguimiento](../../../docs/reviews/table-row-reads/2026-11-12-code-04.md),
[resoluciones](../../../docs/reviews/table-row-reads/2026-11-12-code-03-resolutions.md)).

## Tiempo

- Estimaciones: 6 h – 10 h para la optimización sin PRD (L1); 1–2 h más unas 4 h de ejecuciones completas para
  implementar el PRD (L20); 3 h – 5 h para la segunda ronda, T6 y T7 (L33), a la que se añadió T8; 6 h – 10 h para la
  tanda «Hazlo todo» (L42: el banco solo con Krummi y Voron, W25, W23, las ejecuciones completas, la limpieza y la
  fusión). Inicio real: 2026-11-11 12:05:51 +0000. Fin: 2026-11-12 23:50:41 +0000.
- **Duración activa: 15 h 11 min** en total. La primera parte (optimización, PRD e implementación del PRD) llevó
  7 h 53 min; la segunda ronda, 3 h 48 min; la **tercera ronda**, de 20:20:12 a 23:50:41, **3 h 30 min**. Con las
  3 h 06 min del banco solo con Krummi y Voron ([informe](../benchmark/benchmark-krummi-voron-report.md)), la tanda «Hazlo todo»
  llevó **6 h 36 min**, en el borde bajo de su estimación de 6–10 h. Se la llevaron sobre todo las dos ejecuciones
  completas finales (1 h 07 min en segundo plano, contadas como aceptación) y las revisiones; los checks finales y la
  corrección del test intermitente (T13) suman la fase de checks posterior a `6ffad00`, en la que también entran,
  sin marca propia, la revisión RDD del último tramo, la ronda 3 de Codex y la limpieza.
- Esperas por el usuario: las del registro de fases (abajo); la mayor, 8 h 06 min, es el reinicio de Windows de esa
  noche, que cortó la ejecución completa (L29). Entre rondas hay pausas de 1 h 11 min y 8 h 07 min.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 12 min 07 s | 1 % | 3 |
| Plan (documento de ejecución) | 36 min 32 s | 4 % | 1 |
| Código y tests | 3 h 56 min | 26 % | 7 |
| Checks (fmt, clippy, suite de tests) | 2 h 17 min | 15 % | 4 |
| Aceptación | 3 h 38 min | 24 % | 7 |
| Revisión RDD | 4 h 23 min | 29 % | 8 |
| Correcciones de hallazgos | 7 min 24 s | 1 % | 1 |
| Documentación | 0 min 07 s | 0 % | 1 |
| Cierre (informe y merge) | 0 min 07 s | 0 % | 3 |
| **Total activo** | **15 h 11 min** | **100 %** | **35** |

- Inicio: 2026-11-11 12:05:51 +00:00. Fin: 2026-11-12 23:50:41 +00:00.
- Arranque confirmado: 08:24:26. Activo desde el arranque: **15 h 11 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 11 h 13 min.
  - 12:45:50–13:17:38 (31 min 48 s): W27 decision
  - 14:41:09–14:58:58 (17 min 49 s): W27 and W28 scope
  - 16:53:01–17:34:30 (41 min 29 s): W27 how far
  - 18:10:43–18:28:35 (17 min 52 s): PRD malformed rows
  - 19:05:07–20:23:30 (1 h 18 min): PRD authorization
  - 22:28:52–06:35:29 (8 h 06 min): Windows restart
- Pausas entre sesiones (fuera del activo): 9 h 19 min.
  - 2026-11-12 07:13:14 +00:00 – 2026-11-12 08:24:26 +00:00 (1 h 11 min)
  - 2026-11-12 12:12:37 +00:00 – 2026-11-12 20:20:12 +00:00 (8 h 07 min)
  - 2026-11-12 23:04:47 +00:00 – 2026-11-12 23:05:18 +00:00 (0 min 31 s)

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | codex-R1 | 13:26:52 | 13:36:40 | 9 min 48 s |
| Revisión RDD | codex-R2 | 13:53:37 | 13:59:25 | 5 min 48 s |
| Aceptación | complete windows | 14:10:12 | 14:15:49 | 5 min 37 s |
| Aceptación | complete wsl | 14:15:49 | 14:16:22 | 0 min 33 s |
| Revisión RDD | codex-PRD-R1 | 18:34:59 | 18:42:47 | 7 min 48 s |
| Revisión RDD | codex-PRD-R2 | 18:51:36 | 18:57:08 | 5 min 32 s |
| Revisión RDD | codex-PRD-R3 | 18:57:26 | 19:01:40 | 4 min 14 s |
| Revisión RDD | rdd-lenses | 20:45:49 | 20:46:56 | 1 min 07 s |
| Aceptación | single loads W26 W27 | 20:59:28 | 21:01:27 | 1 min 59 s |
| Revisión RDD | codex-code-R1 | 21:01:49 | 21:11:14 | 9 min 25 s |
| Revisión RDD | codex-code-R2 | 21:21:11 | 21:28:22 | 7 min 11 s |
| Revisión RDD | codex-code-R3 | 21:28:57 | 21:31:38 | 2 min 41 s |
| Revisión RDD | rdd-last-slice | 21:32:22 | 21:33:04 | 0 min 42 s |
| Revisión RDD | rdd-final | 21:40:59 | 21:41:39 | 0 min 40 s |
| Revisión RDD | rdd-final-2 | 21:43:17 | 21:43:37 | 0 min 20 s |
| Aceptación | complete windows | 21:44:25 | 22:28:52 | 44 min 27 s |
| Aceptación | tables windows | 06:35:43 | 06:42:19 | 6 min 36 s |
| Aceptación | tables wsl | 06:42:19 | 06:52:41 | 10 min 22 s |
| Aceptación | tables windows pinned | 06:53:28 | 06:59:24 | 5 min 56 s |
| Aceptación | w29 wsl | 06:59:24 | 06:59:28 | 0 min 04 s |
| Aceptación | w29 wsl | 06:59:52 | 07:07:58 | 8 min 06 s |
| Aceptación | w23 w25 single | 07:11:03 | 07:12:45 | 1 min 42 s |
| Aceptación | single loads T6-T8 | 09:53:51 | 10:05:47 | 11 min 56 s |
| Aceptación | W28 t67 t8 | 10:06:10 | 10:09:05 | 2 min 55 s |
| Revisión RDD | rdd-T6-T7 | 10:45:34 | 10:47:15 | 1 min 41 s |
| Revisión RDD | codex-T6-R1 | 10:58:01 | 11:07:19 | 9 min 18 s |
| Revisión RDD | codex-T6-R2 | 11:09:30 | 11:14:22 | 4 min 52 s |
| Checks (fmt, clippy, suite de tests) | recovery sweep | 10:45:36 | 11:53:47 | 1 h 08 min |
| Aceptación | final tables windows | 11:56:44 | 12:02:31 | 5 min 47 s |
| Aceptación | final tables wsl | 12:02:31 | 12:12:12 | 9 min 41 s |
| Aceptación | W23 bisection | 20:20:12 | 20:25:35 | 5 min 23 s |
| Aceptación | W25 e1 A/B | 20:32:00 | 20:38:48 | 6 min 48 s |
| Aceptación | W23-W29 e2 A/B | 20:53:36 | 21:08:46 | 15 min 10 s |
| Aceptación | W23-W25 e3 A/B | 21:31:10 | 21:37:41 | 6 min 31 s |
| Revisión RDD | codex-R1 tables | 21:39:34 | 21:52:26 | 12 min 52 s |
| Revisión RDD | codex-R2 tables | 21:52:58 | 21:56:11 | 3 min 13 s |
| Aceptación | final full windows | 21:57:14 | 22:19:58 | 22 min 44 s |
| Aceptación | final full wsl | 22:19:58 | 23:03:47 | 43 min 49 s |
| Revisión RDD | codex-R3 tables | 23:46:22 | 23:50:15 | 3 min 53 s |
| **Checks (fmt, clippy, suite de tests) (total)** | | | | **1 h 08 min** |
| **Aceptación (total)** | | | | **3 h 36 min** |
| **Revisión RDD (total)** | | | | **1 h 31 min** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `a840f02` | perf(tables): read row values from the stored row instead of building them | T1, T2 | 38 min 14 s | Exploración y mapa 1 min 42 s, Código y tests 8 min 24 s, Aceptación 28 min 08 s |
| `e036803` | docs(odd): record the row reads run and the W27 decision | T2 | 1 min 45 s | Aceptación 1 min 45 s |
| `ea08579` | refactor(tables): apply the review findings of the row reads | T2 | 9 min 01 s | Revisión RDD 9 min 01 s |
| `7cf73ca` | docs(odd): record the review of the row reads slice | T2 | 0 min 06 s | Revisión RDD 0 min 06 s |
| `9941b62` | fix(tables): read every column of a compound key in one walk (Codex R1) | T2 | 26 min 28 s | Revisión RDD 26 min 28 s |
| `f1889a4` | docs(reviews): record the first round of the row reads code review | T2 | 0 min 15 s | Revisión RDD 0 min 15 s |
| `ae39594` | docs(reviews): record the approval of the row reads code review | T2 | 6 min 17 s | Revisión RDD 6 min 17 s |
| `8091626` | refactor(tables): clarify the key element contract and its test | T2 | 7 min 30 s | Revisión RDD 7 min 30 s |
| `a02838b` | docs(odd): record the review of the last row reads slice | T2 | 0 min 01 s | Revisión RDD 0 min 01 s |
| `5409362` | perf(tables): add typed reads of a row's columns | T2, T3, T4 | 33 min 35 s | Código y tests 24 min 49 s, Aceptación 7 min 57 s, Revisión RDD 0 min 49 s |
| `963d126` | perf(tables): read checked rows through a per-column plan | T4 | 54 min 32 s | Código y tests 54 min 32 s |
| `9e8fdaa` | perf(tables): check a read row's columns without building their values | T4 | 18 min 25 s | Código y tests 18 min 25 s |
| `2802e07` | perf(tables): remember the leaves whose rows a read transaction checked | T4 | 16 min 38 s | Código y tests 16 min 38 s |
| `82b2918` | refactor(tables): keep a table's checked leaves with its reader | T4 | 18 min 39 s | Código y tests 18 min 39 s |
| `8e1de70` | docs(odd): record the row reads optimizations and measurements | T4 | 5 min 54 s | Código y tests 5 min 54 s |
| `1abe766` | test(tables): escape the NUL in the check_stored test text | T4 | 0 min 54 s | Código y tests 0 min 54 s |
| `ed61733` | perf(tables): read typed columns through a compact per-column record | T4 | 8 min 34 s | Código y tests 8 min 34 s |
| `7852933` | perf(tables): inline the step of a table scan | T4 | 9 min 32 s | Código y tests 9 min 32 s |
| `e7dd8d2` | perf(tables): lend the rows of a scan with next_row | T4 | 16 min 21 s | Código y tests 16 min 21 s |
| `ab6aa96` | docs(prd): draft the table row reads PRD | T4, PRD | 7 min 29 s | Plan (documento de ejecución) 6 min 24 s, Código y tests 1 min 05 s |
| `bf8f6d8` | docs(prd): apply the first round of the row reads PRD review | PRD | 16 min 36 s | Plan (documento de ejecución) 16 min 36 s |
| `77325f6` | docs(prd): name the history page for a missing schema version in L1 | PRD | 5 min 45 s | Plan (documento de ejecución) 5 min 45 s |
| `15bf568` | docs(prd): add the row reads PRD translation and its annotations | PRD | 7 min 47 s | Plan (documento de ejecución) 7 min 47 s |
| `5dcfa29` | feat(tables): stop checking row content in reads and scans | T5 | 21 min 05 s | Código y tests 9 min 11 s, Checks (fmt, clippy, suite de tests) 11 min 54 s |
| `3ecc294` | refactor(tables): apply the RDD review of the row reads slice | T5 | 14 min 25 s | Checks (fmt, clippy, suite de tests) 0 min 06 s, Revisión RDD 14 min 19 s |
| `2bc4fe4` | fix(tables): read the raw bytes of an older row's column | T3, T5 | 21 min 52 s | Aceptación 2 min 41 s, Revisión RDD 12 min 06 s, Correcciones de hallazgos 7 min 05 s |
| `bb496a1` | perf(tables): find a transaction's kept readers in a hash trie |  | 0 min 00 s |  |
| `8b87414` | docs(reviews): add the whole-run code review and its resolutions | T5 | 0 min 18 s | Correcciones de hallazgos 0 min 18 s |
| `6167d9e` | test(tables): pass the column name by value in the many-tables test | T5 | 7 min 40 s | Revisión RDD 7 min 39 s, Correcciones de hallazgos 0 min 01 s |
| `9ea1b59` | docs(reviews): add the code review follow-up and the R4 resolution | T5 | 0 min 07 s | Revisión RDD 0 min 07 s |
| `5d5f7a7` | docs(reviews): add the approving code review follow-up | T5 | 2 min 59 s | Revisión RDD 2 min 59 s |
| `5174433` | test(tables): apply the RDD review of the last row reads slice | T5 | 8 min 36 s | Revisión RDD 8 min 36 s |
| `d6a88b7` | test(tables): name the concurrent readers test for what it checks | T5 | 2 min 39 s | Revisión RDD 2 min 39 s |
| `30f0458` | docs: add the table row reads closing report | T5, T3 | 1 h 23 min | Aceptación 37 min 45 s, Revisión RDD 45 min 41 s |
| `7b645e5` | perf(tables): patch an update's fixed-width columns and write it in place | T6 | 12 min 29 s | Exploración y mapa 4 min 40 s, Checks (fmt, clippy, suite de tests) 7 min 49 s |
| `c76626e` | perf(pager): skip a journal entry's unchanged 64-byte blocks | T6, T8 | 1 h 12 min | Código y tests 0 min 22 s, Checks (fmt, clippy, suite de tests) 1 h 11 min |
| `5bd0f92` | revert(pager): compare journal entries word by word again | T8 | 52 min 51 s | Aceptación 52 min 51 s |
| `33f7f7b` | test(tables): apply the RDD review of the update patch | T8, T6 | 15 min 53 s | Aceptación 3 min 13 s, Revisión RDD 12 min 40 s |
| `c873418` | perf(tables): find an update's key columns in constant time | T6 | 11 min 25 s | Revisión RDD 11 min 25 s |
| `b5f82bd` | docs(reviews): add the update patch code review and its resolution |  | 0 min 00 s |  |
| `a714bb0` | docs(reviews): add the approving follow-up of the update patch review | T6 | 5 min 19 s | Revisión RDD 5 min 19 s |
| `bb1408b` | refactor(tables): name the primary key mark of the per-column plan | T6 | 1 min 50 s | Revisión RDD 1 min 50 s |
| `190cd84` | docs: update the table row reads closing report with the W28 round | T6 | 56 min 04 s | Aceptación 16 min 08 s, Revisión RDD 39 min 56 s |
| `1feb103` | docs(prd): annotate child PRD 7.5 with the JSON column design and restructure the tables annotations |  | 0 min 00 s |  |
| `a030203` | docs(reviews): record the Codex review of the tables PRD JSON annotation and its approval |  | 0 min 00 s |  |
| `cac715b` | docs(prd): add the Krummi and Voron only benchmark PRD |  | 0 min 00 s |  |
| `e05d783` | feat(bench): measure only Krummi and Voron with --solo-krummi-voron |  | 0 min 00 s |  |
| `e95287d` | feat(bench): measure the engines on the performance cores and report where |  | 0 min 00 s |  |
| `eb24cdf` | refactor(bench): clarify the limit of the engine processes after the review |  | 0 min 00 s |  |
| `47f3e8d` | fix(bench): give the workers absolute store paths for a relative work folder |  | 0 min 00 s |  |
| `93ba937` | docs(benchmark): add the Krummi and Voron only runs on Windows and Linux |  | 0 min 00 s |  |
| `1e9dffc` | docs(bench): explain the absolute store paths and the limit flag |  | 0 min 00 s |  |
| `a65a111` | fix(bench): start Windows engine processes inside their core limit |  | 0 min 00 s |  |
| `440f8ac` | docs(reviews): add the Krummi and Voron only code review and its resolutions |  | 0 min 00 s |  |
| `b02b34b` | fix(bench): start an engine process without its limit when the launcher cannot limit it |  | 0 min 00 s |  |
| `1531bd0` | docs(reviews): record the review of the launcher fixes |  | 0 min 00 s |  |
| `18e38f7` | docs: add the Krummi and Voron only benchmark closing report |  | 0 min 00 s |  |
| `9027c66` | perf(tables): check a row given in schema order without the name map | T9, T10 | 19 min 46 s | Exploración y mapa 5 min 45 s, Código y tests 14 min 01 s |
| `a71297b` | perf(tables): add ordered inserts at the end of the rows tree's last leaf | T10 | 29 min 08 s | Código y tests 29 min 08 s |
| `2c7bb1d` | refactor(tables): share the merge rule and tie the kept leaf to its root | T10 | 8 min 43 s | Código y tests 0 min 14 s, Revisión RDD 8 min 29 s |
| `caa86c3` | fix(tables): find the last leaf only on the insert's own path |  | 20 min 09 s | Revisión RDD 20 min 09 s |
| `29d0694` | test(tree): pin the last leaf of an insert whose merge does not fit |  | 1 min 36 s | Revisión RDD 1 min 36 s |
| `ce11e62` | docs(tree): describe a parent without separators as a valid state |  | 13 min 18 s | Revisión RDD 13 min 18 s |
| `e4e19de` | docs(reviews): add the third round code review of the row reads and its resolution |  | 0 min 00 s |  |
| `6ffad00` | docs(reviews): record the approval of the third round code review | T12 | 4 min 22 s | Aceptación 0 min 27 s, Revisión RDD 3 min 55 s |
| `d92a88f` | test(tables): repeat the default borrow read until no transfer cuts it | T12, T13 | 1 h 45 min | Checks (fmt, clippy, suite de tests) 37 min 43 s, Aceptación 1 h 07 min, Documentación 0 min 07 s |
| `3641ace` | docs(benchmark): add the final Krummi and Voron only runs of the row reads | T13 | 0 min 22 s | Checks (fmt, clippy, suite de tests) 0 min 22 s |
| `abf5555` | test(tree): explain the sizes of the last leaf merge test and validate its tree | T13 | 2 min 50 s | Checks (fmt, clippy, suite de tests) 2 min 50 s |
| — | después del último commit | T13 | 4 min 28 s | Checks (fmt, clippy, suite de tests) 4 min 21 s, Cierre (informe y merge) 0 min 07 s |
| **Total** | | | **15 h 11 min** | |

## Informe de cambios

Toda la rama (`bb8acca..HEAD`). Incluye el banco solo con Krummi y Voron (`crates/krummi-bench`, `README.md` y parte de
`docs` y `odd`), que tiene su propio informe:

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 3.654 | 259 | — | — | 44 |
| `crates/krummi` | 3.032 | 172 | 2.635 | 290 | 14 |
| `crates/krummi-bench` | 1.374 | 87 | 1.064 | 133 | 17 |
| `odd` | 1.159 | 0 | — | — | 6 |
| `crates/krummi-tree` | 442 | 17 | 361 | 66 | 2 |
| `README.md` | 6 | 2 | — | — | 1 |
| **Total** | **9.667** | **537** | **4.060** | **489** | **84** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+32 −0)

Tercera ronda (`18e38f7..HEAD`):

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 394 | 22 | 347 | 38 | 5 |
| `crates/krummi-tree` | 354 | 12 | 292 | 51 | 2 |
| `docs` | 311 | 0 | — | — | 5 |
| `odd` | 115 | 0 | — | — | 2 |
| **Total** | **1.174** | **34** | **639** | **89** | **14** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

## Resumen

Las cargas de tablas que leen filas, leer por la clave y recorrer intervalos, cuestan ahora en Krummi lo que en Voron
frente a su propio árbol: recorrer una tabla rinde 0,66 de lo que rinde recorrer el árbol sin tabla, y en Voron 0,69.
Una fila leída ya no se decodifica entera: guarda referencias a la clave y a sus bytes en la página, y cada columna se
lee cuando se pide, con lecturas tipadas que no construyen un `Value`. Los recorridos prestan una sola fila, como el
`TableValueHolder` de Voron. Por decisión del usuario (PRD), las lecturas se apoyan en la integridad xxhash de las
páginas (G6) y no comprueban el contenido de cada fila; las escrituras que reemplazan una fila sí la comprueban, para
no sellar una fila mal formada.

## Decisiones tomadas sin preguntar

- Las cargas de tablas se corrieron fuera del orquestador, que no permite elegirlas: `work/rr/tables.py` las corre
  como él (L30).
- En Windows vale la tanda con los dos motores fijados a los núcleos de rendimiento; la tanda sin fijar se descartó
  porque Voron salió a la mitad de lo habitual (L31).
- Las bajadas de más de 0,05 se comprobaron con cargas sueltas: W29 en Linux dos veces más; W23 en Windows alternando
  el binario de 7.1 y el de ahora.
- Avisos de RDD aceptados tal cual o descartados con evidencia, como dice la tabla de arriba.
- Tercera ronda: W25 se midió con A/B solo de Krummi en rondas alternas (`work/bkv/ab.py`), porque Voron no cambia; la
  ejecución de Linux corre sin fijar núcleos, porque WSL 2 no distingue los de rendimiento (B4 del PRD del banco), y
  compila el archivo de `29d0694`, que solo difiere de la cabeza en un comentario (L50).
- El test intermitente de T13 se corrigió en el test, no en el almacén: que una página cambie de dirección cuando la
  transferencia la lleva al archivo de datos es el diseño del pager (L51).

## Pendiente (no bloquea)

- **AVX2 en el journal** no se probó: la diferencia por bloques no ganó nada, así que ese bucle no es el cuello de
  botella (L35).
- **Los motores distintos de Voron** no se midieron con esta rama: las ejecuciones completas finales usan
  `--solo-krummi-voron`.
- **Copia en Engram** del documento de ejecución pendiente: Engram no aceptó escrituras en esta sesión.
- **Codex no revisó el cambio del test e11** (`d92a88f`): su tercera ronda, la última de la cadena, lo dejó fuera de
  su alcance de seguimiento; lo revisó RDD (L52, L53). Una revisión más necesita la autorización del usuario.
- **Pregunta de producto del banco**, abierta en su [informe](../benchmark/benchmark-krummi-voron-report.md): en Linux, varias
  `cpu_capacity` casi iguales cuentan como clases distintas (B4).
- `writer_check_tests::ac_h1_a_write_never_checks_the_nodes_it_changed` falló una vez por tiempos (L11); es código de
  los árboles, ajeno a esta ejecución.

**Riesgo:** item 1 en la tercera ronda (`append_last` añade la fila a la última hoja sin bajar por el árbol: lo cubren
las equivalencias byte a byte con `try_add`, el test de modelo de la tabla, el test de una hoja dañada, el verificador
independiente, dos revisiones RDD escaladas y Codex). Item 1 en la segunda ronda (la actualización escribe la fila en su hoja con `page_mut`, sin pasar por
`set`: lo cubren el test de equivalencia de bytes, la lectura tras confirmar, el test del árbol con una lectura abierta
antes, la lente de riesgo de RDD y las revisiones de Codex). Item 4 (los lectores guardados son un trie de solo inserción con `OnceLock`, compartido por los hilos de una
transacción de lectura; lo cubren `threads_of_one_read_transaction_read_each_table_with_its_own_schema`, la lente de
riesgo de RDD y la reproducción concurrente de Codex en su segunda ronda). Además, item 3 por petición del usuario: E19
cambia para las lecturas (PRD L1–L3).

## Limpieza

Hecha al final de la tercera ronda: las compilaciones, fuentes, binarios, perfiles y almacenes de `work/rr/` y
`work/bkv/` (unos 6,5 GB) y, en WSL, `~/rr`, `~/rrprof` y `~/bkv` (unos 3,5 GB). Quedan los archivos sueltos de
medidas y scripts (`*.tsv`, `*.py`, `*.sh`, `*.md`, registros: 3,5 MB) que cita el documento de ejecución.
