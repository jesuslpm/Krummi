# Banco comparativo terminado: dos ejecuciones completas, revisado por RDD y por Codex; Krummi cumple 5 de 22 umbrales en Windows y 7 de 22 en Linux

Informe de cierre de `benchmark` (2026-11-04). Enfoque de ejecución: ODD.
Fuente de producto: [benchmark.prd.md](../../docs/prd/krummi/benchmark.prd.md) (módulo 6). Documento de ejecución:
[benchmark.md](benchmark.md). Informes del banco: [Windows](../../docs/benchmark/2026-11-03-windows.md) y
[Linux](../../docs/benchmark/2026-11-03-linux.md). Plan de optimización: [plan.md](../../docs/performance/plan.md).

Rama `feat/benchmark`, sin merge ni push: queda para que la revises.

## Resultado

- **Especificación:** AC-K1 a AC-K14 entregados (S1–S14 del documento de ejecución). AC-K8 se cierra con las citas de
  la sección [Durabilidad de cada motor](#durabilidad-de-cada-motor-ac-k8). Checks al final: `cargo fmt --all --check`
  correcto; `cargo clippy --workspace --all-targets -- -D warnings` limpio; `cargo test --workspace` 519 pasados, 0
  fallidos, 30 ignorados; `cargo test --release -p krummi-bench -- --include-ignored` con `KRUMMI_BENCH_RAVENDB`:
  41 + 8 + 8 + 9 pasados (las ocho ejecuciones de punta a punta con Voron en 473 s), en `384e13e`, el último
  commit con código.
- **Aceptación:** ejecuciones completas en Windows 11 (NTFS, `7e70319`, 5.830 s, código de salida 1) y en WSL 2 Ubuntu
  24.04 (ext4, `a881d48`, 6.958 s, código de salida 1), en el mismo equipo (i9-13980HX). El código 1 es el de K13:
  Krummi incumple umbrales. Resultado: **Windows 5 de 22, Linux 7 de 22** umbrales cumplidos. Cada informe tiene las 22
  cargas, los seis motores en sus versiones de K1 y `no aplica` en las 12 celdas de canopydb y fjall que AC-K1 pide.
- **Revisiones y verificación:** un verificador independiente (ODD Verify) sobre la funcionalidad entera, sin
  bloqueos (L7); RDD en 17 revisiones, todas aprobadas (sección [Revisiones RDD](#revisiones-rdd)); revisión cruzada de
  Codex del PRD, aprobada en la segunda ronda, y del código, aprobada en la cuarta (autorizaste la cuarta, más allá de
  las tres automáticas).
- **Hallazgos corregidos por el camino:**
  - La primera ejecución completa en WSL falló: un escritor de Voron esperó más de 30 s su cerrojo de escritura, que no
    es justo (`a881d48`, después limitado a 10 minutos en `de31a97`).
  - El verificador: las características predeterminadas de los motores (fjall había perdido su LZ4), una ruta de
    informe que nombra una carpeta, un bucle con una métrica de Voron de 0 (`7286171`).
  - Codex del PRD: tres aclaraciones, entre ellas la razón y el umbral con una métrica de Voron de 0 (decisión tuya).
  - Codex del código: W12 contaba lecturas fuera del tiempo del escritor (`74c8697`, exacto en `9cb6a49`); la carpeta
    del informe sin permiso de escritura se descubría al final (`6afd038`); Ctrl+C durante el cierre dejaba el informe
    (`6c5ebd1`, `6d35cd5`); la prueba de punta a punta rechazaba `∞` (`dcd39f8`); el escritor de W12 publicaba el fin
    sin barrera completa, y otro núcleo podía contar una lectura después del fin (`384e13e`). Detalle y estado en
    [2026-11-04-code-resolutions.md](../../docs/reviews/benchmark/2026-11-04-code-resolutions.md).

## Cobertura de los criterios de aceptación

| Criterio | Test o comprobación | Resultado |
|---|---|---|
| AC-K1 | Las dos ejecuciones completas; sus informes en `docs/benchmark/` | PASS: 22 cargas, seis motores, 12 celdas `no aplica` en cada uno |
| AC-K2 | `report::tests` (informe entero byte a byte, umbrales, `0,7996`, `0,80`, Voron en 0, reducida, desconocidos) | PASS |
| AC-K3 | `measure::tests::the_threshold_is_eighty_percent_of_voron`, `report::tests` | PASS |
| AC-K4 | `measure::tests::the_metric_is_the_median_of_the_measured_repetitions` | PASS |
| AC-K5 | `tests/run.rs` (reducida con Voron), `tests/worker.rs` (cinco motores de Rust) | PASS |
| AC-K6 | Trabajo `benchmark` de `.github/workflows/ci.yml` (Windows y Linux) | Configurado; no ejecutado en GitHub en esta ejecución |
| AC-K7 | Resumen de datos comparado con el de Krummi en cada carga y ejecución; `worker::tests::the_digest_agrees_with_the_voron_program` | PASS |
| AC-K8 | Revisión con citas, abajo | PASS |
| AC-K9 | `tests/cli.rs`, `cli::tests`, `orchestrator::tests`; Ctrl+C a mano en T4 | PASS |
| AC-K10 | `tests/run.rs` y `tests/worker.rs` con `KRUMMI_BENCH_FAIL` | PASS |
| AC-K11 | `tests/run.rs` (orden de procesos y motores) | PASS |
| AC-K12 | `size::tests` | PASS |
| AC-K13 | `tests/run.rs` (nombre por defecto, subcarpeta borrada) | PASS |
| AC-K14 | `crates/krummi/tests/bench_dependencies.rs`, `unsafe_policy.rs` | PASS |

### Durabilidad de cada motor (AC-K8)

Cada confirmación del banco usa lo que la documentación de cada motor da para llevar los cambios al medio físico:

| Motor | Cómo confirma el banco | Cita | En *Opciones cambiadas* |
|---|---|---|---|
| Krummi | `WriteTransaction::commit` | "Commits the transaction durably: it returns once its changes survive a crash of the process or the system and a power cut" (`crates/krummi/src/lib.rs:1347`) | No (predeterminado) |
| Voron 7.2.7 | Confirmación predeterminada; diario en `JournalMode.Safe` | `JournalWriter(..., PalFlags.JournalMode mode = PalFlags.JournalMode.Safe)` (`src/Voron/Impl/Journal/JournalWriter.cs:38`); Windows `FILE_FLAG_NO_BUFFERING \| FILE_FLAG_WRITE_THROUGH` (`src/Raven.Pal/src/win/journal.c:25`); Linux `O_DSYNC \| O_DIRECT`, o `O_DSYNC` si el sistema de archivos no admite `O_DIRECT` (`src/Raven.Pal/src/posix/journal.c:45-47`) | No (predeterminado) |
| LMDB 0.9.70 | Confirmación sin `MDB_NOSYNC` | `MDB_NOSYNC`: "Don't flush system buffers to disk when committing a transaction" (`lmdb.h:590-591`); sin ese indicador cada confirmación lleva los cambios al disco | No (predeterminado); sí el mapa de 1 GiB y los indicadores de W19–W22 |
| redb 4.4.0 | `Durability::Immediate` | "Commits with this durability level are guaranteed to be persistent as soon as `WriteTransaction::commit` returns" (`src/transactions.rs:378-380`) | No (predeterminado) |
| canopydb 0.2.5 | `commit_with(true)` | "Commits the transaction and optionally performs a durable `fsync` to make the transaction (and all the ones before it) durable in the storage" (`src/lib.rs:1243-1244`) | Sí: "confirmación que espera al disco del diario" |
| fjall 3.1.13 | `PersistMode::SyncData` en cada confirmación | `PersistMode::SyncData => self.file.get_mut().sync_data()` (`src/journal/writer.rs:226`) | Sí: "llevar el diario al disco (SyncData) en cada confirmación" |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `aed68a6..8008b69` (PRD) | — | pasivo, sin revisión | — | — |
| `8008b69..d9b4822` (núcleo) | riesgo, legibilidad, fiabilidad | aprobado | R1-001 sugerencia (`deny` en vez de `forbid`): se mantiene, AC21 limita `unsafe` al adaptador de LMDB; R2-001 aviso (texto de la razón y umbral con comparaciones distintas): corregido; R3 aviso (bucle con Voron en 0): ya corregido en `7286171` | `review-d098d5e49cf43551` |
| `d9b4822..5ca33da` (CI) | riesgo, fiabilidad | aprobado | R1-001 sugerencia (RavenDB por etiqueta): corregido, CI comprueba el commit; R3 sugerencia (sin `--all-features`): corregido | `review-3ac00a52d5953365` |
| `5ca33da..fa65b43` (Voron) | fiabilidad, resiliencia, legibilidad | aprobado | Cinco avisos y sugerencias corregidos (fórmula de la clave, `KRUMMI_BENCH_FAIL` mal formado, comentarios, `selftest` con aserciones) | `review-d408eabb06d29d47` |
| `fa65b43..24baebb` (worker de Rust) | las cuatro | aprobado | Avisos: canopydb fijaba una caché de 1 GiB no listada, corregido; paso multivalor sin nombre, corregido; una sugerencia se mantiene | `review-8afa7395edc131e4` |
| `24baebb..fa4f9c8` (orquestador) | las cuatro | aprobado | Características predeterminadas: se mantienen por K3; sin tiempo máximo por proceso: fuera de alcance; almacenes conservados y nombre repetido: corregidos; Ctrl+C sin prueba automática, `desconocido` y stderr en una línea: se mantienen con motivo | `review-d8124be01bd4049e` |
| `fa4f9c8..4af47d6` (correcciones) | riesgo, legibilidad, fiabilidad | aprobado | R3-001 aviso: descartado con evidencia; sugerencias se mantienen | `review-44b9db0772ddd82e` |
| `4af47d6..e4ac4f2` (espera de Voron, informes) | resiliencia, fiabilidad | aprobado | Espera sin límite que se cuelga: corregido con 10 minutos; sin prueba del arreglo: se mantiene, no determinista; informe de Windows con el límite de 30 s: se mantiene, no cambia ninguna medición | `review-ea064a0969f64ddb` |
| `e4ac4f2..d60b000` | resiliencia | aprobado | ninguno | `review-8821f19933732e16` |
| `d60b000..7dba24d` (PRD de Codex, prueba de Voron en 0) | fiabilidad, legibilidad | aprobado | Prueba no byte a byte: corregido; etiqueta R1 ambigua: corregido | `review-91f58e65622bdb58` |
| `7dba24d..6cef8b1` | fiabilidad | aprobado | ninguno | `review-d17f096f73e107b8` |
| `6cef8b1..a935cc9` (arreglos de Codex, ronda 1) | fiabilidad, resiliencia, riesgo | aprobado | Dos avisos con las dos métricas de W12 en 0: descartados, la razón es `0,00`; nombre de la sonda: corregido; permisos de la prueba: corregido | `review-520f94853eb8bbac` |
| `a935cc9..5ac8573` | fiabilidad | aprobado | Dos sugerencias se mantienen (una comprobación por proceso) | `review-df58601ea9c18b8a` |
| `8972ddf..d05ba48` (arreglos de Codex, ronda 2) | fiabilidad, resiliencia, legibilidad | aprobado | Rama imposible del escritor: `unreachable!`; espera de 5 s documentada; sin prueba unitaria del conteo de Voron: se mantiene, ninguna prueba ejecuta el programa .NET fuera del banco | `review-59de3858f39c465c` |
| `d05ba48..daad8fd` | fiabilidad | aprobado | Aviso del `unreachable!`: descartado, la suma recorre `finished[..readers]` | `review-183151eac0a9f867` |
| `daad8fd..c8e52eb` (log) | legibilidad | aprobado | Dos sugerencias (zona horaria en una etiqueta del registro de fases, ruta completa en una cita de L23): se mantienen, el registro solo crece y sus horas son UTC | `review-6299c9491bc7568e` |
| `c8e52eb..384e13e` (arreglo de R1, ronda 3) | riesgo, fiabilidad | aprobado | Aviso: el comentario de `SetOnDrop` quedó sobre `finish`, corregido en `c148cdf`; sugerencia (la prueba detecta el fallo por azar): la prueba dice que es de estrés | `review-58ce011a99713c98` |
| `384e13e..c148cdf` | legibilidad | aprobado | ninguno | `review-2e6dd2c7d5373dde` |

Evaluaciones sin revisión: `9ba17b7..a881d48` y `e4ac4f2..ba6a541` quedaron `under_budget` y entraron en el tramo
siguiente. `c148cdf..784175e` (registro de las rondas 3 y 4): pasivo, sin revisión.

## Revisión cruzada de Codex

| Revisión | Ronda | Informe | Resultado |
|---|---|---|---|
| PRD | 1 | [2026-11-04-prd.md](../../docs/reviews/benchmark/2026-11-04-prd.md) | `findings`: R1 mayor (W12 con Voron en 0, decidido por ti), R2 y R3 menores, corregidos |
| PRD | 2 | [2026-11-04-prd-02.md](../../docs/reviews/benchmark/2026-11-04-prd-02.md) | `approved`: R1–R3 verificados |
| Código | 1 | [2026-11-04-code.md](../../docs/reviews/benchmark/2026-11-04-code.md) | `findings`: R1, R2 mayores; R3, R4 menores, corregidos |
| Código | 2 | [2026-11-04-code-02.md](../../docs/reviews/benchmark/2026-11-04-code-02.md) | El script dio `error` (límite de uso en el mensaje final), pero el informe está completo: R2 y R4 verificados; R1, R3 y el nuevo R5 abiertos, corregidos |
| Código | 3 | [2026-11-04-code-03.md](../../docs/reviews/benchmark/2026-11-04-code-03.md) | `findings`: R3 y R5 verificados; R1 abierto como menor (publicación del fin entre núcleos), corregido |
| Código | 4 | [2026-11-04-code-04.md](../../docs/reviews/benchmark/2026-11-04-code-04.md) | `approved`: R1 verificado (cero observaciones tardías en un millón de rondas). Su prueba de punta a punta con Voron falló al restaurar NuGet en su entorno; aquí pasó |

Resoluciones: [PRD](../../docs/reviews/benchmark/2026-11-04-prd-resolutions.md),
[código](../../docs/reviews/benchmark/2026-11-04-code-resolutions.md).

## Tiempo

- Estimación: 7 h – 11 h. Inicio real: 2026-11-03 10:11:23 +00:00. Fin: 2026-11-04 12:36:53 +00:00.
- **Duración activa desde el arranque: 9 h 34 min**, dentro de la estimación. Lo que más tiempo llevó: la aceptación
  (4 h 17 min, las tres ejecuciones completas) y la revisión (3 h 30 min).
- Esperas (fuera del activo): tres por el límite de uso de Codex (13 h 24 min, 23 min 39 s y 2 h 56 min) y una por
  tu autorización de la cuarta ronda (6 min 51 s).
- **Fases de esta mañana inexactas:** de 06:49 a 11:50 no marqué cambios de fase, así que el registro cuenta como
  «Revisión RDD» y con la etiqueta «Codex PRD R1» también las correcciones de las rondas 1 y 2 de Codex, el perfilado
  y el plan de rendimiento ([plan.md](../../docs/performance/plan.md), `7ecd78a`), que no es parte del banco. El
  total activo es correcto; el reparto entre revisión y correcciones no.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 5 min 24 s | 1 % | 1 |
| Plan (documento de ejecución) | 1 min 05 s | 0 % | 1 |
| Código y tests | 1 h 03 min | 11 % | 2 |
| Checks (fmt, clippy, suite de tests) | 15 min 18 s | 3 % | 2 |
| Aceptación | 4 h 17 min | 45 % | 1 |
| Revisión RDD | 3 h 30 min | 37 % | 9 |
| Correcciones de hallazgos | 21 min 24 s | 4 % | 2 |
| Documentación | 0 min 40 s | 0 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **9 h 34 min** | **100 %** | **20** |

- Inicio: 2026-11-03 10:11:23 +00:00. Fin: 2026-11-04 12:36:53 +00:00.
- Arranque confirmado: 10:11:23. Activo desde el arranque: **9 h 34 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 16 h 51 min.
  - 17:00:49–06:24:57 (13 h 24 min): Codex usage limit until 2026-11-04 06:48
  - 06:25:23–06:49:02 (23 min 39 s): Codex usage limit until 06:48
  - 08:53:55–11:50:29 (2 h 56 min): Codex usage limit until 11:49
  - 12:22:53–12:29:44 (6 min 51 s): authorize Codex round 4

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | windows complete run | 12:32:49 | 14:10:05 | 1 h 37 min |
| Aceptación | wsl complete run | 14:11:03 | 14:45:22 | 34 min 19 s |
| Aceptación | wsl complete run 2 | 14:53:27 | 16:49:29 | 1 h 56 min |
| Revisión RDD | codex-code-R1 | 07:42:03 | 07:54:51 | 12 min 48 s |
| Revisión RDD | codex-code-R2 | 08:19:33 | 08:29:36 | 10 min 03 s |
| Revisión RDD | codex-R4 | 12:29:55 | 12:36:04 | 6 min 09 s |
| **Aceptación (total)** | | | | **4 h 07 min** |
| **Revisión RDD (total)** | | | | **29 min 00 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `8008b69` | docs: add the comparative benchmark PRD (module 6) |  | 0 min 01 s | Plan (documento de ejecución) 0 min 01 s |
| `06a37d3` | feat(bench): add the benchmark library core | T1 | 16 min 10 s | Exploración y mapa 5 min 24 s, Plan (documento de ejecución) 1 min 04 s, Código y tests 9 min 42 s |
| `838fb35` | refactor(bench): make the worker a role of the benchmark executable | T1 | 1 min 09 s | Código y tests 1 min 09 s |
| `d9b4822` | fix(bench): use a 1 GiB LMDB map | T1 | 0 min 53 s | Código y tests 0 min 53 s |
| `955a590` | docs: accept WSL 2 on ext4 as Linux for the full benchmark runs | T1 | 1 min 37 s | Código y tests 1 min 37 s |
| `5ca33da` | ci: run the reduced benchmark with its six engines on Linux and Windows | T1 | 3 min 24 s | Código y tests 3 min 24 s |
| `fa65b43` | feat(bench): add the Voron worker in .NET | T1 | 13 min 23 s | Código y tests 13 min 23 s |
| `24baebb` | feat(bench): add the Rust worker and the adapters of five engines | T1 | 3 min 20 s | Código y tests 3 min 20 s |
| `586bb7f` | feat(bench): add the orchestrator of the krummi-bench command | T1, assess T1-T3,T5, T4, seam | 40 min 35 s | Código y tests 29 min 42 s, Checks (fmt, clippy, suite de tests) 10 min 47 s, Revisión RDD 0 min 06 s |
| `642d5ab` | docs: add the benchmark execution document | seam | 0 min 15 s | Checks (fmt, clippy, suite de tests) 0 min 15 s |
| `7286171` | fix(bench): keep the engines' default features and reject report paths that name a folder | seam, RDD aed68a6..642d5ab, verifier advisories | 33 min 44 s | Checks (fmt, clippy, suite de tests) 0 min 09 s, Revisión RDD 29 min 09 s, Correcciones de hallazgos 4 min 26 s |
| `fa4f9c8` | docs: update the benchmark phase log | verifier advisories, RDD aed68a6..7286171 | 1 min 45 s | Revisión RDD 1 min 36 s, Correcciones de hallazgos 0 min 09 s |
| `4af47d6` | fix(bench): address the RDD findings of the benchmark slices | RDD aed68a6..7286171 | 23 min 07 s | Revisión RDD 23 min 07 s |
| `229ae90` | docs: record the benchmark RDD reviews | RDD aed68a6..7286171 | 1 min 55 s | Revisión RDD 1 min 55 s |
| `7e70319` | docs: update the benchmark phase log | RDD aed68a6..7286171 | 0 min 08 s | Revisión RDD 0 min 08 s |
| `9ba17b7` | docs(bench): add the first complete Windows report | T6 complete runs | 1 h 37 min | Aceptación 1 h 37 min |
| `a881d48` | fix(bench): let Voron writers wait for the write lock without a limit | T6 complete runs | 42 min 45 s | Aceptación 42 min 45 s |
| `e4ac4f2` | docs(bench): add the first complete Linux report | T6 complete runs | 1 h 56 min | Aceptación 1 h 56 min |
| `de31a97` | fix(bench): limit the Voron write-lock wait to ten minutes | T6 complete runs, RDD last slice 4af47d6..e4ac4f2 | 6 min 16 s | Aceptación 0 min 20 s, Revisión RDD 5 min 56 s |
| `d60b000` | docs: record the complete runs and the last-slice RDD review |  | 0 min 00 s |  |
| `ba90afd` | docs: record the final checks before the Codex review | T6/T7 records, final checks | 4 min 47 s | Checks (fmt, clippy, suite de tests) 4 min 07 s, Documentación 0 min 40 s |
| `7ce3710` | docs(prd): optimizations that change nothing observable need no PRD | Codex PRD R1 | 0 min 26 s | Revisión RDD 0 min 26 s |
| `2918222` | docs(prd): address the Codex review of the benchmark PRD | Codex PRD R1 | 9 min 10 s | Revisión RDD 9 min 10 s |
| `ba6a541` | docs(prd): fix the ratio and threshold of a zero Voron metric | Codex PRD R1 | 30 min 37 s | Revisión RDD 30 min 37 s |
| `7dba24d` | docs: record the approved Codex review of the benchmark PRD | Codex PRD R1 | 5 min 14 s | Revisión RDD 5 min 14 s |
| `6cef8b1` | test(bench): compare the whole row and summary of a zero Voron metric | Codex PRD R1 | 1 min 44 s | Revisión RDD 1 min 44 s |
| `74c8697` | fix(bench): count only the W12 reads that end while the writer writes | Codex PRD R1 | 23 min 11 s | Revisión RDD 23 min 11 s |
| `6afd038` | fix(bench): check that the report folder accepts a new file before the run | Codex PRD R1 | 2 min 33 s | Revisión RDD 2 min 33 s |
| `6c5ebd1` | fix(bench): treat Ctrl+C while the run finishes as an interruption | Codex PRD R1 | 1 min 50 s | Revisión RDD 1 min 50 s |
| `dcd39f8` | test(bench): accept an infinite ratio in the reduced end-to-end run | Codex PRD R1 | 2 min 19 s | Revisión RDD 2 min 19 s |
| `a935cc9` | docs: record the Codex code review and its fixes | Codex PRD R1 | 11 min 19 s | Revisión RDD 11 min 19 s |
| `14730a4` | fix(bench): give the report folder probe a unique name | Codex PRD R1 | 1 min 47 s | Revisión RDD 1 min 47 s |
| `5ac8573` | docs: record the RDD review of the code review fixes |  | 0 min 00 s |  |
| `8972ddf` | docs: record the last RDD review of the code review fixes | Codex PRD R1 | 0 min 47 s | Revisión RDD 0 min 47 s |
| `9cb6a49` | fix(bench): count the W12 reads that end in the writer's time exactly | Codex PRD R1 | 18 min 15 s | Revisión RDD 18 min 15 s |
| `6d35cd5` | fix(bench): retry removing the report and the folder probe while they are held | Codex PRD R1 | 2 min 43 s | Revisión RDD 2 min 43 s |
| `7ecd78a` | docs: add the performance plan to meet the benchmark thresholds | Codex PRD R1 | 4 min 45 s | Revisión RDD 4 min 45 s |
| `d05ba48` | docs: record the second round of the Codex code review | Codex PRD R1 | 6 min 17 s | Revisión RDD 6 min 17 s |
| `e1aaa18` | refactor(bench): fail loudly on a misplaced W12 writer and document the removal wait | Codex PRD R1 | 1 min 43 s | Revisión RDD 1 min 43 s |
| `daad8fd` | docs: record the RDD review of the second round fixes |  | 0 min 00 s |  |
| `c8e52eb` | docs: record the last RDD review before the third Codex round | Codex PRD R1 | 0 min 39 s | Revisión RDD 0 min 39 s |
| `384e13e` | fix(bench): publish the W12 writer's finishing phase with a full fence | Codex code R3, codex-R3 R1 | 29 min 28 s | Revisión RDD 12 min 52 s, Correcciones de hallazgos 16 min 36 s |
| `4e878f2` | docs: add the third round of the Codex code review | codex-R3 R1, rdd 384e13e | 0 min 23 s | Revisión RDD 0 min 10 s, Correcciones de hallazgos 0 min 13 s |
| `c148cdf` | docs(bench): restore the SetOnDrop doc comment and mark the W12 ordering test as a stress test | rdd 384e13e | 1 min 21 s | Revisión RDD 1 min 21 s |
| `a06573b` | docs: record the third round of the Codex code review | rdd 384e13e | 1 min 07 s | Revisión RDD 1 min 07 s |
| `784175e` | docs: record the fourth round of the Codex code review | rdd 384e13e, codex-R4 | 6 min 49 s | Revisión RDD 6 min 49 s |
| — | después del último commit | codex-R4 | 0 min 25 s | Revisión RDD 0 min 25 s |
| **Total** | | | **9 h 34 min** | |

## Informe de cambios (`aed68a6..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi-bench` | 7.277 | 0 | 5.102 | 493 | 30 |
| `docs` | 1.892 | 0 | — | — | 13 |
| `odd` | 309 | 0 | — | — | 2 |
| `crates/krummi` | 91 | 9 | 78 | 11 | 3 |
| `.github` | 46 | 0 | 33 | 6 | 1 |
| `README.md` | 21 | 1 | — | — | 1 |
| `Cargo.toml` | 1 | 1 | 1 | 0 | 1 |
| **Total** | **9.637** | **11** | **5.214** | **510** | **51** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+1468 −30), `docs/benchmark/2026-11-03-linux.md` (+62 −0), `docs/benchmark/2026-11-03-windows.md` (+62 −0)

## Resumen

El banco compara Krummi con Voron, LMDB, redb, canopydb y fjall en 22 cargas que cubren todo lo que el producto hace
(escrituras durables sueltas, concurrentes y por lotes, lecturas al azar con 1, 4 y 16 hilos, recorridos, lecturas
con un escritor al lado, borrados, apertura, valores grandes, multivalor y tamaño fijo) y evalúa los umbrales contra
Voron: al menos 0,80 de sus operaciones por segundo en cada carga. Una sola orden, `cargo run --release -p krummi-bench -- --ravendb <carpeta>`, compila
el programa de Voron contra el código de RavenDB 7.2.7, ejecuta cada carga de cada motor en su propio proceso, uno
tras otro, comprueba cada lectura con sumas conocidas y el mismo flujo de datos en todos los motores, y escribe un
informe en Markdown con las dos tablas y las opciones que cambió. La integración continua ejecuta la versión reducida
con los seis motores en Windows y en Linux. El resultado: Krummi incumple la mayoría de los umbrales; el perfilado
posterior encontró las causas principales (el checksum de cada página en cada lectura, dos cerrojos globales por
acceso a página, copias en la confirmación) y [plan.md](../../docs/performance/plan.md) ordena cómo atacarlas.

## Valores tomados sin preguntar

- Alineación entre los dos workers (L6): las claves del escritor de W13 usan `index_base(W13)`; los tiempos de lectura
  incluyen abrir la transacción; el tiempo de varios hilos va de la liberación de la barrera al fin del último; los
  fallos simulados ocurren al empezar cada repetición; las causas de error en español.
- La caché de 1 GiB de canopydb es la predeterminada y no se lista como opción cambiada.
- Los escritores de Voron esperan su cerrojo hasta 10 minutos (D8); no es una opción de almacenamiento y no se lista.
- En W12 una lectura termina cuando su lector la registra, justo después de comprobar la suma de su valor (D10).
- El informe de la ronda 2 de Codex se tomó como esa ronda aunque el script dio `error`, porque estaba completo.
- R1 de la ronda 3: el escritor publica `FINISHING` con un almacenamiento SeqCst y una barrera completa (Voron:
  `Interlocked.Exchange`); la prueba es de estrés, porque un orden entre núcleos no se puede forzar en una prueba.

## Pendiente (no bloquea)

- **Umbrales incumplidos:** 17 en Windows y 15 en Linux. Con la anotación de K13 del 2026-11-04, cada optimización
  sin cambio observable va directa a ODD; el orden y el diagnóstico están en [plan.md](../../docs/performance/plan.md)
  (O1–O4; O3 necesita PRD). No bloquea: K13 cierra el módulo con su primer informe.
- **Perfilado solo en Linux:** falta Windows, donde W4 incumple (0,74).
- **Sin tiempo máximo por proceso** y **una muerte forzada deja la subcarpeta de trabajo** (Ctrl+C sí la borra): fuera
  de alcance en el PRD.
- **Sin prueba automática** de una carpeta sin permiso de escritura en Windows (necesitaría cambiar ACL) ni del conteo
  de W12 en el programa de Voron (ninguna prueba lo ejecuta fuera de una ejecución del banco); la regla de Rust tiene
  su prueba determinista.
- **AC-K6 no se ejecutó en GitHub**: el trabajo está configurado y la versión reducida pasó en local en Windows y WSL.
- Ningún hallazgo de Codex ni de RDD queda abierto.

**Riesgo:** ítem 5 (CI y dependencias: un trabajo nuevo de integración continua con .NET y RavenDB, y las dependencias
del banco en `Cargo.lock`; el motor no las usa, AC-K14 lo comprueba). El código del motor no cambió.

## Limpieza manual

- Carpeta temporal de la sesión (`scratchpad`, 1,1 GB): `t3`, `voron-probe`, `prof`, `hashbench`, `r1fix`, scripts.
- En WSL: `~/krummi-bench-src`, `~/krummi-bench-target`, `~/krummi-prof-target`, `~/krummi-sse2-target`,
  `~/krummi-avx2-target`, `~/krummi-wsl-target`, `~/hashbench`, `~/prof`, `~/bench-work`, `~/bench-complete.*`,
  `~/bench-reduced.*` (2,3 GB en total, sobre todo las carpetas `target`); `samply` en `~/.cargo/bin` de WSL y de Windows. `perf_event_paranoid` vuelve a 2 al reiniciar WSL.
- `work/ravendb-7.2.7` (330 MB, ignorado por git): hace falta para ejecutar el banco; bórralo solo si no vas a ejecutarlo.
