# CI con GitHub Actions terminada: Linux y Windows en verde, y un bug del Pager corregido

Informe de cierre del cambio `github-actions-ci` (2026-10-22). Enfoque de ejecución: ODD.
Producto: sin PRD, porque es tooling sin cambio de producto, salvo la corrección del Pager, que restaura el
comportamiento que ya fijaba el [PRD del Pager](../../docs/prd/muninndb/pager.prd.md).
Documento de ejecución: [github-actions-ci.md](github-actions-ci.md).

Rama `ci/github-actions`, subida a GitHub con permiso. Se mergeó en `main` en local con fast-forward. **`main` no
se subió:** subirlo o abrir un pull request queda a tu decisión.

## Resultado

- **Especificación:** S1–S4 cumplidas.
  - **S1:** matriz `ubuntu-latest` + `windows-latest`, con fmt, clippy y tests, en cada push y en cada pull
    request a `main`.
  - **S2:** barrido completo de AC10 en los dos sistemas.
  - **S3:** caché de Cargo.
  - **S4:** verificado con corridas reales en GitHub.
- **Aceptación:** la última corrida, `39812202793` sobre `b3a2995`, está en verde. Linux tardó 1 min 07 s y
  Windows 2 min 50 s. El barrido completo de AC10 tardó 42,6 s en Linux y 74,6 s en Windows. Los tests son 27 de
  fallos en Windows (26 en Linux), 19 de apertura, 15 de páginas más el barrido, 3 de política de `unsafe` y 2
  de fachada.
- **Revisiones y verificación:** dos revisiones RDD, las dos aprobadas (ver abajo). No hubo verificador
  independiente: la corrida real de la CI en los dos sistemas es la verificación de este cambio.
- **Hallazgos corregidos por el camino:**
  - **Bug del Pager, también en `main`.** El renombrado por handle en Windows pasaba el nombre destino sin
    terminador NUL, y kernel32 lo lee como cadena terminada en NUL. Según la longitud de la ruta, el archivo
    quedaba como `muninn.data` seguido de basura y el almacén dejaba de reconocerse. Lo detectó el runner de
    Windows. Lo reproduje en local con carpetas de 1 a 12 caracteres y lo corregí en `6a8d000`.
  - La revisión pidió alinear los disparadores con el README y probar el buffer de forma determinista (`b3a2995`).

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `4d37d64..c673e94` | risk, resilience, reliability | aprobada | WARNING `R3-002`: el README decía "en cada push", pero el workflow filtraba ramas; corregido en `b3a2995`. SUGGESTION `R3-001`: la prueba por longitud de ruta solo detectaba el fallo por probabilidad; se agregó una prueba unitaria determinista del buffer en `b3a2995`. | `review-ca79ed5edb730044` |
| `c673e94..b3a2995` | risk, reliability | aprobada | ninguno | `review-5ccd94683f19400d` |

En la primera revisión, la primera consulta de estado agotó su tiempo (`operation_timeout`) sin modificar nada.
Se repitió la consulta de partida y el inicio se reanudó (`replayed`).

## Tiempo

- Estimación: 30–60 min. Inicio real: 2026-10-22 16:55:15 +0100. Fin: 17:17:57 +0100.
- **Duración activa desde el arranque: 22 min 41 s**, el 76 % del mínimo estimado. Lo que más pesó fue
  diagnosticar y corregir el bug del Pager que encontró el runner de Windows (63 % en "Correcciones").
- Esperas por el usuario: ninguna.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 0 min 39 s | 3 % | 1 |
| Código y tests | 0 min 27 s | 2 % | 1 |
| Aceptación | 3 min 43 s | 16 % | 1 |
| Revisión RDD | 3 min 35 s | 16 % | 2 |
| Correcciones de hallazgos | 14 min 17 s | 63 % | 2 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **22 min 41 s** | **100 %** | **8** |

- Inicio: 2026-10-22 16:55:16 +01:00. Fin: 2026-10-22 17:17:57 +01:00.
- Arranque confirmado: 16:55:16. Activo desde el arranque: **22 min 41 s** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `a64152d` | ci: run the engine checks on Linux and Windows with GitHub Actions | T1 | 1 min 04 s | Plan (documento de ejecución) 0 min 39 s, Código y tests 0 min 25 s |
| `ea2f292` | ci: report every failing test instead of stopping at the first crate | T1, T2, windows-runner | 3 min 46 s | Código y tests 0 min 02 s, Aceptación 3 min 43 s, Correcciones de hallazgos 0 min 01 s |
| `6a8d000` | fix(pager): NUL-terminate the target name of the Windows rename by handle | windows-runner | 5 min 31 s | Correcciones de hallazgos 5 min 31 s |
| `c673e94` | docs: note that CI runs the full damage sweep on Linux and Windows | windows-runner | 3 min 58 s | Correcciones de hallazgos 3 min 58 s |
| `b3a2995` | ci: run on every push and test the Windows rename buffer deterministically | windows-runner, ci-branch, R3-001-R3-002 | 4 min 11 s | Revisión RDD 2 min 52 s, Correcciones de hallazgos 1 min 19 s |
| — | después del último commit | R3-001-R3-002, last-commit | 4 min 11 s | Revisión RDD 0 min 43 s, Correcciones de hallazgos 3 min 28 s |
| **Total** | | | **22 min 41 s** | |

Las corridas de CI posteriores a la primera, que eran esperas de entre 2 y 3 minutos cada una, quedaron
registradas dentro de "Correcciones de hallazgos": no marqué la fase de aceptación al lanzarlas. El total es
correcto; el reparto entre fases está sesgado hacia correcciones.

## Informe de cambios (`4d37d64..b3a2995`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/muninndb-pager` | 90 | 18 | 73 | 13 | 2 |
| `.github` | 56 | 0 | 41 | 4 | 1 |
| `odd` | 37 | 0 | — | — | 2 |
| `README.md` | 6 | 1 | — | — | 1 |
| **Total** | **189** | **19** | **114** | **17** | **6** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

No había previsión de líneas. La tabla es la de `b3a2995`, antes de este informe.

## Resumen

Cada push a cualquier rama, y cada pull request a `main`, compila y prueba el motor en Linux y en Windows reales:
formato, clippy sin advertencias, la suite completa y el barrido de daño byte a byte de AC10. La primera
corrida ya pagó la inversión: encontró un bug de memoria en el único código `unsafe` del renombrado en Windows,
que en local pasaba por casualidad.

## Decisiones por defecto, sin preguntar

- **Acciones fijadas a su commit:** `actions/checkout` v7.0.1 y `Swatinem/rust-cache` v2.9.3.
- **Rust 1.99.0 fijado en el workflow**, la misma versión que en local, para que lints o mensajes del
  compilador nuevos (el test de trybuild) no rompan la CI sin aviso. No agregué `rust-toolchain.toml`, así las
  máquinas locales quedan como están.
- **Seguridad de la corrida:** token de solo lectura, y un push nuevo cancela la corrida anterior de la misma
  rama.
- **`--no-fail-fast`** en los tests, para ver todos los fallos de una corrida.

## Pendiente (no bloqueante)

- **`main` local tiene la corrección del Pager, pero GitHub no.** `origin/main` sigue en `4d37d64`, con el bug
  del renombrado. Para publicarla hay que subir `main` o abrir un pull request desde `ci/github-actions`.
- **La versión de Rust de la CI se sube a mano.** Cuando se actualice el toolchain local, hay que cambiar
  `RUST_TOOLCHAIN` en el workflow.
- **Sin comprobación de MSRV.** El manifiesto declara `rust-version = "1.89"`, pero la CI no lo verifica.

**Riesgo:** ítem 5 (CI y entrega: workflow nuevo) e ítem 2 (código `unsafe` del renombrado en Windows). Cubiertos
por las corridas reales en los dos sistemas, la prueba determinista del buffer y dos revisiones RDD.
