# Reorganización de `docs` y `odd/tasks` terminada: ejecuciones archivadas por módulo y PRD agrupados por su árbol, rama verificada

Informe de cierre de `docs-layout` (2026-11-13). Enfoque de ejecución: ODD.
Fuente de producto: ninguna; es una reorganización de documentación y herramientas, sin PRD (`CLAUDE.md`).
Documento de ejecución: [docs-layout.md](docs-layout.md).

Rama `feat/docs-layout`, creada desde `main` en `80027e1`. Sin fusionar y sin publicar: queda para la revisión del
usuario. Su último commit archiva esta misma ejecución en `odd/done/tooling/` con el script nuevo.

## Result

- **Especificación:**
  - **S1:** las 36 ejecuciones terminadas (114 archivos) están en `odd/done/<módulo>/`; `odd/tasks/` queda para las
    que estén en curso.
  - **S2:** los 23 PRD con sus traducciones (46 archivos) están en `docs/prd/krummi/<módulo>/`, siguiendo su árbol;
    `krummi.prd.md` y su traducción siguen en la raíz.
  - **S3:** los 160 movimientos reescribieron las referencias de 258 archivos, enlaces relativos y rutas desde la
    raíz, incluidos los informes de Codex. Los enlaces rotos son los mismos 37 de antes, con la misma ruta de destino
    (comparados por script, L5).
  - **S4:** `CLAUDE.md`, `AGENTS.md`, `README.md`, los skills de cierre, PRD y Codex y los skills de revisión
    describen la estructura nueva. El skill de cierre archiva la ejecución como último paso (paso 6).
  - **S5:** `scripts/docs/move_docs.py` (`archive`, `reopen`, `move`, `check`), con 25 tests en
    `scripts/docs/test_move_docs.py`, todos pasan.
- **Checks:** `cargo fmt --all --check` limpio en `ddff0e1`. `cargo clippy` y `cargo test --workspace` (831 pasan, 0
  fallan, 34 ignorados) se ejecutaron en `e3a390c`; desde ahí no cambió ningún archivo de `crates`, `tools`,
  `Cargo.toml` ni `Cargo.lock`. Los tests de los scripts de cierre pasan, y los 20 de Pester del wrapper de Codex
  también, en `e3a390c` (su script no cambió después).
- **Revisiones y verificación:**
  - **RDD:** la revisión de todo el tramo se detuvo con `lens_context_budget_exceeded`, porque la migración son unas
    59.700 líneas. Se revisó en candidatos más pequeños (tabla de abajo); el commit de la migración, `2503b38`, se
    verificó mecánicamente (S3) y lo revisó Codex dentro del rango completo.
  - **Codex:** cuatro rondas; la cuarta, autorizada por el usuario tras el cierre.
    - Ronda 1: R1 (mayor), R2 (mayor) y R3 (menor).
    - Ronda 2: R2 y R3 verificados; R1 seguía abierto y apareció R4 (mayor), una regresión de la corrección de R2.
    - Ronda 3: R4 verificado; R1 seguía abierto con otra variante.
    - Ronda 4: R1 verificado sobre `9518704..438889a` (`b2674f1`, `240d1a6`, `82b1a85`, `ddff0e1`); aprobado, sin
      hallazgos abiertos ([informe](../../../docs/reviews/docs-layout/2026-11-13-code-04.md)).
- **Hallazgos corregidos por el camino:** un enlace de un comentario de Rust que el script no reescribía (L5), y las
  correcciones de RDD y Codex de L7–L19, cada una con un test que falla sin ella.

## Acceptance criteria coverage

| Spec | Test o comprobación | Resultado |
|---|---|---|
| S1, S2 | Manifiesto de 160 movimientos sin archivos fuera, salvo la raíz y esta ejecución (L5) | PASS |
| S3 | `check` antes y después de la migración, comparado por archivo y destino | PASS: los mismos 37 |
| S3, S5 | `scripts/docs/test_move_docs.py`: enlaces en línea y de referencia, `<...>`, paréntesis, Rust, notas al pie, rutas desde la raíz, archivo y reapertura, validación completa antes de escribir y reversión | PASS (25) |
| S4 | Lectura de los cambios de las instrucciones; la revisan RDD (`2503b38..c8b751a`) y Codex | Hecho |

## RDD reviews

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `80027e1..e3a390c` | readability, reliability | detenido: `lens_context_budget_exceeded` | ninguno; no se creó autoridad; se partió en los de abajo | `review-0606d9f3b306502b` |
| `80027e1..0956809` | reliability, readability | aprobado | 6 WARNING y 3 SUGGESTION; uno descartado como comportamiento (R2-003) y el resto corregido en `c8b751a` (L7) | `review-0d938d1b24126f81` |
| `2503b38..c8b751a` | readability, reliability | aprobado | 3 WARNING y 2 SUGGESTION, corregidos en `8bb718f` (L8) | `review-144db57e6b9df6e1` |
| `c8b751a..8bb718f` | reliability | aprobado | 1 WARNING (notas al pie), corregido en `700e276` (L9) | `review-849d614c0293a1e6` |
| `8bb718f..700e276` | reliability | aprobado | 2 SUGGESTION sobre el test, aplicados en `c20308b` (L10) | `review-939756838c9a509e` |
| `700e276..c20308b` | reliability | aprobado | 1 WARNING y 1 SUGGESTION: el test seguía sin probar el movimiento; corregido en `a406e8e` (L11) | `review-52d682c98601b663` |
| `c20308b..a406e8e` | reliability | aprobado | 1 SUGGESTION descartado con justificación (L12) | `review-f084bff4cd70f760` |
| `a406e8e..e62cc4f` | reliability, readability | aprobado | 2 WARNING y 3 SUGGESTION, corregidos en `780c08a` (L14) | `review-5fa68e0f7b624933` |
| `e62cc4f..780c08a` | readability, reliability | aprobado | 1 WARNING y 1 SUGGESTION: la comparación sin mayúsculas se mantiene a propósito, y el comentario lo dice (`9518704`, L15) | `review-ac10c3b0dc80e6f7` |
| `780c08a..bf74160` | readability, reliability | aprobado | 2 WARNING y 4 SUGGESTION; uno descartado con justificación (R2-002), el resto corregido en `240d1a6` (L17) | `review-e059736b6e56f71a` |
| `bf74160..240d1a6` | reliability | aprobado | 2 WARNING y 1 SUGGESTION, corregidos en `82b1a85` (L18) | `review-834aa9c11e321ef5` |
| `240d1a6..82b1a85` | reliability | aprobado | 1 SUGGESTION, aplicado en `ddff0e1` (L19) | `review-03ce4adc925b4db6` |
| `82b1a85..ddff0e1` | reliability | aprobado | ninguno | `review-79acd6237a555037` |

Todas las evaluaciones dieron riesgo medio, `under_budget`; cada revisión se pidió con `--force`, como tramo del
cierre.

## Time

- Estimación: 1–2 h. Arranque real: 2026-11-13 10:07:24. Fin: 11:46:05.
- **Duración activa desde el arranque: 1 h 38 min**, dentro de la estimación.
  - El código del script y la migración llevaron 44 min.
  - La revisión llevó 26 min, más 17 min de correcciones: trece revisiones RDD y tres rondas de Codex, porque cada
    corrección trajo una revisión que encontró algo más del script (rutas raras, la reversión, nombres de Windows).
- Esperas por el usuario: ninguna.

### Time by phase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 2 min 15 s | 2 % | 1 |
| Código y tests | 44 min 12 s | 45 % | 3 |
| Checks (fmt, clippy, suite de tests) | 9 min 06 s | 9 % | 2 |
| Revisión RDD | 26 min 31 s | 27 % | 10 |
| Correcciones de hallazgos | 16 min 37 s | 17 % | 9 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **1 h 38 min** | **100 %** | **26** |

- Inicio: 2026-11-13 10:07:24 +00:00. Fin: 2026-11-13 11:46:05 +00:00.
- Arranque confirmado: 10:07:24. Activo desde el arranque: **1 h 38 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | rdd-slice | 11:02:43 | 11:02:50 | 0 min 07 s |
| Revisión RDD | rdd-script | 11:03:03 | 11:04:24 | 1 min 21 s |
| Revisión RDD | rdd-instructions | 11:09:09 | 11:10:09 | 1 min 00 s |
| Revisión RDD | rdd-fix2 | 11:11:57 | 11:12:56 | 0 min 59 s |
| Revisión RDD | rdd-fix3 | 11:14:51 | 11:15:27 | 0 min 36 s |
| Revisión RDD | rdd-test | 11:16:33 | 11:17:11 | 0 min 38 s |
| Revisión RDD | rdd-test2 | 11:17:53 | 11:18:50 | 0 min 57 s |
| Revisión RDD | codex-R1 | 11:19:00 | 11:22:15 | 3 min 15 s |
| Revisión RDD | codex-R2 | 11:26:10 | 11:29:14 | 3 min 04 s |
| Revisión RDD | rdd-codex-fixes | 11:30:47 | 11:32:11 | 1 min 24 s |
| Revisión RDD | rdd-case | 11:33:10 | 11:34:01 | 0 min 51 s |
| Revisión RDD | codex-R3 | 11:34:18 | 11:37:04 | 2 min 46 s |
| Revisión RDD | rdd-r3 | 11:39:16 | 11:40:29 | 1 min 13 s |
| Revisión RDD | rdd-undo | 11:42:08 | 11:43:02 | 0 min 54 s |
| Revisión RDD | rdd-undo2 | 11:44:24 | 11:45:01 | 0 min 37 s |
| Revisión RDD | rdd-last | 11:45:19 | 11:45:43 | 0 min 24 s |
| **Revisión RDD (total)** | | | | **20 min 06 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `405e155` | feat(scripts): add a script that moves documents and rewrites their references | T0, T1 | 3 min 19 s | Exploración y mapa 2 min 15 s, Código y tests 1 min 04 s |
| `0956809` | fix(scripts): rewrite the links of Rust doc comments when documents move | T1, T2 | 3 min 35 s | Código y tests 3 min 35 s |
| `2503b38` | docs: archive finished executions and group the PRDs by module | T2 | 0 min 10 s | Código y tests 0 min 10 s |
| `d71f11e` | feat(scripts): reopen an archived execution | T2, T3 | 39 min 17 s | Código y tests 39 min 17 s |
| `e3a390c` | docs: describe the archived executions and the PRD folders in the instructions |  | 0 min 00 s |  |
| `c8b751a` | fix(scripts): address the review of the document move script | T3, T4 | 14 min 31 s | Código y tests 0 min 06 s, Checks (fmt, clippy, suite de tests) 8 min 52 s, Revisión RDD 2 min 34 s, Correcciones de hallazgos 2 min 59 s |
| `8bb718f` | fix(scripts): keep each file kind's rules for reference definitions | T4 | 3 min 40 s | Revisión RDD 1 min 12 s, Correcciones de hallazgos 2 min 28 s |
| `700e276` | fix(scripts): leave footnote definitions alone | T4 | 2 min 44 s | Revisión RDD 1 min 23 s, Correcciones de hallazgos 1 min 21 s |
| `c20308b` | test(scripts): prove that footnotes stay out of both reference patterns | T4 | 1 min 53 s | Revisión RDD 1 min 42 s, Correcciones de hallazgos 0 min 11 s |
| `a406e8e` | test(scripts): make the footnote test fail on a rewritten footnote | T4 | 1 min 20 s | Revisión RDD 1 min 20 s |
| `977e8b7` | docs(reviews): add the docs layout code review | T4 | 4 min 43 s | Revisión RDD 4 min 43 s |
| `ec5011b` | fix(scripts): check the destination folders, follow angle-bracket links and speak Spanish | T4 | 3 min 18 s | Correcciones de hallazgos 3 min 18 s |
| `87c0ce0` | docs(reviews): record the resolutions of the docs layout code review | T4 | 0 min 16 s | Correcciones de hallazgos 0 min 16 s |
| `687caad` | docs(reviews): add the docs layout follow-up review | T4 | 3 min 37 s | Revisión RDD 3 min 37 s |
| `349fb94` | fix(scripts): compare normalized paths and keep parentheses in reference destinations | T4 | 0 min 44 s | Correcciones de hallazgos 0 min 44 s |
| `e62cc4f` | docs(reviews): record the second round of the docs layout resolutions | T4 | 0 min 16 s | Correcciones de hallazgos 0 min 16 s |
| `780c08a` | fix(scripts): compare move destinations without case and compose the link patterns | T4 | 2 min 23 s | Revisión RDD 1 min 34 s, Correcciones de hallazgos 0 min 49 s |
| `9518704` | docs(scripts): say why move destinations are compared without case everywhere | T4 | 1 min 08 s | Revisión RDD 1 min 08 s |
| `075e2d9` | docs(reviews): add the third docs layout review round | T4 | 3 min 12 s | Revisión RDD 3 min 12 s |
| `b2674f1` | fix(scripts): refuse names Windows rewrites and undo a move that fails midway | T4 | 1 min 31 s | Correcciones de hallazgos 1 min 31 s |
| `bf74160` | docs(reviews): record the third round of the docs layout resolutions | T4 | 0 min 15 s | Correcciones de hallazgos 0 min 15 s |
| `240d1a6` | fix(scripts): make the undo of a failed move failure-safe and narrow the reserved names | T4 | 2 min 52 s | Revisión RDD 1 min 35 s, Correcciones de hallazgos 1 min 17 s |
| `82b1a85` | fix(scripts): report the folders the undo cannot remove and prove where texts are restored | T4 | 2 min 16 s | Revisión RDD 1 min 04 s, Correcciones de hallazgos 1 min 12 s |
| `ddff0e1` | test(scripts): check the restored file and the left folder after a failed undo | T4 | 0 min 55 s | Revisión RDD 0 min 55 s |
| — | después del último commit | T4 | 0 min 46 s | Checks (fmt, clippy, suite de tests) 0 min 14 s, Revisión RDD 0 min 32 s |
| **Total** | | | **1 h 38 min** | |

La fila de `d71f11e` (39 min) acumula el trabajo de T2 y T3 (migración, comprobaciones e instrucciones) hasta ese
commit, porque la fase no cambió entre ellos. La de `e3a390c` queda a cero por la misma razón.

## Change report (`80027e1..ddff0e1`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 1.479 | 1.058 | — | — | 188 |
| `odd` | 523 | 259 | — | — | 116 |
| `scripts` | 754 | 0 | 559 | 89 | 2 |
| `README.md` | 17 | 15 | — | — | 1 |
| `.claude` | 18 | 5 | 0 | 1 | 4 |
| `crates/krummi-bench` | 5 | 5 | 0 | 1 | 5 |
| `CLAUDE.md` | 5 | 4 | — | — | 1 |
| `AGENTS.md` | 4 | 4 | — | — | 1 |
| `.agents` | 4 | 3 | — | — | 2 |
| `crates/krummi` | 2 | 2 | 0 | 2 | 2 |
| `crates/krummi-pager` | 1 | 1 | 0 | 1 | 1 |
| `crates/krummi-tree` | 1 | 1 | 0 | 1 | 1 |
| `crates/krummi-tx` | 1 | 1 | 0 | 1 | 1 |
| `tools` | 1 | 1 | — | — | 1 |
| **Total** | **2.815** | **1.359** | **559** | **96** | **326** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Casi todo el volumen de `docs` y `odd` son los 160 renombrados con sus enlaces reescritos. Los tres informes de Codex y
sus resoluciones suman unas 400 líneas de `docs`, y el documento de ejecución unas 250 de `odd`.

## Summary

`odd/tasks` y `docs/prd/krummi` ya no crecen sin límite:
- **Ejecuciones:** las terminadas se archivan por módulo en `odd/done/`, y `odd/tasks/` solo tiene las que están en
  curso.
- **PRD:** se agrupan por módulo, igual que su árbol.
- **Script:** un solo script con tests hace los movimientos y reescribe cada enlace y cada ruta. Lo usaron esta
  migración y el archivo de esta ejecución, y lo usará el skill de cierre en cada ejecución que termine.
- **Seguridad del script:** un movimiento inválido se rechaza antes de escribir nada, y uno que falla a medias se
  deshace.

## Defaults chosen without asking

- **Reparto por módulo:** el de L3, que el usuario vio antes de arrancar.
- **Revisión de la migración:** al no caber en RDD, se verificó con la comparación de enlaces rotos y con Codex, en
  vez de reescribir la historia para partirla.
- **Mayúsculas:** los destinos se comparan sin mayúsculas en todos los sistemas, también en Linux (L15).
- **Nombres de Windows:** se rechazan los nombres que Windows no conserva tal cual (L16, L17).
- **Enlaces en código:** los enlaces dentro de código se tratan como enlaces (el docstring lo dice). Por eso `check`
  lista 45, los 37 de antes más 8 ejemplos de los informes de Codex.

## Pending (non-blocking)

- **`check` y los ejemplos en código:** `check` no distingue el código y lista 8 ejemplos de los informes de Codex.
  Para comparar antes y después de un archivo basta con el recuento.
- **La otra sesión:** usa los PRD de tablas, que ahora están en `docs/prd/krummi/tables/`. Encontrará los archivos en
  la ruta nueva, con sus enlaces reescritos.

**Risk:** item 3, contratos que otros consumen: cambian las rutas que siguen las instrucciones, los skills y las otras
sesiones. Está mitigado, porque las referencias se reescribieron y `check` lo comprueba.

## Manual cleanup

- `work/docs-layout/`: manifiesto, listas de enlaces rotos, scripts de corrección y de mutación, sobres de RDD y
  resultados de Codex. Es la evidencia que cita el documento de ejecución.
