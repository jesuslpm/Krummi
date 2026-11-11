# Tablas con esquema terminadas: verificadas, revisadas por RDD y por Codex (aprobado); tres umbrales del banco incumplidos, que pasan a un cambio aparte (K13)

Informe de cierre de `tables-schema` (2026-11-11). Enfoque de ejecución: ODD.
Fuente de producto: [tables-schema.prd.md](../../docs/prd/krummi/tables-schema.prd.md) (7.1), hijo de
[tables.prd.md](../../docs/prd/krummi/tables.prd.md). Documento de ejecución: [tables-schema.md](tables-schema.md).
Diseño: [docs/design/tables](../../docs/design/tables/README.md) (documentos 01, 02, 03, 05, 07 y 11).

Rama `feat/tables-schema`, sin merge ni push: queda para que la revises.

## Resultado

- **Especificación:** AC1–AC19 del PRD y AC-N1 a AC-N6, AC-N9 y AC-N10 del padre (S1–S27 del documento de
  ejecución), en T1–T12. Las tablas con esquema (catálogo, cambios de esquema, filas, intervalos de la clave primaria,
  carga masiva atómica), las secuencias, los generadores `ulid`, `uuid` v7 y `uuid` secuencial, y el formato 11.
  - `cargo test --workspace` en `HEAD`: 783 pasados, 0 fallos, 31 ignorados.
  - `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings`: limpios.
- **Aceptación (T12, AC16, AC-N9, AC-N10):** ejecuciones completas del banco en Windows y en Linux (WSL 2), bajo el
  plan de energía del banco, sobre `d0d4ef6` (L19):
  - [Windows](../../docs/benchmark/2026-11-11-windows-tables.md): 5.981 s; Krummi cumple 26 de 29 umbrales.
  - [Linux](../../docs/benchmark/2026-11-11-linux-tables.md): 7.468 s; Krummi cumple 28 de 29 umbrales.
  - Los dos informes muestran 29 umbrales, `no aplica` en los motores que no son Voron y las cuatro cargas de tablas
    en la tabla de tamaños; las sumas de comprobación de W26 y W27 coinciden con las esperadas en Krummi y en Voron.
  - Tablas, Krummi / Voron:

    | Carga | Windows | Linux |
    |---|---|---|
    | `tabla-insercion-en-orden` (W23) | 1,17 | 1,09 |
    | `tabla-insercion-al-azar` (W24) | 0,91 | 1,01 |
    | `tabla-carga-masiva` (W25) | 0,88 | 1,06 |
    | `tabla-lecturas` (W26) | **0,75** | 0,88 |
    | `tabla-intervalos` (W27) | **0,15** | **0,19** |
    | `tabla-actualizacion` (W28) | **0,73** | 0,87 |
    | `tabla-borrado` (W29) | 2,20 | 1,91 |

  - Las 22 cargas anteriores cumplen su umbral en los dos sistemas.
  - Por AC-N9 y K13 del banco, un umbral incumplido no impide cerrar el PRD hijo: se convierte en un cambio aparte
    (*Pendiente*).
- **Mediciones de una sola carga (AC-N10):** rotaciones alternadas de `main` (`d487cde`) y la rama, dos rondas por
  carga.
  - W1–W22, rama / main: en Windows, de 0,88 a 1,06; en WSL 2, de 0,65 a 1,52. Ninguna carga queda por debajo de 0,95
    en las dos rondas en ninguno de los dos sistemas. Las desviaciones mayores se contradicen entre rondas (WSL: W16
    0,65 y 1,52; W21 0,75 y 1,20), así que son ruido y no una regresión.
  - Cargas nuevas en la rama (op/s, rondas 1 y 2):

    | Carga | Windows | WSL 2 |
    |---|---|---|
    | W23 | 378.932 / 379.545 | 123.795 / 128.987 |
    | W24 | 95.685 / 96.953 | 58.592 / 58.404 |
    | W25 | 365.188 / 365.073 | 411.004 / 389.713 |
    | W26 | 554.317 / 563.965 | 562.625 / 543.601 |
    | W27 | 1.693.464 / 1.651.835 | 1.975.145 / 1.985.901 |
    | W28 | 96.609 / 95.773 | 62.331 / 62.331 |
    | W29 | 121.300 / 119.682 | 68.396 / 69.497 |

  - Las líneas `FAILED W13` de las rotaciones son del script: W13 se mide dentro del proceso de W12 (K3) y su valor
    sale de esa ejecución.
- **Revisiones y verificación:**
  - Dieciséis revisiones RDD aprobadas y una parada antes de empezar (tabla abajo).
  - Codex, tres rondas ([informe](../../docs/reviews/tables-schema/2026-11-11-code.md),
    [ronda 2](../../docs/reviews/tables-schema/2026-11-11-code-02.md),
    [ronda 3](../../docs/reviews/tables-schema/2026-11-11-code-03.md),
    [resoluciones](../../docs/reviews/tables-schema/2026-11-11-code-resolutions.md)): la ronda 3 aprobó, sin
    hallazgos ni preguntas abiertas.
- **Hallazgos corregidos durante el trabajo:**
  - Codex R1 (mayor): un valor por defecto grande y válido hacía saltar una aserción al crear la tabla. Con la decisión
    del usuario (Q2, «Aceptar el límite»), un esquema cuya forma guardada pasa de 256 MiB falla con esquema no válido
    y la regla nueva `SchemaTooLarge`, la última de E5 (`b0288e3`, `6e4a04a`; PRD anotado en `461bccc`).
  - Codex R2 (mayor): leer filas antiguas copiaba los valores por defecto de las columnas añadidas; ahora se prestan
    desde la página del catálogo (`b0288e3`).
  - Codex R3 (menor): la norma de los vectores sumaba la cola en otro orden que el del diseño 05 (`574f7d3`).
  - RDD: avisos resueltos en la tarea siguiente a cada revisión (tabla abajo), entre ellos el texto de
    `DifferentTreeType` entre árboles, comprobaciones de daño más estrictas al decodificar, y tests de los huecos que
    señalaron las lentes de fiabilidad (T10).
  - Orquestador del banco: borraba el almacén de W23 tras su primera carga dependiente, y W26–W29 parten todas de él
    (`released`, T11).
  - El comentario XML del `VoronBench.csproj` con `--` rompía el proyecto (T11).

## RDD reviews

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `d487cde..17d8a82` (T1) | risk, readability | aprobada | 4 avisos (WARNING: el texto de `DifferentTreeType` entre árboles perdía "tree"; SUGGESTION: formato de depuración en `Display` y una coma colgante): corregidos en `7841b95` | `review-6baa1e8b06e51441` |
| `17d8a82..56c5a6b` (T2) | risk, reliability | aprobada | ninguno | `review-518dbc901dd2906a` |
| `0edd736..cab64a6` (T3) | risk, readability, reliability | aprobada | 7 avisos (WARNING: documentación de nanosegundos invertida, codificación de filas sin comprobar el tipo, dominio temporal al leer, precisión de `decimal` en la clave; SUGGESTION: duplicados y truncado de `zip`): corregidos en `7841b95` | `review-197a225f96da64f8` |
| `7841b95` (T5) | risk, reliability | aprobada | R3-001 a R3-005: los pánicos por aridad y tipo son comprobaciones deliberadas del llamador; `Display` de `ColumnType` sin test y flotantes de la fila sin comprobar al leer, corregidos al empezar T6 | `review-893e6d1ac090d520` |
| `7b32e4f` (T5) | risk, reliability, readability | aprobada | 6 avisos (WARNING: guarda de `put_table` solo en debug, comprobaciones de daño débiles al decodificar, cobertura del tipo 4 dañado; SUGGESTION: id cero silencioso, inicialización duplicada, nombres): corregidos al empezar T6; 2 más cubiertos por `520fd73` | `review-c04c39664134ff6e` |
| `7b32e4f..28ea646` (T5, tests) | reliability | aprobada | R3-001, R3-002 (WARNING: el test de G6 tomaba la página dañada antes de un commit con copia; AC13 solo miraba nombres listados): corregidos en T10 (`21d1502`) | `review-784681b0f1fdad21` |
| `28ea646..426f263` (T6) | risk, resilience, readability, reliability | aprobada | R1-001 (`count -= 1` sin comprobar con un recuento dañado), R2-001 a R2-003 (nombres, camino de inserción duplicado, `Option<bool>`), R3 (cubierto por `b535c62`): corregidos al empezar T7 | `review-1cdfcbbd17d7f560` |
| `426f263..c0056b4` (T6, tests) | reliability | aprobada | R3-001 (sin test de no nulos cuando un upsert inserta), R3-002 (varios búferes de 256 MiB en tests paralelos): corregidos en T10 | `review-e9f524372a1b69e2` |
| `0a698e5` (T9) | reliability | aprobada | R3-001 (una secuencia renombrada conserva su valor tras el commit): test en T10 | `review-a7c7be094a0ae49a` |
| `f0388bc` (T7) | reliability | aprobada | R3-scan-failure-fuse-untested: test en T10 | `review-8f963b3ed9d247e7` |
| `9d1b4aa` (T8) | reliability, resilience | aprobada | R4-source-display-opaque, R3-timeout-success-untested, R3-load-side-panic-untested: corregidos en T10 | `review-a4f42bb8d1d7e529` |
| `2266828..21d1502` (T10) | reliability | aprobada | 3 avisos: la causa duplicada en la cadena, corregida en `18e83d1`; los otros dos aceptados | `review-920afc2eea7fcd84` |
| `21d1502..12dd86b` (T11) | reliability, readability | aprobada | R2-001 (`is_table` dependía del orden de las variantes), R3 (W28 y W29 sin comprobar su estado final): corregidos en `18e83d1` | `review-7767835617d87908` |
| `12dd86b..9dcceaa` | readability, reliability | aprobada | 2 corregidos en `93bfa4d` (un solo formateador de claves en el banco; test de un error de origen con su propia causa); 2 SUGGESTION aceptados | `review-cc56ac99ac96c65f` |
| `1479c19..b0288e3` (Codex R1–R3) | risk, readability, reliability | aprobada | R1-001 y R3-fit-check-coverage descartados (`modify` solo permite nulos, que no agrandan la entrada); R2-001 resuelto con la decisión Q2; 4 SUGGESTION aceptados | `review-1b1106545ae7a5fc` |
| `6e4a04a` (Q2) | readability, reliability | aprobada | R2-stale-check-fits-doc corregido en `ed4a2bc`; R2-alter-overrides-check-fits-name y R3-002 son el comportamiento decidido, ahora documentado; R3-001 descartado | `review-b92e1a87e7a74c7a` |
| `6e4a04a..269916e` | readability | parada: `lens_context_budget_exceeded` | sin revisión: el tramo son 2.402 líneas, casi todas documentos de JSON de otra sesión (*Pendiente*) | `review-ef2991fba0da2669` |

Sin revisión, por la evaluación: `520fd73` y `b535c62` quedaron `under_budget` y se revisaron dentro de los tramos
siguientes; `ba8b5d2..b60d83c` (registros) fue `passive`.

## Time

- Estimación: 16 h – 26 h. Arranque real: 2026-11-10 21:32:39 +00:00. Fin: 2026-11-11 12:00:25 +00:00.
- **Duración activa desde el arranque: 9 h 56 min**, entre el 38 % y el 62 % de la estimación, por debajo de su
  mínimo, como en las ejecuciones anteriores (alrededor de la mitad). La aceptación se lleva 5 h 32 min (55 %): el
  banco corre en segundo plano y el primer plano lo espera.
- Esperas por el usuario: de 21:31:54 a 21:32:39 (confirmación de la estimación y del arranque) y de 01:25:24 a
  05:56:25, 4 h 31 min (preguntas de producto Q2 y Q1 tras la segunda ronda de Codex).

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 3 min 00 s | 1 % | 1 |
| Plan (documento de ejecución) | 17 min 20 s | 3 % | 1 |
| Código y tests | 1 h 57 min | 20 % | 5 |
| Checks (fmt, clippy, suite de tests) | 9 min 37 s | 2 % | 1 |
| Aceptación | 5 h 32 min | 55 % | 1 |
| Revisión RDD | 1 h 45 min | 18 % | 9 |
| Correcciones de hallazgos | 13 min 37 s | 2 % | 2 |
| Documentación | 1 min 17 s | 0 % | 1 |
| Cierre (informe y merge) | 0 min 01 s | 0 % | 1 |
| **Total activo** | **9 h 59 min** | **100 %** | **22** |

- Inicio: 2026-11-10 21:28:54 +00:00. Fin: 2026-11-11 12:00:25 +00:00.
- Arranque confirmado: 21:32:39. Activo desde el arranque: **9 h 56 min** (compárese con la estimación); antes del arranque: 3 min 00 s.
- Esperas por el usuario (fuera del activo): 4 h 31 min.
  - 21:31:54–21:32:39 (0 min 45 s): estimate and start confirmation
  - 01:25:24–05:56:25 (4 h 31 min): product questions Q2 and Q1

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD T2 | 22:05:19 | 22:05:20 | 0 min 01 s |
| Revisión RDD | RDD T2 | 22:05:48 | 22:05:50 | 0 min 02 s |
| Revisión RDD | RDD T2 | 22:05:57 | 22:06:58 | 1 min 01 s |
| Revisión RDD | codex-R1 | 00:44:26 | 00:52:21 | 7 min 55 s |
| Revisión RDD | codex-R2 | 01:18:47 | 01:24:53 | 6 min 06 s |
| Revisión RDD | codex-R3 | 06:00:16 | 06:05:40 | 5 min 24 s |
| Aceptación | rotations windows | 06:10:06 | 06:35:46 | 25 min 40 s |
| Aceptación | rotations wsl | 06:35:46 | 07:54:04 | 1 h 18 min |
| Aceptación | complete windows | 07:54:04 | 09:33:45 | 1 h 39 min |
| Aceptación | complete wsl | 09:33:45 | 11:38:18 | 2 h 04 min |
| **Aceptación (total)** | | | | **5 h 28 min** |
| **Revisión RDD (total)** | | | | **20 min 29 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `17d8a82` | feat(tables): add the table error classes, catalog classes and format 11 |  | 20 min 12 s | Exploración y mapa 3 min 00 s, Plan (documento de ejecución) 17 min 12 s |
| `56c5a6b` | feat(tables): add the OS random source and the ulid and uuid generators | T2-T4, T1, T5-prep | 15 min 20 s | Plan (documento de ejecución) 0 min 08 s, Código y tests 12 min 39 s, Revisión RDD 2 min 33 s |
| `0edd736` | feat(bench): add the Voron side and the shared data of the table loads W23-W29 | T5-prep | 2 min 03 s | Código y tests 2 min 03 s |
| `cab64a6` | feat(tables): add column values, the primary key encoding and the row format | T5-prep | 11 min 18 s | Código y tests 11 min 18 s |
| `69c9f72` | docs(odd): record T1 to T4 of the tables with schema run | T3 | 1 min 59 s | Revisión RDD 1 min 59 s |
| `7841b95` | fix(tables): apply the review findings on error texts, keys and rows | T5 | 30 min 19 s | Código y tests 29 min 59 s, Revisión RDD 0 min 20 s |
| `7b32e4f` | feat(tables): add tables to the catalog with schemas and schema changes |  | 0 min 00 s |  |
| `520fd73` | test(tables): cover the table catalog, schemas and schema changes |  | 0 min 00 s |  |
| `28ea646` | docs(odd): record T5 of the tables with schema run | T5 | 3 min 45 s | Revisión RDD 3 min 45 s |
| `426f263` | feat(tables): add the row operations of a table | T6 | 35 min 36 s | Código y tests 35 min 36 s |
| `b535c62` | test(tables): cover inserting, updating, reading and deleting rows |  | 0 min 00 s |  |
| `ce1abca` | chore(tables): declare the range, bulk load and sequence modules | T6 | 1 min 38 s | Revisión RDD 1 min 38 s |
| `c0056b4` | docs(odd): record T6 of the tables with schema run | T6 | 1 min 15 s | Revisión RDD 1 min 15 s |
| `0a698e5` | feat(tables): add sequences to the catalog | T6 | 21 min 38 s | Revisión RDD 21 min 38 s |
| `f0388bc` | feat(tables): add primary key ranges and range removal | T6 | 0 min 40 s | Revisión RDD 0 min 40 s |
| `9d1b4aa` | feat(tables): add the bulk load of a table | T6 | 0 min 23 s | Revisión RDD 0 min 23 s |
| `2266828` | docs(odd): record T7 to T9 of the tables with schema run | T6, T7-T9 | 4 min 47 s | Revisión RDD 4 min 47 s |
| `21d1502` | test(tables): cover damage of tables and sequences and close the review test gaps | T10-T11 | 10 min 51 s | Código y tests 10 min 51 s |
| `12dd86b` | feat(bench): add the Krummi side of the table loads W23-W29 and 29 thresholds | T10-T11 | 14 min 43 s | Código y tests 14 min 43 s |
| `18e83d1` | fix(tables): keep bulk load source errors once in the chain and check the bench end states | T10-T11 | 3 min 33 s | Código y tests 0 min 11 s, Revisión RDD 3 min 22 s |
| `9dcceaa` | docs: describe the tables with schema in the README | T10-T11 | 0 min 35 s | Revisión RDD 0 min 35 s |
| `2fd277a` | docs(odd): record T10 and T11 of the tables with schema run | T10-T11 | 0 min 17 s | Revisión RDD 0 min 17 s |
| `93bfa4d` | test(tables): pin the error chain of a failing bulk load source | T10-T11 | 1 min 43 s | Revisión RDD 1 min 43 s |
| `1479c19` | docs(odd): record the last slice review and the final checks | T10-T11 | 12 min 05 s | Revisión RDD 12 min 05 s |
| `574f7d3` | fix(tables): add the tail of a vector norm after its lanes, in order (Codex R3) | codex-R1 | 32 min 15 s | Revisión RDD 32 min 15 s |
| `b0288e3` | fix(tables): borrow the defaults of old rows and refuse schemas too large for the catalog (Codex R1, R2) |  | 0 min 00 s |  |
| `ba8b5d2` | docs(reviews): record the first round of the tables with schema code review | codex-R1 | 0 min 46 s | Revisión RDD 0 min 46 s |
| `b60d83c` | docs(reviews): record the second round of the tables with schema code review | codex-R1, RDD codex fixes | 8 min 04 s | Revisión RDD 8 min 04 s |
| `6e4a04a` | fix(tables): report a schema too large for the catalog as its own rule (Codex R1) | Q2 schema too large | 2 min 29 s | Correcciones de hallazgos 2 min 29 s |
| `461bccc` | docs(prd): annotate the schema size limit and the temporal domain after the code review |  | 0 min 00 s |  |
| `dbc0917` | docs(reviews): record the product decisions of the tables with schema code review | Q2 schema too large | 0 min 10 s | Correcciones de hallazgos 0 min 10 s |
| `ed4a2bc` | docs(tables): state the rule and column of the schema size check | RDD R1 rule | 1 min 04 s | Revisión RDD 1 min 04 s |
| `bb5bd93` | docs(odd): record the review of the schema size rule | RDD R1 rule | 0 min 08 s | Revisión RDD 0 min 08 s |
| `d0d4ef6` | docs(reviews): record the approval of the tables with schema code review | RDD R1 rule | 5 min 34 s | Revisión RDD 5 min 34 s |
| `62df5d5` | docs(tables): design JSON column storage and indexes over JSON | RDD R1 rule, T12 | 3 h 30 min | Aceptación 3 h 29 min, Revisión RDD 0 min 13 s |
| `e36a9b7` | docs(tables): keep f64 equality out of JSON index candidates and pin duplicate-key corpus cases | T12 | 9 min 50 s | Aceptación 9 min 50 s |
| `7f3bd04` | docs(reviews): record the Codex review of the JSON designs and its resolutions | T12 | 0 min 16 s | Aceptación 0 min 16 s |
| `3bbd2da` | docs(reviews): record the approval of the JSON designs review | T12 | 5 min 20 s | Aceptación 5 min 20 s |
| `d7bf6b8` | docs(tables): add temporal JSON extraction and record dictionary ranges as open | T12 | 31 min 19 s | Aceptación 31 min 19 s |
| `99277d1` | docs(tables): settle numeric and timestamp extraction from JSON | T12 | 33 min 08 s | Aceptación 33 min 08 s |
| `e352bca` | docs(tables): keep hashed terms in mind for dictionary prefix scans | T12 | 6 min 26 s | Aceptación 6 min 26 s |
| `5caac57` | docs(reviews): record the review of the post-approval JSON range and its resolution | T12 | 0 min 18 s | Aceptación 0 min 18 s |
| `9beee7f` | docs(reviews): record the approval of the post-approval JSON range | T12 | 5 min 20 s | Aceptación 5 min 20 s |
| `8a297b7` | docs(tables): list documents 11, 13 and 14 in the design set README | T12 | 4 min 36 s | Aceptación 4 min 36 s |
| `d09408c` | docs(tables): state that inverted indexes can serve ranges and order over one path | T12 | 15 min 05 s | Aceptación 15 min 05 s |
| `269916e` | docs(bench): add the complete runs of the tables with schema | T12 | 22 min 59 s | Aceptación 10 min 53 s, Correcciones de hallazgos 10 min 58 s, Documentación 1 min 08 s |
| — | después del último commit | T12 | 9 min 47 s | Checks (fmt, clippy, suite de tests) 9 min 37 s, Documentación 0 min 09 s, Cierre (informe y merge) 0 min 01 s |
| **Total** | | | **9 h 59 min** | |

La aceptación ocupa la mayor parte: rotaciones y ejecuciones completas en dos sistemas, 5 h 28 min en segundo plano.
La implementación (T1–T11) se delegó casi toda en escritores en paralelo, así que el primer plano la muestra como
código y tests y como revisión RDD mientras trabajaban. Los commits `62df5d5` a `d09408c` son de otra sesión y se
hicieron mientras corría T12: el tiempo por commit les asigna el tiempo de aceptación que pasó entre ellos, no trabajo
sobre ellos.

## Change report (`d487cde..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 17.066 | 21 | 13.034 | 1.766 | 32 |
| `crates/krummi-bench` | 1.645 | 56 | 970 | 145 | 18 |
| `docs` | 618 | 0 | — | — | 8 |
| `crates/krummi-pager` | 565 | 16 | 459 | 81 | 8 |
| `tools` | 343 | 0 | — | — | 6 |
| `odd` | 293 | 0 | — | — | 2 |
| `crates/krummi-tree` | 151 | 8 | 103 | 37 | 1 |
| `crates/krummi-tx` | 50 | 4 | 38 | 8 | 3 |
| `README.md` | 18 | 1 | — | — | 1 |
| **Total** | **20.749** | **106** | **14.604** | **2.037** | **79** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `docs/design/tables/13-json-column-storage.md` (+978 −0), `docs/design/tables/14-json-indexes.md` (+489 −0), `docs/design/tables/README.md` (+3 −0), `docs/reviews/tables-json/2026-11-11-code-02.md` (+148 −0), `docs/reviews/tables-json/2026-11-11-code-03-resolutions.md` (+41 −0), `docs/reviews/tables-json/2026-11-11-code-03.md` (+130 −0), `docs/reviews/tables-json/2026-11-11-code-04.md` (+128 −0), `docs/reviews/tables-json/2026-11-11-code-resolutions.md` (+58 −0), `docs/reviews/tables-json/2026-11-11-code.md` (+96 −0)

La línea *Aparte* lista los documentos de diseño y de revisión de JSON que otra sesión commiteó en la rama; no son
de esta ejecución. No se hizo una previsión de líneas.

## Summary

Krummi tiene tablas con esquema. Una tabla se crea o se abre con su esquema y sus modos (abrir, crear nueva, abrir o
crear, crear o vaciar, vaciar), se renombra, se vacía y se borra. Su esquema cambia sin reescribir filas: se añaden,
quitan y renombran columnas, y se permiten nulos; las filas antiguas se leen con la versión de su esquema. Las filas
se insertan, actualizan, sustituyen, leen y borran por clave primaria, y se recorren o borran por intervalos de la
clave, hacia delante y hacia atrás. La carga masiva es atómica, en su propia transacción. Hay secuencias y generadores
de `ulid`, `uuid` v7 y `uuid` secuencial con el orden de SQL Server. Decisiones técnicas: las claves usan la
codificación de tuplas de FoundationDB, las filas el formato v1 del diseño 01, la entrada del catálogo guarda el
esquema vigente y un árbol de historial guarda las versiones antiguas y las identidades, y el formato del almacén
pasa a 11.

## Defaults chosen without asking

Los registra el documento de ejecución, en el orden en que se tomaron: L3 (texto de `DifferentTreeType`,
`Box<str>`), L5 (generadores: error de E/S sin carpeta, extracción de 10 bytes, reloj anterior a 1970), L7 (W24 con
`uuid` v4, milisegundo de los `ulid`), L8 (nanosegundos en los valores temporales, sin ensanchar enteros, orden de los
fallos de `key_range`), L9 (operaciones de árbol sobre una tabla, orden de las comprobaciones de `modify`), L10
(claves con más o menos valores, orden del upsert, rangos de tamaño), L11 (identidades de secuencias, secuencia
agotada vacía, API de intervalos y de carga masiva), L12 (W25 crea la tabla antes del cronómetro) y L20 (el último
tramo queda sin RDD).

## Pending (non-blocking)

- **Umbrales incumplidos** (AC-N9, K13): `tabla-intervalos` en los dos sistemas (0,15 y 0,19), y `tabla-lecturas` y
  `tabla-actualizacion` en Windows (0,75 y 0,73). Cada uno es un cambio aparte del módulo de tablas. Diagnóstico de
  W27 (perfil de samply en Windows): `Reader::decode` se lleva el 70 % de la carga y el recorrido del árbol el 8 %.
  Cada fila reserva y libera dos vectores (el de la clave decodificada y su `Vec<Value>`, un 22 %), y los movimientos
  de `Value` de 48 bytes a través de `Result<Option<Value>>` provocan paradas de reenvío de memoria (un 25 % de tiempo
  propio en `decode`). Voron devuelve un puntero a la fila. Una lectura sin materializar los valores cambiaría la
  interfaz pública, así que ese cambio necesitaría su PRD.
- **`ed4a2bc` sin RDD:** 6 líneas de comentario en `table/schema.rs`. El tramo que lo contiene superó el presupuesto de
  contexto por los documentos de JSON, y revisarlo solo exige un checkout desacoplado de la copia principal, que
  movería HEAD bajo la otra sesión. Codex lo revisó y lo aprobó en la ronda 3.
- **Commits de otra sesión en la rama:** `62df5d5` a `d09408c` (diseños 13 y 14 de JSON, sus revisiones de Codex y el
  README del diseño). No son de esta ejecución; el informe de cambios los lista aparte.
- **Test con carrera ya existente:** `krummi-tx` `recovery::ac_j9_a_failed_transfer_refuses_writes_and_recovery_brings_everything_back`
  falló una vez bajo carga y pasó en todas las repeticiones (L13). Es código que esta ejecución no cambia.
- **Espejo en Engram:** pendiente; `mem_save` falla con "host session already ended" desde el principio (L4).

**Risk:** item 1 (formato del almacén 11, formatos nuevos de clave, fila y catálogo) e item 3 (interfaz pública nueva
de tablas y secuencias, y la regla nueva `SchemaTooLarge`). Cubiertos por las revisiones RDD escaladas y por Codex.

## Manual cleanup

- `work/t12` (3,0 GB): fuentes, compilaciones, almacenes y salidas de T12; los informes ya están en
  `docs/benchmark/`.
- `~/t12` en WSL 2 Ubuntu (2,4 GB): las compilaciones y los almacenes de Linux de T12.
- `work/prof/release` (877 MB) y `work/prof/w27.*` (1,1 MB): la compilación para perfilar y el perfil de W27 del
  diagnóstico; el resto de `work/prof` es de ejecuciones anteriores.
- `work/rdd-*.json` (1,1 MB): los sobres de RDD de esta ejecución.
