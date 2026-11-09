# Comprobación de los nodos de la escritura al confirmar — PRD

**Versión**: 1.0
**Fecha**: 2026-11-09
**Documento padre**: [Comprobaciones de la escritura — PRD](pager-writer-checks.prd.md) (v1.0)

## Objetivo y contexto

Hacer más baratas las transacciones de escritura que cambian muchas veces los mismos nodos. Hoy, la primera lectura
de un nodo después de cada cambio comprueba entera su organización (S1 de las comprobaciones de la escritura): una
hoja que recibe treinta inserciones en una transacción se comprueba treinta veces. Con este cambio, la escritura usa
sin comprobar los nodos que ella misma cambió, y comprueba cada uno una sola vez, al confirmar, antes de su punto sin
retorno.

**Por qué**: en el banco comparativo (módulo 6), `carga-masiva` (W3) es el único umbral que Krummi incumple en Linux,
con 0,78 veces Voron, y en Windows queda en 0,82 (informes del banco del 2026-11-08). En el diagnóstico del margen
del 2026-11-09 del plan de rendimiento, esa comprobación se lleva el 33 % de W3 en Windows y el 39 % en Linux: W3
guarda un millón de claves al azar en una sola escritura, unas treinta por hoja, y cada inserción vuelve a comprobar
su hoja. Las cargas en orden (W1) repiten el mismo patrón. Según el código de Voron que revisó la investigación del
2026-11-09, Voron no comprueba la organización de ninguna página en producción (su comprobación de las páginas del
árbol, `TreePage.DebugValidate`, solo existe en las compilaciones de depuración).

**Por qué ahora**: es la propuesta W3 b) del diagnóstico del margen del 2026-11-09, que el usuario autorizó con las
recomendaciones del agente.

Este PRD es un cambio de las comprobaciones de la escritura. Hereda todo lo del padre y sus ascendientes; aquí solo
cambia cuándo se comprueba la organización de un nodo que cambió la escritura. Lo que cambia reglas anteriores está
declarado en sus decisiones y anotado en los documentos afectados (ver [Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

### Decisiones de producto

| ID | Decisión |
|---|---|
| H1 | **Lectura de un nodo que cambió la escritura** (cambia S1 de las comprobaciones de la escritura y la frase de U1 sobre la copia de una versión). Una transacción de escritura cambia un nodo cuando lo crea o cuando obtiene una copia suya para modificarla, como define S1. Sus lecturas de un nodo que cambió no comprueban su organización: lo usan tal como lo dejaron sus cambios. Las demás comprobaciones de cada lectura de un nodo (que la página pueda contener datos, su tipo, su nivel y la coherencia de cada entrada que se usa, C1) siguen en cada lectura. La página de la que se obtiene la copia se comprueba como hoy cuando se lee como nodo (C1–C3, U1); la copia, al confirmar (H2). |
| H2 | **Comprobación al confirmar** (cambia la penúltima frase de S3 y el punto de Fuera de alcance «Comprobar la organización al sellar o al confirmar» del padre, y añade un fallo a los que E2 hace antes del punto sin retorno). Al confirmar una escritura con cambios, en los dos modos de confirmación (E5) y también en modo temporal, la transacción comprueba una vez la organización de cada nodo que cambió y que conserva al confirmar, es decir, de cada página de nodo que la confirmación escribe; un nodo ocupa una página. Uno que cambió y después liberó no se comprueba, porque liberar descarta sus cambios (F8). Las páginas que cambió sin ser nodos (el mapa de espacio libre, los valores fuera de su hoja y el encabezado) no se comprueban. La comprobación va después de los fallos de T11 y antes de todo lo demás que E2 hace antes del punto sin retorno, también antes de guardar las páginas en el área temporal en modo temporal, así que un nodo inconsistente prevalece sobre cualquier otro fallo de la confirmación: si una operación anterior de la transacción ya falló, la confirmación falla con la clase de ese fallo, como hoy (T11), sin comprobar nada. Si un nodo no pasa, la confirmación falla con la clase de daño y los datos de un error de daño (D8, G6), con el número de la página de menor número que no pasa; no aplica nada de la transacción, el archivo de datos conserva lo que haya crecido (T10, P11), el almacén sigue aceptando escrituras, las confirmaciones anteriores que ya estaban en la cola del diario no se ven afectadas y el turno pasa a la siguiente petición de escritura, como en cualquier confirmación que falla antes de su punto sin retorno (T11, E2). Lo que se comprueba al confirmar no se consulta ni se guarda en la memoria compartida de nodos recordados (C2), como S1 decía de los nodos que cambia la escritura. Un abandono, y una confirmación sin nodos cambiados, no comprueban nada. |
| H3 | **Garantía resultante** (amplía la excepción de G6 del padre raíz y el riesgo residual de S3 del padre). Ningún nodo cuya organización es inconsistente llega a confirmarse por una escritura, también los que cambió y no volvió a leer, que hoy se sellan sin comprobar. Cambia lo siguiente: los nodos que cambia una escritura salen de nodos que el árbol leyó y comprobó para llegar a ellos, así que solo un fallo del propio motor, o del hardware o de la memoria (P16), puede dejarlos inconsistentes; en ese caso, una operación posterior de la misma transacción que lee ese nodo puede devolver datos que no son los escritos, fallar con otra clase o terminar con un pánico en lugar de un error de daño, y la confirmación, si llega, falla sin aplicar nada. Un cambio de bytes que no rompe la organización se sigue sellando, como dice S3. Se acepta como riesgo residual, como en Voron, que tampoco comprueba lo que confirma. |
| H4 | **Rendimiento**. Las ejecuciones de una sola carga se hacen antes y después del cambio, en la misma sesión, en dos rondas que alternan el orden, en Windows y en Linux, con el plan de energía del banco que describe el método de medición del plan de rendimiento. Una carga mejora cuando su mediana después supera a la de antes en las dos rondas de una plataforma. Una carga que queda por debajo en las dos rondas se mide en dos rondas nuevas, y empeora si también queda por debajo en las dos. El cambio se acepta cuando W1 y W3 mejoran en las dos plataformas y ninguna de las cargas de escritura W1–W6, W13–W15, W17, W19 y W21 empeora. Una carga que queda por encima en una ronda y por debajo en la otra no mejora ni empeora. Si W1 o W3 no mejoran en alguna plataforma, o alguna carga empeora, el informe de cierre lo dice y el usuario decide; no cumplir el umbral (K7) no impide cerrar el cambio. |

### Estado de partida

Verificado el 2026-11-09 en el código y en los perfiles del diagnóstico del margen:

| Aspecto | Situación |
|---|---|
| Comprobación de los nodos cambiados | La primera lectura de un nodo después de cada cambio lo comprueba entero (S1). |
| Confirmación | No comprueba la organización: un nodo cambiado que no se vuelve a leer se sella sin comprobar (S3). |
| Coste | La comprobación de organización es el 33 % de W3 en Windows y el 39 % en Linux, casi toda de las hojas que cambian en cada inserción. |
| Voron (referencia) | Comprueba el código de integridad de cada página del archivo una vez por proceso y no comprueba la organización de ninguna página en producción. |

## Actores

- **Desarrollador integrador**: lee y escribe como hasta ahora; no cambia nada de lo que usa.
- **Mantenedor**: ejecuta las pruebas y las ejecuciones de una sola carga de H4 en Windows y en Linux.

## Flujo

1. Una transacción de escritura lee los nodos que no cambió comprobándolos como hoy, y los que cambió sin comprobar
   su organización (H1).
2. Al confirmar, después de los fallos de T11, comprueba cada nodo que cambió y conserva; si todos pasan, confirma
   como hoy (H2).
3. Si alguno no pasa, la confirmación falla con la clase de daño y el número de la página de menor número, sin aplicar
   nada (H2).

## Validaciones y mensajes

La confirmación de una escritura con un nodo inconsistente falla con la clase de daño, con los datos de un error de
daño y el número de la página de menor número que no pasa (H2). No hay mensajes nuevos.

## Salida

Nada de lo que devuelve una operación cambia de forma, y la versión de formato sigue siendo `10`.

## Reutilización

- La comprobación de organización de cada formato de nodo, la de los nodos con entradas de tamaño variable y la de
  los de tamaño fijo, sin cambios en lo que acepta.
- Las ejecuciones de una sola carga del plan de rendimiento, para H4.

## Fuera de alcance

- Dejar de comprobar los nodos que cambia la escritura, también al confirmar, como hace Voron.
- Repetir al confirmar las demás comprobaciones de la lectura de un nodo (C1): que la página pueda contener datos, su
  tipo, su nivel y la coherencia de sus entradas. Un nodo cambiado que no se vuelve a leer las pasa solo si su
  organización las incluye.
- Comprobar al confirmar las páginas que no son nodos, o la integridad de lo que se sella, que se calcula sobre los
  bytes de cada página como hoy.
- Comprobar en un abandono, en una confirmación sin cambios, en hacer durable, en el cierre, el backup, la
  compactación o la verificación.
- Reintentar una confirmación que falla (T11).
- Cambiar cómo se comprueban los nodos del archivo de datos, de la memoria del almacén o del área temporal.
- Límites de tiempo o de memoria de la comprobación de una transacción muy grande.
- Una opción pública para volver a comprobar en cada lectura.

## Dejado a la implementación

- Cómo sabe la confirmación qué páginas cambiadas son nodos y de qué formato.
- Cómo cuentan y provocan las pruebas las comprobaciones al confirmar, sin una opción pública.
- Las repeticiones de las ejecuciones de H4.

## Criterios de aceptación

Valen los de los ascendientes, que deben seguir pasando sin cambios salvo los que este PRD sustituye (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)). AC-H1 a AC-H6 se comprueban en un almacén en disco y en modo
temporal. Las pruebas usan el conocimiento de la implementación para contar las comprobaciones, para alterar bytes y
para liberar páginas.

1. **AC-H1** — Dada una transacción de escritura que crea un árbol normal, uno de tamaño fijo y uno multivalor, y
   guarda claves en orden en cada uno y en el subárbol de una clave del multivalor, con divisiones de hojas y de ramas,
   cuando cada guardado lee el camino, entonces ninguna lectura comprueba la organización de un nodo que la transacción
   cambió, y al confirmar se comprueba exactamente una vez cada página de nodo que la confirmación escribe, de esos
   árboles y del catálogo, aunque haya cambiado varias veces, incluidos los nodos que crea una división y no se
   vuelven a leer; y lo que se lee después de confirmar es lo que tiene un modelo de las claves y valores guardados
   (H1, H2). Este criterio sustituye a AC-S1.
2. **AC-H2** — Dado un nodo que una transacción de escritura cambió, cuando la prueba altera su organización y
   confirma sin que ninguna operación posterior lo lea, en modo durable y en modo diferido, entonces la confirmación
   falla con la clase de daño y el número de su página; otra transacción lee las claves y los valores de antes; y la
   siguiente escritura confirma (H2). Con dos nodos alterados, el error da el de menor número. Este criterio sustituye
   a AC-S2.
3. **AC-H3** — Dado un nodo que una transacción de escritura cambió en una página que ningún árbol referencia, cuando
   la prueba altera su organización y la transacción libera la página y confirma, entonces la confirmación no lo
   comprueba y confirma (H2, F8).
4. **AC-H4** — Dada una transacción de escritura con un nodo alterado como en AC-H2, sin que ninguna operación lo lea,
   y una operación posterior que falla con otra clase, cuando se confirma, entonces la confirmación falla con la
   clase de esa operación y no comprueba ningún nodo (T11, H2).
5. **AC-H5** — Dada una transacción de escritura que cambia nodos y se abandona, y otra que solo reserva y libera
   páginas y confirma, entonces ninguna comprueba un nodo (H2).
6. **AC-H6** — Dado un nodo que una transacción de escritura cambió, cuando la prueba cambia su nivel, y en otro caso
   su tipo, sin romper su organización, entonces la siguiente lectura de ese nodo en la transacción falla con la clase
   de daño y el número de su página (H1, C1).
7. **AC-H7** — Dadas las ejecuciones de una sola carga de H4, cuando se comparan, entonces W1 y W3 mejoran en las dos
   plataformas y ninguna de las cargas de escritura de H4 empeora; y el informe de cierre incluye esas comparaciones
   (H4).

## Anotaciones a otros PRD

Este PRD cambia reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Padre raíz, G6: la excepción de I8, ampliada por S3, se amplía con H3: dentro de una transacción de escritura, una
  lectura de un nodo que ella cambió puede devolver datos que no son los escritos, fallar con otra clase o terminar
  con un pánico si un fallo del motor, del hardware o de la memoria lo dejó inconsistente; la confirmación no aplica
  nada en ese caso.
- Comprobaciones de la escritura: S1 deja de valer para las lecturas de los nodos que cambió la escritura (H1) y su
  comprobación pasa a la confirmación (H2), también sus frases sobre una comprobación que falla y lo que recuerda; los
  pasos 2 y 3 del Flujo, la penúltima frase de S3 y el punto de Fuera de alcance «Comprobar la organización al sellar
  o al confirmar» dejan de valer (H2); el riesgo residual de S3 se amplía con el de H3. AC-S1 se sustituye por AC-H1 y
  AC-S2 por AC-H2; AC-S3 deja de aplicarse, porque no hay comprobación en la lectura. S4 y su sección de Validaciones
  y mensajes: la confirmación que falla con la clase de daño es una situación nueva de esa clase, que cubre G6.
- Comprobación recordada de las versiones del área temporal: en U1, la copia de una versión para modificarla se
  comprueba al confirmar (H2), no en su primera lectura; en U2, lo mismo para la copia de una versión alterada.
- Comprobación de organización recordada: C3, en lo que su anotación del 2026-11-06 dice de los nodos que modifica la
  escritura, pasa a decir que se comprueban una vez al confirmar (H2).
- Confirmación de grupo del diario: E2 añade a lo que la confirmación hace antes del punto sin retorno la comprobación
  de H2, después de los fallos de T11.

## Referencias

- Padre: [pager-writer-checks.prd.md](pager-writer-checks.prd.md) v1.0 (S1, S3, AC-S1–AC-S3)
- Raíz: [krummi.prd.md](krummi.prd.md) (G6, D8)
- Relacionados: [trees-node-check-cache.prd.md](trees-node-check-cache.prd.md) (C1–C3),
  [pager-version-checks.prd.md](pager-version-checks.prd.md) (U1, U2), [transactions.prd.md](transactions.prd.md)
  (T10, T11, T18), [journal-group-commit.prd.md](journal-group-commit.prd.md) (E2, E5),
  [free-space.prd.md](free-space.prd.md) (F8), [benchmark.prd.md](benchmark.prd.md) (K7)
- Plan de rendimiento: [docs/performance/plan.md](../../performance/plan.md), diagnóstico del margen del 2026-11-09
