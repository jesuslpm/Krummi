# Comprobación de los nodos de la escritura después de confirmar — PRD

**Versión**: 1.0
**Fecha**: 2026-11-09
**Documento padre**: [Comprobaciones de la escritura — PRD](pager-writer-checks.prd.md) (v1.0)

## Objetivo y contexto

Hacer más baratas las transacciones de escritura que cambian muchas veces los mismos nodos. Hoy, la primera lectura
de un nodo después de cada cambio comprueba entera su organización (S1 de las comprobaciones de la escritura): una
hoja que recibe treinta inserciones en una transacción se comprueba treinta veces. Con este cambio, la escritura no
comprueba la organización de los nodos que ella misma cambió, ni al leerlos ni al confirmar: los confirma como están,
y las transacciones siguientes comprueban cada uno en su primera lectura, como ya hacen hoy con los nodos que una
escritura cambia y no vuelve a leer.

**Por qué**: en el banco comparativo (módulo 6), `carga-masiva` (W3) es el único umbral que Krummi incumple en Linux,
con 0,78 veces Voron, y en Windows queda en 0,82 (informes del banco del 2026-11-08). En el diagnóstico del margen
del 2026-11-09 del plan de rendimiento, esa comprobación se lleva el 33 % de W3 en Windows y el 39 % en Linux: W3
guarda un millón de claves al azar en una sola escritura, unas treinta por hoja, y cada inserción vuelve a comprobar
su hoja. Las cargas en orden (W1) repiten el mismo patrón. Según el código de Voron que revisó la investigación del
2026-11-09, Voron no comprueba la organización de ninguna página en producción (su comprobación de las páginas del
árbol, `TreePage.DebugValidate`, solo existe en las compilaciones de depuración).

Se midió también comprobar cada nodo cambiado una sola vez, al confirmar: W3 mejoraba, pero `carga-al-azar` (W2)
bajaba entre un 5 % y un 8 % en Windows, porque cada una de sus confirmaciones cambia unas mil hojas que nadie vuelve
a leer y que hoy no se comprueban.

**Por qué ahora**: es la propuesta W3 b) del diagnóstico del margen del 2026-11-09, que el usuario autorizó con las
recomendaciones del agente.

Este PRD es un cambio de las comprobaciones de la escritura. Hereda todo lo del padre y sus ascendientes; aquí solo
cambia cuándo se comprueba la organización de un nodo que cambió la escritura. Lo que cambia reglas anteriores está
declarado en sus decisiones y anotado en los documentos afectados (ver [Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

### Decisiones de producto

| ID | Decisión |
|---|---|
| H1 | **Los nodos que cambia la escritura** (cambia S1 de las comprobaciones de la escritura y la frase de U1 sobre la copia de una versión). Vale solo para los nodos de los árboles. Una transacción de escritura cambia un nodo cuando lo crea o cuando obtiene una copia suya para modificarla, como define S1. La transacción no comprueba la organización de los nodos que cambió, ni cuando los lee ni al confirmar: los usa y los confirma tal como los dejaron sus cambios. Las demás comprobaciones de cada lectura de un nodo (que la página pueda contener datos, su tipo, su nivel y la coherencia de cada entrada que se usa, C1) siguen en cada lectura. La página de la que se obtiene la copia se comprueba como hoy cuando se lee como nodo (C1–C3, U1). |
| H2 | **Comprobación de lo confirmado**. Cada transacción posterior comprueba la organización de un nodo confirmado en su primera lectura de él, desde el área temporal (U1) o desde el archivo de datos o la memoria del almacén (C1, C2). La versión que publica la confirmación es nueva: nada de lo comprobado de la página de la que se copió vale para ella (U1, C1). Un nodo que no pasa hace fallar esa lectura, y cada lectura siguiente, con la clase de daño y el número de su página. |
| H3 | **Garantía resultante** (amplía la excepción de G6 del padre raíz y el riesgo residual de S3 del padre). Los nodos que cambia una escritura son nuevos o copias de nodos que el árbol leyó y comprobó para llegar a ellos, así que solo un fallo del propio motor, o del hardware o de la memoria (P16), puede dejarlos inconsistentes. En ese caso, una operación posterior de la misma escritura que lee ese nodo puede devolver datos que no son los escritos, fallar con otra clase o terminar con un pánico en lugar de un error de daño; la confirmación guarda el nodo inconsistente, y desde entonces cada lectura de él por otra transacción falla con la clase de daño (H2). Hoy pasa lo mismo con los nodos que la escritura cambia y no vuelve a leer (S3). Se acepta como riesgo residual, como en Voron, que no comprueba la organización de ninguna página. |
| H4 | **Rendimiento**. Las ejecuciones de una sola carga se hacen antes y después del cambio, en la misma sesión, en dos rondas que alternan el orden, en Windows y en Linux, con el plan de energía del banco que describe el método de medición del plan de rendimiento. Una carga mejora cuando su mediana después supera a la de antes en las dos rondas de una plataforma. Una carga que queda por debajo en las dos rondas se mide en dos rondas nuevas, y empeora si también queda por debajo en las dos. Una carga que queda por encima en una ronda y por debajo en la otra no mejora ni empeora. El cambio se acepta cuando W1 y W3 mejoran en las dos plataformas y ninguna de las cargas de escritura W1–W6, W13–W15, W17, W19 y W21 empeora. Si no se cumple, el informe de cierre lo dice y el usuario decide; no cumplir el umbral (K7) no impide cerrar el cambio. |

*Anotación del 2026-11-09 a H1 (decisión del usuario al cerrar la ejecución): en las compilaciones de depuración, la
escritura sí comprueba la organización de los nodos que cambió, en cada lectura, como el modo `VALIDATE` de Voron,
para que las pruebas del motor encuentren dentro de la misma escritura un nodo que el propio motor dejó
inconsistente; esa comprobación falla con la clase de daño y el número de la página, como las demás, y no cuenta
entre las comprobaciones que mide AC-H1. Las compilaciones de producción siguen H1 sin cambios, así que H3 y AC-H1 a
AC-H4 valen para ellas.*

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
2. Al confirmar, guarda los nodos que cambió como están (H1).
3. La primera lectura de cada nodo confirmado por otra transacción comprueba su organización (H2).

## Validaciones y mensajes

No hay situaciones ni mensajes nuevos. Lo que una escritura deja de detectar está en H3.

## Salida

Nada de lo que devuelve una operación cambia de forma, y la versión de formato sigue siendo `10`.

## Reutilización

- La comprobación de organización recordada de las versiones del área temporal (U1) y la de los nodos del almacén
  (C1, C2), sin cambios.
- Las ejecuciones de una sola carga del plan de rendimiento, para H4.

## Fuera de alcance

- Comprobar al confirmar los nodos que cambió la escritura (ver Objetivo y contexto).
- Cambiar las demás comprobaciones de la lectura de un nodo (C1): que la página pueda contener datos, su tipo, su
  nivel y la coherencia de sus entradas.
- Cambiar cómo se comprueban los nodos del archivo de datos, de la memoria del almacén o del área temporal.
- Detectar dentro de la escritura un nodo que ella dejó inconsistente, o evitar que lo confirme (H3).
- Una opción pública para volver a comprobar en cada lectura.

## Dejado a la implementación

- Cómo cuentan las pruebas las comprobaciones y cómo alteran los bytes de un nodo cambiado, sin una opción pública.
- Las repeticiones de las ejecuciones de H4.

## Criterios de aceptación

Valen los de los ascendientes, que deben seguir pasando sin cambios salvo los que este PRD sustituye (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)). AC-H1 a AC-H3 se comprueban en un almacén en disco y en modo
temporal. Las pruebas usan el conocimiento de la implementación para contar las comprobaciones, para alterar bytes y
para mantener versiones en el área temporal sin trasladarlas. AC-H4 es una medición, no una prueba.

1. **AC-H1** — Dada una transacción de escritura que crea un árbol normal, uno de tamaño fijo y uno multivalor, y
   guarda claves en orden en cada uno y en el subárbol de una clave del multivalor, con divisiones de hojas y de ramas,
   y otra que quita la mayoría de las claves de un árbol y fusiona sus nodos, cuando cada operación lee el camino,
   entonces ninguna lectura comprueba la organización de un nodo que la transacción cambió; lo que se lee después de
   confirmar es lo que tiene un modelo de las claves y valores guardados, y esa lectura comprueba al menos un nodo
   confirmado (H1, H2). Este criterio sustituye a AC-S1.
2. **AC-H2** — Dado un nodo cuyo contenido de antes del cambio está en la memoria compartida de nodos recordados, que
   una transacción de escritura cambia, cuando la prueba altera su organización y la transacción confirma sin que
   ninguna operación posterior lo lea, en modo durable y en modo diferido, entonces la confirmación pasa, y dos
   lecturas de ese nodo por otras transacciones, desde el área temporal y, después del traslado, desde el archivo de
   datos o la memoria del almacén, fallan con la clase de daño y el número de su página (H1, H2, H3). Este criterio
   sustituye a AC-S2.
3. **AC-H3** — Dado un nodo que una transacción de escritura cambió, cuando la prueba cambia su nivel, y en otro caso
   su tipo, sin romper su organización, entonces la siguiente lectura de ese nodo en la transacción falla con la clase
   de daño y el número de su página, y la transacción no puede confirmar (H1, C1, T11).
4. **AC-H4** — Dadas las ejecuciones de una sola carga de H4, cuando se comparan, entonces W1 y W3 mejoran en las dos
   plataformas y ninguna de las cargas de escritura de H4 empeora; y el informe de cierre incluye esas comparaciones
   (H4).

## Anotaciones a otros PRD

Este PRD cambia reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Padre raíz, G6: la excepción de I8, ampliada por S3, se amplía con H3: dentro de una transacción de escritura, una
  lectura de un nodo que ella cambió puede devolver datos que no son los escritos, fallar con otra clase o terminar
  con un pánico si un fallo del motor, del hardware o de la memoria lo dejó inconsistente; la confirmación lo guarda,
  y cada lectura de él por otra transacción falla con la clase de daño.
- Comprobaciones de la escritura: S1 deja de valer para los nodos que cambió la escritura, que ella no comprueba
  (H1), también sus frases sobre una comprobación que falla y lo que se recuerda; los pasos 2 y 3 del Flujo y el punto
  de Fuera de alcance «Dejar de comprobar la organización de los nodos que modifica la escritura, como hace Voron»
  dejan de valer; el riesgo residual de S3 se amplía con el de H3. AC-S1 se sustituye por AC-H1 y AC-S2 por AC-H2; AC-S3 deja de
  aplicarse, porque la escritura no comprueba sus nodos.
- Comprobación recordada de las versiones del área temporal: en U1 y U2, la copia de una versión que una escritura
  obtiene para modificarla no la comprueba esa escritura; la versión que confirma se comprueba en su primera lectura
  por otra transacción (U1). AC-U5 pasa a decir que la lectura de la copia por la escritura no la comprueba, y que la
  primera lectura de la versión confirmada por otra transacción sí.
- Comprobación de organización recordada: C3, en lo que su anotación del 2026-11-06 dice de los nodos que modifica la
  escritura, pasa a decir que la escritura no los comprueba (H1).

## Referencias

- Padre: [pager-writer-checks.prd.md](pager-writer-checks.prd.md) v1.0 (S1, S3, AC-S1–AC-S3)
- Raíz: [krummi.prd.md](krummi.prd.md) (G6)
- Relacionados: [trees-node-check-cache.prd.md](trees-node-check-cache.prd.md) (C1–C3),
  [pager-version-checks.prd.md](pager-version-checks.prd.md) (U1, U2), [benchmark.prd.md](benchmark.prd.md) (K7)
- Plan de rendimiento: [docs/performance/plan.md](../../performance/plan.md), diagnóstico del margen del 2026-11-09
