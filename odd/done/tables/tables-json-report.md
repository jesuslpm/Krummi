# Columnas JSON de las tablas (7.5): implementadas y verificadas, sin fusionar

Informe de cierre de `tables-json` (2026-11-21). Enfoque de ejecución: ODD.
Fuente de producto: [tables-json.prd.md](../../../docs/prd/krummi/tables/tables-json.prd.md). Documento de ejecución:
[tables-json.md](tables-json.md).

Rama `feat/tables-json`, sin fusionar en `main` y sin publicar. En la rama hay también commits de otra sesión con el
documento de diseño 15 (perfil de consultas); no son de esta ejecución y quedan aparte en las cifras de cambios.

## Resultado

- **Especificación:** JS1–JS14 y AC-JS1–AC-JS14 de 7.5. Un tipo de columna `json` que valida y normaliza el
  documento al escribirlo, lo devuelve entero sin copia, lo navega (propiedades, elementos, puntero RFC 6901) y lee
  sus valores como texto, booleano, `i64`, `u64`, `decimal`, `f64`, fecha e instante; sus errores llevan la causa, la
  posición y el puntero; el formato pasa a 14. Pruebas nuevas: 20 del lector (`json_reader`), 32 unitarias del módulo
  (`table::json`), 14 de tablas (`table_json`); la suite entera pasa (ver *Verificación*).
- **Aceptación:** AC-JS11 (9), fuzzing en Linux una hora por objetivo: `json_normalize` 13.314.383 ejecuciones en
  3.601 s y `json_navigate` 2.447.585 en 3.689 s, sin fallos ni artefactos. JS13 y AC-JS14: medido en Windows y en
  Linux (sección *Mediciones*). Validar al escribir cuesta entre 1,08 y 2,11 veces la escritura del mismo texto como
  `string`; leer el documento entero, 1,02–1,05 (1,46 con 64 KB en Linux); la última propiedad de un documento de
  64 KB, 2,85 en Windows y 8,05 en Linux. En el A/B no hay una regresión que se repita en los dos sistemas.
- **Revisiones y verificación:** un verificador independiente sobre todo el documento, sin bloqueos; la revisión de
  Codex del rango, sin hallazgos sobre JSON (su único hallazgo es del documento 15 de la otra sesión y queda para tu
  decisión); RDD aprobó el último tramo, después de T4, sin bloqueos, y no estuvo disponible para T1–T4 (sección
  *Revisiones RDD*).
- **Defectos encontrados por el camino:** la igualdad de `Json` comparaba una marca interna y un documento leído no
  era igual a uno dado con el mismo texto (aviso del verificador, `e302ea9`); el objetivo de fuzzing de navegación no
  compilaba (`94a5150`); en T3, el desbordamiento de `i64` al acumular los dígitos de un exponente enorme antes de
  acotarlo, encontrado por sus pruebas antes del commit; en T2, un mutante del desempate de claves repetidas que
  sobrevivía, matado con una prueba nueva.

## Cobertura de los criterios de aceptación

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-JS1 | `table_json::round_trip::ac_js1_the_corpus_round_trips_through_a_table`, `..._reads_the_same_after_closing_and_reopening` | PASS |
| AC-JS2 | `json_reader::ac_js2_examples_are_read_as_their_normalized_text`, `ac_js2_an_indented_document_of_about_4_kb_gives_its_compact_text` | PASS |
| AC-JS3 | `json_reader::ac_js3_every_text_of_the_table_fails_with_its_cause_position_and_pointer` (22 filas, 27 textos); `table_json::writes::ac_js3_each_fault_fails_every_write_with_its_data`, `ac_js3_the_wrong_type_and_the_null_not_allowed` | PASS |
| AC-JS4 | `table_json::round_trip::ac_js4_the_column_null_is_not_the_document_null`; `nav::tests::ac_js4_null_property_and_missing_one` | PASS |
| AC-JS5 | `nav::tests::ac_js5_*` (5 pruebas) | PASS |
| AC-JS6 | `number::tests::ac_js6_*`, `time::tests::ac_js6_*`, `js7_*` | PASS |
| AC-JS7 | `table_json::reads::ac_js7_reads_point_into_the_row_and_the_index_entry`, `ac_js7_a_json_column_is_prepared_only_as_json` | PASS |
| AC-JS8 | `table_json::schema::ac_js8_json_columns_in_keys_indexes_and_type_changes`, `ac_js8_json_defaults` | PASS |
| AC-JS9 | `table_json::writes::ac_js9_an_update_with_the_same_normalized_text_is_identical` | PASS |
| AC-JS10 | `table_json::writes::ac_js10_the_longest_document` (N un byte por debajo del de `string`) | PASS |
| AC-JS11 (1)–(8) | `json_reader::corpus`, `nav::tests::ac_js11_2_*`, `json_reader::differential`, `nav::tests::ac_js11_4_*`, `json_reader::exhaustive`, `json_reader::properties`, `json_reader::limits`; 20 mutantes del lector, todos matados | PASS |
| AC-JS11 (9) | `fuzz/`: `json_normalize` y `json_navigate`, una hora cada uno en WSL | PASS: sin fallos |
| AC-JS12 | `table_json::ac_js12_a_store_of_format_13_is_of_a_previous_format` | PASS |
| AC-JS13 | `table_json::schema::ac_js13_json_columns_have_no_statistics`, `vector_columns_keep_the_rule_vector_column` | PASS |
| AC-JS14 | este informe, sección *Mediciones* | PASS (informativa, sin umbral) |

## Verificación

Sobre `40e2d92` (el código no cambia después), el 2026-11-21:

- `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` pasan.
- `cargo test --workspace --no-fail-fast`: 61 binarios, 1.096 tests pasan, 0 fallan y 35 se ignoran.
- La prueba del pager `ac_p6_concurrent_openers_of_a_missing_folder_get_one_store` falló una vez con la máquina
  cargada durante la ejecución y pasó 5 de 5 sola; en esta corrida pasa.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `cf73c36..d86eb6e` (T1–T4) | — | no disponible | ver abajo | — |
| `d86eb6e..40e2d92` (fuzzing, medición, igualdad de `Json`, registros de revisión, documento 15) | legibilidad, fiabilidad | aprobada | AVISO R2-cost-bound-contradiction: el documento 15 promete topes que son tasas esperadas (el mismo punto que R1 de Codex, para tu decisión). SUGERENCIAS: el diagrama del HTML del documento 15 no sigue a su texto; el README de `fuzz/` enlaza la traducción inglesa del PRD (a propósito: el README está en inglés y el archivo existe); `json_measure` lee siempre entero el `string` aunque se le pida otra lectura (solo se le pide entero) y una escala 0 divide por cero (no es una entrada válida). Sin cambios | `review-1c5b12feaf9b3379` |

T1–T4 no tuvieron revisión nativa:

- `cf73c36..HEAD` con el ítem 1 escalado (formato 14): el preflight STATUS falló dos veces con
  `operation_timeout` (fase `pre_native`, sin mutación), la segunda a los 26 s con la máquina en reposo.
- La unidad de riesgo alto sola (`--base-ref cbd7978`): tras confirmar los archivos sin seguir de la ejecución
  (`ba15c45`) y conceder el consentimiento (regla de `CLAUDE.md`), START falló con `lens_context_budget_exceeded`:
  el candidato no cabe en el presupuesto de contexto de los lentes y no se creó ninguna autoridad. El rango mezcla
  2.136 líneas de esta ejecución con unas 900 del documento 15 de la otra sesión, y partirlo exigiría reescribir una
  rama en la que otra sesión confirma.

Evaluaciones sin revisión: T1 y el tramo T1–T3 dieron `medium`, `under_budget`. El código de T1–T4 lo cubren el
verificador independiente y la revisión de Codex. El último tramo, después de T4, se evaluó `high` por un proceso
lanzado en un objetivo de fuzzing; antes de revisarlo se confirmaron los informes de Codex (`40e2d92`) para que el
candidato fuera solo de commits.

## Tiempo

Estimación al empezar: 14 h – 22 h. Activo desde el arranque confirmado: **15 h 27 min**, dentro de la estimación.
La aceptación se lleva el 66 %: la fase quedó abierta desde T5 (12:56) mientras corrían en segundo plano las
mediciones de Windows y de Linux (10 h 11 min de las 12 h 18 min de aceptación en segundo plano; el resto es el
fuzzing, solapado con fases anteriores). El informe de cierre se escribió durante esa espera. La tabla por commit intercala los commits del documento 15 de la otra sesión, que se confirmaron
en la misma rama mientras corría esta ejecución; el tiempo de cada fila es el de esta ejecución.

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 4 min 00 s | 0 % | 1 |
| Plan (documento de ejecución) | 1 min 42 s | 0 % | 1 |
| Código y tests | 2 h 09 min | 14 % | 3 |
| Checks (fmt, clippy, suite de tests) | 41 min 18 s | 4 % | 3 |
| Aceptación | 10 h 18 min | 66 % | 1 |
| Verificación independiente | 48 min 48 s | 5 % | 1 |
| Revisión RDD | 1 h 27 min | 9 % | 3 |
| Cierre (informe y merge) | 1 min 19 s | 0 % | 3 |
| **Total activo** | **15 h 31 min** | **100 %** | **16** |

- Inicio: 2026-11-21 07:40:02 +00:00. Fin: 2026-11-21 23:29:01 +00:00.
- Arranque confirmado: 08:01:01. Activo desde el arranque: **15 h 27 min** (compárese con la estimación); antes del arranque: 4 min 00 s.
- Esperas por el usuario (fuera del activo): 16 min 59 s.
  - 07:44:02–08:01:01 (16 min 59 s): estimate and start confirmation
- Pausas entre sesiones (fuera del activo): 0 min 22 s.
  - 2026-11-21 23:18:49 +00:00 – 2026-11-21 23:19:11 +00:00 (0 min 22 s)

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | fuzz AC-JS11 (9) | 10:45:12 | 10:50:08 | 4 min 56 s |
| Aceptación | fuzz AC-JS11 (9) run 2 | 10:50:40 | 11:50:46 | 1 h 00 min |
| Revisión RDD | codex-R1 | 11:57:20 | 12:40:32 | 43 min 12 s |
| Revisión RDD | codex-R2 | 12:41:17 | 12:45:06 | 3 min 49 s |
| Aceptación | fuzz json_navigate | 11:51:47 | 12:53:44 | 1 h 01 min |
| Aceptación | tj-build-windows | 12:56:59 | 13:07:22 | 10 min 23 s |
| Aceptación | tj-windows | 13:07:22 | 14:56:12 | 1 h 48 min |
| Aceptación | tj-linux | 14:56:12 | 18:01:36 | 3 h 05 min |
| Aceptación | tj2-windows | 18:06:00 | 18:06:02 | 0 min 02 s |
| Aceptación | tj2-linux | 18:06:02 | 21:52:09 | 3 h 46 min |
| Aceptación | tj3-windows | 21:52:34 | 23:13:20 | 1 h 20 min |
| Revisión RDD | rdd-last-slice | 23:16:41 | 23:17:36 | 0 min 55 s |
| **Aceptación (total)** | | | | **12 h 18 min** |
| **Revisión RDD (total)** | | | | **47 min 56 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `f5f7e42` | feat(tables): add the JSON error rules and the JSON module interface | T1 | 14 min 37 s | Exploración y mapa 4 min 00 s, Plan (documento de ejecución) 1 min 42 s, Código y tests 8 min 55 s |
| `b9196a0` | docs(design): add sampled executions and mergeable aggregates to the query profile design | T1, T2 | 37 min 26 s | Código y tests 37 min 26 s |
| `5d7d588` | docs(design): sample executions with a decayed count and a global budget in the query profile design | T2 | 27 min 42 s | Código y tests 22 min 02 s, Checks (fmt, clippy, suite de tests) 5 min 40 s |
| `59190d5` | docs(design): key sampling by shape and plan and describe a new shape's trajectory | T2 | 7 min 55 s | Checks (fmt, clippy, suite de tests) 7 min 55 s |
| `28b9363` | feat(tables): validate and normalize JSON texts | T2 | 1 min 32 s | Checks (fmt, clippy, suite de tests) 1 min 32 s |
| `cbd7978` | feat(tables): navigate JSON documents and read their values as Krummi types |  | 0 min 00 s |  |
| `5397965` | docs(design): cap sampled rows per shape, plan and second in the query profile design | T2, T4 | 45 min 00 s | Código y tests 44 min 24 s, Checks (fmt, clippy, suite de tests) 0 min 36 s |
| `47f2d02` | docs(design): add an illustrated companion to the query profile design | T4 | 29 min 54 s | Código y tests 16 min 21 s, Checks (fmt, clippy, suite de tests) 13 min 33 s |
| `d86eb6e` | feat(tables): add JSON columns to the tables | T4 | 2 min 05 s | Checks (fmt, clippy, suite de tests) 2 min 05 s |
| `cfe78a5` | test(tables): add fuzz targets for the JSON reader and navigation | T4 | 1 min 47 s | Checks (fmt, clippy, suite de tests) 0 min 22 s, Verificación independiente 1 min 25 s |
| `e834b10` | test(bench): add the informative measurement of JSON columns | T4 | 5 min 08 s | Verificación independiente 5 min 08 s |
| `94a5150` | fix(tables): compile the JSON navigation fuzz target | T4 | 0 min 32 s | Verificación independiente 0 min 32 s |
| `c28784b` | docs(design): move profile work to one consumer and stage the query profile design | T4 | 6 min 17 s | Verificación independiente 6 min 17 s |
| `31c2f6a` | docs(design): record the snapshot read, the rings' life cycle and the reassessment of the query profile | T4 | 7 min 01 s | Verificación independiente 7 min 01 s |
| `e302ea9` | fix(tables): compare JSON values by their bytes | T4 | 28 min 00 s | Verificación independiente 28 min 00 s |
| `ba15c45` | docs(odd): add the JSON columns execution document and phase log | T4 | 22 min 18 s | Verificación independiente 0 min 25 s, Revisión RDD 21 min 53 s |
| `40e2d92` | docs(reviews): add the tables-json code review reports and resolutions | T4, codex-R1, T5, closing report, RDD last slice | 11 h 21 min | Aceptación 10 h 18 min, Revisión RDD 1 h 03 min, Cierre (informe y merge) 0 min 27 s |
| — | después del último commit | RDD last slice, closing report, final checks | 12 min 41 s | Checks (fmt, clippy, suite de tests) 9 min 35 s, Revisión RDD 2 min 14 s, Cierre (informe y merge) 0 min 52 s |
| **Total** | | | **15 h 31 min** | |

## Informe de cambios (`cf73c36..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 5.324 | 83 | 4.608 | 405 | 48 |
| `crates/krummi-bench` | 272 | 0 | 236 | 19 | 1 |
| `fuzz` | 138 | 0 | 99 | 9 | 4 |
| `odd` | 116 | 0 | — | — | 2 |
| `crates/krummi-pager` | 91 | 18 | 60 | 27 | 5 |
| `README.md` | 5 | 4 | — | — | 1 |
| `crates/krummi-tx` | 4 | 4 | 2 | 2 | 2 |
| `.gitignore` | 7 | 0 | — | — | 1 |
| `.gitattributes` | 3 | 0 | — | — | 1 |
| **Total** | **5.960** | **109** | **5.005** | **462** | **65** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

En `crates/krummi`, el código de la biblioteca suma 2.569 + y 60 − en 20 archivos, y las pruebas 2.752 + y 23 − en
27 archivos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+1 −0); el corpus JSONTestSuite en
`crates/krummi/tests/data/json_test_suite/` (320 archivos: la licencia, el README y 318 textos de prueba guardados
como binario); y los commits del documento de diseño 15 de la otra sesión, `docs/design/tables/` (+1.122 −34, 3
archivos).

## Mediciones (JS13)

Banco en modo `full`, una carga por proceso, bajo el plan de energía del banco. En Windows corre en los núcleos de
rendimiento (máscara `0xFFFF`) con el lanzador del banco; en Linux, en WSL 2 con todo dentro del sistema de archivos
de WSL. La base es `cf73c36`, la punta de la rama al empezar la ejecución (el PRD y documentos de diseño, sin código), y la
rama es `ba15c45`, las dos compiladas igual desde
`git archive`. Cada corrida guarda la mediana de ns/op de las repeticiones medidas 1–3, y las tablas dan la mediana de
las corridas. «op/s» es la razón de la rama sobre la base: más de 1 es más rápida. En cada ronda, los datos de cada
carga coinciden entre las dos versiones. Cada versión prepara sus propios almacenes, porque la rama escribe el formato
14, que la base no abre.

### Columnas JSON (`json_measure`, 10 rondas)

El `example` de `krummi-bench` usa una tabla con clave entera y una columna, con documentos de un pedido generados
con semilla fija:

- unos 200 B, con 100.000 filas;
- unos 4 KB, con 100.000 filas;
- unos 64 KB, con 10.000 filas.

Cada pedido tiene `id` como primera propiedad raíz, un array grande y `note` como última. El ejemplo mide insertar en
escrituras de 1.000 filas, leer el documento entero por clave en orden aleatorio, y leer la primera y la última
propiedad. El suelo es la misma tabla con el mismo texto normalizado en una columna `string`, leída entera.

Corridas el 2026-11-21: Linux de 17:56 a 18:01 UTC, Windows de 21:52 a 21:57 UTC. Un primer intento en Windows, a las
14:48, falló al lanzarse («Access is denied») por el envoltorio de afinidad del guion, no por el ejemplo, y se repitió
con el lanzador del banco.

| Tamaño | Medida | Windows (ns/op) | × suelo | Linux (ns/op) | × suelo |
|---|---|---|---|---|---|
| 200 B | insertar `string` (suelo) | 1.944 | 1,00 | 7.486 | 1,00 |
| 200 B | insertar `json` | 3.090 | 1,59 | 8.060 | 1,08 |
| 200 B | leer entero `string` (suelo) | 1.156 | 1,00 | 1.042 | 1,00 |
| 200 B | leer entero `json` | 1.181 | 1,02 | 1.076 | 1,03 |
| 200 B | leer la primera propiedad | 1.280 | 1,11 | 1.165 | 1,12 |
| 200 B | leer la última propiedad | 1.577 | 1,36 | 1.473 | 1,41 |
| 4 KB | insertar `string` (suelo) | 16.969 | 1,00 | 33.175 | 1,00 |
| 4 KB | insertar `json` | 35.354 | 2,08 | 55.260 | 1,67 |
| 4 KB | leer entero `string` (suelo) | 6.253 | 1,00 | 2.829 | 1,00 |
| 4 KB | leer entero `json` | 6.411 | 1,03 | 2.893 | 1,02 |
| 4 KB | leer la primera propiedad | 6.522 | 1,04 | 2.905 | 1,03 |
| 4 KB | leer la última propiedad | 11.934 | 1,91 | 9.055 | 3,20 |
| 64 KB | insertar `string` (suelo) | 264.667 | 1,00 | 370.397 | 1,00 |
| 64 KB | insertar `json` | 527.163 | 1,99 | 782.251 | 2,11 |
| 64 KB | leer entero `string` (suelo) | 46.816 | 1,00 | 15.666 | 1,00 |
| 64 KB | leer entero `json` | 49.216 | 1,05 | 22.859 | 1,46 |
| 64 KB | leer la primera propiedad | 49.306 | 1,05 | 20.012 | 1,28 |
| 64 KB | leer la última propiedad | 133.405 | 2,85 | 126.173 | 8,05 |

Lo que muestran las cifras:

- **Insertar.** Validar y normalizar al escribir cuesta entre 1,08 y 2,11 veces la escritura del mismo texto como
  `string`. La razón crece con el tamaño, porque el lector recorre cada byte del documento.
- **Leer entero.** Leer el documento cuesta lo mismo que leer el `string` (1,02–1,05), salvo con 64 KB en Linux
  (1,46).
- **Primera propiedad.** Leerla cuesta casi lo mismo que leer el documento entero.
- **Última propiedad.** Leerla recorre el texto que hay antes: hasta 2,85 veces el suelo en Windows y 8,05 en Linux
  con 64 KB. Es el coste que el índice de navegación del módulo 17 decidirá con su carga del banco (JS6, documento
  13).

JS13 no fija umbral: la medición es informativa.

### A/B de una sola carga: una ronda de W1–W32

W13 se mide dentro del proceso de W12, porque no tiene proceso propio. Corridas el 2026-11-21: Windows de 13:07 a
13:45 UTC, Linux de 15:03 a 16:09 UTC.

| Carga | Windows base / rama (ns/op) | op/s | Linux base / rama (ns/op) | op/s |
|---|---|---|---|---|
| W1 | 4.287,2 / 1.903,0 | 2,253 | 9.543,9 / 9.607,1 | 0,993 |
| W2 | 16.137,8 / 8.868,0 | 1,820 | 25.553,2 / 20.679,4 | 1,236 |
| W3 | 2.456,2 / 2.528,2 | 0,972 | 2.797,7 / 2.800,1 | 0,999 |
| W4 | 125.168,7 / 125.409,1 | 0,998 | 8.808.133,1 / 11.039.981,3 | 0,798 |
| W5 | 64.803,0 / 118.121,2 | 0,549 | 4.518.652,4 / 4.441.898,4 | 1,017 |
| W6 | 125.020,0 / 88.817,0 | 1,408 | 1.068.961,9 / 1.086.094,9 | 0,984 |
| W7 | 996,2 / 982,0 | 1,014 | 1.218,6 / 1.241,0 | 0,982 |
| W8 | 745,6 / 1.011,2 | 0,737 | 489,6 / 436,2 | 1,122 |
| W9 | 397,9 / 450,5 | 0,883 | 451,0 / 457,3 | 0,986 |
| W10 | 171,4 / 158,3 | 1,083 | 33,5 / 33,5 | 1,000 |
| W11 | 250,2 / 78,8 | 3,176 | 53,1 / 54,2 | 0,980 |
| W12 | 960,2 / 968,7 | 0,991 | 694,8 / 653,9 | 1,062 |
| W13 | 28.833,8 / 29.813,2 | 0,967 | 109.307,7 / 113.706,0 | 0,961 |
| W14 | 8.628,7 / 8.299,5 | 1,040 | 14.191,6 / 14.417,9 | 0,984 |
| W15 | 9.058,8 / 9.263,2 | 0,978 | 16.319,9 / 17.960,1 | 0,909 |
| W16 | 16.733.157,0 / 17.806.110,0 | 0,940 | 3.842.899,6 / 4.342.667,2 | 0,885 |
| W17 | 133.396,0 / 128.701,1 | 1,036 | 276.179,1 / 289.698,2 | 0,953 |
| W18 | 6.017,2 / 7.179,9 | 0,838 | 6.716,8 / 7.770,7 | 0,864 |
| W19 | 17.488,8 / 7.948,2 | 2,200 | 15.022,2 / 15.489,6 | 0,970 |
| W20 | 123,4 / 124,9 | 0,989 | 116,4 / 122,1 | 0,953 |
| W21 | 6.684,9 / 6.593,6 | 1,014 | 18.297,8 / 19.368,7 | 0,945 |
| W22 | 469,6 / 460,7 | 1,019 | 524,8 / 524,6 | 1,000 |
| W23 | 2.102,9 / 2.070,6 | 1,016 | 8.224,4 / 8.174,0 | 1,006 |
| W24 | 10.180,1 / 10.246,0 | 0,994 | 17.445,3 / 17.316,0 | 1,007 |
| W25 | 2.145,8 / 2.119,2 | 1,013 | 2.046,4 / 2.081,9 | 0,983 |
| W26 | 1.323,2 / 1.326,7 | 0,997 | 1.368,7 / 1.400,9 | 0,977 |
| W27 | 82,8 / 84,0 | 0,986 | 79,7 / 81,7 | 0,976 |
| W28 | 7.994,6 / 8.070,7 | 0,991 | 13.789,5 / 15.192,6 | 0,908 |
| W29 | 8.352,3 / 8.384,2 | 0,996 | 23.312,1 / 14.271,9 | 1,633 |
| W30 | 26.016,1 / 30.908,8 | 0,842 | 33.817,3 / 29.769,7 | 1,136 |
| W31 | 3.456,1 / 3.474,8 | 0,995 | 3.268,0 / 3.489,6 | 0,936 |
| W32 | 1.086,4 / 1.088,6 | 0,998 | 1.411,5 / 1.293,8 | 1,091 |

Una sola ronda tiene saltos en los dos sentidos, como W11 en Windows (3,176) o W5 en Windows (0,549). Por eso se
midieron después con 10 rondas:

- las cargas de las tablas, W23–W29, que pasan por el código que cambia;
- todas las que quedaron fuera de ±3 %:
  - en Windows: W1, W2, W5, W6, W8, W9, W10, W11, W12 (con W13), W14, W16, W17, W18, W19 y W30;
  - en Linux: W2, W4, W8, W12 (con W13), W15, W16, W17, W18, W20, W21, W30, W31 y W32.

### A/B con 10 rondas

Alternando qué versión va primero. Corridas el 2026-11-21:

| Cargas | Windows (UTC) | Linux (UTC) |
|---|---|---|
| W23–W29 | 13:45–14:48 | 16:09–17:56 |
| las demás | 21:57–23:13 | 18:06–21:52 |

La última columna cuenta las rondas en que la rama tardó más que la base.

| Sistema | Carga | base / rama (ns/op) | op/s | Rondas con la rama más lenta |
|---|---|---|---|---|
| Windows | W1 | 701,0 / 706,3 | 0,993 | 7 de 10 |
| Windows | W2 | 8.283,0 / 8.294,3 | 0,999 | 7 de 10 |
| Windows | W5 | 50.950,4 / 51.099,7 | 0,997 | 4 de 10 |
| Windows | W6 | 43.061,9 / 43.183,2 | 0,997 | 7 de 10 |
| Windows | W8 | 235,9 / 229,2 | 1,030 | 1 de 10 |
| Windows | W9 | 184,3 / 173,2 | 1,064 | 3 de 10 |
| Windows | W10 | 30,9 / 31,1 | 0,995 | 6 de 10 |
| Windows | W11 | 46,8 / 46,7 | 1,003 | 4 de 10 |
| Windows | W12 | 450,5 / 413,1 | 1,091 | 2 de 10 |
| Windows | W13 | 13.177,2 / 12.640,0 | 1,043 | 3 de 10 |
| Windows | W14 | 7.966,1 / 7.979,0 | 0,998 | 5 de 10 |
| Windows | W16 | 15.210.954,5 / 15.150.210,0 | 1,004 | 4 de 10 |
| Windows | W17 | 131.159,9 / 129.243,1 | 1,015 | 3 de 10 |
| Windows | W18 | 5.903,1 / 5.917,5 | 0,998 | 4 de 10 |
| Windows | W19 | 7.440,2 / 7.441,4 | 1,000 | 4 de 10 |
| Windows | W23 | 2.101,6 / 2.097,5 | 1,002 | 5 de 10 |
| Windows | W24 | 10.310,3 / 10.436,2 | 0,988 | 5 de 10 |
| Windows | W25 | 2.126,2 / 2.129,5 | 0,998 | 6 de 10 |
| Windows | W26 | 1.332,9 / 1.339,3 | 0,995 | 8 de 10 |
| Windows | W27 | 82,6 / 83,0 | 0,996 | 7 de 10 |
| Windows | W28 | 8.357,4 / 8.312,9 | 1,005 | 5 de 10 |
| Windows | W29 | 9.211,1 / 8.737,5 | 1,054 | 5 de 10 |
| Windows | W30 | 18.158,5 / 18.208,0 | 0,997 | 8 de 10 |
| Linux | W2 | 15.372,8 / 15.306,4 | 1,004 | 5 de 10 |
| Linux | W4 | 5.678.847,9 / 5.743.130,9 | 0,989 | 5 de 10 |
| Linux | W8 | 369,3 / 371,1 | 0,995 | 4 de 10 |
| Linux | W12 | 427,3 / 408,9 | 1,045 | 7 de 10 |
| Linux | W13 | 74.515,2 / 75.894,5 | 0,982 | 6 de 10 |
| Linux | W15 | 14.713,6 / 14.653,0 | 1,004 | 4 de 10 |
| Linux | W16 | 3.100.511,6 / 3.621.445,4 | 0,856 | 5 de 10 |
| Linux | W17 | 277.278,0 / 262.671,6 | 1,056 | 5 de 10 |
| Linux | W18 | 6.861,5 / 6.801,0 | 1,009 | 6 de 10 |
| Linux | W20 | 116,7 / 118,6 | 0,984 | 6 de 10 |
| Linux | W21 | 17.717,7 / 17.608,2 | 1,006 | 4 de 10 |
| Linux | W23 | 8.042,1 / 8.136,5 | 0,988 | 7 de 10 |
| Linux | W24 | 20.161,9 / 18.827,2 | 1,071 | 5 de 10 |
| Linux | W25 | 2.229,0 / 2.195,1 | 1,015 | 7 de 10 |
| Linux | W26 | 1.553,6 / 1.492,1 | 1,041 | 4 de 10 |
| Linux | W27 | 96,1 / 100,1 | 0,960 | 5 de 10 |
| Linux | W28 | 16.168,9 / 15.599,1 | 1,037 | 5 de 10 |
| Linux | W29 | 16.178,4 / 17.180,9 | 0,942 | 5 de 10 |
| Linux | W30 | 25.818,8 / 25.995,0 | 0,993 | 5 de 10 |
| Linux | W31 | 3.262,1 / 3.288,0 | 0,992 | 6 de 10 |
| Linux | W32 | 1.258,5 / 1.245,7 | 1,010 | 4 de 10 |

**Windows.** Con 10 rondas, todas las cargas quedan en ±3 % salvo tres lecturas que salen más rápidas en la rama: W9,
W12 y W13. Las tres son cargas de árboles, que no usan las tablas. W29, la única de las tablas fuera de ±3 %,
también sale más rápida. Las cargas de las tablas W23–W28 quedan entre 0,988 y 1,005.

**Linux.** Las cargas de las tablas fuera de ±3 % se reparten en los dos sentidos: W24, W26 y W28 salen más rápidas,
y W27 y W29 más lentas. En cada una, la rama tarda más que la base en 4 o 5 de las 10 rondas. Las rondas tienen saltos
de hasta 2–6 veces la mediana, por ejemplo una de W29 en la rama de 99,3 µs/op frente a una mediana de 17,2. Fuera de
las tablas:

- W12 y W17 salen más rápidas.
- W16 (apertura) sale un 14 % más lenta, pero la rama tarda más en solo 5 de las 10 rondas, y las rondas van de 2,1 a
  4,6 ms/op en la base y de 2,7 a 5,3 en la rama. En el informe de las estadísticas, la misma carga salió un 25 % más
  rápida.

No hay una regresión que se repita en los dos sistemas. La causa de las diferencias que quedan no se ha medido.

## Resumen

Las tablas tienen desde 7.5 columnas `json`: la aplicación da el documento como texto o como bytes, Krummi lo valida
al escribirlo con un lector propio en Rust seguro (sin dependencias nuevas de la biblioteca) y lo guarda como texto
normalizado (sin espacios, una sola forma de cada cadena, las propiedades en su orden y los números tal como se
escribieron) detrás de un byte de forma, que reserva el índice de navegación del módulo 17 sin reescribir filas. Leer
devuelve el texto entero sin copia, desde la fila o desde la entrada de un índice; navegar y leer valores tipados
recorre el texto, con aritmética exacta sobre el texto de los números. Un JSON no puede ser clave primaria ni clave
de un índice, no tiene estadísticas, y el formato del almacén pasa a 14.

## Decisiones tomadas sin preguntar

- Los accesores y la columna preparada devuelven `Json<'t>` (como `Cow`), no `JsonValue<'t>`, para que una fila
  copiada pueda devolver su propia copia (PY3); se navega con `document()`.
- E5 e IX3 reutilizan las reglas `InvalidKeyColumn` y `VectorIndexKey`, cuyos textos en inglés nombran ahora JSON.
- Cambiar una columna a `json` o desde `json` falla con `UnsupportedChange` (el cambio de tipo que N3 no admite).
- Un sustituto alto que no sigue de `\u` y un bajo es suelto, antes que un fallo de sintaxis posterior; en una misma
  posición, UTF-8 no válido gana a un texto que termina antes de tiempo; la fracción de un instante se redondea desde
  sus nueve primeras cifras (el mismo resultado); un paso de puntero demasiado grande para `usize` no está.
- La igualdad de `Json` compara solo sus bytes.
- El corpus JSONTestSuite (MIT) se guarda en el repositorio como binario, para que sus bytes no cambien.
- `serde_json`, solo como dependencia de las pruebas, con sus opciones por defecto.
- El fuzzing usa nightly y cargo-fuzz instalados en WSL, fuera del repositorio.
- La medición de JS13 es un `example` de `krummi-bench`, no una carga del banco: no tiene umbral (JS13).
- RDD no disponible: no se reescribió la historia compartida para partir el candidato.

## Pendiente (no bloqueante)

- **Necesita tu decisión: R1 de Codex**, en el documento de diseño 15 de la otra sesión, no en esta ejecución: la
  sección de costes promete «never more than `R` rows per second per (shape, plan) nor `B` overall», aunque el
  mecanismo propuesto ajusta tasas esperadas con instantáneas retrasadas (con `R = 10` y 10.000 ejecuciones/s, la
  primera décima de segundo retiene unas 1.000). Lo descarté por alcance; Codex lo mantiene abierto porque el rango
  revisado incluye esos commits ([informe](../../../docs/reviews/tables-json/2026-11-21-code-02.md),
  [resoluciones](../../../docs/reviews/tables-json/2026-11-21-code-resolutions.md)). RDD dio el mismo aviso
  (R2-cost-bound-contradiction). Lo corrige quien lleva ese diseño.
- **Posible defecto de Gentle AI:** el preflight de RDD sobre el rango entero se agota en 26 s con la máquina en
  reposo. Si quieres, se puede informar en `Gentleman-Programming/gentle-ai` (necesita tu consentimiento).
- Navegar por posición o por nombre recorre el texto (O(bytes antes del valor)); el índice de navegación lo decide el
  módulo 17 con su carga del banco (JS6, documento 13).

**Risk:** ítem 1 (formato 14 y forma guardada v1 de los valores JSON) e ítem 3 (comprobaciones de claves, índices y
estadísticas compartidas con los vectores, cubiertas por las pruebas existentes y una de regresión).

## Limpieza manual

Carpetas y archivos de trabajo fuera del repositorio o ignorados por git; ninguno hace falta para la rama:

- Copias del verificador y de T4: `C:\Users\jesus.lopez\AppData\Local\Temp\tmp.6RocWxQXIo` y
  `C:\Users\JESUS~1.LOP\AppData\Local\Temp\tmp.1MlVjG6aT0`.
- En `C:\Users\JESUS~1.LOP\AppData\Local\Temp\`: `value.rs.bak`, `rows.rs.bak`, `ws_test.log`, `ws_test2.log`,
  `jsonq` y `fixjson.py`.
- La copia de T3 en Git Bash: `/tmp/tmp.RnoZS3ZtvQ`.
- En WSL: `~/krummi-fuzz` (fuzzing) y `~/tj` (compilaciones y resultados de las mediciones).
- `work/tj/` en el repositorio (ignorado): las fuentes, las compilaciones de la base y de la rama, y los resultados
  de las mediciones.
