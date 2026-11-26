# Índices vectoriales aproximados implementados, medidos en Linux y aprobados por Codex

Informe de cierre de `tables-vector-indexes` (2026-11-25 a 2026-11-26). Enfoque de ejecución: ODD.
Fuente de producto: [PRD 7.6](../../docs/prd/krummi/tables/tables-vector-indexes.prd.md) (v1.2). Documento de
ejecución: [tables-vector-indexes.md](tables-vector-indexes.md).

Rama `feat/tables-vector-indexes`, sin fusionar ni publicar. La revisión cruzada de Codex aprobó en su cuarta ronda,
con sus seis hallazgos corregidos (*Revisión cruzada de Codex*), y todos los commits pasaron por RDD.

## Resultado

- **Especificación:** VI1–VI16 implementados. AC-VI1 a AC-VI17 cubiertos por pruebas, con AC-VI10 y AC-VI11 en su
  versión de solo Linux (anotación v1.1 del PRD, decisión del usuario «Solo Linux, anotado»). La suite del workspace,
  en `42d5743`, da 1.336 pruebas que pasan, 36 ignoradas (de aceptación) y 1 que falla: `trees_compile`, con 7
  discrepancias de texto de `trybuild` que también aparecen en `main` (`7793a51`) con el rustc de esta máquina.
  `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` limpios.
- **Decisiones del usuario tras la revisión de Codex** (anotación v1.2 del PRD):
  - El límite de VI15 para la inserción se mide con confirmaciones durables («Solo durables (Recomendado)»).
  - VI12 se cumple tal como está escrito: al abrir el almacén, el trabajador arranca y lee el catálogo en segundo
    plano («Reanudar al abrir», «Leer el catálogo al abrir»).
- **Aceptación (VI15, Linux):** Cohere 1 M × 768 con L2, un hilo, caché caliente.
  - Recall@10 de 0,9582 con un p95 de 16,40 ms al 4 % de las listas, que es el sondeo por defecto (umbral: ≥ 0,95 con
    ≤ 20 ms).
  - Insertar una fila, con confirmación durable: el p95 con el índice es 0,928 veces el de sin él (umbral ≤ 1,10).
  - El lote del trabajador que más retuvo el turno: 11,65 ms durante la construcción del millón (umbral ≤ 15 ms) y
    9,14 ms en AC-VI3 con 100.000 filas y escrituras continuas.
  - Contra pgvector 0.8.7 sobre PostgreSQL 17.12, con la misma recall@10 de 0,958: 16,4 ms de p95 en Krummi y 42,1 ms
    en pgvector. Con filtro, la comparación con recall ≥ 0,95 está en *Comparación con pgvector*.
  - Banco (N11): todas las cargas dentro de ±3 % en una ronda (W10, un 5 % más rápida), y W23–W32 y W10 dentro de
    ±1,7 % en 10 rondas.
- **Revisiones y verificación:** 19 revisiones RDD, que cubren todos los commits de la rama (tabla más abajo). Una
  pidió una corrección nativa (un CRÍTICO, corregido y validado). Cuatro rondas de Codex: la primera encontró R1–R5,
  la tercera R6; las seis están corregidas y verificadas por Codex.
- **Hallazgos corregidos por el camino:**
  - El relleno de la construcción no terminaba si la tabla crecía por el final. Lo encontró AC-VI3, y lo resolví
    fijando la última clave al entrenar.
  - Las reconstrucciones automáticas entraban en bucle con datos agrupados. Ahora una generación guarda sus
    indicadores al publicarse y el disparo exige que crezcan.
  - Un `kick` podía lanzar un hilo después del `stop`.
  - El trabajador releía sus propios lotes diferidos: 453.973 lotes y turnos de 82 ms en la prueba en pequeño. Ahora
    espera la durabilidad fuera del turno.
  - El tamaño de los lotes no contaba la confirmación: un lote de 18,9 ms. Ahora el tamaño sigue al turno entero.
  - El filtro amplio materializaba todas las filas que pasan: 230–360 ms. Ahora el filtro se evalúa sobre las
    candidatas en orden de distancia, con las mismas filas como resultado: 27,6 ms.
  - Reanudar al abrir (VI12) mostró pruebas que dependían de leer antes que el trabajador. Las que cuentan
    comprobaciones esperan su primera lectura (`open_settled`), y la ayuda que toma el motor (`with_engine_mut`) lo para
    antes.

## Cobertura de los criterios de aceptación

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-VI1 | `definitions::ac_vi1_create_every_admitted_index_and_refuse_the_invalid_ones`, `the_most_indexes_count_both_kinds` | PASS |
| AC-VI2 | `pending::ac_vi2_every_row_write_marks_the_rows_it_changes`, `approximate::ac_vi2_the_search_sees_the_pending_rows` | PASS |
| AC-VI3 | `worker::ac_vi3_the_index_follows_the_writes`; aceptación `worker::ac_vi3_acceptance_100000_rows_and_the_turn` (release) | PASS: 52 lotes, el más largo 9,14 ms |
| AC-VI4 | `crash::ac_vi4_a_crash_during_the_training_the_build_or_a_batch`, `crash::ac_vi4_power_cuts_across_a_build` | PASS |
| AC-VI5 | `approximate::ac_vi5_the_index_and_the_exact_search` | PASS |
| AC-VI6 | `approximate::ac_vi6_filter_first` (umbral fijado por gancho; equivalencia con todas las listas y todas las candidatas) | PASS |
| AC-VI7 | `approximate::ac_vi7_an_open_read_keeps_its_snapshot` | PASS |
| AC-VI8 | `rebuild::ac_vi8_a_requested_rebuild` | PASS |
| AC-VI9 | `definitions::ac_vi9_the_changes_of_the_table`, `pending::ac_vi9_the_bulk_load_marks_each_loaded_row`, `worker::emptying_and_dropping_while_the_worker_works` | PASS |
| AC-VI10 (Linux) | `approximate::ac_vi10_the_same_file_gives_the_same_results`, `distance_tests::f32_distances_have_the_pinned_bits_of_the_experiment` (bits tomados en win-x64 por el experimento) | PASS |
| AC-VI11 (Linux) | `krummi-ann-lab motor` (`work/ann/motor.md`), sección *Mediciones* | PASS: 0,9582 / 16,40 ms; 0,928 ×; 11,65 ms |
| AC-VI12 | `exact::ac_vi12_the_distances_and_their_order`, `distance_tests::*` | PASS |
| AC-VI13 | `exact::ac_vi13_the_filters_pass_the_rows_of_sql`, `approximate::ac_vi13_the_widened_probe_and_the_null_vectors`, `rebuild::ac_vi13_an_emptied_index_has_no_indicators` | PASS |
| AC-VI14 | `vector_fault_tests::ac_vi14_a_failed_batch_is_retried`, `ac_vi14_a_failed_commit_refuses_writes`, `definitions::ac_vi14_a_store_of_format_15_is_of_a_previous_format` | PASS |
| AC-VI15 | `worker::ac_vi15_the_index_trains_from_10000_rows`, `ac_vi15_the_lists_of_large_tables`, `approximate::ac_vi15_a_search_while_training_is_exact`; con confirmación diferida, `codex::r6_an_index_created_by_a_deferred_commit_trains` y `vector_fault_tests::a_vector_index_created_by_a_deferred_commit_trains` | PASS |
| AC-VI16 | `rebuild::ac_vi16_automatic_rebuilds_and_requests` | PASS |
| AC-VI17 | `worker::ac_vi17_close_and_open_in_each_state`, `worker::vi12_the_worker_resumes_when_the_store_opens`, `rebuild::ac_vi17_the_information_in_each_state`, Describe en AC-VI1 | PASS |

## Mediciones

Kubuntu (Linux 7.0), 16 núcleos lógicos, 30 GiB. Cohere Wikipedia 22-12, 1 M × 768, de la réplica de VectorDBBench
(`train.parquet` de 3.131.995.162 bytes, como el experimento). La verdad exacta L2 del laboratorio coincide con la de
la fuente (recall@10 1,0000). Un hilo busca; las tablas, en `work/ann/motor.md` y `work/ann/pgvector*.md`.

| Medida (VI15) | Krummi | Umbral |
|---|---|---|
| recall@10 / p95, sin filtro, 4 % de las listas (por defecto) | 0,9582 / 16,40 ms | ≥ 0,95 con ≤ 20 ms: cumple |
| Insertar una fila, durable, p95 con / sin índice (trabajador en pausa) | 3,263 / 3,515 ms (0,928 ×) | ≤ 1,10 ×: cumple |
| Insertar una fila, diferida, p95 con / sin índice | 0,035 / 0,031 ms (1,118 ×) | informativo (decisión del usuario) |
| Insertar una fila, durable, con el trabajador en marcha (p50 / p95) | 5,422 / 6,305 ms | informativo |
| Lote que más retuvo el turno (construcción del millón) | 11,65 ms (1.287 lotes) | ≤ 15 ms: cumple |
| Entrenar y construir 1 M (un hilo, VI4) | 188,4 s | — |
| Listas, códigos saturados, lista mayor | 1.000, 0,20 %, 3,82 × la media | — |

Sondeos sin filtro: 3 % (0,9418 / 12,67 ms), 4 % (0,9582 / 16,40 ms), 5 % (0,9679 / 19,98 ms).

Filtros (200 consultas, k = 10):

| Filtro | Filas que pasan | Camino | Krummi sin índice sobre `cat` (p95) | Krummi con `by_cat` (p95) | recall@10 |
|---|---|---|---|---|---|
| `cat = 7` | 10.000 | exacto | 134 ms (recorre la tabla) | 8,3 ms | 1,0000 |
| `cat < 50` | 500.000 | aproximado | 27,6 ms (4 %, por defecto); 31,49 ms (5 %, recall 0,9600) | 40,1 ms (antes de `3d4ab30`) | 0,9425 al 4 % |

Umbral de VI6: el camino exacto y el índice se cruzan entre 10.000 y 20.000 filas que pasan (9,0 frente a 10,4 ms,
18,0 frente a 15,7 ms), donde el índice da recall 0,82–0,86. Queda en 20.000.

### Comparación con pgvector (informativa)

PostgreSQL 17.12 y pgvector 0.8.7 en Docker, en la misma máquina. Un hilo (`max_parallel_workers_per_gather = 0`),
`ivfflat.iterative_scan = relaxed_order` e IVFFlat de 1.000 listas. Cliente en un contenedor `python:3.13-slim`
(`scripts/bench/pgvector_compare.sh`).

| Medida | Krummi | pgvector |
|---|---|---|
| p95 con recall@10 ≈ 0,958 | 16,40 ms (40 sondeos) | 42,10 ms (30 sondeos) |
| p95 con recall@10 ≈ 0,97 | 19,98 ms (0,9679) | 54,19 ms (0,9706) |
| Insertar una fila, p95 con / sin índice | 3,263 / 3,515 ms | 3,676 / 3,065 ms (1,199 ×); en otra corrida, 3,623 / 3,320 ms (1,091 ×) |
| `cat = 7`, p95 con recall@10 ≥ 0,95 | 134 ms sin índice sobre `cat` y 8,3 ms con `by_cat` (exacto, 1,0) | 134,59 ms (IVFFlat, 120 sondeos, 0,9765); 41 ms con su recorrido exacto (1,0) |
| `cat < 50`, p95 con recall@10 ≥ 0,95 | 31,49 ms (50 sondeos, 0,9600) | 44,32 ms (30 sondeos, 0,9515); 54,50 ms (40 sondeos, 0,9685) |
| Crear el índice | 188,4 s (un hilo) | 47,5 s (trabajadores de mantenimiento por defecto) |
| `SELECT 1`, p95 | — | 0,026 ms |

Con filtro, cada fila de pgvector se planificó con su propio vector y usa IVFFlat con el filtro sobre sus candidatos.
Los planes de `EXPLAIN (ANALYZE)` están en `work/ann/pgvector-filters-r4b.md`, por ejemplo
`Index Scan using near on docs`, `Filter: (cat < 50)`, `Rows Removed by Filter: 11`. Una primera medición daba
recall 1,0 con 350 ms para `cat < 50`, porque psycopg prepara la sentencia tras cinco ejecuciones y el plan genérico
de PostgreSQL recorre la tabla. De esa medición sale el recorrido exacto de `cat = 7` (41 ms). Sin el índice
`by_cat`, el camino exacto de Krummi para `cat = 7` (134 ms) es más lento que ese recorrido de PostgreSQL.

### Perfil del camino exacto con filtro

A petición del usuario se perfiló `cat = 7` sin índice sobre `cat` (L30 del documento de ejecución; `perf` con
punteros de marco y samply 0.13.1, perfiles en `work/prof/`). Son unas 1.500 instrucciones por fila, con 1,9
instrucciones por ciclo y 2,3 fallos de caché por fila: trabajo de CPU, no ancho de banda. Reparto inclusivo:
decodificar la fila 35 %, recorrer las hojas 35 %, `Filter::meets` 16 % (unos 9 puntos para codificar el valor como
clave), `TableRow::value` 16 % y distancias 3,6 %. El recorrido interno de `passing` no usa proyecciones. Un recorrido
de `range_rows` que lee `cat` con una proyección costó entre un 25 y un 45 % menos que leyéndolo por nombre, pero esas
cifras variaron mucho entre procesos. El trabajo de optimizar los recorridos de tabla se dejó preparado para otra
rama, en otra máquina (`work/handoff/perf-scan-prompt.md`), y no entra en esta ejecución.

### Banco (N11)

Krummi solo, escala completa, una carga por proceso (`scripts/bench/ab_loads.py`), base `7793a51` contra la rama
(`3d4ab30`).

- Una ronda de todas las cargas: todas dentro de ±3 %, salvo W10, un 5,0 % más rápida en la rama.
- 10 rondas de W23–W32 y W10: todas dentro de ±1,7 %. Razón de op/s de la rama sobre la base: W10 1,003, W23 1,000,
  W24 1,009, W25 1,015, W26 0,995, W27 1,017, W28 1,003, W29 1,003, W30 1,008, W31 0,992 y W32 1,004.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `7793a51..caa533e` | riesgo, fiabilidad | aprobada | ninguno | `review-65892a7ace81cb36` |
| `caa533e..c2fbfa0` (escalada por el ítem 1, formato 16) | riesgo, fiabilidad | aprobada | 2 WARNING: desplazamiento de la prueba de la condición y falta de pruebas de la sección vectorial; corregidos en `9d9add9` | `review-7fa2180227cd5afb` |
| `c2fbfa0..15b67d3` | fiabilidad | aprobada | 2 SUGGESTION: camino de mezcla (descartado con evidencia: el parche nunca cambia un vector) y segundo índice en el borrado por rango (corregido) | `review-84a601c123f2e5d1` |
| `15b67d3..8ee9045` (trabajador) | riesgo, resiliencia, fiabilidad | corrección, luego aprobada | CRÍTICO: despertar perdido en `stop`, corregido en `597fee2` y validado. WARNING corregidos en `308516e`: fallos aislados por índice, espera propia del reintento, caché acotada, raíces de cada instantánea, pendientes antes del entrenamiento, pruebas AC-VI4 y AC-VI14. Descartado: formato del registro de estado (no existe fuera de la rama) | `review-dd7a80cc74645096` |
| `597fee2..6b06454` | riesgo, fiabilidad | aprobada | WARNING: sincronización con `sleep`; corregido en `57c56c7` | `review-051f2d94bfafcfca` |
| `6b06454..57c56c7` | riesgo, resiliencia, fiabilidad | aprobada | 2 WARNING de reintento sin fin y 1 SUGGESTION de caché, descartados con argumento (VI13; `f32` de 4 bytes) | `review-40e5fb82a059637f` |
| `57c56c7..587907e` | los cuatro | aprobada | 4 WARNING y 1 SUGGESTION, corregidos en el commit siguiente | `review-28b9280c3ad6ab2c` |
| el commit de esas correcciones | fiabilidad, legibilidad | aprobada | ninguno | `review-f185432df41a3778` |
| `ccc2564..7c61c20` (envoltorio de Codex e informe) | fiabilidad, legibilidad | aprobada | 2 WARNING (`$IsWindows` en PowerShell 5.1) descartados con evidencia: el script exige la versión 7; 1 SUGGESTION en *Pendiente* | `review-99d04abda3d18e6a` |
| `7c61c20..7ce3e50` | legibilidad | aprobada | ninguno | `review-584893515d187337` |
| `7ce3e50..3d170ff` (R1, R3, R2, R5 de Codex) | riesgo, resiliencia, fiabilidad (escalada por el ítem 1) | aprobada | WARNING de coste por paso, corregido en `08a238f`; WARNING de crecimiento de los cubos, descartado con evidencia (256 claves como mucho por cubo) | `review-7f50fd181d458674` |
| `3d170ff..08a238f` | resiliencia, fiabilidad | aprobada | ninguno | `review-0b0f5e0aba2d3f55` |
| `08a238f..ff2d002` (R4, VI12) | resiliencia, fiabilidad, legibilidad (escalada por el ítem 4) | aprobada | 2 WARNING (comentarios fuera de sitio), corregidos en `a4f44e3`; 2 SUGGESTION sobre el límite de `open_settled`, mantenido y documentado | `review-c7dc7fc497f81fbc` |
| `a4f44e3` | legibilidad | aprobada | ninguno | `review-8f7f5516684179e8` |
| `a4f44e3..a02a079` (R6) | resiliencia, fiabilidad (escalada por el ítem 4) | aprobada | SUGGESTION: la reproducción depende del tiempo, respondida con la prueba siguiente | `review-a5555188ebeab550` |
| `f55af4a` | fiabilidad | aprobada | WARNING (pausa fija) y SUGGESTION (reanudar el diario si falla), corregidos en `7def9ed` | `review-40fb50c81e161b2c` |
| `7def9ed` | fiabilidad | aprobada | WARNING (esperar solo el aviso), corregido en `42d5743` | `review-c6b21ad7ac18a581` |
| `42d5743` | fiabilidad | aprobada | ninguno | `review-f607e4cba75a6ce1` |
| `42d5743..314a37f` (registros de revisión) | legibilidad | aprobada | ninguno | `review-9c7eb5c051d3dcea` |

Evaluaciones que no iniciaron una revisión: `7793a51..d9fe1da` y `7793a51..caa533e`, medium con `under_budget` (se
revisaron después), y `08a238f..76d8820`, también `under_budget`, revisado dentro de `08a238f..ff2d002`. La revisión del
rango entero `7793a51..c2fbfa0` se paró con `lens_context_budget_exceeded` y se partió en tramos.

## Revisión cruzada de Codex

| Ronda | Informe | Alcance | Resultado | Hallazgos |
|---|---|---|---|---|
| 1 | [2026-11-25-code.md](../../docs/reviews/tables-vector-indexes/2026-11-25-code.md) | `7793a51..daaacd1` | `findings` | R1 y R2 (bloqueantes), R3 y R4 (mayores), R5 (menor), y dos preguntas de producto |
| 2 | [2026-11-25-code-02.md](../../docs/reviews/tables-vector-indexes/2026-11-25-code-02.md) | `daaacd1..5eaa9ba` | `approved` | R1–R5 verificados |
| 3 | [2026-11-26-code.md](../../docs/reviews/tables-vector-indexes/2026-11-26-code.md) | `5eaa9ba..aeed1a0` | `findings` | R6 (mayor), causado por el cambio de VI12 |
| 4 | [2026-11-26-code-02.md](../../docs/reviews/tables-vector-indexes/2026-11-26-code-02.md) | `aeed1a0..c9d4fa8` | `approved` | R6 verificado |

Resoluciones: [2026-11-25-code-resolutions.md](../../docs/reviews/tables-vector-indexes/2026-11-25-code-resolutions.md).

- **R1:** quitar otra columna mientras el trabajador trabajaba borraba entradas válidas. Lee el vector por la
  identidad de su columna en cada instantánea (`732cd88`).
- **R2:** una clave primaria de más de 2.039 bytes hacía entrar en pánico al trabajador. Las claves largas reciben un
  número propio en el árbol de datos (`52fe704`).
- **R3:** las filas sin vector podían dejar una reconstrucción activa para siempre. Ahora cuentan solo las filas con
  vector (`7f7a39c`).
- **R4:** la comparación filtrada con pgvector no tenía la misma recall. Se midió de nuevo, con un plan por consulta
  y los planes registrados (`abc006f`).
- **R5:** `motor` ignoraba `--conjunto`. Ahora rechaza todo lo que no sea cohere (`a21f742`).
- **R6:** una creación diferida podía quedarse en entrenamiento. El trabajador espera a que sean durables las
  confirmaciones que lo despertaron (`02e3ccb`).

## Reutilización

- **Núcleos.** Los del laboratorio (`krummi-ann-lab/src/kernels.rs`, `sq8.rs`, `ivf.rs`) entraron en
  `table/distance.rs` e `ivf.rs`, con el orden fijo de `vector_norm`. Se ampliaron a `f16`, `i8` y `bit`.
- **Filtros.** Reutilizan `stats_column`, `element_column` y `normalize` de 7.3, e `index_range` y `split_entry` de 7.2
  para recorrer un índice secundario.
- **Trabajador.** Sigue el modelo del hilo de estadísticas: `current_name`, `find_write_table`, `open_write_unless` y
  la adopción del contexto de fallos.
- **Mantenimiento.** El registro de pendientes cuelga de `Maintained` y de los caminos de escritura de filas existentes.
- **Medición.** El laboratorio prepara los datos y la verdad exacta, y su subcomando nuevo `motor` mide el motor.
- **Sin reutilizar, a propósito.** La caché de generaciones no comparte código con `StatsMemory`: sus claves y su
  política de vaciado son distintas.

## Tiempo

- Estimación: 26 h – 40 h. Arranque real: 2026-11-25 13:34:26 +00:00. Fin del activo medido: 2026-11-26 09:22:12 +00:00.
- **Duración activa desde el arranque: 10 h 28 min**, entre la cuarta y las dos quintas partes de la estimación. Lo
  que más tiempo llevó fue la aceptación (34 %): las mediciones de 1 M, pgvector y el banco, en segundo plano. Le
  siguen el código (33 %) y las revisiones (25 %), que incluyen las correcciones de los hallazgos de Codex.
- Esperas por el usuario: 9 h 18 min, fuera del activo. Casi todas (8 h 59 min) entre la primera pregunta de
  producto y su respuesta.
- Lo que el registro de fases no separa:
  - Las correcciones de Codex quedaron marcadas como revisión, no como correcciones.
  - El perfilado que pidió el usuario y la preparación del traspaso a Windows quedaron dentro de *Código y tests*,
    entre las 06:44 y la siguiente marca.
  - La primera medición de pgvector para R4 (unos 20:20–21:05 según su registro) no tiene su tramo de segundo plano,
    porque `bg-start` y `bg-end` fallaron por un argumento mal pasado.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 1 min 53 s | 0 % | 1 |
| Plan (documento de ejecución) | 4 min 59 s | 1 % | 1 |
| Código y tests | 3 h 28 min | 33 % | 6 |
| Checks (fmt, clippy, suite de tests) | 39 min 04 s | 6 % | 2 |
| Aceptación | 3 h 33 min | 34 % | 1 |
| Revisión RDD | 2 h 39 min | 25 % | 5 |
| Correcciones de hallazgos | 0 min 00 s | 0 % | 1 |
| Cierre (informe y merge) | 1 min 41 s | 0 % | 2 |
| **Total activo** | **10 h 28 min** | **100 %** | **19** |

- Inicio: 2026-11-25 13:34:26 +00:00. Fin: 2026-11-26 09:22:12 +00:00.
- Arranque confirmado: 13:34:26. Activo desde el arranque: **10 h 28 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 9 h 18 min.
  - 19:22:52–19:40:05 (17 min 13 s): Codex trust of the folder
  - 21:41:15–06:40:17 (8 h 59 min): product question 1
  - 06:42:10–06:44:54 (2 min 44 s): VI12 design question

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | AC-VI3 100k release | 15:37:35 | 15:37:46 | 0 min 11 s |
| Aceptación | Cohere prepare and truth | 15:41:09 | 15:43:37 | 2 min 28 s |
| Aceptación | VI15 load 1M | 15:51:01 | 15:54:49 | 3 min 48 s |
| Aceptación | VI15 search 1M | 15:55:10 | 16:15:01 | 19 min 51 s |
| Aceptación | VI15 index, threshold, search | 16:16:24 | 16:30:37 | 14 min 13 s |
| Aceptación | VI15 search lazy filter | 16:33:31 | 16:44:05 | 10 min 34 s |
| Aceptación | VI15 search windows | 16:45:21 | 16:51:27 | 6 min 06 s |
| Aceptación | pgvector 1M | 16:53:09 | 17:20:50 | 27 min 41 s |
| Aceptación | pgvector filters forced | 17:21:34 | 17:45:14 | 23 min 40 s |
| Aceptación | bench A/B 1 round | 17:46:28 | 18:05:11 | 18 min 43 s |
| Aceptación | bench A/B 10 rounds | 18:06:12 | 19:12:53 | 1 h 06 min |
| Revisión RDD | codex-R1 | 19:19:43 | 19:19:43 | 0 min 00 s |
| Revisión RDD | codex-R1 | 19:20:37 | 19:20:39 | 0 min 02 s |
| Revisión RDD | codex-R1 | 19:40:05 | 19:51:37 | 11 min 32 s |
| Aceptación | pgvector-R4 | 21:11:05 | 21:29:18 | 18 min 13 s |
| Revisión RDD | codex-R2 | 21:30:43 | 21:40:58 | 10 min 15 s |
| Revisión RDD | codex-R3 | 08:42:05 | 08:56:09 | 14 min 04 s |
| Revisión RDD | codex-R4 | 09:10:11 | 09:21:38 | 11 min 27 s |
| **Aceptación (total)** | | | | **3 h 32 min** |
| **Revisión RDD (total)** | | | | **47 min 20 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `69918a0` | fix(tooling): make the worktree guard hook POSIX so it runs under dash |  | 4 min 49 s | Plan (documento de ejecución) 4 min 49 s |
| `d9fe1da` | docs(prd): validate 7.6 on Linux only and open its execution document |  | 0 min 00 s |  |
| `caa533e` | feat(tables): add the distance kernels, the search filters and the exact nearest search | T2 | 9 min 02 s | Plan (documento de ejecución) 0 min 10 s, Código y tests 8 min 52 s |
| `c2fbfa0` | feat(tables): define vector indexes in the catalog (format 16) | T2, T4 | 13 min 02 s | Código y tests 13 min 02 s |
| `9d9add9` | test(tables): pin the vector index section of the catalog encoding | T5, T4 | 5 min 57 s | Código y tests 0 min 27 s, Revisión RDD 5 min 30 s |
| `15b67d3` | feat(tables): mark the rows each write changes as pending for the vector indexes | T5 | 4 min 04 s | Código y tests 4 min 04 s |
| `8ee9045` | feat(tables): train, build and maintain the vector indexes in a background worker | T6 | 1 h 03 min | Código y tests 1 h 03 min |
| `597fee2` | fix(tables): order the vector worker's stop after its waiters' checks | T6 | 7 min 55 s | Código y tests 1 min 36 s, Revisión RDD 6 min 19 s |
| `9986904` | feat(tables): search the nearest rows through a vector index, filter first | T6 | 10 min 59 s | Revisión RDD 10 min 59 s |
| `6b06454` | test(tables): cover vector index rebuilds and their information |  | 0 min 00 s |  |
| `308516e` | fix(tables): isolate vector worker failures, bound its cache and read fresh roots | T6 | 4 min 37 s | Revisión RDD 4 min 37 s |
| `57c56c7` | test(tables): synchronize the vector worker tests on its held points | T6 | 1 min 26 s | Revisión RDD 1 min 26 s |
| `3a9fcae` | docs(odd): record the vector index reviews and decisions | T6 | 0 min 22 s | Revisión RDD 0 min 22 s |
| `b21f9ce` | fix(tables): let the vector worker read its own batches and size them by the whole turn | T9 | 11 min 07 s | Aceptación 11 min 07 s |
| `4d82143` | feat(ann-lab): measure the vector indexes of the engine with the Cohere data |  | 0 min 00 s |  |
| `2fad948` | perf(tables): check the filter on the index candidates in distance order and probe 4 % of the lists | T9 | 42 min 32 s | Aceptación 42 min 32 s |
| `0cd926c` | feat(ann-lab): measure the engine's filters with and without a secondary index |  | 0 min 00 s |  |
| `3d4ab30` | perf(tables): sort only the index candidates a search reaches and borrow their keys | T9 | 18 min 10 s | Aceptación 18 min 10 s |
| `96bb348` | feat(bench): compare the vector indexes with pgvector and A/B single bench loads | T9 | 1 min 38 s | Aceptación 1 min 38 s |
| `587907e` | docs: add the vector search guide and annotate design document 12 | T9, T10 | 2 h 24 min | Checks (fmt, clippy, suite de tests) 4 min 02 s, Aceptación 2 h 20 min |
| `ccc2564` | fix(tables): read the pending rows of a search once and clarify the worker's durable wait | T11 | 1 min 54 s | Revisión RDD 1 min 54 s |
| `fe459aa` | docs(odd): record the vector index measurements and the last reviews | T11 | 0 min 24 s | Revisión RDD 0 min 24 s |
| `6e3d022` | fix(tooling): find the Codex CLI without an extension outside Windows | T11 | 0 min 54 s | Revisión RDD 0 min 54 s |
| `4e46310` | docs: add the vector indexes closing report, pending the Codex review | T11 | 2 min 15 s | Revisión RDD 0 min 47 s, Cierre (informe y merge) 1 min 28 s |
| `7c61c20` | docs: correct the sizes of the work folders in the closing report |  | 0 min 00 s |  |
| `7ce3e50` | docs(tooling): explain the Codex CLI lookup outside Windows |  | 0 min 00 s |  |
| `daaacd1` | docs(odd): record the trust of the folder in Codex |  | 0 min 00 s |  |
| `732cd88` | fix(tables): read the indexed vector by its column's identity in each snapshot | T11 | 13 min 53 s | Revisión RDD 13 min 53 s |
| `7f7a39c` | fix(tables): decide automatic rebuilds by the rows with a vector and end a rebuild that cannot train | T11 | 2 min 52 s | Revisión RDD 2 min 52 s |
| `52fe704` | fix(tables): give long primary keys a number in the vector index so the worker never exceeds the key limit | T11 | 23 min 33 s | Revisión RDD 23 min 33 s |
| `a21f742` | fix(ann-lab): reject a dataset other than cohere in the motor command |  | 0 min 00 s |  |
| `3d170ff` | docs(reviews): record the Codex code review of the vector indexes | T11 | 1 min 31 s | Revisión RDD 1 min 31 s |
| `08a238f` | perf(tables): count the rows with a vector only when a rebuild indicator passes its threshold | T11 | 1 min 58 s | Revisión RDD 1 min 58 s |
| `abc006f` | fix(bench): plan each pgvector search with its own vector and record the filtered plans | T11, checks after Codex R1 fixes | 1 h 06 min | Checks (fmt, clippy, suite de tests) 24 min 15 s, Revisión RDD 42 min 21 s |
| `5eaa9ba` | docs(reviews): record the resolutions of the Codex code review of the vector indexes |  | 0 min 00 s |  |
| `76d8820` | docs(reviews): record the approving Codex follow-up of the vector indexes | checks after Codex R1 fixes | 10 min 43 s | Checks (fmt, clippy, suite de tests) 10 min 43 s |
| `433843a` | fix(tables): resume the vector index worker when the store opens | checks after Codex R1 fixes, VI12 resume at open | 1 h 57 min | Exploración y mapa 1 min 53 s, Código y tests 1 h 55 min, Checks (fmt, clippy, suite de tests) 0 min 04 s |
| `ff2d002` | docs(prd): record the decisions on VI15 and VI12 after the Codex review |  | 0 min 00 s |  |
| `a4f44e3` | docs(tables): put two doc comments back on their items | VI12 resume at open | 1 min 14 s | Código y tests 1 min 14 s |
| `aeed1a0` | docs(odd): record the VI12 change and its reviews | VI12 resume at open | 0 min 16 s | Código y tests 0 min 16 s |
| `02e3ccb` | fix(tables): wait for the commits that woke the vector worker to be durable before reading | Codex R3 | 19 min 45 s | Revisión RDD 19 min 45 s |
| `a02a079` | docs(reviews): record the third Codex review of the vector indexes |  | 0 min 00 s |  |
| `f55af4a` | test(tables): pin with the journal stopped that a deferred creation of a vector index trains | Codex R3 | 2 min 02 s | Revisión RDD 2 min 02 s |
| `7def9ed` | test(tables): let the deferred creation test go on once the worker took its wake-up | Codex R3 | 1 min 06 s | Revisión RDD 1 min 06 s |
| `42d5743` | test(tables): keep the journal stopped a while after the worker took its wake-up | Codex R3 | 0 min 36 s | Revisión RDD 0 min 36 s |
| `c9d4fa8` | docs(reviews): record the resolution of R6 | Codex R3 | 4 min 29 s | Revisión RDD 4 min 29 s |
| `314a37f` | docs(reviews): record the approving fourth Codex review of the vector indexes | Codex R3 | 12 min 02 s | Revisión RDD 12 min 02 s |
| — | después del último commit | closing report | 0 min 13 s | Cierre (informe y merge) 0 min 13 s |
| **Total** | | | **10 h 28 min** | |

## Cambios (`7793a51..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 8.649 | 144 | 7.430 | 818 | 42 |
| `docs` | 1.316 | 14 | — | — | 13 |
| `crates/krummi-ann-lab` | 555 | 8 | 501 | 32 | 5 |
| `odd` | 537 | 0 | — | — | 3 |
| `scripts` | 373 | 0 | 287 | 42 | 3 |
| `crates/krummi-pager` | 38 | 13 | 23 | 15 | 4 |
| `crates/krummi-tx` | 4 | 4 | 2 | 2 | 2 |
| `.claude` | 4 | 2 | 1 | 2 | 2 |
| `README.md` | 5 | 1 | — | — | 1 |
| **Total** | **11.481** | **186** | **8.244** | **911** | **75** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+1 −0)

## Resumen

Una tabla puede tener índices vectoriales IVF-SQ8 sobre columnas `f32`, `f16` o `i8`, con L2, producto escalar o
coseno. Crear uno es una escritura corta. Un hilo del almacén lo entrena desde 10.000 filas con vector, lo construye
mientras la aplicación sigue escribiendo y lo mantiene al día en lotes de su propia escritura: cada escritura de la
aplicación solo anota la fila como pendiente. La búsqueda de los k vecinos filtra primero con las condiciones de VI7
(con nulos como en SQL), elige el camino exacto o el índice según cuántas filas pasan, evalúa con distancia exacta las
filas que el índice todavía no refleja, y devuelve distancias deterministas con empates por clave primaria. Hay además
una búsqueda exacta pública, información del índice y reconstrucciones automáticas o pedidas. Al abrir el almacén, el
trabajador arranca y sigue con el trabajo que quedó al cerrarlo. El formato pasa a 16.

## Decisiones tomadas sin preguntar

- pgvector en Docker y el cliente en un contenedor de Python: sin `sudo` no había `venv` ni `pip`.
- Datos de Cohere bajados otra vez de la réplica pública a `work/`.
- Una clave primaria de más de 2.039 bytes recibe un número en el árbol de datos del índice (R2). El número de una
  clave cuya fila se borró se queda hasta que se elimina el índice o se vacía la tabla.
- Las pruebas que cuentan comprobaciones esperan como mucho 2 s la primera lectura del trabajador al abrir.
- Con 20.000 filas el costo es parejo y el exacto da recall 1,0, así que el umbral de VI6 se mantiene en 20.000.
- Sondeo por defecto del 4 % de las listas.
- Hasta 2 × k candidatas reordenadas, al menos 20.
- Margen del cuantizador: 2 % del rango muestreado.
- Lotes con objetivo de 5 ms de turno y como mucho el doble del anterior.
- Umbrales de reconstrucción automática: 10 % de códigos saturados, lista mayor 12 × la media, filas 2 × o ½ × con
  otro número de listas, y crecimiento de 1,5 × sobre la base publicada.
- La medida de inserción del umbral es la confirmación durable con el trabajador en pausa («sin pendientes y el
  trabajador sin trabajo»).

## Pendiente

- **Plataformas:** Windows no se probó ni se midió, por decisión del usuario (anotación v1.1 del PRD y de la raíz).
- **Recorridos de tabla:** con un filtro selectivo y sin índice sobre su columna, el camino exacto de Krummi (134 ms)
  es más lento que el recorrido exacto de PostgreSQL (41 ms). Optimizar los recorridos queda para otra rama
  (*Perfil del camino exacto con filtro*).
- **Pruebas del envoltorio de Codex:** sus pruebas de Pester usan un falso `codex` en `.cmd` y no se corrieron en Linux
  después del arreglo `fix(tooling)`.
- **`trybuild`:** 7 discrepancias de texto de diagnóstico en `tests/ui/trees/*_cannot_be_sent_to_another_thread.rs`,
  también en `main` con este rustc. No es de esta ejecución.
- **Prueba intermitente:** `krummi-tx` `group_commit::ac_e13_failures_of_the_journals_writer` falló 1 de 20 veces en la
  rama y 1 de 40 en una copia de la base. No es de esta ejecución.
- **`open_settled`:** espera 2 s como mucho y no comprueba si se le acabó el tiempo, porque no puede distinguir un
  trabajador lento de uno que reintenta un catálogo dañado.
- **Espejo en Engram:** no se pudo guardar; el modo automático no revisa `mem_save` porque un hook cambia su entrada.

**Riesgo:**

- Ítem 1: el formato persistido pasa a 16; un almacén de formato 15 se rechaza, como fija VI14.
- Ítem 4: un hilo nuevo que toma el turno de escritura, arranca en cada apertura del almacén y lee el catálogo en
  segundo plano. Tras cada aviso espera a que las confirmaciones sean durables. Cubierto por las pruebas de cierre,
  caída, fallos y carrera, y revisado con corrección.

## Limpieza manual

- `work/ann/` (12 GB): `raw/` con los parquet (3,1 GB), `data/` con la base preparada (3,1 GB) y `motor/`, el
  almacén de 1 M con su índice.
- `work/bench/` (324 MB: las fuentes y el binario de la base) y `work/cohere/` (vacía).
- Las imágenes de Docker `pgvector/pgvector:pg17` y `python:3.13-slim`.
- Del perfilado: `work/prof/` (3,3 MB), `work/prof-target/` y `work/prof-target-fp/` (992 MB cada una), y samply en
  `~/.cargo/bin`.
- Del traspaso a Windows: `work/handoff/` y `work/transfer/` (8,4 MB cada una).
