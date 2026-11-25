# Experimento ANN del documento 12 terminado: medido, revisado y sin fusionar

Informe de cierre de `ann-experiment` (2026-11-25). Enfoque de ejecución: ODD.

- Origen del producto: el [documento de diseño 12](../../docs/design/tables/12-vector-indexes.md), *Benchmarks*, pasos 1 a 3. El PRD 7.6 se escribe después, con estas mediciones (S1).
- Documento de ejecución: [ann-experiment.md](ann-experiment.md).
- Informe del experimento: [2026-11-24-ann-experiment.md](../../docs/benchmark/2026-11-24-ann-experiment.md).

Estado de la entrega:
- Rama `feat/tables-vector-indexes`, con commits locales.
- Sin fusionar en `main` y sin subir al remoto.
- El cambio no toca el motor: es un crate de laboratorio no publicado, `krummi-ann-lab`.

## Resultado

- **Especificación.** S1 a S8 cumplidas, con un límite de S6 que aceptaste (ver Pendiente).
  - Crate aparte (S2).
  - Datos: SIFT1M, GloVe-100 y Cohere 768 (S3).
  - Resultados idénticos bit a bit en Windows y Linux (S4).
  - Escaneo SQ8, IVF-Flat e IVF-SQ8 contra la búsqueda exacta (S5).
  - Medidas: recall, latencia, tamaño y memoria (S6).
  - Veredicto contra los umbrales (S7).
  - Núcleos del documento 5 con bits fijados (S8).
  - `cargo test --workspace`: 1284 pasan, 0 fallan. `cargo fmt --all --check` y
    `cargo clippy --workspace --all-targets -- -D warnings`, limpios.
- **Aceptación** con los datos reales, a 1 M × 768, un hilo, a frecuencia base:

  | Caso | Mínimo para recall@10 ≥ 0.95 | p95 | Umbral |
  |---|---|---|---|
  | Cohere L2 (caso de referencia) | 4.0 % de las listas | 17.20 ms | cumple |
  | Cohere producto escalar | 7.2 % | 20.90 ms | no cumple por 0.9 ms |
  | GloVe | 14.7 % | 6.18 ms | no cumple el ~10 % de las listas |
  | HNSW externo, Cohere L2 | — | 11.19 ms | — |
  | HNSW externo, producto escalar | — | 5.89 ms | — |

  El HNSW externo no llega a 0.95 en GloVe. La elección del algoritmo queda para el PRD 7.6.
- **Revisiones y verificación:**
  - RDD: seis revisiones aprobadas y reconocidas; el detalle está más abajo.
  - Codex: cuatro rondas, de las que la cuarta aprobó; los informes están en
    [docs/reviews/ann-experiment/](../../docs/reviews/ann-experiment/2026-11-24-code.md), con sus
    [resoluciones](../../docs/reviews/ann-experiment/2026-11-24-code-resolutions.md).
- **Hallazgos corregidos por el camino:**
  - El test de paquetes del workspace fallaba desde T1, porque no conocía el crate nuevo (L13).
  - `--sondeos` descartaba sin avisar los valores mayores que el número de listas (RDD R1-001, CRITICAL).
  - Había opciones que una orden ignoraba en silencio, y una verdad en caché desactualizada daba recall falsa (avisos
    de RDD).
  - Codex R1: más opciones ignoradas. Codex R3: la recall salía mal con menos de k filas.
  - Codex R2: faltaba la memoria. Se midió el RSS en Linux, y se separaron la carga útil, lo asignado y el RSS
    observado.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `dfba65f..32ad2c6`, la rama entera | riesgo, fiabilidad | detenida: `lens_context_budget_exceeded`, sin autoridad | Ninguno. Se revisó por tramos | — |
| `dfba65f..b703169` (T1–T3, parquet, ítem 5) | riesgo, fiabilidad | aprobada | WARNING, caché de verdad desactualizada: corregido. WARNING, sin test del chequeo de empates: corregido. WARNING, ivecs vacío ilegible: corregido rechazando `--filas 0`. SUGGESTION, recall NaN: inalcanzable tras ese chequeo | `review-101b877689b09e1e` |
| `b703169..8984bc9` (T4–T5) | fiabilidad, legibilidad | aprobada | WARNING, corte con menos de 1600 filas: corregido. WARNING, literal de k-means: corregido. SUGGESTION, doc de `push_codes`: corregido. WARNING, el escaneo cronometrado repite `sq8_scan`: se mantiene, porque es el código que se midió | `review-257b7856991921e1` |
| `8984bc9..d07655c` (código de T7, `hnsw_rs`, ítem 5) | riesgo, fiabilidad | corregida y aprobada | CRITICAL R1-001, `--sondeos` descartado en silencio: corregido en `9d0cf2d` y traído como `f36bc95`. WARNINGs de opciones ignoradas, `--metrica` y parseo: corregidos | `review-8333364a434ecddf` |
| `d07655c..32ad2c6` (informe, tests, registro) | fiabilidad, legibilidad | aprobada | WARNING, recall@100 plana del HNSW: corregido con una nota. SUGGESTION, etiqueta del registro de fases: se mantiene, el registro solo se amplía | `review-0ec2b05544894344` |
| `32ad2c6..1fdce49` (arreglos de los avisos) | fiabilidad, legibilidad | aprobada | Seis avisos informativos (L15): pendientes | `review-cc6339465020a82c` |
| `1fdce49..a9ce492` (arreglos de Codex) | fiabilidad, legibilidad | aprobada | Avisos informativos (L19): pendientes | `review-d0e0ca1d71bed224` |

## Tiempo

- Estimación: 3 h 30 min – 6 h. Arranque real: 2026-11-24 06:21:21 UTC. Fin: 2026-11-25 06:09:33 UTC.
- **Duración activa desde el arranque: 8 h 02 min.** Supera en 2 h la estimación alta. Las fases que más tiempo
  llevaron:
  - las corridas de aceptación: 2 h 36 min de T6 y 2 h 20 min de T7, en segundo plano;
  - las rondas de memoria que pidió Codex: 44 min de corridas en Linux y sus arreglos.
- Esperas y pausas, que el registro de fases no marcó y se midieron con las marcas de tiempo del historial de la
  sesión:
  - de 09:37:05 a 19:32:52 (9 h 55 min 47 s): la sesión perdió la conexión con la API ("Can't reach the API
    server") y siguió cuando escribiste "Dale, seguí";
  - de 00:12:10 a 06:02:15 (5 h 50 min 05 s): espera de tu respuesta sobre la cobertura de memoria.

  Las 23 h 48 min que da la herramienta, menos esas dos esperas, son las 8 h 02 min.

### Tiempo por fase

Salida de `phase_times.py report`, sin editar. Incluye las dos esperas sin marcar: 9 h 56 min en *Aceptación* y
5 h 50 min en *Correcciones de hallazgos*. Sin ellas, Aceptación activa es 5 h 07 min y Correcciones, 1 h 07 min.

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 1 min 52 s | 0 % | 1 |
| Código y tests | 28 min 50 s | 2 % | 3 |
| Checks (fmt, clippy, suite de tests) | 13 min 08 s | 1 % | 1 |
| Aceptación | 15 h 03 min | 63 % | 2 |
| Revisión RDD | 59 min 52 s | 4 % | 2 |
| Correcciones de hallazgos | 6 h 57 min | 29 % | 1 |
| Documentación | 3 min 47 s | 0 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **23 h 48 min** | **100 %** | **12** |

- Inicio: 2026-11-24 06:21:21 +00:00. Fin: 2026-11-25 06:09:33 +00:00.
- Arranque confirmado: 06:21:21. Activo desde el arranque: **23 h 48 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Código y tests | T2 downloads | 06:27:23 | 06:36:31 | 9 min 08 s |
| Código y tests | T3 ground truth | 06:40:31 | 06:48:07 | 7 min 36 s |
| Aceptación | T6 Windows and Linux runs | 06:52:03 | 08:20:26 | 1 h 28 min |
| Aceptación | T6 Linux run | 08:20:56 | 09:29:55 | 1 h 08 min |
| Aceptación | T6-T7 Windows fine sweep, HNSW and boost | 19:34:14 | 21:54:40 | 2 h 20 min |
| Revisión RDD | codex-R1 | 22:35:44 | 22:43:19 | 7 min 35 s |
| Correcciones de hallazgos | Linux memory run, dot product | 23:08:10 | 23:15:49 | 7 min 39 s |
| Revisión RDD | codex-R2 | 23:17:44 | 23:21:49 | 4 min 05 s |
| Correcciones de hallazgos | Linux memory run 2, 100k and 1M | 23:23:49 | 00:00:08 | 36 min 19 s |
| Revisión RDD | codex-R3 | 00:08:19 | 00:11:09 | 2 min 50 s |
| Revisión RDD | codex-R4 | 06:04:59 | 06:07:37 | 2 min 38 s |
| **Código y tests (total)** | | | | **16 min 44 s** |
| **Aceptación (total)** | | | | **4 h 57 min** |
| **Revisión RDD (total)** | | | | **17 min 08 s** |
| **Correcciones de hallazgos (total)** | | | | **43 min 58 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `14f7346` | feat(ann-lab): add the exact kernels and top-k of the ANN experiment | T1 | 4 min 15 s | Plan (documento de ejecución) 1 min 52 s, Código y tests 2 min 23 s |
| `b703169` | feat(ann-lab): prepare the SIFT, GloVe and Cohere data sets and their exact ground truth | T1, T2 | 14 min 46 s | Código y tests 14 min 46 s |
| `8984bc9` | feat(ann-lab): measure the approximate SQ8 scan, IVF-Flat and IVF-SQ8 against the exact search | T3 | 10 min 47 s | Código y tests 10 min 47 s |
| `d07655c` | feat(ann-lab): measure an external in-memory HNSW and sweep chosen nprobes per metric | T3, T6, T7, T6 report | 15 h 06 min | Código y tests 0 min 54 s, Aceptación 15 h 03 min, Documentación 1 min 55 s |
| `ddefad1` | docs(benchmark): report the ANN experiment and annotate design document 12 with its result | T6 report | 1 min 46 s | Documentación 1 min 46 s |
| `8e4af84` | test: list the ANN lab crate among the workspace packages and keep it out of the library | T6 report, closing checks | 13 min 09 s | Checks (fmt, clippy, suite de tests) 13 min 03 s, Documentación 0 min 06 s |
| `32ad2c6` | docs(odd): track the phase log of the ANN experiment | closing checks, RDD last slice | 2 min 00 s | Checks (fmt, clippy, suite de tests) 0 min 05 s, Revisión RDD 1 min 55 s |
| `f36bc95` | fix(ann-lab): reject an explicit nprobe above the number of lists instead of dropping it | RDD last slice | 10 min 28 s | Revisión RDD 10 min 28 s |
| `1fdce49` | fix(ann-lab): reject options a command would ignore and stale truth caches, and skip sets without the chosen metric | RDD last slice | 9 min 33 s | Revisión RDD 9 min 33 s |
| `3f1efde` | docs(odd): record the RDD review of the last slice | RDD last slice | 1 min 14 s | Revisión RDD 1 min 14 s |
| `56c24ce` | fix(ann-lab): reject --filas with preparar and --informe outside medir and grafo | RDD last slice | 16 min 52 s | Revisión RDD 16 min 52 s |
| `f5b4294` | fix(ann-lab): compute recall over the neighbours a base smaller than k has |  | 0 min 00 s |  |
| `96ee454` | feat(ann-lab): report the RSS growth of each structure on Linux and label the HNSW link size as an estimate |  | 0 min 00 s |  |
| `ffd2609` | docs(benchmark): report the measured RSS of each structure and record the Codex review resolutions | RDD last slice, Codex R1 fixes (marked late, L16) | 25 min 06 s | Revisión RDD 15 min 09 s, Correcciones de hallazgos 9 min 57 s |
| `fe18410` | feat(ann-lab): report the bytes the IVF index keeps allocated beside its payload | Codex R1 fixes (marked late, L16) | 6 min 09 s | Correcciones de hallazgos 6 min 09 s |
| `5878c31` | docs(benchmark): separate payload, allocated bytes and RSS growth, with the 1 M memory of the reference case | Codex R1 fixes (marked late, L16) | 44 min 28 s | Correcciones de hallazgos 44 min 28 s |
| `f302d58` | docs(benchmark): state that IVF is measured after SQ8 in the same process | Codex R1 fixes (marked late, L16) | 3 min 41 s | Correcciones de hallazgos 3 min 41 s |
| `a613ff1` | docs(odd): record the accepted memory coverage limit | Codex R1 fixes (marked late, L16) | 5 h 52 min | Correcciones de hallazgos 5 h 52 min |
| `a9ce492` | docs(reviews): record the approval of the fourth Codex round | Codex R1 fixes (marked late, L16), codex-R4 | 3 min 06 s | Revisión RDD 3 min 05 s, Correcciones de hallazgos 0 min 01 s |
| — | después del último commit | codex-R4 | 1 min 36 s | Revisión RDD 1 min 36 s |
| **Total** | | | | **23 h 48 min** |

Cómo leer la tabla:
- Las corridas de aceptación (T6 y T7) y las mediciones de memoria son la mayor parte del tiempo real.
- Los commits `d07655c` y `a613ff1` cargan las dos esperas sin marcar.
- Algunas marcas de fase faltan o se escribieron tarde (L16), por eso hay filas con 0 min y la etiqueta "RDD last
  slice" se arrastra a los arreglos de Codex.

## Cambios (`dfba65f..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 2.778 | 0 | — | — | 15 |
| `crates/krummi-ann-lab` | 2.728 | 0 | 2.319 | 193 | 15 |
| `odd` | 215 | 0 | — | — | 2 |
| `crates/krummi` | 9 | 3 | 5 | 4 | 2 |
| `Cargo.toml` | 1 | 1 | 1 | 0 | 1 |
| **Total** | **5.731** | **4** | **2.325** | **197** | **35** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+754 −4)

No hubo previsión de líneas. Gran parte de `docs` son las salidas sin editar del laboratorio, guardadas junto al
informe.

## Resumen

Antes de escribir el PRD 7.6, el experimento mide en memoria cuánto pierden los índices vectoriales aproximados
frente a la búsqueda exacta de Krummi, y cuánto ganan en tiempo:
- el escaneo SQ8 con reordenación;
- IVF-Flat e IVF-SQ8;
- un HNSW externo (`hnsw_rs`).

La conclusión: IVF-SQ8 cumple los umbrales del documento 12 en su caso de referencia. Dos criterios de cambio se
disparan por poco, el producto escalar y GloVe, y el HNSW externo solo arregla el primero. Las decisiones técnicas
principales:
- núcleos exactos deterministas en `f64`, con resúmenes que se comparan entre plataformas;
- el plan de energía del banco con los núcleos P en Windows;
- el RSS medido en Linux, sin código `unsafe`.

## Decisiones tomadas sin preguntar

Están en el documento de ejecución, en *Defaults taken* y en el Log (L4, L7, L8, L10, L16 y L17):
- la réplica de VectorDBBench para Cohere;
- los núcleos P y el turbo apagado en Windows;
- la asignación de IVF por L2 en el espacio de los códigos;
- `hnsw_rs` con M 16 y ef_construction 200, solo en Windows;
- el barrido fino de nprobe;
- el RSS medido solo en Linux;
- la memoria a 100 000 filas y a 1 M en el caso de referencia.

## Pendiente (no bloqueante)

- **Límite aceptado de S6.** Falta el RSS en Windows, y a 1 M el de SIFT, GloVe y el producto escalar de Cohere. Lo
  decidiste vos ("Aceptar el límite (Recomendado)", L18): el caso de referencia está medido, y lo asignado es igual en
  las dos plataformas.
- **No medido, porque es del índice almacenado de 7.6:** 10 M, filtros, deriva, caché fría, conjunto de trabajo,
  concurrencia y durabilidad. Tampoco el p95 con turbo a 1 M: por eso el producto escalar no se da por cumplido.
- **Avisos informativos de RDD que no se atendieron** (L15 y L19). Ninguno afecta las cifras medidas:
  - el mensaje de error de la verdad nombra los dos conteos;
  - `--sondeos` se comprueba por conjunto;
  - el test de `allocated_bytes` solo pone una cota inferior;
  - un invariante de longitudes en `recall` sin aserción;
  - el orden de muestreo de RSS y cronómetro;
  - dos copias de la conversión a MiB.
- **Espejo de Engram pendiente:** `mem_save` no pudo correr en esta sesión, porque un hook modifica la llamada y el
  modo automático no puede revisarla.
- **La elección del algoritmo** es una pregunta de producto del PRD 7.6.

**Riesgo:** ítem 5, por las dependencias nuevas en el workspace: `parquet` (con `zstd-sys`, que compila C) y
`hnsw_rs` (con `rayon`, `mmap-rs` y otras). Solo las usa el crate de laboratorio, y el test `bench_dependencies`
prueba que no llegan a la biblioteca.

## Limpieza manual

- `work/ann/` (9.8 GB): datos descargados, cachés y salidas, ignorados por git. Hacen falta si se repite el
  experimento.
- `~/ann` dentro de WSL (4.2 GB): copia de los datos y del `target` de Linux.
- `target/codex-ann-review-8r5f4z8h/`, `target/codex-ann-r2-4x5vg926/` y `target/codex-ann-followup-fhy6w8d0/`:
  fixtures de Codex. Sus permisos impiden leerlos desde esta sesión.
