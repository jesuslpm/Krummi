# O5, O6 y O7: implementados y verificados; el alcance de O5 y la bajada de W22 esperan tu decisión

Informe de cierre de `write-path` (2026-11-07). Enfoque de ejecución: ODD. Fuentes: O5 del
[plan de rendimiento](../../docs/performance/plan.md), sin PRD (anotación de K13 del 2026-11-04), y
[pager-writer-checks.prd.md](../../docs/prd/krummi/pager-writer-checks.prd.md) para O6 y O7. Documento de ejecución:
[write-path.md](write-path.md).

Rama `perf/write-path`, que parte de `perf/commit-copies` (`9ac34a8`), sin fusionar en `main` ni subir.

## Resultado

- **O5 (S1–S3), parcial:**
  - Cada página cambiada se sella directamente en su hueco del área temporal (`TempArea::store_with`, código
    `unsafe` nuevo). Desaparecen `SealedPage`, `alloc_sealed` y una reserva y copia de 8 KiB por página.
  - Un registro solo en su grupo se escribe desde su propio búfer.
  - `lz4_flex` comprime sin `safe-encode` (anotación D12).
  - **Quedan dos copias que S1 pedía quitar:** la de la carga sin comprimir (LZ4 comprime un bloque contiguo) y la
    de un grupo de varios registros (colocarlos en su sitio entre hilos es rediseñar la concurrencia). Están en L6;
    aceptarlo así es decisión tuya (R2 de Codex).
  - Los bytes del journal y del archivo de datos no cambian: lo fija un test grabado con el código anterior
    (`bytes_tests`).
- **O6 (S1 del PRD):** el escritor comprueba la organización de un nodo cambiado una vez después de cada cambio.
- **O7 (S2 y S3 del PRD):** el escritor usa la integridad de las páginas recordada mientras el almacén está
  abierto. Abrir el almacén sigue comprobando.
- **Tests:** `cargo test --workspace`: 41 suites, 555 pasadas, 0 fallidas, 30 ignoradas. `cargo fmt --all --check`
  y `cargo clippy --workspace --all-targets -- -D warnings` limpios (en `06f933b`; los commits posteriores son solo
  documentación).
- **Verificadores independientes:**
  - O5 (L7): PASS. El journal es idéntico a la base en 120 rondas, el `unsafe` se sostiene y una preparación que
    falla libera sus huecos.
  - O6 y O7 (L11): PASS. Una sonda de hash da 0 violaciones y su control negativo salta; el diferencial contra
    `6794ebc`, `6e32369` y `2b11374` es idéntico.
- **Cargas sueltas (L12):** dos rondas con el orden alternado, en Windows y WSL 2.
  - O5: en Windows, W2, W14, W15, W19 y W21 suben entre un 14 % y un 24 %, y W17 sale mixto. En WSL 2, W14 +6 %, W15
    +9 % y W2 +1 %; W17 y W21 salen mixtos, y W19 baja un 0,4–0,5 %.
  - O6+O7: W1 +66 % a +95 % en Windows y +15 % a +18 % en WSL 2; W3 +49 % a +51 % en las dos.
- **AC-S8: cumplido en Windows, no en WSL 2 por la regla de S5.** W2 sale mixto (−2,5 %, +6,9 %) y W21 empeora (−2,9 %,
  −3,2 %). Una repetición de diagnóstico da los dos mejorando (W2 +2,0 %/+2,4 %, W21 +0,9 %/+4,7 %), pero no
  reemplaza a las rondas de la regla. El ruido típico entre repeticiones es del 4,5–5,6 %, mayor que esos efectos.
- **Ejecuciones completas (L13), contra las del 2026-11-06:**
  - [Windows](../../docs/benchmark/2026-11-07-windows.md): 12 de 22 umbrales (antes 10; suma W1 y W3).
  - [Linux](../../docs/benchmark/2026-11-07-linux.md): 13 de 22 (antes 10; suma W1, W3 y W18).
  - Razón Krummi / Voron en W1: 0,47 → 0,90 en Windows y 0,76 → 1,04 en Linux.
  - De las cinco bajadas de más de 0,05, las cargas sueltas solo confirman una: **W22 en Windows, alrededor de −5 %**.
- **W22 acotada (L13, L18):**
  - Con el mismo almacén: O6 +1,3 % sobre O5, y O7 −4,2 % bajo O6, más bajo en las 6 rondas.
  - O7 no cambia lo que hace una transacción de lectura. El mecanismo no está probado: el código generado es una
    hipótesis, y comprobarla exige perfilar.
  - Arreglarla o diferirla es decisión tuya (R3 de Codex).

## Acceptance criteria coverage

| Criterio | Test o comprobación | Resultado |
|---|---|---|
| AC-S1 | `ac_s1_each_changed_node_is_checked_once_after_each_change`, `ac_s1_merged_nodes_and_reused_pages_are_checked_once_after_each_change` | PASS |
| AC-S2 | `ac_s2_a_changed_node_with_a_damaged_layout_fails_every_read` | PASS |
| AC-S3 | `ac_s3_the_next_write_checks_a_changed_node_again` | PASS |
| AC-S4 | `ac_s4_a_write_uses_the_remembered_pages`, `ac_s4_a_write_remembers_a_page_on_its_first_read` | PASS |
| AC-S5 | `ac_s5_a_write_fails_on_every_read_of_a_damaged_page` | PASS |
| AC-S6 | `ac_s6_a_write_uses_a_remembered_leaf_altered_while_open`, `ac_s6_a_write_uses_a_remembered_fixed_size_leaf_altered_while_open` | PASS |
| AC-S7 | `ac_s7_a_version_in_the_scratch_area_is_checked_on_every_read_of_a_write` | PASS |
| AC-S8 | Cargas sueltas de L12; ejecuciones completas y confirmaciones de L13 | Windows: PASS. WSL 2: no por la regla (W2 mixto, W21 −3 %); aceptado como paso intermedio (anotación del 2026-11-07) |

## RDD reviews

| Tramo | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `b9514cf..6794ebc` (O5) | risk, resilience, reliability | approved | R1-001 warning, el codificador LZ4 inseguro: aceptado (D12). R3 warning, el filtro de grabación global: descartado, el estado es por hilo. R3 warning, la alineación de la escritura directa: descartado, el registro es un `AlignedBuf`. R3 suggestion, el relleno de un hueco reutilizado: cubierto por un test | `review-1bb41836c2a9b9bc` |
| `6794ebc..6e32369` (O6 y el arreglo del test) | risk, reliability | approved | R1-001 y R3 warning, el test de bytes debilitado: fija también la página 1 (`f5e1d1c`). R3 suggestion, faltaban fusiones en AC-S1: test añadido en T3 | `review-088e5f8ba4d50d7c` |
| `6e32369..2b11374` (O7) | risk, reliability | approved | R1-001 warning, el residuo que S3 acepta: aceptado. R3 suggestion, un comentario ambiguo: `1ac73e3` | `review-ef52b399f8a44a80` |
| `2b11374..0489410` (informes y scripts de energía) | risk, resilience, readability | approved | Tres suggestions: cómo quitar la exclusión de Defender, las constantes duplicadas y la comparación con regex en Exit. Corregidas en `5ab24ce` | `review-7113eda8adf9703c` |
| `0489410..5ab24ce` | resilience, readability | approved | Dos suggestions: archivo de estado vacío y un solo patrón de GUID. Corregidas en `eb351fc` | `review-73a5e35d63154d6e` |
| `5ab24ce..eb351fc` | resilience, readability | approved | Dos suggestions de comentarios: `5e2b208` | `review-8d9de4cd219de387` |
| `eb351fc..5e2b208` | readability | approved | R2-001 suggestion, dudaba de que el cast diera `null`: descartado con evidencia en PowerShell 5.1 (L17) | `review-eef78e117d2a4433` |
| `5e2b208..46e3611` (documentación) | readability | approved | R2-001 warning, la causa de W22 escrita como hecho: corregida como hipótesis (`39bed7f`) | `review-bb5bf2e8bcce7738` |

Los commits posteriores a `46e3611` son solo documentación: la corrección de ese aviso y este informe.

## Codex reviews

- Ronda 1 ([informe](../../docs/reviews/write-path/2026-11-07-code.md)): R1 blocker (AC-S8 en WSL 2), R2 major (O5
  parcial), R3 major (W22 en Windows) y R4 minor (el historial daba mal la duración de la integridad recordada). Sin
  defectos de memoria, atomicidad, durabilidad ni integridad.
- Ronda 2 ([informe](../../docs/reviews/write-path/2026-11-07-code-02.md)): R4 resuelto; los registros de R2,
  corregidos. R1, R2 (el alcance) y R3 siguen abiertos como decisiones tuyas. El bucle se para en dos rondas, como
  con O4: una tercera solo repetiría eso.
- [Resoluciones](../../docs/reviews/write-path/2026-11-07-code-resolutions.md).
- En la ronda 1, Codex vio fallar una vez
  `recovery::ac_j9_a_failed_transfer_refuses_writes_and_recovery_brings_everything_back`, que pasó en sus dos
  repeticiones y en mi ejecución. Lo atribuye a una carrera en el propio test, que el rango no toca.

## Time

- Estimación: entre 12 h y 17 h. Inicio: 2026-11-06 20:58:15 UTC. Fin: 2026-11-07 06:39:23 UTC.
- **Activo desde el arranque: 9 h 39 min, por debajo de la estimación** (unas 2 h 20 min menos que el mínimo), sin
  esperas por el usuario. Como en las ejecuciones anteriores, la estimación sobraba.
- Lo que más tiempo llevó fue la aceptación: 7 h 40 min, casi todo mediciones en segundo plano. Las ejecuciones
  completas sumaron 4 h 32 min; las cargas sueltas, 2 h 36 min; las confirmaciones y la bisección de W22, otros
  28 min. En ese tramo también entran tus preguntas sobre Defender y el plan de energía, y los scripts.

### Time by phase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Código y tests | 54 min 03 s | 9 % | 3 |
| Checks (fmt, clippy, suite de tests) | 21 min 30 s | 4 % | 4 |
| Aceptación | 7 h 40 min | 79 % | 2 |
| Revisión RDD | 36 min 34 s | 6 % | 3 |
| Correcciones de hallazgos | 3 min 32 s | 1 % | 1 |
| Documentación | 2 min 49 s | 0 % | 2 |
| Cierre (informe y merge) | 0 min 59 s | 0 % | 2 |
| **Total activo** | **9 h 39 min** | **100 %** | **17** |

- Inicio: 2026-11-06 20:58:15 +00:00. Fin: 2026-11-07 06:39:23 +00:00.
- Arranque confirmado: 20:58:15. Activo desde el arranque: **9 h 39 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 0 min 00 s.
  - 20:58:15–20:58:15 (0 min 00 s): estimate
- Pausas entre sesiones (fuera del activo): 1 min 34 s.
  - 2026-11-07 06:36:59 +00:00 – 2026-11-07 06:38:33 +00:00 (1 min 34 s)

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | T4 windows | 22:29:49 | 23:20:37 | 50 min 48 s |
| Aceptación | T4 wsl | 23:20:37 | 01:05:53 | 1 h 45 min |
| Aceptación | T5 complete windows | 01:05:53 | 02:27:47 | 1 h 21 min |
| Aceptación | T5 complete wsl | 02:27:47 | 05:38:48 | 3 h 11 min |
| Aceptación | T5 confirm windows | 05:40:09 | 05:42:13 | 2 min 04 s |
| Aceptación | T5 confirm wsl | 05:42:13 | 06:07:20 | 25 min 07 s |
| Aceptación | T5 W22 bisect | 06:07:38 | 06:08:29 | 0 min 51 s |
| Revisión RDD | codex-R1 | 06:15:56 | 06:24:43 | 8 min 47 s |
| Revisión RDD | codex-R2 | 06:32:13 | 06:36:35 | 4 min 22 s |
| **Aceptación (total)** | | | | **7 h 37 min** |
| **Revisión RDD (total)** | | | | **13 min 09 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `b9514cf` | docs(prd): add the write checks PRD (O6, O7) | T0 | 1 min 37 s | Documentación 1 min 37 s |
| `6794ebc` | perf(pager): seal pages into their temporary slots and write lone records in place | T0, T1 | 22 min 21 s | Código y tests 17 min 49 s, Checks (fmt, clippy, suite de tests) 4 min 19 s, Documentación 0 min 13 s |
| `f7b19d8` | perf(tree): check a node the write changed once after each change | T1, T2 | 28 min 46 s | Código y tests 24 min 15 s, Checks (fmt, clippy, suite de tests) 4 min 31 s |
| `6e32369` | test(tx): leave the varying header pages out of the pinned data file bytes | T2 | 0 min 49 s | Checks (fmt, clippy, suite de tests) 0 min 49 s |
| `f5e1d1c` | test(tx): pin page 1 of the data file too | T2, T6 | 2 min 21 s | Checks (fmt, clippy, suite de tests) 0 min 12 s, Revisión RDD 2 min 09 s |
| `af41983` | docs: record O5, O6 and their reviews | T6 | 0 min 19 s | Revisión RDD 0 min 19 s |
| `2b11374` | perf(pager): let a write use the remembered page integrity | T3 | 15 min 11 s | Código y tests 11 min 59 s, Checks (fmt, clippy, suite de tests) 3 min 12 s |
| `1ac73e3` | test(tx): say which data file pages the bytes test pins | T3, T6 | 1 min 32 s | Checks (fmt, clippy, suite de tests) 0 min 23 s, Revisión RDD 1 min 09 s |
| `c98d8c9` | docs: record O7 and its reviews | T6 | 16 min 48 s | Revisión RDD 16 min 48 s |
| `4e1410a` | chore(scripts): add a steady power plan for benchmark runs | T6, T4, T5 | 7 h 42 min | Aceptación 7 h 40 min, Revisión RDD 1 min 45 s, Documentación 0 min 53 s |
| `0489410` | docs(bench): add the complete reports after O5, O6 and O7 |  | 0 min 00 s |  |
| `5ab24ce` | fix(scripts): restore only a saved power plan that is a valid GUID | T5, T6 | 1 min 41 s | Revisión RDD 1 min 35 s, Documentación 0 min 06 s |
| `eb351fc` | fix(scripts): read an empty saved power plan file as no plan | T6 | 1 min 28 s | Revisión RDD 1 min 28 s |
| `5e2b208` | docs(scripts): explain the GUID pattern and the empty state file read | T6 | 1 min 02 s | Revisión RDD 1 min 02 s |
| `b5739cd` | docs: record the RDD reviews of the last slices | T6 | 0 min 55 s | Revisión RDD 0 min 55 s |
| `06f933b` | docs: record the Codex review of the write path and narrow the W22 drop | T6 | 12 min 56 s | Revisión RDD 9 min 24 s, Correcciones de hallazgos 3 min 32 s |
| `1a53854` | docs: record the checks before the Codex follow-up | T6 | 3 min 22 s | Checks (fmt, clippy, suite de tests) 3 min 22 s |
| `46e3611` | docs: record the Codex follow-up of the write path | T6 | 4 min 42 s | Checks (fmt, clippy, suite de tests) 4 min 42 s |
| `39bed7f` | docs: state the W22 drop's cause as a hypothesis | T6 | 0 min 09 s | Cierre (informe y merge) 0 min 09 s |
| — | después del último commit | T6 | 0 min 50 s | Cierre (informe y merge) 0 min 50 s |
| **Total** | | | **9 h 39 min** | |

El tramo de `4e1410a` concentra la aceptación porque es el primer commit después de las mediciones. El de `46e3611`
figura como «Checks» aunque fue sobre todo la espera del seguimiento de Codex, porque la marca de fase anterior era
la de las comprobaciones.

## Change report (`9ac34a8..39bed7f`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 847 | 0 | — | — | 16 |
| `crates/krummi` | 671 | 107 | 565 | 70 | 3 |
| `crates/krummi-tx` | 273 | 36 | 206 | 50 | 3 |
| `odd` | 276 | 0 | — | — | 2 |
| `crates/krummi-pager` | 146 | 94 | 98 | 45 | 8 |
| `scripts` | 134 | 0 | 82 | 35 | 2 |
| `crates/krummi-tree` | 49 | 20 | 31 | 16 | 2 |
| `README.md` | 3 | 2 | — | — | 1 |
| **Total** | **2.399** | **259** | **982** | **216** | **37** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Incluye el PRD de O6 y O7 (`b9514cf`) y los informes de las ejecuciones completas. Sin archivos de bloqueo ni
generados.

## Summary

Confirmar una escritura cuesta menos y el escritor deja de repetir comprobaciones que ya hizo.

- **O5:** el commit sella cada página cambiada directamente en el área temporal y escribe sin copia un registro que
  va solo.
- **O6:** el escritor comprueba la organización de un nodo cambiado una vez por cambio, no en cada lectura.
- **O7:** el escritor usa la integridad recordada de las páginas del almacén, como ya hacían los lectores. Acepta el
  residuo de S3: una página alterada desde fuera mientras el almacén está abierto no se vuelve a comprobar.

La carga en orden (W1) casi duplica su velocidad en Windows, y la carga masiva (W3) sube un 50 % en las dos
plataformas.

## Defaults chosen without asking

- Sin preguntar no se tomó ninguna decisión de producto. Lo que pedía una (AC-S8, el alcance de O5 y W22) queda
  abajo como pendiente.
- **Repetición de diagnóstico en WSL 2:** se midieron dos rondas más de W2 y W21 para tener más información, sin
  que reemplacen a las de la regla.
- **Bisección de W22:** se construyó un binario de solo O6 (`f7b19d8`) para separar O6 de O7.
- **Bucle de Codex:** se paró en dos rondas.
- **Cambios de entorno:** la exclusión de Defender y los scripts de energía se hicieron a pedido tuyo (L13).

## Pending (non-blocking)

- **AC-S8 en WSL 2 (R1):** resuelto por tu decisión: se cumple como paso intermedio (anotación del 2026-11-07). La
  próxima medición empieza con una prueba A/A.
- **Alcance de O5 (R2):** cerrar O5 como paso parcial, siguiendo las dos copias restantes, o seguir con ellas.
  Decisión tuya.
- **W22 en Windows (R3):** −4 % a −5 % por O7, con el mecanismo sin probar. Perfilarla y corregirla ahora o
  diferirla. Decisión tuya; no afecta a la corrección.
- **Base de la próxima comparación:** las ejecuciones de Windows de esta noche fueron las últimas con Defender
  escaneando. La próxima comparación vuelve a medir su base con la exclusión y, si se usan, con los scripts de
  energía.
- **Registro de Engram pendiente** (L4): el servidor de memoria rechazó las escrituras en esta sesión.

**Risk:** items 1, 2 and 4. O5 adds `unsafe` code that seals pages into temporary slots and changes how commits
write (item 4: concurrency of the slots; item 1: the bytes written, pinned by `bytes_tests`). O6 and O7 change when
a writer checks layout and integrity (item 2, a guard), within what the PRD accepts. Covered by the independent
verifiers of L7 and L11 and the RDD reviews above.

## Manual cleanup

Carpetas temporales de las mediciones y los perfiles, que se pueden borrar:

- En el repositorio, `work/` (ignorada por git): `o3` 1,4 GB, `o4` 1,7 GB, `o5` 2,2 GB, `complete` 25 MB, `prof`
  6,6 MB. `work/bench` (vacía) y `work/ravendb-7.2.7` se usan en las próximas ejecuciones.
- En WSL 2, en `~`: `o3` 1,4 GB, `o4` 270 MB, `o4full` 545 MB, `o5` 1,1 GB, `prof` 2,5 GB; `krummi-o*-target*`
  269 MB cada una (seis); `krummi-o*-src` y `krummi-o*-before/after` unos 7 MB cada una; `bench-work-o*` vacías.
- Las carpetas de los verificadores en `/tmp` de WSL ya no existen.
