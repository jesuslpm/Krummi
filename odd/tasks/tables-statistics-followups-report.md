# Seguimientos de las estadísticas de las tablas: avisos y sugerencias cerrados, aprobados por Codex

Informe de cierre de `tables-statistics-followups` (2026-11-20). Enfoque de ejecución: ODD.
Fuente de producto: ninguna nueva; son los pendientes no bloqueantes del
[informe de cierre de tables-statistics](../done/tables/tables-statistics-report.md), sin cambio de producto, así que
no llevan PRD. Documento de ejecución: [tables-statistics-followups.md](tables-statistics-followups.md).

Rama `feat/tables-statistics`, de `134c6f2` a este cierre, sin fusionar y sin publicar. La rama queda para tu revisión:
la fusión y la publicación son decisiones tuyas.

## Resultado

- **Especificación:** S1–S3 del documento de ejecución, en T1–T7, cada una con su commit:
  - T1 (S1c): prueba propia de `write_unless`: con el turno libre abre la escritura, en la cola respeta el orden de
    llegada y, cuando `stop` lo pide, se va con la clase de tiempo agotado y deja pasar a la siguiente (`104b9f3`).
  - T2 (S1d): pruebas de daño de los caminos sin verificar: una tabla de 31.000 filas muestreada por hojas al azar y
    una escritura que muestrea su propia vista, ambas con daño en medio de los runs de valores grandes (`9ab22f3`,
    corregida en `e2a12bd`).
  - T3 (S1b): los destinos retirados de una tabla borrada se olvidan; el límite de lo que queda está documentado
    (`095d908`, `a1a03dc`).
  - T4 (S1a): el nombre actual de la tabla muestreada se busca con una lectura antes del turno de escritura. Bajo el
    turno solo se recorren las tablas si la tabla se renombró o se borró en medio, y nunca para una tabla borrada
    (`17040b3`).
  - T5 (S1e y la sugerencia de `test-hooks`): nota en las guardas de pánico del muestreo sobre `panic = "abort"`
    (`e9e2c99`). `test-hooks` ya estaba solo en las dependencias de desarrollo: `cargo tree` no lo muestra en las
    compilaciones de `krummi` ni de `krummi-bench`.
  - T6 (S2, gancho del banco): `KRUMMI_BENCH_ESTIMATES` rechaza los valores desconocidos; cuando fallan la escritura y
    el hilo de estimaciones se informan los dos errores; prueba de W28 con el hilo (`0290241`).
  - T7 (S3): limpieza de `work/st` y de `~/st` en WSL (abajo).
- **Pruebas:** `cargo test --workspace`: 59 binarios, 1.030 pasan, 0 fallan, 35 ignoradas (8 nuevas). `cargo fmt --all
  --check` y `cargo clippy --workspace --all-targets -- -D warnings` limpios.
- **Mutaciones:** las pruebas de T2 fallan si `resolve_unchecked` verifica el run; la de escritura también falla si solo
  `WriteTransaction::run_unchecked` lo verifica. La prueba de los dos errores falla con el `Ok((written?, rounds?))`
  anterior. La de T3 fue RED antes de la corrección (quedaban 4 destinos y se esperaban 2).
- **Revisiones:** dos revisiones RDD aprobadas (tabla de abajo). Codex: ronda 1 con un hallazgo mayor (R1) y ronda 2
  aprobada ([informe](../../docs/reviews/tables-statistics-followups/2026-11-20-code.md),
  [seguimiento](../../docs/reviews/tables-statistics-followups/2026-11-20-code-02.md),
  [resoluciones](../../docs/reviews/tables-statistics-followups/2026-11-20-code-resolutions.md)).
- **Hallazgos corregidos:** R1 de Codex: la prueba de muestreo en una escritura muestreaba por una lectura, porque la
  escritura no cambiaba la tabla; ahora añade una columna y muestrea su propia vista (`e2a12bd`).

## RDD reviews

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `134c6f2..3addc0e` | riesgo, resiliencia, fiabilidad | aprobada | SUGERENCIA ×2 y AVISO: el guardado en segundo plano abre una lectura antes de la cola y, si falla al cerrar el almacén, devuelve un error en vez de `Ok(())`; sin cambio, porque solo lo llaman los trabajos del hilo, que descartan el resultado. SUGERENCIA: la poda corría en cada retirada; corregida en `a1a03dc`. SUGERENCIA: la prueba de `write_unless` usaba un límite de reloj; quitado en `a1a03dc` | `review-2dc33303854f3e5d` |
| `3addc0e..341a002` | fiabilidad, resiliencia | aprobada | SUGERENCIA y AVISO: si `write_unless` tuviera un plazo propio, la prueba no distinguiría el `stop`; sin cambio, porque no lo tiene (`write_checked(None, Some(stop))`) y una regresión colgaría la prueba en vez de pasarla | `review-b1bfbacaeec45264` |

## Tiempo

- Estimación: 3–5 h. Arranque real: 2026-11-20 18:04:19 +00:00. Fin: 2026-11-20 19:09:53 +00:00.
- **Duración activa desde el arranque: 1 h 05 min**, un tercio del mínimo estimado. La mitad del activo fueron las
  correcciones de hallazgos, sobre todo R1 de Codex, con la suite completa corriendo.
- Esperas por el usuario: ninguna.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 1 min 17 s | 2 % | 1 |
| Código y tests | 17 min 21 s | 26 % | 6 |
| Checks (fmt, clippy, suite de tests) | 10 min 10 s | 16 % | 1 |
| Revisión RDD | 3 min 10 s | 5 % | 2 |
| Correcciones de hallazgos | 32 min 48 s | 50 % | 2 |
| Documentación | 0 min 48 s | 1 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **1 h 05 min** | **100 %** | **14** |

- Inicio: 2026-11-20 18:04:19 +00:00. Fin: 2026-11-20 19:09:53 +00:00.
- Arranque confirmado: 18:04:19. Activo desde el arranque: **1 h 05 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | rdd-followups | 18:34:43 | 18:35:23 | 0 min 40 s |
| Revisión RDD | codex-R1 | 18:37:24 | 18:51:57 | 14 min 33 s |
| Revisión RDD | codex-R2 | 19:04:45 | 19:08:29 | 3 min 44 s |
| Revisión RDD | rdd-last-slice | 19:09:12 | 19:09:34 | 0 min 22 s |
| **Revisión RDD (total)** | | | | **19 min 19 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `104b9f3` | test(tx): cover write_unless in the write queue | T1 | 1 min 43 s | Exploración y mapa 1 min 17 s, Código y tests 0 min 26 s |
| `9ab22f3` | test(tables): cover damaged runs in random-leaf and write sampling | T1, T2 | 1 min 12 s | Código y tests 1 min 12 s |
| `095d908` | fix(tables): forget the removed targets of a deleted table | T2, T3 | 1 min 34 s | Código y tests 1 min 34 s |
| `17040b3` | perf(tables): look up a sampled table's name before the write turn | T3, T4 | 11 min 02 s | Código y tests 11 min 02 s |
| `e9e2c99` | docs(tables): note that the sampling panic guards need unwinding | T4, T5 | 0 min 31 s | Código y tests 0 min 29 s, Documentación 0 min 02 s |
| `0290241` | fix(bench): validate KRUMMI_BENCH_ESTIMATES and report both estimate errors | T5, T6 | 2 min 50 s | Código y tests 2 min 04 s, Documentación 0 min 46 s |
| `3addc0e` | docs: add the tables-statistics follow-ups execution | T6, T7 | 10 min 39 s | Código y tests 0 min 34 s, Checks (fmt, clippy, suite de tests) 10 min 05 s |
| `a1a03dc` | refactor: apply the RDD suggestions on the statistics follow-ups | rdd-followups | 3 min 23 s | Checks (fmt, clippy, suite de tests) 0 min 05 s, Revisión RDD 2 min 03 s, Correcciones de hallazgos 1 min 15 s |
| `7bcc9db` | docs: record the RDD review of the statistics follow-ups | rdd-followups | 0 min 06 s | Correcciones de hallazgos 0 min 06 s |
| `e2a12bd` | test(tables): sample a damaged run through the write's own view | rdd-followups, codex-R1 | 27 min 21 s | Correcciones de hallazgos 27 min 21 s |
| `dd00194` | docs(reviews): record Codex round 1 of the statistics follow-ups | codex-R1 | 0 min 01 s | Correcciones de hallazgos 0 min 01 s |
| `341a002` | docs(reviews): record the approval of Codex round 2 of the statistics follow-ups | codex-R1 | 4 min 05 s | Correcciones de hallazgos 4 min 05 s |
| — | después del último commit |  | 1 min 07 s | Revisión RDD 1 min 07 s |
| **Total** | | | **1 h 05 min** | |

Las marcas de fase se pusieron al empezar cada tarea, así que algunas etiquetas de la tabla por commit incluyen la
tarea siguiente. La ronda 1 de Codex (14 min 33 s en segundo plano) fue la tarea más larga.

## Informe de cambios (`134c6f2..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 167 | 6 | 132 | 28 | 5 |
| `crates/krummi-bench` | 136 | 35 | 114 | 16 | 1 |
| `docs` | 166 | 0 | — | — | 3 |
| `odd` | 83 | 0 | — | — | 2 |
| `crates/krummi-tx` | 58 | 0 | 49 | 6 | 1 |
| **Total** | **610** | **41** | **295** | **50** | **12** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Sin archivos de bloqueo ni generados.

## Resumen

Cierra los avisos y sugerencias que las revisiones RDD de tables-statistics dejaron abiertos, sin cambiar el
producto. El cambio de código más visible es que el guardado de estadísticas ya no recorre todas las tablas mientras
tiene el turno de escritura: busca antes el nombre actual, con una lectura. La memoria de estadísticas olvida los
destinos retirados de una tabla borrada. Lo demás son pruebas de cobertura que faltaban (`write_unless` y los caminos
sin verificar del muestreo) y el gancho del banco, que ahora rechaza los valores desconocidos e informa los dos errores.

## Decisiones tomadas sin preguntar

- Limpieza: borré las fuentes y compilaciones de la base y de la rama, y conservé los scripts y los resultados de las
  mediciones (L2).
- `find_write_table`: la búsqueda va antes del turno, en vez de añadir un índice por identidad al catálogo, que sería
  más código para un caso raro (un renombrado o un borrado entre el muestreo y el guardado).
- Poda de los destinos retirados: solo los de las tablas borradas, cuyo suelo `u64::MAX` ya excluye toda estadística.
  Los de las tablas vivas y las entradas de `retired_at` y `floors` siguen hasta que se cierra el almacén: podarlos
  exigiría saber qué construcciones siguen en curso.
- Las guardas de pánico con `panic = "abort"`: solo una nota; impedir esa compilación también se la impediría a quien
  integra la biblioteca.
- Los seguimientos van en su propio documento de ejecución y registro de fases, para no alterar el tiempo medido de
  tables-statistics.

## Pendiente (no bloqueante)

- **Lecturas más lentas sin causa medida** (W7, W9 y W20 en Windows, W18 en Linux): fuera del alcance por tu
  decisión (L2); siguen como pendiente de tables-statistics.
- Los destinos retirados de las tablas vivas y las entradas de `retired_at` y `floors` no se podan: una entrada pequeña
  por columna o índice quitado y por tabla vaciada o borrada, hasta que se cierra el almacén.
- El guardado en segundo plano puede devolver el error de su lectura al cerrarse el almacén; el hilo lo descarta
  (hallazgos de `review-2dc33303854f3e5d`).

**Riesgo:** ítem 4 (concurrencia: la búsqueda del nombre antes del turno de escritura; un renombrado o un borrado en
medio se resuelve bajo el turno por identidad) e ítem 6 (el ahorro de T4 es de coste y ninguna prueba lo observa).

## Limpieza manual

Ninguna pendiente. `work/st` pasó de 1,3 GB a 171 KB y `~/st` en WSL de 602 MB a 68 KB; quedan los scripts y los
resultados de las mediciones de tables-statistics, que podés borrar cuando quieras.
