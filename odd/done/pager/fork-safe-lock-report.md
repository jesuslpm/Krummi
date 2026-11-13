# Bloqueo seguro ante fork terminado: CI en verde en Linux y Windows

Informe de cierre del cambio `fork-safe-lock` (2026-10-22). Enfoque de ejecución: ODD.
Producto: sin PRD nuevo. Es la corrección de un bug que incumplía P9 del [PRD del Pager](../../docs/prd/muninndb/pager.prd.md):
"Cerrar el almacén, o dejar de usarlo sin cerrarlo explícitamente, libera el bloqueo".
Documento de ejecución: [fork-safe-lock.md](fork-safe-lock.md).

La rama `fix/fork-safe-lock` se subió a GitHub con permiso y se mergeó en `main` en local con fast-forward. **`main`
no se subió.**

## Resultado

- **Especificación:** S1–S3 cumplidas.
  - **S1:** un proceso hijo creado con el almacén abierto ya no lo retiene.
  - **S2:** la segunda apertura, en el mismo proceso o en otro, y por cualquier alias, sigue dando "en uso". Un
    intento fallido dentro del proceso no suelta el bloqueo.
  - **S3:** Windows mantiene `LockFileEx`.
- **Aceptación:** la CI `39817011810` sobre `456d63a` está en verde en los dos sistemas: Linux en 1 min 23 s y
  Windows en 1 min 44 s, con 31 pruebas de fallos, 19 de apertura, 15 de páginas, 3 de política y 2 de fachada. El
  barrido de AC10 tardó 40,7 s en Linux y 49,2 s en Windows. En WSL la suite completa pasó 5 de 5 corridas
  seguidas.
- **Revisiones y verificación:**
  - **RDD:** cuatro revisiones, todas aprobadas.
  - **Verificador independiente** (sonnet): sin bloqueantes. Su sonda hizo entre 40.000 y 43.000 creaciones de
    procesos en 25 s mientras 8 hilos reabrían almacenes, con 0 fallos. Contra `30baaeb` la misma sonda fallaba
    en segundos.
- **Hallazgos corregidos por el camino:**
  - la limpieza del temporal cuando su bloqueo falla;
  - la prueba de `fork`, que ahora es determinista;
  - las pruebas del alias por symlink y de las dos ramas de la limpieza, con mutaciones que las hacen fallar;
  - `rustix`, que estaba declarada solo para Linux;
  - el desbloqueo explícito al cerrar en Windows.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `30baaeb..67bf998` | risk, resilience, reliability | aprobada | WARNING: una creación cuyo bloqueo falla dejaba `muninn.data.new`; corregido en `377563b`. SUGGESTION: handshake determinista en la prueba de `fork` y prueba de alias por symlink; hechas en `377563b`. SUGGESTION: el mutex global serializa las aperturas; documentado y mantenido. | `review-0e144bcb102749f8` |
| `67bf998..377563b` | resilience, reliability | aprobada | WARNING: la limpieza solo se probaba a través de un ayudante; corregido en `601d591`. WARNING: un error de E/S al bloquear podría borrar un archivo que tiene otro proceso; pendiente (ver abajo). SUGGESTION: registrar el fallo de limpieza; no aplicado. | `review-cbf154ecf09f6c96` |
| `377563b..6958306` | risk, reliability | aprobada | WARNING: se perdió la prueba del caso "en uso" de la limpieza; corregido en `456d63a`. SUGGESTION sobre el reinicio de los fallos inyectados: no aplica, porque ese estado es por hilo. | `review-0e52271452c30923` |
| `6958306..456d63a` | reliability | aprobada | Dos SUGGESTION que no aplican: la prueba ya reabre la carpeta y comprueba que solo quede `muninn.data`; el estado de los fallos es por hilo. | `review-3869eb3634ef8dc4` |

## Tiempo

- Estimación: 40–80 min. Inicio real: 2026-10-22 17:28:34 +0100. Fin: 17:50:30 +0100.
- **Duración activa desde el arranque: 21 min 56 s**, el 55 % del mínimo estimado. La mayor parte se fue en los
  hallazgos de las revisiones y del verificador (56 %).
- Esperas por el usuario: ninguna.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 0 min 32 s | 2 % | 1 |
| Código y tests | 4 min 54 s | 22 % | 1 |
| Aceptación | 1 min 55 s | 9 % | 1 |
| Revisión RDD | 2 min 15 s | 10 % | 1 |
| Correcciones de hallazgos | 12 min 20 s | 56 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **21 min 56 s** | **100 %** | **6** |

- Inicio: 2026-10-22 17:28:34 +01:00. Fin: 2026-10-22 17:50:30 +01:00.
- Arranque confirmado: 17:28:34. Activo desde el arranque: **21 min 56 s** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `67bf998` | fix(pager): use process-owned POSIX locks so child processes never keep a store locked | T1 | 5 min 22 s | Exploración y mapa 0 min 32 s, Código y tests 4 min 50 s |
| `377563b` | fix(pager): remove a just-created store file whose lock fails, and harden the lock tests | T1, R4-R3 | 4 min 25 s | Código y tests 0 min 04 s, Revisión RDD 2 min 15 s, Correcciones de hallazgos 2 min 06 s |
| `601d591` | test(pager): prove the creation cleanup through an injected lock failure | R4-R3 | 4 min 48 s | Correcciones de hallazgos 4 min 48 s |
| `6958306` | fix(pager): declare rustix for every Unix and unlock explicitly on Windows close | R4-R3 | 1 min 47 s | Correcciones de hallazgos 1 min 47 s |
| `456d63a` | test(pager): cover a creation whose new file another opener holds | R4-R3 | 2 min 55 s | Correcciones de hallazgos 2 min 55 s |
| — | después del último commit | R4-R3, CI | 2 min 39 s | Aceptación 1 min 55 s, Correcciones de hallazgos 0 min 44 s |
| **Total** | | | **21 min 56 s** | |

Las tres revisiones RDD posteriores a la primera y las corridas de CI intermedias quedaron registradas dentro de
"Correcciones de hallazgos": no marqué esos cambios de fase. El verificador corrió en segundo plano sin marcas
`bg-start`/`bg-end`. El total es correcto; el reparto entre fases, no del todo.

## Informe de cambios (`30baaeb..456d63a`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/muninndb-pager` | 345 | 27 | 284 | 39 | 4 |
| `odd` | 36 | 0 | — | — | 2 |
| **Total** | **381** | **27** | **284** | **39** | **6** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

No había previsión de líneas.

## Resumen

En Linux, el bloqueo de un almacén ya no se hereda con `fork`: un proceso que lanza otros procesos mientras tiene
almacenes abiertos puede cerrarlos y reabrirlos sin un "en uso" falso.
- **El bloqueo** pasa de `flock`, que pertenece a la descripción de archivo abierto y se comparte con los hijos, a
  un bloqueo POSIX `fcntl F_SETLK`, que pertenece al proceso y no se hereda.
- **La tabla en memoria.** Los bloqueos POSIX no distinguen aperturas del mismo proceso, y se pierden al cerrar
  cualquier descriptor del archivo. Por eso hay una tabla en memoria por dispositivo e inodo, como en SQLite. La
  tabla detecta la segunda apertura antes de que exista un segundo descriptor, y los descriptores se cierran
  solo con ella tomada.
- **Windows** no cambia, salvo el desbloqueo explícito al cerrar.

## Decisiones por defecto, sin preguntar

- **Bloqueos POSIX con tabla en memoria**, el diseño de SQLite.
- **`WouldBlock` como fallo inyectado** del paso de bloqueo, solo en pruebas, para representar a otro proceso que
  tiene el archivo.

## Pendiente (no bloqueante)

- **Error de E/S al bloquear el temporal recién creado.** Si justo otro proceso lo tomó por un resto, este lo
  borraría, cuando ese otro proceso iba a borrarlo de todos modos. Un conflicto de bloqueo se informa como "en
  uso" y no se borra, así que el caso exige un error de E/S en ese mismo instante.
- **El mutex de la tabla serializa las aperturas y cierres** de todos los almacenes del proceso. Es correcto, pero
  puede notarse en sistemas de archivos lentos o de red, que el PRD deja fuera de alcance.
- **La identidad por dispositivo e inodo** supone inodos estables. Algunos montajes FUSE o 9p no los garantizan,
  y el PRD los deja fuera de alcance.
- **macOS** no está en la CI, porque D3 lo deja fuera; la dependencia ya está declarada para todo Unix.

**Riesgo:** ítem 4 (concurrencia: bloqueo del almacén entre procesos e hilos). Lo cubren la reproducción
determinista en Linux, la CI en los dos sistemas, el verificador independiente con sondas de estrés y cuatro
revisiones RDD.
