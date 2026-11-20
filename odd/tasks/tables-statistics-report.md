# Estadísticas de las tablas: implementadas, verificadas y aprobadas por Codex

Informe de cierre de `tables-statistics` (2026-11-20). Enfoque de ejecución: ODD.
Fuente de producto: [tables-statistics.prd.md](../../docs/prd/krummi/tables/tables-statistics.prd.md) (PRD hijo 7.3).
Documento de ejecución: [tables-statistics.md](tables-statistics.md).

Rama `feat/tables-statistics`, desde `main` en `ba7f27a`, sin fusionar y sin publicar. El último commit de código es
`879b73c`. La rama queda para tu revisión: la fusión y la publicación son decisiones tuyas.

## Resultado

- **Especificación:** se cumplen ST1–ST18 y AC-ST1–AC-ST14 del PRD; AC-ST15 son las mediciones de este informe.
  - **Biblioteca:**
    - el contador de modificaciones persistente de cada tabla y el formato 13;
    - estadísticas por columna (nulos, valores grandes, distintos, frecuentes, histograma) y por prefijo de índice;
    - estimar igualdades, intervalos e igualdades en varias columnas sin suponer nunca: muestrea al momento si faltan;
    - la memoria compartida del almacén, el hilo de segundo plano que guarda y renueva, analizar en una escritura y en
      el almacén, y leer las estadísticas;
    - el muestreo de filas guardadas en tramos leyendo solo las páginas que usa, sin la suma del tramo (ST1).
  - **Tests:** `cargo test --workspace` pasa: 59 binarios, 1.022 tests sin fallos y 35 ignorados (última corrida
    completa, sobre `879b73c`).
  - **Lint:** `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` pasan.
- **Aceptación (ST15):** medido en Windows y en Linux (sección *Mediciones*). La primera estimación sobre W23 tarda
  82,4 ms en Windows y 79,8 ms en Linux. El hilo que estima cuesta a W23 un 2,9 % y un 6,5 %, y a W28 un 3,7 % y un
  1,6 %. En el A/B, todas las cargas quedan en ±3 % salvo cuatro lecturas que no toca el cambio (W7, W9 y W20 en
  Windows, W18 en Linux), con su causa sin medir.
- **Revisiones y verificación:**
  - **Verificador independiente** sobre `ba7f27a..ea5d7b3` (989 tests): dos bloqueos, B1 (leer las estadísticas no
    veía las de memoria) y B2 (faltaba leer las de índice en una escritura), corregidos en `4386d3b` y confirmados en
    la segunda pasada; avisos A1–A7 (L9–L10 del documento).
  - **RDD:** aprobó todas las porciones (tabla abajo).
  - **Codex:** la revisión del PRD quedó aprobada en la ronda 2; la del código, sobre el rango entero, en la ronda 13,
    con los 21 hallazgos verificados ([resoluciones](../../docs/reviews/tables-statistics/2026-11-19-code-resolutions.md),
    último informe [2026-11-20-code-04.md](../../docs/reviews/tables-statistics/2026-11-20-code-04.md)). La anotación
    de N11 en el PRD de las tablas quedó aprobada en su seguimiento
    ([2026-11-20-prd.md](../../docs/reviews/tables/2026-11-20-prd.md)).
- **Hallazgos corregidos durante la ejecución:** los 21 de Codex, entre ellos:
  - **R1:** un daño al muestrear en una escritura no hacía fallar la escritura (T11).
  - **R2:** el muestreo leía tramos enteros con valores grandes. Se resolvió con lecturas parciales sin verificar
    (`91111a3`), la clasificación por longitud guardada y guardas de pánico por columna (`da56d24`), y el gancho que
    cuenta las páginas leídas (`a1946cf`, `aa0b474`).
  - **R11:** las estadísticas de memoria no se retiraban con los cambios confirmados. Ahora se retiran en el punto sin
    vuelta del commit y cada construcción se valida por el commit de su instantánea (`265ed1f`, `bc82fd7`).
  - **R12:** cerrar no cancelaba un muestreo en curso.
  - **R13:** faltaban casos de fallos (disco lleno, entrada y salida, memoria agotada, daño) en el guardado, la
    renovación y analizar.
  - **R21:** el hilo que estima de ST15 juntaba avisos y hacía en W28 menos rondas que commits (`879b73c`; las
    estimaciones se midieron otra vez).
  - **RDD:** tres pruebas de carreras esperaban 100 ms fijos; ahora esperan el contador de muestreos (`f5a2cd1`,
    `51a2779`).

## Cobertura de los criterios de aceptación

| Criterio | Test o comprobación | Resultado |
|---|---|---|
| AC-ST1 | `ac_st1_the_estimates_follow_the_rows`, `ac_st1_a_large_table_is_sampled` | PASS |
| AC-ST2 | `ac_st2_the_first_estimate_samples_and_the_others_use_memory`, `ac_st2_what_an_estimate_samples_is_saved_in_the_background` | PASS |
| AC-ST3 | `ac_st3_each_row_operation_adds_what_st4_says` | PASS |
| AC-ST4 | `ac_st4_stale_statistics_are_used_and_renewed` | PASS |
| AC-ST5 | `ac_st5_the_newer_build_wins_in_either_order` | PASS |
| AC-ST6 | `ac_st6_table_changes_keep_or_drop_the_statistics` | PASS |
| AC-ST7 | `ac_st7_a_save_holds_the_turn_like_any_write` | PASS |
| AC-ST8 | `stats_fault_tests` (5 tests) y los `r13_*` de `codex.rs` | PASS |
| AC-ST9 | `ac_st9_the_errors_of_an_estimate`, `ac_st9_the_errors_of_reading_and_analyze` | PASS |
| AC-ST10 | los casos de AC-ST2, AC-ST4 y AC-ST5 sobre un almacén temporal (`stores`) | PASS |
| AC-ST11 | `ac_st11_a_store_of_format_12_is_of_a_previous_format` | PASS |
| AC-ST12 | `ac_st12_the_statistics_read_are_those_of_the_rows`, `ac_st12_staleness_and_an_empty_table`, `ac_st12_a_sample_does_not_read_the_pages_of_large_values`, `ac_st12_a_sample_counts_the_pages_before_the_values` | PASS |
| AC-ST13 | `ac_st13_index_prefixes_and_the_biased_backoff` | PASS |
| AC-ST14 | `ac_st14_a_write_samples_its_own_table_for_itself` | PASS |
| AC-ST15 | este informe, sección *Mediciones* | PASS |

## RDD reviews

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `b04cba7..8d1fb6e` (T2) | riesgo | aprobada | ninguno | `review-73b4e0b9c2555c7d` |
| `8d1fb6e..0ddf2a1` (T3–T5) | riesgo, resiliencia, fiabilidad | aprobada | ninguno | `review-2013388a3a92733c` |
| `0ddf2a1..ea5d7b3` (T6, T7a) | riesgo, fiabilidad | aprobada | AVISO: la recolección de prefijos guarda los elementos de cada fila; sin cambio, acotada por IX10 y ~30.000 filas | `review-6f99665bc52bc8dd` |
| `f6e56d9..4386d3b` (correcciones del verificador) | fiabilidad | aprobada | AVISO: el hilo no se reinicia; sin cambio, solo termina al parar el almacén | `review-e10b0bcd401bbbf2` |
| `4386d3b..bf69c8b` | riesgo, resiliencia, fiabilidad | aprobada | AVISOS: `find_write_table` abre todas las tablas con el turno; `write_unless` sin prueba propia; r7 ordenada por una espera fija (corregida en `f5a2cd1`) | `review-8a1ddb485d4376ba` |
| `bf69c8b..6b6430a` | riesgo, resiliencia, fiabilidad | aprobada | AVISOS: r11 y r16 con espera fija (corregidas en `f5a2cd1`); retirar antes del commit (ya sustituido en `265ed1f`) | `review-6ebd4c9a8f72400c` |
| `6b6430a..88bacde` | riesgo, resiliencia, fiabilidad | aprobada | AVISOS: `Integrity::Unchecked` relaja la integridad por diseño (ST1); el conjunto de destinos retirados no se poda; los caminos sin verificar de hojas al azar y de escritura sin prueba de daño; aislamiento de la prueba de commit (ya sustituida en `da56d24`). SUGERENCIA: `test-hooks` fuera de producción | `review-799413ea30a2ac85` |
| `88bacde..c6b0331` | riesgo, resiliencia, fiabilidad | aprobada | AVISO: las guardas de pánico no sirven con `panic=abort` (el proyecto compila con desenrollado) | `review-85d2949481d8f1fc` |
| `c6b0331..d6e3a8a` | fiabilidad | aprobada | AVISOS: la espera sin plazo (corregida en `51a2779`); el orden contador-después-de-instantánea solo documentado (se cumple por el código) | `review-ecaa3549b076a9b1` |
| `d6e3a8a..51a2779` | fiabilidad | aprobada | SUGERENCIA: el mensaje del plazo podría mostrar los contadores | `review-fe219df848f1e6ad` |
| `51a2779..f1aa8f0` (gancho del banco, documentos) | legibilidad, fiabilidad | aprobada | AVISO R2-001: la primera estimación tomaba `n` por posición (corregido en `b80503a`). SUGERENCIAS: el error del hilo se pierde si también falla la escritura; ninguna prueba corre W28 con el hilo; un valor desconocido de `KRUMMI_BENCH_ESTIMATES` mide sin estimaciones y sin aviso; la entrada de `Cargo.lock` (de `bc82fd7`, commiteada aparte en `ce991db`) | `review-fb0cd071ffaf8627` |
| `f1aa8f0..106d3c9` (R21, revisiones, documentos) | fiabilidad | aprobada | ninguno | `review-dfbad5d365070c30` |

El rango entero de las correcciones de Codex (4.204 líneas) superó el presupuesto de contexto de las lentes
(`lens_context_budget_exceeded`); se revisó en cuatro porciones, cada una con HEAD separado en su último commit.
Después de `106d3c9` solo hay documentos del cierre (este informe y el archivado).

## Mediciones (ST15)

Banco en modo `full`, una carga por proceso, bajo el plan de energía del banco; en Windows, en los núcleos de
rendimiento (máscara `0xFFFF`) con el lanzador del banco; en Linux, WSL 2 con todo dentro del sistema de archivos de
WSL. Cada corrida guarda la mediana de ns/op de las repeticiones medidas 1–3; la tabla da la mediana de las corridas.
El gancho `KRUMMI_BENCH_ESTIMATES` (`d4ff730`, corregido en `b80503a` y `879b73c`) solo escribe en stderr; el A/B
corrió con `d4ff730`, cuyas corridas sin el gancho no cambian después.

### Estimaciones (ST15), `879b73c`, 5 rondas

Corridas el 2026-11-20: Windows 13:48–13:58 UTC, Linux 13:58–14:17 UTC. Repiten las de `d4ff730`, cuyo hilo juntaba
en una ronda los avisos que llegaban mientras estimaba y hacía en W28 entre 996 y 999 rondas para 1.000 commits
(hallazgo R21 de Codex); ahora cada commit tiene su ronda.

| Medición | Windows | Linux (WSL 2) |
|---|---|---|
| Primera estimación sobre W23 (1.000.000 filas, muestrea una vez) | mediana 82,4 ms (78,9–107,8; 20 tomas) | mediana 79,8 ms (68,6–90,9; 20 tomas) |
| W23 sin hilo / con hilo (ns/op) | 2.134,9 / 2.199,0 (0,971 op/s) | 7.812,1 / 8.358,0 (0,935 op/s) |
| W28 sin hilo / con hilo (ns/op) | 8.036,9 / 8.341,8 (0,963 op/s) | 12.927,6 / 13.134,3 (0,984 op/s) |
| Rondas de estimación del hilo por proceso | W23 y W28 1.000 | W23 y W28 1.000 |
| Muestreos por proceso con el hilo | W23 29–31; W28 47–49 | W23 73–77; W28 53 |

- La primera estimación se toma una vez por repetición (4 por proceso, 5 procesos); cada una muestrea una vez.
- El hilo pide una estimación de cada columna tras cada lote de 1.000 filas, una ronda por commit. Los muestreos son
  los del contador del almacén: el primero y las renovaciones de las viejas.
- **W23 con hilo** cuesta un 2,9 % en Windows y un 6,5 % en Linux. En Windows, las 5 rondas con hilo (2.184,8–2.387,7)
  superan a 3 de las 5 sin hilo; las otras dos (2.185,6 y 2.192,5) se cruzan con ellas. En Linux, las 5 con hilo
  (8.138,0–8.477,7) superan a 4 de las 5 sin hilo.
- **W28 con hilo** cuesta un 3,7 % en Windows (con dos rondas altas, 8.505,0 y 8.656,0) y un 1,6 % en Linux, donde
  las 5 rondas con hilo quedan por encima de 4 de las 5 sin hilo.

### A/B de una sola carga, `ba7f27a` contra `d4ff730`

Tres rondas, alternando qué versión va primero; los datos de cada carga coinciden entre las dos versiones en cada
ronda. W13 se mide dentro del proceso de W12 (no tiene proceso propio); una primera corrida que la pidió sola se cortó
con «carga no válida: W13» y se repitió sin ella. Corridas el 2026-11-20: Windows 08:23–09:28 UTC, Linux 09:28–11:38
UTC. «op/s» es la razón de la rama sobre la base: más de 1 es más rápida.

| Carga | Windows base / rama (ns/op) | op/s | Linux base / rama (ns/op) | op/s |
|---|---|---|---|---|
| W1 | 787,8 / 749,7 | 1,051 | 6.583,0 / 6.503,8 | 1,012 |
| W2 | 8.433,5 / 8.396,4 | 1,004 | 15.248,5 / 15.107,1 | 1,009 |
| W3 | 2.408,3 / 2.384,1 | 1,010 | 2.777,3 / 2.738,7 | 1,014 |
| W4 | 123.403,9 / 120.026,6 | 1,028 | 5.642.878,7 / 5.604.614,2 | 1,007 |
| W5 | 52.466,6 / 51.537,3 | 1,018 | 2.756.062,3 / 2.642.651,4 | 1,043 |
| W6 | 43.027,3 / 43.053,1 | 0,999 | 686.222,7 / 686.683,6 | 0,999 |
| W7 | 909,5 / 944,5 | 0,963 | 1.141,4 / 1.169,6 | 0,976 |
| W8 | 283,6 / 408,0 | 0,695 | 369,7 / 383,8 | 0,963 |
| W9 | 272,4 / 283,6 | 0,960 | 283,8 / 332,8 | 0,853 |
| W10 | 35,6 / 31,0 | 1,146 | 33,3 / 33,6 | 0,994 |
| W11 | 46,9 / 46,8 | 1,004 | 51,3 / 50,9 | 1,008 |
| W12 | 641,9 / 665,3 | 0,965 | 407,1 / 406,1 | 1,002 |
| W13 | 19.075,1 / 19.485,1 | 0,979 | 74.288,1 / 73.625,5 | 1,009 |
| W14 | 7.939,3 / 7.967,3 | 0,996 | 13.852,2 / 13.941,3 | 0,994 |
| W15 | 8.712,9 / 8.597,6 | 1,013 | 14.762,0 / 14.984,4 | 0,985 |
| W16 | 16.323.728,0 / 17.140.450,0 | 0,952 | 3.364.374,4 / 2.688.723,7 | 1,251 |
| W17 | 132.849,9 / 138.987,9 | 0,956 | 256.864,0 / 257.755,3 | 0,997 |
| W18 | 5.944,6 / 6.103,6 | 0,974 | 6.688,8 / 6.910,0 | 0,968 |
| W19 | 7.549,8 / 7.508,8 | 1,005 | 14.459,4 / 14.294,9 | 1,012 |
| W20 | 124,7 / 120,9 | 1,032 | 113,2 / 115,9 | 0,976 |
| W21 | 6.276,5 / 6.324,7 | 0,992 | 17.636,1 / 17.707,1 | 0,996 |
| W22 | 444,7 / 448,4 | 0,992 | 518,4 / 524,0 | 0,989 |
| W23 | 2.024,4 / 2.050,9 | 0,987 | 7.681,4 / 7.680,0 | 1,000 |
| W24 | 9.945,4 / 9.899,9 | 1,005 | 16.786,8 / 16.793,1 | 1,000 |
| W25 | 2.116,3 / 2.116,2 | 1,000 | 2.063,0 / 2.049,3 | 1,007 |
| W26 | 1.329,1 / 1.332,7 | 0,997 | 1.356,1 / 1.347,0 | 1,007 |
| W27 | 82,1 / 83,4 | 0,985 | 81,0 / 82,0 | 0,988 |
| W28 | 8.092,1 / 7.822,7 | 1,034 | 13.343,2 / 13.356,1 | 0,999 |
| W29 | 8.266,2 / 8.286,7 | 0,998 | 14.091,3 / 13.839,6 | 1,018 |
| W30 | 18.070,1 / 18.118,2 | 0,997 | 25.814,6 / 25.707,3 | 1,004 |
| W31 | 3.477,5 / 3.584,9 | 0,970 | 3.313,2 / 3.247,4 | 1,020 |
| W32 | 1.083,5 / 1.097,7 | 0,987 | 1.250,0 / 1.233,3 | 1,014 |

Varias cargas cortas o con varios hilos tienen una ronda disparada (W10 en Windows: 31,1 y 35,6 ns, y después 341,1),
así que las que quedaron fuera de ±3 % se midieron otra vez con 10 rondas: en Windows W1, W7, W8, W9, W10, W12, W16,
W17, W20 y W28 (W13 sale con W12), y en Linux W5, W8, W9, W16 y W18, más W7, la única peor en los dos sistemas.
Corridas el 2026-11-20: Windows 11:39–12:13 UTC, Linux 12:13–13:05 UTC.

| Sistema | Carga | base / rama (ns/op) | op/s | Rondas con la rama más lenta |
|---|---|---|---|---|
| Windows | W1 | 712,8 / 713,5 | 0,999 | 5 de 10 |
| Windows | W7 | 921,4 / 946,7 | 0,973 | 10 de 10 |
| Windows | W8 | 326,4 / 334,6 | 0,976 | 8 de 10 |
| Windows | W9 | 294,6 / 320,6 | 0,919 | 8 de 10 |
| Windows | W10 | 31,5 / 31,5 | 0,999 | 4 de 10 |
| Windows | W12 | 660,9 / 680,2 | 0,972 | 5 de 10 |
| Windows | W13 | 19.836,9 / 20.170,9 | 0,983 | 4 de 10 |
| Windows | W16 | 15.273.490,0 / 15.370.138,5 | 0,994 | 6 de 10 |
| Windows | W17 | 132.813,8 / 133.245,0 | 0,997 | 6 de 10 |
| Windows | W20 | 116,3 / 121,1 | 0,961 | 10 de 10 |
| Windows | W28 | 8.005,6 / 7.978,1 | 1,003 | 3 de 10 |
| Linux | W5 | 2.632.058,0 / 2.609.853,3 | 1,009 | 5 de 10 |
| Linux | W7 | 1.206,5 / 1.225,6 | 0,984 | 6 de 10 |
| Linux | W8 | 425,0 / 413,9 | 1,027 | 4 de 10 |
| Linux | W9 | 455,6 / 458,4 | 0,994 | 4 de 10 |
| Linux | W16 | 3.128.878,1 / 3.139.072,2 | 0,997 | 5 de 10 |
| Linux | W18 | 6.840,2 / 7.140,2 | 0,958 | 7 de 10 |

- **Ruido:** W1, W5, W8, W9 en Linux, W10, W12, W13, W16, W17 y W28 vuelven a ±3 % y alternan cuál versión es más
  lenta, ronda a ronda.
- **Más lentas en casi todas las rondas:** W7 en Windows (−2,7 %, 10 de 10) y W20 en Windows (−3,9 %, 10 de 10).
  W9 en Windows (−8,1 %, 8 de 10) varía de 201,4 a 345,0 ns en la base. W18 en Linux da −4,2 % en 7 de 10. W7 en
  Linux queda en −1,6 %.
- **Causa no medida.** W7, W9 y W20 son lecturas de árboles y W18 lee valores grandes, y el cambio no toca su camino:
  en `krummi-tree`, `krummi-tx` y `krummi-pager` solo agrega funciones (`run_unchecked`, `sample_leaves`,
  `write_unless`, `commit_applying`). El cambio de la ubicación del código en el binario es una hipótesis sin medir.
  El PRD no fija umbrales para esta comparación (ST15).

## Tiempo

Estimación al empezar: 14 h – 22 h. Activo desde el arranque confirmado: **18 h 19 min**, dentro de la estimación.
Las correcciones de los hallazgos de Codex (13 rondas) se llevan el 36 %. El segundo tramo de «Exploración y mapa»
(06:57–13:06 del 2026-11-20, unas 6 h) quedó con esa etiqueta, pero abarca el gancho del banco de ST15 y la espera de
sus mediciones en segundo plano. La pausa de la noche entre sesiones (2026-11-19 21:29:11 a 2026-11-20 05:28:04) sale
del historial de la sesión; se anotó en el registro de fases durante el cierre, porque no se había marcado.

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 6 h 15 min | 34 % | 2 |
| Plan (documento de ejecución) | 0 min 47 s | 0 % | 1 |
| Código y tests | 13 min 40 s | 1 % | 1 |
| Checks (fmt, clippy, suite de tests) | 2 h 49 min | 15 % | 8 |
| Revisión RDD | 2 h 26 min | 13 % | 6 |
| Correcciones de hallazgos | 6 h 39 min | 36 % | 5 |
| Documentación | 0 min 08 s | 0 % | 1 |
| Cierre (informe y merge) | 0 min 01 s | 0 % | 1 |
| **Total activo** | **18 h 25 min** | **100 %** | **25** |

- Inicio: 2026-11-19 11:45:27 +00:00. Fin: 2026-11-20 14:25:48 +00:00.
- Arranque confirmado: 12:07:36. Activo desde el arranque: **18 h 19 min** (compárese con la estimación); antes del arranque: 6 min 10 s.
- Esperas por el usuario (fuera del activo): 15 min 59 s.
  - 11:51:37–12:03:17 (11 min 40 s): product questions
  - 12:03:17–12:07:36 (4 min 19 s): estimate and start confirmation
- Pausas entre sesiones (fuera del activo): 7 h 58 min.
  - 2026-11-19 21:29:11 +00:00 – 2026-11-20 05:28:04 +00:00 (7 h 58 min)

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | codex-R1 | 14:46:30 | 14:58:10 | 11 min 40 s |
| Revisión RDD | codex-R2 | 16:05:21 | 16:15:50 | 10 min 29 s |
| Revisión RDD | codex-R3 | 16:43:55 | 16:54:10 | 10 min 15 s |
| Revisión RDD | codex-R10 | 05:49:33 | 05:59:43 | 10 min 10 s |
| Revisión RDD | codex-R11 | 06:11:47 | 06:19:28 | 7 min 41 s |
| Aceptación | st15-windows | 07:10:18 | 07:28:29 | 18 min 11 s |
| Aceptación | st15-linux | 07:28:29 | 08:22:49 | 54 min 20 s |
| Aceptación | st15-ab-windows | 08:23:30 | 09:28:22 | 1 h 04 min |
| Aceptación | st15-ab-linux | 09:28:22 | 11:38:40 | 2 h 10 min |
| Aceptación | st15-ab10-windows | 11:39:29 | 12:13:46 | 34 min 17 s |
| Aceptación | st15-ab10-linux | 12:13:46 | 13:05:58 | 52 min 12 s |
| Revisión RDD | codex-R12 | 13:25:21 | 13:34:40 | 9 min 19 s |
| Aceptación | st15-est2-windows | 13:47:55 | 13:58:04 | 10 min 09 s |
| Aceptación | st15-est2-linux | 13:58:04 | 14:17:24 | 19 min 20 s |
| Revisión RDD | codex-R13 | 14:18:20 | 14:21:11 | 2 min 51 s |
| Revisión RDD | codex-prd-N11 | 14:21:34 | 14:24:20 | 2 min 46 s |
| **Aceptación (total)** | | | | **6 h 23 min** |
| **Revisión RDD (total)** | | | | **1 h 05 min** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `b04cba7` | docs(prd): sample run rows by pages and use a new read's snapshot in a write (7.3) |  | 6 min 57 s | Exploración y mapa 6 min 10 s, Plan (documento de ejecución) 0 min 47 s |
| `8d1fb6e` | feat(table): keep a persistent modification counter per table (format 13) | T1, T2 | 29 min 01 s | Código y tests 13 min 40 s, Checks (fmt, clippy, suite de tests) 15 min 13 s, Documentación 0 min 08 s |
| `061c879` | docs(odd): record T2 and its review | T2 | 2 min 28 s | Checks (fmt, clippy, suite de tests) 0 min 45 s, Revisión RDD 1 min 43 s |
| `7fa8911` | feat(table): build, save and read column statistics, and analyze | T2, T3 | 23 min 33 s | Checks (fmt, clippy, suite de tests) 14 min 31 s, Revisión RDD 9 min 02 s |
| `93bac6d` | feat(table): estimate rows by a column's statistics, sampling at once when there are none | T3, T4 | 16 min 08 s | Checks (fmt, clippy, suite de tests) 16 min 08 s |
| `0ddf2a1` | feat(table): save and renew statistics in a background thread | T4, T5 | 24 min 38 s | Checks (fmt, clippy, suite de tests) 24 min 38 s |
| `34cbe58` | feat(table): estimate equalities in several columns with index prefix statistics | T5, T3-T5, T6 | 16 min 14 s | Checks (fmt, clippy, suite de tests) 10 min 48 s, Revisión RDD 5 min 26 s |
| `ea5d7b3` | feat(table): sample tables of more than 30,000 rows by random leaves | T6, T7 | 15 min 10 s | Checks (fmt, clippy, suite de tests) 15 min 10 s |
| `f6e56d9` | docs(odd): record T6 and the large-table sampling | T7 | 0 min 13 s | Checks (fmt, clippy, suite de tests) 0 min 13 s |
| `4386d3b` | fix(table): read statistics in memory and index statistics in a write | T7, T6-T7a | 29 min 18 s | Checks (fmt, clippy, suite de tests) 0 min 21 s, Revisión RDD 28 min 57 s |
| `564aa01` | docs(odd): record the verification and the last reviews | T6-T7a | 1 min 37 s | Revisión RDD 1 min 37 s |
| `8f31235` | docs(reviews): add the first Codex code review of the table statistics | T6-T7a | 12 min 06 s | Revisión RDD 12 min 06 s |
| `b93f241` | fix(table): resolve the first Codex review of the table statistics | T6-T7a, codex-R1 | 1 h 04 min | Revisión RDD 43 min 24 s, Correcciones de hallazgos 20 min 44 s |
| `32af503` | test(table): cover failed statistics saves (AC-ST8) | codex-R1 | 1 min 50 s | Correcciones de hallazgos 1 min 50 s |
| `bf69c8b` | docs(reviews): record the resolutions of the first Codex code review | codex-R1 | 0 min 33 s | Correcciones de hallazgos 0 min 33 s |
| `05d3d7f` | fix(table): resolve the second Codex review of the table statistics | codex-R1 | 26 min 54 s | Correcciones de hallazgos 26 min 54 s |
| `f1fc1a0` | fix(table): keep clippy clean on the statistics build | codex-R1 | 11 min 28 s | Correcciones de hallazgos 11 min 28 s |
| `25d1fc5` | docs(reviews): record the second round of the Codex code review | codex-R1 | 0 min 01 s | Correcciones de hallazgos 0 min 01 s |
| `065ad86` | docs(odd): record the Codex review rounds | codex-R1 | 0 min 21 s | Correcciones de hallazgos 0 min 21 s |
| `363788c` | docs(reviews): add the third round of the Codex code review | codex-R1 | 10 min 37 s | Correcciones de hallazgos 10 min 37 s |
| `8f7f4e4` | fix(table): resolve the third Codex review of the table statistics | codex-R1, codex-R3 | 1 h 04 min | Correcciones de hallazgos 1 h 04 min |
| `6b6430a` | docs(reviews): record the third round's fixes | codex-R3 | 0 min 01 s | Correcciones de hallazgos 0 min 01 s |
| `deba2b3` | docs(reviews): add the fourth round of the Codex code review | codex-R3 | 13 min 58 s | Correcciones de hallazgos 13 min 58 s |
| `31c5eac` | fix(table): resolve the fourth Codex review of the table statistics | codex-R3 | 38 min 19 s | Correcciones de hallazgos 38 min 19 s |
| `87352be` | docs(reviews): record the fourth round's fixes |  | 0 min 00 s |  |
| `b77ff51` | docs(reviews): add the fifth round of the Codex code review | codex-R3 | 8 min 58 s | Correcciones de hallazgos 8 min 58 s |
| `265ed1f` | fix(table): retire statistics at the commit's point of no return | codex-R3, codex-R5 | 30 min 51 s | Correcciones de hallazgos 30 min 51 s |
| `1d687b3` | docs(reviews): record the fifth round's fixes |  | 0 min 00 s |  |
| `ace8aeb` | docs(reviews): add the sixth round of the Codex code review | codex-R5 | 9 min 21 s | Correcciones de hallazgos 9 min 21 s |
| `bc82fd7` | fix(table): validate statistics builds by their snapshot's commit, and cover the fault matrix | codex-R5 | 26 min 15 s | Correcciones de hallazgos 26 min 15 s |
| `91111a3` | feat(table): sample rows stored in runs by the pages they use (T7b) | codex-R5 | 14 min 55 s | Correcciones de hallazgos 14 min 55 s |
| `88bacde` | docs(reviews): record the sixth round's fixes |  | 0 min 00 s |  |
| `144acf2` | fix(table): resolve the seventh Codex review of the table statistics | codex-R5 | 24 min 35 s | Correcciones de hallazgos 24 min 35 s |
| `4012b31` | docs(reviews): record the seventh round and its fixes |  | 0 min 00 s |  |
| `da56d24` | fix(table): resolve the eighth Codex review of the table statistics | codex-R5 | 24 min 13 s | Correcciones de hallazgos 24 min 13 s |
| `8240385` | docs(reviews): record the eighth round and its fixes |  | 0 min 00 s |  |
| `4aac063` | test(table): cover analyze running out of memory after sampling and analyze in a write of a damaged table | codex-R5 | 18 min 32 s | Correcciones de hallazgos 18 min 32 s |
| `a1946cf` | feat(table): count the run pages a statistics sample reads (ST14 hook) | codex-R5, resume after the night pause | 21 min 28 s | Correcciones de hallazgos 21 min 28 s |
| `e14a61c` | docs(reviews): record the ninth round and its fixes |  | 0 min 00 s |  |
| `aa0b474` | fix(table): count the run pages before a sampled row's values | resume after the night pause | 22 min 16 s | Correcciones de hallazgos 22 min 16 s |
| `3e0641a` | docs(reviews): record the tenth round and its fix |  | 0 min 00 s |  |
| `d3244ed` | docs(reviews): record the approval of the eleventh round | resume after the night pause | 7 min 54 s | Correcciones de hallazgos 7 min 54 s |
| `c6b0331` | docs(odd): mark the RDD review of the fix commits | resume after the night pause, RDD review of the fix commits | 2 min 01 s | Revisión RDD 1 min 14 s, Correcciones de hallazgos 0 min 47 s |
| `f5a2cd1` | test(table): order statistics race tests by the sample counter, not by sleeps | RDD review of the fix commits | 13 min 35 s | Revisión RDD 13 min 35 s |
| `d6e3a8a` | docs(odd): record the RDD review of the fix commits |  | 0 min 00 s |  |
| `51a2779` | test(table): fail the statistics race tests when analyze never starts its sample | RDD review of the fix commits | 1 min 32 s | Revisión RDD 1 min 32 s |
| `5c94644` | docs(odd): record the last RDD review and the state of T7 | RDD review of the fix commits | 0 min 45 s | Revisión RDD 0 min 45 s |
| `d4ff730` | feat(bench): estimates beside W23 and W28 for the statistics measurements (ST15) | RDD review of the fix commits, ST15 measurements: bench map | 26 min 08 s | Exploración y mapa 5 min 59 s, Revisión RDD 20 min 09 s |
| `59bc95d` | docs: raise the Codex review round limit to ten | ST15 measurements: bench map | 4 h 19 min | Exploración y mapa 4 h 19 min |
| `05128f4` | docs(prd): measure only the affected loads in depth after 7.3 (N11) | ST15 measurements: bench map | 0 min 01 s | Exploración y mapa 0 min 01 s |
| `ce991db` | chore: record the krummi test-hooks dev-dependency in Cargo.lock | ST15 measurements: bench map | 0 min 04 s | Exploración y mapa 0 min 04 s |
| `f1aa8f0` | docs(odd): record the phases of the ST15 measurements | ST15 measurements: bench map | 1 h 44 min | Exploración y mapa 1 h 44 min |
| `b80503a` | fix(bench): pick the first estimate's column by name (ST15) | T7 | 17 min 50 s | Checks (fmt, clippy, suite de tests) 11 min 46 s, Revisión RDD 5 min 56 s, Correcciones de hallazgos 0 min 08 s |
| `c6c43ce` | docs(odd): record the ST15 measurements and the last RDD review |  | 0 min 00 s |  |
| `79a5d05` | docs(reviews): record the tables-statistics changes after Codex round 11 | T7 | 0 min 23 s | Checks (fmt, clippy, suite de tests) 0 min 23 s |
| `879b73c` | fix(bench): one estimating round per commit in the ST15 thread (R21) | T7 | 22 min 40 s | Checks (fmt, clippy, suite de tests) 22 min 40 s |
| `0d3af54` | docs(reviews): record Codex round 12 and the R21 fix | T7 | 30 min 23 s | Checks (fmt, clippy, suite de tests) 30 min 23 s |
| `e0699ca` | docs(reviews): record the approval of Codex round 13 | T7 | 3 min 10 s | Checks (fmt, clippy, suite de tests) 3 min 10 s |
| `106d3c9` | docs(reviews): record the Codex review of the N11 annotation | T7 | 3 min 08 s | Checks (fmt, clippy, suite de tests) 3 min 08 s |
| — | después del último commit | T7 | 1 min 15 s | Checks (fmt, clippy, suite de tests) 0 min 07 s, Revisión RDD 1 min 07 s, Cierre (informe y merge) 0 min 01 s |
| **Total** | | | **18 h 25 min** | |

## Informe de cambios (`ba7f27a..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 5.596 | 164 | 4.812 | 513 | 35 |
| `docs` | 1.950 | 6 | — | — | 22 |
| `odd` | 270 | 0 | — | — | 2 |
| `crates/krummi-bench` | 214 | 12 | 183 | 23 | 1 |
| `crates/krummi-tree` | 126 | 0 | 105 | 14 | 1 |
| `crates/krummi-tx` | 87 | 12 | 60 | 17 | 4 |
| `crates/krummi-pager` | 32 | 12 | 22 | 8 | 6 |
| `.claude` | 3 | 3 | — | — | 1 |
| `CLAUDE.md` | 1 | 1 | — | — | 1 |
| **Total** | **8.279** | **210** | **5.182** | **575** | **73** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+1 −0)

## Resumen

Las tablas tienen ahora estadísticas para estimar cuántas filas cumple una condición: igual a un valor, en un
intervalo o igual en varias columnas. Nunca responden con una suposición: si faltan, muestrean al momento, hasta
30.000 filas enteras o una muestra de hojas al azar, y las viejas se usan mientras un hilo propio del almacén las
renueva y las guarda. El almacén las comparte en memoria entre transacciones y las retira en el instante en que se
confirma el cambio que las invalida. Los valores grandes se cuentan sin leerse y las filas guardadas en tramos se
muestrean leyendo solo las páginas que hacen falta, sin la suma del tramo entero.

## Decisiones tomadas sin preguntar

S5 («toma la decisión más conservadora o recomendada»), registradas en L4, L5 y L7 del documento:

- T3 primero muestrea con el recorrido de filas; la lectura parcial de tramos y el muestreo de hojas al azar fueron
  pasos propios (T7a, T7b).
- Las estadísticas de una columna borrada se ignoran por identidad en lugar de borrarse del árbol.
- Un guardado espera el turno en sondeos de 20 ms para que cerrar pueda soltarlo.
- Las estadísticas privadas de una escritura guardan columnas, no prefijos de índice.
- La instantánea confirmada sirve a una estimación de una escritura solo si tiene todas las columnas e índices de su
  esquema.
- Las mediciones de ST15 usan un gancho del banco, `KRUMMI_BENCH_ESTIMATES`, fuera de su interfaz, que solo escribe
  en stderr: no hay cargas nuevas ni umbrales nuevos.
- El A/B midió W12 y W13 juntas, en el proceso de W12, porque W13 no tiene proceso propio; y las cargas fuera de ±3 %
  se midieron otra vez con 10 rondas.

Decisiones del usuario durante la ejecución (L23): el límite de rondas automáticas de Codex pasa a 10 (`59bc95d`) y
N11 del PRD de las tablas se anota para medir a fondo solo las cargas afectadas después de 7.3 (`05128f4`).

## Pendiente (no bloqueante)

- Avisos de RDD que quedan abiertos (tabla de arriba): el recorrido de todas las tablas en `find_write_table`, el
  conjunto de destinos retirados sin podar, `write_unless` sin prueba propia, los caminos sin verificar de hojas al
  azar y de escritura sin prueba de daño, y las guardas de pánico bajo `panic=abort`. Ninguno es un fallo demostrado.
- Sugerencias de RDD sobre el gancho del banco (`review-fb0cd071ffaf8627`): informar los dos errores cuando fallan la
  escritura y el hilo, una prueba de W28 con el hilo y rechazar valores desconocidos de `KRUMMI_BENCH_ESTIMATES`.
- **Lecturas más lentas sin causa medida:** W7 (−2,7 %), W9 (−8,1 %, con ruido) y W20 (−3,9 %) en Windows, y W18 (−4,2 %)
  en Linux, en 10 rondas. El cambio no toca su camino; se podría comprobar con un perfil o con un A/B que solo cambie
  la ubicación del código.
- Avisos A2, A3, A5–A7 del verificador, resueltos después por Codex (R14, R11, R6, R7, R13).

**Riesgo:** ítems 1 (formato persistido 13), 4 (concurrencia: hilo propio, retirada en el commit) y 6 (los ganchos
de prueba sostienen la cobertura del muestreo parcial).

## Limpieza manual

- `work/st` (1,3 GB, ignorado por git): las fuentes y compilaciones de la base y de la rama, los scripts y los
  resultados de las mediciones (`*.json`, `*.log`).
- `~/st` dentro de WSL (602 MB): las mismas fuentes y compilaciones para Linux.
- El verificador ya no dejó carpetas en `%TEMP%`.
