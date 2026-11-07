# Candidatos 3, 4 y 5 de las escrituras: implementados, medidos y revisados por RDD; falta la revisión de Codex

Informe de cierre de la reutilización de recursos de escritura (2026-11-07). Enfoque de ejecución: ODD.
Fuente de producto: ninguna PRD (ningún cambio observable; anotación K13 de 2026-11-04). Documento de ejecución:
[write-reuse.md](write-reuse.md). Plan: [plan de rendimiento](../../docs/performance/plan.md), *Writes after O5–O7*.

Rama `perf/write-reuse`, que sale de `perf/writes-diagnosis` (`ca19201`); ninguna de las dos está fusionada en `main`.
Nada se ha subido.

## Resultado

- **Especificación:** S1–S5 entregados en T1–T5. Los bytes de los registros del diario y del archivo de datos no
  cambian: pasan los hashes dorados de O4 y los tests de bytes del commit (S4). Pasan `cargo fmt --all --check` y
  `cargo clippy --workspace --all-targets -- -D warnings`, y `cargo test --workspace` da 568 aprobados, 0 fallidos
  y 30 ignorados.
- **Aceptación:** cargas sueltas antes (`ca19201`) y después (`f63507f`) en Windows y WSL 2, en dos rondas
  alternadas, con la mediana de tres repeticiones medidas (tabla en L11 del documento de ejecución).
  - **Windows:** las diez cargas suben en las dos rondas. W2, W14 y W15 suben entre +23 % y +27 %, W21 un +16–17 %,
    W19 un +8 % y W17 un +54 % / +76 %.
  - **WSL 2:** W2, W15 y W19 suben entre +2 % y +4 %, y W17 un +1 % / +11 %. W1, W14 y W21 salen mixtas.
  - **W12 en WSL 2:** en el primer par bajó en las dos rondas (−13 % / −9,5 %). La repetí con los cinco binarios
    (antes, los tres commits intermedios y después) y salió mixta: −4,1 % / +2,6 %, sin ningún commit más bajo en las
    dos rondas. El mismo binario de partida dio entre 1,63 M y 1,75 M lecturas/s según la sesión.

  Ninguna carga queda más baja en las dos rondas tras la repetición (S5).
- **Revisiones y verificación:**
  - Tres revisiones RDD, todas aprobadas (sección siguiente).
  - Un verificador independiente para T1 (ítem de riesgo 4, concurrencia): S1 y S4 cumplen y no hay bloqueos (L10).
  - La revisión de Codex no se pudo hacer: la cuenta llegó a su límite de uso hasta el 2026-11-11 a las 06:49
    (sección *Revisión de Codex*).
- **Hallazgos corregidos por el camino:**
  - La revisión RDD de T1 encontró que los archivos retirados dependían del aviso del `enqueue` y que `close` los
    cerraba después del último grupo. Lo corrigió `95f3ff0`, y su test comprueba ahora también el caso positivo.
  - Al implementar T4 fallaron tres tests de `runs.rs`, porque los archivos de `krummi.temp` cuentan contra el tamaño
    máximo. Desde entonces los trozos propios se guardan solo en almacenes sin máximo.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `ca19201..f4e0e8a` | resilience, reliability | aprobada | R4-001 y R3-retired-wakeup-dependency (warning): el despertar de los archivos retirados; corregido en `95f3ff0`. R4-002 (suggestion): `close` retenía los retirados; corregido en `95f3ff0`. R3-closing-test-only-negative (warning): corregido en `95f3ff0` | `review-7d830f865fc9696e` |
| `f4e0e8a..f63507f` | risk, resilience, reliability | aprobada | R3-padding-tail (warning): descartado con evidencia (`journal.rs:276`). R1-001 (suggestion): descartado. R4-001 (suggestion): retención de memoria, aceptada como coste y anotada en el plan. R3-queue-capacity (suggestion): aceptado, acotado por la retención | `review-e90cf5ae517e6501` |
| `f63507f..8821a1b` | readability | aprobada | ninguno | `review-b5e2ef6e35719da6` |

Las valoraciones de `33a10d0`, `20ceb1e` y `f63507f` dieron *medium, under budget*; los tres se revisaron juntos en la
segunda porción. `2434050` (solo documentación, posterior a la última revisión) queda sin revisar.

## Revisión de Codex

No se hizo. La ronda 1 (`-Range ca19201..2434050`) terminó con `status: error` sin dejar informe: «You've hit your
usage limit … try again at Nov 11th, 2026 6:49 AM». El hilo es `01a1b65f-b36d-77f0-8286-ae6075f0e711`. La telemetría
registra como coordinador `gpt-6.1-sol` (high) y como revisor `gpt-6-astra` (xhigh); el revisor no llegó a terminar.

## Tiempo

- Estimación: entre 4 h y 7 h. Arranque real: 2026-11-07 11:28:14 UTC. Fin: 2026-11-07 14:26:20 UTC.
- **Duración activa desde el arranque: 2 h 58 min**, por debajo de la estimación (−26 % sobre el mínimo). La mayor
  parte fueron las mediciones de T5 (1 h 59 min en segundo plano). La revisión de Codex no llegó a correr; con sus
  rondas, la cifra habría sido mayor.
- Esperas por el usuario: ninguna.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 2 min 48 s | 2 % | 1 |
| Código y tests | 2 h 46 min | 93 % | 3 |
| Checks (fmt, clippy, suite de tests) | 3 min 36 s | 2 % | 1 |
| Documentación | 5 min 42 s | 3 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **2 h 58 min** | **100 %** | **7** |

- Inicio: 2026-11-07 11:28:14 +00:00. Fin: 2026-11-07 14:26:20 +00:00.
- Arranque confirmado: 11:28:14. Activo desde el arranque: **2 h 58 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | rdd T1 | 11:37:47 | 11:38:33 | 0 min 46 s |
| Revisión RDD | rdd T2-T4 | 12:05:41 | 12:07:40 | 1 min 59 s |
| Aceptación | T5 windows | 12:15:44 | 12:49:59 | 34 min 15 s |
| Aceptación | T5 wsl | 12:49:59 | 13:49:14 | 59 min 15 s |
| Aceptación | T5 wsl round 1 rerun | 13:49:52 | 13:49:57 | 0 min 05 s |
| Aceptación | T5 wsl round 1 rerun | 13:50:11 | 14:05:26 | 15 min 15 s |
| Aceptación | T5 wsl W12 per commit | 14:06:11 | 14:16:25 | 10 min 14 s |
| Revisión RDD | rdd T5-T6 docs | 14:17:18 | 14:17:45 | 0 min 27 s |
| Revisión RDD | codex-R1 | 14:18:00 | 14:22:33 | 4 min 33 s |
| **Aceptación (total)** | | | | **1 h 59 min** |
| **Revisión RDD (total)** | | | | **7 min 45 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `f4e0e8a` | perf(journal): close the old journal file on the journal thread | T1 | 9 min 14 s | Exploración y mapa 2 min 48 s, Código y tests 6 min 26 s |
| `95f3ff0` | fix(journal): wake the writer from the roll and close retired files before the last group | T1, T2 | 1 min 50 s | Código y tests 1 min 50 s |
| `33a10d0` | perf(journal): build records in reused aligned buffers and keep up to 32 MiB | T2 | 5 min 02 s | Código y tests 5 min 02 s |
| `20ceb1e` | perf(tx): reuse the page payloads of earlier commits for page copies | T3 | 6 min 41 s | Código y tests 6 min 41 s |
| `f63507f` | perf(temp): keep released own chunks for the next value runs of their size | T3 | 14 min 20 s | Código y tests 14 min 20 s |
| `233fcdc` | docs(odd): record T1-T4 of the write reuse run | T3 | 10 min 22 s | Código y tests 10 min 22 s |
| `8821a1b` | docs(perf): record the write reuse measurements and close candidates 3, 4 and 5 | T3 | 2 h 01 min | Código y tests 2 h 01 min |
| `2434050` | docs(odd): record the last RDD review of the write reuse run | T6 | 0 min 53 s | Documentación 0 min 53 s |
| — | después del último commit | T6 | 8 min 25 s | Checks (fmt, clippy, suite de tests) 3 min 36 s, Documentación 4 min 49 s |
| **Total** | | | **2 h 58 min** | |

Lectura de la tabla:

- El registro de fases no se marcó al pasar de T4 a T5, así que la fase «Código y tests» de `8821a1b` (2 h 01 min)
  incluye la espera a las mediciones en segundo plano y el análisis de W12. El tiempo de código propiamente dicho son
  unos 37 minutos (de `f4e0e8a` a `f63507f`).
- En la columna de etiquetas, T3 se arrastra hasta `8821a1b`, y las revisiones aparecen como *Revisión RDD*.
- La fila `codex-R1` es la llamada fallida a Codex.

## Informe de cambios (`ca19201..2434050`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi-pager` | 438 | 98 | 331 | 89 | 6 |
| `odd` | 147 | 0 | — | — | 2 |
| `crates/krummi-tx` | 81 | 2 | 63 | 11 | 3 |
| `docs` | 6 | 2 | — | — | 1 |
| **Total** | **672** | **102** | **394** | **100** | **12** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Las cifras no incluyen este informe ni su registro de fases final.

## Resumen

Los commits que escriben hacen menos trabajo de sistema por confirmación, sin cambiar nada de lo que se guarda:

- El hilo del diario cierra el archivo de diario viejo; antes lo cerraba el hilo que confirma, y en Windows ese
  cierre es caro.
- Los registros se comprimen directamente en búferes alineados que el escritor devuelve cuando son durables.
- Las páginas que se copian para escribir reutilizan las cargas de páginas de confirmaciones anteriores, en lugar de
  reservar memoria nueva.
- Los trozos propios del área temporal que se liberan sirven a la siguiente serie de valores de su tamaño, sin crear
  ni borrar un archivo cada vez.

Cada reserva está acotada a 32 MiB. Windows es la plataforma que más gana: hasta +27 % en las cargas de escritura al
azar y +54–76 % en W17.

## Decisiones tomadas sin preguntar

- T1 mueve solo el cierre del archivo viejo. El siguiente archivo se sigue creando en el hilo que confirma: crearlo
  antes lo haría aparecer en la carpeta y en la recuperación antes de que haga falta (L3).
- La retención es de 32 MiB por reserva, frente a los 8 MiB de antes (L3).
- T4 reutiliza solo trozos del mismo tamaño exacto, como máximo 32 MiB, y solo en almacenes sin tamaño máximo (L3,
  L8).
- El primer par de W12 en WSL 2 se tomó como ruido tras repetirlo por commit (L11).

## Pendiente (no bloquea)

- **Revisión de Codex del rango `ca19201..2434050`:** pendiente por el límite de uso, que se libera el 2026-11-11 a las
  06:49. Es una exigencia de CLAUDE.md, así que la rama no debería fusionarse sin ella; aun así no bloquea medir ni
  revisar a mano.
- **Coste de memoria (R4-001):** un almacén en disco puede quedarse hasta con 32 MiB por reserva, frente a unos 16 MiB
  de antes. Está anotado en el plan.
- **Creación del siguiente archivo del diario:** sigue en el hilo que confirma, entre un 1 % y un 2 % en Windows.
- **Ejecuciones completas en Windows y Linux:** el plan cierra cada paso con ellas, y para esta rama todavía no se han
  hecho.
- **Commit `2434050`:** solo documentación, quedó después de la última revisión RDD.

**Riesgo:** ítem 4 (concurrencia: el hilo del diario cierra archivos y devuelve búferes a una reserva compartida),
verificado por el verificador independiente y por RDD.

## Limpieza manual

- `work/wr` (893 MB, ignorada por git): binarios, almacenes y salidas de las mediciones.
- `~/wr` en WSL Ubuntu (842 MB): fuentes, binarios y almacenes de las mediciones.
