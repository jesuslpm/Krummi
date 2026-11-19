# CI: el test intermitente de AC-H1 corregido y los barridos de aceptación en un job propio

Informe de cierre de `ci-ac10-and-h1` (2026-11-19). Enfoque de ejecución: ODD. No hay PRD: es una corrección de
tests y de herramientas sin cambio de producto. Documento de ejecución: [ci-ac10-and-h1.md](ci-ac10-and-h1.md).

Rama `fix/ci-ac10-and-h1`, desde `main` en `e5b5f0b`, sin fusionar y sin publicar. La fusión y el push quedan para tu
decisión.

## Resultado

- **Especificación:**
  - **S2, test intermitente:** `ac_h1_a_write_never_checks_the_nodes_it_changed` es ahora determinista (`58a9798`).
    El fallo venía del test, no del motor. Su última aserción cuenta `version_checks()`, y ese contador solo incluye
    las versiones leídas del área de trabajo temporal. Si el hilo de transferencia pasaba las versiones confirmadas
    al almacén antes de la lectura, la comprobación ocurría fuera del contador, como dice Q2 de
    `pager-written-pages`. El test ahora pausa la transferencia.
  - **S1, barridos AC10:** no se encontró ninguna regresión. Los casos de los barridos no cambiaron, y en local el
    costo por caso es el mismo en los dos commits. Su historial en CI varía entre 8,7 y más de 30 min con código casi
    igual, contra un límite de 30 min. Los barridos pasan a un job propio, `sweeps`, que corre en paralelo con
    `checks`, con 60 min para AC10 y 180 para el job (`bcbe4ef`).
- **Checks:**
  - `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` pasan.
  - `cargo test --workspace` pasa con 970 tests (escritor de la T1).
  - El workflow parsea con PyYAML: jobs `checks`, `sweeps` y `benchmark`, con los límites previstos.
- **Aceptación:**
  - **Test intermitente:** antes fallaba 0 de 200 veces solo y 2 de 320 bajo carga. Ahora falla 0 de 200 y 0 de 480.
  - **Cambio de CI:** no se puede probar sin una corrida en GitHub, que requiere publicar la rama o fusionarla.
- **Revisiones:**
  - RDD aprobó las dos porciones (tabla abajo).
  - Codex aprobó en la ronda 2 ([resoluciones](../../../docs/reviews/ci-ac10-and-h1/2026-11-19-code-resolutions.md)).
- **Hallazgos corregidos:** R1 de Codex. El README decía que cada barrido tenía su propio runner, pero los cuatro
  comparten un job; se corrigió en `34b54a0`.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `e5b5f0b..bcbe4ef` | resiliencia, fiabilidad (ítem 5 escalado) | aprobada | Avisos sobre la causa del nuevo límite, explicada en el comentario del job y en L3, y sobre los pasos movidos, que es lo buscado. `RUST_TOOLCHAIN` está definido para todo el workflow. El camino de transferencia lo cubren otros tests | `review-da0dc1b06955ab8f` |
| `bcbe4ef..d5f8e8c` | legibilidad | aprobada | Estado de T4 y registro de fases (este cierre) | `review-7e357816b8832be0` |

## Tiempo

- Estimación: entre 2 h y 4 h. Arranque real: 2026-11-19 06:38:20 +0000. Fin: 2026-11-19 08:46:55 +0000.
- **Duración activa desde el arranque: 2 h 08 min**, en el borde inferior de la estimación. La mayor parte se fue en
  la investigación de AC10, unas 1 h 30 min de mediciones en copias, y en la revisión de Codex, repetida tras
  reinstalar su CLI.
- Esperas por el usuario: ninguna registrada. La decisión de reinstalar Codex se tomó dentro del tiempo de revisión.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 1 h 35 min | 74 % | 1 |
| Revisión RDD | 33 min 33 s | 26 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **2 h 08 min** | **100 %** | **3** |

- Inicio: 2026-11-19 06:38:20 +00:00. Fin: 2026-11-19 08:46:55 +00:00.
- Arranque confirmado: 06:38:20. Activo desde el arranque: **2 h 08 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | codex-R1 | 08:14:33 | 08:14:34 | 0 min 01 s |
| Revisión RDD | codex-R1 | 08:32:32 | 08:44:20 | 11 min 48 s |
| Revisión RDD | codex-R2 | 08:44:41 | 08:46:34 | 1 min 53 s |
| **Revisión RDD (total)** | | | | **13 min 42 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `9238b36` | docs(odd): start the CI fixes execution | investigate AC10 and H1 | 0 min 59 s | Exploración y mapa 0 min 59 s |
| `58a9798` | test(tree): pause the transfer in the AC-H1 writer check test | investigate AC10 and H1 | 30 min 27 s | Exploración y mapa 30 min 27 s |
| `bcbe4ef` | ci: run the acceptance sweeps in their own job | investigate AC10 and H1 | 1 h 03 min | Exploración y mapa 1 h 03 min |
| `86d9ccd` | docs(odd): record the CI fixes and their review | investigate AC10 and H1, RDD T1-T3 | 1 min 17 s | Exploración y mapa 0 min 11 s, Revisión RDD 1 min 06 s |
| `34b54a0` | docs: say the sweeps share one job apart from the checks (Codex review R1) | RDD T1-T3 | 30 min 13 s | Revisión RDD 30 min 13 s |
| `0daf35c` | docs(reviews): record the resolution of the CI fixes code review |  | 0 min 00 s |  |
| `d5f8e8c` | docs(reviews): record the approval of the CI fixes code review | RDD T1-T3 | 2 min 14 s | Revisión RDD 2 min 14 s |
| — | después del último commit |  | 0 min 00 s |  |
| **Total** | | | **2 h 08 min** | |

El registro de fases quedó en «Exploración» durante T1–T3, que corrieron en paralelo con agentes delegados, y no se
marcaron `implement` ni `fix`. La espera por la decisión de reinstalar Codex quedó dentro de «Revisión RDD». El total
es exacto; el reparto por fase no.

## Informe de cambios (`e5b5f0b..d5f8e8c`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 215 | 0 | — | — | 3 |
| `odd` | 65 | 0 | — | — | 2 |
| `.github` | 24 | 1 | 18 | 3 | 1 |
| `crates/krummi` | 4 | 0 | 1 | 3 | 1 |
| `README.md` | 2 | 1 | — | — | 1 |
| **Total** | **310** | **2** | **19** | **6** | **8** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

## Resumen

La CI de `main` fallaba por dos causas. La primera era un test de comprobaciones que, bajo carga, competía con el
hilo de transferencia; ahora pausa la transferencia. La segunda era el paso de barridos de aceptación, que tardaba
según el runner y no tenía margen ni en su límite de 30 min ni en el de 60 min del job. Los barridos van ahora en su
propio job, en paralelo con los checks, con límites que dejan margen. El motor no cambió.

## Valores por defecto elegidos sin preguntar

- **Límites:** 60 min para AC10 y 180 para el job `sweeps`, sobre los 22–27 min medidos recientemente. Los otros
  pasos conservan sus límites.
- **Estructura:** se movieron los cuatro barridos y no solo AC10, porque el job `checks` tampoco tenía margen.

## Pendiente (no bloqueante)

- **Probar el cambio de CI en GitHub:** necesita publicar la rama o fusionarla en `main`.
- **Coste de los barridos:** el tiempo por caso lo dominan escribir el archivo de 64 KiB y reabrir el almacén. Hacer
  los barridos más baratos, por ejemplo invirtiendo solo el byte dañado, queda como mejora opcional.

**Riesgo:** ítem 5 (CI). Lo verifican RDD con escalada y la revisión de Codex, y falta la corrida real en GitHub.
