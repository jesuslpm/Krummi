# O3, integridad recordada de las páginas: implementada, verificada y aceptada (AC-I12 cumplido tras O4)

Informe de cierre de `integrity-once` (2026-11-06). Enfoque de ejecución: ODD. Fuente de producto:
[pager-integrity-once.prd.md](../../docs/prd/krummi/pager-integrity-once.prd.md). Documento de ejecución:
[integrity-once.md](integrity-once.md).

Rama `perf/integrity-once` (de `prd/integrity-once` con los informes del 2026-11-05 fusionados; base de código
`3ec8641`), sin fusionar en `main` ni subir. O4 continúa en una rama que parte de esta.

## Resultado

- **Especificación:** I1–I9 y AC-I1–AC-I11 implementados. Las transacciones de lectura recuerdan, por apertura, las
  páginas y los valores fuera de su hoja cuya integridad ya comprobaron (un bit por página en una reserva que no se
  mueve); las de escritura comprueban todo lo que leen; toda escritura en su sitio olvida antes sus páginas.
  `cargo test --workspace`: 543 pasadas, 0 fallidas, 30 ignoradas. AC-I8 sustituye a AC-C7 y a la cuarta parte de
  AC-X19.
- **Aceptación:** ejecuciones de una sola carga antes (`3ec8641`) y después (`5a9f9f0`), Windows y WSL 2 (L8): W7
  +87 % a +126 %, W8 +99 % a +145 %, W9 +163 % a +233 %, W22 +104 % a +261 %, W20 +72 % a +114 %, W10 0 % a
  +21 %; ninguna carga de lectura empeora. AC-I12 (ejecuciones completas tras O4, `2fdf8e9`, L16): W7 y W22 suben su
  razón con Voron más de 0,05 y cumplen su umbral (W7 0,50 → 1,23 en Windows y 0,54 → 1,03 en Linux; W22 0,52 → 1,27
  y 0,38 → 1,21; 10 de 22 umbrales en ambas, antes 6 y 5). «Ninguna carga baja su razón más de 0,05» **no se cumple
  a la letra**: bajan Windows W10, W11 y W20 y Linux W2, W4–W6 y W14–W17; las ejecuciones de una sola carga contra
  `3ec8641` no muestran ninguna más lenta (W10 +5 % / +10 %; W2, W14, W15, W17 +0 % a +9 %; W16 disperso). Por la anotación de AC-I12 del 2026-11-06 (L19), AC-I12 se cumple.
- **Revisiones y verificación:** verificador independiente sin bloqueos (L6); cinco revisiones RDD aprobadas
  (tabla abajo); revisión cruzada de Codex en tres rondas: R2, R3 y R4 resueltos y verificados, R1 reabierto y
  pendiente de una decisión de producto (abajo).
- **Hallazgos corregidos:** reserva de 16 GiB de direcciones por almacén en disco (bajada a 1 GiB) y error que
  informaba mal su tamaño (RDD R4); valor recordado solo por su primera página (RDD R3); páginas interiores leídas
  como páginas sueltas (RDD); extensión de un valor no exacta (Codex R1, parcialmente, ver pendiente); lectura de
  cero páginas con pánico (Codex R2); marca de longitud reemplazable que obligaba a recomprobar (Codex R3);
  reescritura de la primera página con los mismos bytes que no olvidaba el valor (Codex R4).

## RDD reviews

| Commits | Lenses | Result | Findings and resolution | Lineage |
|---|---|---|---|---|
| `08ffc84..d6ae1c7` | risk, resilience, reliability | approved | R4 warning: reserva de 16 GiB y tamaño mal informado, corregido en `5a9f9f0`; R3 suggestion: valor recordado por su primera página, corregido en `5a9f9f0` | `review-b598396ed8c88c55` |
| `d6ae1c7..f247ef0` | risk, reliability | approved | R1 warning: tope de 64 TiB, mantenido (pregunta de producto); R3 warning: páginas interiores leídas como páginas, corregido en `5482072`; R3 warning: bytes del error sin prueba, prueba añadida | `review-c952ab83310012c9` |
| `f247ef0..362674a` | risk, reliability | approved | R3 warning: colisión improbable del número de página en datos de un valor, aviso; dos sugerencias, aviso | `review-14d2b05e2c604d05` |
| `362674a..95ec765` | risk, reliability | approved | R3 warning y suggestion: pruebas de reescritura de la primera página y de otras extensiones, añadidas en `6d9c923`; aviso sobre la tabla compartida | `review-ed093f8836c99912` |
| `95ec765..c5062a4` | risk, reliability | approved | R1/R3 warnings: el residuo de R1 de Codex, registrado; sugerencias de longitud fuera de rango y marca vacía, corregidas en `bcb71ce` | `review-dcfd54d316821ac3` |

Evaluación sin revisión: `d6ae1c7..71fcbff` dio `under_budget` y se revisó dentro del tramo siguiente. Los commits
`bcb71ce..8ee5401` (una corrección de 20 líneas y documentación) se revisaron con el primer tramo de O4,
`c5062a4..ba2a3ae` (risk, resilience, reliability; approved; `review-8c24a99484b62d18`; sus avisos de O3 son la
marca antigua que deja una longitud fuera de rango, segura, y R1 de Codex, ya pendiente).


## Time

- Estimación: 7 h – 11 h para O3, O4 y las ejecuciones completas juntas. Inicio: 2026-11-06 07:02:41 UTC. Fin de
  O3: 09:23:17 UTC.
- **Duración activa de O3: 2 h 20 min**; la estimación cubre las tres partes; el total, 07:02:41–17:37:10 UTC (10 h 34 min
  de reloj, sin esperas), está en el [informe de O4](commit-copies-report.md).
- Esperas por el usuario: 07:02:21–07:02:41 (20 s), la estimación.

### Time by phase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 5 min 08 s | 4 % | 2 |
| Plan (documento de ejecución) | 0 min 29 s | 0 % | 1 |
| Código y tests | 10 min 40 s | 8 % | 1 |
| Checks (fmt, clippy, suite de tests) | 6 min 29 s | 5 % | 1 |
| Aceptación | 33 min 18 s | 24 % | 2 |
| Verificación independiente | 12 min 43 s | 9 % | 1 |
| Revisión RDD | 48 min 36 s | 35 % | 7 |
| Correcciones de hallazgos | 23 min 13 s | 17 % | 4 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **2 h 20 min** | **100 %** | **20** |

- Inicio: 2026-11-06 07:02:21 +00:00. Fin: 2026-11-06 09:23:17 +00:00.
- Arranque confirmado: 07:02:41. Activo desde el arranque: **2 h 20 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 0 min 20 s.
  - 07:02:21–07:02:41 (0 min 20 s): estimate of O3, O4 and the complete runs

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Checks (fmt, clippy, suite de tests) | T1 workspace tests | 07:19:38 | 07:24:58 | 5 min 20 s |
| Aceptación | T2 windows | 07:38:21 | 07:38:26 | 0 min 05 s |
| Aceptación | T2 windows | 07:50:31 | 07:58:59 | 8 min 28 s |
| Aceptación | T2 wsl | 07:38:26 | 08:07:13 | 28 min 47 s |
| Aceptación | T2 windows 2 | 08:07:32 | 08:07:33 | 0 min 01 s |
| Aceptación | T2 windows 3 | 08:07:45 | 08:13:58 | 6 min 13 s |
| Aceptación | T2 wsl 2 | 08:13:59 | 08:22:46 | 8 min 47 s |
| Revisión RDD | codex-R1 | 08:30:28 | 08:39:14 | 8 min 46 s |
| Revisión RDD | codex-R2 | 08:49:13 | 08:58:11 | 8 min 58 s |
| Revisión RDD | codex-R3 | 09:11:50 | 09:15:44 | 3 min 54 s |
| Revisión RDD | codex-R3b | 09:16:03 | 09:22:42 | 6 min 39 s |
| **Checks (fmt, clippy, suite de tests) (total)** | | | | **5 min 20 s** |
| **Aceptación (total)** | | | | **52 min 21 s** |
| **Revisión RDD (total)** | | | | **28 min 17 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `d6ae1c7` | perf(pager): remember the pages whose integrity a read verified | T0, T1 | 22 min 38 s | Exploración y mapa 5 min 08 s, Plan (documento de ejecución) 0 min 29 s, Código y tests 10 min 40 s, Checks (fmt, clippy, suite de tests) 6 min 21 s |
| `4179055` | docs: add the O3 execution document | T1 | 0 min 08 s | Checks (fmt, clippy, suite de tests) 0 min 08 s |
| `d6b5478` | docs: record the O3 verification | T3, T2 | 13 min 17 s | Aceptación 0 min 34 s, Verificación independiente 12 min 43 s |
| `61a4bff` | docs: add the O3 phase log | T2, T3 | 0 min 09 s | Aceptación 0 min 01 s, Revisión RDD 0 min 08 s |
| `5a9f9f0` | fix(pager): remember a value run with all its pages and reserve less address space | T3 | 10 min 34 s | Revisión RDD 4 min 58 s, Correcciones de hallazgos 5 min 36 s |
| `71fcbff` | docs: record the O3 RDD review | T3 | 0 min 01 s | Correcciones de hallazgos 0 min 01 s |
| `f247ef0` | docs: record the O3 single-load measurements | T3, T2 | 33 min 40 s | Aceptación 32 min 37 s, Correcciones de hallazgos 1 min 03 s |
| `5482072` | fix(pager): check a remembered page that does not name its own number | T2, T3 | 6 min 28 s | Aceptación 0 min 06 s, Revisión RDD 1 min 41 s, Correcciones de hallazgos 4 min 41 s |
| `362674a` | docs: record the review of the last O3 slice |  | 0 min 00 s |  |
| `4c422b5` | docs: record the review of the O3 guard | T3 | 0 min 47 s | Revisión RDD 0 min 40 s, Correcciones de hallazgos 0 min 07 s |
| `bf4cce5` | docs: record the Codex review of O3 | T3 | 11 min 08 s | Revisión RDD 11 min 08 s |
| `e5ea536` | fix(pager): remember a value run with its exact extent | T3 | 5 min 44 s | Correcciones de hallazgos 5 min 44 s |
| `0f6ec0e` | docs: record the resolutions of the O3 Codex review | T3 | 0 min 01 s | Correcciones de hallazgos 0 min 01 s |
| `95ec765` | docs: update the O3 phase log | T3 | 0 min 08 s | Correcciones de hallazgos 0 min 08 s |
| `6d9c923` | test(pager): pin the extents a remembered run does not vouch for | T3 | 1 min 43 s | Revisión RDD 1 min 43 s |
| `7c9c6f3` | docs: record the review of the O3 Codex fix | T3 | 0 min 01 s | Revisión RDD 0 min 01 s |
| `2d8849c` | docs: record the O3 Codex follow-up review | T3 | 11 min 02 s | Revisión RDD 11 min 02 s |
| `c29ac23` | fix(pager): keep a value run remembered by all its pages, its length mark as a check | T3 | 5 min 44 s | Correcciones de hallazgos 5 min 44 s |
| `5e80277` | docs: record the second resolutions of the O3 Codex review |  | 0 min 00 s |  |
| `c5062a4` | docs: update the O3 phase log | T3 | 0 min 08 s | Correcciones de hallazgos 0 min 08 s |
| `bcb71ce` | fix(pager): leave no run mark a slot cannot hold | T3 | 5 min 43 s | Revisión RDD 5 min 43 s |
| `5a6479b` | docs: record the review of the second O3 Codex fix |  | 0 min 00 s |  |
| `8ee5401` | docs: record the third O3 Codex review | T3 | 11 min 24 s | Revisión RDD 11 min 24 s |
| — | después del último commit | T3 | 0 min 08 s | Revisión RDD 0 min 08 s |
| **Total** | | | **2 h 20 min** | |

La revisión RDD (35 %) y las correcciones (17 %) llevaron más de la mitad: tres rondas de Codex y cinco de RDD sobre
cómo recordar la extensión exacta de un valor con un bit por página.

## Change report (`08ffc84..8ee5401`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 649 | 110 | 549 | 56 | 3 |
| `crates/krummi-pager` | 577 | 11 | 414 | 114 | 6 |
| `docs` | 464 | 0 | — | — | 4 |
| `odd` | 242 | 0 | — | — | 2 |
| `crates/krummi-tx` | 20 | 0 | 12 | 5 | 1 |
| `crates/krummi-tree` | 6 | 4 | 0 | 6 | 1 |
| `README.md` | 4 | 3 | — | — | 1 |
| **Total** | **1.962** | **128** | **975** | **181** | **18** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

## Summary

Las lecturas dejan de calcular el código de integridad de una página que ya lo pasó en la apertura en curso, como
hace Voron; las escrituras siguen comprobando todo lo que leen, así que ningún daño se sella en una versión nueva. Lo
recordado es un bit por página (16 KiB por GiB) en una reserva de direcciones que se compromete al crecer, sin
bloqueos para los lectores; un valor fuera de su hoja se recuerda con todas sus páginas y una marca de su longitud
en la tabla de contenidos comprobados. Las lecturas al azar con un hilo se duplican.

## Defaults chosen without asking

- O3 y O4 como dos ejecuciones con rama, documento, revisiones e informe propios; O4 parte de la rama de O3 (L2).
- Las ejecuciones completas de AC-I12 se hacen una vez, tras O4, y miden O3 y O4 juntos (L2).
- Tope de 64 TiB para un almacén en disco (L7, L9).
- Con R1 de Codex abierto, el código queda con los bits de todas las páginas y la marca de longitud como
  comprobación adicional (L15).

## Pending (non-blocking)

- **R1 de Codex (mayor), residuo aceptado por el usuario** (L17): tras perder su marca, una lectura con otra
  longitud desde la primera página de un valor no se comprueba. Solo la provoca una referencia equivocada en una hoja
  que pasó su comprobación. Queda anotado en I1. Cerrarlo sin coste pediría guardar la longitud en el encabezado del
  valor, un cambio de formato con PRD.
- **Tope de 64 TiB, aceptado por el usuario** (L18): anotado en P10 del PRD del Pager.
- **AC-I12, cumplido por decisión del usuario** (L19): una bajada de más de 0,05 entre ejecuciones completas cuenta solo si la confirman las ejecuciones de una sola carga de la misma sesión, y ninguna se confirma. Queda anotado en AC-I12 y en la regla de ruido del plan.
- **RED no observado antes del código** (L5): se sustituyó por tres mutaciones que fallan las pruebas.
- Avisos de RDD sin cambios: colisión improbable del número de página en los datos de un valor; la prueba de reserva
  supone 64 bits.

**Risk:** item 4 (concurrencia: el mapa de bits lo comparten lectores, escritor e hilo de traslado), cubierto por el
verificador independiente, cinco revisiones RDD y tres rondas de Codex.

## Manual cleanup

- `work/o3/` (binarios, tar y almacenes de medición) y, en WSL, `~/o3`, `~/krummi-o3-src` y `~/krummi-o3-target`.
