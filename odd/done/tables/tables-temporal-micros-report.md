# Valores temporales en microsegundos: terminado y verificado, sin fusionar

Informe de cierre de `tables-temporal-micros` (2026-11-22). Enfoque de ejecución: ODD.
Fuente de producto: [tables-temporal-micros.prd.md](../../../docs/prd/krummi/tables/tables-temporal-micros.prd.md).
Documento de ejecución: [tables-temporal-micros.md](tables-temporal-micros.md).

Rama `feat/tables-json` (la del PRD), commits `c813083..fa081f6`, más el de este informe y el del archivado. No se
fusionó ni se publicó nada.

## Resultado

- **Especificación:** S1–S4 del documento de ejecución.
  - `time`, `timestamp` y `duration` se dan y se leen en microsegundos, como `i64`, en todos los lugares de TU1:
    `Value`, las lecturas tipadas de una fila y de una proyección, las entradas de un índice, las estadísticas y el
    instante leído de un JSON.
  - Un valor dado ya no se redondea; solo se comprueba su dominio.
  - El instante de un JSON sigue redondeando la fracción de su texto al microsegundo más cercano (TU4).
  - Los bytes guardados y la versión del formato no cambian.
  - Las guías explican una sola unidad.
- **Checks** (sobre `fa081f6`, que no cambia código desde `3a5ed50`):
  - `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` pasan;
  - `cargo test --workspace`: 63 binarios, 1.222 pasados, 0 fallidos, 35 ignorados.
- **Aceptación:** una ronda de W1–W32 en Windows y en Linux, con los datos del banco iguales entre base y rama
  (*Mediciones*). Las diez rondas de confirmación no se hicieron, por decisión del usuario (L10).
- **Revisiones y verificación:**
  - RDD aprobó los tres tramos (*Revisiones RDD*).
  - Codex aprobó el código en la primera ronda, sin hallazgos:
    [informe](../../../docs/reviews/tables-temporal-micros/2026-11-22-code.md) y
    [resoluciones](../../../docs/reviews/tables-temporal-micros/2026-11-22-code-resolutions.md).
  - No hubo verificador independiente: el cambio de API es el pedido, así que no es un ítem de alto riesgo (*Riesgo*).
- **Hallazgos corregidos por el camino:**
  - Las cuatro sugerencias del primer RDD, atendidas en `3a5ed50`.
  - La sugerencia del último RDD sobre el L11, atendida en `fa081f6`.

## Cobertura de los criterios de aceptación

| Criterio | Test o comprobación | Resultado |
|---|---|---|
| AC-TU1 | `values_are_given_and_read_back_in_microseconds_by_every_write_and_read` y `values_outside_their_domain_and_the_integer_extremes_fail_without_effect` (`crates/krummi/tests/table_temporal.rs`) | PASS |
| AC-TU2 | `defaults_bounds_conditions_and_statistics_are_in_microseconds` | PASS |
| AC-TU3 | Los tipos de las firmas públicas (`i64`), comprobados al compilar los tests de AC-TU1–AC-TU6, y las lecturas tipadas de `values_are_given_and_read_back_…` | PASS |
| AC-TU4 | `primary_keys_store_the_bytes_stored_before_the_change`, `rows_store_the_bytes_stored_before_the_change` y `temporal_entries_store_the_bytes_stored_before_the_change` (`index_key_tests.rs`), con bytes capturados en la base `c813083`; y la comprobación de ejecución del L5 | PASS |
| AC-TU5 | `json_instants_are_read_in_microseconds` | PASS |
| AC-TU6 | `text_forms_and_duplicate_keys_keep_their_text` | PASS |
| AC-TU7 | *Mediciones*: una ronda de W1–W32 en Windows y Linux, con K4 igual que antes | PASS parcial: sin las 10 rondas de N11 (L10) |
| AC-TU8 | Doctests de las guías (`data-types.md`, `rows.md`, `json.md`) en `cargo test --workspace` | PASS |

En AC-TU4, los bytes de filas, claves primarias y entradas de índice se comparan contra los de la base. Del valor por
defecto y de las estadísticas no hay bytes fijados. La comprobación del L5 los cubre por su lectura:

- un almacén escrito por la base y abierto por la rama da cada valor, valor por defecto, fila de índice y estadística
  igual a la lectura de la base dividida entre 1.000;
- el mismo almacén escrito por la rama se lee igual.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `c813083..41043c7` (el cambio) | reliability, readability | aprobada | cuatro SUGGESTION, atendidas en `3a5ed50`: R2-rounded-table-misnamed (el test AC6 de 7.1 y su tabla, renombrados), R2-normalize-doc-wrap, R2-midfile-imports, R3-001 (test de una clave cuyo entero no cabe en un `i64`) | `review-3cad7342c5da613f` |
| `41043c7..8d749f6` (las sugerencias y el informe de Codex) | reliability | aprobada | dos avisos sin atender: R3-normalize-doc-semantics (WARNING), que comparaba con un texto intermedio de esta ejecución; R3-temporal-key-test-attribution (SUGGESTION): el test fija el rechazo de `u64::MAX`, no su causa | `review-218b53af7edd6bf9` |
| `8d749f6..8d70b7f` (el log y las mediciones) | readability | aprobada | R2-001 (SUGGESTION): el L11 nombraba solo tres razones fuera del ±3 % y no decía qué cargas tocan valores temporales; atendida en `fa081f6` | `review-b35b832d6dae30af` |

Evaluación sin revisión: `8d70b7f..fa081f6` (la corrección de R2-001, solo el log) salió `under_budget`. No se
revisó aparte porque solo atiende una sugerencia de la revisión anterior. El informe y el archivado son
documentación de la ejecución.

## Tiempo

- Estimación: 6 h – 10 h. Arranque real: 2026-11-22 07:08:13 +00:00. Fin del registro: 10:02:39 +00:00.
- **Activo desde el arranque: 2 h 54 min**, por debajo de la estimación. Dos cosas lo explican:
  - el cambio de código fue de una sola pasada (37 min);
  - las mediciones se quedaron en una ronda (L10).
  Las mediciones se llevan la mitad del tiempo (*Aceptación*, 1 h 30 min de reloj en primer plano, sobre todo la
  pasada de Linux en WSL, 1 h 03 min).
- Esperas por el usuario: ninguna registrada. Las preguntas del usuario durante las mediciones no se registraron
  como espera.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 3 min 04 s | 2 % | 1 |
| Código y tests | 37 min 06 s | 21 % | 1 |
| Checks (fmt, clippy, suite de tests) | 11 min 54 s | 7 % | 1 |
| Aceptación | 1 h 30 min | 52 % | 1 |
| Revisión RDD | 17 min 37 s | 10 % | 1 |
| Cierre (informe y merge) | 14 min 37 s | 8 % | 2 |
| **Total activo** | **2 h 54 min** | **100 %** | **7** |

- Inicio: 2026-11-22 07:08:13 +00:00. Fin: 2026-11-22 10:02:39 +00:00.
- Arranque confirmado: 07:08:13. Activo desde el arranque: **2 h 54 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | codex-R1 | 08:04:00 | 08:16:37 | 12 min 37 s |
| Aceptación | tm-build-windows | 08:17:54 | 08:20:03 | 2 min 09 s |
| Aceptación | tm-windows | 08:20:03 | 08:44:23 | 24 min 20 s |
| Aceptación | tm-linux | 08:44:23 | 09:47:31 | 1 h 03 min |
| Checks (fmt, clippy, suite de tests) | final-checks | 09:49:36 | 10:02:33 | 12 min 57 s |
| **Checks (fmt, clippy, suite de tests) (total)** | | | | **12 min 57 s** |
| **Aceptación (total)** | | | | **1 h 29 min** |
| **Revisión RDD (total)** | | | | **12 min 37 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `f36b0eb` | test(tables): pin the stored bytes of temporal values before changing their unit |  | 3 min 01 s | Exploración y mapa 3 min 01 s |
| `41043c7` | feat(tables)!: give and read temporal values in microseconds | T2 | 48 min 44 s | Exploración y mapa 0 min 03 s, Código y tests 37 min 06 s, Checks (fmt, clippy, suite de tests) 11 min 35 s |
| `3a5ed50` | test(tables): act on the review suggestions for temporal values | T2, RDD T1-T3 | 3 min 56 s | Checks (fmt, clippy, suite de tests) 0 min 19 s, Revisión RDD 3 min 37 s |
| `8d749f6` | docs(reviews): add the temporal values code review | RDD T1-T3 | 13 min 05 s | Revisión RDD 13 min 05 s |
| `d99dfc1` | docs(odd): record the RDD review of the last temporal slice | RDD T1-T3 | 0 min 55 s | Revisión RDD 0 min 55 s |
| `275f840` | docs(odd): mark the start of the temporal measurements |  | 0 min 00 s |  |
| `8d70b7f` | docs(odd): record the temporal measurements | T4 | 1 h 30 min | Aceptación 1 h 30 min |
| `fa081f6` | docs(odd): name the loads that touch temporal values in the measurements | T4, T5 | 1 min 32 s | Aceptación 0 min 06 s, Cierre (informe y merge) 1 min 26 s |
| — | después del último commit | T5 | 13 min 11 s | Cierre (informe y merge) 13 min 11 s |
| **Total** | | | **2 h 54 min** | |

La fase *Aceptación* es casi toda la espera de las dos pasadas en segundo plano. *Código y tests* incluye el cambio de
tipo en los cinco códecs, los tests y las tres guías.

## Informe de cambios (`c813083..fa081f6`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 742 | 288 | 662 | 53 | 14 |
| `docs` | 139 | 92 | — | — | 5 |
| `odd` | 139 | 0 | — | — | 2 |
| `crates/krummi-bench` | 4 | 10 | 3 | 1 | 1 |
| **Total** | **1.024** | **390** | **665** | **54** | **22** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

No hubo previsión de líneas.

## Mediciones

N11 con su anotación del 2026-11-20 pide una ronda de cada carga y luego entre 10 y 20 rondas de W23–W32 y de las que
se muevan más de un 3 %. Esta ejecución hizo solo la primera parte; el usuario pidió no hacer las diez rondas (L10).

- Base `c813083` contra rama `275f840`, con builds de release y el plan de energía del banco.
- Windows: de 08:20:04Z a 08:42:53Z. Linux (WSL 2 Ubuntu): de 08:47:16Z a 09:47:30Z.
- Las comprobaciones de datos del banco (K4) dieron lo mismo en base y rama en los dos sistemas, y las dos pasadas
  salieron con código 0.

La razón es el tiempo de la base sobre el de la rama: por encima de 1, la rama es más rápida.

Solo W23–W32 tocan valores temporales: son las cargas de tablas, y todas las filas tienen la columna `timestamp` `ts`.

- Fuera del ±3 % entre ellas: Windows W32 1,089; Linux W23 0,923, W25 1,070, W27 1,031 y W28 1,043.
- La misma carga no se mueve igual en los dos sistemas: W23 da 1,024 en Windows y 0,923 en Linux.
- Cargas que no tocan valores temporales se mueven igual o más: en Windows W9 1,348, W12 1,132 y W17 1,179; en Linux
  W12 1,322, W13 1,219, W16 1,164, W17 0,891 y W19 0,925.

Con una ronda, ninguna razón queda confirmada, y ninguna se atribuye al cambio.

| Carga | Windows base (ns) | Windows rama (ns) | Windows razón | Linux base (ns) | Linux rama (ns) | Linux razón |
|---|---:|---:|---:|---:|---:|---:|
| W1 | 738,5 | 717,7 | 1,029 | 9.940,2 | 9.958,3 | 0,998 |
| W2 | 8.469,7 | 8.455,1 | 1,002 | 19.348,7 | 19.517,7 | 0,991 |
| W3 | 2.416,0 | 2.494,0 | 0,969 | 2.940,2 | 2.897,7 | 1,015 |
| W4 | 119.365,9 | 119.253,2 | 1,001 | 8.943.755,0 | 9.014.510,4 | 0,992 |
| W5 | 50.586,9 | 50.930,4 | 0,993 | 4.484.814,0 | 4.429.566,1 | 1,012 |
| W6 | 43.179,1 | 43.137,5 | 1,001 | 1.054.695,0 | 1.074.408,7 | 0,982 |
| W7 | 928,1 | 936,5 | 0,991 | 1.175,4 | 1.170,3 | 1,004 |
| W8 | 229,8 | 232,9 | 0,987 | 323,3 | 299,9 | 1,078 |
| W9 | 254,4 | 188,8 | 1,348 | 459,3 | 446,4 | 1,029 |
| W10 | 30,7 | 31,0 | 0,991 | 37,3 | 38,3 | 0,974 |
| W11 | 46,7 | 46,5 | 1,004 | 50,0 | 53,5 | 0,935 |
| W12 | 560,0 | 494,8 | 1,132 | 500,9 | 378,9 | 1,322 |
| W13 | 16.589,0 | 14.705,5 | 1,128 | 97.477,2 | 79.985,0 | 1,219 |
| W14 | 8.188,4 | 8.205,9 | 0,998 | 14.898,7 | 14.406,0 | 1,034 |
| W15 | 8.717,6 | 8.787,0 | 0,992 | 14.864,8 | 15.029,2 | 0,989 |
| W16 | 15.279.532,0 | 15.279.800,0 | 1,000 | 5.269.852,5 | 4.528.355,6 | 1,164 |
| W17 | 140.563,9 | 119.193,8 | 1,179 | 280.369,0 | 314.605,5 | 0,891 |
| W18 | 6.076,9 | 6.010,0 | 1,011 | 7.231,5 | 6.562,7 | 1,102 |
| W19 | 7.555,9 | 7.503,9 | 1,007 | 14.962,4 | 16.171,1 | 0,925 |
| W20 | 121,7 | 115,6 | 1,053 | 117,2 | 120,3 | 0,974 |
| W21 | 6.397,1 | 6.384,5 | 1,002 | 18.562,0 | 18.624,1 | 0,997 |
| W22 | 459,9 | 463,1 | 0,993 | 546,0 | 561,1 | 0,973 |
| W23 | 2.110,9 | 2.062,1 | 1,024 | 8.567,6 | 9.282,7 | 0,923 |
| W24 | 10.033,1 | 10.075,9 | 0,996 | 17.272,6 | 17.756,3 | 0,973 |
| W25 | 2.132,0 | 2.106,5 | 1,012 | 2.144,3 | 2.004,4 | 1,070 |
| W26 | 1.300,1 | 1.318,6 | 0,986 | 1.451,6 | 1.429,2 | 1,016 |
| W27 | 81,8 | 81,1 | 1,009 | 81,7 | 79,3 | 1,031 |
| W28 | 8.037,9 | 8.048,4 | 0,999 | 13.923,4 | 13.345,0 | 1,043 |
| W29 | 8.345,9 | 8.370,4 | 0,997 | 14.197,5 | 14.216,2 | 0,999 |
| W30 | 18.284,5 | 18.323,7 | 0,998 | 26.522,9 | 26.441,7 | 1,003 |
| W31 | 3.410,2 | 3.386,7 | 1,007 | 3.224,0 | 3.273,6 | 0,985 |
| W32 | 1.162,0 | 1.067,4 | 1,089 | 1.227,1 | 1.227,3 | 1,000 |

## Resumen

Una aplicación da y lee ahora los `time`, `timestamp` y `duration` en una sola unidad, el microsegundo, como un `i64`.
Es la misma unidad que Krummi guarda, así que lo que se escribe es exactamente lo que se lee, sin redondeos ocultos. El
único redondeo que queda es el de un texto con más de seis decimales, al leer un instante de un JSON. Los almacenes
existentes se abren sin migración, porque los bytes no cambiaron.

## Decisiones tomadas sin preguntar

- La rama `feat/tables-json`, donde está el PRD (L2).
- T2 y T3 en un mismo commit, para que los doctests de las guías no fallaran entre los dos.
- Los dos avisos del segundo RDD, sin atender (*Revisiones RDD*).
- La corrección `fa081f6`, sin otra revisión RDD.

## Pendiente (no bloqueante)

- **Confirmación de las mediciones:** las diez rondas de N11 no se hicieron (L10). No bloquea: los datos del banco son
  iguales y ninguna razón de una ronda se atribuye al cambio.
- **AC-TU4 de valores por defecto y estadísticas:** cubierto por la lectura del L5, no por bytes fijados. No bloquea:
  esos valores se codifican con los mismos códecs de fila y clave cuyos bytes sí se fijaron.
- Los dos avisos del segundo RDD (*Revisiones RDD*).

**Riesgo:** ítem 3 (contratos que otros consumen): cambia el tipo y la unidad de `Value::{Time, Timestamp,
Duration}`, de las lecturas tipadas, de las proyecciones y de `as_timestamp`. Es el cambio pedido por el PRD, así que no
es un ítem de alto riesgo y no hubo verificador independiente. Los bytes guardados no cambian (AC-TU4).

## Limpieza manual

- `work/tm` (1,3 GB): las fuentes, builds y resultados de las mediciones (ignorado por git).
- `~/tm` en WSL Ubuntu (596 MB): lo mismo en Linux.
- `C:\Users\JESUS~1.LOP\AppData\Local\Temp\claude\abbc23f7a7515b82\6ce1b30c-0c7f-475e-a6c5-b31016b08203\scratchpad\probe`
  (461 MB): el crate de la comprobación de compatibilidad del L5, con su `target`.
- `work/rdd-tm*.json` y `work/rdd-tm*.err`: los sobres de RDD de esta ejecución (ignorados por git).
