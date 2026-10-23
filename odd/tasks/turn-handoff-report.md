# Traspaso del turno de escritura: terminado, verificado y fusionado en `main`

Informe de cierre de `turn-handoff` (2026-10-23). Enfoque de ejecución: ODD.
Fuente de producto: ninguna nueva (rendimiento del turno de T3 de
[transactions.prd.md](../../docs/prd/krummi/transactions.prd.md), sin cambio de producto).
Documento de ejecución: [turn-handoff.md](turn-handoff.md).

Rama `perf/turn-handoff`: dos commits sobre `8704e27` y el de este informe, fusionada en `main` por avance rápido. No
se subió nada.

## Resultado

- **Especificación:** S1–S3 cumplidos.
  - **S1:** al soltar el turno se despierta solo a la petición que lo recibe. Cada petición en espera tiene su propia
    variable de condición, y al rechazar escrituras se despierta a todas (T12).
  - **S2:** el orden de llegada, los límites de espera y las clases de error no cambian.
  - **S3:** medido (abajo).
  - Suite en Windows: 198 pruebas pasan con escritura directa del diario y 198 con `KRUMMI_JOURNAL_BUFFERED=1`; fmt
    y clippy limpios.
- **Medición** (`commit_rate` en release, base `8704e27` y rama alternadas; commits/s, base / rama):

  | Escritura del diario | Rondas | 1 hilo | 4 hilos | 16 hilos |
  |---|---|---|---|---|
  | Directa | 3 | 14.320 / 15.250 | 28.701 / 29.949 | 13.351 / 46.265 |
  | Alternativa (`KRUMMI_JOURNAL_BUFFERED=1`) | 6 | 352 / 345 | 754 / 720 | 3.140 / 3.079 |

  - Con 16 hilos y escritura directa se multiplica por 3,5.
  - El resto queda dentro del ruido: las muestras de cada pareja se solapan.
  - El verificador repitió la medición: 40.266–47.950 en la rama frente a 10.145–13.991 en la base.
- **Diagnóstico previo (L1):** contadores en una copia temporal mostraron que, con 16 hilos, cada commit causaba de
  18 a 24 despertares del turno, y las escrituras del diario por segundo caían a la mitad.
  - Esto corrige el informe del group commit, que atribuía la caída a los despertares por durabilidad. Esos son
    solo 1,6 por commit.
  - Mi previsión de que 16 hilos se quedarían en torno a los 30.000 commits/s de 4 hilos era errónea. Los ~2 commits
    por escritura del diario eran un síntoma del traspaso lento, no un límite del turno exclusivo.
- **Revisiones y verificación:**
  - Verificador independiente (Sonnet, alto): sin bloqueos. Se le pidió cerrar a los 30 min, y los puntos que no
    terminó están en el registro L4.
  - Una revisión RDD, aprobada.

## Cobertura

| Spec | Prueba o comprobación | Resultado |
|---|---|---|
| S1 | `releasing_the_turn_wakes_only_the_request_that_gets_it` | PASS (RED: 36, 29 y 24 despertares para 8 traspasos) |
| S1 (T12) | `ac_t14_a_failed_write_makes_waiting_and_new_requests_fail_at_once`; sonda del verificador con 12 peticiones en espera | PASS: las 12 fallan con E/S en menos de 500 ms (20 rondas) |
| S2 | Pruebas de `queue.rs` (AC7, AC-T6, AC-T7, AC-T8, AC-T9, AC-T14, AC-T18, AC-T22); sondas de orden con límites que vencen | PASS: 30 ejecuciones y 10 × 8 y 15 × 40 copias en paralelo sin fallos |
| S3 | `commit_rate` antes y después | PASS (tabla de arriba) |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `8704e27..6b0a2c3` (`b0e3066`, `6b0a2c3`) | resilience, reliability | aprobada | R3-timeout-handoff-untested (WARNING): ninguna prueba ejercita la rama que pasa el turno cuando una petición se va por su límite. Esa rama no se puede alcanzar (ver Pendiente) y queda como línea defensiva. R4-001 y R3-wakeup-bound-counts-spurious (SUGGESTION): el límite de 12 de la prueba cuenta también despertares espurios; margen de 4, se mantiene | `review-21b8329fa80ad2c4` |

## Tiempo

- Estimación: 40 min – 1 h 20 min. Arranque real: 2026-10-23 22:21:55 +0100. Fin de la verificación: 23:11:14
  +0100.
- **Duración activa desde el arranque: 49 min 19 s**, dentro de la estimación.
  - La verificación independiente se llevó 32 min 37 s (66 %): dos clones en debug y release, cientos de
    ejecuciones en paralelo y la búsqueda del fallo sin explicar. El usuario señaló la demora y se le pidió cerrar.
  - El código y las pruebas, con los primeros intentos de reproducir ese fallo, ocuparon 7 min.
- Esperas por el usuario: ninguna dentro de la ejecución.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 0 min 57 s | 2 % | 1 |
| Código y tests | 7 min 00 s | 14 % | 1 |
| Checks (fmt, clippy, suite de tests) | 3 min 36 s | 7 % | 1 |
| Aceptación | 3 min 31 s | 7 % | 1 |
| Verificación independiente | 32 min 37 s | 66 % | 1 |
| Revisión RDD | 1 min 38 s | 3 % | 1 |
| **Total activo** | **49 min 19 s** | **100 %** | **6** |

- Inicio: 2026-10-23 22:21:55 +01:00. Fin: 2026-10-23 23:11:14 +01:00.
- Arranque confirmado: 22:21:55. Activo desde el arranque: **49 min 19 s**; antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.
- Fase abierta, no contada: Cierre (informe y merge) desde 23:11:14.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD two lenses | 23:09:56 | 23:10:47 | 0 min 51 s |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `b0e3066` | perf: wake only the request that gets the write turn | T1 | 15 min 03 s | Plan 0 min 57 s, Código y tests 7 min 00 s, Checks 3 min 36 s, Aceptación 3 min 30 s |
| `6b0a2c3` | docs: record the turn-handoff verification and phase log | T1 | 32 min 38 s | Aceptación 0 min 01 s, Verificación independiente 32 min 37 s |
| — | después del último commit | T1 | 1 min 38 s | Revisión RDD 1 min 38 s |
| **Total** | | | **49 min 19 s** | |

## Informe de cambios (`8704e27..6b0a2c3`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi-tx` | 72 | 25 | 56 | 12 | 2 |
| `odd` | 62 | 0 | — | — | 2 |
| **Total** | **134** | **25** | **56** | **12** | **4** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell,
JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

## Resumen

Con muchos hilos confirmando a la vez, cada vez que se soltaba el turno de escritura se despertaba a todos los que
esperaban, aunque solo uno pudiera seguir. Ahora cada petición en espera duerme en su propia variable de condición,
y al soltar el turno solo se despierta a la primera de la cola. El orden de llegada, los límites de espera y el
fallo inmediato de todas las peticiones cuando el almacén deja de aceptar escrituras no cambian. Con escritura
directa del diario en Windows, 16 hilos pasan de ~13.000 a ~46.000 confirmaciones durables por segundo.

## Decisiones tomadas sin preguntar

- La rama que despierta al siguiente cuando una petición se va por su límite se dejó como línea defensiva, aunque el
  análisis de la revisión muestra que no se alcanza.
- El hook de pruebas `turn_wakeups` cuenta cada vuelta de una espera, espurias incluidas.

## Pendiente (no bloquea)

- **Rama inalcanzable:** el `wake_next` tras irse por el límite en `wait_turn`. Bajo el mismo lock, una petición
  comprueba si puede tomar el turno antes que su plazo, así que solo se va si el turno tiene dueño o si no es la
  primera. Se puede quitar o dejar comentada como defensiva.
- **Fallo sin explicar (L2):** una ejecución de las pruebas de la cola terminó sin mostrar pánico ni línea de
  resultado, lo que apunta a una salida anormal del proceso. No se reprodujo en unas 700 ejecuciones de la rama ni
  en 600 de la base, así que no se sabe si lo causa este cambio.
- **Sin probar con una sonda:** un fallo dentro de `begin_write` tras tomar el turno; solo se revisó leyendo el
  código.
- El límite de 12 despertares de la prueba nueva cuenta los espurios.
- Sin ejecución en Linux.

**Riesgo:** item 4 (el traspaso del turno con variables de condición por petición). Lo cubren el verificador
independiente y la revisión RDD.
