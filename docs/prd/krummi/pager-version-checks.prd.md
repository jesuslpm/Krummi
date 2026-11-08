# Comprobación recordada de las versiones del área temporal — PRD

**Versión**: 1.0
**Fecha**: 2026-11-08
**Documento padre**: [Comprobaciones de la escritura — PRD](pager-writer-checks.prd.md) (v1.0)

## Objetivo y contexto

Hacer más baratas las lecturas de las **versiones confirmadas que todavía están en el área temporal** (J7, T16): las
páginas que una escritura confirmó y que el traslado todavía no escribió en el archivo de datos. Hoy, cada lectura de
un nodo desde una de esas versiones comprueba entera su organización (C3, S1), porque las versiones no tienen
comprobación de integridad (I3) con cuyo código recordarla. Una versión no cambia mientras está en el área temporal,
así que la misma comprobación se repite sobre los mismos bytes en cada lectura de cada transacción.

Con este cambio, cada versión del área temporal se comprueba la primera vez que una transacción la lee, y las
lecturas siguientes, de esa transacción o de otras, la usan sin comprobarla. Además, las lecturas de transacciones
distintas dejan de esperarse entre sí para encontrar la página que ven.

**Por qué**: en el banco comparativo (módulo 6), Krummi incumple el umbral de `lecturas-con-escritor` (W12) con 0,37
veces Voron en Windows y 0,63 en Linux (informes `2026-11-07-windows-write-reuse` y `2026-11-07-linux-write-reuse`
de `docs/benchmark`). En esa carga, cuatro hilos leen claves al azar mientras una escritura confirma sin parar.
Mientras el área temporal tiene versiones, cada lectura de una página espera a las demás lecturas para encontrarla,
sea o no una versión; y las versiones que leen, la raíz, las ramas y las hojas que la escritura cambió hace poco, se
comprueban enteras en cada lectura. Las escrituras también leen las versiones que confirmaron las anteriores: en el
diagnóstico del 2026-11-07 del plan de rendimiento, la comprobación de organización entera se lleva el 15 % de W2 y
el 22 % de W19 en Linux, y la de las versiones es una parte de eso que no se midió por separado. Según el código de
Voron que revisó la investigación del 2026-11-08, Voron no comprueba la organización de ninguna página.

**Por qué ahora**: es el candidato 2 del diagnóstico de las escrituras y la segunda palanca de la investigación del
2026-11-08 sobre lo que separa a Krummi de Voron, junto a escribir solo los cambios de cada página en el diario
(O8, con su propio PRD).

Este PRD cambia cuándo se comprueban las versiones del área temporal, para las transacciones de lectura y de
escritura, y precisa cómo las encuentran las lecturas. Hereda todo lo del padre y sus ascendientes. Lo que cambia
reglas anteriores está declarado en sus decisiones y anotado en los documentos afectados (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

### Decisiones de producto

| ID | Decisión |
|---|---|
| U1 | **Organización de las versiones del área temporal** (cambia C3 de la comprobación de organización recordada y la última frase de S1). La primera lectura de un nodo desde una versión confirmada que está en el área temporal comprueba su organización, como hoy, la haga una transacción de lectura o de escritura. Si la pasa, las lecturas siguientes de esa misma versión, de cualquier transacción, no vuelven a comprobarla mientras la versión siga en el área temporal. Una comprobación que falla no se recuerda, como en C1 y S1: cada lectura de esa versión vuelve a fallar con la clase de daño y el número de la página, y una escritura que la lee no puede confirmar (T11). Lo que se recuerda de una versión vale solo para ella: no se consulta ni se guarda en la memoria compartida de nodos recordados (C2) ni en la de los cambios de una escritura (S1), y no sirve para otra versión de la misma página, más nueva o de otro nodo que reutiliza la página. Una escritura que obtiene una copia de una versión recordada para modificarla hace un cambio (S1): la copia se comprueba en su primera lectura. Las demás comprobaciones de la lectura de un nodo (que la página pueda contener datos, su tipo, su nivel y la coherencia de cada entrada que se usa) se siguen haciendo en cada lectura (C1). Recordar no reserva memoria que pueda faltar, así que no hace fallar ninguna operación, y no cuenta para el tamaño máximo (P10). C5 no vale para esta memoria: se recuerda cada versión mientras está en el área temporal, sin descartes. |
| U2 | **Garantía resultante** (amplía el riesgo residual de C7 y de S3, anotados en ellos). Las versiones del área temporal no tienen comprobación de integridad, así que hoy la de organización, en cada lectura, es lo único que puede detectar que los bytes de una versión cambian en el área temporal sin que el motor los escriba (un fallo del hardware, del disco o de la memoria, o un programa ajeno, que P16 deja fuera de alcance), y solo cuando el cambio rompe su organización. Con este cambio, eso solo se detecta si ocurre antes de la primera lectura de la versión. Después, una lectura puede devolver datos que no son los escritos o terminar con un pánico en lugar de un error de daño. Si una escritura copia esa versión para modificarla, la comprueba en su primera lectura, que puede no detectarlo; si lo confirma, el cambio queda sellado con un código válido, como en S3. Si el traslado escribe la versión alterada en el archivo de datos, su código ya no coincide y la primera lectura desde allí falla con la clase de daño (G6). Se acepta como riesgo residual, como en Voron: afecta a lo que el motor lee y guarda solo ante fallos que el motor no causa. Todo daño hecho con el almacén cerrado se sigue detectando (AC10), porque el área temporal no sobrevive a un cierre (J7). |
| U3 | **Lecturas sin esperarse entre sí**: encontrar la página que ve una transacción no hace esperar a una lectura por otras lecturas, de la misma transacción o de otras. Lo que G3 y J5 dicen de las esperas entre lecturas, escritura y traslado no cambia. Abrir y cerrar una transacción de lectura no cambia. |
| U4 | **Sin cambios visibles**: como S4. Lo que se lee y lo que se confirma es lo mismo que hoy. |
| U5 | **Rendimiento**: como S5, sobre W12 (que debe mejorar en las dos plataformas) y con al menos W2, W7, W10, W12, W13, W14 y W19 medidas, ninguna de las cuales debe empeorar. Las ejecuciones completas de referencia son las del 2026-11-07 tras la reutilización de recursos de escritura (`docs/benchmark/2026-11-07-*-write-reuse.md`). |

### Estado de partida

Verificado el 2026-11-08 en el código, en los perfiles de la investigación de ese día, en los informes del banco y
en el plan de rendimiento:

| Aspecto | Situación |
|---|---|
| Organización de las versiones | Una lectura que encuentra la versión de una página en el área temporal la entrega sin código de integridad, y los árboles comprueban su organización en cada lectura (C3), también en las escrituras (S1, AC-S7). |
| Vida de las versiones | Una versión publicada no se vuelve a escribir mientras está en el área temporal. Sale de ella cuando ninguna instantánea abierta puede verla; el traslado la escribe en el archivo de datos sin sacarla mientras alguna instantánea pueda verla. |
| Búsqueda de versiones | Mientras el área temporal tiene versiones, cada lectura de una página, sea o no una versión, pasa por una espera exclusiva que comparten todas las lecturas, la escritura al publicar y el traslado. |
| W12 | Cuatro hilos de lectura y una escritura que confirma lotes de 100 entradas. Razón con Voron: 0,37 en Windows y 0,63 en Linux (`docs/benchmark/2026-11-07-*-write-reuse.md`). |
| Coste en las escrituras | La comprobación de organización entera es el 15 % de W2 y el 22 % de W19 en Linux; la parte que corresponde a las versiones del área temporal no se midió (diagnóstico del 2026-11-07, candidato 2). |

## Actores

- **Desarrollador integrador**: lee y escribe como hasta ahora; no cambia nada de lo que usa.
- **Mantenedor**: ejecuta las pruebas y las ejecuciones de una sola carga y completas de U5 en Windows y en Linux.

## Flujo

1. Una escritura confirma: sus páginas quedan como versiones en el área temporal.
2. La primera lectura de un nodo desde una de esas versiones, de cualquier transacción, comprueba su organización;
   las lecturas siguientes de esa versión no la repiten (U1).
3. Las lecturas de varias transacciones encuentran sus páginas a la vez, sin esperarse entre sí (U3).
4. Cuando la versión sale del área temporal, sus lecturas pasan al archivo de datos como hoy (C2, C3).

## Validaciones y mensajes

| Situación | Resultado |
|---|---|
| Lectura de una versión con la organización dañada | Falla con la clase de daño y el número de su página, en cada lectura (U1). |
| Escritura que leyó una versión dañada | No puede confirmar (T11). |

No hay situaciones, clases de error ni datos de error nuevos. Lo que se deja de detectar está en U2.

## Salida

Nada de lo que devuelve una operación cambia de forma, y la versión de formato sigue siendo `9` (U4).

## Reutilización

- La comprobación de organización de los nodos, sin cambios en lo que comprueba.
- Las reglas del traslado y de la vida de las versiones (J5, J7, T14, T16), de las que depende que una versión no
  cambie mientras está recordada.
- Las ejecuciones de una sola carga y las completas del plan de rendimiento, para U5.

## Fuera de alcance

- Dejar de comprobar la organización de las versiones del área temporal, como hace Voron.
- Comprobar la integridad de las versiones del área temporal (I3).
- Cambiar cómo se comprueban los nodos que modifica la propia escritura (S1) o las páginas del archivo de datos (C2,
  I2, S2).
- Recordar las demás comprobaciones de la lectura de un nodo (C1).
- Acotar el tamaño de lo que U1 recuerda como C5.
- Que las lecturas no esperen a la escritura al publicar ni al traslado (G3, J5), y las esperas al abrir o cerrar una
  transacción (U3).
- Las páginas que no son nodos: el mapa de espacio libre, los valores fuera de su hoja y el encabezado (C3).
- Cambiar qué comprueban la apertura, la consulta de información del almacén, el backup, la compactación, la
  migración, la verificación, el diario o la recuperación.
- La compresión de páginas hoja (5.6) y el cifrado (módulo 9): cada uno decide si usa lo que U1 recuerda.
- Detectar que algo que no es el motor cambia una versión del área temporal con el almacén abierto (U2, P16).
- Las demás optimizaciones del plan de rendimiento, entre ellas escribir solo los cambios de cada página en el diario
  (O8).
- Una opción pública para desactivar este cambio, y estadísticas de lo comprobado en la información del almacén.

## Dejado a la implementación

- Dónde y con qué forma se guarda lo que U1 recuerda de cada versión, dentro de lo que U1 permite.
- Cómo se encuentran las páginas sin que las lecturas se esperen entre sí (U3).
- Cómo cuentan las pruebas las comprobaciones de organización y cómo alteran los bytes de una versión del área
  temporal, sin una opción pública.
- Las cargas de U5 además de las que nombra, y las repeticiones.

## Criterios de aceptación

Valen los de los ascendientes, que deben seguir pasando sin cambios salvo los que este PRD sustituye (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)), incluidos AC10 del padre raíz y AC-C5. Cada criterio se
comprueba en un almacén en disco y en modo temporal, y en un árbol normal, en uno de tamaño fijo y en el subárbol de
una clave de un árbol multivalor. Las pruebas usan el conocimiento de la implementación para contar las
comprobaciones, para mantener versiones en el área temporal sin trasladarlas y para alterar bytes. Las cuentas se
hacen con un solo hilo.

1. **AC-U1** — Dada una versión confirmada de un nodo que está en el área temporal, cuando una transacción de lectura
   la lee dos veces, otra transacción de lectura la lee después y una transacción de escritura la lee dos veces sin
   cambiarla, entonces todas devuelven lo mismo y su organización se comprueba solo en la primera de esas lecturas, y
   su integridad en ninguna (U1). Este criterio sustituye a AC-S7.
2. **AC-U2** — Dada una versión del área temporal con la organización dañada (AC-C5), cuando una transacción de
   lectura la lee dos veces y una de escritura la lee después, entonces cada lectura falla con la clase de daño y el
   número de esa página, y la escritura no puede confirmar (U1, T11).
3. **AC-U3** — Dada una versión recordada, cuando una escritura confirma una versión más nueva de la misma página
   mientras una transacción de lectura abierta antes sigue leyendo la anterior, entonces esa transacción sigue viendo
   el contenido anterior y su organización no se vuelve a comprobar, y la primera lectura de la versión nueva la
   comprueba una vez (U1, G3).
4. **AC-U4** — Dada una versión recordada cuya página se libera, cuando una escritura posterior la reutiliza para otro
   nodo y lo confirma, entonces la primera lectura de ese nodo comprueba su organización (U1).
5. **AC-U5** — Dada una versión recordada, cuando una escritura la copia para modificarla y la lee, entonces su
   organización se comprueba en esa lectura (U1, S1).
6. **AC-U6** — Dada una versión recordada con sitio para recordar en la memoria compartida y un contenido que esa
   memoria no recuerda, cuando la versión sale del área temporal y una transacción de lectura nueva lee el nodo del
   archivo de datos dos veces, entonces su organización se comprueba en la primera de esas lecturas y no en la segunda
   (U1, C2).
7. **AC-U7** — Dados cuatro hilos con transacciones de lectura que leen, al menos mil veces cada uno, nodos cuyas
   versiones están en el área temporal, una de ellas con la organización dañada, mientras una escritura confirma
   versiones nuevas de esas páginas y el traslado las escribe, entonces cada lectura devuelve los datos de su
   instantánea y la versión dañada falla en todas sus lecturas con la clase de daño; el número de comprobaciones no se
   cuenta (U1, U3).
8. **AC-U8** — Dadas las ejecuciones de una sola carga de U5, cuando se comparan, entonces W12 mejora en las dos
   plataformas y ninguna otra carga medida empeora; y el informe de cierre incluye esas comparaciones y las de las
   ejecuciones completas con las de referencia (U5). U3 se comprueba con este criterio y con AC-U7.

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes, también en sus
traducciones (`*.prd.en.md`):

- Comprobación de organización recordada: C3, el paso 4 del Flujo y el punto de Fuera de alcance «recordar su
  organización» de las versiones del área temporal pasan a decir que cada versión se comprueba en su primera lectura y
  se recuerda mientras sigue en el área temporal (U1); C1 y C4 valen para la memoria compartida, y C5 no vale para la
  de U1; el riesgo residual de C7 se amplía con U2. AC-C5 no cambia.
- Comprobaciones de la escritura: la última frase de S1 y el punto de Fuera de alcance «Cambiar cómo se comprueban
  las versiones confirmadas del área temporal» dejan de valer (U1); el riesgo residual de S3 se amplía con U2. AC-S7
  se sustituye por AC-U1.

## Referencias

- Padre: [pager-writer-checks.prd.md](pager-writer-checks.prd.md) v1.0 (S1, S3, S4, S5, AC-S7)
- Comprobación de organización recordada: [trees-node-check-cache.prd.md](trees-node-check-cache.prd.md) (C1–C5, C7,
  AC-C5)
- Integridad recordada: [pager-integrity-once.prd.md](pager-integrity-once.prd.md) (I2, I3)
- Pager: [pager.prd.md](pager.prd.md) v1.0 (P10, P12, P16)
- Raíz: [krummi.prd.md](krummi.prd.md) v1.1 (G3, G6, AC10)
- Relacionados: [transactions.prd.md](transactions.prd.md) (T11, T14, T16), [journal.prd.md](journal.prd.md) (J5, J7),
  [benchmark.prd.md](benchmark.prd.md) (K7, K13)
- Plan de rendimiento: [docs/performance/plan.md](../../performance/plan.md), diagnóstico de las escrituras del
  2026-11-07, candidato 2
