# Diario y recuperación terminados: verificados en Windows y Linux, con dos revisiones RDD aprobadas

Informe de cierre del cambio `journal` (2026-10-23). Enfoque de ejecución: ODD.
Producto: [PRD de diario y recuperación](../../docs/prd/muninndb/journal.prd.md) v1.0, con su ascendencia
([raíz](../../docs/prd/muninndb/muninndb.prd.md)) y los módulos anteriores
([Pager](../../docs/prd/muninndb/pager.prd.md), [Transacciones](../../docs/prd/muninndb/transactions.prd.md)).
Documento de ejecución: [journal.md](journal.md).

Rama `feat/journal`, con 12 commits desde `401014f`. Se mergea en `main` en local con fast-forward. No se ha subido
nada.

## Resultado

- **Especificación:** S1–S9 cumplidas; todos los criterios del PRD tienen prueba (tabla abajo).
  - Suite de Windows en verde: 49 pruebas unitarias del Pager; 21 de apertura; 15 de páginas; 17 de confirmación; 8
    de la cola; 3 de hilos; 7 del diario; 23 de recuperación; 5 de la API pública; 2 de almacén; 3 de la política
    de `unsafe`.
  - Las pruebas de recuperación cortan la corriente en cada operación de disco de una ejecución grabada, con
    cuatro modelos: solo lo llevado a disco, la primera escritura pendiente a medias, las pendientes reordenadas, y
    muerte del proceso. Además hay una caída real de un proceso hijo.
- **Aceptación** (Linux en WSL, Ubuntu, Rust 1.99.0, sobre `e500816`):
  - `cargo test --workspace` en verde (81 s).
  - Barridos AC10 completos en verde: Pager 51 s, almacén con transacciones 1.362 s, almacén caído con diario
    3.666 s.
  - En Windows, la suite completa en verde; los barridos completos no se corrieron en local (los corre la CI).
- **Revisiones y verificación:**
  - **RDD:** dos revisiones, las dos aprobadas (detalle abajo).
  - **Verificador independiente** (Sonnet, esfuerzo alto): encontró dos bloqueantes, ya corregidos, y su
    recomprobación pasó.
    - Sus sondas cubrieron rotación del diario en cada confirmación, una caída en mitad de un punto de control, y
      ocho sesiones seguidas de abrir, confirmar y caer.
    - Hizo estrés de 3 escritores y 3 lectores.
    - Revisó el código `unsafe` nuevo y no encontró defectos.
- **Hallazgos corregidos por el camino:**
  - **B1** (verificador): con una lectura abierta, una rotación del diario y una confirmación diferida, el
    encabezado podía dar por trasladado algo que no estaba en `muninn.data`. Tras un corte se perdían todas las
    confirmaciones durables desde la lectura. Corregido en `6e8f297`: el diario se hace durable antes de registrar
    el avance. Prueba en RED antes de corregir.
  - **B2** (verificador): un registro escrito tras rotar de archivo guardaba un `durable_upto` viejo, así que un
    daño se descartaba en silencio en vez de informarse. Corregido en `6e8f297`.
  - **R1** (RDD, CRITICAL): si el `fsync` del diario fallaba por falta de espacio, el almacén seguía aceptando
    escrituras y la confirmación siguiente reutilizaba el número. Corregido en `96b6f46`: todo fallo al llevar a
    disco es de entrada y salida y bloquea las escrituras, como dice J12.
  - **Avisos corregidos** (`e500816`):
    - un traslado cuyo candado quedaba envenenado por un pánico no volvía a ejecutarse;
    - un registro cuyo borrado tras un fallo también fallaba podía quedar entero; ahora el almacén deja de aceptar
      escrituras.

## Cobertura de criterios de aceptación

| Criterio | Prueba | Resultado |
|---|---|---|
| AC1, AC3 | `ac1_ac3_a_killed_process_keeps_what_it_committed_and_nothing_else` (proceso hijo), `ac3_an_open_write_leaves_nothing_after_a_crash` | PASS |
| AC2, AC4 | `ac1_ac2_ac4_returned_commits_survive_any_crash_and_none_is_half_applied` (cada operación × 4 modelos) | PASS |
| AC5 | `ac5_an_interrupted_recovery_is_repeated_with_the_same_result` | PASS |
| AC10 | `ac10_every_altered_byte_with_the_journal` (completo en Linux), muestra en la suite | PASS |
| AC11, AC-J14 | `ac_j14_a_failed_or_interrupted_commit_never_appears_and_blocks_nothing` | PASS |
| AC12 | `ac12_a_failed_journal_flush_refuses_writes_and_reopening_recovers`, `r1_…` | PASS |
| AC14 | `ac14_a_temporary_store_with_transactions_touches_no_file` (con las operaciones nuevas) | PASS |
| AC18 | suite en Windows y en Linux | PASS |
| AC20, AC-T15, AC-T16 | pruebas de Transacciones, ahora con las versiones en `muninn.temp` | PASS |
| AC21 | `unsafe_policy` | PASS |
| AC-J1 | `ac_j1_the_journal_appears_with_commits_and_goes_with_a_clean_close` | PASS |
| AC-J2 | `ac_j2_commits_held_back_by_a_read_are_recovered` | PASS |
| AC-J3 | `ac_j3_a_damaged_tail_is_discarded_and_damage_before_a_durable_commit_is_reported`, `b2_…` | PASS |
| AC-J4 | `ac_j4_a_missing_journal_file_or_a_reserved_name_of_the_wrong_kind_is_damage` | PASS |
| AC-J5 | `ac_j5_disk_full_during_recovery` | PASS |
| AC-J6 | `ac_j6_a_store_of_format_1_is_refused_as_older_without_touching_it` | PASS |
| AC-J7 | `ac_j7_deferred_commits_survive_as_a_prefix`, `b1_…` | PASS |
| AC-J8 | `ac_j8_making_what_is_committed_durable` | PASS |
| AC-J9 | `ac_j9_a_failed_transfer_refuses_writes_and_recovery_brings_everything_back` | PASS |
| AC-J10 | `ac_j10_recovery_ignores_the_maximum_size` | PASS |
| AC-J11 | `ac_j11_retained_versions_live_in_muninn_temp_until_close` | PASS |
| AC-J12 | `ac_j12_either_damaged_header_copy_still_recovers_everything` | PASS |
| AC-J13 | `ac_j13_a_durable_commit_is_invisible_until_it_is_durable` | PASS |
| AC-J15 | `ac_j15_compressible_commits_take_less_space_in_the_journal`, `ac_j15_incompressible_commits_are_recovered` | PASS |
| AC-J16 | `ac_j16_journal_files_without_a_store_are_not_a_store` | PASS |
| AC-J17 | `ac_j17_a_running_transfer_blocks_neither_writers_nor_readers` | PASS |
| AC-J18 | `ac_j18_the_journal_stays_bounded_and_outside_the_maximum_size` | PASS |

AC-T21 deja de valer (J6). La sustituye `j6_the_close_records_the_last_commit_in_both_header_copies`.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `f351812..96b6f46` (T1–T6, B1/B2, R1) | risk, resilience, reliability | corregida y después aprobada | CRITICAL R1: un `fsync` del diario fallido por falta de espacio dejaba aceptar escrituras y reutilizar el número de confirmación; corregido en `96b6f46` con prueba en RED. Avisos (WARNING/SUGGESTION): rama de cierre de AC-J7 débil, versiones diferidas que esperan a una durable, `unwrap` del candado del encabezado, recuperación que lee archivos enteros, clase del error al borrar, candado del traslado envenenado, borrado fallido y rotación, latencia de una diferida al trasladar. Corregidos dos en `e500816`; el resto, aceptados. | `review-ed0f0e4daf403899`, recuperado como `review-c3db515f25eeb4cc` |
| `96b6f46..e500816` (avisos) | resilience, reliability | aprobada | Solo avisos (candado envenenado sin prueba, inyección por posición frágil, rama de fallo del `fsync` del borrado sin prueba, reutilización del número sin aserción); aceptados. | `review-7ad40524f997b227` |

La rama completa superó el presupuesto de contexto de una revisión (`lens_context_budget_exceeded`), así que se
revisó desde el commit del PRD.

## Tiempo

- Estimación: 2–4 h. Arranque: 2026-10-22 21:16:32 +0100. Fin: 2026-10-23 00:44:22 +0100.
- **Duración activa desde el arranque: 3 h 27 min**, dentro de la estimación y cerca de su máximo. Por primera
  vez una ejecución no termina en torno a la mitad del mínimo. Pesaron la aceptación final en Linux (1 h 26 min,
  casi todo los barridos AC10 completos, que ahora sincronizan disco en cada cierre) y la verificación
  independiente, que tardó unos 47 min.
- Esperas por el usuario: ninguna. El usuario pidió implementar junto con el PRD, así que la estimación se anotó
  en el documento de ejecución sin preguntar.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 3 min 19 s | 2 % | 1 |
| Plan (documento de ejecución) | 1 min 01 s | 0 % | 1 |
| Código y tests | 23 min 40 s | 11 % | 6 |
| Aceptación | 2 h 14 min | 63 % | 2 |
| Revisión RDD | 35 min 35 s | 17 % | 3 |
| Correcciones de hallazgos | 13 min 30 s | 6 % | 2 |
| **Total activo** | **3 h 31 min** | **100 %** | **15** |

- Inicio: 2026-10-22 21:13:13 +01:00. Fin: 2026-10-23 00:44:22 +01:00.
- Arranque confirmado: 21:16:32. Activo desde el arranque: **3 h 27 min**; antes del arranque: 3 min 19 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | linux | 22:10:42 | 22:10:43 | 0 min 01 s |
| Aceptación | linux2 | 22:11:16 | 22:11:23 | 0 min 07 s |
| Aceptación | linux3 | 22:11:44 | 23:17:41 | 1 h 05 min |
| Aceptación | linux-final | 23:17:54 | 00:44:10 | 1 h 26 min |
| **Aceptación (total)** | | | | **2 h 32 min** |

Tarea en segundo plano sin cerrar, no contada: Revisión RDD / rdd-t1-t3 desde 21:41:07.

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `f351812` | docs(prd): add the journal and recovery PRD and annotate the earlier ones |  | 3 min 30 s | Exploración y mapa 3 min 19 s, Plan (documento de ejecución) 0 min 11 s |
| `3ddebc5` | feat(pager): add format 2 header with journal checkpoint and alternating header writes | T1 | 6 min 13 s | Plan (documento de ejecución) 0 min 50 s, Código y tests 5 min 23 s |
| `0bc6c94` | feat(pager): keep committed page versions in mapped files of muninn.temp | T2 | 4 min 01 s | Código y tests 4 min 01 s |
| `a6184cc` | feat(pager): journal commits with LZ4 records, deferred mode and transfer after each commit | T3 | 9 min 35 s | Código y tests 9 min 35 s |
| `5c435a4` | docs: add the journal execution document and phase log | T4, T1-T3 | 0 min 44 s | Código y tests 0 min 18 s, Revisión RDD 0 min 26 s |
| `8d01031` | feat(pager): recover journaled commits when a store opens after a crash | T1-T3 | 28 min 59 s | Revisión RDD 28 min 59 s |
| `d6a74c9` | feat: expose deferred commits and making commits durable in the public API | T1-T3, T5 | 1 min 21 s | Código y tests 1 min 17 s, Revisión RDD 0 min 04 s |
| `dd6b2af` | test: crash, power-cut and damage acceptance suites for the journal | T5, T6 | 2 min 51 s | Código y tests 2 min 51 s |
| `6e8f297` | fix(pager): make the journal durable before recording the transfer and record durability after a rollover | T6, B1-B2 | 51 min 11 s | Código y tests 0 min 15 s, Aceptación 47 min 36 s, Correcciones de hallazgos 3 min 20 s |
| `96b6f46` | fix(pager): treat any failed flush to disk as an input/output failure that refuses writes | B1-B2, rdd-successor, R1 | 11 min 39 s | Revisión RDD 4 min 44 s, Correcciones de hallazgos 6 min 55 s |
| `e500816` | fix(pager): refuse writes when a failed append cannot be wiped and recover a poisoned transfer lock | R1 | 3 min 05 s | Correcciones de hallazgos 3 min 05 s |
| — | después del último commit | R1, rdd-advisories, linux-final | 1 h 28 min | Aceptación 1 h 26 min, Revisión RDD 1 min 22 s, Correcciones de hallazgos 0 min 10 s |
| **Total** | | | **3 h 31 min** | |

El total es correcto, pero el reparto entre fases está sesgado porque algunas marcas no se pusieron al cambiar
de trabajo:

- **T4.** La recuperación (`8d01031`) se escribió mientras la primera revisión RDD corría en segundo plano, con
  la marca de fase en «Revisión RDD». Esos 29 min son casi todos código y pruebas.
- **Verificador.** Trabajó mientras la marca de fase decía «Aceptación», sin marca de verificación propia. Por
  eso «Aceptación» incluye unos 47 min de verificación independiente.
- **Lanzador de capturas.** La tarea `rdd-t1-t3` no se cerró: el lanzador de capturas se relanzaba a sí mismo
  (ver Pendiente).

## Informe de cambios (`401014f..e500816`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/muninndb-pager` | 1.810 | 115 | 1.430 | 258 | 13 |
| `crates/muninndb-tx` | 1.294 | 19 | 1.119 | 96 | 5 |
| `docs` | 643 | 0 | — | — | 8 |
| `odd` | 90 | 0 | — | — | 2 |
| `crates/muninndb` | 55 | 11 | 26 | 26 | 3 |
| `README.md` | 19 | 14 | — | — | 1 |
| `.github` | 1 | 0 | 1 | 0 | 1 |
| **Total** | **3.912** | **159** | **2.576** | **380** | **33** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell,
JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+7 −0).

No hubo previsión de líneas.

## Resumen

Una confirmación que devuelve sin error sobrevive ahora a la caída del proceso, del sistema y a un corte de
corriente. Al abrir un almacén caído, la biblioteca lo recupera sola: todo lo durable está, y nada a medias.

- **Modo diferido.** La aplicación puede confirmar en modo diferido, que vuelve sin esperar al disco y sobrevive
  a la caída del proceso. Con `make_durable` hace durable todo lo anterior.
- **Diario.** Archivos `muninn.journal.<19 cifras>`. Cada confirmación es un registro LZ4 alineado a 4 KiB, con
  comprobaciones de integridad y un campo `durable_upto`. Ese campo distingue una cola a medias, que se descarta,
  de un daño real, que se informa.
- **Área temporal.** Las versiones confirmadas que aún no están en `muninn.data` viven en archivos proyectados de
  `muninn.temp`.
- **Traslado.** Se hace al final de cada confirmación, con un candado de intento, así que nunca espera a otro
  traslado.
- **Encabezado.** El formato 2 registra hasta dónde llega lo trasladado, alternando sus dos copias. Los archivos
  del diario se borran solo cuando las dos copias los dejan atrás.

## Decisiones por defecto, sin preguntar

- **Formato 1.** Los encabezados de formato 1 se decodifican con su propia disposición, para que un almacén del
  módulo 2 se rechace como formato anterior y no como dañado.
- **Durabilidad del traslado.** El traslado solo mueve versiones durables. Las diferidas esperan a una
  confirmación durable, a `make_durable`, a una rotación del diario o al cierre.
- **Registro del avance.** El encabezado registra el avance al rotar el diario (archivos de 16 MiB). Al cerrar y
  tras recuperar se escriben las dos copias.
- **Registro interrumpido.** Un registro cuya escritura falla o se interrumpe con un pánico se borra, y el
  borrado se lleva a disco. Si no se puede borrar, el almacén deja de aceptar escrituras.
- **Herramientas de prueba.**
  - Modelos de corte: `Flushed`, `TornFirst`, `SkipFirst` y `Written`.
  - Acción `Panic`.
  - Ganchos de traslado y de `fsync` del diario.
  - Punto de control de prueba que rota el diario.
- **Documento de ejecución.** Se commiteó antes de la primera revisión, porque RDD no acepta archivos sin seguir
  y no documenta el formato de selección.

## Pendiente (no bloqueante)

- **Duración de los barridos AC10 completos.** En Linux pasaron de segundos a 23 min (transacciones) y 61 min
  (diario), porque cada cierre sincroniza disco y cada intento copia el almacén. La CI los corre en los dos
  sistemas y su duración crece. No es un defecto, pero conviene acotarlos (por ejemplo, sincronizar menos en las
  pruebas o barrer solo el diario completo).
- **Avisos aceptados de RDD y del verificador** (no bloqueantes; ninguno abre una pérdida de datos):
  - `make_durable` espera a una confirmación que está en su `fsync`.
  - Las versiones diferidas esperan a una durable para trasladarse.
  - Con una lectura larga, cada traslado sincroniza y reescribe el encabezado.
  - El área temporal reserva bloques de hasta 32 MiB.
  - La recuperación lee los archivos del diario enteros en memoria.
  - El modelo de corte de Windows nunca hace durables los borrados.
  - La rama de cierre de AC-J7 no simula un corte.
- **Incidente del lanzador de capturas RDD.** Un script de la sesión se relanzaba a sí mismo porque su nombre
  coincidía con su propio patrón. Generó unos 6.000 procesos, que se terminaron. Solo afectó a la sesión, no al
  repositorio.
- La rama no se ha subido; push y PR quedan a decisión del usuario.

**Riesgo:** ítems 1 (formato persistido: versión 2 y diario), 2 (código `unsafe` nuevo en el Pager), 4
(concurrencia del traslado con escritores y lectores) y 5 (CI: un barrido de aceptación más). Los cubren:

- las pruebas de cortes en cada operación;
- el verificador independiente, con su recomprobación;
- dos revisiones RDD;
- la suite y los barridos completos en Linux.

No se pasaron Miri ni loom.
