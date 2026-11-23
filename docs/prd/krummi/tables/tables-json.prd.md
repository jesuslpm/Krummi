# Columnas JSON de las tablas — PRD

**Versión**: 1.0
**Fecha**: 2026-11-21
**Documento padre**: [Tablas de Krummi — PRD](tables.prd.md) (v1.0)

## Objetivo y contexto

Construir el PRD hijo **7.5** del [reparto del módulo de tablas](tables.prd.md#reparto-en-prd-hijos): un tipo de
columna **JSON** que guarda un documento JSON por fila, lo valida al escribirlo, lo devuelve entero sin copiarlo y
permite navegarlo y leer sus propiedades como tipos de Krummi.

**Por qué ahora**: 7.1, 7.2 y 7.3 están cerrados, y la anotación del 2026-11-10 del padre construye 7.5 después de 7.3
y antes del módulo 17, para que las consultas puedan filtrar por rutas del JSON desde el principio.

El diseño técnico está decidido en el [documento de diseño 13](../../../design/tables/13-json-column-storage.md), que es
el estado de partida técnico de este PRD, y casi todo lo que 7.5 debía fijar quedó fijado en la anotación del
2026-11-12 del padre (*Reparto en PRD hijos*): el texto normalizado como forma guardada, qué es un valor JSON válido, el
nulo de la columna frente al `null` del JSON, los números, las claves repetidas, el orden de las propiedades, la
igualdad al guardar, la profundidad, las clases de error, el lector propio y la lectura tipada. Este PRD no las vuelve a
decidir: las precisa hasta que no dejan huecos observables y fija lo que la anotación dejó a 7.5 (si el cambio es
incompatible, cómo se declara la columna, los datos de sus errores y su precedencia, sus cargas del banco), además de lo
que los PRD hermanos le dejaron: las estadísticas de las columnas JSON (ST1 de
[tables-statistics.prd.md](tables-statistics.prd.md)) y su columna preparada en las proyecciones
([tables-projection.prd.md](tables-projection.prd.md), *Fuera de alcance*).

Decisiones del usuario del 2026-11-21, sobre las propuestas del borrador: el texto se valida al escribirlo, con la
tabla y la columna en el error («Al escribir», JS3); las columnas JSON no tienen estadísticas («Ninguna, como vector»,
JS10); no hay un constructor de documentos sin texto («No, fuera de 7.5», JS11); y no hay cargas nuevas del banco, sino
una medición informativa en el informe de cierre («Medición informativa», JS13).

Este PRD hereda todo lo de sus ascendientes. Precisa o amplía reglas de su padre y de
[tables-schema.prd.md](tables-schema.prd.md) (7.1), [tables-projection.prd.md](tables-projection.prd.md) y
[tables-statistics.prd.md](tables-statistics.prd.md) (7.3), que llevan su anotación (*Anotaciones a otros PRD*).

En este documento, un **documento** es el valor de una columna JSON en una fila; un **valor JSON** es el documento o
cualquier valor que contiene (un objeto, un array, una cadena, un número, `true`, `false` o `null`); una **propiedad**
es un par de un nombre y un valor dentro de un objeto; el **texto normalizado** es la forma en que Krummi guarda y
devuelve un documento (JS4); y **no está** es el resultado de navegar a un valor que el documento no tiene, distinto de
un valor `null`.

### Decisiones de producto

| ID | Decisión |
|---|---|
| JS1 | **El tipo de columna**: N4 gana el tipo `json`, sin parámetros, que se declara como cualquier otro tipo de columna (E4) y que Describir muestra como `json`. Una columna JSON admite nulos salvo que se declare que no, puede tener un valor por defecto (JS9) y sigue los cambios de esquema de N3 y sus reglas de E5 como cualquier columna: se puede añadir una que admite nulos o que tiene valor por defecto, quitarla (salvo si está en un índice, también como columna incluida, E5), renombrarla y permitir nulos; cambiar el tipo de una columna a `json` o desde `json` no se admite (N3). No puede ser columna de la clave primaria: la regla de E5 «una columna de la clave que admite nulos, es un vector o tiene valor por defecto» incluye la columna JSON. No puede ser columna de la clave de un índice secundario (IX3); sí una columna incluida (IX1), como cualquier otro tipo. Las columnas JSON funcionan igual en un almacén temporal (G9). |
| JS2 | **Qué es un documento válido** (precisa la anotación del 2026-11-12 del padre): un texto en UTF-8 válido que es exactamente un valor de RFC 8259, también un valor suelto en la raíz (`42`, `"x"`, `true`, `null`), con espacios en blanco opcionales antes, después y entre sus elementos; los espacios en blanco son los cuatro de RFC 8259 (espacio, tabulador, salto de línea y retorno de carro) y ningún otro. No es válido: un texto vacío o solo de espacios; datos después del valor; una marca BOM; un comentario; `NaN`, `Infinity`, `+1`, `01`, `1.`, `.5`, `0x10` o cualquier número fuera de la gramática de RFC 8259; una comilla simple como delimitador; una coma final (`[1,]`, `{"a":1,}`); un carácter de control U+0000 a U+001F sin escapar dentro de una cadena; un escape que RFC 8259 no define (`"\x"`, `"\u12"`); un sustituto UTF-16 suelto, escapado (`"\ud800"`, `"\udc00"`, o uno alto seguido de algo que no es un bajo); un objeto con dos propiedades del mismo nombre, comparados los nombres por sus caracteres después de decodificar sus escapes y sin ninguna normalización de Unicode (`{"a":1,"a":2}` no es válido; `"é"` escrito como un carácter y como `e` seguido del acento combinante son nombres distintos); y más de 128 objetos o arrays anidados (el documento `[[…]]` con 128 corchetes de apertura es válido y con 129 no). Un par de sustitutos escapado es su carácter; U+0000, escapado como `\u0000`, y los no caracteres de Unicode son válidos, como en una columna `string` (E7). Los números no tienen límite de dígitos ni de exponente más allá del tamaño de la fila (`1e400`, `123456789012345678901234567890`). |
| JS3 | **Escribir un documento**: la aplicación da el valor de una columna JSON como un valor de clase JSON, hecho de un texto o de unos bytes, que pueden no ser UTF-8 válido; para guardar un documento leído, da su texto (JS6), mientras la lectura vale (B9). Un valor `string` o `bytes` en una columna JSON, o un valor JSON en una columna de otro tipo, falla con valor no válido y la regla tipo distinto del de la columna (E17). Insertar, actualizar, insertar o actualizar y la carga masiva validan y normalizan el documento antes de cambiar nada, con las reglas de E17 (JS8); un texto que no es un documento válido falla con valor no válido, y en la carga masiva la carga aborta entera (E13). Hacer el valor JSON no valida nada: la validación ocurre solo al escribir, así que su error lleva la tabla y la columna (usuario, entre validar al escribir y validar al construir el valor: «Al escribir»). |
| JS4 | **El texto normalizado**: Krummi guarda y devuelve cada documento en esta forma, que conserva todo lo que el documento significa (usuario, en la anotación del 2026-11-12: «Sí seguimos con el texto normalizado.»): (1) sin espacios en blanco fuera de las cadenas; (2) `true`, `false` y `null` tal cual; (3) cada número con los mismos caracteres con que se escribió (`1.50e+3` sigue siendo `1.50e+3`, `-0` sigue siendo `-0`); (4) cada cadena y cada nombre de propiedad con sus escapes decodificados y escritos otra vez así: todos los caracteres tal cual en UTF-8, salvo `"` como `\"`, `\` como `\\`, U+0008 como `\b`, U+000C como `\f`, U+000A como `\n`, U+000D como `\r`, U+0009 como `\t`, y los demás de U+0000 a U+001F como `\u00` seguido de dos cifras hexadecimales en minúscula (`\u0000`, `\u001f`); así, `\/` se escribe `/`, `é` se escribe `é` y `😀` se escribe `😀`; (5) las propiedades de cada objeto en el orden en que llegaron. Normalizar un texto normalizado da el mismo texto, y el texto normalizado nunca es más largo que el dado. Por ejemplo, ` { "b" : 1.50 , "a" : [ true , "é\/" ] } ` se guarda y se lee como `{"b":1.50,"a":[true,"é/"]}`. El texto normalizado no es canónico: `{"a":1,"b":2}` y `{"b":2,"a":1}`, o `{"n":1}` y `{"n":1.0}`, son documentos distintos (anotación del padre). El tamaño de la fila que cuenta E18 incluye el texto normalizado, no el dado: un documento indentado cuyo texto pasa del máximo de la fila se admite si su texto normalizado cabe. |
| JS5 | **Leer un documento**: una columna JSON se lee como cualquier columna, por su nombre o por su posición (E11), en una proyección con una columna preparada JSON (PY2, PY3), y como columna incluida de una entrada de índice (IX8). Devuelve nulo si la columna es nula o, si no, el documento como un valor JSON (JS6), cuyo texto normalizado entero se obtiene sin copiarlo (D11), con la vida de las demás referencias de la transacción (B9, E16): de la fila, al leerla por su clave, al recorrerla o con una proyección, y de la entrada del índice, sin leer la fila, al leerlo como columna incluida (IX8). De una fila copiada (PY3) se devuelve en una copia propia de la aplicación, como sus demás valores, que se navega y se lee como un tipo igual que un documento leído (JS6, JS7), y cuyos valores y textos valen mientras viva esa copia. Una columna JSON se prepara solo como JSON, y una de otro tipo no se prepara como JSON: los dos casos fallan con esquema distinto, con el tipo pedido y el de la columna (PY2). Leer no comprueba el documento guardado (L1): navegar o leer como un tipo un documento de una fila mal formada sigue L2, así que puede devolver un valor que no es el escrito o terminar con un pánico normal del hilo, nunca con un desbordamiento de la pila ni sin terminar. La igualdad al guardar es la del padre: una actualización con un documento cuyo texto normalizado es el mismo que el guardado es idéntica (E10), así que otro documento que solo difiere en espacios o en la forma de sus escapes no cambia la fila; `{"n":1}` frente a `{"n":1.0}` sí la cambia. |
| JS6 | **Navegar un valor JSON**: de cualquier valor JSON se obtiene su **clase**, una de objeto, array, cadena, número, booleano o `null`, y su **texto**, el fragmento del texto normalizado del documento que lo forma, sin copiarlo, que es a su vez un texto normalizado válido: de `{"a":[1,2],"s":"x"}`, el valor de `a` tiene el texto `[1,2]` y el de `s`, `"x"`, con sus comillas. De un objeto se obtienen su número de propiedades, sus propiedades en el orden guardado, cada una con su nombre y su valor, y el valor de una propiedad por su nombre, comparado como en JS2, o no está; de un array, su número de elementos, sus elementos en orden y el elemento de una posición, contando desde 0, o no está si la posición es la del número de elementos o mayor. Pedir a un valor de otra clase sus propiedades, su número de propiedades o una propiedad, o sus elementos, su número de elementos o un elemento, da no está; un objeto o un array vacíos dan cero y ninguna propiedad o elemento. Un nombre de propiedad se obtiene como texto, sin comillas, con las reglas de copia de una cadena leída como texto (JS7). De cualquier valor JSON se obtiene también el valor de un **puntero JSON** (RFC 6901), relativo a ese valor. Un puntero vacío (`""`) da el propio valor; si no, es una secuencia de pasos, cada uno `/` seguido de un nombre en el que `~1` es `/` y `~0` es `~`; cada paso sobre un objeto da su propiedad de ese nombre; sobre un array, el elemento de esa posición si el paso es `0` o un número de cifras decimales sin ceros a la izquierda (`10`, no `010` ni `+1`), y no está en otro caso, también `-`, que en RFC 6901 nombra la posición después del último elemento; y sobre una cadena, un número, un booleano o `null`, no está. Así, en `{"a/b":{"~":[10,20]}}`, el puntero `/a~1b/~0/1` da `20`. Un puntero mal formado, uno no vacío que no empieza por `/` o con un `~` que no sigue de `0` ni de `1`, falla con valor no válido y la regla puntero JSON mal formado, sin tabla, sin columna y sin más datos, antes de navegar, también si un paso anterior ya diera no está. Navegar no lee páginas que no sean las de la fila, no es un cambio para T10 y no termina referencias (E16). |
| JS7 | **Leer un valor JSON como un tipo de Krummi** (precisa la anotación del 2026-11-12 del padre): de cualquier valor JSON se pide su lectura como uno de estos tipos, y da el valor o nulo, nunca un error por el valor. **Texto**, de una cadena, sin sus comillas, sin copiarlo si la cadena no tiene escapes en el texto normalizado y decodificado en una copia propia de la aplicación si los tiene (`"a\nb"`). **Booleano**, de `true` o `false`. **`i64`** y **`u64`**, de un número cuyo valor exacto es un entero en su rango, en cualquier notación: `100`, `100.0`, `1e2` y `1.00e2` dan 100, `-0` y `0e999999999` dan 0, y `100.6`, `1e-1`, `9223372036854775808` (en `i64`) o `-1` (en `u64`) dan nulo. **`decimal(p, s)`**, de un número redondeado a la escala `s` como en E8 (`1.235` da `1.24` en un `decimal(10, 2)`), y nulo si redondeado tiene más de `p` cifras (`99.995` en un `decimal(4, 2)`). **`f64`**, de un número redondeado al más cercano, con `-0` como `+0` (N4): `0.1` y `0.10000000000000001` dan el mismo `f64`, `1e-400` y `-1e-400` dan `+0`, y fuera del rango finito da nulo (`1e400`). Un número se lee sin desarrollar su exponente: el tiempo y la memoria de la lectura crecen con la longitud de su texto, no con su valor (`1e999999999999` da nulo en `i64` y en `decimal` sin construir sus cifras). **Fecha**, de una cadena exactamente `AAAA-MM-DD`, con un año de cuatro cifras, un mes de 01 a 12 y un día que existe en ese mes, dentro del dominio de E8 (`"2026-11-11"`). **Instante**, de una cadena `AAAA-MM-DDTHH:MM:SS`, con la fecha como en la lectura de una fecha, la hora de 00 a 23, los minutos y los segundos de 00 a 59, una fracción de segundo opcional, un punto seguido de una o más cifras, y un desfase `Z`, `+HH:MM` o `-HH:MM` con la hora de 00 a 23 y los minutos de 00 a 59 (`-00:00` es UTC), con `T` y `Z` en mayúscula; se convierte a UTC, la fracción se redondea al microsegundo como en E8 y el dominio de E8 se comprueba después de convertir y de redondear (`"2026-11-11T10:23:00.1234567Z"` da `2026-11-11T10:23:00.123457Z`; `"2026-11-11T00:30:00+01:00"` da `2026-11-10T23:30:00Z`). Dan nulo: un valor de otra clase (un número donde se pide un texto, una fecha o un instante; una cadena donde se pide un número), `null`, una fecha que no existe (`"2026-02-30"`), un año que no tiene cuatro cifras o es `0000`, un instante sin desfase, en minúsculas, con un espacio en lugar de `T`, con un segundo intercalar (`:60`), con un punto sin cifras o fuera del dominio. Una `p` o una `s` fuera de los límites de N4 (1 ≤ p ≤ 38, 0 ≤ s ≤ p) falla con valor no válido y la regla nueva escala de lectura no válida, sin tabla, sin columna y sin más datos, antes de mirar el valor. Lo que no está no tiene valor que leer: la lectura tipada se pide sobre un valor JSON obtenido al navegar (JS6). |
| JS8 | **Errores: datos y precedencia** (amplía E17): un documento no válido falla con **valor no válido**, con la tabla, la columna y la regla nueva **JSON no válido**, que va en la lista de E17 después de texto mal formado; como una columna JSON solo puede incumplir nulo no admitido, tipo distinto o esta, el orden de E17 no cambia para las demás. Además lleva: la **causa**, una de UTF-8 no válido, sintaxis, sustituto suelto, propiedad repetida (con el nombre repetido, decodificado) o más de 128 niveles; la **posición**, el número de bytes del texto dado antes del fallo, contando desde 0; y el **puntero JSON** (RFC 6901) del objeto o el array más interior abierto en esa posición, con cada nombre escrito con `~0` y `~1` y sus demás caracteres tal cual, `""` si es la raíz o no hay ninguno. Cada causa tiene su posición: **UTF-8 no válido**, el primer byte de la primera secuencia que no es UTF-8 válido, también una incompleta al final; **sintaxis**, solo la gramática de RFC 8259 (sin las reglas de las demás causas), en el primer carácter con el que el texto deja de ser el principio de algún texto de esa gramática, o la longitud del texto si termina antes de completar uno (un texto vacío da la posición 0); **sustituto suelto**, la `\` del escape que no forma par; **propiedad repetida**, la comilla de apertura del nombre que repite uno anterior del mismo objeto, cuando ese nombre es una cadena sin fallos; y **más de 128 niveles**, el `{` o el `[` que abre el nivel 129. Si el texto tiene varios fallos, se informa el de menor posición y, en la misma posición, en el orden de la lista de causas. Por ejemplo, `{"a":1,"b":{"a":2,"a":3}}` falla con propiedad repetida, el nombre `a`, la posición 18 y el puntero `/b`. Un documento válido que hace la fila más grande que su máximo falla con tamaño no válido (E18), después de valor no válido según E17, con el tamaño de la fila que cuenta JS4. En una carga masiva, el error es el de la primera fila que falla, como en E13. Los datos de los errores se dan aparte de su texto (D8). |
| JS9 | **Valor por defecto** (precisa la anotación del padre): el valor por defecto de una columna JSON se da como en JS3, se guarda normalizado y Describir lo devuelve normalizado; el documento `null` es un valor por defecto, no la falta de él, así que en una columna JSON no vale la frase de E4 «declarar nulo como valor por defecto equivale a no tenerlo» para el documento `null` (el nulo de la columna sigue equivaliendo a no tenerlo). Uno que no es un documento válido hace fallar la creación, el cambio de esquema o la apertura con un esquema con esquema no válido y la regla de E5 de un valor por defecto que no cumple las reglas de su columna, con la tabla y la columna, sin la causa, la posición ni el puntero de JS8, con la precedencia de E17 (antes que esquema distinto). El esquema demasiado grande de E5 cuenta el valor por defecto normalizado. Al abrir una tabla con un esquema (E6), dos valores por defecto JSON son el mismo cuando su texto normalizado es el mismo: `{ "a" : 1 }` es compatible con `{"a":1}`, y el valor por defecto `null` no lo es con la falta de valor por defecto, que da esquema distinto. |
| JS10 | **Estadísticas** (lo que ST1 deja a 7.5): como las columnas vector, las columnas JSON no tienen estadísticas (usuario, entre ninguna y solo la fracción de nulos: «Ninguna, como vector»); estimar, leer las estadísticas o analizar una columna JSON falla con esquema no válido y una regla nueva, columna JSON, con la tabla y la columna, como la regla columna vector de ST12 y en su mismo lugar de la precedencia, y una igualdad en varias columnas (ST18) que incluye una columna JSON falla igual. Analizar la tabla entera construye las de las demás columnas, que son las mismas que en una tabla igual sin la columna JSON, y el muestreo no lee los documentos. Las estadísticas de las rutas dentro de un JSON llegan con los índices sobre expresiones (documento de diseño 14). |
| JS11 | **Construir un documento**: «construirlo», del reparto del padre, es guardarlo desde su texto (JS3). No hay un constructor de objetos y arrays sin texto: la aplicación construye el texto con su propia biblioteca de JSON y Krummi lo valida al escribirlo (usuario, entre dejarlo fuera de 7.5 y uno mínimo: «No, fuera de 7.5»). Añadirlo después es un cambio compatible. |
| JS12 | **Formato**: un motor del formato 13 no sabe leer un almacén con columnas JSON, así que el cambio es incompatible (B12) y la versión del formato pasa a 14: un almacén del formato 13 es de formato anterior y este motor no lo abre (F11). La información del almacén (P12) no cambia de forma. |
| JS13 | **Medición** (N11, con la anotación del 2026-11-20 del padre): una pasada de una ronda de las cargas W1–W32 antes y después del cambio, y de 10 a 20 rondas de las afectadas por él, que son las de tablas sin índices, W23 a W29 (insertar, cargar, leer, recorrer, actualizar y borrar filas, cuya codificación gana un tipo de columna), y de las que en esa pasada se aparten más de un 3 % de la base. No hay cargas nuevas del banco ni umbrales nuevos: Voron no tiene un tipo JSON (usuario, entre una medición informativa, cargas nuevas frente a Voron con el blittable de RavenDB y ninguna medición nueva: «Medición informativa»). Además, el informe de cierre mide, en Windows y en Linux, con la mediana de 10 a 20 rondas, sobre una tabla con una clave entera y una columna JSON y con documentos generados con una semilla fija, de unos 200 bytes (100.000 filas), unos 4 KB (100.000 filas) y unos 64 KB (10.000 filas), cada uno con propiedades antes y después de un array grande: insertarlos en escrituras de 1.000 filas, como W23; leer por la clave el documento entero; y leer por la clave la primera propiedad de la raíz y la última. Cada medida se compara con la misma tabla con el mismo texto normalizado en una columna `string`, leído entero, que es su suelo. |
| JS14 | **El lector, «muy bien probado»** (usuario, en la anotación del padre: «El propio, pero muy bien probado.»): Krummi analiza el texto con un lector propio, sin dependencias nuevas de la biblioteca y sin código no seguro (D12); las pruebas pueden usar `serde_json` con sus opciones por defecto, solo como dependencia de las pruebas, que no llega a la biblioteca. Se prueba por capas: un corpus de conformidad público, un modelo de referencia, una comparación con otra biblioteca, números fuera de `f64` y `u64`, entradas pequeñas exhaustivas, pruebas de propiedades, límites, mutantes y fuzzing (AC-JS11). |

*Anotaciones del 2026-11-22 desde los valores temporales en microsegundos ([tables-temporal-micros.prd.md](tables-temporal-micros.prd.md)):*

- *JS7: el instante leído se devuelve en microsegundos, en un entero de 64 bits con signo; una hora o una duración que
  la aplicación convierte desde el tipo más cercano se dan a Krummi en microsegundos (TU4).*

*Anotaciones del 2026-11-23 desde la intercalación de las columnas de texto ([tables-collation.prd.md](tables-collation.prd.md), 7.7):*

- *AC-JS12: un almacén nuevo es del formato que fija IC10, no del 14.*

### Estado de partida

Verificado el 2026-11-21 en el código, en los PRD de los ascendientes y hermanos y en los documentos de diseño:

- Las tablas tienen 21 tipos de columna (N4) y ninguno es JSON; en la biblioteca no hay código JSON, ni lector ni
  dependencia que analice JSON, y no existe aún el código de las consultas (módulo 17).
- El catálogo guarda el tipo de cada columna y rechaza como daño un tipo que no conoce. La versión del formato es la 13.
- Las columnas `string`, `bytes` y vector se leen sin copia (D11), por su nombre, por su posición, en proyecciones con
  columnas preparadas de texto, bytes y vector, y como columnas incluidas de un índice; las demás, en una copia propia
  de la aplicación. Leer no comprueba el contenido de la fila (L1, L2).
- Las columnas vector son el precedente de un tipo que no ordena: no pueden ser de la clave primaria (esquema no
  válido, E5) ni de la clave de un índice (esquema no válido, IX3), sí columnas incluidas, y no tienen estadísticas
  (esquema no válido con la regla columna vector, ST12).
- 7.2 ya excluye el JSON de la clave de un índice (IX1, IX3) y devuelve sin copia el JSON de una columna incluida (IX8);
  7.3 deja a 7.5 las estadísticas de las columnas JSON (ST1); las proyecciones dejan a 7.5 su columna preparada.
- Valor no válido lleva la tabla, la columna y una regla de la lista de E17; esquema no válido, la tabla, la columna y
  una regla de E5.
- El banco tiene 32 cargas; las de tablas, W23 a W32, solo las ejecutan Krummi y Voron, y ninguna tiene una columna
  JSON.

## Actores

- **Desarrollador de la aplicación (Rust)**: declara columnas JSON, guarda documentos y los lee enteros, los navega y
  lee sus propiedades como tipos de Krummi.
- **Consultas (módulo 17)**: usarán la navegación y la lectura tipada de este PRD para filtrar por rutas.
- **Mantenedor**: ejecuta las pruebas del lector y las mediciones de JS13 en Windows y en Linux.

## Flujo

1. La aplicación crea una tabla con una columna JSON, o la añade a una tabla existente (JS1).
2. Guarda filas con el documento como texto; Krummi lo valida y lo guarda normalizado, o falla sin cambiar nada (JS3,
   JS8).
3. Lee la fila y obtiene el documento entero como texto, sin copia (JS5).
4. Navega a una propiedad por su nombre, su posición o un puntero JSON (JS6), y la lee como un tipo de Krummi (JS7).

## Validaciones y mensajes

Valen las de los ascendientes. Este PRD añade, solo en la biblioteca:

| Situación | Clase de error |
|---|---|
| Crear una tabla con una columna JSON en la clave primaria | Esquema no válido, regla de E5 de una columna de la clave que admite nulos, es un vector o tiene valor por defecto (JS1) |
| Crear un índice con una columna JSON en la clave | Esquema no válido, regla de IX3 de una columna vector o JSON en la clave |
| Cambiar el tipo de una columna a `json` o desde `json` | Esquema no válido, regla de E5 de un cambio que N3 no admite |
| Un valor por defecto de una columna JSON que no es un documento válido, al crear, al cambiar el esquema o al abrir | Esquema no válido, regla de E5 de un valor por defecto que no cumple las reglas de su columna (JS9) |
| Abrir una tabla con un esquema cuyo valor por defecto JSON tiene otro texto normalizado, o lo tiene donde la tabla no | Esquema distinto (E6, JS9) |
| Guardar en una columna JSON un texto o unos bytes que no son un documento válido (JS2) | Valor no válido, regla JSON no válido, con la causa, la posición y el puntero (JS8) |
| Guardar un valor `string` o `bytes` en una columna JSON, o un valor JSON en una de otro tipo | Valor no válido, regla tipo distinto del de la columna (E17) |
| Guardar un nulo en una columna JSON que no los admite | Valor no válido, regla nulo no admitido (E17) |
| Un documento válido que hace la fila más grande que su máximo | Tamaño no válido (E18, JS4) |
| Preparar una columna JSON como otro tipo, o una de otro tipo como JSON | Esquema distinto (PY2, JS5) |
| Navegar con un puntero JSON mal formado | Valor no válido, regla puntero JSON mal formado (JS6) |
| Leer como `decimal(p, s)` con `p` o `s` fuera de los límites de N4 | Valor no válido, regla escala de lectura no válida (JS7) |
| Abrir un almacén del formato 13 | Formato anterior (F11, JS12) |
| Estimar, leer las estadísticas o analizar una columna JSON, o estimar una igualdad en varias columnas que la incluye | Esquema no válido, regla columna JSON (JS10) |

Navegar a algo que no está, o leer como un tipo un valor de otra clase, no es un error (JS6, JS7).

## Salida

El módulo no escribe nada en pantalla. Leer una columna JSON devuelve nulo o el documento, cuyo texto entero es el
texto normalizado de JS4, sin copia (JS5). Navegar devuelve valores JSON, cuyo texto es un fragmento del normalizado,
sin copia, o no está (JS6). La lectura tipada devuelve el valor o nulo (JS7); el texto de una cadena o de un nombre de
propiedad sin escapes, sin copia, y con escapes, en una copia propia de la aplicación. Describir muestra el tipo `json`
y el valor por defecto normalizado (JS1, JS9). Los errores llevan los datos de JS8. Las lecturas de las demás
columnas, los recorridos, las estimaciones de las demás columnas y la información del almacén no cambian su salida.

## Reutilización

- La validación previa de los valores y la precedencia de los errores de 7.1 (E17), con reglas nuevas.
- Las lecturas sin copia de las columnas variables (D11), las proyecciones (PY2, PY3), las columnas incluidas (IX8) y
  las reglas de una fila mal formada (L1, L2).
- Las reglas de las columnas vector para un tipo que no ordena: fuera de las claves (E5, IX3) y sin estadísticas
  (ST12, JS10).
- El redondeo y los dominios de E8 para la lectura tipada como `decimal`, fecha e instante.
- La igualdad de la actualización idéntica (E10), la compatibilidad de esquemas (E6) y el tamaño máximo de una fila
  (E18).
- El generador de valores al azar de las pruebas de las tablas para las pruebas de propiedades del lector (AC-JS11).

## Fuera de alcance

- Actualizar o quitar una propiedad por su ruta dentro del motor: la aplicación lee el documento, lo cambia y lo
  guarda (anotación del padre; usuario: «no en 7.5»).
- Leer una propiedad sin recorrer el texto del documento: lo decide el módulo 17 con su carga del banco que filtra por
  una ruta, y la forma guardada lo permite sin reescribir las filas (anotación del padre; usuario: «Después, pero que
  quede anotado»).
- Las rutas como listas de pasos en las consultas, la igualdad por significado (propiedades en cualquier orden, números
  por su valor), los filtros y las proyecciones por rutas, las condiciones sobre los elementos de un array y la lectura
  tipada dentro de las consultas: módulo 17.
- La lectura tipada como los demás tipos de N4 (`i8` a `i32`, `u8` a `u32`, `f32`, `uuid`, `ulid`, `time`,
  `duration`, `bytes`): la aplicación lee el tipo más cercano de JS7 y lo convierte.
- Los índices sobre JSON (sobre expresiones, multientrada e invertidos): documento de diseño 14.
- Una forma guardada binaria o canónica, y la compresión de los documentos (7.4, aplazada).
- Ordenar documentos: un JSON no es clave primaria ni columna de la clave de un índice.
- JSONPath (RFC 9535), JSON5, comentarios y validar documentos contra un esquema JSON.
- Guardar el texto exacto que llegó, con sus espacios y sus escapes: para eso está una columna `string` o `bytes`.
- Convertir una columna `string` en `json`, o al revés (N3).
- Construir un documento sin texto, con un constructor de objetos y arrays (JS11).
- La interfaz C y el binding C#, incluido subir a 128 la profundidad de .NET al leer los documentos (módulo 18).
- Importar y exportar documentos por líneas (módulo 19) y mostrar o validar documentos desde la herramienta de línea
  de comandos (módulos 12 y siguientes).

## Dejado a la implementación

- Los bytes exactos que se guardan de cada documento, con el punto de partida de *Stored form v1* del documento de
  diseño 13, siempre que su texto sea el de JS4 y la forma permita añadir más adelante la lectura sin recorrer el
  texto sin reescribir las filas.
- Cómo valida y normaliza el lector, siempre que cumpla JS2, JS4 y JS8 y no desborde la pila ni tarde un tiempo
  cuadrático con ningún texto (AC-JS11).
- El texto en inglés de los errores (D8); los nombres de las operaciones, los tipos y las reglas, en inglés y con las
  convenciones de Rust.
- Cómo se construyen los ganchos de prueba de los criterios: son mecanismos de prueba, no contratos públicos.

## Criterios de aceptación

Valen los de los ascendientes. Cada decisión la comprueban: JS1, AC-JS1 y AC-JS8; JS2, AC-JS1, AC-JS3 y AC-JS11; JS3,
AC-JS3; JS4, AC-JS1, AC-JS2 y AC-JS10; JS5, AC-JS4, AC-JS7 y AC-JS9; JS6, AC-JS4 y AC-JS5; JS7, AC-JS6; JS8, AC-JS3 y
AC-JS10; JS9, AC-JS8; JS10, AC-JS13; JS11, ninguno (no añade operaciones); JS12, AC-JS12; JS13, AC-JS14; JS14,
AC-JS11.

1. **AC-JS1** — Dado un corpus de documentos con cada clase de valor, Unicode de 1 a 4 bytes, todos los escapes, los
   números `-0`, `0`, `1E400`, `-1e-400`, `123456789012345678901234567890`, `18446744073709551616`, `0.1`,
   `0.10000000000000000000001`, `1.0` y `1`, documentos de 128 niveles (solo objetos, solo arrays, alternados
   empezando por cada uno, y diez combinaciones al azar con una semilla fija), un objeto y un array vacíos y objetos
   de miles de propiedades, cuando se guarda cada uno en una columna JSON y se lee, entonces el texto leído es su texto
   normalizado (JS4), representa el mismo valor que el dado según el modelo de referencia de AC-JS11, con el orden de
   las propiedades y el texto de cada número, y normalizado otra vez da el mismo texto; y cuando se cierra y reabre el
   almacén, y en un almacén temporal, se leen igual.
2. **AC-JS2** — Dados los textos ` { "b" : 1.50 , "a" : [ true , "é\/" ] } `, `"A\u0000\u001F\t\""`,
   `"😀"` y un documento de unos 4 KB indentado, cuando se guardan, entonces se leen como
   `{"b":1.50,"a":[true,"é/"]}`, `"A\u0000\u001f\t\""`, `"😀"` y el mismo texto que el documento sin indentar,
   byte a byte.
3. **AC-JS3** — Dado cada texto de esta tabla, cuando se inserta, se actualiza y se inserta o actualiza en una
   escritura, entonces cada operación falla con valor no válido, la regla JSON no válido y la causa, la posición y el
   puntero de la tabla; la tabla, el contador de modificaciones (ST4) y la transacción quedan como antes y la
   transacción puede confirmar sus otros cambios; y cuando se carga en masa una secuencia cuya última fila trae uno de
   ellos, entonces la carga aborta entera con ese error (E13). Un `string` o unos `bytes` en una columna JSON fallan
   con tipo distinto, y un nulo en una que no los admite, con nulo no admitido.

   | Texto (un byte que no es texto, en hexadecimal entre `<` y `>`; `\` es siempre el escape de JSON) | Causa | Posición | Puntero |
   |---|---|---|---|
   | (vacío) | sintaxis | 0 | `""` |
   | tres espacios | sintaxis | 3 | `""` |
   | `1 2` | sintaxis | 2 | `""` |
   | `<EF><BB><BF>1` (BOM) | sintaxis | 0 | `""` |
   | `NaN`, `+1`, `.5`, `'a'` | sintaxis | 0 | `""` |
   | `01` | sintaxis | 1 | `""` |
   | `1.` | sintaxis | 2 | `""` |
   | `[1,]` | sintaxis | 3 | `""` |
   | `{"a":1,}` | sintaxis | 7 | `""` |
   | `{"a":1}/*c*/` | sintaxis | 7 | `""` |
   | `["a` + tabulador + `"]` | sintaxis | 3 | `""` |
   | `"\x"` | sintaxis | 2 | `""` |
   | `"\u12"` | sintaxis | 5 | `""` |
   | `"\ud800"`, `"\udc00"`, `"\ud800A"` | sustituto suelto | 1 | `""` |
   | `<FF>` | UTF-8 no válido | 0 | `""` |
   | `["<E2><28>"]` | UTF-8 no válido | 2 | `""` |
   | `"<E2><82>` | UTF-8 no válido | 1 | `""` |
   | 129 `[` y 129 `]` | más de 128 niveles | 128 | `/0` repetido 127 veces |
   | `{"a":1,"a":2}` | propiedad repetida, `a` | 7 | `""` |
   | `{"a":1,"b":{"a":2,"a":3}}` | propiedad repetida, `a` | 18 | `/b` |
   | `{"x":{"a/~":[{"k":1,"k":2}]}}` | propiedad repetida, `k` | 20 | `/x/a~1~0/0` |
   | `{"\ud800":1,"\ud800":2}` | sustituto suelto | 2 | `""` |

4. **AC-JS4** — Dada una tabla con una columna JSON que admite nulos, cuando se guardan una fila con la columna nula y
   otra con el documento `null`, entonces la primera se lee nula y la segunda como un valor de clase `null`; y en el
   documento `{"a":null}`, navegar a `a` da un valor `null` y navegar a `b` da no está.
5. **AC-JS5** — Dado el documento `{"a/b":{"~":[10,20]},"x":[1,2,3],"s":"t","o":{},"e":[]}`, cuando se navega,
   entonces sus propiedades vuelven en ese orden con su nombre y su valor; `/a~1b/~0/1` da `20`, `""` da el documento,
   `/x/0` da `1`, y `/x/3`, `/x/-`, `/x/01`, `/x/+1`, `/s/0` y `/nada` dan no está; el texto del valor de `x` es
   `[1,2,3]` y el de `s`, `"t"`; `x` tiene 3 elementos, `o` cero propiedades, `e` cero elementos y el documento 5
   propiedades; pedir propiedades o una propiedad a `x`, o elementos o un elemento a `o` o a `s`, da no está; y los
   punteros `a`, `/~2`, `/a~` y `/s/0/~2` fallan con valor no válido y la regla puntero JSON mal formado.
6. **AC-JS6** — Dados números y cadenas en los límites de cada lectura tipada de JS7 y uno más allá (`i64::MIN`,
   `i64::MAX`, `u64::MAX` y el siguiente en cada notación; `decimal(38, s)` con 38 cifras y con 39 tras redondear; el
   mayor `f64` finito, `1e400` y `1e-400`; `1e999999999999` como `i64` y como `decimal`, que tarda, en la mediana de
   cinco ejecuciones, como mucho 10 veces lo que leer `1e9`; fechas e instantes en los extremos del dominio de E8, también cuando el redondeo o el desfase los sacan; el
   29 de febrero de un año bisiesto y de uno que no lo es; los desfases `+23:59`, `-00:00`, `+24:00` y `+00:60`),
   cuando se leen, entonces cada uno da el valor o el nulo de JS7, y cada ejemplo de JS7 da lo que dice; y leer como
   `decimal(0, 0)` y `decimal(39, 0)` falla con la regla escala de lectura no válida, también sobre una cadena.
7. **AC-JS7** — Dada una fila con una columna JSON, cuando se lee por posición, por nombre y con una proyección,
   entonces el texto del documento y el de una cadena sin escapes apuntan a la memoria de la fila leída, y cuando se lee
   como columna incluida de una entrada de índice, a la memoria de esa entrada, sin leer la fila, comprobado como las
   pruebas de 7.1 y 7.2 comprueban que el texto de una columna `string` no se copia; una cadena con escapes y un nombre con escapes
   se devuelven en una copia, y una fila copiada devuelve el documento en una copia propia de la aplicación; y preparar
   la columna JSON como `string` o como `bytes`, o una columna `string` como JSON, falla con esquema distinto.
8. **AC-JS8** — Dada una definición de tabla con una columna JSON en la clave primaria, y una de índice con una en la
   clave, cuando se crean, entonces fallan sin efecto con esquema no válido y sus reglas (JS1); un índice con una
   columna JSON incluida se crea y la devuelve (IX8), y quitar esa columna falla con esquema no válido (E5); cambiar el
   tipo de una columna a `json` o desde `json` falla con esquema no válido; cuando se añade a una tabla con filas una
   columna JSON que admite nulos, otra con el valor por defecto `{"a": 1}` y otra con el valor por defecto `null`,
   entonces las filas anteriores leen nulo, `{"a":1}` y un valor `null`, y Describir muestra `json` y `{"a":1}`; un
   valor por defecto que no es un documento válido falla con esquema no válido y la regla de E5 de un valor por
   defecto que no cumple las reglas de su columna (JS9); y abrir la tabla con `{ "a" : 1 }` como valor por defecto es compatible, y sin valor por defecto en la
   columna de `null`, da esquema distinto.
9. **AC-JS9** — Dada una fila con `{"a":1,"s":"é"}`, cuando se actualiza con `{ "a" : 1 , "s" : "é" }`, entonces
   la actualización es idéntica (E10) y no suma al contador de modificaciones (ST4); y cuando se actualiza con
   `{"a":1.0,"s":"é"}` o con `{"s":"é","a":1}`, entonces la fila cambia y suma.
10. **AC-JS10** — Dada una tabla con una clave entera y una columna JSON, cuando se busca la longitud N del texto
    normalizado más largo que admite la columna, entonces un documento de N bytes se guarda y se lee igual, uno de N + 1
    falla con tamaño no válido, y N es como mucho 8 bytes menor que el texto más largo que admite una columna `string`
    en una tabla igual; y el documento de N bytes indentado, cuyo texto dado pasa del máximo de la fila, se guarda
    (JS4).
11. **AC-JS11** — Las pruebas del lector (JS14), en Windows y en Linux salvo el fuzzing: (1) el corpus JSONTestSuite de
    Nicolas Seriot, donde los `y_` se aceptan y los `n_` se rechazan, cada `i_` tiene un resultado fijado por una
    prueba, y los `y_` que Krummi rechaza por una regla más estricta son excepciones nombradas con su regla y su error
    (hoy `y_object_duplicated_key.json` y `y_object_duplicated_key_and_value.json`, propiedad repetida), de modo que
    una diferencia no listada hace fallar la prueba; (2) un modelo de referencia de solo las pruebas, con los números
    como texto y las propiedades en orden, que genera los documentos sin pasar por ningún analizador de JSON, contra
    lo que Krummi lee de ellos; (3) una comparación con `serde_json` con sus opciones por defecto sobre cada documento
    del corpus y generado, que coincide en aceptar o rechazar salvo las diferencias listadas (claves repetidas,
    profundidad y números que `serde_json` no representa), y en el valor de los documentos cuyos números representa
    exactamente; (4) los números de AC-JS1 fuera de `f64` y `u64`, guardados y leídos con su texto, distintos entre sí y
    nulos en las lecturas tipadas que no los admiten; (5) todos los textos hasta una longitud fija sobre los caracteres
    de los números (`0-9 . e E + -`) contra la gramática de RFC 8259, y cada escape `\uXXXX` solo y en pares
    (sustitutos emparejados, sueltos e invertidos); (6) pruebas de propiedades con una semilla fija, con documentos al
    azar, reescritos con espacios y escapes legales al azar, que dan el mismo texto normalizado, y un byte al azar
    cambiado en un documento válido, que da un error o otro documento válido, nunca un pánico; (7) los niveles de
    AC-JS1, aceptados con 128 y rechazados con 129, y validar un objeto de 1.000.000 de propiedades distintas, y otro
    con una repetida al final, tarda, en la mediana de cinco ejecuciones, como mucho 20 veces lo que uno de 100.000; (8)
    mutantes hechos a mano de la tabla de escapes, la comprobación de claves repetidas, el contador de profundidad y la
    gramática de los números, cada uno detectado por alguna prueba; y (9) fuzzing en Linux, al menos una hora por objetivo, del
    lector sobre bytes arbitrarios, sin pánicos ni desbordamientos de pila, y de la navegación y la lectura tipada
    sobre documentos válidos, sin pánicos; el informe de cierre da la duración y el número de ejecuciones de cada
    objetivo, y las entradas que hagan fallar algo se corrigen y se añaden a las pruebas de (6).
12. **AC-JS12** — Dado un almacén del formato 13, cuando lo abre este motor, entonces falla con formato anterior (F11);
    y un almacén nuevo es del formato 14.
13. **AC-JS13** — Dada una tabla con una columna JSON y otras columnas, cuando se estima, se leen
    las estadísticas o se analiza la columna JSON, o se estima una igualdad en varias columnas que la incluye, entonces
    falla con esquema no válido y la regla columna JSON; y cuando se analiza la tabla, las demás columnas tienen las
    mismas estadísticas que en una tabla igual sin la columna JSON, y la columna JSON no tiene.
14. **AC-JS14** — Dado el cierre de 7.5, cuando se revisa su informe, entonces incluye las mediciones de JS13 en
    Windows y en Linux: la pasada de W1–W32 antes y después, las rondas de las cargas afectadas, y cada medida de los
    documentos con la de su suelo `string` y la razón entre las dos.

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones del 2026-11-21 (en sus
traducciones también):

- Tablas, N3 («ninguna de tipo vector» en la clave primaria) y N5 («Los vectores no se ordenan: no pueden ser clave
  primaria ni columnas de un índice»): valen también para las columnas JSON, que pueden ser columnas incluidas (JS1).
  N4: gana el tipo `json` (JS1). *Reparto*, anotación del 2026-11-12 de 7.5: lo que «Sigue fijando 7.5» queda fijado en
  este PRD (JS1, JS8, JS12, JS13).
- Tablas con esquema, E4 y E5: la regla de una columna de la clave que es un vector incluye la columna JSON (JS1). E4:
  en una columna JSON, el documento `null` como valor por defecto no equivale a no tenerlo (JS9). E6: dos valores por
  defecto JSON son el mismo cuando su texto normalizado es el mismo (JS9). E17: la lista de reglas de valor no válido
  gana, después de texto mal formado, JSON no válido, puntero JSON mal formado y escala de lectura no válida (JS6, JS7,
  JS8); las dos últimas son de la navegación y la lectura tipada, nunca de una fila, así que no compiten con las demás. E20: la versión del formato pasa a 14 (JS12).
- Proyecciones, PY2: hay columnas preparadas JSON, y una columna JSON solo se prepara como JSON (JS5). PY3: el JSON se
  devuelve sin copia, como el texto, los bytes y los vectores, salvo de una fila copiada. *Fuera de alcance*: la columna
  preparada JSON la fija este PRD.
- Estadísticas, ST1 y *Fuera de alcance*: las columnas JSON no tienen estadísticas, como las vector (JS10). ST12: la
  regla columna JSON va con la regla columna vector. ST18: «las columnas que no son vector» excluye también las JSON.

## Referencias

- Padre: [tables.prd.md](tables.prd.md) (N3, N4, N5, N11, *Reparto en PRD hijos* con las anotaciones de 7.5 del
  2026-11-10 y del 2026-11-12, *Salida*); raíz: [krummi.prd.md](../krummi.prd.md) (D8, D11, D12, G9).
- Hermanos: [tables-schema.prd.md](tables-schema.prd.md) (E4, E5, E6, E7, E8, E10, E11, E13, E17, E18, E20),
  [tables-row-reads.prd.md](tables-row-reads.prd.md) (L1, L2), [tables-indexes.prd.md](tables-indexes.prd.md) (IX1,
  IX3, IX8), [tables-statistics.prd.md](tables-statistics.prd.md) (ST1, ST4, ST12, ST18),
  [tables-projection.prd.md](tables-projection.prd.md) (PY2, PY3).
- Árboles y transacciones: [trees.prd.md](../trees/trees.prd.md) (B9, B12),
  [transactions.prd.md](../transactions/transactions.prd.md) (T10).
- Diseño: [documento 13](../../../design/tables/13-json-column-storage.md) (estado de partida técnico) y
  [documento 14](../../../design/tables/14-json-indexes.md) (índices sobre JSON).
- Normas: RFC 8259 (JSON), RFC 6901 (puntero JSON), RFC 3339 (fechas e instantes).

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 6ce1b30c | 2026-11-21 06:26 UTC+00:00 | 48 min | 0 min | 48 min | 45 min | 3 min | 4 |

<!-- prd-time: [{"session": "6ce1b30c-0c7f-475e-a6c5-b31016b08203", "start": "2026-11-21T06:26:46.585000+00:00", "wall": 2881, "discarded": 0, "agent": 2692, "human": 189, "interventions": 4, "idle_minutes": 10}] -->
