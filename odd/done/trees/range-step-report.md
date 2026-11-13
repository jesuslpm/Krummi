# Paso de los recorridos terminado: `intervalos` llega a 0,98 de Voron y `tabla-intervalos` a 0,91, rama verificada

Informe de cierre de `range-step` (2026-11-13). Enfoque de ejecución: ODD.
Fuente de producto: ninguna; es una optimización sin cambio de comportamiento, que no necesita PRD (`CLAUDE.md`).
Documento de ejecución: [range-step.md](range-step.md).

Rama `feat/range-step`, creada desde `main` en `0257204`. Sin fusionar y sin publicar: queda para la revisión del
usuario. La rama también lleva `03cf21f` (docs(tables): rename the JSON multi-valued index to multi-entry index), un
commit de la otra sesión que trabaja en esta copia con los documentos de JSON. Lo hizo a las 08:13, mientras esta rama
estaba activa. No forma parte de esta ejecución y se dejó tal cual (L9).

## Resultado

- **Especificación:** las tres optimizaciones del paso de un recorrido de árbol (`krummi_tree::Range`), sin cambio
  observable:
  - **S1:** cada extremo comprueba su cota y el prefijo una vez por hoja, con la última clave de la hoja (el frontal) o
    la primera (el trasero), en lugar de en cada entrada.
  - **S2:** la clave y el valor de una entrada salen de una sola lectura de la cabecera de su celda (`node::entry`).
  - **S3:** al entrar en una hoja cuyas claves están todas dentro de su cota, se pide al procesador la hoja vecina, que
    es la siguiente que leerá (`PageSource::prefetch` → `Pager::prefetch`, 256 bytes).

  Suite en `a3527a6`: `cargo test --workspace` 831 pasan, 0 fallan, 34 ignoradas; `cargo fmt --all --check` y
  `cargo clippy --workspace --all-targets -- -D warnings` limpios.
- **Aceptación:**
  - Ejecución completa de Krummi y Voron en Windows ([informe](../../../docs/benchmark/2026-11-13-windows-range-step.md)):
    **29 de 29** umbrales. Frente a la ejecución final anterior (`6ffad00`):

    | Carga | Krummi | Cociente frente a Voron |
    |---|---|---|
    | `intervalos` | +11,1 % | 0,90 → **0,98** |
    | `tabla-intervalos` | +7,8 % | 0,86 → **0,91** |
    | `recorrido` | +29,4 % | 1,88 → 2,22 |

  - Las bajadas de más del 5 % en cargas que no recorren se comprobaron con cargas sueltas, `0257204` contra
    `44b1094`, y no se repiten: `escrituras-sueltas` 8.507 → 8.494 y `lecturas-al-azar-16` 3,33 M → 3,44 M, medianas
    de tres rondas (L7). Son variación entre ejecuciones.
  - En WSL 2 solo valió una ronda: la máquina estaba ocupada con otro trabajo. Fue W11 0,83 → 0,99 y W27 1,04 → 1,12,
    solo indicativo (L11).
- **Medición de cada optimización** (cargas sueltas en Windows, fijadas a los núcleos de rendimiento y con el plan de
  energía del banco, cada una contra el binario anterior en la misma tanda):

  | Optimización | W11 | W27 |
  |---|---|---|
  | S1 | +6,5 % | +4,2 % |
  | S2 | ~+1,5 % | ~+2,3 % |
  | S3 | +4,0 % | −0,9 % (dentro de la dispersión entre rondas) |

  La precarga de la página entera restaba un 10 % en W11; la de 64 bytes ganaba menos que la de 256 (L6).
- **Revisiones y verificación:**
  - RDD aprobó el tramo y su corrección (tabla de abajo).
  - Codex aprobó en la ronda 1, sin hallazgos ([informe](../../../docs/reviews/range-step/2026-11-13-code.md)). No hubo
    nada que resolver, así que no hay archivo de resoluciones.
- **Hallazgos corregidos por el camino:** R3-prefetch-slice-bound de RDD. Ahora una aserción en tiempo de compilación
  garantiza que los bytes que se precargan caben en una página (`a3527a6`).

## Cobertura

| Spec | Test o comprobación | Resultado |
|---|---|---|
| S1, S4 | `bounded_scans_stop_inside_a_leaf_from_both_ends`: 300 casos con dos cotas y prefijo, en tres niveles, hacia delante, hacia atrás y alternando | PASS. Pasa también sobre la base; las mutaciones que deciden con la clave equivocada de la hoja lo hacen fallar en el caso 0 (L4) |
| S2, S4 | Suite de recorridos, árboles, valores grandes y tablas, que pasan por `node::entry` y `node::value` | PASS (831) |
| S3 | `a_prefetch_never_fails_and_changes_nothing`: páginas de cabecera, del almacén, más allá del final y `u64::MAX`, en disco y en memoria | PASS |
| S5 | Cargas sueltas por optimización, ejecución completa en Windows y cargas sueltas en WSL | Hechas; en WSL, solo una ronda útil |

## RDD reviews

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `0257204..0d99b0e` | risk, reliability | aprobado | R3-prefetch-slice-bound (SUGGESTION): el corte a 256 bytes depende de que la página sea mayor; corregido con `const _: () = assert!(PREFETCH_BYTES <= PAGE_SIZE)` en `a3527a6` | `review-8fd0dd68fb12ca45` |
| `0d99b0e..a3527a6` | reliability | aprobado | ninguno | `review-4148cf1268b680aa` |

Las evaluaciones de `fab4085`, `05fbba3` y `44b1094` dieron riesgo medio, `under_budget`: quedaron pendientes en el
tramo, que se revisó entero como último tramo.

## Time

- Estimación: 2–4 h. Arranque real: 2026-11-13 07:36:40. Fin: 09:18:35.
- **Duración activa desde el arranque: 1 h 41 min**, por debajo del límite inferior de la estimación.
  - En «Checks» constan 55 min, de los que unos 28 corresponden a la espera de la ejecución completa del banco
    (08:22–08:46). Corrió en segundo plano y, mientras tanto, no se lanzó nada que usara CPU; el registro no marcó una
    fase aparte para esa espera.
  - Los otros 27 min de checks son las suites completas de T1, T2 y T3, de 7 a 11 min cada una.
- Esperas por el usuario: ninguna.
- Antes del arranque, sin registro de fases: el perfilado y los experimentos que dieron las cifras (L2), de 07:06 a
  07:35 según la hora de sus archivos en `work/w11`.

### Time by phase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 4 min 57 s | 5 % | 1 |
| Código y tests | 4 min 54 s | 5 % | 3 |
| Checks (fmt, clippy, suite de tests) | 54 min 58 s | 54 % | 3 |
| Aceptación | 16 min 40 s | 16 % | 4 |
| Revisión RDD | 11 min 45 s | 12 % | 2 |
| Correcciones de hallazgos | 8 min 37 s | 8 % | 1 |
| Cierre (informe y merge) | 0 min 04 s | 0 % | 1 |
| **Total activo** | **1 h 41 min** | **100 %** | **15** |

- Inicio: 2026-11-13 07:36:40 +00:00. Fin: 2026-11-13 09:18:35 +00:00.
- Arranque confirmado: 07:36:40. Activo desde el arranque: **1 h 41 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | full windows | 08:22:43 | 08:46:17 | 23 min 34 s |
| Revisión RDD | rdd-last-slice | 08:51:32 | 08:52:22 | 0 min 50 s |
| Revisión RDD | rdd-fix | 09:02:01 | 09:02:24 | 0 min 23 s |
| Revisión RDD | codex-R1 | 09:02:29 | 09:10:11 | 7 min 42 s |
| Aceptación | wsl W11 W27 | 09:11:01 | 09:18:02 | 7 min 01 s |
| **Aceptación (total)** | | | | **30 min 35 s** |
| **Revisión RDD (total)** | | | | **8 min 55 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `fab4085` | perf(tree): check a range's bounds once per leaf | T0, T1 | 21 min 23 s | Plan (documento de ejecución) 4 min 57 s, Código y tests 2 min 32 s, Checks (fmt, clippy, suite de tests) 11 min 19 s, Aceptación 2 min 35 s |
| `05fbba3` | perf(tree): read a range entry's key and value from one cell header | T1, T2 | 9 min 26 s | Código y tests 1 min 14 s, Checks (fmt, clippy, suite de tests) 7 min 13 s, Aceptación 0 min 59 s |
| `03cf21f` | docs(tables): rename the JSON multi-valued index to multi-entry index | T2, T3 | 5 min 35 s | Código y tests 1 min 08 s, Checks (fmt, clippy, suite de tests) 0 min 06 s, Aceptación 4 min 21 s |
| `44b1094` | perf(tree): prefetch the next leaf of a range scan | T3 | 9 min 20 s | Checks (fmt, clippy, suite de tests) 8 min 05 s, Aceptación 1 min 15 s |
| `0d99b0e` | docs(benchmark): add the Windows run of the range step | T3 | 28 min 00 s | Checks (fmt, clippy, suite de tests) 28 min 00 s |
| `a3527a6` | fix(pager): assert at build time that a prefetch fits in a page | T3, T4 | 11 min 31 s | Checks (fmt, clippy, suite de tests) 0 min 15 s, Revisión RDD 2 min 45 s, Correcciones de hallazgos 8 min 31 s |
| `7cf21b8` | docs(reviews): add the range step code review | T4 | 8 min 45 s | Revisión RDD 8 min 39 s, Correcciones de hallazgos 0 min 06 s |
| — | después del último commit | T4 | 7 min 55 s | Aceptación 7 min 30 s, Revisión RDD 0 min 21 s, Cierre (informe y merge) 0 min 04 s |
| **Total** | | | **1 h 41 min** | |

La fila de `03cf21f` es tiempo de esta ejecución (T3) que cae entre dos commits suyos, porque ese commit de la otra
sesión se coló en medio. La de `0d99b0e` es casi toda la espera de la ejecución completa.

## Change report (`0257204..7cf21b8`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 138 | 0 | — | — | 2 |
| `odd` | 135 | 0 | — | — | 2 |
| `crates/krummi-tree` | 83 | 14 | 65 | 12 | 3 |
| `crates/krummi` | 90 | 0 | 83 | 6 | 1 |
| `crates/krummi-pager` | 59 | 0 | 37 | 16 | 3 |
| `crates/krummi-tx` | 6 | 0 | 3 | 2 | 1 |
| **Total** | **511** | **14** | **188** | **36** | **12** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte, por ser el commit `03cf21f` de la otra sesión: `docs/design/tables/13-json-column-storage.md` (+2 −2),
`docs/design/tables/14-json-indexes.md` (+25 −11), `docs/design/tables/README.md` (+1 −1),
`docs/prd/krummi/tables/tables.prd.en.md` (+9 −6), `docs/prd/krummi/tables/tables.prd.md` (+9 −6). La herramienta los rotula como
«lockfiles, generados o binarios» porque así llama a todo lo que se pasa con `--apart`.

## Summary

Los recorridos por intervalos de Krummi eran más lentos que los de Voron por el paso de una entrada a la siguiente, no
por la búsqueda del inicio. Krummi buscaba un 30 % más rápido, pero avanzaba unos 8 ns más lento por entrada (L2). El
paso ahora:
- comprueba la cota una vez por hoja;
- lee la clave y el valor de una vez;
- pide por adelantado la hoja siguiente, cuyo primer acceso era una espera a memoria.

Con eso, `intervalos` queda casi a la par de Voron, `tabla-intervalos` sube del 0,86 al 0,91 y el recorrido completo
mejora un 29 %. El único `unsafe` nuevo, la instrucción de precarga, queda en `krummi-pager`.

## Defaults chosen without asking

- **S3 se reformuló** de «abaratar `Pager::run_with`» a una precarga de la hoja vecina. El perfil mostró que su costo es
  el primer acceso a la página, no la comprobación de integridad. Se conservaría solo si medía una ganancia (L3); con
  256 bytes, W11 +4 % y W27 −0,9 % (L6).
- **El tamaño de la precarga, 256 bytes,** se eligió entre tres medidos (página entera, 64 y 256).
- **La ejecución completa fue solo en Windows;** WSL se midió con cargas sueltas de W11 y W27 (L3).

## Pending (non-blocking)

- **W27 con la precarga:** −0,9 % en cargas sueltas, con las rondas solapadas. En la ejecución completa,
  `tabla-intervalos` mejoró un 7,8 % en conjunto. No bloquea, porque el cociente sube y el umbral se cumple.
- **WSL 2:** solo una ronda útil, con la máquina ocupada. Una ejecución completa en Linux confirmaría las cifras.
- **Sin precarga fuera de x86-64:** en aarch64 `prefetch` no hace nada, porque Rust estable no tiene ahí una
  instrucción de precarga. El comportamiento es el mismo, sin la ganancia.
- **`tabla-intervalos` sigue en 0,91:** el resto de la diferencia está en leer y comprobar las columnas de la fila
  (L2 de esta ejecución; L11–L16 de `table-row-reads`), fuera del paso del árbol.
- **El commit `03cf21f` de la otra sesión** está en esta rama. Lo que se haga con él lo decide el usuario al fusionar.

**Risk:** item 6, por la precarga: ningún test detectaría que deje de ganar o que empeore el rendimiento, que solo
miden las cargas del banco. La corrección del comportamiento sí la cubren los tests.

## Manual cleanup

- `work/w11` en Windows (1,1 GB): binarios (`k0.exe`, `k1.exe`, `kT1.exe`, `kT2.exe`, `kT3*.exe`), `target`, `win-src`,
  almacenes `kbase` y `w23`, perfiles y registros. Los registros `*.log`, los perfiles `*.json.gz` y los scripts son la
  evidencia que cita el documento de ejecución.
- `~/w11prof` en WSL (3,7 GB): fuentes, cuatro carpetas `target*`, almacenes `kbase`, `vbase`, `w23` y `vw23`, compilación de
  Voron y perfiles.
- `perf_event_paranoid` quedó en 1 en WSL; vuelve a 2 al reiniciar WSL.
