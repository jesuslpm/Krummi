# Proyección de columnas, conjunto de ejecuciones y suma corta terminados: 29 de 29 umbrales en Windows y Linux, rama verificada

Informe de cierre de `table-projection` (2026-11-16). Enfoque de ejecución: ODD.
Fuente de producto: [tables-projection.prd.md](../../docs/prd/krummi/tables/tables-projection.prd.md) (la proyección;
M1 y M2 son optimizaciones sin cambio de producto, sin PRD). Documento de ejecución:
[table-projection.md](table-projection.md).

Rama `feat/table-projection`, creada desde `main` en `28f9d3b`. Sin fusionar y sin publicar: queda para la revisión
del usuario.

## Resultado

- **Especificación:**
  - **S1, la proyección preparada** (PY1–PY7). Se prepara una vez por esquema y el almacén la guarda, hasta 1.024.
    Se obtienen columnas preparadas con su tipo, se liga una fila y se lee columna a columna. Las filas de versiones
    antiguas del esquema se leen con su plan, que se construye al leer la primera fila de esa versión.
  - **S2, W26 y W27 leen con la proyección.** Preparan la proyección dentro del tiempo medido;
    `KRUMMI_BENCH_PROJECTION=off` lee por posición.
  - **S3 (M1).** El almacén conserva hasta 32 MiB de búferes de ejecuciones de valores para las escrituras
    siguientes. Los entrega puestos a cero.
  - **S4 (M2).** Suma corta del banco, hasta 16 bytes, en Rust y en C#, con la misma suma de comprobación.
  - **Suite final en `3c7cb4b`:**
    - `cargo test --workspace`: 852 pasan, 0 fallan, 34 ignoradas;
    - `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings`: limpios.
- **Aceptación:**
  - Ejecuciones completas de `f4b6c2f` con Krummi y Voron, **29 de 29** umbrales en las dos:
    - Windows, fijada a los núcleos de rendimiento (1.262 s):
      [informe](../../docs/benchmark/2026-11-15-windows-projection.md);
    - WSL 2 (2.685 s): [informe](../../docs/benchmark/2026-11-15-linux-projection.md).
  - Las sumas de comprobación de los dos motores coinciden en todas las cargas.

    | Sistema | Razón de W27 con Voron |
    |---|---|
    | Windows | 0,91 → **0,96** |
    | Linux | 0,95 → **1,04** |

  - **AC-PY11:** su condición de medir cada carga sola no se cumplió en nueve cargas (L9). Por decisión del usuario
    sobre R1 de Codex, se cumple con el A/B de Krummi solo, `main` contra la rama: cinco rondas alternas, con las
    sumas de comprobación iguales (L10).

    | Carga | A/B (rama / `main`) |
    |---|---|
    | Windows W1 | 1,025 |
    | Windows W9 | 1,019 |
    | Windows W12 | 1,103 |
    | Windows W13 | 1,090 |
    | Windows W23 | 0,996 |
    | Linux W9 | 1,044 |
    | Linux W18 | 1,150 |
    | Linux W21 | 1,001 |
    | Linux W26 | 1,002 |

    La decisión está anotada en AC-PY11, en el PRD y en su traducción (L19).
  - **AC-PY12:** el almacén de W23 lo escribió `main` y lo leyó la rama, con y sin la proyección. Las cuatro
    lecturas dan las mismas sumas de comprobación (L11).
  - **Medición de la proyección** (A/B de Krummi solo, tres rondas, L8):
    - W27: 95,2 ns por fila con la proyección frente a 106,1 por posición (−10,3 %);
    - W26: 1.351 ns frente a 1.372 (−1,6 %).
- **Revisiones y verificación:**
  - Un verificador independiente por cada cambio con Risk, sin bloqueos:
    - L12, items 3 y 4: diferencial de las lecturas tipadas idéntico a `main` y concurrencia de la caché y del
      conjunto de ejecuciones;
    - L16, el arreglo de identidades.
  - RDD aprobó los seis tramos (tabla de abajo).
  - Codex aprobó en la ronda 3, la última que permite el bucle: [ronda 1](../../docs/reviews/tables-projection/2026-11-16-code.md),
    [ronda 2](../../docs/reviews/tables-projection/2026-11-16-code-02.md),
    [ronda 3](../../docs/reviews/tables-projection/2026-11-16-code-03.md) y
    [resoluciones](../../docs/reviews/tables-projection/2026-11-16-code-resolutions.md).
- **Hallazgos corregidos por el camino:**
  - **R2 de Codex (mayor).** Un estado de esquema de una escritura abandonada, o el identificador de una tabla
    borrada, podía repetirse y hacer que una proyección se ligara a otro esquema. Codex lo reprodujo forzando el
    generador. Corregido en `3a548b2`: el almacén no vuelve a sortear esos valores mientras está abierto. Lo cubre
    `ac_py5_a_repeated_identity_never_aliases_a_schema`, en RED antes del arreglo; además fallan tres mutantes.
  - **R3-001 de RDD.** Al preparar otra vez una proyección guardada después de un renombrado, sus errores nombraban
    la tabla vieja (`2277110`).
  - **Lectura lenta de W27.** El primer plan hacía que W27 leyera un 3,4 % más lento con la proyección (L8);
    `c00f2dc` pone en el plan de cada columna su tipo, su posición y su tramo fijo.
  - **Lectura de la sección fija duplicada.** La proyección y `slot_bytes` leían la sección fija cada uno por su
    lado: ahora usan `FixedSlot` (`3d296fb`, `daccfcd`).

## Cobertura de los criterios de aceptación

| Criterio | Test o comprobación | Resultado |
|---|---|---|
| AC-PY1 | `ac_py1_each_prepared_column_reads_as_by_its_position` | PASS |
| AC-PY2 | `ac_py2_older_rows_read_their_defaults_and_nulls` (también la `z` fija con nulos, R3-002), `ac_py2_a_row_of_a_version_missing_from_the_history_fails_before_binding` | PASS |
| AC-PY3 | `ac_py3_preparing_fails_with_the_first_rule_in_order` | PASS |
| AC-PY4 | `ac_py4_a_prepared_column_of_another_type_fails` | PASS |
| AC-PY5 | `ac_py5_rows_of_another_table_store_or_schema_do_not_bind`, `ac_py5_a_repeated_identity_never_aliases_a_schema` | PASS |
| AC-PY6 | `ac_py6_a_column_of_another_projection_panics` | PASS |
| AC-PY7 | `ac_py7_the_store_keeps_and_shares_a_projection` | PASS |
| AC-PY8 | `ac_py8_the_store_keeps_at_most_1024_projections` | PASS |
| AC-PY9 | `ac_py9_a_text_that_is_not_utf8_panics_only_when_read_as_text` | PASS |
| AC-PY10 | `ac_py10_a_copied_row_reads_its_own_copies` | PASS |
| AC-PY11 | Ejecuciones completas (L9) y A/B `main`/rama (L10) | PASS por decisión del usuario (anotación del 2026-11-16) |
| AC-PY12 | `ac_py12_a_reopened_store_reads_with_projections` y el diferencial con un almacén de `main` (L11) | PASS |
| AC-PY13 | Anotaciones en `a7043fc` | PASS |
| S2 (PY7) | `w26_and_w27_read_with_a_projection` | PASS |
| S3 (M1) | `run_pool_tests`, 4 de 4; mutación sin puesta a cero falla | PASS |
| S4 (M2) | `the_short_byte_sum_equals_the_sum_of_every_byte`; `selftest` de Voron `byteSum(0..33 bytes) ok` | PASS |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Lineage |
|---|---|---|---|---|
| `a7043fc..4774ba4` | risk, readability, reliability | aprobada | R2-001 (W, T3 y T4 figuraban pendientes): actualizado. R2-002 (S, `key` con dos sentidos): renombrado `list`. R2-003 (S, columnas del banco por índice): por nombre. R3-001 (W, error con el nombre viejo tras renombrar): corregido. Todo en `2277110` | `review-4976a6e6cdfe7836` |
| `4774ba4..704d18b` | readability, reliability | aprobada | R2-001 (W) y R2-002 (S), lectura fija duplicada y tupla sin nombre: `FixedSlot` en `3d296fb`. R3-002 (S, sin prueba de una columna fija con nulos): AC-PY2 ampliado. R2-003 (W) y R3-001 (S), «Krummi: desconocido» en el informe de Linux: se mantiene (WSL compila desde `git archive`) | `review-0f3dc3663cbdd4a8` |
| `704d18b..3d296fb` | readability, reliability | aprobada | R2-001 (S, brazo `Fixed` inalcanzable en `slot_bytes`): corregido en `daccfcd` | `review-ba947c7922df2d79` |
| `3d296fb..605dd96` | risk, reliability, readability | aprobada | R2-001 (W, el comentario del caso borrado prometía más) y R3 (W, sin caso de creación abandonada): corregidos en `aa013e0`. R1-001 y R3 (S, los conjuntos retirados solo crecen): se mantiene, documentado. R2-002 (S, los conjuntos viven en `Projections`): se mantiene | `review-7b867378ec08eb40` |
| `605dd96..f2566fb` | readability, reliability | aprobada | R2-001 (W, identidades esperadas sin explicar) y R3-001 (S, sin aserción de las identidades de la creación abandonada): corregidos en `3c7cb4b` | `review-8aaa0b251c91e621` |
| `f2566fb..3c7cb4b` | readability | aprobada | ninguno | `review-dc4887c834fb0ed3` |

Evaluaciones sin revisión: `0175298` (T1) y `bee2dcb` (T2), `medium`, `under_budget`. Quedaron cubiertas por el primer
tramo.

## Reutilización

- Las lecturas tipadas de `TableRow` usan los mismos decodificadores `ReadAs` que la proyección. Un verificador
  comprobó con un diferencial que dan los mismos resultados que en `main` (L12).
- `FixedSlot` es la única lectura de la sección fija de una fila, para `RowLayout::slot_bytes` y para la proyección.
- M1 sigue el precedente de los datos de página que el motor ya conservaba hasta 32 MiB por almacén (L2).
- El arreglo de R2 reutiliza `draw_identity` y su generador; solo añade a cada comprobación de valor conocido el
  conjunto de valores retirados.

## Tiempo

- Estimación: entre 7 h y 11 h. Inicio real: 2026-11-15 17:51:53 +00:00. Fin: 2026-11-16 08:08:24 +00:00.
- **Activo desde el arranque: 4 h 44 min**, por debajo de la estimación. Como en las ejecuciones anteriores, el run tardó
  cerca de la mitad de lo estimado. Las fases más largas fueron código y tests y las revisiones (RDD y Codex). Las
  ejecuciones de aceptación (2 h 06 min) corrieron en segundo plano.
- Esperas por el usuario: una, 07:27:44–07:52:43 (24 min 59 s), la pregunta sobre R1.
- **Pausa no activa de 9 h 06 min** (2026-11-15 21:14:57 – 2026-11-16 06:21:09). Un comando en segundo plano quedó
  esperando la entrada estándar (un `cat` sin entrada) y la sesión se detuvo hasta el siguiente mensaje del usuario. Las
  horas salen del historial de la sesión y se añadieron al registro de fases como `end` y `mark`. Sin ellas, el
  registro contaba esas horas como correcciones.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 3 min 32 s | 1 % | 1 |
| Código y tests | 2 h 43 min | 57 % | 3 |
| Checks (fmt, clippy, suite de tests) | 25 min 57 s | 9 % | 3 |
| Verificación independiente | 4 min 32 s | 2 % | 2 |
| Revisión RDD | 51 min 37 s | 18 % | 9 |
| Correcciones de hallazgos | 33 min 44 s | 12 % | 4 |
| Cierre (informe y merge) | 2 min 05 s | 1 % | 3 |
| **Total activo** | **4 h 44 min** | **100 %** | **25** |

- Inicio: 2026-11-15 17:51:53 +00:00. Fin: 2026-11-16 08:08:24 +00:00.
- Arranque confirmado: 17:51:53. Activo desde el arranque: **4 h 44 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 24 min 59 s.
  - 07:27:44–07:52:43 (24 min 59 s): R1 question to the user
- Pausas entre sesiones (fuera del activo): 9 h 06 min.
  - 2026-11-15 21:14:57 +00:00 – 2026-11-16 06:21:09 +00:00 (9 h 06 min)
  - 2026-11-16 08:06:19 +00:00 – 2026-11-16 08:07:02 +00:00 (0 min 43 s)

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | rdd-1 | 18:28:43 | 18:29:57 | 1 min 14 s |
| Aceptación | ab w26 w27 | 18:29:33 | 18:32:56 | 3 min 23 s |
| Aceptación | ab2 opt | 18:34:48 | 18:36:03 | 1 min 15 s |
| Aceptación | full windows | 18:36:47 | 18:57:50 | 21 min 03 s |
| Aceptación | full wsl | 18:57:50 | 19:43:29 | 45 min 39 s |
| Aceptación | single loads windows | 19:45:27 | 19:51:37 | 6 min 10 s |
| Aceptación | single loads wsl | 19:51:37 | 20:15:20 | 23 min 43 s |
| Aceptación | ab main-head windows | 20:17:59 | 20:24:34 | 6 min 35 s |
| Aceptación | ab main-head wsl | 20:24:34 | 20:43:23 | 18 min 49 s |
| Revisión RDD | rdd3 | 06:31:15 | 06:31:37 | 0 min 22 s |
| Revisión RDD | codex-R1 | 06:39:19 | 06:50:09 | 10 min 50 s |
| Revisión RDD | codex-R2 | 07:13:25 | 07:18:23 | 4 min 58 s |
| Revisión RDD | rdd4 | 07:19:10 | 07:19:58 | 0 min 48 s |
| Revisión RDD | codex-R3 | 07:53:26 | 07:56:18 | 2 min 52 s |
| Revisión RDD | rdd5 | 07:56:50 | 07:57:19 | 0 min 29 s |
| Revisión RDD | rdd6 | 08:05:51 | 08:06:10 | 0 min 19 s |
| **Aceptación (total)** | | | | **2 h 06 min** |
| **Revisión RDD (total)** | | | | **21 min 52 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `a7043fc` | docs(tables): add the prepared column projection PRD and the performance diagnostic |  | 0 min 06 s | Exploración y mapa 0 min 06 s |
| `0175298` | perf(bench): sum up to 16 bytes on a short path in both engines | T1 | 7 min 19 s | Exploración y mapa 3 min 26 s, Código y tests 3 min 53 s |
| `bee2dcb` | perf(tx): keep the buffers of committed value runs for the next runs | T1 | 5 min 13 s | Código y tests 5 min 13 s |
| `9c9cba6` | feat(tables): prepare column projections kept by the store | T1, T3 | 22 min 58 s | Código y tests 22 min 58 s |
| `5b17ff0` | perf(bench): read the rows of W26 and W27 with a projection | T3 | 0 min 26 s | Código y tests 0 min 26 s |
| `4774ba4` | docs(odd): add the table projection execution document |  | 0 min 00 s |  |
| `2277110` | fix(tables): name the table of each preparation in a projection's errors | T3, RDD a7043fc..4774ba4 | 3 min 51 s | Código y tests 0 min 04 s, Revisión RDD 3 min 47 s |
| `c00f2dc` | perf(tables): plan each projected column with its type and its fixed slot | RDD a7043fc..4774ba4, T4 | 4 min 36 s | Código y tests 2 min 59 s, Revisión RDD 1 min 37 s |
| `f4b6c2f` | docs(odd): record the projection's measurements before the full runs | T4 | 0 min 25 s | Código y tests 0 min 25 s |
| `704d18b` | docs(bench): add the full runs with the table projection | T4, AC-PY12 differential | 2 h 07 min | Código y tests 2 h 07 min, Verificación independiente 0 min 12 s |
| `3d296fb` | refactor(tables): read fixed-width columns through one FixedSlot | AC-PY12 differential, final checks, RDD last slice, RDD last slice advisories, resume after the stalled background command | 40 min 14 s | Checks (fmt, clippy, suite de tests) 17 min 34 s, Verificación independiente 0 min 21 s, Revisión RDD 2 min 16 s, Correcciones de hallazgos 20 min 03 s |
| `daccfcd` | refactor(tables): match a slot's place once in slot_bytes | fix checks, RDD of the FixedSlot fix, RDD R2-001 dead arm | 8 min 36 s | Checks (fmt, clippy, suite de tests) 0 min 38 s, Revisión RDD 1 min 12 s, Correcciones de hallazgos 6 min 46 s |
| `3a548b2` | fix(tables): never draw again an identity a projection may carry | RDD R2-001 dead arm, Codex code review | 29 min 49 s | Revisión RDD 29 min 43 s, Correcciones de hallazgos 0 min 06 s |
| `df72e3f` | docs(odd): record the verification of the identity fix | Codex code review, verifier of the R2 fix | 4 min 18 s | Verificación independiente 3 min 59 s, Revisión RDD 0 min 19 s |
| `605dd96` | docs(reviews): record the second round of the projection code review | Codex code review R2 | 5 min 28 s | Revisión RDD 5 min 28 s |
| `aa013e0` | test(tables): check the identities drawn when the store refuses a repeat | Codex code review R2, RDD of the last slice, RDD advisories of the identity fix | 8 min 42 s | Revisión RDD 2 min 07 s, Correcciones de hallazgos 6 min 35 s |
| `dc35e10` | docs(prd): accept the Krummi A/B for AC-PY11's single-load clause | RDD advisories of the identity fix, R1 answered: accept the A/B | 0 min 50 s | Correcciones de hallazgos 0 min 14 s, Cierre (informe y merge) 0 min 36 s |
| `f2566fb` | docs(reviews): record the approval of the projection code review | R1 answered: accept the A/B, Codex code review R3 | 3 min 12 s | Revisión RDD 3 min 05 s, Cierre (informe y merge) 0 min 07 s |
| `3c7cb4b` | test(tables): explain the draw order in the repeated-identity test | Codex code review R3, RDD of the final slice, final checks | 9 min 04 s | Checks (fmt, clippy, suite de tests) 7 min 45 s, Revisión RDD 1 min 19 s |
| — | después del último commit | RDD of the last test commit, closing report | 2 min 06 s | Revisión RDD 0 min 44 s, Cierre (informe y merge) 1 min 22 s |
| **Total** | | | **4 h 44 min** | |

## Informe de cambios (`28f9d3b..3c7cb4b`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 16.569 | 2 | 2.815 | 398 | 80 |
| `crates/krummi` | 2.392 | 121 | 2.016 | 257 | 9 |
| `crates/krummi-bench` | 254 | 51 | 175 | 27 | 4 |
| `odd` | 261 | 0 | — | — | 2 |
| `crates/krummi-tx` | 181 | 6 | 144 | 24 | 3 |
| `crates/krummi-pager` | 25 | 1 | 18 | 6 | 3 |
| `README.md` | 3 | 0 | — | — | 1 |
| **Total** | **19.685** | **181** | **5.168** | **712** | **102** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `docs/performance/2026-11-15-diagnostic/before-projection.zip` (+- −-), `docs/performance/2026-11-15-diagnostic/followup-01/projection-source.zip` (+- −-), `docs/performance/2026-11-15-diagnostic/followup-02/verified-micro-source.zip` (+- −-), `docs/performance/2026-11-15-diagnostic/followup-02/verified-source.zip` (+- −-)

`docs` incluye el diagnóstico de rendimiento copiado en `a7043fc` (fuentes y registros archivados). No había previsión
de líneas para comparar.

## Resumen

Una aplicación que lee siempre las mismas columnas de una tabla prepara una vez una proyección con sus nombres. El
almacén la guarda y la comparte entre transacciones e hilos. Cada fila se liga a la proyección y se lee con columnas
preparadas de su tipo, sin buscar por nombre en cada fila. Una fila de otra tabla, de otro almacén o de otro esquema no
se liga y falla con «esquema distinto».

Decisiones técnicas:
- El esquema se identifica por el número del almacén abierto, la identidad de la tabla y el estado del esquema.
  Mientras el almacén está abierto, nunca se vuelve a sortear una identidad que una proyección o una fila puedan
  llevar.
- Cada columna lleva en su plan el tipo, la posición y, si es de ancho fijo, su tramo y su bit de nulo.
- Las versiones antiguas se planifican al leer su primera fila.

Además, el motor reutiliza los búferes de las ejecuciones de valores (M1) y el banco suma con un camino corto las
claves y valores cortos (M2).

## Decisiones tomadas sin preguntar

- **Nombres de la interfaz y tipos marcadores** (L5, dentro de *Dejado a la implementación*):
  - `ReadTransaction::projection`, `WriteTransaction::projection`, `Projection::column::<T>`, `Projection::bind` y
    `ProjectedRow::get`;
  - los marcadores `Text`, `TextBytes`, `Bytes`, `Date`, `Time`, `Timestamp`, `Duration` y `Vector`;
  - `SchemaDifference::Projection` y `ValueRule::EmptyProjection`.
- **AC-PY11 se dio por cumplido con el A/B** durante el run (L10). Codex lo marcó (R1), se preguntó al usuario
  después del bucle y el usuario lo aceptó (L19).
- **Sin A/B del refactor `FixedSlot`**, porque el camino caliente hace el mismo trabajo (L13).
- **El arreglo de R2 retira identidades de tabla y pares (tabla, estado), no estados sueltos.** Así se respeta D6: el
  estado de una tabla nueva puede repetir el de otra (L18).
- **«Krummi: desconocido» en el informe de Linux se mantiene.** WSL compila desde `git archive` (L13).

## Pendiente (no bloqueante)

- Un búfer del conjunto de ejecuciones solo se reutiliza con la misma longitud. Afecta solo al rendimiento (L12).
- Las proyecciones preparadas en escrituras abandonadas conservan su plaza en la caché, como permite PY5 (L12).
- Leer con una proyección una fila mal formada puede terminar en un pánico de índice en vez de «fila mal formada».
  PY6 permite los dos y AC-PY9 pasa (L12).
- Los conjuntos de identidades retiradas crecen una entrada por tabla creada o borrada y por cambio de esquema
  mientras el almacén está abierto. Está documentado en `Retired` (L16, L18).
- Los conjuntos retirados viven en `Projections`, la caché del almacén, porque es el estado del almacén que ya
  alcanza cada transacción (L18).

**Risk:** item 3. Las lecturas tipadas de `TableRow`, que usa toda lectura de columnas, pasan por los
decodificadores de la proyección, y el sorteo de identidades de `create_table` y `change_schema` es común a todas las
tablas. Item 4: la caché de proyecciones y los conjuntos retirados están detrás de un mutex del almacén, los planes
antiguos en `OnceLock`, y el conjunto de ejecuciones es compartido. Los dos verificadores independientes (L12, L16)
los comprobaron sin bloqueos.

## Limpieza manual

- `work/tp/` (692 MB, ignorada por git): scripts y salidas de las mediciones, revisiones RDD y compilaciones. Las
  más grandes son `base-target/`, la compilación de `main` (610 MB), y `vb/` (32 MB).
