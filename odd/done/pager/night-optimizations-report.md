# Optimizaciones de la noche: se queda T1 (W17 +21 %, W21 +2,5 %); T2, restaurada, se revierte otra vez por W12 y W13; código de `crates/` igual a `d96b9c2`

Informe de cierre de `night-optimizations` (2026-11-18). Enfoque de ejecución: ODD.
Fuente de producto: ninguna. Son optimizaciones sin cambio de comportamiento, tomadas de los pendientes del
[plan de rendimiento](../../../docs/performance/plan.md). La de T3 cambiaría bytes guardados, así que quedó en un
borrador de PRD: [journal-skip-compression.prd.md](../../../docs/prd/krummi/journal/journal-skip-compression.prd.md).
Documento de ejecución: [night-optimizations.md](night-optimizations.md).

Rama `feat/night-optimizations`, creada desde `feat/ordered-write-cache` (`2e45b31`, código igual a `main` `0cb816c`).
Sin fusionar y sin publicar. La ejecución tuvo dos sesiones:

- **La noche** (L1), sin preguntas ni mensaje de estimación, porque el usuario pidió trabajar solo.
- **La reapertura** (L14). El usuario aceptó restaurar T2 con una excepción en S5: W3 y W25 pueden perder hasta un
  2 % si las cargas habituales ganan.

## Resultado

- **Especificación**:
  - **T1 (S1, se queda, `d96b9c2`).**
    - *Diagnóstico.* Los fallos de página en Windows venían sobre todo de `encode_record_in`. Cada confirmación que
      crecía pedía memoria nueva a su tamaño exacto, y esa rotación expulsaba también páginas calientes de datos.
    - *Cambio.* Ahora el diario reserva sus buffers con margen: un buffer que se conserva se redondea a potencia de
      dos, y si esa reserva falla se pide el tamaño exacto. Los bytes guardados no cambian.
    - *Test.* `growing_commits_reuse_their_buffers`, con RED observado antes del cambio.
  - **T2 (S2, revertida dos veces; la última en `e66de37`).**
    - *Qué hacía.* La página de una transacción se escribía en un hueco libre del área temporal y se sellaba ahí
      mismo, lo que ahorraba una copia.
    - *Restauración.* Se restauró con el hueco en caja (c1, `dfb391e`) y una corrección de RDD (`96ef635`). Un
      verificador independiente la aprobó sin bloqueantes, y lo mismo dos revisiones RDD.
    - *Por qué se revirtió.* Las ejecuciones completas en Windows fallaron W12 y W13 en las dos corridas con T2. En
      la misma sesión, el código sin T2 cumplió los 29 umbrales (L20).
    - *Estado final.* `crates/` queda igual a `5a00edc`, el código con solo T1.
  - **T3 (S3, borrador de PRD).** No existe una regla que evite comprimir y guarde siempre los mismos bytes (L2). El
    borrador recomienda no hacerlo y deja cinco preguntas al usuario.
  - **T4 (S4, sin cambio).** La copia del grupo pesa un 0,3 % en W5 y W6.
  - **T5 (S6).** Ejecuciones completas y dos cadenas de revisión de Codex, las dos aprobadas.
  - **Checks finales**, sobre `e66de37`: fmt y clippy limpios; `cargo test --workspace` da 869 pasados y 0 fallidos.
- **Aceptación (S5)**: A/B de Krummi solo, en Windows salvo donde se indica, con resúmenes de datos iguales. Datos
  completos en L7–L9, L15, L19 y L20.
  - *`t1` frente a `main`:* W17 1,208, W21 1,025, W1 1,027, W2 1,021 y W19 1,017.
    - De las cargas marcadas, todas quedaron limpias en 20 rondas salvo W27 (0,992, más lenta en 14 de 20).
    - Una copia idéntica del exe de `main` mueve W27 a 0,986 (19/20).
  - *T2 con c1 frente a `t1`:*
    - Windows: W21 subió entre un 2,8 y un 6,1 % en cuatro sesiones, y W19 un 6 %.
    - WSL 2, en 10 rondas: W21 0,988 (más lenta en 6 de 10), W19 1,194 y W3 1,059. En Linux, T2 no gana en W21.
  - *W12 y W13, T2 (`t2f`) frente a sin T2 (`not2`), en 20 rondas:* 1,062 (más lenta en 5 de 20) y 1,127 (1 de 20).
    - Con el almacén de partida preparado por T2 se ven igual: en 10 rondas, `not2`/`t2f` da 0,988 y 0,909.
    - El A/B no reproduce lo que pasa en la ejecución completa (abajo). Eso sigue sin diagnosticar.
- **Ejecuciones completas (S6)**, Krummi y Voron:

  | Código | Sistema | Resultado | W12 | W13 |
  |---|---|---|---|---|
  | Sin T2 (`67d92b8`) | Windows | 29 de 29 | 0,85 | 0,87 |
  | Sin T2 (`67d92b8`) | WSL 2 | 29 de 29 | — | — |
  | T2 (`7314419`) | Windows | 27 de 29 | 0,57 | 0,68 |
  | T2 (`7314419`) | WSL 2 | 29 de 29 | 1,17 | 1,07 |
  | T2 (`a4a6bef`, código igual) | Windows | 27 de 29 | 0,60 | 0,75 |
  | Sin T2 (`5a00edc`, misma sesión) | Windows | 29 de 29 | 1,19 | 1,15 |

  - Con T2, Krummi hace en W12 entre 1,31 y 1,43 M op/s; sin T2, 2,69 M.
  - El código final es el de `5a00edc`, así que las dos filas sin T2 lo miden.
  - Los informes están en `work/night/full/`, ignorado por git, y no se guardan en `docs/benchmark/`.
- **Revisiones y verificación:**
  - Cuatro revisiones RDD aprobadas (tabla abajo).
  - Dos verificaciones independientes:
    - El diagnóstico de T2 con builds placebo (opus, alta).
    - El verificador de T2 restaurada (sonnet, alta; ítem 4): pasó las seis preguntas, sin bloqueantes (L16).
  - Codex, cadena 1, sobre `2e45b31..a8052be`: dos correcciones menores en el borrador del PRD, y aprobado en el
    seguimiento. [Informe](../../../docs/reviews/night-optimizations/2026-11-18-code.md),
    [seguimiento](../../../docs/reviews/night-optimizations/2026-11-18-code-02.md),
    [resoluciones](../../../docs/reviews/night-optimizations/2026-11-18-code-resolutions.md).
  - Codex, cadena 2, sobre `5a00edc..a4a6bef` (T2 restaurada): sin defectos de código.
    - R1 (menor): el A/B de Krummi solo no demuestra los umbrales frente a Voron.
    - R1 se corrigió en `e66de37`, y eso llevó a repetir las ejecuciones completas y a la reversión. El seguimiento
      lo aprobó.
    - [Informe](../../../docs/reviews/night-optimizations/2026-11-18-code-03.md),
      [seguimiento](../../../docs/reviews/night-optimizations/2026-11-18-code-04.md),
      [resoluciones](../../../docs/reviews/night-optimizations/2026-11-18-code-03-resolutions.md).
- **Hallazgos corregidos por el camino:**
  - Codex R1 y R2 del borrador de PRD (`4cd6aa1`).
  - RDD: la reserva de los buffers de sobra y el momento del test del puntero publicado (`96ef635`). Se fueron con la
    reversión.
  - Codex R1 de la cadena 2 (`e66de37`).

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `2e45b31..67d92b8` (T1, T2 y su reversión; media, `under_budget`, revisado como último tramo) | resiliencia, fiabilidad | aprobado | R3-room-boundary-untested, SUGGESTION: no hay test de `room` en el límite de `ENCODE_RETAIN`. No se cambia: es `32 << 20`, potencia de dos | `review-e8701dfc804cabfe` |
| `67d92b8..6a5d85b` (T2 restaurada; alta, ítem 4 escalado) | riesgo, resiliencia, fiabilidad | aprobado | R4-stage-spare-infallible-growth, WARNING: corregido en `96ef635`. R4-page-buf-infallible-box, WARNING: descartado, igual que los `BTreeMap` del mismo camino. R3 T5 y el informe desfasados, WARNING: T5 reabierta e informe reescrito. R3 fallo de confirmación con huecos sin test, SUGGESTION: pendiente. R3 momento del test del puntero, SUGGESTION: corregido en `96ef635` | `review-f5c3a3c49dc4a634` |
| `6a5d85b..7314419` (la corrección y sus registros) | resiliencia, fiabilidad | aprobado | R3-spare-reserve-failure-branch-unasserted, SUGGESTION: un fallo real de reserva no se puede inyectar. Se deja | `review-afbfd7e21139eeda` |
| `7314419..e66de37` (la reversión y sus registros; media, `under_budget`, último tramo) | resiliencia, fiabilidad | aprobado | Ninguno | `review-2f04a54e307bd2e5` |

El commit `8ef2d10` y los posteriores solo tocan documentos.

## Tiempo

- **La noche.**
  - Estimación: 8–12 h (L1).
  - Inicio: 2026-11-17 18:42:27. Fin: 2026-11-18 04:30:31.
  - Activo: 9 h 48 min, dentro de la estimación.
- **La reapertura.**
  - Estimación: «Entre 2 h 30 min y 4 h» (L14).
  - Inicio: 06:35:10. Fin: 10:51:30.
  - **Activo: 4 h 16 min**, 16 min por encima del máximo. Lo que más tiempo llevó:
    - Las ejecuciones completas, en total 2 h 15 min en segundo plano. Las dos últimas (45 min) no estaban en el
      plan: se repitieron para responder a Codex R1, y fueron las que mostraron el problema de W12 y W13.
    - Las revisiones RDD y de Codex.
- **En total:** 14 h 04 min activas. Entre las dos sesiones hubo una pausa de 2 h 04 min.
- **Esperas por el usuario:** ninguna. La pregunta del usuario sobre una operación de cargas masivas, y la de las
  cachés de hojas, se contestaron sin detener la ejecución.
- **Nota sobre la herramienta.** El registro de fases toma como arranque el último `--start` (06:35:10), pero su
  activo «desde el arranque» suma también la noche. La cifra de la reapertura sale de restar las dos: 14 h 04 min
  menos 9 h 48 min.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 2 h 26 min | 17 % | 2 |
| Plan (documento de ejecución) | 0 min 31 s | 0 % | 1 |
| Código y tests | 2 h 18 min | 16 % | 5 |
| Checks (fmt, clippy, suite de tests) | 42 min 42 s | 5 % | 4 |
| Aceptación | 2 h 03 min | 15 % | 2 |
| Verificación independiente | 3 h 33 min | 25 % | 2 |
| Revisión RDD | 2 h 49 min | 20 % | 4 |
| Correcciones de hallazgos | 10 min 30 s | 1 % | 2 |
| Cierre (informe y merge) | 0 min 03 s | 0 % | 2 |
| **Total activo** | **14 h 04 min** | **100 %** | **24** |

- Inicio: 2026-11-17 18:42:27 +00:00. Fin: 2026-11-18 10:51:30 +00:00.
- Arranque confirmado: 06:35:10. Activo desde el arranque: **14 h 04 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.
- Pausas entre sesiones (fuera del activo): 2 h 04 min.
  - 2026-11-18 04:30:31 +00:00 – 2026-11-18 06:35:10 +00:00 (2 h 04 min)

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
| Aceptación | wsl-ab-t2 | 06:45:17 | 07:11:43 | 26 min 26 s |
| Aceptación | wsl-ab-t2-10 | 07:12:10 | 07:34:41 | 22 min 31 s |
| Aceptación | full-windows | 08:04:50 | 08:27:52 | 23 min 02 s |
| Aceptación | full-linux | 08:27:52 | 09:13:03 | 45 min 11 s |
| Aceptación | ab-w12-w13 | 09:14:04 | 09:20:03 | 5 min 59 s |
| Aceptación | ab-w12-w13 | 09:20:26 | 09:25:50 | 5 min 24 s |
| Revisión RDD | codex-T2-R1 | 09:26:43 | 09:36:22 | 9 min 39 s |
| Aceptación | full-windows-2 | 09:37:20 | 09:59:37 | 22 min 17 s |
| Aceptación | full-windows-not2 | 10:00:16 | 10:22:39 | 22 min 23 s |
| Revisión RDD | codex-T2-R2 | 10:40:31 | 10:51:12 | 10 min 41 s |
| **Exploración y mapa (total)** | | | | **3 min 23 s** |
| **Aceptación (total)** | | | | **5 h 46 min** |
| **Revisión RDD (total)** | | | | **30 min 03 s** |

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
| `74e1eee` | docs: add the night-optimizations closing report | T5 | 0 min 03 s | Cierre (informe y merge) 0 min 03 s |
| `391d95c` | docs: archive the night-optimizations execution |  | 0 min 00 s |  |
| `dfb391e` | perf(tx): restore the page in a free temporary-area slot, boxed | T2 | 9 min 34 s | Código y tests 0 min 37 s, Checks (fmt, clippy, suite de tests) 8 min 57 s |
| `6a5d85b` | docs(odd): record the WSL A/B and the verifier of the restored T2 | T2 | 1 h 04 min | Checks (fmt, clippy, suite de tests) 0 min 28 s, Aceptación 50 min 22 s, Verificación independiente 13 min 40 s |
| `96ef635` | fix(pager): reserve the commit's spare payloads before staging | T2 | 14 min 19 s | Verificación independiente 0 min 05 s, Revisión RDD 4 min 35 s, Correcciones de hallazgos 9 min 39 s |
| `7314419` | docs(odd): record the RDD review of the restored T2 | T2 | 0 min 11 s | Correcciones de hallazgos 0 min 11 s |
| `a4a6bef` | docs(odd): record the complete runs and the W12/W13 A/B of the restored T2 | T2 | 1 h 22 min | Aceptación 12 min 33 s, Revisión RDD 1 h 10 min, Correcciones de hallazgos 0 min 13 s |
| `e66de37` | revert: drop the page in a free temporary-area slot again | T2 | 1 h 10 min | Checks (fmt, clippy, suite de tests) 10 min 02 s, Aceptación 1 h 00 min, Correcciones de hallazgos 0 min 27 s |
| `8ef2d10` | docs(reviews): record the Codex review of the restored T2 and its resolution | T2 | 2 min 53 s | Checks (fmt, clippy, suite de tests) 0 min 19 s, Revisión RDD 2 min 34 s |
| — | después del último commit | T2 | 11 min 06 s | Revisión RDD 11 min 06 s |
| **Total** | | | **14 h 04 min** | |

- La fila «Código y tests» incluye la espera de los A/B en primer plano de la noche (1 h 03 min y 44 min).
- La fila «Revisión RDD» incluye la espera de las ejecuciones completas que corrían sin otra fase marcada en primer
  plano.

## Informe de cambios (`2e45b31..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `odd` | 512 | 0 | — | — | 3 |
| `docs` | 424 | 0 | — | — | 6 |
| `crates/krummi-pager` | 43 | 2 | 34 | 7 | 1 |
| **Total** | **979** | **2** | **34** | **7** | **10** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

El código neto es el cambio de T1 en `journal.rs`: +43 −2 líneas, de ellas 34 de código y 7 de comentarios. T2 se
añadió y se revirtió dos veces. Estas cifras se tomaron antes del commit de este informe.

## Resumen

Durante la noche y su reapertura se trabajaron las cuatro optimizaciones pendientes del plan:

- **Se queda T1.** El diario conserva sus buffers de registro con margen para crecer. Así deja de pedir memoria nueva
  en casi cada confirmación y de expulsar páginas calientes, sin cambiar un solo byte guardado. Mejoran sobre todo
  los valores grandes (W17 +21 %) y las cargas de tamaño fijo (W21 +2,5 %).
- **T2 no se queda.** Ahorraba una copia por página y ganaba en W21 y W19 en los A/B. En la ejecución completa de
  Windows, en cambio, reduce a la mitad «lecturas con escritor» (W12) y cuesta alrededor de un 40 % a «escritor con
  lectores» (W13), que quedan por debajo del umbral frente a Voron.
- **T3** no se puede hacer sin cambiar lo que guarda el diario, así que es un borrador de PRD para el usuario.
- **T4** no vale la pena (0,3 %).

La lección más importante es de método. El A/B de Krummi solo no ve un efecto que la ejecución completa repite en dos
de dos corridas. Antes de quedarse con una optimización que toca la concurrencia de lectores y escritor, hay que
medir W12 y W13 en la ejecución completa.

## Decisiones tomadas sin preguntar

- **T1 se queda** a pesar de la marca de W27 (0,992, 14/20). Está dentro de la dispersión de una copia idéntica del
  exe, y T1 solo cambia cómo el diario dimensiona sus buffers (L9).
- **T2 se queda tras el A/B de WSL** aunque W21 no ganara en Linux, porque S5 mide en Windows (L15).
- **T2 se revierte otra vez** (L20). La excepción de S5 cubre solo W3 y W25, y la ejecución completa es la que mide
  K7: con T2, W12 y W13 quedan por debajo del umbral en dos de dos corridas.
- **No se repite la ejecución completa del código final.** Su código es igual al de `67d92b8` (Windows y WSL, 29 de
  29) y al de la corrida `not2` (Windows, 29 de 29).
- **T3 queda como borrador de PRD** y no se implementa (L2).
- **El consentimiento de las revisiones automáticas** se concedió siempre, según la regla del proyecto.

## Pendiente (no bloqueante)

- **Por qué T2 frena W12 y W13 solo en la ejecución completa.** Si se diagnostica, T2 podría volver: gana en W21 y W19.
  También explicaría el punto ciego del A/B. Un sospechoso a descartar: en la ejecución completa, las cargas
  anteriores dejan otro estado de memoria, y T2 escribe en memoria mapeada.
- **Una operación de carga masiva** que construya el árbol de abajo arriba a partir de claves ordenadas. La propuso el
  usuario durante la reapertura y necesita un PRD.
- **Las cinco preguntas del borrador de PRD de T3.** El PRD no autoriza nada hasta que se respondan.
- **El test del límite de `room`** (sugerencia de RDD). No bloquea, porque la constante es potencia de dos.
- **El plan de rendimiento** (`docs/performance/plan.md`) no tiene una fila de esta ejecución en su historial. Se puede
  añadir al fusionar, con T2 como intento descartado y la evidencia de L20.

**Riesgo:** ninguno. El código final es T1, que cambia solo la capacidad de buffers privados, y su test cubre la
reutilización y los bytes iguales a una codificación nueva (ítem 6 cubierto).

## Limpieza manual

`work/night/` guarda los JSON y los registros de los A/B y los informes completos. `work/wm/not2-*` y `work/wm/t2f-*`
son los builds del último A/B. Se puede borrar todo cuando ya no haga falta la evidencia.
