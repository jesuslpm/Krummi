# Escritura agrupada y directa del diario: terminada, verificada y fusionada en `main`

Informe de cierre de `journal-group-commit` (2026-10-23). Enfoque de ejecución: ODD.
Fuente de producto: [journal-group-commit.prd.md](../../docs/prd/krummi/journal-group-commit.prd.md).
Documento de ejecución: [journal-group-commit.md](journal-group-commit.md).

Rama `feat/journal-group-commit`, diez commits sobre `d88a74a`, fusionada en `main` por avance rápido. No se subió
nada.

## Resultado

- **Especificación:** S1–S19 (AC-E1–AC-E18 y los criterios heredados). Suite en Windows: 186 pruebas pasan con
  escritura directa y 186 con la alternativa forzada (`KRUMMI_JOURNAL_BUFFERED=1`); fmt y clippy limpios.
- **Aceptación:** Linux (WSL 2 Ubuntu, ext4, Rust 1.99.0) sobre `5b44336`: 186 pruebas pasan en cada modo (58 s y
  61 s), fmt y clippy limpios; barridos AC10 completos en release sobre `4e38ff9`: Pager 1,9 s, transacciones 16 s,
  diario 30 s. Medición de E16 en las dos plataformas (sección Medición).
- **Revisiones y verificación:** cinco revisiones RDD, todas aprobadas (sección siguiente). Verificador
  independiente (Sonnet, alto) sobre `4e38ff9`: sin bloqueos; S1–S14 y S16–S19 cumplidos, S6 parcialmente (el
  punto `Handoff` después del punto sin retorno no se ejercita; los puntos los deja el PRD a la implementación) y
  S15 pendiente de este informe. Su prueba diferencial contra `d88a74a` con la API pública común dio la misma
  salida.
- **Hallazgos corregidos por el camino:**
  - Un pánico al reservar un archivo del diario dejaba el archivo y bloqueaba la confirmación siguiente; ahora se
    borra al salir (T3).
  - La prueba de AC-T5 se colgaba en los puntos nuevos de la confirmación; ahora comprueba todo o nada también
    después del punto sin retorno (T4).
  - La alternativa a la escritura directa solo se activaba si fallaba la apertura; ahora también con una escritura
    de prueba al crear el archivo (revisión de T1–T2).
  - Carreras en las pruebas de AC-E1, AC-E11 (revisión de T4) y AC-J8 (verificador, falló una vez en Linux).
  - Mi filtro de la suite ocultó una vez errores de compilación de las pruebas internas del Pager; desde entonces
    cuento las pruebas que corren.

## Cobertura de los criterios de aceptación

| Criterio | Prueba | Resultado |
|---|---|---|
| AC-E1 | `group_commit::ac_e1_the_turn_is_released_at_the_point_of_no_return` | PASS |
| AC-E2 | `group_commit::ac_e2_commits_waiting_together_share_one_write`, `recovery::ac_e2_many_threads_commit_at_once_and_survive_a_crash` | PASS (16 × 1.000) |
| AC-E3 | `recovery::ac_e3_power_cuts_with_groups_leave_a_prefix` | PASS |
| AC-E4 | `recovery::ac_e4_no_space_to_reserve_the_journal_fails_before_the_point_of_no_return` | PASS |
| AC-E5 | `group_commit::ac_e5_a_failed_group_fails_its_commits_and_the_store_refuses_writes` | PASS |
| AC-E6 | `group_commit::ac_e6_a_panic_after_the_point_of_no_return_keeps_the_commit` | PASS (puntos `CommitQueued` y `CommitWait`) |
| AC-E7 | `group_commit::ac_e7_a_deferred_commit_returns_without_waiting` | PASS |
| AC-E8 | `recovery::ac_e8_deferred_commits_become_durable_like_durable_ones` | PASS |
| AC-E9 | `recovery::ac_e9_journal_files_keep_their_reserved_size` | PASS |
| AC-E10 | `group_commit::ac_e10_the_close_writes_what_is_queued` | PASS |
| AC-E11 | `group_commit::ac_e11_a_commit_without_changes_waits_for_what_it_saw` | PASS |
| AC-E12 | `group_commit::ac_e12_making_durable_waits_for_queued_commits` | PASS |
| AC-E13 | `group_commit::ac_e13_failures_of_the_journals_writer` | PASS |
| AC-E14 | `krummi-pager open::ac_e14_format_3_is_a_previous_format_and_new_stores_have_format_4` | PASS |
| AC-E15 | Este informe, sección Medición | PASS |
| AC-E16 | `recovery::ac_e16_damage_is_judged_by_group` | PASS |
| AC-E17 | `group_commit::ac_e17_a_commit_waiting_for_room_in_the_queue` | PASS |
| AC-E18 | `group_commit::ac_e18_a_temporary_store_shows_a_deferred_commit_at_once` | PASS |
| Heredados | Suite completa en los dos modos, Windows y Linux; adaptaciones de AC-J7, AC-J14, AC-T19, AC-T5, AC-J3, AC10 y AC-J9 según el PRD | PASS |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `d88a74a..92b6345` | risk, resilience, reliability | approved | WARNING R1-001, R4-direct-fallback-open-only y R3-001 (el mismo): la alternativa solo se activaba al fallar la apertura y ninguna prueba la forzaba; corregido en T3 con una escritura de prueba y su test. SUGGESTION R3-002: nada comprobaba que `ALIGN` fuera múltiplo de `DIRECT_ALIGN`; aserción en compilación en T3. | `review-a5c8fce6cba72e73` |
| `92b6345..6507bb2` | risk, resilience, reliability | approved | WARNING R1-001 y R4-failed-durable-commit-applied-on-close: una confirmación que falló puede quedar escrita al cerrar; E12 lo admite y T4 lo documenta en `commit`. WARNING R4-group-failure-loses-diskfull-class y R3-001: el fallo de un grupo siempre es de entrada y salida; lo fija E8, aceptado. WARNING R3-002: faltaba instalar el aviso de fallos; T4. SUGGESTION R3-003: el contador de páginas se guardaba tras la espera; T4 lo guarda en el punto sin retorno. | `review-7e3575952b966f27` |
| `6507bb2..07c655d` | risk, resilience, reliability, readability | approved | WARNING R3-001 y R3-002: carreras en AC-E1 y AC-E11; corregidas en `6165f93` con `next_writer`. SUGGESTION R2-failure-paths-diverge, R2-duplicated-queue-helper y R2-t10-citation: un único `refuse_writes`, el ayudante en el módulo común y fuera la constante sin uso, en `6165f93`. | `review-26a4cd1b5641bf9e` |
| `07c655d..5b44336` | reliability, readability | approved | WARNING R2-001: E16 frente a AC-E16, las dos referencias eran correctas; aclarado en `fee50a9`. SUGGESTION R2-002: la columna de modo repite lo pedido; documentado en `fee50a9`. SUGGESTION R3-next-writer-busy-poll: espera activa en un ayudante de pruebas; aceptado. | `review-2ef84e644b718821` |
| `5b44336..fee50a9` | readability | approved | SUGGESTION R2-l11-journal-writes-label: la tabla de medición debe decir que su columna de modo repite lo pedido; recogido en la sección Medición. | `review-b4905c1137f469b4` |

Evaluaciones sin revisión: T1 (`under_budget`, revisado con T2) y T3 (`under_budget`, elevado a alto por el punto 4,
concurrencia).

## Medición (E16)

`cargo run --release -p krummi-tx --example commit_rate`, 3 s por número de hilos, confirmaciones durables por
segundo. La columna de modo repite lo pedido con `KRUMMI_JOURNAL_BUFFERED`; si un sistema de archivos rechazara la
escritura directa, el motor usaría la alternativa sin decirlo (E6).

| Plataforma | Escritura del diario | 1 hilo | 4 hilos | 16 hilos |
|---|---|---|---|---|
| Windows 11, NTFS | directa | 11.390 | 22.110 | 10.129 |
| Windows 11, NTFS | alternativa | 474 | 1.012 | 3.189 |
| Linux (WSL 2, ext4) | directa | 117 | 256 | 1.572 |
| Linux (WSL 2, ext4) | alternativa | 86 | 274 | 1.239 |

Agrupar multiplica por unos 7 (Windows, alternativa) y por 13 (Linux, directa) con 16 hilos. En Windows con
escritura directa, 16 hilos rinden menos que 4. En WSL las dos columnas se parecen: allí una escritura con `O_DSYNC`
cuesta casi lo mismo que una escritura más `fdatasync`. No hay umbral (D5).

## Tiempo

- Estimación: 3–5 h. Arranque real: 2026-10-23 11:05:52 +0100. Fin de la verificación: 13:14:21 +0100.
- **Duración activa desde el arranque: 2 h 08 min**, el 71 % del mínimo estimado. El código y sus pruebas se
  llevaron casi la mitad; la otra gran parte fue esperar al verificador independiente.
- Esperas por el usuario: una, de 36 s (11:05:16–11:05:52), la confirmación de la estimación.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 1 min 56 s | 1 % | 1 |
| Plan (documento de ejecución) | 2 min 12 s | 2 % | 1 |
| Código y tests | 1 h 02 min | 48 % | 5 |
| Checks (fmt, clippy, suite de tests) | 1 min 07 s | 1 % | 1 |
| Revisión RDD | 7 min 21 s | 6 % | 4 |
| Correcciones de hallazgos | 6 min 07 s | 5 % | 2 |
| Documentación | 48 min 35 s | 37 % | 1 |
| **Total activo** | **2 h 10 min** | **100 %** | **15** |

- Inicio: 2026-10-23 11:03:20 +01:00. Fin: 2026-10-23 13:14:21 +01:00.
- Arranque confirmado: 11:05:52. Activo desde el arranque: **2 h 08 min** (compárese con la estimación); antes del arranque: 1 min 56 s.
- Esperas por el usuario (fuera del activo): 0 min 36 s.
  - 11:05:16–11:05:52 (0 min 36 s): estimate
- Pausas entre sesiones (fuera del activo): 0 min 11 s.
  - 2026-10-23 13:14:10 +01:00 – 2026-10-23 13:14:21 +01:00 (0 min 11 s)

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD T1-T2 | 11:25:59 | 11:27:31 | 1 min 32 s |
| Revisión RDD | RDD T3 | 11:47:50 | 11:49:38 | 1 min 48 s |
| Revisión RDD | RDD T4 | 12:13:17 | 12:14:11 | 0 min 54 s |
| Aceptación | Linux suite and measurement | 12:20:20 | 12:22:31 | 2 min 11 s |
| Aceptación | Linux suites | 12:22:48 | 12:22:52 | 0 min 04 s |
| Aceptación | Linux suites | 12:23:04 | 12:25:07 | 2 min 03 s |
| Aceptación | Linux full AC10 sweeps | 12:25:47 | 12:28:42 | 2 min 55 s |
| Aceptación | Linux suites final | 13:10:54 | 13:12:58 | 2 min 04 s |
| Revisión RDD | RDD last slice | 13:10:54 | 13:12:58 | 2 min 04 s |
| **Aceptación (total)** | | | | **9 min 17 s** |
| **Revisión RDD (total)** | | | | **6 min 18 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `6f419c7` | feat: bump the store format to 4 for grouped journal writes | T1 | 8 min 12 s | Exploración y mapa 1 min 56 s, Plan (documento de ejecución) 2 min 12 s, Código y tests 4 min 04 s |
| `92b6345` | feat: reserve journal files and write them directly to the disk | T1, T2 | 12 min 38 s | Código y tests 12 min 38 s |
| `6507bb2` | feat: write the journal from its own thread in groups | T2, T1-T2, T3 | 22 min 15 s | Código y tests 19 min 51 s, Revisión RDD 2 min 24 s |
| `07c655d` | feat: release the write turn at the commit's point of no return | T3, T4 | 25 min 54 s | Código y tests 24 min 57 s, Revisión RDD 0 min 57 s |
| `6165f93` | fix: share the refuse-writes path and remove races from the group commit tests | T4, T5, T4-review | 6 min 42 s | Código y tests 1 min 26 s, Revisión RDD 0 min 28 s, Correcciones de hallazgos 4 min 48 s |
| `4e38ff9` | feat: add the durable commit rate measurement |  | 0 min 00 s |  |
| `e48581d` | docs: describe grouped and direct journal writes in the README | T4-review, T6 | 5 min 55 s | Checks (fmt, clippy, suite de tests) 1 min 07 s, Correcciones de hallazgos 0 min 07 s, Documentación 4 min 41 s |
| `5b44336` | test: pause the journal's writer before failing the flush in AC-J8 | T6, verifier-A1 | 44 min 42 s | Correcciones de hallazgos 0 min 48 s, Documentación 43 min 54 s |
| `fee50a9` | docs: clarify what the commit rate measurement reports | verifier-A1, last-slice | 3 min 08 s | Revisión RDD 2 min 44 s, Correcciones de hallazgos 0 min 24 s |
| — | después del último commit | last-slice | 0 min 48 s | Revisión RDD 0 min 48 s |
| **Total** | | | **2 h 10 min** | |

Lectura: los 48 min de Documentación son casi todos la espera al verificador independiente (12:21–13:07), que
corrió en segundo plano sin un tramo propio en el registro. Se alargó porque lanzó por su cuenta los barridos AC10
completos en Windows en debug (27 min uno solo) y se lo detuvo; el código y sus pruebas (1 h 02 min) son el resto.
`4e38ff9` sale con 0 min porque se commiteó junto a `6165f93`.

## Informe de cambios (`d88a74a..fee50a9`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi-pager` | 990 | 250 | 774 | 162 | 9 |
| `crates/krummi-tx` | 1.069 | 73 | 911 | 113 | 8 |
| `odd` | 211 | 0 | — | — | 2 |
| `crates/krummi` | 18 | 12 | 2 | 16 | 2 |
| `README.md` | 10 | 5 | — | — | 1 |
| **Total** | **2.298** | **340** | **1.687** | **291** | **22** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Sin archivos de bloqueo ni generados. No hubo previsión de líneas. `6507bb2` (1.369 líneas) superó la guía de unas
400 por commit: el hilo del diario, la cola y la visibilidad solo de lo durable no se podían separar sin romper la
suite.

## Resumen

Varias escrituras confirmadas desde hilos distintos comparten ahora una sola escritura del diario. Una confirmación
entra en la cola del diario y cede el turno (su punto sin retorno); un hilo propio del diario escribe todo lo
encolado de una vez, directamente al disco cuando el sistema de archivos lo admite, y la confirmación durable
devuelve cuando su grupo es durable. La escritura siguiente ve enseguida lo encolado; las lecturas solo ven lo
durable. La diferida devuelve al instante y es durable en el mismo momento que lo sería una durable. Los archivos
del diario nacen con su tamaño final, así que la falta de espacio se detecta antes del punto sin retorno, y después
de él solo puede fallar la entrada y salida, que el cierre reintenta. El formato pasa a `4`.

## Decisiones tomadas sin preguntar

- El traslado a `krummi.data` sigue en el hilo que confirma, después de que la confirmación devuelve.
- El encabezado nunca registra como aplicado algo que no sea durable, así que el traslado ya no espera al diario.
- Los archivos se reservan con `set_len`/`fallocate`, sin rellenar con ceros, y la comprobación de escritura
  directa se hace al crear cada archivo.
- La alternativa se fuerza en las pruebas con `KRUMMI_JOURNAL_BUFFERED`, que solo leen las compilaciones con
  ganchos de prueba; esas compilaciones usan archivos del diario de 256 KiB (16 MiB en las demás).
- Cola del diario de 8 MiB antes de que una confirmación espere sitio.
- Se quitó la prueba del borrado de un registro a medio escribir: ese borrado ya no existe (E8).

## Pendiente (no bloqueante)

- **16 hilos más lentos que 4 con escritura directa en Windows:** cada grupo despierta a todas las confirmaciones
  que esperan, que usan una sola variable de condición. No hay umbral hasta el banco del módulo 6.
- **S6 parcial:** el punto `Handoff` después del punto sin retorno no se interrumpe en las pruebas; un pánico en
  ese gancho, que solo existe en pruebas, saltaría el despertar (como en la base).
- **Relleno sin comprobar:** un byte alterado en el relleno de ceros tras un registro no se detecta (como en la
  base); no cambia ningún dato leído.
- **Rojo no observado** para los criterios de T4: el archivo de pruebas usa los puntos nuevos y no compila contra
  T3.

**Riesgo:** puntos 1 (cambio del formato y de cómo se escribe el diario) y 4 (concurrencia: hilo del diario y turno
cedido antes de ser durable). Cubiertos por las cinco revisiones RDD, el verificador independiente y las suites en
los dos modos y las dos plataformas.

## Limpieza manual

- `~/muninn-accept` en WSL: el clon de aceptación de Linux, ahora en `5b44336`, con su carpeta `target`. Se puede
  borrar o renombrar.
