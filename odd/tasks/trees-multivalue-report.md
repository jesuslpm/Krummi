# Árboles multivalor (5.4) implementados y verificados, con la revisión de Codex aprobada

Informe de cierre de `trees-multivalue` (2026-10-29). Enfoque de ejecución: ODD.
Fuente de producto: [trees-multivalue.prd.md](../../docs/prd/krummi/trees-multivalue.prd.md). Documento de
ejecución: [trees-multivalue.md](trees-multivalue.md).

Rama `feat/trees-multivalue`, sin fusionar en `main` y sin push. Último commit de código `dc317ec`; el informe va en
el commit que cierra la ejecución.

## Resultado

- **Especificación:** las veinte especificaciones S1–S20 (AC-M1–AC-M20) están cubiertas. Comprobaciones finales tras
  el último arreglo, en Windows: `cargo fmt --all --check` correcto, `cargo clippy --workspace --all-targets -- -D
  warnings` limpio y `cargo test --workspace` con 408 pruebas superadas y 0 fallidas (17 ignoradas: los barridos
  completos). Los barridos ignorados de AC-M14 (fallo en cada operación de página y caídas hasta el cierre) y el de
  AC-M15 byte a byte pasaron en T3 (L10: 53,7 s, 13,9 s y 2.637 s); Codex volvió a pasar los dos de AC-M14 sobre
  `c455c3e`. S19 y S20 quedan cumplidas con este informe (sección *Medición*).
- **Aceptación:** la medición de M15 en Windows y en Linux, con las seis parejas de umbrales y una línea base de
  claves compuestas; datos completos en [trees-multivalue-measurements.md](trees-multivalue-measurements.md) y
  resumen abajo.
- **Revisiones y verificación:** cinco revisiones RDD aprobadas (tabla abajo); verificador independiente (`sonnet`,
  `high`) sin bloqueos, con S1–S18 cumplidas, un modelo aleatorio de la API pública frente a un `BTreeMap` y una
  carga de árboles normales comparada con la base `97887ef`, idéntica salvo los bytes de la versión de formato;
  revisión de Codex en dos rondas: R1 *blocker* y R2 *minor*, corregidos y verificados como resueltos en la segunda
  ([informe](../../docs/reviews/trees-multivalue/2026-10-29-code.md),
  [seguimiento](../../docs/reviews/trees-multivalue/2026-10-29-code-02.md),
  [resoluciones](../../docs/reviews/trees-multivalue/2026-10-29-code-resolutions.md)).
- **Hallazgos corregidos por el camino:**
  - El recorrido de claves y la eliminación de intervalos de claves aceptaban una entrada multivalor fuera de su hoja
    (imposible según M1) y devolvían su número de valores en lugar de daño. Salió al rehacer el caso de prueba que la
    revisión RDD 3 señaló como confundido (R3-off-leaf-case-confounded); corregido en `d605cc3`.
  - Codex R1: las cotas públicas `ReadHandle` y `WriteHandle` heredaban un `as_tree()` invocable, así que una función
    genérica obtenía el árbol normal de un árbol multivalor y `set` persistía bytes que al reabrir eran daño.
    Corregido en `4ecf488`: el método pide un token que solo el motor puede construir; caso de compilación que falla
    como regresión (RED observado).
  - Codex R2: el ejemplo de medición entraba en pánico con un `--values` que no fuera múltiplo del tamaño de conjunto;
    corregido en `dc317ec` (solo conjuntos completos).

## Medición (M15) y evaluación de los umbrales (M3)

Máquina: Windows 11 Pro 10.0.26200 y WSL 2 Ubuntu 24.04.1 (ext4) en el mismo equipo, Intel Core i9-13980HX (32
hilos), 64 GiB, NVMe Samsung MZVL22T0HBLB-00B00. 1.000.000 de valores por carga, claves y valores aleatorios de 16
bytes, escrituras de 1.000, confirmación durable. Una primera corrida de Windows se descartó: la línea base tardó
85–108 µs por Añadir frente a los 27 µs que midió 5.2 en este equipo; la repetición dio 26–34 µs.

µs por valor con la pareja elegida, 2.048:1.024, frente a la línea base (árbol normal con clave‖valor):

| Carga | Añadir, Linux | Añadir, Windows | Contiene, Linux | Contiene, Windows | Páginas en uso |
|---|---|---|---|---|---|
| n1 | 48,1 / 44,3 | 37,2 / 33,6 | 4,4 / 4,0 | 4,5 / 4,2 | 7.729 / 6.964 |
| n10 | 42,1 / 42,7 | 28,1 / 29,7 | 3,5 / 3,9 | 3,6 / 4,4 | 3.806 / 6.835 |
| n100 | 40,3 / 40,8 | 23,4 / 28,9 | 3,1 / 3,8 | 3,4 / 4,3 | 3.442 / 7.022 |
| n1000 | 42,9 / 40,1 | 30,7 / 27,4 | 5,0 / 3,8 | 5,2 / 4,1 | 5.020 / 6.982 |
| n10000 | 43,6 / 40,1 | 30,9 / 26,8 | 5,1 / 3,7 | 5,5 / 4,2 | 3.440 / 7.110 |
| n100000 | 42,0 / 40,1 | 29,2 / 26,3 | 5,2 / 3,9 | 5,4 / 4,1 | 4.167 / 6.977 |
| n1000000 | 43,3 / 38,1 | 29,8 / 25,9 | 5,5 / 4,0 | 5,6 / 4,1 | 4.028 / 7.128 |
| zipf | 43,8 / 40,4 | 32,8 / 27,9 | 5,3 / 3,8 | 5,7 / 4,1 | 4.400 / 7.059 |

El árbol multivalor ocupa un 28–52 % menos de páginas en todas las cargas salvo n1 (+11 %). Mientras los valores
de una clave están en su entrada, Contiene es un 10–21 % más rápido; en un subárbol lee una página más y cuesta un
27–40 % más. Añadir cuesta lo mismo o menos con valores en la entrada y un 5–18 % más con subárboles. Solo el
multivalor cuenta los valores de una clave sin leerlos.

Evaluación de M3, seis parejas (512, 2.048 y 4.074 de subida; cada una con una bajada cercana, 7/8, y otra en torno
a la mitad). La complejidad es la misma para todas: son dos constantes y el código no cambia; lo que cambia es el
tamaño del blob que reescribe cada operación. 512 desperdicia espacio en claves de unos cientos de valores (n100:
10.141 páginas frente a 3.442, y +43 % en zipf). 4.074 ahorra un 5 % de páginas en zipf y un 4–6 % de Añadir allí
solo en Windows, y cuesta hasta un 70 % más por operación junto al umbral. Las dos bajadas no se distinguen en los
datos. **Elegida: 2.048:1.024**, con la bajada en la mitad por ser la más simple y la que deja más margen.

*Revisión posterior al cierre (2026-10-30, L23):* los umbrales por defecto pasan a **4.074:2.037**. Con claves y
valores más largos, 2.048 lleva a subárboles de una página casi vacía claves de unos pocos kilobytes, y 4.074 ocupa
hasta un 59 % menos y nunca es peor (*Thresholds at larger sizes* en la medición).

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `9a50bd3..8b5a7c8` (T1, motor) | risk, readability, reliability | aprobada | Avisos: setter global de umbrales (sugerencia; solo lo usa el ejemplo), choque de nombre `free_all`, máximo fijo en un mensaje, desplazamientos literales del blob, umbrales globales en pruebas, daño de blobs sin probar (avisos): corregidos o probados en T3 (L10) | `review-65f9aede008e0181` |
| `cd7b815..182e461` (T2, pruebas) | reliability, readability | aprobada | R3-value-scan-send-unproved (aviso) y cinco sugerencias: incorporadas a T3 | `review-443b1e1d58a73985` |
| `182e461..2f7b39c` (T4 ejemplo y T3) | resilience, readability, reliability | aprobada | R4-blob-order-check-dropped (aviso): decisión del usuario L9, se mantiene; R2-duplicated-multi-seen (aviso): crates de prueba distintos, se mantiene; R3-off-leaf-case-confounded (aviso): rehecho, destapó el defecto de `d605cc3`; R2-test-blob-layout-literals: corregido; R2-duplicated-scan-checker: se mantiene; R2-placeholder-commit-ref: corregido | `review-296029a3f7646019` |
| `2f7b39c..fe52cc9` (último tramo) | resilience, readability, reliability | aprobada | R2-zipf-add-claim-contradicts-linux-table (aviso) y R3-zipf-add-gain-overstated (sugerencia): corregidos en `c455c3e`; R2-tree-open-inside-timed-values-loop (aviso): documentado en la medición, sin repetirla | `review-e827cc5980447c65` |
| `fe52cc9..d5dc52a` (arreglos de Codex) | risk, reliability | aprobada | R3-set-size-truncation-no-automated-regression (sugerencia): se mantiene, el ejemplo no tiene pruebas que corra la suite; la orden de reproducción está en las resoluciones | `review-b2683ba3440bc7f9` |

`cd7b815` (API pública, unas 800 líneas, sobre todo cambios mecánicos de llamadas en pruebas) no pasó por RDD: los
tramos que lo contenían se detuvieron con `lens_context_budget_exceeded` (L6). Lo cubrieron el verificador y Codex,
y el defecto R1 de Codex estaba en ese commit.

## Tiempo

- Estimación: 5 h – 8 h. Arranque real: 2026-10-29 10:20:12 +00:00. Fin: 2026-10-29 20:28:18 +00:00.
- **Duración activa desde el arranque: 10 h 08 min**, 2 h 08 min por encima del extremo alto de la estimación. La
  causa es la medición: 4 h 51 min en segundo plano (Windows y después Linux) y la repetición de Windows (1 h 02 min),
  que el trabajo en primer plano esperó; la corrida de Windows descartada sola ocupó 3 h 25 min.
- Esperas por el usuario: ninguna.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 4 min 19 s | 1 % | 1 |
| Código y tests | 7 h 29 min | 74 % | 3 |
| Checks (fmt, clippy, suite de tests) | 1 h 16 min | 13 % | 3 |
| Revisión RDD | 1 h 16 min | 13 % | 5 |
| Correcciones de hallazgos | 1 min 50 s | 0 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **10 h 08 min** | **100 %** | **14** |

- Inicio: 2026-10-29 10:20:12 +00:00. Fin: 2026-10-29 20:28:18 +00:00.
- Arranque confirmado: 10:20:12. Activo desde el arranque: **10 h 08 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD review 1 lenses (T1) | 11:23:42 | 11:25:17 | 1 min 35 s |
| Revisión RDD | RDD review 3 lenses (T3+T4) | 14:00:16 | 14:01:18 | 1 min 02 s |
| Aceptación | M15 measurement Windows+Linux | 14:03:59 | 18:55:31 | 4 h 51 min |
| Aceptación | M15 Windows rerun | 19:02:06 | 20:04:27 | 1 h 02 min |
| Revisión RDD | RDD review 4 lenses (last slice) | 20:07:14 | 20:08:13 | 0 min 59 s |
| Revisión RDD | codex-R1 | 20:09:13 | 20:15:48 | 6 min 35 s |
| Revisión RDD | RDD review 5 lenses (codex fixes) | 20:22:12 | 20:22:47 | 0 min 35 s |
| Revisión RDD | codex-R2 | 20:23:14 | 20:27:52 | 4 min 38 s |
| **Aceptación (total)** | | | | **5 h 53 min** |
| **Revisión RDD (total)** | | | | **15 min 24 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `9a50bd3` | docs: add the multi-value trees PRD (5.4) and annotate earlier PRDs |  | 0 min 00 s |  |
| `8b5a7c8` | feat(tree): add the multi-value tree engine | T0 exploration, T1 | 21 min 17 s | Exploración y mapa 4 min 19 s, Código y tests 16 min 58 s |
| `cd7b815` | feat(api)!: add multi-value trees to the public API and bump the format to 8 | T1 assess, T2 assess | 45 min 05 s | Revisión RDD 45 min 05 s |
| `182e461` | test(api): pin multi-value trees with integration, compile and format tests | T2 assess | 8 min 22 s | Revisión RDD 8 min 22 s |
| `5a6df2b` | feat(examples): add the multi-value trees measurement | T2 assess, T3 | 2 h 19 min | Código y tests 2 h 16 min, Revisión RDD 3 min 15 s |
| `2f7b39c` | test(tree): pin damage, snapshot, crash and fault behavior of multi-value trees | T3, T3 checks | 4 min 29 s | Código y tests 0 min 12 s, Checks (fmt, clippy, suite de tests) 4 min 17 s |
| `c44fe75` | feat(examples): add a composite-key baseline and page counts to the multi-value measurement | T3 checks, T3 assess, T4 baseline and page counts | 4 min 17 s | Código y tests 1 min 40 s, Checks (fmt, clippy, suite de tests) 0 min 07 s, Revisión RDD 2 min 30 s |
| `ab0d944` | docs: plan document queries by cost from exact index counts in the draft PRD | T4 baseline and page counts | 4 h 40 min | Código y tests 4 h 40 min |
| `d605cc3` | fix(tree): report a multi-value entry off its leaf as damage in key scans | T4 baseline and page counts, T5 test edits and README | 17 min 42 s | Código y tests 13 min 21 s, Checks (fmt, clippy, suite de tests) 4 min 21 s |
| `003a703` | docs: describe multi-value trees in the README | T5 test edits and README | 0 min 01 s | Checks (fmt, clippy, suite de tests) 0 min 01 s |
| `fe52cc9` | docs(tree): fix the multi-value thresholds at 2048:1024 from the M15 measurement | T5 test edits and README | 1 h 04 min | Checks (fmt, clippy, suite de tests) 1 h 04 min |
| `c455c3e` | docs: correct the zipf add claim and note the values-scan timing in the multi-value measurement | T5 test edits and README, T5 RDD last slice | 2 min 56 s | Checks (fmt, clippy, suite de tests) 0 min 11 s, Revisión RDD 2 min 45 s |
| `4ecf488` | fix(api): keep the normal tree inside a multi-value handle out of reach of public bounds | T5 RDD last slice, codex R1 R2 | 12 min 14 s | Checks (fmt, clippy, suite de tests) 2 min 55 s, Revisión RDD 7 min 29 s, Correcciones de hallazgos 1 min 50 s |
| `dc317ec` | fix(examples): use whole sets when --values is not a multiple of the set size |  | 0 min 00 s |  |
| `c65b279` | docs: record the resolutions of the multi-value code review | codex R1 R2 | 0 min 20 s | Checks (fmt, clippy, suite de tests) 0 min 20 s |
| `d7efe4d` | docs: add the multi-value code review report | RDD codex fixes | 0 min 11 s | Revisión RDD 0 min 11 s |
| `d8b130a` | docs: log the verification, the code review fixes and review 5 of multi-value trees | RDD codex fixes | 1 min 23 s | Revisión RDD 1 min 23 s |
| `d5dc52a` | docs: record the lineage of review 5 | RDD codex fixes | 0 min 05 s | Revisión RDD 0 min 05 s |
| `4f1e083` | docs: add the multi-value code review follow-up | RDD codex fixes | 4 min 59 s | Revisión RDD 4 min 59 s |
| — | después del último commit | RDD codex fixes | 0 min 10 s | Revisión RDD 0 min 10 s |
| **Total** | | | **10 h 08 min** | |

Lectura: los dos tramos largos de *Código y tests* (2 h 16 min en `5a6df2b`, que incluye el trabajo delegado de T2 y
T3, y 4 h 40 min en `ab0d944`) y la hora de *Checks* en `fe52cc9` son sobre todo esperas de la medición en segundo
plano: el registro de fases atribuye el tiempo a la última fase marcada. Las fases de *Revisión RDD* de `cd7b815` y
`182e461` incluyen el trabajo delegado del worker de T2. *Correcciones de hallazgos* solo cuenta desde la marca;
el arreglo de R1 y R2 está repartido en *Revisión RDD* y *Checks* de `4ecf488`.

## Informe de cambios (`97887ef..4f1e083`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 5.122 | 512 | 3.536 | 505 | 66 |
| `crates/krummi-tree` | 2.145 | 18 | 1.786 | 242 | 5 |
| `docs` | 1.589 | 4 | — | — | 30 |
| `odd` | 568 | 0 | — | — | 3 |
| `README.md` | 24 | 8 | — | — | 1 |
| `crates/krummi-pager` | 17 | 11 | 10 | 7 | 4 |
| `.github` | 7 | 6 | 3 | 4 | 1 |
| `crates/krummi-tx` | 2 | 2 | 1 | 1 | 1 |
| **Total** | **9.474** | **561** | **5.336** | **759** | **111** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `crates/krummi/tests/data/store-5.3/krummi.data` (+- −-)

`docs` incluye el PRD (`9a50bd3`) y el borrador de PRD de documentos (`ab0d944`), que no son parte de la
implementación. No hubo una previsión de líneas.

## Resumen

Un árbol multivalor guarda, para cada clave, un conjunto ordenado de valores de hasta 2.048 bytes, el valor vacío
incluido, con sus operaciones propias: Añadir, Contiene, número de valores, Quitar valor, Quitar clave, recorridos de
claves (con su número de valores) y de valores, y eliminación por intervalos de claves y de valores. Es un árbol
normal (el padre) cuya entrada por clave es un blob: mientras los valores son pocos, van ordenados dentro de la
entrada; cuando un Añadir supera el umbral de subida (2.048 bytes de clave más blob; 4.074 desde L23), pasan a un
subárbol propio de esa clave, fuera del catálogo, y vuelven a la entrada por debajo del de bajada (1.024; 2.037 desde
L23). Contar los valores y recorrer
las claves lee solo el padre. Vaciar, borrar o sobrescribir el árbol y quitar claves liberan también sus subárboles.
Los dos tipos de árbol tienen aperturas y manejadores propios, de modo que usar una operación de un tipo con el otro
no compila; el catálogo registra el tipo (byte `2`) y el formato pasa a `8`.

## Valores por defecto elegidos sin preguntar

- La carga oscilante cuenta 100.000 operaciones (50.000 pares Añadir/Quitar) (L7).
- El tiempo máximo de la CI para el paso de árboles sube de 60 a 120 min por el barrido byte a byte multivalor (L10).
- `cd7b815` queda sin RDD tras `lens_context_budget_exceeded`; lo cubren el verificador y Codex (L6).
- La corrida de Windows perturbada se descartó y se repitió (L12).
- El recorrido de valores de la medición abre el árbol una vez por clave dentro del tiempo medido; se documenta en vez
  de repetir la medición (L15).
- Con un `--values` que no es múltiplo del tamaño de conjunto, la medición usa solo los conjuntos completos (Codex
  R2).
- Entre las dos bajadas candidatas, la mitad del umbral de subida (M3).

## Pendiente (no bloqueante)

- El barrido byte a byte de AC-M15 y el de fallos de AC-M14 en cada punto no se repitieron localmente tras `d605cc3`
  y `4ecf488`. `d605cc3` solo hace más estricta la detección de daño y `4ecf488` es un cambio de tipos sin efecto en
  ejecución; la CI los corre en el próximo push.
- `set_multi_thresholds` es un valor global del proceso, oculto y usado solo por la medición (D7); una prueba que lo
  llamara en paralelo afectaría a las demás.
- La medición no tiene una prueba automática del recorte a conjuntos completos (sugerencia de RDD 5).
- Avisos de RDD que se mantienen: R4-blob-order-check-dropped (decisión del usuario, L9), R2-duplicated-multi-seen y
  R2-duplicated-scan-checker.

**Riesgo:** ítems 1 y 3 (formato persistente `8` y nuevo tipo en el catálogo; cambio de la API pública
`open_tree(name)`), pedidos por M14 y D5, e ítem 2 (la barrera de tipos entre los dos tipos de árbol, reforzada por
Codex R1). Cubiertos por las pruebas de formato, de compilación y de daño, el verificador independiente y las dos
rondas de Codex.

## Limpieza manual

- WSL: `~/krummi-t5-target` (132 MiB, compilación de Linux de las mediciones) y `~/krummi-m15-stores` (vacía).
- `work/m15/` en el repositorio (ignorado por git): las salidas de las tres corridas de medición.
