# Margen de escritura terminado: W19 +10 %, W24 y W28 algo más rápidos, W20 −3 % aceptado; rama verificada

Informe de cierre de `write-margin` (2026-11-16). Enfoque de ejecución: ODD.
Fuente de producto: ninguna. Son optimizaciones sin cambio de comportamiento ni de formato, sin PRD. Documento de
ejecución: [write-margin.md](write-margin.md).

Rama `feat/write-margin`, creada desde `main` en `6b49bea`. Sin fusionar y sin publicar: queda para la revisión del
usuario.

## Resultado

- **Especificación:**
  - **S1, medición de W28** (L3). Por actualización hay 2 lecturas de rama, con las comprobaciones ya recordadas, y
    1 de hoja:
    - 54 %: versión del área temporal leída por primera vez;
    - 43 %: página del archivo de datos sin recordar;
    - 3 %: página que la transacción ya cambió;
    - 0,1 %: hoja ya recordada.

    Esta medición decidió T4 y deja cifradas W28-A y W28-B, que quedan fuera.
  - **S2, W19-A** (`893468a`, `7f4a182`). Agregar un valor a una clave con valores en línea escribe la entrada en la
    hoja que encontró la búsqueda, sin recorrer el árbol otra vez. Lo hace cuando no se divide ni se fusiona ningún
    nodo; si no, sigue por `set`. Las páginas quedan con los mismos bytes.
  - **S3, W28-C:**
    - las versiones del área temporal se buscan con un hash pensado para números de página (`4da23ed`);
    - la revisión de estructura de una versión se recuerda a través de la marca que devolvió su lectura, sin el
      segundo lock ni la segunda búsqueda (`fdb7af9`).
  - **Suite final** (el código de `1ee31df`, igual al de la cabeza):
    - `cargo test --workspace`: 856 pasan, 0 fallan, 34 ignoradas;
    - `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings`: limpios.
- **Aceptación (S4):**
  - **A/B de Krummi solo, `main` contra la rama** (`work/wm/ab.py`): carga a escala completa, núcleos de
    rendimiento, plan de energía del banco, orden alterno y sumas de comprobación iguales en cada corrida.

    | Carga | Rama / `main` (op/s) | Rondas |
    |---|---|---|
    | W19 multivalor-carga | **1,101–1,111** | 3 + 3 |
    | W24 tabla-insercion-al-azar | 1,024–1,028 | 3 + 3 |
    | W28 tabla-actualizacion | 1,011–1,016 | 3 + 3 |
    | W7 lecturas-al-azar | 1,002–1,018 | 3 + 3 |
    | W8 / W9 lecturas-al-azar-4 / -16 | 0,997 / 0,994 | 3 |
    | W12 / W13 lecturas con escritor | 1,091 / 1,047 | 10 |
    | W26 tabla-lecturas | 0,989–0,993 | 3 + 3 |
    | W1, W3, W23 | 0,999–1,009 | 3 |
    | **W20 multivalor-lectura** | **0,972** (más lenta en 18 de 20) | 20 |

  - **W20** queda un 2–3 % más lento en el binario normal. Con los dos binarios compilados con los bucles alineados
    a 64 bytes, la diferencia desaparece (0,997, más lenta en 5 de 10). La causa probable es la disposición del
    código, no trabajo añadido, aunque no está probado como causa única. **El usuario aceptó esa diferencia**
    (L10, L11) y S4 la nombra como excepción.
  - **Ejecuciones completas con `--solo-krummi-voron`:**
    - **Windows:** **29 de 29** ([informe](../../docs/benchmark/2026-11-16-windows-write-margin.md)).
    - **WSL 2:** **28 de 29**; incumple `tamano-fijo-carga` con 0,71
      ([informe](../../docs/benchmark/2026-11-16-linux-write-margin.md)). Esa corrida se hizo sin el plan de energía
      y los dos motores variaron mucho respecto de ayer. Un A/B de W21 en WSL da la rama más rápida que `main`
      (18.024 contra 18.545 ns, L13): el incumplimiento no viene de la rama.
    - En la corrida de Windows, las lecturas concurrentes de Krummi bajaron entre un 25 y un 35 % respecto de la de
      `main` de esta mañana. El A/B inmediato de W8 y W9 da 0,997 y 0,994: fue el estado de la máquina (L13).
- **Revisiones y verificación:**
  - **Verificador independiente** para T4 (Risk ítem 4): aprobado, sin bloqueos (L6).
  - **RDD:** aprobó los cuatro tramos (tabla de abajo).
  - **Codex:** tres rondas. R1 (W20 contra S4) quedó resuelto por decisión del usuario y verificado en la ronda 3. R2
    es un aviso anterior a la rama y queda pendiente. Documentos:
    [ronda 1](../../docs/reviews/write-margin/2026-11-16-code.md),
    [ronda 2](../../docs/reviews/write-margin/2026-11-16-code-02.md),
    [ronda 3](../../docs/reviews/write-margin/2026-11-16-code-03.md) y
    [resoluciones](../../docs/reviews/write-margin/2026-11-16-code-resolutions.md).
- **Hallazgos corregidos por el camino:**
  - **Rama muerta en `replace`.** La rama para valores del mismo largo no se ejecutaba nunca y ahora devuelve
    `false`, porque ese caso le corresponde a `overwrite`. Se agregó una prueba directa de sus controles (`7f4a182`,
    R3-001 y R3-002 de RDD).
  - **Intento fallido de arreglar W20.** `687ef64` devolvió a `entry` su recorrido propio. Una sesión lo daba por
    bueno y otras dos lo contradijeron, así que se revirtió (`b5b4e29`).
  - **Una causa no probada en S4.** S4 afirmaba la causa de W20; ahora dice solo la excepción aceptada (L12).

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `6b49bea..4da23ed` (T2, T3) | riesgo, fiabilidad | aprobado | 4 SUGGESTION. R1-001 y R1-002 no piden cambios. R3-001 (rama sin uso en `replace`) y R3-002 (sin prueba directa de los controles), corregidos en `7f4a182` | `review-8c1ce0cefd91e0ac` |
| `4da23ed..d148e7d` (T4, riesgo alto ítem 4) | riesgo, fiabilidad | aprobado | 3 SUGGESTION. R3-001 (marca de otra instantánea): nota en `version_checked`. R3-002 (que la marca llegue entera): cubierto por AC-U1–AC-U5, que fallan por mutación. R3-003: registro de T4 | `review-224c85ccfe6aae29` |
| `d148e7d..1ee31df` | fiabilidad | aprobado | 1 SUGGESTION, descartada con evidencia: `long_key` siempre mide `MAX_KEY` | `review-75d1e7d679f238bc` |
| `1ee31df..b7d8083` | legibilidad | aprobado | 2 WARNING y 1 SUGGESTION. S4 afirmaba una causa no probada; fila duplicada en el registro de fases; línea en blanco de más. Corregidos en `9c1fe91` | `review-b01fa0cf505a75c2` |

Evaluaciones sin revisión: `under_budget` después de `893468a` (253 líneas) y de `4da23ed` (319). Las revisó el
primer tramo, que se cerró antes de T4 por ser de riesgo alto. También `under_budget` después de `7f4a182` (109), lo
que revisó el tercer tramo.

## Tiempo

- **Estimación:** 3 h – 5 h. Arranque real: 2026-11-16 11:11:02. Fin de la verificación: 17:20:51.
- **Duración activa desde el arranque: 4 h 20 min**, dentro de la estimación. Más de lo habitual en esta serie de
  runs, por dos razones: investigar W20 llevó unos 40 minutos de exploración y correcciones, y las comprobaciones (la
  suite completa tarda unos 12 minutos) sumaron 1 h 31 min en siete pasadas.
- **Esperas por el usuario:** 1 h 49 min en total.
  - 13:36–14:39, decisión sobre R1 (perfilar W20);
  - 15:17–16:03, decisión de cerrar R1.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 42 min 33 s | 16 % | 2 |
| Plan (documento de ejecución) | 2 min 33 s | 1 % | 1 |
| Código y tests | 13 min 27 s | 5 % | 3 |
| Checks (fmt, clippy, suite de tests) | 1 h 31 min | 35 % | 5 |
| Correcciones de hallazgos | 37 min 22 s | 14 % | 4 |
| Cierre (informe y merge) | 1 h 13 min | 28 % | 2 |
| **Total activo** | **4 h 20 min** | **100 %** | **17** |

- Inicio: 2026-11-16 11:11:02 +00:00. Fin: 2026-11-16 17:20:51 +00:00.
- Arranque confirmado: 11:11:02. Activo desde el arranque: **4 h 20 min**; antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 1 h 49 min.
  - 13:36:13–14:39:49 (1 h 03 min): R1 decision
  - 15:17:44–16:03:10 (45 min 26 s): R1 decision 2

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | ab-krummi | 12:08:53 | 12:28:07 | 19 min 14 s |
| Aceptación | ab-3builds | 12:31:04 | 12:37:18 | 6 min 14 s |
| Aceptación | ab-w20-inline | 12:40:15 | 12:40:26 | 0 min 11 s |
| Revisión RDD | codex-R1 | 12:50:06 | 13:03:19 | 13 min 13 s |
| Aceptación | ab-w20-entry | 13:04:29 | 13:04:44 | 0 min 15 s |
| Aceptación | ab-final | 13:15:39 | 13:29:37 | 13 min 58 s |
| Aceptación | ab-w20-layout | 13:29:56 | 13:30:08 | 0 min 12 s |
| Revisión RDD | codex-R2 | 13:30:54 | 13:35:31 | 4 min 37 s |
| Aceptación | full-windows | 13:36:03 | 14:00:57 | 24 min 54 s |
| Aceptación | full-linux | 14:00:57 | 14:40:08 | 39 min 11 s |
| Aceptación | ab-w20-20rounds | 14:44:28 | 14:44:57 | 0 min 29 s |
| Aceptación | ab-w20-aligned | 15:17:15 | 15:17:30 | 0 min 15 s |
| Revisión RDD | codex-R3 | 16:03:10 | 16:07:31 | 4 min 21 s |
| Aceptación | full-linux | 16:07:58 | 17:01:46 | 53 min 48 s |
| Aceptación | ab-linux-w21-windows-reads | 17:02:28 | 17:17:43 | 15 min 15 s |
| Aceptación | ab-w12-10rounds | 17:18:12 | 17:20:37 | 2 min 25 s |
| **Aceptación (total)** | | | | **2 h 56 min** |
| **Revisión RDD (total)** | | | | **22 min 11 s** |

En el registro, varias marcas de segundo plano de los A/B cortos solo cubren el tramo final de la tarea, porque
la compilación de los binarios va dentro del mismo comando antes de `bg-start`. La primera corrida de Linux
(14:00–14:40) se detuvo en W6 al decidir perfilar W20. Las revisiones de Codex aparecen como «Revisión RDD»
porque es la fase de revisión del registro.

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `902c6ee` | docs(odd): add the write-margin execution document | T0, T1, T2 | 24 min 28 s | Exploración y mapa 4 min 38 s, Plan (documento de ejecución) 2 min 33 s, Código y tests 5 min 15 s, Checks (fmt, clippy, suite de tests) 12 min 02 s |
| `893468a` | perf(tree): write a multi-value add in the leaf its lookup found |  | 0 min 00 s |  |
| `4da23ed` | perf(pager): find page versions with a hash made for page numbers | T2, T3 | 9 min 50 s | Código y tests 0 min 51 s, Checks (fmt, clippy, suite de tests) 8 min 59 s |
| `fdb7af9` | perf(pager): remember a version's layout check through the mark its read returned | T3, T4 | 14 min 58 s | Código y tests 7 min 21 s, Checks (fmt, clippy, suite de tests) 7 min 37 s |
| `d148e7d` | docs(odd): record T2, T3 and the first review of the write-margin run |  | 0 min 00 s |  |
| `7f4a182` | fix(tree): leave same-length values to overwrite and test replace's guards | T4, T2, T5 | 48 min 42 s | Checks (fmt, clippy, suite de tests) 16 min 40 s, Correcciones de hallazgos 32 min 02 s |
| `1ee31df` | docs(odd): record T4, the review fixes and the A/B of the write-margin run |  | 0 min 00 s |  |
| `687ef64` | perf(tree): read multi-value entries through their own descent again | T2, R1 | 22 min 50 s | Checks (fmt, clippy, suite de tests) 22 min 09 s, Correcciones de hallazgos 0 min 41 s |
| `b5b4e29` | revert: read multi-value entries through located_entry again | R1 | 18 min 35 s | Checks (fmt, clippy, suite de tests) 18 min 35 s |
| `864d430` | docs(reviews): record the first write-margin code review and its resolutions | R1 | 0 min 22 s | Checks (fmt, clippy, suite de tests) 0 min 22 s |
| `e05655c` | docs(reviews): record the second write-margin code review | R1 | 5 min 03 s | Checks (fmt, clippy, suite de tests) 5 min 03 s |
| `980df18` | docs(odd): record the user's decision on W20 for the write-margin run | R1 | 38 min 18 s | Exploración y mapa 37 min 55 s, Checks (fmt, clippy, suite de tests) 0 min 23 s |
| `91a1e17` | docs(odd): mark the third write-margin code review in the phase log | R1 | 0 min 01 s | Correcciones de hallazgos 0 min 01 s |
| `b7d8083` | docs(reviews): record the third write-margin code review | R1 | 4 min 38 s | Correcciones de hallazgos 4 min 38 s |
| `9c1fe91` | docs(odd): state W20's exception without an unproven cause | T6 | 1 min 35 s | Cierre (informe y merge) 1 min 35 s |
| — | después del último commit | T6 | 1 h 11 min | Cierre (informe y merge) 1 h 11 min |
| **Total** | | | **4 h 20 min** | |

## Informe de cambios (`6b49bea..9c1fe91`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 387 | 0 | — | — | 4 |
| `crates/krummi-tree` | 233 | 25 | 195 | 31 | 5 |
| `odd` | 200 | 0 | — | — | 2 |
| `crates/krummi-pager` | 141 | 48 | 104 | 27 | 4 |
| `crates/krummi-tx` | 22 | 18 | 16 | 6 | 2 |
| **Total** | **983** | **91** | **315** | **64** | **17** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell,
JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

No hubo previsión de líneas. El código propio son unas 380 líneas, de las cuales unas 190 son pruebas.

## Resumen

Dos de las cargas donde Krummi iba detrás de Voron ganan algo sin quitar ninguna comprobación:
- **Multivalor:** un valor nuevo se escribe en la hoja que encontró la búsqueda, en vez de recorrer el árbol otra
  vez. Es lo mismo que hace `MultiAdd` en Voron. W19 gana un 10 %.
- **Versiones de página:** el área temporal busca las versiones con un hash propio para números de página. Además,
  la primera revisión de una versión se recuerda a través de una marca que devuelve la propia lectura, sin volver a
  tomar el lock. W28 gana un 1–2 % y W24 un 2–3 %.

La medición previa de W28 mostró dónde está el resto de su diferencia con Voron: casi toda actualización vuelve a
verificar entera la hoja que lee. Eso sale de las garantías actuales y queda cifrado para un PRD (W28-A y W28-B).

## Valores por defecto elegidos sin preguntar

- **Ruta:** todas las tareas en línea, sin escritores delegados, porque eran unidades pequeñas y dependientes entre
  sí.
- **Medición de S1:** con una copia instrumentada fuera del repositorio, sin commit.
- **Riesgo de T2:** declarado «ninguno», por lo que no tuvo verificador independiente. Su prueba compara byte a byte
  con `set` en 20.000 operaciones y falla con la mutación del control del cuarto lleno.
- **`#[inline]` en `located_entry`:** se probó y se dejó fuera, porque no recuperaba W20.
- **Informes completos:** a escala completa, como el banco. El de Linux se hizo sin el plan de energía de Windows; el
  script no lo activaba, y queda anotado en L13.
- **Módulo de archivo:** `pager`, donde está la mayor parte del cambio y el resto de la serie de rendimiento (O1–O8).

## Pendiente (no bloqueante)

- **W20, 2–3 % más lento en el binario normal.** Lo aceptó el usuario (L11). Alinear los bucles en el perfil de
  release podría recuperarlo, pero cambia el binario de todas las cargas: es un cambio aparte, con su propio A/B.
- **R2 de Codex, aviso anterior a la rama.** Con la API de bajo nivel del pager, un `ScratchWriter` reutilizado
  después de `publish` no cuenta para la recolección de versiones. Por eso pueden liberarse los slots de una versión
  que sigue en uso, con los bytes de la página y, desde esta rama, con la marca. No bloquea porque ya existía en
  `6b49bea` y ninguna operación del almacén lo alcanza: `WriteTransaction::commit` consume su escritor. Queda para una
  revisión del pager.
- **W28-A y W28-B, fuera de alcance.** Necesitan PRD porque relajan comprobaciones. S1 cifró su efecto: W28-A quita el
  hash de 8 KiB en el 43 % de las actualizaciones y W28-B la revisión de estructura en el 97 %.
- **Avisos opcionales del verificador de T4** (L6):
  - no hay prueba de una marca que sobreviva a su instantánea, aunque el compilador ya lo impide;
  - el contador de pruebas `version_checks` depende de que `Unverified` nunca llegue a una transacción.
- **Corrida de Linux con el plan de energía.** La corrida completa de WSL no es comparable con la de ayer; repetirla
  con el plan activo confirmaría `tamano-fijo-carga`, que el A/B ya muestra que no depende de la rama.

**Risk:** ítem 4 (T4 entrega, desde una lectura, una referencia a un indicador atómico de un slot del área temporal,
tomada bajo el lock y usada después; código `unsafe`). Lo revisaron un verificador independiente y RDD, sin
bloqueos. T2 y T3: ninguno.

## Limpieza manual

Se pueden borrar; están todos fuera de git:
- `work/wm/` (7,5 GB): fuentes y binarios de los A/B, perfiles y almacenes;
- `~/wm` en WSL (577 MB);
- `work/w16exp/` (1,7 GB) y `work/bench/2026-11-16/` (15 KB), de antes del run.
