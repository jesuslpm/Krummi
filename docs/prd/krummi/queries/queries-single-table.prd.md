# Consultas de una tabla — PRD

**Versión**: 1.0
**Fecha**: 2026-11-28
**Documento padre**: [Consultas de Krummi — PRD](queries.prd.md) (v1.0)

## Objetivo y contexto

Construir **17.1, consultas de una tabla**, el primer PRD hijo del [reparto de las consultas](queries.prd.md#reparto-en-prd-hijos):
una aplicación describe en un documento JSON, o con una API fluida de Rust, qué filas de una tabla quiere, filtradas,
ordenadas, limitadas, con algunas de sus columnas o con agregados sobre todas las filas filtradas; Krummi lo valida, lo
planifica por coste sobre la clave primaria, los índices o el recorrido completo, y devuelve las filas por un cursor.
Es la primera vez que una aplicación de Krummi lee sin decir cómo leer.

Este PRD hereda todo lo de sus ascendientes ([queries.prd.md](queries.prd.md) y [krummi.prd.md](../krummi.prd.md)) y
fija lo que la celda *Fija* de 17.1 le encarga. El estado de partida técnico son los documentos de diseño 4, 8, 9 y 11
([README](../../../design/tables/README.md)); cuando difieren, gana el 11, salvo donde este PRD declara otra cosa
(CU24). Lo que cambia o precisa reglas anteriores está anotado (ver [Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

### Decisiones de producto

| ID | Decisión |
|---|---|
| CU1 | **Superficie**: en Rust, en cualquier transacción, de lectura o de escritura (CS1). Una consulta lee una sola tabla. La API fluida produce un documento de consulta sin `bind`, igual, byte a byte, al que escribe Krummi para la misma consulta escrita en JSON (AC-CS2): cada constante que recibe es un parámetro propio, sin nombre, del tipo de Rust de la constante (un `i32` da un `i32`, un texto da un `string`), numerado en el orden en que aparece al construir. Además de preparar y ejecutar, 17.1 ofrece la descripción del almacén, validar sin ejecutar, reescribir un documento en la forma de Krummi sin normalizarlo y el JSON Schema de cada tipo de documento (CU20). Los planes, la explicación, la ejecución acotada y la comparación llegan con 17.2. |
| CU2 | **Documento de consulta** (`query/1`, CS3): un objeto con `krummi`, `table` (el nombre de la tabla), `bind` (opcional, CU5), `params` (las declaraciones de los parámetros, en orden; obligatorio, también vacío) y `root` (el nodo raíz). Los nodos forman una cadena unida por `input`, de abajo arriba: `scan` (sin más miembros); `filter` (`predicate`), opcional; y después **o bien** `aggregate` (`aggregates`), que termina la cadena, **o bien**, en este orden y cada uno opcional, `sort` (`keys`), `limit` (`count` y `offset`, opcionales los dos pero al menos uno) y `project` (`columns`). Una cadena en otro orden, con un nodo repetido, o con `aggregate` y alguno de `sort`, `limit` o `project` es un documento no válido. Sin `project` ni `aggregate`, el resultado tiene todas las columnas de la tabla en el orden de su esquema. Un documento no lleva valores literales: toda constante es un parámetro (P15). |
| CU3 | **Expresiones** del predicado de `filter`: `eq`, `lt`, `le`, `gt` y `ge`, con `col` (un nombre de columna) y `value` (`{"param": i}`); `in`, con `col` y `values` (`{"param": i}` de un parámetro de tipo lista); `is_null`, con `col`; `and` y `or`, con `args` (dos o más expresiones); y `not`, con `arg`. No hay `ne` ni `between` en el documento: la API fluida los escribe como `not` de `eq` y como `and` de `ge` y `le`. Una comparación es siempre de una columna con un parámetro; entre columnas llega con 17.5. |
| CU4 | **Documento de ejecución** (`execution/1`, CS3): `krummi`, `statement` (un documento de consulta), `args` (un valor por declaración, en su orden, en la forma de CU6; tantos como declaraciones) y `limits` (opcional), un objeto con los presupuestos de ejecución de CU16 por su nombre (`memory_bytes`, `spill_bytes`, `rows_examined`, `deadline_ms`, `in_list`), cada uno un `u64` en su forma de texto. Un presupuesto que falta toma el del almacén; uno en otra forma o por encima de su máximo duro falla con opciones no válidas. Los valores de `args` se comprueban después de validar `statement`: uno en otra forma, también de otra clase JSON, falla con valor no válido y la ruta `/args/i`. |
| CU5 | **Parámetros y `bind`**: una declaración es `{"type": ...}` con un `name` opcional, único en el documento y que solo aparece en los errores. Los tipos son los de N4 salvo vector y JSON, con su descriptor (`{"type":"decimal","precision":12,"scale":2}`, `{"type":"uuid"}`), y la lista para `in` (`{"type":"list","element":{...}}`, con un elemento que no es lista). Un parámetro, o el elemento de una lista de `in`, casa con una columna de su mismo tipo y, entre números, con cualquier columna de su familia: un entero con cualquier columna entera, un `f32` o `f64` con cualquier columna de coma flotante, y un `decimal` con cualquier `decimal`; la comparación es por el valor exacto, sin convertir ni redondear (300 contra una columna `u8` es mayor que todas sus filas). Los demás tipos casan solo con el suyo: un `string` con cualquier intercalación, un `uuid` con cualquier orden. Usuario: «Por familia, exacto (Recomendado)». `count` y `offset` son parámetros `u64`. Un parámetro declarado y no usado es un documento no válido. `bind` es `{"table": <identidad>, "columns": {<nombre>: <identidad>, ...}}`, con la identidad de la tabla y la de cada columna que usa la consulta, todas como 16 dígitos hexadecimales en minúscula; Krummi escribe `columns` en el orden de los bytes de sus nombres; lo escriben validar sin ejecutar (CU20) y la explicación de 17.2, no la API fluida. |
| CU6 | **Formas de los valores** (§2.4 del documento 11): la [tabla de formas](#formas-de-los-valores) dice la forma que Krummi escribe, las que acepta y las que rechaza. Son estrictas: un parámetro con más decimales que su escala, o un instante con más de seis decimales, se rechaza en lugar de redondearse como al guardar (E8), porque un documento tiene una sola forma por valor. Un valor en otra forma, o fuera del dominio de E8 de su tipo, falla con valor no válido. Un valor de cualquier tamaño válido para su tipo se acepta, también uno que no cabría en una clave de índice: el resultado es el mismo con cualquier plan (CS7). |
| CU7 | **Normalización** (documento 4): al preparar, Krummi aplana los `and` y `or` anidados, lleva cada `not` hacia dentro por las leyes de De Morgan hasta una comparación de orden, que invierte (`not` de `lt` es `ge`, y así), o hasta un `eq`, un `in` o un `is_null`, sobre los que se queda, quita las dobles negaciones, quita los argumentos repetidos de un `and` o un `or`, sustituye por su argumento un `and` o un `or` que se queda con uno solo, y ordena los argumentos: primero las comparaciones, `in` e `is_null` (también bajo un `not`), por el nombre de su columna (sus bytes), después por la operación en el orden `eq`, `lt`, `le`, `gt`, `ge`, `in`, `is_null`, con su `not` justo detrás, y después por el número de parámetro; luego los `and` y `or` anidados, `and` antes que `or`, y entre dos de la misma operación comparando sus argumentos ya ordenados uno a uno con estas mismas reglas, hasta el primero que difiere; si uno es un prefijo del otro, va antes el más corto; nunca reescribe el predicado en forma normal disyuntiva ni conjuntiva. La forma normalizada es la que devuelve validar sin ejecutar y la que llevará la explicación (17.2); la API fluida escribe el documento tal como se construyó (cambia el documento 4, que la hacía normalizar). Al ejecutar, una lista de `in` se ordena y se quitan sus repetidos, después de normalizar `-0` y NaN (N4) y, en una columna `ci` o `ci_ai`, por su forma plegada. |
| CU8 | **Comparaciones**: con el orden de N5 de las tablas, el de los índices y los intervalos: `-0` es `+0`, todo NaN es igual a NaN y mayor que +∞, el texto `ci` y `ci_ai` se compara por su forma plegada (IC2), y un `uuid` con el orden de SQL Server, en ese orden. Con un nulo a cualquier lado, una comparación o un `in` es desconocido; un `in` con la lista vacía es falso, también sobre un nulo; una lista nula hace desconocido el `in` en todas las filas; `is_null` es verdadero o falso. `not` de desconocido es desconocido, y `and` y `or` siguen la lógica de tres valores (CS7). Las columnas vector y JSON admiten `is_null`, la proyección y `count`; cualquier otra comparación, `ORDER BY`, `min`, `max`, `sum` o `avg` sobre ellas es un documento no válido. |
| CU9 | **Proyección y columnas del resultado**: `columns` es una lista no vacía de nombres de columnas de la tabla, sin repetidos; el resultado tiene esas columnas en ese orden, con sus nombres, sus tipos y si admiten nulos según el esquema. Se puede ordenar por una columna que no se proyecta. El cursor describe las columnas de su resultado: nombre, tipo y si admite nulos. |
| CU10 | **Orden**: `keys` es una lista no vacía de `{"col": ..., "desc": bool, "nulls": "first" o "last"}`, sin columnas repetidas; al leer, `desc` falta como falso y `nulls` como `"first"` en las dos direcciones, como los índices de Krummi en su recorrido hacia delante (N5), así que un orden ascendente sin más puede salir de un índice ascendente creado sin opciones, y uno descendente de un índice descendente creado sin opciones; Krummi los escribe siempre. Usuario: «Primero en las dos (Recomendado)». Las filas empatadas en todas las claves salen en un orden no especificado (CS7). |
| CU11 | **`LIMIT` y `OFFSET`**: `offset` salta ese número de filas del resultado y `count` deja como mucho ese número de las siguientes; un `count` de 0 da un resultado vacío; valores más allá del número de filas, o cuya suma pasa de `u64`, no son un error. Un nulo en cualquiera de los dos falla con valor no válido al ejecutar. |
| CU12 | **Agregados**: `aggregates` es una lista no vacía de `{"fn": ..., "col": ..., "as": ...}`, donde `fn` es `count`, `min`, `max`, `sum` o `avg`; `col` falta solo en `count`, que entonces cuenta todas las filas (`count(*)`); y `as` es el nombre de la columna del resultado, único y con las reglas de los nombres de columna de 7.1. El resultado es siempre una sola fila. `count` es `u64` y nunca nulo, y con una columna cuenta sus valores no nulos. `min` y `max` admiten los tipos que se ordenan (todos salvo vector y JSON), son del tipo de su columna y admiten nulo; los demás agregados ignoran los nulos. Sobre cero filas, o si todos sus valores son nulos, `count` da 0 y los demás dan nulo. `min` y `max` de una columna `ci` o `ci_ai` devuelven el texto guardado de una de las filas con la forma plegada mínima o máxima, sin especificar cuál. `sum` y `avg` admiten los enteros, los de coma flotante y `decimal` (no `duration`): `sum` de enteros con signo es `i64`, de enteros sin signo `u64`, de `f32` o `f64` es `f64` y de `decimal(p, s)` es `decimal(38, s)`; si la suma exacta de todos los valores enteros o `decimal` pasa del rango de su tipo, la ejecución falla con valor no válido, nombrando el agregado, sea cual sea el orden en que se sumen. `sum` de coma flotante es la suma exacta de todos los valores redondeada una sola vez al `f64` más cercano, con empates al par, así que no depende del plan ni del orden de suma: da ±∞ solo si ese resultado exacto no cabe en un `f64`; un NaN entre los valores da NaN, y +∞ con −∞ también. `avg` de enteros y de coma flotante es la suma exacta dividida por la cuenta, redondeada una sola vez al `f64` más cercano, con empates al par, como `sum`, y con las mismas reglas para NaN y los infinitos; de `decimal(p, s)` es `decimal(38, s)` con el redondeo de E8; `avg` nunca desborda. Usuario, a la precisión de las sumas de coma flotante: «Exacta y sin depender del plan (Recomendado)». Usuario: «i64/u64, error al desbordar (Recomendado)». |
| CU13 | **Nombres y almacenes**: preparar con una tabla que no existe falla con no encontrado, y con un nombre que es de un árbol o una secuencia, con tipo distinto, como abrir la tabla (7.1); con una columna que la tabla no tiene, con documento no válido. Un documento con `bind` cuyas identidades no coinciden, también si su tabla ya no existe, falla con esquema distinto ([invalidación](queries.prd.md#invalidación-de-las-sentencias)). Ejecutar una sentencia en una transacción de otro almacén falla con uso incorrecto. Cerrar el almacén falla con uso incorrecto mientras vivan sentencias o conjuntos de valores; un cursor toma prestada su transacción y no puede sobrevivirla. Las consultas no ocupan las proyecciones que el almacén guarda para la aplicación (PY5). |
| CU14 | **Ejecutar**: la llamada que ejecuta comprueba, en este orden, que la sentencia es del almacén de la transacción, que todos los parámetros tienen valor (uso incorrecto), las opciones de la ejecución (opciones no válidas), el catálogo que ve la transacción (CS5: vuelve a enlazar o falla con esquema distinto), los valores de `count` y `offset` (valor no válido) y el presupuesto de las listas de `in` (límite superado); con el primer fallo no devuelve cursor. Si hay que volver a planificar, la llamada que ejecuta espera al muestreo (ST3); esa espera cuenta en el tiempo de la ejecución, y si al terminar la espera el tiempo se agotó o la ejecución se canceló, la llamada falla con su clase y no devuelve cursor. Un testigo ya cancelado hace fallar la llamada con cancelada. Una consulta dentro de una escritura ve los cambios que esa escritura hizo antes de ejecutarla. |
| CU15 | **Cursor**: devuelve las filas por lotes de hasta el tamaño que pide la aplicación al ejecutar, de 1 a 65.536 filas, 256 por defecto; un lote puede tener menos filas sin ser el último, y el final lo dice una lectura sin filas. Los errores de la ejecución aparecen en las lecturas; la lectura que falla no devuelve ninguna fila de su lote, y las filas de los lotes anteriores eran válidas. Después del final, cada lectura vuelve a decir que terminó; después de un error o de una cancelación, cada lectura falla con uso incorrecto. Soltar el cursor sin leerlo entero termina la ejecución (CS9). |
| CU16 | **Límites** (CS8): los de la [tabla de límites](#límites), con su valor por defecto y su máximo duro. Todos se configuran al abrir el almacén; los de los documentos y la planificación también al preparar, y los de la ejecución (memoria, desbordamiento, filas examinadas, tiempo y listas de `in`) también al ejecutar. Un presupuesto ausente toma el valor del almacén; `0` es cero, no «sin límite». Las filas examinadas son las filas y las entradas de índice que la ejecución decodifica del almacén (un atajo como `count(*)` por el número de filas no examina ninguna); el tiempo es de reloj, desde la llamada que ejecuta hasta que el cursor termina, incluido el tiempo entre lecturas. Se comprueban, con la cancelación, en cada lote y cada 1.024 filas que la ejecución examina o que su ordenación mueve, así que una ejecución los nota como mucho 1.024 filas después. Un tiempo que el reloj no puede representar cuenta como sin límite. Llegar justo al límite no falla; pasarlo, sí. Si se cumplen a la vez, gana la cancelación, después el tiempo y después el presupuesto. La aplicación cancela con un testigo que da al ejecutar, desde cualquier hilo; un mismo testigo puede darse a varias ejecuciones y las cancela todas; cancelar después del final o de soltar el cursor no hace nada. Un presupuesto pedido por encima de su máximo duro, o un tamaño de lote fuera de su intervalo, falla con opciones no válidas, nombrando el presupuesto y su máximo. |
| CU17 | **Desbordamiento** (CS9): la ordenación desborda a disco, con archivos propios de cada ejecución, en `krummi.sort`, el nombre reservado que ya usa crear un índice (P3 del Pager): cada ejecución borra los suyos al terminar, fallar, cancelarse o soltarse su cursor; la carpeta se borra cuando ya no tiene archivos, y también al cerrar y al abrir el almacén; no cuenta para el tamaño máximo ni para la información del almacén. Lo desbordado se comprueba al leerlo de vuelta, y un desbordamiento dañado o corto falla con entrada y salida, como al crear un índice (IX2): no es un daño del almacén (G6). Un fallo al desbordar o al leerlo hace fallar solo esa ejecución, también dentro de una escritura, que puede seguir confirmando (CS9). En modo temporal no se desborda ni se cuenta el presupuesto de desbordamiento: la ordenación usa memoria y, pasado su presupuesto, falla con límite superado. |
| CU18 | **Transacciones de escritura**: una consulta no cambia la escritura en que corre; si falla por un presupuesto, el tiempo, una cancelación o su desbordamiento, la escritura puede seguir confirmando. Un daño o un fallo de entrada y salida al leer el almacén sigue T11 y T12, como cualquier lectura de la escritura. |
| CU19 | **Errores** (CS10): clases nuevas **límite superado**, para un presupuesto de una ejecución agotado (memoria, desbordamiento, filas examinadas, lista de `in`), y **cancelada**. El tiempo agotado usa la clase de tiempo de espera agotado (T4), que pasa a cubrir también el tiempo de una ejecución. Un documento de una versión posterior al lector es un documento no válido. El objeto de error de CS10 lleva `class` (los nombres de la [tabla de clases](#clases-en-el-objeto-de-error)), `code` (solo en documento no válido), `path`, `expected`, `found` y `message`; un JSON mal formado lleva `line` y `column` en lugar de `path`. La [tabla de códigos](#códigos-de-documento-no-válido) fija la fase de cada código y a qué apunta su ruta; todos llevan `expected` y `found` salvo `malformed_json` y `too_large`. Fuera de documento no válido: límite superado lleva en `found` el nombre del presupuesto y en `expected` su valor; esquema distinto, en `expected` la identidad fijada y en `found` la actual, o nulo si ya no existe; valor no válido, en `found` el valor o el nombre `as` del agregado que desborda. La validación sigue las fases de §2.5 del documento 11 (sintaxis, estructura, enlace, tipos; la admisión es la de CU14): gana el primer fallo en el orden de las fases y, dentro de una fase, el del valor que aparece antes al leer el documento. Las rutas son del documento enviado (de `/statement/...` en uno de ejecución; del documento que escribió la API fluida, sin normalizar). Un error de la ejecución lleva la ruta del nodo de la consulta cuyo trabajo falló: el `scan` para las filas examinadas, un daño o un fallo de entrada y salida al leer el almacén; el `sort` para su memoria y su desbordamiento; la expresión `in` para su lista; la entrada de `aggregates` para un desbordamiento; el tiempo y la cancelación no llevan ruta. |
| CU20 | **Funciones para agentes** (CS12): la **descripción del almacén** es un documento `store/1`, un quinto tipo de documento que solo escribe Krummi: `{"krummi": "store/1", "tables": [...]}`, con las tablas en el orden de los bytes de su nombre, cada una `{"name", "id", "schema_version", "schema_state", "index_state", "columns", "primary_key", "indexes", "vector_indexes"}`; cada columna, en el orden del esquema, `{"name", "id", "type", "nullable"}`, con `type` el descriptor de la columna: el de CU5 para los tipos que no tienen más atributos; `{"type": "uuid", "order": "rfc9562" o "sql_server"}`; `{"type": "string", "collation": "binary", "ci" o "ci_ai"}`; `{"type": "vector", "element": "f32", "f16", "i8" o "bit", "dim": ..., "cosine": true o false}`; y `{"type": "json"}`. Estos descriptores de columna son de `store/1`; los de los parámetros siguen siendo los de CU5, donde un `uuid` o un `string` casa con cualquier orden o intercalación; `primary_key`, los nombres de sus columnas; cada índice, en orden de nombre, `{"name", "id", "unique", "keys": [{"col", "desc", "nulls"}], "include": [...]}`; y cada índice vectorial de 7.6, `{"name", "id", "col"}`. Las identidades y marcas son 16 dígitos hexadecimales en minúscula y la versión de esquema un entero. **Reescribir** lee un documento de consulta o de ejecución y lo escribe en la forma de Krummi, sin normalizarlo. **Validar sin ejecutar** lee una consulta, la valida contra el catálogo que ve la transacción y devuelve su forma normalizada con `bind`, o el error. El JSON Schema de cada tipo que existe en 17.1 (`query/1`, `execution/1`, `store/1`) lo devuelve una función de la biblioteca; no se distribuye como archivos (pregunta 4 del documento 11). |
| CU21 | **Planificador** (CS5, CS7, CS8): elige, por las estimaciones de 7.3, entre leer por la clave primaria, por un índice (también solo sus entradas, cuando tienen todas las columnas que la consulta necesita) o recorrer la tabla entera, y puede combinar varios caminos; evita ordenar cuando un camino ya da el orden pedido. Su resultado es admisible con cualquier plan. Preparar espera a un muestreo sin límite, como la estimación (ST3); la espera al explicar la fija 17.2. Las reservas M2 y M7 de CS13 las comprueba la revisión del diseño. |
| CU22 | **Medición** (CS14): tres cargas nuevas del banco, cada una con su operación directa como referencia y el umbral de 0,80, sobre el almacén y los datos de esa operación: **W33** `consulta-lecturas`, como W26 con una sentencia que filtra por la clave primaria; **W34** `consulta-intervalos`, como W27 con una sentencia que filtra desde una clave, ordena por la clave primaria y limita a 100; **W35** `consulta-intervalos-de-indice`, como W32 con una sentencia que filtra desde un `ts`, ordena por `ts` y por la clave primaria y limita a 100. La sentencia se prepara una vez fuera de la medida; cada operación medida da sus valores, ejecuta y lee todas sus filas, con la misma suma de comprobación que su referencia. Entran en la ejecución reducida con las cantidades de su referencia y sin umbral; Voron y los demás motores dicen `no aplica`. El informe de cierre compara W33 a W35 y una consulta que filtra por un intervalo de `n`, ordena por `ts` descendente y limita a 20 con las mismas en SQLite, con los mismos datos e índices cargados antes de medir, sentencias preparadas y el registro de escritura anticipada (WAL), sin umbral. |
| CU23 | **Pruebas**: AC-CS1 con 10.000 consultas generadas con la semilla 17, escrita en la prueba; el fuzzing de AC-CS3 es una prueba de propiedades con 100.000 documentos mutados, que fija por número de casos lo que el padre pedía por tiempo; AC-CS7 y AC-CU9 simulan versiones con un gancho de pruebas que cambia la versión del lector; AC-CU17 detiene el muestreo con el gancho de 7.3 (AC-N8); AC-CU8 altera el desbordamiento con un gancho de pruebas. |
| CU24 | **Diferencias con el diseño**: frente al documento 11, `count` y `offset` son `u64` (su ejemplo usaba `u32`); las fechas e instantes solo tienen la forma de los años 0001 a 9999 y `duration` el dominio de E8, no el de un `i64`; un `string` o unos `bytes` se aceptan de cualquier tamaño, no solo hasta el máximo de su columna (CU6); las claves de orden llevan `nulls`; un parámetro no usado es un error; y hay un quinto tipo de documento, `store/1` (cambia CS3, que decía cuatro). Frente al documento 4, la API fluida no normaliza (CU7). |

### Formas de los valores

| Tipo | Forma que escribe Krummi | También acepta | Rechaza (todo lo demás; por ejemplo) |
|---|---|---|---|
| `bool` | `true`, `false` | — | `0`, `"true"` |
| `i8`–`i32`, `u8`–`u32` | Número entero | — | `1.0`, `1e2`, `-0`, fuera de rango |
| `i64`, `u64` | Texto decimal, `"-9223372036854775808"`, `"18446744073709551615"` | — | Números, `"+1"`, `"007"`, `"-0"`, fuera de rango |
| `f32`, `f64` | El número más corto que vuelve a los mismos bits; `"NaN"`, `"Infinity"`, `"-Infinity"` | Cualquier número JSON, redondeado al valor más cercano del tipo con empates al par, también entre los subnormales y el cero: los subnormales se conservan, y solo da cero lo que queda por debajo de la mitad del menor subnormal, o en esa mitad exacta (`1e-45` en un `f32` da el menor subnormal) | Otros textos; un número que al redondear desborda (`1e400`) |
| `decimal(p, s)` | Texto con exactamente `s` decimales (sin punto si `s` es 0), sin exponente, sin `+`, sin ceros a la izquierda ni signo en el cero | — | `"1234.5"` con escala 2, `"1.2e3"`, `"-0.00"`, más de `p` dígitos |
| `string` | Texto JSON, con escapes solo donde JSON los exige | Cualquier escape JSON de los mismos caracteres | Sustitutos sueltos |
| `bytes` | Base64 con el alfabeto de RFC 4648 §4, con relleno y bits de relleno a cero | — | base64url, sin relleno, bits de relleno no nulos, espacios |
| `date` | `"2026-11-10"` | — | `"2026-1-1"`, `"+2026-11-10"`, fuera de E8 |
| `time` | `"13:45:07.123456"`, con seis decimales | — | `"13:45:07"`, más o menos de seis decimales, segundo 60 |
| `timestamp` | `"2026-11-10T13:45:07.123456Z"`, con `T` y `Z` mayúsculas y seis decimales | — | Con zona, minúsculas, más o menos de seis decimales, fuera de E8 |
| `duration` | Texto decimal de microsegundos, dentro de E8 | — | `"PT1M30S"`, fuera de E8 |
| `uuid` | Texto RFC 9562 en minúsculas, en el orden de bytes de RFC, también en columnas con el orden de SQL Server | — | Mayúsculas, llaves, sin guiones |
| `ulid` | 26 caracteres Crockford Base32 en mayúsculas, el primero como mucho `7` | — | Minúsculas, `I`, `L`, `O`, `U`, primero mayor que `7` |
| lista | Array de valores del tipo de su elemento | — | Listas anidadas, elementos nulos |
| nulo | `null`, para cualquier tipo | — | — |

### Límites

| Presupuesto | Nombre | Por defecto | Máximo duro |
|---|---|---|---|
| Tamaño de un documento | `document_bytes` | 1 MiB | 64 MiB |
| Anidación JSON de un documento (techo, no presupuesto) | — | 128 | 128 |
| Profundidad de nodos y expresiones | `depth` | 64 | 128 (el techo de la anidación JSON puede llegar antes) |
| Nodos y expresiones de un documento | `nodes` | 4.096 | 65.536 |
| Parámetros de un documento | `params` | 1.024 | 65.536 |
| Trabajo de planificación | `planning` | 65.536 | 1.048.576 |
| Entradas de la caché de planes (solo por almacén) | `plan_cache` | 256 | 65.536 |
| Elementos de una lista de `in` | `in_list` | 10.000 | 1.000.000 |
| Memoria de una ejecución | `memory_bytes` | 64 MiB | 16 GiB |
| Desbordamiento a disco de una ejecución | `spill_bytes` | 1 GiB | 1 TiB |
| Filas examinadas de una ejecución | `rows_examined` | sin límite | — |
| Tiempo de una ejecución | `deadline_ms` | sin límite | — |

### Códigos de documento no válido

Las fases son las de CU19. La ruta apunta al valor que falla, salvo donde se dice otra cosa.

| Código | Fase | Situación | Ruta |
|---|---|---|---|
| `malformed_json` | Sintaxis | No es JSON bien formado o no es UTF-8 | Sin ruta: `line` y `column` |
| `too_large` | Sintaxis | Pasa `document_bytes` | `""` (el documento) |
| `too_deep` | Sintaxis | Pasa el techo de anidación JSON, o `depth` (en la fase de estructura); `found` dice cuál | El primer valor demasiado profundo |
| `duplicate_member` | Sintaxis | Un miembro repetido; `found` lo nombra | El objeto |
| `unsupported_version` | Estructura | Un `krummi` que no es un tipo y una versión, un tipo que no se acepta aquí (`plan/1` y `explain/1` en 17.1, `store/1` siempre), o una versión que el lector no conoce o es posterior | `/krummi` |
| `removed_construct` | Estructura | Una construcción quitada; `found` la nombra con la versión que la quitó | La construcción |
| `unknown_member`, `missing_member` | Estructura | Un miembro que no corresponde, o falta uno obligatorio; `expected` lo nombra | El miembro; el objeto si falta |
| `unknown_op` | Estructura | Una operación que la versión no tiene | `op` |
| `wrong_kind` | Estructura | Un valor JSON de otra clase que la esperada, salvo en `args` (CU4) | El valor |
| `invalid_option` | Estructura | Un valor de `fn` o de `nulls` que no existe | El valor |
| `invalid_type` | Estructura | Un descriptor de tipo no válido (precisión o escala fuera de N4, lista de listas, vector o JSON) | El descriptor |
| `invalid_arity` | Estructura | Un `and` o `or` con menos de dos argumentos, o `columns`, `keys` o `aggregates` vacíos | La lista |
| `invalid_chain` | Estructura | La cadena de nodos no cumple CU2 | El nodo fuera de lugar |
| `too_many_nodes`, `too_many_params` | Estructura | Pasa `nodes` o `params` | `""` y `/params` |
| `invalid_param_ref`, `unused_param` | Estructura | Una referencia a un parámetro que no existe, o uno declarado sin usar | La referencia; `/params/i` |
| `duplicate_name` | Estructura | Un nombre de parámetro, de columna proyectada, de clave de orden o de `as` repetido | La segunda aparición |
| `invalid_args` | Estructura | `args` de otra longitud que `params` | `/args` |
| `invalid_name` | Estructura | Un `table` vacío o más largo que B4, o un `as` que no cumple las reglas de los nombres de columna de 7.1 | El nombre |
| `invalid_bind` | Estructura | Un `bind` con una identidad que no son 16 dígitos hexadecimales en minúscula, o que no tiene exactamente las columnas que usa la consulta | El valor o `bind` |
| `unknown_column` | Enlace | Una columna que la tabla no tiene, sin `bind` (CU13) | El nombre |
| `type_mismatch` | Tipos | Un parámetro que no casa con su columna, `count` u `offset` que no son `u64`, o una operación no admitida sobre su columna (CU8) | La expresión o el nodo |

### Clases en el objeto de error

| Clase | `class` |
|---|---|
| Acceso | `access` |
| Almacén en uso | `store_in_use` |
| No es un almacén | `not_a_store` |
| Dañado | `damaged` |
| Formato distinto | `different_format` |
| Disco lleno | `disk_full` |
| Entrada y salida | `io` |
| Opciones no válidas | `invalid_options` |
| Tamaño máximo | `max_size` |
| Memoria agotada | `out_of_memory` |
| Tiempo de espera agotado | `timeout` |
| Uso incorrecto | `misuse` |
| Tamaño no válido | `invalid_size` |
| Tipo distinto (tipo de árbol distinto) | `different_type` |
| No encontrado | `not_found` |
| Ya existente | `already_exists` |
| Valor no válido | `invalid_value` |
| Clave duplicada | `duplicate_key` |
| Esquema distinto | `different_schema` |
| Esquema no válido | `invalid_schema` |
| Documento no válido (nueva, CS10) | `invalid_document` |
| Límite superado (nueva) | `limit_exceeded` |
| Cancelada (nueva) | `cancelled` |

### Estado de partida

Verificado el 2026-11-28 en el código y en los documentos de diseño:

| Aspecto | Situación |
|---|---|
| Consultas | No existen. Ningún crate depende de serde en sus dependencias normales; `serde_json` es solo dependencia de pruebas del crate `krummi` (AC-JS11). |
| Errores | Clases de D8 en el código: acceso, almacén en uso, no es un almacén, dañado, formato distinto, disco lleno, entrada y salida, opciones no válidas, tamaño máximo, memoria agotada, tiempo de espera agotado, uso incorrecto, tamaño no válido, tipo de árbol distinto, no encontrado, ya existente, valor no válido, clave duplicada, esquema distinto y esquema no válido. No hay clases de cancelación ni de límite superado. |
| Transacciones | Una lectura se puede pasar a otros hilos y compartir; una escritura se queda en su hilo (T6). Cerrar el almacén con una transacción abierta no compila. Los recorridos toman prestada su transacción; en una escritura viven hasta la operación siguiente que puede cambiarla. |
| Lecturas | Por clave primaria; intervalos de la clave primaria y de un índice, hacia delante y hacia atrás; las entradas de un índice sin leer la fila. Un intervalo de un índice puede pedir solo los nulos de una columna, pero no los nulos y un intervalo de valores juntos; un límite de más de 2.048 bytes falla con tamaño no válido (IX9). |
| Orden | Índices ascendentes o descendentes por columna, con los nulos primero por defecto en el recorrido hacia delante; la clave primaria es ascendente y sin nulos (N5). |
| Estimación | Igualdad (con nulo), intervalo e igualdad sobre varias columnas, en lecturas y escrituras, con valores de cualquier tamaño; sin estadísticas, muestrea y espera (ST2, ST3). |
| Ordenación con desbordamiento | Crear un índice ya ordena con 64 MiB de memoria antes de desbordar en `krummi.sort`, que la apertura borra; en modo temporal no desborda y sigue ordenando en memoria sin ese tope, hasta que el sistema no puede darle más y falla con memoria agotada (IX2). |
| Proyecciones | El almacén guarda en memoria hasta 1.024 proyecciones para la aplicación (PY5). |
| Banco | 32 cargas, W1 a W32, con umbral frente a Voron; las de tablas son W23 a W32. |

## Actores

- **Desarrollador integrador**: escribe consultas con la API fluida o en JSON, las prepara, las ejecuta con sus valores
  y lee el cursor; configura los límites y cancela ejecuciones.
- **Agente o herramienta**: lee la descripción del almacén y los JSON Schema, valida documentos sin ejecutarlos y
  corrige los errores por su código y su ruta (CU20).
- **Mantenedor**: ejecuta las pruebas de CU23 y las cargas y la comparación de CU22.

## Flujo

1. La aplicación construye una consulta con la API fluida o escribe su documento (CU1–CU5).
2. La prepara en una transacción: Krummi la lee, la valida, la enlaza y la planifica (CU7, CU13, CU19, CU21).
3. Da los valores de sus parámetros y la ejecuta con sus límites y su testigo de cancelación (CU6, CU14, CU16).
4. Lee el cursor por lotes y, si hace falta, cancela desde otro hilo (CU15, CU16).

## Validaciones y mensajes

Valen las de los ascendientes. Este PRD fija estas, solo de la biblioteca:

| Situación | Clase de error |
|---|---|
| Un documento que no cumple CU2–CU5 o CU8–CU12, o de una versión posterior | Documento no válido, con su código |
| Una tabla que no existe | No encontrado |
| Un nombre de tabla que es de un árbol o una secuencia | Tipo distinto |
| Un documento con `bind` cuyas identidades no coinciden, también con su tabla borrada | Esquema distinto |
| Un valor de un parámetro en otra forma o fuera de su dominio, o un nulo en `count` u `offset` | Valor no válido |
| Ejecutar en otro almacén o sin todos los valores, cerrar con sentencias o valores vivos, o leer un cursor después de un error o una cancelación | Uso incorrecto |
| Un presupuesto por encima de su máximo duro o un tamaño de lote fuera de 1 a 65.536 | Opciones no válidas |
| Una ejecución que agota su memoria, su desbordamiento, sus filas examinadas o el presupuesto de una lista de `in` | Límite superado (nueva) |
| Una ejecución que agota su tiempo | Tiempo de espera agotado |
| Una ejecución cancelada, también con un testigo ya cancelado al ejecutar | Cancelada (nueva) |
| Al ejecutar, un catálogo con identidades que ya no coinciden | Esquema distinto (CU14) |
| Al preparar, un presupuesto de documento o de planificación por encima de su máximo duro | Opciones no válidas (CU16) |
| Una `sum` cuya suma exacta pasa del rango de su tipo | Valor no válido (CU12) |
| Un desbordamiento dañado o corto | Entrada y salida |

## Salida

El módulo no escribe nada en pantalla. Una consulta de los clientes de España creados desde una fecha, los 20 más
recientes, con dos columnas, en la forma de Krummi con espacios añadidos para leerla:

```json
{ "krummi": "query/1", "table": "customers",
  "params": [ { "type": "string", "name": "country" },
              { "type": "timestamp", "name": "since" },
              { "type": "u64", "name": "n" } ],
  "root": { "op": "project", "columns": [ "id", "created" ],
    "input": { "op": "limit", "count": { "param": 2 },
      "input": { "op": "sort", "keys": [ { "col": "created", "desc": true, "nulls": "first" } ],
        "input": { "op": "filter",
          "predicate": { "op": "and", "args": [
            { "op": "eq", "col": "country", "value": { "param": 0 } },
            { "op": "ge", "col": "created", "value": { "param": 1 } } ] },
          "input": { "op": "scan" } } } } } }
```

Krummi escribe los miembros en el orden de CU2 a CU5 (`krummi`, `table`, `bind`,
`params`, `root`; en cada nodo y expresión, `op`, sus miembros en el orden de CU2 y CU3 e `input` al final; en una
declaración, `type` y después `name`), sin espacios, y el tipo y la versión como
`"query/1"` para la 1.0 y `"query/1.2"` para las demás; al leer acepta `"query/1.0"`, cualquier orden de miembros y
cualquier espacio. Su ejecución con valores, como la usan las pruebas y las herramientas:

```json
{ "krummi": "execution/1", "statement": { "krummi": "query/1", "...": "la consulta de arriba" },
  "args": [ "ES", "2026-01-01T00:00:00.000000Z", "20" ],
  "limits": { "memory_bytes": "67108864" } }
```

Un agregado, `{"op": "aggregate", "aggregates": [{"fn": "count", "as": "n"}, {"fn": "avg", "col": "total", "as":
"media"}], "input": ...}`, devuelve una fila con las columnas `n` y `media`.

Un error de documento no válido:

```json
{ "class": "invalid_document", "code": "unknown_column", "path": "/root/input/input/input/predicate/args/1/col",
  "expected": "a column of table customers", "found": "creatd", "message": "unknown column creatd in table customers" }
```

Los valores de las filas siguen *Salida* del padre: sin copia los de `string`, `bytes`, vector y JSON leídos de una
fila, y en una copia propia los de las claves de las entradas de índice, los agregados y lo que pasó por un
desbordamiento.

## Reutilización

- Las lecturas por clave, por intervalos de la clave primaria y de los índices, y las entradas de los índices (7.1,
  7.2).
- La estimación y el muestreo de 7.3 para el planificador.
- La ordenación con desbordamiento de crear un índice y su nombre reservado `krummi.sort` (P3 del Pager, IX2).
- La forma plegada de 7.7 para comparar y ordenar texto `ci` y `ci_ai`.
- Las cargas de tablas del banco y sus datos (W26, W27, W32).

## Fuera de alcance

- Lo de los PRD hijos siguientes (17.2 a 17.13): planes, explicación, ejecución acotada y comparación; prefijo, `LIKE`
  y JSON; vectores; expresiones, comparaciones entre columnas, columnas calculadas y renombrar columnas en la
  proyección; índices parciales; escrituras; agrupación, `count` de valores distintos y agregados con columnas, orden o
  límites; joins; subconsultas; conjuntos; ventanas; perfil.
- Parámetros de tipo vector o JSON, y por tanto sus formas (cambia la celda *Fija* del padre, que pedía las de `json`:
  llegan con 17.3 y 17.4).
- Ordenar por la posición de una columna o por un agregado.
- Los documentos `plan/1` y `explain/1` y la espera a un muestreo al explicar (17.2); leer `store/1` como entrada.
- La interfaz C y el binding C# (módulo 18) y la línea de órdenes (módulo 12).
- Cancelar o limitar el tiempo de preparar (*Fuera de alcance* del padre).
- Distribuir los JSON Schema como archivos.

## Dejado a la implementación

- Los nombres de los tipos y funciones de Rust de la API fluida, del cursor, del testigo y de los límites, y el texto
  en inglés de los mensajes y de `expected` y `found` (D8).
- Cómo ordena el desbordamiento y cuántos archivos usa, siempre que cumpla CU17 y los presupuestos.
- Cómo se calcula la suma exacta de coma flotante de CU12, siempre que dé su resultado.
- El modelo de coste, cuándo unos valores piden otro plan, la organización de la caché y qué cuenta como memoria de
  una ejecución (CS5, CS8).
- La lista exacta de crates de CS11: es el árbol de dependencias normales de `serde`, `serde_derive` (con los crates
  que usa al compilar) y `serde_json` 1.0 con `float_roundtrip`,
  que da la lectura exacta de los números de coma flotante de CU6, y sin `arbitrary_precision`, `raw_value` ni
  `unbounded_depth`; la ejecución la anota en D12 del padre raíz al añadirlos.
- La unidad de `line` y `column` de `malformed_json`.

## Criterios de aceptación

Valen los de los ascendientes; este PRD comprueba con sus reglas los de su padre que nombran 17.1 (AC-CS1 a AC-CS13,
AC-CS14 en lo de 17.1 y AC-CS17), y añade estos.

1. **AC-CU1** — Dados los documentos de referencia de cada nodo, expresión y tipo de valor, guardados en la forma de
   Krummi, cuando se preparan, se ejecutan y se reescriben (CU20), entonces dan el resultado esperado y los mismos
   bytes; un documento con `"query/1.0"`, con otro orden de miembros o con espacios se acepta y se reescribe en la forma
   de Krummi; y la API fluida escribe, para cada documento sin `bind` ni nombres de parámetros, el mismo documento
   (CU1–CU5).
2. **AC-CU2** — Dados los ejemplos de cada fila de la tabla de formas y, para cada tipo, un valor en el límite de E8 y
   uno fuera, cuando se enlazan, entonces se aceptan con su valor o fallan con valor no válido como dice la tabla (CU6).
3. **AC-CU3** — Dada una consulta con `and` y `or` anidados, `not` sobre cada operación, argumentos repetidos y
   desordenados, cuando se valida sin ejecutar, entonces devuelve la forma normalizada de CU7, con los bytes
   que dan sus reglas, la misma para predicados equivalentes por esas reglas, y un `and` que se queda con un argumento
   sustituido por él; y una lista de `in` con repetidos, `-0`, NaN y textos `ci` que se pliegan
   igual da las filas de la lista normalizada (CU7).
4. **AC-CU4** — Dada una tabla con nulos, `-0`, NaN, ±∞, texto `ci` y `ci_ai` y `uuid` en los dos órdenes, cuando se
   filtra con cada comparación, `in` (también vacío y con la lista nula) e `is_null`, y se ordena por cada columna en
   las dos direcciones, con los nulos primero y último y sin decirlo, entonces las filas son las de un modelo de
   referencia que compara por el orden de N5 y la lógica de tres valores (CU8, CU10).
5. **AC-CU5** — Dadas una proyección, una sin `project` y una que ordena por una columna no proyectada, entonces el
   cursor describe y devuelve las columnas de CU9; y un nombre repetido falla con `duplicate_name` (CU9).
6. **AC-CU6** — Dadas consultas con `count` y `offset` de 0, 1, el número de filas, más, `u64` máximo los dos y un
   nulo, entonces devuelven las ventanas esperadas o fallan con valor no válido (CU11).
7. **AC-CU7** — Dados agregados sobre cero filas, sobre solo nulos, sobre `ci_ai`, sobre columnas vector y JSON
   (`count`) y sobre cada tipo numérico, también en los extremos de su tipo, cuando se ejecutan, entonces dan los
   valores, los tipos y los nulos de CU12, también los de coma flotante sobre los casos de cancelación, desbordamiento
   intermedio, NaN e infinitos, iguales con cualquier plan; de `ci_ai`, una forma
   plegada mínima o máxima; y una `sum` que pasa de `i64`, de `u64` o de 38 dígitos falla con valor no válido (CU12).
8. **AC-CU8** — Dada una ejecución cuya ordenación desborda, cuando se lee entera, cuando se cancela, cuando se suelta a
   medias, cuando corre a la vez que otra que desborda y que la creación de un índice, y cuando se simula una caída y
   se reabre, entonces cada una borra solo sus archivos y `krummi.sort` no queda cuando todas terminan; con el
   desbordamiento alterado por el gancho de CU23 falla con entrada y salida y, dentro de una escritura, la escritura
   confirma sus otros cambios; y en modo temporal, pasada la memoria, falla con límite superado sin crear archivos
   (CU17, CU18).
9. **AC-CU9** — Dado cada código de la tabla de códigos, con el gancho de versiones de CU23 para `removed_construct` y
   `unsupported_version`, cuando se prepara un documento que lo provoca, entonces lleva ese código, su ruta y
   `expected` y `found` salvo donde CU19 dice otra cosa; para `invalid_chain`, la ruta del primer nodo, desde la raíz,
   que rompe el orden de CU2; y documentos con dos fallos de fases distintas
   y con dos de la misma fase dan el que fija CU19 (CU19).
10. **AC-CU10** — Dados los casos de CU13 y CU14 (tabla que no existe y nombre de un árbol; columna que no existe con
    y sin `bind`; `bind` de una tabla borrada; otro almacén; valores sin dar; opciones no válidas; `count` nulo; lista
    de `in` sobre su presupuesto; varios de ellos a la vez; cerrar con sentencias o valores vivos; una consulta en una
    escritura después de insertar), entonces cada uno da la clase, el orden y las filas de CU13 y CU14; y crear y
    ejecutar consultas no reduce las 1.024 proyecciones que puede guardar la aplicación (CU13, CU14).
11. **AC-CU11** — Dado un cursor, cuando se lee con lotes de 1, 256 y 65.536 filas, después del final, después de un
    error, después de una cancelación y de una cancelación tardía, y se pide un lote de 0 o de 65.537, entonces se
    comporta como dicen CU15 y CU16 (CU15, CU16).
12. **AC-CU12** — Dados una ejecución con un tiempo ya vencido, otra cancelada y, sobre una tabla sin índices de N
    filas, que solo se puede recorrer entera, otra con el presupuesto de filas examinadas en N y en N − 1, y una que cumple tiempo, cancelación y presupuesto en la
    misma comprobación, entonces cada una termina o falla como dice CU16, con su ruta de CU19; y un filtro que descarta
    todas las filas nota la cancelación dentro de 1.024 filas examinadas (CU16, CU19).
13. **AC-CU13** — Dada una escritura que cambia una fila y después ejecuta una consulta que falla por un presupuesto,
    por el tiempo, por una cancelación y por un fallo de su desbordamiento, entonces la escritura confirma su cambio; y
    cuando la consulta encuentra un daño o un fallo de entrada y salida al leer el almacén, entonces la escritura ya no
    puede confirmarse (T11) (CU18).
14. **AC-CU14** — Dado un almacén con tablas, columnas de cada tipo e índices, cuando se pide su descripción, entonces
    el documento `store/1` tiene los miembros y el orden de CU20, con
    los valores del catálogo; validar sin ejecutar devuelve la consulta normalizada con su
    `bind`, y cada JSON Schema valida los documentos de referencia de su tipo (CU20).
15. **AC-CU15** — Dada una tabla con estadísticas y otra sin ellas, cuando se preparan consultas que pueden leer por la
    clave primaria, por un índice, por solo las entradas de un índice, por dos índices combinados o por toda la tabla, y
    con un orden que da un índice y un `sort` con `limit`, entonces el gancho de pruebas de AC-CS1 muestra que el
    planificador consideró cada uno de esos caminos y si evitó la ordenación; una igualdad sobre toda la clave primaria
    elige leer por ella, con estadísticas y sin ellas; y el resultado es admisible; y un parámetro de
    texto de 4.096 bytes sobre una columna indexada da el mismo resultado con cada camino (CU6, CU21).
16. **AC-CU16** — Dadas las cargas W33 a W35, cuando se ejecuta el banco, entonces el informe las muestra con su razón
    frente a su operación directa, `no aplica` en los demás motores, y cuenta sus umbrales con los demás; y el informe
    de cierre tiene la comparación con SQLite de CU22 (CU22).
17. **AC-CU17** — Dada una sentencia preparada, cuando cambian las estadísticas de su tabla para que haya que volver a
    planificarla y el muestreo está detenido en el gancho de 7.3, entonces la llamada que ejecuta espera; y si en la
    espera vence el tiempo o se cancela, al liberarse el muestreo la llamada falla con tiempo de espera agotado o con
    cancelada, sin cursor (CU14).

## Anotaciones a otros PRD

Este PRD precisa reglas de documentos anteriores, que llevan las anotaciones del 2026-11-28 (en sus traducciones
también), salvo la de D12 del padre raíz, que pondrá la ejecución al añadir los crates:

- Padre raíz, D8 y *Validaciones y mensajes*: clases nuevas límite superado y cancelada, y los nombres de las clases en
  el objeto de error (CU19).
- Padre raíz, G6: un desbordamiento de una consulta dañado o corto falla con entrada y salida, como el de crear un
  índice; no es un daño del almacén (CU17).
- Transacciones, T4: la clase de tiempo de espera agotado cubre también el tiempo de una ejecución de una consulta
  (CU19).
- Padre raíz, D12: la lista de crates de CS11, al añadirlos (*Dejado a la implementación*).
- Consultas, CS3 y CS12: un quinto tipo de documento, `store/1`, que solo escribe Krummi, y la función de reescribir
  (CU20). CS8: la profundidad de nodos es un presupuesto aparte del techo de anidación JSON (*Límites*).
- Consultas, CS3, CS8, CS9, CS10 y la celda *Fija* de 17.1: lo que dejaban a 17.1 queda fijado aquí; una tabla que no
  existe falla con no encontrado y no con esquema distinto cuando el documento no lleva `bind` (CU13); las formas de
  `json` pasan a 17.3 y 17.4.
- Pager, P3: `krummi.sort` lo usa también el desbordamiento de las consultas, con archivos propios de cada ejecución
  (CU17).
- Índices, IX2: crear un índice borra solo sus archivos de `krummi.sort`, y la carpeta cuando queda vacía (CU17).
- Banco, K7 y *Salida*: tres cargas nuevas, W33 a W35, con su umbral (CU22); «Krummi cumple N de 32 umbrales» pasa a
  contar 35. K4 y K11: empiezan en el almacén de su referencia (W26, W27, W32) y su ejecución reducida tiene las
  cantidades de esa referencia.
- Proyecciones, «las consultas del módulo 17 fijarán si las usan»: no las ocupan (CU13).

## Referencias

- Padre: [queries.prd.md](queries.prd.md) v1.0 (CS1–CS14, AC-CS1–AC-CS17); raíz: [krummi.prd.md](../krummi.prd.md)
- Tablas: [tables.prd.md](../tables/tables.prd.md) (N4, N5), [tables-schema.prd.md](../tables/tables-schema.prd.md) (E1,
  E8), [tables-indexes.prd.md](../tables/tables-indexes.prd.md) (IX2, IX9),
  [tables-statistics.prd.md](../tables/tables-statistics.prd.md) (ST2, ST3),
  [tables-collation.prd.md](../tables/tables-collation.prd.md) (IC2),
  [tables-projection.prd.md](../tables/tables-projection.prd.md) (PY5)
- Pager: [pager.prd.md](../pager/pager.prd.md) (P3); transacciones: [transactions.prd.md](../transactions/transactions.prd.md) (T4, T6, T11, T12)
- Banco: [benchmark.prd.md](../benchmark/benchmark.prd.md) (K4, K6, K7, K11, K13, K14)
- Diseño: [docs/design/tables/](../../../design/tables/README.md) documentos 4, 8, 9, 10 y 11

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 77da4411 | 2026-11-28 09:18 UTC+00:00 | 6 h 2 min | 3 h 17 min | 2 h 45 min | 1 h 39 min | 1 h 6 min | 24 |

<!-- prd-time: [{"session": "77da4411-edaa-49ca-96e6-3afe464b2b23", "start": "2026-11-28T09:18:54.972000+00:00", "wall": 21717, "discarded": 11796, "agent": 5944, "human": 3976, "interventions": 24, "idle_minutes": 10}] -->
