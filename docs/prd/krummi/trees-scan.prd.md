# Recorrido en orden y búsqueda por prefijo de Krummi — PRD

**Versión**: 1.0
**Fecha**: 2026-10-26
**Documento padre**: [Árboles B+ de Krummi — PRD](trees.prd.md) (v1.0)

## Objetivo y contexto

Construir el PRD hijo **5.2** del [reparto del módulo de árboles](trees.prd.md#reparto-en-prd-hijos): recorrer en
orden las entradas de un árbol normal entre dos límites, hacia delante y hacia atrás, limitar el recorrido a las
claves que empiezan por un prefijo y quitar de una vez todas las entradas de un intervalo.

Al cerrar este cambio, una aplicación puede leer un árbol por intervalos y no solo clave a clave: listar todas sus
entradas, las de un intervalo de claves o las de un prefijo, en orden ascendente o descendente, y vaciar un intervalo
sin quitar las claves una a una. Los recorridos siguen la semántica de `BTreeMap::range` de la biblioteca estándar
de Rust, como 5.1 siguió la de `FileMode` y `Dictionary` de .NET, con la diferencia de R3. El reparto del padre habla
de recorrer "desde una clave dada"; los dos límites de este PRD lo generalizan: esa clave es uno de los límites y el
otro puede quedar abierto.

**Por qué ahora**: es el segundo submódulo del reparto; los índices de las tablas (módulo 7) y los valores de una
clave multivalor (5.4) se leen por intervalos y por prefijo.

Este PRD hereda todo lo de su ascendencia, el [módulo de árboles](trees.prd.md) (B1–B12) y la
[raíz](krummi.prd.md), y lo de los módulos anteriores que el padre hereda: el [Pager](pager.prd.md) (P1–P17), las
[Transacciones](transactions.prd.md) (T1–T18), el [Diario](journal.prd.md) (J1–J17), la
[escritura agrupada y directa del diario](journal-group-commit.prd.md) (E1–E16) y el
[Espacio libre](free-space.prd.md) (F1–F14). Reutiliza además lo que fijó su hermano
[Árboles y operaciones por clave](trees-keys.prd.md) (A1–A15 y su anotación). Aquí solo se añade lo propio de 5.2.
Lo que precisa reglas anteriores está declarado en sus decisiones y anotado en los documentos afectados (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

En este documento, un **intervalo** es el conjunto de claves entre un límite inferior y uno superior; cada **límite**
es una clave incluida, una clave excluida o **abierto** (sin límite por ese lado). Un **recorrido** es lo que
devuelve pedir las entradas de un intervalo, con o sin prefijo, de un árbol, y del que se van sacando entradas por
cualquiera de sus dos extremos. Una **apertura** es la de 5.1.

### Decisiones de producto

| ID | Decisión |
|---|---|
| R1 | **Recorrido** (como `BTreeMap::range`): se pide a través de una apertura de un árbol, con un límite inferior y uno superior. Devuelve las entradas del intervalo, cada una como su clave y su valor, en el orden de claves de B5. Se consume por los dos extremos: el **delantero** saca la siguiente entrada en orden ascendente y el **trasero** la siguiente en orden descendente, en cualquier alternancia; cada entrada sale una sola vez, y el recorrido termina cuando los dos extremos se encuentran. Está en las dos clases de transacción. |
| R2 | **Dónde empieza**: las claves límite no tienen por qué existir en el árbol. El extremo delantero empieza en la primera clave mayor o igual que el límite inferior si está incluido, en la primera mayor si está excluido, o en la primera del árbol si está abierto. El trasero empieza en la última clave menor o igual que el límite superior si está incluido, en la última menor si está excluido, o en la última del árbol si está abierto. |
| R3 | **Intervalo vacío** (diferencia con `BTreeMap::range`, que detiene el programa cuando el límite inferior va después del superior, o cuando los dos tienen la misma clave y los dos están excluidos): si el límite inferior va después del superior, o los dos tienen la misma clave y alguno está excluido, o ninguna clave que empiece por el prefijo (R4) puede caer entre los límites (por ejemplo, el prefijo `a` con el límite inferior `c`), el intervalo está vacío y no es un error: el recorrido no devuelve ninguna entrada y Quitar intervalo (R7) quita `0`. Como se decide solo con los argumentos, no lee ninguna página del árbol y no puede fallar por daño. Un intervalo que no está vacío por esta regla pero no tiene claves en el árbol sí lee páginas (R8). |
| R4 | **Prefijo**: un recorrido puede pedir además un prefijo, y entonces devuelve solo las claves del intervalo que empiezan por esos bytes, en los dos sentidos. Una clave igual al prefijo empieza por él. El prefijo vacío no limita nada. Con un prefijo que acaba en bytes `0xFF` se devuelven todas sus claves; la aplicación no tiene que calcular la clave siguiente al prefijo. |
| R5 | **Tamaños** (precisa B5, B7 y B8): cada clave límite se valida como una clave (B5), de 1 a 2.048 bytes, y fuera de ese tamaño falla con tamaño no válido como clave, con mínimo `1` y máximo `2048`; un límite abierto no tiene clave y no se valida. El prefijo es de 0 a 2.048 bytes; fuera de ese tamaño falla con tamaño no válido, cuyo dato de qué incumple su tamaño es el valor nuevo **prefijo**, con mínimo `0` y máximo `2048`. Se valida siempre, aunque el intervalo esté vacío (R3) o el árbol no tenga entradas. Si varios argumentos están fuera de su tamaño, se informa del primero en este orden: límite inferior, límite superior, prefijo. |
| R6 | **Recorridos y cambios** (B9, T8): un recorrido ve lo que ve su transacción en el momento de pedirlo: en una lectura, su instantánea (G3); en una escritura, con todos sus cambios hechos hasta ese momento. Un recorrido de una escritura deja de valer en la siguiente operación que puede modificar la transacción (A10), en ese árbol o en otro, aunque falle sin efecto: seguir usándolo después no compila. Pedir un recorrido y sacar entradas de él no modifica: no terminan las referencias ni otros recorridos. Las claves y los valores que devuelve son referencias sin copia con la vida de las de Leer (D11, T8): en una lectura valen mientras la transacción esté abierta, también después de soltar el recorrido; en una escritura, hasta la siguiente operación que puede modificarla. |
| R7 | **Quitar intervalo**: en una escritura, quita de un árbol todas las entradas de un intervalo, con un prefijo o sin él, con las reglas de R2 a R5, y devuelve cuántas quitó. El número de entradas del árbol (A5) baja en esa cantidad. El árbol conserva su nombre, su tipo y su identidad (A8), y las demás aperturas suyas en la transacción siguen valiendo y ven el árbol sin las entradas quitadas. Las páginas que deja de usar vuelven al espacio libre (F8), y un árbol que se queda sin entradas, por uno o por varios Quitar intervalo, ocupa lo mismo que uno vaciado (A2). Termina las referencias y los recorridos anteriores aunque no quite nada o falle (A10). Para T10 es un cambio solo si quita al menos una entrada; a diferencia de Vaciar, quitar todo de un árbol ya vacío no es un cambio. No está en las transacciones de lectura: usarlo en una no compila. |
| R8 | **Errores**: no hay clases nuevas. **Precedencia** (amplía A7): primero el tamaño no válido, en el orden de R5; después, una apertura de otra transacción (uso incorrecto) y una apertura de un árbol que la misma escritura borró (árbol no encontrado, con el último nombre del árbol, A6), las dos sin leer ninguna página; después, los fallos al leer las páginas del árbol. Un recorrido falla con la clase de daño y el número de la página cuando necesita una página dañada, como Leer (A9). Necesita las páginas que contienen claves del intervalo, las que llevan desde la raíz hasta donde empieza y donde acaba el intervalo, también cuando no tiene claves, y las vecinas de esas que lea para saber que el intervalo terminó. Quitar intervalo necesita esas mismas páginas y además las vecinas que lee para reorganizar el árbol. Una página que no es ninguna de esas no hace fallar ni a un recorrido ni a Quitar intervalo. Valen también los demás fallos de T11: un recorrido puede fallar con memoria agotada o con entrada y salida, y Quitar intervalo además con disco lleno o tamaño máximo. En una escritura, todos siguen T11. **Después de un fallo**, el recorrido termina: los dos extremos dejan de devolver entradas, sin repetir el error; las entradas que devolvió antes son correctas. En una lectura, la transacción sigue utilizable y un recorrido nuevo funciona, aunque vuelve a fallar igual si necesita la misma página dañada (F9). |
| R9 | **Hilos**: un recorrido sigue la regla de su apertura (B1): uno de una lectura se puede pasar a otro hilo, y pasar a otro hilo uno de una escritura no compila. |
| R10 | **Formato**: el cambio no modifica lo que se guarda en disco (B12): la versión de formato vigente sigue siendo `6`, los almacenes creados con 5.1 se abren y se recorren sin conversión, y las páginas que deja Quitar intervalo cumplen las mismas comprobaciones de consistencia que las que dejan las operaciones de 5.1, de modo que un motor de 5.1 las lee y las modifica sin riesgo. |
| R11 | **Medición** (B12): el cambio incluye una medición reproducible, en Windows y en Linux, en un almacén en disco con confirmación durable, sobre el almacén cargado al azar de A15 de 5.1 (1 millón de claves aleatorias de 16 bytes con su semilla fija y valores de 100 bytes), tras cerrarlo y reabrirlo. Mide, en una sola transacción de lectura y leyendo los bytes de cada clave y cada valor devueltos: recorrer todas las entradas hacia delante y después hacia atrás; y 10.000 recorridos hacia delante de 100 entradas desde claves límite al azar. Después, quitar todas las entradas con 1.000 intervalos contiguos que dividen las claves en cortes al azar, el primero con el límite inferior abierto y el último con el superior abierto, uno por escritura (A15 usaba 1.000 operaciones por escritura; aquí cada escritura es un solo Quitar intervalo). Cada parte de la medición parte de una copia del almacén cargado. El informe incluye el sistema operativo, el procesador y el disco. Sus resultados van en el informe de cierre. |

*Anotaciones del 2026-10-27 desde los valores grandes ([trees-large-values.prd.md](trees-large-values.prd.md)):*

- *R8: un daño en las páginas de un valor guardado fuera de su hoja hace fallar a un recorrido que necesita su entrada, pero nunca a Quitar intervalo, que no lee esas páginas (decisión V7).*
- *R10, Salida y AC-R14: la versión de formato vigente pasa a ser `7`; un almacén de formato `6`, también el creado con 5.1 y conservado como dato de prueba, es de formato anterior (decisión V9).*
- *Fuera de alcance: los valores de más de 1.024 bytes en recorridos y en Quitar intervalo llegan con los valores grandes (decisiones V1 y V2).*

### Estado de partida

Verificado el 2026-10-26 en el código:

| Aspecto | Situación |
|---|---|
| API pública | Un árbol normal se abre, se vacía, se renombra, se lista y se borra; sus entradas se guardan, se añaden, se leen y se quitan de una en una por su clave, y se consulta su número de entradas (A1–A5). No hay ninguna forma pública de recorrer un árbol ni de buscar por intervalo o por prefijo. |
| Recorrido interno | La biblioteca solo recorre internamente un árbol entero, de principio a fin: el catálogo, que también es un árbol, para Listar; un árbol que se borra o se vacía, para liberar sus páginas; y los árboles, en las pruebas. No recorre parte de un árbol ni en sentido descendente. |
| Errores | Dieciséis clases: las doce del estado de partida del padre más tamaño no válido, tipo de árbol distinto, árbol no encontrado y árbol ya existente. Tamaño no válido informa nombre, clave o valor como lo que incumple su tamaño. Una apertura de otra transacción se comprueba antes que la de un árbol borrado. |
| Formato | La versión de formato vigente es `6` (A13). |

## Actores

- **Desarrollador integrador**: recorre árboles por intervalos y por prefijo en los dos sentidos, quita intervalos y
  trata los errores por su clase.
- **Mantenedor**: ejecuta la medición de R11 en Windows y en Linux.

## Flujo

1. La aplicación abre una transacción y un árbol normal (A1).
2. Pide un recorrido con dos límites y, si quiere, un prefijo (R1–R5).
3. Saca entradas por el extremo delantero, por el trasero o por los dos, hasta que el recorrido termina o lo suelta.
4. En una escritura, quita intervalos (R7); una modificación termina los recorridos abiertos (R6).
5. La confirmación aplica todos los cambios, o ninguno (G1).

## Validaciones y mensajes

Valen las de los ascendientes y las de 5.1, incluidos los fallos de T11 y T12. Este PRD añade estas situaciones, solo
de la biblioteca (la herramienta no abre almacenes hasta el módulo 12); las de tamaño no válido, árbol no encontrado
y uso incorrecto por una apertura de otra transacción son de validación pública (B7). La precedencia es la de R8.

| Situación | Regla | Clase de error |
|---|---|---|
| Una clave límite vacía o de más de 2.048 bytes, al recorrer o al quitar un intervalo | R5 | Tamaño no válido, como clave |
| Un prefijo de más de 2.048 bytes, al recorrer o al quitar un intervalo | R5 | Tamaño no válido, como prefijo |
| Límites invertidos, o iguales con alguno excluido | R3 | Ninguna: intervalo vacío |
| Se recorre o se quita un intervalo a través de una apertura de otra transacción | R8 | Uso incorrecto |
| Se recorre o se quita un intervalo a través de una apertura de un árbol que la misma escritura borró | R8, A6 | Árbol no encontrado |
| Un recorrido o Quitar intervalo necesita una página dañada del árbol | R8, A9 | Daño, con el número de la página |

## Salida

El módulo no escribe nada en pantalla. La información del almacén (P12) conserva sus datos y la versión de formato
sigue siendo `6` (R10). Un recorrido devuelve pares de clave y valor; Quitar intervalo, un número. Ejemplo, en un
árbol con las claves `a`, `ab`, `b`, `ba` y `c`:

| Pedido | Extremo | Claves devueltas |
|---|---|---|
| Desde `ab` incluido hasta `b` incluido | delantero | `ab`, `b` |
| Desde `ab` incluido hasta `b` incluido | trasero | `b`, `ab` |
| Desde `a` excluido hasta `ba` incluido | delantero | `ab`, `b`, `ba` |
| Desde `aa` excluido hasta abierto | delantero | `ab`, `b`, `ba`, `c` |
| Abierto hasta `b` excluido | trasero | `ab`, `a` |
| Prefijo `b`, límites abiertos | delantero | `b`, `ba` |
| Prefijo `b`, límites abiertos | trasero | `ba`, `b` |
| Prefijo `a`, desde `aa` incluido hasta abierto | delantero | `ab` |
| Prefijo vacío, límites abiertos | delantero | `a`, `ab`, `b`, `ba`, `c` |
| Desde `c` incluido hasta `a` incluido | cualquiera | ninguna |
| Prefijo `d`, límites abiertos | cualquiera | ninguna |

Sacando alternativamente por los dos extremos un recorrido de todo el árbol, empezando por el delantero, salen `a`,
`c`, `ab`, `ba` y `b`, y después ninguna más por ningún extremo. Quitar el intervalo con prefijo `b` y límites
abiertos en ese árbol devuelve `2`; quitar desde `ab` incluido hasta `ba` excluido con prefijo `b` devuelve `1` (solo
`b`).

Ejemplo de los datos de un error de tamaño no válido, por un prefijo de 2.049 bytes en un almacén en disco:

| Dato | Valor |
|---|---|
| Carpeta | la del almacén |
| Qué incumple su tamaño | prefijo |
| Tamaño recibido | `2049` |
| Mínimo | `0` |
| Máximo | `2048` |

## Reutilización

- Las reglas comunes del módulo de árboles (B1–B12) y las de 5.1: aperturas, errores, precedencia, daño, identidad y
  cambios (A5–A10).
- El orden de claves de B5, el mismo que ya usan Leer y Listar.
- Las transacciones, su instantánea y las lecturas sin copia (T2, T8, T16, D11).
- El espacio libre (F4–F8) para las páginas que libera Quitar intervalo, y la liberación de páginas de Vaciar (A2).
- La simulación de caídas, cortes de corriente y fallos de las pruebas de los módulos 3 a 5.1.

## Fuera de alcance

- Cursores que se reposicionan o cambian de sentido sin pedir otro recorrido (modelo de Voron y LMDB), y búsquedas
  sueltas de la clave anterior o siguiente a una dada, o de la primera o la última (las da un recorrido con un solo
  paso por un extremo).
- Recorridos que siguen valiendo después de modificar la transacción, quitar o guardar entradas a través de un
  recorrido, y quitar un intervalo devolviendo sus entradas.
- Contar las entradas de un intervalo sin recorrerlo.
- Recorrer o quitar varios intervalos en una sola operación, o intervalos de varios árboles a la vez.
- Listar los árboles del catálogo por intervalo o por prefijo de nombre.
- Valores de más de 1.024 bytes en recorridos y en Quitar intervalo: llegan con los valores grandes (5.3), que
  extiende estas operaciones.
- Recorridos de árboles multivalor, de tamaño fijo y comprimidos (5.4, 5.5 y 5.6, que fijan sus propios recorridos).
- Copiar un intervalo a otro árbol.

## Dejado a la implementación

- Cómo se recorre y cómo se quita un intervalo por dentro, y la organización de las páginas, siempre que no cambie el
  formato (R10).
- En qué paso aparece un fallo: al pedir el recorrido, al sacar la primera entrada de la página que falla o antes,
  al sacar las últimas de la página anterior.
- Qué página dañada se informa cuando una operación de R8 encuentra varias, y qué páginas vecinas lee un recorrido
  para saber que el intervalo terminó y Quitar intervalo para reorganizar el árbol (R8).
- Cuántas páginas libera un Quitar intervalo que deja entradas en el árbol, siempre que se cumpla R7 cuando lo deja
  vacío.
- Qué ve la propia escritura de un árbol cuyo Quitar intervalo falló con un fallo de T11 (la escritura ya no puede
  confirmarse), como en 5.1.
- Los nombres de las operaciones y de la representación de los límites, que siguen las convenciones de Rust y están
  en inglés.
- La orden que ejecuta la medición de R11 y la forma de su informe.

## Criterios de aceptación

Valen las condiciones de prueba de los módulos anteriores y la cabecera de los criterios del padre: cada criterio va
en un almacén en disco y en modo temporal, salvo las partes que reabren el almacén o dañan sus archivos, que son
solo en disco. Este PRD comprueba el criterio del padre asignado a 5.2 (AC-B7 en su parte de 5.2), y AC-B17 y
AC-B18 con Quitar intervalo, que cambia la estructura de un árbol (AC-R7 y AC-R12); repite AC10 y AC20 de la raíz con
los recorridos, y añade estos. Las pruebas que provocan divisiones, fusiones, páginas dañadas o fallos en puntos
concretos usan el conocimiento de la propia implementación.

1. **AC-R1** — Dado el árbol del ejemplo de *Salida*, cuando se piden los recorridos de su tabla, entonces cada uno
   devuelve exactamente las claves indicadas, en ese orden, con sus valores; sacando alternativamente por los dos
   extremos un recorrido de todo el árbol, empezando por el delantero, salen `a`, `c`, `ab`, `ba` y `b`, y después
   ninguna más por ningún extremo; y los dos Quitar intervalo del ejemplo, cada uno sobre el árbol original,
   devuelven `2` y `1` y quitan esas claves (R1–R4, R7).
2. **AC-R2** — Dado un árbol con entradas suficientes para tener varios niveles, cuando se recorre entero hacia
   delante, hacia atrás y alternando extremos al azar, entonces salen todas sus claves, en orden, en orden inverso y
   una vez cada una, y su número coincide con el número de entradas (A5); y cuando se piden recorridos cuyos límites
   son una clave existente, una clave entre dos claves vecinas (también donde cambian de página) o una clave antes
   de la primera o después de la última, cada uno incluido y excluido, entonces cada extremo empieza donde dice R2
   (R1, R2).
3. **AC-R3** — Dadas claves con bytes de `0x00` a `0xFF`, incluidas `FF`, `FF FF`, `FF FF 00` y `FF FF FF`, cuando se
   recorre el prefijo `FF FF`, entonces salen `FF FF`, `FF FF 00` y `FF FF FF` hacia delante, y en orden inverso hacia
   atrás; con el prefijo vacío, todas las claves; con un prefijo de 2.048 bytes igual a una clave, esa clave; y con
   un prefijo que no tiene ninguna clave, ninguna (R4).
4. **AC-R4** — Dados límites invertidos, límites con la misma clave y alguno excluido, y el prefijo `a` con el
   límite inferior `c`, cuando se recorre y se quita el intervalo, entonces no sale ninguna entrada, Quitar intervalo
   devuelve `0` y no fallan, también con la raíz del árbol dañada; a través de una apertura de un árbol borrado,
   fallan igual con árbol no encontrado; con la misma clave incluida en los dos
   límites, sale esa entrada si existe, una sola vez; dada una escritura en disco que solo pide recorridos, quita
   intervalos vacíos o sin claves en el árbol y operaciones que fallan por tamaño no válido, cuando se confirma,
   entonces termina sin error y los archivos del almacén quedan idénticos byte a byte; y una escritura en disco que
   quita una sola entrada con Quitar intervalo sí escribe en el almacén (R3, R7, T10).
5. **AC-R5** — Dada una transacción de lectura y una de escritura, cuando recorren con un límite inferior o
   superior vacío o de 2.049 bytes, y la escritura además quita un intervalo con ellos, entonces falla con tamaño no
   válido sin efecto, como clave, con el tamaño recibido, mínimo `1` y máximo `2048`; con un prefijo de 2.049 bytes,
   como prefijo, con mínimo `0` y máximo `2048`; con límites de 1 y de 2.048 bytes y prefijos de 0 y de 2.048 bytes,
   no falla; con los dos límites fuera de tamaño, el error es del inferior, y con el superior y el prefijo, del
   superior; con un límite fuera de tamaño en un árbol sin entradas o en un intervalo invertido, falla igual; y la
   transacción sigue utilizable (R5, B7).
6. **AC-R6** — Dada una escritura que guarda y quita entradas y después pide un recorrido, cuando lo consume,
   entonces ve esos cambios; y dada una escritura con un árbol abierto dos veces, cuando quita un intervalo a través
   de una apertura, entonces la otra ve el árbol sin las entradas quitadas y el número de entradas nuevo (R6, R7).
7. **AC-R7** — Dada una transacción de lectura con un recorrido a medias y otro sin empezar, cuando una escritura
   quita intervalos y guarda entradas hasta cambiar la altura del árbol y se confirma, entonces los dos recorridos
   devuelven exactamente las entradas de su instantánea, y una lectura nueva ve las de la escritura (R6, G3,
   AC-B17).
8. **AC-R8** — Dado el código de una aplicación de prueba con un recorrido de una escritura, cuando lo usa después de
   guardar, añadir, quitar, quitar un intervalo, abrir un árbol en un modo distinto de Abrir, borrar o renombrar un
   árbol, aunque fallen o no cambien nada, entonces no compila; cuando lo usa después de Leer, de abrir un árbol en
   modo Abrir, de Existe, de Listar, del número de entradas o de pedir otro recorrido, entonces compila; una clave
   devuelta por un recorrido de una lectura se usa después de soltar el recorrido, y una de una escritura se usa
   después de soltarlo pero no después de la siguiente operación que puede modificar; Quitar intervalo en una
   transacción de lectura no compila; y pasar a otro hilo un recorrido de una lectura compila, y uno de una escritura
   no (R6, R7, R9, AC-B11).
9. **AC-R9** — Dado un árbol con varios niveles, cuando se quitan intervalos que caen dentro de una página, que
   cruzan varias páginas y niveles, con prefijo y sin él, entonces cada uno devuelve cuántas entradas quitó, solo
   esas claves desaparecen, el número de entradas baja en esa cantidad y un recorrido del árbol entero devuelve las
   demás; y tras confirmar y reabrir, se conserva (R7, A5).
10. **AC-R10** — Dado un almacén con un árbol ya creado, que se mantiene dentro de una sola región del espacio libre,
    tomando las páginas en uso como en AC-A15, cuando se crea un árbol vacío, se confirma, se toma el valor, se llena
    con entradas suficientes para ocupar varias páginas y varios niveles, se confirma, se le quitan todas sus
    entradas con un intervalo de límites abiertos y se confirma, entonces el valor final es el tomado con el árbol
    vacío; y lo mismo cuando se le quitan con diez intervalos contiguos, en escrituras distintas (R7, A2, F10).
11. **AC-R11** — Dada una escritura con un árbol abierto dos veces, cuando lo borra a través de una apertura y
    después pide un recorrido o quita un intervalo con la otra, entonces falla con árbol no encontrado, con el último
    nombre del árbol, sin efecto; dada una apertura de una transacción ya terminada, cuando se usa en otra para
    recorrer o quitar un intervalo, entonces falla con uso incorrecto sin efecto; en cada uno de esos dos casos, con
    además un límite de 2.049 bytes, gana tamaño no válido, y con además la raíz del árbol dañada, gana su clase (uso
    incorrecto o árbol no encontrado) sobre el daño; y en todos los casos la transacción sigue utilizable (R8).
12. **AC-R12** — Dada una escritura que quita intervalos con fusiones de páginas que cambian la altura del árbol,
    cuando se provoca una caída del proceso y simulada de corriente en cada punto en que su confirmación escribe o
    lleva datos al disco, entonces al reabrir el árbol tiene todas sus entradas de antes o todas las de después y el
    número de entradas coincide; y cuando se simula un fallo de T11 en cada punto en que Quitar intervalo reserva, lee
    o libera una página, entonces confirmar falla con la clase del primer fallo y no aplica nada, y tras abortar el
    árbol y su número de entradas están como antes (R7, G1, G5, AC-B18).
13. **AC-R13** — Dado un árbol con una página hoja dañada, y otro con una página intermedia dañada, cuando una
    lectura los recorre por los dos extremos hasta necesitar esa página, entonces falla con la clase de daño y el
    número de la página, las entradas devueltas antes son las correctas, después no devuelve más entradas por ningún
    extremo, la transacción sigue utilizable y un recorrido nuevo que la necesita falla igual; un recorrido y un
    Quitar intervalo cuyas claves están en otra parte del árbol, ni en esa página ni en sus vecinas ni en el camino
    hacia ellas, no fallan; cuando una escritura recorre o quita un intervalo que la necesita y falla con daño,
    entonces confirmarla falla con la clase de daño y no aplica nada; y cuando se simula un fallo de memoria agotada
    o de entrada y salida a mitad de un recorrido, entonces falla con esa clase y termina igual (R8, A9, T11).
14. **AC-R14** — Dado un almacén creado y llenado con la versión de 5.1, conservado como dato de prueba, cuando se
    abre con la de 5.2, entonces su versión de formato es `6` y se recorre sin error; y cuando se le quitan
    intervalos con fusiones que cambian la altura de un árbol, se confirma y se reabre, entonces un recorrido entero
    y la lectura de cada clave que queda no fallan con daño; y cuando ese almacén se abre con la versión de 5.1,
    entonces la lista de árboles, cada clave que queda y el número de entradas se leen sin error, y guardar y quitar
    entradas hasta provocar divisiones y fusiones, confirmar y reabrir funcionan sin error. La parte de la versión de
    5.1 es una comprobación de la ejecución, con esa versión compilada aparte y su resultado en el informe de cierre,
    no una prueba que se guarda en el repositorio (R10).
15. **AC-R15** — Dada la medición de R11, cuando se revisa el informe de cierre, entonces contiene sus tiempos y los
    datos del equipo en Windows y en Linux (R11).

## Anotaciones a otros PRD

Este PRD precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Módulo de árboles, B5: las claves límite de un recorrido se validan como claves, de 1 a 2.048 bytes, y el prefijo
  es de 0 a 2.048 bytes (R5).
- Módulo de árboles, B7: el orden de los argumentos fuera de tamaño sigue con límite inferior, límite superior y
  prefijo (R5).
- Módulo de árboles, B8: tamaño no válido puede informar **prefijo** como lo que incumple su tamaño, con mínimo `0` y
  máximo `2048` (R5).
- Árboles y operaciones por clave, A7: la precedencia vale para los recorridos y Quitar intervalo, con uso incorrecto
  antes que árbol no encontrado (R8).
- Árboles y operaciones por clave, A9: un recorrido falla con daño en las páginas que necesita, como Leer, y Quitar
  intervalo en esas y en las vecinas que lee para reorganizar el árbol; una página que no necesitan nunca los hace
  fallar (R8).
- Árboles y operaciones por clave, A10: Quitar intervalo termina las referencias, y es un cambio para T10 solo si
  quita alguna entrada; pedir un recorrido y sacar entradas no las termina (R6, R7).

## Referencias

- Padre: [trees.prd.md](trees.prd.md) v1.0; raíz: [krummi.prd.md](krummi.prd.md) v1.1
- Hermano: [trees-keys.prd.md](trees-keys.prd.md) v1.0
- Relacionados: [transactions.prd.md](transactions.prd.md), [free-space.prd.md](free-space.prd.md)
- Rust, `BTreeMap::range`: <https://doc.rust-lang.org/std/collections/struct.BTreeMap.html#method.range>
- Voron, árboles: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/BTrees>

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 65ee1db9 | 2026-10-26 06:27 UTC+00:00 | 45 min | 1 min | 44 min | 18 min | 26 min | 9 |

<!-- prd-time: [{"session": "65ee1db9-fa7b-4f9e-a6a0-63629e012fb8", "start": "2026-10-26T06:27:50.067000+00:00", "wall": 2686, "discarded": 42, "agent": 1087, "human": 1556, "interventions": 9, "idle_minutes": 10}] -->
