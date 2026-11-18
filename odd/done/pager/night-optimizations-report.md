# Optimizaciones de la noche: se queda T1 (W17 +21 %, W21 +2,5 %), T2 revertida por W3; código de `crates/` igual a `d96b9c2`

Informe de cierre de `night-optimizations` (2026-11-18). Enfoque de ejecución: ODD.
Fuente de producto: ninguna. Son optimizaciones sin cambio de comportamiento, tomadas de los pendientes del
[plan de rendimiento](../../../docs/performance/plan.md). La de T3 cambiaría bytes guardados, así que quedó en un
borrador de PRD: [journal-skip-compression.prd.md](../../../docs/prd/krummi/journal/journal-skip-compression.prd.md).
Documento de ejecución: [night-optimizations.md](night-optimizations.md).

Rama `feat/night-optimizations`, creada desde `feat/ordered-write-cache` (`2e45b31`, código igual a `main` `0cb816c`).
Sin fusionar y sin publicar. El usuario pidió trabajar solo durante la noche (L1), así que no hubo preguntas ni
mensaje de estimación.

## Resultado

- **Especificación**:
  - **T1 (S1, se queda, `d96b9c2`).**
    - *Diagnóstico.* Los fallos de página en Windows venían sobre todo de `encode_record_in`. Cada confirmación que
      crecía pedía memoria nueva a su tamaño exacto, y esa rotación expulsaba también páginas calientes de datos.
    - *Cambio.* Ahora el diario reserva sus buffers con margen: un buffer que se conserva se redondea a potencia de
      dos, y si esa reserva falla se pide el tamaño exacto. Los bytes guardados no cambian.
    - *Test.* `growing_commits_reuse_their_buffers`, con RED observado antes del cambio.
  - **T2 (S2, revertida en `131a172`).** La página de una transacción se escribía en un hueco libre del área temporal
    y se sellaba ahí mismo, lo que ahorraba una copia. Ganaba W21 y W19 entre un 4 y un 6 %, pero W3 seguía más
    lento en todas las rondas.
  - **T3 (S3, borrador de PRD).** No existe una regla que evite comprimir y guarde siempre los mismos bytes (L2). El
    borrador recomienda no hacerlo y deja cinco preguntas al usuario.
  - **T4 (S4, sin cambio).** La copia del grupo pesa un 0,3 % en W5 y W6.
  - **T5 (S6).** Ejecuciones completas y revisión de Codex.
  - **Checks:** fmt y clippy limpios; `cargo test --workspace`: 869 pasados, 0 fallidos.
- **Aceptación (S5).** A/B de Krummi solo en Windows, con el plan de energía del banco y los resúmenes de datos
  iguales. Datos completos en L7–L9.
  - *`t1` frente a `main`:*
    - W17 1,208, W21 1,025, W1 1,027, W2 1,021, W19 1,017.
    - De las cargas marcadas, todas quedaron limpias en 20 rondas salvo W27: 0,992, más lenta en 14 de 20.
    - El diagnóstico mostró que una copia byte a byte del exe de `main` mueve W27 a 0,986 (19/20) y W26 a 0,952
      (20/20). En la misma sesión, `t1` frente a `main` dio W27 1,003 (8/20).
  - *`t2` frente a `t1`:*
    - W21 1,052 y W19 1,061; W2, W14, W15, W24, W28, W29, W23 y W6 entre 1,033 y 1,045.
    - Su mejor variante, c1 (hueco en caja), deja W3 entre 0,981 y 0,985, más lenta en 8–10 de 10 rondas. Es peor
      que el peor placebo de W3 (0,989).
- **Ejecuciones completas (S6)** sobre `67d92b8`, cuyo código es el final:
  - Windows: «Krummi cumple 29 de 29 umbrales».
  - WSL 2: «Krummi cumple 29 de 29 umbrales».
  - Los informes están en `work/night/full/`, ignorado por git, y no se guardan en `docs/benchmark/`.
- **Revisiones y verificación:**
  - La revisión RDD del tramo salió aprobada (tabla abajo).
  - El diagnóstico de T2 lo hizo un trabajador independiente (opus, alta) con builds placebo; contó como
    verificación.
  - No hubo verificador independiente de T1: su línea de riesgo no nombra ningún ítem de alto riesgo.
  - Codex, en la primera ronda, pidió dos correcciones menores en el borrador del PRD, sin defectos de código. En la
    segunda ronda aprobó: [informe](../../../docs/reviews/night-optimizations/2026-11-18-code.md),
    [seguimiento](../../../docs/reviews/night-optimizations/2026-11-18-code-02.md) y
    [resoluciones](../../../docs/reviews/night-optimizations/2026-11-18-code-resolutions.md).
- **Hallazgos corregidos por el camino:**
  - El borrador nombraba el formato `11`, que ya es el vigente (Codex R1).
  - El borrador invertía el orden entre codificar y reservar sitio (Codex R2).
  - Las dos correcciones están en `4cd6aa1`.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `2e45b31..67d92b8` (T1, T2 y su reversión; evaluación media, `under_budget`, revisado como último tramo) | resiliencia, fiabilidad | aprobado | R3-room-boundary-untested, SUGGESTION: no hay un test de `room` en el límite de `ENCODE_RETAIN`. No se cambia, porque `ENCODE_RETAIN` es `32 << 20`, potencia de dos (`journal.rs:295`) | `review-e8701dfc804cabfe` |

Los commits posteriores (`a8052be..`) solo tocan documentos.

## Tiempo

- Estimación: 8–12 h (L1). Inicio real: 2026-11-17 18:42:27. Fin: 2026-11-18 04:30:31.
- **Duración activa desde el inicio: 9 h 48 min**, dentro de la estimación. Lo que más tiempo llevó:
  - El diagnóstico de T2: 3 h 19 min, que el registro de fases anota como «Verificación independiente».
  - La exploración y el diagnóstico de T1: 2 h 17 min.
  - Los A/B: 2 h 53 min en segundo plano.
- Esperas por el usuario: ninguna.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 2 h 26 min | 25 % | 2 |
| Plan (documento de ejecución) | 0 min 31 s | 0 % | 1 |
| Código y tests | 2 h 18 min | 23 % | 4 |
| Checks (fmt, clippy, suite de tests) | 22 min 56 s | 4 % | 2 |
| Verificación independiente | 3 h 19 min | 34 % | 1 |
| Revisión RDD | 1 h 20 min | 14 % | 1 |
| Cierre (informe y merge) | 0 min 03 s | 0 % | 1 |
| **Total activo** | **9 h 48 min** | **100 %** | **12** |

- Inicio: 2026-11-17 18:42:27 +00:00. Fin: 2026-11-18 04:30:31 +00:00.
- Arranque confirmado: 18:42:27. Activo desde el arranque: **9 h 48 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Exploración y mapa | t4-wsl-profile | 21:21:28 | 21:24:51 | 3 min 23 s |
| Aceptación | ab-windows | 21:54:07 | 22:55:23 | 1 h 01 min |
| Aceptación | ab-t1-repeat20 | 22:56:10 | 23:40:11 | 44 min 01 s |
| Aceptación | full-windows | 03:11:40 | 03:33:01 | 21 min 21 s |
| Aceptación | full-linux | 03:33:01 | 04:19:29 | 46 min 28 s |
| Revisión RDD | codex-R1 | 04:20:00 | 04:27:23 | 7 min 23 s |
| Revisión RDD | codex-R2 | 04:27:57 | 04:30:17 | 2 min 20 s |
| **Exploración y mapa (total)** | | | | **3 min 23 s** |
| **Aceptación (total)** | | | | **2 h 53 min** |
| **Revisión RDD (total)** | | | | **9 min 43 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `5b94fab` | docs(odd): start the night optimizations execution | T0 | 0 min 31 s | Plan (documento de ejecución) 0 min 31 s |
| `929ff6b` | docs(prd): draft the journal skip-compression PRD with open questions | T1 | 5 min 09 s | Exploración y mapa 5 min 09 s |
| `d96b9c2` | perf(pager): reserve the journal's record buffers with room to grow | T1 | 2 h 30 min | Exploración y mapa 2 h 17 min, Código y tests 0 min 21 s, Checks (fmt, clippy, suite de tests) 12 min 36 s |
| `2cb4db5` | docs(odd): record the T1 diagnosis and change | T1, T2 | 1 min 58 s | Código y tests 0 min 03 s, Checks (fmt, clippy, suite de tests) 1 min 55 s |
| `f77ab22` | docs(odd): record the T4 profile of the group copy | T2, T4 | 5 min 00 s | Exploración y mapa 3 min 46 s, Código y tests 1 min 14 s |
| `cdf0164` | perf(tx): change a page in a free temporary-area slot and seal it in place | T4, T2 | 28 min 13 s | Exploración y mapa 0 min 05 s, Código y tests 28 min 08 s |
| `6e8d8ca` | docs(odd): record T2 and the acceptance A/B | T2 | 1 h 03 min | Código y tests 1 h 03 min |
| `020b3b7` | docs(odd): record the T1 repeats and the T2 diagnosis | T2 | 44 min 21 s | Código y tests 44 min 21 s |
| `131a172` | revert: drop the page in a free temporary-area slot | T2-diagnosis, T2 | 3 h 19 min | Código y tests 0 min 05 s, Verificación independiente 3 h 19 min |
| `67d92b8` | docs(odd): record the T2 diagnosis and the decisions on T1 and T2 | T2 | 0 min 29 s | Código y tests 0 min 29 s |
| `a8052be` | docs(odd): record the RDD review and the complete runs | T2, T5, T1 | 1 h 18 min | Código y tests 0 min 08 s, Checks (fmt, clippy, suite de tests) 8 min 25 s, Revisión RDD 1 h 10 min |
| `4cd6aa1` | docs(prd): fix the format number and the encode order in the compression draft | T1 | 7 min 56 s | Revisión RDD 7 min 56 s |
| `74ab2fb` | docs(reviews): record the night-optimizations code review resolutions |  | 0 min 00 s |  |
| `5a00edc` | docs(reviews): record the Codex follow-up of night-optimizations | T1 | 2 min 36 s | Revisión RDD 2 min 36 s |
| — | después del último commit | T5 | 0 min 03 s | Cierre (informe y merge) 0 min 03 s |
| **Total** | | | **9 h 48 min** | |

- La fila «Código y tests» incluye la espera de los A/B en primer plano (1 h 03 min y 44 min).
- La fila «Revisión RDD» incluye la espera de las ejecuciones completas (21 y 46 min), porque no se marcó otra fase
  en primer plano mientras corrían.

## Informe de cambios (`2e45b31..5a00edc`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 309 | 0 | — | — | 4 |
| `odd` | 191 | 0 | — | — | 2 |
| `crates/krummi-pager` | 43 | 2 | 34 | 7 | 1 |
| **Total** | **543** | **2** | **34** | **7** | **7** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

El código neto es el cambio de T1 en `journal.rs`: +43 −2 líneas, de ellas 34 de código y 7 de comentarios. T2 sumó
+684 −76 líneas y se revirtió entera.

## Resumen

Durante la noche se trabajaron las cuatro optimizaciones pendientes del plan:

- **Se queda T1.** El diario conserva sus buffers de registro con margen para crecer. Así deja de pedir memoria nueva
  en casi cada confirmación y de expulsar páginas calientes, sin cambiar un solo byte guardado. Mejoran sobre todo
  los valores grandes (W17 +21 %) y las cargas de tamaño fijo (W21 +2,5 %).
- **Se revirtió T2**, que ahorraba una copia por página, porque W3 empeoraba.
- **T3** no se puede hacer sin cambiar lo que guarda el diario, así que es un borrador de PRD para el usuario.
- **T4** no vale la pena (0,3 %).

La medición dejó una lección: en esta máquina, una copia idéntica del exe puede mover una carga de lectura hasta un
5 % en todas las rondas, así que la regla Q7 no filtra los efectos de disposición del código.

## Decisiones tomadas sin preguntar

- **T1 se queda** a pesar de la marca de W27 (0,992, 14/20). Está dentro de la dispersión de una copia idéntica del
  exe (0,986, 19/20) y en la misma sesión dio 1,003; T1 solo cambia cómo el diario dimensiona sus buffers (L9).
- **T2 se revierte por S5**, con su mejor variante descrita en L9 para recuperarla.
- **T3 queda como borrador de PRD** y no se implementa: cualquier regla que ahorre la compresión cambia bytes
  guardados (L2).
- **El consentimiento de las revisiones automáticas** se concedió siempre, según la regla del proyecto.

## Pendiente (no bloqueante)

- **Decisión del usuario sobre T2.** Hay que elegir si vale la pena ganar W21 y W19 un 4–6 % (y entre un 3 y un 4 %
  en W2, W14, W15, W24, W28, W29, W23 y W6) a cambio de que W3 pierda un 1,5–1,9 %.
  - Para recuperarla: `git revert 131a172` y el cambio de c1 descrito en L9.
  - El resto del costo de W3 no tiene explicación. W3 no usa huecos, así que es el camino de código nuevo.
  - No bloquea, porque el código actual no lo incluye.
- **Las cinco preguntas del borrador de PRD de T3.** No bloquean: el PRD no autoriza nada hasta que se respondan.
- **El test del límite de `room`** (sugerencia de RDD). No bloquea, porque la constante es potencia de dos.
- **El plan de rendimiento** (`docs/performance/plan.md`) no tiene una fila de esta ejecución en su historial. Se
  puede añadir al fusionar.

**Riesgo:** ninguno. T1 cambia solo la capacidad de buffers privados, y su test cubre la reutilización y los bytes
iguales a una codificación nueva (ítem 6 cubierto).

## Limpieza manual

`work/night/` (0,9 MB) guarda los JSON y los registros de los A/B, los diffs de los candidatos y los informes
completos. Ya se borraron los builds, los almacenes y la carpeta `~/night` de WSL, salvo sus registros (28 KB). Se
puede borrar todo cuando ya no haga falta la evidencia.
