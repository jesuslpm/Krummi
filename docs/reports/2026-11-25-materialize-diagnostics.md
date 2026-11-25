# Diagnóstico del simulador de cortes: terminado y verificado en su rama

Informe de cierre de `materialize-diagnostics` (2026-11-25). Forma de ejecución: ODD, camino pequeño, sin documento
de ejecución. Origen: el fallo intermitente de `ac_j7_deferred_commits_survive_as_a_prefix` en el CI de Linux (run
45616344885), con el mensaje `no entry found for key` en `crates/krummi-pager/src/sys/faults.rs:263`. Es un cambio de
tests, sin cambio de producto, así que no tiene PRD.

Rama `test/materialize-diagnostics`, creada desde `main` (`dfba65f`), con dos commits: `3fcb1b8` y `b0d32df`. No se
fusionó ni se subió nada.

## Resultado

- **Cambio:** cuando una grabación escribe, cambia de tamaño, sincroniza o renombra un archivo que no está vivo,
  `materialize` ya no paniquea con `no entry found for key`. Ahora el mensaje dice:
  - qué operación lo usa y cuántas operaciones tiene la grabación;
  - qué operaciones crean, renombran o borran ese archivo antes y después de ella, que es la pista de un orden
    cruzado entre hilos.

  Un archivo de fuera de la carpeta de origen que sobreviviría da un mensaje propio, con el archivo y la carpeta. Una
  eliminación de un nombre que no está vivo sigue sin hacer nada, y ahora un test fija ese comportamiento.
- **Tests:** 6 tests nuevos en `sys::faults::tests`, todos en verde. Los 5 primeros se vieron fallar antes del cambio
  con las panics anteriores (`263`, `269`, `276` y `303`).
- **Checks:**
  - sobre `3fcb1b8`: `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings` limpios, y
    `cargo test --workspace` con 1263 superados y 0 fallados;
  - sobre `b0d32df`: fmt, clippy del workspace y los tests de `sys::faults` en verde (6 superados). Solo cambió
    `faults.rs`, que únicamente compila en los tests.
- **Revisiones:**
  - dos revisiones RDD, aprobadas (detalle en la sección siguiente);
  - Codex aprobó en la primera ronda, sin hallazgos:
    [2026-11-25-code.md](../reviews/materialize-diagnostics/2026-11-25-code.md), `threadId`
    `01a21332-0e24-7fc2-a464-34ff967aded9`, coordinador y revisor `gpt-6.1-sol`/`high` según la telemetría.
- **Hallazgos corregidos:** las tres sugerencias de la primera revisión RDD que se aplicaron (siguiente sección).

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `dfba65f..3fcb1b8` | readability, reliability | aprobada | 5 SUGGESTION. R2-001: el nombre de un test no coincidía con lo que comprueba; renombrado en `b0d32df`. R2-002 y R3-002: el comentario de `materialize` prometía más de lo que hacen la ruta de fuera del origen y la eliminación; reescrito, con un test nuevo para la eliminación, en `b0d32df`. R2-003 y R3-001: el registro de fases parecía ajeno al cambio y tener columnas irregulares; descartadas, porque es el registro que exige el `closing-report` y `cat -A` muestra cuatro columnas en todas las filas | `review-ad9b63335cd47511` |
| `3fcb1b8..b0d32df` | readability, reliability | aprobada | 3 SUGGESTION, descartadas. La fila del registro de fases sin columna final: `cat -A` muestra la columna en todas las filas, así que el revisor vio el archivo a medio escribir. La eliminación probada con un solo modelo: en `materialize`, `Remove` no depende del modelo, que solo se aplica al final, en `after_cut`. El mensaje de fuera del origen sin test: lo cubre `a_file_outside_the_source_folder_is_named_with_the_folder`. Queda sin test el caso de un archivo de fuera del origen que no sobrevive | `review-42de622b2f63b776` |

## Tiempo

- Estimación: entre 30 y 60 minutos. Inicio: 2026-11-25 14:33:10. Fin: 2026-11-25 15:04:00.
- **Duración activa desde el inicio: 30 min 50 s**, en el extremo bajo de la estimación.
- Esperas por el usuario: ninguna.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Código y tests | 4 min 30 s | 15 % | 1 |
| Checks (fmt, clippy, suite de tests) | 10 min 50 s | 35 % | 1 |
| Revisión RDD | 14 min 47 s | 48 % | 2 |
| Correcciones de hallazgos | 0 min 43 s | 2 % | 1 |
| Cierre (informe y merge) | 0 min 00 s | 0 % | 1 |
| **Total activo** | **30 min 50 s** | **100 %** | **6** |

- Inicio: 2026-11-25 14:33:10 +00:00. Fin: 2026-11-25 15:04:00 +00:00.
- Arranque confirmado: 14:33:10. Activo desde el arranque: **30 min 50 s** (compárese con la estimación); antes del
  arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `3fcb1b8` | test(pager): name the operation and path when a recording uses a file that is not live |  | 15 min 19 s | Código y tests 4 min 30 s, Checks (fmt, clippy, suite de tests) 10 min 49 s |
| `b0d32df` | test(pager): describe what materialize reports and pin removals of names that are not live |  | 3 min 12 s | Checks (fmt, clippy, suite de tests) 0 min 01 s, Revisión RDD 2 min 28 s, Correcciones de hallazgos 0 min 43 s |
| — | después del último commit | codex-R1 | 12 min 19 s | Revisión RDD 12 min 19 s |
| **Total** | | | **30 min 50 s** | |

La mitad del tiempo se fue en las revisiones (las dos de RDD y la de Codex, que cuenta como `review`) y un tercio en
la suite completa. El código fue lo de menos.

## Cambios (`main..b0d32df`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi-pager` | 210 | 7 | 187 | 12 | 1 |
| `docs` | 4 | 0 | — | — | 1 |
| **Total** | **214** | **7** | **187** | **12** | **2** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell,
JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

## Resumen

El simulador de cortes de luz de los tests reconstruye un disco a partir de las operaciones grabadas por los hilos del
almacén. Cuando la grabación sale desordenada, antes fallaba con un mensaje que no decía nada. Ahora dice qué operación
falló y qué operaciones crearon, renombraron o borraron ese archivo, tanto antes como después de ella. Así, si
`ac_j7_deferred_commits_survive_as_a_prefix` vuelve a fallar en el CI, el log va a mostrar qué orden se dio. El
comportamiento con las grabaciones correctas no cambia.

## Decisiones tomadas sin preguntar

- La rama sale de `main` y no de `feat/tables-vector-indexes`, porque el cambio no depende de esa rama.
- La eliminación de un nombre que no está vivo se mantiene como hasta ahora, sin hacer nada. Que paniqueara también
  sería otro diagnóstico, pero podría romper tests que hoy pasan.

## Pendiente (no bloqueante)

- **La causa del fallo de `ac_j7` sigue sin conocerse.** No se reprodujo en 30 ejecuciones en Windows, 300 en Linux
  (WSL) ni unas 50 pasadas del binario entero con 4 CPUs. Este cambio solo mejora lo que dirá el log la próxima vez.
- **La grabación no guarda qué hilo hizo cada operación.** Sería la pista más directa, pero cambia el tipo público
  `Recorded`. Queda para cuando el nuevo mensaje no alcance.
- **Sin test:** un archivo de fuera del origen que no sobrevive no paniquea (sugerencia de la segunda revisión RDD).

**Riesgo:** ítem 6. Un error en un mensaje de diagnóstico no lo atraparía ningún test que no sea de estos mensajes;
los 6 tests nuevos fijan su formato.
