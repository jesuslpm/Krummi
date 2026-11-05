# O3a terminada: xxh3 elegido en tiempo de ejecución, verificado y sin fusionar

Informe de cierre de O3a del [plan de rendimiento](../../docs/performance/plan.md) (2026-11-05). Enfoque de
ejecución: ODD. Fuente de producto: ninguna PRD (optimización sin cambio observable, anotación K13 del
2026-11-04). Documento de ejecución: [fast-xxh3.md](fast-xxh3.md).

Rama `perf/fast-xxh3` desde `main` `c245fee`, último commit `4f9d631` antes de este informe. Sin fusionar y sin
push.

## Resultado

- **Especificación:** S1–S3 entregadas. `krummi-pager` usa `twox-hash` 2.1.5 (`std`, `xxhash3_64`), que elige
  AVX2, SSE2 o escalar en tiempo de ejecución, en lugar de `xxhash-rust` 0.8.19, que lo fijaba al compilar (SSE2
  en el destino `x86-64` por defecto). Una sola función, `format::xxh3_64`, nombra el crate. La prueba
  `xxh3_keeps_the_values_of_the_stored_format` fija 16 valores grabados con el crate anterior. `cargo test
  --workspace`: 523 pasan, 0 fallan, 30 ignoradas.
- **Aceptación (S4):** ejecuciones de una carga de W7–W12 a escala completa, dos rondas por plataforma
  alternando los binarios, cada carga en su propia llamada (mediana op/s, antes → después, por par):

  | Carga | Windows | WSL 2 |
  |---|---|---|
  | W7 | +57 %, +28 % | +25 %, +21 % |
  | W8 | +15 %, +31 % | +29 %, +26 % |
  | W9 | +52 %, +28 % | +51 %, +25 % |
  | W10 | −11 %, −9 %; repetida sola: +4 %, +2 %, +18 % | +22 %, 0 % |
  | W11 | +7 %, +11 % | +18 %, +14 % |
  | W12 | +8 %, +8 % | +3 %, +15 % |
  | W13 | −7 %, −6 % | −3 %, +1 % |

  Las cifras absolutas están en L8 del documento de ejecución. Ninguna carga empeora más allá del ruido. Todavía no
  se cumple ningún umbral: Windows W7 queda en 726.311–763.664 op/s, contra los 1.271.825 de Voron del informe
  completo del 2026-11-03 (las dos mediciones no son comparables entre sí). Las ejecuciones completas del plan
  quedan pendientes: corren una vez tras O1, O2 y O3a.
- **Revisiones y verificación:**
  - **Verificador independiente** (sonnet/high): S1–S3 PASS, sin bloqueos. Comparó los dos crates en 25.266
    entradas con la variante automática y con AVX2, SSE2 y escalar forzadas: 0 diferencias.
  - **RDD:** cuatro revisiones, todas aprobadas.
  - **Codex:** aprobó sin hallazgos en la primera ronda.
- **Hallazgos corregidos por el camino:**
  - La prueba solo recorría la variante AVX2 del equipo. Ahora un paso de CI la corre con SSE2 y con escalar
    forzadas.
  - Ese paso pasaba en verde si no encontraba la prueba o si twox-hash quitaba un cfg. Ahora falla en los dos
    casos.
  - Ese paso corría sin `pipefail`. Ahora un checksum distinto falla en `cargo test`, con su causa.
  - La fila del plan daba W11 y W12 como ruido. Ahora indica sus mejoras.

## RDD reviews

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `c245fee..42c77f9` | risk, reliability | aprobado | R3 warning: los valores solo se fijan en la variante del equipo (AVX2); corregido en `3fdb108` con un paso de CI que fuerza SSE2 y escalar. R3 suggestion: tabla de tareas desactualizada; corregida. | `review-8ff8e3d7a05ccf2b` |
| `42c77f9..f7a055b` | resilience, readability | aprobado | R4 warning: el paso pasa si el filtro no encuentra la prueba o si falta el cfg; corregido en `8fca09f` (`--exact`, comprobación de `1 passed` y del cfg en el twox-hash bloqueado). R2 warning: la fila del plan subestimaba W11 y W12; corregida en `e4e1ff4`. R2 suggestion: el comentario de CI omitía el límite; corregido. | `review-d465ddadaa76ee5b` |
| `f7a055b..ccd0834` | resilience | aprobado | R4 warning: sin `shell: bash` no hay `pipefail`, y un checksum distinto se informaría como "did not run"; corregido en `c544ed3`. | `review-9bd2e648ba72501b` |
| `ccd0834..e85311f` | resilience | aprobado | ninguno | `review-d931b6a637d20618` |

La evaluación de `c245fee..42c77f9` dio `medium`, `under_budget`; se revisó igual por los ítems 1 y 5.

Codex: [informe](../../docs/reviews/fast-xxh3/2026-11-05-code.md),
[resoluciones](../../docs/reviews/fast-xxh3/2026-11-05-code-resolutions.md), `approved`, sin hallazgos.

## Reuse

Se reutilizó el `twox-hash` 2.1.5 que ya estaba en el lock (vía `lz4_flex`), así que no se agregó ningún
paquete al grafo. Las nueve llamadas existentes conservan su nombre `xxh3_64` y sus entradas: ahora importan la
función de `format.rs` en lugar del crate.

## Time

- Estimación: 1 h 30 min – 2 h 30 min. Inicio real: 2026-11-05 12:56:19 UTC. Fin: 13:41:46 UTC.
- **Duración activa desde el arranque: 45 min 27 s**, la mitad del mínimo estimado. El código llevó menos de
  5 minutos; el tiempo se fue en las revisiones (RDD y Codex) y en las correcciones del paso de CI.
- Esperas por el usuario: 12:56:04–12:56:19 (15 s), estimación y arranque.

### Time by phase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 0 min 22 s | 1 % | 1 |
| Plan (documento de ejecución) | 2 min 07 s | 5 % | 1 |
| Código y tests | 2 min 45 s | 6 % | 2 |
| Checks (fmt, clippy, suite de tests) | 3 min 01 s | 7 % | 1 |
| Aceptación | 0 min 15 s | 1 % | 1 |
| Verificación independiente | 6 min 46 s | 15 % | 1 |
| Revisión RDD | 12 min 40 s | 28 % | 4 |
| Correcciones de hallazgos | 17 min 32 s | 38 % | 3 |
| Documentación | 0 min 05 s | 0 % | 1 |
| Cierre (informe y merge) | 0 min 16 s | 1 % | 1 |
| **Total activo** | **45 min 49 s** | **100 %** | **16** |

- Inicio: 2026-11-05 12:55:42 +00:00. Fin: 2026-11-05 13:41:46 +00:00.
- Arranque confirmado: 12:56:19. Activo desde el arranque: **45 min 27 s** (compárese con la estimación); antes del arranque: 0 min 22 s.
- Esperas por el usuario (fuera del activo): 0 min 15 s.
  - 12:56:04–12:56:19 (0 min 15 s): estimate and start of O3a

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Checks (fmt, clippy, suite de tests) | T2 workspace tests | 13:01:26 | 13:10:46 | 9 min 20 s |
| Aceptación | T3 windows | 13:10:58 | 13:18:39 | 7 min 41 s |
| Aceptación | T3 wsl | 13:18:39 | 13:27:47 | 9 min 08 s |
| Revisión RDD | codex-R1 | 13:32:37 | 13:41:17 | 8 min 40 s |
| **Checks (fmt, clippy, suite de tests) (total)** | | | | **9 min 20 s** |
| **Aceptación (total)** | | | | **16 min 49 s** |
| **Revisión RDD (total)** | | | | **8 min 40 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `f034f8f` | test(pager): pin xxh3-64 to the values of the stored format | T1 | 3 min 09 s | Exploración y mapa 0 min 22 s, Plan (documento de ejecución) 2 min 07 s, Código y tests 0 min 40 s |
| `8c80d79` | perf(pager): pick the xxh3 SIMD variant at run time with twox-hash | T1, T2 | 1 min 56 s | Código y tests 1 min 56 s |
| `42c77f9` | docs: record the O3a verification | T2, T4, T3 | 10 min 11 s | Código y tests 0 min 09 s, Checks (fmt, clippy, suite de tests) 3 min 01 s, Aceptación 0 min 15 s, Verificación independiente 6 min 46 s |
| `3fdb108` | ci: check the checksum values with the SSE2 and scalar xxh3 variants | T4 | 17 min 51 s | Revisión RDD 1 min 29 s, Correcciones de hallazgos 16 min 22 s |
| `f7a055b` | docs: record the O3a measurements and RDD review | T4 | 0 min 32 s | Correcciones de hallazgos 0 min 32 s |
| `8fca09f` | ci: fail the xxh3 variant step when its test or twox-hash's cfg is missing | T3, T4 | 1 min 21 s | Revisión RDD 1 min 16 s, Documentación 0 min 05 s |
| `e4e1ff4` | docs: state the W11 and W12 gains of O3a in the plan |  | 0 min 00 s |  |
| `ccd0834` | docs: record the second O3a RDD review | T4 | 0 min 11 s | Correcciones de hallazgos 0 min 11 s |
| `c544ed3` | ci: run the xxh3 variant step with pipefail | T4 | 0 min 56 s | Revisión RDD 0 min 35 s, Correcciones de hallazgos 0 min 21 s |
| `e85311f` | docs: record the third O3a RDD review |  | 0 min 00 s |  |
| `4f9d631` | docs: record the O3a Codex review | T4 | 9 min 42 s | Revisión RDD 9 min 20 s, Correcciones de hallazgos 0 min 06 s, Cierre (informe y merge) 0 min 16 s |
| — | después del último commit |  | 0 min 00 s |  |
| **Total** | | | **45 min 49 s** | |

Lectura de la tabla:
- Las correcciones (17 min 32 s) incluyen el tiempo que el primer plano esperó a las mediciones en segundo plano,
  desde que se registró el cambio de CI hasta que terminó la ronda de WSL a las 13:27.
- La revisión RDD (12 min 40 s) incluye la espera a Codex, de 8 min 40 s.
- La fase de aceptación del primer plano solo registra el lanzamiento: las mediciones corrieron en segundo plano.

## Change report (`c245fee..4f9d631`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `odd` | 134 | 0 | — | — | 2 |
| `docs` | 87 | 0 | — | — | 3 |
| `crates/krummi-pager` | 44 | 5 | 34 | 7 | 3 |
| `.github` | 23 | 0 | 15 | 7 | 1 |
| **Total** | **288** | **5** | **49** | **14** | **9** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+1 −1)

Sin previsión de líneas.

## Summary

El motor verifica una suma xxh3-64 en cada página que lee y la sella en cada página que escribe. Hasta ahora esa
suma corría con instrucciones SSE2 aunque la CPU tuviera AVX2, porque el crate elegía la variante al compilar.
Con `twox-hash` la elección se hace al arrancar, así que las cargas de lectura ganan entre un 21 % y un 57 % en
W7–W9, en Windows y en Linux. Las sumas no cambian: son parte del formato guardado, y una prueba y un paso de CI
lo fijan para las tres variantes.

## Defaults chosen without asking

- Se tomó el aviso de RDD sobre las variantes SSE2 y escalar y se agregó el paso de CI, aunque el verificador lo
  había dejado como opcional. Motivo: el checksum es formato persistido.
- El paso de CI corre solo en Linux: el código del hash es el mismo en los dos sistemas.

## Pending (non-blocking)

- Las ejecuciones completas del plan tras O1, O2 y O3a. Antes de aceptarlas hay que revisar el efecto de Windows
  sobre las cargas que siguen a W9 (L7 de [scratch-lock.md](scratch-lock.md)).
- O3: escribir su PRD (verificar cada página una vez por apertura). Es lo que puede acercar W7 al umbral.
- El paso de CI se apoya en cfgs internos de twox-hash. Si una versión futura los quita, el paso falla con un
  mensaje claro, pero entonces habrá que buscar otra forma de forzar las variantes.

**Risk:** ítem 1 (el checksum es parte del formato guardado; fijado por la prueba de valores grabados y el paso de
CI) e ítem 5 (cambia una dependencia y un paso de CI).

## Manual cleanup

- `work/o3a` en el repositorio (279 MB): binarios, tars y almacenes de medición.
- `target/xxh3-sse2` (248 MB) y `target/xxh3-scalar` (228 MB): compilaciones de la prueba local del paso de CI.
- En WSL: `~/o3a` (270 MB), `~/krummi-o3a-before` y `~/krummi-o3a-after` (7,1 MB cada una). `~/krummi-wsl-target` sigue sirviendo.
