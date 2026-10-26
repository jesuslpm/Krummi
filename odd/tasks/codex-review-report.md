# Revisión cruzada Claude–Codex automatizada: verificada en la rama, sin fusionar

Informe de cierre de `codex-review` (2026-10-26). Enfoque de ejecución: ODD.
Fuente de producto: ninguna (herramienta sin cambio de producto, sin PRD). Documento de ejecución:
[codex-review.md](codex-review.md).

Rama `feat/codex-review` sobre `c6fa485`, último commit del cambio `a63e2ca` más este informe. Sin merge ni push.

## Resultado

- **Especificación:** S1–S10 entregadas. La skill [codex-review](../../.claude/skills/codex-review/SKILL.md) y el
  script [Invoke-CodexReview.ps1](../../.claude/skills/codex-review/scripts/Invoke-CodexReview.ps1) lanzan una
  revisión de Codex (PRD, rango de commits o seguimiento), comprueban la instalación, protegen el repositorio con
  una huella antes/después y devuelven un JSON con `approved`, `findings`, `error` o `violation`. Pester: 19/19.
  La CLI de Codex se actualizó de 0.138.0 a 0.164.0.
- **Aceptación (prueba controlada):** Codex revisó esta misma rama con la skill, en tres rondas reales, sin tocar
  código de producto. La ronda 1 dio `findings` (R1 y R2, ambos major), la 2 dio `findings` (R1 residual, minor) y
  la 3 dio `approved`. En las tres, el guardián solo vio el informe nuevo. La telemetría confirmó
  `gpt-6.1-sol`/`high` en el coordinador y en el revisor `krummi-review` de cada ronda. Antes, un primer intento
  real se detuvo en la comprobación previa con `error`, sin aprobar nada (L8).
- **Revisiones y verificación:** cadena de Codex en [docs/reviews/codex-review/](../../docs/reviews/codex-review/)
  (informe, `-02`, `-03` y resoluciones); revisión RDD del tramo, corregida y aprobada (abajo).
- **Hallazgos corregidos por el camino:**
  - La clave real de `codex doctor --json` es `"config.load"` (`f46e9d7`).
  - R1: el seguimiento rechazaba originales numerados (`c47b0be`, y para tres cifras `2499228`).
  - R2: si `taskkill` era denegado, la espera tras el timeout no tenía límite (`7afccd2`).
  - Un `approved` de tipo texto pasaba como aprobación (`e670fb4`).
  - RDD: `-FixRange` se ignoraba en silencio en los seguimientos de PRD (`0f6e5c9`).

## Cobertura de requisitos

| Spec | Prueba o comprobación | Resultado |
|---|---|---|
| S1 informe conservado e inmutable | Pester: nombre `-02`, informe guardado, original intacto; rondas reales `-02` y `-03` | PASS |
| S2 permisos y guardián | Sonda de `read-only` + `--add-dir` (sin escritura, L5); Pester: cambio rastreado y commit dan `violation`, artefactos ignorados permitidos | PASS (`workspace-write` en todas, L5) |
| S3 error nunca es aprobación | Pester: fallo, resultado inválido, sin informe, timeout, `approved` no booleano, CLI vieja, configuración no cargada; preflight real (L8) | PASS |
| S4 independencia y alcance del seguimiento | Pester: petición literal en la revisión inicial y en el seguimiento; rondas 2 y 3 limitadas a R1/R2 | PASS |
| S5 trazabilidad | Resoluciones con hilo, modelo real y veredictos; log L9–L12 | PASS |
| S6 bucle de tres rondas | Rondas reales 1–3 hasta `approved` | PASS |
| S7 `dismissed with evidence` | Instrucción de la skill y del prompt de seguimiento | No ejercido en vivo: ningún hallazgo resultó incorrecto |
| S8 frontera de producto, sin merge ni push | Skill; esta rama sigue sin fusionar | PASS |
| S9 sin infraestructura duplicada | Reutiliza `AGENTS.md`, las skills y agentes de Codex, `phase_times.py`; sin cambios en `AGENTS.md` ni `.codex/` | PASS |
| S10 CLI actualizada, prueba controlada | `codex --version` 0.164.0; rondas reales | PASS |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `c6fa485..d364887` | risk, resilience, reliability | corregido y luego aprobado | `R3-fixrange-ignored-prd`, CRITICAL, determinista: un seguimiento de PRD ignoraba `-FixRange` sin avisar → `0f6e5c9`, validación dirigida aprobada | `review-f83911e0a6f71a43` |
| `0f6e5c9..a63e2ca` | risk, reliability | aprobado | ninguno | `review-7ca83fd38fbc9883` |

Evaluaciones sin revisión: después de T1 (`eb64eef`), de T2 (`e088636`) y de la ronda 3 (`d364887`), riesgo medio,
`under_budget`.

## Auditoría adicional antes del merge

El usuario pidió evaluar dos observaciones de una auditoría independiente (log L14):

- **Archivos ignorados por Git.** El sandbox de Codex deja escribir en el repositorio, pero mantiene `.git`,
  `.codex` y `.agents` en solo lectura (política del hilo de la ronda 3). De los ignorados, dos cambian lo que hace
  Claude fuera de ese sandbox: `.claude/settings.local.json` (permisos y *hooks*; hoy no existe) y `.atl/`
  (registro de skills). Riesgo real, arreglo mínimo `a63e2ca`: la huella también los incluye. Prueba nueva con
  rojo observado; Pester 20/20. `target/`, `__pycache__/`, `work/` y los temporales siguen sin vigilarse, como
  pidió el usuario; está documentado en *Limits* de la skill.
- **Coherencia entre el JSON y el informe.** Las skills de revisión piden un veredicto de una línea en texto libre,
  sin redacción fija, y los informes reales lo confirman. No hay comprobación simple que ate `approved` al informe
  sin interpretar ese texto; sin cambios, documentado en *Limits*. Claude ya lee el informe al registrar cada
  ronda, porque las resoluciones citan su veredicto y el resultado de cada ID.

## Tiempo

- Estimación: 1 h – 1 h 45 min. Arranque real: 2026-10-26 07:32:56 +00:00. Fin: 08:26:44 +00:00.
- **Duración activa desde el arranque: 53 min 48 s** hasta el primer cierre, por debajo del mínimo estimado. La
  evaluación de la auditoría añadió 14 min en una segunda sesión (total activo 1 h 08 min, pausa de 24 min 27 s
  entre sesiones fuera del activo); las tablas de abajo son las del primer cierre. Lo que más pesó fueron las
  correcciones de hallazgos (14 min) y el código con sus tests (15 min). La exploración y la propuesta previas al
  arranque no están en la bitácora.
- Esperas por el usuario: ninguna dentro de la ejecución.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 0 min 00 s | 0 % | 1 |
| Código y tests | 15 min 25 s | 29 % | 2 |
| Checks (fmt, clippy, suite de tests) | 6 min 58 s | 13 % | 1 |
| Aceptación | 12 min 07 s | 23 % | 1 |
| Revisión RDD | 5 min 00 s | 9 % | 1 |
| Correcciones de hallazgos | 14 min 18 s | 27 % | 3 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **53 min 48 s** | **100 %** | **10** |

- Inicio: 2026-10-26 07:32:56 +00:00. Fin: 2026-10-26 08:26:44 +00:00.
- Arranque confirmado: 07:32:56. Activo desde el arranque: **53 min 48 s** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo). Las rondas de Codex se anotaron en la
fase `review` con las etiquetas `codex-R*`; no son revisiones RDD:

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | codex-R1 | 07:48:21 | 07:48:23 | 0 min 02 s |
| Revisión RDD | codex-R1 | 07:49:44 | 07:56:39 | 6 min 55 s |
| Revisión RDD | codex-R2 | 08:00:59 | 08:04:24 | 3 min 25 s |
| Revisión RDD | codex-R3 | 08:07:32 | 08:10:55 | 3 min 23 s |
| **Revisión RDD (total)** | | | | **13 min 45 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `eb64eef` | feat(codex-review): add the Codex review wrapper with a repository guard | T1 | 13 min 58 s | Código y tests 13 min 58 s |
| `e088636` | feat(codex-review): add the skill that drives the Codex review loop | T1, T2 | 1 min 12 s | Código y tests 1 min 12 s |
| `f46e9d7` | fix(codex-review): read the loaded model from the real doctor JSON key | T2, T3 | 1 min 37 s | Código y tests 0 min 15 s, Aceptación 1 min 22 s |
| `c47b0be` | fix(codex-review): accept a numbered original report in follow-ups (R1) | T3 | 9 min 34 s | Aceptación 9 min 34 s |
| `7afccd2` | fix(codex-review): keep the timeout bounded when Codex cannot be stopped (R2) |  | 0 min 00 s |  |
| `e670fb4` | fix(codex-review): require a boolean approved in Codex's final result |  | 0 min 00 s |  |
| `ae51675` | docs(codex-review): record the round 1 Codex review and its resolutions | T3 | 1 min 33 s | Aceptación 1 min 11 s, Correcciones de hallazgos 0 min 22 s |
| `2499228` | fix(codex-review): accept numbered originals of three or more digits (R1) | T3 | 6 min 33 s | Correcciones de hallazgos 6 min 33 s |
| `d607879` | docs(codex-review): record the round 2 follow-up and the R1 fix |  | 0 min 00 s |  |
| `d364887` | docs(codex-review): record the round 3 approval | T3 | 3 min 59 s | Correcciones de hallazgos 3 min 59 s |
| `0f6e5c9` | fix(codex-review): pass -FixRange to Codex on PRD follow-ups too | T3, rdd-slice | 7 min 22 s | Revisión RDD 5 min 00 s, Correcciones de hallazgos 2 min 22 s |
| `4453c58` | docs(codex-review): record the RDD review and the final checks | rdd-slice, final | 8 min 00 s | Checks (fmt, clippy, suite de tests) 6 min 58 s, Correcciones de hallazgos 1 min 02 s |
| — | después del último commit |  | 0 min 00 s |  |
| **Total** | | | **53 min 48 s** | |

Los tres commits de las 07:59:17 se hicieron juntos, partiendo una sola corrección en un commit por hallazgo; por
eso dos de ellos marcan 0 s.

## Informe de cambios (`c6fa485..4453c58`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `.claude` | 749 | 0 | 525 | 31 | 6 |
| `docs` | 329 | 0 | — | — | 4 |
| `odd` | 143 | 0 | — | — | 2 |
| `CLAUDE.md` | 2 | 0 | — | — | 1 |
| **Total** | **1.223** | **0** | **525** | **31** | **13** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Sin previsión de líneas. `docs` son los informes de Codex y las resoluciones.

## Resumen

Claude puede pedir a Codex que revise un PRD o un cambio terminado, corregir lo demostrado, rebatir con evidencia
lo que considere incorrecto y pedir la verificación, hasta tres rondas, sin salir del flujo. Codex sigue siendo
solo revisor, con su `AGENTS.md`, sus skills y sus agentes sin cambios. El script fija la ruta del informe y no
deja que Codex cambie nada más: cualquier otro cambio es `violation` y no se revierte. Antes de lanzar comprueba
la versión de la CLI y que Codex cargue la configuración del proyecto. Lee el modelo y el esfuerzo realmente
ejecutados de los archivos de sesión de Codex, así que las resoluciones dejan de anotar solo los ajustes pedidos.

## Decisiones tomadas sin preguntar

- `workspace-write` en todas las revisiones, también las de PRD: `read-only` impide a Codex guardar su informe,
  incluso con `--add-dir` (sonda, L5), y el usuario pidió asegurar que Codex escriba informes y ejecute tests.
- Seguimientos en sesiones nuevas, sin reanudar la anterior; un archivo de resoluciones por cadena (el del
  informe original), como en `docs/reviews/trees-keys`.
- La comprobación previa rechaza ejecutar cuando Codex no carga `.codex/config.toml`, porque entonces usaría en
  silencio el modelo del usuario y no vería los agentes del proyecto.
- Se endureció el `approved` no booleano, aunque Codex lo anotó como riesgo no confirmado y no como hallazgo.

## Pendiente (no bloqueante)

- Las copias en worktree (`C:\wt\krummi-<n>`) no son de confianza para Codex. El script se niega a ejecutar allí
  y explica que hay que dar confianza a la carpeta en Codex: falla de forma segura, no bloquea.
- En vivo no se ejercieron `violation`, `dismissed with evidence` ni una revisión de PRD. `violation` está cubierta
  por Pester; las otras dos usan el mismo camino del script.
- `0f6e5c9` es posterior a la aprobación de Codex en la ronda 3. Lo cubre RDD, no la cadena de Codex.
- El test intermitente `krummi-tx` `ac12_ac_t20_…` falló una vez en los checks finales; ya estaba registrado en
  `trees-keys`, esta rama no toca Rust y la repetición pasó.
- **Nuevo intermitente del motor:** `krummi-tx` `recovery.rs`
  `b2_damage_in_a_commit_made_durable_by_a_rollover_is_reported` falló una vez en la suite completa ("opening …
  should fail"). Pasó 5/5 aislado, 5/5 con todo `recovery` y en la siguiente suite completa. No bloquea esta rama,
  que no toca Rust, pero conviene investigarlo aparte: es un test de recuperación.
- Vigilancia de ignorados limitada a `.claude/settings.local.json` y `.atl/` (ver *Auditoría adicional*).
- No se comprueba que el `approved` del JSON coincida con el veredicto en texto libre del informe.
- Las pruebas se escribieron después del script, no en rojo primero (L6); las correcciones posteriores sí
  observaron el rojo, salvo R2, que solo se reproduce dentro del sandbox de Codex.

**Riesgo:** ninguno para el producto. La herramienta solo lee el repositorio y lanza Codex, y el guardián
detecta cualquier escritura indebida, aunque no la impide (L5).

## Limpieza manual

- `%TEMP%\codex-review` (unos 480 KB): prompts, eventos y mensajes finales de cada ejecución, útiles para auditar.
- `%TEMP%\krummi-review-focused-5ec07471bb6a4cc5ae603099ec232854` (101 KB) y
  `%TEMP%\krummi-followup-r1-b0920ae8e3e042f593028492d2855bfb` (226 KB): reproducciones que dejó Codex en las
  rondas 1 y 2.
