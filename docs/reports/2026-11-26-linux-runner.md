# Runner de Linux terminado y verificado en Linux; falta probarlo desde Windows

Informe de cierre de `linux-runner` (2026-11-26). Enfoque: ODD (tarea pequeña, sin documento de ejecución).
PRD: ninguno (herramienta sin cambio de producto).

Rama `feat/linux-runner` desde `main` (`dfba65f`), en un clon aparte. Sin `push` ni merge.

## Resultado

- **Entregado:** `scripts/runner/runner.sh` (Linux), `scripts/runner/Invoke-Linux.ps1` (Windows), la guía
  [linux-runner.md](../linux-runner.md) y las instrucciones en `CLAUDE.md`, `AGENTS.md` y `README.md`.
- **Pruebas:** `test_runner.sh` y `Test-InvokeLinux.sh` sin fallos (el segundo, con `ssh` y `scp` simulados).
- **`check` real por el runner** (commit `ad3b650`, rustc 1.100.0): 1251 pasan, **1 falla** (`trees_compile`) y 35
  ignorados. La rama no toca Rust. Causa probable, no comprobada: el CI usa rustc 1.99.0. Clippy y formato
  pasaron. No se repitió tras las correcciones posteriores, que solo cambian los scripts del runner.
- **Ejecución real final** de `run` sobre `562378e` en `~/krummi-runner`: correcta.
- **No probado:** el recorrido desde Windows (PowerShell en Windows y transporte SSH real).
- **Hallazgos corregidos:** `$KRUMMI_RESULTS` se expandía antes de existir; `check` ignoraba argumentos;
  la limpieza conservaba repositorios anidados; dos primeras llamadas simultáneas chocaban.
- **Revisión Codex:** [ronda 1](../reviews/linux-runner/2026-11-26-code.md),
  [ronda 2, aprobada](../reviews/linux-runner/2026-11-26-code-02.md) y
  [resoluciones](../reviews/linux-runner/2026-11-26-code-resolutions.md).

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `dfba65f..ad3b650` | risk, resilience, reliability | Corregido (`d20cf56`) y aprobado | 3 críticos (R1-001 y R3-001, expansión prematura; R3-002, argumentos de `check`), corregidos; 6 avisos y 1 sugerencia no bloqueantes, texto no registrado | review-472c7acda74909f4 |
| `d20cf56..562378e` | resilience, reliability | Aprobado | 1 sugerencia: la prueba de inicialización simultánea es probabilística y solo mira el código de salida | review-e7a1ed44401fc491 |

## Tiempo

- Estimación: 30 a 60 min. **Activo desde el arranque: ver la tabla**; dentro de la estimación o por debajo.
- La mayor parte se fue en revisiones (RDD y las dos rondas de Codex).

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 0 min 00 s | 0 % | 1 |
| Código y tests | 2 min 21 s | 8 % | 1 |
| Checks (fmt, clippy, suite de tests) | 4 min 00 s | 13 % | 2 |
| Revisión RDD | 21 min 32 s | 72 % | 5 |
| Correcciones de hallazgos | 1 min 55 s | 6 % | 6 |
| **Total activo** | **29 min 48 s** | **100 %** | **15** |

- Inicio: 2026-11-26 09:45:21 +00:00. Fin: 2026-11-26 10:22:45 +00:00.
- Arranque confirmado: 09:45:21. Activo desde el arranque: **29 min 48 s** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 7 min 36 s.
  - 09:55:03–09:59:01 (3 min 58 s): trust the clone in Codex
  - 10:11:12–10:14:50 (3 min 38 s): codex violation

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Checks (fmt, clippy, suite de tests) | runner check | 09:47:46 | 09:51:34 | 3 min 48 s |
| Revisión RDD | codex-R1 | 09:54:32 | 09:54:32 | 0 min 00 s |
| Revisión RDD | codex-R1 | 09:54:40 | 09:54:41 | 0 min 01 s |
| Revisión RDD | codex-R1 | 09:54:54 | 09:54:55 | 0 min 01 s |
| Revisión RDD | codex-R1 | 09:59:07 | 10:11:04 | 11 min 57 s |
| Revisión RDD | codex-R2 | 10:15:05 | 10:22:00 | 6 min 55 s |
| **Checks (fmt, clippy, suite de tests) (total)** | | | | **3 min 48 s** |
| **Revisión RDD (total)** | | | | **18 min 54 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `af568c4` | feat(runner): run builds, tests, benchmarks and profiling of a commit on a Linux machine |  | 0 min 47 s | Código y tests 0 min 47 s |
| `6af2ab0` | feat(runner): launch a task on the Linux runner from Windows and bring back its results |  | 1 min 08 s | Código y tests 1 min 08 s |
| `ad3b650` | docs(runner): guide and agent instructions for the Linux runner |  | 0 min 26 s | Código y tests 0 min 26 s |
| `d20cf56` | fix(runner): expand the arguments in the task's shell and reject arguments to check |  | 6 min 06 s | Checks (fmt, clippy, suite de tests) 3 min 56 s, Revisión RDD 1 min 38 s, Correcciones de hallazgos 0 min 32 s |
| `562378e` | fix(runner): wipe nested repositories and serialize the first initialization | codex | 13 min 41 s | Revisión RDD 12 min 18 s, Correcciones de hallazgos 1 min 23 s |
| — | después del último commit | codex, rdd-2 | 7 min 40 s | Checks (fmt, clippy, suite de tests) 0 min 04 s, Revisión RDD 7 min 36 s |
| **Total** | | | **29 min 48 s** | |

## Cambios (`dfba65f..562378e`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `scripts` | 323 | 0 | 235 | 43 | 4 |
| `docs` | 59 | 0 | — | — | 1 |
| `CLAUDE.md` | 15 | 0 | — | — | 1 |
| `README.md` | 8 | 0 | — | — | 1 |
| `AGENTS.md` | 5 | 0 | — | — | 1 |
| **Total** | **410** | **0** | **235** | **43** | **8** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

## Resumen

Desde Windows, un solo comando envía un commit a esta máquina por SSH, ejecuta compilación, tests, benchmarks o
perfilado y trae los resultados con sus metadatos. El código no pasa por GitHub, solo corre una ejecución a la vez
y nada se edita en Linux.

## Decisiones tomadas sin preguntar

- Rama desde `main`, en un clon aparte, para no tocar la carpeta donde trabajaba otra sesión.
- Todo lo del runner vive en `~/krummi-runner`; la configuración de Windows, en `%USERPROFILE%\.krummi-runner.json`.
- Los agentes piden permiso una vez por sesión antes de usar el runner; no di una autorización permanente.
- La revisión de Codex usó el `Invoke-CodexReview.ps1` de `feat/tables-vector-indexes`: el de `main` no encuentra
  Codex en Linux (corregido allí en `6e3d022`).
- Con permiso del usuario: el clon quedó como confiable en `~/.codex/config.toml` y se aceptó el `violation` de la
  ronda 1 (`.atl/skill-registry.md`, escrito por el hook de arranque de Codex).

## Pendiente (no bloqueante)

- Probar desde Windows. Hasta entonces el script de Windows solo está verificado con dobles.
- `trees_compile` con rustc 1.100.0: decidir si se fija la versión (`rust-toolchain.toml`).
- Avisos RDD en `Invoke-Linux.ps1` (líneas 33-36, 48, 55-67) y `runner.sh` (51-52, 58, 75).
- Reforzar la prueba de inicialización simultánea.

**Risk:** 2 (ejecución remota por SSH) y 4 (bloqueos).

## Limpieza manual

- `~/krummi-runner` (caché de compilación del runner; se puede borrar).
- `/tmp/krummi-review-linux-runner.PoXqkR`, `/tmp/krummi-review-runner-concurrency.p0ALgK`,
  `/tmp/krummi-review-init-spot.2JOy0v` (restos de la revisión de Codex).
