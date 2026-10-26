# Crecimiento dentro del tamaño máximo sin versiones pendientes: corregido y verificado, en su rama sin fusionar

Informe de cierre de `max-size-growth` (2026-10-26). Enfoque de ejecución: ODD (corrección sin cambio de producto,
sin PRD). Reglas de producto: J7 de [journal.prd.md](../../docs/prd/krummi/journal.prd.md) y T14 de
[transactions.prd.md](../../docs/prd/krummi/transactions.prd.md). Documento de ejecución:
[max-size-growth.md](max-size-growth.md).

Rama `fix/max-size-growth` desde `main` (`990689b`), sin fusionar. Nada se ha subido. Último commit de código:
`5db200f`.

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
- **Revisión de Codex:** no ejecutada; Codex agotó su cuota de uso hasta las 10:53 PM. Pendiente.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `990689b..fa77b47` | resilience, reliability | aprobada (elevada a alto riesgo con el punto 4) | R4-sync-flush-before-growth (WARNING): crecer espera un traslado síncrono y un fallo de ese traslado hace fallar la reserva; aceptado (crecer es raro porque dobla, los almacenes sin máximo no cambian, el fallo sigue T11 como el traslado antes de fallar). R3-reader-pinned-growth-untested (SUGGESTION): prueba añadida en `5db200f`. R3-paused-flush-assumption (SUGGESTION): comprobación de que no corrió ninguna pasada en segundo plano, en `5db200f`. | `review-e01b32b2e0986d01` |

`5db200f` (solo pruebas) queda tras la revisión; sale bajo presupuesto.

## Tiempo

- Estimación: 1 h – 1 h 45 min. Arranque real: 2026-10-26 18:57:10 +0000 (`date`). Fin: 2026-10-26 19:15:44 +0000.
- **Duración activa desde el arranque: 18 min 16 s**, el 30 % del mínimo estimado: el diagnóstico lo hizo un
  explorador antes del arranque (reproducción con carga y mapa del código, fuera del activo), y la prueba determinista
  salió a la primera.
- Esperas por el usuario: ninguna (el usuario pidió arrancar sin preguntar).

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Código y tests | 3 min 55 s | 21 % | 1 |
| Checks (fmt, clippy, suite de tests) | 3 min 46 s | 21 % | 1 |
| Verificación independiente | 5 min 00 s | 27 % | 1 |
| Revisión RDD | 1 min 47 s | 10 % | 1 |
| Correcciones de hallazgos | 3 min 48 s | 21 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **18 min 16 s** | **100 %** | **6** |

- Inicio: 2026-10-26 18:57:28 +00:00. Fin: 2026-10-26 19:15:44 +00:00.
- Arranque confirmado: 18:57:28. Activo desde el arranque: **18 min 16 s** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `fa77b47` | fix(tx): write pending versions in place before the store grows within its maximum size | T1 RED test, T1 | 7 min 30 s | Código y tests 3 min 55 s, Checks (fmt, clippy, suite de tests) 3 min 35 s |
| `5db200f` | test(tx): growth within the maximum size with a read that keeps versions | T1, RDD review, RDD advisories | 5 min 20 s | Checks (fmt, clippy, suite de tests) 0 min 11 s, Revisión RDD 1 min 47 s, Correcciones de hallazgos 3 min 22 s |
| — | después del último commit | RDD advisories, independent verifier | 5 min 26 s | Verificación independiente 5 min 00 s, Correcciones de hallazgos 0 min 26 s |
| **Total** | | | **18 min 16 s** | |

## Informe de cambios (`990689b..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi-tx` | 96 | 3 | 80 | 14 | 2 |
| `odd` | 65 | 0 | — | — | 2 |
| **Total** | **161** | **3** | **80** | **14** | **4** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

## Resumen

Un almacén con tamaño máximo ya no se queda corto de espacio según cuándo corrió el traslado en segundo plano: antes
de crecer, la escritura lleva a su sitio las versiones que ninguna lectura necesita, como J7 ya pedía antes de fallar
por tamaño máximo. Las versiones que una lectura sí necesita siguen contando (T14). En disco, ese traslado hace
durables antes de tiempo las confirmaciones diferidas pendientes, algo que J5, J10 y E5 permiten.

## Valores por defecto elegidos sin preguntar

- Corregir el motor y no la prueba: J7 dice que una escritura nunca falla por tamaño máximo a causa de versiones que
  ninguna lectura necesita.
- Un solo traslado por reserva: si tras el traslado previo al crecimiento sigue sin caber, falla con tamaño máximo sin
  repetirlo.

## Pendiente (no bloqueante)

- La revisión de Codex del rango `990689b..HEAD`, cuando vuelva su cuota (10:53 PM).
- Una prueba que pause el diario y haga crecer un almacén con máximo se quedaría esperando (advertencia del
  verificador); ninguna lo hace y no es un camino de producción.
- La fusión en `main` y el push son decisiones del usuario.

**Riesgo:** punto 4 (la escritura espera ahora al hilo de traslado antes de crecer; cubierto por las pruebas
deterministas, la revisión RDD y las sondas en paralelo del verificador, sin bloqueos mutuos: el hilo nunca necesita
el turno del escritor).
