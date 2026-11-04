# O1 terminada: los lectores leen el tamaño y el mapeo del pager sin cerrojo; W9 +31 % en Windows y Linux

Informe de cierre de `pager-mapping` (2026-11-04). Enfoque de ejecución: ODD, sin PRD (anotación de K13 del
2026-11-04: una optimización sin cambio observable). Fuente: O1 de [plan.md](../../docs/performance/plan.md).
Documento de ejecución: [pager-mapping.md](pager-mapping.md).

Rama `perf/pager-mapping` desde `main` `7435b05`, sin merge ni push: queda para que la revises.

## Resultado

- **Especificación:** S1 y S2 entregados en `9acf695` y `60dcdc8`: `len` y `raw_pages` ya no toman el mutex; `grow`
  publica el mapeo nuevo y después el tamaño, y un lector lee el tamaño y después el mapeo, así que nunca ve un
  tamaño mayor que el que cubre su mapeo. Prueba nueva `readers_see_every_page_while_the_store_grows`. Checks:
  `cargo fmt --all --check` correcto; `cargo clippy --workspace --all-targets -- -D warnings` limpio;
  `cargo test --workspace` 520 pasados, 0 fallidos, 30 ignorados.
- **Medición (S3, S4):** ejecuciones de una carga, antes y después, en Windows y en WSL 2 (L5, L7):

  | Carga | Windows | WSL 2 |
  |---|---|---|
  | W7 | 435.691 → 509.832 (+17 %) | 356.734 → 500.475 (+40 %) |
  | W8 | 977.970 → 977.961 (0 %) | 907.862 → 1.095.073 (+21 %) |
  | W9 | 359.611 → 469.375 (+31 %) | 372.642 → 490.028 (+31 %) |
  | W10 | 11.841.551 → 17.632.828 (+49 %) | 8.893.205 → 19.215.754 (+116 %) |
  | W11 | 11.154.322 → 13.283.250 (+19 %) | 8.247.194 → 13.149.634 (+59 %) |
  | W12 | 483.333 → 509.161 (+5 %) | 495.771 → 712.318 (+44 %) |
  | W13 | 18.226 → 15.577 (−15 %, ruido) | 6.313 → 7.675 (+22 %) |

  W13 en Windows se repitió dos veces más alternando los binarios: sus repeticiones van de 7.671 a 19.193 dentro de
  una misma ejecución, y las medianas de antes y de después se solapan. Ningún umbral incumplido se cumple todavía:
  W9 sigue por debajo de W8, porque quedan el cerrojo de scratch (O2) y el checksum (O3). Las ejecuciones completas
  van una sola vez después de O1, O2 y O3a (L2).
- **Revisiones y verificación:** verificador independiente sin bloqueos (L8); RDD en tres revisiones, todas
  aprobadas; Codex aprobado en la primera ronda, sin hallazgos (L10). Checks finales en `8a4a8d3`: fmt correcto,
  clippy limpio, 520 pasados, 0 fallidos, 30 ignorados.
- **Hallazgos corregidos por el camino:** la primera versión de la prueba no detectaba la mutación (publicar el
  tamaño antes que el mapeo), porque Windows colocó la vista nueva justo detrás de la vieja: ahora comprueba también
  la primera página de la mitad nueva. RDD: la prueba se colgaba si fallaba una ampliación (`cade256`).

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `7435b05..60dcdc8` (código) | riesgo, fiabilidad, resiliencia | aprobado | Aviso: la prueba se cuelga si falla una ampliación, corregido con un guardia; sugerencia: lectores después de las ampliaciones, corregido con una barrera; sugerencia de `debug_assert`: se mantiene fuera, repetiría la comprobación de tamaño | `review-eaaa09d7e0d22d6e` |
| `60dcdc8..cade256` | fiabilidad | aprobado | Sugerencia: el guardia separaba la prueba de su comentario, corregido en `871cb5c` | `review-d04f3545b1bb4c40` |

| `cade256..8a4a8d3` (tramo final: comentario de la prueba y documentos) | fiabilidad | aprobado | Ninguno | `review-339000624d332a31` |

## Revisión cruzada de Codex

Una ronda sobre `7435b05..7488132`: **aprobado, sin hallazgos** ("Approved — no findings in the requested scope").
Informe: [2026-11-04-code.md](../../docs/reviews/pager-mapping/2026-11-04-code.md); resoluciones:
[2026-11-04-code-resolutions.md](../../docs/reviews/pager-mapping/2026-11-04-code-resolutions.md). Coordinador
`gpt-6.1-sol` / `high`, revisor `krummi-review-critical` `gpt-6-astra` / `xhigh`. La primera ejecución (13:43 UTC) se
paró por el límite de uso de Codex sin revisar nada; se relanzó a las 16:51 UTC.

## Tiempo

Estimación: 2 h 30 min – 4 h. Real: **1 h 06 min** activos desde el arranque confirmado, el 44 % del mínimo. Lo que
más tiempo llevó: las revisiones RDD y la de Codex (25 %), el código con su prueba (20 %) y los checks (19 %). La
espera de 3 h 02 min fue el límite de uso de Codex, no una espera por vos; el registro la anota como espera, así que
no cuenta en el activo.

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 5 min 13 s | 7 % | 1 |
| Plan (documento de ejecución) | 1 min 25 s | 2 % | 1 |
| Código y tests | 14 min 17 s | 20 % | 1 |
| Checks (fmt, clippy, suite de tests) | 13 min 48 s | 19 % | 3 |
| Aceptación | 10 min 34 s | 15 % | 2 |
| Verificación independiente | 8 min 18 s | 12 % | 1 |
| Revisión RDD | 17 min 46 s | 25 % | 3 |
| **Total activo** | **1 h 11 min** | **100 %** | **12** |

- Inicio: 2026-11-04 12:50:46 +00:00. Fin: 2026-11-04 17:04:54 +00:00.
- Arranque confirmado: 12:56:24. Activo desde el arranque: **1 h 06 min** (compárese con la estimación); antes del arranque: 5 min 13 s.
- Esperas por el usuario (fuera del activo): 3 h 02 min.
  - 12:55:59–12:56:24 (0 min 25 s): estimate and start of O1
  - 13:48:58–16:51:20 (3 h 02 min): Codex usage limit until 16:50 UTC

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | T1 baseline windows | 12:58:01 | 12:58:02 | 0 min 01 s |
| Aceptación | T1 baseline wsl | 12:58:02 | 13:00:20 | 2 min 18 s |
| Aceptación | T1 baseline windows | 13:01:06 | 13:01:07 | 0 min 01 s |
| Aceptación | T1 baseline wsl | 13:01:07 | 13:06:32 | 5 min 25 s |
| Aceptación | T1 baseline windows | 13:06:59 | 13:11:55 | 4 min 56 s |
| Aceptación | T3 after windows | 13:21:15 | 13:24:32 | 3 min 17 s |
| Aceptación | T3 after wsl | 13:24:33 | 13:28:36 | 4 min 03 s |
| Aceptación | T3 W12 rerun windows | 13:29:37 | 13:32:03 | 2 min 26 s |
| Revisión RDD | codex-R1 | 13:43:42 | 13:48:43 | 5 min 01 s |
| Revisión RDD | codex-R1 | 16:51:20 | 16:58:07 | 6 min 47 s |
| **Aceptación (total)** | | | | **22 min 27 s** |
| **Revisión RDD (total)** | | | | **11 min 48 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `9acf695` | perf(pager): read the store's size and newest mapping without a lock | T2 | 29 min 28 s | Exploración y mapa 5 min 13 s, Plan (documento de ejecución) 1 min 25 s, Código y tests 14 min 17 s, Checks (fmt, clippy, suite de tests) 8 min 33 s |
| `60dcdc8` | refactor(pager): publish a new mapping's start after keeping it | T2, T3 | 8 min 45 s | Checks (fmt, clippy, suite de tests) 0 min 59 s, Aceptación 7 min 46 s |
| `8d46ce5` | docs: record the O1 measurements | T2, T3 | 3 min 01 s | Checks (fmt, clippy, suite de tests) 0 min 13 s, Aceptación 2 min 48 s |
| `97da17b` | docs: record the O1 verification | T4 | 8 min 19 s | Verificación independiente 8 min 18 s, Revisión RDD 0 min 01 s |
| `c75f3b6` | docs: add the O1 phase log | T4 | 0 min 09 s | Revisión RDD 0 min 09 s |
| `cade256` | test(pager): start the growth readers first and stop them when a growth fails | T4 | 1 min 45 s | Revisión RDD 1 min 45 s |
| `871cb5c` | test(pager): keep the growth test's comment next to the test | T4 | 0 min 43 s | Revisión RDD 0 min 43 s |
| `7488132` | docs: record the O1 RDD reviews | T4 | 0 min 15 s | Revisión RDD 0 min 15 s |
| `8a4a8d3` | docs: record the O1 Codex review and plan history | T4 | 12 min 50 s | Revisión RDD 12 min 50 s |
| — | después del último commit | T4 | 6 min 06 s | Checks (fmt, clippy, suite de tests) 4 min 03 s, Revisión RDD 2 min 03 s |
| **Total** | | | **1 h 11 min** | |

## Informe de cambios (`7435b05..8a4a8d3`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `odd` | 136 | 0 | — | — | 2 |
| `crates/krummi-pager` | 94 | 41 | 66 | 25 | 2 |
| `docs` | 91 | 0 | — | — | 3 |
| **Total** | **321** | **41** | **66** | **25** | **7** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

## Resumen

Cada lectura de una página consultaba el tamaño del almacén y su último mapeo bajo un mutex de todo el proceso; con
16 hilos, la mitad del tiempo se iba en esperar ese mutex. Ahora el tamaño y el comienzo del último mapeo se publican
como atómicos: el escritor, al crecer, publica primero el mapeo y después el tamaño, y los mapeos viejos siguen vivos
hasta cerrar, como antes. Las lecturas no cambian de resultado; solo se quita la espera.

## Valores tomados sin preguntar

- Aceptación por medición de cada carga, con las ejecuciones completas al final de O1, O2 y O3a (me lo dejaste a mí,
  L2).
- Los almacenes temporales (`Storage::Memory`) mantienen su mutex: no están en el camino del banco.

## Pendiente (no bloquea)

- **Umbrales:** O1 no cumple ninguno por sí sola; siguen O2, O3a y el PRD de O3.
- **La prueba de concurrencia depende de dónde coloca el sistema los mapeos:** detecta la mutación en Windows en cada
  ejecución, pero un orden equivocado que dejara una disposición inocua podría pasar; loom y Miri no modelan un mapeo.

**Riesgo:** ítem 4 (concurrencia: el orden de publicación del tamaño y el mapeo entre el escritor y los lectores).

## Limpieza manual

- `work/o1` (ignorado por git): los dos binarios y el almacén base de Windows, 267 MB.
- En WSL: `~/o1` (270 MB) y `~/krummi-o1-src` (7 MB). `~/krummi-wsl-target` (930 MB) es la carpeta de compilación
  de WSL; sirve para O2 y O3a, así que conviene dejarla hasta el final.
