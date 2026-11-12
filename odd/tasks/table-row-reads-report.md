# Lecturas de filas de las tablas terminadas: W26 y W27 cumplen K7 en Windows y Linux, rama verificada sin fusionar

Informe de cierre de `table-row-reads` (2026-11-12). Enfoque de ejecución: ODD.
Fuente de producto: [tables-row-reads.prd.md](../../docs/prd/krummi/tables-row-reads.prd.md) (desde L20; antes, una
optimización sin PRD, anotación del 2026-11-04 a K13 de [benchmark.prd.md](../../docs/prd/krummi/benchmark.prd.md)).
Documento de ejecución: [table-row-reads.md](table-row-reads.md).

Rama `feat/table-row-reads` en `d6a88b7` (más los commits de este informe), creada desde `feat/tables-schema` en
`bb8acca`. Sin fusionar y sin publicar: la revisa el usuario antes.

## Resultado

- **Especificación:** las lecturas por la clave y los recorridos ya no comprueban el contenido de la fila (L1); una
  fila mal formada se devuelve y leer su columna da otro valor o un pánico normal del hilo (L2); actualizar e insertar
  o actualizar siguen comprobando la fila que reemplazan (L3). Antes, las optimizaciones sin cambio observable: filas
  perezosas que guardan sus bytes, lecturas tipadas, un plan por columna, los lectores guardados en la transacción
  (ahora en un trie de hash) y `next_row`, que presta la fila. Suite: `cargo test --workspace` 804 pasan, 0 fallan,
  23 ignoradas; `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` limpios; los 10
  tests de filas mal formadas pasan también con `--release`.
- **Aceptación:** cargas de tablas W23–W29 de Krummi y Voron con `d6a88b7` (decisión del usuario, L30: solo las
  cargas de tablas). W27 pasa de 0,15 a **0,89** en Windows y de 0,19 a **0,97** en Linux; W26 de 0,75 a **0,96** y de
  0,88 a **1,13**. Las sumas de datos de Voron coinciden con las de Krummi en todas las cargas. Detalle en L31.
- **Revisiones y verificación:** siete revisiones RDD, todas aprobadas (sección siguiente); revisión cruzada de Codex
  del PRD (tres rondas, aprobada) y del código en dos cadenas: `bb8acca..7cf73ca` (dos rondas, aprobada) y la de toda
  la ejecución, `bb8acca..3ecc294` (tres rondas, aprobada). Informes y resoluciones en
  [docs/reviews/table-row-reads/](../../docs/reviews/table-row-reads/) y
  [docs/reviews/tables-row-reads/](../../docs/reviews/tables-row-reads/).
- **Hallazgos corregidos por el camino:**
  - Codex R1 (mayor): leer las columnas de una clave compuesta recorría su prefijo una vez por columna; corregido en
    `9941b62`.
  - Codex R2 (mayor): una lectura tipada de una fila de una versión antigua decodificaba su texto, así que
    `text_bytes` daba pánico con un texto que no es UTF-8; corregido en `2bc4fe4`.
  - Codex R3 (mayor): `Readers::keep` recorría una lista de todas las tablas leídas (4.066 ns por lectura en la última
    de 1.000 tablas); un trie de hash en `bb496a1` lo deja en 196 ns.
  - Codex R4 (menor): un test nuevo no pasaba clippy. El comando de checks lo ocultaba porque pasaba clippy por `tail`;
    desde entonces los checks se ejecutan con `pipefail` (L24).
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
| AC-L9 | cargas de tablas en Windows (fijadas) y en Linux (L31) | W26 y W27 cumplen en los dos; las bajadas de más de 0,05 (W23 en Windows, W29 en Linux) quedan dentro del margen con cargas sueltas; W1–W22 no se ejecutaron (decisión del usuario) |

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

Las evaluaciones de cada tramo dijeron `medium`, `under_budget`; todos los tramos se revisaron igualmente.

## Tiempo

- Estimaciones: 6 h – 10 h para la optimización sin PRD (L1); 1–2 h más unas 4 h de ejecuciones completas para
  implementar el PRD (L20). Inicio real: 2026-11-11 12:05:51 +0000. Fin: 2026-11-12 07:13:14 +0000.
- **Duración activa: 7 h 53 min** en total, dentro de la primera estimación, aunque incluye además escribir el PRD y
  su implementación. La implementación del PRD (commits `5dcfa29`–`d6a88b7` y lo posterior en la tabla por commit)
  suma unas 2 h 43 min, con la aceptación reducida a las cargas de tablas (unos 25 min en lugar de unas 4 h). El código
  (3 h 12 min) y la revisión (2 h 24 min) se llevaron la mayor parte; la revisión incluye unos 45 min en los que el
  primer intento de ejecución completa corría en segundo plano.
- Esperas por el usuario: las del registro de fases (abajo); la mayor, 8 h 06 min, es el reinicio de Windows de esa
  noche, que cortó la ejecución completa (L29).

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 1 min 42 s | 0 % | 1 |
| Plan (documento de ejecución) | 36 min 32 s | 8 % | 1 |
| Código y tests | 3 h 12 min | 41 % | 5 |
| Checks (fmt, clippy, suite de tests) | 12 min 00 s | 3 % | 1 |
| Aceptación | 1 h 18 min | 17 % | 4 |
| Revisión RDD | 2 h 24 min | 31 % | 5 |
| Correcciones de hallazgos | 7 min 24 s | 2 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **7 h 53 min** | **100 %** | **19** |

- Inicio: 2026-11-11 12:05:51 +00:00. Fin: 2026-11-12 07:13:14 +00:00.
- Arranque confirmado: 20:23:30. Activo desde el arranque: **7 h 53 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 11 h 13 min.
  - 12:45:50–13:17:38 (31 min 48 s): W27 decision
  - 14:41:09–14:58:58 (17 min 49 s): W27 and W28 scope
  - 16:53:01–17:34:30 (41 min 29 s): W27 how far
  - 18:10:43–18:28:35 (17 min 52 s): PRD malformed rows
  - 19:05:07–20:23:30 (1 h 18 min): PRD authorization
  - 22:28:52–06:35:29 (8 h 06 min): Windows restart

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
| **Aceptación (total)** | | | | **1 h 25 min** |
| **Revisión RDD (total)** | | | | **55 min 16 s** |

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
| — | después del último commit | T5, T3 | 1 h 23 min | Aceptación 37 min 45 s, Revisión RDD 45 min 41 s |
| **Total** | | | **7 h 53 min** | |

## Informe de cambios (`bb8acca..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 2.237 | 142 | 1.937 | 215 | 13 |
| `docs` | 1.273 | 0 | — | — | 19 |
| `odd` | 197 | 0 | — | — | 2 |
| `crates/krummi-bench` | 34 | 31 | 27 | 6 | 1 |
| **Total** | **3.741** | **173** | **1.964** | **221** | **35** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Sin archivos de bloqueo ni generados.

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

## Pendiente (no bloquea)

- **W28 en Windows** sigue en 0,75 (0,73 en el cierre de 7.1): su coste está en la escritura, fuera de este PRD;
  sigue K13 y pide un cambio aparte.
- **W1–W22 no se midieron** en esta ejecución (decisión del usuario, L30); este cambio solo toca el código de las
  tablas.
- **W23 en Windows:** Krummi mide un 3 % menos que con el binario de 7.1 en cargas sueltas, dentro del margen de AC-L9;
  sin investigar.
- **El banco no fija sus procesos** a los núcleos de rendimiento, y en Windows eso puede partir por dos una carga (L13,
  L31): un cambio del banco aparte.
- **Copia en Engram** del documento de ejecución pendiente: Engram no aceptó escrituras en esta sesión.
- `writer_check_tests::ac_h1_a_write_never_checks_the_nodes_it_changed` falló una vez por tiempos (L11); es código de
  los árboles, ajeno a esta ejecución.

**Riesgo:** item 4 (los lectores guardados son un trie de solo inserción con `OnceLock`, compartido por los hilos de una
transacción de lectura; lo cubren `threads_of_one_read_transaction_read_each_table_with_its_own_schema`, la lente de
riesgo de RDD y la reproducción concurrente de Codex en su segunda ronda). Además, item 3 por petición del usuario: E19
cambia para las lecturas (PRD L1–L3).

## Limpieza manual

- `work/rr/` (1,5 GB): binarios, fuentes archivadas, la compilación de Voron y los perfiles de esta ejecución.
- En WSL: `~/rr` (322 MB) y `~/rrprof` (1,5 GB).
