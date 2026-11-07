# Separadores de miles en el informe del banco: implementados y verificados

Informe de cierre de `benchmark-separators` (2026-11-07). Enfoque de ejecución: ODD, tarea pequeña (en línea, sin
documento de ejecución). Fuente: la anotación del 2026-11-07 a
[benchmark.prd.md](../prd/krummi/benchmark.prd.md) (reglas de la forma del informe, AC-K2 y el nuevo AC-K15).

Rama `feat/benchmark-separators`, que parte de `perf/write-path` (`ba310b1`, donde están la anotación y su
traducción), sin fusionar en `main` ni subir.

## Resultado

- **Especificación:** las celdas de operaciones por segundo y de tamaños de las dos tablas del informe se redondean
  al entero más cercano (empates hacia arriba) y llevan un punto cada tres cifras, también con cuatro; la parte entera
  de la razón Krummi / Voron, ya truncada, lleva el mismo separador desde mil. El resto del informe no cambia. El
  progreso de la consola, cuyo formato el PRD deja a la implementación, escribe la cifra igual que el informe.
- **Tests:** `cargo test --workspace` 41 suites, 559 pasadas, 0 fallidas, 30 ignoradas (antes del commit `3ddf0e5`,
  que añade un test más del progreso; la suite del banco, 46 pasadas, tras él). `cargo fmt --all --check` y
  `cargo clippy --workspace --all-targets -- -D warnings` limpios.
- **Aceptación:** los 8 tests de integración del banco, ignorados por defecto, pasan con
  `KRUMMI_BENCH_RAVENDB` (312 s), incluida una ejecución reducida real que escribe su informe con el formato nuevo.
- **Informes guardados (AC-K15):** los 8 de `docs/benchmark/` se convierten en `bb33fbc`, solo en las celdas de sus
  dos tablas (1.199 celdas en 231 filas). Un script de comprobación verificó en cada uno que al quitar los puntos de
  esas celdas el archivo es idéntico byte a byte al original, que solo cambian filas de tabla, que una segunda
  conversión no cambia nada y que no queda ninguna celda de cuatro cifras o más sin separador.
- **Hallazgos corregidos:** `clippy` pidió `is_multiple_of` en el agrupador; RDD señaló que el cambio del progreso no
  estaba documentado ni probado (corregido en `3ddf0e5`).

## Acceptance criteria coverage

| Criterio | Test o comprobación | Resultado |
|---|---|---|
| AC-K2 (anotación del 2026-11-07) | `report::tests::table_numbers_carry_thousands_separators` (999,6 → `1.000`, 86, 0, 1184, 134217728, razón 2500,009 → `2.500,00`); filas esperadas de `a_report_with_every_threshold_met_has_the_exact_shape` y `a_zero_voron_metric_shows_infinity_or_zero_and_meets`; `measure::tests::whole_numbers_carry_a_dot_every_three_digits`, `a_metric_is_rounded_before_it_is_grouped`, `a_ratio_from_one_thousand_carries_a_thousands_separator` | PASS |
| AC-K15 | Script de conversión y comprobación sobre los 8 informes; recuento de celdas sin separador: 0 | PASS |
| AC-K1, AC-K5 | `tests/run.rs` con `--include-ignored` (lee las celdas agrupadas) | PASS |

## RDD reviews

| Tramo | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `ba310b1..bb33fbc` | readability, reliability | approved | R2-001 y R3 warning: el progreso de la consola cambia de formato sin decirlo ni probarlo (también redondea los empates hacia arriba en vez de al par). Intencional: se extrae `progress_line` con su comentario y su test (`3ddf0e5`) | `review-fdb89a6ff90f36bd` |
| `bb33fbc..3ddf0e5` | readability, reliability | approved | ninguno | `review-1a8f6114e1950e2d` |

## Codex reviews

- Anotación del PRD ([informe](../reviews/benchmark-separators/2026-11-07-code.md)): approved, sin hallazgos.
- Código, `ba310b1..3ddf0e5` ([informe](../reviews/benchmark-separators/2026-11-07-code-02.md)): approved, sin
  hallazgos.

## Time

- Estimación: entre 1 h y 2 h. Inicio: 2026-11-07 08:18:38 UTC. Fin: 08:38:06 UTC.
- **Activo desde el arranque: 19 min 28 s, muy por debajo de la estimación.** El cambio cabía en una función y sus
  tests; lo más largo fueron los tests de integración del banco (5 min 50 s) y las revisiones.
- Esperas por el usuario: ninguna. La anotación del PRD, su revisión y su traducción se hicieron antes del arranque y
  no están en este tiempo.

### Time by phase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Código y tests | 2 min 23 s | 12 % | 1 |
| Checks (fmt, clippy, suite de tests) | 3 min 49 s | 20 % | 1 |
| Aceptación | 5 min 50 s | 30 % | 1 |
| Revisión RDD | 7 min 26 s | 38 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **19 min 28 s** | **100 %** | **5** |

- Inicio: 2026-11-07 08:18:38 +00:00. Fin: 2026-11-07 08:38:06 +00:00.
- Arranque confirmado: 08:18:38. Activo desde el arranque: **19 min 28 s** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | codex-R1 | 08:32:18 | 08:37:52 | 5 min 34 s |
| **Revisión RDD (total)** | | | | **5 min 34 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `73c5850` | feat(bench): write report numbers with thousands separators | estimate 1-2 h | 11 min 55 s | Código y tests 2 min 23 s, Checks (fmt, clippy, suite de tests) 3 min 42 s, Aceptación 5 min 50 s |
| `bb33fbc` | docs(bench): add thousands separators to the saved reports |  | 0 min 00 s |  |
| `3ddf0e5` | test(bench): pin the progress line's rate format |  | 1 min 18 s | Checks (fmt, clippy, suite de tests) 0 min 07 s, Revisión RDD 1 min 11 s |
| `e190242` | docs: add the Codex review of the benchmark separators |  | 6 min 15 s | Revisión RDD 6 min 15 s |
| — | después del último commit |  | 0 min 00 s |  |
| **Total** | | | **19 min 28 s** | |

## Change report (`ba310b1..e190242`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 300 | 231 | — | — | 9 |
| `crates/krummi-bench` | 149 | 24 | 124 | 16 | 4 |
| **Total** | **449** | **255** | **124** | **16** | **13** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

`docs` incluye los 8 informes convertidos (231 líneas cambiadas, solo celdas) y el informe de Codex.

## Summary

Los números del informe del banco se leen de un vistazo: `1.038.701` en lugar de `1038701`. El formato es fijo,
con punto, sin depender de la configuración regional, y los informes ya guardados se leen igual que los nuevos. El
agrupador (`thousands`) y el redondeo (`whole`) están en un solo sitio y los usan las tablas, la razón y el progreso.

## Defaults chosen without asking

- El progreso de la consola usa el mismo formato que el informe (el PRD lo deja a la implementación; te lo dije
  antes de arrancar).
- El script de conversión de los informes guardados no se añade al repositorio: es de un solo uso y su resultado
  está comprobado y en `bb33fbc`.

## Pending (non-blocking)

- Ninguno. Las tablas copiadas de los informes a otros documentos quedan como estaban, como fija la anotación.

**Risk:** item 3: changes the public report output of the benchmark. Requested by the user and fixed in the PRD
annotation; no program in the repository reads the reports, and the integration test that parses them was updated.
