# O4, menos búferes puestos a cero en el commit: implementada y verificada; sus cargas objetivo no cumplen el umbral

Informe de cierre de `commit-copies` (2026-11-06). Enfoque de ejecución: ODD, sin PRD (anotación del 2026-11-04 a
K13). Fuente: O4 del [plan de rendimiento](../../docs/performance/plan.md). Documento de ejecución:
[commit-copies.md](commit-copies.md).

Rama `perf/commit-copies`, que parte de `perf/integrity-once` (`13489d1`), sin fusionar en `main` ni subir.

## Resultado

- **Especificación:** S1 y S2 cumplidos. Los registros del journal y los búferes de grupo se construyen copiando sus
  partes en memoria alineada sin inicializar y poniendo a cero solo el relleno (`AlignedBuf::from_parts`). El búfer
  de la carga sin comprimir y el de salida de LZ4 se conservan entre commits, y la salida solo se pone a cero al
  crecer; los dos se liberan tras un commit de más de 8 MiB. Los bytes de los registros no cambian: cinco hashes xxh3
  de registros completos, grabados con el código anterior, siguen pasando. `cargo test --workspace`: 546 pasadas, 0
  fallidas, 30 ignoradas.
- **Cargas sueltas (S3, S4):** antes `13489d1` y después `cf61713`, dos rondas con el orden cambiado (L7, L11).
  Windows: W21 +17 % a +20 %, W14 +9 % a +10 %, W13 +2 % a +11 %, W2 +3 % a +9 %, W19 +5 %; el resto, dentro del ruido.
  WSL 2: W13 +6 % / +7 %; el resto, dentro del ruido. Las bajadas sueltas (W17 en Windows, W4 y W15 en WSL) no se
  repiten en las rondas siguientes.
- **Ejecuciones completas** tras O3 y O4 (`2fdf8e9`, L10): [Windows](../../docs/benchmark/2026-11-06-windows.md) y
  [Linux](../../docs/benchmark/2026-11-06-linux.md), 10 de 22 umbrales en ambas (antes 6 y 5). **Ninguna de las nueve
  cargas objetivo de O4 cumple su umbral** (Krummi / Voron de 0,24 a 0,53 en Windows y de 0,31 a 0,76 en Linux), y
  es lo que la anotación de K13 fija como aceptación: R1 de Codex, pendiente de tu decisión. Las cargas cuyo número de
  Krummi bajó respecto al 2026-11-05 no bajan medidas sueltas contra `3ec8641`.

## RDD reviews

| Tramo | Lentes | Resultado | Hallazgos | Linaje |
|---|---|---|---|---|
| `c5062a4..ba2a3ae` (último arreglo de O3 y O4) | risk, resilience, reliability | approved | R4-001 warning: hasta ~16 MiB retenidos por almacén, aceptado; R3 warning: la lista de partes del grupo se reservaba sin control de fallo, corregido en `d95da27`; dos avisos de O3 sin cambios | `review-8c24a99484b62d18` |
| `ba2a3ae..4e1e38d` | resilience | approved | ninguno | `review-998d9967b748de57` |

Los commits posteriores a `4e1e38d` son solo documentación e informes.

## Codex reviews

- Ronda 1 ([informe](../../docs/reviews/commit-copies/2026-11-06-code.md)): R1 blocker: las cargas objetivo no
  cumplen el umbral de K13, y faltaban las cargas sueltas W1, W3, W13, W15 y W19. Ningún defecto en el código. Un
  intento anterior, a las 11:36 UTC, falló por el límite de uso de Codex.
- Ronda 2 ([informe](../../docs/reviews/commit-copies/2026-11-06-code-02.md)): las mediciones que faltaban,
  verificadas en Windows; las de WSL se copiaron después a `work/o4/` para poder comprobarlas. Los umbrales siguen
  abiertos. El bucle se detuvo en dos rondas: una tercera solo podía repetir que falta tu decisión.
- [Resoluciones](../../docs/reviews/commit-copies/2026-11-06-code-resolutions.md).

## Time

- Estimación: 7 h – 11 h para O3, O4 y las ejecuciones completas juntas. Inicio: 2026-11-06 07:02:41 UTC. Fin:
  17:37:10 UTC. **Total: 10 h 34 min de reloj, dentro de la estimación**, sin esperas por el usuario. O3 tomó 2 h
  20 min activas ([su informe](integrity-once-report.md)); los 1 min 23 s entre el fin de O3 (09:23:17) y el inicio
  del registro de O4 no está en ningún registro de fases.
- El registro de fases de O4 empieza a las 09:24:40: el mapa de O4 lo hizo un explorador durante la revisión de
  Codex de O3 (L2), dentro del tiempo de O3.
- Lo que más tiempo llevó fue la aceptación: 7 h 25 min de mediciones en segundo plano. Las ejecuciones completas
  sumaron 3 h 47 min. El resto fueron las rondas extra para separar el ruido de las bajadas (W17, W4, W15) y las
  cargas sueltas que pidió Codex.

### Time by phase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 0 min 20 s | 0 % | 1 |
| Código y tests | 2 min 01 s | 0 % | 1 |
| Checks (fmt, clippy, suite de tests) | 5 min 23 s | 1 % | 1 |
| Aceptación | 6 h 25 min | 78 % | 2 |
| Revisión RDD | 16 min 16 s | 3 % | 3 |
| Correcciones de hallazgos | 1 h 22 min | 17 % | 2 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **8 h 12 min** | **100 %** | **11** |

- Inicio: 2026-11-06 09:24:40 +00:00. Fin: 2026-11-06 17:37:10 +00:00.
- Sin marca de arranque: todo el activo cuenta desde el inicio.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | T3 windows | 09:38:01 | 09:57:03 | 19 min 02 s |
| Aceptación | T3 wsl | 09:57:03 | 11:02:05 | 1 h 05 min |
| Aceptación | T3 windows extra | 11:03:24 | 11:04:10 | 0 min 46 s |
| Aceptación | T3 wsl extra | 11:04:10 | 11:28:12 | 24 min 02 s |
| Revisión RDD | codex-R1 | 11:36:34 | 11:37:27 | 0 min 53 s |
| Aceptación | complete windows | 11:38:31 | 13:06:45 | 1 h 28 min |
| Aceptación | complete wsl | 13:06:45 | 15:26:34 | 2 h 19 min |
| Aceptación | full check windows | 15:31:37 | 15:33:14 | 1 min 37 s |
| Aceptación | full check wsl | 15:33:14 | 16:06:40 | 33 min 26 s |
| Revisión RDD | codex-R1 | 16:07:52 | 16:17:25 | 9 min 33 s |
| Aceptación | T3 rest windows | 16:17:56 | 17:01:13 | 43 min 17 s |
| Aceptación | T3 rest wsl | 17:01:13 | 17:26:05 | 24 min 52 s |
| Aceptación | T3 W13 windows | 17:26:37 | 17:29:26 | 2 min 49 s |
| Aceptación | T3 W13 wsl | 17:29:26 | 17:32:19 | 2 min 53 s |
| Revisión RDD | codex-R2 | 17:32:49 | 17:36:40 | 3 min 51 s |
| **Aceptación (total)** | | | | **7 h 25 min** |
| **Revisión RDD (total)** | | | | **14 min 17 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `489bd4e` | test(pager): pin journal records to the bytes of the stored format | T0, T1 | 0 min 50 s | Exploración y mapa 0 min 20 s, Código y tests 0 min 30 s |
| `cf61713` | perf(pager): build journal records and group writes without zeroing what they overwrite | T1, T2 | 6 min 42 s | Código y tests 1 min 31 s, Checks (fmt, clippy, suite de tests) 5 min 11 s |
| `8d4f375` | docs: record the O4 implementation |  | 0 min 00 s |  |
| `ba2a3ae` | docs: record the O4 single-load runs | T2, T3 | 1 h 56 min | Checks (fmt, clippy, suite de tests) 0 min 12 s, Aceptación 1 h 56 min |
| `d95da27` | fix(pager): allocate the list of group parts fallibly | T3, T4 | 7 min 56 s | Aceptación 0 min 05 s, Revisión RDD 1 min 46 s, Correcciones de hallazgos 6 min 05 s |
| `4e1e38d` | docs: record the O4 RDD review |  | 0 min 00 s |  |
| `2fdf8e9` | docs: record the review of the last O4 slice | T4 | 1 min 57 s | Correcciones de hallazgos 1 min 57 s |
| `47ce11c` | docs(bench): add the complete reports after O3 and O4 | T4, benchmark | 4 h 29 min | Aceptación 4 h 29 min, Correcciones de hallazgos 0 min 07 s |
| `f1acc21` | docs: record the O4 Codex review and the remaining single-load runs | benchmark, T4, T3 | 1 h 24 min | Aceptación 0 min 05 s, Revisión RDD 10 min 05 s, Correcciones de hallazgos 1 h 14 min |
| `7dd119f` | docs: record the O4 Codex follow-up review | T3, T4 | 4 min 18 s | Revisión RDD 4 min 17 s, Correcciones de hallazgos 0 min 01 s |
| — | después del último commit | T4 | 0 min 08 s | Revisión RDD 0 min 08 s |
| **Total** | | | **8 h 12 min** | |

## Change report (`13489d1..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 358 | 0 | — | — | 6 |
| `crates/krummi-pager` | 203 | 30 | 162 | 32 | 2 |
| `odd` | 179 | 5 | — | — | 4 |
| **Total** | **740** | **35** | **162** | **32** | **12** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

## Summary

El commit deja de poner a cero la memoria que después sobrescribe: los registros del journal y los búferes de grupo
se construyen copiando sus partes en memoria sin inicializar, y la compresión reutiliza sus dos búferes entre
commits. Los bytes del journal no cambian, como fijan los hashes grabados antes. Ganan, sobre todo en Windows, las
cargas con más páginas por commit (W21 y W14). En WSL 2 las escrituras quedan como estaban. Ninguna carga objetivo
alcanza todavía 0,80 veces Voron.

## Defaults chosen without asking

- Alcance de O4 (L3): sin sellar páginas en su sitio, sin quitar la copia de un grupo de un solo registro y sin
  escrituras vectoriales; son cambios mayores, para un paso posterior.
- Las ejecuciones completas antes que la revisión de Codex, mientras su cuota estaba agotada (L9).
- Hasta 8 MiB por búfer retenido, la misma cota que la cola del journal (L8).
- W13 medida como W12, porque se ejecuta en el mismo proceso (L11).

## Pending (non-blocking)

- **Decisión de producto, R1 de Codex (blocker):** la anotación de K13 acepta una optimización sin PRD solo si sus
  cargas objetivo cumplen su umbral, y ninguna de las de O4 lo cumple. O1, O2 y O3a se cerraron como pasos sin
  umbrales cumplidos, pero K13 no lo prevé.
- La reserva de memoria fallida de la lista de partes y del búfer de grupo no se puede provocar en una prueba (no hay
  punto de fallo para ella): el camino de un registro por escritura se revisó, pero no se ejecutó.
- W12, una carga de lectura que no es objetivo de O4, da 0,96 / 0,97 en Windows, medida en el mismo proceso que W13.

**Risk:** item 1 (bytes del journal), cubierto por los hashes grabados antes del cambio; sin verificador
independiente, porque los bytes no cambian y las pruebas lo fijan.

## Manual cleanup

- `work/o4/` (binarios, almacenes, salidas) y `work/complete/` (`*-o4.*`, `o4.tar`); en WSL, `~/o4`, `~/o4full`,
  `~/krummi-o4-src`, `~/krummi-o4-target`, `~/bench-work-o4` y `~/bench-complete-o4.*`.
