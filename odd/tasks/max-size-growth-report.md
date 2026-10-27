# Crecimiento dentro del tamaño máximo sin versiones pendientes: corregido y verificado, con la revisión de Codex resuelta

Informe de cierre de `max-size-growth` (2026-10-26; actualizado el 2026-10-27 con la revisión de Codex). Enfoque de
ejecución: ODD (corrección sin cambio de producto, sin PRD). Reglas de producto: J7 de [journal.prd.md](../../docs/prd/krummi/journal.prd.md) y T14 de
[transactions.prd.md](../../docs/prd/krummi/transactions.prd.md). Documento de ejecución:
[max-size-growth.md](max-size-growth.md).

Rama `fix/max-size-growth` desde `main` (`990689b`), fusionada en `main` por avance rápido (`e341f00`) a pedido del
usuario. La corrección del hallazgo de Codex está en la rama `fix/max-size-growth-transfer-wait` desde `e341f00`, sin
fusionar. Nada se ha subido. Último commit de código: `a7937d5`.

## Resultado

- **Especificación:** S1 (J7: el crecimiento no depende de que haya versiones que ninguna lectura necesita
  esperando el traslado) y S2 (`ac_f18_the_maximum_size_in_a_temporary_store` pasa siempre, también con carga).
  `cargo test --workspace --no-fail-fast` tras el arreglo: 326 pasan, 0 fallan, 11 ignoradas; tras las pruebas de la
  revisión RDD, `cargo test -p krummi-tx`: 136 pasan, 0 fallan. `cargo fmt --all --check` y `cargo clippy --workspace
  --all-targets -- -D warnings` sin avisos.
- **Causa:** `WriteTransaction::reserve` limitaba el crecimiento a «máximo menos memoria», y esa memoria contaba las
  versiones que en ese instante esperaban al traslado en segundo plano. Según cuándo hubiera corrido la última pasada,
  el almacén crecía hasta 128 páginas o se quedaba en 129–131 con el espacio mal repartido; nunca encoge, así que las
  escrituras siguientes necesitaban una página más que el máximo (`MaxSize { max_size: 1097728, needed: 1105920 }`).
- **Arreglo (`fa77b47`):** antes de crecer, si el área temporal tiene versiones, la escritura pide el mismo traslado
  síncrono que J7 ya usaba antes de fallar, y solo después dimensiona el crecimiento; como mucho un traslado por
  reserva.
- **Pruebas:** `growth_within_the_maximum_size_ignores_versions_no_read_needs` es determinista (traslado pausado):
  rojo antes del arreglo (129 páginas en vez de 128), verde después, y comprueba que el espacio lo hizo el traslado de
  la propia escritura. `growth_within_the_maximum_size_counts_versions_a_read_needs` (revisión RDD) cubre una lectura
  que retiene versiones mientras el almacén crece; también falla con el código anterior. Con la carga que hacía fallar
  `ac_f18` (cuatro binarios a la vez), 0 fallos de 12 corridas tras el arreglo, frente a 3 de 8 antes.
- **Revisiones y verificación:** revisión RDD aprobada (abajo); verificador independiente sin bloqueantes (L5), con
  sondas en paralelo sobre almacenes temporales y en disco, confirmaciones diferidas, lecturas abiertas y rotación,
  que pasan con el arreglo y fallan contra la base.
- **Revisión de Codex:** no se pudo correr al cerrar (cuota agotada); se corrió el 2026-10-27.
  [2026-10-27-code.md](../../docs/reviews/max-size-growth/2026-10-27-code.md) pidió cambios: R1 (major), antes de
  cada crecimiento de un almacén con máximo la escritura esperaba al traslado aunque hubiera sitio de sobra, contra
  J5 y AC-J17. Corregido en `ce4f264` (D3, S3): el traslado previo corre solo cuando las versiones en espera
  recortarían el crecimiento, con el objetivo de crecimiento del pager compartido (`growth_target`). Prueba roja antes
  y verde después: `growth_with_ample_room_does_not_wait_for_a_running_transfer`. El seguimiento
  [2026-10-27-code-02.md](../../docs/reviews/max-size-growth/2026-10-27-code-02.md) aprueba: "R1 verified resolved".
  Resoluciones en
  [2026-10-27-code-resolutions.md](../../docs/reviews/max-size-growth/2026-10-27-code-resolutions.md).
- **Tras la corrección de R1:** `cargo test --workspace --no-fail-fast`: 328 pasan, 0 fallan, 11 ignoradas; fmt y
  clippy sin avisos; las pruebas de tamaño máximo en cuatro procesos a la vez, cinco veces cada uno: 20 de 20.
  Verificador independiente sin bloqueantes (L10): sonda de borde de 346 casos (máximos de 128 a 300 páginas,
  traslado pausado frente a vaciado tras cada confirmación) sin diferencias, que sí las muestra contra `990689b`.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `990689b..fa77b47` | resilience, reliability | aprobada (elevada a alto riesgo con el punto 4) | R4-sync-flush-before-growth (WARNING): crecer espera un traslado síncrono y un fallo de ese traslado hace fallar la reserva; aceptado (crecer es raro porque dobla, los almacenes sin máximo no cambian, el fallo sigue T11 como el traslado antes de fallar). R3-reader-pinned-growth-untested (SUGGESTION): prueba añadida en `5db200f`. R3-paused-flush-assumption (SUGGESTION): comprobación de que no corrió ninguna pasada en segundo plano, en `5db200f`. | `review-e01b32b2e0986d01` |
| `e341f00..ce4f264` | resilience, reliability | aprobada (elevada a alto riesgo con el punto 4) | R3-cap-prediction-duplicated (WARNING): `reserve` predice el tope del pager por su cuenta; descartado con evidencia (misma longitud, necesidad y memoria, objetivo compartido; la memoria solo puede bajar bajo el turno del escritor; L9). R3-boundary-untested (SUGGESTION): el lado que recorta ya lo cubre `growth_within_the_maximum_size_ignores_versions_no_read_needs`; no añadida. R3-timeout-flake (SUGGESTION): espera de 5 s a 30 s en `a7937d5`. | `review-91c1935f1e800699` |
| `ce4f264..3478433` (último tramo) | reliability | aprobada, sin hallazgos | — | `review-837d82fd48e9ccd3` |

`5db200f` (solo pruebas) quedó tras la primera revisión, bajo presupuesto. El último tramo (`a7937d5` y la
documentación) se revisó al cerrar la corrección de Codex.

## Tiempo

- Estimación: 1 h – 1 h 45 min. Arranque real: 2026-10-26 18:57:10 +0000 (`date`). Primer cierre: 2026-10-26
  19:15:44 +0000, con 18 min 16 s activos.
- El 2026-10-27 se reanudó para la revisión de Codex y su corrección (pausa de 11 h 35 min entre sesiones, fuera del
  activo). La documentación (registros de las dos revisiones de Codex, incluida la del seguimiento de 5.2 en
  `a6302fe`, y la espera en primer plano al verificador) es la fase más larga.
- Esperas por el usuario: ninguna.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Código y tests | 3 min 55 s | 9 % | 1 |
| Checks (fmt, clippy, suite de tests) | 7 min 18 s | 17 % | 2 |
| Verificación independiente | 5 min 00 s | 11 % | 1 |
| Revisión RDD | 4 min 57 s | 11 % | 2 |
| Correcciones de hallazgos | 5 min 23 s | 12 % | 2 |
| Documentación | 17 min 09 s | 39 % | 1 |
| Cierre (informe y merge) | 0 min 06 s | 0 % | 3 |
| **Total activo** | **43 min 48 s** | **100 %** | **12** |

- Inicio: 2026-10-26 18:57:28 +00:00. Fin: 2026-10-27 07:18:08 +00:00.
- Arranque confirmado: 18:57:28. Activo desde el arranque: **43 min 48 s** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.
- Pausas entre sesiones (fuera del activo): 11 h 36 min.
  - 2026-10-26 19:15:44 +00:00 – 2026-10-27 06:50:55 +00:00 (11 h 35 min)
  - 2026-10-27 07:16:27 +00:00 – 2026-10-27 07:18:08 +00:00 (1 min 41 s)

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | codex-R1 | 06:42:26 | 06:50:02 | 7 min 36 s |
| Revisión RDD | codex-R2 | 07:09:50 | 07:15:51 | 6 min 01 s |
| **Revisión RDD (total)** | | | | **13 min 37 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `fa77b47` | fix(tx): write pending versions in place before the store grows within its maximum size | T1 RED test, T1 | 7 min 30 s | Código y tests 3 min 55 s, Checks (fmt, clippy, suite de tests) 3 min 35 s |
| `5db200f` | test(tx): growth within the maximum size with a read that keeps versions | T1, RDD review, RDD advisories | 5 min 20 s | Checks (fmt, clippy, suite de tests) 0 min 11 s, Revisión RDD 1 min 47 s, Correcciones de hallazgos 3 min 22 s |
| `e341f00` | docs: add the max-size growth closing report | RDD advisories, independent verifier | 5 min 26 s | Verificación independiente 5 min 00 s, Correcciones de hallazgos 0 min 26 s |
| `ce4f264` | fix(tx): transfer before growing only when the waiting versions would cut the growth | codex-R1 | 4 min 59 s | Checks (fmt, clippy, suite de tests) 3 min 24 s, Correcciones de hallazgos 1 min 35 s |
| `a7937d5` | test(tx): give the held-transfer growth test a margin for slow machines | codex-R1 | 1 min 42 s | Checks (fmt, clippy, suite de tests) 0 min 08 s, Revisión RDD 1 min 34 s |
| `a6302fe` | docs: record the Codex follow-up that verifies the trees-scan fixes | codex-R1 | 0 min 59 s | Revisión RDD 0 min 59 s |
| `3478433` | docs: record the Codex review of the max-size growth fix and its resolution | codex-R1 | 11 min 07 s | Revisión RDD 0 min 37 s, Documentación 10 min 30 s |
| — | después del último commit | codex-R1 | 6 min 45 s | Documentación 6 min 39 s, Cierre (informe y merge) 0 min 06 s |
| **Total** | | | **43 min 48 s** | |

## Informe de cambios (`990689b..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `odd` | 229 | 0 | — | — | 3 |
| `crates/krummi-tx` | 166 | 6 | 141 | 21 | 2 |
| `docs` | 125 | 0 | — | — | 2 |
| `crates/krummi-pager` | 9 | 3 | 6 | 2 | 2 |
| **Total** | **529** | **9** | **147** | **23** | **9** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte (lockfiles, generados o binarios): `docs/reviews/trees-scan/2026-10-26-code-02-resolutions.md` (+10 −0), `docs/reviews/trees-scan/2026-10-27-code.md` (+77 −0), `odd/tasks/trees-scan-phases.tsv` (+2 −0), `odd/tasks/trees-scan-report.md` (+4 −2), `odd/tasks/trees-scan.md` (+5 −0)

`a6302fe` registra el seguimiento de Codex de 5.2 (otra ejecución); sus archivos quedan aparte.

## Resumen

Un almacén con tamaño máximo ya no se queda corto de espacio según cuándo corrió el traslado en segundo plano: antes
de crecer, la escritura lleva a su sitio las versiones que ninguna lectura necesita, como J7 ya pedía antes de fallar
por tamaño máximo. Las versiones que una lectura sí necesita siguen contando (T14). En disco, ese traslado hace
durables antes de tiempo las confirmaciones diferidas pendientes, algo que J5, J10 y E5 permiten. Tras la revisión
de Codex, ese traslado previo solo corre cuando las versiones en espera recortarían el crecimiento: con sitio de
sobra, la escritura crece sin esperar al traslado (J5, AC-J17).

## Valores por defecto elegidos sin preguntar

- Corregir el motor y no la prueba: J7 dice que una escritura nunca falla por tamaño máximo a causa de versiones que
  ninguna lectura necesita.
- Un solo traslado por reserva: si tras el traslado previo al crecimiento sigue sin caber, falla con tamaño máximo sin
  repetirlo.

## Pendiente (no bloqueante)

- Una prueba que pause el diario y haga crecer un almacén con máximo cerca del límite se quedaría esperando
  (advertencia del verificador); ninguna lo hace y no es un camino de producción.
- `reserve` predice el tope del pager con `growth_target` compartido; si la fórmula del tope cambiara, las pruebas de
  crecimiento lo detectarían (advertencia del verificador y de RDD).
- La fusión de `fix/max-size-growth-transfer-wait` en `main` y el push son decisiones del usuario.

**Riesgo:** punto 4 (la escritura espera al hilo de traslado antes de crecer solo cuando las versiones en espera
recortarían el crecimiento; cubierto por las pruebas deterministas, dos revisiones RDD, la sonda de borde del
verificador y el seguimiento de Codex; sin bloqueos mutuos: el hilo nunca necesita el turno del escritor).
