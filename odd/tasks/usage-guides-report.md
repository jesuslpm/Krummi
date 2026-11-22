# Guías de uso de Krummi: terminadas y verificadas, sin fusionar

Informe de cierre de `usage-guides` (2026-11-22). Enfoque de ejecución: ODD.
Fuente de producto: ninguna; documentación sin cambio de producto, sin PRD (`CLAUDE.md`, *Workflow*). Documento de
ejecución: [usage-guides.md](usage-guides.md).

Rama `feat/tables-json` (la elegida por el usuario), commits `7f405ae..1bb6cd2` más el de este informe y el del
archivado. No se fusionó ni se publicó nada.

## Resultado

- **Especificación:** S1–S4 de [usage-guides.md](usage-guides.md). Catorce guías en `docs/guides/` y su índice
  ([docs/guides/README.md](../../docs/guides/README.md)), enlazados desde el `README.md` en una sección *Usage
  guides*:
  - lo básico: primeros pasos, almacenes, transacciones y errores;
  - los árboles normales, de varios valores y de tamaño fijo;
  - las tablas y sus esquemas, las filas y las secuencias e identificadores;
  - los tipos de datos y las columnas JSON;
  - los índices secundarios y las estadísticas.
  Cada ejemplo Rust es un programa completo que corre como doctest de `krummi` (módulo `guides` de
  `crates/krummi/src/lib.rs`). `cargo test -p krummi --doc -- guides::`: 113 pasados y 3 `compile_fail` pasados,
  0 fallidos. Un test, `crates/krummi/tests/guides.rs`, falla si una guía con ejemplos Rust no está registrada (RED
  observado con `statistics.md` sin registrar, luego GREEN).
- **Checks** (sobre `2e81b0f`, antes de las correcciones, que solo tocan guías y un test): `cargo fmt --all --check` y
  `cargo clippy --workspace --all-targets -- -D warnings` pasan; `cargo test --workspace`: 61 binarios, 1.212
  pasados, 0 fallidos, 35 ignorados. Después de las correcciones: los doctests de las guías (113 + 3) y el test de
  registro (1) pasan; Codex corrió en su seguimiento la suite completa (*Revisiones*).
- **Aceptación:** no aplica: no hay PRD ni datos reales; los ejemplos son la prueba de lo que dicen las guías.
- **Revisiones y verificación:**
  - RDD aprobó los dos tramos con código (*Revisiones RDD*).
  - Codex pidió cambios en la primera ronda, con dos hallazgos menores, y aprobó la segunda
    ([informe](../../docs/reviews/usage-guides/2026-11-22-code.md),
    [seguimiento](../../docs/reviews/usage-guides/2026-11-22-code-02.md),
    [resoluciones](../../docs/reviews/usage-guides/2026-11-22-code-resolutions.md)).
  - Los enlaces y las anclas de las guías y del README resuelven todos (comprobación propia en Python; Codex: 256
    enlaces y 26 anclas).
- **Hallazgos corregidos en el camino:**
  - El usuario señaló que la guía de tipos no explicaba por qué el valor temporal va en nanosegundos y lo guardado en
    microsegundos. Se añadió la sección *Nanoseconds in your code, microseconds in the store* (`49d18d3`). El usuario
    pidió después cambiar la unidad: es el PRD [tables-temporal-micros.prd.md](../../docs/prd/krummi/tables/tables-temporal-micros.prd.md),
    fuera de esta ejecución.
  - Codex R1: escribir el texto de un identificador en una columna `uuid` o `ulid` falla con `WrongType`, no con
    `MalformedText` (`3cdd6db`).
  - Codex R2: los vectores no admiten NaN; la tabla de conversiones solo normaliza NaN en `f32` y `f64` (`0bb936e`).
  - Aviso previo de Codex: la carga masiva solo se deshace si falla antes de confirmar; un fallo de disco al confirmar
    tiene un resultado desconocido, nunca parcial (`a172447`).
  - RDD R3-guide-registry-drift: el test de registro (`62e544d`).

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `fb95203..1bb6cd2`, el rango entero | readability, reliability | detenida: `lens_context_budget_exceeded`, sin autoridad creada | ninguno; se revisó por partes | — |
| `fb95203..7f405ae` (módulo de doctests, guías vacías) | reliability | aprobada | R3-guide-registry-drift (SUGGESTION): nada comprueba que cada guía esté registrada; resuelto con el test de `62e544d` | `review-641192694d67d165` |
| `49d18d3..62e544d` (test de registro) | reliability | aprobada | cinco avisos informativos: R3-registration-match-brittle y R3-non-recursive-scan (WARNING), R3-fence-detection, R3-msrv-is-none-or y R3-reverse-direction (SUGGESTION); no se actuó: el registro es de una línea por guía en una carpeta sin subcarpetas | `review-1da10a831654401e` |

Evaluaciones sin revisión: `7f405ae..49d18d3` (las guías y el README) salió `passive`; las correcciones posteriores de
guías, también pasivas.

## Tiempo

- Estimación: 3 h – 5 h. Arranque real: 2026-11-22 05:43:16 +00:00. Fin del registro: 06:49:36 +00:00.
- **Activo desde el arranque: 1 h 06 min**, muy por debajo de la estimación: cinco redactores escribieron las catorce
  guías en paralelo en unos 12 minutos de reloj, y el resto fue verificación.
- Esperas por el usuario: ninguna registrada. El registro es grueso: la fase *Checks* (05:57–06:09) incluye esperar a
  los redactores y la corrección de las unidades temporales, y la fase *Revisión RDD* (06:09–06:49) incluye las
  correcciones de Codex y, entrelazado, el trabajo del PRD de microsegundos, que empezó cuando el usuario lo pidió y no
  es de esta ejecución.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 1 min 00 s | 2 % | 1 |
| Checks (fmt, clippy, suite de tests) | 12 min 17 s | 19 % | 1 |
| Revisión RDD | 40 min 06 s | 60 % | 1 |
| Documentación | 12 min 57 s | 20 % | 1 |
| **Total activo** | **1 h 06 min** | **100 %** | **4** |

- Inicio: 2026-11-22 05:43:16 +00:00. Fin: 2026-11-22 06:49:36 +00:00.
- Arranque confirmado: 05:43:16. Activo desde el arranque: **1 h 06 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | codex-R1 | 06:13:46 | 06:29:26 | 15 min 40 s |
| Revisión RDD | codex-R2 | 06:40:36 | 06:43:57 | 3 min 21 s |
| **Revisión RDD (total)** | | | | **19 min 01 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `7f405ae` | docs(guides): add the guide stubs and run their examples as doctests |  | 1 min 00 s | Plan (documento de ejecución) 1 min 00 s |
| `e56c581` | docs(guides): add the tree, multi-value tree and fixed-size tree guides | guides (parallel writers) | 8 min 52 s | Documentación 8 min 52 s |
| `47d9ec2` | docs(guides): add the table, row and sequence guides | guides (parallel writers) | 1 min 42 s | Documentación 1 min 42 s |
| `bdc6ddb` | docs(guides): add the data type and JSON column guides | guides (parallel writers) | 1 min 47 s | Documentación 1 min 47 s |
| `2fa37df` | docs(guides): add the secondary index and statistics guides | guides (parallel writers) | 0 min 18 s | Documentación 0 min 18 s |
| `14a9381` | docs(guides): add the getting started, store, transaction and error guides | guides (parallel writers) | 0 min 17 s | Documentación 0 min 17 s |
| `2e81b0f` | docs: add the guide index and link the guides from the README |  | 0 min 00 s |  |
| `49d18d3` | docs(guides): explain the nanosecond values and microsecond precision of temporal columns | guides (parallel writers) | 12 min 07 s | Checks (fmt, clippy, suite de tests) 12 min 06 s, Documentación 0 min 01 s |
| `62e544d` | test(guides): check that every guide with Rust examples runs as a doctest | RDD assess | 3 min 23 s | Checks (fmt, clippy, suite de tests) 0 min 11 s, Revisión RDD 3 min 12 s |
| `3cdd6db` | docs(guides): say that identifier text written to a column fails with WrongType | RDD assess | 27 min 32 s | Revisión RDD 27 min 32 s |
| `0bb936e` | docs(guides): limit NaN normalization to scalar floats |  | 0 min 00 s |  |
| `a172447` | docs(guides): qualify the bulk load rollback with the commit's unknown outcome |  | 0 min 00 s |  |
| `c93259f` | docs(reviews): add the usage guides code review and its resolutions | RDD assess | 0 min 17 s | Revisión RDD 0 min 17 s |
| `1bb6cd2` | docs(reviews): add the usage guides follow-up review | RDD assess | 3 min 40 s | Revisión RDD 3 min 40 s |
| — | después del último commit | RDD assess | 5 min 25 s | Revisión RDD 5 min 25 s |
| **Total** | | | **1 h 06 min** | |

La revisión se lleva el 60 %: las dos de RDD, las dos rondas de Codex con sus correcciones y, dentro del mismo tramo,
el borrador del PRD de microsegundos. La escritura en paralelo de las guías costó 13 minutos.

## Informe de cambios (`fb95203..1bb6cd2`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 6.997 | 0 | — | — | 18 |
| `crates/krummi` | 59 | 0 | 53 | 2 | 2 |
| `odd` | 42 | 0 | — | — | 2 |
| `README.md` | 18 | 0 | — | — | 1 |
| **Total** | **7.116** | **0** | **53** | **2** | **23** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

`docs` son las catorce guías, su índice y los informes y resoluciones de Codex. No había previsión de líneas.

## Resumen

Krummi tiene ahora una guía paso a paso por cada parte de su API pública: cómo abrir un almacén, usar las
transacciones y tratar los errores; guardar datos en árboles normales, de varios valores y de tamaño fijo; y definir
tablas, escribir y leer filas, elegir los tipos de las columnas, guardar JSON, indexar y estimar. Cada operación lleva
sus reglas, límites y errores, y un ejemplo que corre como test, de modo que una guía que deje de ser cierta rompe
`cargo test --workspace`. El README las enlaza en su sección *Usage guides*.

## Decisiones tomadas sin preguntar

- Las guías en inglés, por la regla del repositorio (código y documentación técnica en inglés).
- Catorce guías en `docs/guides/`, con un índice propio, en lugar de un único documento largo.
- Los ejemplos como doctests de `krummi`, con `include_str!`, para que no envejezcan sin aviso.
- Cinco redactores en paralelo, cada uno con sus archivos; el padre escribió el índice, el README y el módulo de
  doctests.
- El README ganó las secciones *A quick tour* y *How it works* para separar el recorrido de código y la explicación
  interna de la nueva sección de guías.
- La línea de dependencia Git de *Getting started* lleva un marcador, `<URL of the Krummi repository>`, en lugar de la
  URL del remoto.

## Pendiente (no bloqueante)

- **Unidades temporales:** las guías explican hoy los nanosegundos del valor y los microsegundos guardados. El PRD
  [tables-temporal-micros.prd.md](../../docs/prd/krummi/tables/tables-temporal-micros.prd.md) los unifica en
  microsegundos; su implementación reescribirá esas secciones (su TU8). No bloquea: las guías describen el código de
  hoy.
- **Rarezas del API que encontraron los redactores** (no se tocó código):
  - los handles de árboles, tablas y secuencias, y los recorridos de entradas de un índice, no implementan `Debug`,
    así que `unwrap_err()` no compila sobre sus resultados;
  - `tree_exists` documenta solo árboles, pero también ve tablas y secuencias;
  - funciones de codificación internas son públicas sin `#[doc(hidden)]` (`TableSchema::encode`, `decode`, `layout`,
    `key::encode_key` y otras), igual que `value::normalize`;
  - `TableRow` no tiene lectura tipada de `decimal` ni de vectores;
  - `SchemaRule::VectorIndexKey` es también la regla de una columna JSON en la clave de un índice;
  - el rustdoc de `IndexDefinition` no menciona que una columna JSON tampoco puede ser clave;
  - `BulkLoadError::source()` devuelve la causa del error interno, no el error interno;
  - el rustdoc de `bulk.rs` sigue prometiendo deshacer la carga sin la salvedad del resultado desconocido al
    confirmar (las guías ya la dicen);
  - el error de tamaño máximo informa del doble del valor en un almacén temporal.

  Ninguna bloquea: las guías describen lo que el código hace.
- Los cinco avisos de RDD sobre el test de registro, que no se atendieron (*Revisiones RDD*).

**Riesgo:** ninguno. Solo documentación, un módulo `#[cfg(doctest)]` que no entra en la biblioteca compilada y un
test.

## Limpieza manual

- `C:\Users\JESUS~1.LOP\AppData\Local\Temp\claude\abbc23f7a7515b82\6ce1b30c-0c7f-475e-a6c5-b31016b08203\scratchpad\probe`:
  el crate de prueba de los bordes del instante JSON y de la forma de texto, con su `target`.
- `work/rdd-ug*.json` y `work/rdd-ug*.err`: los sobres de RDD de esta ejecución (ignorados por git).
