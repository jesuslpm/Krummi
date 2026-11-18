# Proyecciones de columnas de una tabla — PRD

**Versión**: 1.0
**Fecha**: 2026-11-15
**Documento padre**: [Lecturas de filas sin comprobar su contenido — PRD](tables-row-reads.prd.md) (v1.0)

## Objetivo y contexto

Leer las columnas de las filas de una tabla con una **proyección**: la aplicación nombra una vez las columnas que va a
leer, Krummi prepara una sola vez todo lo que depende del esquema y no de cada fila, y después cada fila se lee sin
repetir ese trabajo. La proyección preparada se guarda en memoria en el almacén y la comparten todas sus
transacciones.

**Por qué**: hoy cada lectura de una columna repite, en cada fila, trabajo que solo depende del esquema: buscar la
columna y comprobar su tipo y su forma de guardarse. Voron lee los campos de sus filas directamente por su posición,
sin tipos. En la última ejecución completa del banco, `tabla-intervalos` (W27) tiene una razón con Voron de 0,91 en
Windows (2026-11-13) y de 0,95 en Linux (2026-11-12). En el diagnóstico del 2026-11-15, un prototipo que prepara ese
trabajo una vez por esquema y lo comparte entre transacciones redujo el tiempo de W27 un 6,7 %, con mejora en las
cuatro rondas (su razón pasó de 0,926 a 0,992), y el de `tabla-lecturas` (W26) un 2,8 %, con una ronda en contra. Leer
con columnas preparadas de antemano midió más que leer por posición dentro de la proyección (6,7 % frente a 3,8 % en
W27).

El usuario aceptó guardar la proyección en memoria en el almacén («Me parece bien entonces.»), retenerla hasta cerrarlo
con un tope de entradas («Sí me sirve»), ligarla al esquema con el que se preparó, devolver un error por lo que depende
del esquema y un pánico por un fallo del programa, ofrecer solo columnas preparadas de todos los tipos y no ofrecer un
precalentamiento de las versiones antiguas («Sí» a cada una), para el banco: «Usa la proyección siempre que sea útil.»,
y los criterios que cierran los huecos de la revisión del borrador («Sí»).

Este PRD adelanta las proyecciones que [tables-schema.prd.md](tables-schema.prd.md) deja para el módulo 17, amplía
**esquema distinto**, la lista de reglas de **valor no válido** y la regla de daño de ese PRD, y precisa cómo leen sus
filas W26 y W27 y K7 del banco; todos llevan su anotación (*Anotaciones a otros PRD*).

### Decisiones de producto

| ID | Decisión |
|---|---|
| PY1 | **Preparar una proyección**: desde una apertura de una tabla, en una transacción de lectura o de escritura, con una lista de nombres de columnas vivas en cualquier orden, de la clave o no, cada una una sola vez. La proyección queda ligada a la tabla y a su **esquema**, el estado del esquema que ve la apertura (E2): el que identifican las identidades y el estado, no el número de versión, así que el de una escritura abandonada nunca coincide con otro. El orden de la lista cuenta: da las posiciones de la proyección, y la misma lista en otro orden es otra proyección. Preparar no lee filas, no es un cambio para T10 ni termina referencias (E16), como Leer, y sus errores son de validación pública (B7): no cambian la transacción. Se comprueba una sola vez, con la precedencia de E17: (1) un nombre que no mide de 1 a 1.024 bytes falla con **tamaño no válido**, con los datos de B8, el primero en el orden de la lista; (2) una apertura de una tabla que la escritura borró, con **árbol no encontrado**, como toda operación a través de ella (E17); (3) un nombre que no es de una columna viva de ese esquema, también el de una columna quitada o el de una que añade un esquema posterior, con **esquema distinto**, con la tabla y la columna, el primero en el orden de la lista; (4) una columna repetida, con **valor no válido** y la regla columna repetida, la que se informa como en las filas (E17); y una lista vacía, con **valor no válido** y la regla nueva proyección sin columnas, con la tabla. La tabla que llevan estos errores y los de PY2 y PY3 es la de la proyección, con el nombre que tenía al prepararla. |
| PY2 | **Columnas preparadas**: de una proyección se obtiene, para cada posición de su lista, una columna preparada con el tipo con que se va a leer, uno de los tipos de columna de N4 que existen hoy, sin importar si la columna admite nulos; una columna `string` se puede preparar también para leerse como sus bytes (L2). Hay columnas preparadas para todos esos tipos, también para las columnas de la clave. Un tipo que no es el de la columna falla con **esquema distinto**, con la tabla, la columna y la diferencia (el tipo pedido y el de la columna), como E6, sin cambiar la transacción. Una posición fuera de la lista termina con un pánico normal del hilo, como el de L2: es un fallo del programa y no cambia el almacén ni la transacción. No hay lectura por posición dentro de la proyección: se lee solo con columnas preparadas. |
| PY3 | **Leer una fila con la proyección**: toda fila leída por su clave o recorrida en un intervalo en cualquier sentido, en una transacción de lectura o de escritura, y también una **fila copiada** por la aplicación (la que conserva sus valores después del siguiente cambio de su transacción, con su tabla, su almacén y su esquema), se liga primero a la proyección y después se lee columna a columna con las columnas preparadas, cada una con su valor o nulo. Los valores son los mismos, con la misma normalización (N4, E7), que leer la columna de la fila por su posición (E11), y se devuelven igual: sin copia el texto, los bytes y los vectores (D11, *Salida* de [tables.prd.md](tables.prd.md)), y decodificados en una copia propia de la aplicación los demás; de una fila copiada, todos se devuelven como al leerla hoy por su posición, en una copia propia de la aplicación que puede vivir más que la fila copiada (usuario, entre esa copia y un préstamo de la fila copiada: «A»). Una fila guardada con una versión antigua del esquema se lee también, con nulo o con su valor por defecto en las columnas añadidas después de guardarla (N3). Ligar a la proyección una fila de otra tabla, también de una que se creó con el mismo nombre después de borrar la suya, de otro almacén abierto (también uno abierto desde una copia de los mismos archivos) o de una transacción que ve otro esquema de la tabla falla con **esquema distinto**, con la tabla, sin cambiar la transacción. Leer con una columna preparada de otra proyección, también de la misma lista en otro orden, termina con un pánico normal del hilo, como el de L2: es un fallo del programa y no cambia el almacén ni la transacción. Una columna preparada sirve para toda proyección de la misma lista, en el mismo orden, sobre el mismo esquema, la guarde el almacén o no (PY5). Preparar, obtener columnas preparadas, ligar y leer no leen páginas, así que no fallan con daño: los daños de la fila y del historial siguen apareciendo en la lectura o en el paso del recorrido que devuelve la fila (L1). |
| PY4 | **Cambios de esquema**: una proyección no se adapta a otro esquema de su tabla (PY3), tampoco dentro de la misma escritura. Tras un cambio de esquema que da un esquema nuevo (E16), la aplicación prepara otra proyección desde una apertura que lo ve. Lo que no cambia el esquema (un cambio de esquema que no cambia nada, vaciar la tabla, renombrarla) no la afecta; borrar la tabla y crear otra con su nombre da otra tabla (PY3). |
| PY5 | **La proyección en el almacén**: se guarda en memoria en el almacén abierto, nunca en sus archivos, así que cerrarlo la pierde; vale mientras el almacén esté abierto, y el compilador impide usarla después (T8). La comparten todas sus transacciones y sus hilos (T6). Preparar otra vez la misma lista, en el mismo orden, sobre el mismo esquema devuelve la proyección guardada. El almacén guarda hasta cerrarse como mucho 1.024 proyecciones, las primeras que se preparan, cuenten las de cualquier tabla, esquema o transacción, también las de una escritura que se abandona; con varias preparaciones a la vez cuando queda una plaza, se guarda una de ellas, sin fijar cuál. Pasado el tope, preparar una lista nueva funciona igual, pero da una proyección que no se guarda. Nunca se quita una proyección guardada antes de cerrar el almacén. |
| PY6 | **Contenido de la fila** (L1, L2 del padre): leer con la proyección no comprueba el contenido de la fila. Una fila mal formada se lee como dice L2: una columna, también una de la clave, puede dar un valor que no es el escrito o terminar con un pánico normal del hilo, y un texto que no es UTF-8 leído como texto termina siempre con un pánico; leído como sus bytes, se devuelve tal cual. Con una compilación del motor que aborta en los pánicos, cada pánico de este PRD termina el proceso (L2). |
| PY7 | **Las cargas del banco** (K7): Krummi lee con una proyección las filas de W26 y W27, y de las cargas de tablas del banco que lean valores de columnas en PRD futuros, salvo que su PRD diga otra cosa. Es la lectura con las capacidades reales del motor, la regla que ya sigue W28. Preparar la proyección cuenta en el tiempo medido de cada repetición, antes de su primera fila, la devuelva guardada el almacén o no. Las cargas no cambian: las mismas filas, las mismas columnas y la misma suma de comprobación en los dos motores. Los umbrales no cambian. |

*Anotaciones del 2026-11-18 desde los índices secundarios ([tables-indexes.prd.md](tables-indexes.prd.md)):*

- *PY3: las filas de un recorrido de un índice se ligan a una proyección como las de un recorrido de la clave primaria; las entradas de un índice no (decisión IX8).*

### Estado de partida

- Una fila leída o recorrida se lee columna a columna, por el nombre o la posición de la columna entre las columnas
  vivas (E11). Cada lectura comprueba el tipo de la columna; leer una columna con otro tipo termina con un pánico.
- Nada de lo que se prepara para leer las filas de una tabla se comparte entre transacciones: cada una lo vuelve a
  preparar al abrir la tabla por primera vez.
- No hay proyecciones: [tables-schema.prd.md](tables-schema.prd.md) deja «Leer solo algunas columnas de una fila
  (proyecciones)» para el módulo 17.
- W26 y W27 leen en Krummi todas las columnas de cada fila columna a columna y calculan la suma de comprobación de
  *Cargas del banco* de [tables-schema.prd.md](tables-schema.prd.md); Voron lee cada campo por su posición.
- Razones de la última ejecución completa: W26, 0,97 en Windows y 1,10 en Linux; W27, 0,91 y 0,95. Son contexto, no
  criterios: la aceptación es AC-PY11.
- Ningún dato guardado cambia: el formato de las filas, de las claves y del almacén es el de 7.1 (E20).

## Actores

- **La aplicación** que lee filas de una tabla y quiere leer sus columnas con el menor coste por fila.
- **El banco comparativo**, cuyas cargas W26 y W27 leen las columnas de las filas de Krummi (K7).

## Flujo

1. La aplicación abre la tabla y prepara una proyección con los nombres de las columnas que va a leer (PY1).
2. Obtiene de la proyección una columna preparada por cada posición, con su tipo (PY2).
3. Lee o recorre filas como hoy (E11, E12), liga cada una a la proyección y lee sus columnas con las columnas
   preparadas (PY3).
4. En otra transacción, prepara de nuevo la misma lista y obtiene la proyección guardada (PY5); tras un cambio de
   esquema, prepara otra con el esquema nuevo (PY4).

## Validaciones y mensajes

Valen las de los ascendientes. Este PRD añade, solo en la biblioteca, en el orden de su precedencia:

| Situación | Clase de error |
|---|---|
| Preparar con un nombre que no mide de 1 a 1.024 bytes | Tamaño no válido (B7, E18), con los datos de B8 |
| Preparar desde una apertura de una tabla que la escritura borró | Árbol no encontrado (A6, E17) |
| Preparar con un nombre que no es de una columna viva del esquema que ve la apertura | Esquema distinto, con la tabla y la columna |
| Preparar con una columna repetida | Valor no válido, regla columna repetida, con la tabla y la columna |
| Preparar con una lista vacía | Valor no válido, regla proyección sin columnas (nueva), con la tabla |
| Obtener una columna preparada con un tipo que no es el de la columna | Esquema distinto, con la tabla, la columna y la diferencia |
| Ligar a la proyección una fila de otra tabla, de otro almacén o de otro esquema | Esquema distinto, con la tabla |
| Obtener una columna preparada de una posición fuera de la lista; leer con una columna preparada de otra proyección | Pánico normal del hilo, no un error (PY2, PY3) |

Los errores no cambian la transacción (B7); los pánicos siguen L2. Preparar, obtener columnas preparadas, ligar y leer
no fallan con daño (PY3).

## Salida

El módulo no escribe nada en pantalla. Leer una columna preparada de una fila ligada da su valor o nulo, con la forma y
los préstamos de leer la columna por su posición: sin copia el texto, los bytes y los vectores de una fila leída o
recorrida, y en una copia propia de la aplicación los de una fila copiada (PY3). Las lecturas, los recorridos y las lecturas de columnas que ya
existen no cambian su salida. El informe del banco no cambia de forma.

## Reutilización

- Las lecturas y los recorridos de filas (E11, E12), que siguen devolviendo las filas que la proyección lee.
- Los valores, su normalización y sus préstamos de las lecturas de columnas de hoy (N4, E7, D11).
- La lectura de filas de versiones antiguas del esquema con sus valores por defecto (N3) y la falta de comprobación de
  su contenido (L1, L2).
- Las clases de error, sus datos y su precedencia (E6, E17, E18, B7, B8).

## Fuera de alcance

- Leer por posición dentro de la proyección, sin columnas preparadas (PY2).
- Una proyección que se adapte a otro esquema de su tabla, o que sirva para varias tablas (PY3, PY4).
- Precalentar las versiones antiguas del esquema antes de leer sus filas.
- Configurar el tope de proyecciones guardadas, quitarlas antes de cerrar el almacén, guardarlas en sus archivos o
  informar de ellas en la información del almacén (P12).
- Leer una columna como otro tipo que el suyo, salvo un `string` como sus bytes (PY2).
- Escribir filas con una proyección.
- Leer menos de cada fila del almacenamiento: la fila se sigue leyendo entera de su página; la proyección solo ahorra
  trabajo al leer sus columnas.
- Cambiar las lecturas de columnas que ya existen, que siguen como hoy.
- Los tipos de columna que añadan PRD futuros (JSON en 7.5), que fijarán sus columnas preparadas.
- Los lectores de filas del motor en PRD futuros: los índices secundarios (7.2), las estadísticas (7.3), las columnas
  JSON (7.5), las consultas (17), la interfaz C y el binding C# (18) y la importación y exportación (19), que fijarán si
  usan proyecciones. Las cargas del banco siguen PY7.

## Dejado a la implementación

- Los nombres de las operaciones y de los tipos, en inglés y con las convenciones de Rust (D8); los nombres de
  columna son texto, así que uno que no es UTF-8 no se puede dar.
- Cuánto del tipo de N4 pide una columna preparada: pedir el tipo exacto de la columna siempre se acepta y pedir uno de
  otra clase falla (PY2); si los parámetros del tipo (la precisión y la escala de un `decimal`, el tipo de elemento y la
  dimensión de un vector, el orden de un `uuid`) forman parte de lo que se pide, lo fija la implementación con las
  convenciones de las lecturas de columnas que ya existen.
- Cómo se identifica el esquema al que queda ligada una proyección, con la regla de PY1: nunca el nombre de la tabla ni
  el número de versión.
- Cuándo y cómo se prepara lo que depende de cada versión antigua del esquema, y cuánta memoria ocupa cada proyección.
- Si el propio motor usa proyecciones donde lee columnas de filas, sin cambiar nada observable.
- Cómo comprueban las pruebas que una proyección se guardó o no, y que W26 y W27 leen con una proyección, y cómo se
  ejecuta W27 sin ella para la medición de AC-PY11: son ganchos de prueba y de diagnóstico, no contratos públicos.

## Criterios de aceptación

1. **AC-PY1** — Dada una tabla con una columna de cada tipo de N4 que existe hoy (con vectores de cada tipo de
   elemento, `uuid` en los dos órdenes y `decimal` de varias escalas), con columnas que admiten nulos y una clave de
   varias columnas, y filas con valores y con nulos, cuando se preparan una proyección con todas sus columnas en un
   orden distinto del del esquema, una con una sola columna, una solo con columnas de la clave y una con algunas
   columnas, y se leen con ellas filas por su clave y recorriendo intervalos hacia delante y hacia atrás, en una
   transacción de lectura y en una de escritura que antes cambió filas, entonces cada columna preparada da el mismo
   valor que leer la columna por su posición, nulo donde la fila tiene nulo, y el texto, los bytes y los vectores se
   devuelven sin copia, en la misma dirección; una columna `string` preparada como bytes da sus bytes; y las pruebas
   pasan igual compiladas en depuración y en optimizado.
2. **AC-PY2** — Dada una tabla con filas guardadas antes de añadir una columna con valor por defecto, antes de añadir
   una que admite nulos y antes de quitar una, cuando se leen con una proyección del esquema actual, entonces dan los
   mismos valores que leer sus columnas por su posición, y las columnas añadidas dan su valor por defecto o nulo; y una
   fila de un esquema que falta en el historial falla con daño en la lectura o en el paso del recorrido, antes de
   ligarla, como en AC-L3.
3. **AC-PY3** — Cuando se prepara una proyección con un nombre de 0 bytes, uno de 1.025 bytes, uno que la tabla no
   tiene, el de una columna quitada, el de una que añadió una escritura confirmada después de abrir la transacción, una
   columna repetida y una lista vacía, y desde una apertura de una tabla que la escritura borró, entonces fallan con
   tamaño no válido (con los datos de B8), esquema distinto, esquema distinto, esquema distinto, valor no válido con la
   regla columna repetida, valor no válido con la regla proyección sin columnas y árbol no encontrado, cada uno con sus
   datos; un nombre de 1.024 bytes de una columna viva se acepta. Con varios fallos a la vez se informa el de PY1 que
   va antes: un nombre largo desde una apertura de una tabla borrada da tamaño no válido; un nombre desconocido desde
   esa apertura, árbol no encontrado; uno desconocido y uno repetido, esquema distinto; y con dos nombres desconocidos,
   o dos largos, se informa el primero de la lista. La transacción de escritura en la que fallan se confirma después
   como si no hubieran ocurrido; una escritura que solo prepara proyecciones no tiene cambios para T10; y una fila leída
   antes de preparar sigue valiendo después.
4. **AC-PY4** — Cuando se obtiene una columna preparada con un tipo de otra clase que el de la columna (por ejemplo
   `i64` de un texto, texto de un `i64` y `f64` de un `decimal`), entonces falla con esquema distinto con la tabla, la
   columna y la diferencia, y la transacción de escritura se confirma después; con su tipo, se obtiene; y con una
   posición fuera de la lista termina con un pánico.
5. **AC-PY5** — Dada una proyección, cuando se liga a ella una fila de otra tabla con las mismas columnas, una de una
   tabla creada con el mismo nombre después de borrar la suya, una de otro almacén abierto desde una copia de los
   mismos archivos, una de una transacción que ve la tabla después de un cambio de esquema (en otra escritura y en la
   misma) y una de una transacción anterior al cambio con una proyección del esquema nuevo, entonces fallan con esquema
   distinto con la tabla, y la transacción de escritura se confirma después; una proyección preparada con el esquema
   que ve cada fila la lee; tras renombrar la tabla y tras vaciarla, la proyección sigue leyendo sus filas, y sus
   errores llevan el nombre que la tabla tenía al prepararla; y una proyección de una escritura abandonada no liga las
   filas de otra escritura que llega al mismo número de versión con otro esquema.
6. **AC-PY6** — Dadas una columna preparada de otra proyección de la misma tabla y una de la misma lista en otro orden,
   cuando se lee con ellas una fila ligada, en una transacción de lectura y en una de escritura, entonces terminan con un
   pánico normal del hilo; si la aplicación lo captura alrededor de esa lectura, la transacción sigue intacta, y si lo
   captura fuera, la transacción termina o se descarta como en AC-L5.
7. **AC-PY7** — Dada una proyección preparada en una transacción, cuando se prepara la misma lista en el mismo orden
   sobre el mismo esquema en otra transacción, entonces se obtiene la proyección guardada según el gancho de pruebas, y
   la misma lista en otro orden da otra proyección; usada a la vez desde cuatro hilos y desde dos transacciones de
   lectura, da los mismos valores que leer las columnas por su posición; tras cerrar y abrir el almacén, la misma lista
   ya no está guardada hasta prepararla; y usar una proyección después de cerrar su almacén no compila.
8. **AC-PY8** — Dadas 1.023 proyecciones guardadas en un almacén, contadas también las de una escritura abandonada y
   las de un esquema anterior, cuando se preparan dos listas nuevas, entonces la primera se guarda y la segunda no,
   según el gancho de pruebas; la segunda lee las filas con los mismos valores, y una columna preparada de una
   preparación de la segunda sirve para otra preparación de la misma lista; y las 1.024 guardadas siguen así hasta
   cerrar el almacén.
9. **AC-PY9** — Dada una fila con un texto que no es UTF-8 en una página con la integridad correcta, cuando se lee con
   la proyección, entonces sus demás columnas dan sus valores, leer ese texto con una columna preparada como texto
   termina con un pánico y leerlo con una preparada como bytes lo devuelve tal cual (L2); igual compiladas en depuración
   y en optimizado.
10. **AC-PY10** — Dada una fila copiada por la aplicación, cuando se liga a la proyección y se lee después del siguiente
    cambio de su transacción, entonces da los mismos valores que leer las columnas de esa copia por su posición; su
    texto, sus bytes y sus vectores siguen valiendo después de soltar la fila copiada, como los de la lectura por
    posición; y ligada a una proyección de otro esquema de su tabla, falla con esquema distinto.
11. **AC-PY11** — Dadas las ejecuciones completas del banco en Windows y en Linux, con W26 y W27 leyendo con la
    proyección (según el gancho de pruebas), cuando se comparan con las últimas ejecuciones completas (Windows del
    2026-11-13, Linux del 2026-11-12), entonces las sumas de comprobación de W26 y W27 coinciden en los dos motores; la
    razón de W27 con Voron sube en las dos, salvo que una medición de W27 sola, con sus repeticiones, en el mismo
    equipo, con y sin la proyección, muestre que la proyección baja su tiempo; y ninguna otra carga, W26 incluida,
    baja más de 0,05 su razón con Voron, salvo que la mediana de una medición de esa carga sola, con sus repeticiones,
    la iguale a la de la ejecución anterior dentro de ese margen. Un umbral de K7 que siga incumplido no es un fallo de
    este PRD: sigue K13 y se informa en el cierre.

    *Anotación del 2026-11-16, desde la revisión de código de la ejecución (decisión del usuario): la condición de la
    medición de una carga sola no se cumplió en nueve cargas (W1, W9, W12, W13 y W23 en Windows; W9, W18, W21 y W26 en
    Linux). Se acepta en su lugar una comparación de Krummi solo, antes y después de este cambio, carga a carga, con
    las rondas en orden alterno y los mismos almacenes de partida, que no mostró que el cambio bajara ninguna carga y
    cuyas sumas de comprobación coinciden. Las sumas de comprobación de W26 y W27 coinciden en los dos motores y la
    razón de W27 con Voron subió en los dos sistemas.*
12. **AC-PY12** — Dado un almacén escrito antes de este PRD, cuando se abre y se leen sus tablas con proyecciones,
    entonces da los mismos valores que leer sus columnas por su posición, sin migrar nada.
13. **AC-PY13** — Dados E6, E17, E19, el *Fuera de alcance* y las *Cargas del banco* de
    [tables-schema.prd.md](tables-schema.prd.md), N10 de [tables.prd.md](tables.prd.md) y K7 de
    [benchmark.prd.md](../benchmark/benchmark.prd.md), en el original y en su traducción, cuando se leen, entonces
    llevan la anotación de este PRD.

## Anotaciones a otros PRD

Este PRD cambia reglas de documentos anteriores, que llevan estas anotaciones del 2026-11-15 (en sus traducciones
también):

- [tables-schema.prd.md](tables-schema.prd.md), *Fuera de alcance*, «Leer solo algunas columnas de una fila
  (proyecciones): módulo 17»: *«Las proyecciones de columnas entran antes, en
  [tables-projection.prd.md](tables-projection.prd.md): una lista de columnas preparada una vez por esquema, guardada
  en el almacén, con la que se leen las filas; las consultas del módulo 17 fijarán si las usan.»*
- [tables-schema.prd.md](tables-schema.prd.md), E6: *«También fallan con esquema distinto preparar una proyección con
  un nombre que no es de una columna viva (con la tabla y la columna), obtener una columna preparada con un tipo de
  otra clase (con la tabla, la columna y la diferencia) y ligar a una proyección una fila de otra tabla, de otro almacén
  o de otro esquema (con la tabla) (PY1–PY3 de [tables-projection.prd.md](tables-projection.prd.md)).»*
- [tables-schema.prd.md](tables-schema.prd.md), E17: *«La lista de reglas de valor no válido añade al final
  proyección sin columnas, y la precedencia vale para preparar una proyección como dice PY1 de
  [tables-projection.prd.md](tables-projection.prd.md).»*
- [tables-schema.prd.md](tables-schema.prd.md), E19: *«Preparar una proyección, obtener sus columnas preparadas, ligarle
  una fila y leer con ella no fallan con daño: los daños de la fila y del historial aparecen en la lectura o en el paso
  del recorrido que la devuelve (PY3 de [tables-projection.prd.md](tables-projection.prd.md)).»*
- [tables-schema.prd.md](tables-schema.prd.md), *Cargas del banco*, W26 y W27: *«Krummi lee las columnas de las filas
  con una proyección, y preparar la proyección cuenta en el tiempo medido de cada repetición; también lo harán las
  cargas de tablas futuras que lean valores de columnas, salvo que su PRD diga otra cosa. Las cargas, sus sumas de
  comprobación y sus umbrales no cambian (PY7 de [tables-projection.prd.md](tables-projection.prd.md)).»*
- [tables.prd.md](tables.prd.md), N10: *«Esquema distinto también cubre los usos de una proyección que dependen del
  esquema (PY1–PY3 de [tables-projection.prd.md](tables-projection.prd.md)).»*
- [benchmark.prd.md](../benchmark/benchmark.prd.md), K7: *«En W26 y W27, y en las cargas de tablas futuras que lean
  valores de columnas salvo que su PRD diga otra cosa, Krummi lee sus filas con una proyección, la lectura con las
  capacidades reales del motor, como en W28, y preparar la proyección cuenta en el tiempo medido de cada repetición;
  Voron lee sus campos por su posición. Las cargas y los umbrales no cambian (PY7 de
  [tables-projection.prd.md](../tables/tables-projection.prd.md)).»*

## Referencias

- PRD padre: [tables-row-reads.prd.md](tables-row-reads.prd.md) (L1, L2, AC-L3, AC-L5, AC-L9); su padre,
  [tables-schema.prd.md](tables-schema.prd.md) (E2, E6, E11, E12, E16, E17, E18, E19, E20, *Cargas del banco*); su
  padre, [tables.prd.md](tables.prd.md) (N3, N4, N10, *Salida*); la raíz, [krummi.prd.md](../krummi.prd.md) (D8, D11).
- Árboles: [trees.prd.md](../trees/trees.prd.md) (A6, B7, B8).
- Transacciones: [transactions.prd.md](../transactions/transactions.prd.md) (T6, T8, T10).
- Banco: [benchmark.prd.md](../benchmark/benchmark.prd.md) (K7, K13).
- Diagnóstico de rendimiento del 2026-11-15,
  [informe-rendimiento.md](../../../performance/2026-11-15-diagnostic/informe-rendimiento.md), tercera ronda
  (proyección preparada compartida), con los A/B de W26 y W27, los costes de preparar y de comprobar, y las pruebas
  del prototipo.
