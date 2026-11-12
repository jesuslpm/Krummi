# Banco solo con Krummi y Voron, en los núcleos de rendimiento: terminado y verificado, rama sin fusionar

Informe de cierre de `benchmark-krummi-voron` (2026-11-12). Enfoque de ejecución: ODD.
Fuente de producto: [benchmark-krummi-voron.prd.md](../../docs/prd/krummi/benchmark-krummi-voron.prd.md). Documento
de ejecución: [benchmark-krummi-voron.md](benchmark-krummi-voron.md).

Rama `feat/table-row-reads` en `1531bd0` (más los commits de este informe), desde `cac715b`; sin fusionar y sin
publicar. Es la primera parte de la tanda «Hazlo todo»; la fusión en `main` llega al final de la tanda.

## Resultado

- **Especificación:** S1–S6 del documento de ejecución (B1–B6, AC-B1 a AC-B4). `cargo test -p krummi-bench` pasa
  en Windows (60 pruebas de la biblioteca, 3 del lanzador, 26 más de integración) y en WSL 2 Ubuntu (58 de la
  biblioteca); las ejecuciones ignoradas con .NET y RavenDB pasan en los dos sistemas.
- **Aceptación (AC-B1):** ejecuciones completas con `--solo-krummi-voron` bajo el plan de energía del banco. Windows
  (`47f3e8d`, 23 min 24 s): **29 de 29** umbrales, `Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden
  en ellos)`. WSL 2 (`eb24cdf`, 45 min 34 s): **29 de 29**, `Núcleos: sin fijar (el sistema no distingue núcleos de
  rendimiento)`. Informes: [Windows](../../docs/benchmark/2026-11-12-windows-krummi-voron.md) y
  [Linux](../../docs/benchmark/2026-11-12-linux-krummi-voron.md). La ejecución de los seis motores tardaba unas 4 h.
- **Revisiones y verificación:** tres revisiones RDD aprobadas (sección siguiente); verificador independiente
  (Sonnet, alto; ítem 5, dependencias nuevas): S1–S6 PASS sin bloqueos; Codex
  ([informe](../../docs/reviews/benchmark-krummi-voron/2026-11-12-code.md),
  [resoluciones](../../docs/reviews/benchmark-krummi-voron/2026-11-12-code-resolutions.md)): R1 (bloqueante) y R2
  (menor) corregidos, y aprobado en el seguimiento
  ([informe](../../docs/reviews/benchmark-krummi-voron/2026-11-12-code-02.md)).
- **Hallazgos corregidos por el camino:**
  - Un fallo previo del banco: con una `--carpeta` relativa, Voron resolvía la ruta del almacén contra la carpeta de
    su programa, así que su almacén base quedaba vacío y la ejecución completa de Windows fallaba en
    `lecturas-al-azar`. Reproducido a mano sin el banco; el orquestador pasa ahora rutas absolutas (`47f3e8d`,
    prueba `a_relative_work_folder_holds_every_store`, RED y luego GREEN).
  - Codex R1: en Windows el proceso del motor arrancaba antes de entrar en el job con el límite. Ahora arranca a
    través del propio ejecutable del banco como lanzador, que entra en el job antes de crear el proceso (`a65a111`);
    tras la RDD de esa corrección, un lanzador que no consigue limitarse no lanza nada y el banco arranca el proceso
    sin límite (`b02b34b`).
  - Codex R2: la prueba de integración rechazaba `Núcleos: desconocido`, que B5 permite.

## Cobertura de los criterios de aceptación

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-B1 | ejecuciones completas de Windows y WSL 2 (L6, L7) | PASS: 29 de 29 en los dos, 29 + 12 filas, sin `no aplica`, solo Krummi y después Voron |
| AC-B2 | `a_krummi_and_voron_only_report_has_the_exact_shape`, `a_krummi_and_voron_only_report_keeps_the_thresholds_and_the_reduced_rules`, `the_cores_line_says_where_the_engines_were_measured` | PASS |
| AC-B3 | `the_fastest_class_is_chosen_only_when_there_are_several`, `a_limited_process_and_the_ones_it_starts_run_on_its_cores` (Windows y Linux), `a_process_that_cannot_be_limited_runs_without_the_limit`, `a_process_whose_launcher_cannot_start_runs_without_the_limit`, `a_failure_with_the_cores_changes_only_the_cores_line`, `tests/cores.rs` | PASS |
| AC-B4 | `invalid_arguments_name_the_first_one`, `an_invalid_argument_is_named_and_ends_with_code_2`, `a_krummi_and_voron_only_run_stops_on_a_failure` | PASS |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `cac715b..e95287d` | risk, resilience, readability, reliability (escalada: ítem 5) | aprobada | 3 SUGGESTION de legibilidad, corregidas en `eb24cdf`; WARNING de la prueba que suponía el núcleo 0, corregida; 2 WARNING sobre clases de capacidad casi iguales en Linux, mantenidas por B4 (pregunta pendiente) | `review-4db0f4a645af578f` |
| `e95287d..93ba937` | readability, reliability | aprobada | WARNING del respaldo de `absolute`, comentado; 2 SUGGESTION, corregidas en `1e9dffc`; SUGGESTION de la prueba ignorada, descartada (CI la ejecuta) | `review-2bf473a71af1ed9a` |
| `93ba937..440f8ac` | risk, resilience, reliability (escalada: ítem 4) | aprobada | WARNING del motor huérfano sin job, del arranque fallido del lanzador y de la cadena de Windows sin prueba: corregidas en `b02b34b`; WARNING de Ctrl+C y de la prueba que acepta `desconocido`: mantenidas, con su razón en las resoluciones | `review-57046d6b0ba0caf5` |

`b02b34b` y `1531bd0` los revisó Codex en su seguimiento, aprobado.

## Tiempo

- Estimación: 6 h – 10 h para toda la tanda (este PRD, W25, W23, ejecuciones, limpieza y fusión). Inicio real:
  2026-11-12 17:12:58 +0000. Fin de esta parte: 20:19:18.
- **Duración activa: 3 h 06 min**, de la que la aceptación se llevó 1 h 36 min (dos ejecuciones de Windows que
  fallaron por la carpeta relativa antes de la buena) y las revisiones 50 min (dos rondas de Codex y tres RDD).
- Esperas por el usuario: ninguna.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 6 min 02 s | 3 % | 1 |
| Plan (documento de ejecución) | 0 min 22 s | 0 % | 1 |
| Código y tests | 15 min 48 s | 8 % | 1 |
| Checks (fmt, clippy, suite de tests) | 9 min 43 s | 5 % | 1 |
| Aceptación | 1 h 36 min | 52 % | 2 |
| Revisión RDD | 49 min 41 s | 27 % | 2 |
| Correcciones de hallazgos | 7 min 46 s | 4 % | 1 |
| Cierre (informe y merge) | 0 min 01 s | 0 % | 1 |
| **Total activo** | **3 h 06 min** | **100 %** | **10** |

- Inicio: 2026-11-12 17:12:58 +00:00. Fin: 2026-11-12 20:19:18 +00:00.
- Arranque confirmado: 17:12:58. Activo desde el arranque: **3 h 06 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | reduced solo run | 17:27:31 | 17:35:00 | 7 min 29 s |
| Aceptación | full solo windows base | 17:51:19 | 18:02:37 | 11 min 18 s |
| Aceptación | full solo wsl base | 18:02:37 | 18:48:11 | 45 min 34 s |
| Aceptación | full solo windows base2 | 18:48:58 | 19:00:40 | 11 min 42 s |
| Aceptación | full solo windows fixed | 19:10:08 | 19:33:32 | 23 min 24 s |
| Revisión RDD | codex-R1 | 19:36:16 | 19:48:14 | 11 min 58 s |
| Revisión RDD | codex-R2 | 20:13:45 | 20:18:44 | 4 min 59 s |
| **Aceptación (total)** | | | | **1 h 39 min** |
| **Revisión RDD (total)** | | | | **16 min 57 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `e05d783` | feat(bench): measure only Krummi and Voron with --solo-krummi-voron | T1 | 9 min 39 s | Exploración y mapa 6 min 02 s, Plan (documento de ejecución) 0 min 22 s, Código y tests 3 min 15 s |
| `e95287d` | feat(bench): measure the engines on the performance cores and report where | T1, T2 | 22 min 11 s | Código y tests 12 min 33 s, Checks (fmt, clippy, suite de tests) 9 min 38 s |
| `eb24cdf` | refactor(bench): clarify the limit of the engine processes after the review | T2 | 4 min 21 s | Checks (fmt, clippy, suite de tests) 0 min 05 s, Revisión RDD 4 min 16 s |
| `47f3e8d` | fix(bench): give the workers absolute store paths for a relative work folder | T3, T4 | 1 h 20 min | Aceptación 1 h 12 min, Revisión RDD 0 min 17 s, Correcciones de hallazgos 7 min 45 s |
| `93ba937` | docs(benchmark): add the Krummi and Voron only runs on Windows and Linux | T4, T3 | 24 min 12 s | Aceptación 24 min 11 s, Correcciones de hallazgos 0 min 01 s |
| `1e9dffc` | docs(bench): explain the absolute store paths and the limit flag | T3 | 2 min 21 s | Aceptación 0 min 19 s, Revisión RDD 2 min 02 s |
| `a65a111` | fix(bench): start Windows engine processes inside their core limit |  | 29 min 06 s | Revisión RDD 29 min 06 s |
| `440f8ac` | docs(reviews): add the Krummi and Voron only code review and its resolutions |  | 0 min 01 s | Revisión RDD 0 min 01 s |
| `b02b34b` | fix(bench): start an engine process without its limit when the launcher cannot limit it |  | 8 min 27 s | Revisión RDD 8 min 27 s |
| `1531bd0` | docs(reviews): record the review of the launcher fixes |  | 0 min 00 s |  |
| — | después del último commit |  | 5 min 33 s | Revisión RDD 5 min 32 s, Cierre (informe y merge) 0 min 01 s |
| **Total** | | | **3 h 06 min** | |

## Informe de cambios (`cac715b..1531bd0`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi-bench` | 1.340 | 56 | 1.037 | 127 | 16 |
| `docs` | 320 | 0 | — | — | 4 |
| `odd` | 136 | 0 | — | — | 2 |
| `README.md` | 6 | 2 | — | — | 1 |
| **Total** | **1.802** | **58** | **1.037** | **127** | **23** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+32 −0)

## Resumen

El banco tiene un modo `--solo-krummi-voron` que mide solo los dos motores que comparan los umbrales: una ejecución
completa baja de unas 4 h a unos 25 min en Windows y 45 min en WSL 2, y vale para AC19 (B6). En toda ejecución, los
procesos de los motores corren en los núcleos de rendimiento cuando el sistema los distingue, y el informe lo dice en
la línea *Núcleos*. Las clases salen del comando `cores` del programa de Voron (`EfficiencyClass` en Windows,
`cpu_capacity` en Linux). El límite usa solo interfaces seguras (K14): en Windows, un job de `win32job` en el que
entra un lanzador antes de crear el proceso del motor; en Linux, la afinidad del hilo que lo arranca (`nix`).

## Decisiones tomadas sin preguntar

- D1: el mecanismo del límite y de las clases (dejado a la implementación por el PRD), sin código no seguro.
- La corrección de la carpeta relativa, un fallo previo del banco sin cambio de producto: sin PRD.

## Pendiente (no bloqueante)

- **Pregunta de producto:** en un núcleo Linux que da valores de `cpu_capacity` algo distintos a núcleos del mismo
  tipo, la clase más rápida puede tener uno o dos núcleos; B4 cuenta cada valor como clase, así que el código lo
  sigue tal cual. WSL 2 no da clases, así que no afecta a las mediciones de hoy.
- `Cores.cs` no tiene prueba aislada; lo cubren las ejecuciones reales (16 de 32 en Windows).
- Núcleos de un segundo grupo de procesadores de Windows (más de 64) dan `Núcleos: desconocido`.
- El informe de Windows dice «con cambios sin confirmar» porque el registro de fases, versionado, se escribía
  durante la ejecución.

**Riesgo:** ítem 5 (dependencias nuevas `win32job` y `nix`, solo del banco) e ítem 4 (orden de arranque de los
procesos de los motores respecto a su límite); cubiertos por el verificador independiente, las RDD escaladas y las
dos rondas de Codex.
