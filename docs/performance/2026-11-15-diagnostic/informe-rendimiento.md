**Krummi — diagnóstico de rendimiento y optimizaciones candidatas**

*Copia del 2026-11-15 de `work/diag-20261115/informe-rendimiento.md`, sin cambios salvo esta nota y los enlaces que salen de su carpeta, ajustados a su nueva ubicación. Se copiaron los archivos de evidencia que enlaza (resultados, registros, scripts y fuentes de los prototipos); los conjuntos de datos, las copias de trabajo y los ejecutables (unos 75 GB) y los registros de `work/w11/` siguen solo en `work/`, que git ignora.*

Fecha de las mediciones: 2026-11-15, zona Atlantic/Canary (UTC+00:00).

Revisión de partida: `main`, `28f9d3bcdb5377249bd0ce7fc75fde08daae3646`.

Este documento conserva el diagnóstico entregado en el chat e incorpora la corrección posterior sobre Microsoft Defender. No es una revisión de un diff ni una propuesta de cambios ya lista para integrar.

**Resultado principal**

Se comprobaron tres mejoras mediante variantes en copias experimentales:

- Proyección de columnas para **tabla-intervalos**: **12,4 % menos tiempo**.
- Reutilización de buffers privados para **valores-grandes-carga**: **11,4 % menos tiempo**.
- Checksum corto para **intervalos**: **7,0 % menos tiempo**, correspondiente al banco, no al motor.

En **tabla-lecturas** no se demostró una desventaja estable. En **tabla-actualizacion** y **multivalor-carga**, el commit de Krummi resultó más rápido que el de Voron: la diferencia se concentra antes de confirmar.

No se modificaron archivos versionados, no se hicieron commits ni se cambió el estado de Git. Al cerrar el diagnóstico, HEAD, el diff y los hashes de los 663 archivos versionados coincidían con el estado inicial. Se restauró el plan de energía original. Véase [repository-verification.json](repository-verification.json).

**Alcance y referencias**

Se inspeccionaron los caminos de las siete cargas en `krummi`, `krummi-tree`, `krummi-tx`, `krummi-pager` y `krummi-bench`, y sus equivalentes en las fuentes locales de Voron 7.2.7.

Referencias de partida:

- [Banco completo de Windows del 2026-11-13](../../benchmark/2026-11-13-windows-range-step.md).
- [Historial de optimizaciones y método de medida](../../performance/plan.md).
- [Cierre de range-step](../../../odd/done/trees/range-step-report.md).

No se repitieron las propuestas de comprobar límites por hoja, combinar la lectura de clave y valor en `node::entry`, ni cambiar el prefetch de hojas vecinas. Esas optimizaciones ya estaban presentes.

Todos los scripts, copias, binarios y conjuntos de datos generados se mantuvieron en `work/diag-20261115/`. `src/` es una copia experimental que pasó por distintas variantes; no debe confundirse con un checkout limpio de la revisión de partida. Los ejecutables utilizados y sus hashes se conservan en los registros de cada serie.

**Máquina y método**

Todas las cifras de este informe corresponden a la misma máquina y condiciones de ejecución:

| Propiedad | Valor |
|---|---|
| Máquina | MSI-W11-01 |
| Procesador | Intel Core i9-13980HX |
| Sistema | Windows 11, compilación 26200 |
| Afinidad de los procesos medidos | `0xffff`: hilos lógicos 0–15, correspondientes a P-cores |
| Energía durante las mediciones | Plan Krummi bench, boost desactivado |
| Rust | 1.99.0 |
| Voron | Fuentes locales de RavenDB 7.2.7 |
| .NET | Runtime 10.0.5; SDK de compilación 10.0.201 |
| Escala | `full`, ejecutando solamente las cargas seleccionadas |
| Repeticiones por proceso | Un calentamiento y tres repeticiones medidas |
| Comparaciones A/B | Dos rondas, invirtiendo el orden; cuatro rondas para el pool de runs |

Las ejecuciones medidas fueron seriales y no se solaparon con compilaciones. En escala completa, las cargas de tablas y multivalor consideradas realizan un millón de operaciones; los intervalos recorren diez millones de entradas, en 100.000 recorridos de 100 entradas. W17 carga 4.096 valores de 64 KiB, en lotes de 256.

Los valores iniciales son medianas de seis repeticiones medidas. Los A/B usan sus propios controles: no se compara una variante de una serie posterior con el control de otra serie para calcular su ganancia.

Los tiempos internos son aproximaciones de perfil, no componentes que deban sumarse: algunos son inclusivos y se contienen entre sí. En Krummi se muestrearon funciones pequeñas a razón de 1/64; la sonda vacía costó unos 53 ns por muestra. Los componentes de Voron correspondientes a `ReadByKey`, preparación de filas, `Update` e `Insert` citados abajo proceden de la instrumentación de todas las operaciones en `voron-parts-all`. Se descartó para `Insert` la extrapolación del muestreo periódico, porque daba un tiempo incompatible con el total medido. Las ganancias de las variantes se comprobaron aparte, sin esas sondas.

**Corrección sobre Defender**

La salida de `Get-MpPreference` aportada por el usuario incluye `C:\projects\Krummi\work`, que cubre el directorio de los experimentos.

La consulta del agente devolvía las exclusiones ocultas: la comprobación posterior mostró `HideExclusionsFromLocalUsers = true` y un token sin permisos de administrador. Solicitar una ejecución fuera del sandbox no acreditaba elevación administrativa de Windows. Se confundió una enumeración no visible con una lista de exclusiones inexistente.

Por lo tanto, **queda retirada la afirmación de que faltaba la exclusión de `work/` y de que existía una diferencia de configuración de Defender respecto del banco histórico**. No se modificó Defender. Las mediciones permanecen; la diferencia de W17 respecto del histórico sigue sin una explicación ambiental demostrada.

Evidencia y corrección: [defender-observation.json](defender-observation.json). Comportamiento documentado por [Microsoft](https://learn.microsoft.com/windows/client-management/mdm/defender-csp#hideexclusionsfromlocalusers).

**Comparación inicial con Voron**

| Carga | Krummi, ns/op | Voron, ns/op | Krummi / Voron |
|---|---:|---:|---:|
| W28 — tabla-actualizacion | 8.895,7 | 7.815,3 | 0,879 |
| W27 — tabla-intervalos | 102,04 | 92,33 | 0,905 |
| W24 — tabla-insercion-al-azar | 10.337,3 | 9.482,9 | 0,917 |
| W17 — valores-grandes-carga | 196.101,7 | 161.674,1 | 0,824 |
| W19 — multivalor-carga | 8.551,1 | 8.268,3 | 0,967 |
| W26 — tabla-lecturas | 1.353,99 | 1.332,99 | 0,984 |
| W11 — intervalos | 60,14 | 59,51 | 0,989 |

En W27 y W11, la unidad es ns por entrada. Krummi / Voron es el cociente de rendimiento, equivalente a tiempo de Voron dividido por tiempo de Krummi: más alto favorece a Krummi.

Evidencia: [baseline-aa/results.json](baseline-aa/results.json) y [baseline-aa/machine.json](baseline-aa/machine.json).

**Diagnóstico por carga**

**W28 — Tabla-actualizacion: búsqueda y preparación de la fila**

La búsqueda localizada cuesta aproximadamente **3.006 ns**, frente a **2.085 ns** de `ReadByKey` en Voron. Krummi añade **623 ns** de comprobación/normalización y **209 ns** de preparación del parche. La sobrescritura cuesta menos: **1.388 frente a 1.707 ns**; también el commit: **3.695 frente a 3.983 ns**.

Referencias: `crates/krummi/src/table/rows.rs:331` — comprobación de entradas; `rows.rs:522` — parche; `rows.rs:1329` — escritura; `work/ravendb-7.2.7/src/Voron/Data/Tables/Table.cs:178` — `ReadByKey`; `Table.cs:359` — `Update`.

La diferencia está localizada en búsqueda y preparación, pero no se aisló mediante contadores de hardware la causa de cada nanosegundo adicional de la búsqueda. La actualización actual ya evita un segundo descenso cuando puede sobrescribir la celda localizada.

Descartado:

- Simplificar la validación del prefijo `id,n`: **9.104 → 9.071 ns**, sin mejora consistente al invertir el orden.
- Agrupar diferencias del journal: **8.899 → 8.997 ns**, sin mejora.
- Eliminar la comprobación del contenido de la fila existente: no se ensayó, porque L3 exige conservarla. Su coste aproximado era **115 ns/op**, incluido en la decodificación, no adicional a ella.

Evidencia: [probe](probe/results.json), [voron-parts-all](voron-parts-all/results.json), [prefix-ab](prefix-ab/results.json), [diff-ab](diff-ab/results.json).

**W27 — Tabla-intervalos: acceso a columnas y trabajo del banco**

Leer y consumir una fila ya en caché cuesta **58,4 ns en Krummi frente a 41,0 ns en Voron**. El acceso genérico de Krummi repite selección de representación, tipo y versión; Voron accede a los campos mediante offsets.

Una proyección experimental que conoce las seis columnas, conservando el checksum completo, reduce la carga de **102,82 a 90,07 ns/entrada: 12,4 % menos tiempo**. El prototipo es específico del esquema del banco; no demuestra por sí solo que una API general consiga toda esa ganancia.

Referencias: `crates/krummi/src/table/rows.rs:1011` — acceso tipado; `crates/krummi-bench/src/worker/tables.rs:239` — consumo de la fila; `work/ravendb-7.2.7/src/Voron/Data/Tables/TableValueReader.cs:61` — lectura por offsets.

El banco también influye: al reemplazar la suma de bytes por un control que consume solamente longitudes, Krummi pasa de **103,41 a 86,92 ns** y Voron de **93,45 a 84,38 ns**. Esa eliminación es una prueba de aislamiento, no una optimización válida, y sus diferencias no deben sumarse a las de la proyección.

Descartado como solución principal: optimizar solamente el checksum corto dio **101,98 → 101,46 ns**, dentro de la variación. No se modificó el paso de `Range` ya optimizado.

Evidencia: [micro-krummi.log](micro-krummi.log), [micro-voron.log](micro-voron.log), [projection-ab](projection-ab/results.json), [byte-count-ab](byte-count-ab/results.json), [short-sum-ab](short-sum-ab/results.json).

**W24 — Tabla-insercion-al-azar: preparación de filas y más páginas modificadas**

Krummi dedica aproximadamente **918 ns a comprobar/normalizar** y **388 ns a codificar**, antes de los **4.557 ns** de actualización del árbol. Voron prepara su builder en unos **86 ns**, pero la codificación efectiva está incluida en `Table.Insert`, que cuesta **5.259 ns**: comparar únicamente los builders sería incorrecto.

Krummi modifica **0,948 páginas/op**, frente a **0,835** de Voron. El commit cuesta **4.551 frente a 4.206 ns**. Esto separa el coste adicional anterior al árbol del trabajo adicional por páginas; no demuestra que un cambio concreto de distribución de páginas vaya a mejorar el resultado.

Referencias: `crates/krummi/src/table/rows.rs:331`; `crates/krummi/src/table/row.rs:265`; `work/ravendb-7.2.7/src/Voron/Data/Tables/Table.cs:718`.

Descartado:

- Exceso de bytes de journal como explicación principal: Krummi escribe **357 B/op frente a 454 B/op**. Su espera explícita de durabilidad es aproximadamente **249 ns/op**.
- Preasignar el vector de validación: **10.499 → 10.414 ns**, sin consistencia entre rondas.
- Agrupar diferencias: **10.472 → 10.729 ns**, empeoramiento en ambas rondas.

Evidencia: [probe](probe/results.json), [voron-parts-all](voron-parts-all/results.json), [prefix-insert-ab](prefix-insert-ab/results.json), [diff-ab](diff-ab/results.json).

**W17 — Valores-grandes-carga: asignación/reutilización de memoria y copias**

El perfil sitúa unos **46.802 ns** en reservar el run —incluye asignación y crecimiento, no es una medida aislada del asignador— y **32.407 ns** en almacenarlo en scratch.

La prueba causal fue reutilizar buffers privados conservando su puesta a cero: **208.266 → 184.528 ns/op, 11,4 % menos tiempo**, con mejora en las cuatro rondas. El ejecutable original leyó correctamente después los **4.096 valores de 64 KiB** producidos por la variante.

Referencias: `crates/krummi-pager/src/scratch.rs:104` — asignación de runs; `crates/krummi-tx/src/transaction.rs:677` — reserva; `crates/krummi-pager/src/temp.rs:187` — almacenamiento y copia; `crates/krummi-pager/src/journal.rs:356` — compresión.

Descartado:

- Mayor volumen persistido: **65.787 B/op en Krummi frente a 65.967 B/op en Voron**.
- Agrupar diferencias: **197.654 → 201.945 ns**, sin mejora.

LZ4 consume unos **15.288 ns/op sin reducir el payload** en estos datos: el tamaño bruto y el almacenado coinciden. Es un coste residual medido, no una optimización validada. No se ensayó una política que evite intentos infructuosos de compresión.

Evidencia: [probe](probe/results.json), [run-pool-ab](run-pool-ab/results.json), [verify-pool-W18.json](verify-pool-W18.json), [diff-ab](diff-ab/results.json).

**W19 — Multivalor-carga: localizar, reconstruir y volver a recorrer**

Localizar y decodificar la entrada cuesta **2.274 ns/op**. Codificar el blob inline cuesta **244 ns**; almacenarlo vuelve a entrar por `set` y cuesta **2.122 ns**, incluyendo la modificación. Voron conserva la página localizada en el camino común de `MultiAdd`.

Referencias: `crates/krummi-tree/src/multi.rs:258` — entrada; `multi.rs:334` — codificación inline; `multi.rs:368` — almacenamiento; `multi.rs:472` — adición; `work/ravendb-7.2.7/src/Voron/Data/BTrees/Tree.MultiTree.cs:220` — `MultiAdd`.

La duplicación del recorrido está comprobada; cuánto ahorraría eliminarla sigue siendo una estimación. No se puede descontar todo el tiempo del primer recorrido: localizar, modificar y copiar tienen trabajos distintos y diferente estado de caché.

Descartado como causa principal: el commit cuesta **4.135 ns en Krummi frente a 4.566 en Voron**, aunque Krummi escribe más journal. La variante de diferencias tampoco ayudó: **8.665 → 8.741 ns**. Los diez valores de esta carga permanecen inline; no se está pagando una promoción a subárbol.

Evidencia: [probe](probe/results.json), [diff-ab](diff-ab/results.json).

**W26 — Tabla-lecturas: diferencia demasiado pequeña para atribuirla con firmeza**

El coste adicional de los accesores existe, pero queda diluido en la búsqueda aleatoria. La proyección especializada pasó de **1.355,5 a 1.345,8 ns**, apenas **0,7 %**. En la comparación inicial, las repeticiones abarcaron **1.324–1.382 ns en Krummi** y **1.293–1.432 ns en Voron**.

Referencias: `crates/krummi/src/table/rows.rs:1208` — lectura por clave; `rows.rs:1011` — acceso a columnas; `work/ravendb-7.2.7/src/Voron/Data/Tables/Table.cs:178` — lectura por clave.

Descartado: presentar la proyección como una solución importante para W26 o afirmar que el pequeño déficit observado tiene una causa de motor ya demostrada. La mejora aislada de accesores no establece una desventaja estable de la carga completa.

Evidencia: [baseline-aa](baseline-aa/results.json), [projection-ab](projection-ab/results.json), [micro-krummi.log](micro-krummi.log), [micro-voron.log](micro-voron.log).

**W11 — Intervalos: oportunidad real en el checksum del banco**

La desventaja inicial cambia de signo entre rondas. Al sustituir el checksum por un control que solamente consume longitudes, Krummi pasa de **61,67 a 42,23 ns/entrada**; Voron, de **60,51 a 52,13**. No es una optimización válida, pero aísla trabajo del banco.

La variante válida conserva la suma y especializa entradas de hasta 16 bytes: **60,87 → 56,59 ns, 7,0 % menos tiempo**. Pasó **16.392 casos de equivalencia**, además de las comprobaciones de las cargas completas.

Referencias: `crates/krummi-bench/src/data.rs:97`; `crates/krummi-bench/voron/Data.cs:101`.

Descartado: sustituir generalmente la suma por una reducción `u32`; para 100 bytes empeoró de **14,18 a 17,57 ns**. La mejora demostrada se restringe al camino corto y no justifica tocar nuevamente `Range`.

Evidencia: [baseline-aa](baseline-aa/results.json), [byte-count-ab](byte-count-ab/results.json), [short-sum-ab](short-sum-ab/results.json), [short-sum-check.log](short-sum-check.log), [micro-krummi.log](micro-krummi.log).

**Costes comunes y específicos**

| Ámbito | Cargas | Resultado |
|---|---|---|
| Preparación, validación y codificación de filas | W24, W28 | Coste común medido; la simplificación de nombres y preasignación ensayada no dio una mejora consistente. |
| Acceso tipado y proyección de columnas | W26, W27 | Impacto importante en recorridos; pequeño en búsquedas aleatorias. |
| Consumo de bytes y checksum del banco | W11, W26, W27 | El camino corto mejora W11; no se confirmó una ganancia importante en las tablas. |
| Commit, scratch y journal | W24, W28, W17, W19 | No explican de forma común todos los déficits: Krummi ya confirma más rápido en W28 y W19. |
| Runs de valores grandes | W17 | Reutilización de buffers con ganancia demostrada; quedan copias y compresión infructuosa. |
| Doble recorrido y reconstrucción inline | W19 | Trabajo adicional identificado; ahorro de eliminarlo todavía estimado. |

**Optimizaciones candidatas, ordenadas por ganancia esperada**

Los porcentajes indican reducción de tiempo, no aumento de throughput. Las ganancias no deben sumarse. Ninguna candidata requiere cambiar el formato en disco.

| Prioridad | Qué cambia | Cargas y ganancia | Complejidad | Formato en disco |
|---|---|---|---|---|
| 1 | Preparar una proyección de columnas por esquema y versión. | W27: objetivo 10–12 %, con 12,4 % medido en el prototipo especializado. W26: 0–1 %. | Media | No cambia |
| 2 | Reutilizar runs privados después de copiarlos a scratch. | W17: 8–15 %, con 11,4 % medido y mejora en cuatro rondas. | Media | No cambia |
| 3 | Añadir al banco un camino de checksum para entradas de hasta 16 bytes. | W11: alrededor de 7 % medido. Sin ganancia clara en W26/W27. | Baja | No cambia |
| 4 | Reutilizar el camino localizado al modificar un multivalor inline. | W19: 3–7 % estimado, todavía sin A/B. | Media/alta | No cambia |
| 5 | Reutilizar buffers de filas y fusionar recorridos de validación/codificación. | W24: 2–5 % estimado. W28: 1–3 % estimado, con menor confianza. | Media | No cambia |

Riesgos de corrección y garantías:

1. **Proyección preparada.** Debe soportar versiones antiguas, nulos, defaults y tipos. El prototipo solamente conoce el esquema del banco. D11 exige conservar los préstamos de texto/bytes y su vida ligada a la transacción; T6 exige que la información compartida por lectores sea inmutable. Riesgo medio.
2. **Pool de runs.** Reciclar memoria todavía publicada rompería D11 y el aislamiento por instantánea. Hay que conservar la puesta a cero, los errores de asignación y el confinamiento del escritor conforme a T6. El límite experimental fue de 32 MiB; no constituye una decisión de producto. Riesgo medio.
3. **Checksum corto.** Debe producir exactamente la misma suma, incluidos tamaños límite. Pasaron 16.392 casos de equivalencia. No afecta D11 ni T6; no altera el motor. Riesgo bajo.
4. **Camino localizado de multivalor.** Debe conservar copy-on-write, splits y validez del camino cuando crece la celda. D11 impide modificar versiones visibles y T6 mantiene la operación en el escritor. Riesgo medio/alto.
5. **Buffers y recorridos de filas.** Deben mantenerse el orden de errores E17, la validación L3, los bytes del formato y los préstamos D11. Los buffers deben ser exclusivos del escritor para T6. La simple preasignación ya probada no alcanza para prometer las ganancias estimadas. Riesgo medio.

Garantías de referencia: [D11](../../prd/krummi/krummi.prd.md), [T6](../../prd/krummi/transactions/transactions.prd.md), [L3](../../prd/krummi/tables/tables-row-reads.prd.md).

**Decisiones de producto abiertas**

- Exposición de una API de proyección preparada y su alcance para esquemas históricos.
- Presupuesto de memoria y ámbito de retención del pool de runs.
- Política adaptativa de compresión compatible con J16. El coste observado sin ahorro ronda 15.288 ns/op en W17, pero no se validó una heurística que lo elimine conservando la garantía. El formato ya admite payloads sin comprimir; la decisión pendiente es de política y comportamiento, no necesariamente de formato.

**Comandos y trazabilidad**

Los comandos siguientes identifican las series ejecutadas desde `C:\projects\Krummi`. Cada `results.json` guarda el comando completo de cada worker, su salida, errores y tiempos; el `machine.json` correspondiente guarda máquina, afinidad, plan de energía y hashes de los binarios.

Los destinos ya existen. Para repetir una serie, deben usarse etiquetas y destinos nuevos; estos comandos documentan la ejecución histórica, no autorizan sobrescribir sus resultados.

```powershell
python -u work/diag-20261115/run.py baseline-aa baseline,voron W28,W24,W17,W19,W26,W27,W11 2
python -u work/diag-20261115/run.py probe probe,v-probe W28,W24,W17,W19 1
python -u work/diag-20261115/run.py voron-parts-all v-parts-all W28,W24 1
python -u work/diag-20261115/run.py diff-ab baseline,diff W28,W24,W17,W19 2
python -u work/diag-20261115/run.py byte-count-ab baseline,byte-count,voron,v-byte-count W26,W27,W11 2
python -u work/diag-20261115/run.py prefix-ab baseline,prefix W28 2
python -u work/diag-20261115/run.py prefix-insert-ab baseline,prefix W24 2
python -u work/diag-20261115/run.py run-pool-ab baseline,run-pool W17 4
python -u work/diag-20261115/run.py projection-ab baseline,projection W26,W27 2
python -u work/diag-20261115/run.py short-sum-ab baseline,short-sum W11,W27,W26 2
python work/diag-20261115/micros.py
work/diag-20261115/bin/checksum-micro.exe
work/diag-20261115/bin/diff-check.exe
work/diag-20261115/bin/short-sum-check.exe
```

La lectura posterior de los valores grandes se ejecutó con el binario original:

```powershell
$env:KRUMMI_BENCH_ROLE = 'worker'
work/diag-20261115/bin/baseline.exe run krummi W18 full work/diag-20261115/verify-pool-W18 work/diag-20261115/run-pool-ab/3-run-pool-W17/rep-3
```

El comando completo y su resultado están en [verify-pool-W18.json](verify-pool-W18.json). Los comandos de los microbenchmarks de ambos motores están en [micro-commands.json](micro-commands.json). Los scripts de preparación e instrumentación se conservan junto a este informe; [variant.py](variant.py) contiene las variantes de Rust y [run.py](run.py) el ejecutor serial.

**Comprobaciones y límites**

- Se auditaron **118 ejecuciones de workers sin errores ni divergencias de las huellas de entrada**, excluyendo deliberadamente del control de huellas las variantes que reemplazan el checksum por longitudes: [result-audit.json](result-audit.json).
- El cálculo alternativo de diferencias pasó 2.000 casos de equivalencia, aunque no mejoró las cargas: [diff-check.log](diff-check.log).
- El checksum corto pasó 16.392 casos de equivalencia: [short-sum-check.log](short-sum-check.log).
- El ejecutable original leyó los 4.096 valores grandes producidos por el pool experimental.
- HEAD, el diff y los 663 archivos versionados coincidían con la línea de base al finalizar las mediciones. El plan de energía volvió a Balanced, GUID `381b4222-f694-41f0-9685-ff5bb260df2e`.
- No se ejecutó una batería de recuperación ante caídas ni de concurrencia sobre los prototipos. Las ganancias medidas justifican priorizarlos, pero no prueban su corrección para producción.
- Los perfiles no aíslan cada causa de hardware ni explican por completo toda diferencia residual. Las cifras estimadas de las candidatas 4 y 5 no son resultados A/B.
- La diferencia de W17 respecto del banco histórico no queda explicada por Defender; esa atribución fue retirada.

---

<a id="seguimiento-2026-11-15"></a>

**Seguimiento del 2026-11-15 — respuesta a los seis puntos de Claude**

Este añadido conserva intactos los bytes del informe anterior. Las conclusiones y prioridades de esta sección actualizan las anteriores cuando difieren. Las nuevas series usan etiquetas `r2-*`; no se sobrescribieron las series originales. El código experimental nuevo está en `followup-01/src/` y sus scripts y pruebas en `followup-01/`.

Se mantuvo la revisión `28f9d3bcdb5377249bd0ce7fc75fde08daae3646`, la misma máquina MSI-W11-01, Windows 11 26200, i9-13980HX, Rust 1.99.0 y Voron 7.2.7 sobre .NET 10.0.5. Los workers se fijaron a `0xffff`, bajo el plan Krummi bench con boost desactivado. Cada proceso hizo un calentamiento y tres repeticiones medidas, con orden inverso en la segunda ronda; W17 se comparó en cuatro rondas. No se solaparon compilaciones con las mediciones. El plan original se restauró al terminar.

Resultados consolidados y comprobación del repositorio: [followup-01/summary.json](followup-01/summary.json). En los porcentajes siguientes, un valor negativo indica menos tiempo y uno positivo, más tiempo.

**1. Checksum corto: aplicación original y comparación bilateral**

La variante original `short-sum` modificó **solamente `crates/krummi-bench/src/data.rs`**. No modificó `voron/Data.cs`. El 7 % informado era un A/B de Krummi, no una medición nueva del cociente con Voron. Esa distinción debía quedar más explícita.

Se midieron ahora cinco binarios en la misma serie:

- Rust original y Rust `short-sum` original, conservados de la primera ronda.
- Voron original.
- Voron con la misma regla literal: para hasta 16 bytes, acumular escalarmente en `uint` y devolver `ulong`.
- Voron con camino corto nativo: para 16 bytes conservar SSE2 y sumar un único vector, y para menos de 16 bytes usar la reducción escalar. Esto evita degradar deliberadamente una implementación que ya utilizaba SIMD.

| Comparación W11 | Krummi, ns/entrada | Voron, ns/entrada | Krummi / Voron |
|---|---:|---:|---:|
| Antes: originales | 60,441 | 59,296 | 0,981 |
| Después: camino corto escalar literal en ambos | 56,751 | 65,492 | 1,154 |
| Después: Rust corto y Voron corto conservando SSE2 | 56,751 | 59,458 | 1,048 |

La transcripción literal a C# empeora Voron aproximadamente un **10,4 %**; no corresponde usar el cociente 1,154 como evidencia de una ventaja del motor. Conservando su implementación nativa, Voron cambia aproximadamente un **0,3 %**, dentro de la variación. Rust reduce el tiempo un **6,1 %** en esta ronda y el cociente pasa de **0,981 a 1,048** en la comparación que mantiene SIMD en Voron.

Una diferencia de implementación con idéntico checksum no invalida por sí sola un A/B local de Krummi. Lo que faltaba era volver a medir el comparador y no extrapolar el resultado local al cociente. La candidata sigue siendo una mejora del **banco**, no del motor.

Las tres funciones C# —original, corta literal y corta SIMD— pasaron **16.392 casos de equivalencia**, incluyendo tamaños 0–4096 y 65.536, con cuatro patrones de datos. El Rust corto ya tenía los 16.392 casos de la primera ronda. Las cargas completas también terminaron con sus comprobaciones correctas.

Referencias de código: `crates/krummi-bench/src/data.rs:97`, `crates/krummi-bench/voron/Data.cs:101`, rama `short-sum` de [variant.py](variant.py). Variantes C# preservadas: [Data-short.cs](followup-01/Data-short.cs) y [Data-short-simd.cs](followup-01/Data-short-simd.cs).

Evidencia: [r2-checksum-ab/results.json](r2-checksum-ab/results.json), [machine.json](r2-checksum-ab/machine.json), [checksum-equivalence.log](followup-01/checksum-equivalence.log).

**2. W17: J16, granularidad de compresión y nuevos A/B**

Texto exacto de J16, en `docs/prd/krummi/journal/journal.prd.md:46`:

> **Compresión**: lo que se guarda en el diario ocupa menos que las páginas que cambia cuando sus datos se comprimen bien; los datos que no se comprimen se guardan igualmente, sin error. La compresión no cambia nada de lo que leen las transacciones ni de lo que contiene `krummi.data`.

En *Dejado a la implementación*, el mismo PRD, línea 212, dice:

> El algoritmo de compresión y la organización interna del diario y de `krummi.temp` (padre), incluido cómo se reconoce que una confirmación es durable según el diario (J9).

AC-J15, líneas 309–311, concreta una comprobación:

> Dada una confirmación que cambia 100 páginas llenas de un mismo byte, cuando se mide lo que ocupa en el diario según la propia implementación, entonces es menos que esas páginas; y dada una con bytes aleatorios, se confirma sin error y se recupera tras una caída (J16).

Por tanto, J16 no obliga a intentar siempre un algoritmo concreto ni prescribe una decisión valor por valor. Permite elegir algoritmo y organización, conservando el resultado exigido. En el formato actual, `encode_record_in` reúne las entradas de páginas de una confirmación y comprime ese payload como **un registro**. La cabecera tiene un indicador comprimido/sin comprimir para ese registro, no un indicador independiente por valor. Elegir entre ambas representaciones de un registro no cambia el formato; introducir una nueva representación individual por valor requeriría otro diseño y verificar su compatibilidad.

Hay una precisión importante a la sugerencia: **guardar sin comprimir cuando LZ4 no ahorra ya está implementado** en `crates/krummi-pager/src/journal.rs:356`: solo se acepta el resultado si `n < raw_len`; en caso contrario, se copia el payload original y se deja el indicador de compresión desactivado. Repetir esa política no elimina los 15.288 ns del intento de LZ4. Para ahorrar esa CPU hay que evitar trabajo del intento o usar otro algoritmo/implementación.

Se ensayaron dos controles distintos, manteniendo intactas las cabeceras y la representación existente:

| Experimento W17 | Control, ns/op | Variante, ns/op | Cambio de tiempo | Interpretación |
|---|---:|---:|---:|---|
| Muestrear 16 fragmentos de 4 KiB distribuidos por registros de al menos 1 MiB; omitir LZ4 completo si la muestra no ahorra | 197.311 | 201.830 | +2,3 % | No mejora; el predictor no evitó intentos en esta carga. |
| Omitir incondicionalmente LZ4 en registros de al menos 1 MiB | 199.485 | 199.155 | −0,17 % | Ablación del coste, no política admisible para datos compresibles; sin mejora estable de tiempo total. |

En una ejecución separada con trazas, **ninguno de los 64 registros grandes observados evitó LZ4** con la primera política. Las muestras de 65.536 bytes comprimían a entre **64.376 y 65.362 bytes**, aunque el intento sobre el payload completo de W17 no ahorra. La muestra introduce trabajo y predice mal precisamente esta situación. Las trazas no se usaron para calcular la ganancia del A/B.

Además, la política de muestreo tiene un falso negativo comprobado: un payload de **2.097.152 bytes**, con datos aleatorios en las posiciones muestreadas y ceros fuera de ellas, se reduce a **74.074 bytes** con LZ4, pero la política decide no intentarlo. El round-trip sigue siendo correcto, pero se pierde el ahorro de un caso claramente compresible. **No la considero una implementación compatible con J16 tal como está.** No basta con que pase el ejemplo de páginas uniformes y datos completamente aleatorios.

La segunda variante sirve para comprobar cuánto cambia la carga si se quita el intento, sin afirmar que pueda enviarse a producción. Sus medianas por ronda fueron:

| Ronda | Control, ns/op | Sin LZ4 en registros grandes, ns/op |
|---|---:|---:|
| 1 | 207.227 | 198.246 |
| 2, orden invertido | 199.731 | 193.259 |
| 3 | 195.504 | 191.222 |
| 4, orden invertido | 204.133 | 224.744 |

Las tres primeras rondas mejoran; la cuarta empeora y la mediana conjunta queda prácticamente igual. No convierto los **15.288 ns/op de un perfil de otra ejecución en un 8 % garantizado** del tiempo total: este A/B no lo confirma de forma estable. Tampoco demuestra que el coste de CPU de LZ4 no exista. No se midieron contadores de CPU ni se aisló la interacción con el trabajo de E/S en segundo plano.

El ejecutable original pudo leer los 4.096 valores producidos por la variante sin LZ4, y la prueba de la política cubrió round-trip de datos aleatorios, uniformes, mixtos y del contraejemplo. No se ejecutó una nueva batería de caídas para estas variantes. La decisión de retirar un intento infructuoso no requiere por sí misma cambiar J16; **aceptar falsos negativos como el comprobado sí supondría relajar la garantía**, y no se asume esa decisión.

Evidencia: [r2-compression-ab](r2-compression-ab/results.json), [r2-compression-observe](r2-compression-observe/results.json), [r2-compression-ceiling-ab](r2-compression-ceiling-ab/results.json), [prueba de la política](followup-01/compression-policy-test-03.log), [lectura posterior](followup-01/verify-ceiling-W18.json). Fuentes experimentales: [journal-sampling-final.rs](followup-01/journal-sampling-final.rs), [journal-ceiling.rs](followup-01/journal-ceiling.rs).

**3. Proyección preparada en ejecución: cuánto queda del 12,4 %**

Se construyó un segundo prototipo sin nombres de columnas ni offsets del esquema del banco dentro de su implementación. La preparación resuelve nombres y tipos en ejecución; la primera fila de cada versión prepara sus slots y posiciones, que se conservan para las filas siguientes. Los offsets finales de columnas variables siguen leyéndose de la fila, como exige el formato. Los accesores tipados utilizados son los seis tipos de la carga; el prototipo no pretende ser una API pública completa y terminada.

El prototipo verifica la identidad del esquema al enlazar una fila y la pertenencia de los handles a la proyección. Conserva nulos, defaults, claves, filas propias y versiones históricas. Para defaults almacenados utiliza el mecanismo existente que toma los bytes del catálogo. Los resultados de texto mantienen el préstamo de la transacción, no copian su contenido. El cache mutable de planes pertenece al consumidor; no se modificó el contrato de compartir la transacción de lectura de T6. Se comprobó que el tipo de proyección es `Send + Sync`.

La prueba funcional alternó filas de versiones antiguas y actuales, incluyó nulos, defaults de texto y booleanos, solicitudes de tipos/nombres incorrectos y una fila convertida a propia. Para el texto prestado comprobó igualdad de contenido y de dirección con el acceso existente. La prueba pasó. La selección por nombres se hizo en ejecución y la preparación quedó dentro del tiempo medido, una vez por consumidor y por versión encontrada.

| Carga | Control, ns/op o entrada | Proyección preparada, ns/op o entrada | Cambio |
|---|---:|---:|---:|
| W27 | 102,633 | 101,690 | −0,9 % |
| W26 | 1.336,212 | 1.384,701 | +3,6 % |

**Este prototipo conserva aproximadamente un 0,9 % de mejora en W27 y empeora W26. Retiro el 10–12 % como expectativa respaldada para una proyección general.** El 12,4 % anterior sigue siendo la medida del prototipo especializado, no de una API general. Otros diseños podrían reducir el coste de binding y comprobaciones de handles, pero no están medidos; no asigno una ganancia a ese trabajo futuro.

Evidencia: [r2-projection-ab](r2-projection-ab/results.json), [prueba funcional final](followup-01/projection-test-03.log), [implementación del prototipo](followup-01/prepared-projection.rs), [fuentes exactas usadas y prueba](followup-01/projection-source.zip).

**4. Pool de runs: propiedad de los bytes, límite y fallos**

El argumento que permite reciclar el buffer privado es la separación entre dos asignaciones, no que hayan terminado todos los lectores:

1. El `RunBuf` privado es un `Box<[u8]>` de la escritura. `seal_runs` lo entrega a `writer.commit`.
2. `Scratch::stage`, en `crates/krummi-pager/src/scratch.rs:1390`, llama a `TempArea::store_run`. Esta función, en `temp.rs:187`, **copia** el contenido a slots propios del área temporal mediante `copy_from_slice`.
3. El journal toma sus referencias de esos slots preparados y `publish` publica versiones que contienen `Arc<Slot>`, no referencias al `Box` privado.
4. El prototipo llama a `keep_runs(runs)` después de que `writer.commit` retorna. En ese punto, lectores, traslado y journal usan las copias preparadas/publicadas; pueden seguir usándolas aunque el buffer privado se reutilice inmediatamente.

Esto conserva la separación de D11 y el aislamiento por instantánea. Las referencias dentro de la propia escritura no sobreviven a consumirla con `commit`, conforme a los préstamos existentes. La reutilización es local al hilo, compatible con el confinamiento del escritor de T6; no recicla los `Slot` ni sus `Arc`.

Los **32 MiB son un límite de bytes de buffers inactivos retenidos por hilo**, no un límite de transacción, de almacén ni de memoria total del proceso. No incluyen el vector de metadata, el scratch, el journal ni los buffers activos. Como el cache es `thread_local`, distintos hilos pueden retener cada uno hasta ese límite y el cache puede sobrevivir al cierre de un almacén, hasta que termine el hilo.

- Si una transacción necesita más de 32 MiB, sigue asignando sus runs normalmente. Al devolverlos se retienen solamente los que caben; los demás se liberan. Un run individual mayor que el límite no se conserva. No se truncan valores ni se rechaza la transacción por ese límite.
- La búsqueda exige el tamaño exacto del buffer. Si no hay uno adecuado, se asigna uno nuevo. No se expulsan tamaños antiguos para privilegiar otros nuevos en este prototipo.
- Al sacar un buffer del cache se descuenta su tamaño. Si el punto de fallo de asignación devuelve error, ese buffer se libera y no queda contabilizado; si continúa, se pone a cero completo antes de entregarlo.
- Si la escritura falla o aborta antes de llegar a devolver sus runs, sus buffers activos se liberan normalmente. El cache previo no se borra por ese motivo.
- Si `writer.commit` devuelve un error de preparación o encolado, `keep_runs` se ejecuta antes de propagarlo. Los slots parcialmente preparados se descartan por sus propietarios y no se publican; conservar los buffers privados no hace visibles esos cambios. Si falla la espera de durabilidad después del punto sin retorno, las copias publicadas siguen siendo independientes del cache.

Se añadieron dos pruebas dirigidas sobre el código del prototipo original:

- Límite exacto, descarte de un run mayor que 32 MiB, puesta a cero, reutilización de la misma asignación y fallo de asignación inyectado al reutilizar: **PASS**.
- Una lectura antigua conserva dirección y contenido durante tres escrituras posteriores; una transacción de **40 MiB** confirma y se lee completa; abortar otra escritura no cambia el valor visible: **PASS**, en almacén temporal.

**Limitación nueva identificada:** `keep_runs` usa `Vec::push` sin reserva fallible para su metadata. Si ese crecimiento agota memoria, puede abortar el proceso en vez de simplemente renunciar al cache, incluso después de publicar. Antes de integrar habría que hacer la retención opcional y no fallante para la operación —por ejemplo, liberar el buffer si no se puede reservar metadata— y verificar los caminos de error. Las pruebas de fallos indicadas cubren el `alloc_point` del run, no esa asignación infalible ni una batería completa de E/S y caídas. Por ello se mantiene la ganancia medida, pero no se declara el prototipo listo para producción.

Fuentes: rama `run-pool` de [variant.py](variant.py), copia previa a la siguiente variante en [before-projection.zip](before-projection.zip), `crates/krummi-tx/src/transaction.rs:549`, `crates/krummi-pager/src/scratch.rs:1322`, `scratch.rs:1426`, `scratch.rs:1450` y `crates/krummi-pager/src/temp.rs:187`.

Pruebas: [pool-cap-test.log](followup-01/pool-cap-test.log), [pool-snapshot-test.log](followup-01/pool-snapshot-test.log). Su preparación y comandos están en [tail-checks.py](followup-01/tail-checks.py).

**5. Prioridades actualizadas: medidas separadas de hipótesis**

Esta tabla sustituye la clasificación anterior para decidir próximos trabajos. No se suman ganancias entre variantes.

| Prioridad entre mejoras medidas | Candidata | Evidencia | Condición antes de implementar |
|---|---|---|---|
| M1 | Pool de runs privados, antes candidata 2 | W17: −11,4 % en la primera ronda, cuatro comparaciones favorables; nuevas pruebas dirigidas de propiedad, límite y fallo pasan. | Corregir la retención infalible de metadata y decidir presupuesto/ámbito del cache; faltan pruebas completas de error y recuperación. Sin cambio de formato. |
| M2 | Checksum corto del banco, antes candidata 3 | W11: −6,1 % en esta ronda; cociente 0,981 → 1,048 conservando SSE2 en Voron. | Mantener equivalencia y una implementación eficiente en ambos lados. No es mejora del motor. Sin cambio de formato. |

| Medido, pero no promovido | Resultado | Decisión |
|---|---|---|
| Proyección preparada general, antes candidata 1 | W27: −0,9 %; W26: +3,6 %. | No respaldar el 10–12 % para la versión general ni priorizar este prototipo. |
| Política de muestreo para LZ4 | W17: +2,3 %; no evita intentos en los registros observados y tiene un falso negativo que pierde compresión sustancial. | Descartada tal como está por rendimiento y por J16. |
| Omitir LZ4 en registros grandes | Mediana conjunta: −0,17 %, con rondas discordantes. | Solo ablación; no política compatible con J16 ni mejora estable demostrada. |

Las siguientes quedan **fuera del orden de las medidas**. No se hicieron nuevos A/B para ellas en este seguimiento:

| Hipótesis no medida | Candidata anterior | Estimación anterior, no validada | Riesgo principal |
|---|---|---|---|
| H1: reutilizar el camino localizado de multivalor | 4 | W19: 3–7 %. | Copy-on-write, splits y validez del camino; preservar D11 y confinamiento T6. No se prevé cambio de formato. |
| H2: buffers de filas y fusión de recorridos | 5 | W24: 2–5 %; W28: 1–3 %. | Orden de errores E17, validación L3, préstamos D11 y buffers privados del escritor. No se prevé cambio de formato. |

Esas cifras son hipótesis basadas en los costes observados, no ganancias demostradas ni compromisos de implementación.

**6. W17 frente al banco del 2026-11-13: qué se pudo comprobar**

| Condición | Evidencia histórica y actual | Alcance de la conclusión |
|---|---|---|
| Plan de energía | `work/w11/full.sh` invoca el plan del banco; `work/w11/power.log` dice `boost off`. Las dos rondas del diagnóstico usaron ese plan; el valor AC comprobado en esta ronda fue `0x00000000`. | Coincide el ajuste de boost y el plan; no se registró una traza de frecuencia/temperatura de ambas fechas. |
| Afinidad | El histórico declara 16 hilos lógicos de rendimiento y su ejecutor fija los workers. Los nuevos registros guardan `0xffff` y el inventario de cores 0–15 de rendimiento. | Coincide el criterio de usar los 16 hilos P; no hay una máscara histórica cruda guardada para afirmar identidad byte a byte del registro de afinidad. |
| Rust, SO, CPU y Voron | Histórico: Rust 1.99.0, Windows 26200, i9-13980HX, Voron 7.2.7, .NET 10.0.5. Actual: los mismos; `rustc -Vv` identifica `b940084d7`, host `x86_64-pc-windows-msvc`, LLVM 23.1.1. | Coinciden las versiones publicadas. No se conserva el `-Vv` histórico completo. |
| Revisión y binario | Histórico: `44b10940c4f3`; el log ODD aclara que lo sin confirmar eran documentos y que el código era el de ese commit. Diagnóstico: `28f9d3b`, recompilado, con `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`. | No son binarios cuya identidad se haya demostrado. El diff de crates entre revisiones contiene comentarios, tests y un assert de compilación del límite de prefetch; no un cambio del camino W17. No hay hash histórico verificable ni entorno completo de compilación para afirmar equivalencia binaria. |
| Disco | Ambos identifican NVMe SAMSUNG MZVL22T0HBLB-00B00 y NTFS. La consulta nueva informa estado Healthy/OK y 956.717.514.752 bytes libres. | No hay estado histórico comparable de espacio libre, temperatura, actividad de fondo, cache de escritura o presión de E/S. Un estado actual saludable no prueba igualdad de condiciones durante ambas mediciones. |
| Secuencia de ejecución | Histórico: banco completo de las 29 cargas. Diagnóstico: workers de cargas aisladas y A/B, con un calentamiento y tres repeticiones por proceso. | El acondicionamiento previo del sistema y del disco no es idéntico; no se aisló cuánto influye. |
| Defender | La exclusión de `work` fue aportada por el usuario; la consulta del agente la ocultaba. | No hay evidencia de una diferencia de configuración. Se mantiene retirada esa explicación. |

En números absolutos, el histórico publica 5.187 op/s en Krummi y 5.440 en Voron: aproximadamente **192.790 y 183.824 ns/op**. La comparación inicial de este diagnóstico dio **196.102 y 161.674 ns/op**. Krummi tarda alrededor de un 1,7 % más y Voron alrededor de un 12,0 % menos; el cambio del cociente procede principalmente del mejor tiempo de Voron en esta ejecución, no de una gran regresión demostrada de Krummi.

**La diferencia con el histórico sigue sin explicación causal demostrada.** No se atribuye a Defender, al SSD, al JIT ni a otra condición no aislada. Las discrepancias de secuencia y de identidad de binario son límites de comparabilidad, no causas probadas.

Evidencia: [registro histórico del plan](../../../work/w11/power.log), [comando histórico](../../../work/w11/full.sh), [banco histórico](../../benchmark/2026-11-13-windows-range-step.md), [ODD L7](../../../odd/done/trees/range-step.md), [diff de fuentes](followup-01/historical-source-diff.patch), [entorno y Rust](followup-01/environment.json), [consulta de disco](followup-01/environment-disk.json), [consulta del boost](followup-01/power-query.log).

**Comandos nuevos y comprobaciones finales**

Desde `C:\projects\Krummi`, con los binarios preparados por los scripts conservados en `followup-01/`:

```powershell
python -u work/diag-20261115/run.py r2-checksum-ab baseline,short-sum,voron,v-r2-short,v-r2-short-simd W11 2
python -u work/diag-20261115/run.py r2-compression-ab baseline,r2-compression-sample W17 4
python -u work/diag-20261115/run.py r2-projection-ab baseline,r2-projection-runtime W27,W26 2
python -u work/diag-20261115/run.py r2-compression-observe r2-compression-observe W17 1
python -u work/diag-20261115/run.py r2-compression-ceiling-ab baseline,r2-compression-ceiling W17 4
```

Los comandos completos, stdout/stderr y repeticiones están en los `results.json` de esas series; cada `machine.json` registra los binarios y sus hashes. Los comandos exactos de compilación y pruebas dirigidas están en los logs de `followup-01/`. Los scripts Python de esta ronda se ejecutaron con `-P` cuando fue necesario para evitar que el nombre local `compression.py` ocultara el paquete homónimo de la biblioteca estándar de Python 3.14. Los intentos fallidos de preparación/compilación de los prototipos se conservaron en logs distintos; las comprobaciones finales citadas arriba pasaron.

Se auditaron **35 workers nuevos**, sin errores ni divergencias de las huellas de entrada. Las pruebas dirigidas finales fueron: equivalencia C# de 16.392 casos; round-trip y contraejemplo de la política de compresión; proyección con versiones/nulos/defaults/préstamos; límite/puesta a cero/fallo de asignación del pool; snapshot y transacción de 40 MiB; lectura con el binario original de los valores producidos por la ablación de LZ4. No sustituyen una validación completa de recuperación y concurrencia.

La comprobación previa a anexar esta sección encontró **HEAD, diff y los 663 archivos versionados intactos**, y el informe anterior idéntico a su copia inicial. La incorporación se hace en modo append y se verifica que todos los bytes anteriores permanezcan como prefijo exacto. No se cambiaron archivos versionados ni se hicieron commits.


---

<a id="proyeccion-ronda-3-2026-11-15"></a>

**Tercera ronda — proyección preparada compartida — 2026-11-15, Atlantic/Canary (UTC+00:00)**

Este apartado se agrega al informe anterior; no sustituye sus resultados ni modifica sus bytes. El alcance es exclusivamente la proyección de W26 y W27. Todo el código experimental, los ejecutables, las bases y las mediciones están en `work/diag-20261115/`. No se ha modificado el repositorio versionado.

### Resultado y control de la medición

El diseño general con handles tipados reduce la mediana de **W27 un 6,7 %**, con mejora en las cuatro rondas. En **W26 la mediana baja 2,8 %**, pero una ronda empeora; la comprobación adicional de abajo también muestra una señal menor y no uniforme. La variante por índice reduce las medianas **3,8 %** y **1,9 %**, respectivamente. La preparación está fuera del cronómetro y se mide por separado. El resultado de los handles conserva aproximadamente **54 % del porcentaje de mejora** del prototipo especializado anterior (12,4 %); es una comparación orientativa entre rondas, no un A/B directo contra aquel prototipo. La evidencia más clara para priorizar este diseño es W27.

**Incidencia detectada y corregida antes de entregar.** El primer `cargo build` de esta ronda reutilizó un ejecutable del directorio `target` compartido. Los SHA-256 demostraron que `r3-baseline.exe` era exactamente `r2-compression-ceiling.exe` (`f56804f603848a8da9128cbd4c08ebb9c75b277720d7eb11dd5c390e3606ca8c`). Las copias habían conservado las fechas de los archivos fuente y el log terminó en 0,54 s sin recompilar. Aunque la ablación afecta al diario de escrituras, no uso ese ejecutable como referencia limpia de `main`.

Se conservan las series `r3-projection-ab`, `r3-prepared-ab`, `r3-projection-micro` y `r3-projection-bounds` como **provisionales**. Los porcentajes comunicados durante ellas quedan sustituidos para la conclusión de esta ronda por la serie `r3-verified-ab`. Se recompilaron el control y las variantes desde un directorio de compilación nuevo, `followup-02/target-verified`, se repitieron los A/B y los microbancos y se verificaron sus fuentes y hashes. El control limpio tiene SHA-256 `3ecd0fefdf6770a15385e3d8a4a6bcf22b996eaf224885006d462306ef49c7d6`. Los logs de compilación registran la compilación efectiva de los cinco crates propios.

**Corrección al supuesto sobre W26 de la ronda anterior.** El prototipo anterior preparaba dentro del tiempo medido, pero **una vez en la primera fila de cada repetición**, no en cada búsqueda: `followup-02/previous-tables.rs:73` declara la caché antes del bucle; `:78` la pasa por referencia y `:252` usa `get_or_insert_with`. Ese archivo se extrajo de `followup-01/projection-source.zip`. Las ramas por columna sí permanecían en aquel prototipo. La nueva medición separa ambas cuestiones.

### Diseño que se ha ejecutado

El código de referencia es [projection-final.rs](followup-02/projection-final.rs), y la copia compilada completa de los archivos fuente Rust está en [verified-source.zip](followup-02/verified-source.zip). Los offsets no están escritos para el esquema del banco: se calculan desde `RowLayout`, los nombres llegan mediante `black_box`, y la preparación está marcada `inline(never)`.

Las referencias de esta tabla son relativas a `work/diag-20261115/followup-02/`.

| Aspecto | Implementación y ubicación |
|---|---|
| Caché compartida | `Store` guarda `ProjectionReaders`; todas las transacciones la usan. Clave `(table_id, schema_state, version)`, dentro del almacén, y token `Arc` canónico: `projection-final.rs:16` y `:26`; `verified-src/crates/krummi/src/lib.rs:124`. No se usa el nombre ni una página reutilizable. |
| Identidad que no se repite mientras sea visible | Las identidades retenidas se incluyen en la comprobación al sortear identidades nuevas: `projection-final.rs:20`; `verified-src/crates/krummi/src/table/catalog.rs:569` y `:657`. Esto también distingue un cambio abortado de otro posterior con el mismo número de versión. Cada proyección tiene además su token propio para sus handles. |
| API y preparación | `ReadTable::projection(&[...]) -> Result<Arc<Projection>>` valida nombres y tipos admitidos y reutiliza la proyección: `projection-final.rs:128`. `column::<T>(índice)` comprueba una vez el tipo esperado del handle: `:230`. No hay un tipo esperado deducible solo del nombre; el acceso por índice lo comprueba en cada llamada. |
| Plan por versión | Posición, ancho, byte/máscara de nulo, elemento de clave, posiciones de offsets y defaults se resuelven en `:170`. Se preparan las tres posibilidades de ancho de offset: 1, 2 y 4 bytes. Ese ancho depende de cada fila, no solo de su esquema. |
| Lectura de la fila | `read` comprueba el token de tabla/esquema, distingue almacenada/propia y elige el plan una vez para todas las columnas: `:236`. No se clona ningún `Arc` por fila. |
| Versiones antiguas | El plan actual se prepara en `projection`. Cada versión antigua se prepara una sola vez al encontrarla, a partir del layout que ya resolvió el decodificador de filas, y se publica con `OnceLock<Arc<...>>`. Después lo comparten las transacciones. El primer encuentro tiene un coste separado abajo; no afirmo que un recorrido frío con versiones mezcladas quede libre de él. |
| Clases de lectura | El camino directo distingue columna fija, clave y una única columna variable; las otras combinaciones usan funciones elegidas al preparar: `:199`. No vuelve a ejecutar `RowLayout::slot_bytes` ni un `match` genérico de `SlotPlace` por columna. Los parámetros de estos caminos siguen siendo datos calculados en ejecución. |
| Variante A: índice | `r.i64(1)`, `r.text_bytes(5)`, etc.: límite del índice, etiqueta de tipo y localización comprobada del descriptor: `:259`. |
| Variante B: handle | `r.get(&Col<i64>)`: sin etiqueta de tipo por llamada; conserva comprobación de pertenencia del handle y límite de su posición: `:254`. Un handle de otra proyección devuelve error, aunque ambas proyecten la misma tabla. |

Se cubren `Ulid`, `i64`, `f64`, `timestamp`, `bool`, texto y también bytes. Nombre desconocido, tipo no admitido, índice incorrecto, handle de tipo equivocado y fila de otro esquema devuelven errores. Se conservan los límites de la API actual para filas malformadas (L2): las comprobaciones de slices de Rust pueden producir un pánico ante contenido malformado; no se han eliminado ni se ha añadido `unsafe`.

La variante exploratoria con todas las lecturas indirectas queda archivada en `projection.rs` y `measured-source.zip`. La variante definitiva incorpora lecturas directas para las clases frecuentes y selección directa del plan de versión actual. No atribuyo por separado una ganancia a cada uno de esos dos cambios.

### A/B definitivo de W26 y W27

Máquina de todas las cifras nuevas: **MSI-W11-01, Intel Core i9-13980HX, Windows 11 26200**, afinidad **`0xffff`** (16 hilos lógicos P), plan temporal del banco con boost desactivado. Rust **1.99.0** (`b940084d7`, LLVM 23.1.1), `x86_64-pc-windows-msvc`, release con `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`; Voron **7.2.7**, .NET **10.0.5**, SDK **10.0.201**. No se cambió Defender. Se restauró el plan Balanced al terminar.

Cuatro rondas seriales, orden invertido entre rondas; cada worker ejecuta un calentamiento y tres repeticiones medidas. Cada celda resume **12 repeticiones**, sin retirar valores atípicos. W26 hace 1.000.000 de búsquedas por repetición; W27 visita 9.999.205 filas. Los digests y las sumas esperadas coinciden en todas las ejecuciones. No hubo compilaciones concurrentes con las medidas.

`control` contiene la misma caché y la misma preparación fuera del cronómetro, pero consume las filas con los accesores originales de `TableRow`; permite separar su efecto del acceso proyectado. `baseline` es el código limpio de `28f9d3bcdb5377249bd0ce7fc75fde08daae3646`. Los workers conservan dentro del tiempo medido la transacción y apertura de tabla originales; las variantes preparan y sueltan otra apertura previa para llenar la caché compartida.

| Binario | W27: ns/fila, mediana [mín.–máx.] | W26: ns/búsqueda, mediana [mín.–máx.] |
|---|---:|---:|
| Krummi limpio | 102,55 [99,68–104,70] | 1409,38 [1349,09–2040,79] |
| Control con caché | 102,19 [100,27–107,68] | 1419,55 [1365,44–6425,15] |
| A: índice + etiqueta | 98,62 [96,49–139,28] | 1382,94 [1346,61–7648,82] |
| B: handles tipados | 95,70 [94,11–103,73] | 1370,59 [1340,54–3565,23] |
| Voron sin modificar | 94,93 [91,73–109,20] | 1329,10 [1271,65–1591,93] |

Cociente de rendimiento **Krummi / Voron = tiempo de Voron / tiempo de Krummi**; más es mejor para Krummi.

| Carga | Limpio | A: índice | B: handles | B frente al control con caché |
|---|---:|---:|---:|---:|
| W27 | 0,926 | 0,963 | 0,992 | −6,3 % de tiempo |
| W26 | 0,943 | 0,961 | 0,970 | −3,4 % de tiempo |

Comprobación de consistencia: medianas de cada ronda y reducción de B frente al control limpio.

| Ronda | W27 limpio → B, ns/fila | Cambio | W26 limpio → B, ns/búsqueda | Cambio |
|---|---:|---:|---:|---:|
| 1 | 99,77 → 94,38 | -5,4 % | 1416,24 → 1364,42 | -3,7 % |
| 2 | 102,14 → 95,91 | -6,1 % | 1365,91 → 1347,76 | -1,3 % |
| 3 | 102,55 → 95,67 | -6,7 % | 1923,52 → 2141,14 | 11,3 % |
| 4 | 104,03 → 95,74 | -8,0 % | 1402,53 → 1373,73 | -2,1 % |

**Comprobación adicional de W26, fijada en cuatro pares base/handles ante la dispersión de la serie principal.** Sin descartar ni mezclar sus datos con los anteriores: base **1388,78 ns/op** [1365,13–1428,01], handles **1371,12 ns/op** [1339,86–1407,29]; reducción de la mediana **1,3 %**. Cambios de cada ronda: -1,6 %, -3,6 %, 0,5 %, -2,2 %. Hay mejora en tres de cuatro rondas, y un retroceso pequeño en la restante. W26 ofrece una señal de ganancia modesta, todavía dependiente del entorno; no doy por confirmado un ahorro uniforme. No hubo Voron en esta comprobación adicional y no se calcula un cociente cruzando sesiones.

En la serie principal de W26 hay pausas grandes (hasta varios microsegundos adicionales por operación en una repetición); no se aisló su causa. Se conservan todas las muestras y la ronda discordante. Las cifras no prueban una ventaja universal frente a Voron: son estas cargas y este entorno. La variación de Voron en W26 también limita la interpretación del cociente. No se suman estas ganancias a las de otras candidatas.

### Coste de preparar y de comprobar

Microbancos seriales en la misma máquina, afinidad y plan. Cinco procesos independientes; dentro de cada proceso, ocho rondas con orden invertido, descartando la ronda 0 como calentamiento: **35 muestras** para los costes sostenidos. Cada muestra de accesores o `read` repite 2.000.000 veces; cada muestra de preparación repite 20.000. Los tiempos de primera llamada son cinco observaciones individuales, con código/asignador en su estado de primer uso, y no son equivalentes a las preparaciones repetidas.

| Operación | Mediana | Alcance |
|---|---:|---|
| Primera `table.projection` | 25,90 µs [21,10–57,20] | Una proyección de seis columnas, cinco procesos |
| Primeros seis handles | 10,90 µs [7,10–12,20] | Una construcción del conjunto por proceso |
| Preparar otra vez, sin caché | 3,18 µs [3,11–6,53] | Asignador caliente; incluye liberación del resultado |
| Encontrar una proyección preparada | 54,97 ns [54,45–89,53] | Incluye devolver/liberar el Arc |
| Construir/liberar seis handles, caliente | 138,33 ns [136,95–223,32] | Validación de tipos y referencias |
| Primer `read` de otra versión de fila | 9,70 µs [9,30–13,40] | Plan antiguo; el decodificador ya resolvió su layout |
| `read` actual, sostenido | 5,79 ns/fila [5,72–8,24] | Identidad/esquema, clase de fila y selección del plan |
| `read` de versión antigua, ya preparada | 6,75 ns/fila [6,68–7,41] | Incluye consulta de OnceLock |

Ablaciones de comprobaciones, sobre una fila caliente y el mismo contenido. Se informa la mediana de las **diferencias emparejadas**, dividida por seis cuando se leen seis columnas.

| Comprobación | Coste incremental medido | Qué se comparó |
|---|---:|---|
| Etiqueta de tipo | 0,40 ns/acceso [0,05–1,11] | Índice con etiqueta frente al mismo camino sin etiqueta; conserva los límites |
| Pertenencia del handle | 0,22 ns/acceso [-0,12–0,56] | Handle con comparación de token frente al mismo camino sin esa comparación |
| Tabla y versión de esquema | 0,48 ns/fila [0,36–0,80] | `read` con comparación de token frente al mismo `read` sin ella |
| Localizar descriptor y comprobar su índice, conjuntamente | 1,42 ns/acceso [0,94–1,94] | Handle sin comparación de dueño frente a referencias al descriptor ya localizadas y comprobadas para esa fila |

El grupo de seis accesores cuesta **28,36 ns por índice** y **26,63 ns con handles**, sin incluir `read`. En la ablación del límite, el conjunto pasa de **33,03 a 24,40 ns**. Esta última cifra **no aísla el salto del límite**: elimina también la búsqueda del descriptor y cambia la representación de su acceso. No atribuyo todo ese ahorro al salto, ni propongo omitir la validación. Las referencias prelocalizadas solo sirven para esa fila/versión; no se ha medido su preparación por cada fila de un recorrido.

Las funciones que omiten comprobaciones existen únicamente en los ejecutables diagnósticos de los microbancos, después de validar su entrada. **Los binarios A y B del A/B completo conservan las comprobaciones.** Estos deltas dependen del código generado, del patrón de accesos y de la caché; no son precios independientes que puedan sumarse para predecir el resultado de W27. El coste de la etiqueta es pequeño y no explica por sí solo la diferencia total entre los diseños.

### Corrección, riesgos y decisión pendiente

- **D11:** los planes no guardan referencias a filas ni páginas. El texto/bytes almacenado se presta desde la fila; los defaults de texto/bytes se prestan desde el catálogo ya retenido por la fila; una fila propia presta desde su propio contenido. Las referencias quedan ligadas a la fila/transacción, no al `Arc` de la proyección. Las pruebas comparan direcciones además de contenido en los préstamos, y una prueba de compilación obtiene los errores esperados `E0515` al intentar devolver esos bytes fuera de la fila/transacción.
- **T6 e instantáneas:** el plan publicado es inmutable; se comparten referencias `Arc`, las inserciones en caché están sincronizadas y los planes antiguos se publican una vez. El token canónico distingue tabla, esquema y almacén. Se probaron lecturas en cuatro hilos, varias transacciones, filas de escritura y propias, versiones antiguas y la misma versión numérica después de abortar un esquema. `Projection`, `Col<i64>` y la fila proyectada cumplen `Send + Sync` en comprobaciones de compilación.
- **Formato:** no cambia el formato en disco ni la codificación de filas. La selección de offsets conserva los anchos de 1, 2 y 4 bytes; se probaron filas con las tres anchuras, claves compuestas, nulos, defaults y distinto orden de columnas. No se modifica `Range`, el paso por hoja ni su precarga.
- **Riesgo de integración:** moderado. Una identidad o un plan de versión incorrectos pueden interpretar columnas con otro layout; el prototipo incorpora errores explícitos y pruebas de identidad. La caché añade retención de memoria y trabajo en las aperturas/cambios de esquema; solo se ha medido W26/W27 y los microbancos. No se ha ejecutado una batería de fallos de asignación ni la aceptación completa Windows/Linux. La compatibilidad con todos los tipos fuera de los siete del prototipo queda fuera de esta medición.
- **Pregunta abierta de producto:** presupuesto y política de retención/evicción de la caché. El prototipo retiene entradas hasta cerrar el almacén; cualquier política de desalojo debe preservar la identidad mientras existan proyecciones o instantáneas que la observen. También conviene decidir si se ofrece un precalentamiento explícito de las versiones antiguas para evitar su primer coste dentro de una consulta. No se decide aquí por el producto.

**Candidata resultante, en una línea:** preparar y compartir planes de lectura por identidad de esquema y versión de fila, y ofrecer handles tipados para consumirlos sin repetir la comprobación de tipo. Mejora confirmada en W27 y una señal menor, todavía variable, en W26; no cambia el formato. La proyección general vuelve a tener evidencia para considerarla candidata; la expectativa del 12,4 % especializado sigue sin estar justificada para una API general. Los valores de la tabla son evidencia de este prototipo, no una promesa para esquemas con otros patrones de nulos, defaults o versiones.

### Reproducción y verificaciones

Todos los comandos se ejecutaron desde `C:\projects\Krummi`; los scripts registran las rutas absolutas de los workers. Las órdenes principales fueron:

```powershell
python -u work/diag-20261115/followup-02/build-verified.py
python -u work/diag-20261115/followup-02/finish-verified.py
```

El primero crea la copia limpia y compila el control y los tres modos. El segundo compila/verifica los microbancos y ejecuta las tres primeras órdenes siguientes en serie; la cuarta se ejecutó después como comprobación adicional de W26. Los scripts rechazan destinos existentes: para repetir la ronda hay que usar directorios y etiquetas nuevos.

```powershell
python -u work/diag-20261115/run.py r3-verified-ab r3-verified-baseline,r3-verified-control,r3-verified-index,r3-verified-handles,voron W27,W26 4
python -u work/diag-20261115/followup-02/run-verified-micro.py
python -u work/diag-20261115/followup-02/run-verified-bounds.py
python -u work/diag-20261115/followup-02/run-w26-validation.py
```

Compilación de cada modo, con `CARGO_TARGET_DIR=C:\projects\Krummi\work\diag-20261115\followup-02\target-verified` y `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`:

```powershell
cargo build --manifest-path C:\projects\Krummi\work\diag-20261115\followup-02\verified-src\Cargo.toml --locked --offline --release -p krummi-bench --bin krummi-bench
cargo test --manifest-path C:\projects\Krummi\work\diag-20261115\followup-02\verified-src\Cargo.toml --locked --offline --release -p krummi --test diagnostic_projection -- --nocapture
cargo test --manifest-path C:\projects\Krummi\work\diag-20261115\followup-02\verified-src\Cargo.toml --locked --offline --release -p krummi --lib table:: -- --test-threads=1
```

Resultados: **4 pruebas dirigidas y 25 pruebas existentes seleccionadas, todas pasan**. La comprobación negativa de préstamos falla como se esperaba; no es un fallo del prototipo. Se conservan también los intentos de compilación que fallaron durante la construcción experimental. No se ejecutaron `clippy`, el workspace completo ni el banco completo de aceptación: esta ronda mide un prototipo desechable de lectura, no una entrega de producto.

Evidencia definitiva:

- [Resumen numérico, dispersión, deltas y auditoría](followup-02/summary-verified-complete.json).
- [W26/W27: comandos, repeticiones, digests y resultados](r3-verified-ab/results.json), [máquina y hashes de ejecutables](r3-verified-ab/machine.json).
- [Comprobación adicional de W26](r3-w26-validation/results.json), [su máquina y binarios](r3-w26-validation/machine.json).
- [Preparación y comprobaciones](r3-verified-micro/results.json), [su máquina y binario](r3-verified-micro/machine.json).
- [Localización y límites](r3-verified-bounds/results.json), [su máquina y binario](r3-verified-bounds/machine.json).
- [Fuentes y hashes del control limpio](followup-02/verified-baseline-source.json), [compilación del control](followup-02/build-r3-verified-baseline.log), [prototipo compilado](followup-02/verified-source.zip), [fuentes de microbancos](followup-02/verified-micro-source.zip).
- [Pruebas dirigidas](followup-02/verified-projection-tests.log), [pruebas existentes](followup-02/verified-existing-table-tests.log), [rechazo del préstamo que escaparía](followup-02/verified-borrow-fail.log).
- [Consulta del plan usado](followup-02/power-query-verified.log), [restauración final](followup-02/power-exit-w26-validation.log).

Se verificaron los 663 archivos versionados contra el estado inicial, además de HEAD, diff y estado de Git. El contenido anterior del informe se conserva byte por byte. No hubo commits ni cambios de rama, índice o fuentes versionados.
