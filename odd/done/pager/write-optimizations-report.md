# Optimizaciones de escritura terminadas: W28 +7 % en las dos plataformas, W6 −0,33 % en Windows aceptado; rama verificada

Informe de cierre de `write-optimizations` (2026-11-17). Enfoque de ejecución: ODD.
Fuente de producto: [pager-written-pages.prd.md](../../../docs/prd/krummi/pager/pager-written-pages.prd.md) (S1); S2–S5
son optimizaciones sin cambio de comportamiento. Documento de ejecución:
[write-optimizations.md](write-optimizations.md).

Rama `feat/write-optimizations`, creada desde `feat/write-margin` (`10077aa`, sin fusionar). Sin fusionar y sin
publicar: queda para la revisión del usuario.

## Resultado

- **Especificación:**
  - **S1, páginas escritas recordadas** (T1: `56f8a79`, `757901b`, `0c8bea7`; correcciones `cf8fd80`, `b025d58`):
    - una página que el traslado escribe en su sitio queda recordada como comprobada con el código que selló el
      motor (Q1); el daño posterior se detecta al reabrir, como en Voron;
    - una hoja cuyo único cambio fue sobrescribir valores del mismo largo conserva la comprobación de organización
      a través de la confirmación (Q3);
    - las compilaciones de depuración vuelven a comprobar esas hojas (Q4).
  - **S2, compresión por muestras** (T2, `0a5603c`): **revertida** en `f45269d`. Codex R1 mostró que una repetición
    de más de 32 KiB entre muestras quedaba sin comprimir (una serie que comprime 31× se guardaba en crudo). Ninguna
    regla de muestreo cumple Y1 para todo registro, así que se renuncia a la ganancia de W17 (L11).
  - **S3, paso de los intervalos** (T3, `ce8a668`): cada extremo de un intervalo guarda su hoja y su índice, y
    recorre una hoja ya comprobada con una sola comparación.
  - **S4, caché de hojas recientes** (T4): **revertida** en `8b54d0b`. W1 ganaba un 9–12 %, pero W3
    (`carga-masiva`) perdía un 5–6 % en cinco de cinco rondas (L6).
  - **S5, alineación de bucles** (T5): **no adoptada**. W20 no vuelve (0,998) y W6 baja a 0,964 (L7).
  - **Suite final** (código de `b025d58`, igual al de la cabeza: `git diff b025d58 HEAD -- crates` está vacío):
    - `cargo test --workspace`: 868 pasan, 0 fallan, 35 ignoradas;
    - `cargo test --release -p krummi written_pages`: 7 pasan;
    - `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings`: limpios, repetidos sobre la
      cabeza al cerrar.
- **Aceptación (S6, AC-Q10):** A/B de Krummi solo, la cabeza de write-margin (`10077aa`) contra el código final.
  Usa los núcleos de rendimiento, el plan de energía del banco, orden alterno y sumas de comprobación iguales. La
  regla de Q7 es: una carga empeora si su mediana queda por debajo y además queda por debajo en la mayoría de las
  rondas.

  | Carga | Windows | WSL 2 |
  |---|---|---|
  | W28 tabla-actualizacion | **1,075** | **1,071** |
  | W27 tabla-intervalos | 1,053 | 1,166 |
  | W3 carga-masiva | 1,025 | 1,071 |
  | W23 tabla-insercion-en-orden | 1,005 (20 rondas) | 1,085 |
  | W17 valores-grandes-carga | 1,022; 1,010 en un segundo A/B de 10 rondas | 1,036 |
  | W12 lecturas-con-escritor | 1,028 | 1,036 (20 rondas) |
  | W7, W22, W26 | 1,016–1,021 | 1,012–1,021 |
  | W19 multivalor-carga | 1,001 (20 rondas) | 1,026 |
  | **W6 escrituras-concurrentes-16** | **0,997, más lenta en 11 de 20** | 1,004 |
  | Resto | 0,995–1,005 | 0,990–1,023, ninguna empeora con la regla de Q7 |

  - **W28 mejora en las dos plataformas.**
  - **W6 empeora un 0,33 % en Windows** según la letra de Q7. Está por debajo de la dispersión de una corrida, pero
    Q7 deja esa decisión al usuario, que **la aceptó** (2026-11-17, «Sí»; L19): AC-Q10 cumplido.
  - En WSL, ninguna carga empeora después de repetir con 20 rondas las que la primera matriz marcaba (L13, L14).
- **Ejecuciones completas con `--solo-krummi-voron`, código final** (L17):
  - **Primera corrida:**
    - Windows: **27 de 29**; incumplen `lecturas-con-escritor` (0,73) y `escritor-con-lectores` (0,79).
    - WSL: **28 de 29**; incumple `lecturas-al-azar-16` (0,68).
    - Informes: [Windows](../../../docs/benchmark/2026-11-17-windows-write-optimizations-final-1.md) y
      [WSL](../../../docs/benchmark/2026-11-17-linux-write-optimizations-final-1.md).
    - Voron se movió tanto como Krummi, y los A/B no dan caída en esas cargas, así que se repitió sin cambios.
  - **Segunda corrida: 29 de 29 en las dos plataformas.** Informes:
    [Windows](../../../docs/benchmark/2026-11-17-windows-write-optimizations-final-2.md) y
    [WSL](../../../docs/benchmark/2026-11-17-linux-write-optimizations-final-2.md).
  - Los informes del código intermedio `8b54d0b` (L8, L9; 29 de 29) siguen en `docs/benchmark/` con su fecha.
- **Revisiones y verificación:**
  - **Verificadores independientes:**
    - T1 y T2: S1 y S2 pasan, sin bloqueos; recuperaron el diario de cada build con la otra (L4).
    - T4: pasa, sin bloqueos (L5).
    - El arreglo `b025d58` (solo depuración) no tuvo verificador propio; lo revisaron RDD y la ronda 3 de Codex.
  - **RDD:** aprobó los siete tramos (tabla de abajo).
  - **Codex:**
    - PRD: dos rondas, aprobado ([ronda 1](../../../docs/reviews/pager-written-pages/2026-11-16-prd.md),
      [ronda 2](../../../docs/reviews/pager-written-pages/2026-11-16-prd-02.md),
      [resoluciones](../../../docs/reviews/pager-written-pages/2026-11-16-prd-resolutions.md)).
    - Código: tres rondas, aprobado en la tercera
      ([ronda 1](../../../docs/reviews/write-optimizations/2026-11-17-code.md),
      [ronda 2](../../../docs/reviews/write-optimizations/2026-11-17-code-02.md),
      [ronda 3](../../../docs/reviews/write-optimizations/2026-11-17-code-03.md),
      [resoluciones](../../../docs/reviews/write-optimizations/2026-11-17-code-resolutions.md)).
- **Hallazgos corregidos por el camino:**
  - **R1 de Codex** (las muestras de T2 no ven repeticiones largas): T2 revertida (`f45269d`).
  - **R2 de Codex** (un `Vec<bool>` nuevo en cada confirmación, contra Q6) y **R4 de Codex** (un mutex de depuración en
    cada acierto de C2, contra C6), corregidos en `cf8fd80`:
    - un `PageChange` por página en lugar de las marcas;
    - un arreglo de atómicos solo en depuración en lugar del mapa.
  - **R5 de Codex** (mayor, causado por el arreglo de R4): si el traslado escribía otra página del mismo slot entre el
    acierto y la consulta, se saltaba la comprobación de Q4 en depuración. Corregido en `b025d58`, con prueba en RED y
    luego en GREEN.
  - **R3 de Codex** (la matriz de Q7 incompleta, y después mal resumida): matriz completa en las dos plataformas, con
    cifras recalculadas por la regla de Q7 (L13, L14).
  - **R4-001 de RDD**: una aserción del orden de las páginas preparadas (`0c8bea7`).
  - **Avisos de RDD del último tramo** (R2-001, R2-003): L17 corregido (`bd3f934`).

## Cobertura de los criterios de aceptación

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-Q1 | `ac_q1_a_leaf_the_transfer_wrote_is_read_without_checks_until_the_store_reopens` | PASS (debug y release) |
| AC-Q2 | `ac_q2_the_transfer_remembers_pages_but_not_values_off_their_leaf` | PASS |
| AC-Q3 | `ac_q3_a_page_whose_transfer_write_fails_stays_forgotten` | PASS |
| AC-Q4 | `ac_q4_damage_of_a_written_page_is_found_after_reopening` | PASS |
| AC-Q5 | `ac_q5_same_length_overwrites_keep_the_layout_check` | PASS (debug y release) |
| AC-Q6 | `ac_q6_any_other_change_of_the_leaf_drops_the_check` | PASS (debug y release) |
| AC-Q7 | `ac_q7_debug_builds_check_a_leaf_that_kept_its_check`, y `a_kept_content_replaced_after_its_hit_is_still_checked` (R5) | PASS (solo depuración) |
| AC-Q8 | `cargo test --release -p krummi written_pages` | PASS, 7 pasan |
| AC-Q9 | `ac_q9_concurrent_reads_while_overwrites_are_transferred` | PASS |
| AC-Q10 | A/B de Q7 y las dos ejecuciones completas | W28 mejora en las dos plataformas; W6 −0,33 % en Windows, **aceptado por el usuario** (L19) |

Todas las pruebas están en `crates/krummi/src/written_pages_tests.rs`, salvo la de R5, que está en
`crates/krummi-pager/src/checked.rs`.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `ce8a668..757901b` (T1, ítem 1) | riesgo, resiliencia, fiabilidad | aprobado | R4-001 SUGGESTION, aserción en `0c8bea7`. 2 WARNING descartados con evidencia: la marca de serie de valores la pone la única ruta que guarda series, y `copy_for_write` lee como `Snapshot::page` | `review-8e27299c4cffef65` |
| `aa82944..ce8a668` (T2, T3) | riesgo, resiliencia, fiabilidad | aprobado | 3 WARNING sobre la última clave de un extremo del intervalo, descartados: `guard` detiene los dos extremos ante cada `Ok(None)` y error. 1 SUGGESTION, cubierta por las pruebas de alternancia al azar | `review-a2930b6b3ea7e376` |
| `757901b..3f35631` (T4, ítem 6) | riesgo, resiliencia, fiabilidad | aprobado | R1-001 (setter público), oculto en `49fc00d`. R4-001 se dejó al A/B. R3-001 lo comprobó el verificador. R3-002 se mantuvo. T4 se revirtió después | `review-6e9928eb4aa5471f` |
| `3f35631..e0e4fbe` | fiabilidad, legibilidad | aprobado | R2-001 y R2-002 (identidad del código medido), respondidos en L10 | `review-d6eb8d83236a06a9` |
| `e0e4fbe..5bbeb4e` | riesgo, resiliencia, fiabilidad | aprobado | R3-001 WARNING (orden de memoria en depuración sin prueba entre hilos) y R3-002 SUGGESTION (`PageChange::new(true, true)`), pendientes | `review-56d68a6bfbe776dd` |
| `5bbeb4e..6dc29e0` (R5) | fiabilidad, resiliencia | aprobado | R3-001 WARNING (como el anterior), pendiente. R3-002 SUGGESTION: lo dice la documentación de `layout_kept`. R3-003 (hora y código de L14), corregido | `review-c10d9a58577cf96f` |
| `6dc29e0..9bb3c00` | legibilidad | aprobado | R2-001 WARNING y R2-003 SUGGESTION, L17 corregido en `bd3f934`. R2-002 SUGGESTION: nota fechada en las resoluciones | `review-a242d2b9304e896e` |

El primer intento de revisar el run entero (`10077aa..757901b`, 2.839 líneas) paró en
`lens_context_budget_exceeded` sin crear autoridad, así que se dividió (L3). Las evaluaciones sin revisión
(`under_budget`) no se registraron una a una. Este informe y el archivado van después del último tramo revisado.

## Tiempo

- **Estimación:** 8 h – 12 h. Arranque real: 2026-11-16 18:38:06. Fin de la verificación: 2026-11-17 12:04:56.
- **Duración activa desde el arranque: 17 h 26 min** según el registro, un 45 % sobre el máximo de la estimación.
  - La aceptación es el 50 % del activo. En segundo plano corrieron 12 h 38 min de mediciones: la matriz de Q7 en
    WSL y su repetición de 20 rondas (4 h 44 min), que pidió la R3 de Codex, y tres pares de ejecuciones completas en
    lugar de uno (3 h 22 min): uno sobre `8b54d0b` y dos sobre el código final.
  - El registro no separa las horas en que la sesión solo esperaba esas mediciones. Por ejemplo, las 2 h 23 min de
    «Documentación» de `df8abb1` y de «Revisión RDD» de `9bb3c00` fueron esperas de la repetición en WSL y de las
    corridas completas, contadas en la última fase marcada.
- **Esperas por el usuario:** ninguna. El usuario pidió terminar sin preguntar.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 1 min 38 s | 0 % | 1 |
| Código y tests | 40 min 40 s | 4 % | 2 |
| Checks (fmt, clippy, suite de tests) | 1 h 02 min | 6 % | 3 |
| Aceptación | 8 h 47 min | 50 % | 4 |
| Revisión RDD | 3 h 52 min | 22 % | 4 |
| Correcciones de hallazgos | 38 min 24 s | 4 % | 1 |
| Documentación | 2 h 23 min | 14 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **17 h 26 min** | **100 %** | **17** |

- Inicio: 2026-11-16 18:38:06 +00:00. Fin: 2026-11-17 12:04:56 +00:00.
- Arranque confirmado: 18:38:06. Activo desde el arranque: **17 h 26 min**; antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | ab-opt | 20:47:05 | 21:09:16 | 22 min 11 s |
| Aceptación | ab-opt-2 | 21:22:41 | 21:45:14 | 22 min 33 s |
| Aceptación | ab-bisect | 21:52:25 | 22:22:23 | 29 min 58 s |
| Aceptación | ab-t4b | 23:08:30 | 23:42:44 | 34 min 14 s |
| Aceptación | ab-align | 23:45:51 | 00:31:16 | 45 min 25 s |
| Aceptación | full-windows | 00:31:42 | 00:53:25 | 21 min 43 s |
| Aceptación | full-linux | 00:53:26 | 00:53:47 | 0 min 21 s |
| Aceptación | full-linux-2 | 00:54:09 | 01:39:27 | 45 min 18 s |
| Aceptación | ab-linux | 01:39:57 | 02:08:17 | 28 min 20 s |
| Revisión RDD | codex-R1 | 02:20:16 | 02:31:47 | 11 min 31 s |
| Aceptación | q7-windows | 02:56:57 | 03:38:36 | 41 min 39 s |
| Aceptación | q7-w23-w21 | 03:38:52 | 03:51:22 | 12 min 30 s |
| Aceptación | q7-linux | 03:51:37 | 06:11:25 | 2 h 19 min |
| Aceptación | q7r-wsl | 06:12:26 | 08:37:54 | 2 h 25 min |
| Revisión RDD | codex-R2 | 08:44:30 | 08:53:51 | 9 min 21 s |
| Aceptación | q7r-windows | 09:04:42 | 09:34:26 | 29 min 44 s |
| Revisión RDD | codex-R3 | 09:36:30 | 09:41:15 | 4 min 45 s |
| Aceptación | full-windows | 09:42:20 | 10:03:51 | 21 min 31 s |
| Aceptación | full-linux | 10:03:51 | 10:50:03 | 46 min 12 s |
| Aceptación | full-windows-2 | 10:51:54 | 11:15:31 | 23 min 37 s |
| Aceptación | full-linux-2 | 11:15:31 | 11:59:39 | 44 min 08 s |
| Aceptación | ab-w17-10rounds | 12:00:37 | 12:02:24 | 1 min 47 s |
| Aceptación | ab-w17-10rounds-2 | 12:02:44 | 12:04:31 | 1 min 47 s |
| **Aceptación (total)** | | | | **12 h 38 min** |
| **Revisión RDD (total)** | | | | **25 min 37 s** |

Notas sobre el registro:
- La primera `full-linux` (00:53) se detuvo al instante, porque quedaba el informe anterior en `~/wm` (L8).
- Los dos `ab-w17` cubren solo el tramo final de cada A/B. El primero no dio cifra (L17).
- Las revisiones de Codex aparecen como «Revisión RDD» porque es la fase de revisión del registro.

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `10bba76` | docs(prd): add the remembered checks of what the engine writes |  | 0 min 00 s |  |
| `aa82944` | docs(odd): add the write-optimizations execution document | T0 | 0 min 31 s | Plan (documento de ejecución) 0 min 31 s |
| `0a5603c` | perf(journal): store large incompressible records without compressing them whole | T0, T1-T3 | 15 min 11 s | Plan (documento de ejecución) 1 min 07 s, Código y tests 14 min 04 s |
| `ce8a668` | perf(tree): step through a clear leaf of a range without the full checks | T1-T3 | 12 min 41 s | Código y tests 12 min 41 s |
| `56f8a79` | perf(pager): remember the pages the transfer writes and the leaves whose values changed in place | T1-T3, T1 | 19 min 17 s | Código y tests 11 min 27 s, Checks (fmt, clippy, suite de tests) 7 min 50 s |
| `757901b` | docs(prd): annotate AC-I9 and AC-S5 for the pages the transfer remembers | T1 | 0 min 01 s | Checks (fmt, clippy, suite de tests) 0 min 01 s |
| `0c8bea7` | fix(pager): assert the order of staged pages and drop a release-only unused import | T1, T2 | 15 min 57 s | Checks (fmt, clippy, suite de tests) 0 min 25 s, Revisión RDD 15 min 32 s |
| `3f35631` | perf(tree): remember the last leaf a write transaction reached | T2 | 28 min 48 s | Revisión RDD 28 min 48 s |
| `49fc00d` | fix(tx): skip the remembered leaf in a failed transaction and hide its setter | T2, T4 | 35 min 44 s | Checks (fmt, clippy, suite de tests) 35 min 40 s, Revisión RDD 0 min 04 s |
| `197d41c` | perf(tx): back off from remembering leaves when keys do not come close together | T4, T6 | 35 min 45 s | Checks (fmt, clippy, suite de tests) 0 min 48 s, Aceptación 34 min 57 s |
| `3c11fba` | perf(tree): take the remembered leaf only on a hit and remember it on the usual walk | T6 | 1 h 45 min | Aceptación 1 h 45 min |
| `b11e425` | docs(odd): record T4 and the verifiers of the write-optimizations run | T6 | 35 min 18 s | Aceptación 35 min 18 s |
| `8b54d0b` | revert(tree): drop the recent-leaf cache | T6 | 0 min 07 s | Aceptación 0 min 07 s |
| `e0e4fbe` | docs(odd): record the acceptance runs of the write-optimizations run | T6, T5 | 2 h 35 min | Código y tests 2 min 28 s, Aceptación 2 h 33 min |
| `effd169` | docs(odd): record the last-slice review of the write-optimizations run | T6 | 1 min 02 s | Aceptación 1 min 02 s |
| `2417fd9` | docs(odd): mark the code review of the write-optimizations run | T6, T7 | 0 min 07 s | Aceptación 0 min 06 s, Revisión RDD 0 min 01 s |
| `35f3b5d` | docs(odd): mark the first code review of the write-optimizations run | T7 | 12 min 03 s | Revisión RDD 12 min 03 s |
| `f45269d` | revert(journal): compress every commit record whole again | T7 | 0 min 01 s | Revisión RDD 0 min 01 s |
| `cf8fd80` | fix(pager): carry the keep-check flag without a new allocation and record kept checks without a lock | T7 | 14 min 50 s | Revisión RDD 14 min 50 s |
| `5bbeb4e` | docs(odd): record the first Codex code review of the write-optimizations run | T7, R3 | 3 h 25 min | Aceptación 3 h 15 min, Revisión RDD 9 min 47 s |
| `df8abb1` | docs(odd): record the Q7 matrix, the last-slice review and the code review resolutions | R3, rdd-fixes, L12, T7 | 2 h 31 min | Checks (fmt, clippy, suite de tests) 5 min 55 s, Aceptación 0 min 51 s, Revisión RDD 1 min 25 s, Documentación 2 h 23 min |
| `b025d58` | fix(pager): check a kept leaf's layout in debug builds when its slot changes after the hit | T7, R5 | 20 min 09 s | Checks (fmt, clippy, suite de tests) 12 min 12 s, Correcciones de hallazgos 7 min 57 s |
| `6dc29e0` | docs(odd): record the second Codex code review round and the corrected Q7 figures | R5 | 30 min 11 s | Correcciones de hallazgos 30 min 11 s |
| `8bff765` | docs(odd): record the review of the R5 fix | R5, rdd-R5 | 1 min 41 s | Revisión RDD 1 min 25 s, Correcciones de hallazgos 0 min 16 s |
| `886db96` | docs(odd): record the third Codex code review round | rdd-R5 | 5 min 12 s | Revisión RDD 5 min 12 s |
| `9bb3c00` | docs(bench): record the complete runs on the final write-optimizations code | rdd-R5 | 2 h 23 min | Revisión RDD 2 h 23 min |
| — | después del último commit | rdd-R5 | 0 min 08 s | Revisión RDD 0 min 08 s |
| **Total** | | | **17 h 26 min** | |

Las etiquetas siguen la última marca de cada momento, no el commit. Por eso `0c8bea7`–`49fc00d` llevan «T2» y
«Revisión RDD»: entonces corrían a la vez la revisión de T1 y el escritor de T4. La revisión RDD del último tramo y el
cierre (11 min después de `9bb3c00`) quedaron fuera del registro, porque `end` se marcó al terminar la verificación.

## Informe de cambios (`10077aa..9bb3c00`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 2.078 | 0 | — | — | 28 |
| `crates/krummi` | 913 | 37 | 775 | 99 | 5 |
| `crates/krummi-pager` | 351 | 34 | 226 | 108 | 7 |
| `crates/krummi-tree` | 291 | 54 | 224 | 56 | 4 |
| `odd` | 270 | 0 | — | — | 2 |
| `crates/krummi-tx` | 137 | 18 | 89 | 41 | 2 |
| **Total** | **4.040** | **143** | **1.314** | **304** | **48** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell,
JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

`docs` incluye el PRD (`10bba76`) con su traducción, las revisiones de Codex y los informes del banco. De las 775
líneas de código de `crates/krummi`, casi todas son las pruebas de `written_pages_tests.rs`. T2 y T4 se revirtieron,
así que no cuentan en el total neto. No hubo previsión de líneas.

## Resumen

Krummi deja de volver a verificar lo que él mismo acaba de escribir:
- **Páginas escritas.** Una página que el motor escribe en el archivo de datos queda recordada como comprobada con
  el código que él mismo selló. Si alguien la daña desde fuera con el almacén abierto, se detecta al reabrir, como
  en Voron.
- **Hojas sobrescritas.** Una hoja en la que solo se sobrescribieron valores del mismo largo conserva su
  comprobación de organización después de confirmar. En depuración se sigue comprobando.
- **Intervalos.** El recorrido de un intervalo avanza dentro de una hoja ya comprobada con una sola comparación.

Ganancias contra la cabeza de write-margin:

| Carga | Windows | WSL |
|---|---|---|
| W28 tabla-actualizacion | +7,5 % | +7,1 % |
| W27 tabla-intervalos | +5,3 % | +16,6 % |
| W3 carga-masiva | +2,5 % | +7,1 % |

La compresión por muestras, la caché de hojas recientes y la alineación de bucles se probaron y se dejaron fuera.
Romper una regla del producto, hacer más lenta otra carga o no ganar nada no compensaba.

## Valores por defecto elegidos sin preguntar

- **Ruta:**
  - T1, T2 y T3: escritores en paralelo, por ser unidades independientes y por el límite de contexto de la sesión.
  - T4: escritor después de T1.
  - T5 y T6: en línea.
- **T2 revertida** por la R1 de Codex en lugar de buscar otra regla de muestreo: ninguna cumple Y1 para todo registro.
- **T4 revertida** porque W3 perdía un 5–6 %. No se probó una caché limitada a escrituras en orden, que podría
  conservar el +9–12 % de W1.
- **T5 no adoptada:** W20 no volvía y W6 empeoraba.
- **Repetir la medición en lugar de aceptar la primera matriz de Q7:**
  - W6, W19 y W23 de Windows, con 20 rondas;
  - las cargas que WSL marcaba, con 20 o 6 rondas.
- **Repetir la ejecución completa** al ver 27 y 28 de 29, en lugar de aceptarla o investigar una regresión, porque
  los A/B intercalados de esas cargas no daban caída. Se conservan las dos corridas.
- **Informes nuevos (`-final-1`, `-final-2`)** en lugar de pisar los de `8b54d0b`, que L8 y L9 citan.
- **Un A/B de W17** ante su caída en las corridas completas: 1,010, sin empeorar.
- **Avisos que se quedan como están:**
  - el nombre de la prueba de AC-U6, que ya no coincide con lo que comprueba, porque lo citan informes archivados;
  - `PageChange::new(true, true)`, porque una aserción contradiría la prueba que fija ese caso.
- **Módulo de archivo:** `pager`.

## Pendiente (no bloqueante)

- **W6 en Windows, −0,33 %** (20 rondas, más lenta en 11 de 20): aceptado por el usuario (L19), no queda
  pendiente. Está por debajo de la dispersión de una corrida, y en WSL W6 da 1,004.
- **Orden de memoria del registro de depuración** (R3-001 de RDD, dos tramos): se sostiene por el orden de sus
  escrituras y lecturas, que ninguna prueba ejerce entre hilos. No bloquea porque solo afecta a la comprobación extra
  de las compilaciones de depuración, nunca a lo que devuelve una lectura.
- **`PageChange::new(true, true)`** devuelve `Created` y descarta la marca de conservar la comprobación (R3-002 de
  RDD). No bloquea porque una página creada nunca se marca.
- **Caché de escrituras en orden** (W1, +9–12 % medido con T4): sin probar; sería un cambio aparte, con su A/B.
- **La ganancia de W17 de T2:** pediría una regla exacta de cuándo comprimir que cumpla Y1. Queda para un PRD o un
  ODD aparte.
- **R2 de la revisión de write-margin** (recolección de versiones de un `ScratchWriter` reutilizado): fuera del
  alcance de este run. Sigue anotado en el informe de write-margin.

**Risk:**
- **ítem 1:** las páginas escritas quedan recordadas, así que el daño posterior se detecta solo al reabrir;
- **ítem 2:** relaja comprobaciones de integridad;
- **ítem 3:** el PRD reescribe la garantía I4 y anota AC-I9 y AC-S5 de los ascendientes;
- **ítem 4:** el registro de depuración usa atómicos entre el traslado y las lecturas.

Lo revisaron un verificador independiente, RDD (ítem 1 nombrado en el tramo de T1) y tres rondas de Codex, sin
bloqueos abiertos.

## Limpieza manual

Se pueden borrar; están todos fuera de git:
- `work/wm/` (12 GB): fuentes, binarios y almacenes de los A/B y de las ejecuciones completas;
- `work/rdd-clone/` (43 MB): el clon local de la revisión de T2 y T3;
- `~/wm` en WSL (1,4 GB).
