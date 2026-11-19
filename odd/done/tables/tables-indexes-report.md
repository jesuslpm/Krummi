# Índices secundarios de las tablas: implementados, verificados y aprobados por Codex

Informe de cierre de `tables-indexes` (2026-11-19). Enfoque de ejecución: ODD.
Fuente de producto: [tables-indexes.prd.md](../../../docs/prd/krummi/tables/tables-indexes.prd.md) (PRD hijo 7.2).
Documento de ejecución: [tables-indexes.md](tables-indexes.md).

Rama `feat/tables-indexes`, desde `main` en `0158fd4`, sin fusionar y sin publicar. El último commit de código es
`09edff7`. La rama queda para tu revisión: la fusión y la publicación son decisiones tuyas.

## Resultado

- **Especificación:** se cumplen IX1–IX15 y AC-IX1–AC-IX16 del PRD.
  - **Biblioteca:** crear un índice en los dos modos, también sobre filas existentes, ordenando en memoria y en
    `krummi.sort`. Además, borrarlo, describirlo y mantenerlo en toda escritura de filas, y recorrerlo devolviendo
    filas o entradas. El formato pasa a 12.
  - **Tests:** `cargo test --workspace` pasa (58 binarios de test).
    - La última corrida completa del padre, después de la corrección de W27, dio 963 tests sin fallos.
    - Las correcciones de Codex R1–R4 se verificaron con la suite completa del escritor y con una comprobación
      puntual del padre: `table_index_sort` 12 de 12 y los 4 tests `ac_ix11`.
  - **Lint:** `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` pasan.
- **Aceptación:** corridas completas del banco del código `1de443b` con `--solo-krummi-voron`, bajo el plan de
  energía del banco: **32 de 32 umbrales en Windows y en WSL 2**. Detalle en L15 del documento.

  | Carga | Windows | WSL 2 |
  |---|---|---|
  | W30 (inserción con índices) | 1,07 | 1,20 |
  | W31 (búsqueda por índice) | 1,06 | 1,30 |
  | W32 (intervalos de índice) | 0,98 | 0,99 |

  En las dos plataformas, los checksums de Krummi y Voron coinciden en todas las cargas.
  - **Antes y después del cambio:** el A/B solo de Krummi en Windows, sobre la base `0158fd4`, aísla el efecto del
    cambio (L16). W23–W26, W28 y W29 quedan neutras, entre 0,993 y 1,006. W27 era un 7 % más lenta y se corrigió
    en `7220476`: 82,5 contra 84,8 ns por fila de la base (L17).
  - **Bajadas en las corridas completas:** las demás bajadas frente a las corridas anteriores son de Voron y de la
    máquina, no del cambio. La corrida completa no se repitió después de la corrección de W27.
- **Revisiones y verificación:**
  - **Verificadores independientes:** revisaron T2, T3, T4 y T5 sin encontrar bloqueos. Compararon el
    comportamiento con la base:
    - las tablas sin índices, en T3;
    - los recorridos por clave primaria, en T4;
    - la apertura, el cierre y el abandono del almacén, en T5.
  - **RDD:** aprobó todas las porciones (tabla abajo).
  - **Codex:** la revisión de código del rango entero quedó aprobada en la ronda 3
    ([resoluciones](../../../docs/reviews/tables-indexes/2026-11-19-code-resolutions.md)).
- **Hallazgos corregidos durante la ejecución:**
  - **Archivos de ordenación sin longitud registrada:** un archivo truncado se leía como un final normal y el
    índice quedaba sin entradas. Ahora la creación falla con la clase de entrada y salida (`1de443b`, RDD de T5).
  - **W27 un 7 % más lenta**, por copias de la entrada en cada fila. Se corrigió en `7220476`.
  - **Codex R1:** el proceso abortaba si faltaba memoria para una entrada. Ahora falla con memoria agotada y T11
    (`8f1343a`).
  - **Codex R2 y R4:** las identidades de índice no se comparaban con todo el catálogo, y la primera corrección
    copiaba los valores por defecto de otras tablas sin poder fallar. Se resolvió en `2d2912b` y `09edff7`.
  - **Codex R3:** una apertura que fallaba después de la recuperación borraba `krummi.sort`. Se corrigió en
    `4bf9b3c`.
  - **Avisos de las revisiones:** varios fortalecimientos de tests, en la tanda de correcciones `1de443b`.

## Cobertura de los criterios de aceptación

| Criterio | Test o comprobación | Resultado |
|---|---|---|
| AC-IX1 | `ac_ix1_indexes_created_over_rows_have_their_entries_in_n5_order`; tests unitarios de orden de `index_key_tests` | PASS |
| AC-IX2 | `ac_ix2_every_row_write_keeps_the_indexes_in_step`, `ac_ix2_an_abort_and_a_crash_keep_the_indexes_of_the_last_commit`, `ac_ix2_an_identical_update_writes_no_page_of_the_indexes` | PASS |
| AC-IX3 | `ac_ix3_scans_of_every_index_give_its_rows_and_entries_in_order`, `ac_ix3_a_primary_key_column_among_the_included_comes_once_in_a_copy` | PASS |
| AC-IX4 | `ac_ix4_every_interval_gives_the_entries_of_ix9`, `ac_ix4_malformed_intervals_fail_with_their_rule`, `ac_ix4_a_prefix_that_does_not_fit_fails_with_its_size` | PASS |
| AC-IX5 | `ac_ix5_random_indexes_follow_the_model_of_n5`, con un modelo independiente desde `1de443b` | PASS |
| AC-IX6 | `ac_ix6_a_row_that_repeats_a_unique_index_fails_without_effect` | PASS |
| AC-IX7 | `ac_ix7_a_creation_that_fails_leaves_the_store_as_without_it`, `ac_ix14_a_duplicate_is_found_the_same_in_files_and_leaves_no_effect` | PASS |
| AC-IX8 | `ac_ix8_a_bulk_load_that_fails_leaves_the_table_and_its_indexes_as_before`, `ac_ix8_a_bulk_load_out_of_order_gives_the_entries_of_inserts_one_by_one` | PASS |
| AC-IX9 | `ac_ix9_*`: reglas de IX3, nombres y límites, los dos modos y las precedencias al crear, al guardar una fila y al recorrer | PASS |
| AC-IX10 | `ac_ix10_dropping_an_index`, `ac_ix10_table_operations_and_indexes`, `ac_ix10_schema_changes_and_indexes` | PASS |
| AC-IX11 | `ac_ix11_describe_identities_and_the_index_state`, `ac_ix11_identities_equal_to_known_ones_are_drawn_again`, `..._known_to_the_store_outside_the_table_...`, `..._of_other_catalog_objects_...` | PASS |
| AC-IX12 | `ac_ix12_what_changes_and_what_does_not`, con comparación de archivos byte a byte | PASS |
| AC-IX13 | `ac_ix13_*` en `index_tests.rs`, `table_indexes.rs` y `table_index_scans.rs` | PASS |
| AC-IX14 | `ac_ix14_*` en `table_index_sort.rs` (12 tests) | PASS |
| AC-IX15 | `ac_ix15_a_store_of_format_11_is_of_a_previous_format`, más la sonda del verificador con un almacén real del formato 11 (L6) | PASS |
| AC-IX16 | Corridas completas en Windows y WSL 2 (32 de 32), corrida reducida y selftest de Voron (L11, L15, L16) | PASS. W27 se midió después de la corrección solo con el A/B |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `0158fd4..036c982` (T1) | fiabilidad, legibilidad | aprobada | 4 avisos: aserciones vacías y entradas mal formadas sin test. Se corrigieron en T4 y `1de443b` | `review-fa901f8836220898` |
| `036c982..9d57e7f` (T2a) | riesgo, resiliencia, legibilidad, fiabilidad | aprobada | 13 avisos. El pánico provisional se resolvió en T3 y la construcción en memoria en T5. El supuesto cambio de disposición de la clave al permitir nulos se descartó con evidencia (L5) | `review-37246ff2b6d8aa00` |
| `9d57e7f..e7b95e9` (T2b, tests) | fiabilidad | aprobada | 4 avisos de calidad de tests. Se corrigieron en T3 y `1de443b` | `review-6793bcb98f2f04dd` |
| `e7b95e9..4a4c345` (T3) | resiliencia, fiabilidad, legibilidad | aprobada | Avisos: Vaciar sin test de los árboles, que se cubrió en `1de443b`, y sugerencias de documentación | `review-6b19eb30331bd5ee` |
| `ba9de2e..025531a` (T4a) | fiabilidad, legibilidad, resiliencia | aprobada | Faltaban los tests de recorridos, que llegaron en T4b. Un pánico en debug con una entrada mal formada es el comportamiento elegido | `review-f1a75d485521db85` |
| `025531a..ba3d843` (T4b) | fiabilidad | aprobada | 4 avisos de calidad de tests (modelo, filas completas). Se corrigieron en `1de443b` | `review-6074aa258a6f86ef` |
| `ba3d843..e340580` (T6) | fiabilidad, legibilidad | aprobada | ninguno | `review-38cf23bdc604d2e6` |
| `e340580..7c33f57` (T5) | riesgo, resiliencia, fiabilidad, legibilidad | aprobada | El final de los archivos de ordenación no se verificaba: se corrigió en `1de443b`. También hubo avisos sobre el orden de fallos documentado y el límite del buffer, cubiertos en `1de443b` | `review-a4fa65d8e5d34771` |
| `7c33f57..1de443b` (correcciones) | resiliencia, fiabilidad | aprobada | Un encabezado dañado dentro de una corrida que sigue cabiendo se lee como dato. Queda pendiente | `review-a5da2fc5ba9c4e33` |
| `1de443b..7220476` (W27) | legibilidad, fiabilidad | aprobada | Comentario engañoso, reescrito en `767e251`. Procedencia del informe de Windows, aclarada en L17 | `review-3c09936c657c00f6` |
| `7220476..767e251` (última porción) | legibilidad | aprobada | Redacción del commit medido en L15 | `review-2e490fdaa1ace3c2` |
| `767e251..8f1343a` (Codex R1–R3) | riesgo, resiliencia, fiabilidad | aprobada | `store_knows` recorre todo el catálogo en cada sorteo. Es aceptable porque crear o borrar un índice es raro | `review-4d2dc3647d685d63` |
| `8f1343a..09edff7` (Codex R2/R4) | resiliencia, fiabilidad, legibilidad | aprobada | El test de fallos inyectados no ve la copia de los valores por defecto; la evidencia es la sonda de Codex | `review-47b0a4cbadbf4f44` |

Evaluaciones sin revisión: `09edff7..d88af5c`, `passive` (solo documentos). El primer candidato `0158fd4..0cb4c39`
(T1 + T2) y T2 sola se detuvieron con `lens_context_budget_exceeded` sin crear autoridad. T2, un commit local sin
publicar, se rehizo en T2a y T2b con `git reset --soft` (L5). En T5, un commit de documentos hecho durante la
revisión movió HEAD; el linaje aprobado se reconoció con su propio reconocimiento (L13).

## Reutilización

- **Codificación de claves:** el codificador de índices extiende el de claves primarias. `key.rs` gana `KeyColumn`,
  `Direction` y `Nulls` sobre el mismo elemento ascendente. Los límites de intervalo comparten `range_bounds` con
  `key_range`.
- **Recorridos:** usan el mismo recorrido de filas que la clave primaria, `ranges::Scan`, ahora genérico sobre su
  localizador. También reutilizan el préstamo de filas y la proyección de 7.1.
- **Escrituras:** las entradas de índice salen de un solo `EntryLayout::entry`, compartido por la creación y por las
  escrituras de filas.
- **Archivos y sorteos:** `krummi.sort` sigue el mismo ciclo de vida que `krummi.temp`. Los sorteos de identidad usan
  `identity::draw` y `known_identity`.
- **Lectura del esquema:** `stored_identities` comparte su capa de lectura con `TableSchema::decode`.
- **Escrito nuevo:** el mantenimiento de índices (`maintain.rs`), la ordenación externa (`table/sort.rs` y
  `krummi-pager/src/sort.rs`) y el esquema `Indexed` de Voron.

## Tiempo

- Estimación: entre 10 h y 16 h. Arranque real: 2026-11-18 18:07:06 +0000. Fin de la verificación:
  2026-11-19 04:12:37 +0000.
- **Duración activa desde el arranque: 10 h 05 min**, en el borde inferior de la estimación.
  - **Aceptación:** 4 h 03 min. Incluye las corridas completas, el A/B de las cargas de tablas y la investigación de
    la regresión de W27, que tomó unas 2 h 15 min.
  - **Revisiones:** 3 h 43 min.
- Esperas por el usuario: ninguna.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 4 min 15 s | 1 % | 1 |
| Código y tests | 1 h 31 min | 15 % | 1 |
| Checks (fmt, clippy, suite de tests) | 9 min 03 s | 1 % | 1 |
| Aceptación | 4 h 03 min | 40 % | 1 |
| Revisión RDD | 3 h 43 min | 37 % | 3 |
| Correcciones de hallazgos | 31 min 54 s | 5 % | 1 |
| Cierre (informe y merge) | 0 min 29 s | 0 % | 1 |
| **Total activo** | **10 h 05 min** | **100 %** | **9** |

- Inicio: 2026-11-18 18:07:13 +00:00. Fin: 2026-11-19 04:12:37 +00:00.
- Arranque confirmado: 18:07:13. Activo desde el arranque: **10 h 05 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | full-windows | 22:43:53 | 23:09:37 | 25 min 44 s |
| Aceptación | full-linux | 23:09:38 | 00:00:50 | 51 min 12 s |
| Aceptación | ab-windows | 00:01:51 | 00:31:47 | 29 min 56 s |
| Revisión RDD | codex-R1 | 02:57:34 | 03:05:58 | 8 min 24 s |
| Revisión RDD | codex-R2 | 03:43:14 | 03:43:15 | 0 min 01 s |
| Revisión RDD | codex-R2 | 03:43:25 | 03:50:43 | 7 min 18 s |
| Revisión RDD | codex-R3 | 04:04:20 | 04:11:56 | 7 min 36 s |
| **Aceptación (total)** | | | | **1 h 46 min** |
| **Revisión RDD (total)** | | | | **23 min 19 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `1dc55aa` | docs(odd): start the secondary indexes execution | map code for 7.2 | 4 min 14 s | Exploración y mapa 4 min 14 s |
| `2657249` | feat(tables): encode index keys with direction and nulls order | map code for 7.2, T1 | 26 min 28 s | Exploración y mapa 0 min 01 s, Código y tests 26 min 27 s |
| `036c982` | docs(odd): record T1 of the secondary indexes | T1 | 0 min 10 s | Código y tests 0 min 10 s |
| `9d57e7f` | feat(tables): create, drop and describe secondary indexes | T1, RDD T1 slice, RDD T1+T2 | 1 h 12 min | Código y tests 1 h 05 min, Revisión RDD 6 min 40 s |
| `e7b95e9` | test(tables): cover creating, dropping and describing secondary indexes | RDD T1+T2 | 4 min 04 s | Revisión RDD 4 min 04 s |
| `770ab66` | docs(odd): record T2 and its reviews | RDD T1+T2 | 3 min 25 s | Revisión RDD 3 min 25 s |
| `7c7e3c3` | docs(odd): record the T2 verification | RDD T1+T2 | 1 min 10 s | Revisión RDD 1 min 10 s |
| `4a4c345` | feat(tables): keep secondary indexes in step with every row write | RDD T1+T2 | 48 min 44 s | Revisión RDD 48 min 44 s |
| `1075298` | docs(odd): record T3 and its review | RDD T1+T2 | 4 min 10 s | Revisión RDD 4 min 10 s |
| `ba9de2e` | docs(odd): record the T3 verification | RDD T1+T2 | 27 min 42 s | Revisión RDD 27 min 42 s |
| `025531a` | feat(tables): scan secondary indexes for rows or entries | RDD T1+T2 | 12 min 05 s | Revisión RDD 12 min 05 s |
| `ba3d843` | test(tables): cover index scans, intervals and their failures | RDD T1+T2 | 3 min 06 s | Revisión RDD 3 min 06 s |
| `99f6a95` | docs(odd): record T4 and its reviews | RDD T1+T2 | 3 min 09 s | Revisión RDD 3 min 09 s |
| `f0f621c` | docs(odd): record the T4 verification | RDD T1+T2 | 6 min 20 s | Revisión RDD 6 min 20 s |
| `e340580` | feat(bench): add the secondary index loads W30-W32 | RDD T1+T2 | 13 min 50 s | Revisión RDD 13 min 50 s |
| `7c33f57` | feat(tables): sort large index builds in krummi.sort run files | RDD T1+T2 | 14 min 18 s | Revisión RDD 14 min 18 s |
| `f058814` | docs(odd): record T5 and T6 | RDD T1+T2 | 0 min 42 s | Revisión RDD 0 min 42 s |
| `e808a33` | docs(odd): record the T5 review and verification | RDD T1+T2 | 14 min 51 s | Revisión RDD 14 min 51 s |
| `1de443b` | fix(tables): fail an index build whose sort run file is short | RDD T1+T2 | 15 min 54 s | Revisión RDD 15 min 54 s |
| `65bd539` | docs(odd): record the fix pass and its review | RDD T1+T2, T7 full runs | 2 min 04 s | Aceptación 1 min 47 s, Revisión RDD 0 min 17 s |
| `b41b09d` | docs(bench): save the secondary indexes complete runs | T7 full runs | 1 h 16 min | Aceptación 1 h 16 min |
| `59674af` | docs(odd): record the A/B of the table loads | T7 full runs | 30 min 25 s | Aceptación 30 min 25 s |
| `7220476` | perf(tables): keep primary-key range scans as fast as before indexes | T7 full runs, W27 fix checks | 2 h 22 min | Checks (fmt, clippy, suite de tests) 7 min 06 s, Aceptación 2 h 15 min |
| `767e251` | docs(tables): reword a scan step comment and record the W27 fix | W27 fix checks | 1 min 39 s | Checks (fmt, clippy, suite de tests) 1 min 39 s |
| `ac3c38c` | docs(reviews): add the Codex code review of the secondary indexes | W27 fix checks, RDD last slice | 9 min 56 s | Checks (fmt, clippy, suite de tests) 0 min 18 s, Revisión RDD 9 min 38 s |
| `4bf9b3c` | fix(pager): keep krummi.sort when a store open fails after recovery | RDD last slice | 10 min 42 s | Revisión RDD 10 min 42 s |
| `2d2912b` | fix(tables): redraw index identities known anywhere in the store | RDD last slice | 9 min 26 s | Revisión RDD 9 min 26 s |
| `8f1343a` | fix(tables): fail index entries that get no memory with out of memory | RDD last slice | 13 min 22 s | Revisión RDD 13 min 22 s |
| `d1f71e3` | docs(reviews): record the resolutions of the Codex code review | RDD last slice, Codex R1-R3 RDD | 3 min 09 s | Revisión RDD 0 min 14 s, Correcciones de hallazgos 2 min 55 s |
| `fdeb9e4` | docs(reviews): add the Codex follow-up of the secondary indexes code review | Codex R1-R3 RDD | 8 min 00 s | Correcciones de hallazgos 8 min 00 s |
| `09edff7` | fix(tables): check index identities against every catalog object | Codex R1-R3 RDD | 11 min 47 s | Correcciones de hallazgos 11 min 47 s |
| `7ae6487` | docs(reviews): record the second round of the Codex code review | Codex R1-R3 RDD | 1 min 23 s | Correcciones de hallazgos 1 min 23 s |
| `d88af5c` | docs(reviews): record the approval of the secondary indexes code review | Codex R1-R3 RDD | 7 min 49 s | Correcciones de hallazgos 7 min 49 s |
| `8c41ebe` | docs: record the secondary indexes in the performance plan and close the tasks | closing report | 0 min 29 s | Cierre (informe y merge) 0 min 29 s |
| — | después del último commit |  | 0 min 00 s |  |
| **Total** | | | **10 h 05 min** | |

La fase «Revisión RDD» de esta tabla incluye la implementación de T3–T6 por escritores delegados. Las marcas de fase
no se cambiaron a `implement` al lanzar cada tarea y se quedaron en `review` después de la primera revisión. Esa
fase incluye también las revisiones y verificaciones, que corrían en paralelo. La fase «Aceptación» incluye la
investigación y la corrección de W27. El total activo es exacto; el reparto entre implementación y revisión de esas
horas no lo es.

## Informe de cambios (`0158fd4..8c41ebe`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 11.284 | 245 | 9.269 | 1.473 | 30 |
| `crates/krummi-bench` | 834 | 137 | 444 | 98 | 14 |
| `crates/krummi-pager` | 803 | 54 | 618 | 145 | 9 |
| `docs` | 638 | 0 | — | — | 7 |
| `odd` | 355 | 0 | — | — | 3 |
| `crates/krummi-tx` | 18 | 6 | 9 | 8 | 4 |
| `README.md` | 2 | 1 | — | — | 1 |
| **Total** | **13.934** | **443** | **10.340** | **1.724** | **68** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

El código se reparte así: `crates/krummi` tiene la codificación, el catálogo, el mantenimiento, los recorridos y la
ordenación; los tests de integración suman unas 5.000 líneas. `crates/krummi-pager` tiene los archivos de
ordenación y la apertura, y `crates/krummi-bench`, W30–W32 en Rust y C#. No hubo previsión de líneas.

## Resumen

Una tabla puede tener ahora hasta 64 índices secundarios, únicos o no, de 1 a 32 columnas, cada una ascendente o
descendente y con los nulos primero o último, y con columnas incluidas. La aplicación puede:

- crear un índice sobre una tabla con filas, en una sola escritura y sin efecto si falla;
- borrarlo;
- verlo en Describir, con su identidad y el estado de los índices;
- recorrerlo por intervalos, pidiendo filas o solo entradas.

Toda escritura de filas mantiene los índices al día y comprueba la unicidad antes de cambiar nada.

**Decisiones técnicas:**

- Cada índice es un árbol B+ cuya clave es la clave del índice seguida de la clave primaria, según el documento de
  diseño 3. Las definiciones se guardan en la entrada de la tabla del catálogo, con formato 12.
- La creación ordena las entradas en memoria y, cuando no caben en 64 MiB, en archivos de `krummi.sort` que se
  mezclan después. Detecta los duplicados y el tamaño de las entradas antes de reservar ninguna página.
- El banco suma tres cargas que comparan los índices con los de Voron.

## Valores por defecto elegidos sin preguntar

Todas son decisiones conservadoras tomadas bajo S5 y están registradas en el documento de ejecución.

- **Validación de definiciones:**
  - Un nombre de columna de más de 1.024 bytes en una definición falla con tamaño no válido del nombre de columna,
    como en E17 para las filas y las proyecciones. IX10 solo nombra los nombres de tabla e índice (L6).
  - Las columnas incluidas se guardan como si admitieran nulos, y un valor incluido más grande que el máximo de una
    fila falla con el error de tamaño de la fila (L4).
- **Mantenimiento:**
  - Un pánico al borrar un intervalo impide confirmar la escritura solo si ya se borró alguna fila. Es la lectura
    literal de IX12 (L7).
  - Una entrada que ya existe al añadirla es un daño de su hoja (L7).
- **Recorridos:** una entrada mal formada se devuelve sin error, y leer un valor dañado termina en un pánico normal
  del hilo (IX12 y L2; L9).
- **Banco:**
  - En Voron, `ix_ts` cubre `ts` e `id`, para que los empates sigan el orden de la clave primaria.
  - W30 crea la tabla y sus índices antes del cronómetro (L11).
- **Ordenación:** un `krummi.sort` sobrante que no se puede borrar al empezar una ordenación hace fallar esa creación
  con la clase de acceso. Solo un fallo de entrada y salida al escribir sus archivos aplica T12 (L12).
- **API:** `index_rows` devuelve los tipos nuevos `ReadIndexRows` y `WriteIndexRows`, en vez de un parámetro de tipo
  público (L17).

## Pendiente (no bloqueante)

- **Comportamiento anterior a esta ejecución**, detectado por las revisiones y no cambiado:
  - La apertura borra `krummi.temp` antes de la última comprobación que puede hacerla fallar.
  - Las identidades de columna y el estado del esquema que sortean `create_table` y `change_schema` solo se
    comparan con valores locales.
  - La codificación de una fila al escribirla reserva memoria sin poder fallar.

  Ninguno lo introdujo este cambio, y corregirlos cambia comportamiento de 7.1. Quedan para una corrección aparte.
- **Archivos de ordenación sin código de integridad:** un encabezado dañado dentro de una corrida que todavía cabe
  se lee como dato. El PRD no pide verificar estos archivos y nunca se leen para recuperar.
- **Coste de algunas operaciones:**
  - `store_knows` recorre todo el catálogo en cada sorteo de identidad de índice. Es aceptable porque crear y
    borrar índices es raro.
  - La sonda de unicidad cuesta unas 3 veces una búsqueda simple en debug.
  - `IndexEntry::value(i)` recorre los elementos anteriores en cada llamada.

  Ninguno afecta a W30–W32, que cumplen sus umbrales.
- **Pruebas sin cubrir:**
  - Que los archivos de ordenación no cuentan para el tamaño máximo (P10) ni para la información del almacén (P12)
    se cumple por construcción, sin test propio. La sonda del verificador dio la misma información y el mismo error
    de tamaño máximo con archivos y sin ellos.
  - AC-IX15 reescribe la cabecera de un almacén del formato 12 en vez de usar uno real de 7.1. El verificador lo
    probó con un almacén real (L6).
- **Si un enlace o junction se llama `krummi.sort`, toda apertura falla con la clase de acceso**, sin seguir el
  destino. IX2 lo permite.
- **Corridas completas:** no se repitieron después de la corrección de W27. El A/B la mide, y W27 ya cumplía su
  umbral antes (Windows 1,00, Linux 1,08).

**Riesgo:**

- Ítem 1: el formato 12 es incompatible, y la apertura borra `krummi.sort`.
- Ítem 3: la API pública de errores y de índices, y el código compartido de escrituras, recorridos, apertura y
  esquema.
- Ítem 6: el arnés de Voron no tiene test de cargo, y no hay test de rendimiento.

Todos tuvieron su verificación: verificadores independientes, RDD con escalada y la revisión de Codex aprobada.

## Limpieza manual

- `work/ti/` (ignorado por git): `ab/` (3,5 GB) con las builds
  `base-target`, `head-target` y `fix-target`, los almacenes `ab-stores` y la copia `e3-src`. Se pueden borrar;
  conviene conservar los JSON y logs como evidencia del A/B.
- En WSL, `~/night/runs`, `~/night/src` y `~/night/target`, de la corrida de Linux.
