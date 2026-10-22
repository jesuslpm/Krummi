# Transacciones terminadas: verificadas en Windows y Linux, con cuatro revisiones RDD aprobadas

Informe de cierre del cambio `transactions` (2026-10-22). Enfoque de ejecución: ODD.
Producto: [PRD de transacciones](../../docs/prd/muninndb/transactions.prd.md) v1.1, con su ascendencia
([Pager](../../docs/prd/muninndb/pager.prd.md) y [raíz](../../docs/prd/muninndb/muninndb.prd.md)).
Documento de ejecución: [transactions.md](transactions.md).

Rama `feat/transactions`, mergeada en `main` en local con fast-forward. **No se subió nada**: la CI de GitHub no ha
corrido sobre estos commits.

## Resultado

- **Especificación:** S1–S7 cumplidas, con todos los criterios del PRD trazados a una prueba en el documento de
  ejecución.
  - Suite de Windows en verde: 41 + 1 pruebas unitarias del Pager con el área temporal, 19 de apertura, 15 de
    páginas, 17 de confirmación, 8 de la cola, 3 de hilos, 4 de la API pública, 2 de almacén y 3 de la política
    de `unsafe`.
  - Las pruebas de no compilación cubren el cierre con transacciones abiertas, el reuso tras confirmar y las
    referencias a páginas que sobreviven a su transacción o a un cambio. Las de «no es `Send`/`Sync`» son
    aserciones estáticas.
- **Aceptación** (Linux en WSL, Ubuntu, Rust 1.99.0):
  - `cargo test --workspace` en verde;
  - barrido AC10 completo del Pager: 69,56 s;
  - barrido AC10 completo de un almacén escrito con transacciones: 24,31 s.

  El barrido nuevo se añadió a la CI, que lo correrá en los dos sistemas al subir la rama.
- **Revisiones y verificación:**
  - **RDD:** cuatro revisiones, todas aprobadas (detalle abajo).
  - **Verificador independiente** (Sonnet, esfuerzo alto):
    - Lanzó sondas de estrés de 4 escritores × 300 confirmaciones con 6 lectores y de 12 hilos × 400 peticiones
      con límites al azar. No vio confirmaciones a medias, cuelgues ni actualizaciones perdidas.
    - El módulo 1 da una salida idéntica byte a byte a la de la base.
    - Encontró un bloqueante (B1), ya corregido.
- **Hallazgos corregidos por el camino:**
  - **`ScratchWriter` era `Sync`** (revisión 1): desde código seguro se podían lanzar dos escrituras en su sitio a
    la vez. Ahora no es ni `Send` ni `Sync` (`f87b5d9`).
  - **B1** (verificador): las versiones que ya nadie podía leer se apartaban como retiradas y solo se liberaban al
    terminar la lectura más antigua. Una lectura larga y 3000 reescrituras de una página retenían unos 24 MiB
    fuera del tamaño máximo. Ahora se liberan en el acto (`610bc40`), con la prueba en RED antes de corregir. La
    sonda del verificador pasó a 16 KiB, y a 0 al terminar la lectura.
  - **Pruebas de la cola que dependían del reloj** (revisión 2): corregidas en `1632e39`.
  - **Comprobación de `Sync` frágil.** El texto del compilador cambia entre `Sync` y `std::marker::Sync` según los
    features unificados, y la prueba fallaba con `--workspace`. Se sustituyó por una aserción estática.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `ab1223c..8ee4c66` (T1) | risk, resilience, reliability | aprobada | WARNING: `ScratchWriter` era `Sync`; corregido en `f87b5d9`. WARNING: `allow_writes_again` también borra un rechazo causado por un `fsync` fallido; aceptado, porque en el módulo 2 nada sincroniza antes del cierre, y el cierre vuelve a sincronizar e informa. WARNING y SUGGESTION: APIs nuevas del Pager sin pruebas propias; cubiertas por las de transacciones. | `review-20ce3d3cb04d18df` |
| `8ee4c66..0aab615` (T2) | risk, resilience, reliability | aprobada | Dos WARNING de pruebas de orden dependientes del reloj y una SUGGESTION sobre el traspaso; corregidas en `1632e39`. SUGGESTION: contrato `Send`/`Sync` sin prueba; hecho en `c97af52`. SUGGESTION: el error de E/S de un almacén temporal lleva una carpeta vacía; aceptado, porque es la convención de `Error::Io` desde el módulo 1. | `review-8a0f199c928b52a3` |
| `0aab615..ce893ab` (T3–T4) | readability, reliability | aprobada | SUGGESTION: nombre de un caso de no compilación y un valor de hilo sin usar; corregidos en `c88d1fa`. SUGGESTION: las sondas `NotSend`/`NotSync` están copiadas en tres crates; aceptado, una copia junto a su uso. WARNING: el texto de `trybuild` depende del compilador; aceptado, porque la CI fija Rust 1.99.0. | `review-5b0933ca6fbb647a` |
| `ce893ab..c88d1fa` (corrección de B1) | risk, reliability | aprobada | WARNING: posible doble resta del contador de bytes; comprobado que no ocurre. SUGGESTION: falta una instantánea intermedia en la prueba nueva; ya la cubre `a_version_an_open_snapshot_reads_is_kept`. | `review-e0c7e39c5e00b78f` |

La evaluación del último tramo dio `under_budget` (85 líneas). Se revisó igualmente, porque cierra la ejecución.

## Tiempo

- Estimación: 1,5–3 h. Inicio real: 2026-10-22 19:18:22 +0100. Fin de la verificación: 20:06:31 +0100.
- **Duración activa desde el arranque: 48 min 09 s**, el 53 % del mínimo estimado, en línea con las ejecuciones
  anteriores. Lo que más pesó fue código y pruebas (42 %) y la verificación independiente (21 %), que destapó B1.
- Esperas por el usuario: una, de 10 min 34 s (19:07:48–19:18:22), por la estimación. En esa espera se aclaró
  dónde vive el área temporal en Voron.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 7 min 18 s | 13 % | 1 |
| Plan (documento de ejecución) | 2 min 57 s | 5 % | 1 |
| Código y tests | 23 min 25 s | 42 % | 4 |
| Checks (fmt, clippy, suite de tests) | 5 min 16 s | 9 % | 2 |
| Verificación independiente | 11 min 48 s | 21 % | 1 |
| Revisión RDD | 4 min 02 s | 7 % | 4 |
| Correcciones de hallazgos | 0 min 41 s | 1 % | 1 |
| **Total activo** | **55 min 27 s** | **100 %** | **14** |

- Inicio: 2026-10-22 19:00:30 +01:00. Fin: 2026-10-22 20:06:31 +01:00.
- Arranque confirmado: 19:18:22. Activo desde el arranque: **48 min 09 s** (compárese con la estimación); antes del arranque: 7 min 18 s.
- Esperas por el usuario (fuera del activo): 10 min 34 s.
  - 19:07:48–19:18:22 (10 min 34 s): estimate

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD T1 | 19:28:45 | 19:30:27 | 1 min 42 s |
| Revisión RDD | RDD T2 | 19:43:52 | 19:45:28 | 1 min 36 s |
| Revisión RDD | RDD T3-T4 | 19:51:16 | 19:52:02 | 0 min 46 s |
| Aceptación | Linux suite and AC10 sweeps | 19:50:30 | 19:52:47 | 2 min 17 s |
| Revisión RDD | RDD fix | 20:05:39 | 20:06:15 | 0 min 36 s |
| **Aceptación (total)** | | | | **2 min 17 s** |
| **Revisión RDD (total)** | | | | **4 min 40 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `9398106` | feat(pager): add the scratch area with snapshots and in-place flushes | T1 | 16 min 41 s | Exploración y mapa 7 min 18 s, Plan (documento de ejecución) 2 min 57 s, Código y tests 6 min 26 s |
| `8ee4c66` | docs: add the transactions execution document | T1 | 0 min 23 s | Código y tests 0 min 11 s, Revisión RDD 0 min 12 s |
| `f87b5d9` | fix(pager): keep the scratch writer from being shared between threads | T1, T2 | 3 min 22 s | Código y tests 1 min 44 s, Revisión RDD 1 min 38 s |
| `0aab615` | feat(tx): add transactions with snapshots, a FIFO writer queue and scratch accounting | T2 | 11 min 39 s | Código y tests 11 min 39 s |
| `1632e39` | test(tx): make the queue tests independent of thread timing | T2, T3 | 3 min 00 s | Código y tests 2 min 16 s, Revisión RDD 0 min 44 s |
| `c97af52` | feat: open, commit and abort transactions through the public API | T3 | 3 min 51 s | Checks (fmt, clippy, suite de tests) 3 min 51 s |
| `ce893ab` | ci: run the every-byte damage sweep of a store written through transactions | T3, T4 | 0 min 37 s | Código y tests 0 min 36 s, Checks (fmt, clippy, suite de tests) 0 min 01 s |
| `610bc40` | fix(pager): free page versions nobody can read at once instead of retiring them | T4, T3-T4, T1-T4, B1 | 13 min 24 s | Código y tests 0 min 33 s, Verificación independiente 11 min 48 s, Revisión RDD 0 min 29 s, Correcciones de hallazgos 0 min 34 s |
| `c88d1fa` | test: name the reuse compile-fail case and drop an unused thread result | B1, final | 1 min 14 s | Checks (fmt, clippy, suite de tests) 1 min 07 s, Correcciones de hallazgos 0 min 07 s |
| — | después del último commit | final, fix | 1 min 16 s | Checks (fmt, clippy, suite de tests) 0 min 17 s, Revisión RDD 0 min 59 s |
| **Total** | | | **55 min 27 s** | |

El reparto entre fases tiene tres imprecisiones:

- **Fases sin marca.** No marqué cambio de fase al pasar de código a pruebas a mitad de una tarea, ni al
  investigar B1 mientras corría el verificador. Por eso «Verificación independiente» incluye ese análisis, y
  «Checks» el tramo que terminó en `c97af52`.
- **Exploración del ODD contada dos veces.** La exploración de antes del arranque (7 min 18 s) quedó también dentro
  del tiempo de elaboración del PRD, porque `prd_time.py` volvió a correr al corregir T14 en esta misma sesión.
- **El total sí es correcto**; lo impreciso es solo cómo se reparte entre fases.

## Informe de cambios (`ab1223c..c88d1fa`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/muninndb-tx` | 1.573 | 0 | 1.325 | 104 | 13 |
| `crates/muninndb-pager` | 910 | 61 | 689 | 148 | 8 |
| `crates/muninndb` | 258 | 15 | 155 | 46 | 9 |
| `odd` | 83 | 0 | — | — | 2 |
| `README.md` | 25 | 11 | — | — | 1 |
| `.github` | 6 | 3 | 4 | 2 | 1 |
| `Cargo.toml` | 1 | 1 | 1 | 0 | 1 |
| **Total** | **2.856** | **91** | **2.174** | **300** | **35** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `Cargo.lock` (+12 −0)

La tabla no incluye este informe ni las últimas entradas del documento de ejecución, que van en el commit del
informe. El PRD (`ab1223c`) es la base. No había previsión de líneas.

## Resumen

Una aplicación ya puede abrir transacciones de lectura desde cualquier hilo y una de escritura cada vez. Las demás
escrituras esperan su turno por orden de llegada, con un límite opcional. Puede confirmar y abortar, y el compilador
impide cerrar el almacén con transacciones abiertas o reusar una ya terminada. Guardar datos llega con las
estructuras del módulo 5.

Decisiones técnicas:

- **Área temporal como la de Voron.** Cada página conserva su número. La escritura cambia copias, y al confirmar
  esas copias pasan a ser la versión nueva. La versión nueva vive en memoria mientras alguna lectura necesite la
  anterior y después se escribe en su sitio en `muninn.data`, sin bloquear nunca a los lectores. El código no
  seguro sigue solo en el Pager: el área temporal, las instantáneas y la escritura de una página mientras otras
  están prestadas.
- **Crate `muninndb-tx`, sin `unsafe`.** Contiene la cola, las transacciones, la página de metadatos y la cuenta
  del tamaño máximo.
- **Sin diario todavía.** Lo confirmado llega a disco a más tardar al cerrar o al soltar el almacén, y el cierre
  reintenta la escritura si antes falló.

## Decisiones por defecto, sin preguntar

- **Página de metadatos.** La página 2 guarda la siguiente página por reservar. Es una página de datos, no parte
  del encabezado (T17), así que el formato sigue siendo `1`. Un almacén del módulo 1 la tiene en blanco y abre
  como vacío.
- **Cuenta del tamaño máximo.** Suma `muninn.data`, las versiones del área temporal y los cambios de la escritura
  abierta. Antes de fallar con «tamaño máximo», la escritura lleva a su sitio lo que ninguna lectura necesita: así
  el espacio «se recupera cuando terminan las lecturas» (T14).
- **«No es `Send`/`Sync`» como aserción estática** y no como prueba de no compilación, porque el texto del
  compilador cambia según los features.
- **Lectura de AC-T16.** El almacén llega al máximo cuando este no deja sitio para lo que retiene una lectura
  larga. Con el diseño de Voron (T14 v1.1), una lectura larga retiene como mucho una versión extra por página, así
  que reescribir las mismas páginas no hace crecer la memoria sin límite. El verificador esperaba un fallo con
  cualquier máximo; ver Pendiente.

## Pendiente (no bloqueante)

- **AC-T16 anotado** en el PRD y su traducción (después del cierre, a petición del usuario): el fallo llega cuando
  el máximo no deja sitio para las versiones que retiene una lectura larga, no con cualquier máximo.
- **CI de GitHub.** No ha corrido: no se subió nada. El barrido AC10 nuevo está en el workflow.
- **Avisos aceptados:**
  - **Copia sellada sin contar.** La copia sellada que hace la confirmación no cuenta en el máximo; es
    transitoria y como mucho del tamaño de lo que se escribe.
  - **Dirección de una segunda lectura.** Una segunda lectura de la misma página dentro de una transacción puede
    dar otra dirección tras una escritura en su sitio. Los bytes son los mismos y las referencias ya entregadas no
    cambian (AC-T15); AC20 pide la misma dirección solo sin confirmaciones intermedias.
  - **Rechazo tras un `fsync` fallido.** `allow_writes_again` también borra ese rechazo; no ocurre en el módulo 2.
  - **E/S en almacenes temporales.** El error de E/S de un almacén temporal lleva una carpeta vacía, como desde el
    módulo 1.
- **Memoria de una lectura larga.** Sin el diario, una lectura larga sobre un almacén muy escrito retiene en
  memoria la versión nueva de cada página cambiada desde que empezó, si no hay tamaño máximo. Los archivos
  temporales del área llegan con el módulo 3, como se decidió.

**Riesgo:** ítem 4 (concurrencia: instantáneas, cola de escritores, escritura en su sitio con lectores abiertos) e
ítem 2 en su sentido de código no seguro (`unsafe` nuevo en el Pager). Lo cubren:

- las pruebas de concurrencia y de fallos inyectados;
- las sondas de estrés del verificador en disco y en memoria;
- las cuatro revisiones RDD;
- la suite completa en Windows y Linux.

No se pasaron Miri ni loom.
