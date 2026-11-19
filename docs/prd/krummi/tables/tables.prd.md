# Tablas de Krummi — PRD

**Versión**: 1.0
**Fecha**: 2026-11-09
**Documento padre**: [Krummi, motor de almacenamiento embebido inspirado en Voron — PRD](../krummi.prd.md) (v1.1)

## Objetivo y contexto

Construir las **tablas**, el módulo 7 del [reparto del padre](../krummi.prd.md#reparto-en-prd-hijos): colecciones con
nombre de filas con un esquema de columnas tipadas, una clave primaria e índices secundarios, sobre los árboles B+
del módulo 5. Es la primera forma estructurada de guardar datos en Krummi: hasta hoy, una aplicación guarda bytes en
árboles y decide ella cómo codificarlos y ordenarlos. Las consultas (módulo 17) y la interfaz C con el binding C#
(módulo 18) se apoyan en estas tablas, y el producto documental aparcado
([documents.prd.md](../../documents/documents.prd.md)) guardaría sus colecciones en ellas.

El módulo es demasiado grande para una sola ejecución, así que este documento es su paraguas, como el de los
[árboles](../trees/trees.prd.md) lo es del módulo 5: fija las reglas comunes a todas las tablas y reparte el módulo en PRD
hijos, cada uno con su propia ejecución y su propio cierre (ver [Reparto en PRD hijos](#reparto-en-prd-hijos)). Este
documento no tiene ejecución propia.

**Por qué así**: el diario tuvo que cambiar de formato después de construido, por rendimiento (O8 del
[plan de rendimiento](../../../performance/plan.md)). Para no repetirlo, el diseño técnico se escribió antes que este PRD:
los [documentos de diseño de las tablas](../../../design/tables/README.md) comparan Voron con otros sistemas de
almacenamiento estructurado en cada tema y registran, con sus palabras, cada decisión del usuario. Este PRD toma de
ellos lo observable; el cómo queda en esos documentos, que son el estado de partida técnico de cada PRD hijo.

**Por qué ahora**: los árboles (módulo 5) y el banco comparativo (módulo 6) están cerrados, y Krummi cumple los 22
umbrales del banco en Windows y en Linux (ejecuciones completas del 2026-11-09).

Este PRD hereda todo lo del padre y lo de los árboles (B1–B12) y sus hijos; aquí solo se añade lo común del módulo.
Lo que cambia o precisa reglas anteriores está declarado en sus decisiones y anotado en los documentos afectados (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

En este documento, una **tabla** es una colección con nombre de filas dentro de un almacén; una **fila** es un valor
para cada columna del esquema; el **esquema** es la lista de columnas de la tabla, con su nombre, su tipo, si admiten
nulos y sus opciones (N4, N6), y su clave primaria; la **clave primaria** es la lista de columnas cuyos valores
identifican una fila; un **índice secundario** es una lista de columnas, distinta de la clave primaria, por la que se
pueden encontrar filas.

### Decisiones de producto

| ID | Decisión |
|---|---|
| N1 | **Superficie pública y catálogo**: desde este módulo, las aplicaciones crean tablas con un esquema, guardan, leen, actualizan y borran filas, y las recorren por la clave primaria y por sus índices, en Rust; las abren por su nombre dentro de una transacción con las reglas de los árboles (B1, B9) y con los modos de apertura y las operaciones del catálogo de los árboles (A1, A3) que fije 7.1. Tablas y árboles comparten el catálogo y su espacio de nombres: un nombre es de un árbol o de una tabla, nunca de los dos. Listar el catálogo muestra los dos, cada uno con su tipo. Las operaciones de árbol nunca destruyen una tabla, ni las de tabla un árbol: abrir, borrar, renombrar o vaciar como árbol algo que es una tabla, o al revés, falla sin efecto con la clase de tipo de árbol distinto (B3; en este documento, *tipo distinto*), y renombrar un árbol con sobrescritura a un nombre que es de una tabla, o al revés, también; sin sobrescritura falla con la clase de ya existente, como entre árboles de tipos distintos (A3); Existe dice que el nombre existe sea de un árbol o de una tabla, porque comparten el espacio de nombres. Los árboles que forman una tabla (sus filas, sus índices y sus estadísticas) no aparecen en el catálogo, no se pueden abrir como árboles y no ocupan ningún nombre del usuario; las operaciones que copian o comprueban el almacén entero (el backup y la compactación de los módulos 10 y 11, y la verificación y el rescate de la herramienta de línea de comandos del módulo 12, D9 del padre) los tratan como árboles. Consultar tablas con filtros, orden y agregados es del módulo 17. |
| N2 | **Reparto y cierre**: el módulo se reparte en los PRD hijos de la [tabla](#reparto-en-prd-hijos), que se construyen en ese orden; este documento no tiene ejecución propia. Cada regla común la implementa y la prueba el primer PRD hijo que la necesita, y los siguientes la siguen cumpliendo. El módulo 7 está cerrado cuando lo están 7.1, 7.2 y 7.3; la compresión por diccionario (7.4) queda aplazada y fuera del objetivo 2 y de D4 del padre, como la compresión de páginas hoja del módulo 5. |
| N3 | **Esquema y sus cambios**: una tabla tiene de 1 al máximo de columnas que fija 7.1, cada una con un nombre (texto UTF-8 comparado por sus bytes, como los nombres de árbol, B4), un tipo (N4) y si admite nulos; toda columna los admite salvo que se declare que no. La **clave primaria es obligatoria**: una o varias columnas distintas, ninguna de tipo vector y ninguna que admita nulos; un esquema que no lo cumple falla sin efecto al crear la tabla. El esquema puede cambiar sin reescribir las filas, y cada cambio da una versión nueva del esquema de la tabla: añadir una columna que admite nulos (las filas anteriores la leen como nula); añadir una columna con un valor por defecto, una constante fijada al añadirla, que leen las filas anteriores; quitar una columna que no es de la clave primaria ni de un índice; renombrar una columna; y permitir nulos en una columna que no los admitía y no es de la clave primaria. Una columna añadida con el nombre de una que se quitó es nueva: los valores de la anterior nunca reaparecen. Cambiar el tipo de una columna, cambiar la clave primaria o prohibir los nulos en una columna existente no se admite en este módulo. |
| N4 | **Tipos de columna**: `bool`; enteros con signo y sin signo de 8, 16, 32 y 64 bits; `f32` y `f64`, donde `-0` se guarda como `+0` y todo NaN se guarda como un único NaN; `decimal(p, s)` de hasta 38 dígitos; `string`, texto UTF-8; `bytes`; `date`, `time`, `timestamp` (un instante en UTC con resolución de microsegundos, sin zona horaria guardada) y `duration`; `uuid`, con el orden de RFC 9562 o, si la columna lo declara, el de SQL Server (N5); `ulid`; y `vector(elemento, dimensión)`, con elementos `f32`, `f16`, `i8` o `bit`, una dimensión fija por columna de 1 a 4.096, múltiplo de 8 para `bit`, y opcionalmente, salvo con elementos `bit`, la métrica coseno: una columna que la declara guarda la norma junto a cada vector y no admite vectores cero. Ningún vector admite NaN ni infinitos. Las columnas autonuméricas son enteros de 64 bits (N6). Un valor que no cumple las reglas de su columna (fuera del dominio de su tipo, un vector de otra dimensión o con NaN o infinitos, un vector cero en una columna con métrica coseno, un `decimal` que no cabe en su precisión) o un nulo en una columna que no los admite falla con la clase nueva de valor no válido (N10). |
| N5 | **Orden**: las filas de una tabla y las entradas de un índice se ordenan por los valores de sus columnas, columna a columna, y la primera que difiere decide: `bool`, con falso antes que verdadero; los números por su valor numérico, con el NaN después de +∞; `string` y `bytes` byte a byte, sin intercalación; las fechas, horas, instantes y duraciones en orden cronológico; `ulid` por su tiempo y después por sus bytes; `uuid` por sus 16 bytes en el orden de RFC 9562, o, en una columna que lo declara, en el orden en que SQL Server compara un `uniqueidentifier`. En un índice, cada columna puede ordenarse ascendente o descendente, y sus nulos van primero (por defecto) o último en el orden del recorrido hacia delante de ese índice. Los vectores no se ordenan: no pueden ser clave primaria ni columnas de un índice. La clave primaria y los índices únicos comparan los valores después de la normalización de N4, así que `-0` y `+0` son el mismo valor, y dos NaN también. |
| N6 | **Filas y clave primaria**: una fila se lee, se actualiza y se borra por su clave primaria, y las filas se recorren por un intervalo de la clave primaria, hacia delante y hacia atrás, en el orden de N5. Una apertura informa al instante, sin recorrer la tabla, cuántas filas tiene la tabla según lo que ve su transacción, como el número de entradas de un árbol (A5). Se puede borrar un intervalo de la clave primaria de una vez, y los índices quedan como si se hubieran borrado sus filas una a una. La **carga masiva** inserta de una vez una secuencia de filas que da la aplicación, con las reglas de insertar, y es más rápida cuando las filas llegan en el orden de la clave primaria; para cargar una tabla con índices conviene cargarla primero y crear los índices después (N7). Hay tres operaciones para guardar una fila, y todas pasan la fila entera: **insertar**, que falla sin efecto con la clase nueva de clave duplicada (N10) si la clave ya existe; **actualizar**, que reemplaza la fila si existe y, si no, no hace nada e informa de que no la encontró; e **insertar o actualizar**, que reemplaza o inserta. Krummi genera valores de clave primaria crecientes, también dentro del mismo milisegundo: `ulid`; `uuid` versión 7, para columnas con el orden de RFC 9562; `uuid` secuenciales en el orden de SQL Server, para columnas con ese orden; y columnas autonuméricas de 64 bits, cuyo valor asigna Krummi al insertar, creciente y nunca reutilizado: en la misma tabla, el valor de una fila confirmada no vuelve a asignarse, aunque la fila se borre, la tabla se vacíe o el proceso caiga. |
| N7 | **Índices secundarios**: un índice es único o no único, con columnas distintas entre sí, ascendentes o descendentes (N5), y puede llevar columnas incluidas, cuyos valores devuelve sin la fila. Cada escritura de una fila deja todos sus índices de acuerdo con la tabla en la misma transacción (G1). En un índice único, dos filas con algún nulo en sus columnas no chocan entre sí, como en el estándar SQL. Una entrada de índice demasiado grande para una clave de árbol (B5) falla sin efecto con la clase de tamaño no válido (B7). Los índices parciales (con una condición) quedan reservados en la definición del índice y se implementan cuando exista el árbol de consulta (módulo 17); la reserva la comprueba la revisión del diseño de 7.2, no una prueba. |
| N8 | **Estadísticas**: cada tabla tiene estadísticas de sus columnas, también de las que no tienen índice, como las de SQL Server, para que el planificador del módulo 17 estime cuántas filas cumple cada condición. No tienen que ser exactas ni estar siempre al día. Desde 7.3 hay una operación pública de **estimación** (por ejemplo, cuántas filas tiene un intervalo de una columna, o qué fracción tiene un valor), la que usará el planificador del módulo 17: cuando no encuentra las estadísticas de la columna, o las encuentra **viejas**, responde con una estimación por defecto y pide que se construyan en segundo plano; otra operación pública, **analizar**, las construye a petición. La tabla lleva un contador de modificaciones que sobrevive a cerrar el almacén y nunca disminuye (vaciar la tabla o borrar un intervalo suman las filas que quitan), y unas estadísticas de una columna son viejas cuando las modificaciones contadas desde la instantánea de la que se construyeron superan el umbral de SQL Server: 500 si la tabla tenía hasta 500 filas al construirlas y, si tenía más, el menor entre 500 más el 20 % de las filas y la raíz cuadrada de 1.000 por las filas. Unas estadísticas construidas de una instantánea nunca reemplazan a otras de la misma columna construidas de una instantánea posterior. Ninguna otra operación de la aplicación espera a que se calculen (analizar puede esperar, como fije 7.3); guardarlas es una escritura corta que toma el turno de escritura (G4), así que una escritura de la aplicación puede esperarla como a cualquier otro escritor, y esa espera cuenta para su tiempo límite (T4). |
| N9 | **Compresión por diccionario**: aplazada (N2). Desde 7.1, el formato de las tablas reserva lo que necesitará, para que añadirla después no cambie la disposición de las filas ni obligue a reescribir las existentes; lo comprueba la revisión del diseño de 7.1, no una prueba. Reservarlo no hace compatible activarla: si su cambio es incompatible lo declara 7.4 con B12. |
| N10 | **Errores nuevos** (D8 del padre): **clave duplicada**, al insertar una fila cuya clave primaria ya existe, o al guardar una fila con los valores de un índice único que ya tiene otra fila, sin nulos en esas columnas (N6, N7), y en ese caso el error nombra el índice, como SQL Server, que informa los dos casos como clave duplicada; **esquema distinto**, al abrir o crear una tabla con un esquema que no es el suyo, en los casos que fije 7.1; y **valor no válido**, para un valor que no cumple las reglas de su columna o un nulo en una columna que no los admite (N4). Una fila, una clave primaria, un nombre de columna o una entrada de índice más grandes que su máximo fallan con la clase de tamaño no válido (B7). Los tres se detectan antes de que la operación cambie nada y siguen la regla de los errores de validación pública de B7. Sus datos y su precedencia con los demás fallos los fija el PRD hijo que los introduce. |
| N11 | **Medición**: cada PRD hijo incluye mediciones de una sola carga antes y después de su cambio, en Windows y en Linux, con el plan de energía del banco. 7.1 y 7.2 añaden al banco comparativo (módulo 6) cargas de tablas frente a las tablas de Voron, con el umbral de K7 (Krummi al menos 0,80 veces Voron): 7.1 insertar con clave `ulid` en orden y con `uuid` al azar, la carga masiva en orden de clave primaria, leer por clave primaria, recorrer un intervalo de la clave primaria, actualizar y borrar; 7.2 insertar con dos índices, buscar por un índice y recorrer un intervalo de un índice. Los demás motores del banco no ejecutan estas cargas y sus celdas dicen `no aplica`, como en K6: son almacenes clave-valor sin esquema ni índices secundarios (las tablas de redb son mapas de clave y valor), y el banco no los emula. 7.3 mide, con una sola carga, cuánto cuesta a las cargas de escritura de 7.1 construir las estadísticas en segundo plano. |

*Anotaciones del 2026-11-10 desde las tablas con esquema ([tables-schema.prd.md](tables-schema.prd.md), 7.1):*

- *N1 y AC-N5: el catálogo tiene tres clases de objeto, árboles, tablas y secuencias, con un solo espacio de nombres;
  Existe y Listar valen para las tres y las demás operaciones fallan con tipo distinto sobre otra clase (E14).*
- *N4: no hay columnas autonuméricas; la frase «Las columnas autonuméricas son enteros de 64 bits (N6)» deja de valer
  (E14). Los dominios de las fechas y las horas son los de E8.*
- *N6: «todas pasan la fila entera» y «reemplaza la fila» dejan de valer: actualizar, e insertar o actualizar cuando la
  fila existe, cambian solo las columnas que se dan (E10). Los generadores de `ulid` y `uuid` son funciones públicas
  (E15). Las columnas autonuméricas se sustituyen por secuencias (E14), con la misma promesa: un valor confirmado nunca
  vuelve a darse, aunque se borren filas, se vacíe la tabla o el proceso caiga.*
- *N10: se añade una cuarta clase, esquema no válido (E5).*
- *AC-N3: la segunda inserción o actualización actualiza las columnas que se dan; desaparecen los casos de una clave
  generada que choca con una explícita y del contador autonumérico agotado; y una carga fallida deja las secuencias como
  antes (E13). Los casos con índices los comprueba 7.2.*
- *AC-N4: la segunda parte vale para las secuencias en lugar de la columna autonumérica (AC11 de 7.1).*
- *AC-N7: quitar una columna de un índice falla con esquema no válido (E5).*
- *Fuera de alcance: «Actualizar parte de una fila: actualizar reemplaza la fila entera (N6)» deja de valer, y
  «Reiniciar o fijar el contador de una columna autonumérica» pasa a ser reiniciar o fijar una secuencia después de
  crearla.*

*Anotación del 2026-11-15 desde las proyecciones ([tables-projection.prd.md](tables-projection.prd.md)): N10: esquema
distinto también cubre los usos de una proyección que dependen del esquema (PY1–PY3).*

*Anotaciones del 2026-11-18 desde los índices secundarios ([tables-indexes.prd.md](tables-indexes.prd.md)):*

- *N5: una columna vector puede ser columna incluida de un índice, pero no columna de su clave (decisión IX1).*
- *N10: esquema distinto cubre también crear si no existe un índice con otra definición y recorrer un índice que no existe (decisiones IX2 e IX9).*
- *Salida: los valores de las columnas de la clave de un índice y de la clave primaria leídos de una entrada se devuelven decodificados en una copia propia de la aplicación, también el texto y los bytes (decisión IX8).*

*Anotaciones del 2026-11-19 desde las estadísticas ([tables-statistics.prd.md](tables-statistics.prd.md), 7.3):*

- *N8: no hay estimación por defecto. Sin estadísticas, la estimación muestrea en ese momento y espera a la muestra,
  así que «Ninguna otra operación de la aplicación espera a que se calculen» deja de valer para la estimación; con
  estadísticas viejas, las usa y pide renovarlas en segundo plano (ST2, ST3). Leer las estadísticas es también una
  operación pública (ST16); analizar se pide en el almacén o en una escritura (ST7); lo que suma al contador lo fija ST4.
  Usuario: «si no espera al muestreo entonces tiene que usar "suposiciones" (guesses) y es lo que no quiero».*
- *Flujo, paso 5: la estimación muestrea cuando faltan las estadísticas y las renueva en segundo plano cuando están
  viejas.*
- *AC-N8: «responde la estimación por defecto sin esperar» y «responde la estimación por defecto y se vuelven a
  construir» dejan de valer; los casos pasan a AC-ST2 y AC-ST4, y AC-ST2 a AC-ST8 cumplen el criterio.*
- *Reparto, fila 7.3: lo que fija queda fijado en tables-statistics.prd.md.*
- *Fuera de alcance: «Estadísticas de varias columnas juntas» pasa a excluir solo las declaradas sobre columnas sin
  índice; las de los prefijos de los índices entran en 7.3 (ST17, ST18).*

### Estado de partida

Verificado el 2026-11-09 en el código, en los PRD de los ascendientes y en los documentos de diseño:

| Aspecto | Situación |
|---|---|
| Datos | Las aplicaciones guardan bytes en árboles con nombre: normales, multivalor y de tamaño fijo (módulo 5). En los normales y multivalor, las claves son bytes de 1 a 2.048, ordenados byte a byte (B5); los valores de los normales, de 0 a 256 MiB (B6), y cada valor de un multivalor, hasta 2.048 bytes. Los de tamaño fijo tienen claves enteras sin signo de 64 bits, en orden numérico, y valores de la longitud fija del árbol, de 0 a 2.048 bytes. No hay tipos, esquema ni índices secundarios. |
| Catálogo | Guarda árboles con su tipo: normal, multivalor o de tamaño fijo (B3), con los modos de apertura y las operaciones de A1 y A3. |
| Formato | La versión de formato vigente es `10`. |
| Banco | 22 cargas con umbral frente a Voron (K7); ninguna de tablas. |
| Voron (referencia) | Tablas con esquema e índices, cuyas filas no llevan tipos ni nulos y cuyas claves construye la aplicación; compresión opcional por diccionario (documentos de diseño 1, 2 y 7). |
| Diseño | Ocho documentos de diseño con las decisiones del usuario y una verificación de sus afirmaciones contra fuentes primarias ([README](../../../design/tables/README.md), [verificación](../../../design/tables/verification.md)). *Anotación del 2026-11-10:* desde ese día son doce; los cuatro añadidos tratan del formato de intercambio de las consultas, las herramientas para agentes, el contrato de las consultas y los índices vectoriales aproximados. *Anotación del 2026-11-12:* son catorce; los dos añadidos tratan de las columnas JSON y de los índices sobre JSON. |

## Actores

- **Desarrollador integrador**: define tablas con su esquema y sus índices, y guarda, lee, actualiza, borra y recorre
  filas dentro de transacciones, en Rust.
- **Mantenedor**: ejecuta en Windows y en Linux las pruebas y las mediciones de N11.

## Flujo

1. La aplicación crea una tabla con su esquema y su clave primaria (N3), y después sus índices (N7).
2. Dentro de una transacción abre la tabla por su nombre (N1).
3. En una escritura inserta, actualiza y borra filas; cada valor se valida contra su columna antes de cambiar nada
   (N4, N10), y los índices quedan de acuerdo con la tabla (N7).
4. En cualquier transacción lee filas por su clave primaria y las recorre por intervalos de la clave primaria o de
   un índice (N6, N7).
5. Al pedir una estimación, las estadísticas que faltan o están viejas se construyen en segundo plano (N8).

## Reparto en PRD hijos

Cada submódulo es un PRD hijo de este, con su propia ejecución. El orden es el de construcción. La columna *Fija*
nombra lo que ese PRD hijo debe decidir, además de su contenido y de sus propios errores.

| # | PRD hijo | Contenido | Fija |
|---|---|---|---|
| 7.1 | **Tablas con esquema** | Crear, abrir, borrar, renombrar, vaciar y listar tablas (N1); los tipos de columna (N4); la clave primaria (N6); insertar, actualizar, insertar o actualizar, leer y borrar filas por la clave primaria; recorrer y borrar por intervalos de la clave primaria; el número de filas; la carga masiva; los generadores de N6; los cambios de esquema (N3); lo que reserva para la compresión (N9); sus cargas del banco (N11). | **Catálogo**: qué modos de apertura (A1) y qué operaciones del catálogo (A3) tienen las tablas, con qué clases fallan una tabla que no existe o que ya existe, si renombrar una tabla puede sobrescribir otra tabla, qué vacía vaciar una tabla (sus índices y sus estadísticas incluidos), y cuáles de sus operaciones son cambios para T10 y terminan las referencias (A10), por ejemplo actualizar sin encontrar la fila o insertar o actualizar con una fila idéntica; la precedencia de sus fallos (A7) y cuáles pueden fallar con daño (A9). **Límites**: el máximo de columnas y de bytes de un nombre de columna; el tamaño máximo de una fila y de una clave primaria (N10 fija su clase). **Filas**: qué pasa al leer o borrar una fila que no existe; si se puede actualizar la clave primaria de una fila y cómo; qué pasa con las columnas que faltan, sobran o tienen un nulo explícito al guardar una fila; los dominios de `date`, `time`, `timestamp`, `duration` y `decimal`, y qué pasa con un `decimal` con más decimales que su escala o un instante o una duración con más resolución que el microsegundo; si un `string` admite el carácter nulo y si el texto vacío y los bytes vacíos son claves válidas; cómo se pasan los valores `uuid` con el orden de SQL Server (en el orden de bytes de RFC 9562 o en el de .NET), los elementos `f16` y el orden de los bits de un elemento `bit`; si un `-0` dentro de un vector se guarda tal cual; si hay conversión de `ulid` y `uuid` a texto y desde texto; si las columnas de la clave primaria pueden ser descendentes; la semántica de los límites de un intervalo de una clave primaria compuesta (prefijo, inclusivo o exclusivo), al recorrerlo y al borrarlo, y si borrar un intervalo informa cuántas filas borró. **Carga masiva**: si exige una tabla vacía o sin índices; qué hace con filas fuera de orden; si una fila con una clave repetida o un valor no válido hace fallar la carga entera sin efecto o solo esa fila; si cabe en una sola escritura y qué informa al terminar. **Generadores**: si los de `ulid` y `uuid` son funciones públicas o asignan el valor al insertar, y cómo conoce la aplicación el valor asignado; el signo de las columnas autonuméricas, su primer valor, si deja huecos, cuántas puede tener una tabla y si deben ser de la clave primaria; si se admite un valor explícito en ellas y qué hace insertar o actualizar con una fila sin ese valor; si una escritura abortada consume valores; con qué clase falla una inserción cuando el contador llega a su máximo; si los generadores son monótonos por almacén o por proceso, y qué hacen si el reloj retrocede o se agota el incremento. **Esquema**: con qué clase falla crear una tabla con un esquema no válido (sin clave primaria, con una columna de la clave que admite nulos o es un vector, con nombres de columna repetidos, con una clave que nombra una columna que no existe, con un vector o un `decimal` fuera de sus límites, con la métrica coseno sobre elementos `bit`); cuándo abrir una tabla es un esquema distinto; con qué clase falla un cambio de esquema no admitido; qué pasa al añadir una columna con un nombre que ya existe o que pasa del máximo, al renombrar a un nombre que ya existe, y al añadir una columna con el nombre de una que se quitó (sus valores anteriores nunca reaparecen); si una columna puede tener valor por defecto al crear la tabla, si el valor por defecto puede ser nulo, con qué clase falla un valor por defecto que no cumple las reglas de su columna, y si una columna añadida con valor por defecto puede no admitir nulos; si se puede añadir a una tabla con filas una columna autonumérica o un vector con métrica coseno; si la versión del esquema es visible para la aplicación; qué ven las aperturas de una tabla cuyo esquema cambia en la misma escritura. **Errores**: los datos de clave duplicada, esquema distinto y valor no válido, y su precedencia (A7). **Banco**: los datos de sus cargas (número de filas, columnas y tamaño de fila, distribución de las claves), su esquema en Voron, sus números y nombres, su lugar en el informe, en la tabla de tamaños y en la ejecución reducida (K4, K11). **Formato**: si su cambio es incompatible (B12). **Pruebas**: los vectores de prueba del orden de SQL Server (AC-N1), el reloj de prueba de los generadores (AC-N4), el gancho que muestra que ningún nombre alcanza un árbol interno (AC-N5) el que cuenta las páginas que escribe un cambio de esquema (AC-N6) y el que cuenta las páginas que lee pedir el número de filas (AC-N3). **Información**: si los árboles internos cuentan en los números de árboles y de páginas de la información del almacén (P12). |
| 7.2 | **Índices secundarios** | Crear y borrar índices únicos y no únicos sobre una tabla con filas; mantenerlos en cada escritura; leer y recorrer por un índice; columnas incluidas, descendentes y el orden de los nulos (N5, N7); sus cargas del banco (N11). | Cómo se nombran los índices y sus límites (bytes del nombre, columnas por índice e índices por tabla); si la definición de una tabla puede llevar sus índices al crearla; con qué clase fallan las definiciones de índice no válidas (sobre una tabla o una columna que no existe, sobre una columna vector, con una columna repetida en la clave o en las incluidas, o en las dos, igual a la clave primaria o a un prefijo de ella, igual a otro índice, o con un nombre que ya existe); si una columna vector puede ser columna incluida; si crear un índice sobre una tabla grande se hace en una sola transacción y dónde ordena sus entradas; qué pasa al crear un índice único sobre filas que ya repiten sus valores; qué pasa con un índice al renombrar una de sus columnas; los datos de clave duplicada en un índice único; qué operaciones devuelven las filas por un índice y si devuelven sin copia los valores de las columnas incluidas; la semántica de los límites de un intervalo de un índice (como en 7.1) y si un intervalo puede incluir los nulos o pedir solo ellos; cuándo una entrada de índice es demasiado grande y qué dice su error; los datos de sus cargas del banco, como en 7.1. |
| 7.3 | **Estadísticas** | Las estadísticas por columna, la operación de estimación, su construcción en segundo plano y la operación analizar (N8); su medición (N11). | Qué estimaciones ofrece y cuál es la estimación por defecto; en qué transacciones se piden la estimación y analizar, y si analizar espera a que se guarden; qué cuenta como modificación para el umbral (por ejemplo, insertar o actualizar con una fila idéntica, actualizar sin encontrar la fila o una escritura abortada); si una escritura que pide una estimación ve sus propios cambios; si existen en modo temporal; qué pasa si guardarlas falla, dónde se informa y qué pasa con ellas en un almacén que no acepta escrituras (T12), incluido si un fallo al guardarlas es el primer fallo de T12 y quién lo informa; cuándo se crea y cuándo termina el hilo que muestrea, si cerrar el almacén espera a un muestreo en curso, y en qué lugar de la cola de escritura (T4) entra el guardado; qué informa la información del almacén (P12) de ellas, si algo; qué pasa con ellas al quitar o renombrar una columna o borrar la tabla; si las columnas vector tienen estadísticas; los ganchos de pruebas que detienen el muestreo y el guardado de las estadísticas (AC-N8). |
| 7.4 | **Compresión por diccionario** | Aplazado (N2, N9). | — |

*Anotación del 2026-11-10 (PRD hijo 7.5):* se añade el PRD hijo **7.5, Columnas JSON**, que se construye después de
7.3 y antes del módulo 17, para que las consultas puedan filtrar por rutas del JSON desde el principio. Usuario: «Sí
anótalo en el reparto del PRD de tablas como PRD hijo 7.5, después de 7.3».

- **Contenido**:
  - un tipo de columna JSON guardado en un formato binario que se lee sin copiar ni analizar el texto, al estilo del
    blittable de RavenDB o del `jsonb` de PostgreSQL;
  - construirlo, navegarlo y leer sus propiedades;
  - su conversión a texto y desde texto;
  - sus cargas del banco (N11).
- **Fija**:
  - el formato binario (uno de esos dos o uno propio) y si su cambio es incompatible (B12);
  - cómo se representan los números (enteros, `f64`, `decimal`), las claves repetidas y el orden de las propiedades,
    y cuándo dos documentos son iguales;
  - sus límites de tamaño y de profundidad;
  - con qué clases fallan un texto o un documento no válidos;
  - si el análisis del texto usa una dependencia (con su anotación en D12, como P11 del documento de diseño 11) o un
    lector propio;
  - que un JSON no es clave primaria ni columna de un índice.
- Con esta anotación, N2 cierra el módulo 7 cuando lo están 7.1, 7.2, 7.3 y 7.5, y «JSON» deja de estar en *Fuera de
  alcance*.
- Indexar rutas dentro de un JSON queda fuera de 7.5 (índices sobre expresiones, después de 7.2 y del módulo 17).

*Anotación del 2026-11-12 (PRD hijo 7.5, desde los documentos de diseño 13 y 14):* el diseño de las columnas JSON se
decidió con el usuario el 2026-11-11 en el [documento de diseño 13](../../../design/tables/13-json-column-storage.md),
que es el estado de partida técnico de 7.5 (los bytes exactos que se guardan son su *Stored form v1*), y cambia o
precisa la anotación anterior:

- **Contenido**: deja de valer «guardado en un formato binario que se lee sin copiar ni analizar el texto, al estilo
  del blittable de RavenDB o del `jsonb` de PostgreSQL». Un valor JSON se guarda como texto JSON normalizado, detrás
  de un byte que identifica su forma: sin espacios fuera de las cadenas, con una sola forma de escribir cada cadena,
  con las propiedades en el orden en que llegan y con cada número tal como se escribió. El documento entero se lee
  sin copiarlo, como texto. 7.5 navega el documento y lee sus propiedades recorriendo el texto; hacerlo sin
  recorrerlo queda para cuando lo pida la medición de las consultas del módulo 17, y el byte de la forma reserva lo
  que necesitará, sin reescribir las filas guardadas. N4 gana el tipo de columna JSON; cómo se declara lo fija 7.5.
  Usuario: «Sí, pero ¿el texto canónico no exigiría que dos json con el mismo significado fueran iguales en texto?»
  y, aclarado que el texto es normalizado y no canónico, «Sí seguimos con el texto normalizado.»; sobre leer
  propiedades sin recorrer el texto, «Después, pero que quede anotado».
- **Queda fijado**, de lo que fija 7.5:
  - el formato: el texto normalizado de arriba;
  - qué es un valor JSON válido: cualquier valor de RFC 8259 en UTF-8 válido, también uno suelto en la raíz (`42`,
    `"x"`, `true`, `null`), con espacios alrededor que se descartan; no lo son un texto vacío, datos después del
    valor, una marca BOM, `NaN`, `Infinity`, `+1` ni `01`, ni un sustituto UTF-16 suelto (`"\ud800"`); un par de
    sustitutos escapado se guarda como su carácter; U+0000 y los no caracteres de Unicode son válidos, como en una
    columna `string` (E7 de 7.1). Usuario: «Sí adóptalas.»;
  - el nulo de la columna y el `null` del JSON son distintos: el primero es la columna sin valor, el segundo un
    documento guardado. Una columna JSON admite un valor por defecto como cualquier otra (E4 de 7.1), una constante
    que se guarda normalizada; por eso, a diferencia de E4, un valor por defecto que es el documento `null` es un
    valor por defecto, no la falta de él, y uno que no es JSON válido falla con esquema no válido (E5). Usuario: el
    mismo «Sí adóptalas.»;
  - los números: se guardan tal como se escribieron, sin perder ningún dígito;
  - las claves repetidas: un objeto con dos propiedades del mismo nombre, también si solo coinciden al leer sus
    escapes, no es válido. Usuario, a rechazarlas nombrando el objeto y el nombre repetido: «Sí»;
  - el orden de las propiedades: se conserva;
  - cuándo dos documentos son iguales: al guardar una fila (la actualización idéntica de E10 de 7.1), cuando su texto
    normalizado es el mismo, así que `{"n":1}` y `{"n":1.0}` son distintos; la igualdad por significado, con las
    propiedades en cualquier orden y los números por su valor (`1`, `1.0` y `1e0` son el mismo número), es una
    operación de las consultas;
  - la profundidad: como mucho 128 objetos o arrays anidados. Usuario: «128»;
  - la clase de error: un texto que no es un valor JSON válido, con claves repetidas o con más de 128 niveles falla
    con valor no válido (N10); un documento que hace la fila más grande que su máximo falla con tamaño no válido (E18
    de 7.1);
  - el análisis del texto: un lector propio, sin dependencia nueva, probado por capas. Usuario: «El propio, pero muy
    bien probado.»;
  - la lectura de un valor del JSON como un tipo de Krummi, en 7.5 y en las consultas del módulo 17: un texto, de una
    cadena, sin copiarlo si la cadena no tiene escapes y decodificado en una copia propia de la aplicación si los tiene,
    una excepción a D11 y AC20 como la de los demás valores decodificados (*Salida*), mientras el documento entero se
    lee siempre sin copia (usuario, entre una copia solo para las cadenas con escapes, una copia siempre y una vista sin
    copia que decodifica al vuelo: «A»); un booleano, de `true` o `false`; un entero `i64` o `u64`, solo si su valor es
    entero exacto, en cualquier notación (usuario: «valor entero exacto»); un `decimal`, redondeado a su escala como en
    E8 de 7.1 (usuario: «Redondear no es inventar, yo redondearía.»); un `f64`, redondeado al más cercano, con la
    normalización de N4 (`-0` como `+0`); una fecha, solo de un texto `AAAA-MM-DD`; un instante, solo de un texto
    `AAAA-MM-DDTHH:MM:SS` con fracción de segundo opcional y desfase `Z`, `+HH:MM` o `-HH:MM` (`2026-11-11T10:23:00Z`,
    `2026-11-11T00:30:00+01:00`, con `T` y `Z` en mayúscula), convertido a UTC y con la fracción redondeada al
    microsegundo como en E8 (usuario, a aceptar desfases y redondear: «Sí»; a añadir fechas e instantes: «Las dos
    cosas»). Lo que falta, es `null`, es de otra clase o no cabe da nulo, nunca un error: un entero con fracción
    (`100.6`) o fuera de rango; un `decimal` que redondeado pasa de su precisión; un `f64` fuera de su rango (`1e400`);
    una fecha que no existe (`2026-02-30`) o fuera de los dominios de E8; un instante sin desfase, en minúsculas, con un
    espacio en lugar de `T` o con un segundo intercalar; un número donde se pide una fecha o un instante.
- **Se precisa**, de la anotación anterior, que un JSON no es clave primaria ni columna de la clave de un índice
  secundario; si puede ser una columna incluida de un índice lo fija 7.2.
- **Sigue fijando 7.5**: si su cambio es incompatible (B12); cómo se declara una columna JSON; con qué clase falla un
  esquema con un JSON en la clave primaria o en la clave de un índice; los datos de sus errores y su precedencia; sus
  cargas del banco (N11).
- **Fuera de 7.5**:
  - actualizar o quitar una propiedad por su ruta dentro del motor: la aplicación lee el documento, lo cambia y lo
    guarda. Usuario: «no en 7.5»;
  - los índices sobre JSON, sobre expresiones, multientrada e invertidos, que se diseñan en el
    [documento de diseño 14](../../../design/tables/14-json-indexes.md) (usuario, a diseñarlos en un documento propio:
    «documento propio»). Un índice multientrada da varias entradas a una misma fila, una por elemento de un array;
    se llamaba «multivalor» hasta el 2026-11-13, nombre que en Krummi es el del árbol de una clave con varios valores
    (usuario: «Sí, creo que es un nombre mucho más adecuado índice multientrada (multi-entry index)»). Salvo el índice
    multientrada sobre varias propiedades de un mismo elemento, que es requisito del módulo 17 (abajo), sus preguntas
    de producto, incluido en qué PRD entra cada clase de índice, quedan abiertas hasta que se planifiquen (usuario:
    «Sí»); hasta entonces, N2 y el reparto no cambian, y si esos índices cambian la definición de índice secundario
    (una lista de columnas, N7) lo decide el PRD que los introduzca.
- **Para el módulo 17**, sin cambiar este PRD, el documento 13 deja como requisitos de sus consultas:
  - las rutas dentro de un JSON como listas de pasos, cada uno un nombre de propiedad o una posición de array, sin un
    texto que analizar; un paso que espera un array o un objeto y encuentra otra cosa da «no está»;
  - las condiciones sobre los elementos de un array: que alguno, o que todos, cumplan una condición sobre sus propias
    propiedades, de modo que un mismo elemento cumpla la condición entera, por ejemplo los pedidos con una línea de un
    producto P y una cantidad mayor que C; «todos» es verdadero en un array vacío; un elemento cuya condición es
    desconocida no cuenta para «alguno», y hace desconocido «todos» salvo que otro elemento la haga falsa. Usuario:
    «sí»;
  - el índice multientrada sobre varias propiedades de un mismo elemento, que resuelve esas condiciones (documento de
    diseño 14), con el mismo «sí», a anotarlo junto con ellas;
  - la lectura tipada de arriba, también para fechas e instantes;
  - una carga del banco que filtre por una ruta, con la que decide si se lee una propiedad sin recorrer el texto.

*Anotación del 2026-11-10 (PRD hijo 7.6):* se añade el PRD hijo **7.6, Índices vectoriales aproximados**, que se
construye después de 7.5 y antes del módulo 17, para que las consultas puedan buscar de forma aproximada los vecinos
más cercanos desde el principio. Usuario: «Además los índices ANN hay que inluirlos en el alcance» y, sobre el orden,
«Tras 7.5, antes del 17». Su estado de partida técnico es el
[documento de diseño 12](../../../design/tables/12-vector-indexes.md).

- **Contenido**:
  - un índice vectorial aproximado sobre una columna vector, de una clase distinta de los índices secundarios de 7.2,
    que el documento 12 recomienda construir con particiones (IVF) y códigos de 8 bits por componente (SQ8);
  - su mantenimiento asíncrono: la escritura de una fila solo deja anotado que la fila cambió, y el almacén pone el
    índice al día en segundo plano, en lotes cortos, también al crearlo y al reconstruirlo;
  - una operación de búsqueda de los k vecinos más cercanos que la aplicación pide expresamente como aproximada y que
    devuelve solo filas visibles en su instantánea, que cumplen sus filtros, con su distancia exacta;
  - sus cargas del banco (N11).

  Las búsquedas exactas por distancia nunca usan el índice y devuelven lo mismo que sin él.
- **Fija**:
  - el algoritmo, después de medir el experimento del documento 12 contra sus umbrales de recall y latencia, y la
    alternativa si no los cumple;
  - qué tipos de elemento y qué métricas admite;
  - si una búsqueda ve las filas que el índice aún no ha puesto al día y qué hace cuando son demasiadas;
  - si un índice puede declarar columnas de partición, por ejemplo por inquilino;
  - si crearlo y reconstruirlo se hace sin bloquear las escrituras;
  - cuánto tiempo puede retener el turno de escritura (G4) el trabajo en segundo plano, y con cuántos hilos y cuánta
    memoria entrena;
  - si las reconstrucciones son explícitas y qué métricas las recomiendan;
  - qué hace una búsqueda mientras el índice no está listo y cuántas filas necesita para entrenarse;
  - con qué clases fallan las definiciones no válidas;
  - qué informa del índice la información del almacén (P12);
  - qué pasa con el índice al borrar la tabla o la columna, al vaciar la tabla y en la carga masiva;
  - si su cambio es incompatible (B12).

  Las seis preguntas de producto del documento 12 son parte de lo que fija.
- Con esta anotación:
  - N2 cierra el módulo 7 cuando lo están 7.1, 7.2, 7.3, 7.5 y 7.6;
  - la frase de N7 «Cada escritura de una fila deja todos sus índices de acuerdo con la tabla en la misma transacción
    (G1)» vale para los índices secundarios, no para los vectoriales aproximados, que se ponen al día después de la
    transacción;
  - «no pueden ser clave primaria ni columnas de un índice», de N5, se refiere a la clave primaria y a los índices
    secundarios;
  - e «Índices vectoriales» deja de estar en *Fuera de alcance*.
- Filtrar por un umbral de distancia sigue siendo exacto y del módulo 17.

## Validaciones y mensajes

Valen las de los ascendientes. Este módulo añade estas situaciones, solo de la biblioteca, con la regla de N10:

| Situación | Clase de error |
|---|---|
| Se inserta una fila cuya clave primaria ya existe | Clave duplicada (nueva) |
| Se guarda una fila con los valores de un índice único que ya tiene otra fila, sin nulos en esas columnas | Clave duplicada (nueva), con el nombre del índice |
| Se abre o se crea una tabla con un esquema que no es el suyo, en los casos que fije 7.1 | Esquema distinto (nueva) |
| Un valor que no cumple las reglas de su columna, o un nulo en una columna que no los admite | Valor no válido (nueva) |
| Una fila, una clave primaria, un nombre de columna o una entrada de índice más grandes que su máximo | Tamaño no válido (B7) |
| Una operación de árbol sobre una tabla, o de tabla sobre un árbol, incluido renombrar con sobrescritura sobre el otro tipo | Tipo distinto (B3) |

Actualizar una fila que no existe no es un error: la operación informa de que no la encontró (N6). Cada PRD hijo
añade las situaciones de sus operaciones y fija los datos de cada error.

## Salida

El módulo no escribe nada en pantalla. Una lectura devuelve los valores guardados, con la normalización de N4 (`-0` y
NaN); los de `string`, `bytes`, vector y JSON (desde 7.5), sin copiarlos (D11, T16, B10), y los demás, decodificados en
una copia propia de la aplicación (excepción a D11, como Listar en A3). La información del almacén (P12) no añade datos
salvo lo que fije 7.3. El informe del banco añade las cargas de N11 y cuenta sus umbrales con los demás.
*Anotación del 2026-11-12 (desde la anotación de 7.5 de ese día):* de un JSON, el documento entero se devuelve sin
copiarlo; una cadena del JSON leída como texto se devuelve sin copiarla si no tiene escapes, y decodificada en una copia
propia de la aplicación si los tiene.

## Reutilización

- Los árboles B+, su catálogo, sus reglas de nombres, tamaños y errores (B1–B12, A1–A15), y sus recorridos (5.2).
- Las transacciones, el diario y la recuperación: los cambios de las tablas, sus índices y sus estadísticas son
  cambios de árboles (G1, G5).
- Las mediciones de una sola carga del plan de rendimiento y el banco comparativo (módulo 6).

## Fuera de alcance

- Consultas con filtros, orden, límites y agregados, el planificador por coste y los planes de ejecución (módulo 17).
- La interfaz C y el binding C# (módulo 18).
- Índices vectoriales: filtrar por distancia es un recorrido del módulo 17. *Anotación del 2026-11-10:* los índices
  vectoriales aproximados entran en el alcance como PRD hijo 7.6 (ver *Reparto en PRD hijos*), con el
  [documento de diseño 12](../../../design/tables/12-vector-indexes.md) como estado de partida técnico.
- Las secciones de datos en bruto: guardar bytes bajo un identificador lo cubren los containers (módulo 13).
- Joins, `GROUP BY`, vistas y disparadores; las claves foráneas, restricciones entre tablas, y las restricciones
  `CHECK`.
- Arrays, enums, JSON, zonas horarias guardadas, `f16` como escalar y la comparación de texto sin distinguir
  mayúsculas. *Anotación del 2026-11-12:* JSON deja de estar fuera de alcance desde la anotación del 2026-11-10 del
  PRD hijo 7.5 (ver *Reparto en PRD hijos*).
- Cambiar el tipo de una columna o la clave primaria de una tabla, prohibir nulos en una columna existente, y cambiar
  el valor por defecto de una columna o el orden de las columnas.
- Actualizar parte de una fila: actualizar reemplaza la fila entera (N6). *Anotación del 2026-11-12:* deja de valer
  desde las anotaciones del 2026-11-10 desde 7.1 (ver *Decisiones de producto*).
- Filas más grandes que el máximo que fije 7.1 (streams, módulo 8).
- Tablas sin clave primaria.
- Índices parciales (reservados, N7) y la compresión por diccionario (aplazada, N9).
- Conteos exactos de las filas entre dos claves (el número de filas de la tabla entera sí está, N6).
- Reiniciar o fijar el contador de una columna autonumérica. *Anotación del 2026-11-12:* pasa a ser reiniciar o fijar
  una secuencia después de crearla, desde las anotaciones del 2026-11-10 desde 7.1 (ver *Decisiones de producto*).
- Renombrar un índice o cambiar su definición: se borra y se vuelve a crear.
- Estadísticas de varias columnas juntas.
- Importar y exportar archivos de filas (CSV, JSON por líneas, Parquet): módulo 19. Desde la aplicación, la carga
  masiva importa (N6) y un recorrido exporta.
- Cifrar las tablas: la confidencialidad en reposo se delega en el sistema de archivos (anotación del 2026-11-09 al
  padre).
- Mostrar desde la herramienta de línea de comandos las filas decodificadas según su esquema (módulo 12); la
  verificación y el rescate sí cubren las tablas, como árboles (N1).
- Varias escrituras a la vez sobre tablas distintas del mismo almacén (G4).

## Dejado a la implementación

- Cómo se codifican las filas y las claves y cómo se organizan las tablas, sus índices y sus estadísticas en árboles,
  siempre que cumplan las garantías del padre y este PRD. Los documentos de diseño fijan su punto de partida:
  filas agrupadas por la clave primaria, claves con la codificación de tuplas de FoundationDB, y entradas de índice
  formadas por sus columnas y la clave primaria.
- Los nombres de las operaciones públicas, de los tipos y de las clases de error nuevas, que siguen las convenciones
  de Rust y están en inglés (D8 del padre).

## Criterios de aceptación

Valen los de los ascendientes. Cada criterio lo comprueba el PRD hijo que se nombra.

1. **AC-N1** — Dada una tabla con una columna de cada tipo de N4 que se puede ordenar, como clave primaria de una
   columna y como parte de una de varias, cuando se insertan filas y se recorren por la clave primaria hacia delante y
   hacia atrás, entonces vuelven en el orden de N5 para cada tipo, incluidos `ulid`, enteros negativos, `-0` y NaN, y
   `uuid` en los dos órdenes: el de RFC 9562 es el de sus 16 bytes, y el de SQL Server es el que da `SqlGuid` de .NET
   a los mismos valores, sobre vectores de prueba que fija 7.1 ordenados con `SqlGuid`, y `bytes` vacíos y con el
   byte cero en cualquier posición (y `string` vacío y con el carácter nulo si 7.1 los admite) delante de otras
   columnas de la clave; cuando se recorre un intervalo de la clave primaria, sobre una clave entera y sobre un prefijo
   de una compuesta, con cada combinación de extremos incluidos, excluidos y sin extremo, entonces vuelven exactamente
   las filas del intervalo según la semántica que fije 7.1; cuando una prueba de propiedades guarda filas al azar en
   tablas con claves primarias al azar, de una a cuatro columnas de todos los tipos que se pueden ordenar, con sus
   valores extremos (`-0` y `+0`, NaN con distintos bits, `uuid` en los dos órdenes, los tipos de fecha y hora y
   `ulid`) y claves en el límite de tamaño, entonces el orden de los recorridos, las filas de cada intervalo y las
   claves duplicadas son los de un modelo de referencia de N5 que compara los valores por su tipo, sin usar su
   codificación; y cada fila se lee con los
   valores guardados, `-0` como `+0` y todo NaN como el único NaN (7.1).
2. **AC-N2** — Dada una tabla, cuando se inserta una fila con una clave primaria que ya existe, un valor que no cumple
   las reglas de su columna (uno por cada regla de N4) o un nulo en una columna que no los admite, entonces la operación
   falla con su clase (N10), la tabla y la transacción quedan como antes, y la transacción puede confirmar sus otros
   cambios; insertar `+0` con una clave `-0` guardada, o un NaN con otro, da clave duplicada (N5); cuando se guardan
   valores en los límites de su tipo (vectores de dimensión 1, u 8 para `bit`, y 4.096, de cada tipo de elemento, y un
   `decimal` de 38 dígitos), entonces se leen iguales; cuando se crea una tabla con cada uno de los esquemas no válidos
   que enumera 7.1, entonces falla sin efecto con la clase que fije 7.1; y cuando se abre una tabla en cada caso de
   esquema distinto que fije 7.1, entonces falla con esquema distinto (7.1).
3. **AC-N3** — Dada una tabla, cuando se actualiza una fila que no existe, entonces la tabla no cambia y la operación
   informa de que no la encontró; cuando se inserta o actualiza una fila que no existe y después otra vez con otros
   valores, entonces la primera vez la inserta y la segunda la reemplaza; leer y borrar una fila que no existe dan lo
   que fije 7.1; el número de filas que informa una apertura es, tras cada operación de la prueba, el de filas que ve su
   transacción, leyendo las mismas páginas con 10 que con 100.000 filas según el gancho de pruebas de 7.1, también en
   una lectura concurrente que no ve la escritura; cuando se borra un intervalo de la clave primaria, entonces quedan
   exactamente las filas de fuera del intervalo; y cuando se cargan en masa 100.000 filas en orden de clave primaria,
   entonces la tabla tiene las mismas filas que si se hubieran insertado una a una; y las cargas con filas fuera de
   orden, repetidas o no válidas dan lo que fije 7.1, también cuando la que falla es la última, cuando una clave
   generada choca con una explícita o con una fila que ya existía y cuando se agota el contador autonumérico: si 7.1
   fija que la carga falla entera, entonces la tabla, sus índices y sus contadores quedan como antes de la carga,
   también cuando la fuente de las filas falla o la aplicación cancela la carga a mitad, y otra escritura obtiene
   después su turno (7.1).
4. **AC-N4** — Dados 100.000 valores de cada generador de N6, generados con el reloj de prueba que fije 7.1, tantos en
   el mismo milisegundo como permita el incremento que fije 7.1 y el resto en milisegundos sucesivos, desde un solo
   hilo, cuando se ordenan con el orden de su columna, entonces quedan en el orden en que se generaron; y dada una tabla
   con una columna autonumérica, cuando se confirman inserciones, se borra la fila de mayor valor y se confirma, se
   vacía la tabla y se confirma, y se simula una caída del proceso y se reabre, entonces las inserciones siguientes en
   esa tabla nunca repiten un valor de una fila confirmada (7.1).
5. **AC-N5** — Dados un árbol y una tabla en un almacén, cuando se crea una tabla con el nombre de la tabla, entonces
   falla con la clase de ya existente que fije 7.1; cuando se crea una tabla con el nombre del árbol, o un árbol con
   el de la tabla, entonces falla con tipo distinto, que gana a ya existente (B3); cuando se abren, borran,
   renombran o vacían como árbol la tabla y como tabla el árbol, o se renombra uno al nombre del otro con
   sobrescritura, entonces cada operación falla sin efecto con tipo distinto; cuando se renombra uno al nombre del
   otro sin sobrescritura, entonces falla sin efecto con ya existente; Existe dice que los dos
   nombres existen; y listar el catálogo muestra la tabla con su tipo y ninguno de sus árboles internos, que, según el
   gancho de pruebas que fije 7.1, ningún nombre alcanza como árbol (7.1).
6. **AC-N6** — Dada una tabla con filas, cuando se añade una columna que admite nulos, una con valor por defecto, se
   quita una columna, se renombra otra y se permiten nulos en otra, entonces las filas guardadas antes se leen con el
   esquema nuevo, con nulo o con el valor por defecto en las columnas añadidas, una columna añadida con el nombre de una
   quitada se lee nula, y la escritura de cada cambio escribe las mismas páginas con 10 que con 100.000 filas, según el
   gancho de pruebas que fije 7.1; y cuando se pide cada cambio no admitido de N3, o quitar una columna de la clave
   primaria, entonces falla sin efecto con la clase que fije 7.1 (7.1).
7. **AC-N7** — Dada una tabla con filas, cuando se crean índices únicos y no únicos, ascendentes y descendentes, con
   nulos primero y último, con cada combinación de dirección y orden de los nulos en columnas consecutivas, también
   detrás de columnas de `bytes` con el byte cero (y de `string` con el carácter nulo si 7.1 lo admite), y con columnas
   incluidas, entonces cada índice tiene las entradas de las filas que ya había; cuando una escritura inserta,
   actualiza, borra filas y borra un intervalo de la clave primaria y se confirma, y en otro caso se aborta o se simula
   una caída del proceso, entonces cada índice tiene exactamente las entradas de las filas de la tabla en el orden de
   N5, y dos filas con nulos en un índice único no chocan; cuando se borra un índice, entonces desaparece y la tabla no
   cambia; cuando se lee y se recorre por cada índice hacia delante y hacia atrás, entonces vuelven las filas, o los
   valores de sus columnas incluidas, en ese orden; cuando se recorre un intervalo de una columna de un índice con las
   anteriores fijadas, con cada combinación de extremos incluidos, excluidos y sin extremo, entonces vuelven exactamente
   las filas del intervalo según la semántica que fije 7.2; la misma prueba de propiedades de AC-N1, con índices al azar
   de una a cuatro columnas de cualquier dirección y orden de los nulos, da el orden y las filas del modelo de
   referencia; y cuando se guarda una fila que repite los valores sin nulos de un índice único, o cuya entrada de índice
   es demasiado grande, o se quita una columna de un índice, entonces falla sin efecto con clave duplicada, con el
   nombre del índice, con tamaño no válido o con la clase que fije 7.1, también al insertar y al actualizar en una tabla
   con varios índices únicos cuando falla cada uno de ellos por turno, y la fila, los demás índices y el número de filas
   quedan como estaban y la transacción puede confirmar sus otros cambios; y cuando se crea un índice único sobre filas
   que ya repiten sus valores y la escritura confirma otros cambios, entonces el índice no existe, la tabla y sus otros
   índices no cambian, y, si 7.2 fija que ese caso falla sin efecto, la información del almacén (P12) es la que habría
   sin el intento (7.2).
8. **AC-N8** — Dada una tabla con columnas sin índice y el muestreo detenido en el gancho que fije 7.3, cuando se pide
   una estimación sin estadísticas, entonces responde la estimación por defecto sin esperar, y las lecturas y escrituras
   de la aplicación terminan; cuando se libera el muestreo, entonces las estadísticas se guardan y la misma estimación,
   sobre datos elegidos para que difiera de la estimación por defecto, las usa; cuando las modificaciones superan el
   umbral de N8 y se pide otra estimación, entonces responde la estimación por defecto y se vuelven a construir, también
   cuando parte de esas modificaciones se hizo antes de cerrar y reabrir el almacén y cuando se vacía la tabla; cuando
   se pide analizar, entonces, cuando termina como fije 7.3, la estimación usa las estadísticas nuevas, también si se
   pide mientras se construyen en segundo plano y cualquiera de las dos termina antes, y sin que ninguna se quede
   esperando a la otra en las transacciones en que 7.3 permita pedirlo; si 7.3 permite analizar dentro de una escritura,
   cuando esa escritura cambia filas, analiza y aborta, entonces las filas, el contador y las estadísticas quedan como
   antes, y cuando cambia más filas después de analizar y confirma, entonces esas modificaciones cuentan para el umbral;
   cuando una carga masiva falla mientras se construyen estadísticas en segundo plano, entonces las que se guardan
   cuentan solo las filas de la carga que quedaron confirmadas según fije 7.1 (ninguna si la carga falla entera); y con
   el guardado de las estadísticas detenido en su gancho, una escritura de la aplicación con tiempo límite falla con
   tiempo de espera agotado al vencer (T4) (7.3).
9. **AC-N9** — Dadas las cargas de N11, cuando se comparan con las tablas de Voron en una ejecución completa del banco,
   entonces Krummi llega al umbral de K7 en cada una, en Windows y en Linux; un umbral incumplido sigue K13: no
   impide cerrar el PRD hijo y se convierte en un cambio aparte (7.1, 7.2).
10. **AC-N10** — Dado el cierre de cada PRD hijo, cuando se revisa su informe, entonces incluye las mediciones de una
    sola carga de N11, en Windows y en Linux: las cargas del banco que ya existían, antes y después de su cambio, y
    sus cargas nuevas después (7.1, 7.2, 7.3).

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones del 2026-11-10 (en sus
traducciones también):

- Padre raíz, reparto: la fila del módulo 7 pasa a decir que fija también los tipos de columna (N4) y el error de
  valor no válido (N10), sin las secciones de datos en bruto (fuera de alcance; las cubre el módulo 13), con la
  compresión por diccionario aplazada (N2). Se añaden el **módulo 17, Consultas** (árbol de consulta construido con
  una API fluida, validación, planificador por coste, planes de ejecución que la interfaz pública también acepta, y
  ejecución por cursor) y el **módulo 18, Interfaz C y binding C#**, que se construyen justo después del 7, en ese
  orden, y el **módulo 19, Importar y exportar archivos de filas** (CSV, JSON por líneas, Parquet), después del 18;
  numerados así para que no se muevan los números que ya citan otros documentos.
- Padre raíz, objetivo 2 y D4: el producto está completo con los módulos del reparto sin el 9 (anotación del
  2026-11-09 sobre el cifrado), con el 17, el 18 y el 19, y sin la compresión por diccionario (N2), aunque Voron la
  tiene.
- Padre raíz, D2: la superficie pública añade la interfaz C y el binding C# del módulo 18.
- Padre raíz, D11 y AC20: los valores de las columnas que no son `string`, `bytes`, vector ni JSON (desde 7.5) se
  devuelven decodificados en una copia propia de la aplicación (Salida), como Listar en A3. *Anotación del 2026-11-12
  (desde la anotación de 7.5 de ese día):* una cadena de un JSON leída como texto se devuelve decodificada en una copia
  propia de la aplicación cuando tiene escapes, y sin copia cuando no los tiene; el documento JSON entero sigue siendo
  una referencia sin copia. El padre raíz lleva la anotación del 2026-11-12.
- Padre raíz, Fuera de alcance: el punto «Servidor de red, protocolo remoto, lenguaje de consulta o SQL» pasa a
  excluir solo el servidor de red, el protocolo remoto, un lenguaje de consulta de texto y SQL: el árbol de consulta y
  los planes de ejecución entran en el módulo 17. El punto «Bindings para otros lenguajes (API C, .NET, Python)» pasa
  a excluir los demás lenguajes: la interfaz C y el binding .NET entran en el módulo 18.
- Padre raíz, D12 y AC21: la interfaz C (módulo 18) es, con el Pager, la única parte del motor que puede usar código
  no seguro; una dependencia que usa código no seguro se admite caso por caso, si es muy usada y mantenida, solo
  Rust (sin C), sin una alternativa segura razonable, y anotada con su motivo en el PRD que la introduce; las
  dependencias con código C quedan fuera salvo decisión expresa ([diseño](../../../design/tables/README.md)).
- Padre raíz, módulos 10, 11 y 12 del reparto: el backup, la compactación y la verificación y el rescate copian o
  comprueban también los árboles internos de las tablas, como árboles (N1).
- Padre raíz, AC19: las cargas del banco son las de K6 y K7 más las de tablas de N11.
- Banco, K7 y *Salida*: además de las 22 cargas de los módulos 1 a 5, tienen umbral las cargas de tablas de N11
  (D5), solo frente a Voron; el informe cuenta todos los umbrales, así que «Krummi cumple N de 22 umbrales» pasa a
  contar sobre el total, y K13 vale para ellos: incumplir uno termina con código `1`. K6: las cargas de tablas
  siguen su regla de motores que no las ofrecen (`no aplica`). K4 y K11: los datos de las cargas de tablas y su
  lugar en la ejecución reducida los fijan 7.1 y 7.2.
- Árboles, B3: el catálogo guarda también tablas (N1). Pedir como árbol algo que es una tabla, o al revés, falla con
  tipo distinto, con la misma precedencia sobre ya existente; y las operaciones de árbol nunca destruyen una tabla,
  así que la frase «Las operaciones que nombran un árbol sin pedir un tipo (como borrarlo) nunca fallan con tipo de
  árbol distinto» deja de valer cuando el nombre es de una tabla. B8: los datos del tamaño no válido de los
  tamaños nuevos (una entrada de índice, un nombre de columna, una clave primaria, una fila) los fijan 7.1 y 7.2,
  y los del tipo de árbol distinto pueden nombrar el tipo tabla; la lista de tipos de B3 no cambia, porque una tabla
  no es un tipo de árbol, pero comparte su catálogo.
  B11: los árboles internos de las tablas no tienen límite propio; el límite de índices por tabla que fije 7.2 es
  un límite de las tablas, no del número de árboles.
  B4 sigue valiendo porque los árboles internos de una tabla no ocupan nombres. B7: la regla de los errores de
  validación pública vale también para las tres clases de N10.
- Operaciones por clave, A1, A3, A6, A7, A9 y A10: borrar, renombrar o vaciar como árbol una tabla falla con tipo
  distinto, y Renombrar con sobrescritura no borra un destino que es una tabla, también falla con tipo distinto
  (A3 decía «sea del tipo que sea»); sin sobrescritura, un destino que es una tabla falla con árbol ya existente,
  como dice A3; Existe y Listar cubren también las tablas, con su tipo; los modos de apertura,
  las clases de no encontrado y ya existente, la precedencia, el daño y los cambios de las operaciones de tabla los
  fija 7.1.
- Transacciones, T4: la escritura que espera puede esperar a la escritura interna que guarda las estadísticas (N8). Con
  un límite de cero, una escritura falla si ese guardado está en la cola. T12: esa escritura interna puede ser la que
  encuentra el primer fallo de entrada y salida; cómo se informa lo fija 7.3.
- Árboles, Fuera de alcance «Tablas con esquema, índices secundarios y compresión de valores por diccionario (módulo
  7)»: lo cubre este módulo.

## Referencias

- Padre: [krummi.prd.md](../krummi.prd.md) v1.1 (D2, D4, D5, D8, D9, D11, D12, G1, G4, G5, AC19, AC21, reparto)
- Árboles: [trees.prd.md](../trees/trees.prd.md) (B1–B12) y sus hijos, en especial [trees-keys.prd.md](../trees/trees-keys.prd.md)
  (A1–A15) y [trees-scan.prd.md](../trees/trees-scan.prd.md)
- Transacciones: [transactions.prd.md](../transactions/transactions.prd.md) (T4, T10, T11, T12, T16)
- Banco: [benchmark.prd.md](../benchmark/benchmark.prd.md) (K4, K6, K7, K11, K13, *Salida*)
- Diseño: [docs/design/tables/README.md](../../../design/tables/README.md) y sus documentos 1 a 14
- Producto documental (aparcado): [documents.prd.md](../../documents/documents.prd.md)
