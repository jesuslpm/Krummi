# Árboles de tamaño fijo de Krummi — PRD

**Versión**: 1.0
**Fecha**: 2026-11-02
**Documento padre**: [Árboles B+ de Krummi — PRD](trees.prd.md) (v1.0)

## Objetivo y contexto

Construir el PRD hijo **5.5** del [reparto del módulo de árboles](trees.prd.md#reparto-en-prd-hijos): árboles de
tamaño fijo, cuyas claves son enteros sin signo de 64 bits en orden numérico (B5) y cuyos valores tienen todos la
misma longitud, fijada al crear el árbol (B3), con las operaciones por clave de 5.1 y los recorridos de 5.2.

Al cerrar este cambio, una aplicación que indexa por un número (un identificador, una marca de tiempo, un número de
página) puede guardar esas entradas sin convertir el número en bytes ordenables y sin pagar por cada entrada las
longitudes de una clave y de un valor que no cambian. Como en Voron, el motivo es la densidad y el coste de CPU, no
una función nueva: un árbol normal con la clave guardada en 8 bytes big-endian ya ordena igual. Por eso el cambio
incluye una medición que compara los dos (X13).

**Por qué ahora**: es el quinto submódulo del reparto. Las estructuras posteriores que guardan enteros por número,
como las tablas (módulo 7), pueden apoyarse en él.

Este PRD hereda todo lo de su ascendencia, el [módulo de árboles](trees.prd.md) (B1–B12) y la
[raíz](krummi.prd.md), y lo de los módulos anteriores que el padre hereda: el [Pager](pager.prd.md) (P1–P17), las
[Transacciones](transactions.prd.md) (T1–T18), el [Diario](journal.prd.md) (J1–J17), la
[escritura agrupada y directa del diario](journal-group-commit.prd.md) (E1–E16) y el
[Espacio libre](free-space.prd.md) (F1–F14). Reutiliza además lo que fijaron sus hermanos
[Árboles y operaciones por clave](trees-keys.prd.md) (A1–A15 y sus anotaciones),
[Recorrido en orden y búsqueda por prefijo](trees-scan.prd.md) (R1–R11),
[Valores grandes](trees-large-values.prd.md) (V1–V11) y [Árboles multivalor](trees-multivalue.prd.md) (M1–M15), y
el cambio [Comprobación de organización recordada de los nodos](trees-node-check-cache.prd.md) (C1–C9). Aquí solo
se añade lo propio de 5.5. Lo que cambia o precisa reglas anteriores está declarado en sus decisiones y anotado en
los documentos afectados (ver [Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

En este documento, la **longitud** de un árbol de tamaño fijo es el número de bytes que tienen todos sus valores.

### Decisiones de producto

| ID | Decisión |
|---|---|
| X1 | **Longitud de los valores** (precisa B3 y B6): la longitud de un árbol de tamaño fijo se fija al crearlo y es de 0 a 2.048 bytes, el mismo máximo que una clave (B5) y que un valor multivalor (M1). No cambia mientras el árbol existe: vaciarlo (A2) y renombrarlo (A3) la conservan. Todo valor que recibe una operación tiene exactamente esa longitud; con otra, falla con tamaño no válido, como valor, con el tamaño recibido y la longitud del árbol como mínimo y como máximo (B8). Con longitud `0`, el árbol guarda solo claves y cada valor es vacío; Leer sigue distinguiendo una clave con su valor vacío de una que no está (A4). En los árboles de tamaño fijo no hay valores fuera de su hoja (5.3). |
| X2 | **Longitud pedida no válida**: toda apertura de un árbol de tamaño fijo pide su longitud, en todos los modos de A1, también en Abrir y en una transacción de lectura. Una longitud pedida de más de 2.048 bytes falla con tamaño no válido, como valor, con el número pedido como tamaño recibido, mínimo `0` y máximo `2048`, sin crear ni abrir nada. Como una `ArgumentException` de .NET, es un argumento no válido por sí mismo, que se decide solo con los argumentos sin mirar el almacén, así que gana a todo lo demás salvo al tamaño no válido del nombre (B7, X11): a árbol no encontrado, a árbol ya existente, al tipo de árbol distinto y a cualquier fallo al leer el catálogo. |
| X3 | **El tipo incluye la longitud** (precisa B3 y B8): el tipo de un árbol de tamaño fijo es «tamaño fijo de N bytes», y dos longitudes distintas son dos tipos distintos. Abrir o crear, en cualquier modo de A1, como tamaño fijo de N bytes un árbol existente de tamaño fijo con otra longitud, o un árbol normal o multivalor, falla con tipo de árbol distinto; y abrir o crear como normal o multivalor un árbol de tamaño fijo, también. Los datos del error (B8) llevan el tipo que tiene el árbol y el tipo pedido, cada uno con su longitud cuando es de tamaño fijo. Listar (A3) informa el tipo de cada árbol de tamaño fijo con su longitud, y Tipo (X15) el de un árbol por su nombre. Las operaciones que nombran un árbol sin pedir un tipo (Existe, Tipo, Borrar, Renombrar) nunca fallan con tipo de árbol distinto (B3). |
| X4 | **Apertura por tipo** (como M4): un árbol de tamaño fijo se abre con los mismos modos de A1, con su propia apertura, que solo tiene las operaciones de su tipo. Usar con su apertura una operación de los árboles normales (5.1, 5.2) o multivalor (5.4) no compila, y usar las operaciones de este PRD con la apertura de un árbol normal o multivalor tampoco. El número de entradas (A5) está en las aperturas de los tres tipos. El tipo pedido al abrir se sigue comprobando contra el catálogo (X3). Las operaciones del catálogo (Existe, Borrar, Renombrar y Listar, A3, y Tipo, X15) valen para los tres tipos. Las reglas de las aperturas de B1, B9, A6, A7, A8 y de la excepción a B1 anotada en el padre (uso incorrecto con una apertura de otra transacción) valen igual para las de un árbol de tamaño fijo. |
| X5 | **Claves** (precisa B5): una clave es un entero sin signo de 64 bits, de `0` a `2^64−1`, y las claves se ordenan por su valor numérico. Todo entero es una clave válida, así que ninguna clave ni ningún límite de un recorrido falla nunca con tamaño no válido. Las operaciones reciben y devuelven las claves como enteros, no como bytes. |
| X6 | **Operaciones por clave** (como A4 y A5): **Guardar**, **Añadir**, **Leer** y **Quitar**, con las reglas de A4, y el **número de entradas**, con las de A5. Leer devuelve el valor como una referencia sin copia (D11, T16), con la vida de las de Leer de 5.1 (B9, A10, T8); con longitud `0`, el valor es vacío. Leer está en las dos clases de transacción; Guardar, Añadir y Quitar, solo en las de escritura. La única validación de tamaño es la longitud del valor en Guardar y Añadir (X1); Añadir con un valor de otra longitud falla aunque la clave ya esté. |
| X7 | **Recorridos y Quitar intervalo** (como R1–R3 y R6–R9): un recorrido recibe un límite inferior y uno superior, cada uno un entero incluido o excluido, o abierto, y se consume por los dos extremos con las reglas de R1 y R2. Devuelve cada entrada como su clave, un entero, y su valor, una referencia sin copia con la vida de R6. El intervalo está vacío cuando ningún entero cumple los dos límites (amplía R3 a las claves enteras): cuando el límite inferior es mayor que el superior; cuando los dos son el mismo entero y alguno está excluido; cuando los dos están excluidos y son enteros contiguos, como `5` y `6`; cuando el inferior es `2^64−1` excluido; o cuando el superior es `0` excluido. Esos son exactamente los casos de la regla. Como en R3, se decide solo con los argumentos: el recorrido no devuelve nada y Quitar intervalo quita `0` sin leer ninguna página, así que no pueden fallar por daño. **Quitar intervalo** quita las entradas de un intervalo con esas mismas reglas y las de R7, y devuelve cuántas quitó. No hay prefijo (R4): un prefijo de bytes no tiene sentido sobre enteros, y los límites expresan cualquier intervalo de claves. |
| X8 | **Cambios y referencias** (como A10, R6 y R7): terminan las referencias y los recorridos anteriores todas las operaciones que pueden modificar: los modos distintos de Abrir, Borrar, Renombrar, Guardar, Añadir, Quitar y Quitar intervalo, aunque no cambien nada o fallen. Abrir, Existe, Tipo, Listar, Leer, el número de entradas y pedir un recorrido o sacar entradas de él no los terminan. Para T10 son cambios los mismos que en A10 y R7: Guardar lo es siempre, también con el mismo valor que ya tenía la clave; Añadir, Quitar y Quitar intervalo, cuando añaden o quitan algo. No lo son esas operaciones cuando no cambian nada ni las que fallan por un error de validación pública. Vaciar es siempre un cambio (A2). |
| X9 | **Espacio** (F8): las páginas que un árbol de tamaño fijo deja de usar vuelven al espacio libre, y todas las suyas vuelven al vaciarlo (A2), al borrarlo y al sobrescribirlo con Renombrar (A3). Un árbol de tamaño fijo vaciado ocupa lo mismo que uno recién creado vacío, y uno que se queda sin entradas, por cualquier combinación de Quitar y Quitar intervalo, lo mismo que uno vaciado. |
| X10 | **Daño** (G6, como A9 y R8): los modos Abrir, Crear nuevo y Abrir o crear, Existe, Tipo, Listar, Renombrar sin sobrescritura o a su mismo nombre y el número de entradas nunca fallan con daño en las páginas del propio árbol. Leer, Guardar, Añadir, Quitar, los recorridos y Quitar intervalo fallan con la clase de daño y el número de la página cuando necesitan una página dañada del árbol, también cuando el resultado habría sido no añadir o no quitar nada; las páginas que necesitan son las de A9 y R8. Borrar, vaciar y Renombrar con sobrescritura pueden fallar con daño en cualquier página del árbol afectado. Una página dañada falla igual en cada operación posterior que la necesite (F9), salvo el riesgo residual de la excepción a G6 del padre raíz (X14, C7). En una escritura, todo fallo por daño sigue T11. |
| X11 | **Errores**: no hay clases nuevas. La precedencia es la de A7 y R8: primero el tamaño no válido, del nombre antes que el de la longitud pedida al abrir (X2); en Guardar y Añadir, el del valor (X1); después, una apertura de otra transacción (uso incorrecto) y, tras ella, una apertura de un árbol que la misma escritura borró (árbol no encontrado, con el último nombre del árbol), las dos sin leer ninguna página; después, los fallos al leer el catálogo, el tipo de árbol distinto, árbol no encontrado y árbol ya existente; y, por último, los fallos al leer las páginas del árbol. |
| X12 | **Formato `9`**: el cambio es incompatible (B12): un motor de formato `8` no conoce el tipo de tamaño fijo y toma su entrada en el catálogo por dañada, así que leería mal un almacén sano: Listar, y abrir, borrar o renombrar sobre ese árbol, fallarían con la clase de daño. El cambio no cambia el tamaño ni las páginas libres de un almacén sin árboles (A12). La versión de formato vigente pasa a ser `9`, y un almacén de formato `8` es de formato anterior aunque esté vacío; antes de la 1.0 no hay migración y los datos se recrean (D7). Los números que el formato guarda para un árbol de tamaño fijo, como su longitud y sus claves, se guardan en little endian, como el resto del formato (V3); el orden de las claves es el numérico (X5), no el de sus bytes. |
| X13 | **Medición** (B12, sin umbral por D5): el cambio incluye una medición reproducible, en Windows y en Linux, en un almacén en disco con confirmación durable, que compara un árbol de tamaño fijo con un árbol normal que guarda la misma clave en 8 bytes big-endian, con valores de 0, 8 y 100 bytes de contenido aleatorio. Las claves son 1 millón de enteros distintos elegidos al azar entre `0` y `2^64−1`, y las claves y los valores son los mismos en todas las cargas por una semilla fija. En el árbol normal, convertir cada clave en sus 8 bytes al guardarla y de vuelta en un entero al leerla forma parte del tiempo medido. Para cada tipo de árbol y cada longitud de valor mide: guardar las entradas en orden de clave en un almacén y, en otro, al azar, las dos cargas en escrituras de 1.000 entradas; el tamaño del almacén en bytes que da su información (P12) tras cada una de las dos cargas; y, en el almacén cargado al azar, tras cerrarlo y reabrirlo sin vaciar la caché del sistema, leer todas las entradas al azar en una sola transacción de lectura y recorrerlas todas hacia delante en otra, leyendo en los dos casos la clave y los bytes de cada valor devueltos, y quitarlas todas al azar en escrituras de 1.000. El informe incluye el sistema operativo, el procesador y el disco. Sus resultados van en el informe de cierre. |
| X14 | **Comprobación de organización recordada** (precisa C1): las páginas de un árbol de tamaño fijo son nodos a efectos de C1–C8 y usan la comprobación recordada igual que los demás nodos y sobre los mismos bytes: se recuerda que el contenido de un nodo pasó la comprobación de organización, identificado por el número de su página y por su código de integridad, en la memoria de nodos recordados del almacén, compartida con los demás árboles. Valen para ellos la garantía y el riesgo residual de C7. Si lo recordado se usa para todos sus nodos o solo a partir de cierto número de entradas queda dejado a la implementación, como para los demás nodos (dejado a la implementación de la comprobación recordada). |
| X15 | **Tipo de un árbol** (amplía A3): una operación nueva del catálogo, **Tipo**, recibe un nombre y devuelve el tipo del árbol que lo tiene, con su longitud cuando es de tamaño fijo, o que no existe; no falla porque no exista, como Existe. Vale para los tres tipos y está en las dos clases de transacción. Valida el tamaño del nombre (B4), nunca falla con tipo de árbol distinto (B3) y solo lee el catálogo: puede fallar con los fallos al leer el catálogo (daño, memoria agotada, entrada y salida), pero nunca con daño en las páginas del propio árbol. Su precedencia es la de Existe: primero el tamaño no válido del nombre y después los fallos al leer el catálogo. No modifica nada: no termina las referencias ni los recorridos (X8) y no es un cambio para T10. Ve lo que ve su transacción, como Existe y Listar (A3): en una escritura, con sus propios cambios del catálogo (árboles creados, borrados, renombrados o recreados con otro tipo), y en una lectura, su instantánea; devuelve lo mismo que Listar informa para ese nombre. |

*Anotaciones del 2026-11-05 desde la integridad recordada de las páginas ([pager-integrity-once.prd.md](pager-integrity-once.prd.md)):*

- *X10: una página dañada sigue fallando en cada operación que la necesite, salvo el riesgo residual de la decisión I8, que sustituye al de C7.*
- *AC-X19: su primera parte se precisa como AC-C1 (la integridad de cada nodo se comprueba en su primera lectura, no en todas), y su cuarta parte (un byte cambiado con el almacén abierto en una hoja recordada) se sustituye por AC-I8.*

*Anotación del 2026-11-06 desde las comprobaciones de la escritura ([pager-writer-checks.prd.md](pager-writer-checks.prd.md)): la cuarta parte de AC-X19, que AC-I8 había sustituido, pasa a AC-S6.*

### Estado de partida

Verificado el 2026-11-02 en el código:

| Aspecto | Situación |
|---|---|
| API pública | Los árboles normales tienen las operaciones de 5.1, los recorridos y Quitar intervalo de 5.2 y los valores de hasta 256 MiB de 5.3; los multivalor, las de 5.4. Cada tipo tiene su propia apertura, y usar una operación de un tipo con la apertura de otro no compila (M4). El número de entradas es común a los dos tipos. |
| Tipos | Hay dos tipos, normal y multivalor; la lista de tipos admite tipos nuevos sin romper el código de las aplicaciones que la usan, y no tiene ningún valor reservado para el tamaño fijo. Listar devuelve el nombre y el tipo de cada árbol; no hay ninguna operación que dé el tipo de un solo árbol por su nombre, y Existe solo dice si existe. El tipo de árbol distinto informa el tipo que tiene y el pedido, sin más datos. |
| Claves | Las claves de los árboles normales y multivalor son bytes de 1 a 2.048, ordenados byte a byte (B5). Un entero guardado en big-endian se ordena en orden numérico; en little endian, no. |
| Densidad | Una entrada de un árbol normal con una clave de 8 bytes y un valor de N bytes ocupa 14 + N bytes de su hoja: 8 de la clave, N del valor y 6 de su posición y sus dos longitudes. |
| Errores | Dieciséis clases, las del estado de partida de 5.4. Tamaño no válido informa nombre, clave, valor o prefijo. |
| Formato | La versión de formato vigente es `8` (M14). Se conservan como datos de prueba un almacén de formato `6` creado con la versión de 5.1 y uno de formato `7` creado con la de 5.3; no se conserva ninguno de formato `8`. |
| Medición | Hay mediciones de los árboles normales (A15), de los recorridos (R11), de los valores grandes (V11), de los multivalor (M15) y de la comprobación recordada (C9); ninguna compara claves enteras. |
| Voron (referencia) | Sus árboles de tamaño fijo tienen claves enteras de 64 bits con signo y valores de una longitud fija que se da al abrirlos; tienen añadir, borrar, leer, contar, recorrer desde una clave y borrar un intervalo. |

## Actores

- **Desarrollador integrador**: crea y abre árboles de tamaño fijo con su longitud, guarda, lee, quita y recorre
  entradas por clave entera, y trata los errores por su clase.
- **Mantenedor**: ejecuta la medición de X13 en Windows y en Linux.

## Flujo

1. La aplicación abre una transacción y un árbol de tamaño fijo con su propia apertura, su nombre y su longitud
   (X2–X4).
2. En una escritura guarda, añade y quita entradas, o quita intervalos de claves (X6, X7).
3. En cualquier transacción lee entradas por clave, consulta el número de entradas y recorre el árbol (X6, X7).
4. La confirmación aplica todos los cambios, o ninguno (G1).

## Validaciones y mensajes

Valen las de los ascendientes y las de 5.1 a 5.4, incluidos los fallos de T11 y T12, con la precedencia de X11.
Este PRD añade estas situaciones, solo de la biblioteca (la herramienta no abre almacenes hasta el módulo 12):

| Situación | Regla | Clase de error |
|---|---|---|
| Se abre o se crea un árbol de tamaño fijo pidiendo una longitud de más de 2.048 bytes, en cualquier modo | X2 | Tamaño no válido, como valor, con mínimo `0` y máximo `2048` |
| Se guarda o se añade un valor de una longitud distinta de la del árbol | X1, X6 | Tamaño no válido, como valor, con la longitud del árbol como mínimo y como máximo |
| Se abre o se crea como tamaño fijo de N bytes un árbol de tamaño fijo de otra longitud, o uno normal o multivalor; o como normal o multivalor uno de tamaño fijo | X3 | Tipo de árbol distinto, con los dos tipos y sus longitudes |
| Se opera con la apertura de un árbol de tamaño fijo de otra transacción del mismo almacén | X4, X11 | Uso incorrecto |
| Se opera con la apertura de un árbol de tamaño fijo que la misma escritura borró | X4, X11, A6 | Árbol no encontrado |
| Una operación necesita una página dañada del árbol | X10, G6 | Daño, con el número de la página |
| Se usa una operación de un tipo con la apertura de un árbol de otro tipo, o una operación de escritura en una transacción de lectura | X4, X6, B1 | Ninguna: no compila |
| Cualquier clave o límite entero, incluidos `0` y `2^64−1` | X5 | Ninguna: es válido |
| Se pide el tipo de un nombre que no tiene ningún árbol | X15 | Ninguna: devuelve que no existe |
| Se añade una clave que ya está, se lee o se quita una que no está, o se recorre un intervalo vacío o sin claves | X6, X7 | Ninguna: devuelve que no añadió, que no está, que no estaba, `0` o nada |

## Salida

El módulo no escribe nada en pantalla. La información del almacén (P12) conserva sus datos; la versión de formato
que informa pasa a ser `9` (X12). Listar informa cada árbol de tamaño fijo con su tipo y su longitud; los normales y
los multivalor se siguen informando como hasta ahora. Los valores se devuelven con los bytes guardados, sin cambios,
y las claves como enteros, en orden numérico.

Ejemplo, para un árbol de tamaño fijo `contadores` de longitud 8, con las claves `0`, `7` y `2^64−1`:

| Operación | Resultado |
|---|---|
| Listar | `contadores`, tamaño fijo de 8 bytes |
| Tipo de `contadores` | tamaño fijo de 8 bytes |
| Tipo de `pedidos`, que no existe | no existe |
| Recorrido completo, hacia delante | `0`, `7`, `2^64−1`, cada una con su valor de 8 bytes |
| Recorrido desde `7` excluido hasta el final, hacia atrás | `2^64−1` |
| Recorrido de `8` a `5`, los dos incluidos | nada (intervalo vacío) |
| Leer `3` | no está |
| Añadir `7` con un valor de 8 bytes | no añadió |
| Quitar intervalo de `1` a `2^64−1`, los dos incluidos | `2`; el número de entradas pasa a `1` |

Ejemplo de los datos de un error de tamaño no válido, por guardar un valor de 9 bytes en ese árbol, en un almacén
en disco:

| Dato | Valor |
|---|---|
| Carpeta | la del almacén |
| Qué incumple su tamaño | valor |
| Tamaño recibido | `9` |
| Mínimo | `8` |
| Máximo | `8` |

Ejemplo de los datos de un error de tipo de árbol distinto, por abrir `contadores` como tamaño fijo de 16 bytes:

| Dato | Valor |
|---|---|
| Carpeta | la del almacén |
| Nombre | `contadores` |
| Tipo que tiene | tamaño fijo de 8 bytes |
| Tipo pedido | tamaño fijo de 16 bytes |

## Reutilización

- Las reglas comunes del módulo de árboles (B1–B12) y las de 5.1 a 5.4: modos de apertura, catálogo, errores,
  precedencia, daño, identidad, cambios, número de entradas, recorridos con sus límites y extremos, Quitar intervalo
  y aperturas por tipo.
- La clase de tamaño no válido con su dato de valor, y la de tipo de árbol distinto, sin clases nuevas (X11).
- El espacio libre (F4–F8) para las páginas que se liberan (X9).
- Las transacciones, su instantánea y las lecturas sin copia (T2, T14, T16, D11).
- El diario y la recuperación (G1, G5): los cambios de los árboles de tamaño fijo viajan como cualquier otro cambio.
- La comprobación de integridad y la clase de daño (G6), y la comprobación de organización recordada (C1–C8, X14).
- La simulación de caídas, cortes de corriente y fallos de las pruebas de los módulos 3 a 5.4.
- La medición de 5.1 (A15) como modelo de cargas y de informe para X13.
- Los almacenes de formato `6` y `7` conservados como datos de prueba, que siguen rechazándose como formato anterior,
  y uno de formato `8` que la ejecución crea con la versión anterior a este cambio y conserva igual (AC-X16).

## Fuera de alcance

- Claves enteras con signo (padre): la aplicación que las necesita las convierte ella misma en enteros sin signo
  ordenados; claves enteras de otro ancho (32 o 128 bits) y claves de tamaño fijo que no son enteros.
- Valores de longitud variable o de más de 2.048 bytes en un árbol de tamaño fijo, valores fuera de su hoja, y árboles
  de tamaño fijo multivalor.
- Cambiar la longitud de un árbol existente y convertir un árbol de un tipo en otro (B3, X1, X3).
- Abrir un árbol de tamaño fijo sin dar su longitud (X2); quien no la sabe la obtiene con Tipo (X15) o con Listar (X3).
- Búsqueda por prefijo en los árboles de tamaño fijo (X7), y lo que el recorrido en orden ya deja fuera de alcance,
  como contar las entradas de un intervalo sin recorrerlo.
- Árboles de tamaño fijo guardados dentro de una entrada de otro árbol, como hace Voron; aquí son siempre árboles del
  catálogo.
- Operaciones de lectura y escritura combinadas, como incrementar un contador (padre), y leer o guardar varias
  entradas en una sola operación.
- Usar árboles de tamaño fijo dentro del propio motor, por ejemplo para el espacio libre.
- Compresión de páginas hoja (5.6); si los árboles de tamaño fijo se pueden comprimir lo decide 5.6.
- Migrar un almacén de formato `8` (D7).
- Umbrales de rendimiento (D5, módulo 6).

## Dejado a la implementación

- La organización de las páginas de un árbol de tamaño fijo, siempre que cumpla X12, y su política de división y
  fusión (padre).
- Cómo se pide el tipo con su longitud al abrir y cómo se representa en Listar, en Tipo y en los datos de tipo de árbol
  distinto, siempre que cumpla X3; y los nombres de la apertura y de las operaciones nuevas (padre).
- Qué página se informa en un error de daño cuando una operación necesita varias dañadas (X10), siempre que sea una
  de las que necesita, también al repetir la operación.
- Qué fallo de T11 se informa cuando una misma operación provocaría varios, según lo que intente primero, y qué ve la
  propia escritura después de ese fallo, que ya no puede confirmarse.
- La orden que ejecuta la medición de X13 y la forma de su informe.

## Criterios de aceptación

Valen las condiciones de prueba de los módulos anteriores y la cabecera de los criterios del padre: cada criterio va
en un almacén en disco y en modo temporal, salvo las partes que reabren el almacén o dañan sus archivos, que son
solo en disco. Este PRD comprueba los criterios del padre asignados a 5.5: AC-B7 en su parte de tamaño fijo
(AC-X1), AC-B16 (AC-X3) y AC-B8 con tamaño fijo (AC-X4), y AC-B17 y AC-B18 con árboles de tamaño fijo (AC-X11 y
AC-X12); repite con árboles de tamaño fijo AC10 y AC20 de la raíz (AC-X13 y AC-X14) y AC-B4, AC-B9 y AC-B13 del padre
con la longitud pedida (AC-X17), y añade estos. Las pruebas que dependen de la organización de las páginas, de
páginas dañadas, de fallos en puntos concretos o de contar las comprobaciones de organización usan el conocimiento de
la propia implementación.

1. **AC-X1** — Dado un árbol de tamaño fijo, cuando se guardan en orden aleatorio las claves `0`, `1`, `2^63−1`,
   `2^63`, `2^64−2` y `2^64−1` y otras al azar, entonces un recorrido completo las devuelve en orden numérico hacia
   delante y en el inverso hacia atrás, en la misma escritura, en una lectura posterior y, en disco, tras reabrir
   (X5, AC-B7).
2. **AC-X2** — Dada una escritura, cuando crea árboles de tamaño fijo de longitudes `0`, `1`, `8` y `2048`,
   entonces se crean, y en cada uno se guardan y se leen idénticos valores de su longitud; en el de longitud `0`,
   Leer distingue una clave guardada, con su valor vacío, de una que no está; y cuando abre o crea uno de longitud
   `2049`, en cada modo de A1 y también en Abrir en una transacción de lectura, con y sin un árbol de ese nombre,
   entonces falla sin efecto con la clase de tamaño no válido, como valor, con el tamaño recibido `2049`, mínimo `0`
   y máximo `2048`, y no crea nada (X1, X2).
3. **AC-X3** — Dado un árbol de tamaño fijo de longitud 8, cuando se guarda o se añade un valor de 0, 7 o 9 bytes,
   también con una clave que ya está, entonces falla sin efecto con la clase de tamaño no válido, como valor, con el
   tamaño recibido, mínimo `8` y máximo `8`, y la transacción sigue utilizable; y con un árbol de longitud 0 y un
   valor de 1 byte, igual, con mínimo `0` y máximo `0` (X1, X6, AC-B16).
4. **AC-X4** — Dado un árbol de tamaño fijo de longitud 8, uno normal y uno multivalor, cuando se abre o se crea, en
   cada modo de A1, el de tamaño fijo como tamaño fijo de 16 bytes, como normal y como multivalor, y los otros dos
   como tamaño fijo de 8 bytes, entonces falla con la clase de tipo de árbol distinto, con el nombre, el tipo que tiene
   y el tipo pedido, con sus longitudes, y el árbol queda intacto; falla igual con una página propia del árbol dañada
   y el catálogo intacto; borrarlo sin pedir tipo nunca falla con esa clase; Listar informa cada árbol con su tipo y,
   en el de tamaño fijo, su longitud; renombrarlo y vaciarlo conservan su longitud; Renombrar con sobrescritura borra
   un árbol de tamaño fijo de destino; y tras borrar el de tamaño fijo, crear otro con su nombre y otra longitud lo
   crea (X3, X4, AC-B8).
5. **AC-X5** — Dado un árbol de tamaño fijo vacío, cuando se guardan, se añaden, se leen y se quitan entradas, con
   las claves `0` y `2^64−1` entre ellas, entonces Guardar crea o sobrescribe, Añadir devuelve que añadió la primera
   vez y que no añadió la segunda sin cambiar nada, Leer devuelve el valor guardado o que no está, Quitar devuelve que
   estaba la primera vez y que no estaba la segunda, y el número de entradas coincide en cada paso con las claves
   guardadas y no quitadas (X6, A4, A5).
6. **AC-X6** — Dado un árbol de tamaño fijo con entradas, cuando se recorre con límites incluidos, excluidos y abiertos,
   con `0` y `2^64−1` como límites y con límites que no son claves del árbol, alternando los dos extremos, entonces cada
   recorrido devuelve exactamente lo que dicen R1 y R2 en orden numérico; con cada caso de intervalo vacío de X7 (el
   límite inferior mayor que el superior, los dos iguales y alguno excluido, `5` y `6` excluidos, desde `2^64−1`
   excluido y hasta `0` excluido), el recorrido no devuelve nada y Quitar intervalo quita `0`, también con la página
   raíz del árbol dañada (en disco); y cuando se quitan intervalos, también todo el árbol, entonces Quitar intervalo
   devuelve cuántas quitó, el número de entradas baja en esa cantidad y un recorrido devuelve las demás (X7, R1–R3, R7).
7. **AC-X7** — Dado el código de una aplicación de prueba, cuando usa con la apertura de un árbol de tamaño fijo una
   operación de los árboles normales o multivalor, o una operación de este PRD con la apertura de un árbol normal o
   multivalor, o Guardar, Añadir, Quitar o Quitar intervalo en una transacción de lectura, o usa una referencia o un
   recorrido después de una de las operaciones de X8 que falló sin efecto, o pasa a otro hilo la apertura de un árbol
   de tamaño fijo de una escritura o un recorrido suyo, entonces no compila; pasar a otro hilo la de una lectura y
   sus recorridos compila; y Leer, el número de entradas y los recorridos no terminan las referencias ni los
   recorridos anteriores (X4, X8, B1, R9, AC-B11).
8. **AC-X8** — Dado un almacén con un árbol de tamaño fijo ya creado, que se mantiene dentro de una sola región del
   espacio libre, tomando las páginas en uso como en AC-A15: cuando se crea un árbol de tamaño fijo vacío, se confirma,
   se toma el valor, se le guardan entradas suficientes para varios niveles y se confirma, y después se le quitan todas
   con cada una de estas formas en pruebas separadas (Quitar, Quitar intervalo, una combinación de los dos y vaciarlo),
   entonces el valor final es el tomado con el árbol vacío; y cuando el árbol se borra o se sobrescribe con Renombrar,
   el valor final es el de antes de crearlo cuando su entrada no hizo crecer el catálogo, como en AC-A15 (X9, A2, A3).
9. **AC-X9** — Dada una escritura con un árbol de tamaño fijo abierto dos veces, y otra transacción del mismo
   almacén, cuando lo borra a través de una apertura y usa la otra con cada operación de este PRD, también tras crear
   otro árbol con el mismo nombre, entonces falla sin efecto con árbol no encontrado y el último nombre del árbol;
   cuando usa la apertura en la otra transacción, entonces falla sin efecto con uso incorrecto; y con además un valor
   de otra longitud, en los dos casos, el error es de tamaño no válido (X4, X11, A6, A7, AC-B14).
10. **AC-X10** — Dado un árbol de tamaño fijo en disco de al menos dos niveles, con una página dañada en la parte que
    lleva a unas claves, cuando se leen, se guardan, se añaden o se quitan esas claves, también una que ya está o que no
    está, o se recorre o se quita un intervalo que las contiene, entonces cada operación falla con la clase de daño y el
    número de la página, y vuelve a fallar igual al repetirla (salvo el riesgo residual de C7, que la prueba no
    provoca); el recorrido termina tras el fallo y sus entradas anteriores son correctas; las claves cuyo camino desde
    la raíz no pasa por esa página se leen sin error; los modos Abrir, Crear nuevo y Abrir o crear, Existe, Tipo,
    Listar, Renombrar sin sobrescritura y el número de entradas nunca fallan con daño en las páginas del propio árbol, y
    dan su resultado o su error de siempre (por ejemplo, Crear nuevo sobre ese árbol falla con árbol ya existente); y
    cuando una escritura vacía, borra o sobrescribe con Renombrar ese árbol, entonces puede fallar con la clase de daño
    y la escritura sigue T11 (X10, A9, R8, G6, F9).
11. **AC-X11** — Dada una transacción de lectura abierta y una escritura que, con inserciones y borrados, divide y
    fusiona páginas de un árbol de tamaño fijo hasta cambiar su altura, cuando la lectura lee todas las entradas y el
    número de entradas antes y después de que la escritura se confirme, entonces ve exactamente las de su instantánea;
    y una lectura nueva ve las de la escritura (G3, T14, AC-B17).
12. **AC-X12** — Dada una escritura que divide y fusiona páginas de un árbol de tamaño fijo, cuando se provoca una
    caída del proceso y simulada de corriente en cada punto en que su confirmación escribe o lleva datos al disco,
    entonces al reabrir el árbol tiene todas sus entradas de antes o todas las de después, con su número de entradas;
    y cuando se simula un fallo de T11 distinto de entrada y salida en cada punto en que una operación reserva, lee o
    libera páginas, entonces confirmar falla con la clase del primer fallo y no aplica nada, y tras abortar la
    escritura el árbol y su número de entradas están como antes y una escritura nueva funciona sin error (G1, G5,
    T11, AC-B18).
13. **AC-X13** — Dado un almacén pequeño con un árbol de tamaño fijo de al menos dos niveles, cuando se altera cada
    uno de sus bytes por separado, se abre y se leen todas sus entradas, entonces cada alteración se detecta como daño
    o no tiene efecto, con las reglas de AC10 de la raíz y sus anotaciones; nunca se devuelve una clave o un valor con
    un byte distinto (X10, G6, AC10).
14. **AC-X14** — Dados los valores que devuelven Leer y los recorridos de un árbol de tamaño fijo de longitud 8, cuando
    se leen, entonces las referencias están dentro de la memoria del motor que fija T16 y no en una copia, en disco y en
    modo temporal, y usar una referencia después de terminar la transacción no compila (X6, X7, AC20).
15. **AC-X15** — Dada una escritura en disco que solo hace Abrir o crear sobre un árbol de tamaño fijo existente,
    Añadir de claves que ya están, Quitar de claves que no están, Quitar intervalo que no quita nada, Leer, el número
    de entradas, recorridos, y operaciones que fallan por tamaño no válido o por tipo de árbol distinto, cuando se
    confirma, entonces termina sin error y los archivos del almacén quedan idénticos byte a byte (X8, T10).
16. **AC-X16** — Dado un almacén de formato `8` que la ejecución crea con la versión anterior a este cambio, compilada
    aparte, y conserva como dato de prueba, y los de formato `6` y `7` ya conservados, cuando se abren, entonces
    fallan con la clase de formato distinto como formato anterior y no se modifican; un almacén nuevo informa la
    versión de formato `9`, con el mismo tamaño y las mismas páginas libres que un almacén nuevo de formato `8`
    mientras no tiene árboles; y leyendo en disco los bytes de una hoja de un árbol de tamaño fijo, con el
    conocimiento de la implementación, sus claves, tal como la hoja las guarda, y la longitud del árbol están en little
    endian (X12, A12, AC-B12).
17. **AC-X17** — Dada una operación que abre un árbol de tamaño fijo con un nombre de 1.025 bytes y una longitud de
    2.049, cuando se ejecuta, entonces el error de tamaño no válido es del nombre; y dada una apertura con un nombre
    válido y una longitud de 2.049, cuando se ejecuta sobre un árbol existente de tamaño fijo de otra longitud, sobre
    uno normal, sobre uno que no existe en el modo Abrir, sobre uno existente en el modo Crear nuevo y, en disco, con
    la página del catálogo dañada, entonces falla en todos los casos con tamaño no válido, sin efecto; en un almacén
    que no acepta escrituras (T12), falla con esa clase y no con la de entrada y salida; y una escritura con cambios
    antes y después de fallos de X1 y X2 se confirma sin error y aplica todos esos cambios (X2, X11, AC-B4, AC-B9,
    AC-B13).
18. **AC-X18** — Dada una escritura en disco con un árbol de tamaño fijo, cuando hace solo uno de estos cambios en
    pruebas separadas (Guardar con el mismo valor que ya tenía la clave, vaciar un árbol ya vacío, Añadir de una clave
    nueva, Quitar de una clave que está, o Quitar intervalo que quita al menos una entrada) y se confirma, entonces los
    archivos del almacén cambian, como en cualquier escritura con cambios (X8, T10).
19. **AC-X19** — Dado un árbol de tamaño fijo con sus páginas en el archivo de datos y sitio para recordar todos los
    nodos que lee la prueba, cuando una transacción de lectura lee la misma clave varias veces y otra la lee después,
    entonces todas devuelven lo mismo, y la organización de cada nodo del camino se comprueba solo en la primera
    lectura de ese nodo y su integridad en todas; y dada una página de una hoja suya en el archivo de datos con la
    organización dañada y el código de integridad recalculado para que sea correcto, cuando se lee dos veces en la
    misma transacción de lectura y una vez en otra, entonces cada lectura falla con la clase de daño y el número de
    esa página, también cuando antes se había recordado otra versión correcta de esa página; esa misma página dañada
    en el área temporal (confirmada y sin trasladar) falla igual las dos veces; un byte alterado en la página de un
    nodo recordado sin recalcular su código hace fallar la lectura siguiente con daño; y tras cerrar y reabrir el
    almacén, la primera lectura de cada nodo comprueba su organización otra vez. La prueba usa nodos a los que la
    implementación aplica lo recordado (X14, C1–C4, C7, AC-C1, AC-C4, AC-C5, AC-C7, AC-C9).
20. **AC-X20** — Dados un árbol normal, uno multivalor y dos de tamaño fijo de longitudes `0` y `8`, cuando se pide su
    Tipo por su nombre en una transacción de lectura y en una de escritura, entonces devuelve el tipo de cada uno, con
    su longitud en los de tamaño fijo, igual que Listar; con un nombre que no tiene ningún árbol, también uno borrado en
    la misma escritura, devuelve que no existe sin fallar; tras renombrar un árbol y tras borrarlo y recrearlo con otro
    tipo en la misma escritura, devuelve el tipo vigente con su nombre nuevo; con un nombre vacío o de 1.025 bytes,
    falla sin efecto con tamaño no válido, como nombre; con una página de uno de esos árboles dañada y el catálogo
    intacto (en disco), no falla; en disco, con la página del catálogo dañada, falla con la clase de daño, y con un
    nombre de 1.025 bytes falla con tamaño no válido; en un almacén que no acepta escrituras (T12), funciona igual; no
    termina las referencias ni los recorridos anteriores, y una escritura en disco que solo pide tipos se confirma
    dejando los archivos idénticos byte a byte (X15, X8, T10).
21. **AC-X21** — Dado el informe de cierre, cuando se revisa, entonces contiene la medición de X13 con sus tiempos,
    por fase, y los tamaños del almacén, para cada tipo de árbol y cada longitud de valor, y los datos del equipo, en
    Windows y en Linux (X13).

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Módulo de árboles, B3 y B8: el tipo de un árbol de tamaño fijo incluye su longitud, y los datos de tipo de árbol
  distinto la llevan (X3).
- Módulo de árboles, B6: en los árboles de tamaño fijo, cada valor tiene exactamente la longitud del árbol, de 0 a
  2.048 bytes (X1).
- Módulo de árboles, B7 y B8: la longitud pedida al abrir un árbol de tamaño fijo se valida como un valor, con
  mínimo `0` y máximo `2048`, después del nombre (X2, X11).
- Árboles y operaciones por clave, A1: un árbol de tamaño fijo se abre en los mismos modos, con su longitud (X2, X4).
- Árboles y operaciones por clave, A3: Listar informa la longitud de los árboles de tamaño fijo (X3), y hay una
  operación nueva del catálogo, Tipo, que da el tipo de un árbol por su nombre para los tres tipos (X15).
- Árboles y operaciones por clave, *Fuera de alcance* (un Existe que pida un tipo): Tipo no es un Existe con tipo,
  devuelve el tipo del árbol; sigue fuera de alcance que Existe compruebe un tipo (X15).
- Árboles y operaciones por clave, A5: el número de entradas está también en las aperturas de los árboles de tamaño
  fijo (X4).
- Árboles y operaciones por clave, A9 y A10; Recorrido en orden, R6, R7 y R8: valen igual para las operaciones de los
  árboles de tamaño fijo (X8, X10).
- Árboles y operaciones por clave, A7: el tamaño no válido de la longitud pedida va después del del nombre (X11).
- Árboles y operaciones por clave, *Fuera de alcance*: los árboles de tamaño fijo los fija este PRD (X1–X15).
- Comprobación de organización recordada, C1 y *Fuera de alcance*: las páginas de los árboles de tamaño fijo son nodos
  que usan lo recordado, sobre los mismos bytes que los demás (X14).
- Recorrido en orden, *Fuera de alcance*: los recorridos y Quitar intervalo de los árboles de tamaño fijo los fija
  este PRD, sin prefijo (X7).
- Recorrido en orden, R3: en los árboles de tamaño fijo, el intervalo también está vacío cuando ningún entero cumple
  los dos límites (X7).
- Valores grandes, *Fuera de alcance*: los árboles de tamaño fijo no tienen valores fuera de su hoja (X1).
- Árboles multivalor, M4: las aperturas de los árboles de tamaño fijo son un tercer tipo con la misma regla (X4).
- Padre raíz, D7; Pager, P5; Diario, J3; escritura agrupada, E15; Espacio libre, F11; Transacciones, todo el
  documento; Árboles y operaciones por clave, A13 y su anotación de 5.3; Recorrido en orden, R10, *Salida* y AC-R14;
  Valores grandes, V9, *Salida* y AC-V12; Árboles multivalor, M14, *Salida* y AC-M18; Comprobación de organización
  recordada, *Salida*: la versión de formato vigente pasa a ser `9`, y un almacén de formato `8` es de formato
  anterior (X12).

## Referencias

- Padre: [trees.prd.md](trees.prd.md) v1.0; raíz: [krummi.prd.md](krummi.prd.md) v1.1
- Hermanos: [trees-keys.prd.md](trees-keys.prd.md) v1.0, [trees-scan.prd.md](trees-scan.prd.md) v1.0,
  [trees-large-values.prd.md](trees-large-values.prd.md) v1.0, [trees-multivalue.prd.md](trees-multivalue.prd.md)
  v1.0
- Relacionados: [trees-node-check-cache.prd.md](trees-node-check-cache.prd.md), [free-space.prd.md](free-space.prd.md),
  [transactions.prd.md](transactions.prd.md), [pager.prd.md](pager.prd.md)
- Voron, árboles de tamaño fijo: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/Fixed>

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 67d5698f | 2026-10-31 08:14 UTC+00:00 | 47 h 39 min | 43 h 45 min | 3 h 54 min | 2 h 52 min | 1 h 2 min | 21 |

<!-- prd-time: [{"session": "67d5698f-6683-4ac4-83d2-7287ae689237", "start": "2026-10-31T08:14:08.343000+00:00", "wall": 171523, "discarded": 157472, "agent": 10311, "human": 3738, "interventions": 21, "idle_minutes": 10}] -->
