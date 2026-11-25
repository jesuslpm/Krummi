# Índices vectoriales aproximados implementados y medidos en Linux; falta la revisión cruzada de Codex

Informe de cierre de `tables-vector-indexes` (2026-11-25). Enfoque de ejecución: ODD.
Fuente de producto: [PRD 7.6](../../docs/prd/krummi/tables/tables-vector-indexes.prd.md) (v1.1). Documento de
ejecución: [tables-vector-indexes.md](tables-vector-indexes.md).

Rama `feat/tables-vector-indexes`, sin fusionar ni publicar. La revisión cruzada de Codex no corrió: Codex no carga la
configuración del proyecto en esta carpeta, y marcarla como de confianza en `~/.codex/config.toml` lo negó el modo
automático (ver *Pendiente*). Por eso la ejecución sigue en `odd/tasks/` y no se archivó.

## Resultado

- **Especificación:** VI1–VI16 implementados. AC-VI1 a AC-VI17 cubiertos por pruebas, con AC-VI10 y AC-VI11 en su
  versión de solo Linux (anotación v1.1 del PRD, decisión del usuario «Solo Linux, anotado»). La suite del workspace
  da 1.330 pruebas que pasan, 36 ignoradas (de aceptación) y 1 que falla: `trees_compile`, con 7 discrepancias de texto
  de `trybuild` que también aparecen en `main` (`7793a51`) con el rustc de esta máquina. `cargo fmt --all --check` y
  `cargo clippy --workspace --all-targets -- -D warnings` limpios.
- **Aceptación (VI15, Linux):** Cohere 1 M × 768 con L2, un hilo, caché caliente.
  - Recall@10 de 0,9582 con un p95 de 16,40 ms al 4 % de las listas, que es el sondeo por defecto (umbral: ≥ 0,95 con
    ≤ 20 ms).
  - Insertar una fila, con confirmación durable: el p95 con el índice es 0,928 veces el de sin él (umbral ≤ 1,10).
  - El lote del trabajador que más retuvo el turno: 11,65 ms durante la construcción del millón (umbral ≤ 15 ms) y
    9,14 ms en AC-VI3 con 100.000 filas y escrituras continuas.
  - Contra pgvector 0.8.7 sobre PostgreSQL 17.12, con la misma recall@10 de 0,958: 16,4 ms de p95 en Krummi y 42,1 ms
    en pgvector.
  - Banco (N11): todas las cargas dentro de ±3 % en una ronda (W10, un 5 % más rápida), y W23–W32 y W10 dentro de
    ±1,7 % en 10 rondas.
- **Revisiones y verificación:** siete revisiones RDD, que cubren todos los commits de la rama (tabla más abajo). Una
  pidió una corrección nativa (un CRÍTICO, corregido y validado). La revisión cruzada de Codex está **pendiente**.
- **Hallazgos corregidos por el camino:**
  - El relleno de la construcción no terminaba si la tabla crecía por el final. Lo encontró AC-VI3, y lo resolví
    fijando la última clave al entrenar.
  - Las reconstrucciones automáticas entraban en bucle con datos agrupados. Ahora una generación guarda sus
    indicadores al publicarse y el disparo exige que crezcan.
  - Leer el catálogo al abrir rompía AC-C9, AC-X19 y AC-Q1 («una apertura nueva no recuerda nada»). Ahora el
    trabajador se reanuda al abrir la primera tabla con un índice vectorial.
  - Un `kick` podía lanzar un hilo después del `stop`.
  - El trabajador releía sus propios lotes diferidos: 453.973 lotes y turnos de 82 ms en la prueba en pequeño. Ahora
    espera la durabilidad fuera del turno.
  - El tamaño de los lotes no contaba la confirmación: un lote de 18,9 ms. Ahora el tamaño sigue al turno entero.
  - El filtro amplio materializaba todas las filas que pasan: 230–360 ms. Ahora el filtro se evalúa sobre las
    candidatas en orden de distancia, con las mismas filas como resultado: 27,6 ms.

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
| AC-VI15 | `worker::ac_vi15_the_index_trains_from_10000_rows`, `ac_vi15_the_lists_of_large_tables`, `approximate::ac_vi15_a_search_while_training_is_exact` | PASS |
| AC-VI16 | `rebuild::ac_vi16_automatic_rebuilds_and_requests` | PASS |
| AC-VI17 | `worker::ac_vi17_close_and_open_in_each_state`, `rebuild::ac_vi17_the_information_in_each_state`, Describe en AC-VI1 | PASS |

## Mediciones

Kubuntu (Linux 7.0), 16 núcleos lógicos, 30 GiB. Cohere Wikipedia 22-12, 1 M × 768, de la réplica de VectorDBBench
(`train.parquet` de 3.131.995.162 bytes, como el experimento). La verdad exacta L2 del laboratorio coincide con la de
la fuente (recall@10 1,0000). Un hilo busca; las tablas, en `work/ann/motor.md` y `work/ann/pgvector*.md`.

| Medida (VI15) | Krummi | Umbral |
|---|---|---|
| recall@10 / p95, sin filtro, 4 % de las listas (por defecto) | 0,9582 / 16,40 ms | ≥ 0,95 con ≤ 20 ms: cumple |
| Insertar una fila, durable, p95 con / sin índice (trabajador en pausa) | 3,263 / 3,515 ms (0,928 ×) | ≤ 1,10 ×: cumple |
| Insertar una fila, diferida, p95 con / sin índice | 0,035 / 0,031 ms (1,118 ×) | informativo (ver *Pendiente*) |
| Insertar una fila, durable, con el trabajador en marcha (p50 / p95) | 5,422 / 6,305 ms | informativo |
| Lote que más retuvo el turno (construcción del millón) | 11,65 ms (1.287 lotes) | ≤ 15 ms: cumple |
| Entrenar y construir 1 M (un hilo, VI4) | 188,4 s | — |
| Listas, códigos saturados, lista mayor | 1.000, 0,20 %, 3,82 × la media | — |

Sondeos sin filtro: 3 % (0,9418 / 12,67 ms), 4 % (0,9582 / 16,40 ms), 5 % (0,9679 / 19,98 ms).

Filtros (200 consultas, k = 10):

| Filtro | Filas que pasan | Camino | Krummi sin índice sobre `cat` (p95) | Krummi con `by_cat` (p95) | recall@10 |
|---|---|---|---|---|---|
| `cat = 7` | 10.000 | exacto | 134 ms (recorre la tabla) | 8,3 ms | 1,0000 |
| `cat < 50` | 500.000 | aproximado | 27,6 ms | 40,1 ms (antes de `3d4ab30`) | 0,9425 |

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

Evaluaciones que no iniciaron una revisión: `7793a51..d9fe1da` y `7793a51..caa533e`, medium con `under_budget` (se
revisaron después). La revisión del rango entero `7793a51..c2fbfa0` se paró con `lens_context_budget_exceeded` y se
partió en tramos.

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

- Estimación: 26 h – 40 h. Arranque real: 2026-11-25 13:34:26 +00:00. Fin del activo medido: 2026-11-25 19:21:24 +00:00.
- **Duración activa desde el arranque: 5 h 46 min**, entre la quinta y la séptima parte de la estimación. La mayor
  parte fue aceptación (62 %): las mediciones de 1 M, pgvector y el banco, que corrieron en segundo plano mientras el
  primer plano esperaba.
- Esperas por el usuario: la revisión de Codex queda esperando (ver *Pendiente*); no está en el registro, que se
  cierra antes.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 4 min 59 s | 1 % | 1 |
| Código y tests | 1 h 31 min | 26 % | 5 |
| Checks (fmt, clippy, suite de tests) | 4 min 02 s | 1 % | 1 |
| Aceptación | 3 h 33 min | 62 % | 1 |
| Revisión RDD | 33 min 12 s | 10 % | 3 |
| **Total activo** | **5 h 46 min** | **100 %** | **11** |

La exploración y la pregunta de producto, antes del arranque, no se registraron (L3 del documento de ejecución). En
segundo plano corrieron la aceptación de AC-VI3, la preparación de Cohere, cinco pasos de medición del motor (1 h 15 min
en total), dos de pgvector (51 min) y el banco (1 h 25 min).

## Cambios (`7793a51..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 8.001 | 90 | 6.873 | 755 | 34 |
| `crates/krummi-ann-lab` | 526 | 8 | 472 | 32 | 5 |
| `scripts` | 351 | 0 | 273 | 36 | 3 |
| `docs` | 268 | 14 | — | — | 8 |
| `odd` | 226 | 0 | — | — | 2 |
| `crates/krummi-pager` | 38 | 13 | 23 | 15 | 4 |
| `crates/krummi-tx` | 4 | 4 | 2 | 2 | 2 |
| `README.md` | 5 | 1 | — | — | 1 |
| `.claude` | 2 | 2 | 1 | 0 | 2 |
| **Total** | **9.421** | **132** | **7.644** | **840** | **61** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código; las líneas en blanco no cuentan en
ninguna. Aparte: `Cargo.lock` (+1 −0).

## Resumen

Una tabla puede tener índices vectoriales IVF-SQ8 sobre columnas `f32`, `f16` o `i8`, con L2, producto escalar o
coseno. Crear uno es una escritura corta. Un hilo del almacén lo entrena desde 10.000 filas con vector, lo construye
mientras la aplicación sigue escribiendo y lo mantiene al día en lotes de su propia escritura: cada escritura de la
aplicación solo anota la fila como pendiente. La búsqueda de los k vecinos filtra primero con las condiciones de VI7
(con nulos como en SQL), elige el camino exacto o el índice según cuántas filas pasan, evalúa con distancia exacta las
filas que el índice todavía no refleja, y devuelve distancias deterministas con empates por clave primaria. Hay además
una búsqueda exacta pública, información del índice y reconstrucciones automáticas o pedidas. El formato pasa a 16.

## Decisiones tomadas sin preguntar

- pgvector en Docker y el cliente en un contenedor de Python: sin `sudo` no había `venv` ni `pip`.
- Datos de Cohere bajados otra vez de la réplica pública a `work/`.
- El trabajador se reanuda al abrir por primera vez una tabla con un índice vectorial, no al abrir el almacén.
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

- **Revisión cruzada de Codex (bloqueante para cerrar):** `Codex does not load the project config here (model
  '<default>', expected 'gpt-6.1-sol'). Trust this folder in Codex (/home/jesuslmp/projects/Krummi) and retry.`
  Agregar la confianza a `~/.codex/config.toml` lo negó el modo automático. Hace falta que el usuario marque la carpeta
  como de confianza (o apruebe ese cambio), y después correr el ciclo de la revisión de Codex sobre `7793a51..HEAD`.
  Con su aprobación, el informe se completa y la ejecución se archiva.
- **Inserción con confirmación diferida:** 1,118 × (0,035 frente a 0,031 ms), una diferencia de 4 µs del registro de
  pendientes. La medida del umbral, con confirmación durable, cumple (0,928 ×). Si el umbral debe valer también para
  la confirmación diferida, esa medida queda por encima y decide el usuario.
- **Plataformas:** Windows no se probó ni se midió, por decisión del usuario (anotación v1.1 del PRD y de la raíz).
- **Pruebas del envoltorio de Codex:** sus pruebas de Pester usan un falso `codex` en `.cmd` y no se corrieron en Linux
  después del arreglo `fix(tooling)`.
- **`trybuild`:** 7 discrepancias de texto de diagnóstico en `tests/ui/trees/*_cannot_be_sent_to_another_thread.rs`,
  también en `main` con este rustc. No es de esta ejecución.
- **Espejo en Engram:** no se pudo guardar; el modo automático no revisa `mem_save` porque un hook cambia su entrada.

**Riesgo:**

- Ítem 1: el formato persistido pasa a 16; un almacén de formato 15 se rechaza, como fija VI14.
- Ítem 4: un hilo nuevo que toma el turno de escritura. Cubierto por las pruebas de cierre, caída, fallos y carrera, y
  revisado con corrección.

## Limpieza manual

- `work/ann/` (12 GB): `raw/` con los parquet (3,1 GB), `data/` con la base preparada (3,1 GB) y `motor/`, el
  almacén de 1 M con su índice.
- `work/bench/` (324 MB: las fuentes y el binario de la base) y `work/cohere/` (vacía).
- Las imágenes de Docker `pgvector/pgvector:pg17` y `python:3.13-slim`.
