# Valores grandes (5.3) implementados y verificados, con la revisión de Codex aprobada

Informe de cierre de `trees-large-values` (2026-10-27). Enfoque de ejecución: ODD.
Fuente de producto: [trees-large-values.prd.md](../../docs/prd/krummi/trees-large-values.prd.md). Documento de
ejecución: [trees-large-values.md](trees-large-values.md).

Rama `feat/trees-large-values`, sin fusionar en `main` y sin push. Último commit de código `7584790`
(2026-10-28); el informe va en el commit que cierra la ejecución.

## Resultado

- **Especificación:** las catorce especificaciones S1–S14 (AC-V1–AC-V14, con AC-B3, AC-B12, AC-B17, AC-B18, AC10 y
  AC20 dentro) están cubiertas por pruebas. Comprobaciones finales tras el último arreglo, en Windows: `cargo fmt
  --all --check` correcto, `cargo clippy --workspace --all-targets -- -D warnings` limpio y `cargo test --workspace`
  con 379 pruebas superadas, 0 fallidas y 14 ignoradas; en Linux (WSL 2 Ubuntu), `-p krummi-pager -p krummi-tx -p
  krummi` con 363 superadas, 0 fallidas y 14 ignoradas (cifras del worker del último arreglo, `7584790`). Los barridos completos de AC-V8 que están ignorados
  (`ac_v8_a_failure_at_every_page_operation…` y `ac_v8_crashes_through_the_close`) pasan. El barrido completo de AC10
  byte a byte con un valor fuera de su hoja pasó en la CI (run `40654101766` sobre `3c4d536`, paso de árboles:
  13 min 22 s en Linux y 10 min 39 s en Windows; la estimación local de unas 2 h era errónea). La sonda de bytes
  alterados del verificador sobre 6.893 bytes dio 4.841 detectados, 2.052 sin efecto y 0 datos erróneos.
- **Aceptación:** la medición de V11 en Windows y en Linux, con los dos umbrales candidatos; datos en
  [trees-large-values-measurements.md](trees-large-values-measurements.md). Con el umbral elegido (media hoja),
  guardar y leer 256 MiB de valores: 1 KiB, 6,24 s / 1,22 s en Windows y 5,58 s / 1,24 s en Linux; 8 MiB,
  0,64 s / 0,10 s y 0,82 s / 0,04 s. Carga mixta (1 millón de claves): guardar 33,17 s, leer 4,79 s y recorrer
  0,43 s en Windows; 44,69 s, 4,68 s y 0,23 s en Linux; 107.046 páginas en uso en los dos.
- **Revisiones y verificación:** ocho revisiones RDD aprobadas (tabla abajo); verificador independiente (`sonnet`,
  `high`) sin bloqueos, con S1–S12 cumplidas y una comparación con la base `27a5e5b` de 15.480 líneas de operaciones
  con valores en la hoja, idéntica byte a byte en disco y en modo temporal; revisión de Codex en cinco rondas (la
  cuarta y la quinta, autorizadas por el usuario por encima del límite de tres): R1 *major* verificado como
  resuelto en la ronda 4, que abrió R2 *major*, corregido en `7584790` y verificado como resuelto en la ronda 5
  ([informe](../../docs/reviews/trees-large-values/2026-10-28-code-03.md), aprobada, sin hallazgos abiertos). Las
  correcciones R1–R4 del PRD, verificadas por Codex en su
  [revisión de seguimiento](../../docs/reviews/trees-large-values/2026-10-28-prd.md).
- **Hallazgos corregidos por el camino:** R1 de Codex, en tres partes: los trozos del área temporal hechos para un
  único rango grande no se liberaban ni contaban para el tamaño máximo (`7b4286b`), un rango retenido a medias
  seguía contando solo sus páginas vivas (`e1cc4ab`) y un rango retirado que retenía un lector más nuevo dejaba de
  contar (`098e621`); R2 de Codex, una regresión de ese último arreglo: una escritura fallaba con tamaño máximo por
  rangos que solo retenía su propia instantánea, contra J7 (`7584790`). En los cuatro, la prueba de regresión
  reproducía las cifras de Codex antes del arreglo. Una prueba de T2 que comparaba punteros competía con la transferencia en segundo plano
  (`46fdd12`). De las revisiones RDD: la reserva del modo temporal limitada por el máximo y alineada a la página del
  sistema (RED en Linux, `mprotect` EINVAL), la comprobación de lectura de un rango en todo su tamaño, el aserto de
  páginas enteras al publicar un rango, la longitud 0 rechazada en una referencia fuera de la hoja y varios
  arreglos de legibilidad.

## Cobertura de los criterios de aceptación

El documento de ejecución traza cada S# a sus pruebas (L4–L16); el verificador independiente dio estos veredictos:

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-V1 | `tree_values.rs::ac_v1_a_value_of_256_mib_under_a_2048_byte_key` | PASS |
| AC-V2 | `tree_values.rs::ac_v2_values_around_the_threshold_and_the_run_boundaries`; `tree_value_model.rs::threshold_sweep_keys_and_values` | PASS |
| AC-V3 | `value_tests.rs::ac_v3_…`; `runs.rs::a_run_s_reference_is_the_engine_s_memory_where_it_lives`; `trees_compile.rs` | PASS |
| AC-V4 | `tree_values.rs::ac_v4_scans_and_range_removals_over_values_off_their_leaf` | PASS |
| AC-V5 | `tree_values.rs::ac_v5_values_off_their_leaf_give_their_pages_back` | PASS |
| AC-V6 | `value_tests.rs::ac_v6_…`; `runs.rs::a_read_keeps_a_run_that_later_writes_free_and_reuse` | PASS |
| AC-V7 | `tree_values.rs::ac_v7_a_damaged_value_fails_its_reads_but_never_its_removal` | PASS |
| AC-V8 | `tree_integrity.rs::ac_v8_…`; `tests.rs::ac_v8_…` (muestra y barrido completo ignorado) | PASS |
| AC-V9 | `value_tests.rs::ac_v9_a_value_without_a_contiguous_hole_goes_to_the_frontier`; `runs.rs::growing_runs_…`, `shrinking_runs_…` | PASS |
| AC-V10 | `tree_values.rs::ac_v10_writes_that_change_nothing_with_large_values` | PASS |
| AC-V11 | `tree_integrity.rs::ac10_sample_of_altered_bytes_with_trees`; `pages.rs::ac_v11_…` | PASS (muestra en local; barrido completo en la CI, run `40654101766`) |
| AC-V12 | `tree_scans.rs::ac_v12_…`; `tree_values.rs::ac_v12_the_entry_of_a_value_off_its_leaf_is_little_endian` | PASS |
| AC-V13 | este informe y [trees-large-values-measurements.md](trees-large-values-measurements.md) | PASS |
| AC-V14 | evaluación de V4 en [trees-large-values-measurements.md](trees-large-values-measurements.md) | PASS |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `27a5e5b..656acdf` (T1) | risk, resilience, reliability | aprobada | Tres WARNING sobre la reserva de 64 GiB del modo temporal y su alineación: corregidos en T2 (reserva `min(máximo, 64 GiB)`, alineación a 64 KiB). SUGGESTION de búsqueda lineal de huecos: descartada con evidencia (ya para en el primer hueco para una página). | `review-6ccf3988157feb31` |
| `656acdf..244548d` (T2) | risk, resilience, reliability | aprobada | WARNING: la lectura comprobaba solo la primera página del rango; WARNING: ramas de daño de `Snapshot::run` sin pruebas; SUGGESTION: aserto de páginas enteras en `stage`. Los tres corregidos en T4. | `review-033cd994e42cec40` |
| `244548d..9a596b2` (T3) | risk, reliability, readability | aprobada | Dos WARNING: vaciar o borrar un árbol ahora lee sus hojas, así que una hoja dañada lo impide; aceptado (A9 lo permite, V7 protege las páginas del valor). Legibilidad (WARNING y SUGGESTION) y referencia de longitud 0 (SUGGESTION): corregidos en T5. | `review-27cf815bcee22c3d` |
| `9a596b2..fa3f291` (último tramo) | risk, reliability, readability | aprobada | Siete SUGGESTION: tres corregidas en `3a0b336` (resta saturada, número de escrituras derivado, el bucle de fallos al confirmar comprueba que el fallo no se tragó); cuatro aceptadas (tamaño del valor del barrido explicado, ayudantes de prueba duplicados entre pruebas de integración y unitarias, cota de lectura por páginas actuales solo alcanzable con una hoja manipulada). | `review-6e7966cf3bbe5d18` |
| `62f7571..3a528ec` (arreglo R1) | risk, resilience, reliability | aprobada | SUGGESTION de comentario sin código: corregida en `c0b8d1a`. Aceptadas como trabajo posterior: WARNING de un archivo por rango pequeño con el área fragmentada, SUGGESTION de llamadas al sistema bajo el cerrojo, WARNING de igualdad exacta de bytes en la prueba, SUGGESTION de tamaños fijos en las sondas. | `review-695b76ad34d5b00d` |
| `3a528ec..e1cc4ab` (arreglo R1, ronda 2) | risk, resilience, reliability | aprobada | WARNING: un rango ya retirado tras su traslado no cuenta mientras lo retiene un lector; aceptado al principio, Codex lo refutó en la ronda 3 y se corrigió en `098e621`. | `review-713a431a84a3734d` |
| `373208f..098e621` (arreglo R1, ronda 3) | risk, resilience, reliability | aprobada | WARNING: el cierre podría dejar rangos retirados y romper el aserto de cuenta cero; descartado con evidencia (sin lectores ni escritor, `oldest = u64::MAX` libera todo). SUGGESTION de aserción directa en la prueba: aceptada sin cambio. | `review-aba90cc61b80a4bb` |
| `45c1bf2..7584790` (arreglo R2) | risk, resilience, reliability | aprobada | Dos WARNING: la liberación en el vaciado del escritor depende de que sus referencias tomen prestado al escritor; descartados con evidencia (`ScratchWriter::snapshot(&self) -> &Snapshot<'a>` y `Snapshot::page`/`run(&self)` atan el resultado al préstamo). SUGGESTION de prueba con un lector abierto durante el vaciado: descartada (ya la cubre `shrinking_runs_retired_under_a_newer_read_count_until_freed`). | `review-48af2c09d934d928` |

Sin revisión propia: `d0be3f0` (T4, `medium`, `under_budget`; revisado dentro del último tramo) y los commits solo de
documentación y del registro de fases posteriores a `e1cc4ab`. Dos intentos no llegaron a empezar (el script de
la revisión llamó al `bash` de WSL, y un archivo sin seguimiento pedía selección); se repitieron.

## Reutilización

- El rango de valor reutiliza la reserva de rangos contiguos del espacio libre (F5) y su liberación (F8) sin
  cambiarlas, y `seal_page`/`verify_page` pasan a ser el caso de un rango de una página de `seal_run`/`verify_run`.
- El área temporal amplía sus huecos libres a rangos que se fusionan, y las versiones por página apuntan al rango
  con su índice; el diario reutiliza su registro con una marca por entrada (`RAW_ENTRY`).
- El árbol reutiliza su división en dos, la fusión y el arreglo de la raíz; la liberación recorre las hojas con
  `free_subtree`, que antes saltaba las hojas.
- Las pruebas reutilizan los barridos de caídas, de T11 y de AC10, los ganchos de fallos y `tests/support`.
- Escrito nuevo: el modo temporal en un único rango de direcciones reservado (`sys::Reservation`) y la contabilidad
  por rango del área temporal.

## Tiempo

- Estimación: 6 h – 9 h. Arranque real: 2026-10-27 18:39:54 +0000 (`date`). Fin: 2026-10-27 22:36:32 +0000.
- **Duración activa desde el arranque: 3 h 56 min**, por debajo del mínimo de la estimación (66 %). La mayor parte
  fue código y pruebas (2 h 32 min), hecho por workers uno tras otro; las correcciones de los dos hallazgos de Codex
  sumaron 36 min. Las esperas de Codex y del verificador corrieron en paralelo con otras tareas.
- Esperas por el usuario: ninguna dentro de la ejecución del 2026-10-27.
- **2026-10-28, continuación (no medida por fase):** las rondas 3 y 4 de Codex y los arreglos `098e621` y `7584790`
  se hicieron en una sesión posterior que no marcó el registro de fases, así que su tiempo no está en las tablas
  de abajo; sus commits van de `373208f` a `1c072c3`. Incluye esperas por el usuario para restaurar el inicio de
  sesión de la CLI de Codex. La ronda 5, que aprobó el arreglo de R2, se lanzó sobre `88f76ce`
  (2026-10-28) y tampoco está en el registro de fases.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 5 min 35 s | 2 % | 1 |
| Código y tests | 2 h 32 min | 64 % | 5 |
| Checks (fmt, clippy, suite de tests) | 20 min 35 s | 9 % | 1 |
| Revisión RDD | 22 min 05 s | 9 % | 7 |
| Correcciones de hallazgos | 35 min 55 s | 15 % | 2 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **3 h 56 min** | **100 %** | **17** |

- Inicio: 2026-10-27 18:40:01 +00:00. Fin: 2026-10-27 22:36:32 +00:00.
- Arranque confirmado: 18:40:01. Activo desde el arranque: **3 h 56 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD review 1 lenses (T1) | 19:13:52 | 19:15:12 | 1 min 20 s |
| Revisión RDD | RDD review 2 (T2) | 19:50:05 | 19:50:05 | 0 min 00 s |
| Revisión RDD | RDD review 2 (T2) retry | 19:51:02 | 19:53:12 | 2 min 10 s |
| Revisión RDD | RDD review 3 (T3) | 20:29:08 | 20:30:41 | 1 min 33 s |
| Revisión RDD | RDD review 4 (last slice) | 21:22:33 | 21:23:27 | 0 min 54 s |
| Checks (fmt, clippy, suite de tests) | final checks Windows and Linux | 21:24:39 | 21:28:12 | 3 min 33 s |
| Checks (fmt, clippy, suite de tests) | final checks rerun | 21:29:43 | 21:35:07 | 5 min 24 s |
| Revisión RDD | codex-R1 | 21:35:20 | 21:42:50 | 7 min 30 s |
| Revisión RDD | RDD review 5 (R1 fix) | 22:08:27 | 22:08:28 | 0 min 01 s |
| Revisión RDD | RDD review 5 (R1 fix) retry | 22:10:24 | 22:11:22 | 0 min 58 s |
| Revisión RDD | codex-R3 | 22:33:21 | 22:36:31 | 3 min 10 s |
| **Checks (fmt, clippy, suite de tests) (total)** | | | | **8 min 57 s** |
| **Revisión RDD (total)** | | | | **17 min 36 s** |

Tarea en segundo plano sin cerrar, no contada: Revisión RDD / codex-R2 desde 22:12:12.

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `656acdf` | feat(pager): store value runs contiguously and move to format 7 | T0 exploration, T1 | 32 min 42 s | Exploración y mapa 5 min 35 s, Código y tests 27 min 07 s |
| `49c4913` | docs: record T1 and its RDD review in the large values execution document | T1, T1 assess | 3 min 19 s | Código y tests 0 min 08 s, Revisión RDD 3 min 11 s |
| `244548d` | feat(tx): read, write and free value runs in transactions | T2 | 33 min 25 s | Código y tests 33 min 25 s |
| `3c40e94` | docs: record T2 and its RDD review in the large values execution document | T2, T2 assess, T3 | 4 min 06 s | Código y tests 3 min 32 s, Revisión RDD 0 min 34 s |
| `9a596b2` | feat(trees): store values up to 256 MiB off their leaf | T3 | 35 min 13 s | Código y tests 35 min 13 s |
| `0a45d13` | docs: record T3 in the large values execution document | T3 assess | 0 min 22 s | Revisión RDD 0 min 22 s |
| `bd0d8f2` | docs: record RDD review 3 in the large values execution document | T4 | 1 min 52 s | Código y tests 1 min 52 s |
| `d0be3f0` | test(trees): crash, failure and byte sweeps with values off their leaf | T4 | 20 min 15 s | Código y tests 20 min 15 s |
| `9000d9e` | docs: record T4 in the large values execution document | T4, T4 assess | 0 min 21 s | Código y tests 0 min 04 s, Revisión RDD 0 min 17 s |
| `899244a` | feat(trees): measure large values and keep the half-page leaf threshold | T5 | 30 min 25 s | Código y tests 30 min 25 s |
| `fa3f291` | docs: record the T5 commit in the large values execution document |  | 0 min 00 s |  |
| `3a0b336` | test(trees): assert an injected commit failure is never swallowed and tidy two value helpers | T5, last slice assess | 2 min 13 s | Código y tests 0 min 20 s, Revisión RDD 1 min 53 s |
| `4eb4807` | docs: record RDD review 4 in the large values execution document |  | 0 min 00 s |  |
| `46fdd12` | test(tx): pause the transfer before the first commit when a run test compares references | final checks | 5 min 19 s | Checks (fmt, clippy, suite de tests) 5 min 19 s |
| `62f7571` | docs: record the final checks and the Codex code review of the large values run | final checks, codex R1 | 15 min 50 s | Checks (fmt, clippy, suite de tests) 15 min 16 s, Correcciones de hallazgos 0 min 34 s |
| `7b4286b` | fix(pager): release a value run's own temporary chunk with the run | codex R1 | 22 min 37 s | Correcciones de hallazgos 22 min 37 s |
| `3a528ec` | test(trees): keep the independent verifier's probes as regression tests | codex R1, R1 fix assess | 2 min 24 s | Revisión RDD 2 min 13 s, Correcciones de hallazgos 0 min 11 s |
| `c0b8d1a` | test(trees): drop a comment with no code behind it in the verifier probes | R1 fix assess | 1 min 19 s | Revisión RDD 1 min 19 s |
| `831d38b` | docs: record the Codex R1 fix, the verification and RDD review 5 | R1 fix assess | 0 min 25 s | Revisión RDD 0 min 25 s |
| `63e7105` | docs: phase log | R1 fix assess | 0 min 04 s | Revisión RDD 0 min 04 s |
| `befeccd` | docs: add the Codex follow-up code review of the large values run | R1 fix assess | 7 min 28 s | Revisión RDD 7 min 28 s |
| `e1cc4ab` | fix(pager): count a retained value run whole until its last version leaves | codex R1 round 2 | 12 min 30 s | Correcciones de hallazgos 12 min 30 s |
| `dfb1191` | docs: phase log | codex R1 round 2, R1 round 2 RDD | 0 min 04 s | Revisión RDD 0 min 01 s, Correcciones de hallazgos 0 min 03 s |
| `6cc0198` | docs: record the second resolution of the Codex code review | R1 round 2 RDD | 1 min 07 s | Revisión RDD 1 min 07 s |
| `57dc55b` | docs: phase log |  | 0 min 00 s |  |
| — | después del último commit | R1 round 2 RDD | 3 min 11 s | Revisión RDD 3 min 11 s |
| **Total** | | | **3 h 56 min** | |

Código y pruebas se llevó dos tercios del activo, repartido en cinco tareas de unos 30 min cada una. La revisión de
Codex de las 21:35 y su seguimiento abrieron la fase de correcciones. El registro no tiene el fin de la ronda 2 de
Codex (`codex-R2`, iniciada a las 22:12:12): no se registró su `bg-end`; su informe se guardó en `befeccd`. El
verificador independiente corrió en paralelo desde las 21:24 y no tiene tramo propio en el registro.

## Informe de cambios (`27a5e5b..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 2.214 | 157 | 1.884 | 224 | 14 |
| `crates/krummi-pager` | 1.549 | 262 | 1.105 | 369 | 13 |
| `crates/krummi-tx` | 996 | 40 | 824 | 120 | 4 |
| `crates/krummi-tree` | 344 | 87 | 256 | 70 | 5 |
| `odd` | 431 | 0 | — | — | 3 |
| `docs` | 393 | 0 | — | — | 3 |
| `README.md` | 7 | 3 | — | — | 1 |
| **Total** | **5.934** | **549** | **4.069** | **783** | **43** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Sin archivos de bloqueo ni generados. No hubo previsión de líneas: el documento de ejecución no la registró.

## Resumen

Una aplicación puede guardar en un árbol valores de hasta 256 MiB con las mismas operaciones de 5.1 y 5.2, y
leerlos y recorrerlos como una sola secuencia contigua de bytes, sin copia, también en modo temporal. Un valor cuya
entrada no cabe en media hoja se guarda en un rango de páginas contiguas con una sola cabecera de integridad, y su
hoja guarda la primera página y la longitud en little endian; quitar, sobrescribir, vaciar o borrar nunca lee las
páginas del valor, así que un valor dañado siempre se puede quitar. El formato pasa a `7`. El modo temporal vive en
un único rango de direcciones reservado, la escritura guarda cada rango en un búfer contiguo, el diario lleva sus
páginas sin cabecera propia, el área temporal conserva cada rango contiguo y lo cuenta entero para el tamaño
máximo, y la transferencia escribe por orden de confirmación. La evaluación de V4 mantuvo el umbral de media hoja.

## Valores por defecto elegidos sin preguntar

- Entrega en una sola rama, sin fusionar, para la revisión del usuario.
- Reserva del modo temporal sin tamaño máximo: 64 GiB de direcciones (D3); con máximo, el menor de los dos.
- Sobrescribir un valor fuera de la hoja siempre escribe un rango nuevo y libera el anterior (D4).
- Umbral de la hoja: media hoja, elegido con la medición (D7).
- No guardar la sonda del verificador que comparaba todos los archivos de la carpeta (L15): AC-V10 ya la cubre y,
  en Windows, la sonda leía vacío el `krummi.data` bloqueado.
- Aceptar sin cambio los hallazgos consultivos listados en *Pendiente*.

## Pendiente (no bloqueante)

- **Hallazgos consultivos aceptados:** con el área temporal fragmentada, cada rango pequeño sin hueco contiguo crea
  su propio archivo mientras un lector lo retiene (descriptores y latencia, no corrección); liberar un trozo propio
  hace llamadas al sistema bajo el cerrojo del área; la prueba de regresión de R1 exige igualdad exacta de bytes de
  `krummi.temp`, que un antivirus que retenga un archivo en Windows podría romper; vaciar o borrar un árbol con una
  hoja dañada falla con daño (A9 lo permite).
- **Límite del modo temporal:** sin tamaño máximo, un almacén temporal no crece más allá de 64 GiB y, con un límite
  de espacio de direcciones (`ulimit -v`), puede fallar al abrir con memoria agotada.
- **CI:** hasta `3c4d536` la CI no ejecutaba el barrido completo de fallos T11 con valores fuera de la hoja
  (`ac_v8_a_failure_at_every_page_operation…`); se añadió al paso de árboles después del cierre.
- **Copia en Engram:** pendiente: Engram rechaza escrituras en esta sesión (L3); el documento de ejecución es la
  única copia.

**Riesgo:** ítem 1 (cambia el formato en disco: rangos de valor, entradas del diario y formato `7`) e ítem 4
(concurrencia: orden de la transferencia, liberación de trozos y contabilidad del área temporal con lectores
abiertos).

## Limpieza manual

Ninguna en el repositorio. Los directorios de compilación de Linux y las copias del verificador ya se borraron; en la
carpeta temporal de la sesión quedan solo archivos pequeños de trabajo (`scratchpad`).
