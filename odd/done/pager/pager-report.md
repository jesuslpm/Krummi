# Pager terminado: verificado en Windows y Linux, y mergeado en main

Informe de cierre del módulo `pager` (2026-10-22). Enfoque de ejecución: ODD.
Producto: [PRD del Pager](../../docs/prd/muninndb/pager.prd.md) (v1.0), hijo del [PRD raíz](../../docs/prd/muninndb/muninndb.prd.md) (v1.1).
Documento de ejecución: [pager.md](pager.md).

La rama `feat/pager` se mergeó en `main` con fast-forward. **No se hizo ningún push.**

## Resultado

- **Especificación:** las decisiones P1–P17 y los criterios AC-P1–AC-P17, más los del padre adaptados a este
  módulo (AC8, AC9, AC10, AC11, AC12, AC14, AC16, AC18, AC20, AC21). En Windows pasan 64 tests: 26 de fallos
  dentro del crate, 18 de apertura, 15 de páginas, 3 de la política de `unsafe` y 2 de la fachada pública. Un
  test más está ignorado a propósito: el barrido completo de AC10. En Linux pasan los mismos, salvo el de
  mayúsculas, que solo corre en Windows.
- **Aceptación:** el barrido completo de AC10 altera uno a uno los 65.536 bytes de un almacén y lo lee
  entero. Pasó en Linux (Ubuntu 24.04 sobre WSL 2, ext4) dos veces: en `d9bf85e`, en 113 s, y en `339d21c`, en
  518 s, con WSL compartido con otras corridas. El código de páginas no cambió después. En Windows no terminó:
  el antivirus analiza el archivo en cada apertura después de cada escritura (ver *Pendiente*).
- **Revisiones y verificación:**
  - **RDD:** cuatro revisiones, todas aprobadas (ver *Revisiones RDD*).
  - **Verificador independiente** (sonnet): sin bloqueantes y PASS en todos los S# y AC. Probó unas 60
    sondas, entre ellas 6 procesos compitiendo durante 55 rondas, la precedencia del orden de apertura y los
    alias por enlace duro.
- **Hallazgos corregidos por el camino:**
  - Una carrera en la creación concurrente, que la revisión encontró dos veces. La primera corrección
    introducía otra.
  - Un fallo intermitente de AC-P6 en Linux: el temporal se renombraba entre el listado de la carpeta y su
    comprobación.
  - Tres avisos del verificador: carpetas creadas que no se sincronizaban, nombres reservados sensibles a
    mayúsculas en Windows y un desbordamiento al pedir un crecimiento imposible.

## Cobertura de criterios

| Criterio | Test o comprobación | Resultado |
|---|---|---|
| AC-P1, AC-P2, AC-P3, AC-P4 | `tests/open.rs`: `ac_p1_*`, `ac_p2_*`, `ac_p3_*`, `ac_p4_*` | PASS |
| AC-P5 | `ac_p5_a_killed_process_leaves_the_store_unlocked`, `ac_p5_dropping_*` | PASS |
| AC-P6 | `ac_p6_concurrent_openers_*` (20 rondas de 4 hilos; 15/15 en Linux, 8/8 en Windows), `ac_p6_another_path_*` | PASS |
| AC-P7, AC-P8 | `ac_p7_*`, `ac_p8_*` | PASS |
| AC-P9 | `ac_p9_a_creation_killed_at_any_point_*` (proceso hijo abortado en cada punto), `ac_p9_a_power_cut_*` (modelo de corte de corriente) | PASS |
| AC-P10, AC-P11, AC-P12, AC-P13 | `tests/pages.rs`: `ac_p10_*`, `ac_p11_*`, `ac_p12_*`, `ac_p13_*`; `tests/open.rs`: `ac_p11_*`, `ac_p13_*` | PASS |
| AC-P14 | `ac_p14_*` (temporal, falta de memoria en los dos modos, también al abrir) | PASS |
| AC-P15, AC-P16, AC-P17 | `ac_p15_*`, `ac_p16_*`, `ac_p17_references_survive_growth` | PASS |
| AC8, AC9 | `ac8_*`, `ac9_*` | PASS |
| AC10 | `ac10_sample_*` en la suite; `ac10_no_single_damaged_byte_*` (ignorado) en Linux | PASS (completo solo en Linux) |
| AC11, AC12 | `ac_p15_and_ac11_*`, `ac12_*` | PASS |
| AC14 | `ac14_a_temporary_store_creates_no_files` (proceso hijo con su propio TMP), `ac14_a_new_temporary_store_is_empty` | PASS |
| AC16 | `ac_p9_*` | PASS |
| AC18 | suite completa en Windows x64 y Linux x86-64 | PASS |
| AC20 | `ac20_page_reads_point_into_the_mapping_*`, `tests/ui/page_after_close.rs` (no compila: E0505) | PASS |
| AC21 | `crates/muninndb/tests/unsafe_policy.rs` (con fixture que debe fallar) | PASS |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `8878db4..1105fef` | risk, resilience, reliability | aprobada | WARNING `R3-leftover-race`: carrera al descartar el temporal de otro proceso; corregida en `8f4968b`. WARNING `R3-untested-page-io`: páginas y fallos sin tests en ese punto; cubiertos por T4 y T5. | `review-b0abba04fa9b5c7a` |
| `1105fef..d9bf85e` | risk, resilience, reliability | aprobada | WARNING `R3-001`: la primera corrección soltaba el bloqueo antes de renombrar y podía mover el temporal de otro proceso; corregida en `0c1b901`. | `review-a9f4a54633a71055` |
| `d9bf85e..339d21c` | risk, resilience, reliability | aprobada | WARNING `R1-001`: un enlace simbólico llamado `muninn.data.new` ya no se sigue y da "no es un almacén"; es más seguro y se mantiene. | `review-c5566d3fb9596c39` |
| `339d21c..b936f24` | risk, resilience, reliability | aprobada | WARNING `R4-1`: si falla sincronizar una carpeta recién creada, un reintento no vuelve a sincronizar los padres; queda pendiente. | `review-b905410fb5d0b74c` |

Las dos últimas evaluaciones dieron `under_budget`. Se revisaron igual, porque `CLAUDE.md` pide revisar todo lo
que quede antes del cierre.

## Tiempo

- Estimación: 3–5 h. Inicio real: 2026-10-22 11:30:12 +0100. Fin de la verificación: 12:39:33 +0100.
- **Duración activa desde el arranque: 1 h 09 min**, el 38 % del mínimo estimado, en línea con las ejecuciones
  anteriores, que acabaron por debajo de la mitad. Lo que más pesó fueron las correcciones de hallazgos (49 %):
  - las dos rondas de la carrera de creación;
  - el fallo intermitente de AC-P6 en Linux;
  - los avisos del verificador;
  - el diagnóstico del barrido de AC10 en Windows.
- Esperas por el usuario: ninguna desde el arranque.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 2 min 06 s | 3 % | 1 |
| Código y tests | 21 min 26 s | 31 % | 5 |
| Checks (fmt, clippy, suite de tests) | 0 min 55 s | 1 % | 1 |
| Revisión RDD | 9 min 37 s | 14 % | 4 |
| Correcciones de hallazgos | 34 min 15 s | 49 % | 3 |
| Documentación | 0 min 48 s | 1 % | 1 |
| Cierre (informe y merge) | 0 min 07 s | 0 % | 1 |
| **Total activo** | **1 h 09 min** | **100 %** | **16** |

- Inicio: 2026-10-22 11:30:19 +01:00. Fin: 2026-10-22 12:39:33 +01:00.
- Arranque confirmado: 11:30:19. Activo desde el arranque: **1 h 09 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD T1-T3 reviewers | 11:46:44 | 11:47:53 | 1 min 09 s |
| Revisión RDD | RDD last-slice reviewers | 12:20:12 | 12:20:51 | 0 min 39 s |
| Aceptación | Linux suite + AC10 full sweep | 12:18:14 | 12:20:55 | 2 min 41 s |
| Aceptación | Linux AC10 full sweep (final) | 12:28:40 | 12:37:20 | 8 min 40 s |
| **Aceptación (total)** | | | | **11 min 21 s** |
| **Revisión RDD (total)** | | | | **1 min 48 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `8878db4` | build: add Cargo workspace with pager and facade crates | T1 | 5 min 01 s | Plan (documento de ejecución) 2 min 06 s, Código y tests 2 min 55 s |
| `251f74d` | feat(pager): add on-disk format with mark, double header and page checksums | T1, T2 | 1 min 15 s | Código y tests 1 min 15 s |
| `1105fef` | feat(pager): open, create, lock and close stores | T2, T3 | 8 min 47 s | Código y tests 8 min 47 s |
| `8f4968b` | fix(pager): make concurrent store creation race-free and test injected faults | T3, T1-T3, T4, R3-leftover-race, T5, T4-T5 | 32 min 43 s | Código y tests 8 min 29 s, Checks (fmt, clippy, suite de tests) 0 min 42 s, Revisión RDD 1 min 05 s, Correcciones de hallazgos 22 min 27 s |
| `12f808f` | test(pager): cover pages, growth, maximum size, temporary mode and zero-copy reads |  | 0 min 00 s |  |
| `d9bf85e` | docs: describe the Pager, the crate layout and the engine checks | T4-T5, T6 | 0 min 46 s | Checks (fmt, clippy, suite de tests) 0 min 13 s, Documentación 0 min 33 s |
| `0c1b901` | fix(pager): keep the prepared store open and locked through its rename | T6, last-slice, R3-001 | 4 min 24 s | Revisión RDD 2 min 34 s, Correcciones de hallazgos 1 min 35 s, Documentación 0 min 15 s |
| `339d21c` | fix(pager): look at the folder again when the leftover vanishes while classifying it | R3-001 | 1 min 40 s | Correcciones de hallazgos 1 min 40 s |
| `b936f24` | fix(pager): make created folders durable and match reserved names case-insensitively on Windows | R3-001, fixes, verifier-advisories | 11 min 43 s | Revisión RDD 3 min 56 s, Correcciones de hallazgos 7 min 47 s |
| — | después del último commit | verifier-advisories, advisories, report | 2 min 55 s | Revisión RDD 2 min 02 s, Correcciones de hallazgos 0 min 46 s, Cierre (informe y merge) 0 min 07 s |
| **Total** | | | **1 h 09 min** | |

Los 22 min de "Correcciones de hallazgos" en `8f4968b` incluyen más que la corrección de la carrera:
- los tests de páginas de T4;
- el diagnóstico del barrido de AC10 en Windows;
- las pruebas de fallos de T5.

Todo eso se hizo con la fase de corrección abierta, porque no marqué un cambio de fase al empezarlo. El tiempo
total es correcto; su reparto entre "Código y tests" y "Correcciones" está sesgado hacia correcciones.

## Informe de cambios (`2a4fe73..b936f24`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/muninndb-pager` | 2.833 | 0 | 2.364 | 244 | 13 |
| `crates/muninndb` | 275 | 0 | 229 | 22 | 5 |
| `odd` | 80 | 0 | — | — | 2 |
| `README.md` | 35 | 2 | — | — | 1 |
| `Cargo.toml` | 13 | 0 | 10 | 1 | 1 |
| **Total** | **3.236** | **2** | **2.603** | **267** | **22** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+390 −0)

No había previsión de líneas. La tabla es la de `b936f24`, antes de este informe y del cierre del documento de
ejecución.

## Resumen

MuninnDB ya puede crear, abrir, consultar y cerrar almacenes, en disco o en memoria:
- un almacén es una carpeta con un solo `muninn.data` de páginas de 8 KiB;
- la creación es atómica y durable: sobrevive a la muerte del proceso y a un corte de corriente en cualquier
  punto;
- un solo proceso puede tener abierto un almacén, y la carrera entre varios que lo crean a la vez tiene
  siempre un único ganador;
- cada página lleva un checksum que cubre su número, y el encabezado va en dos copias.

Las lecturas no copian: devuelven referencias a la proyección en memoria, que el compilador ata a la vida del
almacén y que sobreviven al crecimiento del archivo, porque las proyecciones viejas se mantienen hasta el cierre.

Decisiones técnicas clave:
- `unsafe` solo en el crate `muninndb-pager`, con un comentario `SAFETY` obligatorio por bloque; el resto lo
  prohíben los lints del espacio de trabajo y un test.
- Las escrituras de página usan `&mut self` y escrituras posicionadas, así los errores de E/S se informan.
- El temporal se renombra sin reemplazar y con el archivo abierto y bloqueado: por handle en Windows, y con
  comprobación de identidad y `renameat2(NOREPLACE)` en Linux.
- Los fallos se prueban con una capa de inyección que también modela cortes de corriente.

## Decisiones por defecto, sin preguntar

- Dos crates: `muninndb` (API pública, sin `unsafe`) y `muninndb-pager` (interno).
- Dependencias: `memmap2`, `zerocopy`, `xxhash-rust` (xxh3), `rustix` en Linux y `windows-sys` en Windows; en
  desarrollo, `tempfile` y `trybuild`.
- Almacén nuevo de 64 KiB; el archivo crece duplicándose, con un salto máximo de 1 GiB.
- Marca de identificación `\x89MuninnDB\r\n\x1a\n` seguida de tres ceros (16 bytes), al estilo de la firma
  PNG.
- El barrido completo de AC10 es un test ignorado que corre como aceptación en Linux; la suite corre una
  muestra (la marca, las dos copias del encabezado y un byte de cada 1.021).
- La creación vuelve a mirar la carpeta, en lugar de fallar, cuando otro proceso cambia el temporal.

## Pendiente (no bloqueante)

- **Desviación de P11 en Windows (necesita tu decisión).** Si la proyección falla después de reservar
  espacio, que solo pasa al agotar el espacio de direcciones, el almacén conserva su tamaño y sigue
  utilizable, pero el espacio reservado queda en `muninn.data`. Windows no deja recortar un archivo con
  proyecciones abiertas, y P15 obliga a mantenerlas. En Linux el tamaño se restaura. Opciones: anotarlo en el
  PRD como excepción de Windows, o aceptarlo sin anotar. No bloquea porque el caso es prácticamente
  inalcanzable en 64 bits.
- **AC10 completo no corre en Windows.** El antivirus lo hace inviable: 17 s de CPU en más de 15 min de
  reloj, sin terminar. Pasó completo en Linux, y en Windows corre la muestra.
- **R4-1.** Si falla sincronizar una carpeta recién creada, un reintento no vuelve a sincronizar sus padres. Es
  muy acotado: requiere un error de `fsync` en un directorio.
- **Escáner de `unsafe`.** No entiende las cadenas raw ni el literal `'"'`. Es defensa en profundidad: los
  lints `forbid(unsafe_code)` siguen protegiendo.
- **Memoria de las proyecciones viejas.** Duran hasta el cierre, unas dos veces el tamaño final en espacio
  de direcciones. El módulo 2 podrá liberarlas cuando no queden transacciones que las usen, como hace Voron.
- **Sin test automático:** la reserva real de espacio en disco y el fallo físico de lectura sobre la
  proyección (P17). Están cubiertos por construcción.

**Riesgo:** ítem 4 (concurrencia: bloqueo y creación concurrente del almacén) e ítem 1 (formato en disco
nuevo, que fija la versión 1). Los cubren los tests de carrera y las cuatro revisiones RDD, y el verificador
independiente revisó el ítem 4.

## Limpieza manual

Ninguna. Ya se borraron la carpeta temporal del verificador (190 MB) y los dos clones de WSL (433 MB y 517 MB).
