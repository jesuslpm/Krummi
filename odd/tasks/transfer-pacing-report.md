# Ritmo del traslado: terminado, verificado y fusionado en `main`

Informe de cierre de `transfer-pacing` (2026-10-23). Enfoque de ejecución: ODD.
Fuente de producto: ninguna nueva (refactorización y rendimiento del traslado de J5, sin cambio de producto).
Documento de ejecución: [transfer-pacing.md](transfer-pacing.md). Antecedente:
[transfer-background-thread-report.md](transfer-background-thread-report.md).

Rama `refactor/transfer-pacing`: cuatro commits sobre `54a9141` y el de este informe, fusionada en `main` por
avance rápido. No se subió nada.

## Resultado

- **Especificación:** S1–S5 cumplidos.
  - **S1:** la pasada urgente se dispara cuando el área temporal crece 32 MiB desde la pasada anterior, no cuando ya
    está por encima del umbral.
  - **S2:** las páginas escritas se retiran del área temporal con un solo lock por pasada.
  - **S3:** un único plazo sustituye a `requested`, `urgent` y `retry_at`.
  - **S4:** se quitó el intervalo de 2 s entre fsync de datos.
  - **S5:** sin pérdida de rendimiento medible.
  - Suite en Windows: 197 pruebas pasan con escritura directa del diario y 197 con `KRUMMI_JOURNAL_BUFFERED=1`; fmt
    y clippy limpios.
- **Medición (`commit_rate`, release, escritura directa, 3 s por número de hilos, commits/s con 1, 4 y 16 hilos):**

  | Ejecución | 1 hilo | 4 hilos | 16 hilos |
  |---|---|---|---|
  | Base `e3ea257` (media de 3) | 14.189 | 29.604 | 12.699 |
  | Tras T1 (media de 3) | 15.007 | 30.968 | 13.078 |
  | T2, archivos de 16 MiB, con / sin el intervalo (3 rondas) | 14.960 / 14.309 | 30.435 / 30.273 | 12.959 / 13.713 |
  | T2, archivos de 1 MiB, con / sin el intervalo (6 rondas) | 12.515 / 12.661 | 22.078 / 21.152 | 12.317 / 11.970 |

  - Las comparaciones de T2 usaron una compilación temporal del ejemplo, que no se confirmó: tamaño de archivo por
    `KRUMMI_FILE_LIMIT` y una columna de fsync de datos.
  - Las muestras de cada pareja se solapan (dispersión de ±10 %).
  - Los fsync de datos en 3 s pasaron de 2 a entre 9 y 20 sin el intervalo.
- **Revisiones y verificación:**
  - Verificador independiente (Sonnet, alto): sin bloqueos.
    - Con una lectura que retiene unos 49 MiB, 200 confirmaciones hicieron 1 pasada en la rama y 200 en la base.
    - Un fallo en la escritura 3 de 8 deja 6 páginas en el área temporal, igual que en la base.
    - Escritores y lectores concurrentes, con cierre o drop y reapertura, conservaron todos los valores.
  - Dos revisiones RDD, aprobadas (sección siguiente).
- **Hallazgos corregidos por el camino:** a la prueba de S1 le faltaba la mitad positiva (R3-growth-test-one-sided).
  Ahora comprueba que volver a crecer el umbral dispara una pasada en menos de 2 s con un intervalo de 5 s. Sin la
  regla de crecimiento falla: la pasada llegó a los 4,97 s.

## Cobertura

| Spec | Prueba o comprobación | Resultado |
|---|---|---|
| S1 | `a_pass_starts_at_once_only_when_the_scratch_area_grew_by_the_threshold` | PASS (RED: 50 pasadas para 50 confirmaciones; sin la regla, la pasada llega a los 4,97 s) |
| S2 | Suite completa; sonda del verificador con fallo a mitad de pasada | PASS |
| S3 | Pruebas de `transfer.rs` (agrupación, reintento con ritmo, carrera) | PASS: 11 pruebas en 10 ejecuciones del verificador |
| S4 | Medición L4; `data_flushes_do_not_grow_with_commits_while_no_file_can_be_released` | PASS |
| S5 | `commit_rate` antes y después | Sin pérdida fuera del ruido (tabla de arriba) |

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Linaje |
|---|---|---|---|---|
| `54a9141..fdcf161` (`0c6af85`, `e6a9fb5`, `fdcf161`) | resilience, reliability | aprobada | R4-stale-kept-during-pass (SUGGESTION): una confirmación durante una primera pasada lenta puede costar una pasada extra; acotado, se mantiene. R3-growth-test-one-sided (SUGGESTION): corregido en `093b4cd`. R3-set-pacing-deadline (SUGGESTION): acortar el intervalo con el hook no adelanta un plazo pendiente; solo afecta a las pruebas, se mantiene | `review-466d1175ad27d417` |
| `fdcf161..093b4cd` | reliability | aprobada | R3-growth-positive-attribution (WARNING): la mitad positiva podría pasar sin la regla si un plazo pendiente cayera en su ventana; exige que la primera mitad tarde más de 3 s, queda pendiente. R3-growth-wallclock-bound (SUGGESTION): la ventana de 2 s incluye 150 reservas y un commit; queda pendiente | `review-74f106f7a3a80f30` |

## Tiempo

- Estimación: 45 min – 1 h 30 min. Arranque real: 2026-10-23 19:55:31 +0100. Fin de la verificación: 20:31:49
  +0100.
- **Duración activa desde el arranque: 36 min 18 s**, el 81 % del mínimo estimado.
  - La verificación independiente (13 min) y los checks (9 min) se llevaron la mayor parte.
  - Las mediciones ocuparon unos 8 min. Ese tiempo incluye además escribir el código de T2: marqué la fase de
    aceptación antes de empezarlo, así que el registro no los separa.
- Esperas por el usuario: ninguna dentro de la ejecución. Las preguntas y la estimación fueron antes del arranque.

### Tiempo por fase

| Fase | Duración | % del activo | Tramos |
|---|---|---|---|
| Plan (documento de ejecución) | 0 min 45 s | 2 % | 1 |
| Código y tests | 1 min 40 s | 5 % | 1 |
| Checks (fmt, clippy, suite de tests) | 9 min 12 s | 25 % | 3 |
| Aceptación | 8 min 40 s | 24 % | 3 |
| Verificación independiente | 13 min 18 s | 37 % | 1 |
| Revisión RDD | 2 min 10 s | 6 % | 2 |
| Correcciones de hallazgos | 0 min 33 s | 2 % | 1 |
| **Total activo** | **36 min 18 s** | **100 %** | **12** |

- Inicio: 2026-10-23 19:55:31 +01:00. Fin: 2026-10-23 20:31:49 +01:00.
- Arranque confirmado: 19:55:31. Activo desde el arranque: **36 min 18 s**; antes del arranque: 0 min 00 s.
- Esperas por el usuario: ninguna.
- Fase abierta, no contada: Cierre (informe y merge) desde 20:31:49.

En segundo plano (se solapa con las fases de arriba; no suma al activo):

| Fase | Tarea | Inicio | Fin | Duración |
|---|---|---|---|---|
| Revisión RDD | RDD two lenses | 20:27:28 | 20:28:14 | 0 min 46 s |
| **Revisión RDD (total)** | | | | **0 min 46 s** |

Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):

| Commit | Mensaje | Etiquetas | Activo | Desglose |
|---|---|---|---|---|
| `0c6af85` | perf: pace transfer passes with one deadline | baseline, T1 | 7 min 19 s | Plan 0 min 45 s, Código y tests 1 min 40 s, Checks 3 min 17 s, Aceptación 1 min 37 s |
| `e6a9fb5` | perf: flush krummi.data whenever it releases a journal file | T1, T2 | 10 min 40 s | Checks 3 min 37 s, Aceptación 7 min 03 s |
| `fdcf161` | docs: record the transfer-pacing verification and phase log | T2, T1-T2 | 13 min 35 s | Checks 0 min 17 s, Verificación independiente 13 min 18 s |
| `093b4cd` | test: check that growth by the threshold starts a transfer pass at once | T1-T2, R3-growth | 3 min 48 s | Checks 1 min 51 s, Revisión RDD 1 min 24 s, Correcciones 0 min 33 s |
| — | después del último commit | R3-growth | 0 min 56 s | Checks 0 min 10 s, Revisión RDD 0 min 46 s |
| **Total** | | | **36 min 18 s** | |

## Informe de cambios (`54a9141..093b4cd`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi-pager` | 135 | 168 | 100 | 31 | 2 |
| `crates/krummi-tx` | 53 | 25 | 43 | 9 | 3 |
| `odd` | 77 | 0 | — | — | 2 |
| `README.md` | 3 | 3 | — | — | 1 |
| **Total** | **268** | **196** | **143** | **40** | **8** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell,
JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

El código de `krummi-pager` queda con 33 líneas menos. Desaparecen `Flushing`, `retry_at`, el intervalo de datos y
dos indicadores del estado de la señal.

## Resumen

El hilo de traslado decide cuándo hacer la siguiente pasada con un único plazo:

- Una señal lo adelanta hasta el fin del intervalo de 200 ms desde la pasada anterior, o hasta ya mismo si el área
  temporal creció 32 MiB desde entonces.
- Un fallo que no hace rechazar escrituras lo fija para reintentar al acabar el intervalo.

Con una lectura larga que retiene mucho, las confirmaciones ya no disparan una pasada cada una. La pasada retira
sus páginas con un solo lock. `krummi.data` se lleva al disco en cuanto eso libera un archivo del diario: medido, el
intervalo de 2 s no aportaba rendimiento.

## Decisiones tomadas sin preguntar

- Para medir T2 se usó una compilación temporal de `commit_rate`, con tamaño de archivo configurable y una columna de
  fsync; no se confirmó, para no cambiar la salida de la herramienta.
- "Peor más allá del ruido" se juzgó por el solapamiento de las muestras de cada pareja; con 1 MiB se hicieron 6
  rondas para decidir.
- La prueba de reintento con ritmo pasó a llamarse `a_failing_pass_is_retried_once_per_pass_interval`, porque ya no
  existe la liberación pospuesta que usaba.

## Pendiente (no bloquea)

- **R3-growth-positive-attribution y R3-growth-wallclock-bound:** la mitad positiva de la prueba de S1 usa una
  ventana de reloj de 2 s. En la práctica solo daría un falso positivo si la primera mitad tardara más de 3 s, y
  podría fallar en un equipo muy lento.
- **R4-stale-kept-during-pass:** como mucho una pasada extra tras una primera pasada lenta con una lectura larga.
- **R3-set-pacing-deadline / aviso del verificador:** acortar el intervalo con el hook de pruebas no adelanta un plazo
  ya pendiente.
- Sin ejecución en Linux ni medición con `KRUMMI_JOURNAL_BUFFERED=1`.

**Riesgo:** item 4 (las reglas de despertar del hilo de traslado). Lo cubren el verificador independiente y dos
revisiones RDD.
