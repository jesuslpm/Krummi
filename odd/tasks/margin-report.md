# Margen frente a Voron: 22 de 22 umbrales en Windows y en Linux, verificado y sin fusionar

Informe de cierre de `margin` (2026-11-09). Enfoque de ejecución: ODD. Fuente de producto: las propuestas del
diagnóstico *Margin after the cheap gaps* del [plan de rendimiento](../../docs/performance/plan.md) y el PRD
[pager-commit-checks.prd.md](../../docs/prd/krummi/pager-commit-checks.prd.md) (H1–H4). Documento de ejecución:
[margin.md](margin.md).

Rama `perf/margin`, 17 commits sobre `main` (`22d045a`), último `febecbd` más este informe. Sin fusionar y sin
publicar: la fusión con `main` la pidió el usuario para después del cierre.

## Resultado

- **Especificación:** S1 entregado salvo W11 (M6, medido y descartado). W3 a, la comprobación en una pasada de los
  nodos sin huecos (M1); W3 b, la escritura no comprueba los nodos que cambió y las transacciones posteriores
  comprueban lo confirmado, con su PRD (M2, M3); W12, lectores sin cerrojo para páginas sin versiones (M4); W17,
  trozos temporales compartidos que crecen sobre pocos huecos dispersos (M5); W10 medido solo (M7). Además, por
  decisión del usuario, las compilaciones de depuración comprueban los nodos que cambió la escritura, como el modo
  `VALIDATE` de Voron (M8, `11829c6`). S2: 594 pruebas pasan, 0 fallan, 31 ignoradas; `cargo fmt --all --check` y
  `cargo clippy --workspace --all-targets -- -D warnings` limpios en `febecbd`.
- **Aceptación (S3):** rotaciones de una sola carga, `22d045a` → `908b65e`, dos rondas alternadas, plan de energía
  del banco. Windows: W1 +39,9 % / +38,5 %, W3 +75,9 % / +55,7 %, W12 +25,5 % / +55,2 %, W17 +58,5 % / +58,0 %.
  WSL 2: W3 +53,4 % / +47,4 %, W12 +32,0 % / +25,7 %. Ejecuciones completas en `908b65e` (Linux desde un archivo
  idéntico a ese commit): **22 de 22 umbrales en Windows y en Linux** (eran 21 de 22 en cada una). Razón Krummi /
  Voron: Windows W17 0,63 → 0,91, W3 0,82 → 1,17, W1 1,04 → 1,43, W12 0,87 → 1,16; Linux W3 0,78 → 1,21, W12
  0,85 → 1,10. Informes: [Windows](../../docs/benchmark/2026-11-09-windows-margin.md),
  [Linux](../../docs/benchmark/2026-11-09-linux-margin.md).
- **Costes:** confirmado solo W11 en Windows, más bajo en las cuatro rondas, −1,9 % de media (razón 0,86, cumple).
  No confirmados en rondas alternadas: W10, W14, W18 y W16 en WSL 2; W22 en Windows (+3,1 % / +2,7 %). W18 en
  Windows, más bajo en cuatro rondas, viene de la copia del almacén y no del binario: en la prueba cruzada el binario
  queda mezclado sobre cada almacén, y los dos archivos de datos difieren en 18 bytes, todos en las dos copias de la
  cabecera (L13).
- **Revisiones y verificación:** dos verificadores independientes (sonnet, high) sin bloqueantes; la sonda
  diferencial de M1 sobre 600.000 nodos dañados sin desacuerdos; la prueba de estrés del filtro de versiones con
  182.760 confirmaciones y 630.082 instantáneas sin una violación (L9). Cuatro revisiones RDD aprobadas (abajo).
  Revisión de Codex pendiente (ver *Pendiente*).
- **Hallazgos corregidos por el camino:** la primera versión de M3 (comprobar al confirmar) bajaba W2 un 5–8 % en
  Windows y se rediseñó (L6); la primera de M5 (más trozos propios guardados) no cambiaba nada y se reemplazó por la
  regla de crecimiento (L5); con tamaño máximo, un trozo compartido podía fallar donde cabía uno propio, corregido en
  `43e6f2a`; el filtro de versiones bajaba su cuenta también para páginas sin versiones, corregido en `43e6f2a`;
  AC-H2 no cubría el caso sin instantánea, corregido en `a5e7c41`; el registro L12–L13 perdió el dígito anterior a
  cada `%` al reajustar sus líneas, corregido en `febecbd`.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `22d045a..5096ba0` | risk, resilience, reliability | aprobada (alta, escalada con el ítem 4) | R4-temp-grow-max-size y R3-shared-chunk-under-max-size (WARNING): con tamaño máximo, corregido en `43e6f2a`; R3-filter-unconditional-decrement (WARNING): corregido en `43e6f2a`; R3-ach2-scratch-path-unpinned (SUGGESTION): AC-H2 sostiene una instantánea; R4-filter-fixed-footprint y R3-stress-test-timing (SUGGESTION): aceptadas como documentadas | `review-84f63d463f2430fb` |
| `5096ba0..43e6f2a` | resilience, reliability | aprobada (alta) | R4-bounded-own-chunk-latency (WARNING): descartado con evidencia, con tamaño máximo vale la regla de `22d045a`; R3-001 (WARNING): descartado con evidencia, `flush_all` solo corre con acceso exclusivo; R3-002 (WARNING): corregido en `a5e7c41` | `review-08b5d34b424a9250` |
| `43e6f2a..8d6576d` | readability, reliability | aprobada (media, revisada por la regla de cierre) | R2-001 y R3-margin-log-truncated-percentages (WARNING): cifras truncadas en L12–L13, corregido en `febecbd`; R2-002 (SUGGESTION): resuelto por la misma corrección; R2-003 y R3-linux-report-unknown-revision (SUGGESTION): descartados con evidencia, el banco escribe `desconocido` fuera de un checkout y L13 registra que el archivo es idéntico a `908b65e`; R2-004 (SUGGESTION): corregido; R3-ach2-unheld-first-pass-path-unpinned (SUGGESTION): corregido el comentario | `review-eef9ca39c4f007f2` |
| `8d6576d..febecbd` | reliability | aprobada (media, revisada por la regla de cierre) | ninguno | `review-3807470b88c4de7e` |

Este informe va en un commit propio de documentación, después del último límite revisado.

## Tiempo

- Estimación: no se pidió; el usuario autorizó la ejecución «sin preguntar» (L1). Inicio real: 2026-11-09 06:07:28
  UTC. Fin: 2026-11-09 16:12:05 UTC.
- **Duración activa desde el inicio: 7 h 20 min**, sin estimación con que compararla. La aceptación se lleva el 62 %
  (4 h 32 min), casi todo ejecuciones en segundo plano: las rotaciones, las dos ejecuciones completas (3 h 17 min) y
  las confirmaciones.
- Esperas por el usuario: 13:03:28–15:47:11 (2 h 43 min), trabajo en el PRD de las tablas, ajeno a esta ejecución.
  Esa marca de espera se insertó a mano en el registro de fases, con la hora medida del fin de la última
  confirmación (`bg-end` de las 13:03:28), porque no se marcó al empezar ese trabajo.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 0 min 00 s | 0 % | 1 |
| Código y tests | 1 h 55 min | 26 % | 2 |
| Checks (fmt, clippy, suite de tests) | 14 min 54 s | 3 % | 2 |
| Aceptación | 4 h 32 min | 62 % | 1 |
| Verificación independiente | 19 min 49 s | 4 % | 1 |
| Revisión RDD | 12 min 54 s | 3 % | 2 |
| Correcciones de hallazgos | 1 min 15 s | 0 % | 1 |
| Documentación | 4 min 06 s | 1 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **7 h 20 min** | **100 %** | **12** |

- Inicio: 2026-11-09 06:07:28 +00:00. Fin: 2026-11-09 16:12:05 +00:00.
- Arranque confirmado: 06:07:28. Activo desde el arranque: **7 h 20 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 2 h 43 min.
  - 13:03:28–15:47:11 (2 h 43 min): tables PRD work, not this run

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | rotations windows | 08:31:56 | 08:48:37 | 16 min 41 s |
| Aceptación | rotations wsl | 08:48:37 | 09:34:17 | 45 min 40 s |
| Aceptación | complete windows | 09:34:17 | 10:59:12 | 1 h 24 min |
| Aceptación | complete wsl | 10:59:12 | 12:52:18 | 1 h 53 min |
| Aceptación | confirmation windows | 12:53:23 | 12:55:17 | 1 min 54 s |
| Aceptación | confirmation wsl | 12:55:17 | 12:55:21 | 0 min 04 s |
| Aceptación | confirmation wsl | 12:57:30 | 13:03:28 | 5 min 58 s |
| Aceptación | confirmation windows W22 | 16:00:30 | 16:01:37 | 1 min 07 s |
| Aceptación | confirmation wsl W16 | 16:01:37 | 16:01:51 | 0 min 14 s |
| **Aceptación (total)** | | | | **4 h 29 min** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `da18e52` | perf(tree): check a node without holes in one walk, without the coverage map | M1 | 3 min 32 s | Código y tests 3 min 32 s |
| `e16d218` | docs(prd): check the nodes a write changed once, at commit | M1 | 19 min 18 s | Código y tests 19 min 18 s |
| `a583aab` | perf(tx): check the nodes a write changed once, at commit | M1 | 23 min 25 s | Código y tests 23 min 25 s |
| `f6c8f6a` | perf(pager): grow the shared temporary chunks when only a few scattered slots are free | M1 | 8 min 29 s | Código y tests 8 min 29 s |
| `77b57e7` | perf(pager): let readers skip the temporary-area lock for pages without versions | M1 | 14 min 13 s | Código y tests 14 min 13 s |
| `c5093a5` | perf(tx): never check at commit, nor on a read, the nodes a write changed | M1 | 36 min 23 s | Código y tests 36 min 23 s |
| `181b749` | test(krummi): pin the remembered content and the repeated reads of a committed damaged node | M1 | 8 min 10 s | Código y tests 8 min 10 s |
| `2ae318a` | docs(prd): leave the nodes a write changed to the first read after the commit | M1 | 0 min 07 s | Código y tests 0 min 07 s |
| `7a8e012` | test(krummi): pin a damaged root of one leaf, and fix the comments of the write's checks | M1, M1-M5 | 15 min 35 s | Código y tests 0 min 08 s, Verificación independiente 15 min 27 s |
| `85324fc` | test(tx): keep the verification's stress of the version filter | M1-M5 | 3 min 18 s | Verificación independiente 3 min 18 s |
| `5096ba0` | docs(odd): record the verification of the margin run | M1-M5 | 0 min 56 s | Verificación independiente 0 min 56 s |
| `43e6f2a` | fix(pager): keep exact own chunks with a maximum size, and drop a filter count only for a page with versions | M1-M5 | 8 min 01 s | Verificación independiente 0 min 08 s, Revisión RDD 7 min 53 s |
| `a5e7c41` | test(krummi): cover a committed damaged node with and without a snapshot that keeps it | M1-M5 | 1 min 11 s | Revisión RDD 1 min 11 s |
| `908b65e` | docs(odd): record the RDD reviews of the margin run | M1-M5 | 0 min 20 s | Revisión RDD 0 min 20 s |
| `11829c6` | feat(tree): check the nodes a write changed in debug builds | M1-M5, M7, M3 | 4 h 44 min | Código y tests 1 min 51 s, Checks (fmt, clippy, suite de tests) 10 min 01 s, Aceptación 4 h 32 min, Revisión RDD 0 min 42 s |
| `8d6576d` | docs(odd): record the margin run's measurements and complete runs | M3, M7 | 4 min 08 s | Checks (fmt, clippy, suite de tests) 0 min 09 s, Documentación 3 min 59 s |
| `febecbd` | docs(odd): fix the margin log's figures and record the last slice's review | M7 | 3 min 15 s | Revisión RDD 2 min 48 s, Correcciones de hallazgos 0 min 20 s, Documentación 0 min 07 s |
| — | después del último commit | M7 | 5 min 39 s | Checks (fmt, clippy, suite de tests) 4 min 44 s, Correcciones de hallazgos 0 min 55 s |
| **Total** | | | **7 h 20 min** | |

Lectura: la aceptación (rotaciones, ejecuciones completas y confirmaciones) es casi dos tercios del activo, y se
carga a `11829c6` porque ese commit vino después. Las etiquetas de las fases son gruesas: `M1` cubre la
implementación de M1 a M6, y la marca de `implement` de las 15:47 dice `M3` aunque el trabajo fue M8. Durante el
tramo de aceptación de 08:31 a 13:03, con las mediciones en segundo plano, también se trabajó en el diseño de las
tablas; el registro no separa ese trabajo, así que el activo de la aceptación lo incluye.

## Informe de cambios (`22d045a..febecbd`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 524 | 0 | — | — | 13 |
| `crates/krummi` | 147 | 189 | 113 | 33 | 2 |
| `crates/krummi-tx` | 191 | 49 | 149 | 32 | 3 |
| `crates/krummi-pager` | 197 | 25 | 145 | 41 | 3 |
| `odd` | 156 | 0 | — | — | 1 |
| `crates/krummi-tree` | 97 | 25 | 75 | 18 | 3 |
| **Total** | **1.312** | **288** | **482** | **124** | **25** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Sin archivos de bloqueo ni generados. No hubo previsión de líneas.

## Resumen

Krummi pasa a cumplir los 22 umbrales del banco frente a Voron en las dos plataformas. Las cargas que faltaban
(W17 en Windows, W3 en Linux) y las que estaban justas (W1, W12) suben con cuatro cambios. La comprobación de
organización de los nodos sin huecos se hace en una sola pasada. Una escritura ya no vuelve a comprobar los nodos
que ella misma cambió: los comprueba la primera lectura posterior a la confirmación, igual que Voron, y lo fija el
PRD `pager-commit-checks`. Los lectores consultan un filtro sin cerrojo antes de buscar versiones de una página.
Los trozos temporales compartidos crecen en lugar de reutilizar pocos huecos dispersos. El precio es que una
escritura no detecta un nodo que el propio motor dejó inconsistente hasta la lectura posterior (H3); las
compilaciones de depuración sí lo comprueban, para que las pruebas lo encuentren.

## Decisiones tomadas sin preguntar

- La comprobación de los nodos propios va solo con `debug_assertions`, no también con `test-hooks` como se había
  planteado: `krummi-tree` no tiene esa feature y las pruebas compilan en depuración (L11).
- Se confirmaron también W22 en Windows y W16 en WSL 2, que no estaban en las rotaciones, porque las ejecuciones
  completas mostraron caídas de Krummi en ellas (L13).
- La marca de espera de las 13:03:28 en el registro de fases se añadió a mano con una hora medida (ver *Tiempo*).
- Los hallazgos R2-003 y R3-linux-report-unknown-revision se descartaron sin editar el informe generado por el banco.

## Pendiente (no bloquea)

- **Revisión cruzada de Codex** del PRD `pager-commit-checks` y del código de la ejecución (`22d045a..HEAD`): el
  proyecto la pide antes del informe de cierre, pero Codex está bloqueado por su límite de uso hasta el 2026-11-11
  a las 06:49. Queda para entonces, sobre la rama o sobre `main` si ya se fusionó.
- **W11 en Windows, −1,9 %**: coste pequeño confirmado; la carga cumple (0,86). M6 se descartó porque el coste está
  en el primer acceso a la memoria de la hoja (L8).
- **W18 bimodal en Windows**: depende de la copia del almacén; ya se había visto antes y no viene de esta rama.

**Riesgo:** ítem 4 (el filtro de versiones sin cerrojo es concurrencia), cubierto con la revisión RDD alta, la prueba
de estrés del verificador y la prueba determinista del filtro; ítem 6 no aplica a M8, que tiene su prueba.

## Limpieza manual

- `work/margin` (7,1 GB): binarios, almacenes de las rotaciones y confirmaciones, y scripts de medición.
- En WSL 2: `~/margin2` (3,6 GB), `~/krummi-mg-target` (269 MB), `~/krummi-mg-src` (8,2 MB) y `~/bench-work-mg`.
