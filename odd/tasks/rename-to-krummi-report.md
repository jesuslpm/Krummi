# El producto se llama Krummi: código, almacén, documentación y herramientas renombrados y verificados

Informe de cierre de `rename-to-krummi` (2026-10-23). Enfoque de ejecución: ODD.
Fuente de producto: [rename-to-krummi.prd.md](../../docs/prd/krummi/rename-to-krummi.prd.md). Documento de
ejecución: [rename-to-krummi.md](rename-to-krummi.md).

Rama `feat/rename-to-krummi`, fusionada en `main` de la copia principal sin commit de fusión. No se publicó nada
(push). El renombre del repositorio de GitHub, el cambio de `origin` y la copia de la memoria se hacen después de la
fusión (R9); su resultado está en [Pasos externos del cierre](#pasos-externos-del-cierre).

## Resultado

- **Especificación:** R1–R8 y R10–R12 entregadas en el repositorio:
  - Paquetes `krummi`, `krummi-pager` y `krummi-tx`.
  - Archivos `krummi.data`, `krummi.data.new`, `krummi.journal.<19 cifras>` y `krummi.temp`.
  - Marca nueva, de 16 bytes: `\x89Krummi\r\n\x1a\n` y cinco ceros.
  - Formato `3`.
  - Errores que nombran Krummi.
  - Documentación y PRD renombrados, con su anotación; los PRD están en `docs/prd/krummi/`.
  - Copias de trabajo `krummi-<n>`.

  Windows: 164 pruebas pasan, 0 fallan y 3 quedan ignoradas (los barridos completos, que se corren aparte). `cargo fmt`
  y `cargo clippy -D warnings` limpios. Linux: suite en verde sobre `6da69ea`. Herramientas: Python 33 + 62 + 24 OK;
  PowerShell `Test-Worktree.ps1` 41 de 41.
- **Aceptación:** barridos AC10 completos en verde en las dos plataformas, en modo release:
  - Linux, sobre `6da69ea`: Pager 43 s, transacciones 32 s y diario 19 s.
  - Windows, sobre el `crates` de HEAD: Pager 17 s, transacciones 32 s y diario 181 s. Se corrieron con los temporales
    de las pruebas en una carpeta excluida de Microsoft Defender.
- **Revisiones y verificación:**
  - Un verificador independiente revisó T2, que toca el formato en disco: S1–S4 pasan, sin bloqueantes. Usó almacenes
    creados por el motor anterior, cerrados sin error y con confirmaciones pendientes en el diario. Todos dan
    NotAStore y quedan con el mismo hash.
  - Cuatro revisiones RDD aprobadas (ver [Revisiones RDD](#revisiones-rdd)).
- **Hallazgos corregidos durante la ejecución:**
  - Ninguna prueba fijaba el conjunto de paquetes (aviso de RDD en T1). Se añadió `crates/krummi/tests/packages.rs`.
  - El documento de ejecución enlazaba la carpeta vieja de los PRD (aviso del verificador). Corregido.
  - El ejemplo del README no habría aplicado la exclusión del antivirus, porque usaba `%TEMP%` con su ruta corta.
    Corregido en `0a6c094`, que además restaura `TMP`.

## Cobertura de los criterios de aceptación

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-R1 | `ac_r1_a_new_store_uses_the_krummi_names_mark_and_format_3` (`crates/krummi-tx/tests/recovery.rs`) | PASS |
| AC-R2 | `ac_r2_ac_r6_what_muninndb_left_is_not_a_store` (cinco carpetas) y sondas del verificador con almacenes del motor anterior | PASS |
| AC-R3 | `ac_r3_the_muninndb_mark_is_not_the_mark` | PASS |
| AC-R4 | `ac_r4_muninndb_entries_in_a_krummi_store_are_foreign` (uso y cierre; caída y recuperación) | PASS |
| AC-R5 | `ac_r5_the_workspace_has_only_the_krummi_packages`, la prueba de documentación y un proyecto nuevo fuera del repositorio con `krummi = { path = ... }` | PASS. La API pública todavía no guarda datos, así que "lee lo escrito" se limita a confirmar y reabrir |
| AC-R6 | `ac_r2_ac_r6_...` (no es un almacén) y `ac9_other_format_versions_are_refused_without_touching_the_store` con la sonda del verificador (formato distinto) | PASS |
| AC-R7 | `git ls-files \| grep -i muninn` (vacío) y `git grep -c -i muninn`: solo en R10 (a)–(e). Comprobación por secciones de los PRD vigentes: 0 líneas fuera de la historia | PASS |
| AC-R8 | Lectura del README, `CLAUDE.md`, `docs/worktrees.md` y `docs/methodology/prompts-worktrees.md` | PASS |
| AC-R9 | `Test-Worktree.ps1` (41 escenarios con `krummi-<n>`) | PASS en Windows |
| AC-R10 | `gh repo rename` y `git remote get-url origin` | ver [Pasos externos del cierre](#pasos-externos-del-cierre) |
| AC-R11 | Enlaces relativos y anclas: PRD 119, 0 rotos. README, `CLAUDE.md`, la guía y la metodología: 74, 25 rotos | PASS en lo que cambió. Los 25 rotos están en `docs/methodology` y ya lo estaban en `main` (pendiente) |
| AC-R12 | Proyecto de memoria detectado y copia de las memorias | ver [Pasos externos del cierre](#pasos-externos-del-cierre) |
| AC-R13 | Suite y barridos AC10 completos en Windows y Linux; Python en las dos; PowerShell en Windows | PASS |
| AC9 adaptado | `ac9_...` con formato `0`, `2` y `4`; `ac_j6_...` con formato `1` | PASS |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `main..903cd08` | reliability | aprobada | Dos avisos, los dos corregidos. WARNING R3-ac-r5-no-package-set-assertion: ninguna prueba fija los paquetes; se añadió `packages.rs` en T2. SUGGESTION R3-ledger-t1-status: tabla de tareas desactualizada; se actualizó | `review-bcdaf9e94782e648` |
| `5129427..6da69ea` | risk, reliability | aprobada | Dos avisos, aceptados: la prueba de paquetes compara el manifiesto como texto a propósito, para fijar su forma exacta. WARNING R3-workspace-members-literal; SUGGESTION R3-package-name-parse | `review-2bff1273b1ee719b` |
| `6da69ea..d132208` | readability, reliability | aprobada | WARNING R2-001, R2-002, R3-002 y SUGGESTION R3-003 dan por hecho que el código y los PRD no se renombraron: no aplican, se renombraron en `6da69ea` y `5129427`. WARNING R2-003 y R3-001 tratan de copias creadas con el prefijo viejo: no aplican, no hay ninguna (R8). SUGGESTION R2-004, alineación de la ayuda de `WorktreeCommon.psm1`: aceptada tal cual | `review-4effe6bb7ceabfe2` |
| `d132208..4526f12` | risk, readability | aprobada | WARNING R1-001: la carpeta excluida no se analiza mientras dure la exclusión; el README lo advierte y pide quitarla al terminar (`0a6c094`). SUGGESTION R2-tmp-not-restored: el ejemplo restaura `TMP` (`0a6c094`). WARNING R2-remove-path-fallback y SUGGESTION R2-duplicated-admin-check: aceptadas (dos scripts pequeños con la misma carpeta por defecto) | `review-0784cc851286cc43` |

Evaluaciones que no abrieron revisión:
- `903cd08..5129427` (T5, PRD): pasiva. La revisión de `903cd08..6da69ea` se negó por `lens_context_budget_exceeded`, porque la mudanza de los PRD cuenta archivos enteros, y se partió en dos tramos.
- `4526f12..0a6c094` (README): pasiva.

## Pasos externos del cierre

Hechos tras la fusión en `main` (`ec70386`), en el orden de R9, con la autorización dada al arrancar:

1. **GitHub** (AC-R10, PASS): `gh repo rename Krummi -R jesuslpm/MuninnDB --yes` salió con código 0, y
   `gh repo view jesuslpm/Krummi` responde `jesuslpm/Krummi`, `https://github.com/jesuslpm/Krummi`.
2. **Remoto** (AC-R10, PASS): `origin` es `https://github.com/jesuslpm/Krummi.git`, y `git fetch` funciona.
   `origin/main` sigue en `8168fa1`: no se publicó nada.
3. **Memoria** (AC-R12, PASS en la copia, pendiente en la detección):
   - Se exportó `muninndb`, que tenía 24 memorias, incluidas las guardadas en esta ejecución. Se importaron en
     `krummi` con `sync_id` y fechas nuevos, dentro de una sesión nueva que las agrupa; las sesiones viejas no se
     copiaron.
   - Resultado: `krummi` tiene 24 memorias y `muninndb` conserva las suyas, 24. Comparadas por tipo, título,
     contenido, ámbito y clave de tema, las dos colecciones son idénticas.
   - Engram no tenía cómo renombrar ni fusionar el proyecto. La importación pide que exista la sesión de cada
     memoria: probada antes en una base desechable.
   - En `C:\projects\MuninnDB`, Engram sigue detectando `muninndb`, porque esta carpeta ya tiene sesiones de ese
     proyecto. En un clon nuevo con el `origin` nuevo detecta `krummi`. La detección se comprueba después de mover la
     carpeta a `C:\projects\Krummi` (R12).

## Tiempo

- Estimación: 1 h 15 min – 2 h. Arranque real: 2026-10-23 08:39:40. Fin de la verificación: 09:54:12.
- **Duración activa desde el arranque: 1 h 10 min**, por debajo del mínimo de la estimación. A eso se suma el cierre
  de después del informe (fusión, GitHub, memoria), que no está en el registro de fases.
- Esperas por el usuario: 09:41:13–09:45:18 (4 min 05 s), para que agregara la exclusión del antivirus.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 0 min 49 s | 1 % | 1 |
| Código y tests | 13 min 46 s | 20 % | 4 |
| Checks (fmt, clippy, suite de tests) | 2 min 09 s | 3 % | 2 |
| Aceptación | 5 min 24 s | 8 % | 1 |
| Revisión RDD | 43 min 19 s | 62 % | 4 |
| Documentación | 4 min 45 s | 7 % | 1 |
| **Total activo** | **1 h 10 min** | **100 %** | **13** |

- Inicio: 2026-10-23 08:39:55 +01:00. Fin: 2026-10-23 09:54:12 +01:00.
- Arranque confirmado: 08:39:55. Activo desde el arranque: **1 h 10 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 4 min 05 s.
  - 09:41:13–09:45:18 (4 min 05 s): antivirus exclusion by the user

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | T1 RDD reliability lens | 08:45:26 | 08:46:17 | 0 min 51 s |
| Aceptación | Linux suite and full AC10 sweeps | 08:54:53 | 08:55:49 | 0 min 56 s |
| Aceptación | Linux full AC10 sweeps | 08:56:19 | 08:56:20 | 0 min 01 s |
| Aceptación | Linux full AC10 sweeps | 08:57:03 | 08:57:05 | 0 min 02 s |
| Aceptación | Linux full AC10 sweeps | 08:57:26 | 08:59:05 | 1 min 39 s |
| Revisión RDD | T2 RDD risk and reliability lenses | 09:01:42 | 09:02:24 | 0 min 42 s |
| Revisión RDD | T3-T4 RDD readability and reliability lenses | 09:04:27 | 09:04:58 | 0 min 31 s |
| Aceptación | Windows full AC10 sweeps | 08:54:23 | 09:46:28 | 52 min 05 s |
| Revisión RDD | last slice RDD risk and readability lenses | 09:51:01 | 09:51:27 | 0 min 26 s |
| **Aceptación (total)** | | | | **54 min 43 s** |
| **Revisión RDD (total)** | | | | **2 min 30 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `f95451f` | docs: add the rename-to-krummi PRD and execution document |  | 0 min 49 s | Plan (documento de ejecución) 0 min 49 s |
| `903cd08` | refactor: rename the packages to krummi, krummi-pager and krummi-tx | T5, T1 | 3 min 08 s | Código y tests 3 min 08 s |
| `5129427` | docs: rename the PRDs to Krummi and move them to docs/prd/krummi | T1, T2 | 1 min 50 s | Código y tests 0 min 27 s, Revisión RDD 1 min 23 s |
| `6da69ea` | feat!: name the store files krummi.*, replace the mark and move to format 3 | T2 | 8 min 20 s | Código y tests 8 min 20 s |
| `95918db` | chore: name the working copies krummi-<n> in the worktree tooling | T2, T3, T4 | 8 min 52 s | Código y tests 1 min 51 s, Revisión RDD 2 min 16 s, Documentación 4 min 45 s |
| `d132208` | docs: rename the product to Krummi in the README, instructions and guides |  | 0 min 00 s |  |
| `4526f12` | chore: add scripts to exclude a benchmark folder from Microsoft Defender | T2, T6, T3-T4 | 38 min 19 s | Checks (fmt, clippy, suite de tests) 0 min 16 s, Revisión RDD 38 min 03 s |
| `0a6c094` | docs: use the long path of the excluded folder and restore TMP in the Defender example | antivirus measurement, last slice | 6 min 47 s | Aceptación 5 min 24 s, Revisión RDD 1 min 23 s |
| — | después del último commit | last slice, T6 | 2 min 07 s | Checks (fmt, clippy, suite de tests) 1 min 53 s, Revisión RDD 0 min 14 s |
| **Total** | | | **1 h 10 min** | |

Lectura de la tabla:
- La "Revisión RDD" pesa 43 min, pero las revisiones en sí tomaron 2 min 30 s en segundo plano. Unos 36 min de esa fase
  (09:05–09:41, dentro de `4526f12`) fueron el diagnóstico de la lentitud del barrido de Windows, que no se marcó como
  fase aparte.
- Los barridos de Linux aparecen tres veces vacíos: dos arranques que fallaron por cómo `wsl` pasa la orden.
- El barrido de Windows sin exclusión (52 min) se cortó al confirmar la causa. El que vale se corrió después, con la
  exclusión: 3 min 50 s.

## Informe de cambios (`main..0a6c094`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 762 | 244 | — | — | 12 |
| `crates/krummi-tx` | 237 | 66 | 200 | 28 | 15 |
| `crates/krummi-pager` | 89 | 87 | 46 | 43 | 18 |
| `scripts` | 92 | 29 | 55 | 29 | 8 |
| `crates/krummi` | 65 | 31 | 51 | 11 | 13 |
| `README.md` | 35 | 22 | — | — | 1 |
| `odd` | 54 | 0 | — | — | 2 |
| `.claude` | 10 | 10 | 8 | 0 | 3 |
| `CLAUDE.md` | 4 | 4 | — | — | 1 |
| `.github` | 3 | 3 | 3 | 0 | 1 |
| `Cargo.toml` | 2 | 2 | 1 | 1 | 1 |
| **Total** | **1.353** | **498** | **364** | **112** | **75** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+34 −34)

No hubo previsión de líneas.

## Resumen

El producto pasa a llamarse Krummi en todo lo que ve quien lo usa o lo mantiene. El integrador declara la dependencia
`krummi`, y un almacén nuevo es `krummi.data`, con la marca nueva y el formato `3`. Una carpeta con lo que dejaba
MuninnDB no es un almacén de Krummi y no se toca. Eso sale de las reglas existentes de la marca y de los archivos
ajenos, sin código que recuerde el nombre anterior. El nombre viejo solo aparece en la historia y en la última
sección de pruebas de `crates/krummi-tx/tests/recovery.rs`, que construye esos restos para comprobarlo.

Al medir, apareció que en esta máquina Microsoft Defender analiza cada archivo modificado al abrirlo. Con el
contenido del formato 3 tarda unas 3,5 veces más: los archivos solo difieren en el byte de versión y en las sumas de
control del encabezado. Con la carpeta excluida desaparece del todo: la muestra baja de 15 s a 0,06 s en los dos
formatos. Por eso se añadieron `scripts/antivirus/` y su uso en el README.

## Decisiones tomadas sin preguntar

- Ejecución en la copia principal, sin copia de trabajo (R9, fijada en el PRD).
- Las pruebas del nombre viejo viven en una sola sección de `crates/krummi-tx/tests/recovery.rs`, que ya tenía la
  maquinaria de caídas; así el resto permitido por R10 (e) queda en un archivo.
- La revisión de T2 se partió del commit de los PRD por el presupuesto de contexto de RDD.
- Se cortó el barrido de Windows sin exclusión al confirmar la causa, y se repitió con la exclusión.

## Pendiente (no bloqueante)

- Que Engram detecte `krummi` en la copia principal: hoy detecta `muninndb` por la carpeta. Se comprueba después de
  mover la carpeta a `C:\projects\Krummi`, que es un paso del usuario (R12). Las memorias ya están copiadas.

- 25 enlaces rotos en `docs/methodology`, que apuntan a PRD e informes de chicago-crime. Ya estaban rotos en `main` y
  este cambio no los toca.
- En Windows, con el antivirus activo, el barrido completo del Pager es lento en esta máquina. En `main` se estimaban
  unos 23 min y con formato 3 unos 85 min, a partir de la muestra. La CI le da 30 min: si su ejecutor de Windows
  analiza los archivos igual, puede pasarse. No se pudo comprobar sin publicar.
- Avisos de RDD aceptados (ver la tabla): la forma literal de la prueba de paquetes, la alineación de la ayuda de
  `WorktreeCommon.psm1` y la normalización de la ruta en `Remove-DefenderExclusion.ps1`.
- Defender guardó la exclusión con la ruta corta de `%TEMP%`. Aun así se aplica a los archivos abiertos por la ruta
  larga, no a los abiertos por la corta.

**Riesgo:** item 1 (T2 cambia el formato en disco: almacenes de formato 2 dejan de abrirse, como pide R6). Revisado
con el verificador independiente y RDD (riesgo y fiabilidad).

## Limpieza manual

- Mover la copia principal a `C:\projects\Krummi` con el editor cerrado (R12) y, ya abierta en la carpeta nueva,
  comprobar que Engram detecta `krummi`.

- Quitar la exclusión de Defender, como administrador:
  `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/antivirus/Remove-DefenderExclusion.ps1`.
- `~/muninn-accept` en WSL (1,2 GB): el clon de aceptación de Linux, con su carpeta `target`. Se puede borrar o
  renombrar.
