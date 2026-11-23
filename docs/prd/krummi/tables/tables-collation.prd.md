# Intercalación de las columnas de texto — PRD

**Versión**: 1.0
**Fecha**: 2026-11-23
**Documento padre**: [Tablas de Krummi — PRD](tables.prd.md) (v1.0)

## Objetivo y contexto

Construir el PRD hijo **7.7** del [reparto del módulo de tablas](tables.prd.md#reparto-en-prd-hijos): una columna
`string` declara su **intercalación**, que decide cuándo dos textos son el mismo valor. Con `binary`, la de hoy y la
que se toma por defecto, dos textos son iguales solo si sus bytes lo son. Con `ci`, la igualdad no distingue mayúsculas
de minúsculas. Con `ci_ai`, tampoco distingue los acentos. La intercalación de la columna rige la clave primaria, los
índices secundarios, la unicidad, los intervalos y las estadísticas.

**Por qué**: hoy la clave primaria, los índices y las estadísticas comparan el texto byte a byte, sin intercalación
(N5). Buscar `josé` no encuentra `José`, un índice único admite a la vez `Ana` y `ANA`, y el mismo texto escrito con
la `é` como un carácter o como una `e` con el acento combinante son dos valores distintos. En una base de datos eso es
poco útil. El diseño previó resolverlo con índices sobre expresiones (documento de diseño 3), pero entonces la
aplicación tendría que repetir la expresión en cada consulta. Con la intercalación como propiedad de la columna, la
regla vive en el esquema y ninguna consulta la puede olvidar. Usuario, el 2026-11-21: «Me vale con la collation como
propiedad de la columna (binary por defecto, ci y ci_ai). Me vale con que la igualdad, los prefijos y la unicidad no
distingan mayúsculas ni acentos».

**Por qué ahora**: 7.1, 7.2, 7.3 y 7.5 están cerrados, y la intercalación tiene que existir antes de que las consultas
(módulo 17) fijen cómo comparan el texto.

El orden alfabético de un idioma queda fuera: los textos de una columna `ci` o `ci_ai` se ordenan por los bytes de su
forma plegada (IC2, IC3), no como un diccionario. Ese orden deja contiguos todos los textos cuya forma plegada empieza
por la forma plegada de un mismo texto, y en eso se apoyará la búsqueda de texto por prefijo del módulo 17: en una
columna `ci`, los que empiezan por `jos` son los que empiezan por `JOS`, `Jos` o `jOs`. En este documento, «prefijo»
sin más es el de E12 e IX9, las primeras columnas de una clave; el de un texto se llama siempre **prefijo de texto**.

Este PRD hereda todo lo de sus ascendientes. Cambia N5 y *Fuera de alcance* de su padre y la decisión de intercalación
del documento de diseño 3, y precisa reglas de [tables-schema.prd.md](tables-schema.prd.md) (7.1),
[tables-indexes.prd.md](tables-indexes.prd.md) (7.2), [tables-statistics.prd.md](tables-statistics.prd.md) (7.3) y
[tables-json.prd.md](tables-json.prd.md) (7.5), que llevan su anotación (*Anotaciones a otros documentos*).

En este documento, la **intercalación** de una columna `string` es `binary`, `ci` o `ci_ai`; la **forma plegada** de
un texto es el texto que resulta de aplicarle la intercalación de su columna (IC2); y dos textos son **equivalentes** en
una columna cuando sus formas plegadas son iguales byte a byte.

### Decisiones de producto

| ID | Decisión |
|---|---|
| IC1 | **Declaración**: una columna `string` declara su intercalación como un atributo de su declaración, junto a su tipo y a si admite nulos, al crear la tabla o al añadir la columna (E4): `binary`, `ci` o `ci_ai`. Sin declararla, es `binary`, así que las tablas y las aplicaciones de hoy no cambian. Solo las columnas `string` tienen intercalación: declarar cualquiera, también `binary`, en una columna de otro tipo (`bytes`, `json` o cualquier otro) falla sin efecto con esquema no válido y la regla nueva **intercalación en una columna que no es de texto** (IC9). Una columna de la clave primaria o de un índice puede tener cualquier intercalación. La intercalación de una columna no cambia nunca: renombrarla la conserva; modificar una columna (E4) recibe también su intercalación, y darle otra, o no declararla en una columna que no es `binary`, falla sin efecto con esquema no válido y la regla de un cambio que N3 no admite. Preparar una columna en una proyección (PY2) pide el tipo `string` sin intercalación, y vale para cualquiera. Describir (E2) muestra la intercalación de cada columna `string`, también `binary`, y no muestra intercalación en las demás. |
| IC2 | **Forma plegada**, con los datos de **Unicode 17.0**, fijos. Con `binary`, el texto tal cual. Con `ci`, el resultado de descomponer el texto (NFD), plegarlo con el plegado de mayúsculas completo de Unicode (las entradas de estado C y F de `CaseFolding.txt`, sin las T, que son las del turco), volver a descomponerlo y componerlo (NFC). Con `ci_ai`, la forma plegada `ci`, descompuesta (NFD), sin las marcas que no ocupan espacio (categoría general Mn) cuya **base** tiene la escritura (propiedad Script de Unicode) latina, griega o cirílica, y compuesta (NFC); la base de una marca es el valor escalar más cercano que la precede y no es Mn, y solo cuenta la escritura de la base, no la de la marca. Se conserva la tilde (U+0303) cuyo valor escalar inmediatamente anterior en esa forma descompuesta, antes de quitar nada, es una `n`, así que la `ñ` y la `Ñ` son letras propias: `año` y `ano` no son equivalentes, y `Año` y `AÑO` sí; una tilde separada de la `n` por otra marca se quita (`n` + U+0301 + U+0303 da `n`). Las marcas cuya base es de otra escritura (el devanagari, el japonés, el tailandés, el hebreo, el árabe, el armenio, el georgiano o el copto) o no tiene base se conservan, así que en esas escrituras `ci_ai` da lo mismo que `ci`. Como `ci_ai` parte de la forma `ci`, dos textos equivalentes en `ci` lo son también en `ci_ai`. Usuario, entre quitar las marcas de todas las escrituras o solo de esas tres, y conservar la `ñ` o igualarla a la `n`: «Latín/griego/cirílico, con ñ». Plegar dos veces da lo mismo que plegar una. La forma plegada no es siempre minúscula: Unicode pliega el cheroqui a mayúsculas. Un valor escalar que Unicode 17.0 no asigna queda como está. Cambiar la versión de Unicode o el plegado es un cambio incompatible del formato (B12), porque cambia lo que está guardado en las claves. Ejemplos en *Salida*. |
| IC3 | **Comparación y orden**: allí donde Krummi compara valores de una columna `ci` o `ci_ai` (la clave primaria, las claves de los índices, la unicidad, los intervalos, las estimaciones y las estadísticas), compara sus formas plegadas byte a byte, con la regla de N5 para `string`. Una columna descendente invierte ese orden, y los nulos siguen donde los pone su índice (N5). Las entradas de un índice cuyas columnas de la clave son equivalentes siguen el orden de las entradas con la misma clave (IX8). Los valores de un intervalo (el prefijo y los límites de E12 e IX9) y de una estimación (ST2) se pliegan antes de comparar, y la semántica de los límites es la de E12 sobre las formas plegadas: en una columna `ci`, `JOSÉ`, `josé` y `José` dan el mismo intervalo; de `ANA` incluido a `ana` incluido es el intervalo de un valor; con `ana` excluido, vacío. Un intervalo vacío o invertido después de plegar no devuelve nada y no es un error (E12). |
| IC4 | **Clave primaria**: en una columna de la clave con `ci` o `ci_ai`, dos claves equivalentes son la misma fila, con todas las reglas de E10 a E13: insertar `ANA` cuando existe `Ana` falla con clave duplicada; leer, borrar, actualizar e insertar o actualizar con `ana` encuentran la fila `Ana`, e insertar o actualizar informa de que actualizó; una carga masiva con dos claves equivalentes falla como con dos claves repetidas (E13); y borrar un intervalo borra las filas de su intervalo plegado. Como la clave primaria de una fila no cambia (E10), la fila sigue siendo `Ana` aunque se haya pedido con `ANA`, y una actualización que solo da esa clave es idéntica. |
| IC5 | **Lo que se lee**: leer una fila devuelve cada texto tal como se guardó, sin copia, también en las columnas de la clave (D11, E11); la intercalación no cambia ninguna lectura de filas ni de proyecciones (PY3). Actualizar una columna `ci` que no es de la clave de `Ana` a `ANA` cambia la fila: no es una actualización idéntica (E10), y leerla da `ANA`. Las entradas de un índice (IX8) devuelven en sus columnas `ci` y `ci_ai`, de la clave del índice y de la clave primaria, el texto tal como se guardó en la fila, el mismo que daría leerla, nunca su forma plegada, decodificado en una copia propia de la aplicación como los demás valores de la clave (IX8); las columnas incluidas lo devuelven como siempre. Usuario, entre el texto original, que hace crecer las entradas, y la forma plegada: «El texto original». |
| IC6 | **Unicidad**: en un índice único, dos filas chocan cuando sus columnas son equivalentes y ninguna es nula (IX7), y crear un índice único sobre filas que ya repiten valores equivalentes falla sin efecto con clave duplicada (IX11). Clave duplicada, de la clave primaria o de un índice único, lleva los datos de E17 e IX10 con los textos de la fila que choca tal como quedarían guardados (los que dio la aplicación y, al actualizar, los que la fila conserva; en una carga, los de la fila de la carga que choca) o, al crear un índice, tal como están guardados en la fila que IX10 nombra; nunca su forma plegada. |
| IC7 | **Tamaño**: una clave primaria, una entrada de índice y un valor del prefijo o un límite de un intervalo (E12, IX9) se miden, como dicen E18 e IX10, con la forma plegada de sus columnas `ci` y `ci_ai`, que puede tener más bytes que el texto (`ŉ`, de 2 bytes, da `ʼn`, de 3; el factor máximo es 3) o menos (un texto `ci_ai` con muchas marcas). Un texto que cabe en la clave de una columna `binary` puede no caber en la de una `ci` y fallar con tamaño no válido y los datos de E18 e IX10, cuyo tamaño es el medido con la forma plegada, y al revés. Un valor de una fila sigue midiéndose como se guarda en la fila (E18), y el texto original que guarda una entrada de índice para devolverlo (IC5) no cuenta para el máximo de la clave: lo limita solo el máximo de la fila. En una estimación, la forma plegada decide si un valor es grande (ST2), que nunca da tamaño no válido. |
| IC8 | **Estadísticas** (7.3): las de una columna `ci` o `ci_ai`, las de los prefijos de índice que la incluyen (ST17) y la igualdad en varias columnas (ST18) se calculan sobre formas plegadas: `Ana` y `ANA` cuentan como un solo valor distinto; la fracción de valores grandes (ST1) cuenta los textos de más de 2.048 bytes, que no se leen, como dice ST1, y además los que tienen una forma plegada de más de 2.048 bytes; y los valores frecuentes y los límites del histograma que se leen (ST16) son formas plegadas, acortadas como dice ST16 en un límite de valor escalar de la forma plegada. Estimar la igualdad o un intervalo con `ANA` o con `ana` da lo mismo. |
| IC9 | **Errores**: la regla nueva de esquema no válido, intercalación en una columna que no es de texto, va en la lista de E5 justo después de la de la métrica coseno sobre elementos `bit`, con la columna como dato; como toda regla de E5, gana a esquema distinto en los modos que reciben un esquema (E1). Esquema distinto (E6) compara también la intercalación: la diferencia nueva **intercalación** va después de la de tipo y antes de la de nulabilidad, con lo esperado y lo encontrado (`binary`, `ci` o `ci_ai`); un esquema dado que no declara la intercalación de una columna `string` espera `binary`; entre varias columnas, se informa la primera como en las demás diferencias de E6. Dos valores por defecto de una columna `ci` son el mismo solo si son iguales byte a byte (E6), porque las filas los leen tal como se guardaron: `Ana` y `ANA` son valores por defecto distintos. No hay reglas nuevas de valor no válido: cualquier texto UTF-8 válido es válido en las tres intercalaciones (E7). |
| IC10 | **Formato**: un motor del formato 14 no sabe leer la intercalación de una columna, así que el cambio es incompatible (B12) y la versión del formato sube en uno, con las reglas de JS12: a 15 si ningún otro cambio la ha subido antes desde la 14. Un almacén del formato anterior es de formato anterior y este motor no lo abre (F11), aunque solo tenga columnas `binary`. |
| IC11 | **Unicode estable**: lo que Krummi guarda en las claves no puede cambiar porque se actualice una dependencia. La biblioteca no gana dependencias nuevas para plegar el texto, y no usa código no seguro (D12); las pruebas pueden leer los ficheros de datos de Unicode y usar otras bibliotecas solo como dependencias de las pruebas (AC-IC2). |
| IC12 | **Medición** (N11, con la anotación del 2026-11-20 del padre): una pasada de una ronda de las cargas W1–W32 antes y después del cambio, y de 10 a 20 rondas de las afectadas por él, que son las de tablas, W23 a W32, cuya codificación de claves y de esquema cambia, y de las que en esa pasada se aparten más de un 3 % de la base. No hay cargas nuevas del banco ni umbrales nuevos: las tablas de Voron no tienen intercalación. El informe de cierre mide además, en Windows y en Linux, con la mediana de 10 a 20 rondas, sobre una tabla de 100.000 filas con una clave `string` de unos 24 bytes de texto con mayúsculas y acentos, generado con una semilla fija, y un índice único sobre otra columna `string` igual: insertar en escrituras de 1.000 filas, como W23; leer por la clave; y buscar por el índice. Cada medida se compara con la misma tabla con las dos columnas `binary`, que es su suelo. |
| IC13 | **Cierre del módulo**: 7.7 se construye después de 7.5 y antes del módulo 17, y N2 cierra el módulo 7 cuando lo están 7.1, 7.2, 7.3, 7.5, 7.6 y 7.7. |

### Estado de partida

Verificado el 2026-11-23 en el código y en los PRD de los ascendientes y hermanos:

- Una columna tiene identidad, nombre, tipo, si admite nulos y valor por defecto; el tipo `string` no tiene
  parámetros. Describir devuelve esas cinco cosas de cada columna, y esquema distinto compara la falta de la columna, el
  tipo, la nulabilidad, el valor por defecto y la clave primaria (E2, E6).
- El texto se compara byte a byte en todas partes: la clave primaria, las claves de los índices, la comprobación de
  unicidad al escribir y al crear un índice, los intervalos de E12 e IX9, las estadísticas y las estimaciones. Nada en
  la biblioteca normaliza, pliega ni pasa a minúsculas el texto guardado, y la biblioteca no depende de ninguna
  biblioteca de Unicode.
- Las columnas `string` de la clave primaria se guardan también en la fila, así que leer una fila devuelve el texto
  guardado sin copia. Las entradas de un índice devuelven las columnas de su clave y de la clave primaria decodificadas
  de la propia entrada, sin leer la fila (IX8).
- No hay ninguna operación de búsqueda por prefijo de texto. Las estimaciones son igualdad, intervalo e igualdad en
  varias columnas (ST2).
- Modificar una columna solo admite permitir los nulos en una que no es de la clave (E4).
- La versión del formato es la 14 (JS12).
- El banco tiene 32 cargas; las de tablas, W23 a W32, tienen claves `ulid` y `uuid` e índices sobre un entero y un
  instante, sin ninguna columna `string` en una clave.

## Actores

- **Desarrollador de la aplicación (Rust)**: declara la intercalación de sus columnas de texto y busca, recorre y
  garantiza la unicidad sin preocuparse de mayúsculas ni acentos.
- **Consultas (módulo 17)**: compararán el texto con la intercalación de su columna y harán la búsqueda por prefijo de
  texto sobre el orden de IC3.
- **Mantenedor**: ejecuta las pruebas y las mediciones de IC12 en Windows y en Linux.

## Flujo

1. La aplicación crea una tabla, o añade una columna, con una columna `string` `ci` o `ci_ai` (IC1).
2. Guarda filas con el texto como lo escribe el usuario; Krummi lo guarda tal cual y usa su forma plegada para la clave
   primaria, los índices y las estadísticas (IC2, IC3).
3. Busca, recorre intervalos y comprueba la unicidad con cualquier combinación de mayúsculas y acentos, y obtiene las
   filas con su texto original (IC3 a IC6).

## Validaciones y mensajes

Valen las de los ascendientes. Este PRD añade, solo en la biblioteca:

| Situación | Clase de error |
|---|---|
| Declarar una intercalación, también `binary`, en una columna que no es `string`, al crear la tabla, al añadir la columna o al abrirla con un modo que recibe un esquema | Esquema no válido, regla intercalación en una columna que no es de texto (IC9) |
| Modificar una columna `string` dándole otra intercalación, o sin declararla cuando no es `binary` | Esquema no válido, regla de un cambio que N3 no admite (IC1) |
| Abrir una tabla con un esquema que da a una columna `string` otra intercalación, o no la declara y la de la tabla no es `binary` | Esquema distinto, diferencia intercalación (IC9) |
| Insertar, o cargar en masa, una fila cuya clave primaria es equivalente a la de otra | Clave duplicada (IC4, IC6) |
| Guardar una fila cuyas columnas de un índice único son equivalentes a las de otra, sin nulos | Clave duplicada, con el nombre del índice (IC6) |
| Crear un índice único sobre filas con valores equivalentes | Clave duplicada, sin cambiar nada (IX11, IC6) |
| Una clave primaria, una entrada de índice, un valor del prefijo o un límite cuya forma plegada pasa del máximo de la clave | Tamaño no válido (IC7) |
| Abrir un almacén del formato 14 | Formato anterior (F11, IC10) |

## Salida

El módulo no escribe nada en pantalla. Las filas y las proyecciones devuelven el texto tal como se guardó (IC5).
Describir muestra la intercalación de cada columna `string`, y su valor por defecto tal como se declaró (IC1). Los
recorridos de una columna `ci` o `ci_ai` devuelven las filas en el orden de sus formas plegadas (IC3). Las estadísticas
que se leen muestran formas plegadas (IC8). Esquema distinto y clave duplicada llevan los datos de IC9 e IC6. Los
almacenes temporales se comportan igual. Las columnas `binary` no cambian ninguna salida.

Ejemplos de formas plegadas y de equivalencia (IC2):

| Texto | Forma plegada `ci` | Forma plegada `ci_ai` |
|---|---|---|
| `José` | `josé` | `jose` |
| `JOSÉ` | `josé` | `jose` |
| `Jose` + U+0301 (acento combinante) | `josé` | `jose` |
| `Straße` | `strasse` | `strasse` |
| `ΣΑΣ` | `σασ` | `σασ` |
| `ᾳ` (alfa con iota suscrita) | `αι` | `αι` (la iota suscrita se pliega a una `ι`, que es una letra) |
| `µ` (micro) + U+0301 | `μ́` | `μ` (pliega a la `μ` griega, cuyas marcas se quitan) |
| `ﬁn` | `fin` | `fin` |
| `İstanbul` | `i̇stanbul` (una `i` con el punto combinante U+0307) | `istanbul` |
| `Istanbul` | `istanbul` | `istanbul` |
| `ılık` (con `ı` sin punto) | `ılık` | `ılık` |
| `Ærø` | `ærø` | `ærø` (la `æ` y la `ø` no se descomponen, así que no son `ae` ni `o`) |
| `Año` | `año` | `año` (la `ñ` es una letra propia) |
| `Ångström` | `ångström` | `angstrom` |
| `Ёлка` | `ёлка` | `елка` |
| `Йод` | `йод` | `иод` (la `й` pierde su marca, como la `ä` o la `å`) |
| `ña` + U+0323 (punto debajo) | `ñạ` | `ña` |
| `n` + U+0301 + U+0303 | `ń̃` | `n` (la tilde no sigue directamente a la `n`) |
| U+0301 solo | U+0301 | U+0301 (no sigue a ninguna letra) |
| `が` | `が` | `が` (el japonés conserva sus marcas) |
| `कु` | `कु` | `कु` (el devanagari conserva sus marcas) |

## Reutilización

- La codificación de las claves de 7.1 y 7.2, sus direcciones y el orden de sus nulos, aplicada a la forma plegada.
- La comprobación de unicidad (IX7), la creación de índices sobre filas que ya existen (IX2, IX11) y los datos de clave
  duplicada (E17, IX10).
- La compatibilidad de esquemas (E6), Describir (E2) y la lista de reglas de esquema no válido (E5).
- Las estadísticas y las estimaciones de 7.3 sobre los valores codificados como en la clave.
- La lectura sin copia de las columnas `string` (D11).
- El generador de valores al azar y las pruebas de propiedades con modelo de referencia de AC-N1 y AC-N7.

## Fuera de alcance

- El orden alfabético de un idioma (por ejemplo, la `ñ` entre la `n` y la `o`) y las intercalaciones por idioma, como
  las reglas del turco: en `ci`, `İ` no es equivalente a `i` ni `ı` a `i`; en `ci_ai`, `İ` sí lo es a `i`, porque pierde
  su punto como un acento, y `ı` sigue sin serlo (IC2).
- Quitar los acentos de las escrituras que no son latinas, griegas ni cirílicas, e igualar la `ñ` a la `n` (IC2).
- Distinguir o igualar por anchura (`Ａ` y `A`), igualar hiragana con katakana, y las formas de compatibilidad (NFKC);
  la `æ`, la `œ`, la `ø` o la `ł` no son equivalentes a otras letras (IC2).
- Normalizar el texto de una columna `binary`: sigue comparándose byte a byte.
- Cambiar la intercalación de una columna existente: se añade una columna nueva con la intercalación deseada y se copian
  los valores (IC1).
- Una intercalación por índice o por consulta, distinta de la de su columna, y la cláusula de intercalación de una
  consulta (módulo 17).
- Comparar los valores por defecto por su forma plegada (IC9).
- Los índices sobre expresiones (documento de diseño 14).
- Una operación de búsqueda por prefijo de texto, `LIKE` o «empieza por»: módulo 17, sobre el orden de IC3.
- La intercalación de las cadenas dentro de un documento JSON (documento de diseño 14) y la de `bytes`.
- Los nombres de tablas, columnas e índices, que siguen comparándose por sus bytes (B4).
- Unicode posterior a 17.0 (IC2).
- La interfaz C y el binding C# (módulo 18).

## Dejado a la implementación

- Cómo se generan y se guardan los datos de Unicode que usa el plegado, siempre que den exactamente la forma plegada de
  IC2 y cumplan IC11.
- Cómo guarda una entrada de índice el texto original de IC5, siempre que no cuente para el máximo de la clave (IC7).
- El texto en inglés de los errores (D8); los nombres de las operaciones, los tipos, las intercalaciones y las reglas,
  en inglés y con las convenciones de Rust.

## Criterios de aceptación

1. **AC-IC1** — Dada una tabla creada con columnas `string` sin intercalación declarada, `binary`, `ci` y `ci_ai`, y una
   columna `ci` añadida después, cuando se describe, entonces cada columna `string` muestra su intercalación (`binary`
   las dos primeras) y las de otros tipos ninguna; cuando se renombra una columna `ci` y se modifica otra para permitir
   nulos dando `ci`, entonces las dos conservan su intercalación; cuando se modifica una columna `ci` dando `binary` o
   sin declararla, entonces falla sin efecto con esquema no válido y la regla de un cambio que N3 no admite; cuando se
   crea una tabla, se añade una columna o se abre con un modo que recibe un esquema, con una intercalación (también
   `binary`) en una columna `bytes`, `json`, entera y vector, entonces falla sin efecto con esquema no válido, la regla
   intercalación en una columna que no es de texto y la columna; con una intercalación en una columna `bytes` y la
   métrica coseno sobre `bit` en otra, gana la métrica coseno, y con una intercalación en una columna `bytes` y un valor
   por defecto no válido en otra, gana la intercalación; y cuando se abre la tabla con un esquema que da otra
   intercalación a una columna `string`, o no la declara en la `ci`, entonces falla con esquema distinto, la diferencia
   intercalación y lo esperado y lo encontrado, y con otro tipo y otra intercalación a la vez, con la diferencia tipo;
   cuando se abre con un esquema que da a una columna `ci` el valor por defecto `ANA` donde la tabla tiene `Ana`,
   entonces falla con esquema distinto y la diferencia valor por defecto; y cuando se abre con un esquema que declara una
   intercalación en una columna `bytes` y otra intercalación en una `string`, entonces falla con esquema no válido, no
   con esquema distinto.
2. **AC-IC2** — Dados todos los valores escalares de Unicode (sin U+D800–U+DFFF), cuando se pliega cada uno en `ci` y en
   `ci_ai`, entonces la forma plegada es la que da la definición de IC2 calculada en la prueba con los ficheros de datos
   de Unicode 17.0 (`CaseFolding.txt`, `UnicodeData.txt`, con su categoría general y su descomposición, y `Scripts.txt`), plegar la forma plegada da la misma, y un
   valor no asignado queda como está; cuando se pliegan las cadenas de `NormalizationTest.txt` de Unicode 17.0, entonces
   las tres columnas canónicamente equivalentes de cada línea (c1, c2 y c3: el texto, su NFC y su NFD) dan la misma
   forma plegada; cuando una prueba de propiedades pliega secuencias al azar de letras latinas, griegas, cirílicas y de
   otras escrituras con marcas, incluidas `ŉ` + U+0303, `ẚ` + U+0301, `µ` + U+0301, `a` + U+200D + U+0301 y `ñ` +
   U+0323, entonces la forma plegada es la de la definición, plegarla otra vez da la misma, y dos secuencias
   equivalentes en `ci` lo son en `ci_ai`; y para los ejemplos de *Salida*, la forma plegada es la de la tabla.
3. **AC-IC3** — Dada una tabla con una clave primaria `ci` y otra con una `ci_ai`, cuando se inserta `Ana`, entonces
   insertar `ANA` (y en `ci_ai` también `Àna`) falla con clave duplicada con el texto `ANA` (o `Àna`) y la tabla queda
   como estaba; leer y borrar con `ana` encuentran la fila `Ana`; actualizar con `ANA` cambia las demás columnas y la
   clave sigue siendo `Ana` al leerla; insertar o actualizar con `ANA` informa de que actualizó; una actualización que
   solo da `ANA` es idéntica; borrar `Ana` e insertar `ANA` en la misma escritura deja la fila `ANA`; una carga masiva
   con `Bea` y `BEA` falla como con dos claves repetidas (E13); borrar el intervalo de `ANA` incluido a `ana` incluido
   borra la fila `Ana` y devuelve 1; y actualizar una columna `ci` que no es de la clave de `Eva` a `EVA` no es una
   actualización idéntica, y leer la fila, por su nombre y en una proyección, da `EVA` sin copia.
4. **AC-IC4** — Dada una tabla con un índice único sobre una columna `ci` y otro sobre una `ci_ai`, cuando se guarda una
   fila con un valor equivalente al de otra fila, también al actualizarla, entonces falla con clave duplicada, con el
   nombre del índice y los textos de la fila que choca tal como quedarían guardados (IC6), y la fila, los índices y el
   número de filas quedan como estaban; al actualizar la fila de clave `Ana` pidiéndola con `ANA` y dando a la columna
   del índice un valor equivalente al de otra fila, la clave primaria del error es `Ana`, la que la fila conserva (IC4); dos filas con nulos no chocan; actualizar una fila de `Ana` a `ANA` en la columna del índice no choca con
   ella misma; una carga masiva con dos filas cuyos valores de un índice único son `Bea` y `BEA` falla con clave
   duplicada, con el nombre del índice y los textos de la fila de la carga que choca; y cuando se crea cada índice único
   sobre filas que ya tienen `Ana` y `ANA`, entonces falla sin efecto con clave duplicada (IX11), con los textos tal como
   están guardados.
5. **AC-IC5** — Dada una tabla con columnas `ci` y `ci_ai` en la clave primaria y en índices ascendentes y descendentes
   con nulos primero y último, cuando una prueba de propiedades guarda filas al azar con textos que mezclan mayúsculas,
   acentos precompuestos y combinantes, `ß`, `ŉ`, letras griegas con sigma final, textos formados solo por marcas y
   texto vacío, y recorre la tabla y cada índice hacia delante y hacia atrás, y por intervalos con cada combinación de
   extremos incluidos, excluidos y sin extremo, dados con mayúsculas y acentos al azar, y borra intervalos de la clave
   primaria, entonces el orden, las filas de cada intervalo, las filas borradas y las claves duplicadas son los de un
   modelo de referencia que compara las formas plegadas de AC-IC2 y no usa la codificación de las claves; y cada fila se
   lee con el texto que se guardó; también en un almacén temporal.
6. **AC-IC6** — Dado un índice con columnas `ci` y `ci_ai` en su clave y una columna `ci` incluida, sobre una tabla con
   una clave primaria `ci`, cuando se leen sus entradas, entonces devuelven en las columnas de la clave del índice, en
   las de la clave primaria y en la incluida el texto tal como se guardó en la fila, también después de actualizar una
   columna de la clave del índice de `Ana` a `ANA`, y en el orden de las formas plegadas.
7. **AC-IC7** — Dada una columna `ci` y un texto de `ŉ` repetido cuya forma plegada pasa del máximo de la clave pero el
   texto no, cuando se inserta como clave primaria, se guarda en una columna de un índice, se da como límite de un
   intervalo o como valor del prefijo, entonces falla con tamaño no válido y los datos de E18 e IX10, aunque el
   intervalo esté vacío; en una columna `binary` el mismo texto se guarda; y en una columna `ci_ai`, un texto de letras
   con acentos combinantes que pasa del máximo de la clave pero cuya forma plegada no, se guarda como clave primaria y
   se lee entero.
8. **AC-IC8** — Dada una tabla de 30.000 filas o menos, para que las estadísticas lean todas sus filas (ST1), con una
   columna `ci` y otra `ci_ai` con valores que solo difieren en mayúsculas o acentos y un índice sobre la `ci` y una
   columna entera, cuando se analizan, entonces el número de valores distintos cuenta una vez cada forma plegada, los
   valores frecuentes y los límites del histograma son formas plegadas, un límite acortado (ST16) lo está en un límite de
   valor escalar de la forma plegada, la fracción de valores grandes cuenta un texto `ci` de menos de 2.048 bytes cuya forma
   plegada pasa de ellos, las estadísticas del prefijo del índice (ST17)
   cuentan las combinaciones por su forma plegada, y estimar la igualdad, un intervalo o la igualdad en varias columnas
   (ST18) con cualquier combinación de mayúsculas y acentos da lo mismo que con la forma plegada.
9. **AC-IC9** — Dado un almacén del formato anterior, también uno solo con columnas `binary`, cuando este motor lo abre,
   entonces falla con formato anterior (F11); y un almacén creado por este motor tiene la versión de IC10.
10. **AC-IC10** — Dadas las pruebas de 7.1, 7.2, 7.3 y 7.5, con columnas `string` sin intercalación declarada, cuando se
    ejecutan, entonces dan lo mismo que antes del cambio, salvo la versión del formato, que es la de IC10.
11. **AC-IC11** — Dado el cierre de este PRD, cuando se revisa su informe, entonces tiene, en Windows y en Linux, las
    medidas de las cargas de IC12 antes y después del cambio y las de la tabla de IC12 con columnas `ci` frente a
    `binary`.
12. **AC-IC12** — Dadas las dependencias de la biblioteca antes y después del cambio, sin las de las pruebas, cuando se
    comparan, entonces son las mismas (IC11).

## Anotaciones a otros documentos

Este PRD cambia o precisa reglas de documentos anteriores, que llevan la anotación del 2026-11-23 (en sus traducciones
también):

- Padre, N2: el módulo 7 se cierra cuando lo están 7.1, 7.2, 7.3, 7.5, 7.6 y 7.7 (IC13). N4: una columna `string`
  declara su intercalación (IC1). N5: «`string` y `bytes` byte a byte, sin intercalación» pasa a valer para `bytes` y
  para las columnas `string` `binary`; las `ci` y `ci_ai` se ordenan y se comparan por su forma plegada (IC2, IC3), y
  la clave primaria y los índices únicos comparan las formas plegadas. *Reparto*: se añade el PRD hijo 7.7,
  *Intercalación de las columnas de texto*. *Fuera de alcance*: «la comparación de texto sin distinguir mayúsculas»
  deja de estar fuera.
- 7.1, E2: Describir muestra la intercalación de cada columna `string` (IC1). E4: una columna `string` declara su
  intercalación como parte de su tipo al crearla o añadirla, y modificarla con otra es un cambio no admitido (IC1). E5:
  la regla nueva de IC9. E6: la intercalación es parte de la compatibilidad, con la diferencia intercalación (IC9).
  E10 a E13 y E17: las claves equivalentes son la misma fila, los intervalos se pliegan, y clave duplicada lleva el texto
  dado (IC3, IC4, IC6). E18: la clave primaria se mide con la forma plegada (IC7). E20: el formato sube en uno (IC10).
- 7.2, IX7 e IX11: la unicidad compara valores equivalentes (IC6). IX8: las entradas devuelven el texto guardado, no la forma plegada (IC5). IX9: los
  intervalos se pliegan (IC3). IX10: la entrada, el prefijo y los límites se miden con la forma plegada (IC7), y clave
  duplicada lleva el texto dado o el guardado (IC6). *Fuera de alcance*: «Intercalaciones y comparación de texto sin
  distinguir mayúsculas» deja de estar fuera, salvo lo que excluye este PRD.
- 7.3, ST1, ST2, ST16, ST17 y ST18: las estimaciones y las estadísticas de una columna `ci` o `ci_ai` usan formas
  plegadas; un texto cuya forma plegada pasa de 2.048 bytes también es grande, y un límite se acorta en un límite de
  valor escalar de la forma plegada (IC7, IC8).
- Proyecciones, PY2: una columna `string` se prepara con el tipo `string`, sea cual sea su intercalación (IC1).
- 7.5, AC-JS12: un almacén nuevo es del formato de IC10, no del 14.
- [Documento de diseño 3](../../../design/tables/03-keys-and-indexes.md), *Strings and collation*, riesgo *No
  collation* y pregunta de producto 5, y el [README del diseño](../../../design/tables/README.md): la decisión del
  2026-11-09 de orden binario sin intercalación pasa a valer solo para las columnas `binary`, y la comparación sin
  distinguir mayúsculas deja de ser un índice sobre expresiones futuro (IC1, IC2).

## Referencias

- Padre: [tables.prd.md](tables.prd.md) (N2–N5, N10, N11, reparto, *Fuera de alcance*)
- Árboles: [trees.prd.md](../trees/trees.prd.md) (B4, B5, B12); espacio libre:
  [free-space.prd.md](../free-space/free-space.prd.md) (F11)
- Hermanos: [tables-schema.prd.md](tables-schema.prd.md) (E1, E2, E4–E7, E10–E13, E17, E18, E20),
  [tables-indexes.prd.md](tables-indexes.prd.md) (IX1, IX2, IX7–IX11), [tables-statistics.prd.md](tables-statistics.prd.md)
  (ST1, ST2, ST16–ST18), [tables-json.prd.md](tables-json.prd.md) (JS12), [tables-projection.prd.md](tables-projection.prd.md)
  (PY2, PY3)
- Raíz: [krummi.prd.md](../krummi.prd.md) (D8, D11, D12)
- Diseño: [documento 3, claves e índices](../../../design/tables/03-keys-and-indexes.md) (*Strings and collation*) y
  [documento 14, índices sobre JSON](../../../design/tables/14-json-indexes.md)
- Unicode 17.0: `CaseFolding.txt`, `UnicodeData.txt`, `Scripts.txt`, `NormalizationTest.txt` (Unicode Character
  Database)

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 69c105b1 | 2026-11-21 06:51 UTC+00:00 | 48 h 43 min | 47 h 40 min | 1 h 3 min | 42 min | 21 min | 7 |

<!-- prd-time: [{"session": "69c105b1-9a71-44c2-9e9d-c3c6315b92d5", "start": "2026-11-21T06:51:30.637000+00:00", "wall": 175372, "discarded": 171570, "agent": 2531, "human": 1269, "interventions": 7, "idle_minutes": 10}] -->
