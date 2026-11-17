# Caché de escrituras en orden descartada por decisión del usuario: W1 +13,5 %, pero W3 −3 %; código igual a `main`

Informe de cierre de `ordered-write-cache` (2026-11-17). Enfoque de ejecución: ODD.
Fuente de producto: ninguna. Es una optimización sin cambio de comportamiento, como S4 de
[la ejecución de optimizaciones de escritura](../pager/write-optimizations.md), y nace de su informe de cierre
(pendiente «Caché de escrituras en orden»). Documento de ejecución: [ordered-write-cache.md](ordered-write-cache.md).

Rama `feat/ordered-write-cache`, creada desde `main` (`0cb816c`). Sin fusionar y sin publicar. El código de `crates/`
es idéntico al de `main` (`bbefd28`): la rama solo añade los registros de la ejecución y la revisión de Codex.

## Resultado

- **Especificación:** S1 se implementó y se verificó. Luego se revirtió por decisión del usuario (L14):
  - **Primera versión (T1, `aef6348`):** una transacción de escritura recordaba la última hoja del árbol que alcanzaba
    un recorrido. Un cambio por clave desde su primera clave, sin división ni fusión, la cambiaba sin recorrer el árbol.
  - **Segunda versión (`b12e08b`):** la hoja se recordaba con el `LastLeaf` que `change_leaf` ya calcula cuando un alta
    va al final del árbol; `change` quedaba como en `main`.
  - Las dos olvidaban la hoja con una clave menor del mismo árbol, al reservar o liberar páginas y ante un fallo.
  - Con la caché, `cargo test --workspace` daba 873 pasados, 0 fallidos y 36 ignorados.
  - Tras la reversión, fmt y clippy salen limpios y `cargo test --workspace` da 868 pasados, 0 fallidos y 35 ignorados.
- **Aceptación (S2):** A/B de Krummi solo en Windows, `main` contra la segunda versión, con el plan de energía del banco
  y los resúmenes de datos iguales:
  - **W1 mejora:** 1,135 (más rápida en las 3 rondas).
  - **Peores:**
    - W3: 0,970, más lenta en 10 de 10 rondas.
    - W5: 0,995, más lenta en 11 de 20.
    - W7: 0,997, más lenta en 12 de 20.
  - **Iguales o mejores:** el resto de las cargas de S2. W12 y W22, que salieron peores con 10 rondas, quedaron
    igual y mejor con 20.
  - No se hizo el A/B en WSL. S2 no lo pide.
- **Ejecuciones completas (S3):** la de Windows terminó: Krummi cumple 29 de 29 umbrales
  (`work/owc/full/windows.md`). La de WSL se detuvo en W12 cuando el usuario decidió. Ninguna se guarda en
  `docs/benchmark/`, porque miden código que no se fusiona.
- **Revisiones y verificación:**
  - Dos verificadores independientes, uno por versión, aprobaron S1 sin bloqueantes:
    - Compararon contra `0cb816c` 144 y 375 recorridos al azar de resúmenes de resultados y de todos los bytes de
      cada página, en producción y depuración: todos idénticos.
    - La caché se usó 69.202 y 529.911 veces.
    - Cada mutación de los olvidos fue detectada.
  - Tres revisiones RDD aprobadas (tabla abajo).
  - Codex aprobó en la primera ronda: [informe](../../../docs/reviews/ordered-write-cache/2026-11-17-code.md) y
    [resoluciones](../../../docs/reviews/ordered-write-cache/2026-11-17-code-resolutions.md).
- **Hallazgos corregidos por el camino:**
  - **El costo de la primera versión** en las cargas al azar: el A/B de 3 rondas dio W3 0,969, W14 0,970 y W15
    0,974. Se buscó su origen con cuatro variantes (L6–L10):
    - Lo que hace la caché al ejecutarse no cuesta: la medición dentro del mismo proceso, con la caché encendida y
      apagada, da W3 −3,0 % a favor de la caché.
    - La parte de la transacción tampoco: W3 da 0,997 en `main` con solo el `transaction.rs` nuevo.
    - Forzar que `change_leaf` se expanda en línea no ayuda.
    - La segunda versión dejó W14 y W15 iguales y W1 entre +12,6 % y +14,8 %, pero W3 sigue perdiendo un 2–3,7 %
      con cualquier cambio en `update`. Lo más probable es la disposición del código compilado, que no se midió.
  - **Un comentario** que nombraba el retroceso de T4 (verificador): `77be581`.
  - **La invariante de las fusiones**, escrita en el código, y `try_add_at_last` añadido al test de equivalencia de
    tres árboles (RDD R1-001 y R3-001): `7d434e4`.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `0cb816c..41a3703` (T1, ítem 1) | riesgo, fiabilidad | aprobado | R1-001 y R3-001 WARNING (una redistribución entre hojas hermanas dejaría la hoja obsoleta): no existe, porque `merge_child` no mueve nada cuando la pareja no cabe; queda escrito en `7d434e4`, con `try_add_at_last` en el test. R1-002 SUGGESTION (`remember_leaf` público): se mantiene, porque otro crate lo llama | `review-7fb7fdf34f3669c3` |
| `41a3703..d48f747` (segunda versión, ítem 1) | riesgo, fiabilidad | aprobado | R3-001 WARNING (los tests de equivalencia no comprobarían que se usa la hoja recordada): descartado con evidencia, porque exigen más de 1.000 y de 150 aciertos (`tests.rs:1321`, `multi_tests.rs:1234`) | `review-af03f889b03835d4` |
| `d48f747..3092600` (reversión y registros) | fiabilidad | aprobado | ninguno | `review-35d081d38face02c` |

La evaluación de `d48f747..3092600` dio `under_budget`. Se revisó igual, como último tramo, antes de este informe.

## Tiempo

- **Estimación:** 4 h – 7 h. Arranque real: 2026-11-17 14:06:57. Fin de la verificación: 2026-11-17 18:31:10.
- **Duración activa desde el arranque: 4 h 24 min**, dentro de la estimación. Sin la decisión del usuario faltaban la
  ejecución completa en WSL (unos 70–100 min) y el cierre, y el run habría quedado cerca del máximo.
  - La aceptación es el 54 % del activo. En segundo plano corrieron 2 h 43 min de mediciones: siete A/B en Windows (2
    h 12 min) para aislar el costo de W3 y repetir las cargas marcadas, y las ejecuciones completas (31 min).
  - El registro cuenta la espera de esas mediciones en la última fase marcada. Por ejemplo, «Revisión RDD» de
    `bbefd28` (32 min 53 s) incluye las ejecuciones completas.
- **Esperas por el usuario:** ninguna. La decisión del usuario llegó mientras corrían las ejecuciones completas.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 1 min 29 s | 1 % | 1 |
| Código y tests | 10 min 37 s | 4 % | 1 |
| Checks (fmt, clippy, suite de tests) | 25 min 00 s | 9 % | 2 |
| Aceptación | 2 h 21 min | 54 % | 1 |
| Verificación independiente | 37 min 18 s | 14 % | 2 |
| Revisión RDD | 48 min 01 s | 18 % | 3 |
| Correcciones de hallazgos | 0 min 14 s | 0 % | 1 |
| **Total activo** | **4 h 24 min** | **100 %** | **11** |

- Inicio: 2026-11-17 14:06:57 +00:00. Fin: 2026-11-17 18:31:10 +00:00.
- Arranque confirmado: 14:06:57. Activo desde el arranque: **4 h 24 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | rdd-T1 | 14:45:50 | 14:46:59 | 1 min 09 s |
| Aceptación | ab-windows | 14:54:57 | 15:29:04 | 34 min 07 s |
| Aceptación | ab-windows-v2 | 15:33:43 | 15:46:55 | 13 min 12 s |
| Aceptación | ab-windows-v3 | 15:50:08 | 16:11:21 | 21 min 13 s |
| Aceptación | ab-windows-txo | 16:14:50 | 16:17:44 | 2 min 54 s |
| Aceptación | ab-windows-v5 | 16:21:05 | 16:25:18 | 4 min 13 s |
| Aceptación | ab-windows-final | 16:34:23 | 17:15:44 | 41 min 21 s |
| Aceptación | ab-windows-repeat | 17:30:33 | 17:45:28 | 14 min 55 s |
| Revisión RDD | rdd-b12e08b | 17:46:08 | 17:46:43 | 0 min 35 s |
| Aceptación | full-windows | 17:46:59 | 18:08:27 | 21 min 28 s |
| Aceptación | full-linux | 18:08:27 | 18:18:21 | 9 min 54 s |
| Revisión RDD | rdd-last | 18:27:22 | 18:27:54 | 0 min 32 s |
| Revisión RDD | codex-R1 | 18:28:04 | 18:30:53 | 2 min 49 s |
| **Aceptación (total)** | | | | **2 h 43 min** |
| **Revisión RDD (total)** | | | | **5 min 05 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `aef6348` | perf(tree): remember the tree's last leaf for writes whose keys come in order | T0, T1 | 21 min 20 s | Plan (documento de ejecución) 1 min 29 s, Código y tests 10 min 37 s, Checks (fmt, clippy, suite de tests) 9 min 14 s |
| `77be581` | test(tree): describe why only some changes find the remembered leaf | T1 | 15 min 32 s | Checks (fmt, clippy, suite de tests) 7 min 43 s, Verificación independiente 7 min 49 s |
| `41a3703` | docs(odd): start the ordered-write cache execution | T1 | 0 min 38 s | Verificación independiente 0 min 10 s, Revisión RDD 0 min 28 s |
| `7d434e4` | test(tree): add through try_add_at_last beside the remembered leaf and note why merges keep it valid | T1 | 10 min 12 s | Revisión RDD 10 min 12 s |
| `b12e08b` | perf(tree): remember the last leaf only after an add at the tree's end | T1, T2 | 1 h 39 min | Aceptación 1 h 39 min, Revisión RDD 0 min 14 s |
| `6836f90` | docs(odd): record the ordered-write cache A/Bs and the variant kept |  | 0 min 00 s |  |
| `d48f747` | docs(odd): record the final Windows A/B and the verifier of the last-leaf rule | T2, T1 | 1 h 11 min | Aceptación 42 min 25 s, Verificación independiente 29 min 19 s |
| `bbefd28` | revert(tree): drop the ordered-write cache | T1 | 33 min 01 s | Revisión RDD 32 min 53 s, Correcciones de hallazgos 0 min 08 s |
| `3092600` | docs(odd): record the user's decision to drop the ordered-write cache | T1 | 8 min 03 s | Checks (fmt, clippy, suite de tests) 7 min 57 s, Correcciones de hallazgos 0 min 06 s |
| — | después del último commit | T1 | 4 min 20 s | Checks (fmt, clippy, suite de tests) 0 min 06 s, Revisión RDD 4 min 14 s |
| **Total** | | | **4 h 24 min** | |

Las cuatro variantes del A/B se llevaron la mayor parte: el costo de W3 no estaba donde apuntaba cada hipótesis.

## Informe de cambios (`0cb816c..3092600`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `odd` | 180 | 0 | — | — | 2 |
| **Total** | **180** | **0** | **—** | **—** | **2** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

El código de la caché llegó a +760 líneas (T1, la mayoría tests) y se revirtió entero. Esta tabla no incluye este
informe, el archivado ni la revisión de Codex, que van en commits posteriores.

## Resumen

La caché aceleraba la carga de claves en orden (W1) un 13,5 %: la transacción recordaba la última hoja del árbol y las
altas siguientes no recorrían el árbol. Las cargas al azar pagaban un 3 % (W3), y W5 y W7 salían apenas por debajo. El
usuario decidió que la ganancia en W1 no compensa esas pérdidas, y el código de la rama volvió a ser el de `main`. Ya
son dos intentos de caché de hojas recientes (T4 de la ejecución anterior y este) descartados por su costo en W3.

## Valores por defecto elegidos sin preguntar

- **Sin PRD:** S1 no cambia ningún comportamiento observable. La regla de CLAUDE.md pide PRD solo para cambios de
  comportamiento de producto.
- **Diseño:** recordar solo la última hoja, porque W1 escribe siempre en ella (L2), en lugar de cualquier hoja, como T4.
- **Investigar el costo de W3** con variantes en lugar de dar el cambio por cerrado tras el primer A/B. Se quedó la
  segunda versión, la mejor en W1 y neutra en W14 y W15.
- **Repetir con 20 rondas** las cargas cortas marcadas (W5, W7, W12, W22) y con 10 la de W3, como en la ejecución
  anterior.
- **Revertir con `git checkout 0cb816c -- crates`** en un commit, en lugar de `git revert` de los cuatro commits: uno
  de ellos (`7d434e4`) también tocaba los registros.
- **No guardar la ejecución completa de Windows** en `docs/benchmark/`, porque mide código que no se fusiona.

## Pendiente (no bloqueante)

- **La causa del costo de W3** con cualquier cambio en `update`: no se midió. Si se quiere entender, hay que comparar
  perfiles (por ejemplo con samply) de `main` y de la segunda versión. No bloquea, porque la caché se descartó.
- **El A/B en WSL** no se hizo. No bloquea: la decisión se tomó con el de Windows.
- **Los pendientes del informe de optimizaciones de escritura** siguen como estaban: el orden de memoria del registro
  de depuración, `PageChange::new(true, true)` y la ganancia de W17 de T2.

**Riesgo:** ninguno en lo que se fusionaría: el código es el de `main`. La caché, mientras existió, tocaba el ítem 1
(una hoja recordada obsoleta habría corrompido árboles guardados) y la revisaron dos verificadores y RDD.

## Limpieza manual

- `work/wm/main-*`, `work/wm/owc-*`, `work/wm/owc2-*`, `work/wm/owc3-*`, `work/wm/owc5-*`, `work/wm/txo-*` y `work/owc`:
  unos 3,7 GB en total, las compilaciones y resultados de los A/B.
- `~/owc` en WSL: 880 MB, la ejecución completa detenida.
- Las limpiezas que dejó el informe de optimizaciones de escritura siguen pendientes (`work/wm`, `~/wm` en WSL).
