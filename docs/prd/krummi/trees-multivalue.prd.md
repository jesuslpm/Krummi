# Árboles multivalor de Krummi — PRD

**Versión**: 1.0
**Fecha**: 2026-10-29
**Documento padre**: [Árboles B+ de Krummi — PRD](trees.prd.md) (v1.0)

## Objetivo y contexto

Construir el PRD hijo **5.4** del [reparto del módulo de árboles](trees.prd.md#reparto-en-prd-hijos): árboles
multivalor, cuyas claves tienen un conjunto ordenado de valores distintos (B3), con operaciones para añadir, quitar y
comprobar valores, contar los valores de una clave y recorrer las claves y los valores de una clave.

Al cerrar este cambio, una aplicación puede guardar en un árbol, para cada clave, un conjunto de valores, como hace
un índice secundario: la clave es el valor indexado y sus valores, las claves de los registros que lo tienen. Como en
Voron, los valores de una clave con pocos valores viajan juntos dentro de su entrada, y los de una clave con muchos
pasan a un árbol propio de esa clave (M2), de modo que las dos situaciones son eficientes.

**Por qué ahora**: es el cuarto submódulo del reparto. Los índices de las tablas (módulo 7) guardan, para cada valor
indexado, el conjunto de registros que lo tienen.

Este PRD hereda todo lo de su ascendencia, el [módulo de árboles](trees.prd.md) (B1–B12) y la
[raíz](krummi.prd.md), y lo de los módulos anteriores que el padre hereda: el [Pager](pager.prd.md) (P1–P17), las
[Transacciones](transactions.prd.md) (T1–T18), el [Diario](journal.prd.md) (J1–J17), la
[escritura agrupada y directa del diario](journal-group-commit.prd.md) (E1–E16) y el
[Espacio libre](free-space.prd.md) (F1–F14). Reutiliza además lo que fijaron sus hermanos
[Árboles y operaciones por clave](trees-keys.prd.md) (A1–A15 y sus anotaciones),
[Recorrido en orden y búsqueda por prefijo](trees-scan.prd.md) (R1–R11) y
[Valores grandes](trees-large-values.prd.md) (V1–V11). Aquí solo se añade lo propio de 5.4. Lo que cambia o precisa
reglas anteriores está declarado en sus decisiones y anotado en los documentos afectados (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

En este documento, los **valores de una clave** son el conjunto de valores que tiene una clave de un árbol
multivalor; y un **subárbol** es el árbol propio en el que se guardan los valores de una clave cuando superan el
umbral de subida (M2).

### Decisiones de producto

| ID | Decisión |
|---|---|
| M1 | **Valores** (B6): cada valor de un árbol multivalor es una secuencia de 0 a 2.048 bytes, el mismo máximo que una clave (B5); el valor vacío es válido y, por el orden de B5, va antes que cualquier otro. Los valores de una clave son distintos entre sí y se ordenan con el orden de B5. Todo valor que recibe una operación se valida igual, también el que se pasa para buscarlo o quitarlo y el que hace de límite de un recorrido de valores (M7). Un valor fuera de ese tamaño falla con tamaño no válido, como valor, con mínimo `0` y máximo `2048` (B7, B8). En los árboles multivalor no hay valores fuera de su hoja (5.3). |
| M2 | **Dos representaciones con dos umbrales**: mientras los valores de una clave no superan el **umbral de subida**, se guardan juntos dentro de su entrada; cuando un Añadir lo supera, pasan a un subárbol de esa clave. Vuelven a la entrada solo cuando, al quitar valores, quedan por debajo del **umbral de bajada**, que es estrictamente menor que el de subida; entre los dos umbrales, la clave conserva la representación que tenga. Así, añadir y quitar valores alternativamente cerca de un umbral no hace pasar la clave de una representación a otra en cada operación. El paso ocurre en la misma operación que cruza el umbral, también cuando Quitar intervalo de valores (M8) quita de una vez valores desde por encima del de subida hasta por debajo del de bajada. El paso no es visible en los resultados de las operaciones: sus valores devueltos, sus errores de validación y el orden de los valores son los mismos en las dos representaciones; solo se nota en el espacio ocupado (M10) y en las páginas que una operación necesita (M11). Los subárboles no son árboles del catálogo: no tienen nombre, no aparecen en Listar, no cuentan como árboles para Existe ni para B11, no tienen identidad propia (A8) y se liberan con su clave (M10). |
| M3 | **Umbrales** (precisa *Dejado a la implementación* del padre): los dos umbrales de M2 quedan dejados a la implementación, pero su elección se apoya en mediciones. La ejecución mide al menos tres umbrales de subida candidatos: uno bajo, que pasa pronto a un subárbol; uno alto, que mantiene los valores en la entrada mientras sea posible; y uno intermedio; y, para cada uno, al menos dos umbrales de bajada candidatos: uno cercano al de subida y otro en torno a su mitad. Corre todas las cargas de la medición (M15) con cada una de esas parejas, al menos seis, y elige solo con esos datos, por rendimiento, espacio ocupado y complejidad que añade; si los datos no separan claramente a los candidatos, elige la pareja más simple y lo dice. La evaluación, con los resultados de cada pareja, la elegida y por qué, va en el informe de cierre. |
| M4 | **Apertura por tipo** (precisa A1 y B3): un árbol multivalor se abre con los mismos modos de A1, y cada tipo se abre con su propia apertura, que solo tiene las operaciones de su tipo. Usar una operación de un tipo con la apertura de un árbol de otro tipo no compila: Guardar, Añadir, Leer y Quitar de 5.1, y los recorridos y Quitar intervalo de 5.2, con la apertura de un árbol multivalor; y las operaciones de este PRD con la de un árbol normal. El número de entradas (A5) está en las aperturas de los dos tipos. El tipo pedido al abrir se sigue comprobando contra el catálogo (B3). Las operaciones del catálogo (Existe, Borrar, Renombrar y Listar, A3) valen para los dos tipos; Listar informa el tipo multivalor. Las reglas de las aperturas de B1, B9, A6, A7, A8 y de la excepción a B1 anotada en el padre (uso incorrecto con una apertura de otra transacción) valen igual para las de un árbol multivalor. |
| M5 | **Operaciones por valor** (como un diccionario de `SortedSet` de .NET): **Añadir** (clave, valor) añade el valor a los valores de la clave, creando la clave si no está, y devuelve si lo añadió; si el valor ya estaba, no cambia nada, como `HashSet.Add`. **Quitar valor** (clave, valor) quita el valor y devuelve si estaba. **Contiene** (clave, valor) dice si la clave tiene ese valor. Ninguna falla por la presencia o la ausencia de la clave o del valor, pero todas validan antes sus tamaños (B7): la clave, y después el valor. Contiene está en las dos clases de transacción; Añadir y Quitar valor, solo en las de escritura. |
| M6 | **Claves** (precisa A4 y A5): una clave existe mientras tiene al menos un valor. Nace con su primer Añadir y desaparece cuando pierde su último valor, por Quitar valor o por Quitar intervalo de valores (M8); no hay claves sin valores, así que saber si una clave existe es saber si su número de valores no es `0`. **Quitar clave** (clave) quita la clave con todos sus valores y devuelve cuántos valores quitó, o `0` si no estaba; está solo en las de escritura. El **número de entradas** del árbol (A5) es su número de claves. El **número de valores** de una clave (clave), en las dos clases de transacción, devuelve cuántos valores tiene según lo que ve su transacción, sin leer sus valores, o `0` si la clave no está; valida el tamaño de la clave. No hay un total de valores del árbol. |
| M7 | **Recorridos** (como R1–R6, R8 y R9): hay dos. El **recorrido de claves** recorre las claves del árbol con las reglas de R1 a R6, R8 y R9, y devuelve cada clave con su número de valores, no con sus valores; sus límites son claves y se validan como en R5. El **recorrido de valores** (clave, límites, prefijo) recorre los valores de una clave con esas mismas reglas aplicadas a los valores: dos límites, un prefijo y los dos extremos; con una clave que no está, no devuelve nada y no falla. Sus límites son valores y se validan como un valor (M1), de 0 a 2.048 bytes: el valor vacío es un límite válido, incluido o excluido; el prefijo es de 0 a 2.048 bytes, como en R5. Su orden de validación es la clave, el límite inferior, el límite superior y el prefijo. Un intervalo vacío por sus límites o su prefijo (R3) se decide antes de leer nada, también la entrada de la clave. Después de un fallo, el recorrido de valores termina como dice R8. Las claves y los valores que devuelven los dos recorridos son referencias sin copia con la vida de R6. |
| M8 | **Quitar intervalo** (como R7): hay dos, solo en las de escritura. **Quitar intervalo de claves** quita cada clave del intervalo con todos sus valores, con las reglas de R2 a R5, y devuelve cuántas claves quitó, no cuántos valores; el número de entradas baja en esa cantidad. **Quitar intervalo de valores** (clave, límites, prefijo) quita de una clave los valores del intervalo, con las reglas de M7, y devuelve cuántos valores quitó; si la clave se queda sin valores, desaparece (M6). Con una clave que no está, devuelve `0`. |
| M9 | **Cambios y referencias** (como A10, R6 y R7, con las operaciones de este PRD): terminan las referencias y los recorridos anteriores Añadir, Quitar valor, Quitar clave y los dos Quitar intervalo, aunque no cambien nada o fallen. Contiene, el número de valores y pedir un recorrido o sacar elementos de él no los terminan. Para T10 son cambios: Añadir cuando añade, Quitar valor cuando quita, y Quitar clave y los Quitar intervalo cuando quitan algún valor o alguna clave; no lo son esas mismas operaciones cuando no cambian nada ni las que fallan por un error de validación pública. Vaciar un árbol multivalor es un cambio, como dice A2. |
| M10 | **Espacio** (F8): los valores de una clave en un subárbol siguen las reglas de espacio de un árbol normal: las páginas que el subárbol deja de usar al quitar valores vuelven al espacio libre (F8). Todas las páginas de un subárbol vuelven al espacio libre en la misma escritura en que la clave vuelve a su entrada, desaparece o se quita, y al vaciar el árbol (A2), al borrarlo y al sobrescribirlo con Renombrar (A3), y quedan disponibles según F6. Un árbol multivalor vaciado ocupa lo mismo que uno recién creado vacío (A2), y uno que se queda sin claves, por cualquier combinación de las operaciones de este PRD, lo mismo que uno vaciado, también si tenía subárboles. Fuera de esos casos, el espacio que ocupa un árbol depende también de su historia, porque una clave entre los dos umbrales conserva su representación (M2): dos árboles con las mismas claves y los mismos valores pueden ocupar distinto. |
| M11 | **Daño** (G6, amplía A9 y R8): Añadir, Quitar valor, Contiene, el recorrido de valores y Quitar intervalo de valores fallan con la clase de daño y el número de la página cuando necesitan una página dañada del árbol o del subárbol de su clave, también cuando el resultado habría sido no añadir o no quitar nada. El número de valores de una clave y el recorrido de claves nunca leen los subárboles, así que una página dañada de un subárbol no los hace fallar; sí fallan con daño en las páginas del árbol que necesitan, como Leer (A9) y R8. Una página que una operación no necesita no la hace fallar, y las demás claves no se ven afectadas. Las operaciones que liberan un subárbol entero leen las páginas que necesitan para encontrar las suyas, y pueden fallar con daño en cualquier página del subárbol, como Borrar en A9: Quitar clave, Quitar intervalo de claves, vaciar el árbol (A2), borrarlo y sobrescribirlo con Renombrar (A3), y Quitar valor o Quitar intervalo de valores cuando la clave vuelve a su entrada o desaparece. A diferencia de un valor fuera de su hoja (V7), una clave con su subárbol dañado puede no poderse quitar. Una página dañada falla igual en cada operación posterior que la necesite (F9). En una escritura, todo fallo por daño sigue T11. |
| M12 | **Errores**: no hay clases nuevas. La precedencia es la de A7 y R8, con los tamaños en el orden de M5 y M7: primero el tamaño no válido; después, una apertura de otra transacción (uso incorrecto) y una apertura de un árbol que la misma escritura borró (árbol no encontrado), las dos sin leer ninguna página; y después, los fallos al leer las páginas del árbol y de sus subárboles. El tipo de árbol distinto lleva los datos de B8. |
| M13 | **Límites** (amplía B11): tampoco hay un máximo artificial de valores por clave; valen los límites físicos y de representación de B11. |
| M14 | **Formato `8`**: el cambio es incompatible (B12): un motor de formato `7` no conoce el tipo multivalor y toma la entrada de un árbol multivalor en el catálogo por dañada, así que leería mal un almacén sano: Listar, y abrir, borrar o renombrar sobre ese árbol, fallarían con la clase de daño. La versión de formato vigente pasa a ser `8`, y un almacén de formato `7` es de formato anterior aunque esté vacío; antes de la 1.0 no hay migración y los datos se recrean (D7). Los números que el formato guarda para un árbol multivalor, como el número de valores de una clave y la página de su subárbol, se guardan en little endian, como el resto del formato (V3). |
| M15 | **Medición** (B12, sin umbral por D5): el cambio incluye una medición reproducible, en Windows y en Linux, en un almacén en disco con confirmación durable, con claves y valores aleatorios de 16 bytes, con semilla fija. **Cargas por tamaño de conjunto**: siempre 1 millón de valores en total, repartidos en claves de N valores cada una, para N = 1, 10, 100, 1.000, 10.000, 100.000 y 1.000.000 (de 1 millón de claves con un valor a una sola clave con un millón). **Carga sesgada**: 1 millón de valores, cada uno asignado a una clave elegida al azar entre 100.000 con una distribución de Zipf de exponente 1, de modo que unas pocas claves acaparan muchos valores y la mayoría tiene pocos. En cada una de estas cargas, en un almacén nuevo, mide por separado: añadir todos los valores en orden aleatorio, intercalando claves, en escrituras de 1.000 valores; el tamaño del almacén en bytes que da su información (P12) tras la última confirmación de esa fase; tras cerrarlo y reabrirlo, sin vaciar la caché del sistema, comprobar con Contiene cada valor al azar en una sola transacción de lectura; recorrer las claves de todo el árbol; recorrer los valores de cada clave; y quitar todos los valores al azar con Quitar valor, en escrituras de 1.000. **Carga oscilante**: en un almacén nuevo, una clave con los valores justos para quedar en el umbral de subida, de modo que el siguiente Añadir lo supere; se alternan 100.000 veces, empezando por Añadir, Añadir de un valor nuevo y Quitar valor de ese mismo valor, en escrituras de 1.000 operaciones; mide el tiempo y el tamaño del almacén tras la última confirmación. Cada carga se ejecuta con cada pareja de umbrales candidata de M3. El informe incluye el sistema operativo, el procesador y el disco. Sus resultados van en el informe de cierre y alimentan la evaluación de M3. |

*Anotación del 2026-10-30 desde la comprobación de organización recordada de los nodos ([trees-node-check-cache.prd.md](trees-node-check-cache.prd.md)): M11 sigue valiendo, salvo el riesgo residual de la excepción a G6 del padre raíz: una alteración con el almacén abierto que coincide en el código de integridad de un nodo ya comprobado (decisión C7).*

### Estado de partida

Verificado el 2026-10-29 en el código:

| Aspecto | Situación |
|---|---|
| API pública | Los árboles normales tienen las operaciones de 5.1, los recorridos y Quitar intervalo de 5.2 y los valores de hasta 256 MiB de 5.3. Un árbol se abre por su nombre y su tipo, y la apertura es la misma sea cual sea el tipo; las operaciones de un árbol reciben su apertura. |
| Tipos | El único tipo de árbol es el normal. La clase de tipo de árbol distinto existe, pero no se puede provocar porque no hay un segundo tipo. La lista de tipos admite tipos nuevos sin romper el código de las aplicaciones que la usan. Listar devuelve el nombre y el tipo de cada árbol. |
| Catálogo | Registra cada árbol con su nombre, su tipo y su identidad, y no ocupa páginas hasta que se crea el primer árbol (A12). |
| Claves | La clave vacía no es válida (B5), y los árboles del motor no guardan claves vacías. |
| Errores | Dieciséis clases, las del estado de partida de 5.3. Tamaño no válido informa nombre, clave, valor o prefijo. |
| Formato | La versión de formato vigente es `7` (V9). Se conserva como dato de prueba un almacén de formato `6` creado con la versión de 5.1; no se conserva ninguno de formato `7`. |
| Voron (referencia) | Un árbol multivalor guarda los valores de una clave en una página anidada dentro de su entrada y, cuando no caben, los pasa a un árbol propio de la clave; esos árboles no se registran en el catálogo. |

## Actores

- **Desarrollador integrador**: crea y abre árboles multivalor, añade, quita y comprueba valores, cuenta y recorre
  claves y valores, y trata los errores por su clase.
- **Mantenedor**: ejecuta la medición de M15 en Windows y en Linux y revisa la evaluación de M3.

## Flujo

1. La aplicación abre una transacción y un árbol multivalor con su propia apertura (M4).
2. En una escritura añade valores a sus claves y los quita, o quita claves enteras e intervalos (M5, M6, M8).
3. En cualquier transacción comprueba si una clave tiene un valor, cuenta sus valores y recorre las claves del árbol
   o los valores de una clave (M5–M7).
4. La confirmación aplica todos los cambios, o ninguno (G1).

## Validaciones y mensajes

Valen las de los ascendientes y las de 5.1, 5.2 y 5.3, incluidos los fallos de T11 y T12, con la precedencia de
M12. Este PRD añade estas situaciones, solo de la biblioteca (la herramienta no abre almacenes hasta el módulo 12):

| Situación | Regla | Clase de error |
|---|---|---|
| Un valor de más de 2.048 bytes, al añadir, quitar, comprobar o como límite de un recorrido de valores o de Quitar intervalo de valores | M1, M7 | Tamaño no válido, como valor, con mínimo `0` y máximo `2048` |
| Una clave vacía o de más de 2.048 bytes, en cualquier operación de este PRD que recibe una clave, o como límite de un recorrido de claves o de Quitar intervalo de claves | B5, R5 | Tamaño no válido, como clave |
| Un prefijo de más de 2.048 bytes, en un recorrido o Quitar intervalo de claves o de valores | M7, R5 | Tamaño no válido, como prefijo |
| Se abre o se crea como multivalor un árbol normal, o como normal uno multivalor | B3 | Tipo de árbol distinto |
| Se opera con la apertura de un árbol multivalor de otra transacción del mismo almacén | M4, M12 | Uso incorrecto |
| Se opera con la apertura de un árbol multivalor que la misma escritura borró | M4, M12, A6 | Árbol no encontrado |
| Una operación necesita una página dañada del árbol o de un subárbol (M11) | M11, G6 | Daño, con el número de la página |
| Se usa una operación de un tipo con la apertura de un árbol de otro tipo | M4 | Ninguna: no compila |
| Se añade un valor que ya está, se quita un valor o una clave que no está, o se recorren los valores de una clave que no está | M5–M8 | Ninguna: devuelve que no añadió, que no estaba, `0` o nada |

## Salida

El módulo no escribe nada en pantalla. La información del almacén (P12) conserva sus datos; la versión de formato
que informa pasa a ser `8` (M14). Listar informa el tipo multivalor junto al nombre de cada árbol multivalor. Los
valores se devuelven con los bytes guardados, sin cambios, en el orden de B5, con el valor vacío primero si está.

Ejemplo, para un árbol multivalor con la clave `rojo` y los valores vacío, `a` y `b`, y la clave `verde` con el valor
`a`:

| Operación | Resultado |
|---|---|
| Recorrido de claves completo, hacia delante | `rojo` con `3`; `verde` con `1` |
| Recorrido de valores de `rojo`, hacia delante | vacío, `a`, `b` |
| Recorrido de valores de `rojo`, hacia atrás | `b`, `a`, vacío |
| Recorrido de valores de `rojo`, desde `a` excluido | `b` |
| Número de valores de `azul` | `0` |
| Añadir `rojo`, `a` | no añadió |
| Quitar valor `verde`, `a` | estaba; `verde` desaparece y el número de entradas pasa a `1` |
| Quitar clave `rojo` | `3` |

Ejemplo de los datos de un error de tamaño no válido, por un valor de 2.049 bytes en un almacén en disco:

| Dato | Valor |
|---|---|
| Carpeta | la del almacén |
| Qué incumple su tamaño | valor |
| Tamaño recibido | `2049` |
| Mínimo | `0` |
| Máximo | `2048` |

## Reutilización

- Las reglas comunes del módulo de árboles (B1–B12) y las de 5.1, 5.2 y 5.3: modos de apertura, catálogo, errores,
  precedencia, daño, identidad, cambios, recorridos con sus límites, prefijo y extremos, y Quitar intervalo.
- Los árboles normales del motor, para los subárboles (M2).
- El espacio libre (F4–F8) para las páginas que se liberan (M10).
- Las transacciones, su instantánea y las lecturas sin copia (T2, T14, T16, D11).
- El diario y la recuperación (G1, G5): los cambios de los árboles multivalor viajan como cualquier otro cambio.
- La comprobación de integridad y la clase de daño (G6).
- La simulación de caídas, cortes de corriente y fallos de las pruebas de los módulos 3 a 5.3.
- El almacén de formato `6` conservado como dato de prueba, que sigue rechazándose como formato anterior, y uno de
  formato `7` que la ejecución crea con la versión de 5.3 y conserva igual (AC-M18).

## Fuera de alcance

- Valores repetidos dentro de una clave (multiconjuntos) y comparadores de valores propios de la aplicación (B5).
- Valores de más de 2.048 bytes en los árboles multivalor, y valores fuera de su hoja en ellos.
- Leer un único valor de una clave con una operación propia, como el primero o el último (lo da un recorrido de
  valores con un solo paso por un extremo), una operación propia para saber si una clave existe (lo da su número de
  valores, M6), y sustituir de una vez todos los valores de una clave.
- Recorridos que devuelven a la vez varias claves con sus valores, como pares clave–valor, y recorridos con límites de
  clave y de valor a la vez.
- Un total de valores del árbol, y contar las claves o los valores de un intervalo sin recorrerlo.
- Añadir o quitar varios valores, o valores de varias claves, en una sola operación.
- Operaciones de conjuntos sobre los valores (unión, intersección, inclusión), y copiar o mover los valores de una
  clave a otra clave o a otro árbol.
- Convertir un árbol normal en multivalor o al revés (B3).
- Elegir o configurar los umbrales desde la aplicación, elegir la representación de los valores de una clave o saber
  cuál tiene, y estadísticas de los subárboles (informe de almacenamiento, módulo 12).
- Reorganizar las claves existentes si una versión futura cambia los umbrales.
- Reparar un subárbol dañado para poder quitar su clave (M11; herramienta de línea de comandos, módulo 12).
- Árboles de tamaño fijo y compresión de páginas hoja (5.5 y 5.6); si los árboles multivalor se pueden comprimir lo
  decide 5.6.
- Migrar un almacén de formato `7` (D7).

## Dejado a la implementación

- Los dos umbrales de M2, con la evaluación que exige M3, y su unidad (bytes, número de valores o páginas), elegida de
  modo que los valores de una clave que están en su entrada siempre quepan en ella.
- Cómo se representan los valores de una clave dentro de su entrada y en su subárbol, dónde se guarda su número de
  valores, siempre que cumpla M11, y cómo se guarda el valor vacío en cada representación. Preferencia del usuario:
  en el subárbol, una marca en la entrada de la clave que dice que tiene el valor vacío, de modo que el subárbol no
  guarde claves vacías; en la entrada, el valor vacío como un valor más.
- Qué página se informa en un error de daño cuando una operación necesita varias dañadas (M11), siempre que sea una
  de las que necesita, también al repetir la operación; y qué páginas de un subárbol lee una operación que lo libera
  entero, más allá de las que necesita para encontrar las suyas.
- Qué fallo de T11 se informa cuando una misma operación provocaría varios, según lo que intente primero, y qué ve la
  propia escritura después de ese fallo, que ya no puede confirmarse.
- Los nombres de las aperturas y de las operaciones nuevas (padre, *Dejado a la implementación*), y cómo se pide cada
  tipo al abrir, siempre que cumpla M4.
- La orden que ejecuta la medición de M15 y la forma de su informe.

## Criterios de aceptación

Valen las condiciones de prueba de los módulos anteriores y la cabecera de los criterios del padre: cada criterio va
en un almacén en disco y en modo temporal, salvo las partes que reabren el almacén o dañan sus archivos, que son
solo en disco. Este PRD comprueba los criterios del padre asignados a 5.4: AC-B7 en su parte multivalor (AC-M1),
AC-B15 (AC-M2), AC-B4 en su parte de tipo de árbol distinto, AC-B6 en su segunda parte y AC-B8 con normal y
multivalor (AC-M10), y AC-B17 y AC-B18 con árboles multivalor (AC-M13 y AC-M14); repite AC10 y AC20 de la raíz con
árboles multivalor (AC-M15 y AC-M16), y añade estos. Las pruebas que dependen de los umbrales, de la representación
de los valores de una clave, de páginas dañadas o de fallos en puntos concretos usan el conocimiento de la propia
implementación.

1. **AC-M1** — Dado un árbol multivalor vacío, cuando se añaden a claves de 1 y de 2.048 bytes, en orden aleatorio,
   valores vacíos, de 1 byte y de 2.048 bytes, con bytes de `0x00` a `0xFF` y valores que son prefijo de otros,
   entonces cada Añadir devuelve que añadió la primera vez y que no añadió la segunda, sin cambiar nada; Contiene los
   encuentra y no encuentra los que no se añadieron; el número de valores de cada clave coincide con los añadidos,
   y es `0` para una clave que no está; el número de entradas es el número de claves; y el recorrido de valores de
   cada clave los devuelve ordenados byte a byte sin signo, con el vacío primero y cada valor que es prefijo de otro
   antes que él, en la misma escritura, en una lectura posterior y, en disco, tras reabrir (M1, M5, M6, AC-B7).
2. **AC-M2** — Dado un árbol multivalor, cuando se añade, se quita, se comprueba, o se usa como límite de un
   recorrido de valores o de Quitar intervalo de valores, un valor de 2.049 bytes, entonces falla sin efecto con la
   clase de tamaño no válido, como valor, con el tamaño recibido `2049`, mínimo `0` y máximo `2048`, y la transacción
   sigue utilizable; cuando cada operación de este PRD que recibe una clave la recibe vacía o de 2.049 bytes,
   también el número de valores y Quitar clave, entonces falla igual como clave, con mínimo `1` y máximo `2048`; con
   una clave y un valor de 2.049 bytes, el error es de la clave; con un límite inferior y un prefijo fuera de tamaño,
   del límite inferior; y con un prefijo de 2.049 bytes, del prefijo, con mínimo `0` y máximo `2048` (M1, M6, M7,
   M12, AC-B15).
3. **AC-M3** — Dada una clave con valores, cuando se le quitan uno a uno con Quitar valor, entonces cada uno devuelve
   que estaba la primera vez y que no estaba la segunda; al quitar el último, la clave desaparece: Contiene, el número
   de valores (`0`), el recorrido de claves y el número de entradas lo reflejan; y cuando se le vuelve a añadir un
   valor, la clave reaparece con un solo valor (M5, M6).
4. **AC-M4** — Dadas una clave con un valor, otra con varios valores en su entrada y otra con valores en un subárbol,
   cuando se quita cada una con Quitar clave, entonces devuelve su número de valores (`1` para la primera) y la clave
   desaparece; y con una clave que no está, devuelve `0` sin fallar (M6, M2).
5. **AC-M5** — Dada una clave, con los dos umbrales que la prueba toma de la implementación, cuando se le añaden
   valores, uno a uno, hasta pasar el umbral de subida, y después se le quitan hasta quedar por debajo del de
   bajada, con el valor vacío entre ellos y sin él, en escrituras separadas y en la misma escritura, entonces tras
   cada paso Contiene, el número de valores y el recorrido de valores, hacia delante y hacia atrás, coinciden con los
   valores añadidos y no quitados, en la misma escritura, en una lectura posterior y, en disco, tras reabrir; los
   valores pasan al subárbol justo al superar el umbral de subida, siguen en él mientras están entre los dos umbrales
   y vuelven a la entrada justo al quedar por debajo del de bajada; con Quitar intervalo de valores que salta de una
   vez de por encima del de subida a por debajo del de bajada, vuelven a la entrada en esa operación; y el valor vacío
   conserva su lugar al pasar de una representación a la otra (M2, M8, M1).
6. **AC-M6** — Dado un árbol multivalor con claves con valores en su entrada y en un subárbol, cuando se recorren sus
   claves y los valores de cada clave con límites incluidos, excluidos y abiertos, con el valor vacío como límite
   incluido y excluido, con prefijos (también vacío y acabado en bytes `0xFF`), y alternando los dos extremos,
   entonces cada recorrido devuelve exactamente lo que dicen R1 a R4 aplicados a las claves o a los valores, cada
   clave con su número de valores; el recorrido de valores de una clave que no está, y el de claves de un árbol vacío
   o que se quedó sin claves, no devuelven nada; con límites invertidos, el intervalo está vacío, también con la
   página de la entrada de la clave dañada (en disco); y cuando una escritura modifica el árbol a través de otra
   apertura, seguir usando un recorrido anterior no compila (M7, R1–R4, R6).
7. **AC-M7** — Dado un árbol multivalor, cuando se quitan intervalos de valores de una clave en su entrada y en un
   subárbol, también todos los de la clave, entonces cada Quitar intervalo de valores devuelve cuántos quitó, y la
   clave que se queda sin valores desaparece; y cuando se quitan intervalos de claves, también con claves con
   subárbol, entonces devuelve cuántas claves quitó, el número de entradas baja en esa cantidad y un recorrido de
   claves devuelve las demás con su número de valores (M8, M6).
8. **AC-M8** — Dado un almacén con un árbol multivalor ya creado, que se mantiene dentro de una sola región del
   espacio libre, tomando las páginas en uso como en AC-A15: cuando se crea un árbol multivalor vacío, se confirma,
   se toma el valor, se le añaden claves con valores en su entrada y en subárboles y se confirma, y después se le
   quitan todas las claves con cada una de estas formas en pruebas separadas (Quitar valor, Quitar clave, Quitar
   intervalo de valores, Quitar intervalo de claves y vaciarlo), entonces el valor final es el tomado con el árbol
   vacío; cuando a una clave con subárbol se le quitan valores hasta volver a su entrada y se confirma, entonces las
   páginas en uso son las mismas que con un árbol al que se le añadieron esos mismos valores sin que la clave llegara
   nunca al subárbol (las pruebas lo preparan sabiendo la implementación); y cuando el árbol se borra o se
   sobrescribe con Renombrar, el valor final es el de antes de crearlo cuando su entrada no hizo crecer el catálogo,
   como en AC-A15 (M10, A2, A3).
9. **AC-M9** — Dado el código de una aplicación de prueba, cuando usa Guardar, Añadir, Leer o Quitar de 5.1, un
   recorrido o Quitar intervalo de 5.2 con la apertura de un árbol multivalor, o una operación de este PRD con la de
   un árbol normal, o Añadir, Quitar valor, Quitar clave o Quitar intervalo en una transacción de lectura, o usa una
   referencia o un recorrido después de una de las operaciones de M9 que falló sin efecto, o pasa a otro hilo la
   apertura de un árbol multivalor de una escritura o un recorrido suyo, entonces no compila; y pasar a otro hilo la
   de una lectura y sus recorridos compila; y Contiene, el número de valores y los recorridos no terminan las
   referencias ni los recorridos anteriores (M4, M9, B1, R9, AC-B11).
10. **AC-M10** — Dado un árbol normal y otro multivalor, cuando se abren o se crean con su nombre como el otro tipo,
    en cada modo de A1, entonces falla con la clase de tipo de árbol distinto, con el nombre, el tipo que tiene y el
    tipo pedido, y el árbol queda intacto; falla igual con una página propia del árbol dañada y el catálogo intacto;
    y borrarlo sin pedir tipo nunca falla con esa clase; dada una escritura con cambios hechos antes y después de una
    operación que falla por tipo de árbol distinto, cuando se confirma, entonces termina sin error y aplica todos
    esos cambios; dada una transacción de lectura, cuando una apertura falla por tipo de árbol distinto, entonces
    sigue leyendo; dado un almacén que no acepta escrituras (T12), cuando una apertura falla por tipo de árbol
    distinto, entonces falla con esa clase y no con la de entrada y salida, y confirmar esa escritura sin cambios
    termina sin error; dado un árbol normal que se borra, cuando se crea otro con su nombre como multivalor,
    entonces se crea; Listar informa cada árbol con su tipo; y Renombrar con sobrescritura borra el árbol de destino
    sea del tipo que sea, también un multivalor con subárboles (M4, M12, AC-B4, AC-B6, AC-B8, A3).
11. **AC-M11** — Dada una escritura con un árbol multivalor abierto dos veces, y otra transacción del mismo almacén,
    cuando lo borra a través de una apertura y usa la otra con cada operación de este PRD, también tras crear otro
    árbol con el mismo nombre, entonces falla sin efecto con árbol no encontrado y el último nombre del árbol; cuando
    usa la apertura en la otra transacción, entonces falla sin efecto con uso incorrecto; y con además un argumento
    fuera de su tamaño, en los dos casos, el error es de tamaño no válido (M4, M12, A6, A7, AC-B14).
12. **AC-M12** — Dado un árbol multivalor en disco, en pruebas separadas, con la página raíz dañada del subárbol de
    una clave (un subárbol de al menos dos niveles) y con una página dañada en la parte del árbol que lleva a otra
    clave, cuando se añade, se quita o se comprueba un valor, también uno que ya está o que no está, o se recorren
    los valores de esas claves, entonces cada operación que necesita la página falla con la clase de daño y su
    número, y vuelve a fallar igual al repetirla; el recorrido de valores termina tras el fallo, sus valores
    anteriores son correctos y en una lectura un recorrido nuevo vuelve a fallar igual; las demás claves se leen y
    se modifican sin error; el número de valores de la clave con el subárbol dañado y el recorrido de claves no
    fallan; y cuando una escritura quita la clave con el subárbol dañado con Quitar clave, le quita valores hasta
    que vuelve a su entrada, quita un intervalo de claves que la contiene, vacía el árbol, lo borra o lo sobrescribe
    con Renombrar, entonces falla con la clase de daño y el número de una página del subárbol, y la escritura sigue
    T11 (M11, A9, R8, G6, F9).
13. **AC-M13** — Dada una transacción de lectura abierta y una escritura que, con Añadir y Quitar valor, hace pasar
    claves de su entrada a un subárbol y de vuelta, y divide y fusiona páginas del árbol y de los subárboles hasta
    cambiar su altura, cuando la lectura lee todas las claves, sus valores, su número de valores y el número de
    entradas antes y después de que la escritura se confirme, entonces ve exactamente los de su instantánea; y una
    lectura nueva ve los de la escritura (G3, T14, AC-B17).
14. **AC-M14** — Dada una escritura que añade y quita valores pasando claves de una representación a otra y que
    divide y fusiona páginas, cuando se provoca una caída del proceso y simulada de corriente en cada punto en que su
    confirmación escribe o lleva datos al disco, entonces al reabrir el árbol está entero como antes de la escritura
    o entero como después: cada clave con sus valores y su número de valores, y el número de entradas; y cuando se
    simula un fallo de T11 distinto de entrada y salida en cada punto en que una operación reserva, lee o libera
    páginas, también al pasar los valores de una clave a un subárbol y de vuelta, entonces confirmar falla con la
    clase del primer fallo y no aplica nada, y tras abortar la escritura el árbol, sus claves, sus valores y el número
    de entradas están como antes y una escritura nueva funciona sin error (G1, G5, T11, AC-B18).
15. **AC-M15** — Dado un almacén pequeño con un árbol multivalor con claves con valores en su entrada y en un
    subárbol, cuando se altera cada uno de sus bytes por separado, se abre y se leen todos sus valores, entonces cada
    alteración se detecta como daño o no tiene efecto, con las reglas de AC10 de la raíz y sus anotaciones; nunca se
    devuelve un valor con un byte distinto (M11, G6, AC10).
16. **AC-M16** — Dadas las claves y los valores que devuelven el recorrido de claves y el recorrido de valores, de
    claves con valores en su entrada y en un subárbol, cuando se leen, entonces las referencias están dentro de la
    memoria del motor que fija T16 y no en una copia, en disco y en modo temporal, y usar una referencia después de
    terminar la transacción no compila (M7, AC20).
17. **AC-M17** — Dada una escritura en disco que solo hace Añadir de valores que ya están, Quitar valor de valores
    que no están, Quitar clave de claves que no están, Quitar intervalo de claves y de valores que no quitan nada,
    Contiene, el número de valores, recorridos, y operaciones que fallan por tamaño no válido o por tipo de árbol
    distinto, cuando se confirma, entonces termina sin error y los archivos del almacén quedan idénticos byte a byte
    (M9, T10).
18. **AC-M18** — Dado un almacén de formato `7` que la ejecución crea con la versión de 5.3, compilada aparte, y
    conserva como dato de prueba, y el de formato `6` de 5.1, cuando se abren, entonces fallan con la clase de
    formato distinto como formato anterior y no se modifican; un almacén nuevo informa la versión de formato `8`; y
    leyendo en disco los bytes de la entrada de una clave con subárbol, con el conocimiento de la implementación, su
    número de valores y la página de su subárbol están en little endian (M14, AC-B12).
19. **AC-M19** — Dado el informe de cierre, cuando se revisa, entonces contiene la medición de M15 con sus tiempos,
    por fase, y los tamaños del almacén para cada carga y cada pareja de umbrales candidata, y los datos del equipo,
    en Windows y en Linux (M15).
20. **AC-M20** — Dado el informe de cierre, cuando se revisa, entonces contiene la evaluación de M3 con al menos las
    seis parejas de umbrales candidatas que nombra, los datos de la medición de cada una, la complejidad que añade
    cada una, la elegida y por qué, justificada con esos datos (M3).

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Módulo de árboles, B6: en los árboles multivalor, cada valor es de 0 a 2.048 bytes (M1).
- Módulo de árboles, B3: los subárboles de las claves de un árbol multivalor no son árboles del catálogo (M2); cada
  tipo se abre con su propia apertura, y usar una operación de un tipo con un árbol de otro no compila (M4).
- Módulo de árboles, B11: tampoco hay un máximo artificial de valores por clave (M13).
- Módulo de árboles, *Dejado a la implementación*: los umbrales de subida y de bajada de los árboles multivalor
  quedan dejados a la implementación, con la evaluación medida de M3.
- Árboles y operaciones por clave, A1: cada tipo se abre con su propia apertura, con los mismos modos (M4).
- Árboles y operaciones por clave, A5: en un árbol multivalor, el número de entradas es su número de claves (M6).
- Árboles y operaciones por clave, A9; Recorrido en orden, R8: en un árbol multivalor, las operaciones que liberan un
  subárbol entero pueden fallar con daño en cualquier página suya, y Quitar intervalo de claves necesita además las
  páginas de los subárboles que libera (M11).
- Árboles y operaciones por clave, A10; Recorrido en orden, R6 y R7: Añadir, Quitar valor, Quitar clave y los dos
  Quitar intervalo terminan las referencias y los recorridos, y son cambios para T10 solo cuando añaden o quitan algo;
  las operaciones del catálogo y los modos de apertura no cambian, y vaciar sigue siendo siempre un cambio (M9).
- Recorrido en orden, *Fuera de alcance*: los recorridos y Quitar intervalo de los árboles multivalor los fija este
  PRD (M7, M8).
- Valores grandes, *Fuera de alcance*: los árboles multivalor no tienen valores fuera de su hoja; cada valor es de 0
  a 2.048 bytes (M1).
- Padre raíz, D7; Pager, P5; Diario, J3; escritura agrupada, E15; Espacio libre, F11; Transacciones, todo el
  documento; Árboles y operaciones por clave, A13 y su anotación de 5.3; Recorrido en orden, R10, *Salida* y AC-R14;
  Valores grandes, V9, *Salida* y AC-V12: la versión de formato vigente pasa a ser `8`, y un almacén de formato `7`
  es de formato anterior (M14).

## Referencias

- Padre: [trees.prd.md](trees.prd.md) v1.0; raíz: [krummi.prd.md](krummi.prd.md) v1.1
- Hermanos: [trees-keys.prd.md](trees-keys.prd.md) v1.0, [trees-scan.prd.md](trees-scan.prd.md) v1.0,
  [trees-large-values.prd.md](trees-large-values.prd.md) v1.0
- Relacionados: [free-space.prd.md](free-space.prd.md), [transactions.prd.md](transactions.prd.md),
  [pager.prd.md](pager.prd.md)
- Voron, árboles multivalor: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/BTrees>

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| a3b768ea | 2026-10-29 08:33 UTC+00:00 | 1 h 26 min | 10 min | 1 h 16 min | 39 min | 37 min | 16 |

<!-- prd-time: [{"session": "a3b768ea-ddde-4ebc-a01d-ce4fd902416e", "start": "2026-10-29T08:33:25.283000+00:00", "wall": 5150, "discarded": 608, "agent": 2317, "human": 2224, "interventions": 16, "idle_minutes": 10}] -->
