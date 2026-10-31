# División al final de nodo terminada: verificada, revisada por RDD y Codex, lista para merge en main

Informe de cierre de `trees-append-split` (2026-10-31). Enfoque de ejecución: ODD.
Fuente de producto: ninguna (optimización de rendimiento sin cambio de producto; no requiere PRD). Documento de
ejecución: [trees-append-split.md](trees-append-split.md).

Rama `feat/trees-append-split`, sin push. El usuario pidió merge en `main` y push de `main` ("Hazlo pero sin saltar
nada"); ambos se hacen después de este commit.

## Resultado

- **Especificación:** S1–S4 entregados (T1). Un nodo que desborda al final conserva sus celdas y abre un hermano con
  la celda nueva. Se aplica a cualquier nodo y a todos los tipos de árbol, porque todos dividen con `split` en
  `crates/krummi-tree/src/lib.rs`. Tests nuevos:
  - `entries_added_in_order_fill_every_node_but_the_last_of_its_level`
  - `a_stream_that_resumes_before_another_fills_its_leaves`
  - `entries_added_in_reverse_order_still_split_into_halves` (S3)
  - `values_added_in_order_fill_their_subtree`

  Los tres primeros que llenan nodos dan RED en la base y GREEN tras el cambio (L2).
- **Medida (S4, L4):** con 100.000 `set` en orden creciente, las páginas bajan a la mitad (u64 → u64: de 543 a 272
  y la altura de 3 a 2). En orden aleatorio no cambian, salvo +0,6 % con claves de 16 bytes y valores de 100.
- **Checks en `275ecef`:**
  - `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings`: limpios.
  - `cargo test --workspace --no-fail-fast`: 0 fallos.
  - Ignorados de la CI: `cargo test -p krummi --lib -- --ignored` (AC-B18, AC-R12, AC-V8, AC-M14) pasa 4.
    `tree_integrity -- --ignored` en release pasa 7 (3.737 s). `tree_value_model -- --ignored` en release pasa
    1 (509 s).
- **Revisiones:** dos revisiones RDD aprobadas (tabla abajo). Codex: la ronda 1 encontró R1, corregido en
  `275ecef`; la ronda 2 lo verificó y aprobó sin hallazgos abiertos.
  - [Informe R1](../../docs/reviews/trees-append-split/2026-10-31-code.md)
  - [Informe R2](../../docs/reviews/trees-append-split/2026-10-31-code-02.md)
  - [Resoluciones](../../docs/reviews/trees-append-split/2026-10-31-code-resolutions.md)
- **Hallazgos corregidos durante el trabajo:**
  - Tests cuyo alcance dependía de nodos medio llenos (L5):
    - Barridos T11 de AC-B18, AC-R12 y AC-V8: ahora escriben `in_both_orders`.
    - AC-V6: orden decreciente.
    - AC-A15: 120 entradas.
    - AC-M12: clave 5.
  - Codex R1 (menor): el AC-V8 exhaustivo bajó a 131 de sus 140 puntos. Ahora escribe 12 claves en ambos órdenes y
    llega a 158 (la base llega a 156).

## RDD reviews

| Commits | Lentes | Resultado | Hallazgos y resolución | Lineage |
|---|---|---|---|---|
| `86614c4..b7db2f1` | risk, reliability | aprobado | Risk: ninguno. Reliability, 3 avisos no bloqueantes. R3-branch-append-left-size (WARNING): respondido con aritmética (L7). R3-inorder-merge-coverage (WARNING): registrado como pendiente (L6). R3-catalog-coverage (SUGGESTION): no atendido. | `review-b7fc82a5ee615800` |
| `b7db2f1..9fcc7da` (`275ecef`, `9fcc7da`) | reliability | aprobado | ninguno | `review-dde98592f0ebd32d` |

La evaluación de `275ecef` sola dio `medium`, `under_budget` (27 líneas). Por eso entró en la revisión del último
tramo. Los commits posteriores (`a5b02d9`, `7fa189f` y este informe) son documentación pasiva.

## Tiempo

- Estimación: 45 min – 1 h 15 min. Arranque real: 2026-10-31 08:25:57 +00:00. Fin: 2026-10-31 10:59:11 +00:00.
- **Duración activa desde el arranque: 2 h 33 min**, frente a 1 h 15 min como máximo estimado (+1 h 18 min).
  - La desviación viene de las ejecuciones largas de tests, no del código (4 min 58 s).
  - Hubo dos pasadas completas de la suite: una de 54 min y otra de 1 h 14 min con los ignorados de la CI, en la
    que `tree_integrity` tardó 62 min.
  - Además se paró a los 40 min una ejecución anterior con `--include-ignored` sobre todo el workspace.
- Esperas por el usuario: ninguna.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Código y tests | 4 min 58 s | 3 % | 1 |
| Checks (fmt, clippy, suite de tests) | 54 min 21 s | 35 % | 1 |
| Revisión RDD | 1 h 33 min | 61 % | 2 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **2 h 33 min** | **100 %** | **5** |

- Inicio: 2026-10-31 08:25:57 +00:00. Fin: 2026-10-31 10:59:11 +00:00.
- Arranque confirmado: 08:25:57. Activo desde el arranque: **2 h 33 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | codex-R1 | 09:27:40 | 09:35:01 | 7 min 21 s |
| Checks (fmt, clippy, suite de tests) | suite+ignored | 09:38:19 | 10:52:25 | 1 h 14 min |
| Revisión RDD | codex-R2 | 10:54:53 | 10:58:47 | 3 min 54 s |
| **Checks (fmt, clippy, suite de tests) (total)** | | | | **1 h 14 min** |
| **Revisión RDD (total)** | | | | **11 min 15 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `b7db2f1` | perf(tree): split a node that overflows at its end into a new sibling | T1 | 59 min 01 s | Código y tests 4 min 58 s, Checks (fmt, clippy, suite de tests) 54 min 03 s |
| `298ec81` | docs: record the RDD review of the append split | T1 | 2 min 42 s | Checks (fmt, clippy, suite de tests) 0 min 18 s, Revisión RDD 2 min 24 s |
| `275ecef` | test(tree): give the AC-V8 sweep back its margin over the minimum | T1 | 10 min 32 s | Revisión RDD 10 min 32 s |
| `9fcc7da` | docs: add the Codex review of the append split and its resolutions | T1 | 1 h 14 min | Revisión RDD 1 h 14 min |
| `a5b02d9` | docs: record the RDD review of the last append split slice | T1 | 1 min 52 s | Revisión RDD 1 min 52 s |
| `7fa189f` | docs: add the approved Codex follow-up review of the append split | T1 | 4 min 18 s | Revisión RDD 4 min 18 s |
| — | después del último commit | T1 | 0 min 05 s | Revisión RDD 0 min 05 s |
| **Total** | | | **2 h 33 min** | |

El reparto por fase es impreciso. Tras la ronda 1 de Codex no se marcaron las fases `fix` ni `checks`, así que la
corrección de R1 y la espera de la suite en segundo plano (1 h 14 min) cuentan como «Revisión RDD». En el tiempo
activo, la suite pesa en realidad unas 2 h de las 2 h 33 min.

## Informe de cambios (`86614c4..7fa189f`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 212 | 0 | — | — | 3 |
| `crates/krummi-tree` | 193 | 9 | 158 | 26 | 3 |
| `odd` | 102 | 0 | — | — | 1 |
| `crates/krummi` | 21 | 11 | 12 | 8 | 4 |
| **Total** | **528** | **20** | **170** | **34** | **11** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Este informe y el registro de fases se añaden en el commit de cierre, fuera de la tabla. No hubo previsión de líneas.

## Resumen

Cargar claves en orden creciente (identificadores secuenciales, fechas, contadores) ya no deja los nodos medio
llenos. La carga ocupa la mitad de páginas y el árbol puede tener un nivel menos. Las cargas aleatorias apenas
cambian. La regla es la de Voron: si la inserción cae detrás de la última celda del nodo, la hoja queda intacta y el
hermano nuevo empieza con la celda nueva. En una rama, la última clave sube al padre para que el hermano tenga
clave e hijo izquierdo. No cambian el formato, los resultados ni los invariantes D4. El resto de divisiones sigue
partiendo por bytes en mitades.

## Decisiones por defecto tomadas sin preguntar

- AC-V8: la carga de `values_off_their_leaf` pasa de 8 a 12 claves en ambos órdenes, sin bajar el mínimo de 140.
  Así recupera margen sin debilitar el criterio (L8, resoluciones R1).
- Ajustes de los tests existentes que dependían de nodos medio llenos (L5), sin bajar ningún mínimo.

## Pendiente (no bloqueante)

- **L6 / R3-inorder-merge-coverage:** con claves de 2.048 bytes, una rama de una sola clave pasa del cuarto de
  página y nunca se fusiona. Tras borrar, un árbol cargado en orden puede conservar niveles de más.
  - Por qué no bloquea: la regla de fusión es la de la base; la corrección no se pierde y solo sobra espacio.
  - Cuánto se nota: solo con claves grandes.
  - Pide una decisión del usuario sobre la regla de fusión (D4).
- **R3-catalog-coverage:** ningún test propio llena el catálogo en orden. No bloquea porque usa el mismo `split`
  que cubren los tests de árbol.
- **Flujos ordenados intercalados** (`prefijo + id`) no se benefician (L3), igual que en Voron.

**Riesgo:** ninguno (sin cambio de formato, contrato ni concurrencia; los tests de modelo aleatorio y las
comprobaciones de estructura detectan una división incorrecta).

## Limpieza manual

Las carpetas de registros de Codex en `%TEMP%\codex-review\` (`20261031-*-trees-append-split-code`) se pueden borrar.
