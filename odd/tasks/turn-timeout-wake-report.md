# Despertar redundante al vencer el límite del turno: quitado, verificado y fusionado en `main`

Informe de cierre de `turn-timeout-wake` (2026-10-24). Enfoque de ejecución: ODD.
Fuente de producto: ninguna nueva (refactorización del turno de T3 y T4 de
[transactions.prd.md](../../docs/prd/krummi/transactions.prd.md), sin cambio de producto).
Documento de ejecución: [turn-timeout-wake.md](turn-timeout-wake.md). Antecedente:
[turn-handoff-report.md](turn-handoff-report.md).

Rama `refactor/turn-timeout-wake`: dos commits sobre `1d8ebda` y el de este informe, fusionada en `main` por avance
rápido. No se subió nada.

## Resultado

- **Especificación:** S1 y S2 cumplidos.
  - **S1:** una petición que sale de la cola del turno porque venció su límite ya no despierta a la siguiente. Nunca
    hacía falta: con el turno libre, la primera de la cola lo toma antes de comprobar su plazo, y a la primera
    siempre la despierta quien suelta el turno.
  - **S2:** el informe de `turn-handoff` llamaba a esa línea "inalcanzable". Se corrigió a "innecesaria", porque se
    ejecuta mientras `release_turn` corre el hook `Handoff` con el turno libre, aunque solo adelanta un despertar
    que `release_turn` hace igualmente.
  - Suite en Windows: 198 pruebas pasan con escritura directa del diario y 198 con `KRUMMI_JOURNAL_BUFFERED=1`; fmt
    y clippy limpios.
- **Excepción a la prueba primero:** no cambia ninguna regla, así que no hay un RED con sentido. Lo que debe
  mantenerse lo cubren las pruebas de la cola: AC-T6 y AC-T7 (límites que vencen), AC-T18 (orden en el traspaso) y
  `releasing_the_turn_wakes_only_the_request_that_gets_it`.
- **Revisiones y verificación:**
  - Verificador independiente (Sonnet, alto, acotado a unos 10 min; tardó 5 min): sin bloqueos y sin intercalado
    que deje dormida a una petición con el turno libre.
    - Pruebas de la cola: 10 ejecuciones y 8 × 5 copias en paralelo, sin fallos.
    - Sonda de 48 hilos con límites de 0 a 5 ms y una pausa de 3 ms en el hook `Handoff`: sin cuelgues.
  - Una revisión RDD, aprobada.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `1d8ebda..9b32e4e` (`14a33dc`, `9b32e4e`) | reliability | aprobada | R3-timeout-wake-liveness-unpinned (SUGGESTION): ninguna prueba determinista hace vencer el límite de una petición durante el hook `Handoff` y comprueba que la primera recibe el turno; queda pendiente | `review-d12f90191e75aa5f` |

## Tiempo

- Estimación: 20 – 40 min. Arranque real: 2026-10-24 07:03:20 +0100. Fin de la verificación: 07:13:59 +0100.
- **Duración activa desde el arranque: 10 min 39 s**, el 53 % del mínimo estimado. La mitad fue la verificación
  independiente y un tercio los checks.
- Esperas por el usuario: ninguna dentro de la ejecución.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 0 min 22 s | 3 % | 1 |
| Código y tests | 0 min 19 s | 3 % | 1 |
| Checks (fmt, clippy, suite de tests) | 3 min 59 s | 37 % | 1 |
| Verificación independiente | 5 min 03 s | 47 % | 1 |
| Revisión RDD | 0 min 56 s | 9 % | 1 |
| **Total activo** | **10 min 39 s** | **100 %** | **5** |

- Inicio: 2026-10-24 07:03:20 +01:00. Fin: 2026-10-24 07:13:59 +01:00.
- Arranque confirmado: 07:03:20. Activo desde el arranque: **10 min 39 s**.
- Esperas por el usuario: ninguna.
- Fase abierta, no contada: Cierre (informe y merge) desde 07:13:59.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD reliability | 07:13:19 | 07:13:50 | 0 min 31 s |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `14a33dc` | refactor: drop the redundant wake when a turn request times out | T1 | 4 min 40 s | Plan 0 min 22 s, Código y tests 0 min 19 s, Checks 3 min 59 s |
| `9b32e4e` | docs: record the turn-timeout-wake verification and phase log | T1 | 5 min 03 s | Verificación independiente 5 min 03 s |
| — | después del último commit | T1 | 0 min 56 s | Revisión RDD 0 min 56 s |
| **Total** | | | **10 min 39 s** | |

## Informe de cambios (`1d8ebda..9b32e4e`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `odd` | 52 | 5 | — | — | 4 |
| `crates/krummi-tx` | 2 | 4 | 0 | 2 | 1 |
| **Total** | **54** | **9** | **0** | **2** | **5** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell,
JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

## Resumen

`wait_turn` tenía una llamada que despertaba a la siguiente petición cuando una se iba por su límite de espera. No
cambiaba nada: en el único momento en que se ejecutaba, solo adelantaba un despertar que hace igualmente quien
suelta el turno. Se quitó y en su lugar queda un comentario con el porqué. También se corrigió el informe anterior,
que la daba por inalcanzable.

## Decisiones tomadas sin preguntar

- Verificador acotado a unos 10 min con recuentos fijos, según la preferencia del usuario de verificaciones cortas.

## Pendiente (no bloquea)

- **R3-timeout-wake-liveness-unpinned:** se podría añadir una prueba determinista que use el hook `Handoff` para
  hacer vencer el límite de una petición que no es la primera y compruebe que la primera recibe el turno. Hoy esa
  garantía descansa en el razonamiento y en la sonda del verificador.

**Riesgo:** item 4 (el despertar del turno de escritura). Lo cubren el verificador independiente y la revisión RDD.
