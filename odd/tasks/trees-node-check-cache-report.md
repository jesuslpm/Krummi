# Comprobación de organización recordada de los nodos: implementada, verificada y aprobada por Codex

Informe de cierre de `trees-node-check-cache` (2026-10-30). Enfoque de ejecución: ODD.
Fuente de producto: [trees-node-check-cache.prd.md](../../docs/prd/krummi/trees-node-check-cache.prd.md).
Documento de ejecución: [trees-node-check-cache.md](trees-node-check-cache.md).

Rama `feat/trees-node-check-cache`; último commit de código `8cf01cf`, más los de registro y este informe. La revisión
cruzada de Codex falló primero por la cuota de uso (L10); se hizo a las 12:47, abrió R1 y R2, la segunda ronda abrió R3
y la tercera aprobó sin hallazgos abiertos (L12–L15). El usuario pidió fusionar y subir la rama cuando Codex la aprobara
("Lo fusionaremos y subiremos cuando codex lo revise luego.").

## Resultado

- **Especificación:** C1–C9 del PRD. Las lecturas de nodos del archivo de datos siguen verificando el código de
  integridad en cada lectura, y la organización de un contenido ya comprobado (número de página y código verificado)
  no se repite mientras el almacén está abierto. Las versiones del área temporal y las páginas de la escritura siguen
  comprobándose siempre. Pruebas: `cargo test --workspace`, 421 pasadas y 0 fallidas (408 antes de la ejecución, 3
  pruebas unitarias de la tabla, 9 de aceptación y 1 de memoria agotada al abrir, de la revisión de Codex); `cargo fmt --all --check` y
  `cargo clippy --workspace --all-targets -- -D warnings` limpios.
- **Aceptación:** la medición de C9 en Windows (dos veces) y en Linux (WSL 2):
  [trees-node-check-cache-measurements.md](trees-node-check-cache-measurements.md). Con la memoria de nodos, leer una
  hoja de 370 entradas cuesta un 86 % menos en Linux y un 88–90 % menos en Windows; una de 100, un 64 % y un 47–59 %
  menos; Contiene en un árbol multivalor con subárboles, un 66 % y un 71–72 % menos; en el árbol compuesto, un 44 % y
  un 36–55 % menos. Una hoja de una sola entrada cuesta 40 ns más en Linux (+13 %) y lo mismo en Windows.
- **Revisiones y verificación:** cinco revisiones RDD, aprobadas, y tres rondas de Codex, la última aprobada
  (secciones siguientes). Comprobación por mutación de las
  pruebas de aceptación: recordar antes de comprobar la organización hace fallar AC-C4 y AC-C8; no recordar nunca hace
  fallar AC-C1, AC-C2, AC-C3, AC-C6 y AC-C9. La mutación se hizo antes de las correcciones de Codex y no se repitió.
- **Hallazgos corregidos durante la ejecución:**
  - La redacción de C5 prometía que una consulta nunca acierta con el código de otra página; el diseño acordado sí
    puede acertar con una coincidencia de 64 bits, el riesgo que acepta C7. Se corrigió C5 en español e inglés
    (`09a9da4`, L3).
  - Una aserción de AC-C2 falló porque el ayudante de la propia prueba leía la hoja trasladada antes de contar; la
    comprobación de la página se movió al final (L6).
  - Las correcciones de la revisión RDD del último tramo (`ceab08d`, L8).
  - Codex R1 (mayor): abrir un almacén abortaba el proceso si el sistema no daba los 512 KiB de la tabla, en contra de
    P13; ahora la reserva es falible y la apertura falla con la clase de memoria agotada (`161c0e4`, RED observado).
  - Codex R2 (mayor): AC-C8 no garantizaba que los lectores leyeran mientras la escritura cambiaba y trasladaba los
    nodos; ahora se coordinan (`f95c57f`). Codex R3 (menor): esa coordinación colgaba la prueba si fallaba un hilo,
    en vez de fallar; ahora los lectores paran (`8cf01cf`).

## Cobertura de los criterios de aceptación

| Criterio | Prueba o comprobación | Resultado |
|---|---|---|
| AC-C1 | `check_tests::ac_c1_each_node_is_checked_once_while_the_store_is_open` | PASS, en disco y en modo temporal |
| AC-C2 | `check_tests::ac_c2_a_changed_node_is_checked_again_once_in_the_store` | PASS, en disco y en modo temporal |
| AC-C3 | `check_tests::ac_c3_a_reused_page_is_checked_for_its_new_content` | PASS, en disco y en modo temporal |
| AC-C4 | `check_tests::ac_c4_a_damaged_layout_in_the_store_fails_every_time` | PASS, en disco y en modo temporal |
| AC-C5 | `check_tests::ac_c5_a_damaged_layout_in_the_scratch_area_fails_every_time` | PASS, en disco y en modo temporal |
| AC-C6 | `check_tests::ac_c6_an_inconsistent_entry_fails_although_its_layout_is_remembered` | PASS, en disco y en modo temporal |
| AC-C7 | `check_tests::ac_c7_a_byte_changed_in_a_remembered_node_is_detected` | PASS, en disco y en modo temporal; también con un byte de una clave, que deja la organización válida |
| AC-C8 | `check_tests::ac_c8_concurrent_reads_with_competing_slots` | PASS, 4 casillas comprobadas; 4 hilos lectores de al menos 1.000 rondas completas cada uno, que leen mientras se confirman y se trasladan 20 generaciones y ven las 21; 18,5 s en debug |
| AC-C9 | `check_tests::ac_c9_reopening_forgets_the_remembered_nodes` | PASS, en disco |
| AC-C10 | `cargo run --release -p krummi --example node_check_timing` en Windows y Linux | Ejecutada; resultados en la medición |
| C5 y P13 (Codex R1) | `check_tests::opening_without_memory_for_the_table_fails_with_out_of_memory` | PASS, en disco y en modo temporal |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `09a9da4..64912a9` (T1, elevado por el ítem 2) | risk, resilience, reliability | aprobada | R1-001 (sugerencia, confianza en el código de 64 bits ante un escritor externo): ya es el riesgo residual de C7. R4-fixed-table-per-opening y R3-eager-table-allocation (sugerencias, 512 KiB por almacén al abrir): tamaño decidido con la medición. R3-node-at-cache-untested (aviso): las pruebas de T2. R3-with-checked-slots-not-panic-safe (sugerencia): corregida en T2 con un guardián | `review-3073e38929171edc` |
| `64912a9..4057ce7` (T2, T3) | risk, reliability, readability | aprobada | R2-001 (aviso, cifra de Windows que no salía de las tablas): corregida. R3-ac-c8-slot-count-unasserted (aviso): AC-C8 comprueba las 4 casillas. R2-003, R3-ac-c7-checksum-vs-layout, R1-flip-byte-unchecked-preconditions, R2-004, R2-002 (sugerencias): corregidas en `ceab08d`. R3-timing-order-bias (sugerencia): se mantiene, el más rápido de tres rondas lo limita y cambiar el orden no coincidiría con las corridas registradas | `review-9992850ecbe7818c` |
| `4057ce7..ceab08d` (correcciones) | reliability, risk | aprobada | R3-ac-c7-key-flip-offset-and-path-unproved (aviso): descartado con evidencia (el desplazamiento es de la carga útil y el byte cambiado no lo lee la comprobación de organización) | `review-b82822f29a0a84ec` |
| `fc57e47..6b095ff` (Codex R1 y R2) | resilience, reliability | aprobada | R3-ac-c8-hangs-on-failure y R4-reader-loop-hangs-on-writer-failure (avisos): corregidos en `8cf01cf` con Codex R3. R3-oom-test-only-overflow y R4-scratch-assert-while-readers-pin (sugerencias): sin cambios, la reserva usa el mismo ayudante falible que ya cubren las pruebas de fallos inyectados, y al trasladar todas las lecturas abiertas ven la versión más nueva | `review-e32dc28366105d7c` |
| `6b095ff..2fa32aa` (Codex R3) | reliability, resilience | aprobada | R3-001 (sugerencia, informativa): sin cambios | `review-c534a2c3ec2a4bc1` |

Evaluaciones sin revisión: el tramo `308e3a0..09a9da4` (el PRD y el ajuste de C5) no se evaluó aparte, porque la
evaluación mide hasta HEAD; lo cubrió la revisión de T1, que empezó en `09a9da4`, y la revisión de Codex del PRD.
Los commits `c7408c4` y `e529474` solo registran revisiones.

## Revisión cruzada de Codex

| Ronda | Rango | Estado | Hallazgos | Informe |
|---|---|---|---|---|
| R1 | `f49919f..fc57e47` | `findings` | R1 (mayor, la tabla aborta el proceso sin memoria): corregido en `161c0e4`. R2 (mayor, AC-C8 sin solapamiento): corregido en `f95c57f` | [2026-10-30-code.md](../../docs/reviews/trees-node-check-cache/2026-10-30-code.md) |
| R2 | `fc57e47..6b095ff` | `findings` | R1 y R2 verificados. R3 (menor, AC-C8 se cuelga si falla un hilo): corregido en `8cf01cf` | [2026-10-30-code-02.md](../../docs/reviews/trees-node-check-cache/2026-10-30-code-02.md) |
| R3 | `6b095ff..2fa32aa` | `approved` | R3 verificado; ninguno abierto | [2026-10-30-code-03.md](../../docs/reviews/trees-node-check-cache/2026-10-30-code-03.md) |

Resoluciones: [2026-10-30-code-resolutions.md](../../docs/reviews/trees-node-check-cache/2026-10-30-code-resolutions.md).
Antes, a las 09:58, un primer intento falló por la cuota de uso de Codex (L10).

## Tiempo

- Estimación: entre 2 h 30 min y 4 h. Arranque real: 2026-10-30 09:07:30 +00:00 (`date`). Fin de la verificación:
  13:33:46, tras la tercera ronda de Codex.
- **Duración activa desde el arranque: 1 h 29 min**, el 59 % del mínimo estimado: 52 min 13 s hasta el primer
  informe y el resto en el bucle de Codex. El diseño quedó fijado en el estudio previo al PRD, sin escritores
  delegados, y las mediciones y revisiones corrieron en segundo plano.
- Esperas por el usuario: 09:59:46–10:04:15 (4 min 29 s), la decisión sobre el informe con Codex pendiente. La pausa
  de 10:04 a 12:56 (cuota de Codex) no cuenta como activa.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Exploración y mapa | 2 min 09 s | 2 % | 1 |
| Código y tests | 15 min 12 s | 17 % | 3 |
| Checks (fmt, clippy, suite de tests) | 46 min 59 s | 52 % | 4 |
| Aceptación | 6 min 55 s | 8 % | 1 |
| Revisión RDD | 6 min 55 s | 8 % | 4 |
| Correcciones de hallazgos | 11 min 30 s | 13 % | 2 |
| Cierre (informe y merge) | 0 min 12 s | 0 % | 2 |
| **Total activo** | **1 h 29 min** | **100 %** | **17** |

- Inicio: 2026-10-30 09:07:33 +00:00. Fin: 2026-10-30 13:33:46 +00:00.
- Arranque confirmado: 09:07:33. Activo desde el arranque: **1 h 29 min** (compárese con la estimación); antes del arranque: 0 min 00 s.
- Esperas por el usuario (fuera del activo): 4 min 29 s.
  - 09:59:46–10:04:15 (4 min 29 s): Codex usage limit
- Pausas entre sesiones (fuera del activo): 2 h 51 min.
  - 2026-10-30 10:04:15 +00:00 – 2026-10-30 12:56:07 +00:00 (2 h 51 min)

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Aceptación | C9 Windows+Linux | 09:34:46 | 09:38:00 | 3 min 14 s |
| Aceptación | C9 Windows rerun | 09:38:17 | 09:40:44 | 2 min 27 s |
| Revisión RDD | RDD last slice | 09:48:02 | 09:49:19 | 1 min 17 s |
| Revisión RDD | codex-R1 | 09:58:26 | 09:59:30 | 1 min 04 s |
| Revisión RDD | codex-R1 | 12:47:11 | 12:55:51 | 8 min 40 s |
| Revisión RDD | codex-R2 | 13:13:42 | 13:23:48 | 10 min 06 s |
| Revisión RDD | codex-R3 | 13:29:13 | 13:33:22 | 4 min 09 s |
| **Aceptación (total)** | | | | **5 min 41 s** |
| **Revisión RDD (total)** | | | | **25 min 16 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `09a9da4` | docs: state in C5 that a slot match of another page needs the 64-bit coincidence of C7 | map read path, T1 | 12 min 37 s | Exploración y mapa 2 min 09 s, Código y tests 10 min 12 s, Checks (fmt, clippy, suite de tests) 0 min 16 s |
| `6a1396b` | feat(tree): remember the node layouts already checked while the store is open |  | 0 min 00 s |  |
| `64912a9` | docs: add the phase log of the remembered node layout check run | T1, RDD T1 | 1 min 05 s | Checks (fmt, clippy, suite de tests) 0 min 36 s, Revisión RDD 0 min 29 s |
| `1ea3d98` | test: check the remembered node layouts through the public API | RDD T1, T2 | 11 min 56 s | Código y tests 3 min 50 s, Checks (fmt, clippy, suite de tests) 6 min 09 s, Revisión RDD 1 min 57 s |
| `4057ce7` | feat(examples): measure node reads without and with the remembered layout check | T2, T3, C9 measurement | 14 min 21 s | Código y tests 1 min 10 s, Checks (fmt, clippy, suite de tests) 6 min 16 s, Aceptación 6 min 55 s |
| `ceab08d` | test: tighten the remembered layout tests and the measurement docs after review | T3, RDD last slice | 9 min 32 s | Checks (fmt, clippy, suite de tests) 0 min 11 s, Revisión RDD 2 min 02 s, Correcciones de hallazgos 7 min 19 s |
| `c7408c4` | docs: record the review of the remembered layout test fixes | RDD last slice, RDD review fixes | 1 min 13 s | Revisión RDD 0 min 58 s, Correcciones de hallazgos 0 min 15 s |
| `e529474` | docs: record the failed Codex review attempt of the remembered layout check | RDD review fixes, Codex R1 | 1 min 29 s | Revisión RDD 1 min 29 s |
| `fc57e47` | docs: add the remembered node layout check closing report |  | 0 min 00 s |  |
| `161c0e4` | fix(pager): fail the opening with out of memory when the checked layout table cannot be allocated | codex-R1 | 16 min 43 s | Checks (fmt, clippy, suite de tests) 12 min 47 s, Correcciones de hallazgos 3 min 56 s |
| `f95c57f` | test: make the AC-C8 readers overlap every commit and transfer of the writer |  | 0 min 00 s |  |
| `6b095ff` | docs: record the first Codex code review of the remembered layout check and its fixes | codex-R1 | 0 min 34 s | Checks (fmt, clippy, suite de tests) 0 min 34 s |
| `8cf01cf` | test: stop the AC-C8 readers when a reader or the writer fails | codex-R1 | 15 min 38 s | Checks (fmt, clippy, suite de tests) 15 min 38 s |
| `2fa32aa` | docs: record the Codex follow-up and the RDD review of the remembered layout fixes | codex-R1 | 0 min 01 s | Checks (fmt, clippy, suite de tests) 0 min 01 s |
| — | después del último commit | codex-R1, codex-close | 4 min 43 s | Checks (fmt, clippy, suite de tests) 4 min 31 s, Cierre (informe y merge) 0 min 12 s |
| **Total** | | | **1 h 29 min** | |

Las revisiones de Codex aparecen como «Revisión RDD» porque comparten la fase `review`. Durante el bucle de Codex
solo se marcaron las fases a las 12:56 (correcciones) y a las 13:00 (checks), así que el tratamiento de R3, las dos
revisiones RDD de las correcciones y el registro de las rondas cuentan como checks: por eso la fase suma 46 min 59 s.
La revisión RDD de T1 corrió en segundo plano sin quedar en la tabla, porque sus marcas fallaron por un error en los
argumentos (L5). Los commits hechos seguidos de otro (`6a1396b`, `fc57e47`, `f95c57f`) muestran 0 min.

## Informe de cambios (`f49919f..2fa32aa`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 824 | 0 | 690 | 83 | 3 |
| `odd` | 439 | 0 | — | — | 4 |
| `docs` | 357 | 2 | — | — | 5 |
| `crates/krummi-pager` | 251 | 8 | 159 | 66 | 5 |
| `crates/krummi-tx` | 80 | 2 | 46 | 25 | 2 |
| `crates/krummi-tree` | 60 | 2 | 41 | 10 | 1 |
| `README.md` | 3 | 1 | — | — | 1 |
| **Total** | **2.014** | **15** | **936** | **184** | **21** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Sin lockfiles ni archivos generados. La previsión era de unas 900 líneas (L2); el código del motor sumó unas 390 líneas añadidas y
las pruebas y el ejemplo de medición se llevaron el resto.

## Resumen

Las lecturas de los árboles dejan de repetir la comprobación de organización de un nodo cuyo contenido ya pasó esa
comprobación mientras el almacén está abierto. Cada almacén abierto tiene una tabla fija de 65.536 casillas (512 KiB),
una palabra atómica por casilla elegida por el número de página, que guarda el código de integridad verificado. La
lectura de un nodo del archivo de datos verifica ese código como siempre, y solo si coincide con el recordado se salta
`is_valid`; si no, lo comprueba y lo recuerda cuando lo pasa. El Pager devuelve el código junto a la página; las
versiones del área temporal y las páginas de la escritura no traen código y se comprueban siempre. La excepción a G6
que esto supone (C7) quedó anotada en el PRD raíz.

## Valores por defecto elegidos sin preguntar

- La redacción de C5 se ajustó al diseño acordado (una coincidencia de 64 bits con otra página de la misma casilla es
  el riesgo de C7), sin parar la ejecución (L3).
- Entrega con commits locales por tarea en la rama, T1 en un commit propio por su riesgo; partir en pull requests
  queda para el usuario (L2).
- Todos los nodos usan la memoria, sin mínimo de entradas, y la tabla tiene 65.536 casillas por almacén abierto (L7).
- El registro de fases se incluyó en un commit a mitad de la ejecución (`64912a9`) para que no bloqueara la evaluación
  de RDD, que no admite archivos sin seguimiento (L4).
- Para AC-C7, un gancho de prueba del motor cambia un byte en el sitio: toma el motor en exclusiva (ninguna
  transacción abierta) y el cerrojo del traslado, y es el único código no seguro nuevo, dentro del Pager y solo con
  `test-hooks` (L6).

## Pendiente (no bloqueante)

- **Revisión cruzada de Codex del código:** aprobada en la tercera ronda sin hallazgos abiertos (sección *Revisión
  cruzada de Codex*). Esta actualización del informe no pasó por Codex.
- **Observación de Windows:** lo que ahorra no comprobar una hoja de 370 entradas en Windows (12,5 y 14,0 µs por Get)
  supera el coste aislado de la comprobación medido antes del PRD (2,5–2,9 µs); en Linux coincide. No está explicado;
  el efecto relativo es el mismo en los dos sistemas.
- **Orden de las rondas en la medición** (R3-timing-order-bias): la ronda sin memoria va siempre primero; el más
  rápido de tres rondas lo limita.
- **Tiempo de elaboración del PRD:** no se registró, porque `prd_time.py` mide toda la sesión y no tiene forma de
  empezar en otro punto.

**Riesgo:** ítem 2 (el cambio relaja la detección de daño: la comprobación de organización de un contenido recordado
se salta, con la excepción a G6 de C7) e ítem 4 (estado compartido entre hilos: la tabla atómica por almacén). T1 se
revisó aparte con tres lentes, y AC-C4, AC-C7 y AC-C8 lo fijan.

## Limpieza manual

- `work/nct/` (resultados en bruto de la medición, ignorado por git) y `~/krummi-nct-stores` en WSL (vacía; los
  almacenes se borran al terminar cada caso).
