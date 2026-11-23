# Intercalación de las columnas de texto terminada: verificada y sin fusionar

Informe de cierre de `tables-collation` (2026-11-23). Enfoque de ejecución: ODD.
Fuente de producto: [tables-collation.prd.md](../../docs/prd/krummi/tables/tables-collation.prd.md) (7.7).
Documento de ejecución: [tables-collation.md](tables-collation.md).

Rama `feat/tables-collation`, base `6b583a2`; la ejecución tuvo un seguimiento pedido por el usuario tras el primer
cierre (L17: medir el atajo, optimizar el plegado, revisar los avisos de RDD, abandonar la revisión del falso
positivo). No se fusionó ni se publicó nada.

## Resultado

- **Especificación:** IC1–IC11 e IC13 entregadas; AC-IC1–AC-IC10 y AC-IC12 pasan con sus pruebas (tabla de abajo).
  Suite final sobre `c943882`: `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings`
  limpios; `cargo test --workspace --no-fail-fast`: 65 binarios, 1.258 pruebas pasan, 0 fallan, 35 se ignoran.
- **Aceptación (IC12, AC-IC11):** parcial, por decisión del usuario: solo Windows (usuario: «Solo Windows
  (Recomendado)»), sin las 10 rondas de W23–W32 (usuario: «No hagas la 10 rondas ni en window ni en linux»); en el
  seguimiento, 5 rondas de W23, W24 y W28 para el atajo de la clave de una columna. Cifras en *Mediciones*.
- **Revisiones y verificación:** un verificador independiente sobre todo el documento, sin bloqueos; dos cadenas de
  revisión de código de Codex, aprobadas (la del rango, en la ronda 3, tras seis hallazgos corregidos; la del
  seguimiento, en la ronda 2, tras un hallazgo de enlaces); RDD aprobó los dos últimos tramos; la lineage del falso
  positivo se abandonó (sección *Revisiones RDD*).
- **Defectos encontrados por el camino:**
  - RDD (R3-collation-not-applied): la intercalación se declaraba pero no se aplicaba en el primer tramo; la
    corrección acotada `10c1740` pliega claves, unicidad y estadísticas.
  - Codex R1: una estimación decidía si un valor era grande por su texto original (`e5d9b8f`); R2: las estadísticas
    de prefijo leían textos originales grandes, también valores por defecto (`aca3dd3`, `a6a8679`); R3: el texto
    original se guardaba dos veces en una entrada (`61ee7bf`); R4: una entrada devolvía prestado un texto de clave
    (`1faea20`); R5: las estimaciones conjuntas ignoraban la fracción de valores grandes que introdujo el arreglo de
    R2 (`9974d0a`). Cada uno tiene su prueba de regresión, que falló antes del arreglo.
  - El verificador: AC-IC8 no fijaba los límites acortados del histograma (`f6f0d88`); clippy, dos avisos en la
    prueba del modelo, que entró sin pasar por clippy (`22da401`).
  - La medición: W23 y W24 llevaron a ver que la clave primaria había perdido el atajo de una sola columna
    (`cb1c412`); medido en el seguimiento, sin efecto fuera del ruido.
  - Seguimiento: los siete avisos de RDD (seis corregidos en `3373bc7` y `7056968`, uno descartado con evidencia);
    el plegado por segmentos (`154dfec`); Codex encontró enlaces rotos porque `move_docs.py reopen` deja los
    renombres en el índice y `3373bc7` se los llevó sin los enlaces reescritos (`9dc01a7`).

## Cobertura de los criterios de aceptación

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-IC1 | `table_collation::ac_ic1_describe_shows_the_collation_of_each_text_column`, `ac_ic1_renaming_and_modifying_keep_the_collation`, `ac_ic1_a_collation_on_a_column_that_is_not_text_is_an_invalid_schema`, `ac_ic1_another_collation_is_a_different_schema`; `schema_tests::a_collation_decodes_as_it_was_encoded` | PASS |
| AC-IC2 | `table_collation_fold::every_scalar_value_folds_to_the_definition`, `canonically_equivalent_texts_fold_alike`, `random_sequences_fold_to_the_definition`, `the_examples_of_the_prd_fold_as_its_table_says` | PASS |
| AC-IC3 | `table_collation::keys::ac_ic3_equivalent_keys_are_the_same_row`, `ac_ic3_a_bulk_load_with_equivalent_keys_fails_as_with_repeated_ones`, `ac_ic3_a_text_outside_the_key_keeps_its_spelling` | PASS |
| AC-IC4 | `table_collation::indexes::ac_ic4_unique_indexes_compare_equivalent_values`, `ac_ic4_loads_and_builds_report_equivalent_values`; `applied::keys_unique_indexes_scans_and_estimates_fold` | PASS |
| AC-IC5 | `table_collation::model::ac_ic5_scans_ranges_and_duplicates_follow_the_folded_forms` (disco y temporal; mutante sin plegado matado) | PASS |
| AC-IC6 | `table_collation::indexes::ac_ic6_entries_return_the_stored_texts_in_folded_order`; `review::r4_an_entry_returns_a_collated_key_text_in_its_own_copy`; `index_tests::a_collated_column_keeps_its_original_text_once` | PASS |
| AC-IC7 | `table_collation::keys::ac_ic7_keys_and_bounds_are_measured_folded` | PASS |
| AC-IC8 | `table_collation::statistics::ac_ic8_statistics_and_estimates_count_folded_forms`, `ac_ic8_shortened_bounds_are_folded_prefixes`; `review::r1_…`, `r2_…` (2), `r5_…` | PASS |
| AC-IC9 | `table_collation::ac_ic9_a_store_of_format_14_is_of_a_previous_format` | PASS |
| AC-IC10 | La suite: las pruebas de 7.1–7.5 pasan; cambian solo la versión del formato (14 → 15) y el hash del archivo de datos de `bytes_tests`. El verificador comparó 166 líneas de salida de columnas `binary` en la base y en HEAD: iguales | PASS |
| AC-IC11 | Este informe, *Mediciones*: solo Windows, sin las 10 rondas | PARCIAL, por decisión del usuario |
| AC-IC12 | `table_collation_fold::the_library_gains_no_dependency`; sin cambios en `Cargo.toml` ni `Cargo.lock` | PASS |

## Mediciones

Windows, plan de energía del banco.

**Una ronda de W1–W32**, base `6b583a2` frente a cabeza `b33cc40` (proporción de velocidad base ÷ cabeza; más de 1
es más rápido): 26 de 32 cargas dentro del ±3 % o más rápidas. Más lentas: W8 0,836 y W9 0,772 (cargas de árboles, que
este cambio no toca), W24 0,744, W1 0,933, W23 0,945 y W5 0,952. Más rápidas sin causa: W2 1,335, W28 1,343 y W29
1,245. Con una sola ronda son sobre todo ruido (el PRD pedía de 10 a 20 rondas de las afectadas).

**El atajo de la clave de una columna** (`cb1c412`), 5 rondas, base `b33cc40` frente a cabeza `154dfec`: W23 1,016,
W24 1,002 y W28 1,004, dentro del ruido; el 0,744 de W24 de la ronda única era ruido.

**`collation_measure`** (10 rondas, 100.000 filas, clave `string` de unos 24 bytes con mayúsculas y acentos e índice
único sobre otra columna igual; mediana en ns por operación), antes (`b33cc40`) y después (`154dfec`) del plegado
por segmentos:

| Medida | `binary` antes | `ci` antes | `ci` ÷ `binary` antes | `binary` después | `ci` después | `ci` ÷ `binary` después |
|---|---|---|---|---|---|---|
| Insertar en escrituras de 1.000 | 9.705 | 25.430 | 2,62 | 8.906 | 13.598 | 1,53 |
| Leer por la clave | 1.625 | 4.577 | 2,82 | 1.576 | 2.382 | 1,51 |
| Buscar por el índice | 3.837 | 9.934 | 2,59 | 3.798 | 5.544 | 1,46 |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `6b583a2..bead82f` (con los datos de Unicode) | riesgo, legibilidad, fiabilidad | detenida: `lens_context_budget_exceeded` | Ninguno: no se creó autoridad. La rama se rehízo en datos, tabla generada, T1 y T2 | — |
| `320e9a9..bead82f` (T1, T2) | riesgo, legibilidad, fiabilidad | corrección pedida | CRITICAL R3-collation-not-applied (la intercalación no se aplicaba): corregido en `10c1740` (199 líneas) | `review-5fc999f939b056fe` |
| `320e9a9..10c1740` (recuperada por cambio de alcance) | riesgo, legibilidad, fiabilidad | abandonada por el usuario (`operator_disposition`) | Riesgo y legibilidad sin hallazgos. BLOCKER R3-missing-generated-tables: falso positivo (la tabla generada y los datos están en la base, `git ls-tree -r c8be8bb`); sin corrección posible; abandonada en el seguimiento, en la cuarentena auditada | `review-b00b0f72ef61e47d` |
| `320e9a9..cb1c412` (último tramo) | riesgo, legibilidad, fiabilidad | detenida: `lens_context_budget_exceeded` | Ninguno: no se creó autoridad | — |
| `10c1740..cb1c412` (último tramo reducido) | riesgo, legibilidad, fiabilidad | aprobada | Avisos no bloqueantes, sin cambios: R1-001 SUGGESTION, `collation_measure` borra sus subcarpetas `binary`/`ci` de la carpeta dada sin aviso; R2-001 WARNING, `EntryLayout::original` devuelve `None` también ante un valor mal formado; R2-002 SUGGESTION, los huecos de texto original usan la identidad 0 sin comentario; R2-003 SUGGESTION, los valores guardados y los valores por defecto se clasifican grandes de forma distinta; R2-004 y R2-005 SUGGESTION, legibilidad de una guarda y de un `zip` de cuatro; R3-invalid-len-counted-large WARNING, una fila dañada con una longitud no válida cuenta ahora como grande en los prefijos. En el seguimiento: seis corregidos (`3373bc7`, `7056968`) y R3 descartado con evidencia (la recolección por columna ya cuenta como grande una longitud no válida, `statistics.rs:212`) | `review-b2a93e2d2f57646d` |
| `cb1c412..c943882` (seguimiento) | riesgo, legibilidad, fiabilidad | aprobada | Avisos no bloqueantes: R2-001 WARNING, el bucle de prefijos reutiliza `combinations` y `read` para valores distintos (pendiente); R2-002 WARNING, el informe describía el estado anterior al seguimiento (corregido con este informe); R2-003 SUGGESTION, un enum en lugar de `Option<Option<Value>>` en `EntryLayout::original` (pendiente); R3-default-large-unpinned WARNING, descartado con evidencia (esa rama solo recoge prefijos de índice; una columna sin índice sigue con `is_large`, y una forma plegada de más del máximo nunca llega a una clave de índice, IC7) | `review-a4508c4b3c288fcb` |

Evaluaciones sin revisión: T1–T2 dio `medium`, `under_budget` y se elevó a `high` con el ítem 1 (formato del
catálogo); los dos últimos tramos dieron `medium`, `under_budget` y se revisaron por ser el último tramo. Dos preflights del último tramo agotaron su tiempo
(`operation_timeout`) mientras se compilaba para las mediciones; se repitió después.

## Tiempo

- Estimación: 12 h – 18 h. Arranque real: 2026-11-23 08:26:24 +00:00, sin pregunta de confirmación (el usuario pidió
  no preguntar después de la estimación). Primer cierre: 12:11:49. Seguimiento (L17): estimación 2 h – 4 h, arranque
  12:36:15 tras el «Sí» del usuario, fin 13:42:50.
- **Duración activa desde el arranque: 4 h 51 min**: 3 h 45 min hasta el primer cierre, muy por debajo de su
  estimación, y 1 h 06 min de seguimiento, por debajo de la suya. El código y sus pruebas llevaron 1 h 07 min; las
  revisiones (RDD, Codex) 1 h 14 min y sus correcciones 39 min 20 s; la aceptación 1 h 05 min, reducida a Windows sin
  las 10 rondas por decisión del usuario.
- Esperas por el usuario: 08:19:49–08:26:25 (6 min 36 s), las preguntas antes del arranque; la pausa entre el primer
  cierre y el seguimiento (24 min 26 s) no es tiempo activo.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 2 min 59 s | 1 % | 1 |
| Plan (documento de ejecución) | 2 min 16 s | 1 % | 2 |
| Código y tests | 1 h 07 min | 23 % | 4 |
| Checks (fmt, clippy, suite de tests) | 24 min 58 s | 8 % | 2 |
| Aceptación | 1 h 05 min | 22 % | 2 |
| Verificación independiente | 12 min 34 s | 4 % | 1 |
| Revisión RDD | 1 h 14 min | 25 % | 6 |
| Correcciones de hallazgos | 39 min 20 s | 13 % | 2 |
| Documentación | 4 min 57 s | 2 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **4 h 54 min** | **100 %** | **22** |

- Inicio: 2026-11-23 08:16:50 +00:00. Fin: 2026-11-23 13:42:50 +00:00.
- Arranque confirmado: 08:26:25. Activo desde el arranque: **4 h 51 min** (compárese con la estimación); antes del arranque: 2 min 59 s.
- Esperas por el usuario (fuera del activo): 6 min 36 s.
  - 08:19:49–08:26:25 (6 min 36 s): questions before start
- Pausas entre sesiones (fuera del activo): 24 min 26 s.
  - 2026-11-23 12:11:49 +00:00 – 2026-11-23 12:36:15 +00:00 (24 min 26 s)

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | codex-R1 | 10:00:20 | 10:16:22 | 16 min 02 s |
| Revisión RDD | codex-R2 | 10:34:32 | 10:48:00 | 13 min 28 s |
| Revisión RDD | codex-R3 | 11:02:53 | 11:08:12 | 5 min 19 s |
| Aceptación | tc-build-windows | 11:09:16 | 11:20:00 | 10 min 44 s |
| Aceptación | tc-windows | 11:20:00 | 11:49:42 | 29 min 42 s |
| Aceptación | tc-linux | 11:49:43 | 11:49:44 | 0 min 01 s |
| Aceptación | tc2-build-windows | 12:45:17 | 12:52:26 | 7 min 09 s |
| Aceptación | tc2-windows | 12:52:26 | 13:09:16 | 16 min 50 s |
| Revisión RDD | codex-F1 | 13:09:54 | 13:25:42 | 15 min 48 s |
| Revisión RDD | codex-F2 | 13:26:34 | 13:28:58 | 2 min 24 s |
| **Aceptación (total)** | | | | **1 h 04 min** |
| **Revisión RDD (total)** | | | | **53 min 01 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `29fb1cc` | chore(unicode): vendor the Unicode 17.0 data files the collations fold with | T2, T1-T2 | 31 min 26 s | Exploración y mapa 2 min 59 s, Plan (documento de ejecución) 1 min 52 s, Código y tests 25 min 56 s, Revisión RDD 0 min 39 s |
| `320e9a9` | chore(tables): add the Unicode 17.0 tables generated for the collations |  | 0 min 00 s |  |
| `82c6d47` | feat(tables): fold text with the Unicode 17.0 collations ci and ci_ai |  | 0 min 00 s |  |
| `bead82f` | feat(tables): declare the collation of a text column in its schema |  | 0 min 00 s |  |
| `10c1740` | fix(tables): fold the keys, unique checks and statistics of collated columns | T1-T2 | 9 min 55 s | Revisión RDD 9 min 55 s |
| `7094255` | test(tables): pin the primary key and the folded sizes of collated columns | T1-T2, T3 | 3 min 35 s | Código y tests 0 min 26 s, Revisión RDD 3 min 09 s |
| `d4b74ea` | feat(tables): return and report the original texts of collated index entries | T3 | 32 min 22 s | Código y tests 32 min 22 s |
| `65211fb` | feat(tables): count a text as large by its folded form in statistics | T3 | 0 min 01 s | Código y tests 0 min 01 s |
| `90ce084` | test(tables): check collated keys and indexes against a reference model | T3 | 0 min 40 s | Código y tests 0 min 40 s |
| `cdb4df2` | docs(guides): document the collations of text columns | T3, T7 | 1 min 31 s | Código y tests 0 min 12 s, Documentación 1 min 19 s |
| `9d75c3b` | test(bench): add the informative measurement of collated text columns | T7 | 2 min 44 s | Documentación 2 min 44 s |
| `f6f0d88` | test(tables): pin the shortened histogram bounds of a collated column | T7, verifier | 13 min 28 s | Verificación independiente 12 min 34 s, Documentación 0 min 54 s |
| `22da401` | test(tables): keep the collation model test clear of clippy lints | codex-R1 | 0 min 55 s | Revisión RDD 0 min 55 s |
| `61ee7bf` | fix(tables): keep the original text of a collated column once per index entry | codex-R1 | 19 min 47 s | Revisión RDD 19 min 47 s |
| `e5d9b8f` | fix(tables): decide a large estimate value by its folded form |  | 0 min 00 s |  |
| `aca3dd3` | fix(tables): never read a large original through an index prefix in analyze |  | 0 min 00 s |  |
| `1faea20` | fix(tables): return the collated key texts of an index entry in a copy |  | 0 min 00 s |  |
| `a6a8679` | fix(tables): measure a column's default before an index prefix folds it in analyze | codex-R1, codex-R1 fixes | 29 min 41 s | Revisión RDD 14 min 41 s, Correcciones de hallazgos 15 min 00 s |
| `9974d0a` | fix(tables): keep the large fraction apart in joint estimates of an index prefix | codex-R1 fixes | 0 min 01 s | Correcciones de hallazgos 0 min 01 s |
| `b33cc40` | docs(reviews): record the Codex code review of the collation run | codex-R1 fixes | 19 min 33 s | Correcciones de hallazgos 19 min 33 s |
| `cb1c412` | perf(tables): encode a one-column primary key without collecting its values | codex-R1 fixes, T7 measurements, T7 key fast path | 45 min 33 s | Código y tests 4 min 36 s, Aceptación 40 min 47 s, Correcciones de hallazgos 0 min 10 s |
| `42cd6ac` | docs: add the tables-collation closing report | T7 key fast path, final checks, RDD last slice | 17 min 11 s | Código y tests 0 min 01 s, Checks (fmt, clippy, suite de tests) 13 min 38 s, Revisión RDD 3 min 32 s |
| `84ec5fd` | docs: archive the tables-collation execution |  | 0 min 00 s |  |
| `3373bc7` | fix(tables): tell a malformed original text apart from an element that is not collated | follow-up T9-T12, T9, T10 | 5 min 11 s | Plan (documento de ejecución) 0 min 24 s, Revisión RDD 0 min 31 s, Correcciones de hallazgos 4 min 16 s |
| `7056968` | refactor(tables): simplify the collated prefix statistics and joint estimates |  | 0 min 00 s |  |
| `154dfec` | perf(tables): fold only the unstable segments of a collated text | T10, T11 | 3 min 44 s | Código y tests 3 min 24 s, Correcciones de hallazgos 0 min 20 s |
| `9dc01a7` | docs: reopen the tables-collation execution for its follow-up | T11, T12, codex follow-up | 41 min 16 s | Código y tests 0 min 07 s, Aceptación 24 min 37 s, Revisión RDD 16 min 32 s |
| `c943882` | docs(reviews): record the Codex review of the collation follow-up | codex follow-up | 2 min 51 s | Revisión RDD 2 min 51 s |
| — | después del último commit | follow-up final checks, RDD follow-up | 13 min 33 s | Checks (fmt, clippy, suite de tests) 11 min 20 s, Revisión RDD 2 min 13 s |
| **Total** | | | **4 h 54 min** | |

La revisión (25 %) y la aceptación (22 %) pesan cada una tanto como el código (23 %): la corrección acotada del primer
tramo hizo de golpe el plegado de T3, T4 y T6, Codex necesitó tres rondas en la primera cadena y dos en la segunda, y
las mediciones de Windows corrieron en primer plano. El tiempo por commit está corrido: la rama se rehízo a las 08:5x
para partir el primer tramo (`29fb1cc`…`bead82f` tienen la misma hora), y los arreglos de Codex se confirmaron
juntos, así que cada tramo carga el trabajo de los commits que lo preceden con 0 min.

## Informe de cambios (`6b583a2..c943882`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 3.091 | 152 | 2.670 | 246 | 32 |
| `docs` | 661 | 8 | — | — | 12 |
| `odd` | 460 | 0 | — | — | 3 |
| `scripts` | 316 | 0 | 243 | 21 | 2 |
| `crates/krummi-bench` | 205 | 0 | 177 | 12 | 1 |
| `crates/krummi-pager` | 18 | 10 | 12 | 6 | 4 |
| `crates/krummi-tx` | 5 | 4 | 2 | 3 | 2 |
| **Total** | **4.756** | **174** | **3.104** | **288** | **56** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `crates/krummi/src/table/collation_tables.rs` (+2168 −0), `crates/krummi/unicode/17.0.0/CaseFolding.txt` (+1682 −0), `crates/krummi/unicode/17.0.0/CompositionExclusions.txt` (+222 −0), `crates/krummi/unicode/17.0.0/LICENSE.txt` (+39 −0), `crates/krummi/unicode/17.0.0/NormalizationTest.txt` (+20095 −0), `crates/krummi/unicode/17.0.0/README.md` (+19 −0), `crates/krummi/unicode/17.0.0/Scripts.txt` (+3182 −0), `crates/krummi/unicode/17.0.0/UnicodeData.txt` (+40575 −0)

Sin previsión de líneas al empezar. Los datos de Unicode y la tabla generada se versionan aparte por decisión del
usuario («Versionarlos todos (Recomendado)»). `odd` son el documento de ejecución, su registro de fases y este
informe, que el seguimiento commiteó en `odd/tasks` al reabrirlos.

## Resumen

Una columna `string` declara su intercalación: `binary` (la de antes, por defecto), `ci` o `ci_ai`. La clave
primaria, los índices, la unicidad, los intervalos, las estadísticas y las estimaciones comparan la forma plegada,
fijada a Unicode 17.0 (plegado de mayúsculas completo con normalización canónica; en `ci_ai`, sin las marcas de las
letras latinas, griegas y cirílicas, con la `ñ` como letra propia), mientras las filas y las entradas de índice
devuelven el texto tal como se escribió. El plegado usa tablas propias generadas de los ficheros de Unicode, sin
dependencias nuevas; la intercalación se guarda en el catálogo con una bandera y un byte, y el formato pasa a 15.
Una entrada de índice guarda el texto original de sus columnas plegadas una vez por columna, después de las
incluidas. El plegado copia los tramos estables y normaliza solo los segmentos que cambian, así que `ci` cuesta
alrededor de 1,5 veces `binary` con texto acentuado y con mayúsculas.

## Decisiones tomadas sin preguntar

- `modify_column` conserva su firma y espera `binary`; `modify_column_with_collation` recibe la intercalación (L4).
- La rama se rehízo en cuatro commits (datos, tabla generada, T1, T2) para que el primer tramo cupiera en RDD; la
  tabla generada entra antes que su fuente para que cada commit compile.
- La lineage RDD `review-b00b0f72ef61e47d` no se abandonó durante la ejecución (necesita una autorización que lee
  el usuario) ni se simuló una corrección de su falso positivo; el usuario la mandó abandonar en el seguimiento.
- El último tramo RDD se revisó desde `10c1740` porque el rango entero no cabe en el presupuesto de los revisores.
- Las 10 rondas de Windows se cortaron con un vigilante en lugar de editar `all.sh` en marcha.
- En el seguimiento: las mediciones solo en Windows, 5 rondas de W23, W24 y W28 para el atajo (las cargas que pasan
  por `encode_key_of`); la predecesora `review-5fc999f939b056fe` (su hallazgo, válido, se corrigió en `10c1740`) y
  `review-ed0f0e4daf403899` (ajena a esta ejecución) no se abandonaron, porque no se pidieron; R2-001 y R2-003 del
  último tramo quedan pendientes en lugar de abrir otro tramo sin revisar.

## Pendiente (no bloqueante)

- Mediciones incompletas frente a IC12, por decisión del usuario: Linux sin medir y sin las 10 rondas de W23–W32
  en la pasada general.
- Dos lineages RDD siguen en `correction_required` sin que se pidiera tocarlas: `review-5fc999f939b056fe`, la
  predecesora de la abandonada, cuyo hallazgo ya se corrigió en `10c1740`, y `review-ed0f0e4daf403899`, de otro
  trabajo. No bloquean: el rango lo cubren las revisiones aprobadas, Codex y el verificador.
- Avisos de legibilidad del último tramo RDD: R2-001 (nombres reutilizados en el bucle de prefijos) y R2-003 (un enum
  para los tres estados de `EntryLayout::original`). No cambian el comportamiento.
- La medición informativa usa un texto de nombres y apellidos; no hay carga del banco con columnas `ci`, como
  decidió IC12.

**Riesgo:** ítem 1 (el formato del catálogo pasa a 15 y las claves guardan formas plegadas: cambiar el plegado o la
versión de Unicode es un cambio de formato; el plegado por segmentos da exactamente la misma forma, AC-IC2 y su
prueba de segmentos); cubierto por el verificador independiente, las revisiones de Codex y RDD.

## Limpieza manual

- `work/tc/` (1,3 GB) y `work/tc2/` (1,3 GB), los binarios de las mediciones de base y cabeza: se pueden borrar.
