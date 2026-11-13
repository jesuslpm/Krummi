# Pendientes del diario resueltos: barridos AC10 de minutos y cuatro avisos cerrados

Informe de cierre del cambio `journal-followups` (2026-10-23). Enfoque de ejecución: ODD, sin PRD nuevo: no cambia
ninguna regla de producto del [PRD de diario y recuperación](../../docs/prd/muninndb/journal.prd.md).
Documento de ejecución: [journal-followups.md](journal-followups.md). Origen: los pendientes de
[journal-report.md](journal-report.md).

Rama `feat/journal-followups`, con 10 commits desde `8168fa1`. Se mergea en `main` en local con fast-forward. No
se ha subido nada.

## Resultado

- **Especificación:** S1–S5 cumplidas.
  - **S1.** Cerrar un almacén que nadie cambió no escribe nada; si una copia del encabezado llegó dañada, el cierre
    la repara. Los barridos AC10 omiten los `fsync` con un atajo que solo existe en las pruebas.
  - **S2.** La recuperación lee el diario registro a registro.
  - **S3.** Las confirmaciones diferidas ya no se acumulan sin límite en `muninn.temp`.
  - **S4.** El área temporal crece en archivos de 8 MiB como máximo.
  - **S5.** La rama de cierre de AC-J7 corta la corriente después del cierre.
  - Suite en verde en Windows y en Linux (WSL Ubuntu, Rust 1.99.0, sobre `0574433`, 64 s). Cinco pruebas
    nuevas, todas vistas en RED antes de su arreglo, salvo S2: la memoria máxima no tiene una prueba ejecutable
    razonable, y su comportamiento lo cubren las 24 pruebas de recuperación.
- **Aceptación** (Linux): barridos AC10 completos en verde.

  | Barrido | Antes | Ahora |
  |---|---|---|
  | Pager | 51 s | 54 s |
  | Almacén con transacciones | 1.362 s | 27 s |
  | Almacén caído con diario | 3.666 s | 158 s |

- **Revisiones y verificación:**
  - **Verificador independiente** (Sonnet, esfuerzo alto): sin bloqueantes.
    - Comparó con la base `8168fa1` 738 resultados de daño y cortes de corriente, todos idénticos.
    - Corrió 4.245 cortes con confirmaciones diferidas, sin ningún resultado malo.
    - Avisos: uno corregido y uno aceptado (abajo).
  - **RDD:** dos revisiones, aprobadas.
- **Hallazgos corregidos por el camino:**
  - **Copia dañada sin reparar** (verificador): un cierre sin cambios ya no reparaba una copia dañada del
    encabezado, así que dañar después la otra dejaba el almacén sin abrir. Corregido en `ecfbb36`, con prueba en
    RED antes del arreglo.
  - **Lint de tamaño** (clippy, `f567b8f`): el Pager creció y `Created` pasó el umbral de tamaño entre variantes.
    Se mete el Pager en una `Box`.
  - **Atajo apagado** (`0574433`): el barrido del diario seguía tardando 31 min porque activaba el atajo de los
    `fsync` antes de grabar su ejecución, y grabar reinicia el estado de pruebas del hilo. Una medición en Linux
    (1,4 ms por intento) lo destapó.

## RDD reviews

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `8168fa1..63e1d5c` | risk, resilience, reliability | aprobada | Solo avisos. WARNING sobre la condición del cierre sin cambios: comprobado que solo se cumple sin confirmaciones; se mantiene. SUGGESTION sin aserción del orden J5, límite de memoria de S2 sin prueba, atajo de `fsync` sin reiniciar, `fsync` forzado en el traslado, doble lectura en la recuperación, más archivos temporales; aceptadas. | `review-c0a3d835b18c9657` |
| `63e1d5c..0574433` | resilience, reliability | aprobada | SUGGESTION R3-001 sobre la prueba nueva de reparación del encabezado; aceptada. | `review-49ee8fc8ea327cbb` |

## Tiempo

- Estimación: 1 h 30 min – 3 h. Arranque: 2026-10-23 06:33:18 +0100. Fin: 2026-10-23 08:03:09 +0100.
- **Duración activa desde el arranque: 1 h 29 min**, justo en el mínimo de la estimación.
- Hubo dos tramos largos:
  - La verificación independiente: unos 36 min.
  - El barrido del diario: 27 min en «Correcciones», sobre todo la espera de una pasada completa en Linux que no
    mejoró, hasta medir y encontrar el fallo.
- Ese tiempo incluye también las respuestas a varias preguntas del usuario durante la ejecución: escritura directa
  y diff de Voron, el WAL de CanopyDB, la fusión de transacciones de RavenDB, el *group commit* y el nombre del
  producto.
- Esperas por el usuario: 47 s, la confirmación de la estimación.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 0 min 14 s | 0 % | 1 |
| Código y tests | 10 min 42 s | 12 % | 4 |
| Aceptación | 6 min 54 s | 8 % | 2 |
| Verificación independiente | 35 min 46 s | 40 % | 2 |
| Revisión RDD | 1 min 16 s | 1 % | 1 |
| Correcciones de hallazgos | 34 min 59 s | 39 % | 2 |
| **Total activo** | **1 h 29 min** | **100 %** | **12** |

- Inicio: 2026-10-23 06:32:31 +01:00. Fin: 2026-10-23 08:03:09 +01:00.
- Arranque confirmado: 06:33:18. Activo desde el arranque: **1 h 29 min**; antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 0 min 47 s.
  - 06:32:31–06:33:18 (0 min 47 s): estimate

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | lenses | 06:44:57 | 06:46:16 | 1 min 19 s |
| Aceptación | linux | 06:44:31 | 07:18:42 | 34 min 11 s |
| Aceptación | journal-sweep | 07:19:52 | 07:52:36 | 32 min 44 s |
| **Aceptación (total)** | | | | **1 h 06 min** |
| **Revisión RDD (total)** | | | | **1 min 19 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `59d673a` | perf: write nothing when closing an unchanged store and skip flushes in the AC10 sweeps | T1 | 6 min 13 s | Exploración y mapa 0 min 14 s, Código y tests 5 min 59 s |
| `a9baf5d` | perf(pager): recover the journal one record at a time instead of reading whole files | T2 | 0 min 50 s | Código y tests 0 min 50 s |
| `46d64c8` | feat(pager): transfer deferred commits once enough are pending instead of waiting for a durable one | T2, T3 | 1 min 50 s | Código y tests 1 min 50 s |
| `908a069` | fix(pager): grow the temporary area in files of at most 8 MiB and power-cut the AC-J7 close | T4 | 1 min 55 s | Código y tests 1 min 55 s |
| `63e1d5c` | docs: add the journal follow-ups execution document and phase log | T4 | 0 min 08 s | Código y tests 0 min 08 s |
| `3b91a62` | test: reuse one folder in the journal AC10 sweep | linux, sweep | 35 min 37 s | Aceptación 0 min 53 s, Verificación independiente 33 min 53 s, Correcciones de hallazgos 0 min 51 s |
| `ecfbb36` | fix(pager): let a close without changes repair a damaged header copy and document when deferred commits flush | sweep, V1 | 4 min 38 s | Verificación independiente 1 min 53 s, Correcciones de hallazgos 2 min 45 s |
| `f567b8f` | fix(pager): box the created pager to keep the creation result small | V1 | 1 min 34 s | Correcciones de hallazgos 1 min 34 s |
| `0574433` | test: skip flushes in the journal AC10 sweep after recording its run | V1 | 27 min 20 s | Correcciones de hallazgos 27 min 20 s |
| — | después del último commit | V1, rdd-last, linux-final | 9 min 46 s | Aceptación 6 min 01 s, Revisión RDD 1 min 16 s, Correcciones de hallazgos 2 min 29 s |
| **Total** | | | **1 h 29 min** | |

## Informe de cambios (`8168fa1..0574433`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/muninndb-pager` | 158 | 47 | 128 | 21 | 8 |
| `crates/muninndb-tx` | 140 | 18 | 121 | 14 | 5 |
| `odd` | 46 | 0 | — | — | 2 |
| `crates/muninndb` | 2 | 1 | 0 | 2 | 1 |
| **Total** | **346** | **66** | **249** | **37** | **16** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell,
JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

## Resumen

Lo pendiente del módulo 3 quedó cerrado, sin cambiar ninguna regla del producto:

- Abrir y cerrar un almacén sin cambios ya no lo reescribe.
- La recuperación usa memoria acotada por el registro más grande.
- Las confirmaciones diferidas llegan a `muninn.data` aunque nunca llegue una durable.
- El área temporal reserva disco en pasos de 8 MiB como máximo.
- Los barridos de aceptación AC10 pasan de una hora larga a menos de cuatro minutos en Linux, lo que abarata
  mucho la CI.

## Decisiones por defecto, sin preguntar

- **Diferida que alcanza el umbral.** La confirmación diferida que alcanza 256 pendientes o 1 MiB espera al `fsync`
  antes de volver. Eso va contra la letra de J10 («devuelve sin esperar a ser durable»), así que queda
  documentado en la API. Resolverlo bien necesita un hilo aparte, que trae la propuesta de escritura agrupada.
- **Umbrales.** 256 confirmaciones o 1 MiB pendientes, y archivos temporales de 8 MiB.
- **Pruebas sin `fsync`.** Los barridos AC10 desactivan los `fsync`, porque prueban la detección de daño y no la
  durabilidad; la durabilidad la cubren las pruebas de cortes.

## Pendiente (no bloqueante)

- **Lectura fallida en la recuperación.** Si una lectura falla en la segunda pasada, `muninn.data` puede quedar
  a medio escribir. El diario sigue ahí y la siguiente apertura repite la recuperación con el resultado exacto;
  J8 lo admite.
- **Propuesta de PRD «Diario: escritura agrupada y directa»**, surgida de las preguntas del usuario:
  - archivos del diario reservados de antemano, con `sync_data`;
  - escritura directa (`O_DIRECT | O_DSYNC` en Linux, *write-through* en Windows) con alternativa;
  - un hilo del diario que escribe de una vez las confirmaciones encoladas.

  Decisión del usuario: «Commit debería esperar a que todo esté durable, pero primero suelta el turno de
  transacciones». Queda abierto qué ven la siguiente escritura y las lecturas. Medir antes con un microbanco o el
  banco del módulo 6.
- **Cambio de nombre a Krummi**, decisión del usuario: «Pues Krummi sin suijo detrás». MuninnDB está tomado por
  una base de datos de memoria para IA. Va como cambio aparte, con su PRD: toca crates, la marca del archivo y los
  nombres reservados, así que es un formato nuevo, sin migración antes de la 1.0.
- La rama no se ha subido; push y PR quedan a decisión del usuario.

**Riesgo:** ítems 1 (la recuperación y el cierre escriben `muninn.data` y el encabezado) y 4 (el traslado forzado
de diferidas). Los cubren:

- las pruebas de cortes en cada operación;
- las sondas diferenciales del verificador contra la base;
- dos revisiones RDD;
- la suite y los barridos completos en Linux.
