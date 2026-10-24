# Árboles B+ de Krummi — PRD

**Versión**: 1.0
**Fecha**: 2026-10-24
**Documento padre**: [Krummi, motor de almacenamiento embebido inspirado en Voron — PRD](krummi.prd.md) (v1.1)

## Objetivo y contexto

Construir los **árboles B+**, el módulo 5 del [reparto del padre](krummi.prd.md#reparto-en-prd-hijos): árboles con
nombre que guardan entradas ordenadas por clave, con claves y valores de tamaño variable, valores grandes, recorrido
en orden, búsqueda por prefijo, árboles multivalor, árboles de tamaño fijo y compresión de páginas hoja.

Es la primera estructura de datos del producto y la primera forma que tienen las aplicaciones de guardar datos en un
almacén: hasta hoy, leer y escribir páginas es interno de la biblioteca (T1, F1). Las tablas (módulo 7) y las
estructuras posteriores se apoyan en estos árboles.

El módulo es demasiado grande para una sola ejecución, así que este documento es el paraguas del módulo, como el
padre lo es del producto: fija las reglas comunes a todos los árboles (tipos, nombres, claves y su orden, tamaños
máximos, errores y árboles abiertos a la vez) y reparte el módulo en seis PRD hijos, cada uno con su propia ejecución
y su propio cierre (ver [Reparto en PRD hijos](#reparto-en-prd-hijos)). Este documento no tiene ejecución propia
(B2).

**Por qué ahora**: el espacio libre (módulo 4) ya permite reservar y liberar páginas y rangos de páginas contiguas,
que es lo que los árboles necesitan para crecer, encogerse y guardar valores grandes.

Este PRD hereda todo lo del padre, lo del [Pager](pager.prd.md) (P1–P17), lo de las
[Transacciones](transactions.prd.md) (T1–T18), lo del [Diario](journal.prd.md) (J1–J17), lo de la
[escritura agrupada y directa del diario](journal-group-commit.prd.md) (E1–E16) y lo del
[Espacio libre](free-space.prd.md) (F1–F14); aquí solo se añade lo común del módulo. Lo que cambia o precisa reglas
anteriores está declarado en sus decisiones y anotado en los documentos afectados (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

En este documento, un **árbol** es una colección con nombre de entradas ordenadas por clave dentro de un almacén; una
**entrada** es una clave con su valor, o con sus valores en un árbol multivalor; el **catálogo** es el registro de los
árboles de un almacén con su nombre y su tipo; y el **tipo** de un árbol es normal, multivalor o de tamaño fijo.

### Decisiones de producto

| ID | Decisión |
|---|---|
| B1 | **Superficie pública**: desde este módulo, las aplicaciones guardan y leen datos en árboles. Dentro de una transacción se abre un árbol por su nombre; se lee en cualquier transacción y se modifica solo en una de escritura. Modificar un árbol abierto en una transacción de lectura no compila, y usar un árbol después de terminar la transacción en la que se abrió tampoco (como T8). Un árbol abierto en una transacción de lectura se puede pasar a otros hilos y compartir con ellos como la transacción, y uno abierto en una de escritura se queda en su hilo, como ella (T6). Cada PRD hijo fija las operaciones que añade. |
| B2 | **Reparto y cierre**: el módulo se reparte en los seis PRD hijos de la [tabla](#reparto-en-prd-hijos), que se construyen en ese orden. Este documento no tiene ejecución propia: cada regla común la implementa y la prueba el primer PRD hijo que la necesita, y los siguientes la siguen cumpliendo. El módulo 5 está cerrado cuando lo están sus seis PRD hijos; hasta entonces no cuenta como cerrado para el objetivo 2 del padre. |
| B3 | **Tipos de árbol**: un árbol es **normal** (cada clave tiene un valor), **multivalor** (cada clave tiene un conjunto ordenado de valores distintos) o **de tamaño fijo** (claves enteras, B5, y valores de la longitud fija que fija el PRD hijo 5.5). El tipo se fija al crear el árbol y no cambia mientras el árbol existe; borrar un árbol y crear otro con el mismo nombre y otro tipo está permitido. La compresión de páginas hoja (5.6) no es un tipo. Todos los tipos comparten el catálogo y su espacio de nombres: dos árboles de un almacén nunca tienen el mismo nombre. Pedir un árbol existente como un tipo que no es el suyo, al abrirlo o al crearlo, falla con la clase nueva de **tipo de árbol distinto** (B7), que gana a cualquier error de árbol ya existente que fije 5.1 y a un daño en las páginas del propio árbol. Las operaciones que nombran un árbol sin pedir un tipo (como borrarlo) nunca fallan con tipo de árbol distinto. El PRD hijo 5.1 crea el catálogo con los árboles normales; 5.4 y 5.5 añaden sus tipos. |
| B4 | **Nombres de árbol**: un nombre es texto UTF-8 de 1 a 1.024 bytes, contados en su codificación UTF-8; la biblioteca recibe los nombres como texto, así que un nombre que no es UTF-8 válido no se puede pasar. Dos nombres son el mismo solo si sus bytes son iguales: no se normalizan mayúsculas, minúsculas ni formas Unicode, así que `pedidos` y `Pedidos` son dos árboles distintos. Ningún nombre está reservado. Un nombre fuera de esos tamaños falla con la clase nueva de **tamaño no válido** (B7). |
| B5 | **Claves y orden**: en los árboles normales y multivalor, una clave es una secuencia de 1 a 2.048 bytes; la clave vacía no es válida. Voron admite 1.024; Krummi admite 2.048 para que quepan las claves de índices secundarios de varios campos (módulo 7). Toda clave que recibe una operación de esos árboles se valida igual, al guardar, al leer y al borrar; el prefijo y la clave desde la que empieza un recorrido los fija el PRD hijo 5.2. Las claves se ordenan byte a byte, comparando cada byte como un número sin signo de 0 a 255, y una clave que es prefijo de otra va antes que ella. No hay comparadores propios de la aplicación. El mismo orden vale para los valores de una clave en un árbol multivalor y para los nombres del catálogo. En los árboles de tamaño fijo, una clave es un entero sin signo de 64 bits, de 0 a 2^64−1, en orden numérico. Una clave de un árbol normal o multivalor fuera de sus tamaños falla con tamaño no válido (B7). |
| B6 | **Valores**: un valor es una secuencia de 0 a 268.435.456 bytes (256 MiB); el valor vacío es válido. Hasta el PRD hijo 5.3 (valores grandes), el máximo es un número fijo que fija el PRD hijo 5.1, el mismo para cualquier clave válida, también una de 2.048 bytes; 5.3 lo sube a 256 MiB, también para cualquier clave válida. Los valores mayores son cosa de los streams (módulo 8). Los PRD hijos 5.4 y 5.5 pueden fijar máximos menores, o en 5.5 una longitud exacta, para sus tipos. Todo valor que recibe una operación se valida igual, también el que se pasa para buscarlo o borrarlo en un árbol multivalor. Un valor fuera de los tamaños vigentes de su árbol falla con tamaño no válido (B7). |
| B7 | **Errores nuevos** (D8 del padre): **tamaño no válido**, para una clave, un valor o un nombre fuera de sus tamaños (B4–B6), y **tipo de árbol distinto** (B3). Los dos se detectan antes de que la operación cambie nada: la operación falla sin efecto y la transacción sigue utilizable, sea de lectura o de escritura, también cuando el almacén no acepta escrituras (T12). Son errores de validación pública y no están entre los fallos de T11, así que no impiden confirmar: una escritura conserva sus cambios y puede confirmarse o abortarse. Solo los errores de validación pública de este módulo tienen esta regla: estas dos clases y el de una apertura de un árbol borrado (B9). Los fallos de T11, y los usos incorrectos internos que invalidan la transacción según las [validaciones del espacio libre](free-space.prd.md#validaciones-y-mensajes), siguen invalidándola. Si una escritura tuvo además un fallo que la invalida, antes o después, confirmarla falla con la clase del primero de esos fallos, nunca con una de estas dos. Una escritura cuyas únicas operaciones fallaron con estas clases es una escritura sin cambios (T10). **Precedencia**: el tamaño no válido se decide solo con los argumentos de la operación y las propiedades del árbol ya abierto, y gana a todo lo demás; si varios argumentos están fuera de su tamaño, se informa del primero en este orden: nombre, clave, valor. Después gana el primer fallo que la operación encuentra al leer el catálogo (daño, memoria agotada, entrada y salida) y después el tipo de árbol distinto. |
| B8 | **Datos de los errores nuevos** (D8 del padre): los dos llevan los datos de T18 (la carpeta del almacén, o que es temporal). El de tamaño no válido lleva además qué incumple su tamaño (nombre, clave o valor), el tamaño recibido en bytes y los tamaños mínimo y máximo vigentes para ese árbol: `1` y `1024` para un nombre, `1` y `2048` para una clave, y para un valor los tamaños vigentes de su árbol (B6): `0` y el máximo, o la longitud exacta como mínimo y como máximo en un árbol de tamaño fijo. El de tipo de árbol distinto lleva además el nombre del árbol, el tipo que tiene y el tipo pedido. |
| B9 | **Varios árboles abiertos**: una transacción puede tener abiertos a la vez los árboles que quiera, también el mismo dos veces, y todos ven el mismo estado: en una escritura, con todos sus cambios hechos hasta ese momento en cualquiera de ellos. Una referencia leída de cualquier árbol deja de valer en la siguiente operación que puede modificar la transacción, en ese árbol o en otro, aunque falle sin efecto (T8): el compilador lo comprueba sin saber si la operación fallará. Abrir un árbol existente no termina las referencias; crear o borrar un árbol, sí. Cada apertura queda ligada a la identidad del árbol que abrió, que es distinta de su nombre: borrar un árbol y crear otro con el mismo nombre, las veces que sea y con el mismo tipo o con otro, crea siempre un árbol nuevo; el PRD hijo 5.1 fija cómo se garantiza. Si una escritura borra un árbol, las operaciones a través de cualquier otra apertura suya fallan sin efecto con un error de validación pública que fija 5.1, con la regla de B7, también cuando después se crea otro árbol con el mismo nombre: una apertura vieja nunca toca el árbol nuevo. |
| B10 | **Lecturas**: valen D11 del padre, T16 y AC20 para las claves y los valores que devuelven los árboles. El PRD hijo 5.3 fija cómo se devuelve un valor grande y el 5.6 dónde vive la copia descomprimida de una página hoja, dentro de D11. |
| B11 | **Límites**: Krummi no impone límites artificiales al número de árboles ni de entradas. Eso no elimina los límites físicos, el tamaño máximo del almacén (P10), la memoria (P13) y el disco (G7), ni los de representación de los números con que el formato cuenta páginas y entradas, que el formato elige para que no se alcancen antes que los físicos; si una operación llegara a uno, fallaría sin efecto con la clase de tamaño máximo y seguiría T11. Una operación con tamaños válidos que no cabe en los límites físicos falla con su clase y sigue T11. La información del almacén (P12) no añade datos en este módulo. |
| B12 | **Formato y medición**: la versión de formato sube exactamente 1 con cada cambio incompatible del formato (D7 del padre), no por añadir datos persistentes. Un cambio es incompatible cuando un motor de la versión anterior podría leer mal, modificar sin respetar o dañar un almacén con el formato nuevo, o cuando el motor nuevo no puede abrir sin riesgo uno de la versión anterior; ante la duda, se trata como incompatible. Cada PRD hijo que cambia lo que se guarda en disco declara si su cambio es incompatible y por qué; si lo es, un almacén de la versión anterior, aunque esté vacío, es de formato anterior, como en F11. Cada PRD hijo decide si su cambio incluye una medición reproducible, como F13, sin umbral (D5 del padre). |

### Estado de partida

Verificado el 2026-10-24 en el código y en la referencia de Voron que recoge el padre:

| Aspecto | Situación |
|---|---|
| API pública | La biblioteca abre, consulta y cierra almacenes, y abre, confirma y aborta transacciones. No hay ninguna forma pública de guardar ni leer datos: leer y modificar páginas, reservarlas y liberarlas es interno (T1, F1). |
| Páginas | Páginas de 8.192 bytes, con 8.176 útiles después de su comprobación de integridad y su número. |
| Espacio libre | Una escritura reserva una página o un rango de páginas contiguas y libera páginas sueltas o en rangos. Un rango no cruza de una región a otra: el tope es de 65.407 páginas, unos 511 MiB (F5). |
| Errores | Doce clases: acceso, almacén en uso, no es un almacén, daño, formato distinto, disco lleno, entrada y salida, opciones no válidas, tamaño máximo, memoria agotada, tiempo de espera agotado y uso incorrecto. Ninguna trata claves, valores ni árboles. |
| Formato | En esta fecha, la versión de formato vigente es `5` (F11). No hay ningún catálogo de árboles ni estructura de datos en disco. |
| Voron (referencia) | Árboles B+ con claves de hasta 1.024 bytes, valores grandes en páginas aparte, árboles multivalor, compresión de páginas hoja y árboles de tamaño fijo con claves enteras de 64 bits con signo, todos con nombre dentro del almacén (estado de partida del padre). |

## Actores

- **Desarrollador integrador**: crea y abre árboles por su nombre, guarda, lee y borra entradas dentro de
  transacciones, y trata los errores de tamaño no válido y de tipo de árbol distinto por su clase.
- **Mantenedor**: ejecuta en Windows y en Linux las pruebas y las mediciones que fije cada PRD hijo.

La herramienta de línea de comandos no abre almacenes hasta el módulo 12, que fija cómo inspecciona los árboles.

## Flujo

1. La aplicación abre una transacción y, dentro de ella, uno o varios árboles por su nombre y su tipo (B1, B3, B9).
2. En una transacción de lectura lee entradas; en una de escritura además crea y borra árboles, y guarda y borra
   entradas, con las operaciones que fija cada PRD hijo.
3. Cada nombre, clave y valor se valida contra sus tamaños antes de cambiar nada (B4–B7); un error de tamaño o de
   tipo deja la transacción utilizable.
4. La confirmación aplica todos los cambios de los árboles y del catálogo, o ninguno (G1).

## Reparto en PRD hijos

Cada submódulo es un PRD hijo de este, con su propia ejecución. El orden es el de construcción: cada uno se apoya en
los anteriores. La columna *Fija* nombra lo que ese PRD hijo debe decidir, además de su contenido y de sus propios
errores.

| # | PRD hijo | Contenido | Fija |
|---|---|---|---|
| 5.1 | **Árboles y operaciones por clave** | Catálogo de árboles con nombre; árboles normales; guardar, leer y borrar una entrada por su clave; valores hasta el máximo provisional (B6). | Las operaciones del catálogo (crear, abrir, borrar y, si los hay, renombrar y listar árboles); qué pasa al abrir en una lectura un árbol que no existe y al crear uno que ya existe con el mismo tipo; qué hace guardar una clave que ya existe; qué devuelve leer una clave que no está y qué hace borrarla; el error y sus datos para una apertura de un árbol borrado, y cómo se distingue un árbol recreado del anterior (B9); si un daño en las páginas de un árbol se informa al abrirlo o en su primera lectura; si un árbol informa su número de entradas; el máximo provisional de un valor (B6); si su cambio de formato es incompatible y, si lo es, su versión (B12); si un almacén nuevo ocupa más páginas que antes, actualizando los criterios anteriores que lo fijan. |
| 5.2 | **Recorrido en orden y búsqueda por prefijo** | Recorrer las entradas de un árbol en orden, hacia delante y hacia atrás, desde una clave dada; limitar el recorrido a las claves con un prefijo. | Los tamaños válidos del prefijo y de la clave de inicio, y qué significa un prefijo vacío; dónde empieza un recorrido cuando la clave de inicio no existe; qué ve un recorrido cuando la escritura que lo hace modifica el árbol; si hay borrado de un intervalo de claves. |
| 5.3 | **Valores grandes** | Valores de hasta 256 MiB (B6). | Cómo se devuelve un valor grande sin copia (B10), también en modo temporal, donde la memoria crece por trozos separados. |
| 5.4 | **Árboles multivalor** | Árboles cuyas claves tienen un conjunto ordenado de valores distintos (B3). | Las operaciones sobre los valores de una clave; el máximo de cada valor y si el valor vacío es válido; qué pasa al añadir un valor que ya está. |
| 5.5 | **Árboles de tamaño fijo** | Árboles con claves enteras sin signo de 64 bits (B5) y valores de longitud fija (B3). | Cuándo se fija la longitud de los valores y cuál es la máxima; con qué clase y datos falla una longitud no válida al crear el árbol; si abrir un árbol con una longitud distinta de la suya es un tipo de árbol distinto o un error propio, y sus datos; qué operaciones de 5.1 y 5.2 tienen. |
| 5.6 | **Compresión de páginas hoja** | Árboles cuyas páginas hoja se guardan comprimidas. | Cómo se elige comprimir un árbol, de qué tipos, y si se puede cambiar después; dónde vive la copia descomprimida de una página hoja (B10). |

**Antes de implementar 5.1**: su ejecución empieza por una prueba de compilación que demuestra que el modelo de
propiedad y préstamos de Rust permite a la vez B1 y B9: varias aperturas de árboles en una escritura, también del
mismo árbol, con los cambios hechos por una visibles en las demás; referencias que dejan de valer en la siguiente
operación de modificación; y aperturas que no se usan fuera de su transacción. Todo sin código no seguro fuera del
Pager (D12 del padre). Si alguna de esas garantías no se puede dar en tiempo de compilación, la ejecución se para
antes de implementar los árboles y la alternativa vuelve a este PRD como decisión de producto.

## Validaciones y mensajes

Valen las de los ascendientes, incluidos los fallos de T11 y T12 en cualquier operación de un árbol. Este módulo
añade estas situaciones, solo de la biblioteca (la herramienta no abre almacenes hasta el módulo 12). Ninguna sigue
T11: la operación falla sin efecto y la transacción sigue utilizable (B7).

| Situación | Regla | Clase de error |
|---|---|---|
| Un nombre de árbol vacío o de más de 1.024 bytes en UTF-8 | La operación falla sin efecto (B4). | Tamaño no válido (nueva) |
| Una clave de un árbol normal o multivalor vacía o de más de 2.048 bytes, al guardar, leer o borrar | La operación falla sin efecto (B5). | Tamaño no válido (nueva) |
| Un valor fuera de los tamaños vigentes de su árbol (B6) | La operación falla sin efecto. | Tamaño no válido (nueva) |
| Se abre o se crea un árbol existente con un tipo que no es el suyo | La operación falla sin efecto (B3). | Tipo de árbol distinto (nueva) |
| Se opera a través de una apertura de un árbol que la misma escritura borró | La operación falla sin efecto (B9). | La que fije 5.1 |

La precedencia entre estas situaciones y los demás fallos es la de B7. Cada PRD hijo añade las situaciones de sus
operaciones.

## Salida

El módulo no escribe nada en pantalla. Los datos que devuelve una lectura son los bytes guardados, sin cambios. La
información del almacén (P12) no añade datos (B11); la versión de formato que informa es la que fije el último PRD
hijo que la cambie (B12). Ejemplo de los datos de un error de tamaño no válido, por una clave de 2.049 bytes en un
almacén en disco:

| Dato | Valor |
|---|---|
| Carpeta | la del almacén |
| Qué incumple su tamaño | clave |
| Tamaño recibido | `2049` |
| Mínimo | `1` |
| Máximo | `2048` |

Con una clave vacía, el tamaño recibido es `0` y el mínimo y el máximo son los mismos.

## Reutilización

- Las transacciones, su aislamiento por instantánea y la copia de las páginas que se modifican (T2, T14, T16): los
  árboles se leen y se modifican a través de ellas.
- El diario, la recuperación y la atomicidad de la confirmación (G1, G5, J8): los cambios de los árboles y del
  catálogo viajan como cualquier otro cambio.
- El espacio libre (F4–F8): los árboles reservan y liberan páginas y rangos a través de él.
- La comprobación de integridad de las páginas y la clase de daño (G6).
- La clase de error de uso incorrecto (T5) para lo que los PRD hijos traten como llamada no admitida.
- La simulación de caídas, cortes de corriente y fallos de las pruebas de los módulos 3 y 4.

## Fuera de alcance

- Comparadores de claves propios de la aplicación, ordenaciones dependientes del idioma y nombres de árbol que no
  distinguen mayúsculas (B4, B5).
- Claves enteras con signo en los árboles de tamaño fijo (B5).
- Valores de más de 256 MiB, y leer o escribir un valor por partes (streams, módulo 8).
- Cambiar el tipo de un árbol existente (B3).
- Operaciones de lectura y escritura combinadas sobre una entrada, como incrementar un contador o guardar solo si la
  entrada tiene una versión dada.
- Estadísticas de un árbol, como su profundidad o sus páginas (informe de almacenamiento, módulo 12), salvo su
  número de entradas si 5.1 decide informarlo.
- Tablas con esquema, índices secundarios y compresión de valores por diccionario (módulo 7).
- Árboles con claves comprimidas (compact trees, módulo 15), containers, posting lists y lookups (módulos 13, 14 y
  16).
- Varias escrituras a la vez sobre árboles distintos del mismo almacén (G4).
- Inspeccionar, verificar o rescatar árboles desde la herramienta de línea de comandos (módulo 12).
- Cifrar las páginas de los árboles: lo cubre el cifrado de todo lo que el motor escribe (módulo 9).
- Umbrales de rendimiento (módulo 6).

## Dejado a la implementación

- La organización de las páginas de los árboles y del catálogo, a partir de qué tamaño un valor se guarda fuera de
  su página hoja, la política de división y fusión de páginas y su grado de llenado, siempre que cumplan las
  garantías del padre.
- Los nombres de las operaciones públicas, de los tipos y de las clases de error nuevas, que siguen las
  convenciones de Rust y están en inglés, y el texto en inglés de los errores (D8 del padre).

## Criterios de aceptación

Valen las condiciones de prueba de los módulos anteriores, y cada PRD hijo repite AC10 del padre en disco y AC20
también en modo temporal con lo que añade, como dicen esos criterios. Las pruebas que dañan páginas de un árbol o
del catálogo usan el conocimiento de la propia implementación. Cada criterio de esta lista lo comprueba el PRD hijo indicado entre paréntesis, en
un almacén en disco y en modo temporal, salvo las partes que reabren el almacén o dañan sus archivos, que son solo en
disco; los PRD hijos siguientes lo siguen cumpliendo. Los criterios de tipo de árbol distinto no se pueden comprobar
hasta que existe un segundo tipo (5.4).

1. **AC-B1** — Dada una escritura, cuando guarda entradas con claves de 1, de 1.025 y de 2.048 bytes, cada una con
   un valor del máximo vigente, entonces se guardan y se leen idénticas; y cuando intenta guardar, leer o borrar con
   una clave vacía o de 2.049 bytes, entonces falla con la clase de tamaño no válido sin efecto, con los datos de B8:
   clave, tamaño recibido `0` o `2049`, mínimo `1` y máximo `2048` (5.1).
2. **AC-B2** — Dada una escritura, cuando crea árboles con nombres de 1 byte y de 1.024 bytes, incluido uno de 1.024
   bytes formado por caracteres de varios bytes, entonces se crean; y cuando intenta crear uno con un nombre vacío,
   de 1.025 bytes o de 600 caracteres de dos bytes (1.200 bytes), entonces falla con la clase de tamaño no válido sin
   crear nada, con el tamaño recibido en bytes (5.1).
3. **AC-B3** — Dada una escritura, cuando guarda un valor vacío y otro del máximo vigente, los dos con una clave de
   2.048 bytes, entonces se leen idénticos tras confirmar y, en disco, también tras reabrir; y cuando intenta guardar
   uno de un byte más que el máximo, entonces falla con la clase de tamaño no válido sin guardar nada, con mínimo `0`
   y el máximo vigente (5.1 con su máximo provisional; 5.3 con 256 MiB).
4. **AC-B4** — Dada una escritura con cambios ya hechos, cuando una operación falla por tamaño no válido y otra por
   tipo de árbol distinto, entonces la escritura se confirma sin error y aplica todos sus cambios anteriores y
   posteriores a esos fallos; dada una transacción de lectura, cuando una operación falla por tamaño no válido,
   entonces sigue leyendo; dada una escritura en disco cuyas únicas operaciones fallaron con estas clases, cuando se
   confirma, entonces los archivos del almacén quedan idénticos byte a byte (T10); y dado un almacén que no acepta
   escrituras (T12), cuando una operación falla por tamaño no válido, entonces falla con esa clase y no con la de
   entrada y salida, y confirmar esa escritura sin cambios termina sin error (5.1 el tamaño; 5.4 el tipo).
5. **AC-B5** — Dada una escritura que ya tuvo un fallo de tamaño máximo (P10), cuando una operación posterior falla
   por tamaño no válido y después se confirma, entonces la confirmación falla con la clase de tamaño máximo y no
   aplica nada; y dada una escritura que primero tuvo un fallo de tamaño no válido y después uno de tamaño máximo,
   cuando se confirma, entonces falla igual con la clase de tamaño máximo (B7) (5.1).
6. **AC-B6** — Dados los árboles `pedidos` y `Pedidos`, y dos árboles cuyos nombres son `é` precompuesta y `e`
   seguida del acento combinante, cuando se guardan entradas en cada uno, entonces son cuatro árboles distintos y
   cada uno tiene solo sus entradas (5.1); y dado un árbol normal que se borra, cuando se crea otro con su nombre
   como multivalor, entonces se crea (5.4).
7. **AC-B7** — Dadas claves guardadas en orden aleatorio, incluidas claves con bytes de `0x00` a `0xFF` y claves que
   son prefijo de otras, cuando se recorre el árbol en orden, entonces salen ordenadas byte a byte sin signo, y cada
   clave que es prefijo de otra va antes que ella (5.2); los valores de una clave multivalor salen en ese mismo orden
   (5.4); y las claves `0`, `1`, `2^63` y `2^64−1` de un árbol de tamaño fijo salen en orden numérico (5.5).
8. **AC-B8** — Dado un árbol de un tipo, cuando se abre o se crea con su nombre como otro tipo, entonces falla con
   la clase de tipo de árbol distinto, con el nombre, el tipo que tiene y el tipo pedido, y el árbol queda intacto;
   falla igual con una página propia del árbol dañada y el catálogo intacto; y borrarlo sin pedir tipo nunca falla
   con esa clase, aunque puede fallar con la de daño si lee la página dañada (5.4 con normal y multivalor; 5.5 con
   tamaño fijo).
9. **AC-B9** — Dado un almacén en disco cuyo catálogo cabe en una página y esa página está dañada, cuando se abre un
   árbol con un nombre de 1.025 bytes, entonces falla con la clase de tamaño no válido; y con un nombre válido, con la
   clase de daño (B7) (5.1).
10. **AC-B10** — Dada una escritura con dos árboles abiertos, y uno de ellos abierto dos veces, cuando guarda
    entradas en cada uno, entonces cada apertura ve los cambios hechos a través de las otras, y la confirmación las
    aplica todas (B9) (5.1).
11. **AC-B11** — Dado el código de una aplicación de prueba, cuando modifica un árbol abierto en una transacción de
    lectura, usa un árbol después de terminar su transacción o usa una referencia leída después de una operación de
    modificación que falló sin efecto, entonces no compila (B1, B9) (5.1).
12. **AC-B12** — Dado un PRD hijo que declara su cambio de formato incompatible, cuando se abre un almacén de la
    versión anterior, aunque esté vacío, entonces falla con la clase de formato distinto como formato anterior; y
    dado uno que lo declara compatible, cuando se abre un almacén creado antes de ese cambio, entonces se abre y se
    usa sin error (B12) (cada PRD hijo que cambie lo que se guarda en disco, desde 5.1).
13. **AC-B13** — Dada una operación con un nombre y una clave fuera de sus tamaños, cuando se ejecuta, entonces el
    error de tamaño no válido es del nombre; y con una clave y un valor fuera de sus tamaños, de la clave (B7) (5.1).
14. **AC-B14** — Dada una escritura con un árbol abierto dos veces, cuando lo borra a través de una apertura y
    después usa la otra, también tras crear otro árbol con el mismo nombre, entonces la operación falla sin efecto
    con el error que fije 5.1, el árbol nuevo no cambia y la escritura se confirma sin error; repitiendo tres veces
    borrar y crear el árbol, con el mismo tipo y con otro, cada apertura anterior falla igual y solo la última ve el
    árbol vigente; y una transacción de lectura abierta antes de esa escritura sigue viendo el árbol original a
    través de su apertura, también después de confirmarla (B9, G3) (5.1).
15. **AC-B15** — Dado un árbol multivalor, cuando se busca o se borra un valor por encima del máximo de sus valores,
    entonces falla con la clase de tamaño no válido sin efecto (B6) (5.4).
16. **AC-B16** — Dado un árbol de tamaño fijo, cuando se guarda un valor de una longitud distinta de la suya,
    entonces falla con la clase de tamaño no válido, con esa longitud como mínimo y como máximo (B8) (5.5).
17. **AC-B17** — Dada una transacción de lectura abierta y una escritura que, con inserciones y borrados, divide y
    fusiona páginas de un árbol hasta cambiar su altura, cuando la lectura lee todas las entradas del árbol antes y
    después de que la escritura se confirme, entonces ve exactamente las de su instantánea; y una lectura nueva ve
    las de la escritura (G3, T14) (5.1, y cada PRD hijo cuyas operaciones cambian la estructura de un árbol).
18. **AC-B18** — Dada una escritura que divide y fusiona páginas de un árbol, cuando se provoca una caída del proceso
    y simulada de corriente en cada punto en que su confirmación escribe o lleva datos al disco, entonces al reabrir
    el árbol tiene todas sus entradas de antes o todas las de después, y se puede leer y modificar sin error (G1, G5,
    AC4 del padre); y cuando una operación falla a mitad de una división por disco lleno o tamaño máximo, entonces la
    escritura ya no puede confirmarse y, tras abortarla, el árbol queda como estaba (T11) (5.1, y cada PRD hijo
    cuyas operaciones cambian la estructura de un árbol).

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Padre raíz, reparto en PRD hijos, módulo 5: se reparte en seis PRD hijos y está cerrado cuando lo están los seis
  (B2); los tamaños máximos y sus errores que le encarga el padre están en B4–B8.
- Transacciones, T10: una escritura cuyas únicas operaciones fallaron por tamaño no válido, tipo de árbol distinto
  o una apertura de un árbol borrado es una escritura sin cambios (B7, B9).
- Transacciones, T11: los fallos de tamaño no válido y de tipo de árbol distinto no están entre sus fallos y no
  impiden confirmar (B7).
- Transacciones, T8: una operación que puede modificar la transacción termina las referencias aunque falle sin
  efecto; abrir un árbol existente no las termina (B9).

## Referencias

- Padre: [krummi.prd.md](krummi.prd.md) v1.1 (raíz)
- Relacionados: [pager.prd.md](pager.prd.md), [transactions.prd.md](transactions.prd.md),
  [journal.prd.md](journal.prd.md), [journal-group-commit.prd.md](journal-group-commit.prd.md),
  [free-space.prd.md](free-space.prd.md)
- Voron, árboles: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/BTrees>
- Voron, árboles de tamaño fijo: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/Fixed>
- LMDB: <https://github.com/LMDB/lmdb/blob/mdb.master/libraries/liblmdb/mdb.c>

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 31c36231 | 2026-10-24 10:22 UTC+01:00 | 36 min | 0 min | 36 min | 19 min | 17 min | 15 |

<!-- prd-time: [{"session": "31c36231-b691-4c62-8559-6f87e09084cf", "start": "2026-10-24T09:22:37.952000+00:00", "wall": 2181, "discarded": 0, "agent": 1146, "human": 1035, "interventions": 15, "idle_minutes": 10}] -->
