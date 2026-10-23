# Hilo de traslado en segundo plano: terminado, verificado y fusionado en `main`

Informe de cierre de `transfer-background-thread` (2026-10-23). Enfoque de ejecución: ODD.
Fuente de producto: J5 y J6 de [journal.prd.md](../../docs/prd/krummi/journal.prd.md) (corrección de errores, sin PRD
nuevo). Documento de ejecución: [transfer-background-thread.md](transfer-background-thread.md).

Rama `fix/transfer-background-thread`: cinco commits sobre `c075d29` y el de este informe, fusionada en `main` por
avance rápido. No se subió nada.

## Resultado

- **Especificación:** S1–S5 cumplidos.
  - S1: las versiones salen del área temporal sin más confirmaciones, tanto tras una lectura como tras una
    diferida o una carrera.
  - S2: un único hilo de traslado, con señales sin E/S y pasadas agrupadas.
  - S3: fsync de datos solo cuando libera un archivo del diario.
  - S4: cierre, drop y J13 sin cambios.
  - S5: 200 ms, 32 MiB y 2 s como constantes internas.
  - Suite en Windows: 196 pruebas pasan con escritura directa del diario y 196 con `KRUMMI_JOURNAL_BUFFERED=1`. fmt
    y clippy limpios.
- **Aceptación:** sin ejecución de aceptación aparte: es una corrección sin criterio de aceptación nuevo. Las diez
  pruebas nuevas de `crates/krummi-tx/tests/transfer.rs` cubren los casos del encargo, y no se ejecutó en Linux.
- **Revisiones y verificación:**
  - Verificador independiente (Sonnet, alto) sobre `6709631`: S1, S2, S3 y S5 cumplidos. Encontró un bloqueo en S4
    (B1, abajo), corregido en `ae06585`.
  - Sus sondas (6 escritores, 3 lectores, `transfer_now`/`checkpoint` concurrentes, cierre, drop y reapertura)
    pasaron.
  - Tres revisiones RDD, todas aprobadas (sección siguiente).
- **Hallazgos corregidos por el camino:**
  - **Corrección del usuario (L6):** "el hilo de traslado debería ser el único que traslade". El volcado de
    `reserve` por tamaño máximo, `transfer_now` y `checkpoint` trasladaban en el hilo de quien llamaba. Ahora piden
    la pasada al hilo y esperan su resultado. Lo fija `only_the_transfer_thread_transfers`, que falla si el volcado
    de `reserve` vuelve a ser en línea.
  - **B1 (verificador):** un traslado que fallaba con una clase distinta de E/S (disco lleno) dejaba el almacén
    rechazando escrituras para siempre; la base reintentaba con la siguiente confirmación. Ahora solo un fallo que
    hace rechazar escrituras detiene las pasadas y se informa (J13).
  - **R3/R4 (RDD):** tras ese arreglo, una liberación pospuesta ya vencida hacía encadenar pasadas fallidas: entre
    15.617 y 24.965 en 1,5 s.
    - Primer arreglo (`ebf0115`): olvidar el reintento tras un fallo.
    - La segunda revisión vio que eso dejaba versiones esperando a la siguiente confirmación con el almacén
      inactivo.
    - Arreglo final (`e3ea257`): reintentar al acabar el intervalo de pasada.
  - **Pruebas existentes que dependían del traslado síncrono (L7):**
    - AC-E17 forzaba el fallo con un fsync de `krummi.data` que no liberaba nada, justo el bug 2. Ahora falla la
      escritura de página.
    - AC-E11 y AC-E17 pausan el hilo.
    - AC-J9 y AC-J12 esperan al trabajo en segundo plano.
    - AC-J4 cierra un almacén cuyo hilo escribía dentro de las grabaciones siguientes.
    - S3 de journal-followups baja el umbral a 256 KiB.

## Cobertura de los casos del encargo

| Caso | Prueba | Resultado |
|---|---|---|
| 1. Una lectura bloquea, confirmación, la lectura termina | `a_version_a_read_held_back_leaves_once_the_read_ends` | PASS (RED: no se vaciaba en 10 s) |
| 2. La última confirmación es diferida | `a_last_deferred_commit_leaves_once_its_group_is_durable` | PASS (RED: no se vaciaba en 10 s) |
| 3. La carrera del traslado A con la confirmación B | `a_commit_published_during_a_transfer_is_transferred_too` | PASS (RED: no se vaciaba en 10 s) |
| 4. Los fsync de datos no crecen con N | `data_flushes_do_not_grow_with_commits_while_no_file_can_be_released` | PASS (RED: 187 fsync tras 200 confirmaciones frente a 37 tras 50) |
| 5. Agrupación de pasadas | `transfer_passes_coalesce_many_commits` | PASS (RED: una pasada por confirmación) |
| 6. Cierre y drop paran el hilo; fallo J13 | `close_and_drop_end_the_transfer_thread`, `a_transfer_failing_on_the_thread_fails_the_next_operation` | PASS |
| 6. Fallo sin rechazo de escrituras (B1) | `a_transfer_failing_without_refusing_writes_is_retried` | PASS (RED: la siguiente confirmación fallaba con E/S) |
| 6. Fallo repetido sin bucle (R3/R4) | `a_failing_pass_does_not_spin_on_a_due_release` | PASS (RED: 15.617 pasadas; luego 1 pasada sin reintento) |
| S2. Solo traslada el hilo (L6) | `only_the_transfer_thread_transfers` | PASS (falla con el volcado en línea) |
| 7. Pruebas existentes | `cargo test --workspace` | PASS: 196 en cada modo de escritura del diario |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `c075d29..74ab5d1` (`6709631`, `ae06585`, `74ab5d1`) | risk, resilience, reliability | aprobada | R3-retry-at-busy-loop y R4-stale-retry-spin (WARNING, informativos): bucle de pasadas fallidas con una liberación vencida; corregido en `ebf0115` | `review-fec7abde0abc0132` |
| `74ab5d1..ebf0115` | resilience, reliability | aprobada | R3/R4-failure-drops-due-release (WARNING): el reintento olvidado deja versiones esperando a la siguiente confirmación; corregido en `e3ea257`. R3-interval-clamp-unproved (SUGGESTION): la prueba ahora falla sin el recorte. R3-timing-threshold-test (SUGGESTION): se mantiene la ventana de reloj (ver Pendiente) | `review-f25c21c7fef249d3` |
| `ebf0115..e3ea257` | resilience, reliability | aprobada | R4-001 y R3-wallclock-pacing-bounds (SUGGESTION): los límites 2–40 de la prueba dependen del planificador; R3-refused-branch-keeps-retry (SUGGESTION): comprobado que `next_work` no ejecuta pasadas con el almacén fallado, porque la condición `failed` va antes que `retry_at` | `review-335f8ae885f136d8` |

Evaluaciones: `ebf0115` y `e3ea257` salieron `under_budget`, y aun así se revisaron por la regla del último tramo.

## Tiempo

- Estimación: 1 h 30 min – 2 h 30 min. Arranque real: 2026-10-23 18:10:51 +0100. Fin de la verificación: 19:35:56
  +0100.
- **Duración activa desde el arranque: 1 h 25 min**, el 94 % del mínimo estimado. La mayor parte fue código y
  pruebas (48 min), con la reestructuración de `Scratch` y las seis pruebas existentes que dependían del traslado
  síncrono. Le siguen el verificador independiente (12 min) y los checks (11 min).
- Esperas por el usuario: ninguna dentro de la ejecución. La exploración y las preguntas previas al arranque no se
  registraron en el log de fases.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 4 min 42 s | 6 % | 1 |
| Código y tests | 48 min 00 s | 56 % | 1 |
| Checks (fmt, clippy, suite de tests) | 10 min 39 s | 13 % | 3 |
| Verificación independiente | 11 min 43 s | 14 % | 1 |
| Revisión RDD | 6 min 09 s | 7 % | 3 |
| Correcciones de hallazgos | 3 min 51 s | 5 % | 3 |
| **Total activo** | **1 h 25 min** | **100 %** | **12** |

- Inicio: 2026-10-23 18:10:51 +01:00. Fin: 2026-10-23 19:35:56 +01:00.
- Arranque confirmado: 18:10:51. Activo desde el arranque: **1 h 25 min**; antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.
- Pausas entre sesiones (fuera del activo): 0 min 01 s (entre `end` y `close`).
- Fase abierta, no contada: Cierre (informe y merge) desde 19:35:56.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD three lenses (intento fallido del script de lanzamiento, sin capturas) | 19:24:07 | 19:24:07 | 0 min 00 s |
| Revisión RDD | RDD three lenses | 19:24:26 | 19:26:07 | 1 min 41 s |
| Revisión RDD | RDD last slice | 19:29:32 | 19:30:04 | 0 min 32 s |
| Revisión RDD | RDD final slice | 19:35:05 | 19:35:34 | 0 min 29 s |
| **Revisión RDD (total)** | | | | **2 min 42 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `6709631` | fix: transfer committed versions from a background thread | T1 | 57 min 50 s | Plan 4 min 42 s, Código y tests 48 min 00 s, Checks 5 min 08 s |
| `ae06585` | fix: retry transfers that fail without refusing writes | T1, T1-B1 | 14 min 15 s | Checks 0 min 13 s, Verificación independiente 11 min 43 s, Correcciones 2 min 19 s |
| `74ab5d1` | docs: record the transfer-background-thread phase log | T1-B1, T1 | 0 min 25 s | Revisión RDD 0 min 19 s, Correcciones 0 min 06 s |
| `ebf0115` | fix: never rerun a failing transfer pass at once | T1, T1-R3R4 | 5 min 41 s | Checks 1 min 34 s, Revisión RDD 3 min 32 s, Correcciones 0 min 35 s |
| `e3ea257` | fix: retry a failed transfer pass once the pass interval is over | T1-R3R4, T1-R3R4b | 5 min 43 s | Checks 3 min 36 s, Revisión RDD 1 min 16 s, Correcciones 0 min 51 s |
| — | después del último commit | T1-R3R4b | 1 min 10 s | Checks 0 min 08 s, Revisión RDD 1 min 02 s |
| **Total** | | | **1 h 25 min** | |

Código y pruebas se llevó más de la mitad del tiempo: la reestructuración en `Arc<Shared>`, las peticiones al hilo
tras la corrección L6 y la depuración de un cuelgue en AC-E17 (un pánico dentro de un `scope` esperando a un commit
bloqueado).

## Informe de cambios (`c075d29..e3ea257`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi-pager` | 728 | 147 | 547 | 134 | 5 |
| `crates/krummi-tx` | 392 | 15 | 327 | 45 | 6 |
| `odd` | 106 | 0 | — | — | 2 |
| `README.md` | 5 | 2 | — | — | 1 |
| **Total** | **1.231** | **164** | **874** | **179** | **14** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell,
JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

No hubo previsión de líneas. El primer commit (unas 1.100 líneas) supera la heurística de 400: T1 y T2 se unieron
porque la liberación de archivos se reescribió en la misma reestructuración.

## Resumen

Lo confirmado ya no espera a la siguiente confirmación para llegar a `krummi.data`. Un hilo de traslado por almacén
hace todos los traslados mientras está abierto, y las confirmaciones, el fin de una lectura y un grupo del diario
hecho durable solo lo despiertan. Ese hilo agrupa las señales en pasadas de como mucho una cada 200 ms (al momento
desde 32 MiB en el área temporal). Lleva `krummi.data` y la cabecera al disco solo cuando eso libera un archivo del
diario, escribiendo las dos copias de la cabecera en la misma pasada y como mucho una vez cada 2 s.

El volcado de un escritor que necesita espacio y las operaciones de prueba piden la pasada al hilo y esperan, así
que no hay un segundo hilo que traslade. La decisión de qué archivos se liberan sigue en un solo sitio
(`State::removable`, que usan `remove_files` y `can_remove_files`). Un fallo de E/S deja el almacén sin aceptar
escrituras (J13); un disco lleno se reintenta en la pasada siguiente.

## Decisiones tomadas sin preguntar

- El hilo arranca antes de la recuperación y recibe el almacén por un canal: un fallo al arrancarlo deja el almacén
  intacto, como E13 exige al hilo del diario.
- El hook de traslado de pruebas se llama tras elegir lo que se escribe y en todo volcado, también los forzados.
- `transfer_now` ignora el intervalo entre fsync de datos.
- `Scratch::new` (pruebas unitarias del área temporal) no tiene hilo y traslada en línea.
- La prueba S3 de journal-followups baja el umbral a 256 KiB para conservar su límite de 2 MiB.
- Un hook de pruebas nuevo, `set_transfer_paused`, para las pruebas cuyos fallos inyectados van a otro hilo.
- T2 se unió a T1 en un único commit.
- Un fallo que no hace rechazar escrituras se reintenta al acabar el intervalo de pasada, en lugar de esperar a una
  confirmación.

## Pendiente (no bloquea)

- **A1:** `Pager::data_syncs` es público sin `test-hooks`, igual que los demás métodos "(tests)" del crate. No
  cambia ningún comportamiento.
- **A2:** si el hilo muere por un pánico, las peticiones se hacen en el hilo de quien llama y las pasadas paran hasta
  el cierre. Está fuera de dominio, y la base propagaba el pánico al que confirmaba.
- **A3:** con más de 32 MiB retenidos por una lectura larga, cada confirmación lanza una pasada. No es más que el
  traslado por confirmación de la base.
- **R3/R4 (sugerencias):** `a_failing_pass_does_not_spin_on_a_due_release` usa una ventana de reloj de 1,5 s con
  límites de 2 a 40 pasadas. Un recuento de pasadas sin reloj no distingue reintentos con ritmo de un bucle. Puede
  fallar en un equipo muy cargado; no ocurrió en las 5 ejecuciones de su versión final (3 del binario y 2 de la suite).
- Las grabaciones de corte de luz comparten el estado de fallos con los hilos de cualquier almacén abierto por la
  misma prueba. Una prueba nueva que deje un almacén vivo durante otra grabación debe cerrarlo antes, como AC-J4.
- Sin ejecución en Linux.

**Riesgo:** item 4 (un hilo nuevo, el orden de las señales con condvar y el apagado). Lo cubren el verificador
independiente y las tres revisiones RDD.
