# O2 terminada: los lectores sin versiones en scratch no toman su cerrojo; W9 +26 % a +131 %

Informe de cierre de `scratch-lock` (2026-11-04). Enfoque de ejecución: ODD, sin PRD (anotación de K13 del
2026-11-04: una optimización sin cambio observable). Fuente: O2 de [plan.md](../../docs/performance/plan.md).
Documento de ejecución: [scratch-lock.md](scratch-lock.md).

Rama `perf/scratch-lock` desde `main` `b3c0c44`, sin merge ni push: queda para que la revises.

## Resultado

- **Especificación:** S1 entregado en `387e758`. Un snapshot anota al abrirse, bajo el cerrojo, si el área de
  scratch tiene versiones; si no tiene ninguna, `page_and_code` y `run` leen el almacén sin tomar el cerrojo ni buscar
  en `versions`. Es correcto porque una versión publicada después tiene un `txid` mayor que el del snapshot y nunca
  le aplica. S2: W12 y el snapshot del escritor siguen por el camino de siempre cuando hay versiones al abrir.
  Pruebas nuevas: `a_snapshot_opened_without_versions_keeps_its_page_while_newer_ones_are_written` (`55314ce`) y
  `the_writers_snapshot_does_not_see_what_it_publishes` (`f35f3a9`). Checks finales en `f35f3a9`, el último cambio
  de código: `cargo fmt --all --check` correcto; `cargo clippy --workspace --all-targets -- -D warnings` limpio;
  `cargo test --workspace` 522 pasados, 0 fallidos, 30 ignorados.
- **Medición (S3):** W9, la carga de 16 hilos, mejora en todas las parejas alternadas (L6, L8):

  | Carga | Windows (por pareja) | WSL 2 (por pareja) |
  |---|---|---|
  | W9 | +26 %, +66 %, +46 %, +28 %, +58 % | +131 %, +63 % |
  | W7, W8 | +3 %, +4 % (ruido) | ruido |
  | W10, W11, W12 | dentro del ruido | dentro del ruido |

  Ningún umbral se cumple todavía: queda el checksum (O3a, O3).
- **Hallazgo de medición:** en Windows, una carga que corre después de W9 bajo el mismo proceso padre va mucho más
  lenta (W10 de unos 20 M a unos 2,4 M op/s), con cualquiera de los dos binarios (L7). Cruzando los binarios lo
  confirmé: W9 nuevo seguido de W10 viejo dio 2.316.164, y W9 viejo seguido de W10 nuevo, 20.828.040. Ahora W9 satura
  los 16 hilos porque ya no espera el cerrojo, y la máquina frena lo que viene detrás (probablemente el *power
  throttling* de Windows o el reparto a los núcleos E; no está probado). La primera comparación parecía una
  regresión de W10 y W11, y no lo era.
- **Revisiones y verificación:** verificador independiente sin bloqueos (L9); RDD en cuatro revisiones, todas
  aprobadas (L10, L11, L13, L14); Codex aprobado en la primera ronda, sin hallazgos (L12).

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `b3c0c44..2bfeba8` (código) | riesgo, fiabilidad, resiliencia | aprobado | Dos avisos (R1, R3) con la misma duda: el snapshot del escritor también decide al abrir. Es correcto (su `txid` es `last_committed` y publica con `last_committed + 1`); fijado con la prueba de `f35f3a9`, que falla al mutar ese `txid` | `review-d4fe1be3e1ef5a12` |
| `2bfeba8..f35f3a9` | fiabilidad | aprobado | Ninguno | `review-e238fa558daf5d7c` |
| `0fdc957..4c0a2aa` (documentos) | legibilidad | aprobado | Aviso: las resoluciones daban como hecho el modelo de Codex que el informe no pudo ver; corregido en `312ecdf`, también en las de O1 | `review-bb42c9e9e1c418f7` |
| `4c0a2aa..312ecdf` | legibilidad | aprobado | Sugerencia: nombrar los dos archivos en L13; aplicada con este informe | `review-b3eb0eed8c3c4597` |

## Revisión cruzada de Codex

Una ronda sobre `b3c0c44..0fdc957`: **aprobado, sin hallazgos** ("Approved — no findings in the requested scope").
Informe: [2026-11-04-code.md](../../docs/reviews/scratch-lock/2026-11-04-code.md); resoluciones:
[2026-11-04-code-resolutions.md](../../docs/reviews/scratch-lock/2026-11-04-code-resolutions.md).

## Tiempo

Estimación: 1 h 15 min – 2 h. Real: **2 h 37 min** activos desde el arranque confirmado, un 31 % por encima del
máximo. La desviación vino de la medición: la aparente regresión de W10 y W11 en Windows llevó casi una hora de
experimentos (anotados como correcciones, 58 min) y 52 min de tandas en segundo plano, hasta probar que era la
máquina. El código en sí fueron 3 min.

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 0 min 50 s | 1 % | 1 |
| Plan (documento de ejecución) | 11 min 46 s | 7 % | 1 |
| Código y tests | 2 min 51 s | 2 % | 2 |
| Checks (fmt, clippy, suite de tests) | 20 min 17 s | 13 % | 2 |
| Aceptación | 30 min 52 s | 20 % | 1 |
| Verificación independiente | 23 min 44 s | 15 % | 1 |
| Revisión RDD | 8 min 11 s | 5 % | 2 |
| Correcciones de hallazgos | 58 min 43 s | 37 % | 3 |
| Documentación | 0 min 48 s | 1 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **2 h 38 min** | **100 %** | **15** |

- Inicio: 2026-11-04 18:41:58 +00:00. Fin: 2026-11-04 21:22:45 +00:00.
- Arranque confirmado: 18:45:33. Activo desde el arranque: **2 h 37 min** (compárese con la estimación); antes del arranque: 0 min 50 s.
- Esperas por el usuario (fuera del activo): 2 min 45 s.
  - 18:42:48–18:45:33 (2 min 45 s): estimate and start of O2

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | T1 baseline windows | 18:47:45 | 18:51:24 | 3 min 39 s |
| Aceptación | T1 baseline wsl | 18:51:24 | 18:57:09 | 5 min 45 s |
| Aceptación | T3 after windows | 19:07:13 | 19:10:25 | 3 min 12 s |
| Aceptación | T3 after wsl | 19:10:25 | 19:13:47 | 3 min 22 s |
| Aceptación | T3 rerun wsl | 19:13:47 | 19:21:30 | 7 min 43 s |
| Aceptación | T3 rerun windows | 19:21:57 | 19:36:09 | 14 min 12 s |
| Aceptación | T3 isolated windows | 20:11:02 | 20:25:45 | 14 min 43 s |
| Revisión RDD | codex-R1 | 21:14:04 | 21:20:51 | 6 min 47 s |
| **Aceptación (total)** | | | | **52 min 36 s** |
| **Revisión RDD (total)** | | | | **6 min 47 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `387e758` | perf(pager): skip the scratch lock for snapshots that no version can reach | T1, T2 | 21 min 19 s | Exploración y mapa 0 min 50 s, Plan (documento de ejecución) 11 min 46 s, Código y tests 1 min 36 s, Checks (fmt, clippy, suite de tests) 7 min 07 s |
| `f37d700` | docs: record the O2 measurements | T2, T3 | 1 h 20 min | Checks (fmt, clippy, suite de tests) 0 min 15 s, Aceptación 30 min 52 s, Correcciones de hallazgos 49 min 18 s |
| `55314ce` | test(pager): pin a snapshot without versions while newer ones are written in place | T3, T4 | 38 min 03 s | Código y tests 1 min 15 s, Checks (fmt, clippy, suite de tests) 12 min 46 s, Verificación independiente 23 min 44 s, Correcciones de hallazgos 0 min 18 s |
| `2bfeba8` | docs: update the O2 phase log | T4 | 0 min 09 s | Checks (fmt, clippy, suite de tests) 0 min 09 s |
| `f35f3a9` | test(pager): pin that the writer's snapshot does not see what it publishes | T4 | 8 min 35 s | Revisión RDD 1 min 00 s, Correcciones de hallazgos 7 min 35 s |
| `0fdc957` | docs: record the O2 RDD reviews | T4 | 0 min 45 s | Correcciones de hallazgos 0 min 45 s |
| `4c0a2aa` | docs: record the O2 Codex review and plan history | T4 | 7 min 11 s | Revisión RDD 7 min 11 s |
| `312ecdf` | docs: say where the Codex telemetry in the review resolutions comes from | T4 | 0 min 48 s | Documentación 0 min 48 s |
| — | después del último commit | T4 | 0 min 47 s | Correcciones de hallazgos 0 min 47 s |
| **Total** | | | **2 h 38 min** | |

## Informe de cambios (`b3c0c44..312ecdf`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `odd` | 137 | 0 | — | — | 2 |
| `docs` | 97 | 1 | — | — | 4 |
| `crates/krummi-pager` | 63 | 4 | 51 | 8 | 2 |
| **Total** | **297** | **5** | **51** | **8** | **8** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

## Resumen

Cada lectura de una página tomaba también el cerrojo del área de scratch, para buscar si había una versión más nueva
que la del archivo. En las cargas de solo lectura esa búsqueda nunca encuentra nada, pero con 16 hilos el cerrojo los
ponía en fila. Ahora cada snapshot mira una sola vez, al abrirse, si hay versiones en scratch. Si no hay ninguna,
nunca va a haber una que le aplique, porque todo lo que se publique después es más nuevo que el snapshot, y lee
directo del archivo. Las lecturas devuelven lo mismo que antes; solo desaparece la espera.

## Valores tomados sin preguntar

- La primera de las dos opciones del plan (decidir sin el cerrojo si alguna versión puede aplicar), no un mapa sin
  cerrojo: W12 conserva su comportamiento, pero no gana con este cambio (L3).
- W18 y W22 no se midieron por separado, porque necesitan los almacenes de W17 y W21; quedan para las ejecuciones
  completas (L3).
- Comparación final con cada carga en su propia invocación del script de medición, por el efecto de la máquina (L7,
  L8).

## Pendiente (no bloquea)

- **Umbrales:** O2 no cumple ninguno por sí sola; siguen O3a y el PRD de O3, y después las ejecuciones completas.
- **Las ejecuciones completas pueden sufrir el mismo efecto:** en Windows, las cargas que corren después de W9 bajo
  el mismo orquestador pueden salir más lentas por la máquina, no por el motor. Conviene revisarlo antes de dar por
  buenas las ejecuciones completas después de O3a: por ejemplo, con una pausa o un orden distinto, o lanzando cada
  carga desde un proceso nuevo.
- **El snapshot del escritor no está en `live`:** una transferencia puede escribir en su lugar una página que ese
  snapshot lee. Ya pasaba antes de O2 (aviso del verificador, L9).

**Riesgo:** ítem 4 (concurrencia: qué lector puede saltarse el cerrojo del área de scratch).

## Limpieza manual

- `work/o2` (ignorado por git): los binarios y el almacén base de Windows, 273 MB.
- En WSL: `~/o2` (270 MB) y `~/krummi-o2-src` (7 MB). `~/krummi-wsl-target` sigue sirviendo para O3a.
