# Árboles de tamaño fijo terminados: verificados y revisados por RDD; fusionados sin la revisión de Codex (sin cupo hasta el 4 de noviembre)

Informe de cierre de `trees-fixed-size` (2026-11-02). Enfoque de ejecución: ODD.
Fuente de producto: [trees-fixed-size.prd.md](../../docs/prd/krummi/trees-fixed-size.prd.md) (5.5). Documento de
ejecución: [trees-fixed-size.md](trees-fixed-size.md). Medición: [trees-fixed-size-measurements.md](trees-fixed-size-measurements.md).

Rama `feat/trees-fixed-size`, sin merge ni push: queda para que la revises.

## Resultado

- **Especificación:** AC-X1 a AC-X20 entregados y cubiertos por tests (S1–S20 del documento de ejecución). AC-X21 es
  este informe con la medición de X13 (abajo).
  - `cargo test --workspace --no-fail-fast` en `457d135`: 34 suites, 462 tests pasados, 0 fallos, 20 ignorados
    (L17).
  - `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings`: limpios.
  - Barridos ignorados de la CI, en release: `--lib -- --ignored ac_b18 ac_r12 ac_v8 ac_m14 ac_x12` pasa 5 (1,9 s) y
    `tree_integrity -- --ignored --skip ac_x13` pasa 8 (2.989 s); el de todos los bytes de AC-X13 pasó en T3 (3.390 s)
    (L10, L21).
- **Medición (X13, AC-X21):** en Windows y en Linux (WSL 2), con 1.000.000 de claves aleatorias, el árbol de tamaño
  fijo frente a uno normal con la clave en 8 bytes big-endian (resumen abajo, datos completos en la
  [medición](trees-fixed-size-measurements.md)).
- **Revisiones y verificación:**
  - Seis revisiones RDD aprobadas (tabla abajo). Otra paró antes de empezar por tamaño
    (`lens_context_budget_exceeded`) y se repitió partida en dos.
  - Verificador independiente (ODD Verify): S1–S20 pasan y no hay bloqueos (L16). Además de los tests, probó a mano
    el ejemplo de *Salida* del PRD, la precedencia de X1 y X2, todos los intervalos vacíos de X7 contra un modelo y
    que los árboles normales y multivalor dan lo mismo que en `2125833`, con el archivo de datos idéntico salvo la
    cabecera.
  - **Codex: no se ejecutó.** Los dos intentos (12:06 y 12:35 UTC) acabaron en `error` porque se agotó el cupo de
    uso de Codex ("You’ve hit your usage limit"), sin informe. Vuelve a estar disponible el 2026-11-04 a las 06:48
    (L19). No es una aprobación.
- **Hallazgos corregidos durante el trabajo:**
  - RDD 1, R3-002: faltaba un test de que un recorrido se para en un separador sin leer una hoja dañada fuera de
    sus límites. Lo cubre AC-X10 desde `90b7bb1`.
  - RDD 2: redacción de la documentación de `tree_kind` y nombres de operaciones en español en un módulo de tests
    (`541325e`).
  - RDD 4: faltaba el `--` en la línea de uso de la medición, y sus sumas de comprobación no se comparaban con las
    entradas guardadas (`457d135`).
  - Copia duplicada de `scan_fails_at` en `multi_trees.rs` tras moverla a `support` (`90b7bb1`).
  - CI: no ejecutaba el barrido T11 nuevo y habría ejecutado en debug el de todos los bytes de AC-X13 (`9198c8b`,
    L18).

## RDD reviews

| Commits | Lentes | Resultado | Hallazgos y resolución | Lineage |
|---|---|---|---|---|
| `a97017e..d51b613` (T1, elevada con el item 3) | risk, reliability | aprobado | Risk: ninguno. R3-001 (SUGGESTION): una raíz que se divide en el nivel 254 crearía una raíz ilegible; no atendido, el árbol normal tiene el mismo límite y con 510 hijos por rama no se alcanza. R3-002 (SUGGESTION): resuelto en T3 con un test de AC-X10. | `review-c1d9c0da0a7812cb` |
| `d51b613..21f7cbd` (T2 entero) | risk, reliability, readability | parado: `lens_context_budget_exceeded` (3.265 líneas), sin autoridad creada | — | — |
| `d51b613..e279b64` (T2, implementación) | risk, reliability, readability | aprobado | Cuatro WARNING de tests que faltan (R2-dangling-acx16-reference, R3-fixed-public-coverage, R2-unused-fixed-helpers, R3-catalog-fixed-decode-untested): están en el commit siguiente, `21f7cbd`. R3-fixed-module-outside-candidate (SUGGESTION): el motor se revisó en la primera fila. R2-tree-kind-doc-wording y R2-mixed-language-op-names (SUGGESTION): corregidos en `541325e`. | `review-11144303a4987ddc` |
| `e279b64..21f7cbd` (T2, tests) | reliability, readability | aprobado | R2-001, R3-brittle-send-diagnostics y R3-toolchain-help-text (SUGGESTION): los `.stderr` de trybuild fijan campos privados y textos de ayuda de rustc; no atendidos, el resto de la suite de compilación los usa igual. | `review-b629f1b1fc81bb2d` |
| `21f7cbd..541325e` (T3, medición y documentos) | reliability, readability | aprobado | R2-001 (WARNING, `--` en la línea de uso) y R3-001 (SUGGESTION, comprobar lo leído): corregidos en `457d135`. R2-002 (SUGGESTION, nombres del PRD en comentarios de tests): no atendido, los tests existentes hacen lo mismo. R2-003 (SUGGESTION, estado de T4 sin actualizar): ya actualizado. | `review-eabfe89bce05b27f` |
| `541325e..c88ff58` (arreglo de la medición, CI y registro) | resilience, reliability | aprobado | R4-acx13-release-loses-debug-asserts (SUGGESTION): corregido en `d5a378e`. R3-002 (SUGGESTION, orden de las sumas): comentario añadido. R4-acx13-timeout-headroom y R3-001 (SUGGESTION, margen de 120 min sin medir en la CI): pendientes, solo se ven tras un push. | `review-8b8eba8c9f33c8d1` |
| `c88ff58..d5a378e` (CI) | resilience | aprobado | R4-ac-x13-timeout-headroom (SUGGESTION, los 56 min se midieron sin las comprobaciones de depuración): pendiente con el anterior. | `review-135e2bafabd287fe` |

Cada commit de T2 se revisó con HEAD separado en ese commit, porque la revisión toma el tramo desde la base hasta
HEAD. Después de `d5a378e` solo hay commits de documentación (el registro y este informe), que son pasivos.

## Time

- Estimación: 6 h – 9 h. Arranque real: 2026-11-02 09:10:20 +00:00. Fin: 2026-11-02 13:04:16 +00:00.
- **Duración activa desde el arranque: 3 h 53 min**, 2 h 07 min por debajo del mínimo estimado. Como en las
  ejecuciones anteriores, terminó en menos de la mitad del máximo.
  - El código y los tests (2 h 14 min) fueron tres workers delegados uno tras otro: T1 (motor), T2 (API) y T3
    (tests de robustez, el más largo por sus barridos).
  - Las revisiones RDD (1 h 15 min) fueron seis más una parada por tamaño.
  - La medición, en los dos sistemas, tardó 17 min.
  - Los barridos ignorados de la CI (50 min) corrieron en segundo plano mientras se revisaba.
- Esperas por el usuario: la confirmación de la estimación, de 08:01:21 a 09:10:20, antes del arranque. La marca de 4
  s de las 10:23:50 («T3 worker») fue un error del registro, corregido al momento: no hubo espera.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 3 min 01 s | 1 % | 1 |
| Plan (documento de ejecución) | 2 min 32 s | 1 % | 1 |
| Código y tests | 2 h 14 min | 57 % | 4 |
| Checks (fmt, clippy, suite de tests) | 4 min 00 s | 2 % | 1 |
| Aceptación | 17 min 24 s | 7 % | 1 |
| Revisión RDD | 1 h 15 min | 32 % | 5 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **3 h 56 min** | **100 %** | **14** |

- Inicio: 2026-11-02 07:58:20 +00:00. Fin: 2026-11-02 13:04:16 +00:00.
- Arranque confirmado: 09:10:20. Activo desde el arranque: **3 h 53 min** (compárese con la estimación); antes del arranque: 3 min 01 s.
- Esperas por el usuario (fuera del activo): 1 h 09 min.
  - 08:01:21–09:10:20 (1 h 08 min): estimate
  - 10:23:50–10:23:54 (0 min 04 s): T3 worker

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | x13-windows | 11:31:34 | 11:36:37 | 5 min 03 s |
| Aceptación | x13-linux | 11:37:07 | 11:47:38 | 10 min 31 s |
| Checks (fmt, clippy, suite de tests) | full-suite | 12:02:52 | 12:06:24 | 3 min 32 s |
| Revisión RDD | codex-R1 | 12:06:44 | 12:12:38 | 5 min 54 s |
| Revisión RDD | codex-R1 | 12:35:00 | 12:36:00 | 1 min 00 s |
| Checks (fmt, clippy, suite de tests) | ci-ignored | 12:13:26 | 13:03:18 | 49 min 52 s |
| **Checks (fmt, clippy, suite de tests) (total)** | | | | **53 min 24 s** |
| **Aceptación (total)** | | | | **15 min 34 s** |
| **Revisión RDD (total)** | | | | **6 min 54 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `d51b613` | feat(tree): add the fixed-size tree engine with dense nodes | T1 | 47 min 00 s | Exploración y mapa 3 min 01 s, Plan (documento de ejecución) 2 min 32 s, Código y tests 41 min 27 s |
| `e279b64` | feat(trees): open fixed-size trees with their length, add Tipo and format 9 | T1, T2 | 24 min 36 s | Código y tests 22 min 51 s, Revisión RDD 1 min 45 s |
| `21f7cbd` | test(trees): cover fixed-size trees with integration, compile and format tests | T2 | 0 min 01 s | Código y tests 0 min 01 s |
| `edfe769` | feat(examples): measure fixed-size trees against normal trees with 8-byte keys | T2, T4 | 4 min 48 s | Código y tests 2 min 32 s, Revisión RDD 2 min 16 s |
| `90b7bb1` | test(trees): cover damage, snapshots, crashes, faults and remembered checks of fixed-size trees | T4, T3 | 1 h 06 min | Código y tests 1 h 06 min |
| `1352f31` | docs: add the fixed-size trees execution document | T3 | 1 min 17 s | Código y tests 1 min 17 s |
| `c3e9a23` | docs: add the fixed-size trees measurement on Windows and Linux | T3, T4 | 17 min 19 s | Código y tests 0 min 07 s, Aceptación 17 min 12 s |
| `541325e` | docs(trees): clarify the failures of tree_kind and name the fixed-size operations by their API | T4, T2 | 3 min 36 s | Aceptación 0 min 12 s, Revisión RDD 3 min 24 s |
| `457d135` | fix(examples): check what the fixed-size measurement reads against the stored entries | T2 | 2 min 40 s | Revisión RDD 2 min 40 s |
| `f7acb34` | docs: record the reviews, the verification and the checks of fixed-size trees | T2, T5 | 11 min 36 s | Checks (fmt, clippy, suite de tests) 3 min 54 s, Revisión RDD 7 min 42 s |
| `9198c8b` | ci: run the fixed-size tree sweeps, the every-byte one apart in release | T5 | 6 min 43 s | Checks (fmt, clippy, suite de tests) 0 min 06 s, Revisión RDD 6 min 37 s |
| `c88ff58` | docs: record the CI change and the unavailable Codex review | T5 | 23 min 47 s | Revisión RDD 23 min 47 s |
| `d5a378e` | ci: keep debug assertions and overflow checks in the release AC-X13 sweep | T5 | 1 min 16 s | Revisión RDD 1 min 16 s |
| — | después del último commit | T5 | 25 min 52 s | Revisión RDD 25 min 52 s |
| **Total** | | | **3 h 56 min** | |

El reparto por commit es aproximado: las marcas de fase se hicieron al pasar a la tarea siguiente, a veces antes del
commit de la anterior. Por ejemplo, `90b7bb1` incluye la espera al worker de T3, y `c88ff58` y lo posterior, las
revisiones RDD y los barridos en segundo plano.

## Change report (`a97017e..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 4.325 | 178 | 2.777 | 447 | 28 |
| `crates/krummi-tree` | 2.076 | 40 | 1.717 | 235 | 5 |
| `odd` | 316 | 0 | — | — | 2 |
| `crates/krummi-pager` | 23 | 14 | 14 | 9 | 4 |
| `.github` | 15 | 5 | 9 | 6 | 1 |
| `README.md` | 13 | 5 | — | — | 1 |
| `crates/krummi-tx` | 2 | 2 | 1 | 1 | 1 |
| **Total** | **6.770** | **244** | **4.518** | **698** | **42** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `crates/krummi/tests/data/store-5.4/krummi.data` (+- −-)

Previsión al crear el documento: unas 5.100 líneas (T1 1.500, T2 2.000, T3 1.200, T4 400). Se añadieron 6.770,
el 33 % más, sobre todo en tests (los tests del motor, de la API y de robustez suman unas 3.600). Este
informe y el registro de fases se añaden en el commit de cierre, fuera de la tabla.

## Summary

Una aplicación puede guardar entradas indexadas por un número, como un identificador, una marca de tiempo o un número
de página, en un árbol cuyas claves son enteros sin signo de 64 bits en orden numérico y cuyos valores tienen todos la
longitud fijada al crearlo, de 0 a 2.048 bytes. No tiene que convertir el número en bytes ordenables. Tiene Guardar,
Añadir, Leer, Quitar, el número de entradas, recorridos por los dos extremos con límites enteros y Quitar intervalo.
Hay además una operación nueva del catálogo, Tipo, que da el tipo de cualquier árbol con su longitud.

El árbol tiene su propio formato de nodo, denso: cada entrada ocupa solo sus 8 bytes de clave y su valor, frente a
14 + N bytes en un árbol normal. Usa la misma comprobación recordada que los demás nodos y comprueba el tipo de nodo
en cada lectura, porque la memoria de nodos recordados es común a los dos formatos.

El formato pasa a ser el `9`: la entrada del catálogo de estos árboles guarda su longitud. Los almacenes de formato
`8` se rechazan como formato anterior, y se conserva uno como dato de prueba.

Medido con un millón de claves:

- Usa un 43 % menos de páginas que el árbol normal con valores vacíos, un 27–31 % menos con valores de 8 bytes y un
  5 % menos con valores de 100.
- En Windows carga en orden 6,8, 3,2 y 1,6 veces más rápido, y al azar 1,7, 1,8 y 1,1 veces más rápido. En Linux
  domina la confirmación durable, y la ganancia es menor (×1,8, ×1,5 y nada en orden).
- Leer al azar es igual o algo más rápido en los dos sistemas.

## Defaults chosen without asking

- **D1–D6 del documento de ejecución:**
  - nodo denso propio;
  - tipo de nodo comprobado tras lo recordado;
  - división por número de entradas o al final, como en los árboles normales;
  - intervalos normalizados a `[low, high]`;
  - nombres `open_fixed_tree*`, `*_fixed` y `tree_kind`;
  - `TreeKind::FixedSize(u16)` y entrada de catálogo de 27 bytes.
- **Nodos llenos por debajo de un cuarto (T1):** se cuentan los bytes, como en los árboles normales. Por eso una hoja
  con una sola entrada de 2.048 bytes nunca se fusiona: la hoja solo se libera al vaciarse.
- **Almacén de formato `8` (L4):** se creó con el código de `2125833`. Su tamaño y sus páginas libres, comparados con
  un almacén nuevo, se fijan como literales medidos (65.536 bytes, 8 páginas, 4 libres).
- **Tests de robustez (L10):** AC-X10 daña una hoja; AC-X11 cambia la altura de 1 a 2 con longitud 1.000.
- **CI (L18):** `ac_x12` se añade al filtro del paso de barridos de árboles, y el barrido de todos los bytes de AC-X13
  va en un paso propio, en release.
- **Codex (L19):** sin cupo. Se cierra sin su revisión.
- **Merge sin Codex (L23):** el 2026-11-02 a las 15:13 UTC decidiste "Fusionamos sin ella". La rama se fusiona en
  `main` por avance rápido, sin push.

## Pending (non-blocking)

- **Revisión de Codex: no se hizo.** No pudo ejecutarse por el límite de uso, y decidiste fusionar sin ella (L23).
  Si se quiere más adelante, el rango es `a97017e..d5a378e` (el código; lo posterior son documentos).
- **CI (resuelto, L24):** tras el push, la ejecución 41469704522 sobre `d68e6db` pasó en Ubuntu y Windows. El
  barrido de AC-X13 ejecutó su test (`1 passed; 17 filtered out`) en 126 s en `ubuntu-latest` y 146 s en
  `windows-latest`, muy lejos del límite de 120 min; los trabajos completos tardaron 39 y 43 min. Esto también cierra
  R4-ac-x13-timeout-headroom (RDD 6). La diferencia con los 56 min medidos en esta máquina en T3 no se investigó.
- **R3-001 (RDD 1):** una raíz que se divide en el nivel 254 crearía una raíz ilegible. No se alcanza con 510 hijos
  por rama, y el árbol normal tiene el mismo límite.
- **Catálogo vacío:** tras borrar todos los árboles, el catálogo conserva su página raíz (advertencia del
  verificador). Es código compartido que este cambio no tocó.

**Riesgo:** items 1, 3 y 5.

- **Item 1:** cambia el formato guardado (catálogo, formato `9`), como pide X12.
- **Item 3:** cambian la API pública (`TreeKind`) y la lectura común `node_at`.
- **Item 5:** cambia la CI.

Lo cubren el verificador independiente y las revisiones RDD; la de Codex no se hizo (L23).

## Manual cleanup

- `%TEMP%\krummi-x13` (binario de la medición de Windows; las carpetas de almacenes las borra la propia medición).
- En WSL: `~/krummi-x13-target` (compilación) y `~/krummi-x13-stores`, `~/krummi-x13-linux.md`.
- `%TEMP%\codex-review\20261102-*-trees-fixed-size-code` (registros de los dos intentos de Codex).
- `target/timing` y `target/slice-check` dentro del repositorio (ignorados por git; compilaciones de la medición y de
  la comprobación del commit `e279b64`).
