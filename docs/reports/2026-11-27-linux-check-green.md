# El check de Linux queda en verde

Informe de cierre de `linux-check-green` (2026-11-27). Enfoque: ODD (tarea pequeña, sin documento de ejecución).
PRD: ninguno (corrección de tests y de herramienta).

Rama `fix/linux-check-green` desde `main` (`234620e`). Sin merge ni `push`.

## Resultado

- **`check` por el runner** sobre `3d90b0d`, rustc 1.99.0: **1351 pasan, 0 fallan**, 36 ignorados, con formato y
  Clippy, aun con `rust-src` instalado en el Rust del usuario.
- **Test intermitente:** los hilos `krummi-statistics` y `krummi-vectors` heredan el contexto de fallas del hilo del
  test y se llevaban la falla inyectada (comprobado con un registro temporal). No era un error demostrado del motor.
  Cuatro tests esperan ahora a ambos hilos. Antes falló en la corrida 12; después, 200 de 200 correctas.
- **`trees_compile`:** el runner usa su propio Rust (`~/krummi-runner/rustup`) con la versión de `ci.yml`.
  `test_runner.sh` y `Test-InvokeLinux.sh` sin fallos.
- **Codex:** [aprobado sin hallazgos](../reviews/linux-check-green/2026-11-27-code.md).

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `234620e..3d90b0d` | resilience, reliability | Aprobado | 3 avisos y 1 sugerencia, sin resolver (ver Pendiente) | review-5208fc7494777dee |

## Tiempo

Estimación: 45 a 90 min.

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 2 min 54 s | 6 % | 2 |
| Código y tests | 33 min 04 s | 66 % | 1 |
| Checks (fmt, clippy, suite de tests) | 4 min 38 s | 9 % | 1 |
| Revisión RDD | 9 min 22 s | 19 % | 2 |
| Correcciones de hallazgos | 0 min 10 s | 0 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **50 min 08 s** | **100 %** | **8** |

- Inicio: 2026-11-27 07:23:31 +00:00. Fin: 2026-11-27 08:13:39 +00:00.
- Arranque confirmado: 07:23:31. Activo desde el arranque: **50 min 08 s** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | codex-R1 | 08:05:02 | 08:13:25 | 8 min 23 s |
| **Revisión RDD (total)** | | | | **8 min 23 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `3485845` | test(tables): wait for the background threads before injecting a snapshot failure |  | 35 min 58 s | Exploración y mapa 2 min 54 s, Código y tests 33 min 04 s |
| `3d90b0d` | fix(runner): run the CI's Rust version from the runner's own installation |  | 0 min 00 s |  |
| — | después del último commit | rdd, codex | 14 min 10 s | Checks (fmt, clippy, suite de tests) 4 min 38 s, Revisión RDD 9 min 22 s, Correcciones de hallazgos 0 min 10 s |
| **Total** | | | **50 min 08 s** | |

## Cambios (`234620e..3d90b0d`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `scripts` | 38 | 1 | 29 | 7 | 2 |
| `crates/krummi` | 11 | 0 | 8 | 2 | 1 |
| `docs` | 3 | 0 | — | — | 1 |
| **Total** | **52** | **1** | **37** | **9** | **4** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

## Decisiones tomadas sin preguntar

- La versión de Rust del runner sale de `ci.yml`, para no declararla en dos sitios.
- Se corrigieron también tres tests con la misma debilidad que el que fallaba.

## Pendiente (no bloqueante)

- Runner: si falla la instalación del toolchain (sin red, por ejemplo) sale antes de crear los resultados.
- Runner: un `RUST_TOOLCHAIN` sin comillas dobles en `ci.yml` se ignora sin avisar y usa el Rust del usuario.
- Runner: la comprobación de versión instalada usa una expresión regular con puntos sin escapar.
- El `rustup default 1.99.0` puesto ayer en esta máquina ya no hace falta para el runner.

**Risk:** 5 (entorno de las ejecuciones del runner).
