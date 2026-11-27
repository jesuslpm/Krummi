# Estadísticas de las tablas — PRD

**Versión**: 1.0
**Fecha**: 2026-11-19
**Documento padre**: [Tablas de Krummi — PRD](tables.prd.md) (v1.0)

## Objetivo y contexto

Construir el PRD hijo **7.3** del [reparto del módulo de tablas](tables.prd.md#reparto-en-prd-hijos): las estadísticas
de las columnas de cada tabla, el contador de modificaciones que dice cuándo están viejas, la operación pública de
**estimación** que las usa, su construcción en segundo plano y la operación **analizar**, que las construye a petición
(N8 del padre), y su medición (N11).

**Por qué ahora**: 7.1 y 7.2 están cerrados, y N2 del padre construye los PRD hijos en orden. El planificador de las
consultas (módulo 17) estima con estas estadísticas cuántas filas cumple cada condición para elegir entre recorrer la
tabla o un índice.

El diseño técnico ya está decidido en el [documento de diseño 3](../../../design/tables/03-keys-and-indexes.md)
(*Statistics*: qué se guarda por columna, el contador persistente, la construcción desde una instantánea, la regla de
que unas estadísticas nunca reemplazan a otras más nuevas y el orden de los bloqueos, con las rondas de revisión H7, H8
y H18), que es el estado de partida técnico de este PRD, con el [documento 4](../../../design/tables/04-query-tree-and-planner.md)
(cómo las usa el planificador) y el [documento 10](../../../design/tables/10-agent-tooling.md) (C2 y C10). Este PRD
cambia una decisión del documento 3: sin estadísticas, la estimación no responde con una estimación por defecto, sino
que muestrea en ese momento (ST3).

Decisiones del usuario del 2026-11-19, sobre las propuestas del borrador: leer las estadísticas es público, con la
«Lectura completa» (ST16); no hay una opción para desactivar las estadísticas en segundo plano («No, fuera de alcance»);
sin estadísticas no se responde con una suposición: «si no espera al muestreo entonces tiene que usar "suposiciones"
(guesses) y es lo que no quiero», así que la estimación muestrea (ST3); y con estadísticas viejas, «La a»: se usan y se
renuevan en segundo plano (ST2). Para las condiciones de varias columnas, guardar una muestra de filas «me parece muy
costoso»; se adoptan las estadísticas de varias columnas, las de los prefijos de los índices en 7.3 y las declaradas
después («Vale»), y, cuando no las hay, una combinación que prefiera sobrestimar («prefiero sobrestimación.
Generalmente subestimar es peor que sobrestimar», pero «No es cuestión tampoco de ponerse en el peor caso min(s1,
s2)»), el retroceso exponencial «sesgado a sobrestimar» (ST18). Unas estadísticas de menos de 500 filas se vuelven a
muestrear cuando la tabla tiene más del doble («a», ST2).

Este PRD hereda todo lo de sus ascendientes. Precisa o amplía reglas de su padre, de
[tables-schema.prd.md](tables-schema.prd.md) (7.1) y de las transacciones, que llevan su anotación (*Anotaciones a otros
PRD*).

En este documento, las **estadísticas** de una columna son el resumen de sus valores que se construye leyendo filas de
una instantánea; la **instantánea de origen** es la de la transacción de la que se construyeron; y el **contador de
modificaciones** de una tabla es el número de filas que sus escrituras confirmadas han insertado, cambiado o quitado
desde que se creó (ST4).

### Decisiones de producto

| ID | Decisión |
|---|---|
| ST1 | **Qué son y cuáles ve una transacción**: cada columna de cada tabla, tenga índice o no, salvo las columnas vector, puede tener estadísticas: un resumen de sus valores en su instantánea de origen (cuántas filas tenía la tabla, qué fracción tenía la columna nula, qué valores son los más frecuentes y cómo se reparten los demás) con el contador de modificaciones de esa instantánea (ST4). Una tabla de hasta 30.000 filas se lee entera; una mayor, por una muestra. Los **valores grandes**, los de `string` o `bytes` de más de 2.048 bytes (el máximo de una clave, B5, así que nunca están en la clave de un índice), no se leen: el muestreo solo cuenta que la fila tiene uno, y las estadísticas guardan su fracción, fuera de los valores frecuentes y del histograma. Una fila guardada fuera de su hoja, en un tramo de páginas seguidas (B6), se muestrea leyendo solo su primera página, con la cabecera, la parte fija y los desplazamientos de las columnas variables, y las páginas de sus columnas que no son grandes, sin verificar la suma de comprobación del tramo entero, que obligaría a leerlo todo (excepción a G6 de la raíz para el muestreo; usuario: «La a) sí»); los límites se comprueban, así que un tramo dañado nunca termina en un pánico, y un desplazamiento fuera del tramo cuenta como valor grande. Una estadística construida de un tramo dañado puede ser mala, nunca un resultado de una consulta. Para estimar con precisión igualdades sobre valores grandes, la aplicación puede añadir una columna con su hash (usuario: «Si quieres condiciones de igualdad mejor añadir una columna de hash»). Las columnas vector no tienen estadísticas (documento 3). Además, cada índice con dos o más columnas en su clave tiene estadísticas de los prefijos de su clave (ST17); no hay otras estadísticas de varias columnas juntas: las que se declaran sobre columnas sin índice quedan para un PRD posterior. Las de las columnas JSON las fija 7.5. Las estadísticas **guardadas** son datos de la tabla: cada transacción ve las confirmadas en su instantánea, o las que guardó ella misma (ST7), como ve las filas (G1). Además, el almacén tiene en **memoria** las últimas que se han construido de cada columna y de cada prefijo de índice (al muestrear en una estimación, ST3; en segundo plano, ST6; o al analizar en el almacén, ST7), desde que se construyen hasta que se cierra el almacén, unas más nuevas de la misma columna las reemplazan, o la columna, el índice o la tabla dejan de existir o la tabla se vacía, en el momento en que se confirma ese cambio (ST8); las de analizar en una escritura no van a memoria. De una columna o de un prefijo, una transacción **ve** las de mayor contador de origen entre las guardadas que ve y las que hay en memoria, salvo las de memoria construidas antes de un vaciado de la tabla que esa transacción ve; cada vez que las usa ve las de ese momento, así que dos estimaciones de la misma transacción pueden usar estadísticas distintas si entre ellas se publicaron otras más nuevas. |
| ST2 | **Estimar**: en una transacción de lectura o de escritura, sobre una tabla abierta, se pide una estimación de una de estas condiciones: **igual a un valor** en una columna, **en un intervalo** de una columna, con un límite inferior y uno superior, cada uno incluido, excluido o ausente, o **igual en varias columnas** (ST18); las columnas se nombran por su nombre. Las reglas de los valores son las de un intervalo de un índice con prefijo de una o de cero columnas (IX9), con una diferencia: un valor de igualdad nulo pide las filas con la columna nula también en una columna que no admite nulos, donde estima cero, en lugar de fallar con nulo no admitido; con algún límite, las filas con la columna nula no están en el intervalo y, sin ningún límite, sí; los valores se normalizan (N4, E7) y se redondean (E8), así que `-0` estima `+0` y cualquier NaN el NaN guardado. A diferencia de IX9, los valores no tienen límite de tamaño: se acepta cualquier valor válido para su columna (E18), y ninguna estimación falla con tamaño no válido por sus valores. Los valores grandes (ST1) cuentan así: una igualdad con un valor grande estima la fracción de valores grandes, que es una cota superior; una igualdad con un valor que no es grande no los cuenta; y un intervalo con algún límite suma siempre esa fracción, porque no se conoce su lugar en el orden. La estimación devuelve las **filas estimadas**, un número real entre cero y el número de filas de la tabla que ve la transacción (N6); si alguna de las estadísticas que usó está **vieja** (ST5); y si es **conjunta**, es decir, si unas mismas estadísticas cubren todas sus columnas, como siempre en una condición de una columna, o si combina varias (ST18). **Nunca responde con una suposición**: si la transacción ve las estadísticas que necesita, las usa, estén viejas o no, y si están viejas pide además que se renueven en segundo plano (ST6), sin esperar; si le falta alguna, o ve unas cuyas filas de origen son menos de 500 y la transacción ve más del doble de filas (también unas de cero filas, cuando ve alguna), muestrea en ese momento (ST3) y usa lo que leyó. Las filas estimadas son la fracción que dan las estadísticas por el número de filas que ve la transacción, así que una escritura que estima ve sus propias filas en ese número, aunque las estadísticas no ven sus cambios. Si la transacción no ve ninguna fila, toda estimación es cero, sin muestrear ni pedir nada. Después de calcularlas, valen siempre estas **reglas exactas**: un intervalo vacío o invertido estima cero; un intervalo sin límites estima todas las filas; en una columna que no admite nulos, igual a nulo estima cero; y en una columna que es por sí sola la clave primaria o la clave de un índice único, igual a un valor no nulo estima el menor entre lo calculado y una fila. Una escritura cuyas propias modificaciones hacen viejas las estadísticas pide su renovación, aunque esta no verá esas modificaciones hasta que se confirmen. Estimar no es un cambio para T10 ni termina referencias (E16), y solo lee páginas de las filas o de los índices cuando muestrea. |
| ST3 | **Muestrear en una estimación**: la estimación lee la muestra en el hilo de quien la pide y espera a tenerla; es la única operación de la aplicación, además de analizar, que espera a un muestreo (cambia N8 del padre). Lee la instantánea confirmada que ve la transacción, como una transacción de lectura que no bloquea a nadie: la suya en una lectura y, en una escritura, la que vería una lectura nueva, sin sus cambios, que puede ser algo anterior a aquella de la que partió la escritura si una confirmación previa aún no es duradera (usuario: «a) ya que las estadísticas son estadísticas y no tienen que ser exactas ni estar siempre al día. Es el principio fundamental.») (el muestreo no pide el turno de escritura; solo el guardado posterior, una escritura corta, lo pide, ST6; una escritura de la aplicación que estima sigue reteniendo su propio turno mientras espera la muestra, como en cualquier operación suya, G4); construye las estadísticas de todas las columnas de la tabla que no son vector y de los prefijos de sus índices (ST17); las publica en memoria (ST1), con las reglas de descarte de ST6, para que las usen las demás transacciones; y pide al hilo de ST6 que las guarde, salvo en un almacén que no acepta escrituras (T12). Si en esa instantánea la tabla no existe o no es la misma, la escritura la vació después, o falta una columna o un índice que la estimación necesita (porque los creó la escritura), lee lo que ve la escritura, y lo que construye solo vale para esa escritura: no se publica ni se guarda, y se olvida al terminarla. Dos estimaciones que muestrean a la vez la misma tabla no se esperan: cada una muestrea y en memoria queda la de mayor contador de origen (con contadores iguales, cualquiera). Un fallo al leer la muestra (daño, entrada y salida, memoria agotada) hace fallar la estimación con su clase, sin cambiar nada; en una escritura sigue T11. |
| ST4 | **Contador de modificaciones**: cada tabla tiene uno, que empieza en cero al crearla. Es dato de la tabla como sus filas: cada operación suma en la vista de su escritura, así que se confirma con las filas que cuenta, una escritura abortada no deja nada y cada transacción lee el de las filas que ve (G1). Suman: insertar, una por fila; actualizar o insertar o actualizar, una por fila insertada o cambiada; borrar una fila que estaba, una; borrar un intervalo, las filas que borró; vaciar la tabla (E3), las filas que quita; y una carga masiva, las filas que inserta. Insertar con claves crecientes suma igual. No suman: una actualización idéntica (E10), actualizar sin encontrar la fila, borrar una fila que no estaba, toda operación que falla, los cambios de esquema, crear o borrar un índice, renombrar la tabla, pedir un valor de una secuencia (E14) y analizar. Nunca disminuye; sobrevive a cerrar y reabrir el almacén y a una caída (G5); renombrar la tabla lo conserva; una tabla borrada y vuelta a crear empieza en cero. No se lee directamente: la aplicación lo ve solo como el contador de origen de unas estadísticas (ST16) y a través de ST5. |
| ST5 | **Estadísticas viejas**: unas estadísticas de una columna son viejas para una transacción cuando el contador que ve esa transacción menos el contador de su instantánea de origen supera el umbral de N8, calculado con las filas que tenía la tabla en su instantánea de origen. Si la diferencia es negativa (unas en memoria construidas de una instantánea posterior a la de la transacción), no están viejas. |
| ST6 | **Trabajo en segundo plano**: cada almacén tiene como mucho un hilo propio para las estadísticas; se crea con la primera petición y termina al cerrar el almacén. Hace dos clases de trabajo: **guardar** las que muestreó una estimación (ST3) y **renovar** las viejas (ST2), es decir, muestrear, para todas las columnas y prefijos de ST3, una instantánea nueva de una transacción de lectura, que no bloquea a nadie (como toda lectura larga, retiene las páginas que ve mientras dura), publicar lo que construye en memoria (ST1) y guardarlo. Una petición es de una tabla por su identidad (E2), no por su nombre, así que sigue a la tabla si se renombra. Las peticiones esperan en una cola, como mucho una de cada clase pendiente por tabla además de la que se está atendiendo, y se atienden en orden de llegada. Guardar es una escritura corta propia que pide su turno como cualquier escritura (G4, T4), sin límite de espera, al final de la cola; no cambia filas ni el contador, y una escritura de la aplicación puede esperarla y fallar por su límite (N8). Una petición de renovar cuya tabla no existe en la instantánea que muestrea se descarta. **Descarte**: al publicar y al guardar, se descartan las estadísticas de una columna o de un prefijo si la columna o el índice ya no existen, si la tabla ya no es la misma (se borró, también si se volvió a crear con el mismo nombre), si la tabla se vació después de su instantánea de origen, o si las que ya hay (en memoria o guardadas) tienen un contador de origen mayor (N8: nunca reemplazan a unas más nuevas); con contadores iguales, las dos describen las mismas filas y puede quedar cualquiera. Ningún muestreo espera a otro. En un almacén que no acepta escrituras (T12) no se pide ningún trabajo. Guardar estadísticas ocupa páginas como cualquier escritura, así que puede dejar sin espacio a una escritura posterior de la aplicación (G7, P10). |
| ST7 | **Analizar**: construye en el hilo de quien lo pide, como una construcción en segundo plano, las estadísticas de una tabla o de una de sus columnas, y vuelve cuando están guardadas o descartadas por ST6; no devuelve nada, tampoco cuando las descarta. Hay dos formas. **Analizar en el almacén**, fuera de toda transacción, recibe el nombre de la tabla, opcionalmente el de una columna y opcionalmente un límite de espera: construye desde una instantánea nueva, lo publica en memoria (ST1) y después lo guarda con su propia escritura corta, que pide su turno como cualquier escritura (T4); el límite cuenta solo esa espera del turno, que empieza al terminar de muestrear, así que con un límite de cero y otra escritura abierta falla después de muestrear. Un hilo que tiene una escritura abierta en ese almacén recibe uso incorrecto al instante, sin muestrear (T5). Si falla después de muestrear (al esperar el turno o al guardar), lo publicado se queda en memoria, como en ST9: el almacén no cambia y la siguiente estimación que use esas estadísticas pide guardarlas (usuario: «Se queda en memoria»). Informa sus propios fallos con su clase: los de leer la muestra (daño, entrada y salida, memoria agotada) y los de guardar (disco lleno de G7, tamaño máximo de P10, memoria agotada de P13, entrada y salida, que sigue T12 y del que es la operación que lo informa si es el primero). **Analizar en una escritura**, sobre una tabla abierta en ella, con una columna opcional: construye desde lo que ve la escritura, sus cambios incluidos, con el contador de su vista, y guarda dentro de ella, sin pedir otro turno; sus fallos siguen los de cualquier operación de una escritura (T11); si la escritura se aborta, las estadísticas quedan como antes, y si cambia más filas después y confirma, esas modificaciones cuentan para ST5. En una transacción de lectura no existe la operación. Analizar una tabla sin filas guarda estadísticas de cero filas. Analizar en una escritura es un cambio para T10 cuando guarda, y termina las referencias anteriores aunque falle (E16). |
| ST8 | **Estadísticas y cambios de la tabla**: renombrar una columna las conserva (se identifican por la identidad de la columna, E2); permitir nulos en una columna las conserva; una columna añadida no tiene hasta que se construyen; quitar una columna quita las suyas en esa escritura; vaciar la tabla, con Vaciar o con Crear o vaciar, quita todas (E3) y suma al contador las filas que quita (ST4); renombrar la tabla las conserva con su contador; borrar la tabla, también al sobrescribirla al renombrar otra, las borra con ella. Crear o borrar un índice no cambia las de las columnas. Un índice nuevo no tiene estadísticas de sus prefijos hasta que se construyen, y una estimación que las necesita muestrea (ST3); borrar un índice quita las suyas en esa escritura; vaciar la tabla también las quita. |
| ST9 | **Fallos del trabajo en segundo plano**: un fallo al muestrear para renovar (daño, entrada y salida al leer, memoria agotada) descarta esa renovación sin informar a nadie; las estadísticas viejas se siguen usando y la siguiente estimación que las vea viejas la vuelve a pedir. Un fallo al guardar (disco lleno de G7, tamaño máximo de P10, memoria agotada de P13) deja el almacén como estaba y las estadísticas en memoria, que se siguen usando hasta cerrar; la siguiente estimación que use unas que solo están en memoria vuelve a pedir que se guarden. Un fallo de entrada y salida al confirmar el guardado sigue T12: el almacén deja de aceptar escrituras; no lo informa ninguna operación de la aplicación al ocurrir, sino las peticiones de escritura siguientes, que fallan al instante con la clase de entrada y salida, y el cierre, como dice T12; las estadísticas en memoria se siguen usando, y las estimaciones siguen muestreando cuando no ven ninguna (ST3). |
| ST10 | **Cierre del almacén**: cerrar detiene el muestreo en curso del hilo sin publicarlo ni guardarlo, vacía la cola, descarta un guardado que aún espera su turno y espera a que termine el hilo; si su escritura ya tiene el turno, espera a que se confirme. Dejar de usar el almacén sin cerrarlo hace lo mismo. Lo que estaba en memoria sin guardar se pierde, y ninguna petición se recuerda al reabrir: la primera estimación que no vea estadísticas guardadas vuelve a muestrear. |
| ST11 | **Modo temporal**: las estadísticas, el contador, la estimación, la construcción en segundo plano y analizar funcionan igual en un almacén temporal (G9), en memoria. |
| ST12 | **Errores** (amplía E17): un nombre de tabla o de columna que no mide de 1 a 1.024 bytes falla con **tamaño no válido** (B8); estimar o analizar una columna que la tabla no tiene falla con **esquema distinto**, con la tabla y la columna, como recorrer un índice que no existe (IX9); estimar o analizar una columna vector falla con **esquema no válido**, con la tabla, la columna y la regla columna vector (como IX3); leer las estadísticas de un índice que la tabla no tiene falla con **esquema distinto**, con la tabla y el índice; un valor de una estimación que incumple IX9 falla con **valor no válido**, con la tabla, la columna y la regla: tipo distinto del de la columna, fuera del dominio o intervalo no válido (un nulo como límite); en la igualdad en varias columnas, una lista vacía falla con valor no válido y la regla intervalo no válido, y una columna repetida, con la regla columna repetida, antes que los valores; con varios valores que fallan, el primero (en una condición de una columna, el de igualdad, el límite inferior y el superior; en varias columnas, en el orden de la lista), con la primera regla que incumple en el orden de E17, como IX9. Analizar en el almacén una tabla que no existe falla con árbol no encontrado, y un nombre de otra clase de objeto, con tipo distinto (E2). La precedencia es la de E17: primero uso incorrecto (T5); después el tamaño no válido de los nombres; después árbol no encontrado de una apertura de una tabla borrada; después los fallos al leer el catálogo, árbol no encontrado y tipo distinto; después esquema distinto y, tras él, esquema no válido de la columna; después valor no válido; después los fallos al leer la muestra; y, al analizar en el almacén, por último el tiempo de espera agotado, la entrada y salida de T12 al pedir el turno y los fallos al guardar. |
| ST13 | **Formato**: el almacén guarda el contador y las estadísticas de cada tabla, que un motor del formato 12 no sabe leer, así que el cambio es incompatible (B12) y la versión del formato pasa a 13: un almacén del formato 12 es de formato anterior y este motor no lo abre (F11). La información del almacén (P12) no cambia de forma: las páginas de las estadísticas cuentan en sus totales. |
| ST14 | **Ganchos de prueba** (los que fija 7.3 para AC-N8): uno detiene un muestreo de una estimación después de leer y antes de publicar; otro detiene un muestreo del hilo o de analizar en el almacén después de leer y antes de publicar; otro detiene un guardado después de obtener el turno y antes de confirmar; otro informa si el hilo de ST6 existe, cuántas peticiones de cada clase hay en su cola, sin contar la que atiende, y cuántos muestreos se han hecho en el almacén; otro espera a que el hilo no tenga ningún trabajo; otro cuenta las páginas que lee un muestreo; y otro fija la semilla con que se elige la muestra de una tabla de más de 30.000 filas. Son mecanismos de prueba, no contratos públicos. |
| ST15 | **Medición** (N11): además de las mediciones de una sola carga de W1–W32 antes y después del cambio, que miden lo que cuesta llevar el contador, el informe de cierre mide, en Windows y en Linux: lo que tarda la primera estimación sobre la tabla de 1.000.000 de filas de W23, que muestrea (ST3); y W23 y W28 con un segundo hilo que, tras cada escritura de 1.000 filas, pide una estimación de cada columna, de modo que se muestree la primera vez y se renueven en segundo plano cada vez que estén viejas, frente a las mismas cargas sin ese hilo. No hay cargas nuevas del banco ni umbrales nuevos: Voron no tiene estadísticas. |
| ST16 | **Leer las estadísticas** (C2 del documento de diseño 10): en una transacción de lectura o de escritura, sobre una tabla abierta, se piden las estadísticas de una columna por su nombre. Devuelve que no hay, si la transacción no ve estadísticas de esa columna (ST1), o, de las que ve, en una copia propia de la aplicación: las **filas de origen**, el número de filas de la tabla en su instantánea de origen; las **filas leídas** para construirlas (todas, hasta 30.000; ST1); el **contador de origen** (ST4); si están **viejas** para la transacción (ST5); la **fracción de nulos**, un número real de 0 a 1; la **fracción de valores grandes** (ST1), cero en las columnas que no son `string` ni `bytes`; los **distintos**, el número estimado de valores distintos no nulos, un número real no negativo; los **valores frecuentes**, una lista, quizá vacía, de pares de un valor no nulo y su fracción de las filas, de mayor a menor fracción y, a igual fracción, en el orden de N5 del tipo de la columna; y el **histograma**, una lista, quizá vacía, de cubetas sobre los valores no nulos que no están entre los frecuentes, en el orden de N5, cada una con su **límite superior** (el mayor valor de la cubeta), su fracción de las filas y si el límite está **acortado**. Las fracciones se refieren a las filas de origen, y la de nulos, la de valores grandes, las de los valores frecuentes y las de las cubetas suman 1, salvo el redondeo, en una tabla con filas; en una de cero filas, todo es cero y las listas están vacías. Los valores tienen la normalización de N4 y E7 (un `uuid` en el orden de bytes de RFC 9562 también en una columna con el orden de SQL Server) y se decodifican en la copia, también el texto y los bytes (como las claves de IX8). Un límite de texto o de bytes puede estar acortado a un prefijo suyo, el texto en un límite de carácter; entonces el límite que se devuelve es ese prefijo y está marcado como acortado. Leer las estadísticas no muestrea ni pide ningún trabajo, no lee páginas de las filas ni de los índices, no es un cambio para T10 y no termina referencias (E16). Del mismo modo se piden las estadísticas de un índice por su nombre: devuelve que no hay, si la transacción no ve las de ninguno de sus prefijos o el índice tiene una sola columna en la clave, o, por cada prefijo de 2 columnas hasta todas las de la clave, en ese orden: el número de columnas del prefijo, las filas de origen, las leídas, el contador de origen, si están viejas, los **distintos** (el número estimado de combinaciones distintas de valores, contando el nulo como un valor más) y las **combinaciones frecuentes**, una lista, quizá vacía, de pares de una combinación de valores (cualquiera puede ser nulo), en el orden de las columnas del prefijo, y su fracción de las filas, de mayor a menor fracción y, a igual fracción, en el orden del índice (N5, con su dirección y sus nulos). Sus errores son los de estimar (ST12), sin los de los valores. |
| ST17 | **Estadísticas de los prefijos de un índice**: cada índice con dos o más columnas en su clave (IX1) tiene, por cada prefijo de su clave de 2 columnas hasta todas, estadísticas con el número estimado de combinaciones distintas de los valores de esas columnas (el nulo cuenta como un valor más) y las combinaciones más frecuentes con su fracción de las filas, de la misma instantánea de origen y con el mismo contador que las de las columnas (ST1, ST5). Se construyen con la misma muestra, en el mismo muestreo, sin leer el índice; se identifican por la identidad del índice (IX6) y su número de columnas, así que renombrar una de sus columnas las conserva. Sirven para la igualdad en varias columnas (ST18) y se leen con ST16. No hay estadísticas de varias columnas para las columnas que no forman el prefijo de un índice: las declaradas, como `CREATE STATISTICS` de PostgreSQL, quedan para un PRD posterior (usuario: «Vale», a dejarlas para después). |
| ST18 | **Igualdad en varias columnas**: la condición es una lista de pares de una columna y un valor, con columnas distintas, de una a todas las columnas que no son vector; cada valor sigue las reglas de la igualdad de ST2, también el nulo. Las filas estimadas se calculan así: (1) **grupos**: mientras queden columnas de la condición sin cubrir, se toma, entre los prefijos de índice (ST17) cuyas columnas están todas en la condición y ninguna cubierta ya, el de más columnas y, a igual número, el del índice que va primero en el orden de sus nombres según B5; cada columna que no cubre ningún prefijo es un grupo de una columna; (2) **fracción de cada grupo**: la que dan sus estadísticas para los valores de sus columnas, como una igualdad de ST2; (3) **combinación**, sesgada a sobrestimar (usuario: «sesgado a sobrestimar»): con las fracciones de los grupos ordenadas de menor a mayor, f₁ ≤ f₂ ≤ f₃ ≤ f₄, se toma f₁ × f₂^(1/4) × f₃^(1/16) × f₄^(1/64), sin los grupos que pasan del cuarto, que solo podrían reducirla; queda entre el resultado de suponer las columnas independientes (el producto) y el de suponerlas totalmente correlacionadas (la menor fracción); con un solo grupo, es su fracción y la estimación es **conjunta**; (4) se multiplica por el número de filas que ve la transacción. Después valen las reglas exactas de ST2, y además: un nulo en una columna que no admite nulos estima cero; y si los pares dan valores no nulos a todas las columnas de la clave primaria, o a todas las de la clave de un índice único, como mucho una fila. Si falta alguna de las estadísticas de los grupos, la estimación muestrea (ST3); las estadísticas de un prefijo que no se pueden construir porque su índice se creó en la escritura que estima se construyen como dice ST3. Con una sola columna, es la igualdad de ST2. Los exponentes son el retroceso exponencial de SQL Server (s₁ × s₂^½ × s₃^¼ × s₄^⅛) con cada exponente elevado al cuadrado, para que la combinación quede más cerca de la menor fracción. |

*Anotaciones del 2026-11-21 desde las columnas JSON ([tables-json.prd.md](tables-json.prd.md), 7.5):*

- *ST1 y Fuera de alcance: las columnas JSON no tienen estadísticas, como las columnas vector, y el muestreo no lee
  sus documentos (JS10).*
- *ST12: estimar, leer o analizar una columna JSON falla con esquema no válido y la regla columna JSON, en el lugar de
  la regla columna vector (JS10).*
- *ST18: «las columnas que no son vector» excluye también las columnas JSON (JS10).*

*Anotaciones del 2026-11-22 desde los valores temporales en microsegundos ([tables-temporal-micros.prd.md](tables-temporal-micros.prd.md)):*

- *ST2: un valor temporal de una condición ya está en microsegundos y no se redondea; los valores temporales de las
  estadísticas se devuelven en microsegundos (TU1, TU3).*

*Anotaciones del 2026-11-23 desde la intercalación de las columnas de texto ([tables-collation.prd.md](tables-collation.prd.md), 7.7):*

- *ST1, ST2, ST16, ST17 y ST18: las estadísticas y las estimaciones de una columna `ci` o `ci_ai` usan formas
  plegadas; un texto cuya forma plegada pasa de 2.048 bytes también es grande, y un límite se acorta en un límite de
  valor escalar de la forma plegada (IC7, IC8).*

*Anotación del 2026-11-25 desde los índices vectoriales aproximados ([tables-vector-indexes.prd.md](tables-vector-indexes.prd.md), 7.6):*
*el filtro de una búsqueda de vecinos parte de las condiciones de ST2, añade IN y NOT IN y trata los nulos como SQL,
distinto de ST2 (VI7); las estimaciones de 7.3 no cambian.*

### Estado de partida

Verificado el 2026-11-19 en el código y en los documentos de diseño:

- No hay estadísticas, ni contador de modificaciones, ni operaciones de estimación o de analizar. La entrada de una
  tabla en el catálogo guarda su identidad, la raíz y el número de filas de su árbol de filas, su historial de esquemas,
  la versión y el estado del esquema, sus columnas, su clave primaria, el estado de los índices y sus índices, sin
  campos reservados para estadísticas; su lectura rechaza bytes de más.
- Las tablas y las columnas tienen identidades aleatorias que se conservan al renombrar; borrar una tabla retira su
  identidad, así que una vuelta a crear tiene otra.
- Las escrituras que cambian el número de filas pasan por un mismo punto, salvo el camino rápido de añadir al final y
  vaciar la tabla, que lo pone a cero directamente;
  actualizar informa de si no encontró la fila, una actualización idéntica no escribe nada y borrar un intervalo
  informa de cuántas filas borró.
- El almacén tiene dos hilos propios, el del diario y el que lleva las páginas al archivo de datos; cerrar el almacén
  espera a que terminen. Ninguna escritura interna pide el turno de escritura.
- Los árboles no tienen ninguna operación de estimación de un intervalo ni de conteo entre dos claves.
- La información del almacén cuenta páginas, no árboles ni tablas. La versión del formato es la 12.

## Actores

- **Desarrollador de la aplicación (Rust)**: pide estimaciones de las columnas de sus tablas y, cuando lo necesita,
  analiza una tabla o una columna.
- **Planificador de las consultas (módulo 17)**: usará la misma estimación (N8 del padre).
- **Mantenedor**: ejecuta las mediciones de ST15 en Windows y en Linux.

## Flujo

1. La aplicación crea una tabla y guarda filas (7.1); cada escritura suma al contador (ST4).
2. Pide una estimación de una columna (ST2); la primera vez, sin estadísticas, la estimación muestrea la tabla, responde
   con lo que leyó y lo deja en memoria para las demás transacciones (ST3).
3. El hilo del almacén las guarda con una escritura corta (ST6).
4. Las estimaciones siguientes las usan; cuando las modificaciones superan el umbral (ST5), las siguen usando, dicen que
   están viejas y piden que el hilo las renueve.
5. Si quiere estadísticas al día sin esperar a que envejezcan, la aplicación analiza la tabla (ST7).

## Validaciones y mensajes

Valen las de los ascendientes. Este PRD añade, solo en la biblioteca, en el orden de su precedencia (ST12):

| Situación | Clase de error |
|---|---|
| Analizar en el almacén desde un hilo que tiene una escritura abierta en él | Uso incorrecto (T5) |
| Analizar en el almacén con un nombre de tabla, o estimar, leer o analizar con un nombre de columna o de índice, que no mide de 1 a 1.024 bytes | Tamaño no válido (B7, B8) |
| Estimar o analizar a través de una apertura de una tabla que la escritura borró | Árbol no encontrado (A6, E17) |
| Analizar en el almacén una tabla que no existe, o un nombre de otra clase de objeto | Árbol no encontrado; tipo distinto (E2) |
| Estimar, leer o analizar una columna que la tabla no tiene, o leer las estadísticas de un índice que no tiene | Esquema distinto, con la tabla y la columna o el índice |
| Estimar, leer o analizar una columna vector | Esquema no válido, regla columna vector |
| Una igualdad en varias columnas vacía o con una columna repetida; una estimación con un valor de otro tipo, fuera del dominio o con un nulo como límite | Valor no válido (E17) |
| Un fallo al leer la muestra al estimar o al analizar | Daño, entrada y salida o memoria agotada, con T11 en una escritura |
| Analizar en el almacén con un límite de espera que vence antes del turno | Tiempo de espera agotado (T4) |
| Analizar en el almacén cuando el almacén no acepta escrituras | Entrada y salida (T12) |
| Un fallo al guardar al analizar | Disco lleno (G7), tamaño máximo (P10), memoria agotada (P13) o entrada y salida (T12) |

Una estimación sin estadísticas (muestrea), con estadísticas viejas, sobre una tabla vacía o con un intervalo vacío no
es un error.

## Salida

El módulo no escribe nada en pantalla. Estimar devuelve las filas estimadas, un número real, si alguna de las estadísticas
que usó está vieja y si es conjunta (ST2, ST18). Leer las estadísticas de una columna o de un índice devuelve que no hay o los campos de ST16, en una copia propia de la
aplicación. Analizar no devuelve nada. Las lecturas, los recorridos, las escrituras y la información del
almacén no cambian su salida.

## Reutilización

- Las reglas de los valores de un intervalo de un índice (IX9) para las condiciones de una estimación.
- El turno de escritura y su cola (G4, T4) para el guardado en segundo plano y analizar en el almacén.
- Las identidades de las tablas y las columnas (E2) para descartar construcciones de una tabla o una columna que ya no
  existen.
- El número de filas que informa una apertura (N6, E11).
- Las mediciones de una sola carga y las cargas W23 y W28 del banco (módulo 6, 7.1).

## Fuera de alcance

- Leer el contador de modificaciones actual de una tabla (ST4).
- Estadísticas declaradas de varias columnas sin índice (`CREATE STATISTICS` de PostgreSQL): un PRD posterior (ST17).
- La estimación de un intervalo por descenso en el árbol (documento de diseño 3).
- Estimar condiciones que mezclan igualdades con un intervalo, varios intervalos, distinto de, `no es nulo` u `OR`:
  las combina el planificador del módulo 17, con la regla de ST18 para las conjunciones.
- Ajustar las estimaciones con lo que devuelven las consultas, como el *CE feedback* de SQL Server 2022: módulo 17.
- Estadísticas de las columnas vector.
- Una opción para desactivar las estadísticas en segundo plano (pregunta 7 del documento de diseño 10): la añade el
  módulo de las herramientas si la necesita.
- Configurar el tamaño de la muestra, el número de cubetas o de valores frecuentes, o el umbral de ST5.
- Exportar e importar estadísticas.
- Estimaciones por defecto o suposiciones: sin estadísticas, la estimación muestrea (ST3; usuario: «si no espera al
  muestreo entonces tiene que usar "suposiciones" (guesses) y es lo que no quiero»).
- Volver a muestrear en la estimación cuando las estadísticas están viejas: se usan y se renuevan en segundo plano,
  como el modo asíncrono de actualización de SQL Server (ST2).
- Recordar entre aperturas del almacén las peticiones pendientes o las estadísticas en memoria sin guardar.
- Construir estadísticas sin que las pida una estimación o analizar: al abrir el almacén, tras una carga masiva o
  periódicamente.
- Las estadísticas de las columnas JSON: las fija 7.5.
- Copiar, compactar o verificar las estadísticas y el contador: los módulos 10, 11 y 12 los tratan como árboles (N1).
- Cancelar el muestreo de una estimación o ponerle un tiempo límite propio (ST3).
- Leer o construir estadísticas desde la herramienta de línea de comandos (C2 y C10 del documento de diseño 10): los
  fija el módulo de esa herramienta.
- Una precisión prometida para las estimaciones de una tabla muestreada, más allá de AC-ST1.
- Conteos exactos de un intervalo (padre).

## Dejado a la implementación

- Cómo se guardan las estadísticas y el contador en el catálogo y en árboles, con el punto de partida del documento 3.
- El tamaño de la muestra de una tabla de más de 30.000 filas y cómo se elige, el número de cubetas, el número de
  valores frecuentes, la longitud, de menos de 1.000 bytes, a la que se acortan los límites de texto y de bytes, cuántas combinaciones frecuentes guarda un prefijo, y cómo se calcula
  la fracción de cada estimación y de cada grupo con las estadísticas, siempre que cumplan ST1, ST2, ST17, ST18 y los criterios de aceptación.
- Los nombres de las operaciones, los tipos y las clases de error, en inglés y con las convenciones de Rust (D8).
- Cómo se construyen los ganchos de prueba de los criterios: son mecanismos de prueba, no contratos públicos.

## Criterios de aceptación

Valen los de los ascendientes. Este PRD cumple AC-N8 del padre y AC-N10 con lo que estos criterios fijan.

1. **AC-ST1** — Dada una tabla de 10.000 filas, generada con una semilla fija, con una columna de cada tipo de N4 que se
   puede ordenar, cada una (salvo `bool`) con un valor en el 40 % de las filas, un 10 % de nulos y el resto con al menos
   100 valores distintos repartidos de forma uniforme, y una columna `bool` con un 40 % de verdaderos, un 50 % de falsos
   y un 10 % de nulos, analizada, cuando se estiman igual al valor frecuente (en `bool`, verdadero y falso), igual a
   nulo, igual a un valor que no está en la tabla (salvo en `bool`), e intervalos cuyos límites son valores de la tabla
   elegidos con la semilla, con cada combinación de límites incluidos, excluidos y ausentes, entonces no están viejas,
   son conjuntas, igual al valor frecuente e igual a nulo dan sus filas con una diferencia relativa de como mucho 10⁻⁹
   (la tabla se leyó entera, ST1), y el valor que no está y cada intervalo dan sus filas reales con un error de como
   mucho el 5 % de las filas de la tabla; `-0` y NaN estiman los valores normalizados; y las reglas exactas de ST2 dan
   lo que dicen, con la misma diferencia. Dada la misma distribución en una tabla de 200.000 filas, muestreada con la
   semilla del gancho de pruebas (ST14), igual al valor frecuente e igual a nulo dan sus filas reales con el mismo error
   de como mucho el 5 %.
2. **AC-ST2** — Dada una tabla de 10.000 filas como la de AC-ST1, sin estadísticas y con el guardado detenido en su
   gancho (ST14), cuando una lectura estima cada condición de AC-ST1, entonces la primera estimación muestrea (el gancho
   cuenta un muestreo), y todas dan los valores de AC-ST1 y no están viejas; cuando otra lectura, y una escritura, estiman
   después, entonces usan las de memoria sin muestrear; cuando se libera el guardado y se cierra y reabre el almacén,
   entonces la primera estimación usa las guardadas sin muestrear; cuando se cierra con el guardado aún en la cola y se
   reabre, entonces la primera estimación vuelve a muestrear (ST10); cuando dos lecturas estiman a la vez con el
   muestreo de las estimaciones detenido en su gancho, entonces ninguna espera a la otra y las dos responden al
   liberarlo; una tabla vacía estima cero sin muestrear; y una tabla analizada sin filas, a la que se añaden 400, vuelve
   a muestrear en la primera estimación; una analizada con 100 filas vuelve a muestrear con 201 y no con 200, y una
   analizada con 500 no vuelve a muestrear con 1.001; una lectura más antigua que unas estadísticas de memoria las usa y no están
   viejas (ST5); y una escritura que inserta 1.000 filas y estima usa esas filas en el número de filas (AC-N8, cambiado:
   no hay estimación por defecto).
3. **AC-ST3** — Dada una tabla analizada, cuando cada operación de ST4 se confirma en una escritura (insertar con
   claves en orden y al azar, una carga masiva en orden y fuera de orden, y borrar un intervalo), y en otra se
   aborta, entonces el contador, observado a través de ST5, suma exactamente lo que dice ST4: con
   modificaciones hasta el umbral las estadísticas no están viejas y con una más sí, en una tabla de 400 filas (umbral
   500), de 10.000 (2.500) y de 100.000 (10.000); las operaciones que no suman no las hacen viejas; vaciar una tabla
   analizada de 400 filas y volver a analizarla da un contador de origen 400 mayor que el anterior, leído con ST16; una carga masiva que
   falla, entera según E13, no suma nada; y cuando una carga masiva falla mientras una renovación en segundo plano
   está detenida después de muestrear, entonces las estadísticas que se guardan no cuentan ninguna fila de la carga
   (AC-N8).
4. **AC-ST4** — Dada una tabla analizada, cuando se hacen modificaciones por debajo del umbral, se cierra y se reabre
   el almacén, también tras simular una caída del proceso, y se hacen las que faltan para superarlo, entonces la
   estimación siguiente usa las estadísticas, dice que están viejas, no muestrea en la estimación y pide una
   renovación, que el gancho cuenta en la cola; cuando el gancho dice que el hilo no tiene trabajo, entonces la estimación siguiente dice que
   no están viejas y da los valores de las filas nuevas; y cuando se vacía una tabla de más filas que el umbral y se le
   añaden filas, entonces sus estadísticas no existen, también las de memoria, y la estimación siguiente muestrea
   (AC-N8).
5. **AC-ST5** — Dada una renovación en segundo plano detenida en su gancho después de muestrear, y una escritura confirmada después, cuando se analiza
   la tabla en el almacén y termina, se libera la construcción y termina después, y al revés, entonces queda la de la
   instantánea más nueva, sin que ninguna espere a la otra; cuando una escritura cambia filas y analiza mientras está
   pendiente una construcción de una instantánea anterior, entonces no hay bloqueo, confirma, y las estadísticas que
   quedan son las de la escritura; cuando una escritura cambia filas, analiza y aborta, entonces las filas, el contador y
   las estadísticas quedan como antes; y cuando cambia más filas después de analizar y confirma, entonces esas
   modificaciones cuentan para ST5 (AC-N8).
6. **AC-ST6** — Dada una renovación detenida después de muestrear, y por otro lado un muestreo de una estimación
   detenido en su gancho, cuando se renombra la columna, entonces se publican y se
   guardan, y se leen con el nombre nuevo; cuando se quita la columna, entonces no se publican ni se guardan para ella; cuando se borra la
   tabla y se vuelve a crear con el mismo nombre, entonces no se guardan para la nueva; cuando se vacía la tabla,
   entonces no se guardan; cuando se renombra la tabla, entonces se guardan para ella con su nombre nuevo; y en cada
   caso las de las demás columnas siguen ST6. Renombrar la tabla y crear o borrar un índice las conservan; una columna
   añadida no tiene; y una estimación en una escritura sobre una tabla creada en ella muestrea lo que ve la escritura,
   no publica nada en memoria ni pide que se guarde, y otra estimación de la misma escritura no vuelve a muestrear
   (ST3, ST6, ST8).
7. **AC-ST7** — Dado el guardado de una construcción detenido en su gancho con el turno de escritura, cuando una
   escritura de la aplicación pide su turno con un límite de espera, entonces falla con tiempo de espera agotado al
   vencer (T4), y con un límite de cero falla al instante; cuando se cierra el almacén con una renovación detenida
   después de muestrear y otra petición en la cola, entonces el cierre termina sin guardar ninguna, y el gancho dice
   que el hilo ya no existe; con el guardado detenido, el cierre espera a que se confirme y, al reabrir, las
   estadísticas están; y analizar en el almacén con un límite de cero mientras hay otra escritura abierta falla con
   tiempo de espera agotado después de muestrear (AC-N8, ST7, ST10).
8. **AC-ST8** — Dado un fallo de entrada y salida simulado al confirmar un guardado en segundo plano, entonces la
   siguiente petición de escritura falla al instante con la clase de entrada y salida, las estimaciones siguen usando
   las de memoria, una estimación sobre otra tabla sin estadísticas muestrea sin pedir que se guarde, y el cierre sigue
   T12; dados un daño en una página de la tabla, un disco lleno y una memoria agotada simulados durante una renovación,
   con los simuladores de fallos de los ascendientes, entonces se descarta sin error visible, las viejas se siguen usando, el almacén sigue aceptando escrituras y la
   siguiente estimación la vuelve a pedir; dado un disco lleno al guardar, las de memoria se siguen usando y la siguiente estimación pide guardarlas; dados los
   mismos fallos al leer la muestra en una estimación, entonces la estimación falla con su clase, y en una escritura
   con daño, esta ya no puede confirmarse (T11); y los mismos fallos al analizar en el almacén los informa analizar con
   su clase, y el de entrada y salida al guardar deja el almacén sin aceptar escrituras (ST3, ST7, ST9).
9. **AC-ST9** — Dado cada caso de *Validaciones y mensajes*, cuando se estima, se leen o se analiza, entonces falla con su
   clase y sus datos sin cambiar el almacén (un analizar en el almacén que falla después de muestrear deja lo publicado
   en memoria, ST7, y una estimación posterior lo usa sin muestrear), y con dos fallos a la vez gana el que dice ST12; analizar en una transacción de lectura no existe en la
   API (no compila); estimar no es un cambio para T10 ni termina referencias; analizar en una escritura es un cambio y termina
   referencias, y uno que falla las termina sin ser un cambio (ST7).
10. **AC-ST10** — Dado un almacén temporal, cuando se repiten AC-ST2, AC-ST4 y AC-ST5, entonces dan lo mismo, salvo lo
    que depende de reabrir, y no se crea ningún archivo ni carpeta (ST11).
11. **AC-ST11** — Dado un almacén del formato 12 escrito por 7.2, cuando este motor lo abre, entonces falla con la
    versión del formato (B12, F11); un almacén nuevo tiene la versión 13; y la información del almacén no cambia de
    forma (ST13).
12. **AC-ST12** — Dada la tabla de 10.000 filas de AC-ST1, analizada, cuando se leen las estadísticas de cada columna,
    entonces las filas de origen y las leídas son 10.000, el contador de origen es el de la tabla al analizar, no están
    viejas, la fracción de nulos es 0,1 y, salvo en `bool`, el primer valor frecuente es el del 40 % con 0,4, las cubetas van en el orden
    de N5 y las fracciones suman 1 con una diferencia de como mucho 10⁻⁹, y las fracciones de nulos y de
    valores frecuentes tienen esa misma diferencia; con la tabla de 200.000 filas, las leídas son
    menos que las de origen; una columna `string` con valores de 1.000 bytes tiene límites marcados como acortados que son
    prefijos de valores de la tabla; una columna `bytes` con un 20 % de valores de 2.049 bytes y el resto de 2.048 tiene
    una fracción de valores grandes de 0,2, una igualdad con un valor de 4.096 bytes estima el 20 % de las filas sin
    fallar, una con un valor de 2.048 bytes no los cuenta, y un intervalo los suma; el gancho de pruebas que cuenta las
    páginas leídas muestra que el muestreo no lee las páginas de esos valores; la columna `bool` tiene falso con 0,5 como primer valor frecuente; tras superar el umbral, están
    viejas; una columna sin estadísticas, una añadida y una tabla vacía sin analizar dan que no hay, sin que el gancho
    cuente ningún muestreo ni ninguna petición; una tabla analizada sin filas da todo cero; y leer en una escritura no es un
    cambio para T10 ni termina referencias (ST16).
13. **AC-ST13** — Dada una tabla de 10.000 filas, generada con una semilla fija, con columnas `pais` (20 valores),
    `ciudad` (200 valores, cada una de un solo país, así que `ciudad` determina `pais`) y `n` (100 valores, independiente
    de las otras dos), y un índice sobre (`pais`, `ciudad`, `n`), analizada, cuando se estima igual en (`ciudad`,
    `pais`) con una ciudad y su país, entonces es conjunta y da sus filas reales con una diferencia relativa de como
    mucho 10⁻⁹ (un prefijo cubre las dos columnas, en cualquier orden); cuando se estima igual en (`ciudad`, `pais`, `n`),
    entonces es conjunta; cuando se borra el índice y se estiman las mismas condiciones, entonces no son conjuntas y dan
    f₁ × f₂^(1/4), y en tres columnas f₁ × f₂^(1/4) × f₃^(1/16), con las fracciones de cada columna ordenadas, con la
    misma diferencia; con cinco columnas, el quinto grupo no cuenta; con dos índices cuyos prefijos cubren las mismas
    columnas, se usa el de más columnas y, a igualdad, el primero por su nombre; un nulo en una columna que no admite
    nulos estima cero; todas las columnas de la clave primaria, o de un índice único, con valores no nulos estiman como
    mucho una fila; cuando se crea un índice nuevo y se estima por sus columnas, entonces la estimación muestrea (el
    gancho cuenta un muestreo); renombrar una de sus columnas conserva sus estadísticas, y quitarlo o vaciar la tabla las
    quita; leer las estadísticas del índice devuelve los dos prefijos con sus distintos y sus combinaciones frecuentes en
    el orden de ST16; y una lista vacía o con una columna repetida falla con valor no válido y su regla (ST17, ST18).
14. **AC-ST14** — Dada una escritura que añade una columna, crea un índice o vacía la tabla y después estima por ellos,
    entonces muestrea lo que ve la escritura, no publica nada en memoria ni pide guardar, y otra transacción no ve esas
    estadísticas; y dada una escritura que estima con unas estadísticas sacadas en esa misma estimación y cuyas propias
    modificaciones superan el umbral, entonces dice que están viejas y pide una renovación (ST2, ST3).
15. **AC-ST15** — Dado el cierre de este PRD, cuando se revisa su informe, entonces incluye las mediciones de una sola
    carga de W1–W32 antes y después del cambio y las de ST15, en Windows y en Linux (AC-N10).

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan estas anotaciones del 2026-11-19 (en sus
traducciones también):

- [krummi.prd.md](../krummi.prd.md): G6 (excepción para el muestreo: una fila guardada en un tramo se lee por páginas
  sin verificar la suma de comprobación del tramo entero, ST1; anotación del 2026-11-19, antes de la ejecución).
- [krummi.prd.md](../krummi.prd.md): G3 y AC6 (excepción para las estadísticas: las de memoria son compartidas, así que
  una lectura puede usar unas de una instantánea posterior a la suya y dos estimaciones de la misma transacción pueden
  usar estadísticas distintas, ST1; las filas, el contador y las estadísticas guardadas siguen G3).
- [tables.prd.md](tables.prd.md): N8 (cambia: no hay estimación por defecto; sin estadísticas, la estimación muestrea
  en ese momento y espera a la muestra, así que deja de valer «Ninguna otra operación de la aplicación espera a que se
  calculen» para la estimación; con estadísticas viejas las usa y pide renovarlas en segundo plano, ST2, ST3; leer las
  estadísticas también es una operación pública, ST16; analizar en el almacén y en una escritura, ST7; qué suma al
  contador, ST4); *Flujo*, paso 5 (la estimación muestrea cuando faltan y las renueva en segundo plano cuando están
  viejas); AC-N8 (cambia: «responde la estimación por defecto sin esperar» y «responde la estimación por defecto y se
  vuelven a construir» pasan a ser los casos de AC-ST2 y AC-ST4; lo cumplen AC-ST2 a AC-ST8); la fila 7.3 del reparto
  (lo que fija queda fijado aquí).
- [tables-schema.prd.md](tables-schema.prd.md): E3 («reinicia sus estadísticas» quiere decir que las quita, ST8); E16
  (analizar en una escritura es un cambio y termina referencias; estimar no, ST2, ST7); E17 (los errores de ST12); E20
  (la versión del formato pasa a 13, ST13).
- [tables.prd.md](tables.prd.md), *Fuera de alcance*: «Estadísticas de varias columnas juntas» pasa a excluir solo las
  declaradas sobre columnas sin índice; las de los prefijos de los índices entran en 7.3 (ST17, ST18).
- [transactions.prd.md](../transactions/transactions.prd.md): T4 (la escritura interna que guarda estadísticas espera
  su turno como cualquier escritura y sin límite, ST6); T12 (un fallo de entrada y salida de esa escritura no lo
  informa ninguna operación de la aplicación al ocurrir, sino las peticiones de escritura siguientes y el cierre, ST9).

Y los documentos de diseño: el [3](../../../design/tables/03-keys-and-indexes.md), *Statistics* («No query waits for
sampling» y la estimación por defecto dejan de valer para la estimación, que muestrea cuando no hay estadísticas, ST3;
las estadísticas de un índice guardan, por cada prefijo, también sus combinaciones frecuentes, ST17; la estimación por
descenso en el árbol queda fuera de 7.3); el [4](../../../design/tables/04-query-tree-and-planner.md) (una conjunción de
igualdades no se combina con `sel1 * sel2`, sino con el retroceso sesgado de ST18); y el
[10](../../../design/tables/10-agent-tooling.md), C2 (leer las estadísticas es público desde 7.3, ST16).

## Referencias

- Padre: [tables.prd.md](tables.prd.md) (N1, N2, N6, N8, N10, N11, AC-N8, AC-N10); su padre,
  [krummi.prd.md](../krummi.prd.md) (D8, G1, G4, G5, G9, G12).
- Hermanos: [tables-schema.prd.md](tables-schema.prd.md) (E2, E3, E7, E8, E10, E13, E16, E17, E20, *Cargas del banco*),
  [tables-indexes.prd.md](tables-indexes.prd.md) (IX3, IX9).
- Árboles: [trees.prd.md](../trees/trees.prd.md) (B7, B8, B12), [trees-keys.prd.md](../trees/trees-keys.prd.md) (A6).
- Transacciones: [transactions.prd.md](../transactions/transactions.prd.md) (T4, T5, T10, T12).
- Pager: [pager.prd.md](../pager/pager.prd.md) (F11, P10, P12, P13).
- Diseño: `docs/design/tables/` (documentos 3, 4 y 10; README, H7, H8, H18).

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 32ed042c | 2026-11-19 09:08 UTC+00:00 | 1 h 32 min | 0 min | 1 h 32 min | 44 min | 48 min | 17 |

<!-- prd-time: [{"session": "32ed042c-b929-4114-ac1f-a407366961e6", "start": "2026-11-19T09:08:25.908000+00:00", "wall": 5495, "discarded": 0, "agent": 2641, "human": 2853, "interventions": 17, "idle_minutes": 10}] -->
