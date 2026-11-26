# Recorridos de tablas más baratos: terminado y verificado en su rama

Informe de cierre de `table-scans` (2026-11-26). Forma de ejecución: ODD. Origen: el encargo
`work/handoff/perf-scan-prompt.md` (mejora de rendimiento sin cambio de producto, sin PRD). Documento de ejecución:
[table-scans.md](table-scans.md).

Rama `perf/table-scans`, creada desde `feat/tables-vector-indexes` (`76d8820`), con 11 commits hasta `137802c0`, más el
de este informe y el del archivado. No se fusionó ni se subió nada.

## Resultado

- **Especificación:** S1–S6 del documento de ejecución.
  - Línea de partida medida antes de cambiar nada (S2).
  - Perfiles de los recorridos (S3).
  - Cuatro cambios, uno por commit, cada uno con su A/B (S4). Uno de ellos se revirtió por no ganar nada medible.
  - TOAST queda fuera (S5).
  - Una prueba que fija el comportamiento antes de cada cambio, y las mismas filas, el mismo orden y las mismas
    comprobaciones de daño (S6).
- **Medidas:** todo en WSL 2, por decisión del usuario (L2), con el plan de energía del banco y rondas alternadas. Ver la
  tabla de abajo.
- **Checks** sobre `3fa7a3a8`, en Windows:
  - `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings`, limpios.
  - `cargo test --workspace --no-fail-fast`: 1.343 superados y 3 fallados, todos fallos ambientales conocidos que
    también fallan con el código de la base (L14). Fuera de `table_vector_indexes`, el workspace pasa entero.
  - Las tres:
    - `definitions::ac_vi9_the_changes_of_the_table`: `os error 33` al copiar un almacén abierto. Pasa en Linux.
    - `worker::ac_vi3_the_index_follows_the_writes` y `worker::ac_vi15_the_lists_of_large_tables`: agotan sus 120 s
      cuando corren junto al resto de su binario, y pasan solas en Windows y en Linux.
  - Tras la corrección de Codex (`19f1e784`): `exact::` 8 superados, `approximate::` y `pending::` 11 superados,
    `rank_tests` 2 superados, clippy y fmt limpios.
- **Revisiones:**
  - dos revisiones RDD aprobadas (sección siguiente);
  - Codex: en la ronda 1, un bloqueante (R1), corregido; en la ronda 2, aprobado con R1 verificado. Informes
    [2026-11-26-code.md](../../../docs/reviews/table-scans/2026-11-26-code.md) y
    [2026-11-26-code-02.md](../../../docs/reviews/table-scans/2026-11-26-code-02.md); resoluciones en
    [2026-11-26-code-resolutions.md](../../../docs/reviews/table-scans/2026-11-26-code-resolutions.md).
- **Hallazgos corregidos:**
  - Codex R1, bloqueante: un rango sin límites sobre una columna de texto o bytes descartaba todas las filas. El
    camino por rangos se elegía porque el test no tenía valores que lo impidieran. Ahora se elige por el tipo de la
    columna, con una prueba de regresión que falló antes de la corrección (`19f1e784`).
  - RDD R2-001 y R2-002: dos comentarios engañosos, corregidos en `99289503`.

### Medidas antes y después

Base `76d8820` contra `0541b9c2`, en WSL. Cada modo da la mediana de las medianas de 3 rondas alternadas
(`work/ts/final-prof.md`). `0541b9c2` es el último commit de código medido. Lo que vino después (`19f1e784`) solo
cambia qué camino toma una condición sin valores sobre texto o bytes, que estos modos no usan.

| Recorrido | Antes | Después | Después / antes |
|---|---|---|---|
| `nearest` k = 10 con `cat = 7` (camino exacto, 10.000 filas pasan) | 175,8 ms | 136,3 ms | 0,775 |
| `scan`: `range_rows` de la tabla con vectores, `cat` por nombre | 161,3 ms | 122,1 ms | 0,757 |
| `scanv`: lo mismo y el vector de las que pasan | 162,4 ms | 117,2 ms | 0,721 |
| `scanp`: lo mismo con una proyección de `cat` | 105,1 ms | 85,3 ms | 0,812 |
| `tscan`: tabla de W23 (1 M de filas pequeñas), `n` por nombre | 90,9 ms | 66,4 ms | 0,731 |
| `tscanp`: tabla de W23 con una proyección de `n` | 34,8 ms | 34,9 ms | 1,003 |
| `iscanp`: `index_rows` sobre todo `ix_ts` de W30, con proyección | 1.203 ms | 1.218 ms | 1,012 |

Banco, 3 rondas (`work/ts/final-bench.md`), op/s de la rama sobre la base:

| Carga | Razón |
|---|---|
| W23 | 1,025 |
| W24 | 1,031 |
| W25 | 1,007 |
| W26 | 0,989 |
| W27 | 1,001 |
| W28 | 1,014 |
| W29 | 0,997 |
| W30 | 1,013 |
| W31 | 0,989 |
| W32 | 1,004 |

W26 y W31, en 10 rondas (`work/ts/w26-bench.md`): 0,988 (más lenta en 4 de 10 rondas) y 1,005. La pasada de la base
contra sí misma dio razones de 0,987 a 1,009, así que ninguna carga se aparta del ruido.

Aporte de cada cambio, en su propio A/B contra el anterior:

| Commit | Cambio | Efecto medido |
|---|---|---|
| `69a00a8a` | La tabla de nodos comprobados pasa de 65.536 a 524.288 casillas (512 KiB a 4 MiB por almacén abierto) | nearest 0,891, scan 0,866, scanp 0,841; filas pequeñas e índice sin cambio |
| `786cd324` + `ddd950f7` | Leer una columna fija de una fila actual desde su casilla; revertido | 1,08–1,19 más lento; con `decode_with` forzado en línea, 0,986–0,999: sin ganancia |
| `2ef2e8ad` | `TableRow::get` compara los nombres de una tabla de hasta 8 columnas en vez de calcular su hash | tscan 0,727, scan 0,844 |
| `0541b9c2` | El filtro compara los valores de tamaño fijo por su rango, un `u128` que ordena como su clave, sin codificarlos | nearest 0,878 |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `76d8820..0541b9c2` | reliability, readability, resilience | aprobada | R2-001 (WARNING): el comentario de `Test<T>` decía que las filas se comparan con los valores; corregido en `99289503`. R2-002 (SUGGESTION): una cifra puntual en el comentario de `CHECKED_SLOTS` y sin explicar `1 << 19`; corregido en `99289503`. R3-decimal-rank-unchecked (WARNING): el rango de un decimal no comprueba precisión ni escala; descartado, porque la clave tampoco lo hace (`encode_element` usa solo la mantisa) y un decimal guardado fuera de su precisión ya paniquea en `decode_with` antes del filtro | `review-a005a2d2888e3ecd` |
| `0541b9c2..137802c0` | reliability, readability | aprobada | R2-001 (SUGGESTION): el código no fija que todo tipo de tamaño fijo tiene rango. R3 (SUGGESTION): el rango sin límites solo se prueba por la búsqueda exacta. Las dos quedan como pendientes no bloqueantes | `review-1d16c5c961158245` |
| `137802c0..ff3866ae` (este informe y el archivado) | readability | aprobada | R2-001 y R2-002 (WARNING): el archivado reescribió, en los informes de Codex, la ruta del registro de fases en las frases que dejan constancia del estado observado; vuelven a la ruta original. R2-003 (WARNING): la rama tenía 11 commits hasta `137802c0`, no 12. R2-004 (SUGGESTION): faltaba decir de qué lista es el ítem de riesgo. Todo corregido en el commit siguiente | `review-3c65404f3322327b` |

Las evaluaciones de cada commit dieron `medium` / `under_budget`; las dos revisiones cubren toda la rama.

## Tiempo

- Estimación: entre 6 y 10 horas. Inicio: 2026-11-26 08:16:48. Fin: 2026-11-26 12:33:04.
- **Duración activa desde el arranque: 4 h 16 min**, el 71 % del mínimo estimado, en línea con las ejecuciones
  anteriores.
- Esperas por el usuario: 2 min 51 s, la pregunta de dónde perfilar y medir, antes del arranque.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 1 h 02 min | 24 % | 3 |
| Código y tests | 34 min 07 s | 13 % | 1 |
| Checks (fmt, clippy, suite de tests) | 2 h 39 min | 62 % | 1 |
| **Total activo** | **4 h 16 min** | **100 %** | **5** |

- Inicio: 2026-11-26 08:13:57 +00:00. Fin: 2026-11-26 12:33:04 +00:00.
- Arranque confirmado: 08:16:48. Activo desde el arranque: **4 h 16 min**; antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 2 min 51 s.
  - 08:13:57–08:16:48 (2 min 51 s): question: profiling platform

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | T1 baseline | 08:37:24 | 09:06:43 | 29 min 19 s |
| Aceptación | T3a A/B | 09:25:36 | 09:28:53 | 3 min 17 s |
| Aceptación | T3b A/B | 09:32:55 | 09:34:23 | 1 min 28 s |
| Aceptación | T3c A/B | 09:40:38 | 09:41:44 | 1 min 06 s |
| Aceptación | T3d A/B | 09:51:26 | 09:52:33 | 1 min 07 s |
| Aceptación | final A/B | 09:55:21 | 10:40:23 | 45 min 02 s |
| Aceptación | W26 10 rounds | 10:40:49 | 11:36:40 | 55 min 51 s |
| Checks (fmt, clippy, suite de tests) | workspace checks | 11:37:12 | 11:47:59 | 10 min 47 s |
| Checks (fmt, clippy, suite de tests) | workspace tests | 11:50:03 | 12:00:19 | 10 min 16 s |
| Revisión RDD | codex-R1 | 12:10:53 | 12:25:25 | 14 min 32 s |
| Revisión RDD | codex-R2 | 12:27:48 | 12:31:40 | 3 min 52 s |
| **Checks (fmt, clippy, suite de tests) (total)** | | | | **21 min 03 s** |
| **Aceptación (total)** | | | | **2 h 17 min** |
| **Revisión RDD (total)** | | | | **18 min 24 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `69a00a8a` | perf(pager): remember the checked nodes of a 4 GiB working set | T2, T3a | 1 h 04 min | Exploración y mapa 1 h 02 min, Código y tests 1 min 32 s |
| `786cd324` | perf(tables): read a fixed-width column of a current row from its slot | T3a | 11 min 07 s | Código y tests 11 min 07 s |
| `ddd950f7` | revert(tables): read fixed-width columns through the row layout again | T3a | 5 min 54 s | Código y tests 5 min 54 s |
| `2ef2e8ad` | perf(tables): find a column by name without hashing in a table of few columns | T3a | 1 min 56 s | Código y tests 1 min 56 s |
| `0541b9c2` | perf(tables): compare fixed-width filter values by rank, without encoding them | T3a | 10 min 48 s | Código y tests 10 min 48 s |
| `99289503` | docs(tables): describe what the filter's conditions hold and why the checked table has its size | T3a, T4 | 6 min 49 s | Código y tests 2 min 50 s, Checks (fmt, clippy, suite de tests) 3 min 59 s |
| `3fa7a3a8` | docs: record the final measurements of the table scans | T4 | 1 h 39 min | Checks (fmt, clippy, suite de tests) 1 h 39 min |
| `8ed2a76f` | docs: record the checks of the table scans | T4 | 33 min 42 s | Checks (fmt, clippy, suite de tests) 33 min 42 s |
| `19f1e784` | fix(tables): take the filter's rank path by the column's type, so a range without bounds on text passes its rows | T4 | 16 min 17 s | Checks (fmt, clippy, suite de tests) 16 min 17 s |
| `a36cc6c8` | docs: add the table-scans Codex review and its resolutions | T4 | 0 min 46 s | Checks (fmt, clippy, suite de tests) 0 min 46 s |
| `137802c0` | docs: record the Codex verification of the table-scans review | T4 | 4 min 07 s | Checks (fmt, clippy, suite de tests) 4 min 07 s |
| — | después del último commit | T4 | 1 min 09 s | Checks (fmt, clippy, suite de tests) 1 min 09 s |
| **Total** | | | **4 h 16 min** | |

El registro de fases quedó grueso: desde el cierre de T3d, todo el tiempo en primer plano se marcó como «Checks». Ese
tramo incluye la espera de las mediciones finales (1 h 41 min en segundo plano), la revisión RDD, la corrección de
R1 y los registros. Las mediciones, con 2 h 17 min, fueron lo más largo de la ejecución. Las 10 rondas de W26 y W31
llevaron casi una hora, porque cada ronda vuelve a ejecutar W23.

## Cambios (`76d8820..137802c0`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 944 | 82 | 869 | 52 | 6 |
| `docs` | 213 | 0 | — | — | 3 |
| `odd` | 162 | 0 | — | — | 2 |
| `crates/krummi-pager` | 4 | 2 | 1 | 3 | 1 |
| **Total** | **1.323** | **84** | **870** | **55** | **12** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell,
JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

La mayor parte del código añadido son pruebas. El código del motor que cambia es pequeño:
- la constante de `checked.rs`;
- la búsqueda por nombre de `rows.rs`;
- `element_rank` y los dos ayudantes de los flotantes en `key.rs`;
- el `Test<T>` genérico de `filter.rs`.

## Resumen

Todos los recorridos que leen columnas por nombre, y los que recorren tablas mayores que 512 MiB, son ahora entre un
19 % y un 28 % más baratos, sin cambiar qué filas devuelven ni en qué orden. La búsqueda vectorial filtrada que
destapó el problema baja de 176 a 136 ms. Tres cambios lo explican:

1. **La memoria de nodos comprobados.** Antes recordaba los nodos de 512 MiB, y ahora los de 4 GiB. Con la tabla de
   4,2 GB, cada pasada volvía a comprobar la organización de casi todas sus hojas.
2. **`TableRow::get`.** En tablas de hasta 8 columnas compara los nombres, en vez de calcular su hash SipHash en cada
   fila.
3. **El filtro de la búsqueda.** Compara los valores de tamaño fijo como un entero de 128 bits que ordena igual que su
   clave, en vez de codificar una clave por fila. Una prueba compara los dos órdenes para cada tipo, en todos los
   pares de valores, NaN y `-0` incluidos.

Leer las columnas fijas sin volver a validar la cabecera de la fila no ganaba nada, así que se revirtió.

## Decisiones tomadas sin preguntar

- **Tamaño de la memoria de nodos:** `1 << 19` (4 MiB por almacén abierto) y no `1 << 20`. Dio la misma ganancia
  (L5, L6) con la mitad de memoria. C5 del PRD de la memoria de nodos ya pide un tamaño fijo e interno.
- **Umbral de la búsqueda por nombre:** se comparan los nombres en tablas de hasta 8 columnas; con más columnas sigue
  el mapa.
- **T3b:** se revirtió el código y se mantuvieron sus dos pruebas, que fijan cómo se leen las columnas fijas y su
  pánico ante datos mal formados.
- **T3e, no hecho:** leer el filtro antes que el vector en `passing` ahorraría alrededor de un 7,6 %, pero cambia qué
  filas mal formadas paniquean, en contra de S6. Acortar el camino por página (`page_and_check`, unos 25 ns por
  página) toca el aislamiento del pager y no es un cambio pequeño.
- **Medidas solo en WSL**, por la respuesta del usuario.

## Lo que ganaría guardar los vectores fuera de la fila (estimación, fuera de alcance)

Con filas de 3,1 KB caben unas 2 por hoja. Leer una columna fija de 1 M de filas cuesta 85,3 ms en la tabla con
vectores (`scanp`) y 34,9 ms en la de filas pequeñas de W23 (`tscanp`), las dos con proyección y con el mismo número de
filas. La diferencia, unos 50 ms, es el coste de recorrer unas 500.000 hojas en vez de unas 20.000.

Con el vector fuera de la fila, al estilo de TOAST:
- un recorrido que no lee el vector costaría lo que `tscanp`, unas 2,4 veces menos;
- la búsqueda filtrada sería ese recorrido más 10.000 lecturas de vectores fuera de la fila y sus distancias
  (`Exact::key` es un 6,7 % de 138 ms, unos 9 ms).

Eso la dejaría, a ojo, en 50–60 ms, contra los 41 ms de PostgreSQL. **Es una estimación hecha con las medidas de este
informe, no una medición.** Cambia el formato y necesita su propio PRD.

## Pendiente (no bloqueante)

- **El recorrido de un índice completo (`iscanp`, 1,2 s)** busca cada fila desde la raíz y en orden aleatorio. En el
  perfil, eso es un 31 % de búsqueda de claves en las ramas y un 28 % de lectura de páginas. No se encontró un cambio
  pequeño.
- **Las dos sugerencias de la última revisión RDD:**
  - fijar con una prueba o una aserción que todo tipo de tamaño fijo tiene rango;
  - cubrir el rango sin límites sobre texto también en la búsqueda aproximada y en las filas pendientes. Hoy esos
    caminos comparten `Filter::check` y `Filter::meets`.
- **Los tres fallos ambientales de Windows** en `table_vector_indexes`, previos a esta rama.
- **Windows no se midió**, por decisión del usuario.

**Riesgo:** ítem 6 de la lista de riesgo alto de ODD (ningún test atraparía la regresión), por el filtro. Una divergencia entre el orden del rango y el de la clave en un tipo no cubierto
cambiaría resultados sin que fallara un test que no fuera de estos. Lo mitigan `element_ranks_order_as_the_elements`,
que cubre todos los tipos de tamaño fijo en todos los pares, y `the_filters_order_every_fixed_width_type`.

## Limpieza manual

- WSL `~/ts` (6,3 GB): clones de la base y de la rama, sus `target`, los almacenes del banco, los perfiles y los
  registros.
- WSL `~/ann/work/motor` (6,1 GB): el almacén de 1 M de filas con vectores. Se reconstruye con
  `krummi-ann-lab motor --paso cargar`.
- `work/ts` (150 KB) y `work/prof` (9 KB) en la copia de Windows, ignorados por git.
