# Huecos baratos frente a Voron, comprobación recordada de versiones y diario de diferencias: implementados, medidos y revisados por RDD; falta la revisión de Codex

Informe de cierre de la tanda de huecos baratos y de los ítems 1 y 2 de la investigación (2026-11-08). Enfoque de
ejecución: ODD. Fuentes de producto: ninguna PRD para los huecos baratos (cambios internos y un arreglo del banco);
[pager-version-checks.prd.md](../../docs/prd/krummi/pager-version-checks.prd.md) y
[journal-page-diffs.prd.md](../../docs/prd/krummi/journal-page-diffs.prd.md) para los ítems 2 y 1. Documentos de
ejecución: [cheap-gaps.md](cheap-gaps.md), [version-checks.md](version-checks.md) y [page-diffs.md](page-diffs.md).
Plan: [plan de rendimiento](../../docs/performance/plan.md).

Rama `perf/cheap-gaps`, que sale de `perf/write-reuse` (`e0ad729`); no está fusionada en `main`. Nada se ha subido.

## Resultado

- **Especificación:**
  - Huecos baratos: S1–S7 de [cheap-gaps.md](cheap-gaps.md) en T1–T6. El ítem 7 (caché de la última hoja) se
    sustituyó por una comprobación de organización más rápida (L8), y el ítem 6 se quedó en marcar como temporales
    los archivos del área temporal (L5): las copias de más pasaron a O8.
  - Comprobación recordada de versiones: U1–U5 y AC-U1–AC-U8 en V1–V3.
  - Diario de diferencias y huecos en los nodos: Y1–Y7 y AC-Y1–AC-Y8 en P1–P3. Formato de almacén `10`.
  - Pasan `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test
    --workspace` da 589 aprobados, 0 fallidos y 31 ignorados (en `f295627`).
- **Aceptación:** cargas sueltas de cuatro binarios en rotación (`e0ad729`, `d96a92b`, `e08b646` y `7192dc7`) en
  Windows y WSL 2, con el plan de energía del banco y la mediana de tres repeticiones medidas. Cada carga más baja
  en las dos rondas se volvió a medir en dos rondas nuevas, y lo que volvió a bajar, en cinco rondas alternadas
  (tablas en L12 de [cheap-gaps.md](cheap-gaps.md), L6 de [version-checks.md](version-checks.md) y L8 de
  [page-diffs.md](page-diffs.md)).
  - **Huecos baratos:** W18 ×3,0 / ×2,9 en Windows y ×2,6 / ×2,4 en WSL 2 (la suma del banco); W1 +25–27 % en
    Windows y +5–14 % en WSL 2; W10 y W11 +15–24 % en las dos plataformas; W21 +3 % en Windows y mixta en WSL 2.
  - **Versiones:** W12 +65 % / +71 % en Windows y +32 % / +35 % en WSL 2.
  - **Diario de diferencias:** W2, W14, W15 y W19 ×2,1–2,5 en Windows y ×1,7–2,2 en WSL 2; W13 ×2,3–2,4 y ×1,4;
    W21 +43–45 % y +18–20 %.
  - Ninguna carga de las listas de los criterios queda más baja en las dos rondas tras repetirla, salvo W18 en
    Windows con el diario de diferencias. Sus cinco rondas alternadas no son concluyentes: pd sale más bajo en
    cuatro y más alto en una, con la mediana mayor, y los dos binarios caen al mismo modo lento. Dos costes
    confirmados fuera de ellas: W21 en Windows −1,5 % con las versiones (más baja en nueve de nueve rondas), y W3 en
    WSL 2 −3,1 % en la rama entera (más baja en cuatro de cinco rondas alternadas, sin ningún paso más bajo en las dos
    rondas).
  - **Ejecuciones completas**, con el mismo plan de energía que las del 2026-11-07:
    - **Windows** (`4a3a92e`): cumple 21 de 22 umbrales, antes 11. Solo falla W17 (0,63). Krummi / Voron en
      W2 pasa de 0,40 a 1,02, en W1 de 0,72 a 1,04, en W12 de 0,37 a 0,87, en W18 de 0,46 a 1,58 y en W21 de
      0,65 a 0,94.
    - **Linux** (`d679cf3`, el mismo código): cumple 21 de 22, antes 14. Solo falla W3: 0,82 → 0,78, con
      Krummi −3,1 %, el coste ya confirmado. W2 pasa de 0,48 a 1,03 y W18 de 0,51 a 1,62.
    - **Caídas de más de 0,05:** W10 en Linux (1,26 → 0,80), porque Voron sube un +88 % y Krummi un +19 %; y W16
      en Linux (Krummi −11 %), mixta en cinco rondas alternadas de cargas sueltas (medianas 224 y 222, con
      repeticiones de 165 a 353).
    - Informes: [Windows](../../docs/benchmark/2026-11-08-windows-cheap-gaps.md) y
      [Linux](../../docs/benchmark/2026-11-08-linux-cheap-gaps.md).
- **Revisiones y verificación:**
  - Cinco revisiones RDD, todas aprobadas (sección siguiente).
  - Tres verificadores independientes (sonnet, high), sin bloqueos: el tramo de huecos baratos (ítem de riesgo 2, la
    comprobación de organización), las versiones (ítem 4, el cerrojo de lectura y escritura) y el diario de
    diferencias (ítem 1, formato y recuperación).
  - La revisión de Codex no se pudo hacer, ni de las dos PRD ni del código: la cuenta llegó a su límite de uso hasta
    el 2026-11-11 a las 06:49.
- **Hallazgos corregidos por el camino:**
  - AC-F16 encontró que la recuperación con diferencias dejaba una página dañada: `collect_garbage` descartaba
    versiones sin escribir mientras la cabecera del diario daba sus confirmaciones por aplicadas. Antes no se notaba
    porque la recuperación reescribía páginas enteras. Lo corrige `ebbf6ee` con `State::unwritten`.
  - El diario guardaba una entrada vacía para una página copiada que no cambió ningún byte, contra Y1; `7192dc7`.
  - El verificador del diario encontró tres huecos de tests (AC-Y2, y AC-Y3 y AC-Y4 a través de la apertura) y un
    nodo sin celdas que declaraba huecos y pasaba la comprobación; los cierra `4aa2835`.
  - La primera ronda de mediciones de T6 se hizo con subagentes trabajando en la misma máquina y dio una falsa
    regresión de W12 y W13 y una falsa duplicación de W2. Se biseccionó, se descartó, y desde entonces no se mide
    con nada más corriendo (L9 y L10 de [cheap-gaps.md](cheap-gaps.md)).

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `e0ad729..d96a92b` | risk, reliability | aprobada | R3-fixed-leaf-split-precompute (SUGGESTION): ningún test fijaba el límite de división de una hoja de tamaño fijo; `49e25ee` | `review-3be47b92a02e4060` |
| `d96a92b..e08b646` | risk, resilience, reliability | aprobada | R4-001 (SUGGESTION, el contador en producción): ya corregido en `7e4d872`. R3-version-pointer-guard-untested (SUGGESTION): `c980732`. R3-u3-no-deterministic-lock-test (WARNING): descartado con evidencia (cada lectura suelta el cerrojo antes de otra llamada; U3 lo mide AC-U8) | `review-42c285b88cc58af2` |
| `e08b646..335abbb` | risk, resilience, reliability | aprobada | R4-commit-base-expect y R3-commit-base-expect-panic (SUGGESTION, el mismo punto): el commit hacía `expect` si faltaba la base de una página; ahora falla con la clase de daño, `ec9a9cc` | `review-8b7f0a4e693379ce` |
| `335abbb..4a3a92e` | resilience, reliability | aprobada | R4-commit-base-damage-untested (SUGGESTION) y R3-commit-base-damage-untested (WARNING), el mismo punto: ningún test llega al camino de daño de `ec9a9cc`. Descartados con evidencia: no se alcanza por ninguna interfaz (`stage` exige que la página esté dentro del almacén, que solo crece mientras está abierto); el comentario ahora lo dice, `dd378bb`. R3-pd-s6-claim-contradicts-w18 (WARNING): L8 de page-diffs no nombraba W18; corregido | `review-88827c8bc66af220` |
| `4a3a92e..f295627` | readability | aprobada | R2-w18-alternating-claim-overstated (SUGGESTION): la corrección de L8 todavía exageraba; ahora da los recuentos y medianas de las rondas alternadas, `2f175a2` | `review-bca2a488be0037db` |

La primera se escaló con el ítem 2, la segunda con el 4 y la tercera con el 1. Las dos últimas cubren el último
tramo, como pide la regla de cierre. El commit final de registro (`2f175a2`) se evaluó `passive`, y las ejecuciones
completas y este informe son solo documentación.

## Tiempo

- Estimación: entre 3 y 5 horas, solo para la tanda de huecos baratos (L1 de [cheap-gaps.md](cheap-gaps.md)).
  Los ítems 1 y 2 entraron a las 06:3x, con la orden de hacerlos sin preguntar y sin estimación propia. Inicio:
  2026-11-08 06:26:28 UTC. Fin: 19:49:57 UTC.
- **Duración activa desde el arranque: 13 h 23 min.** Los tres trabajos, código, tests, verificadores y revisiones
  RDD incluidos, terminaron con `ec9a9cc` a las 10:06, es decir, 3 h 40 min después del arranque: dentro de la
  estimación de la tanda barata aunque llevara también los ítems 1 y 2. Las 9 h 41 min siguientes fueron
  mediciones:
  - las rotaciones de cuatro binarios en Windows y WSL 2 (3 h 30 min);
  - su confirmación y las rondas alternadas (1 h 44 min);
  - las ejecuciones completas (4 h 09 min).

  En esas horas el registro de fases siguió en *Código y tests*, porque no marqué una fase de espera de las
  mediciones. Por eso la tabla le da a esa fase 12 h 12 min, que son sobre todo mediciones en segundo plano.
- Esperas por el usuario: ninguna. El usuario estuvo fuera; sus mensajes durante la ejecución completa de WSL
  no pararon nada.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 35 min 18 s | 4 % | 1 |
| Código y tests | 12 h 12 min | 91 % | 1 |
| Checks (fmt, clippy, suite de tests) | 33 min 29 s | 4 % | 2 |
| Revisión RDD | 1 min 54 s | 0 % | 1 |
| Documentación | 0 min 06 s | 0 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **13 h 23 min** | **100 %** | **7** |

- Inicio: 2026-11-08 06:26:28 +00:00. Fin: 2026-11-08 19:49:57 +00:00.
- Arranque confirmado: 06:26:28. Activo desde el arranque: **13 h 23 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | T6 windows | 07:05:02 | 07:15:10 | 10 min 08 s |
| Aceptación | T6 wsl | 07:15:10 | 07:15:48 | 0 min 38 s |
| Aceptación | T6 bisect temp attribute | 07:19:48 | 07:27:25 | 7 min 37 s |
| Revisión RDD | RDD cheap slice | 07:50:42 | 07:50:43 | 0 min 01 s |
| Revisión RDD | RDD version slice | 09:06:17 | 09:07:30 | 1 min 13 s |
| Revisión RDD | RDD page diffs slice | 09:59:15 | 10:01:01 | 1 min 46 s |
| Aceptación | rotations windows | 10:10:37 | 11:06:19 | 55 min 42 s |
| Aceptación | rotations wsl | 11:06:19 | 13:40:41 | 2 h 34 min |
| Aceptación | confirmation windows | 13:42:31 | 13:52:56 | 10 min 25 s |
| Aceptación | confirmation wsl | 13:52:56 | 15:13:44 | 1 h 20 min |
| Aceptación | alternating rounds windows | 15:15:01 | 15:23:27 | 8 min 26 s |
| Aceptación | alternating W3 wsl | 15:24:13 | 15:28:09 | 3 min 56 s |
| Aceptación | complete windows | 15:28:58 | 17:06:09 | 1 h 37 min |
| Aceptación | complete wsl | 17:06:09 | 19:38:18 | 2 h 32 min |
| Aceptación | alternating W16 wsl | 19:39:00 | 19:39:30 | 0 min 30 s |
| Revisión RDD | RDD last slice | 19:39:47 | 19:40:26 | 0 min 39 s |
| **Aceptación (total)** | | | | **9 h 41 min** |
| **Revisión RDD (total)** | | | | **3 min 39 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `71c6aa8` | perf(bench): sum read bytes in u16 lanes |  | 4 min 37 s | Exploración y mapa 4 min 37 s |
| `fafb5bf` | perf(pager): mark temporary-area files as temporary on Windows |  | 5 min 51 s | Exploración y mapa 5 min 51 s |
| `3d6695f` | perf(tree): skip the prefix comparisons of an interval without prefix |  | 5 min 49 s | Exploración y mapa 5 min 49 s |
| `8f09c51` | perf(tree): decide a fixed-size leaf change on the node already checked |  | 0 min 00 s |  |
| `d96a92b` | perf(tree): check a slotted node's layout in one pass over its cells |  | 18 min 42 s | Exploración y mapa 18 min 42 s |
| `8fa0a86` | docs(prd): add the PRDs of the version checks and the journal page diffs | T6 builds | 27 min 05 s | Exploración y mapa 0 min 19 s, Checks (fmt, clippy, suite de tests) 26 min 46 s |
| `1781031` | docs(odd): record T1-T5 of the cheap gaps run |  | 0 min 00 s |  |
| `5199efc` | perf(tree): remember the layout check of temporary-area versions | T6 builds, V1 | 14 min 27 s | Código y tests 13 min 35 s, Checks (fmt, clippy, suite de tests) 0 min 52 s |
| `a289acf` | perf(pager): let page reads share the temporary-area lock | V1 | 7 min 13 s | Código y tests 7 min 13 s |
| `f4a717f` | docs(odd): add the version checks execution document | V1 | 0 min 21 s | Código y tests 0 min 21 s |
| `49e25ee` | test(tree): pin the split boundary of a fixed-size leaf | V1 | 2 min 51 s | Código y tests 2 min 51 s |
| `e08b646` | docs(odd): record the verification of the cheap gaps slice and V1-V2 | V1 | 0 min 41 s | Código y tests 0 min 41 s |
| `ebbf6ee` | feat(journal): journal the bytes each commit changes in a page | V1 | 41 min 06 s | Código y tests 41 min 06 s |
| `7e4d872` | perf(pager): count version layout checks only in builds with test hooks | V1 | 0 min 38 s | Código y tests 0 min 38 s |
| `fe2f1b8` | feat(tree): leave holes in slotted nodes instead of moving their cells | V1 | 18 min 22 s | Código y tests 18 min 22 s |
| `7192dc7` | fix(journal): keep no entry for a copied page that changed no byte | V1 | 9 min 49 s | Código y tests 9 min 49 s |
| `ca7c425` | docs(prd): annotate the PRDs the version checks and the journal page diffs change |  | 0 min 00 s |  |
| `5fde9ae` | docs(prd): translate the PRDs of the version checks and the journal page diffs | V1 | 2 min 05 s | Código y tests 2 min 05 s |
| `c980732` | test(pager): pin that only the version that passed keeps its layout check | V1 | 3 min 35 s | Código y tests 3 min 35 s |
| `bc059c3` | docs(odd): record the verification and RDD review of the version checks | V1 | 0 min 12 s | Código y tests 0 min 12 s |
| `4aa2835` | test(journal): recover through the opening after crashes, cuts and damage | V1 | 48 min 44 s | Código y tests 48 min 44 s |
| `335abbb` | docs(odd): record the verification of the journal page diffs | V1 | 0 min 22 s | Código y tests 0 min 22 s |
| `ec9a9cc` | fix(pager): fail with damage, not a panic, when a commit finds no base | V1 | 7 min 58 s | Código y tests 7 min 58 s |
| `d679cf3` | docs(odd): record the RDD review of the journal page diffs |  | 0 min 00 s |  |
| `4a3a92e` | docs(odd): record the single-load rotations of the cheap gaps, version checks and page diffs | V1 | 5 h 23 min | Código y tests 5 h 23 min |
| `dd378bb` | docs(pager): say why a commit cannot miss the base of a copied page | V1, T7 | 4 h 17 min | Código y tests 4 h 11 min, Checks (fmt, clippy, suite de tests) 5 min 40 s |
| `f295627` | docs(odd): record the RDD review of the last slice of the page diffs |  | 0 min 00 s |  |
| `2f175a2` | docs(odd): give the alternating rounds of W18 in the page diffs record | T7 | 1 min 05 s | Checks (fmt, clippy, suite de tests) 0 min 11 s, Revisión RDD 0 min 54 s |
| `a329a36` | docs(bench): add the complete runs after the cheap gaps, the version checks and the page diffs | T7 | 1 min 06 s | Revisión RDD 1 min 00 s, Documentación 0 min 06 s |
| — | después del último commit |  | 0 min 00 s |  |
| **Total** | | | **13 h 23 min** | |

Las fases que más pesaron son las mediciones: las rotaciones de cuatro binarios, sus repeticiones y las dos
ejecuciones completas. Ningún tramo de código tomó más de 49 minutos (`4aa2835`, los tests de recuperación que
pidió el verificador del diario). Las filas de `4a3a92e` y `dd378bb` son las esperas de las mediciones.

## Informe de cambios (`e0ad729..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 1.021 | 2 | — | — | 23 |
| `crates/krummi-pager` | 668 | 118 | 528 | 115 | 10 |
| `crates/krummi-tree` | 503 | 67 | 423 | 67 | 6 |
| `crates/krummi-tx` | 419 | 32 | 350 | 49 | 5 |
| `crates/krummi` | 389 | 57 | 337 | 34 | 10 |
| `odd` | 407 | 0 | — | — | 4 |
| `crates/krummi-bench` | 37 | 1 | 30 | 6 | 1 |
| **Total** | **3.444** | **277** | **1.668** | **271** | **59** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Sin archivos generados ni de bloqueo. `docs` incluye las dos PRD con sus traducciones, sus anotaciones y los
informes del banco. No había previsión de líneas.

## Resumen

La rama acerca Krummi a Voron en las cargas donde la comparación de su código mostraba diferencias baratas, y en las
dos grandes que quedaban en el plan. El banco suma los bytes leídos en 16 carriles de `u16` en vez de byte a byte,
igual que Voron hace con SSE2 y sin código inseguro. Por dentro, los intervalos sin prefijo se ahorran sus
comparaciones, una inserción en un árbol de tamaño fijo lee su hoja una vez, la comprobación de organización de un
nodo recorre sus celdas en una pasada, y los archivos del área temporal se marcan como temporales en Windows. Cada
versión confirmada del área temporal recuerda que pasó la comprobación, y las lecturas comparten su cerrojo. El
diario guarda solo los bytes que cambian en cada página (o la página creada sin sus ceros), la recuperación los
aplica sobre `krummi.data` y comprueba al final cada página aplicada, y los nodos de entradas de tamaño variable
dejan huecos al borrar en vez de mover sus celdas, para que un cambio pequeño cambie pocos bytes. El almacén pasa al
formato `10`.

## Decisiones tomadas sin preguntar

El usuario autorizó escribir las PRD con mis recomendaciones y aplicarlas sin preguntar (L4 de
[cheap-gaps.md](cheap-gaps.md)).

- Ítem 7: comprobación más rápida en vez de una caché de la última hoja, que solo ahorraba los niveles de rama (L8).
- Ítem 6: solo el atributo de temporal; borrar al cerrar no aporta, y las copias de más dependen del formato del
  diario (L5).
- Ítem 3: las dos reservas de memoria de cada `Range` se quedan; quitarlas pedía un vector en línea hecho a mano
  para menos de un 5 % (L6).
- PRD de versiones: memoria propia de cada versión, que no usa ni alimenta la compartida; las anotaciones de C3 y S1.
- PRD del diario: comprobación al final de la recuperación, con la excepción a J8 declarada; huecos sin poner a
  cero; una entrada nunca mucho mayor que hoy; huecos en todos los árboles de tamaño variable y en el modo temporal.
- La regla de medición: una carga más baja en las dos rondas se repite en dos rondas nuevas antes de darla por peor,
  y lo que vuelve a bajar se mide en cinco rondas alternadas.

## Pendiente (no bloquea)

- **Revisión de Codex** de las dos PRD y del código de `e0ad729..HEAD`: bloqueada por el límite de uso hasta el
  2026-11-11 a las 06:49. No bloquea porque las cinco revisiones RDD y los tres verificadores no encontraron
  bloqueos, pero la regla del proyecto la pide antes del cierre.
- **W21 en Windows −1,5 %** con la comprobación recordada de versiones. Hipótesis sin medir: en un nodo de tamaño
  fijo la comprobación que se ahorra cuesta menos que el cerrojo compartido de más que la recuerda. W21 no está en
  la lista de AC-U8, y la rama entera la sube un +46–51 % en Windows.
- **W3 en WSL 2 −3,1 %** en la rama entera, sin ningún paso más bajo en las dos rondas; en Windows no baja. Es el
  único umbral que Linux incumple (0,78): perfilarlo es el primer paso para recuperarlo.
- **W18 en Windows con el diario de diferencias**: inconcluso (pd más bajo en cuatro de cinco rondas alternadas,
  con la mediana mayor). Cota estimada sin medir: la comprobación con huecos añade decenas de nanosegundos por hoja
  a lecturas de unos 6 µs, menos de un 1 %. En la ejecución completa, W18 queda en 1,58 frente a Voron.
- **W17 en Windows** (0,63): el único umbral que Windows incumple. Esta tanda no lo tocó; su coste está diagnosticado
  en el plan (escribir 16 MiB por confirmación y un archivo temporal por valor).
- **`ac10_every_altered_byte_with_the_journal` completo** (unos 75 minutos) no se ejecutó: el verificador lo paró a
  los 25 minutos, y su versión con paso 11 pasó en 420 s.
- **El perfil de W12** no se hizo: la mejora de las versiones se midió, pero no dónde se va el resto del tiempo.
- **W1:** su mayor coste sigue siendo volver a comprobar la hoja tras cada cambio (S1 de las comprobaciones del
  escritor). Cambiarlo es una decisión de producto.

**Riesgo:** ítem 1 (formato del diario y del almacén, recuperación), ítem 2 (la comprobación de organización, una
defensa contra daño) e ítem 4 (el cerrojo de lectura y escritura del área temporal); cada uno tuvo su verificador
independiente y su revisión RDD.

## Limpieza manual

- `work/cg` (9,0 GiB): binarios, almacenes y salidas de las cargas sueltas. Las salidas `rot-*.out`,
  `conf-*.out`, `alt*.out` respaldan las tablas de los documentos de ejecución; conviene guardarlas hasta la
  revisión de Codex.
- `work/complete/target-cg` (586 MiB): la compilación de la ejecución completa de Windows.
- En WSL: `~/cg` (5,9 GiB), `~/krummi-cg-target` (269 MiB), `~/krummi-cg-src` (8 MiB) y `~/bench-work-cg`.
