# Altas al final de la última hoja en los árboles: W1 +48 % en Windows (1,91 frente a Voron); 29 de 29 umbrales en Windows y WSL

Informe de cierre de `tree-append` (2026-11-18). Enfoque de ejecución: ODD.
Fuente de producto: ninguna. Es una optimización sin cambio de comportamiento, propuesta por el usuario (L1): llevar a la
API clave-valor el atajo que las tablas ya usaban. Documento de ejecución: [tree-append.md](tree-append.md).

Rama `feat/tree-append`, creada desde `feat/night-optimizations` (`504cd84`, código igual a `d96b9c2`). Sin fusionar y
sin publicar.

## Resultado

- **Especificación:**
  - **T1 (S1, S2, `bf4b998`).** `set` y `try_add` de un árbol normal agregan una clave mayor que todas las del árbol
    al final de su última hoja recordada, sin recorrer el árbol.
    - *Cómo:* se reutiliza `krummi_tree::append_last`, el atajo de las tablas. Un envoltorio nuevo, `set_at_last`, hace
      con `set` lo que `try_add_at_last` hace con `try_add`. La función `update` del árbol no cambia.
    - *Olvido:* lo recordado se olvida en cualquier otro cambio del árbol: un `set` o `try_add` que no agrega al
      final, `remove`, los borrados por rango, vaciar o borrar el árbol, y un fallo.
    - *Tests:* los tres de `crates/krummi/src/append_tests.rs`, con RED observado antes del código:
      - el atajo deja las entradas, la raíz y la cuenta iguales a recorrer el árbol;
      - cada regla de olvido;
      - el fallo por tamaño máximo.
    - *Equivalencia en el árbol:* `set_at_last` deja los mismos nodos byte a byte que `try_add_at_last`.
    - *Tests añadidos por RDD (`1bcbf5f`):* una clave nueva antes del final no devuelve ninguna hoja, tampoco al final
      de una hoja que no es la última, y un `set` de una clave nueva en el medio olvida lo recordado.
  - **T2 (S3).** A/B en Windows. El usuario detuvo las repeticiones de 20 rondas y aceptó las marcas (L6, L7).
  - **T3 (S4).** Ejecuciones completas, dos revisiones RDD y la revisión de Codex.
  - **Checks finales**, sobre `1bcbf5f`: fmt y clippy limpios; `cargo test --workspace` da 874 pasados y 0 fallidos.
- **Aceptación (S3)**: A/B de Krummi solo en Windows (`not2` = código de la base, `app` = `bf4b998`), con el plan de
  energía del banco. Las cargas largas van con 3 rondas y las cortas con 10 (L5).
  - **W1 1,482** (1.051 → 709 ns por operación; más rápida en 3 de 3).
  - Mejoran o quedan igual: W14 1,013, W23 1,014, W29 1,012, W25 1,009, W15 1,008, W27 1,008, W19 1,007, W22 1,005,
    W2 1,004 y W5 1,001.
  - Marcadas (mediana menor y más lenta en la mayoría de las rondas): W17 0,903 (2/3), W12 0,946 (6/10), W4 0,973
    (3/3), W3 0,978 (3/3), W7 0,978 (9/10), W24 0,981 (2/3), W26 0,985 (3/3), W21 0,993 (3/3), W6 0,998 (7/10) y W28
    0,999 (2/3).
    - W7 y W26 solo leen.
    - En W17 los valores van fuera de la hoja, así que `append_last` se descarta antes de leer una página.
    - Las repeticiones de 20 rondas se detuvieron sin terminar, a pedido del usuario: «No hagas más rondas.»
- **Ejecuciones completas (S4)** sobre el código `bf4b998`, Krummi y Voron:

  | Sistema | Resultado | carga-en-orden | lecturas-con-escritor | escritor-con-lectores |
  |---|---|---|---|---|
  | Windows | 29 de 29 | **1,91** (antes 1,38–1,45) | 0,99 | 1,11 |
  | WSL 2 | 29 de 29 | 1,08 (antes 1,09) | 1,20 | 1,16 |

  - En Windows, Krummi hace 1.295.570 op/s en carga-en-orden.
  - En WSL 2 hace 150.562 op/s, frente a 154.783 del código de la base anoche. En Linux esa carga va a una décima del
    ritmo de Windows, así que el recorrido que se ahorra no se nota.
  - En Windows, W12 pasa con Krummi en 1,43 M op/s, porque Voron también bajó en esa corrida (1,44 M).
  - Los informes están en [Windows](../../../docs/benchmark/2026-11-18-windows-tree-append.md) y
    [Linux](../../../docs/benchmark/2026-11-18-linux-tree-append.md).
- **Revisiones y verificación:**
  - Dos revisiones RDD aprobadas (tabla abajo).
  - No hubo verificador independiente. El ítem 3 se escaló en RDD, y los tests de equivalencia con el recorrido del
    árbol fijan los bytes, la raíz y la cuenta.
  - Codex aprobó en la primera ronda: «Approved: no introduced findings in the reviewed range»
    ([informe](../../../docs/reviews/tree-append/2026-11-18-code.md)). Anota como limitación, no como defecto, que las
    marcas del A/B quedan aceptadas sin repetir y que no se probó de forma independiente qué build midió cada corrida
    completa.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `504cd84..0a9c354` (el cambio y sus registros; alta, ítem 3 escalado) | riesgo, resiliencia, fiabilidad | aprobado | R4-unverified-write-regression, WARNING: las marcas de W12 y W17 se aceptaron sin repetir, y W1 no gana en Linux. No se cambia: lo decidió el usuario (L6, L7); queda pendiente. R3-set-at-last-non-rightmost-coverage, WARNING: faltaba el test de una clave nueva antes del final. Corregido en `1bcbf5f`; pasó a la primera | `review-6b998102177bc84b` |
| `0a9c354..1bcbf5f` (los tests añadidos; media, `under_budget`, último tramo) | fiabilidad | aprobado | Ninguno | `review-4cec57d602188637` |

El commit `85b9979` y los posteriores solo tocan documentos.

## Tiempo

- Estimación: «Entre 4 h y 6 h» (L2). Inicio real: 11:14:46. Fin: 14:03:45.
- **Duración activa: 2 h 46 min**, por debajo de la estimación.
  - La aceptación se llevó el 77 %: el A/B (45 min), las repeticiones detenidas (13 min) y las ejecuciones completas
    (69 min).
  - El código y sus tests llevaron menos de 5 min, porque el mecanismo ya existía en las tablas.
- Esperas por el usuario: 2 min 20 s, la decisión sobre las marcas.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 1 min 24 s | 1 % | 1 |
| Código y tests | 3 min 29 s | 2 % | 1 |
| Checks (fmt, clippy, suite de tests) | 17 min 01 s | 10 % | 2 |
| Aceptación | 2 h 08 min | 77 % | 2 |
| Revisión RDD | 15 min 17 s | 9 % | 2 |
| Correcciones de hallazgos | 1 min 14 s | 1 % | 1 |
| Cierre (informe y merge) | 0 min 01 s | 0 % | 1 |
| **Total activo** | **2 h 46 min** | **100 %** | **10** |

- Inicio: 2026-11-18 11:14:46 +00:00. Fin: 2026-11-18 14:03:45 +00:00.
- Arranque confirmado: 11:14:46. Activo desde el arranque: **2 h 46 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 2 min 20 s.
  - 12:26:12–12:28:32 (2 min 20 s): decision on the marks

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | ab-windows | 11:27:44 | 12:12:17 | 44 min 33 s |
| Aceptación | ab-repeat20 | 12:12:47 | 12:26:12 | 13 min 25 s |
| Aceptación | full-windows | 12:28:39 | 12:50:43 | 22 min 04 s |
| Aceptación | full-linux | 12:50:43 | 13:37:55 | 47 min 12 s |
| Revisión RDD | codex-R1 | 13:53:33 | 14:03:23 | 9 min 50 s |
| **Aceptación (total)** | | | | **2 h 07 min** |
| **Revisión RDD (total)** | | | | **9 min 50 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `bf4b998` | perf(krummi): append ordered sets and adds at the end of a tree's last leaf | T1 | 12 min 48 s | Exploración y mapa 1 min 24 s, Código y tests 3 min 29 s, Checks (fmt, clippy, suite de tests) 7 min 55 s |
| `f2b2b05` | docs(prd): draft the secondary indexes PRD of the tables | T1, T2 | 11 min 59 s | Checks (fmt, clippy, suite de tests) 0 min 09 s, Aceptación 11 min 50 s |
| `605d0ff` | docs(odd): record the tree-append A/B and the user's decision | T2 | 46 min 39 s | Aceptación 46 min 39 s |
| `0a9c354` | docs(odd): record the tree-append complete runs | T2, T3 | 1 h 09 min | Aceptación 1 h 09 min, Revisión RDD 0 min 01 s |
| `1bcbf5f` | test(tree): pin that set_at_last reports no leaf for a new key before the end | T3, T1 | 14 min 04 s | Checks (fmt, clippy, suite de tests) 8 min 51 s, Revisión RDD 3 min 59 s, Correcciones de hallazgos 1 min 14 s |
| `85b9979` | docs(odd): record the RDD reviews of tree-append | T1, T3 | 1 min 08 s | Checks (fmt, clippy, suite de tests) 0 min 06 s, Revisión RDD 1 min 02 s |
| — | después del último commit | T3 | 10 min 16 s | Revisión RDD 10 min 15 s, Cierre (informe y merge) 0 min 01 s |
| **Total** | | | **2 h 46 min** | |

- `f2b2b05` es el borrador del PRD de índices secundarios del usuario. Se confirmó en esta rama durante la ejecución,
  mientras corría el A/B, y no es parte de ella.
- La fila «Aceptación» incluye la espera de los A/B y de las ejecuciones completas en primer plano.

## Informe de cambios (`504cd84..HEAD`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 232 | 5 | 194 | 20 | 2 |
| `odd` | 119 | 0 | — | — | 2 |
| `crates/krummi-tree` | 73 | 0 | 63 | 7 | 2 |
| **Total** | **424** | **5** | **257** | **27** | **6** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Aparte, el commit del usuario `f2b2b05`: `docs/prd/krummi/tables/tables-indexes.prd.md` (+74 −25) y
`docs/reviews/tables-indexes/2026-11-18-prd.md` (+106 −0). Las cifras se tomaron antes del commit de este informe.

La mayor parte del código añadido son tests: `append_tests.rs` y los dos tests del árbol. El cambio de producción es
`put` y su estado en `crates/krummi/src/lib.rs`, más el envoltorio `set_at_last` en `crates/krummi-tree/src/lib.rs`.

## Resumen

Ahora, cuando una aplicación escribe claves crecientes en un árbol con `set` o `try_add` (secuencias, marcas de tiempo,
ULID monótonos), la clave nueva va directo al final de la última hoja, sin bajar por el árbol. En Windows la carga en
orden es un 48 % más rápida y pasa de 1,38–1,45 a 1,91 frente a Voron. En Linux no cambia, porque ahí esa carga no la
limita el recorrido.

La idea fue del usuario: generalizar el atajo que ya tenían las tablas, en la API y sin tocar el núcleo del árbol. Un
intento anterior, `ordered-write-cache`, puso la caché dentro de `update` y ganaba un 13 %, pero empeoraba W3.

## Decisiones tomadas sin preguntar

- **Un `try_add` de una clave que ya está también olvida la hoja**, como en las tablas, aunque no cambie el árbol
  (L4).
- **La rama sale de `feat/night-optimizations`**, para medir contra el código más reciente, que tiene T1.
- **El consentimiento de las revisiones automáticas** se concedió siempre, según la regla del proyecto.

## Pendiente (no bloqueante)

- **Las marcas del A/B** (W17 0,903, W12 0,946 y las de entre 0,97 y 0,999) quedaron sin repetir, por decisión del
  usuario. Si se quiere cerrarlas, hay un A/B de 20 rondas preparado en `work/ta/rep.sh`. Lo señalan RDD R4 y Codex.
- **Por qué esta versión gana un 48 % y la caché anterior un 13 %.** No se midió. Sirve para saber qué costaba el
  recorrido.
- **W1 no gana en Linux** (1,08 frente a 1,09). Un perfil diría en qué se va el tiempo ahí.
- **El plan de rendimiento** (`docs/performance/plan.md`) no tiene una fila de esta ejecución. Se puede añadir al
  fusionar.

**Riesgo:** ítem 3, por escalado. Cambia el camino de `set` y `try_add` de la API pública, aunque sin cambiar su
comportamiento. Lo cubren los tests de equivalencia con el recorrido del árbol (bytes, raíz y cuenta) y uno por regla
de olvido. Lo revisaron RDD, con tres lentes, y Codex.

## Limpieza manual

`work/ta/` guarda los registros y los JSON de los A/B y los informes completos. `work/wm/app-*` y `work/wm/not2-*`
son los builds del A/B. En WSL quedan `~/night/src` y `~/night/target`, del build de la corrida completa. Se puede
borrar todo cuando ya no haga falta la evidencia.
