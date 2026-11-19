# Tablas con esquema de Krummi — PRD

**Versión**: 1.0
**Fecha**: 2026-11-10
**Documento padre**: [Tablas de Krummi — PRD](tables.prd.md) (v1.0)

## Objetivo y contexto

Construir el PRD hijo **7.1** del [reparto del módulo de tablas](tables.prd.md#reparto-en-prd-hijos): crear, abrir,
borrar, renombrar, vaciar, describir y listar tablas; sus tipos de columna, su clave primaria y los cambios de su
esquema; insertar, actualizar, insertar o actualizar, leer y borrar filas por la clave primaria; recorrer y borrar por
intervalos de la clave primaria; la carga masiva; los generadores de `ulid` y `uuid`; las secuencias; y las cargas de
tablas del banco comparativo. Los índices secundarios son de 7.2, las estadísticas de 7.3 y las columnas JSON de 7.5.

Las decisiones se tomaron con el usuario el 2026-11-10, sobre los documentos de diseño de las tablas
(`docs/design/tables/`), cuyas decisiones P1–P20 del documento 11 ya estaban aprobadas. Este PRD cambia reglas de sus
ascendientes, que llevan su anotación (*Anotaciones a otros PRD*):

- **Secuencias en lugar de columnas autonuméricas** (N4, N6, AC-N3, AC-N4 y *Fuera de alcance* del padre). Usuario:
  «Yo preferiría no tener autonuméricos y en su lugar tener secuencias para generarlos. Los autonuméricos tienen muy
  mala fama.»
- **Actualizar cambia solo las columnas que se dan** (N6 y *Fuera de alcance* del padre, que decían que actualizar
  reemplaza la fila entera), para que una versión anterior de la aplicación no borre las columnas que no conoce.
- **Una cuarta clase de error, esquema no válido** (N10 del padre).
- **Los dominios de las fechas y las horas son los de .NET y SQL Server** (P7 del documento de diseño 11 dejaba a 7.1
  decidir si se limitaban).
- **El catálogo tiene tres clases de objeto**: árboles, tablas y secuencias (N1 y AC-N5 del padre).
- **El tamaño no válido que depende del esquema pierde con los fallos del catálogo y del esquema** (A7 y B7, que lo
  ponían por delante de todo). Usuario: los tamaños que necesitan el esquema se comprueban «en la fase
  correspondiente, sin forzar artificialmente la precedencia».

### Decisiones de producto

| ID | Decisión |
|---|---|
| E1 | **Modos de apertura** (A1, con tablas): una tabla se abre por su nombre en uno de los modos de A1. **Abrir** recibe solo el nombre y nunca compara el esquema; si la tabla no existe, falla con árbol no encontrado (A6). **Crear nuevo**, **Abrir o crear** y **Crear o vaciar** reciben además el esquema: crean la tabla vacía con él si no existe; si existe, Crear nuevo falla con árbol ya existente (A6), y los otros dos comprueban que el esquema dado sea compatible (E6) antes de abrirla o de vaciarla. Los modos que reciben esquema lo validan con E5 antes que nada de lo demás del esquema, exista o no la tabla. **Vaciar** recibe solo el nombre y vacía la tabla (E3); si no existe, falla con árbol no encontrado. En una transacción de lectura solo existe Abrir. Las clases de A6 llevan como dato el nombre de la tabla. Un nombre que es de otra clase de objeto del catálogo falla con tipo distinto (N1), que gana a esquema distinto y a ya existente. |
| E2 | **Otras operaciones del catálogo** (A3, con tablas): **Existe**, **Borrar** (borra la tabla con sus filas, sus índices y sus estadísticas y libera sus páginas; si no existe, no hace nada) y **Renombrar**, con las reglas de A3. Renombrar sobrescribe un destino que existe solo si la aplicación lo pide de forma explícita; sin pedirlo, falla con árbol ya existente; renombrar a su mismo nombre no hace nada. Al sobrescribir, la tabla de destino desaparece como con Borrar. La tabla renombrada conserva su esquema, sus filas, sus índices, su identidad y sus aperturas. **Describir** recibe el nombre de una tabla, en las dos clases de transacción, y devuelve su esquema vigente tal como lo ve su transacción (una escritura ve sus propios cambios): por cada columna viva, en el orden del esquema (E4), su identidad, su nombre, su tipo, si admite nulos y su valor por defecto; las columnas de la clave primaria en su orden; la versión del esquema; y el estado del esquema (documento de diseño 11, P12). Si no existe, falla con árbol no encontrado; si el nombre es de otra clase de objeto, con tipo distinto. Una columna conserva su identidad al renombrarla. El estado del esquema cambia con cada cambio de esquema que da versión nueva (E4), y no al renombrar ni vaciar la tabla ni al guardar filas. La versión es informativa: identifican el estado las identidades y el estado del esquema, valores aleatorios que se vuelven a sortear si coinciden con uno que el almacén conoce (P12). Como Listar en A3, Describir devuelve una copia propia de la aplicación. **Listar** devuelve cada nombre del catálogo con su clase, en el orden de nombres de B5: árbol de cada tipo, tabla o secuencia (N1, E14). |
| E3 | **Vaciar una tabla** (modos Crear o vaciar y Vaciar, A2) quita todas sus filas y las entradas de sus índices y libera sus páginas; conserva su esquema, las definiciones de sus índices y su identidad; las demás aperturas de la tabla siguen valiendo y la ven vacía. Reinicia sus estadísticas y suma al contador de modificaciones las filas que quita (N8), las dos cosas desde 7.3. No toca ninguna secuencia. Usuario: «No quiero que vaciar una tabla equivalga a eliminar sus índices.» |
| E4 | **Esquema**: una tabla tiene de 1 a 1.024 columnas vivas (las quitadas no cuentan). El **orden del esquema** es el de las columnas al crear la tabla, con cada columna añadida al final; quitar una no mueve las demás. El nombre de una columna sigue las reglas de los nombres de árbol (B4): texto UTF-8 de 1 a 1.024 bytes, comparado por sus bytes. La clave primaria es una o varias columnas distintas, que no admiten nulos, no son vectores y no tienen valor por defecto, y todas ordenan ascendente. Las dos últimas son simplificaciones deliberadas de esta versión, no límites técnicos: una columna de una clave compuesta podría tener un valor por defecto constante (por ejemplo, un inquilino fijo). Una columna que no es de la clave puede tener valor por defecto al crear la tabla y al añadirla: una constante de su tipo, nunca una expresión; declarar nulo como valor por defecto equivale a no tenerlo. Un valor por defecto y un generador son cosas distintas: el valor por defecto es una constante que guarda el esquema y Krummi pone al insertar; los generadores de E15 y las secuencias de E14 son funciones que la aplicación llama y cuyo valor pone en la fila, y ninguno puede ser el valor por defecto de una columna. Los cambios de esquema se piden dentro de una escritura, con cuatro operaciones: **añadir** una columna, **quitar** una, **renombrar** una y **modificar** una, dando su tipo y si admite nulos. Modificar solo admite permitir los nulos en una columna que no es de la clave (N3); cambiar el tipo, prohibir los nulos o modificar una columna de la clave (que es cambiar la clave primaria) fallan con esquema no válido. Quitar, renombrar o modificar una columna que no existe falla con esquema no válido. Una columna añadida con el nombre de una quitada es nueva: tiene otra identidad, va al final del orden del esquema y nunca recupera los valores de la anterior (N3). Añadir una columna que no admite nulos exige un valor por defecto, tenga filas la tabla o no: N3 solo admite añadir una columna que admite nulos o una con valor por defecto. Una columna vector con métrica coseno se añade como cualquier otra. Una tabla se crea con la versión 1 de su esquema, y cada cambio admitido suma 1, también si hay varios en la misma escritura; un cambio que no cambia nada (renombrar una columna a su mismo nombre, modificar una columna con su mismo tipo y su misma nulabilidad, permitir nulos en una columna que ya los admite) no da versión nueva. Todas las aperturas de la tabla en la escritura que cambia su esquema ven el esquema nuevo desde ese momento (B9). |
| E5 | **Esquema no válido** (clase nueva, D8 del padre raíz): un error de validación pública con la regla de B7 y los datos de T18. Crear una tabla o cambiar su esquema falla sin efecto con **esquema no válido** cuando el esquema que resultaría incumple una regla: sin clave primaria; una columna de la clave que admite nulos, es un vector o tiene valor por defecto; una clave que nombra una columna que no existe o repite una columna; dos columnas con el mismo nombre (también al añadir una o al renombrarla a un nombre que ya existe); más de 1.024 columnas vivas; un vector fuera de los límites de N4; un `decimal(p, s)` fuera de 1 ≤ p ≤ 38 y 0 ≤ s ≤ p; la métrica coseno sobre elementos `bit`; un valor por defecto que no cumple las reglas de su columna (E8); una columna añadida que no admite nulos sin valor por defecto; un cambio que N3 no admite (cambiar el tipo, cambiar la clave primaria, prohibir los nulos); quitar, renombrar o modificar una columna que no existe; quitar una columna de la clave primaria o, desde 7.2, de un índice (AC-N7); o una versión del esquema más allá de 65.535, que se detecta antes de cambiar nada (P19). Lleva como datos la tabla, la columna si la hay y la regla incumplida, que es uno de los casos de esta lista separados por punto y coma; si se incumplen varios, se informa el primero en el orden de esta lista y, dentro de él, la primera columna en el orden del esquema. Los tamaños de los nombres siguen fallando con tamaño no válido (B7). |
| E6 | **Esquema distinto**: un esquema dado a un modo de apertura (E1) es **compatible** con la tabla que existe cuando cada columna que nombra existe en ella con el mismo tipo, la misma nulabilidad y el mismo valor por defecto, y su clave primaria tiene las mismas columnas en el mismo orden; la tabla puede tener además columnas que el esquema no nombra (las que añadió una versión posterior de la aplicación), que siguen en ella y se leen con las demás. Si no es compatible, la apertura falla sin efecto con **esquema distinto**, con la tabla, la columna y la diferencia (lo esperado y lo encontrado) como datos. Guardar una fila que trae una columna que la tabla no tiene también falla con esquema distinto, nombrando la columna. |
| E7 | **Valores de entrada**: un `uuid` se da siempre en el orden de bytes de RFC 9562, también en una columna con el orden de SQL Server, porque el orden es una propiedad de la clave; un `uuid` dado en el orden de bytes de .NET no se puede distinguir y se guarda como otro valor, así que convertirlo es cosa de quien llama (el binding C# del módulo 18 lo hará). Un elemento `f16` se da como IEEE binary16 little-endian. En un vector `bit`, el elemento i es el bit i mod 8, contando desde el menos significativo, del byte i div 8. Un `string` es cualquier texto UTF-8 válido, incluido el carácter U+0000. El texto vacío y los bytes vacíos son valores válidos, también en la clave primaria. Un `-0` dentro de un vector `f32` o `f16` se guarda como `+0`, como los escalares (N4). |
| E8 | **Dominios y redondeo**: `date` va del 0001-01-01 al 9999-12-31; `timestamp`, del 0001-01-01T00:00:00Z al 9999-12-31T23:59:59.999999Z; `time`, de 00:00:00 a 23:59:59.999999, sin segundo intercalar; `duration`, de −922.337.203.685.477.580 a 922.337.203.685.477.580 microsegundos, los extremos de `TimeSpan` de .NET truncados al microsegundo. Un `decimal` con más decimales que su escala se redondea a su escala, con los empates alejándose de cero, como SQL Server (1,235 en un `decimal(10, 2)` da 1,24; −1,235 da −1,24). Un instante, una hora o una duración con fracción por debajo del microsegundo se redondean al microsegundo más cercano, con el empate hacia el valor mayor. Un valor fuera de su dominio, también cuando lo saca el redondeo (99,995 en un `decimal(4, 2)` da 100,00, fuera de su precisión; 9999-12-31T23:59:59.9999995Z pasa del máximo), falla con valor no válido (N4). Valen igual para los valores de una fila y para los límites de un intervalo (E12). |
| E9 | **Texto de `ulid` y `uuid`**: hay conversión de cada uno a texto y desde texto, con una sola forma: `ulid` en 26 caracteres de Base32 de Crockford en mayúsculas, el primero como mucho `7`; `uuid` en el texto de RFC 9562 en minúsculas con guiones (8-4-4-4-12), en el orden de bytes de RFC 9562 también para las columnas con el orden de SQL Server. Un texto que no es esa forma (minúsculas en un `ulid`, mayúsculas, llaves o sin guiones en un `uuid`) falla con valor no válido. |
| E10 | **Guardar filas** (cambia N6 del padre): cada operación recibe la fila como pares de columna y valor, con todas las columnas de la clave primaria; una columna de la clave que falta o una columna que se da dos veces fallan con valor no válido. **Insertar**: una columna que no viene toma su valor por defecto si lo tiene y, si no, nulo; si la clave primaria ya existe, falla sin efecto con clave duplicada, aunque la fila sea idéntica. **Actualizar** busca la fila por su clave primaria y cambia solo las columnas que se dan; las demás conservan su valor; si la fila no existe, no hace nada e informa de que no la encontró. **Insertar o actualizar** actualiza si la fila existe y, si no, inserta, con esas mismas reglas. Un nulo explícito guarda nulo, nunca el valor por defecto. Una columna que no admite nulos y queda sin valor, o con un nulo, falla con valor no válido. Una actualización es **idéntica** cuando la fila que resultaría es igual, tal como la leería E11, a la que hay, por ejemplo la que da solo la clave. Insertar o actualizar informa de si insertó o actualizó. La clave primaria de una fila no cambia: para cambiarla se borra la fila y se inserta con la clave nueva en la misma escritura. |
| E11 | **Leer, borrar y contar**: **Leer** devuelve la fila de una clave primaria completa, o que no está. Una fila leída o recorrida tiene todas sus columnas vivas, en el orden del esquema, cada una accesible por su nombre o por su posición entre las columnas vivas, con nulo o con su valor por defecto en las añadidas después de guardarla (N3); una columna quitada nunca aparece. **Borrar** quita la fila y devuelve si estaba. El número de filas que informa una apertura sigue N6. |
| E12 | **Intervalos de la clave primaria**: un intervalo da valores iguales para las primeras k columnas de la clave (el prefijo, de 0 a todas) y, si k es menor que el número de columnas de la clave, opcionalmente un límite inferior y uno superior sobre la columna siguiente, cada uno incluido, excluido o ausente. Con todas las columnas iguales, el intervalo es una sola fila. Las filas se recorren hacia delante y hacia atrás en el orden de N5. Un intervalo vacío o invertido con valores válidos no devuelve ninguna fila y no es un error. Fallan con valor no válido: un valor de un tipo que no es el de su columna o fuera de su dominio, un nulo (la clave no admite nulos), un prefijo con más columnas que la clave y un límite cuando el prefijo ya da todas las columnas; NaN es un valor válido (N5). Un valor del prefijo o un límite que no cabe en el máximo de la clave falla con tamaño no válido, aunque el intervalo esté vacío, como en los recorridos de árboles. **Borrar un intervalo** toma los mismos límites y devuelve cuántas filas borró. No hay límites sobre más de una columna a la vez. |
| E13 | **Carga masiva**: es atómica y forma su propia transacción de escritura, con todas sus reglas (G1, T4, T5, T12): recibe el nombre de la tabla y una secuencia de filas que puede fallar, y carga las filas con las reglas de insertar. Si la tabla no existe falla con árbol no encontrado, y si el nombre es de otra clase de objeto, con tipo distinto. Espera su turno de escritura con el tiempo límite que se le dé y falla como una escritura al vencer (T4), antes de mirar el catálogo. Mientras carga filas, antes de confirmar, si una fila falla, si la secuencia falla, si la aplicación cancela o si hay un pánico, la carga aborta entera, y la tabla, sus índices, las secuencias y el número de filas quedan como antes; si el proceso termina de golpe en esa fase, la carga nunca se confirmó y la recuperación del almacén la descarta (G5). Su confirmación sigue las reglas de cualquier escritura: pasado el punto sin retorno de la confirmación, la carga queda confirmada aunque después haya un pánico o el proceso termine, y la recuperación la conserva cuando ya era durable (G1, G2, G5; E2 y E4 del diario por grupos). Admite una tabla con filas, y desde 7.2 con índices, y filas en cualquier orden, más rápida en el orden de la clave primaria. Al confirmar, informa de cuántas filas cargó; una secuencia vacía no cambia nada. Un hilo que ya tiene el turno de escritura no puede empezarla: falla al instante con uso incorrecto (T5); una lectura abierta en el mismo hilo no lo impide. |
| E14 | **Secuencias** (cambian las columnas autonuméricas de N4 y N6): una secuencia es un objeto con nombre del catálogo: no es un tipo de árbol, pero comparte el catálogo y el espacio de nombres con árboles y tablas, y un nombre es de una sola clase de objeto. Se abre por su nombre en los modos **Abrir**, **Crear nuevo** y **Abrir o crear** de A1; los modos que vacían no existen para ella. Al crearla se fija su primer valor, 1 si no se da; Abrir o crear sobre una secuencia que existe la abre sin mirar el primer valor dado. **Existe**, **Borrar**, **Renombrar** y **Listar** siguen las reglas de las tablas (E2, A3, A6); Existe y Listar valen para cualquier clase de objeto y nunca fallan por ella (N1). Cualquier otra operación de secuencia sobre una tabla o un árbol, o al revés, falla sin efecto con tipo distinto, también al renombrar con sobrescritura; sin sobrescritura, un destino de otra clase falla con árbol ya existente (N1). **Pedir el siguiente valor**, a través de la apertura, solo se puede en una transacción de escritura y devuelve un entero de 64 bits con signo, uno más que el anterior. Es transaccional: si la escritura aborta, sus valores vuelven a estar disponibles, y un valor de una escritura confirmada nunca vuelve a darse, aunque se borren filas, se vacíe una tabla o el proceso caiga. En el máximo de los enteros de 64 bits con signo, pedir otro valor falla sin efecto con valor no válido, nombrando la secuencia. Una apertura de una secuencia que la misma escritura borró falla con árbol no encontrado (A6, B9). Una secuencia no está atada a ninguna tabla. |
| E15 | **Generadores de `ulid` y `uuid`** (N6): son funciones públicas que la aplicación llama y cuyo valor pone en la fila; no dependen de ningún almacén. Hay tres: **`ulid`**, 48 bits de milisegundos Unix y 80 bits aleatorios; **`uuid` versión 7**, para columnas con el orden de RFC 9562, con los bits de versión y de variante de RFC 9562 y 74 bits aleatorios; y **`uuid` secuencial**, para columnas con el orden de SQL Server, con el algoritmo del usuario: 48 bits de milisegundos Unix en los bytes que SQL Server compara primero y 80 bits aleatorios en los demás, en la disposición de su generador de .NET. En el orden de bytes de .NET (el de `Guid.ToByteArray`, que usa `SqlGuid`): bytes 10–15, el milisegundo en big-endian; bytes 8–9, los 16 bits altos de la parte aleatoria en big-endian; y bytes 6–7, 4–5 y 0–3, sus 64 bits bajos de más a menos significativos, cada grupo en big-endian. Así SQL Server, que compara los grupos 10–15, 8–9, 6–7, 4–5 y 0–3, los ordena por el milisegundo y después por los 80 bits. Krummi lo da en el orden de RFC 9562 (E7), que invierte los grupos 0–3, 4–5 y 6–7 respecto al de .NET. Los bits aleatorios se sortean en cada milisegundo nuevo, y no se pueden predecir a partir de otros valores generados (documento de diseño 11, P20). Cada generador es estrictamente creciente en el proceso, también entre hilos, con tres reglas: (1) en un milisegundo posterior al último usado, sortea bits nuevos; (2) en el mismo milisegundo, o si el reloj da uno anterior (el reloj retrocedió), sigue con el último milisegundo usado y suma 1 a su parte aleatoria, tomada como un solo número, con acarreo; (3) si esa suma desborda la parte aleatoria, usa el milisegundo siguiente al último usado y sortea bits nuevos. En el `uuid` versión 7 la parte aleatoria son sus 74 bits aleatorios: los 4 de versión y los 2 de variante nunca cambian, y el acarreo los salta. Un milisegundo que no cabe en 48 bits (después del año 10889) falla con valor no válido. Si el sistema falla al dar los bits aleatorios, la función falla con entrada y salida. No hay promesa de orden entre procesos ni entre reinicios. |
| E16 | **Cambios y referencias** (A10, T8, T10, B9): son cambios para T10 crear una tabla o una secuencia, vaciar una tabla, borrar una que existe, renombrar a otro nombre, un cambio de esquema que da versión nueva, insertar, actualizar o insertar o actualizar cuando no es idéntico (E10), borrar una fila que estaba, borrar un intervalo con filas, una carga masiva con filas y pedir un valor de una secuencia. No lo son: Abrir o crear sobre un objeto que existe, Borrar de uno que no existe, renombrar a su mismo nombre, un cambio de esquema que no cambia nada, actualizar sin encontrar la fila, una actualización idéntica y toda operación que falla por un error de validación pública. Como en A10, terminan las referencias anteriores, aunque al final no cambien nada o fallen, todas las operaciones que pueden modificar: los modos distintos de Abrir, Borrar, Renombrar, los cuatro cambios de esquema, insertar, actualizar, insertar o actualizar, borrar una fila, borrar un intervalo, crear una secuencia y pedirle un valor. Abrir, Existe, Describir, Listar, Leer, recorrer y el número de filas no las terminan. |
| E17 | **Errores: datos y precedencia** (N10, B7): clave duplicada lleva la tabla y la clave primaria (desde 7.2, también el índice); esquema no válido y esquema distinto, los de E5 y E6; valor no válido, la tabla y la columna cuando las hay, la secuencia cuando es ella, y la regla incumplida, una de estas, en este orden: columna repetida, columna de la clave que falta, nulo no admitido, tipo distinto del de la columna, texto mal formado, fuera del dominio, precisión de un `decimal`, dimensión de un vector, NaN o infinito en un vector, vector cero con métrica coseno, intervalo no válido, secuencia agotada o milisegundo fuera de 48 bits. Si una fila incumple varias, se informa una sola, en tres pasos: (1) si alguna columna se da más de una vez, la primera que se repite en el orden en que se dieron, con columna repetida; (2) si no, si falta alguna columna de la clave, la primera en el orden de la clave, con columna de la clave que falta; (3) si no, la primera columna en el orden del esquema que incumple alguna de las demás reglas, con la primera que incumple en el orden de esta lista. La precedencia amplía A7 y B7: primero uso incorrecto (T5, que falla al instante) y el tiempo de espera agotado de una carga masiva (E13); después el tamaño no válido de lo que se comprueba sin leer el almacén (los nombres; en Renombrar, el de origen antes que el de destino); después una operación a través de una apertura de un objeto borrado, que falla con árbol no encontrado sin leer nada más; después los fallos al leer el catálogo, tipo distinto, árbol no encontrado y árbol ya existente; después esquema no válido y, tras él, esquema distinto; después valor no válido y, tras él, el tamaño no válido de lo que necesita el esquema (una clave primaria o una fila más grandes que su máximo); después los fallos al leer las páginas de la tabla o de la secuencia; y por último clave duplicada. Dos excepciones, porque necesitan leer páginas: el tamaño de la fila que resultaría de actualizar o de insertar o actualizar una fila que existe se comprueba después de leerla, y una secuencia agotada se detecta después de leer su página. |
| E18 | **Límites**: una fila mide como mucho 256 MiB, el máximo de un valor (B6), y la clave primaria, 2.048 bytes (B5), los dos medidos en la forma en que Krummi los guarda, que fijan los documentos de diseño 1 (filas) y 3 (claves); un nombre de columna, de 1 a 1.024 bytes (B4), también el que se da en una fila o en un cambio de esquema, que se comprueba con los nombres, antes que el catálogo (E17). Pasarse falla sin efecto con tamaño no válido, con los datos de B8 (el tamaño recibido, el mínimo y el máximo). |
| E19 | **Daño** (A9, con tablas y secuencias): los modos Abrir, Crear nuevo y Abrir o crear, Existe, Describir, Listar, Renombrar sin sobrescritura o a su mismo nombre y el número de filas nunca fallan con daño en las páginas del propio objeto; leer, guardar, borrar, recorrer, cargar y pedir un valor fallan con daño en una página que necesitan; borrar, vaciar y renombrar con sobrescritura pueden fallar con daño en cualquier página del objeto afectado. En una escritura, todo fallo por daño sigue T11. |
| E20 | **Formato**: el cambio es incompatible (B12), así que la versión del formato pasa a 11. El formato de las filas reserva lo que necesitará la compresión por diccionario (N9); lo comprueba la revisión del diseño. La información del almacén (P12) no cambia de forma: las páginas de las tablas y de las secuencias ya cuentan en sus totales. |
| E21 | **Cargas del banco** (N11, K7): las siete cargas de *Cargas del banco* (abajo), frente a las tablas de Voron, con su umbral de K7; los demás motores dicen `no aplica`. El informe cuenta 29 umbrales. |

*Anotaciones del 2026-11-11, tras la revisión del código de Codex (decisiones del usuario):*

- *E5: se añade un caso al final de su lista: un esquema que, guardado con los nombres y los valores por defecto de sus columnas, pasa de 256 MiB, el máximo de una entrada del catálogo, falla sin efecto con esquema no válido y la regla «esquema demasiado grande», nombrando la columna con la que lo pasa, también al añadir una columna o al renombrarla a un nombre más largo (la columna del cambio). Usuario, ante un valor por defecto de casi 256 MiB que cabía en una fila pero no en el catálogo: «Aceptar el límite».*
- *E8: el dominio se comprueba sobre el valor ya redondeado al microsegundo, que es el que se guarda: un valor dado fuera de su dominio que el redondeo mete dentro se acepta (una hora de −500 ns se guarda como 00:00:00), y uno que el redondeo saca falla con valor no válido. Usuario: «Se acepta».*

*Anotación del 2026-11-11 desde las lecturas de filas ([tables-row-reads.prd.md](tables-row-reads.prd.md)): E19: Las lecturas por la clave y los recorridos ya no comprueban el contenido de la fila (sus valores, sus desplazamientos, sus bits de nulo y su clave); una fila mal formada en una página cuya integridad pasó G6 se devuelve sin error, y leer una de sus columnas puede devolver un valor que no es el escrito o terminar con un pánico; un texto que no es UTF-8 leído como texto termina siempre con un pánico. Siguen fallando con daño la versión de esquema desconocida o ausente del historial, la cabecera que Krummi no escribe y la fila que no llega al final de su parte fija; actualizar e insertar o actualizar siguen comprobando la fila que reemplazan (decisiones L1–L3 de [tables-row-reads.prd.md](tables-row-reads.prd.md)).*

*Anotaciones del 2026-11-15 desde las proyecciones ([tables-projection.prd.md](tables-projection.prd.md)):*

- *E6: también fallan con esquema distinto preparar una proyección con un nombre que no es de una columna viva (con la
  tabla y la columna), obtener una columna preparada con un tipo de otra clase (con la tabla, la columna y la
  diferencia) y ligar a una proyección una fila de otra tabla, de otro almacén o de otro esquema (con la tabla) (PY1–PY3
  de [tables-projection.prd.md](tables-projection.prd.md)).*
- *E17: la lista de reglas de valor no válido añade al final proyección sin columnas, y la precedencia vale para
  preparar una proyección como dice PY1 de [tables-projection.prd.md](tables-projection.prd.md).*
- *E19: preparar una proyección, obtener sus columnas preparadas, ligarle una fila y leer con ella no fallan con daño:
  los daños de la fila y del historial aparecen en la lectura o en el paso del recorrido que la devuelve (PY3 de
  [tables-projection.prd.md](tables-projection.prd.md)).*


#### Cargas del banco

Todas usan una tabla con clave primaria `id` y las columnas `n` (`i64`), `x` (`f64`), `ts` (`timestamp`), `flag`
(`bool`) y `name` (`string` de 75 caracteres ASCII), unos 100 bytes lógicos por fila, como los valores de K4. La semilla
fija de K4 genera todos los valores: `n` uniforme entre todos los `i64`; `x` uniforme en [0, 1), nunca NaN, infinito ni
`-0`; `ts` uniforme entre 2000-01-01T00:00:00Z y 2030-01-01T00:00:00Z; `flag` al azar; `name`, letras y dígitos ASCII;
y las claves, con la disposición de E15 pero con bits de la semilla, no con los generadores públicos, para que todos
los motores y repeticiones reciban los mismos bytes (K4). Voron usa su API `Table` con un `TableSchema` equivalente,
clave primaria sobre `id`, los mismos datos, en el mismo orden y con la misma durabilidad. La generación de los datos
queda fuera del tiempo medido en los dos motores. Cada carga parte de su estado inicial, sin heredar cambios de otra.

| ID | Nombre | Empieza en | Qué hace | Operaciones | Reducida |
|---|---|---|---|---|---|
| W23 | `tabla-insercion-en-orden` | Almacén vacío | Inserta 1.000.000 de filas con claves `ulid` en orden, en escrituras de 1.000. | 1.000.000 | 10.000 filas |
| W24 | `tabla-insercion-al-azar` | Almacén vacío | Inserta 1.000.000 de filas con claves `uuid` al azar, en el orden de RFC 9562, en escrituras de 1.000. | 1.000.000 | 10.000 filas |
| W25 | `tabla-carga-masiva` | Almacén vacío | Carga en masa 1.000.000 de filas con claves `ulid` en orden de clave primaria; el tiempo incluye la confirmación. En Voron, una transacción de escritura con todas las filas. | 1.000.000 | 10.000 filas |
| W26 | `tabla-lecturas` | La tabla de W23 | Lee todas las filas por su clave primaria, al azar, en una transacción de lectura, leyendo todas sus columnas. | 1.000.000 | 10.000 lecturas |
| W27 | `tabla-intervalos` | La tabla de W23 | En una transacción de lectura, 100.000 intervalos hacia delante, cada uno desde una clave existente elegida al azar, que leen hasta 100 filas con todas sus columnas (menos si llegan al final), como W11. | Las filas leídas | 1.000 intervalos |
| W28 | `tabla-actualizacion` | La tabla de W23 | Cambia `n` en todas las filas, al azar, en escrituras de 1.000: Krummi da solo `n` (E10); Voron, que no actualiza una columna sola, lee la fila y la guarda entera con el `n` nuevo. | 1.000.000 | 10.000 filas |
| W29 | `tabla-borrado` | La tabla de W23 | Borra todas las filas al azar, en escrituras de 1.000. | 1.000.000 | 10.000 filas |

W28 compara la misma operación lógica, cambiar `n` en una fila, hecha con las capacidades reales de cada motor,
aunque las operaciones físicas sean distintas. W26, W27 y W28 empiezan en el almacén que deja W23 y W29 en el que deja W23, cada una en su copia, como las cargas del
almacén base (K4). El tamaño del almacén (K9) se mide también tras W23, W24, W25 y W29, sin una compactación que no
haga el otro motor; el tamaño físico incluye la clave, el formato de las filas y sus metadatos, así que no es 100 bytes
por fila. W26 y W27 calculan en los dos motores la misma suma de comprobación de los valores de todas las columnas
leídas, dentro del tiempo medido porque es la forma de leer los valores, y al terminar la comparan con la esperada. Valen las reglas de reproducibilidad del banco (K4, K8, K13): operaciones
durables, las mismas condiciones, tres repeticiones y la mediana.

*Anotación del 2026-11-15 desde las proyecciones ([tables-projection.prd.md](tables-projection.prd.md)): W26 y W27:
Krummi lee las columnas de las filas con una proyección, y preparar la proyección cuenta en el tiempo medido de cada
repetición; también lo harán las cargas de tablas futuras que lean valores de columnas, salvo que su PRD diga otra
cosa. Las cargas, sus sumas de comprobación y sus umbrales no cambian (PY7).*

*Anotaciones del 2026-11-18 desde los índices secundarios ([tables-indexes.prd.md](tables-indexes.prd.md)):*

- *E2: Describir devuelve además los índices de la tabla y el estado de los índices (decisión IX6).*
- *E5: crear un índice sigue las reglas de definición de IX3; el esquema demasiado grande cuenta también las definiciones de los índices, así que añadir o renombrar una columna puede fallar con esa regla en una tabla con índices; y la regla «quitar una columna de la clave primaria o, desde 7.2, de un índice» nombra además el índice cuando la columna no es de la clave primaria (decisiones IX3 e IX5).*
- *E6: esquema distinto cubre también crear si no existe un índice con otra definición, con la primera diferencia, y recorrer un índice que no existe (decisiones IX2 e IX9).*
- *E16: crear un índice y borrar uno que existe son cambios para T10, y los dos terminan las referencias anteriores (decisión IX13).*
- *E17: clave duplicada en un índice único lleva además el índice y los valores de su clave que se repiten; tamaño no válido distingue también una entrada de índice; y los errores de los índices siguen la precedencia de IX10 (decisión IX10).*
- *E19: en una tabla con índices, borrar una fila, borrar un intervalo y crear un índice leen el contenido de las filas sin comprobarlo, y recorrer un índice no comprueba sus entradas (decisión IX12).*

*Anotaciones del 2026-11-19 desde las estadísticas ([tables-statistics.prd.md](tables-statistics.prd.md), 7.3):*

- *E3: «reinicia sus estadísticas» quiere decir que las quita, también las de memoria (ST8).*
- *E16: analizar en una escritura es un cambio para T10 cuando guarda y termina las referencias aunque falle; estimar
  y leer las estadísticas no son cambios ni terminan referencias (ST2, ST7, ST16).*
- *E17: los errores y la precedencia de estimar, leer y analizar los fija ST12.*
- *E20: la versión del formato pasa a 13 (ST13).*

### Estado de partida

- No hay tablas ni secuencias en el código: el catálogo guarda árboles normales, multivalor y de tamaño fijo, cada
  entrada con su identidad, su tipo, su raíz y su número de entradas, y un tipo desconocido es daño. Las identidades
  de los árboles son un contador.
- La API pública tiene los modos de apertura de A1 y las operaciones de A3 para árboles; Listar devuelve el tipo de
  árbol de cada nombre, así que listar tablas y secuencias necesita una clase de objeto del catálogo.
- El banco tiene 22 cargas (W1–W22) y ningún motor mide tablas; el arnés de Voron no usa todavía su API `Table`.
- La información del almacén (P12 del pager, F10) no cuenta árboles, solo páginas.
- La versión del formato es la 10 (B12).
- No hay generadores de `ulid` ni `uuid`, ni una función que lea el generador aleatorio del sistema operativo.

## Actores

- **Desarrollador de la aplicación (Rust)**: define tablas, guarda y lee filas, pide valores a los generadores y a las
  secuencias.
- **Quien ejecuta el banco**: compara las cargas W23–W29 con Voron.

## Flujo

1. La aplicación abre una escritura y crea la tabla con su esquema, o la abre con Abrir o crear y el esquema que
   conoce.
2. Pide un `ulid` o un valor de una secuencia, lo pone en la fila e inserta; actualiza, lee, recorre o borra por la
   clave primaria.
3. Para cargar muchas filas, llama a la carga masiva con el nombre de la tabla, fuera de toda escritura.
4. Para cambiar el esquema, pide el cambio en una escritura; las filas guardadas se leen con el esquema nuevo.

## Validaciones y mensajes

Valen las de los ascendientes. Este PRD añade, solo en la biblioteca:

| Situación | Clase de error |
|---|---|
| Abrir, vaciar, describir o cargar una tabla que no existe; renombrar desde un objeto que no existe; usar la apertura de un objeto que la escritura borró | Árbol no encontrado (A6) |
| Crear nuevo sobre un objeto de la misma clase que existe; renombrar sin sobrescritura a un nombre que existe, de cualquier clase | Árbol ya existente (A6) |
| Una operación de tabla o de secuencia, salvo Existe y Listar, sobre otra clase de objeto del catálogo, incluidos Crear nuevo y renombrar con sobrescritura sobre ella | Tipo distinto (B3, N1) |
| Un esquema que incumple una regla de E5, al crear o al cambiarlo | Esquema no válido (nueva) |
| Abrir con un esquema no compatible; guardar una fila con una columna que la tabla no tiene | Esquema distinto (N10) |
| Un valor que incumple una regla de E17 (de otro tipo, fuera de su dominio también tras redondearlo, un nulo o una columna sin valor que no admite nulos, una columna de la clave que falta o una repetida, un texto de `ulid` o `uuid` mal formado, un intervalo mal formado, una secuencia agotada) | Valor no válido (N10) |
| Insertar una clave primaria que ya existe | Clave duplicada (N10) |
| Un nombre de columna, una clave primaria o una fila más grandes que su máximo; un límite de intervalo que no cabe en la clave | Tamaño no válido (B7) |
| Empezar una carga masiva desde un hilo que ya tiene el turno de escritura | Uso incorrecto (T5) |
| Una carga masiva que no obtiene su turno antes de su tiempo límite | Tiempo de espera agotado (T4) |
| El sistema operativo no da bits aleatorios a un generador | Entrada y salida (D8) |

Actualizar una fila que no existe, leerla, borrarla o recorrer un intervalo vacío no son errores (E10–E12).

## Salida

El módulo no escribe nada en pantalla. Leer y recorrer devuelven filas con la forma de E11 y los valores con la
normalización de N4 y E7, como dice la *Salida* del padre. Describir y Listar devuelven copias propias de la
aplicación (E2). Borrar devuelve si la fila estaba; borrar un intervalo, cuántas filas borró; actualizar, si encontró la
fila; insertar o actualizar, si insertó o actualizó; la carga masiva, cuántas filas cargó; pedir un valor a una
secuencia, el valor. El informe del banco añade las
filas W23–W29 con sus umbrales, con `no aplica` en los motores que no son Voron, y su tabla de tamaños añade W23, W24,
W25 y W29.

## Reutilización

- El catálogo de árboles, sus modos, sus operaciones, sus clases de error, su precedencia y su regla de daño (A1–A10,
  B3–B12), extendidos a tablas y secuencias.
- Los árboles B+ y sus recorridos (5.1–5.4) como almacenamiento de las filas.
- Las transacciones, el diario y la recuperación (G1, G5, T4, T5, T8, T10–T12).
- Ninguna función que lea el generador aleatorio del sistema operativo: 7.1 la añade al pager (documento de diseño
  11, P20) y la comparten los generadores y las identidades del catálogo.
- El banco comparativo, su semilla, sus reglas y su informe (módulo 6).

## Fuera de alcance

- Índices secundarios (7.2), estadísticas (7.3) y columnas JSON (7.5), salvo lo que E3, E5 y E17 dicen de ellos.
- Insertar o ignorar, y cualquier otra variante de insertar que no falle con una clave duplicada.
- Una operación para cambiar la clave primaria de una fila (E10).
- Leer solo algunas columnas de una fila (proyecciones): módulo 17. *Anotación del 2026-11-15: las proyecciones de
  columnas entran antes, en [tables-projection.prd.md](tables-projection.prd.md): una lista de columnas preparada una
  vez por esquema, guardada en el almacén, con la que se leen las filas; las consultas del módulo 17 fijarán si las
  usan.*
- Columnas generadas o autonuméricas, valores por defecto calculados (como la hora actual), generadores como valor por
  defecto, y valores por defecto en la clave primaria (simplificación de esta versión, E4); cambiar el valor por
  defecto de una columna (como el padre).
- En las secuencias: leer su valor actual sin consumirlo, reiniciarlas o fijar su valor después de crearlas, otro paso
  que 1, secuencias descendentes, cíclicas o con caché de valores.
- Columnas de la clave primaria descendentes (los índices de 7.2 sí las tienen).
- Intervalos con límites sobre más de una columna y filtros: módulo 17.
- Fechas y horas fuera de los dominios de E8.
- Promesas de orden de los generadores entre procesos o entre reinicios.
- La interfaz C y el binding C# (módulo 18); la prueba del orden de SQL Server usa C# solo para generar sus vectores.

## Dejado a la implementación

- Cómo se guardan las tablas y las secuencias en el catálogo y en árboles, con el punto de partida de los documentos de
  diseño; las formas de la fila y de la clave que miden los límites de E18 son las de los documentos 1 y 3.
- Los nombres de las operaciones, los tipos y las clases de error, en inglés y con las convenciones de Rust (D8).
- Cómo se construyen los ganchos de prueba de AC4, AC9, AC12 y AC13: son mecanismos de prueba y de diagnóstico, no
  contratos públicos; los criterios fijan lo que demuestran.
- El valor del primer milisegundo de un proceso para los generadores y cómo comparten la secuencia los hilos.

## Criterios de aceptación

Valen los de los ascendientes. Este PRD cumple AC-N1 a AC-N6, AC-N9 y AC-N10 del padre con lo que estos criterios
fijan.

1. **AC1** — Dados los modos de A1 sobre tablas, cuando se abre, se crea nuevo, se abre o crea, se crea o vacía y se
   vacía una tabla que existe y una que no, entonces cada modo hace lo de E1, y en una lectura solo existe Abrir;
   cuando se abre o crea con un esquema compatible al que le faltan columnas que la tabla tiene, entonces la abre y
   sus filas se leen con todas sus columnas; cuando se abre o crea, o se crea o vacía, con un esquema que difiere en
   el tipo, la nulabilidad o el valor por defecto de una columna, nombra una columna que no existe o cambia la clave
   primaria, entonces falla sin efecto con esquema distinto y sus datos; y cuando se abre o crea con cualquier esquema
   un nombre que es de un árbol o de una secuencia, entonces falla con tipo distinto.
2. **AC2** — Dadas una tabla con filas y otra tabla, cuando se renombra una a la otra sin sobrescritura, entonces
   falla con árbol ya existente; con sobrescritura, la de destino desaparece con sus filas y la renombrada conserva
   sus filas, su identidad y sus aperturas; Describir devuelve el esquema de E2 en una lectura y en una escritura, la
   escritura con sus propios cambios, con la misma identidad de una columna antes y después de renombrarla y con otro
   estado del esquema después de cada cambio que da versión nueva; Describir un nombre que no existe falla con árbol
   no encontrado, y el de un árbol o una secuencia, con tipo distinto; Existe dice que existen los nombres de las tres
   clases; y Listar muestra las tablas, los árboles y las secuencias con su clase,
   en el orden de B5.
3. **AC3** — Dada una tabla con filas y otra apertura de ella, cuando se vacía y se confirma, entonces no tiene filas,
   conserva su esquema y su identidad, la otra apertura la ve vacía, y una secuencia que usaba no cambia.
4. **AC4** — Dado cada esquema no válido de E5, cuando se crea la tabla con él o se pide el cambio que lo produciría,
   entonces falla sin efecto con esquema no válido, con la tabla, la columna y la regla, y la transacción puede
   confirmar sus otros cambios; cuando una tabla creada en la versión 1 recibe 65.534 cambios y se pide otro, entonces
   falla sin efecto con esquema no válido; un cambio que no cambia nada no da versión nueva; otra apertura de la
   tabla en la misma escritura ve el esquema nuevo; y cada cambio de esquema escribe las mismas páginas con 10 que
   con 100.000 filas, según su gancho de pruebas (AC-N2, AC-N6).
5. **AC5** — Dada una tabla con valores por defecto, cuando se inserta una fila sin esas columnas, entonces se leen
   sus valores por defecto, y nulo en las que no tienen; con un nulo explícito, nulo; y sin valor ni nulo en una
   columna que no los admite, falla con valor no válido; cuando se actualiza una fila dando solo algunas columnas,
   entonces las demás conservan su valor, también una columna añadida después de que la aplicación definió su
   esquema; cuando se guarda una fila con una columna que la tabla no tiene, entonces falla con esquema distinto; y
   cuando le falta una columna de la clave o repite una columna, con valor no válido.
6. **AC6** — Dados valores en los extremos de cada dominio de E8 y uno más allá de cada extremo, cuando se guardan,
   entonces los de dentro se leen iguales y los de fuera fallan con valor no válido; dados un `decimal` con más
   decimales que su escala en cada caso de empate y de signo, y un instante con fracción de 100 ns, cuando se guardan,
   entonces se leen redondeados como dice E8; 0001-01-01T00:00:00.0000005Z se lee 0001-01-01T00:00:00.000001Z; y
   99,995 en un `decimal(4, 2)` y 9999-12-31T23:59:59.9999995Z fallan con valor no válido.
7. **AC7** — Dados vectores `bit` con un solo bit puesto en cada posición de un byte, un `f16` conocido, un vector con
   `-0` y `string` con U+0000 y vacío, cuando se guardan y se leen, entonces los bits están donde dice E7, el `-0` se
   lee `+0` y los textos se leen iguales; y el texto vacío y los bytes vacíos sirven como clave primaria.
8. **AC8** — Dados `ulid` y `uuid` de cada generador, cuando se convierten a texto y se vuelven a leer, entonces dan
   el mismo valor en la forma de E9; y cada forma no admitida de E9 falla con valor no válido.
9. **AC9** — Dada una tabla con una clave primaria de dos columnas, cuando se recorren intervalos con cada prefijo y
   cada combinación de límites incluidos, excluidos y ausentes, hacia delante y hacia atrás, entonces vuelven las filas
   de E12 (AC-N1); un intervalo invertido no devuelve ninguna; cada intervalo mal formado de E12 falla con valor no
   válido, y un límite que no cabe en la clave, con tamaño no válido aunque el intervalo esté vacío; cuando se borra
   un intervalo, entonces devuelve cuántas filas borró y quedan las de fuera (AC-N3); leer una fila que no existe
   devuelve que no está, y borrarla devuelve falso; y pedir el número de filas lee las mismas páginas con 10 que con
   100.000 filas, según su gancho de pruebas (AC-N3).
10. **AC10** — Dada una tabla con filas, cuando una carga masiva tiene, antes de confirmar, una fila con clave
    duplicada, una no válida, una que falla siendo la última, una fuente que falla, una cancelación o un pánico,
    entonces la tabla, el número de filas y las secuencias quedan como antes y otra escritura obtiene después su
    turno (7.2 añade el caso de una tabla con índices); cuando
    se cargan 100.000 filas fuera de orden, entonces la tabla tiene las mismas filas que si se hubieran insertado una a
    una e informa de 100.000; una carga vacía no cambia nada; cuando un hilo con el turno de escritura empieza una
    carga, falla con uso incorrecto; y una carga que no obtiene su turno antes de su tiempo límite falla con tiempo de
    espera agotado (AC-N3).
11. **AC11** — Dada una secuencia, cuando una escritura pide valores y se confirma, otra pide valores y aborta, se
    vacía una tabla, se borran filas y se simula una caída del proceso y se reabre, entonces los valores siguientes
    continúan desde el último confirmado y ninguno repite uno confirmado; una secuencia creada con primer valor 1.000
    da 1.000, y Abrir o crear con otro primer valor no la cambia; una en el máximo falla sin efecto con valor no
    válido; pedir un valor en una lectura no es posible; las operaciones de secuencia sobre una tabla o un árbol, y al
    revés, fallan con tipo distinto; y renombrar sin sobrescritura una tabla al nombre de una secuencia falla con árbol
    ya existente (AC-N4, AC-N5).
12. **AC12** — Dados 100.000 valores de cada generador de E15 desde un solo hilo, con el reloj de prueba que se
    detiene, retrocede y avanza, y con la parte aleatoria de prueba cerca de agotarse dentro de un milisegundo, cuando
    se ordenan con el orden de su columna (el de SQL Server, comprobado con `SqlGuid`, para el `uuid` secuencial),
    entonces quedan en el orden en que se generaron; cuando varios hilos generan a la vez, entonces todos los valores
    son distintos, los de cada hilo crecen, y un valor generado después de que otro hilo terminó los suyos es mayor que
    todos ellos; dado un fallo simulado del generador del sistema, la función falla con entrada y salida; y dados
    vectores de prueba guardados en el repositorio, cada uno con un milisegundo y unos bits aleatorios fijos y los
    bytes esperados, cuando cada generador los produce con el reloj y los bits de prueba, entonces da exactamente esos
    bytes: los del `uuid` secuencial, generados una vez con la disposición del generador .NET del usuario y ordenados
    con `SqlGuid`, también en el orden de los vectores; los del `uuid` versión 7, con sus bits de versión y de variante
    intactos tras un acarreo que los cruza; y los de cada regla de retroceso y de desbordamiento; y con el reloj de
    prueba en el último milisegundo de 48 bits y el incremento agotado, la función falla con valor no válido (AC-N4).
13. **AC13** — Dadas las operaciones de E16, cuando se ejecutan en una escritura que no hace nada más, entonces son
    cambios para T10 exactamente las que E16 dice; una referencia anterior a cualquier operación que puede modificar
    deja de valer; y listar el catálogo no muestra ningún árbol interno de una tabla, que ningún nombre alcanza como
    árbol según su gancho de pruebas (AC-N5).
14. **AC14** — Dados dos niveles consecutivos de la precedencia de E17, y sus dos excepciones, cuando una operación
    provoca a la vez un fallo de cada uno, entonces gana el que dice E17; cuando una fila o un esquema incumplen dos
    reglas, en dos columnas o en la misma, entonces se informa la que dicen E5 y E17: por ejemplo, con el esquema
    `id, a, b`, `a` y `b` sin nulos, la fila `[(b, 2), (b, 3), (id, 1), (a, nulo)]` da columna repetida en `b`, la
    fila `[(n, nulo), (n, 5)]` con `n` sin nulos da columna repetida en `n`, y una fila sin `id` y con `a` nula da
    columna de la clave que falta en `id`; y los errores llevan los datos de E17.
15. **AC15** — Dados una clave primaria de exactamente 2.048 bytes y otra de 2.049, una fila de exactamente 256 MiB y
    otra de un byte más, en la forma de E18, y nombres de columna de 0, 1, 1.024 y 1.025 bytes, cuando se usan,
    entonces los que están dentro funcionan y los que no fallan sin efecto con tamaño no válido y sus datos.
16. **AC16** — Dadas las cargas W23–W29, cuando se ejecuta el banco completo, entonces el informe muestra 29
    umbrales, `no aplica` en los motores que no son Voron, la tabla de tamaños las cuatro cargas de E21, y las sumas
    de comprobación de W26 y W27 coinciden en los dos motores; y la ejecución reducida usa las cantidades de su
    columna (AC-N9). El informe de cierre incluye las mediciones de una sola carga de AC-N10.
17. **AC17** — Dados vectores de prueba del orden de SQL Server guardados en el repositorio, generados una vez con
    `SqlGuid` de .NET como referencia independiente de Krummi, cuando se ordenan las filas de una columna con ese orden,
    entonces quedan en el orden de los vectores (AC-N1). Prueban la semántica de .NET, no resultados de una instancia
    de SQL Server.
18. **AC18** — Dado un almacén con tablas, cuando lo abre un motor con el formato 10, entonces falla con la versión
    del formato (B12); y la información del almacén no cambia de forma.
19. **AC19** — Dada una tabla y una secuencia con una página propia dañada, cuando se piden Abrir, Crear nuevo, Abrir
    o crear, Existe, Describir, Listar, Renombrar sin sobrescritura o a su mismo nombre y el número de filas, entonces
    ninguna falla con daño; y leer, guardar, recorrer o pedir un valor que necesitan esa página fallan con la clase de
    daño y su número de página (E19).

## Anotaciones a otros PRD

Este PRD cambia reglas de documentos anteriores, que llevan la anotación del 2026-11-10 (en sus traducciones
también):

- [tables.prd.md](tables.prd.md): N1 y AC-N5 (tres clases de objeto en el catálogo, con las secuencias); N4 (no hay
  columnas autonuméricas); N6 (actualizar cambia solo las columnas que se dan, E10; secuencias en lugar de columnas
  autonuméricas, E14; generadores como funciones, E15); N10 (cuarta clase, esquema no válido, E5); AC-N3 (la segunda
  inserción o actualización actualiza las columnas dadas; las secuencias quedan como antes de una carga fallida, en
  lugar del contador autonumérico, y desaparecen los casos de clave generada que choca y de contador agotado); AC-N4
  (las secuencias en lugar de la columna autonumérica); AC-N7 (quitar una columna de un índice falla con esquema no
  válido); *Fuera de alcance*: se quitan «Actualizar parte de una fila: actualizar reemplaza la fila entera (N6)» y
  «Reiniciar o fijar el contador de una columna autonumérica», que pasa a reiniciar o fijar una secuencia después de
  crearla.
- [trees-keys.prd.md](../trees/trees-keys.prd.md): A1, A3, A6, A9 y A10 valen también para las secuencias (E14, E19, E16), y
  las clases de A6 nombran una tabla o una secuencia; A7 (la precedencia de E17: uso incorrecto y el tiempo de espera
  de una carga masiva van primero, y el tamaño no válido que necesita el esquema va después de los fallos del
  catálogo, del esquema y de valor no válido).
- [trees.prd.md](../trees/trees.prd.md): B3 (las secuencias comparten el catálogo como las tablas); B7 (la misma precedencia
  de E17).
- [benchmark.prd.md](../benchmark/benchmark.prd.md): la tabla de cargas añade W23–W29; K4 (W26, W27, W28 y W29 empiezan en el
  almacén de W23, cada una en su copia); K7 y *Salida* (29 umbrales); K9 (W23, W24, W25 y W29).
- Documentos de diseño de las tablas (no son PRD): el documento 1 y el 11 (P7) siguen los dominios de E8; el 2 y el
  README cambian las columnas autonuméricas por secuencias; el 3 genera los vectores del orden de SQL Server con
  `SqlGuid`, no de una instancia de SQL Server.

## Referencias

- Padre: [tables.prd.md](tables.prd.md); ascendientes: [krummi.prd.md](../krummi.prd.md).
- Reglas reutilizadas: [trees.prd.md](../trees/trees.prd.md), [trees-keys.prd.md](../trees/trees-keys.prd.md),
  [trees-scan.prd.md](../trees/trees-scan.prd.md), [transactions.prd.md](../transactions/transactions.prd.md),
  [benchmark.prd.md](../benchmark/benchmark.prd.md), [pager.prd.md](../pager/pager.prd.md).
- Diseño: `docs/design/tables/` (README, documentos 01, 02, 03 y 11).
