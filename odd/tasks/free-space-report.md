# Espacio libre (módulo 4): terminado, verificado, revisado y fusionado en `main`

Informe de cierre de `free-space` (2026-10-24). Enfoque de ejecución: ODD.
Fuente de producto: [free-space.prd.md](../../docs/prd/krummi/free-space.prd.md).
Documento de ejecución: [free-space.md](free-space.md).

Rama `feat/free-space`, diez commits sobre `cf03037` más el de este informe, fusionada en `main` por avance
rápido. No se subió nada.

## Resultado

- **Especificación:** S0–S20 (AC-F1–AC-F20 y la repetición de AC10). Comprobaciones finales sobre `e9192c0`:
  `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` limpios; `cargo test
  --workspace`: 233 pruebas pasan y 5 de aceptación quedan ignoradas, en Windows y en Linux (WSL 2 Ubuntu).
- **Aceptación:** medición de F13 en Windows y en Linux (sección Medición); secuencia completa de AC-F16 (100.000
  operaciones) en release en Windows: pasa en 11,5 s. El barrido AC10 completo con espacio libre no se ejecutó en
  local (más de 15 min en Windows, lo cortó el verificador); lo ejecuta la CI.
- **Revisiones y verificación:** verificador independiente (Sonnet, alto) sobre `0c0df0c` con una tanda de
  correcciones; tres revisiones RDD aprobadas y cerradas (sección siguiente). La comparación del verificador con
  `cf03037` a través de la API pública solo difiere en `format_version` (4 → 5) y en el dato nuevo `free_pages`.
- **Hallazgos corregidos por el camino:**
  - Reservar una página costaba 88,7 µs porque cada búsqueda y cada confirmación recorrían el mapa entero para
    resumir la región; ahora se guarda solo una cota del rango libre más largo: 1,9 µs (`707800f`).
  - El verificador encontró que la cola de confirmaciones publicadas para la información solo se vaciaba al
    consultarla (unos 24 B por confirmación sin consultas); ahora se poda en cada confirmación (`7244491`).
  - La página de la frontera que la confirmación añade para F3 no contaba para el tamaño máximo (`7244491`).
  - Faltaba repetir en modo temporal AC-F3, F4, F9, F10, F11 y F12 (AC-F18); AC-F9 no reabría y la variante
    diferida de AC-F3 liberaba en modo durable (`7244491`).
  - La revisión RDD vio que la prueba con modelo dejaba de reservar al llenarse el almacén y casi todas sus
    operaciones eran escrituras vacías; ahora sigue reservando del mapa y comprueba que reserva y libera de verdad
    (`885ef69`, `e9192c0`).
  - Dos commits se encadenaron sin esperar a las pruebas o a clippy (`ccce542`, `d342ec0`) y se rehicieron con las
    aserciones del formato `4` que faltaban y dos avisos de clippy.

## Cobertura de los criterios de aceptación

Pruebas en `crates/krummi-tx/tests/free_space.rs` salvo que se indique otra cosa.

| Criterio | Prueba | Resultado |
|---|---|---|
| AC-F1 | `ac_f1_reservations_reuse_freed_pages_before_growing` (disco y temporal) | PASS |
| AC-F2 | `ac_f2_a_freed_page_waits_for_the_next_write` (disco y temporal) | PASS |
| AC-F3 | `ac_f3_old_reads_keep_their_version_of_a_reused_page`, `concurrent_reads_see_consistent_snapshots_while_pages_are_reused` | PASS |
| AC-F4 | `ac_f4_an_aborted_or_failed_write_frees_and_reserves_nothing` | PASS |
| AC-F5 | `ac_f5_a_failed_group_never_keeps_a_reuse_without_its_free` | PASS |
| AC-F6 | `ac_f6_crashes_keep_frees_and_reuses_consistent`, `ac_f6_the_store_reaches_the_frontier_after_power_cuts` | PASS; la segunda falla si se quita la página de la frontera (mutación comprobada) |
| AC-F7 | `ac_f7_a_range_reuses_a_run_or_starts_at_the_frontier` (disco y temporal) | PASS |
| AC-F8 | `ac_f8_a_range_that_does_not_fit_skips_to_the_next_region` | PASS |
| AC-F9 | `ac_f9_misuse_fails_and_spoils_the_write` (14 casos, disco con reapertura y temporal), `ac_f9_a_range_as_long_as_the_cap_is_reserved`, `misuse_known_without_the_map_wins_over_damage` | PASS |
| AC-F10 | `ac_f10_freeing_parts_of_ranges_and_changed_pages`, `a_range_freed_by_the_write_that_reserved_it` | PASS |
| AC-F11 | `ac_f11_free_pages_are_reused_when_the_store_cannot_grow` | PASS |
| AC-F12 | `ac_f12_a_long_read_does_not_make_the_store_grow` (disco y temporal) | PASS |
| AC-F13 | `ac_f13_opening_reads_no_map_and_far_free_pages_are_found` | PASS |
| AC-F14 | `ac_f14_a_damaged_map_page_fails_only_what_reads_it`, `free_pages_do_not_depend_on_the_regions_read` | PASS |
| AC-F15 | `ac_f15_free_pages_follow_what_is_durable` | PASS |
| AC-F16 | `ac_f16_sample_of_random_operations` (20.000, en la suite), `ac_f16_random_operations_match_a_model` (100.000, aceptación) | PASS (100.000 en release: 11,5 s) |
| AC-F17 | `krummi-pager open::ac_f17_format_4_is_a_previous_format_and_new_stores_have_format_5` | PASS |
| AC-F18 | `ac_f18_a_temporary_store_follows_the_same_rules`, `ac_f18_the_maximum_size_in_a_temporary_store` y las variantes temporales citadas arriba | PASS |
| AC-F19 | `ac_f19_a_store_truncated_before_its_frontier`, `a_free_page_beyond_a_truncated_end_is_reserved` | PASS |
| AC-F20 | Este informe, sección Medición | PASS |
| AC10 | `ac10_sample_of_altered_bytes_with_free_space` (muestra); `ac10_every_altered_byte_with_free_space` | Muestra PASS; barrido completo no ejecutado en local, lo ejecuta la CI |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `cf03037..7244491` | riesgo, resiliencia, fiabilidad | aprobada | Riesgo y resiliencia: ninguno. Fiabilidad, dos WARNING informativos: la prueba con modelo degeneraba al llenarse el almacén (corregido en `885ef69`); las caídas del modelo solo ocurren tras hacer todo durable (se mantiene: AC-F6 cubre las caídas antes de la durabilidad) | `review-292159d53910b53e` |
| `7244491..885ef69` | fiabilidad, resiliencia | aprobada | WARNING: con el almacén lleno, el modelo podría contar como reutilizable una página liberada por una confirmación aún no durable; premisa falsa (F6 reutiliza desde el punto sin retorno), confirmada con una aserción en `e9192c0`. SUGGESTION: decir que las caídas siguen a `make_durable` (ya es así). SUGGESTION: restaurar el estado de fallos al terminar (es por hilo; se añadió igualmente) | `review-b07c5edfa310785a` |
| `885ef69..e9192c0` | fiabilidad | aprobada | SUGGESTION: una guarda para restaurar el estado de fallos si la prueba entra en pánico; no se aplica, el estado muere con el hilo de la prueba | `review-bb98e2f82312eeec` |

## Reutilización

- La copia de páginas, su conservación en `krummi.temp` y el traslado que espera a los lectores (T14, J5, J7): la
  reutilización inmediata (F6) no añade ninguna espera propia.
- El diario, la recuperación y el punto sin retorno: el mapa, la frontera y el total de libres viajan como páginas
  normales; la recuperación no cambia.
- El contador de páginas de la página de metadatos pasa a ser la frontera; el control de tamaño máximo de la
  transacción (`reserve`) cubre también los rangos y las páginas del mapa.
- `Scratch::visible_txid` comparte con el registro de instantáneas la regla de qué confirmación ve una lectura.
- Las utilidades de caídas simuladas (`Run`, `POWER_CUTS`, `copy_folder`) pasan de `recovery.rs` a `tests/common`
  y las usan las dos baterías.
- Código nuevo: `map.rs` (bits del mapa; un único recorrido de rangos sirve para buscar y para la cota del más
  largo) y `space.rs` (regiones, frontera, total de libres, cotas por región e historial durable para la
  información).

## Medición (F13)

`cargo run --release -p krummi-tx --example free_space_timing`: almacén de 1.179.712 páginas con la mitad liberadas
en huecos sueltos. "Primera" es la primera reserva tras abrir; "siguientes", la media de 100 en escrituras abortadas.

| Sistema | Construcción | Apertura | 1 página (primera / siguientes) | Rango de 16 | Rango de 256 | Rango de 4.096 |
|---|---|---|---|---|---|---|
| Windows (NTFS) | 28,9 s | 70,28 ms | 9,2 / 2,0 µs | 2.743,5 / 5,0 µs | 1.971,5 / 74,4 µs | 13.746,5 / 3.211,3 µs |
| Linux (WSL 2, ext4) | 12,9 s | 0,24 ms | 8,6 / 2,8 µs | 3.073,5 / 6,3 µs | 3.288,6 / 97,8 µs | 6.097,6 / 3.002,4 µs |

La primera reserva de un rango recorre las 19 regiones, porque no hay ningún hueco de 16; las siguientes saltan las
regiones ya conocidas. Los rangos grandes cuestan sobre todo preparar a cero cada página reservada (F8). No hay
umbral (D5).

## Tiempo

- Estimación: 2 h – 3 h 30 min. Arranque real: 2026-10-24 08:28:08 +0100. Fin de la verificación: 09:45:24 +0100.
- **Duración activa desde el arranque: 1 h 17 min**, el 64 % del mínimo estimado. Casi la mitad fue revisión.
- Esperas por el usuario: ninguna dentro de la ejecución (la estimación se confirmó antes del arranque).

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 3 min 41 s | 5 % | 1 |
| Código y tests | 24 min 15 s | 31 % | 3 |
| Revisión RDD | 34 min 39 s | 45 % | 4 |
| Correcciones de hallazgos | 10 min 43 s | 14 % | 3 |
| Documentación | 3 min 58 s | 5 % | 2 |
| **Total activo** | **1 h 17 min** | **100 %** | **13** |

- Inicio: 2026-10-24 08:28:08 +01:00. Fin: 2026-10-24 09:45:24 +01:00.
- Arranque confirmado: 08:28:08. Activo desde el arranque: **1 h 17 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Checks (fmt, clippy, suite de tests) | suite after T2 | 08:43:54 | 08:46:24 | 2 min 30 s |
| Aceptación | F13 Windows | 08:55:56 | 08:56:26 | 0 min 30 s |
| Aceptación | Linux suite and F13 | 08:56:39 | 08:56:39 | 0 min 00 s |
| Aceptación | Linux suite and F13 | 08:57:10 | 08:59:34 | 2 min 24 s |
| Checks (fmt, clippy, suite de tests) | suites after fixes | 09:28:15 | 09:30:45 | 2 min 30 s |
| Revisión RDD | RDD lenses | 09:29:56 | 09:31:56 | 2 min 00 s |
| Revisión RDD | RDD 2 lenses | 09:40:02 | 09:40:34 | 0 min 32 s |
| Checks (fmt, clippy, suite de tests) | final suites | 09:42:45 | 09:45:05 | 2 min 20 s |
| **Checks (fmt, clippy, suite de tests) (total)** | | | | **7 min 20 s** |
| **Aceptación (total)** | | | | **2 min 54 s** |
| **Revisión RDD (total)** | | | | **2 min 32 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `ccce542` | feat(pager): move to format 5 and add free pages to the store information | T1 | 6 min 53 s | Exploración y mapa 3 min 41 s, Código y tests 3 min 12 s |
| `b303e7c` | feat(tx): reuse freed pages with a region map, ranges and free pages in the store information | T1, T2 | 8 min 48 s | Código y tests 8 min 48 s |
| `d342ec0` | test(tx): cover free space across crashes, maximum size, long reads and a random model | T2 | 7 min 56 s | Código y tests 7 min 56 s |
| `707800f` | perf(tx): keep only an upper bound of each region's longest free run | T2, T4 | 4 min 04 s | Código y tests 4 min 04 s |
| `f31070b` | test(tx): add the free-space timing measurement |  | 0 min 00 s |  |
| `0c0df0c` | docs: describe free space in the README and the crate docs, and sweep it in CI | T4, T5 | 1 min 03 s | Código y tests 0 min 15 s, Documentación 0 min 48 s |
| `7244491` | fix(tx): drop published commits no read needs and count the frontier page against the maximum size | T5 | 31 min 22 s | Revisión RDD 28 min 18 s, Correcciones de hallazgos 2 min 33 s, Documentación 0 min 31 s |
| `885ef69` | test(tx): keep the free-space model reserving and freeing to the end and run it in full in CI | T5 | 11 min 20 s | Revisión RDD 4 min 06 s, Correcciones de hallazgos 7 min 14 s |
| `e9192c0` | test(tx): assert a full store reserves from the map in the free-space model | T5 | 2 min 07 s | Revisión RDD 1 min 22 s, Correcciones de hallazgos 0 min 45 s |
| — | después del último commit | T5 | 3 min 43 s | Revisión RDD 0 min 53 s, Correcciones de hallazgos 0 min 11 s, Documentación 2 min 39 s |
| **Total** | | | | **1 h 17 min** |

Lectura: los 28 min de "Revisión RDD" de `7244491` incluyen los 27 min del verificador independiente, que corrió en
segundo plano mientras el registro seguía en la fase de revisión (no se marcó `verify`); la revisión RDD en sí
duró unos 2 min por tanda. El código y sus pruebas llevaron 24 min. La fila "Linux suite and F13" de 0 s es un
primer intento en la distribución de WSL por defecto (la de Docker), que no arrancó.

## Cambios (`cf03037..e9192c0`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi-tx` | 2.134 | 126 | 1.793 | 204 | 8 |
| `odd` | 58 | 0 | — | — | 2 |
| `crates/krummi-pager` | 33 | 15 | 24 | 7 | 5 |
| `README.md` | 15 | 4 | — | — | 1 |
| `crates/krummi` | 5 | 4 | 2 | 3 | 2 |
| `.github` | 4 | 0 | 4 | 0 | 1 |
| **Total** | **2.249** | **149** | **1.823** | **214** | **19** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Del total, el código de producto son 726 líneas añadidas y 69 quitadas (`src` de los tres paquetes); las pruebas y
el ejemplo de medición, 1.446 y 76. Sin archivos generados ni lockfiles.

## Resumen

Las páginas que liberan las escrituras se reutilizan antes de que el almacén crezca. Un mapa de bits por región, con
su página en una posición fija, registra las páginas libres por debajo de la frontera; las de más allá nunca se
usaron. Una escritura reserva páginas sueltas o rangos exactos de hasta una región, primero del mapa y si no en la
frontera (saltando a la región siguiente cuando no caben), y libera páginas para las escrituras que siguen a su
confirmación. Los lectores que todavía ven una página liberada conservan su versión anterior gracias al mecanismo
de versiones existente, así que una lectura larga no hace crecer `krummi.data`. El mapa viaja por el diario como
cualquier página, se lee solo cuando una reserva lo necesita, y la información del almacén da las páginas libres
sin leerlo. El formato pasa a `5`.

## Decisiones tomadas sin preguntar

- Tamaño de región reducido solo para las pruebas (`set_region_pages`, con `test-hooks`), como permite el preámbulo
  de criterios del PRD; la medición y la prueba del tope usan el del producto (65.408 páginas, tope de 65.407).
- La página del mapa de la primera región es la 3, así que la primera página que se reserva pasa de 3 a 4.
- Leer las páginas del encabezado o de metadatos desde una escritura sigue entrando en pánico como antes ("nunca
  reservada"); el PRD solo nombra como uso incorrecto leer o modificar una página liberada y modificar una propia.
- La secuencia de 100.000 operaciones de AC-F16 tarda 71 s en depuración: corre como paso de aceptación (en
  release, y en la CI), y la suite ejecuta una muestra de 20.000, igual que AC10.
- El resumen por región guarda solo una cota del rango libre más largo, aprendida de una búsqueda fallida y olvidada
  cuando una confirmación libera páginas en la región (dejado a la implementación en el PRD).

## Pendiente (no bloquea)

- El barrido AC10 completo con espacio libre y el paso nuevo de AC-F16 de la CI no se han ejecutado en GitHub (no se
  subió nada); en local se ejecutó AC-F16 completo, no el barrido AC10 completo.
- Reservar rangos grandes cuesta unos 0,7–0,8 µs por página por prepararlas a cero (F8); medir con el banco
  comparativo (módulo 6) si importa.
- Las caídas del modelo de AC-F16 ocurren solo después de hacer todo durable (advertencia de la revisión 1); AC-F6
  cubre las caídas antes de la durabilidad.
- El registro de fases cuenta el tiempo del verificador como revisión RDD (sección Tiempo).

**Riesgo:** item 1 (el formato `5` añade páginas del mapa a `krummi.data` y dos campos a la página de metadatos;
los almacenes de formato `4` se recrean) e item 5 (la CI ejecuta dos pasos nuevos de aceptación).
