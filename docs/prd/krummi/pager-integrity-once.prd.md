# Integridad recordada de las páginas — PRD

**Versión**: 1.0
**Fecha**: 2026-11-05
**Documento padre**: [Pager de Krummi — PRD](pager.prd.md) (v1.0)

## Objetivo y contexto

Hacer más baratas las lecturas del motor dejando de repetir la **comprobación de integridad** (G6) de una página que
ya la pasó mientras el almacén sigue abierto. Hoy, cada vez que una operación lee una página del archivo de datos (o,
en modo temporal, de la memoria del almacén), el motor calcula el código de integridad de 64 bits de la página entera
y lo compara con el guardado; un valor guardado fuera de su hoja se comprueba entero en cada lectura. Una página que
no cambia se comprueba igual en cada lectura.

Este cambio hace que el motor **recuerde las páginas cuya integridad ya comprobó** en la apertura en curso, como hace
Voron: la primera lectura de una página en cada apertura la comprueba; las siguientes la usan sin volver a calcular
su código, hasta que el propio motor la reescribe o el almacén se cierra. Solo lo usan las transacciones de lectura:
las de escritura siguen comprobando cada página que leen, así que un daño nunca se sella en una versión nueva.

**Por qué**: la comprobación de integridad es el mayor coste de las lecturas. En el diagnóstico del plan de
rendimiento del 2026-11-04, anterior a las optimizaciones O1, O2 y O3a, se llevaba el 62 % del tiempo de las lecturas
al azar con un hilo (W7) y el 76 % de la lectura de árboles de tamaño fijo (W22); cada lectura de una clave comprueba
unas tres páginas. Krummi incumple el umbral de esas cargas en el banco comparativo (módulo 6), y Voron no repite la
comprobación.

**Por qué ahora**: O1, O2 y O3a quitaron esperas entre lecturas y abarataron el cálculo del código, pero no cambian
cuántas veces se calcula; es el mayor coste que queda en las cargas de lectura.

Este PRD es un cambio del Pager (módulo 1) que nace de un umbral incumplido del banco comparativo (K13). Hereda todo
lo del padre y sus ascendientes; aquí solo se añade cuándo se repite la comprobación de integridad. Lo que cambia
reglas anteriores está declarado en sus decisiones y anotado en los documentos afectados (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

### Decisiones de producto

| ID | Decisión |
|---|---|
| I1 | **Qué se recuerda**: que una página del archivo de datos o, en modo temporal, de la memoria del almacén pasó la comprobación de integridad en la apertura en curso. Un valor guardado fuera de su hoja, que se comprueba entero, se recuerda y se olvida entero. Solo se recuerdan comprobaciones superadas: una página que no la pasa nunca se recuerda, así que vuelve a fallar en cada lectura.<br><br>*Anotación del 2026-11-06: un valor fuera de su hoja se recuerda con todas sus páginas y con una marca de su longitud en la memoria compartida y acotada de los contenidos comprobados. Si otro contenido ocupa el sitio de la marca, una lectura de otra longitud desde la primera página del valor no se comprueba. Solo una referencia con una longitud equivocada hace esa lectura, y esa referencia está en una hoja que pasó su propia comprobación: no viene de un daño en el disco. Se acepta este residuo, porque cerrarlo pide más memoria que I6 o comprobar de nuevo el valor (contra AC-I1). Cerrarlo sin coste exigiría guardar la longitud en el encabezado del valor, un cambio de formato con su propio PRD (decisión del usuario).* |
| I2 | **Lecturas que lo usan**: solo las de las transacciones de lectura. Si la página está recordada, se usa sin calcular su código; si no, se comprueba: si falla, la operación falla con la clase de daño y el número de la página; si pasa, desde ese momento se recuerda. |
| I3 | **Lecturas que no lo usan**: las transacciones de escritura comprueban cada página que leen del archivo de datos o, en modo temporal, de la memoria del almacén, como hoy, aunque esté recordada: al bajar por un árbol, al copiar una página para modificarla (un nodo, una página del mapa de espacio libre, F9) y al leer las páginas y los valores que pasan a otras páginas (divisiones, fusiones, valores que se mueven). Si la comprobación pasa, la página queda recordada para las transacciones de lectura. Toda otra lectura que no es de una transacción de lectura (la de la apertura, la consulta de información del almacén) también comprueba como hoy. El backup, la compactación, la migración y la verificación (módulos 10 a 12) comprueban cada página que leen, como dice G6; sus PRD fijan cómo y si lo que comprueban queda recordado. No cambian, y siguen sin comprobar la integridad: las versiones confirmadas que todavía están en el área temporal (J7, T16), también cuando una escritura las copia, y las copias que modifica la propia escritura. El encabezado sigue con su comprobación al abrir (P6), y el diario y la recuperación con las suyas. |
| I4 | **Reescrituras del motor**: cuando el motor va a escribir una página en su sitio del archivo de datos o, en modo temporal, de la memoria del almacén (el traslado, J5), esa página deja de estar recordada antes de escribirla, y con ella el valor fuera de su hoja que la contenga entero. La primera lectura posterior la comprueba otra vez. Una escritura que falla o queda a medias (G12) deja la página sin recordar. Olvidar una página nunca falla ni necesita memoria. |
| I5 | **Alcance y vida**: hay una sola memoria de páginas recordadas por almacén abierto, también en modo temporal, compartida por todas sus transacciones, árboles e hilos. Empieza vacía al terminar de abrir el almacén (después de la recuperación, G5); las páginas que el archivo gana al crecer (P11) empiezan sin recordar. Se descarta al cerrarlo y no se guarda en disco: el formato no cambia y un almacén se abre igual con o sin este cambio. |
| I6 | **Memoria**: lo recordado ocupa como mucho 16 KiB por cada GiB del archivo de datos o, en modo temporal, de la memoria del almacén, y crece con él. No cuenta para el tamaño máximo (P10). Reservarla forma parte de abrir y de crecer: al abrir, si el sistema no la da, la apertura falla con la clase de memoria agotada (P13); al crecer, el crecimiento falla con esa clase, el archivo conserva el tamaño que tenía (P11) y el almacén sigue utilizable. Un crecimiento que no cabe en el tamaño máximo falla con su clase, como hoy, antes de reservar nada. El tamaño es interno: no es una opción del almacén y no aparece en su información (P12). |
| I7 | **Lecturas concurrentes**: consultar y añadir páginas recordadas no añade esperas bloqueantes entre lecturas ni con la escritura. Varias lecturas que leen a la vez una página no recordada pueden comprobarla cada una; el resultado es el mismo. |
| I8 | **Garantía resultante** (excepción a G6 del padre raíz y a C7 de la comprobación de organización recordada, anotadas en ellos). Lo que se mantiene: toda página del archivo de datos o de la memoria del almacén se comprueba la primera vez que la lee una transacción de lectura en cada apertura, la primera vez que la lee después de que el motor la reescriba, y cada vez que la lee una transacción de escritura. Así, todo daño hecho con el almacén cerrado se sigue detectando (AC10) y ninguna escritura sella un daño en una versión nueva. Lo que cambia: si los bytes de una página recordada cambian sin que el motor los escriba (un fallo del hardware o de la memoria, o un programa ajeno, que P16 deja fuera de alcance), las transacciones de lectura no lo detectan hasta que el almacén se vuelve a abrir o el motor la reescribe. Como la comprobación de organización de un nodo recordado (C2) usa el código guardado en la página tal como se lee, tampoco se repite si ese código no cambió. En ese caso, una lectura puede devolver datos que no son los escritos o terminar con un pánico en lugar de un error de daño (y no con el error de G12). Se acepta como riesgo residual: afecta a lo que leen las transacciones de lectura, nunca a lo que el motor guarda por sí mismo. |
| I9 | **Rendimiento** (aceptación; precisa la anotación del 2026-11-04 a K13, anotada en ella): la base son las ejecuciones completas del banco comparativo en Windows y en Linux sobre el commit anterior al cambio. El cambio se acepta cuando, en las ejecuciones completas posteriores de cada plataforma, ninguna carga baja su razón con Voron más de 0,05 respecto a esa base y las cargas que ataca, W7 y W22, la suben más de 0,05; 0,05 es el ruido que fija el plan de rendimiento, en valor absoluto de la razón. El informe de cierre muestra si W7 y W22 cumplen su umbral (K7) e incluye ejecuciones de una sola carga de lectura antes y después del cambio en las dos plataformas. No cumplir el umbral no impide cerrar el cambio: lo que falte sigue el plan de rendimiento. |

### Estado de partida

Verificado el 2026-11-05 en el código:

| Aspecto | Situación |
|---|---|
| Comprobación de integridad | Código de 64 bits sobre toda la página (que incluye su número), en cada lectura de una página del archivo de datos o de la memoria del almacén temporal: nodos, páginas del mapa de espacio libre y la página de metadatos que se lee al abrir. Un valor fuera de su hoja lleva un solo código para todas sus páginas y se comprueba entero en cada lectura; uno de 256 MiB son unas 32.769 páginas. |
| Lecturas sin comprobación | Las versiones del área temporal y las copias que modifica la escritura; el encabezado tiene su propia comprobación, solo al abrir. |
| Escrituras con el almacén abierto | El traslado escribe en su sitio las versiones selladas en la confirmación, sin volver a comprobarlas; la recuperación comprueba lo que aplica antes de escribirlo, al abrir. El crecimiento añade páginas vacías, que no pasan la comprobación. |
| Memoria por apertura | La comprobación de organización recordada (C1–C9) ocupa un tamaño fijo. No hay ninguna memoria que crezca con el número de páginas. |
| Pruebas de daño | Las del barrido de cada byte (AC10) alteran los archivos con el almacén cerrado y lo reabren. Dos pruebas alteran una página del archivo de datos con el almacén abierto y esperan el daño en la lectura siguiente: AC-C7 y la cuarta parte de AC-X19. |
| Voron (referencia) | Según el plan de rendimiento (RavenDB 7.2.7), marca en un mapa de bits cada página la primera vez que pasa su comprobación tras abrir y no comprueba las páginas más allá del tamaño del archivo al abrir, porque las escribió esa misma apertura. |

El coste viene del diagnóstico del plan de rendimiento del 2026-11-04: la comprobación de integridad era el 62,4 % de
W7 y el 18,5 % de W9. O3a (2026-11-05) hizo el cálculo unas 1,8 veces más rápido; no hay un perfil posterior.

## Actores

- **Desarrollador integrador**: lee y escribe como hasta ahora; no cambia nada de lo que usa.
- **Mantenedor**: ejecuta las pruebas, las ejecuciones completas del banco y las de una sola carga de I9 en Windows
  y en Linux.

## Flujo

1. La aplicación abre un almacén; tras la recuperación, el motor empieza sin páginas recordadas (I5).
2. La primera lectura de una página en la apertura la comprueba y, si la pasa, la recuerda; las transacciones de
   lectura siguientes, desde cualquier hilo, la usan sin comprobarla (I1, I2, I7).
3. Una transacción de escritura comprueba cada página que lee, como hoy (I3); la nueva versión de una página que
   modifica se lee del área temporal como hoy. Cuando el traslado la escribe en su sitio, deja de estar recordada y
   la primera lectura posterior la comprueba (I4).
4. Al cerrar el almacén, lo recordado se descarta; la apertura siguiente vuelve a comprobar cada página la primera vez
   que la lee.

## Validaciones y mensajes

No hay situaciones, clases de error ni datos de error nuevos. Una página cuya integridad falla sigue fallando con la
clase de daño y el número de su página en cada operación que la necesite, en el mismo orden que hoy. No poder reservar
la memoria de I6 es la clase de memoria agotada de P13.

## Salida

Nada de lo que devuelve una operación cambia de forma: los mismos datos, las mismas clases de error y los mismos datos
de error. La información del almacén (P12) no añade datos y la versión de formato sigue siendo `9`. Por ejemplo, la
segunda lectura de una página dañada falla igual que la primera, porque una página que no pasa la comprobación nunca
se recuerda:

| Dato del error | Primera lectura | Segunda lectura |
|---|---|---|
| Clase | daño | daño |
| Carpeta | la del almacén | la del almacén |
| Página | `<número>` | `<número>` |

## Reutilización

- La comprobación de integridad de las páginas y de los valores fuera de su hoja, y la clase de daño (G6), sin
  cambios en lo que comprueban.
- La comprobación de organización recordada (C1–C9), que sigue recordando nodos por su número de página y el código
  guardado en ella, y su regla de coherencia (C8), de la que también depende I4.
- Las reglas del traslado (J5, T14).
- El banco comparativo (módulo 6) y las ejecuciones de una sola carga del plan de rendimiento, para I9.

## Fuera de alcance

- Comprobar la integridad una vez por transacción en lugar de por apertura.
- Comprobar la integridad de páginas que no se leen, en segundo plano o al abrir (como en [pager.prd.md](pager.prd.md)).
- Dejar sin comprobar, como Voron, las páginas que el archivo gana después de abrir: se comprueban la primera vez que
  se leen, como las demás (I5).
- Usar lo recordado en las transacciones de escritura, y en el backup, la compactación, la migración y la
  verificación (I3); los criterios de esas cuatro operaciones son de sus PRD (módulos 10 a 12).
- Comprobar la integridad de las versiones del área temporal y de las copias que modifica la escritura (I3).
- Cambiar la comprobación del encabezado (P6), la del diario o la de la recuperación.
- Detectar que otro programa, o un fallo del hardware o de la memoria, cambia una página recordada con el almacén
  abierto (I8, P16).
- Guardar lo recordado entre aperturas.
- Una opción pública para desactivarlo o limitar su memoria, y estadísticas de páginas recordadas en la información
  del almacén (I6).
- Códigos de integridad más fuertes o criptográficos.
- El cifrado (módulo 9): decide si usa lo recordado y sobre qué bytes.
- Exigir que W7 o W22 cumplan su umbral para cerrar el cambio (I9).

## Dejado a la implementación

- La forma de la memoria de páginas recordadas (por ejemplo, un mapa de bits), cómo crece con el archivo y en qué
  momento del crecimiento se reserva, dentro de I6.
- Cómo se recuerda y se olvida un valor fuera de su hoja (por ejemplo, por su primera página), dentro de I1 e I4.
- Cómo cuentan las pruebas las comprobaciones de integridad sin una opción pública.
- Las cargas exactas y las repeticiones de las ejecuciones de una sola carga de I9.

## Criterios de aceptación

Valen los de los ascendientes, que deben seguir pasando sin cambios salvo los que este PRD sustituye o precisa (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)), incluido AC10 del padre raíz. Cada criterio se comprueba en un
almacén en disco y en modo temporal, salvo las partes que reabren el almacén, que son solo en disco. Las pruebas usan
el conocimiento de la implementación para contar las comprobaciones de integridad, para colocar páginas en el archivo
de datos o en el área temporal, para alterar bytes y para simular la falta de memoria. Las cuentas de comprobaciones
se hacen con lecturas de un solo hilo (I7).

1. **AC-I1** — Dados un árbol y un valor guardado fuera de su hoja con sus páginas en el archivo de datos, cuando una
   transacción de lectura los lee varias veces y otra transacción de lectura los lee después, entonces todas
   devuelven lo mismo y la integridad de cada página y del valor se comprueba solo en su primera lectura; y tras
   cerrar y reabrir el almacén, la primera lectura vuelve a comprobarla (I1, I2, I5).
2. **AC-I2** — Dados los nodos de un árbol, recordados por una transacción de lectura anterior, y una página del mapa
   de espacio libre, recordada por una escritura anterior (I3), todos en el archivo de datos, cuando una transacción de escritura lee una clave dos veces y después guarda una que
   modifica una hoja y reserva páginas, entonces comprueba la integridad de cada página que lee del archivo de datos
   en cada lectura, incluidas la hoja y la página del mapa que copia; y una transacción de lectura posterior no
   vuelve a comprobar los nodos que la escritura no modificó (I2, I3).
3. **AC-I3** — Dada una versión confirmada que todavía está en el área temporal, y las copias que modifica una
   escritura, cuando se leen, entonces su integridad no se comprueba, como hoy (I3).
4. **AC-I4** — Dada una página recordada, cuando una escritura cambia su contenido y se confirma mientras una
   transacción de lectura abierta antes sigue leyéndola, entonces esa transacción sigue viendo el contenido anterior;
   y cuando esa transacción se cierra y la nueva versión se traslada a su sitio, entonces una transacción de lectura
   nueva ve el contenido nuevo y su integridad se comprueba una vez (I4, C8, G3).
5. **AC-I5** — Dado un valor fuera de su hoja recordado, cuando se borra y una escritura posterior reutiliza sus
   páginas para otro contenido que llega al archivo de datos, entonces la lectura del contenido nuevo lo comprueba una
   vez y devuelve lo escrito (I1, I4).
6. **AC-I6** — Dada una página del archivo de datos alterada con el almacén cerrado, cuando se abre y se lee dos
   veces en la misma transacción de lectura y una vez en otra, entonces cada lectura falla con la clase de daño y el
   número de esa página (I1, I2, G6).
7. **AC-I7** — Dada una página que todavía no se leyó en la apertura en curso, cuando la prueba altera uno de sus
   bytes con el almacén abierto y después se lee, entonces la lectura falla con la clase de daño y el número de esa
   página (I2, I8).
8. **AC-I8** — Dada una página recordada sin versiones pendientes de trasladar, cuando la prueba altera uno de sus
   bytes con el almacén abierto, entonces una transacción de escritura que la lee falla con la clase de daño y el
   número de esa página, en esa operación, y no puede confirmar (T11); y tras cerrar y reabrir el almacén, la primera
   lectura de esa página falla igual (I3, I5, I8). Este criterio sustituye a AC-C7 y a la cuarta parte de AC-X19.
9. **AC-I9** — Dados páginas correctas y una dañada como en AC-I6, cuando cuatro hilos con transacciones de lectura
   las leen a la vez, al menos mil veces cada uno, mientras una escritura cambia las correctas y se trasladan,
   entonces cada lectura devuelve los datos de su instantánea y la página dañada falla en todas sus lecturas con la
   clase de daño (I4, I7).
10. **AC-I10** — Dado un almacén con páginas recordadas, cuando crece mientras se leen, entonces las páginas
    recordadas antes de crecer que el traslado no reescribió siguen sin comprobarse otra vez, y las páginas nuevas se
    comprueban la primera vez que se leen (I4, I5, I6).
11. **AC-I11** — Dada una falta de memoria simulada al reservar la memoria de I6, cuando se abre un almacén, entonces
    la apertura falla con la clase de memoria agotada; y cuando un almacén abierto crece, entonces el crecimiento falla
    con esa clase, el tamaño del almacén en su información no cambia y el almacén sigue aceptando lecturas y
    escrituras (I6, P13).
12. **AC-I12** — Dadas las ejecuciones completas del banco en Windows y en Linux sobre el commit anterior al cambio y
    sobre el cambio, cuando se comparan por plataforma, entonces ninguna carga baja su razón con Voron más de 0,05 y
    W7 y W22 la suben más de 0,05; y el informe de cierre incluye esas comparaciones, si W7 y W22 cumplen su umbral y
    las ejecuciones de una sola carga de lectura antes y después (I9).

    *Anotación del 2026-11-06: una carga cuya razón baja más de 0,05 entre dos ejecuciones completas cuenta como
    peor solo si las ejecuciones de una sola carga, antes y después en la misma sesión, confirman la bajada. Las
    ejecuciones completas de días distintos se mueven más que 0,05 también en los motores que no cambian. Con
    esta lectura, AC-I12 se cumple tras O3 y O4: ninguna de las cargas que bajaron en las ejecuciones completas
    del 2026-11-06 baja en sus ejecuciones de una sola carga contra el commit anterior a O3 (decisión del
    usuario).*

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Padre raíz, G6 y AC10: excepción de I8. En las transacciones de lectura, la integridad de una página se comprueba
  la primera vez que se lee en cada apertura y tras cada reescritura del motor; un cambio de una página recordada
  hecho con el almacén abierto por algo que no es el motor puede devolver datos que no son los escritos o terminar en
  un pánico hasta reabrir. Las transacciones de escritura, el backup, la compactación, la migración y la verificación
  siguen comprobando todo lo que leen. AC10 no cambia, porque sus alteraciones se hacen con el almacén cerrado.
- Banco comparativo, anotación del 2026-11-04 a K13: este cambio tiene PRD y su aceptación es la de I9, que no exige
  que W7 y W22 cumplan su umbral.
- Pager, P10, P11 y P13: la memoria de páginas recordadas crece con el almacén, no cuenta para el tamaño máximo y no
  poder reservarla es memoria agotada; un crecimiento que falla por ella deja el archivo con el tamaño que tenía (I6).
- Comprobación de organización recordada: el Objetivo, el Flujo, C2, C3, C5, C7 y el primer punto de Fuera de alcance
  suponían la comprobación de integridad en cada lectura; ahora se hace como fijan I2 e I3, y lo recordado de un nodo
  se busca con el código guardado en su página tal como se lee. C3 pasa a decir: usan lo recordado las lecturas de
  nodos del archivo de datos o, en modo temporal, de la memoria del almacén. El riesgo residual de C7 se amplía con el
  de I8. AC-C1 se precisa: la integridad de cada nodo se comprueba en su primera lectura, no en todas. AC-C7 se
  sustituye por AC-I8.
- Árboles de tamaño fijo, AC-X19: su primera parte se precisa como AC-C1, y su cuarta parte (un byte cambiado con el
  almacén abierto en una hoja recordada) se sustituye por AC-I8.
- Árboles y operaciones por clave (A9), árboles multivalor (M11) y árboles de tamaño fijo (X10): una página dañada
  sigue fallando en cada operación que la necesite, salvo el riesgo residual de I8, que sustituye al de C7.
- Valores grandes, V7 y Dejado a la implementación: un valor fuera de su hoja dañado sigue fallando cada vez que se
  necesita (I1), salvo el riesgo residual de I8; deja de estar abierto si su integridad se comprueba en cada lectura
  o una vez por transacción, porque lo fijan I1 a I4.
- Módulo de árboles, Reutilización: la comprobación de integridad de las páginas deja de hacerse en cada lectura del
  archivo de datos y sigue I2 e I3, con el riesgo residual de I8.

## Referencias

- Padre: [pager.prd.md](pager.prd.md) v1.0
- Raíz: [krummi.prd.md](krummi.prd.md) v1.1 (G5, G6, G12, AC10)
- Relacionados: [trees-node-check-cache.prd.md](trees-node-check-cache.prd.md) (C1–C9),
  [trees-fixed-size.prd.md](trees-fixed-size.prd.md) (X10, AC-X19), [trees-keys.prd.md](trees-keys.prd.md) (A9),
  [trees-multivalue.prd.md](trees-multivalue.prd.md) (M11), [journal.prd.md](journal.prd.md) (J5, J7),
  [transactions.prd.md](transactions.prd.md) (T14, T16), [free-space.prd.md](free-space.prd.md) (F9),
  [benchmark.prd.md](benchmark.prd.md) (K7, K13)
- Plan de rendimiento: [docs/performance/plan.md](../../performance/plan.md), O3
