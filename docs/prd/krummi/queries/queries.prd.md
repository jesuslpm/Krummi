# Consultas de Krummi — PRD

**Versión**: 1.0
**Fecha**: 2026-11-28
**Documento padre**: [Krummi, motor de almacenamiento embebido inspirado en Voron — PRD](../krummi.prd.md) (v1.1)

## Objetivo y contexto

Construir las **consultas**, el módulo 17 del [reparto del padre](../krummi.prd.md#reparto-en-prd-hijos) (anotación
del 2026-11-10): leer las tablas del módulo 7 con filtros, orden, límites, agregados y, más adelante en el módulo, con
expresiones, joins, agrupación, subconsultas, operaciones de conjuntos y funciones de ventana, y modificarlas con
escrituras por consulta. Hoy una aplicación lee una tabla por su clave primaria, por un intervalo de la clave o de un
índice, o con la búsqueda de vecinos de 7.6, y combina ella los resultados. Con este módulo describe lo que quiere como un **árbol de consulta**, Krummi elige cómo leerlo con un
**planificador por coste** que usa las estadísticas de 7.3, y devuelve las filas por un **cursor**. La aplicación
también puede pedir el **plan de ejecución** que eligió, inspeccionarlo, cambiarlo y enviarlo de vuelta.

El objetivo del módulo es que, cuando se cierre, Krummi ofrezca la mayor parte de las consultas del SQL
estándar (el documento de diseño 8: los planes «grow later towards most of standard SQL by adding operators»),
expresadas como árbol y no como texto: un lenguaje de consulta de texto y SQL siguen fuera de alcance (anotación del
2026-11-10 a *Fuera de alcance* del padre). Usuario: «yo subdividiría este PRD en varios y al final tener la mayoría de
las consultas que los sistemas relacionales proporcionan». El módulo es demasiado grande para una sola ejecución, así
que este documento es su paraguas, como el de las [tablas](../tables/tables.prd.md) lo es del módulo 7: fija las reglas
comunes a todas las consultas y reparte el módulo en PRD hijos, cada uno con su propia ejecución y su propio cierre (ver
[Reparto en PRD hijos](#reparto-en-prd-hijos)). Este documento no tiene ejecución propia. Los PRD hijos añaden
operadores sin cambiar el significado de lo anterior (CS3).

**Por qué así**: como en las tablas, el diseño técnico se escribió antes que este PRD. Los documentos de diseño 4
(árbol de consulta y planificador), 8 (planes de ejecución), 9 (formato de intercambio), 10 (herramientas para
agentes), 11 (el contrato de las consultas, que reconcilia los anteriores), 13 y 14 (JSON) y 15 (perfil de las
consultas) registran, con sus palabras, cada decisión del usuario: U1–U9 y P1–P20 del documento 11 están decididas
desde el 2026-11-10. Este PRD toma de ellos lo observable; el cómo queda en esos documentos, que son el estado de
partida técnico de cada PRD hijo.

**Por qué ahora**: el módulo 7 está cerrado (N2 de las tablas, anotación del 2026-11-23: 7.1, 7.2, 7.3, 7.5, 7.6 y
7.7), y el padre construye el 17 justo después del 7.

Este PRD hereda todo lo del padre; aquí solo se añade lo común del módulo. Lo que cambia o precisa reglas anteriores
está declarado en sus decisiones y anotado en los documentos afectados (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

En este documento, una **consulta** es un documento que describe qué filas y qué valores se quieren, sin decir cómo
leerlos; un **plan** es un documento que dice cómo leerlos, con operadores físicos (por ejemplo, un recorrido de un
intervalo de un índice, un filtro o una ordenación); una **sentencia** es una consulta o un plan preparado para
ejecutarse muchas veces; un **parámetro** es un valor que la sentencia declara con su tipo y que la aplicación da en
cada ejecución; un **cursor** es la secuencia de filas de una ejecución, que la aplicación lee por lotes. Para no
confundirlos con las clases de error (D8 del padre), los cuatro de CS3 son **tipos de documento**.

### Decisiones de producto

| ID | Decisión |
|---|---|
| CS1 | **Superficie pública**: desde este módulo, las aplicaciones consultan tablas en Rust, en cualquier transacción, de lectura o de escritura (P3 del documento de diseño 11). Una consulta se escribe como documento JSON tipado o con una API fluida de Rust que produce el mismo documento (U1 del documento 11). La interfaz C y el binding C# (módulo 18) usan los mismos documentos. Hasta 17.6, una consulta no cambia los datos de la aplicación; desde 17.7, las escrituras por consulta los cambian solo en transacciones de escritura. Usuario, a incluirlas: «Sí, tras 17.5 (Recomendado)». Planificar una consulta usa la estimación de 7.3 con sus reglas: si faltan las estadísticas de una columna, espera a muestrearlas en su hilo (ST3), y si están viejas pide renovarlas en segundo plano; nunca responde con una suposición (ST2). Cada PRD hijo extiende las operaciones y los operandos de los documentos (una columna, una ruta de JSON, una expresión, una distancia) con versiones menores (CS3), sin cambiar lo que ya existía. |
| CS2 | **Reparto y cierre**: el módulo se reparte en los PRD hijos de la [tabla](#reparto-en-prd-hijos), que se construyen en ese orden; este documento no tiene ejecución propia. Cada regla común la implementa y la prueba el primer PRD hijo que la necesita, y los siguientes la siguen cumpliendo y la extienden a sus operadores sin cambiarla: la validación de los planes (17.2) sigue las fases, las clases de error y la precedencia que fija 17.1 para las consultas, y cada PRD hijo posterior añade sus operadores al documento de plan, a su validación y a la explicación. Cada PRD hijo deja Krummi utilizable: lo que añade funciona de principio a fin, desde el documento hasta las filas. El módulo 17 está cerrado cuando lo están todos sus PRD hijos. El diseño aprobó para «the first version» del árbol de consulta también el prefijo, la distancia como umbral y los `k` más cercanos, y que la interfaz pública acepte planes desde la primera versión del módulo (README de diseño); con este reparto, esa primera versión son 17.1 a 17.4, y el prefijo entra en 17.3, los planes en 17.2 y la distancia en 17.4. Usuario, a mantener el reparto: «Mantener el reparto (Recomendado)». Los índices parciales y sobre expresiones entran como 17.6 (usuario: «Sí, PRD hijo tras 17.5 (Recomendado)»). El módulo 18 se construye cuando el 17 está cerrado entero, con todo el lenguaje de consultas; usuario: «Tras todo el módulo 17». |
| CS3 | **Tipos de documento y versiones** (P1, P6 y P8 del documento 11): hay cuatro tipos: **consulta**, **plan**, **ejecución** (una consulta o un plan con los valores de una ejecución y sus límites, para pruebas y herramientas) y **explicación** (un plan, la consulta normalizada de la que salió, si salió de una, y los diagnósticos aparte). Cada documento nombra su tipo y su versión, con versionado semántico por tipo: una versión menor solo añade construcciones, y una mayor puede quitarlas. Un único lector, el de la versión vigente, acepta un documento de cualquier versión anterior; uno que usa una construcción quitada falla con documento no válido (CS10), nombrando la construcción y la versión que la quitó, y uno de una versión posterior a la del lector falla con la clase que fije 17.1. **Ninguna construcción cambia nunca de significado**: un significado nuevo lleva un nombre nuevo. Usuario: «en vez de cambiar el significado de like por ejemplo, podríamos añadir cs_like como case sensitive like». Antes de la versión 1.0 del producto no hay promesa, como D7 para los almacenes. Las consultas guardadas por la aplicación son un contrato público; los planes guardados no tienen más promesa que un rechazo con un error de su clase; las sentencias y los planes compilados nunca se guardan (U4). Una explicación no se puede preparar; su plan sí. Cada tipo tiene un JSON Schema publicado con la biblioteca, pero el validador de Krummi es la autoridad (U6). |
| CS4 | **Estructura y valores separados** (P15, U5): un documento de consulta o de plan no lleva valores, solo parámetros declarados con su tipo; también los números de `LIMIT`, `OFFSET` y `k` son parámetros. La API fluida convierte en parámetros las constantes que recibe. Cada tipo de valor tiene una sola forma que Krummi escribe y un conjunto fijado de formas que acepta; Krummi vuelve a escribir byte a byte un documento que escribió, y leerlo devuelve los mismos valores tipados (§2.4 del documento 11). Un nulo es un valor válido de cualquier tipo declarado, salvo como elemento de una lista de `IN`. |
| CS5 | **Sentencias** (P2, P3, P4, P12, P14, P16 y P17 del documento 11): preparar, enlazar los valores y ejecutar es el camino de toda consulta, en Rust en este módulo y en C y C# en el módulo 18. Una sentencia se prepara en una transacción contra el catálogo que esa transacción ve en ese momento, pertenece al almacén y se ejecuta en cualquier transacción, también desde varios hilos a la vez; cada conjunto de valores y cada cursor los usa un hilo cada vez, y una ejecución dentro de una escritura corre en el hilo de esa escritura (T6). Ejecutar exige que todos los parámetros tengan valor. Una sentencia fija las identidades de las tablas y columnas que enlazó (las identidades aleatorias de 7.1 y 7.2, P12 del documento 11, que los documentos escriben como 16 dígitos hexadecimales en minúscula); un documento de consulta puede llevarlas también (Krummi las escribe; al leerlo son opcionales), y entonces falla con esquema distinto si un nombre resuelve a otra identidad, mientras que sin ellas se enlaza por nombre al prepararse. Si la transacción que ejecuta ve otro catálogo que el de la sentencia, una consulta vuelve a resolver sus nombres y exige las mismas identidades, y un plan vuelve a validarse, como dice la [tabla de invalidación](#invalidación-de-las-sentencias). La validez depende solo de lo que ve la transacción que ejecuta, nunca de cómo terminó la que preparó. Una consulta se vuelve a planificar sola cuando cambia el catálogo, se reconstruyen las estadísticas o los valores piden otro plan; un plan nunca se vuelve a planificar: quien quiere fijar un plan prepara el de la explicación (CS6). La política de partida para volver a planificar es la adaptativa del documento 4 (P4 del documento 11). La única excepción es la de 17.13, que hace que una consulta vuelva sola a su último plan bueno, con los límites explícitos que aprobó el usuario en el documento 15 («Automático, límites explícitos»). Un cursor conserva su plan y una copia de los valores con que se ejecutó: cambiar los valores, volver a planificar o cerrar la sentencia no lo afecta. Un cursor no sobrevive a un cambio de su propia transacción de escritura: en Rust, el código que lo intenta no compila. Cerrar el almacén con sentencias vivas falla con uso incorrecto y el almacén sigue abierto. |
| CS6 | **Planes explícitos** (U3, P5 del documento 11): la explicación de una sentencia, con valores o sin ellos (sin ellos, el plan que Krummi elegiría sin conocer los valores, con las estimaciones que fije 17.2), devuelve el plan que Krummi ejecutaría, que la aplicación puede guardar, cambiar y preparar como sentencia. Un plan solo lleva lo que cambia lo que hace; las estimaciones y las métricas van solo en los diagnósticos de la explicación, y los límites de una ejecución son argumentos de la ejecución, nunca miembros del plan. Todo plan enviado pasa la validación completa, sin atajos de confianza: un plan mal formado o incoherente falla con un error de su clase, nunca termina en un pánico ni da un resultado incorrecto en silencio. Un plan que Krummi escribe para una transacción pasa su validación sin cambios en ella y en toda transacción que vea las identidades que fija (la tabla, su marca de estado de esquema, las columnas y los índices que usa; no la marca de los índices, así que un índice nuevo no lo invalida), y ejecutado con los mismos valores devuelve un resultado admisible (CS7) para la sentencia de la que salió; en las demás falla con esquema distinto. Krummi garantiza que un plan escrito por la aplicación se ejecuta de forma segura con su propio significado; no garantiza que exprese lo que la aplicación quería. |
| CS7 | **Semántica común**: una consulta lee la instantánea de su transacción (G3). La lógica de los nulos es la de tres valores de SQL: una comparación con un nulo es desconocida, y un filtro solo deja pasar lo verdadero. Sin una ordenación pedida, el orden de las filas no está especificado. No hay conversiones implícitas de tipos; las comparaciones entre tipos numéricos distintos que se admiten las fija 17.1. El texto se compara con la intercalación de su columna (IC1–IC3 de las tablas). Una consulta devuelve un **resultado admisible** bajo cualquier plan válido de esa misma sentencia, con la regla del documento 10: siendo R el resultado completo sin `LIMIT` ni `OFFSET`, en el orden de su ordenación, sin ellos el resultado es R como multiconjunto, en el orden pedido si se pidió; con ellos y una ordenación, tiene el tamaño de la ventana de R, la misma secuencia de valores de las claves de ordenación que la ventana y es un submulticonjunto de R; con ellos y sin ordenación, tiene el tamaño de la ventana y es un submulticonjunto de R. Agrupar, quitar repetidos, las operaciones de conjuntos y las claves de un join comparan los valores como las claves de N5 de las tablas: dos nulos son iguales entre sí, `-0` es `+0`, dos NaN son iguales y el texto `ci` o `ci_ai` se compara por su forma plegada; qué valor del grupo se devuelve lo fija el PRD hijo de cada operación. Hay dos excepciones: un camino aproximado que la consulta permite expresamente (17.4) devuelve lo que fije su PRD hijo, y un presupuesto de CS8 que cuenta trabajo (las filas examinadas, el tiempo) puede agotarse con un plan y no con otro. |
| CS8 | **Límites** (P9 del documento 11): cada ejecución tiene presupuestos de memoria (64 MiB por defecto), de desbordamiento a disco (1 GiB por defecto), de filas examinadas y de tiempo (sin límite por defecto), y se puede cancelar. Los documentos tienen presupuestos de tamaño, profundidad, nodos y parámetros; la planificación, uno de trabajo, pasado el cual el planificador se queda con el mejor plan encontrado, que sigue siendo correcto, y la explicación lo dice; los valores, el de los elementos de una lista de `IN` (10.000 por defecto); los planes, el de sus intervalos (10.000 por defecto); la caché de planes del almacén, el de sus entradas (256 por defecto), que al llenarse descarta la menos usada; `k` no pasa de 10.000 y la anidación de un documento no pasa de 128, que son techos y no presupuestos (README de diseño, §2.5 del documento 11). Los presupuestos se configuran por almacén y por llamada. Los que protegen la memoria o el tamaño (los de los documentos, la planificación, la caché, las listas de `IN`, los intervalos, la memoria y el desbordamiento) tienen además un **máximo duro**: pedir más falla con un error que nombra el presupuesto y su máximo, nunca se recorta en silencio ni admite «sin límite». Krummi no reserva memoria en proporción a un tamaño que no ha comprobado. No hay un límite para todo el almacén: la memoria de las ejecuciones simultáneas es la suma de sus presupuestos, y la aplicación, que decide cuántas lanza a la vez, es su dueña. Un operador que puede desbordar a disco desborda antes de fallar por memoria; los demás fallan al agotarla. Una ejecución que agota su presupuesto falla sola; las demás siguen. |
| CS9 | **Estructuras temporales** (README de diseño, *Plans*): las estructuras temporales de una ejecución viven fuera del diario y no son durables. Lo que desborda a disco va sin cifrar a un nombre reservado de la carpeta del almacén, que la apertura borra, como `krummi.sort` (P3 del Pager), y que cubre el cifrado del sistema de archivos cuando lo hay (anotación del 2026-11-09 al padre); 17.1 fija si es el mismo nombre. Se borra cuando la ejecución termina, falla, se cancela o se suelta su cursor sin leerlo entero. Un fallo al desbordar (disco lleno, fallo de entrada y salida) hace fallar solo esa ejecución, con su clase (G7, G12), y no impide las escrituras del almacén, porque no es parte de él; tampoco cuenta para su tamaño máximo. |
| CS10 | **Errores** (D8 del padre, P10 del documento 11): una clase nueva, **documento no válido**, para un documento que no es JSON bien formado o no cumple la gramática, los miembros, los tipos o los invariantes de su tipo de documento y su versión, o usa una construcción quitada; su error lleva la ruta del fallo dentro del documento (un JSON Pointer, RFC 6901), lo esperado y lo encontrado, o la línea y la columna si no es JSON bien formado. Un documento o una sentencia cuyas identidades, nombres o marcas de estado no coinciden con el catálogo que ve la transacción falla con **esquema distinto** (N10 de las tablas), nombrando el objeto y las dos identidades. Un valor enlazado que no cumple la declaración de su parámetro falla con **valor no válido** (N10 de las tablas). Los errores de la ejecución llevan la ruta del operador que falló. El error es un mismo objeto en Rust, en C y en las herramientas. Las clases de un documento de versión posterior, de un parámetro sin valor, de un presupuesto superado y de la cancelación las fija 17.1. La validación de documentos y planes nunca termina en un pánico; la lectura de filas mal formadas conserva la excepción a G6 del padre (anotaciones del 2026-11-11 y del 2026-11-18). |
| CS11 | **Dependencias** (P11 del documento 11, bajo la regla de la anotación del 2026-11-10 a D12 del padre): solo el crate de las consultas depende de `serde`, `serde_derive` y `serde_json`, y de los crates que estos traen. Tienen código no seguro, pero son muy usados y mantenidos, solo Rust, y la alternativa segura, un lector propio de los documentos, sería código nuevo en una frontera de seguridad; el usuario lo prefirió así. Es distinto de lo que decidió 7.5 para el texto guardado en las columnas JSON, que lee un lector propio (JS14): aquí se leen los documentos de consulta, no los datos. Los crates del motor no dependen de ellos salvo en sus pruebas. Una comprobación automática compara el árbol de dependencias normales del crate de las consultas con la lista que fija 17.1 en la anotación a D12 y falla si aparece otro crate. Ningún crate del motor ni de las consultas depende de un servidor MCP, de un runtime asíncrono ni de un SDK de IA (U8). |
| CS12 | **Desarrollo asistido por IA** (P13 del documento 11): la biblioteca ofrece lo que las herramientas y los agentes que usan los desarrolladores necesitan, sin depender de ellos: el JSON Schema de cada tipo de documento; la descripción del esquema del almacén con sus identidades, marcas de estado y versiones de esquema (P12 del documento 11); validar un documento sin ejecutarlo y devolver su forma normalizada; la explicación; la ejecución acotada a un número de filas con una marca de que se cortó; comparar, en una misma instantánea, el resultado de una sentencia candidata con el de una consulta de referencia, con la regla de CS7 y uno de tres veredictos: admisible, no admisible o defecto del motor (cuando el resultado de la propia consulta no es admisible); el objeto de error de CS10, y una salida canónica para comparar (C1–C8 del documento 10). La línea de órdenes y la skill del documento 10 son adaptadores sobre esas funciones y se construyen con la herramienta de línea de comandos (módulo 12), no en este módulo, aunque el documento 10 ponía su fase 1 «con el módulo 17». Usuario: «En el módulo 12 (Recomendado)». El servidor MCP, que el documento 10 condiciona a que se demuestre su necesidad (fase 2), queda fuera de este módulo y se decidirá aparte. |
| CS13 | **Perfil de las consultas** (requisitos M1–M7 del documento de diseño 15): la explicación con ejecución informa, por operador, de las filas reales con una marca de completado, falsa cuando un operador superior lo detuvo antes (M1); cada ejecución cuenta sus páginas lógicas y su tiempo de CPU (M3); cada índice cuenta sus búsquedas, recorridos y filas leídas, y cada tabla sus inserciones, actualizaciones, borrados y filas cargadas en masa, con las entradas escritas en cada índice (M4); y una ejecución puede entregar su registro, con la identidad de su transacción, su número de orden dentro de ella y los valores de sus parámetros, a un receptor que da la aplicación; sin receptor no se construye nada (M6). La forma de una consulta y los bytes de un plan valen en las copias del almacén (M5, ya cumplido por P12 del documento 11). El planificador recibe como entradas los índices, las directivas y los planes aceptados (M2), y el catálogo reserva un sitio para directivas y planes aceptados (M7); las dos reservas las comprueba la revisión del diseño de 17.1, no una prueba. El resto de la etapa 1 del documento 15 lo construye 17.13, al final del módulo; el muestreo, el afinador y las directivas quedan fuera. Usuario: «PRD hijo al final del 17 (Recomendado)». |
| CS14 | **Medición**: Voron no tiene consultas, así que el umbral de K7 no se aplica a ellas. Cada PRD hijo mide antes y después de su cambio con la regla de N11 de las tablas y su anotación del 2026-11-20, en Windows y en Linux. Desde 17.1, el banco comparativo tiene cargas de **sobrecoste de las consultas**, cada una una sentencia preparada que hace lo mismo que una operación directa de las tablas (leer una fila por su clave primaria, recorrer un intervalo de la clave primaria, recorrer un intervalo de un índice), con el umbral de que la sentencia rinde al menos 0,80 veces la operación directa; un umbral incumplido sigue K13 del banco: no impide cerrar el PRD hijo y se convierte en un cambio aparte. Además, el informe de cierre de cada PRD hijo compara sus consultas con las mismas consultas en SQLite, en el mismo equipo y sin umbral, como 7.6 con PostgreSQL (VI15). SQLite entra solo en el banco, herramienta del mantenedor cuyas dependencias no llegan a la biblioteca (K14). Usuario: «API directa + SQLite (Recomendado)». |

### Invalidación de las sentencias

Lo que pasa al ejecutar una sentencia en una transacción que ve un catálogo distinto del de la sentencia (§2.2 del
documento 11):

| Lo que ve la transacción que ejecuta | Consulta | Plan |
|---|---|---|
| Una columna renombrada | Falla con esquema distinto: el nombre ya no resuelve | Falla con esquema distinto |
| Una columna que usa la sentencia, quitada | Falla con esquema distinto: el nombre ya no resuelve | Falla con esquema distinto |
| Una columna quitada y otra añadida con su nombre | Falla con esquema distinto: mismo nombre, otra identidad | Falla con esquema distinto |
| Otro cambio de esquema de N3 de las tablas | Se vuelve a enlazar y a planificar | Falla con esquema distinto, con la tabla, las dos versiones de esquema y las dos marcas |
| La tabla renombrada o borrada | Falla con esquema distinto: el nombre ya no resuelve | Falla con esquema distinto |
| La tabla borrada y creada con el mismo nombre | Falla con esquema distinto | Falla con esquema distinto |
| Un índice creado | Se vuelve a planificar | Se ejecuta sin usarlo |
| Un índice que usa el plan, borrado o creado de nuevo | Se vuelve a planificar | Falla con esquema distinto |
| Estadísticas reconstruidas | Se vuelve a planificar en la ejecución siguiente | Se ejecuta igual; solo cambian las estimaciones de la explicación |
| Una confirmación no durable perdida en una caída (G11) y después otros cambios | Falla con esquema distinto | Falla con esquema distinto |
| Una copia restaurada o compactada (módulos 10 y 11, cuando existan) | Igual que en el original hasta que uno de los dos cambia | Igual |

Vaciar la tabla o cambiar sus filas no cambia el catálogo: la sentencia se ejecuta igual.

Dentro de una escritura, preparar y explicar ven sus propios cambios sin confirmar, y cada cambio de esquema o de
índices de esa escritura cuenta como un catálogo nuevo. Un documento enlazado a un estado confirmado que la escritura no
cambió sigue valiendo en ella, después de confirmarla y después de abortarla; uno enlazado a un estado que solo creó esa
escritura vale en ella mientras dure ese estado, en las demás solo después de confirmarla, y falla con esquema distinto
en todas después de abortarla. Un plan que usa un índice falla en toda transacción que ya no ve el índice, y se ejecuta
en una que todavía ve el estado anterior.

## Actores

- **Desarrollador integrador**: escribe consultas con la API fluida o como documentos JSON, las prepara, las ejecuta
  con sus valores y lee sus filas por un cursor; pide la explicación de una sentencia y, si quiere fijar un plan, la
  prepara como plan.
- **Agente o herramienta**: genera documentos con los JSON Schema publicados, los valida, lee los errores por su ruta
  y corrige el documento (CS12).
- **Mantenedor**: ejecuta en Windows y en Linux las pruebas diferenciales, el fuzzing y las mediciones de CS14.

## Flujo

1. La aplicación construye una consulta con la API fluida o como documento JSON (CS1, CS3, CS4).
2. Dentro de una transacción la prepara: Krummi la valida, la enlaza al catálogo que ve la transacción y la planifica
   (CS5, CS10).
3. En cualquier transacción, la aplicación da los valores de sus parámetros y la ejecuta; recibe un cursor y lee sus
   filas por lotes (CS5, CS7, CS8).
4. Si quiere ver o fijar el plan, pide la explicación, la inspecciona o la cambia, y prepara su plan como sentencia
   (CS6).
5. Cuando la transacción que ejecuta ve otro catálogo, la sentencia se vuelve a enlazar o falla con esquema distinto
   ([invalidación](#invalidación-de-las-sentencias)).

## Reparto en PRD hijos

Cada submódulo es un PRD hijo de este, con su propia ejecución. El orden es el de construcción: cada uno se apoya en
los anteriores. La columna *Fija* nombra lo que ese PRD hijo debe decidir, además de su contenido y de sus propios
errores.

| # | PRD hijo | Contenido | Fija |
|---|---|---|---|
| 17.1 | **Consultas de una tabla** | Los documentos de consulta y de ejecución (CS3, CS4); la API fluida de Rust; preparar, enlazar, ejecutar y el cursor (CS5); filtros con `=`, `<`, `≤`, `>`, `≥`, entre, `IN`, es nulo y `AND`, `OR`, `NOT`; proyección; `ORDER BY`, `LIMIT` y `OFFSET`; `count(*)`, `count` de una columna, `min`, `max`, `sum` y `avg` sobre todas las filas filtradas; el planificador por coste, que lee por la clave primaria, por los índices (también solo sus entradas, sin leer la fila) o recorriendo la tabla entera, y puede combinar varios de esos caminos; la ordenación con desbordamiento a disco (CS9); los límites (CS8); los errores (CS10); las dependencias (CS11); la descripción del almacén y validar sin ejecutar (CS12); las reservas de CS13. | **Documentos**: los miembros y los nombres de las operaciones de la consulta y de la ejecución, desde §2.1 del documento 11, fijados con documentos de referencia; las formas escritas y aceptadas de cada tipo de valor, desde §2.4 del documento 11, también las de `json`, que §2.4 no tiene; la clase de un valor enlazado más grande que el máximo de su columna; la normalización de una consulta (por ejemplo, las listas de `IN` ordenadas y sin repetidos) y si la API fluida escribe la forma normalizada; si los JSON Schema se distribuyen también como archivos (pregunta 4 del documento 11). **Semántica**: qué comparaciones entre tipos numéricos distintos se admiten; si se admiten comparaciones, `IN` y `ORDER BY` sobre columnas JSON y vector; dónde van los nulos en `ORDER BY` por defecto; cómo tratan los nulos los agregados, el tipo del resultado de `sum` y `avg` de cada tipo y qué pasa al desbordarse; qué devuelven los agregados sobre cero filas; qué tipos admiten `min` y `max`; si se pueden combinar agregados con columnas sin agregar, con `ORDER BY` o con `LIMIT`; los nombres y el orden de las columnas del resultado, y si se puede ordenar por una columna que no se proyecta; el tipo de `LIMIT` y `OFFSET` y qué hacen un nulo, un valor negativo o uno muy grande en ellos. **Sentencias**: qué pasa con un nombre de tabla o columna que no existe; si una sentencia de un almacén se puede ejecutar en una transacción de otro; si cerrar el almacén espera también a los conjuntos de valores y a los cursores; si las consultas usan las proyecciones guardadas (*Fuera de alcance* de 7.1); qué presupuesto, tiempo o cancelación cubre la espera a un muestreo de ST3 al preparar, explicar o ejecutar, y con qué clases falla; qué ve una consulta dentro de una escritura que cambió filas antes de crear el cursor. **Cursor**: el tamaño de los lotes; si la ejecución empieza al crear el cursor o al leer, y por tanto cuándo aparecen los errores de la ejecución; qué devuelve después del final, de un error o de una cancelación. **Límites**: los valores por defecto que CS8 no fija y los máximos duros de cada presupuesto; qué cuentan las filas examinadas, si el tiempo es de reloj, si cuenta entre lecturas de lotes y cada cuánto se comprueba; cómo se cancela y desde qué hilo; dónde desborda el modo temporal (G9), que no crea archivos; qué hace la apertura con los archivos de desbordamiento que dejó una caída; si lo desbordado se comprueba al leerlo de vuelta (G6). **Errores**: las clases que CS10 le deja, los datos y la precedencia de cada error, y a qué documento apunta la ruta de un error de ejecución de una consulta. **Dependencias**: la lista de crates de la anotación a D12 (CS11). **Pruebas**: el número de casos y la semilla de AC-CS1, el tiempo del fuzzing de AC-CS3 y el gancho que simula versiones de AC-CS7. **Medición**: los datos de sus cargas de sobrecoste (número de filas, columnas, distribución de las claves), sus números y nombres y su lugar en el informe y en la ejecución reducida del banco (K4, K11); qué consultas compara con SQLite y cómo se configura SQLite (CS14). |
| 17.2 | **Planes y explicación** | El documento de plan y preparar un plan como sentencia (CS6); el documento de explicación, con las estimaciones, su confianza, el coste y si la planificación se quedó sin presupuesto, y, tras ejecutar dentro de los límites de la ejecución, las filas, los lotes, el tiempo, la memoria y el desbordamiento reales por operador; la explicación en texto para personas, solo de salida; la ejecución acotada y la comparación de CS12; los contadores y el receptor de CS13. | Los miembros y los operadores del documento de plan, desde el documento 8 y §2.1 del 11; qué validaciones tiene cada operador y sus datos de error; la forma de la explicación en texto; cómo lee la aplicación los contadores de M4, si sobreviven a reabrir el almacén y si se pueden poner a cero; dónde se registra el receptor (por almacén, por sentencia o por ejecución), si se llama en el hilo de la ejecución y qué pasa si falla o tarda; qué operadores pueden informar un completado falso; la forma del registro que recibe el receptor; el efecto de los contadores de M4 en las cargas de las tablas frente a Voron (K7), que mide con la regla de N11. |
| 17.3 | **Condiciones sobre texto y JSON** | Prefijo de texto y `LIKE` con la intercalación de la columna (IC3 de las tablas); rutas dentro de un JSON como listas de pasos, cada uno un nombre de propiedad o una posición de array, sin un texto que analizar, donde un paso que espera un array o un objeto y encuentra otra cosa da «no está»; su existencia, su igualdad y su lectura tipada, también como fecha e instante, en filtros y proyecciones; «alguno» y «todos» sobre los elementos de un array con una condición sobre sus propiedades, cumplida por un mismo elemento, donde «todos» es verdadero en un array vacío y un elemento cuya condición es desconocida no cuenta para «alguno» y hace desconocido «todos» salvo que otro elemento la haga falsa; el índice multientrada sobre varias propiedades de un mismo elemento (documento de diseño 14); una carga del banco que filtra por una ruta, con la que se decide si leer una propiedad sin recorrer el texto del documento (*Fuera de alcance* de 7.5). Requisitos aprobados en las tablas (anotación del 2026-11-12, «sí»). | Los nombres de las operaciones de texto con y sin distinción de mayúsculas (CS3); qué hace `LIKE` con los caracteres de escape; si la igualdad de JSON compara por significado o por texto; si las rutas de JSON tienen estadísticas y qué estima el planificador sin ellas; qué hace el índice multientrada con una entrada demasiado larga o con un valor de otro tipo en la ruta, y si se mantiene en la misma escritura (preguntas 1, 2 y 5 del documento 14); los datos de su carga del banco. |
| 17.4 | **Vectores en las consultas** | La distancia como operando: umbral de distancia, ordenar por distancia, proyectar la distancia y los `k` más cercanos dentro de una consulta con cualquier filtro de 17.1 y 17.3, con la búsqueda exacta o aproximada de 7.6 como camino del planificador. Las distancias son idénticas bit a bit en toda plataforma, así que un umbral devuelve las mismas filas con cualquier plan, y los empates de los `k` más cercanos se rompen por la clave primaria (README de diseño). | Cuándo puede el planificador usar el índice aproximado, cómo lo pide o lo permite la consulta y qué resultado se le exige (CS7), también cuando la búsqueda de VI6 pasa sola a aproximada; qué parte del filtro se aplica antes de buscar (las condiciones de VI7) y cuál después; qué hace con una sentencia un cambio de estado del índice aproximado; cómo lo informa la explicación. |
| 17.5 | **Expresiones y funciones escalares** | Aritmética, `CASE`, `COALESCE`, conversiones explícitas de tipo y funciones de texto, de números y de fechas, en filtros, proyecciones y ordenaciones; comparaciones entre columnas. | La lista de funciones y sus errores (división por cero, desbordamiento, conversión imposible); si una expresión puede usar un índice. |
| 17.6 | **Índices parciales y sobre expresiones** | Índices de 7.2 con una condición de 17.1 o 17.3 (solo tienen las filas que la cumplen) y con columnas que son expresiones de 17.5 o rutas de un JSON de 17.3, mantenidos en cada escritura; el planificador usa un índice parcial solo cuando la condición de la consulta implica la del índice, y uno sobre una expresión cuando la consulta usa la misma expresión. | Qué otras clases de índice sobre JSON del documento de diseño 14 entran (por ejemplo, los invertidos), si alguna; qué condiciones y expresiones admite un índice (por ejemplo, solo las deterministas); cómo se declaran en la definición del índice que reservó 7.2 (N7 de las tablas); qué hace la unicidad de un índice parcial único; con qué clase falla una definición no válida; si tienen estadísticas propias. |
| 17.7 | **Escrituras por consulta** | `UPDATE` con filtro y con `SET` de expresiones de 17.5, `DELETE` con filtro e `INSERT` desde una consulta, solo en transacciones de escritura, con las reglas de las escrituras de 7.1 y 7.2 (validación de cada valor, índices al día, clave duplicada); cada fila se cambia una sola vez, aunque el cambio la mueva dentro del camino por el que se recorre; una sentencia que falla con un error de validación pública (B7 de los árboles: valor no válido, clave duplicada, tamaño no válido) no deja ninguno de sus cambios y la transacción puede confirmar los demás; un fallo que T11 de las transacciones hace fatal para la transacción (disco lleno, tamaño máximo, memoria agotada, daño, entrada y salida) sigue siéndolo, y T12 sigue valiendo para el almacén. | Si los fallos propios de una sentencia de escritura (un presupuesto de CS8 superado, una cancelación) dejan la transacción confirmable sin los cambios de la sentencia; qué informa al terminar (por ejemplo, cuántas filas cambió) y si puede devolver las filas cambiadas; si hay `INSERT` que actualiza cuando la clave ya existe; si un `UPDATE` puede cambiar la clave primaria; sus operadores de plan y su explicación (CS2). Sus cambios cuentan para el umbral de las estadísticas como las inserciones, actualizaciones y borrados de 7.1 (ST4). |
| 17.8 | **Agrupación** | `GROUP BY`, `HAVING`, `DISTINCT` y los agregados por grupo, con desbordamiento a disco (CS9). | Los agregados que se añaden (por ejemplo `count(distinct)`), el orden de los grupos sin `ORDER BY`, qué valor devuelve un grupo de texto `ci` o `ci_ai` (CS7) y el presupuesto de la planificación. |
| 17.9 | **Joins** | Inner, left, right y full outer, semi y anti join entre tablas, con bucle anidado por clave, hash y merge join; el orden de los joins elegido por coste. | El presupuesto de enumeración de órdenes de join (§2.5 del documento 11); cómo se nombran las columnas de varias tablas en un documento. |
| 17.10 | **Subconsultas y CTE** | Subconsultas escalares, `EXISTS`, `IN` y `NOT IN` con subconsulta, y subconsultas correlacionadas; `WITH`, incluido el recursivo. | Qué subconsultas se reescriben como joins; el límite de la recursión. |
| 17.11 | **Operaciones de conjuntos** | `UNION`, `UNION ALL`, `INTERSECT` y `EXCEPT`. | Los nombres y los tipos de las columnas del resultado cuando las dos entradas difieren. |
| 17.12 | **Funciones de ventana** | `ROW_NUMBER`, `RANK`, `DENSE_RANK`, `LAG`, `LEAD` y los agregados con `OVER`, con particiones, orden y marcos. | Los marcos admitidos y la lista de funciones. |
| 17.13 | **Perfil de las consultas** | La etapa 1 del documento de diseño 15: los agregados exactos del perfil por forma de consulta, guardados aparte de los datos del almacén, y forzar el último plan bueno de una forma cuando su plan cambia y rinde peor. | Las nueve preguntas de producto abiertas del documento 15 que toquen la etapa 1; dónde se guarda el perfil (por ejemplo, un nombre reservado de la carpeta del almacén, P3 del Pager) y qué pasa con él al copiar, compactar o borrar el almacén; cómo se detecta una regresión y cómo se deshace el plan forzado. |

## Validaciones y mensajes

Valen las de los ascendientes. Este módulo añade estas situaciones, solo de la biblioteca:

| Situación | Clase de error |
|---|---|
| Un documento que no es JSON bien formado, o que no cumple la gramática, los miembros, los tipos o los invariantes de su tipo y su versión, o usa una construcción quitada | Documento no válido (nueva), con ruta, lo esperado y lo encontrado (CS10) |
| Un documento de una versión posterior a la del lector | La que fije 17.1 (CS3) |
| Un documento o una sentencia cuyas identidades, nombres o marcas de estado no coinciden con el catálogo que ve la transacción | Esquema distinto (N10 de las tablas), con el objeto y las dos identidades |
| Un valor enlazado que no cumple el tipo declarado de su parámetro | Valor no válido (N10 de las tablas) |
| Ejecutar con un parámetro sin valor | La que fije 17.1 (CS5) |
| Un presupuesto pedido por encima de su máximo duro | La que fije 17.1, con el presupuesto y su máximo (CS8) |
| Una ejecución que supera su memoria, su desbordamiento, sus filas examinadas o su tiempo, o que se cancela | Las que fije 17.1 (CS8) |
| Cerrar el almacén con sentencias vivas | Uso incorrecto (CS5) |

Cada PRD hijo añade las situaciones de sus operaciones y fija los datos de cada error.

## Salida

El módulo no escribe nada en pantalla. Un cursor devuelve las filas por lotes. Los valores leídos de las filas y de
las entradas de los índices siguen las reglas de lectura de las tablas (D11 y sus anotaciones del padre: sin copia los
de `string`, `bytes`, vector y JSON de una fila, con las excepciones de las cadenas de JSON con escapes y de las claves
de las entradas de índice). Los valores que la ejecución calcula (agregados, expresiones, distancias) y los que pasaron
por una estructura temporal desbordada a disco (CS9) se devuelven en una copia propia de la aplicación. Los documentos
que escribe Krummi (las explicaciones, los planes y las consultas normalizadas) tienen una sola forma determinista: el
mismo documento se escribe siempre con los mismos bytes (CS4). Las salidas exactas las fija cada PRD hijo con sus
documentos de referencia.

## Reutilización

- Las lecturas de las tablas como caminos de acceso: por clave primaria, por intervalos de la clave y de los índices,
  las entradas de los índices y las proyecciones (7.1, 7.2).
- La estimación de las estadísticas para el planificador (N8 de las tablas, 7.3), como fijó 7.3 («Planificador de las
  consultas (módulo 17): usará la misma estimación»).
- La búsqueda de vecinos y su filtro (7.6), la navegación y la lectura tipada del JSON (7.5) y la forma plegada del
  texto (7.7).
- Las identidades aleatorias y las marcas de estado del catálogo, y la descripción de las tablas (P12, 7.1, 7.2).

## Fuera de alcance

- Un lenguaje de consulta de texto y SQL, un servidor de red y un protocolo remoto (anotación del 2026-11-10 al padre).
- La interfaz C y el binding C# (módulo 18), salvo que sus documentos son los de este módulo.
- La línea de órdenes, la skill y el servidor MCP del documento 10: módulo 12 (CS12).
- El muestreo de ejecuciones, el afinador y las directivas de estimación del documento de diseño 15; crear índices
  automáticamente.
- El servidor MCP del documento 10 (CS12).
- Vistas, vistas materializadas, disparadores, procedimientos almacenados y claves foráneas.
- Paginación por clave (*keyset*), indicaciones al planificador (*hints*) y forzar un camino de otra forma que
  preparando un plan, salvo el último plan bueno de 17.13; expresiones regulares.
- Funciones definidas por la aplicación y funciones de zonas horarias (las tablas no guardan zonas horarias).
- Cursores que retroceden o se reposicionan.
- Insertar filas dadas en el propio documento (las inserta la aplicación con 7.1), guiones de varias sentencias y
  puntos de guardado.
- Cancelar o limitar el tiempo de preparar y de explicar, salvo lo que fije 17.1 sobre la espera a un muestreo.
- Un traductor desde o hacia Substrait (documento de diseño 8).
- Guardar sentencias o la caché de planes (U4).
- Ejecutar una consulta en varios hilos a la vez (README de diseño: «one thread per query in the first version»).
- Búsqueda de texto completo e índices de texto (funciones de RavenDB, fuera de alcance del padre).
- Ajustar las estimaciones con lo que devuelven las consultas, como el *CE feedback* de SQL Server, que 7.3 dejaba
  al módulo 17: queda con las directivas de estimación del documento 15, fuera de este módulo.
- Consultas que abarcan varios almacenes (fuera de alcance del padre).

## Dejado a la implementación

- El modelo de coste, el algoritmo del planificador, cuándo unos valores piden otro plan (con la política del documento 4
  como punto de partida, CS5), la organización de la caché
  de planes salvo su presupuesto (CS8), y la representación interna de los planes, siempre que cumplan este PRD y los
  de sus hijos. Los documentos de diseño 4, 8 y 11 fijan su punto de partida.
- Los nombres de las operaciones públicas de Rust, de sus tipos y de las clases de error nuevas, que siguen las
  convenciones de Rust y están en inglés (D8 del padre). Los nombres dentro de los documentos JSON sí son contrato
  público y los fija cada PRD hijo (CS3).

## Criterios de aceptación

Valen los de los ascendientes. Cada criterio lo comprueba el PRD hijo que se nombra y lo repiten los siguientes con
los operadores que añaden.

1. **AC-CS1** — Dadas consultas generadas al azar, con la semilla y el número de casos que fije 17.1, sobre esquemas y
   filas generados al azar, cuando se ejecutan con el plan que elige el planificador y con cada alternativa que
   consideró, que expone un gancho de pruebas, entonces cada resultado es admisible (CS7) frente al de un plan de
   referencia que recorre la tabla entera, filtra, ordena y proyecta o agrega, sobre la misma instantánea; una consulta
   generada que necesita un resultado exacto añade la clave primaria a sus claves de ordenación (17.1 y cada PRD hijo
   con sus operadores; 17.4 con la regla de su camino aproximado).
2. **AC-CS2** — Dado un documento de cada tipo que escribió Krummi, cuando se lee y se vuelve a escribir, entonces los
   bytes y los valores tipados son idénticos; cuando se prepara una explicación, entonces falla con documento no
   válido; cuando se prepara su plan en la misma transacción y se ejecuta con los mismos valores, entonces devuelve
   un resultado admisible para la sentencia de la que salió; y cuando una consulta se construye con la API fluida y
   la misma se escribe como JSON, entonces los dos documentos son idénticos byte a byte (CS1, CS4, CS6) (17.1 para
   consultas y ejecuciones, 17.2 para planes y explicaciones).
3. **AC-CS3** — Dados documentos y planes mutados (sin miembros, con miembros desconocidos o repetidos, con operadores
   intercambiados, intervalos quitados o tipos cambiados), y la validación sometida a fuzzing durante el tiempo que
   fije 17.1, cuando se preparan y se ejecutan, entonces ninguno termina en un pánico; cada uno que falla lo hace con
   una clase de CS8, de CS10 o de los ascendientes (daño, entrada y salida, disco lleno), y los demás se ejecutan
   hasta el final del cursor (CS6, CS10) (17.1, 17.2 y cada PRD hijo).
4. **AC-CS4** — Dada una sentencia preparada, cuando la transacción que la ejecuta ve cada caso de la
   [tabla de invalidación](#invalidación-de-las-sentencias), entonces la consulta y el plan hacen lo que dice la tabla,
   y una nueva planificación se ve en la explicación de la ejecución siguiente; el caso de la confirmación perdida se
   prueba con una caída simulada, y el de las copias, cuando existan los módulos 10 y 11, en ellos; y cuando una
   escritura prepara una sentencia y después cambia el esquema, confirma o aborta, entonces cada transacción la acepta
   o la rechaza como dice el párrafo que sigue a la tabla, incluida una lectura abierta antes de la confirmación, que
   sigue ejecutando el plan (CS5) (17.1 para las consultas, 17.2 para los planes).
5. **AC-CS5** — Dada una sentencia, cuando 16 hilos la ejecutan a la vez con sus propios valores y cursores, en
   transacciones de lectura distintas, entonces cada cursor devuelve un resultado admisible para su instantánea;
   cuando se cambian los valores después de ejecutar, entonces el cursor abierto no cambia; cuando una de las
   ejecuciones agota su memoria, entonces falla sola y las demás terminan; y cuando se ejecuta con un parámetro sin
   valor, entonces falla con la clase que fije 17.1 (CS5, CS8) (17.1).
6. **AC-CS6** — Dado cada presupuesto de CS8 que hace fallar (los de los documentos, las listas de `IN`, los
   intervalos, la memoria de un operador que no desborda, el desbordamiento y las filas examinadas), cuando un
   documento o una ejecución llega a su límite y cuando lo pasa por uno, entonces la primera funciona y la segunda
   falla con la clase que fije 17.1, sin que un asignador de memoria de pruebas que cuenta las reservas registre una
   proporcional a lo pedido antes de la comprobación; cuando la caché de planes está llena y se prepara otra
   sentencia, entonces se descarta la entrada menos usada y las dos sentencias siguen dando resultados admisibles;
   cuando el tiempo límite de una ejecución ya pasó o se cancela, entonces su cursor termina con la clase que fije
   17.1; cuando se pide un presupuesto por encima de su máximo duro, entonces falla nombrando el presupuesto y su
   máximo; y cuando una ejecución que desbordó a disco termina, falla, se cancela o se suelta su cursor a medias,
   entonces no quedan sus archivos en la carpeta del almacén (CS8, CS9) (17.1).
7. **AC-CS7** — Dados documentos no válidos, al menos uno por cada causa de CS10 (no JSON, un miembro repetido, uno
   desconocido, una operación desconocida, un tipo que no casa, un valor en una forma no aceptada, una construcción
   quitada), cuando se preparan, entonces fallan con documento no válido con la ruta, lo esperado y lo encontrado que
   fija 17.1; un error de ejecución lleva la ruta del operador que falló; y, con el gancho que simula versiones del
   lector, un documento de una versión anterior se acepta con su significado, uno con una construcción quitada falla
   nombrándola con la versión que la quitó, y uno de una versión posterior falla con la clase que fije 17.1 (CS3,
   CS10) (17.1).
8. **AC-CS8** — Dadas filas con nulos en las columnas de una consulta, cuando se filtran con cada operador de
   comparación, `IN`, `AND`, `OR` y `NOT`, entonces solo pasan las filas cuya condición es verdadera según la lógica de
   tres valores (CS7) (17.1 y cada PRD hijo con sus operadores).
9. **AC-CS9** — Dado el crate de las consultas, cuando se compila en la integración continua, entonces la comprobación
   de CS11 falla si su árbol de dependencias normales tiene un crate que no está en la lista de la anotación a D12, si
   un crate del motor tiene `serde` entre sus dependencias normales, o si algún crate del motor o de las consultas
   depende de un servidor MCP, un runtime asíncrono o un SDK de IA (17.1).
10. **AC-CS10** — Dado un cursor de una consulta, cuando devuelve valores de `string`, `bytes`, vector o JSON leídos de
    las filas sin pasar por una estructura desbordada a disco, entonces apuntan a memoria del motor y no a una copia, y
    usarlos después de cerrar la transacción no compila (AC20 del padre); cuando devuelve valores leídos de las claves
    de una entrada de índice, agregados o distancias, entonces son copias propias de la aplicación; un cursor de una
    escritura que se usa después de un cambio de esa escritura no compila; y una consulta dentro de una escritura ve lo
    que fije 17.1 (CS1, CS5, *Salida*) (17.1).
11. **AC-CS11** — Dado un almacén con sentencias vivas, cuando se cierra, entonces falla con uso incorrecto y el almacén
    sigue abierto (CS5) (17.1).
12. **AC-CS12** — Dados los valores de ejemplo de cada forma que fija 17.1 (las escritas, las aceptadas y las
    rechazadas de cada tipo de valor), los extremos de las fechas e instantes del dominio de 7.1, todos los `f32` y 10⁸
    `f64` al azar, cuando se escriben y se leen, entonces cada valor aceptado da un único valor tipado, cada rechazado
    falla con su clase, y cada valor escrito por Krummi se lee igual y se vuelve a escribir con los mismos bytes (CS4)
    (17.1).
13. **AC-CS13** — Dada una consulta guardada con las identidades que escribió Krummi, cuando se quita su columna y se
    añade otra con el mismo nombre y se prepara, entonces falla con esquema distinto; y la misma consulta sin
    identidades se enlaza a la columna nueva (CS5) (17.1).
14. **AC-CS14** — Dados los documentos de referencia de cada tipo, cuando se validan con su JSON Schema publicado,
    entonces todos pasan; la descripción del almacén nombra cada tabla, columna e índice con su identidad y su versión
    de esquema; validar sin ejecutar una consulta devuelve la misma consulta normalizada que lleva su explicación; la
    ejecución acotada a N filas devuelve N filas y la marca de corte cuando hay más, y todas sin la marca cuando no; y
    la comparación da admisible para un plan por índice de una consulta, no admisible para uno que quita su filtro, y
    defecto del motor cuando un gancho de pruebas altera el resultado de la propia consulta (CS12) (17.1 la
    descripción y la validación, 17.2 lo demás).
15. **AC-CS15** — Dada una planificación con un presupuesto de una sola operación de estimación, cuando se planifica y
    se ejecuta, entonces el resultado es admisible y la explicación dice que la planificación se quedó sin presupuesto
    (CS8) (17.2).
16. **AC-CS16** — Dada una ejecución con explicación, cuando un `LIMIT` detiene a un operador, entonces ese operador
    informa sus filas reales con la marca de completado falsa, y los que terminan la llevan verdadera; cuando una
    prueba hace un número conocido de búsquedas y recorridos por un índice y de inserciones, actualizaciones, borrados
    y filas cargadas en masa en una tabla, entonces los contadores de M4 dan esos números; y sin receptor no se
    construye ningún registro, según el gancho de pruebas que fije 17.2, mientras que con él cada ejecución entrega
    uno con su transacción, su número de orden y sus valores (CS13) (17.2).
17. **AC-CS17** — Dadas las cargas de sobrecoste de CS14, cuando se ejecuta el banco completo en Windows y en Linux,
    entonces el informe muestra para cada una la razón entre la sentencia y su operación directa y si llega a 0,80, y
    la orden termina con `1` si alguna no llega (K13); y el informe de cierre de cada PRD hijo incluye las mediciones
    antes y después de N11 y la comparación con SQLite (CS14) (17.1 y cada PRD hijo).

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones del 2026-11-28 (en sus
traducciones también):

- Padre raíz, objetivos: se añade el objetivo 4, **desarrollo asistido por IA**, acotado a los agentes y herramientas
  que usan los desarrolladores: la biblioteca les ofrece los esquemas de sus documentos, la descripción del almacén,
  la explicación y errores con ruta, sin depender de ningún servidor MCP ni SDK de IA (P13 del documento de diseño 11,
  aprobada el 2026-11-10; CS12).
- Padre raíz, reparto: la fila del módulo 17 (anotación del 2026-11-10) añade las sentencias preparadas y los
  documentos con sus esquemas (P2, P8), se reparte en los PRD hijos 17.1 a 17.13 de este documento y su objetivo es la
  mayor parte de las consultas del SQL estándar (CS2); el módulo 18 se construye cuando el 17 está cerrado entero; el
  módulo 12 construye también la línea de órdenes para agentes, con `--json`, y la skill del documento de diseño 10,
  sobre las funciones de CS12.
- Padre raíz, D2: la superficie pública añade los documentos de consulta, plan, ejecución y explicación y sus JSON
  Schema (CS3).
- Padre raíz, D5: el banco compara también con SQLite, que no es un motor de Rust, solo como información en los
  informes de cierre del módulo 17, y las cargas de sobrecoste de las consultas tienen umbral frente a las operaciones
  directas de Krummi (CS14).
- Padre raíz, D7: los documentos tienen su propio versionado, aparte del formato del almacén (CS3).
- Padre raíz, D8 y *Validaciones y mensajes*: nueva clase de error documento no válido (CS10).
- Padre raíz, D11 y AC20: los valores que una consulta calcula y los que pasaron por una estructura desbordada a disco
  se devuelven en una copia propia de la aplicación (*Salida*).
- Padre raíz, D12: bajo la regla de la anotación del 2026-11-10, se admiten `serde`, `serde_derive` y `serde_json`, y
  los crates que traen según la lista que fije 17.1, solo en el crate de las consultas, con el motivo de CS11.
- Padre raíz, *Fuera de alcance*: las dependencias de servidores MCP, runtimes asíncronos y SDK de IA quedan fuera de
  la biblioteca (CS11).
- Banco, K7 y *Salida*: tienen umbral también las cargas de sobrecoste de las consultas (CS14), frente a la operación
  directa de las tablas y no frente a Voron; el informe las cuenta con los demás umbrales y K13 vale para ellas. K2 y
  K14: SQLite entra en el banco solo para la comparación sin umbral de los informes de cierre del módulo 17.
- Pager, P3: el desbordamiento de las consultas usa un nombre reservado de la carpeta del almacén que la apertura borra,
  `krummi.sort` u otro que fije 17.1 (CS9).
- Tablas, N10: esquema distinto cubre también los documentos y sentencias cuyas identidades no coinciden con el
  catálogo, y valor no válido también los valores enlazados que no cumplen la declaración de su parámetro (CS10).
- Tablas, N7: los índices parciales reservados los construye 17.6, con los índices sobre expresiones. *Fuera de
  alcance* de 7.5, «índices sobre expresiones, después de 7.2 y del módulo 17»: entran en 17.6.
- Estadísticas, *Fuera de alcance* «Ajustar las estimaciones con lo que devuelven las consultas [...]: módulo 17»: queda
  fuera del módulo 17, con las directivas de estimación del documento de diseño 15.
- Proyecciones, «las consultas del módulo 17 fijarán si las usan»: lo fija 17.1.
- Intercalación, *Fuera de alcance* «Una operación de búsqueda por prefijo de texto, `LIKE` o «empieza por»: módulo
  17»: la cubre 17.3.
- Columnas JSON, requisitos para el módulo 17 (rutas, «alguno» y «todos», índice multientrada, lectura tipada, carga del
  banco que filtra por una ruta): los cubre 17.3; qué otras clases de índice sobre JSON entran, 17.6.
- Índices vectoriales, *Fuera de alcance* «Filtrar por un umbral de distancia»: lo cubre 17.4.

## Referencias

- Padre: [krummi.prd.md](../krummi.prd.md) v1.1 (D2, D5, D7, D8, D11, D12, G3, G6, G7, G9, G11, G12, AC20, reparto y
  anotaciones del 2026-11-10)
- Tablas: [tables.prd.md](../tables/tables.prd.md) (N3, N5, N7, N8, N10, N11) y sus hijos, en especial
  [tables-schema.prd.md](../tables/tables-schema.prd.md), [tables-indexes.prd.md](../tables/tables-indexes.prd.md),
  [tables-projection.prd.md](../tables/tables-projection.prd.md),
  [tables-statistics.prd.md](../tables/tables-statistics.prd.md), [tables-json.prd.md](../tables/tables-json.prd.md),
  [tables-collation.prd.md](../tables/tables-collation.prd.md) y
  [tables-vector-indexes.prd.md](../tables/tables-vector-indexes.prd.md)
- Pager: [pager.prd.md](../pager/pager.prd.md) (P3); transacciones: [transactions.prd.md](../transactions/transactions.prd.md) (T6, T11, T12)
- Banco: [benchmark.prd.md](../benchmark/benchmark.prd.md) (K2, K4, K7, K11, K13, K14)
- Diseño: [docs/design/tables/README.md](../../../design/tables/README.md) y sus documentos 4, 8, 9, 10, 11, 13, 14 y 15

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 77da4411 | 2026-11-28 09:18 UTC+00:00 | 1 h 31 min | 24 min | 1 h 7 min | 47 min | 20 min | 10 |

<!-- prd-time: [{"session": "77da4411-edaa-49ca-96e6-3afe464b2b23", "start": "2026-11-28T09:18:54.972000+00:00", "wall": 5486, "discarded": 1465, "agent": 2807, "human": 1213, "interventions": 10, "idle_minutes": 10}] -->
