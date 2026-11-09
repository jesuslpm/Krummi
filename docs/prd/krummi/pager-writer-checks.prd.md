# Comprobaciones de la escritura — PRD

**Versión**: 1.0
**Fecha**: 2026-11-06
**Documento padre**: [Integridad recordada de las páginas — PRD](pager-integrity-once.prd.md) (v1.0)

## Objetivo y contexto

Hacer más baratas las transacciones de escritura dejando de repetir dos comprobaciones que hoy hacen en cada lectura:

- la **comprobación de organización** (C1 de la comprobación de organización recordada) de los nodos que la propia
  escritura modificó, que se repite entera cada vez que la escritura vuelve a leerlos;
- la **comprobación de integridad** (G6) de las páginas del archivo de datos o, en modo temporal, de la memoria del
  almacén, que la escritura calcula en cada lectura aunque la página ya esté recordada (I3).

Con este cambio, un nodo que la escritura modificó se comprueba la primera vez que se lee después de cada cambio, y
las transacciones de escritura usan las páginas recordadas como ya hacen las de lectura (I2), igual que Voron.

**Por qué**: en el diagnóstico del plan de rendimiento del 2026-11-06, la comprobación de organización de los nodos
modificados se lleva el 57–63 % del tiempo de las cargas en orden (W1, W3). En esas cargas, cada inserción lee todos
los nodos de su camino, y la raíz y las ramas del final quedan modificadas desde las primeras inserciones de la
transacción, así que se comprueban enteras en cada inserción aunque solo cambien al dividirse un nodo de debajo. La
hoja donde se inserta cambia en cada inserción y se seguirá comprobando una vez por inserción. La comprobación de
integridad en la escritura se lleva el 5,7–10,2 % de las cargas al azar (W2, W14, W15, W21), porque las ramas del
camino se calculan de nuevo en cada inserción de la transacción. Krummi incumple el umbral de todas esas cargas en el
banco comparativo (módulo 6).

**Por qué ahora**: O1 a O4 resolvieron las lecturas. Las escrituras siguen entre 0,24 y 0,76 veces Voron, y estas
dos comprobaciones son los mayores costes que se quitan sin cambiar el formato.

Este PRD es un cambio de la integridad recordada (O6 y O7 del plan de rendimiento). Hereda todo lo del padre y sus
ascendientes; aquí solo se añade cuándo comprueba la escritura. Lo que cambia reglas anteriores está declarado en sus
decisiones y anotado en los documentos afectados (ver [Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

### Decisiones de producto

| ID | Decisión |
|---|---|
| S1 | **Organización de los nodos que modifica la escritura** (cambia C3 y precisa C1 y C4 de la comprobación de organización recordada). Una transacción de escritura cambia un nodo cuando lo crea (una división, una raíz nueva) o cuando obtiene una copia suya para modificarla, y cada escritura posterior de sus bytes en la transacción es otro cambio. La primera lectura de un nodo después de un cambio comprueba su organización, como hoy. Las lecturas siguientes del nodo en la misma transacción lo usan sin comprobarla, hasta el próximo cambio. Una comprobación que falla no se recuerda: la operación falla con la clase de daño y el número de la página, la transacción no puede confirmar (T11) y cada lectura siguiente de ese nodo vuelve a fallar. Lo recordado es solo de esa transacción: empieza vacío, ninguna otra transacción lo ve y se descarta al confirmarla o abandonarla. Un nodo que la transacción cambió nunca consulta ni alimenta la memoria compartida de nodos recordados (C2), aunque conserve el código de integridad de la página de la que se copió, y lo que S1 recuerda de él no pasa a esa memoria al confirmar. Recordar no reserva memoria que pueda faltar, así que no hace fallar ninguna operación. Las versiones confirmadas que todavía están en el área temporal (J7, T16), que una escritura lee sin haberlas cambiado, siguen comprobando su organización en cada lectura, como dice C3. |
| S2 | **Integridad de lo que lee la escritura** (cambia I3 de la integridad recordada). Las transacciones de escritura usan las páginas recordadas como las de lectura (I2) en toda lectura de un nodo, de una página del mapa de espacio libre (F9) o de un valor guardado fuera de su hoja, cuando la versión que leen está en el archivo de datos o, en modo temporal, en la memoria del almacén: al bajar por un árbol, al copiar una página para modificarla y al leer las páginas y los valores que pasan a otras páginas (divisiones, fusiones, valores que se mueven). Si está recordada, se usa sin calcular su código. Si no, se comprueba: si falla, la operación falla con la clase de daño y el número de la página, la transacción no puede confirmar (T11), nada se recuerda y cada lectura siguiente vuelve a fallar; si pasa, desde ese momento queda recordada para todas las transacciones (I1), también si la escritura se abandona después. Las páginas que la escritura ya cambió se leen de su copia, sin comprobación de integridad, como hoy. Las versiones del área temporal siguen sin comprobación de integridad. La apertura y la consulta de información del almacén siguen comprobando como hoy (I3). La versión que una escritura lee del archivo de datos es la última confirmada, así que el traslado no la reescribe mientras la transacción está abierta (J5, T14); I4 sigue olvidando cada página antes de que el traslado la reescriba. |
| S3 | **Garantía resultante** (amplía la excepción de I8 a G6 del padre raíz y el riesgo residual de C7, anotados en ellos). Se mantiene que toda página del archivo de datos o de la memoria del almacén se comprueba la primera vez que la lee cualquier transacción en cada apertura y la primera vez después de que el motor la reescriba (I4), así que todo daño hecho con el almacén cerrado se sigue detectando (AC10), también por una escritura. Cambia lo siguiente. Si los bytes de una página recordada cambian sin que el motor los escriba (un fallo del hardware o de la memoria, o un programa ajeno, que P16 deja fuera de alcance), una transacción de escritura que la lee puede devolver datos que no son los escritos o terminar con un pánico en lugar de un error de daño, como una de lectura (I8). Si además la modifica y confirma, la sella con un código válido en una versión nueva y el daño queda guardado: ninguna lectura posterior lo detecta como daño, tampoco tras reabrir el almacén. El residuo de los valores fuera de su hoja de la anotación a I1 vale también para las escrituras. Y si los bytes de un nodo que la escritura cambió y ya comprobó cambian en memoria antes de su próximo cambio, la escritura no lo detecta. Como hoy, un nodo que la escritura cambia y no vuelve a leer antes de confirmar se sella sin comprobar su organización. Se acepta como riesgo residual, como en Voron: afecta a lo que el motor guarda solo ante fallos que el motor no causa. |
| S4 | **Sin cambios visibles**: no hay situaciones, clases de error, datos de error ni opciones nuevas, el formato no cambia (versión `9`) y la información del almacén (P12) no añade datos. Lo que recuerda S1 dura lo que la transacción y no cuenta para el tamaño máximo (P10). |
| S5 | **Rendimiento** (aceptación como paso intermedio, según la anotación del 2026-11-06 a K13). Las ejecuciones de una sola carga se hacen antes y después del cambio, en la misma sesión, en dos rondas que alternan el orden, en Windows y en Linux. Una carga mejora cuando su mediana después supera a la de antes en las dos rondas de una plataforma, y empeora cuando queda por debajo en las dos. El cambio se acepta cuando W1 y W3 (S1) y W2, W14, W15 y W21 (S2) mejoran en las dos plataformas y ninguna otra carga de escritura medida empeora. Las ejecuciones completas del banco se hacen al final de la ejecución que implementa este PRD junto con O5, y miden los dos cambios juntos frente a las del 2026-11-06. Una carga cuya razón con Voron baja más de 0,05 entre ellas cuenta como peor solo si sus ejecuciones de una sola carga contra el commit anterior a esa ejecución confirman la bajada (anotación del 2026-11-06 a AC-I12). No cumplir el umbral (K7) no impide cerrar el cambio; lo que falte sigue el plan de rendimiento. |

*Anotaciones del 2026-11-08 desde la comprobación recordada de las versiones del área temporal
([pager-version-checks.prd.md](pager-version-checks.prd.md)):*

- *S1, su última frase, y el punto de Fuera de alcance «Cambiar cómo se comprueban las versiones confirmadas del
  área temporal»: dejan de valer; esas versiones se comprueban en su primera lectura (decisión U1).*
- *S3: el riesgo residual se amplía con el de U2. AC-S7 se sustituye por AC-U1.*

*Anotaciones del 2026-11-09 desde la comprobación de los nodos de la escritura después de confirmar
([pager-commit-checks.prd.md](pager-commit-checks.prd.md)):*

- *S1 deja de valer para los nodos que cambió la escritura, que ella no comprueba ni al leerlos ni al confirmar
  (decisión H1), también sus frases sobre una comprobación que falla y lo que se recuerda; la primera lectura de
  otra transacción comprueba lo que confirmó (decisión H2). Los pasos 2 y 3 del Flujo y el punto de Fuera de alcance
  «Dejar de comprobar la organización de los nodos que modifica la escritura, como hace Voron» dejan de valer; el
  riesgo residual de S3 se amplía con el de H3.*
- *AC-S1 se sustituye por AC-H1 y AC-S2 por AC-H2; AC-S3 deja de aplicarse.*

### Estado de partida

Verificado el 2026-11-06 en el código y en el perfil del plan de rendimiento:

| Aspecto | Situación |
|---|---|
| Organización de los nodos modificados | Un nodo que la escritura modificó no tiene un código de integridad comprobado, así que su organización se comprueba entera en cada lectura y no usa la memoria compartida (C3). Una inserción lee una vez cada nodo de su camino, de la raíz a la hoja, con esa comprobación. Es el 57–63 % de W1 y W3 en Linux. |
| Organización de los nodos del almacén | Se recuerda por número de página y código (C2) y no se repite. |
| Integridad en la escritura | Cada lectura del archivo de datos calcula el código, aunque la página esté recordada (I3). Una página sin modificar del camino se calcula en cada inserción, y una hoja varias veces en la inserción que la modifica. Es el 5,7–10,2 % de W2, W14, W15 y W21 en Linux. |
| Integridad en la lectura | Las transacciones de lectura usan las páginas recordadas (I2) desde O3. |
| Voron (referencia) | Según el código de RavenDB 7.2.7 que cita el plan de rendimiento, comprueba cada página del archivo una vez por proceso, también cuando la lee una escritura, y no comprueba la organización de las páginas que modifica. |

## Actores

- **Desarrollador integrador**: lee y escribe como hasta ahora; no cambia nada de lo que usa.
- **Mantenedor**: ejecuta las pruebas, las ejecuciones de una sola carga y las completas de S5 en Windows y en Linux.

## Flujo

1. Una transacción de escritura lee páginas del almacén usando lo recordado (S2).
2. Cada nodo que cambia se comprueba en su primera lectura después del cambio (S1).
3. Al confirmar o abandonar, lo que recordó de sus nodos se descarta; las páginas del almacén que recordó siguen
   recordadas (I1).

## Validaciones y mensajes

No hay situaciones, clases de error ni datos de error nuevos. Lo que una escritura deja de detectar está en S3.

## Salida

Nada de lo que devuelve una operación cambia de forma, y la versión de formato sigue siendo `9` (S4).

## Reutilización

- La memoria de páginas recordadas de la integridad recordada (I1, I4–I7), sin cambios en lo que recuerda ni en cuándo
  olvida.
- La comprobación de organización y la memoria compartida de nodos recordados (C1, C2, C4–C6), sin cambios para los
  nodos del almacén.
- Las ejecuciones de una sola carga y las completas del plan de rendimiento, para S5.

## Fuera de alcance

- Dejar de comprobar la organización de los nodos que modifica la escritura, como hace Voron.
- Comprobar la organización al sellar o al confirmar: un nodo que se cambia y no se vuelve a leer se sella sin
  comprobarla, como hoy (S3).
- Pasar a la memoria compartida (C2) lo que S1 recuerda de un nodo cambiado.
- Comprobar la integridad una vez por transacción de escritura en lugar de usar lo recordado.
- Cambiar cómo se comprueban las versiones confirmadas del área temporal: organización en cada lectura y sin
  integridad (C3, I3).
- Comprobar la organización de las páginas que no son nodos: el mapa de espacio libre, los valores fuera de su hoja y
  el encabezado (C3).
- Cambiar qué comprueban la apertura, la consulta de información del almacén, el backup, la compactación, la
  migración, la verificación, el diario o la recuperación.
- Detectar que otro programa, o un fallo del hardware o de la memoria, cambia una página recordada con el almacén
  abierto, o evitar que una escritura selle ese cambio (S3, P16).
- Las demás optimizaciones del plan de rendimiento (O5, O8).
- Una opción pública para desactivar este cambio, y estadísticas de lo comprobado en la información del almacén.

## Dejado a la implementación

- La forma de lo que recuerda S1 y cómo se olvida en cada cambio.
- Cómo cuentan las pruebas las comprobaciones de organización y de integridad, y cómo alteran los bytes de un nodo
  modificado o de una página recordada, sin una opción pública.
- Las cargas exactas de las ejecuciones de una sola carga de S5 además de las que nombra, y sus repeticiones.

## Criterios de aceptación

Valen los de los ascendientes, que deben seguir pasando sin cambios salvo los que este PRD sustituye o precisa (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)), incluido AC10 del padre raíz. Cada criterio se comprueba en un
almacén en disco y en modo temporal, salvo las partes que reabren el almacén, que son solo en disco. Las pruebas usan
el conocimiento de la implementación para contar las comprobaciones, para colocar páginas en el archivo de datos o en
el área temporal y para alterar bytes. Las cuentas se hacen con un solo hilo.

1. **AC-S1** — Dada una transacción de escritura que guarda claves en orden en un árbol normal, en uno de tamaño fijo y
   en el subárbol de una clave de un árbol multivalor, con divisiones de hojas y de ramas, cuando cada guardado lee el
   camino, entonces la organización de cada nodo cambiado se comprueba una vez después de cada cambio, incluidos los
   nodos que crea una división, y ninguna lectura sin un cambio previo la vuelve a comprobar; y lo que se lee y se
   confirma es lo mismo que hoy (S1).
2. **AC-S2** — Dado un nodo que una transacción de escritura acaba de cambiar, cuyo código de antes del cambio está en
   la memoria compartida de nodos recordados, cuando la prueba altera su organización antes de su primera lectura
   siguiente, entonces esa lectura falla con la clase de daño y el número de su página, una segunda lectura falla
   igual y la transacción no puede confirmar (S1, T11).
3. **AC-S3** — Dado un nodo que una transacción de escritura cambió y comprobó, cuando la transacción se confirma, y
   en otro caso se abandona, y otra transacción de escritura lo cambia y lo lee, entonces su organización se comprueba
   de nuevo en esa lectura (S1).
4. **AC-S4** — Dados los nodos de un árbol, recordados por una transacción de lectura anterior, una página del mapa de
   espacio libre recordada por una escritura anterior y un valor fuera de su hoja recordado, todos en el archivo de
   datos, cuando una transacción de escritura lee una clave dos veces, guarda claves que dividen y fusionan nodos,
   reserva páginas y mueve el valor, entonces no calcula la integridad de ninguna de esas páginas ni del valor; y una
   página del archivo de datos que nadie leyó en la apertura se comprueba en su primera lectura por la escritura y no
   en las siguientes, de esa transacción ni de otras (S2). Este criterio sustituye a AC-I2.
5. **AC-S5** — Dada una página del archivo de datos alterada con el almacén cerrado, cuando se abre y una transacción
   de escritura la lee dos veces, entonces cada lectura falla con la clase de daño y el número de esa página y la
   transacción no puede confirmar (S2, T11).
6. **AC-S6** — Dada una hoja recordada sin versiones pendientes de trasladar, cuando la prueba altera con el almacén
   abierto un byte del valor de una de sus entradas sin romper su organización, entonces una transacción de escritura
   que la lee sin modificarla devuelve el byte alterado sin fallar, y tras cerrar y reabrir el almacén la primera
   lectura de esa hoja falla con la clase de daño y el número de su página; y cuando, antes de reabrir, otra escritura
   modifica otra entrada de esa hoja y confirma, entonces tras el traslado y una nueva apertura la hoja se lee sin
   error de daño y devuelve el byte alterado (S2, S3, I5). Este criterio sustituye a AC-I8.
7. **AC-S7** — Dada una versión confirmada de un nodo que todavía está en el área temporal, cuando una transacción de
   escritura la lee dos veces sin cambiarla, entonces su organización se comprueba en cada lectura y su integridad no
   se comprueba, como hoy (S1, S2).
8. **AC-S8** — Dadas las ejecuciones de una sola carga de S5, cuando se comparan, entonces W1, W3, W2, W14, W15 y W21
   mejoran en las dos plataformas y ninguna otra carga de escritura medida empeora; y el informe de cierre incluye esas
   comparaciones y las de las ejecuciones completas con las del 2026-11-06, con las de una sola carga que confirmen o
   descarten cada bajada de más de 0,05 (S5).

    *Anotación del 2026-11-07: AC-S8 se cumple como paso intermedio (decisión del usuario). En Windows mejoran las seis
    cargas. En Linux (WSL 2), con las dos rondas de S5, W2 sale mixto (−2,5 % y +6,9 %) y W21 queda por debajo en las
    dos (−2,9 % y −3,2 %). Dos rondas más en la misma sesión dan las dos por encima: W2 +2,0 % y +2,4 %, W21 +0,9 % y
    +4,7 %. Esos efectos son menores que el ruido entre las repeticiones de una misma ejecución (del 4,5 % al 5,6 %
    típico), así que dos rondas no los distinguen.*

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Padre raíz, G6: la anotación de la integridad recordada decía que las transacciones de escritura siguen comprobando
  todo lo que leen. Pasa a decir que usan lo recordado (S2), y la excepción de I8 se amplía con S3: una escritura puede
  sellar en una versión nueva un cambio de una página recordada hecho con el almacén abierto por algo que no es el
  motor, y el daño queda guardado. AC10 no cambia.
- Integridad recordada: el Objetivo («solo lo usan las transacciones de lectura»), I3, I8, el paso 3 del Flujo y el
  punto de Fuera de alcance «Usar lo recordado en las transacciones de escritura» dejan de valer para las escrituras,
  que siguen S2 y S3. AC-I2 se sustituye por AC-S4 y AC-I8 por AC-S6.
- Comprobación de organización recordada: C3, el paso 4 del Flujo y el punto de Fuera de alcance sobre no recordar la
  organización de las páginas que modifica la escritura pasan a decir que esas páginas comprueban su organización la
  primera vez que se leen después de cada cambio, con una memoria propia de cada transacción (S1); C1 y C4 valen para
  la memoria compartida, que esas páginas no usan; el riesgo residual de C7 se amplía con el de S3.
- Árboles de tamaño fijo, AC-X19: su cuarta parte, que AC-I8 había sustituido, pasa a AC-S6.

## Referencias

- Padre: [pager-integrity-once.prd.md](pager-integrity-once.prd.md) v1.0 (I1–I9, AC-I2, AC-I8)
- Pager: [pager.prd.md](pager.prd.md) v1.0 (P10, P12, P16)
- Raíz: [krummi.prd.md](krummi.prd.md) v1.1 (G6, AC10)
- Relacionados: [trees-node-check-cache.prd.md](trees-node-check-cache.prd.md) (C1–C7),
  [transactions.prd.md](transactions.prd.md) (T11, T14, T16), [journal.prd.md](journal.prd.md) (J5, J7),
  [free-space.prd.md](free-space.prd.md) (F9), [trees-fixed-size.prd.md](trees-fixed-size.prd.md) (AC-X19),
  [benchmark.prd.md](benchmark.prd.md) (K7, K13)
- Plan de rendimiento: [docs/performance/plan.md](../../performance/plan.md), O6 y O7, diagnóstico del 2026-11-06
