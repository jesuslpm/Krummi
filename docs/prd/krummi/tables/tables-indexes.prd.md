# Índices secundarios de las tablas — PRD

**Versión**: 1.0
**Fecha**: 2026-11-16
**Documento padre**: [Tablas de Krummi — PRD](tables.prd.md) (v1.0)

## Objetivo y contexto

Construir el PRD hijo **7.2** del [reparto del módulo de tablas](tables.prd.md#reparto-en-prd-hijos): crear y borrar
índices únicos y no únicos sobre una tabla, también con filas; mantenerlos en cada escritura; recorrer una tabla por un
índice, devolviendo sus filas o solo las entradas del índice; columnas incluidas, columnas descendentes y el orden de
los nulos (N5, N7 del padre); y las cargas del banco que comparan los índices con los de Voron (N11).

**Por qué ahora**: 7.1 y sus dos hijos (lecturas sin comprobar el contenido y proyecciones) están cerrados, y N2 del
padre construye los PRD hijos en orden: las estadísticas (7.3), las columnas JSON (7.5), los índices vectoriales (7.6) y
las consultas (módulo 17) se apoyan en los índices secundarios.

El diseño técnico ya está decidido en los documentos de diseño de las tablas, que son el estado de partida técnico de
este PRD: el [documento 3](../../../design/tables/03-keys-and-indexes.md) (codificación de las claves, estructura del
índice, sonda de unicidad), el [documento 2](../../../design/tables/02-row-storage.md) (crear un índice sobre filas que
ya existen, H15) y el [documento 11](../../../design/tables/11-query-contract-reconciliation.md) (identidad de cada
índice y estado de los índices, P12 y P17).

Este PRD hereda todo lo de sus ascendientes. Precisa o amplía reglas de su padre, de la raíz, de
[tables-schema.prd.md](tables-schema.prd.md) (7.1), de sus dos hijos, del pager y del banco, que llevan su anotación
(*Anotaciones a otros PRD*).

Decisiones del usuario del 2026-11-18, sobre las propuestas del borrador: ordenar las entradas de un índice nuevo en
«Memoria y archivos temporales», en la «Carpeta del almacén»; los límites «32 columnas, 64 índices»; «Rechazar los dos»
índices redundantes (el que repite la clave primaria o un prefijo suyo, y el que repite la clave de otro); devolver la
clave de una entrada «Decodificadas en una copia»; «Los dos modos» de crear un índice; para las filas mal formadas al
borrar y al crear un índice, «No comprobar, como las lecturas»; el formato «Incompatible, formato 12»; en un almacén
temporal, «Solo memoria en modo temporal», para cumplir G9; y un pánico capturado al borrar un intervalo deja la
escritura sin poder confirmarse: «Ya no puede confirmarse». Y, tras la revisión de Codex, una columna de la clave
primaria entre las incluidas de un índice: «No falla, pero no hace nada, la clave primaria siempre está incluida».

En este documento, la **clave del índice** es la lista ordenada de las columnas por las que se ordena y se busca, cada
una con su dirección y su orden de los nulos; sus **columnas incluidas** son las que guarda además para devolverlas sin
leer la fila; y una **entrada** es lo que el índice guarda por cada fila de la tabla: los valores de su clave, los de sus
columnas incluidas y la clave primaria de la fila.

### Decisiones de producto

| ID | Decisión |
|---|---|
| IX1 | **Definición de un índice**: un nombre, si es único, su clave (de 1 a 32 columnas distintas, cada una ascendente o descendente y con sus nulos primero o último; por defecto, ascendente y con los nulos primero, N5) y sus columnas incluidas (de 0 a todas las demás columnas vivas, distintas entre sí y de las de la clave, en el orden que se den). El orden de los nulos forma parte de la definición también en una columna que no admite nulos. El nombre de un índice sigue las reglas de los nombres de columna (E4): texto UTF-8 de 1 a 1.024 bytes, comparado por sus bytes; es único dentro de su tabla y no ocupa ningún nombre del catálogo (N1). Una tabla tiene como mucho 64 índices. Una columna de la clave puede ser de cualquier tipo de N4 salvo vector (y JSON desde 7.5, anotación de 7.5 del padre), con el orden de N5 de su tipo, también el de SQL Server en un `uuid` que lo declara; una columna incluida, de cualquier tipo, vectores incluidos (precisa N5 del padre). Las columnas de la clave primaria pueden estar en la clave del índice. Entre sus incluidas se admiten, pero no hacen nada, porque toda entrada lleva ya la clave primaria entera: la definición se guarda sin ellas, Describir no las muestra entre las incluidas (IX6), Crear si no existe compara las definiciones sin ellas (IX2) y una entrada las devuelve como columnas de la clave primaria (IX8); las reglas de IX3 se comprueban antes, sobre la definición tal como se da. Los valores de una entrada son los de su fila como los leería E11, así que una fila guardada antes de añadir una columna tiene en el índice su valor por defecto o nulo. La forma guardada de la definición reserva la condición de un índice parcial (N7), que la API de este PRD no ofrece; lo comprueba la revisión del diseño, no una prueba. Los índices funcionan igual en un almacén temporal, salvo dónde se ordenan sus entradas al crearlos (IX2). |
| IX2 | **Crear un índice**: solo en una escritura (en una lectura no existe la operación), sobre una tabla abierta, con su definición, en uno de dos modos. **Crear nuevo** falla con árbol ya existente, con la tabla y el índice, si la tabla ya tiene un índice con ese nombre. **Crear si no existe** no hace nada si ya hay un índice con ese nombre y la misma definición (IX1: unicidad, columnas de la clave con su dirección y sus nulos, y columnas incluidas en el mismo orden, sin las de la clave primaria) y la definición dada cumple IX3; si es la misma pero la dada incumple IX3 (por ejemplo, con una columna de la clave primaria repetida entre sus incluidas), falla con esquema no válido, como en IX3; y falla con **esquema distinto** si su definición es otra, con la tabla, el índice y la primera diferencia, con lo esperado y lo encontrado como en E6, en este orden: la unicidad; las columnas de la clave, posición a posición (la columna, su dirección y sus nulos), y su número; y las incluidas, posición a posición, y su número; dentro de cada parte, las posiciones que las dos tienen antes que su número. Se llama así, y no Abrir o crear como en A1, porque un índice no se abre. Ninguno de los dos modos devuelve nada. Crear un índice sobre una tabla con filas lee todas sus filas, ordena sus entradas y escribe el índice dentro de la misma escritura, como cualquier operación de una escritura (G4). Las entradas se ordenan en la memoria del proceso y, cuando no caben en lo que la implementación reserva para ello, también en archivos temporales dentro de la subcarpeta **`krummi.sort`** de la carpeta del almacén, que solo se crea cuando hacen falta. `krummi.sort` sigue las reglas de `krummi.temp` (J2, J8, J14): es un nombre reservado (P3), así que lo que haya dentro es del almacén y P7 no se aplica; una carpeta con `krummi.sort` y sin `krummi.data` no es un almacén; la creación del índice lo borra al terminar, también cuando falla o un pánico la interrumpe; si no lo puede borrar, queda sin cambiar el resultado de la creación; cerrar el almacén, o dejar de usarlo sin cerrarlo, lo borra; y el que deja una caída, o un pánico con una compilación que aborta, lo borra entero la apertura siguiente en su paso 8, después de comprobar el diario, con todo lo que contenga y también si es un archivo, mientras que una apertura que falla no lo borra y una que no lo puede borrar falla con la clase de acceso. Sus archivos no cuentan para el tamaño máximo (P10) ni en la información del almacén (P12), y nunca se leen para recuperar. Un fallo al crearlos, escribirlos o leerlos hace fallar la creación con su clase (acceso, disco lleno de G7, memoria agotada de P13, o entrada y salida de G12), con T11, sin cambiar el almacén (IX11); un fallo de entrada y salida al escribirlos sigue además T12, como en J7. En un almacén temporal no se crea ningún archivo (G9): las entradas se ordenan solo en memoria y, si el sistema no la da, la creación falla con memoria agotada (P13), con T11. Escribir las páginas del índice puede fallar como cualquier escritura (G7, P10, P13, G12), con T11. Un índice creado se ve desde ese momento en todas las aperturas de la tabla de esa escritura (B9) y, al confirmar, en las transacciones posteriores. La definición de una tabla al crearla (E1) no lleva índices: se crean después, en la misma escritura o en otra. Se puede crear un índice con el nombre de uno borrado antes, también en la misma escritura y con otra definición, y es un índice nuevo, con otra identidad (IX6). |
| IX3 | **Definiciones no válidas**: crear un índice falla sin efecto con **esquema no válido** cuando la definición incumple una regla, con la tabla, el índice, la columna si la hay y la regla, que es una de estas, en este orden: sin columnas de clave; una columna que no existe; una columna vector (o JSON, desde 7.5) en la clave; una columna repetida en la clave, en las incluidas o en las dos; más de 32 columnas de clave; una clave que es la clave primaria entera, o, en un índice no único, un prefijo de ella, con sus columnas en el mismo orden y todas ascendentes, tenga los nulos y las incluidas que tenga (un índice único sobre un prefijo estricto se admite: es una restricción); una clave igual a la de otro índice de la tabla (las mismas columnas en el mismo orden, con la misma dirección y los mismos nulos), sea único o no y tenga las incluidas que tenga; más de 64 índices; y el esquema demasiado grande de la anotación del 2026-11-11 a E5, que desde este PRD cuenta también las definiciones de los índices, nombrando el índice que lo pasa y ninguna columna. Si se incumplen varias, se informa la primera en el orden de esta lista y, dentro de ella, la primera columna en el orden en que la definición las da (la clave y después las incluidas), no en el del esquema como en E5; en la columna repetida, la primera aparición que repite una anterior en ese orden, como el paso 1 de E17. El esquema demasiado grande cuenta, de cada índice, su nombre y su definición tal como se guardan. Un nombre de índice que no mide de 1 a 1.024 bytes falla con tamaño no válido (B8), con los nombres (E17). Como las definiciones cuentan para el esquema demasiado grande, añadir una columna o renombrarla a un nombre más largo puede fallar con esa regla en una tabla con índices (E5). |
| IX4 | **Borrar un índice**: solo en una escritura, sobre una tabla abierta, por su nombre; quita el índice y libera sus páginas, y no devuelve nada. Si la tabla no tiene un índice con ese nombre, no hace nada, como Borrar en A3. Borrar la tabla, también al sobrescribirla al renombrar otra, borra sus índices (E2); vaciarla, con Vaciar o con Crear o vaciar, quita sus entradas y conserva sus definiciones, sus identidades y el estado de los índices (E3); renombrarla los conserva. No se puede renombrar un índice ni cambiar su definición: se borra y se vuelve a crear. |
| IX5 | **Índices y cambios de esquema**: un índice nombra sus columnas por su identidad (E2), así que renombrar una columna no lo afecta y Describir muestra el nombre nuevo. Quitar una columna de la clave o de las incluidas de un índice falla sin efecto con esquema no válido y la regla que E5 ya tiene, «quitar una columna de la clave primaria o, desde 7.2, de un índice», con la tabla y la columna, y además, si la columna no es de la clave primaria, el índice con esa columna que va primero en el orden de B5. Permitir nulos en una columna de un índice se admite (N3); sus nulos siguen el orden de los nulos de esa columna del índice y, en uno único, no chocan entre sí (N7). Crear o borrar un índice no cambia la versión ni el estado del esquema (E2), así que las proyecciones siguen valiendo (PY4). |
| IX6 | **Describir** (amplía E2): devuelve además la lista de los índices de la tabla tal como los ve su transacción, vacía si no tiene, en el orden de sus nombres según B5, cada uno con su identidad, su nombre, si es único, las columnas de su clave (cada una con su identidad, su nombre, su dirección y su orden de los nulos, como se definieron) y sus columnas incluidas (cada una con su identidad y su nombre), en su orden; y el **estado de los índices** de la tabla (documento de diseño 11, P12). La identidad de un índice y el estado de los índices son valores aleatorios de 64 bits que se sortean como los de E2 y se vuelven a sortear si coinciden con uno que el almacén conoce. El estado de los índices se sortea al crear la tabla y otra vez cada vez que se crea o se borra un índice suyo, y con nada más: ni con un intento que falla o no hace nada, ni al renombrar la tabla o una columna, ni al vaciarla; una escritura abortada no deja ninguno de sus sorteos. Las columnas se nombran con el nombre que tienen en el esquema que ve la transacción. Listar no cambia: los índices no son objetos del catálogo. |
| IX7 | **Mantenimiento**: cada escritura de una fila (insertar, actualizar, insertar o actualizar, borrar, borrar un intervalo, la carga masiva) deja todos los índices de la tabla de acuerdo con sus filas en la misma transacción (N7, G1), también en un índice creado antes en la misma escritura. Todas las comprobaciones (valores, tamaños, clave primaria y cada índice único) se hacen antes de cambiar nada, también al actualizar. Una actualización idéntica (E10) no cambia ningún índice ni es un cambio para T10 (E16). En un índice único, dos filas chocan cuando sus columnas de la clave del índice tienen los mismos valores, comparados después de la normalización de N4 (N5), y ninguno es nulo; con algún nulo, nunca chocan (N7). Guardar una fila que choca con otra en un índice único falla sin efecto con **clave duplicada** (IX10). Una carga masiva sobre una tabla con índices sigue E13, con todas sus reglas: una fila que choca, también con otra fila anterior de la misma carga, aborta la carga entera. |
| IX8 | **Recorrer un índice**: en una transacción de lectura o de escritura, se recorre un intervalo (IX9) de un índice hacia delante, en el orden de su clave (N5, con la dirección y los nulos de cada columna; las entradas con la misma clave, también las que tienen nulos, en el orden ascendente de su clave primaria), o hacia atrás, en el orden inverso exacto. Es la única operación que lee por un índice: buscar las filas de un valor es recorrer el intervalo con ese valor en todas las columnas de la clave, que en un índice único sin nulos da como mucho una fila. El recorrido devuelve, a elección de la aplicación, **filas** o **entradas**. Las filas son las de la tabla, con la forma de E11, leídas como en L1 y L2 y que se ligan a una proyección como las de un recorrido de la clave primaria (PY3). Las entradas se leen sin leer las filas: cada una da los valores de las columnas de la clave del índice, de sus incluidas y de las columnas de la clave primaria que no están en la clave del índice, accesibles por su nombre o por su posición en ese orden (la clave del índice en su orden, las incluidas en su orden y el resto de la clave primaria en su orden), cada columna una sola vez. Los valores tienen la normalización de N4 y E7 (un `uuid` en el orden de bytes de RFC 9562 también en una columna con el orden de SQL Server). Los de las columnas incluidas se devuelven como los de una fila: sin copia el texto, los bytes, los vectores y el JSON (D11); los de las columnas de la clave del índice y de la clave primaria, decodificados en una copia propia de la aplicación, también el texto y los bytes (excepción a D11 y a la *Salida* del padre). Una entrada no se liga a una proyección, y no hay copias de entradas que duren más que la referencia. Recorrer un índice no es un cambio para T10 ni termina referencias (E16); las filas y las entradas que presta dejan de valer en la siguiente operación que puede modificar la transacción (B9, E16, IX13). |
| IX9 | **Intervalos de un índice** (como E12): un intervalo da valores para las primeras k columnas de la clave del índice (el prefijo, de 0 a todas) y, si k es menor que el número de columnas de la clave, opcionalmente un límite inferior y uno superior sobre la columna siguiente, cada uno incluido, excluido o ausente. Un valor del prefijo puede ser nulo si la columna admite nulos, y entonces pide las entradas cuya columna es nula. Los límites son de valores, en el orden de N5 del tipo de la columna, sea cual sea su dirección en el índice: el inferior es el menor valor y el superior el mayor, y un recorrido hacia delante de una columna descendente va del superior al inferior. Con algún límite, las entradas cuya columna es nula no están en el intervalo, como en una comparación de SQL; sin ningún límite, sí. Los valores del prefijo y de los límites se normalizan (N4, E7) y se redondean (E8) como los de una fila, así que `-0` encuentra `+0` y cualquier NaN, el NaN guardado. Un intervalo vacío o invertido con valores válidos no devuelve nada y no es un error. Fallan con **valor no válido**: un valor de un tipo que no es el de su columna (regla tipo distinto del de la columna), fuera de su dominio (regla fuera del dominio), un nulo en el prefijo de una columna que no admite nulos (regla nulo no admitido), y un nulo como límite, un prefijo con más columnas que la clave del índice o un límite cuando el prefijo ya da todas las columnas (regla intervalo no válido); NaN es un valor válido (N5). Con varios fallos, un prefijo con más columnas que la clave o un límite con el prefijo completo se informan antes que los valores; después, el primer valor que falla (el prefijo en el orden de la clave, el límite inferior y el superior), con la primera regla que incumple en el orden de E17. Un prefijo con cualquiera de sus límites que, codificado como en una entrada, pasa de 2.048 bytes falla con **tamaño no válido**, con los datos de B8 (qué incumple su tamaño: un prefijo; el tamaño recibido; `1` y `2048`), la tabla y el índice, aunque el intervalo esté vacío. Recorrer un índice que no existe falla con **esquema distinto**, con la tabla y el índice. No hay límites sobre más de una columna a la vez ni sobre las columnas de la clave primaria de las entradas. |
| IX10 | **Errores: datos y precedencia** (amplía E17): la clave primaria repetida falla como en 7.1, con la tabla y la clave primaria y sin índice. Un índice único repetido falla con **clave duplicada** con la tabla, la clave primaria de la fila que se guarda, el índice y los valores de las columnas de su clave que se repiten; al crear un índice único sobre filas que repiten valores, la clave primaria es la de la segunda de las dos primeras filas que los repiten en el orden del índice, y en una carga masiva, la de la fila de la carga que choca, la primera en el orden de la carga. Al guardar una fila, la clave primaria repetida se informa antes que un índice, y entre índices, el primero en el orden de sus nombres según B5. Una entrada más grande que su máximo falla con **tamaño no válido**, con los datos de B8 (qué incumple su tamaño: una entrada de índice; el tamaño recibido; `1` y `2048`), la tabla y el índice; el tamaño de una entrada es el de los valores de la clave del índice seguidos de la clave primaria de la fila, en la forma en que Krummi guarda las claves (E18, documento de diseño 3), en la que una columna descendente o con los nulos último puede ocupar algún byte más; las columnas incluidas no cuentan. Al guardar una fila, el tamaño no válido de sus entradas va después del de la clave primaria y el de la fila, entre índices en el orden de sus nombres, y en el lugar de E17 para el tamaño que necesita el esquema; en una actualización o en insertar o actualizar una fila que existe, después de leerla, como el de la fila (la excepción de E17). Toda operación sobre índices sigue la precedencia de E17: primero el tamaño no válido de los nombres (el de la tabla, si lo hay, y el del índice); después árbol no encontrado de una apertura de una tabla borrada; después árbol ya existente (Crear nuevo), o esquema distinto (Crear si no existe con otra definición, o recorrer un índice que no existe); después esquema no válido (IX3); después valor no válido y el tamaño no válido de un prefijo (IX9); después los fallos al leer páginas y, al crear un índice, los de los archivos temporales (IX2), el primero que ocurra; y al crear un índice, después de leer todas las filas, el tamaño no válido de las entradas, el de la primera fila en el orden de la clave primaria, y por último clave duplicada. |
| IX11 | **Crear un índice único sobre filas que repiten valores** falla sin efecto con clave duplicada (IX10) antes de cambiar nada en el almacén (sus páginas; los archivos temporales de IX2 no son del almacén): el catálogo, las filas, los demás índices, las páginas en uso y libres y la información del almacén (P12) quedan como sin el intento, y la transacción puede confirmar sus otros cambios (H15 del diseño). Igual cuando falla por el tamaño de una entrada. |
| IX12 | **Contenido y daño** (amplía E19, L1–L3 y la excepción de G6 de la raíz para las filas): recorrer un índice no comprueba el contenido de las entradas ni de las filas: una entrada mal formada en una página cuya integridad pasó G6 se devuelve sin error, y leer uno de sus valores puede dar un valor que no es el escrito o terminar con un pánico normal del hilo, como una fila (L2). Al pedir filas, una entrada cuya fila no está en la tabla falla con daño de la página del índice que tiene la entrada. Al mantener un índice, una entrada que la escritura debe quitar y no está falla con daño de la página del índice donde debería estar, y la comprobación de unicidad decide con las entradas, sin leer la fila de la otra. Actualizar e insertar o actualizar siguen comprobando la fila que reemplazan (L3). Borrar una fila, borrar un intervalo y crear un índice, en una tabla con índices, leen los valores de las filas que ya existen para derivar sus entradas sin comprobar su contenido, como las lecturas (L1, decisión del usuario), aunque siguen fallando con daño en los casos que L1 mantiene (una versión de esquema desconocida o que falta en el historial, una cabecera que Krummi no escribe, una fila que no llega al final de su parte fija). Una fila mal formada puede dar entradas que no son las de sus valores escritos, hacer que se quite sin error la entrada de otra fila, hacer que falte la entrada que hay que quitar (y entonces falla con daño, arriba) o terminar con un pánico normal del hilo (L2); qué da cada contenido mal formado queda a la implementación, como en L2. Un pánico al borrar una fila o al crear un índice llega antes de cambiar nada, y la transacción sigue como dice L2; uno al borrar un intervalo puede llegar con parte del intervalo borrado, y entonces, si la aplicación lo captura, la escritura ya no puede confirmarse, como con un daño (T11; excepción a L2, decisión del usuario). Describir y borrar un índice que no existe nunca fallan con daño en las páginas de los índices; recorrer falla con daño en una página que necesita; crear un índice puede fallar con daño en cualquier página de la tabla; borrar un índice, en cualquier página de ese índice. En una escritura, todo fallo por daño sigue T11: la escritura ya no puede confirmarse, aunque una parte de un intervalo se haya borrado antes de encontrar el daño. |
| IX13 | **Cambios y referencias** (amplía E16): son cambios para T10 crear un índice y borrar uno que existe, también cuando se crea y se borra en la misma escritura. No lo son crear si no existe sobre un índice igual, borrar uno que no existe, recorrer un índice y toda operación que falla por un error de validación pública. Crear y borrar un índice terminan las referencias anteriores, aunque al final no cambien nada o fallen. |
| IX14 | **Formato**: el catálogo guarda las definiciones de los índices, sus identidades y el estado de los índices, que un motor del formato 11 no sabe leer, así que el cambio es incompatible (B12) y la versión del formato pasa a 12: un almacén del formato 11, aunque no tenga índices, es de formato anterior y este motor no lo abre (F11). La información del almacén (P12) no cambia de forma: las páginas de los índices cuentan en sus totales. |
| IX15 | **Cargas del banco** (N11, K7): las tres cargas de *Cargas del banco* (abajo), frente a las tablas de Voron con sus índices, con el umbral de K7; los demás motores dicen `no aplica`. El informe cuenta 32 umbrales. |

#### Cargas del banco

Usan la tabla de las *Cargas del banco* de 7.1, con dos índices no únicos: `ix_n` sobre `n` y `ix_ts` sobre `ts`,
ascendentes y sin columnas incluidas. Valen las reglas de esas cargas (los datos de la semilla de K4, el esquema
equivalente en Voron, sin heredar cambios de otra carga) y PY7: Krummi lee las columnas de las filas con una proyección.
En Voron, los dos índices se definen en su `TableSchema` y guardan sus campos de forma que el orden de sus bytes sea el
de Krummi, para que los dos motores devuelvan las mismas filas en el mismo orden.

| ID | Nombre | Empieza en | Qué hace | Operaciones | Reducida |
|---|---|---|---|---|---|
| W30 | `tabla-insercion-con-indices` | Almacén vacío con la tabla y sus dos índices | Inserta 1.000.000 de filas con claves `ulid` en orden, en escrituras de 1.000, como W23. | 1.000.000 | 10.000 filas |
| W31 | `tabla-busqueda-por-indice` | La tabla de W30 | En una transacción de lectura, busca en `ix_n` el valor de `n` de cada fila, en una permutación al azar de las filas (K4), con el intervalo de ese valor (IX8), y lee todas las columnas de la fila que devuelve; cada búsqueda comprueba que devuelve exactamente una fila. | 1.000.000 | 10.000 búsquedas |
| W32 | `tabla-intervalos-de-indice` | La tabla de W30 | En una transacción de lectura, 100.000 intervalos hacia delante de `ix_ts`, cada uno desde un valor de `ts` existente elegido al azar, incluido, que leen hasta 100 filas con todas sus columnas (menos si llegan al final), como W27; las filas con el mismo `ts` van en el orden de su clave primaria (IX8). | Las filas leídas | 1.000 intervalos |

W31 y W32 empiezan en el almacén que deja W30, cada una en su copia (K4). El tamaño del almacén (K9) se mide también tras
W30. W31 y W32 calculan en los dos motores la suma de comprobación de W26 y W27 y la comparan con la esperada.

*Anotaciones del 2026-11-22 desde los valores temporales en microsegundos ([tables-temporal-micros.prd.md](tables-temporal-micros.prd.md)):*

- *IX9: un valor temporal de un límite de un intervalo ya está en microsegundos y no se redondea (TU3).*

*Anotaciones del 2026-11-23 desde la intercalación de las columnas de texto ([tables-collation.prd.md](tables-collation.prd.md), 7.7):*

- *IX7 e IX11: en un índice único, dos filas chocan cuando sus columnas son equivalentes según su intercalación
  (IC6).*
- *IX8: las entradas devuelven en sus columnas `ci` y `ci_ai` el texto guardado en la fila, no la forma plegada, y se
  ordenan por las formas plegadas (IC3, IC5).*
- *IX9: los valores del prefijo y los límites se pliegan antes de comparar (IC3).*
- *IX10: la entrada, el prefijo y los límites se miden con la forma plegada; clave duplicada lleva los textos de la fila
  tal como quedarían guardados o, al crear un índice, tal como están guardados (IC6, IC7).*
- *Fuera de alcance: «Intercalaciones y comparación de texto sin distinguir mayúsculas» deja de estar fuera, salvo lo
  que excluye tables-collation.prd.md.*

*Anotaciones del 2026-11-25 desde los índices vectoriales aproximados ([tables-vector-indexes.prd.md](tables-vector-indexes.prd.md), 7.6):*

- *IX1 e IX3: el máximo de 64 índices por tabla y el tamaño del esquema cuentan también los índices vectoriales, que
  comparten el espacio de nombres de los índices de la tabla (VI1).*
- *IX5: quitar una columna con un índice vectorial falla como con un índice secundario (VI10).*
- *IX6: Describir devuelve además la lista de los índices vectoriales, aparte (VI1).*

### Estado de partida

Verificado el 2026-11-16 en el código y en los documentos de diseño:

- No hay índices secundarios: la entrada de una tabla en el catálogo guarda su identidad, la raíz y el número de filas
  de su árbol de filas, la raíz de su historial de esquemas, la versión y el estado del esquema, sus columnas y su clave
  primaria, sin campos reservados para índices; Describir no devuelve nada de índices; no hay identidades de índice ni
  estado de los índices.
- La regla de E5 «quitar una columna de un índice» solo existe en la documentación del error: el código comprueba solo
  las columnas de la clave primaria.
- Clave duplicada lleva la tabla y la clave primaria, sin índice. Tamaño no válido distingue nombre, clave, valor,
  prefijo, nombre de columna, clave primaria y fila, sin entrada de índice.
- Insertar, actualizar e insertar o actualizar comprueban todo antes de cambiar nada; actualizar sabe qué columnas da y
  decodifica la fila que reemplaza; borrar una fila y borrar un intervalo no la decodifican.
- La carga masiva inserta fila a fila en una escritura propia, rápida cuando las claves llegan en orden; no hay en el
  motor ninguna ordenación externa ni archivos temporales de ordenación.
- Los intervalos de la clave primaria tienen prefijo y límites (E12), hacia delante y hacia atrás; el recorrido presta
  sus filas, que se ligan a una proyección (PY3).
- La versión del formato es la 11. La información del almacén cuenta páginas, no árboles.
- El banco tiene las cargas W23–W29 de tablas y cuenta 29 umbrales; el arnés de Voron define solo la clave primaria de
  su tabla, sin índices.

## Actores

- **Desarrollador de la aplicación (Rust)**: crea y borra índices, guarda filas en tablas con índices y las recorre
  por un índice.
- **Quien ejecuta el banco**: compara las cargas W30–W32 con Voron.

## Flujo

1. La aplicación crea la tabla (E1) y, en la misma escritura o después, sus índices con Crear si no existe (IX2).
2. Inserta, actualiza y borra filas como en 7.1; Krummi mantiene los índices (IX7).
3. Recorre un intervalo de un índice y obtiene sus filas, que lee con una proyección, o solo sus entradas (IX8, IX9).
4. Para cargar muchas filas, carga la tabla sin índices y los crea después (N6 del padre), o carga con ellos.

## Validaciones y mensajes

Valen las de los ascendientes. Este PRD añade, solo en la biblioteca, en el orden de su precedencia (IX10):

| Situación | Clase de error |
|---|---|
| Un nombre de índice que no mide de 1 a 1.024 bytes | Tamaño no válido (B7, B8) |
| Una operación sobre índices a través de una apertura de una tabla que la escritura borró | Árbol no encontrado (A6, E17) |
| Crear nuevo un índice con un nombre que la tabla ya tiene | Árbol ya existente (A6), con la tabla y el índice |
| Crear si no existe un índice con un nombre que la tabla tiene con otra definición | Esquema distinto (E6), con la tabla, el índice y la diferencia |
| Recorrer un índice que la tabla no tiene | Esquema distinto (E6), con la tabla y el índice |
| Una definición de índice que incumple una regla de IX3; quitar una columna de un índice | Esquema no válido (E5) |
| Un intervalo de un índice mal formado (IX9) | Valor no válido (E17) |
| Un prefijo con su límite que no cabe en una entrada; una entrada de índice más grande que su máximo | Tamaño no válido (B7, B8), con la tabla y el índice |
| Guardar una fila que repite, sin nulos, los valores de un índice único de otra fila; crear un índice único sobre filas que los repiten | Clave duplicada (N10), con el índice (IX10) |
| Un fallo al crear, escribir o leer los archivos temporales de la ordenación, o la memoria que no alcanza en un almacén temporal (IX2) | Acceso, disco lleno (G7), memoria agotada (P13) o entrada y salida (G12), con T11 y, al escribir, T12 |
| Abrir un almacén con un `krummi.sort` que no se puede borrar | Acceso (J8) |

Borrar un índice que no existe y recorrer un intervalo vacío no son errores.

## Salida

El módulo no escribe nada en pantalla. Describir devuelve además la lista de índices, vacía si no hay, y el estado de
los índices (IX6), en una copia propia de la aplicación. Recorrer un índice devuelve filas con la forma de E11 o
entradas con la de IX8. Crear y borrar un índice no devuelven nada. Clave duplicada y tamaño no válido añaden el índice
a sus datos cuando el fallo es de un índice. Las lecturas, los recorridos y las escrituras que ya existen no cambian su
salida. El informe del banco añade las filas W30–W32 con sus umbrales, con `no aplica` en los motores que no son Voron,
y su tabla de tamaños añade W30.

## Reutilización

- La codificación de las claves, los intervalos de la clave primaria y su semántica (E12), y la comprobación de dos
  fases de las escrituras de 7.1 (E10, E17).
- Los árboles B+, sus recorridos y su regla de valores grandes fuera de la hoja (B6) para las entradas y sus columnas
  incluidas.
- Las identidades aleatorias del catálogo y su sorteo (E2, P12, P20).
- Las lecturas sin comprobar el contenido (L1, L2) y las proyecciones (PY3, PY7).
- El banco comparativo, su semilla, su tabla de 7.1 y su informe (módulo 6, *Cargas del banco* de 7.1).

## Fuera de alcance

- Índices parciales (reservados, IX1), sobre expresiones, sobre rutas de JSON, multientrada e invertidos (documento de
  diseño 14), e índices vectoriales aproximados (7.6).
- Declarar los índices en la definición de la tabla al crearla (IX2).
- Renombrar un índice o cambiar su definición: se borra y se vuelve a crear (padre).
- Crear un índice en varias transacciones o sin ocupar la escritura durante su construcción; crear varios índices en
  una sola lectura de la tabla.
- Configurar o leer la memoria o la carpeta de la ordenación de un índice nuevo, y ordenar en disco en un almacén
  temporal (IX2).
- Cancelar la creación de un índice, informar de su progreso o ponerle un tiempo límite propio.
- Reconstruir un índice: se borra y se vuelve a crear.
- Una copia de una entrada que dure más que su referencia (IX8).
- Cifrar los archivos temporales de la ordenación: la confidencialidad en reposo se delega en el sistema de archivos
  (anotación del 2026-11-09 a la raíz).
- Una operación de búsqueda por un índice distinta del recorrido de un intervalo (IX8).
- Borrar filas por un intervalo de un índice; contar las entradas de un índice o de un intervalo; el tamaño de un
  índice en Describir o en la información del almacén; estimaciones (7.3).
- Límites sobre más de una columna a la vez, o sobre las columnas de la clave primaria de las entradas, y elegir un
  índice de forma automática: módulo 17.
- Ligar las entradas de un índice a una proyección (IX8).
- Comprobar que un índice está de acuerdo con su tabla: la verificación del módulo 12 trata sus árboles como árboles
  (N1).
- Cargas del banco que actualicen o borren filas de una tabla con índices o que usen un índice único.
- La disposición de SQL Server para los índices únicos (la clave primaria solo en las hojas), que el documento de
  diseño 3 deja como optimización a medir.
- Intercalaciones y comparación de texto sin distinguir mayúsculas (padre); columnas de la clave primaria
  descendentes (7.1).

## Dejado a la implementación

- Cómo se guardan los índices y sus definiciones en el catálogo y en árboles, con el punto de partida de los documentos
  de diseño 2 y 3; la forma de una entrada que mide su tamaño (IX10) es la del documento 3.
- Los nombres de las operaciones, los tipos y las clases de error, en inglés y con las convenciones de Rust (D8).
- Cómo se ordenan las entradas al crear un índice, cuánta memoria usa antes de pasar a los archivos temporales, y
  los nombres y el contenido de los archivos dentro de `krummi.sort`.
- Cómo guarda el arnés de Voron los campos indexados para que su orden de bytes sea el de Krummi.
- Cómo se construyen los ganchos de prueba de los criterios: son mecanismos de prueba, no contratos públicos.

## Criterios de aceptación

Valen los de los ascendientes. Este PRD cumple AC-N7 del padre, los casos con índices de AC-N3 y de AC10 de 7.1, y
AC-N9 y AC-N10 con lo que estos criterios fijan.

1. **AC-IX1** — Dada una tabla con filas, columnas de cada tipo de N4 que se puede ordenar (`uuid` en los dos órdenes,
   `-0`, NaN con distintos bits y los extremos de cada dominio de E8), columnas añadidas después de guardar filas, con
   valor por defecto y sin él, y claves en el límite de tamaño, cuando se crean índices únicos y no únicos,
   ascendentes y descendentes, con los nulos primero y último, con cada combinación de dirección y orden de los nulos
   en columnas consecutivas, también detrás de columnas `bytes` con el byte cero y `string` con U+0000, y con columnas
   incluidas (también un vector), entonces cada índice tiene las entradas de las filas que ya había, en el orden de N5,
   con el valor por defecto o nulo en las columnas añadidas (AC-N7).
2. **AC-IX2** — Dada una tabla con varios índices, uno creado en la misma escritura, cuando una escritura inserta,
   actualiza (dando solo columnas que no son de ningún índice, solo de una incluida y de la clave de un índice, también
   sobre una fila de una versión anterior del esquema), inserta o actualiza, borra filas, borra un intervalo y carga en
   masa, y se confirma, y en otros casos se aborta o se simula una caída del proceso y se reabre, entonces cada índice
   tiene exactamente las entradas de las filas de la tabla; una actualización idéntica no es un cambio para T10 y no
   escribe ninguna página de los índices, según el gancho de pruebas; y dos filas con nulos en las columnas de un
   índice único no chocan (AC-N7).
3. **AC-IX3** — Dados los índices de AC-IX1, con filas que repiten la clave del índice, también con nulos, cuando se
   recorren por cada índice hacia delante y hacia atrás, pidiendo filas y pidiendo entradas, entonces vuelven en el
   orden de IX8 (las repetidas en el orden de su clave primaria, y al revés hacia atrás) las filas, leídas también con
   una proyección, o las entradas, con los valores de IX8 accesibles por nombre y por posición, también con una columna
   de la clave primaria en la clave del índice y otra, de texto, dada entre las incluidas (que la entrada devuelve una
   sola vez, en su posición de la clave primaria y en una copia), el texto, los bytes y los vectores incluidos sin
   copia, los de la clave en una copia, y los `uuid` de orden de SQL Server en el orden de bytes de RFC 9562 (AC-N7).
4. **AC-IX4** — Dados índices sobre columnas ascendentes y descendentes con los nulos primero y último, cuando se
   recorren intervalos con cada prefijo, con un nulo en el prefijo, con `-0` y NaN en el prefijo y en los límites, y con
   cada combinación de límites incluidos, excluidos y ausentes, hacia delante y hacia atrás, entonces vuelven
   exactamente las entradas de IX9; un intervalo invertido no devuelve nada; cada intervalo mal formado de IX9 falla
   con valor no válido y su regla, y un prefijo que no cabe, con tamaño no válido y sus datos, aunque el intervalo esté
   vacío; y recorrer un índice que no existe falla con esquema distinto (AC-N7).
5. **AC-IX5** — Dada la prueba de propiedades de AC-N1, con índices al azar de una a cuatro columnas de cualquier
   dirección y orden de los nulos, únicos y no únicos, cuando se guardan, actualizan y borran filas al azar, entonces
   el orden de los recorridos de cada índice, las entradas de cada intervalo y las claves duplicadas son los del modelo
   de referencia de N5 (AC-N7).
6. **AC-IX6** — Dada una tabla con varios índices únicos, cuando se inserta y se actualiza una fila que repite, sin
   nulos, los valores de cada uno por turno, de dos a la vez, y su clave primaria y un índice a la vez, entonces falla
   sin efecto con clave duplicada y los datos de IX10, el que dice IX10 cuando hay varios; cuando la entrada de una fila
   es demasiado grande, por un `string` en el límite que cabe en un índice ascendente y no en uno descendente, entonces
   falla con tamaño no válido y sus datos, también si la fila repite además un índice único; y en todos los casos la
   fila, los índices y el número de filas quedan como estaban y la transacción puede confirmar sus otros cambios
   (AC-N7).
7. **AC-IX7** — Dada una tabla con filas que repiten los valores de una columna, cuando se crea sobre ella un índice
   único y la escritura confirma otros cambios, entonces falla con clave duplicada y los datos de IX10, el índice no
   existe, la tabla y sus otros índices no cambian, y las páginas en uso y libres y la información del almacén (P12) son
   las que habría sin el intento; y cuando una fila tiene una entrada demasiado grande, entonces falla con tamaño no
   válido, igual sin efecto (AC-N7, IX11).
8. **AC-IX8** — Dada una tabla con índices, cuando una carga masiva tiene, antes de confirmar, una fila que choca en un
   índice único con una fila de la tabla o con otra anterior de la misma carga, una con una entrada demasiado grande,
   una que falla siendo la última, una fuente que falla, una cancelación o un pánico, entonces la tabla, sus índices, el
   número de filas y las secuencias quedan como antes y otra escritura obtiene después su turno; y cuando se cargan
   100.000 filas fuera de orden, entonces la tabla y sus índices tienen las mismas filas y entradas que si se hubieran
   insertado una a una (AC10 de 7.1, AC-N3).
9. **AC-IX9** — Dado cada caso de IX3, y un nombre de índice de 0, 1, 1.024 y 1.025 bytes, cuando se crea el índice,
   entonces los que están dentro funcionan y los demás fallan sin efecto con esquema no válido o tamaño no válido y sus
   datos; con dos reglas incumplidas, se informa la que dice IX3; una tabla con 64 índices no admite otro, y una cerca
   del máximo de su entrada del catálogo, según el gancho de pruebas que la mide, falla con esquema demasiado grande al crear un índice y al añadir una columna;
   Crear nuevo sobre un nombre que existe falla con árbol ya existente; Crear si no existe sobre uno igual no hace nada,
   también cuando uno de los dos da entre sus incluidas columnas de la clave primaria, que Describir no muestra (IX1),
   y sobre uno que difiere en cada parte de IX2, por separado y en dos a la vez, falla con esquema distinto y la
   primera diferencia; y gana el que dice IX10 en cada uno de estos pares: un nombre de índice largo desde una apertura
   de una tabla borrada (tamaño no válido); un nombre que existe con una definición no válida, en Crear nuevo (árbol ya
   existente) y en Crear si no existe (esquema distinto, si difiere de la del índice sin las columnas de la clave
   primaria de sus incluidas; si no difiere, como con las incluidas `[id, id]` sobre un índice sin incluidas y la
   clave primaria `id`, esquema no válido con la columna repetida); un índice que no existe y un intervalo mal formado (esquema
   distinto); un intervalo mal formado y un prefijo que no cabe (valor no válido); una entrada demasiado grande y un
   duplicado, al guardar y al crear (tamaño no válido); la clave primaria y un índice repetidos (la clave primaria); y
   dos índices repetidos (el primero por su nombre).
10. **AC-IX10** — Dada una tabla con índices, cuando se renombra una columna de un índice y se permiten sus nulos,
    entonces sus recorridos devuelven las mismas entradas en el mismo orden y Describir muestra el nombre nuevo con la
    misma identidad; cuando se quita una columna de su clave, otra de sus incluidas y una de la clave primaria que está
    en un índice, entonces falla sin efecto con esquema no válido y la regla de IX5; cuando se borra un índice, entonces
    desaparece, la tabla no cambia y otro índice puede crearse con su nombre y otra definición, también en la misma
    escritura; cuando se vacía con Vaciar y con Crear o vaciar, se renombra, se borra y se sobrescribe al renombrar otra,
    entonces sus índices quedan vacíos con sus definiciones, se conservan, desaparecen y desaparecen; y otra apertura de
    la tabla en la misma escritura ve un índice creado y deja de ver uno borrado.
11. **AC-IX11** — Dada una tabla, cuando se crean y se borran índices en una escritura que se confirma y en otra que se
    aborta, también uno creado y borrado en la misma escritura, entonces Describir devuelve los de IX6 tal como los ve
    cada transacción, en una lectura y en una escritura, y una lista vacía en una tabla sin índices; el estado de los
    índices cambia con cada creación y cada borrado y no con un intento que falla, uno que no hace nada, renombrar la
    tabla o una columna ni vaciarla; la identidad de un índice no cambia al renombrar la tabla o sus columnas, y uno
    vuelto a crear tiene otra; con un generador de prueba que repite un valor, la identidad y el estado se vuelven a
    sortear; el estado y la versión del esquema no cambian; y una proyección preparada antes sigue leyendo las filas.
    Crear y borrar índices en una lectura no es posible.
12. **AC-IX12** — Dadas las operaciones de IX13, cuando se ejecutan en una escritura que no hace nada más, entonces son
    cambios para T10 exactamente las que dice IX13, y crear y borrar un índice terminan las referencias anteriores.
13. **AC-IX13** — Dada una entrada mal formada en una página con la integridad correcta, cuando se recorre, entonces el
    paso no falla, las demás entradas vuelven con sus valores y leer el valor mal formado da un valor o termina con un
    pánico (L2); dada una entrada cuya fila no está en la tabla, cuando se piden filas, entonces falla con daño de la
    página del índice; dada una fila cuya entrada falta en un índice, cuando se borra, entonces falla con daño de la
    página del índice; dada una fila mal formada en una tabla con índices, cuando se actualiza, entonces falla con daño
    de su página y la escritura sigue T11; dada una fila cuyos bytes alterados cambian solo una columna que no está en
    ningún índice, cuando se borra, se borra un intervalo que la incluye y se crea un índice sobre otras columnas,
    entonces no fallan y los índices quedan de acuerdo con las demás filas; dada una fila cuyos bytes alterados cambian
    el valor de una columna de un índice a uno que no tiene ninguna entrada, cuando se borra, entonces falla con daño de
    la página del índice; dada una fila de una versión de esquema que falta en el historial, cuando se borra o se crea
    un índice, entonces falla con daño, como en AC-L3; dado un pánico capturado al borrar un intervalo, la escritura no
    se puede confirmar y no aplica nada; y dada una página de un índice dañada, Describir no falla y recorrer el índice
    falla con daño (IX12).
14. **AC-IX14** — Dada una tabla cuyas entradas no caben en la memoria de la ordenación, reducida con el gancho de
    pruebas, cuando se crea un índice no único y uno único, entonces el índice tiene las mismas entradas que con toda la
    memoria, y un duplicado se detecta igual (IX11); con toda la memoria, no aparece `krummi.sort`; con el gancho de
    pruebas detenido después de escribir el primer archivo temporal, existe `krummi.sort`, y al terminar con éxito, al
    fallar por un duplicado y al capturar un pánico que interrumpe la creación, no queda; cuando se simula una caída
    del proceso con el gancho detenido y se reabre el almacén, entonces `krummi.sort` no queda (tampoco con un archivo
    ajeno dentro, ni cuando `krummi.sort` es un archivo), el índice no existe y los archivos ajenos de la carpeta siguen
    (P7); una apertura que falla no lo borra, y una carpeta con `krummi.sort` y sin `krummi.data` no es un almacén;
    cerrar el almacén con un `krummi.sort` que no se pudo borrar lo borra; en un almacén temporal, no se crea ningún
    archivo ni carpeta (AC14 de la raíz sigue valiendo); con un disco lleno simulado al escribirlos, la creación falla
    con la clase de G7 y la escritura sigue T11; y con un fallo de entrada y salida simulado al escribirlos, falla con
    esa clase y el almacén deja de aceptar escrituras (T12).
15. **AC-IX15** — Dado un almacén del formato 11 escrito por 7.1, cuando este motor lo abre, entonces falla con la
    versión del formato (B12, F11); un almacén nuevo tiene la versión 12; y la información del almacén no cambia de
    forma.
16. **AC-IX16** — Dadas las cargas W30–W32, cuando se ejecuta el banco completo en Windows y en Linux, entonces el
    informe muestra 32 umbrales, `no aplica` en los motores que no son Voron, la tabla de tamaños W30, y las sumas de
    comprobación de W31 y W32 coinciden en los dos motores; la ejecución reducida usa las cantidades de su columna; y el
    informe de cierre incluye las mediciones de una sola carga, en Windows y en Linux, de W1–W29 antes y después del
    cambio y de W30–W32 después (AC-N9, AC-N10). Un umbral incumplido sigue K13.

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan estas anotaciones del 2026-11-18 (en sus
traducciones también):

- [krummi.prd.md](../krummi.prd.md): D11 y AC20 (los valores de las columnas de la clave de un índice y de la clave
  primaria leídos de una entrada se devuelven en una copia propia de la aplicación, también el texto y los bytes, IX8);
  G6 (la excepción de las filas vale también para las entradas de los índices, y para las filas que se borran o de las
  que se crea un índice en una tabla con índices, IX12).
- [journal.prd.md](../journal/journal.prd.md): J2, J8 y J14 (`krummi.sort` es otra subcarpeta reservada, con las reglas
  de `krummi.temp`: no es un almacén sin `krummi.data`, la apertura la borra en su paso 8 y una que falla no, y cerrar
  la borra, IX2).
- [pager.prd.md](../pager/pager.prd.md): P13 (crear un índice falla con memoria agotada cuando el sistema no da la
  memoria de la ordenación, IX2).
- [tables.prd.md](tables.prd.md): N5 (un vector puede ser columna incluida de un índice; no puede ser columna de su
  clave, IX1); N10 (esquema distinto cubre también crear si no existe un índice con otra definición y recorrer un índice
  que no existe, IX2, IX9); *Salida* (la excepción de IX8).
- [tables-schema.prd.md](tables-schema.prd.md): E2 (Describir devuelve los índices y el estado de los índices, IX6);
  E5 (las reglas de las definiciones de índice de IX3, el esquema demasiado grande cuenta las definiciones y la regla
  de quitar una columna de un índice, IX5); E6 (los casos nuevos de esquema distinto); E16 (IX13); E17 (los datos de
  clave duplicada en un índice, el tamaño no válido de una entrada de índice y la precedencia de IX10); E19 (IX12).
- [tables-row-reads.prd.md](tables-row-reads.prd.md): L3 (en una tabla con índices, borrar una fila, borrar un
  intervalo y crear un índice leen el contenido de las filas que ya existen, sin comprobarlo, como L1 y L2, IX12); L2
  (un pánico capturado al borrar un intervalo de una tabla con índices impide confirmar la escritura, IX12); las
  lecturas por un índice siguen L1 y L2.
- [pager.prd.md](../pager/pager.prd.md): P3 (`krummi.sort` es un nombre reservado de la carpeta del almacén, para los
  archivos temporales de crear un índice, que abrir el almacén borra, IX2).
- [tables-projection.prd.md](tables-projection.prd.md): PY3 (las filas de un recorrido de un índice se ligan a una
  proyección; las entradas no, IX8).
- [benchmark.prd.md](../benchmark/benchmark.prd.md): la tabla de cargas añade W30–W32; K4 (W31 y W32 empiezan en el
  almacén de W30); K7 y *Salida* (32 umbrales); K9 (W30).

## Referencias

- Padre: [tables.prd.md](tables.prd.md) (N1, N5, N7, N10, N11, AC-N3, AC-N7, AC-N9, AC-N10); su padre,
  [krummi.prd.md](../krummi.prd.md) (D8, D11, AC20, G1, G4, G6).
- Hermanos: [tables-schema.prd.md](tables-schema.prd.md) (E1–E8, E10–E13, E16–E20, *Cargas del banco*),
  [tables-row-reads.prd.md](tables-row-reads.prd.md) (L1–L3), [tables-projection.prd.md](tables-projection.prd.md)
  (PY3, PY4, PY7).
- Árboles: [trees.prd.md](../trees/trees.prd.md) (B4–B9, B12), [trees-keys.prd.md](../trees/trees-keys.prd.md) (A3, A6, A7,
  A9, A10).
- Transacciones: [transactions.prd.md](../transactions/transactions.prd.md) (T10, T11).
- Pager: [pager.prd.md](../pager/pager.prd.md) (F11, P12).
- Banco: [benchmark.prd.md](../benchmark/benchmark.prd.md) (K4, K7, K9, K11, K13).
- Diseño: `docs/design/tables/` (documentos 2, 3 y 11; README, H15).

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| bdd88ebe | 2026-11-16 10:50 UTC+00:00 | 55 h 16 min | 52 h 29 min | 2 h 46 min | 1 h 49 min | 57 min | 14 |

<!-- prd-time: [{"session": "bdd88ebe-15fe-4364-8a18-4410e2ba4ed3", "start": "2026-11-16T10:50:01.910000+00:00", "wall": 198936, "discarded": 188965, "agent": 6533, "human": 3438, "interventions": 14, "idle_minutes": 10}] -->
