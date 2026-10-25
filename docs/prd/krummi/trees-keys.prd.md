# Árboles y operaciones por clave de Krummi — PRD

**Versión**: 1.0
**Fecha**: 2026-10-25
**Documento padre**: [Árboles B+ de Krummi — PRD](trees.prd.md) (v1.0)

## Objetivo y contexto

Construir el PRD hijo **5.1** del [reparto del módulo de árboles](trees.prd.md#reparto-en-prd-hijos): el catálogo de
árboles con nombre y los árboles normales, con las operaciones por clave para guardar, añadir, leer y quitar
entradas, y valores de hasta 1.024 bytes.

Al cerrar este cambio, una aplicación puede crear, abrir, vaciar, renombrar, listar y borrar árboles dentro de una
transacción, y guardar y leer datos en ellos: es la primera vez que el producto guarda datos de la aplicación. Las
operaciones del catálogo siguen la semántica de los modos de apertura de archivos de .NET (`File` y `FileMode`), y
las operaciones por clave la de un diccionario de .NET (`Dictionary`).

**Por qué ahora**: es el primer submódulo del reparto; los demás (recorrido, valores grandes, multivalor, tamaño
fijo y compresión) se apoyan en el catálogo y en los árboles normales.

Este PRD hereda todo lo de su ascendencia, el [módulo de árboles](trees.prd.md) (B1–B12) y la
[raíz](krummi.prd.md), y lo de los módulos anteriores que el padre hereda: el [Pager](pager.prd.md) (P1–P17), las
[Transacciones](transactions.prd.md) (T1–T18), el [Diario](journal.prd.md) (J1–J17), la
[escritura agrupada y directa del diario](journal-group-commit.prd.md) (E1–E16) y el
[Espacio libre](free-space.prd.md) (F1–F14). Aquí solo se añade lo propio de 5.1. Lo que cambia o precisa reglas
anteriores está declarado en sus decisiones y anotado en los documentos afectados (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

En este documento, una **apertura** es lo que devuelve abrir o crear un árbol dentro de una transacción, y a través
de lo cual se opera con él (B9). **Borrar** se usa solo para árboles y **quitar** para entradas.

### Decisiones de producto

| ID | Decisión |
|---|---|
| A1 | **Modos de apertura** (como `FileMode` de .NET): un árbol se abre por su nombre y su tipo en uno de estos modos. **Abrir**: abre el árbol; si no existe, falla con la clase nueva de **árbol no encontrado**. **Crear nuevo**: crea un árbol vacío; si ya existe, falla con la clase nueva de **árbol ya existente**. **Abrir o crear**: abre el árbol si existe y, si no, lo crea vacío. **Crear o vaciar** (`Create`): crea el árbol vacío si no existe y, si existe, lo vacía. **Vaciar** (`Truncate`): vacía el árbol; si no existe, falla con árbol no encontrado. En una transacción de lectura solo existe el modo Abrir; los demás no compilan en ella. Un árbol existente de otro tipo sigue B3 en todos los modos. En 5.1 el único tipo es el normal. |
| A2 | **Vaciar un árbol** (modos Crear o vaciar y Vaciar): el árbol conserva su nombre, su tipo y su identidad (B9), se queda sin entradas y su número de entradas pasa a `0`. Las demás aperturas del mismo árbol en la transacción siguen valiendo y lo ven vacío. Vaciar libera sus páginas (F8): un árbol vaciado ocupa lo mismo que uno recién creado vacío. Es un cambio, aunque el árbol ya estuviera vacío o se acabe de crear en la misma transacción. |
| A3 | **Otras operaciones del catálogo** (como `File` de .NET): **Existe** dice si hay un árbol con ese nombre; no falla porque no exista. **Borrar** borra el árbol con todas sus entradas y libera sus páginas (F8); si no existe, no hace nada y no falla por eso, como `File.Delete`. **Renombrar** cambia el nombre de un árbol: si el origen no existe, falla con árbol no encontrado, también cuando el destino es el mismo nombre; si el destino ya existe, falla con árbol ya existente, salvo que se pida sobrescribirlo, y entonces el árbol de destino se borra como con Borrar, sea del tipo que sea (comprobable desde 5.4, cuando hay otro tipo). Renombrar un árbol existente a su mismo nombre no hace nada, también con sobrescritura. El árbol renombrado conserva su tipo, sus entradas y su identidad: sus aperturas siguen valiendo con el nombre nuevo. **Listar** devuelve el nombre y el tipo de cada árbol, en el orden de nombres de B5, como una lista propia de la aplicación: es una copia que sigue valiendo después de modificar el catálogo o de terminar la transacción (excepción a D11 del padre raíz y a B10 para el catálogo). Existe y Listar están en las dos clases de transacción; Borrar y Renombrar, solo en las de escritura. En una escritura, Existe y Listar ven sus propios cambios del catálogo; en una lectura, su instantánea. |
| A4 | **Operaciones por clave** (como `Dictionary` de .NET): **Guardar** crea la entrada o sobrescribe su valor si la clave ya está, y no devuelve nada, como el indexador. **Añadir** crea la entrada solo si la clave no está y devuelve si la añadió; si ya estaba, no cambia nada, como `TryAdd`. **Leer** devuelve el valor de la clave o que no está, y distingue una clave con valor vacío de una que no está, como `TryGetValue`. **Quitar** quita la entrada y devuelve si estaba, como `Remove`. Ninguna falla por la presencia o la ausencia de la clave, pero todas validan antes sus tamaños (B7): Añadir con un valor fuera de su tamaño falla aunque la clave ya esté. Leer está en las dos clases de transacción; Guardar, Añadir y Quitar, solo en las de escritura. |
| A5 | **Número de entradas** (como `Count`): una apertura informa el número de entradas del árbol según lo que ve su transacción (T2), con sus propios cambios en una escritura, al instante, sin recorrer el árbol. Consultarlo solo puede fallar con árbol no encontrado, a través de una apertura de un árbol borrado (A6). |
| A6 | **Errores nuevos** (D8 del padre raíz): **árbol no encontrado** y **árbol ya existente**, errores de validación pública con la regla de B7. Árbol no encontrado es también el error de B9 para una operación a través de una apertura de un árbol que la misma escritura borró, directamente o al sobrescribirlo con Renombrar. Los dos llevan los datos de T18 y un nombre de árbol: el pedido; en Renombrar, el de origen para no encontrado y el de destino para ya existente; y para una apertura de un árbol borrado, el último nombre que tuvo el árbol. |
| A7 | **Precedencia** (amplía B7): el tamaño no válido gana a todo y, en Renombrar, el nombre de origen se comprueba antes que el de destino. Después, los fallos al leer el catálogo (daño, memoria agotada, entrada y salida), el tipo de árbol distinto, árbol no encontrado, árbol ya existente y, por último, los fallos al leer las páginas del propio árbol. En Renombrar, un origen que no existe falla con árbol no encontrado sin mirar el destino. Una operación a través de una apertura de un árbol borrado falla con árbol no encontrado sin leer el catálogo ni las páginas del árbol, así que, después del tamaño no válido, gana a todo lo demás. |
| A8 | **Identidad de los árboles** (B9): borrar un árbol, directamente o al sobrescribirlo con Renombrar, termina su identidad. Un árbol creado después con ese nombre es nuevo, también en la misma transacción y aunque se repita: ninguna apertura alcanza nunca un árbol distinto del que abrió. Vaciar y renombrar conservan la identidad. Si alguna vez se agotaran las identidades posibles, crear un árbol seguiría B11. |
| A9 | **Daño en un árbol** (G6): los modos Abrir, Crear nuevo y Abrir o crear, Existe, Listar, Renombrar sin sobrescritura o a su mismo nombre y el número de entradas nunca fallan con daño en las páginas del propio árbol. Leer, Guardar, Añadir y Quitar fallan con daño en una página del árbol que necesitan, con la clase de daño y el número de la página. Borrar, vaciar (modos Crear o vaciar y Vaciar) y Renombrar con sobrescritura a otro nombre (en el árbol de destino) pueden fallar con daño en cualquier página del árbol afectado. Una página dañada falla igual en cada operación posterior que la necesite, como en F9. En una escritura, todo fallo por daño sigue T11. |
| A10 | **Cambios y referencias** (T8, T10, B9): terminan las referencias anteriores todas las operaciones que pueden modificar: los modos distintos de Abrir, Borrar, Renombrar, Guardar, Añadir y Quitar, aunque al final no cambien nada o fallen. Abrir, Existe, Listar, Leer y el número de entradas no las terminan. Para T10 son cambios: crear un árbol, vaciarlo, borrar un árbol que existe, renombrar a otro nombre, Guardar, y Añadir o Quitar cuando añaden o quitan. No lo son: Abrir o crear sobre un árbol existente, Borrar de un árbol que no existe, renombrar a su mismo nombre, Añadir que no añade, Quitar que no quita y toda operación que falla por un error de validación pública. |
| A11 | **Máximo provisional de un valor** (B6): 1.024 bytes. |
| A12 | **Catálogo bajo demanda**: el catálogo no ocupa páginas hasta que se crea el primer árbol, así que un almacén sin árboles cumple sin cambios las reglas de tamaño y de páginas libres de los módulos anteriores (P11, F10, AC-F15). |
| A13 | **Formato `6`**: el cambio es incompatible (B12): un motor de formato `5` no conoce el catálogo ni los árboles y no garantiza conservarlos al escribir en el almacén. La versión de formato vigente pasa a ser `6`, y un almacén de formato `5` es de formato anterior aunque esté vacío. |
| A14 | **Prueba de compilación previa**: vale la que exige el padre antes de implementar 5.1 ([reparto](trees.prd.md#reparto-en-prd-hijos)). |
| A15 | **Medición** (B12): el cambio incluye una medición reproducible, en Windows y en Linux, en un almacén en disco con confirmación durable. Con 1 millón de claves aleatorias de 16 bytes, las mismas para todas las cargas por una semilla fija, y valores de 100 bytes, mide: guardarlas en orden de clave en un almacén y, en otro, al azar, en escrituras de 1.000 entradas; en el almacén cargado al azar, tras cerrarlo y reabrirlo, leerlas todas al azar en una sola transacción de lectura y después quitarlas todas al azar en escrituras de 1.000; y el tamaño del almacén en bytes que da su información (P12) tras cada una de las dos cargas. El informe incluye el sistema operativo, el procesador y el disco. Sus resultados van en el informe de cierre. |

*Anotación del 2026-10-25, antes de ejecutar este PRD: una operación a través de una apertura de otra transacción, también de una ya terminada o de una lectura usada en una escritura, falla sin efecto con la clase de uso incorrecto y la transacción sigue utilizable (excepción a B1, anotada en el padre). Su precedencia es la de una apertura de un árbol borrado (A7): después del tamaño no válido, gana a todo lo demás. Lo comprueba este criterio: dada una apertura de una escritura ya confirmada y una de una lectura, cuando se usan en una escritura nueva del mismo almacén para leer, guardar y consultar el número de entradas, entonces cada operación falla con la clase de uso incorrecto sin efecto y la escritura nueva se confirma con sus demás cambios.*

### Estado de partida

Verificado el 2026-10-25 en el código:

| Aspecto | Situación |
|---|---|
| API pública | La biblioteca abre, consulta y cierra almacenes, y abre, confirma y aborta transacciones. No hay ninguna forma pública de guardar ni leer datos (T1, F1). |
| Catálogo y árboles | No existen. La página de metadatos, después del encabezado, guarda la frontera y el total de páginas libres (F3, F10). |
| Espacio libre | Una escritura reserva y libera páginas sueltas o en rangos (F4–F8). |
| Errores | Las doce clases del estado de partida del padre; las de tamaño no válido y tipo de árbol distinto (B7) no están implementadas. |
| Formato | La versión de formato vigente es `5` (F11). |

## Actores

- **Desarrollador integrador**: crea, abre, vacía, renombra, lista y borra árboles; guarda, añade, lee y quita
  entradas; consulta el número de entradas; y trata los errores nuevos por su clase.
- **Mantenedor**: ejecuta la medición de A15 en Windows y en Linux.

## Flujo

1. La aplicación abre una transacción y, dentro de ella, un árbol normal en uno de los modos de A1.
2. En una escritura guarda, añade y quita entradas (A4); en cualquier transacción las lee y consulta el número de
   entradas (A5).
3. Comprueba si un árbol existe y lista los árboles; en una escritura además los vacía, renombra y borra (A2, A3).
4. La confirmación aplica todos los cambios de los árboles y del catálogo, o ninguno (G1).

## Validaciones y mensajes

Valen las de los ascendientes, incluidas las de B3–B7, y los fallos de T11 y T12 en cualquier operación. Este PRD
añade estas situaciones, solo de la biblioteca (la herramienta no abre almacenes hasta el módulo 12), todas de
validación pública (B7, A6). La precedencia es la de A7.

| Situación | Regla | Clase de error |
|---|---|---|
| Se abre un árbol que no existe en modo Abrir, en cualquier transacción, o en modo Vaciar, en una escritura | A1 | Árbol no encontrado (nueva) |
| Se abre en modo Crear nuevo un árbol que ya existe con el mismo tipo | A1 | Árbol ya existente (nueva) |
| Se renombra un árbol que no existe, también a su mismo nombre | A3 | Árbol no encontrado (nueva) |
| Se renombra un árbol a un nombre que ya existe, sin sobrescribir | A3 | Árbol ya existente (nueva) |
| Se opera a través de una apertura de un árbol que la misma escritura borró, directamente o al sobrescribirlo | A6, B9 | Árbol no encontrado (nueva) |

## Salida

El módulo no escribe nada en pantalla. La información del almacén (P12) conserva sus datos (B11); la versión de
formato que informa pasa a ser `6` (A13). Listar devuelve pares de nombre y tipo; en un almacén sin árboles, una
lista vacía. Ejemplo, tras crear `pedidos`, `Clientes` y `clientes` como árboles normales:

| Nombre | Tipo |
|---|---|
| `Clientes` | normal |
| `clientes` | normal |
| `pedidos` | normal |

Ejemplo de los datos de un error de árbol ya existente, al renombrar `clientes` a `pedidos` sin sobrescribir en un
almacén en disco:

| Dato | Valor |
|---|---|
| Carpeta | la del almacén |
| Nombre | `pedidos` |

## Reutilización

- Las reglas comunes del módulo de árboles (B1–B12).
- Las transacciones, el diario y la recuperación (T2, T14, T16, G1, G5): los cambios de los árboles y del catálogo
  viajan como cualquier otro cambio.
- El espacio libre (F4–F8): los árboles reservan y liberan páginas a través de él.
- La comprobación de integridad de las páginas y la clase de daño (G6).
- La simulación de caídas, cortes de corriente y fallos de las pruebas de los módulos 3 y 4.

## Fuera de alcance

- Recorrer un árbol en orden y buscar por rango o por prefijo (5.2).
- Valores de más de 1.024 bytes (5.3).
- Árboles multivalor, de tamaño fijo y comprimidos (5.4, 5.5 y 5.6).
- Copiar un árbol, o mover entradas de un árbol a otro en una sola operación.
- El modo `Append` de `FileMode`, que no tiene sentido en un árbol ordenado.
- Que Borrar informe si el árbol existía (se pregunta antes con Existe, como en .NET), y un Existe que pida un tipo
  (Listar da el tipo).
- Guardar o quitar varias entradas en una sola operación, y las operaciones combinadas de `Dictionary` como
  `GetOrAdd` o `AddOrUpdate` (operaciones combinadas, fuera de alcance del padre).

## Dejado a la implementación

- Cómo se guarda el catálogo, si devuelve sus páginas cuando se borra el último árbol, si un árbol vacío conserva
  alguna página propia, cómo se distingue la identidad de un árbol de su nombre (A8) y la organización de las
  páginas de los árboles, siempre que se cumplan las garantías de los ascendientes.
- Si vaciar y borrar un árbol leen sus páginas o un registro propio de las páginas del árbol, para saber cuáles
  liberar (A2, A3, A9).
- Qué página dañada se informa cuando una operación de A9 encuentra varias, y qué ve la propia escritura de un árbol
  cuya operación falló con un fallo de T11 (la escritura ya no puede confirmarse).
- Los nombres de las operaciones, de los modos y de las clases nuevas, que siguen las convenciones de Rust y están
  en inglés, y el texto en inglés de los errores.
- La forma de la lista que devuelve Listar, siempre que dé cada nombre con su tipo en el orden de A3.
- La orden que ejecuta la medición de A15 y la forma de su informe.

## Criterios de aceptación

Valen las condiciones de prueba de los módulos anteriores y la cabecera de los criterios del padre: cada criterio va
en un almacén en disco y en modo temporal, salvo las partes que reabren el almacén o dañan sus archivos, que son
solo en disco. Este PRD comprueba los criterios del padre asignados a 5.1 (AC-B1, AC-B2, AC-B3 con su máximo
provisional, AC-B4 y AC-B6 en sus partes de 5.1, AC-B5, AC-B9 a AC-B13, AC-B14 en su parte del mismo tipo, AC-B17 y
AC-B18), repite AC10 y AC20 de la raíz con los árboles y añade estos. Las pruebas que provocan divisiones y fusiones
de páginas, o fallos en puntos concretos de una operación, usan el conocimiento de la propia implementación.

1. **AC-A1** — Dada una escritura, cuando abre en modo Abrir un árbol que no existe, entonces falla con la clase de
   árbol no encontrado con su nombre; en modo Crear nuevo, lo crea vacío, y otra vez en ese modo falla con árbol ya
   existente con su nombre; en modo Abrir o crear, lo crea si no existe y, si existe, lo abre con sus entradas; y
   dada una lectura, cuando abre un árbol que no existe, entonces falla con árbol no encontrado (A1, A6).
2. **AC-A2** — Dado un árbol con entradas, cuando se abre en modo Crear o vaciar o Vaciar, entonces queda sin
   entradas, con número de entradas `0`, y una apertura anterior del mismo árbol lo ve vacío y sigue funcionando;
   dado un árbol que no existe, Crear o vaciar lo crea y Vaciar falla con árbol no encontrado; y dado un árbol creado
   y llenado en la misma escritura, Vaciar lo deja vacío y la confirmación lo guarda vacío (A1, A2).
3. **AC-A3** — Dada una transacción de lectura, cuando abre un árbol, comprueba si existe, lista, lee entradas y
   consulta el número de entradas, entonces funciona; y abrirlo en cualquier otro modo, borrarlo, renombrarlo o
   guardar, añadir o quitar entradas no compila (A1, A3, A4).
4. **AC-A4** — Dado un árbol existente, cuando se pregunta si existe, entonces sí, y tras borrarlo, no; dada una
   escritura en disco que solo borra un árbol que no existe, cuando se confirma, entonces los archivos del almacén
   quedan idénticos byte a byte; y Existe y Borrar con un nombre vacío o de 1.025 bytes fallan con tamaño no válido
   (A3, A10, B4).
5. **AC-A5** — Dados los árboles `a` y `b`, cuando se renombra `a` a `c`, entonces `c` tiene las entradas de `a`,
   `a` no existe y una apertura anterior de `a` sigue funcionando; renombrar `c` a `b` falla con árbol ya existente
   con el nombre `b`, y con sobrescritura deja `b` con las entradas de `c`, sigue valiendo la apertura de `c` y hace
   fallar con árbol no encontrado, con el nombre `b`, una apertura anterior del `b` original; renombrar un árbol que
   no existe falla con árbol no encontrado con su nombre, también a su mismo nombre; renombrar `b` a `b` no cambia
   nada; y dado un árbol `t` abierto, renombrado a `d` y borrado, cuando se usa la apertura de `t`, entonces falla
   con árbol no encontrado con el nombre `d` (A3, A6).
6. **AC-A6** — Dado un árbol `a` renombrado a `c` y otra vez a `a`, cuando se usa su apertura original, entonces
   sigue funcionando; y dado `a` renombrado a `c`, un árbol nuevo creado como `a` y `c` renombrado a `a` con
   sobrescritura, cuando se usa la apertura del `a` nuevo, entonces falla con árbol no encontrado (A3, A8).
7. **AC-A7** — Dados árboles creados en desorden, incluidos nombres que solo difieren en mayúsculas, cuando se listan
   en una lectura, entonces salen con su tipo en orden de bytes; en un almacén sin árboles, la lista está vacía; en
   una escritura, la lista incluye los creados, renombrados y borrados por ella; y la lista obtenida sigue disponible
   después de borrar un árbol y de terminar la transacción (A3).
8. **AC-A8** — Dado un árbol, cuando se guarda una clave nueva y luego la misma clave con otro valor, entonces Leer
   devuelve el segundo valor; Añadir con esa clave devuelve que no añadió y el valor no cambia; Añadir con una clave
   nueva devuelve que añadió; Leer una clave que no está devuelve que no está, y Leer una clave guardada con valor
   vacío devuelve un valor vacío; Quitar devuelve que estaba la primera vez y que no estaba la segunda; y Añadir con
   una clave que ya está y un valor de 1.025 bytes falla con tamaño no válido (A4).
9. **AC-A9** — Dada una escritura en disco que solo hace Añadir sobre claves existentes, Quitar sobre claves que no
   están, Abrir o crear sobre un árbol existente y operaciones que fallan con árbol no encontrado o árbol ya
   existente, cuando se confirma, entonces termina sin error y los archivos del almacén quedan idénticos byte a byte;
   y una escritura en disco que solo vacía un árbol ya vacío sí escribe en el almacén (A10, T10).
10. **AC-A10** — Dado un árbol con entradas suficientes para que se dividan y se fusionen páginas, de modo que el
    árbol gane y pierda niveles, cuando se guardan claves nuevas, se sobrescriben, se añaden claves nuevas y
    existentes, se quitan claves presentes y ausentes, se quitan todas hasta la última y se vacía, entonces tras cada
    operación el número de entradas coincide con las claves que Leer encuentra entre todas las que la prueba usó; una
    lectura abierta antes ve el número de su instantánea; tras abortar una escritura, el número es el de antes; tras
    confirmar y reabrir, el número se conserva; y con una página propia del árbol dañada, consultarlo no falla (A5,
    A9).
11. **AC-A11** — Dada una escritura con un árbol `t`, cuando repite diez veces, alternando, estos dos pasos: (a) abrir
    `t`, borrarlo y crearlo otra vez en modo Crear nuevo; (b) abrir `t`, crear un árbol `u` en modo Crear nuevo y
    renombrar `u` a `t` con sobrescritura; entonces cada apertura de `t` anterior a la última sustitución falla con
    árbol no encontrado, y solo una apertura de `t` hecha después de la última sustitución ve el árbol vigente (A6,
    A8).
12. **AC-A12** — Dado un árbol con una de sus páginas dañada, cuando se abre, se pregunta si existe, se lista o se
    renombra sin sobrescritura, entonces no falla; cuando se lee una entrada de esa página, entonces falla con la
    clase de daño y el número de la página, y vuelve a fallar igual al repetirlo; y cuando una escritura lo borra, lo
    vacía, lo sobrescribe con Renombrar o guarda en esa página y falla con daño, entonces confirmarla falla con la
    clase de daño y no aplica nada (A9, T11).
13. **AC-A13** — Dadas operaciones que cumplen varias situaciones a la vez, cuando se ejecutan, entonces fallan con
    la que gana según A7: renombrar con un origen de 1.025 bytes a un destino existente da tamaño no válido;
    renombrar un origen que no existe a un destino de 1.025 bytes da tamaño no válido; renombrar un origen que no
    existe a un destino existente da árbol no encontrado, también en un catálogo de varias páginas con la del destino
    dañada y la página donde estaría el origen intacta;
    guardar una clave de 2.049 bytes a través de una apertura de un árbol borrado da tamaño no válido; y una
    apertura de un árbol borrado cuya página está dañada da árbol no encontrado (A7).
14. **AC-A14** — Dado un almacén nuevo, cuando se consulta su información antes de crear ningún árbol, entonces su
    tamaño es como mucho 1 MiB, las páginas libres son las páginas menos las propias del almacén (AC-F15) y la lista
    de árboles está vacía (A12).
15. **AC-A15** — Dado un almacén con un árbol ya creado, que se mantiene dentro de una sola región del espacio libre,
    y tomando las páginas en uso (las páginas menos las páginas libres que da la información) cuando cada
    confirmación es durable (en modo temporal, cuando devuelve): cuando dos veces seguidas se crea un árbol `x`, se
    llena con entradas suficientes para ocupar varias páginas y varios niveles, se confirma, se borra y se confirma,
    entonces las páginas en uso tras el segundo borrado son las mismas que tras el primero, y las de antes del primer
    ciclo cuando la entrada de `x` no hizo crecer el catálogo (las pruebas lo saben por la implementación); y cuando
    se crea un árbol vacío, se confirma, se toma el valor, se llena igual, se confirma, se vacía y se confirma,
    entonces el valor final es el tomado con el árbol vacío (A2, A3, F10, F12).
16. **AC-A16** — Dada una escritura que crea, renombra, vacía y borra árboles, y guarda y quita entradas con
    divisiones y fusiones de páginas, cuando se provoca una caída del proceso y simulada de corriente en cada punto en
    que su confirmación escribe o lleva datos al disco, entonces al reabrir la lista de árboles, cada Leer de las
    claves que la prueba usó y el número de entradas de cada árbol están todos como antes de la escritura o todos
    como después, nunca a medias, y el número de entradas coincide con las claves que Leer encuentra (G1, G5, A5).
17. **AC-A17** — Dada una escritura, cuando guarda un valor de 1.024 bytes con una clave de 2.048 bytes, entonces se
    guarda; y uno de 1.025 bytes falla con tamaño no válido, con mínimo `0` y máximo `1024` (A11, B6).
18. **AC-A18** — Dado un almacén de formato `5`, aunque esté vacío, cuando se abre, entonces falla con la clase de
    formato distinto como formato anterior (A13, AC-B12).
19. **AC-A19** — Dada la medición de A15, cuando se revisa el informe de cierre, entonces contiene sus tiempos, los
    tamaños del almacén y los datos del equipo en Windows y en Linux (A15).
20. **AC-A20** — Dado el código de una aplicación de prueba con una referencia leída de un árbol, cuando la usa
    después de abrir otro árbol en modo Abrir, de Existe, de Listar o de Leer, entonces compila; cuando la usa
    después de abrir un árbol en cualquier otro modo, de Borrar o de Renombrar, aunque fallen o no cambien nada,
    entonces no compila; y una apertura de una transacción de lectura se puede pasar a otro hilo, mientras que pasar
    a otro hilo una de una escritura no compila (A10, B1).

## Anotaciones a otros PRD

Este PRD precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Módulo de árboles, B7: los errores de validación pública del módulo incluyen también árbol no encontrado y árbol
  ya existente, y su precedencia es la de A7 (A6, A7).
- Módulo de árboles, B9: el error de una apertura de un árbol borrado es árbol no encontrado, con el último nombre
  del árbol (A6).
- Módulo de árboles, B9: abrir un árbol en modo Abrir o crear termina las referencias aunque el árbol exista, porque
  el compilador no sabe si lo creará (A10).
- Módulo de árboles, AC-B18: la parte de fallos durante un cambio de estructura simula un fallo de T11 en cada punto
  en que la operación reserva, lee o libera una página, también después de haber modificado varias, en divisiones y
  fusiones que se propagan por varios niveles. Confirmar falla entonces con la clase del primer fallo y no aplica
  nada, y tras abortar, o tras esa confirmación fallida, el árbol, el catálogo y el número de entradas están como
  antes y una escritura nueva los lee y modifica sin error. Las pruebas usan el conocimiento de la implementación
  para alcanzar cada punto.
- Padre raíz, D11; módulo de árboles, B10: Listar devuelve una copia propia de la aplicación, no referencias (A3).
- Transacciones, T10 y T11: las operaciones que fallan con árbol no encontrado o árbol ya existente no son cambios y
  no impiden confirmar (A6, A10).
- Padre raíz, D7; Pager, P5; Diario, J3; escritura agrupada, E15; Espacio libre, F11; Transacciones, todo el
  documento: la versión de formato vigente pasa a ser `6`, y un almacén de formato `5` es de formato anterior (A13).

## Referencias

- Padre: [trees.prd.md](trees.prd.md) v1.0; raíz: [krummi.prd.md](krummi.prd.md) v1.1
- Relacionados: [transactions.prd.md](transactions.prd.md), [free-space.prd.md](free-space.prd.md)
- .NET, `File` y `FileMode`: <https://learn.microsoft.com/en-us/dotnet/api/system.io.file?view=net-10.0>,
  <https://learn.microsoft.com/en-us/dotnet/api/system.io.filemode?view=net-10.0>
- .NET, `Dictionary<TKey,TValue>`:
  <https://learn.microsoft.com/en-us/dotnet/api/system.collections.generic.dictionary-2?view=net-10.0>
- Voron, árboles: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/BTrees>

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 31c36231 | 2026-10-24 10:22 UTC+01:00 | 20 h 59 min | 19 h 3 min | 1 h 56 min | 45 min | 1 h 11 min | 32 |

<!-- prd-time: [{"session": "31c36231-b691-4c62-8559-6f87e09084cf", "start": "2026-10-24T09:22:37.952000+00:00", "wall": 75547, "discarded": 68592, "agent": 2676, "human": 4278, "interventions": 32, "idle_minutes": 10}] -->
