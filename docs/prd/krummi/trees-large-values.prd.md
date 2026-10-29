# Valores grandes de Krummi — PRD

**Versión**: 1.0
**Fecha**: 2026-10-27
**Documento padre**: [Árboles B+ de Krummi — PRD](trees.prd.md) (v1.0)

## Objetivo y contexto

Construir el PRD hijo **5.3** del [reparto del módulo de árboles](trees.prd.md#reparto-en-prd-hijos): valores de
hasta 256 MiB en los árboles normales, que se guardan, se leen, se recorren y se quitan con las mismas operaciones
de 5.1 y 5.2, y que se devuelven sin copia como una sola secuencia de bytes, también en modo temporal.

Al cerrar este cambio, una aplicación puede guardar en una entrada un documento, una imagen o cualquier valor de
hasta 268.435.456 bytes, y no solo de hasta 1.024. Como en Voron y en LMDB, un valor que no cabe en su página hoja se
guarda en páginas contiguas dentro del propio almacén, y Leer devuelve una referencia a esas páginas, sin copiarlas.

**Por qué ahora**: es el tercer submódulo del reparto. Las tablas (módulo 7) guardan sus filas como valores de
árbol, y una fila de varios KiB no cabe en el máximo provisional de 1.024 bytes de 5.1.

**Qué se optimiza**: el diseño prioriza los valores de hasta unos pocos MiB. Los valores mayores, hasta el máximo de
256 MiB, se admiten y se prueban, pero no se optimizan (V10). Los valores de tamaño arbitrario, leídos y escritos por
partes, son los streams (módulo 8).

Este PRD hereda todo lo de su ascendencia, el [módulo de árboles](trees.prd.md) (B1–B12) y la
[raíz](krummi.prd.md), y lo de los módulos anteriores que el padre hereda: el [Pager](pager.prd.md) (P1–P17), las
[Transacciones](transactions.prd.md) (T1–T18), el [Diario](journal.prd.md) (J1–J17), la
[escritura agrupada y directa del diario](journal-group-commit.prd.md) (E1–E16) y el
[Espacio libre](free-space.prd.md) (F1–F14). Reutiliza además lo que fijaron sus hermanos
[Árboles y operaciones por clave](trees-keys.prd.md) (A1–A15 y su anotación) y
[Recorrido en orden y búsqueda por prefijo](trees-scan.prd.md) (R1–R11). Aquí solo se añade lo propio de 5.3. Lo que
cambia o precisa reglas anteriores está declarado en sus decisiones y anotado en los documentos afectados (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

En este documento, un **valor fuera de su hoja** es un valor que no se guarda en la página hoja de su entrada, sino
en páginas propias del almacén; su entrada sigue en la hoja, con su clave.

### Decisiones de producto

| ID | Decisión |
|---|---|
| V1 | **Máximo de un valor** (B6): 268.435.456 bytes (256 MiB), para cualquier clave válida, también una de 2.048 bytes. Sustituye al máximo provisional de 5.1 (A11). Con B8, el máximo que informa un error de tamaño no válido de un valor en un árbol normal pasa a ser `268435456`. Añadir sobre una clave que ya está no cambia nada (A4, A10), así que tampoco reserva sitio para el valor y no falla con memoria agotada, tamaño máximo ni disco lleno por su tamaño. |
| V2 | **Lectura contigua sin copia** (precisa D11 del padre raíz y B10): Leer y los recorridos devuelven todo valor, dentro o fuera de su hoja, como una sola secuencia contigua de bytes, con la forma y la vida de las referencias de hoy. La referencia apunta a memoria del motor según T16, también a una versión que se conserva en el área de versiones de `krummi.temp` (J7), o en una escritura a su propia copia del valor, como hoy; nunca a una copia hecha para esa lectura. Vale en disco y en modo temporal, donde la memoria crece por trozos separados. Los bytes de una referencia no cambian mientras vale, aunque una escritura posterior reutilice las páginas del valor (G3). |
| V3 | **Dónde vive un valor fuera de su hoja**: en páginas contiguas del propio almacén, de `krummi.data` o de la memoria del almacén temporal. El cambio no añade archivos al almacén (P7). Los números que el formato guarda para un valor fuera de su hoja, como su página y su longitud, se guardan en little endian, como el resto del formato, nunca en el orden nativo del procesador, de modo que no dependen de él. |
| V4 | **Umbral de la hoja**: a partir de qué tamaño un valor se guarda fuera de su hoja sigue dejado a la implementación (padre, *Dejado a la implementación*). La ejecución evalúa al menos dos umbrales: uno que permita que una entrada llene sola su hoja y otro que mantenga al menos dos entradas por hoja. Los compara con los datos de la medición (V11), por rendimiento, espacio ocupado y complejidad que añaden, elige el más equilibrado y deja la evaluación y la elección en el informe de cierre. |
| V5 | **Espacio** (F8): un valor fuera de su hoja de n bytes ocupa como mucho una página más que las que llenarían sus bytes, es decir, n entre 8.192 redondeado hacia arriba, más una; ninguna otra página del almacén queda retenida por él. Al sobrescribirlo con otro valor, las páginas que el nuevo no ocupa se liberan (F8) en esa misma escritura y quedan disponibles según F6. Las páginas de un valor fuera de su hoja vuelven al espacio libre cuando el valor deja de estar en el árbol: al sobrescribirlo con Guardar, al quitarlo con Quitar o Quitar intervalo, al vaciar el árbol (A2), al borrarlo y al sobrescribirlo con Renombrar (A3). Un árbol vaciado ocupa lo mismo que uno recién creado vacío (A2), y uno que se queda sin entradas por Quitar intervalo, lo mismo que uno vaciado (R7), también si tenía valores fuera de su hoja. |
| V6 | **Páginas contiguas y crecimiento** (F5): un valor fuera de su hoja necesita sus páginas seguidas dentro de una región; el máximo de V1 siempre cabe en una. Si no hay un hueco libre del tamaño necesario, el valor se guarda donde dice F5, aunque el almacén tenga páginas libres sueltas suficientes: no hay error de espacio fragmentado. El crecimiento y sus fallos siguen F5, P10, P11, P13 y G7. |
| V7 | **Daño en un valor fuera de su hoja** (G6, A9): el motor nunca devuelve un valor con algún byte dañado. Leer, y un recorrido que necesita esa entrada, fallan con la clase de daño y el número de una página del valor, y vuelven a fallar con esa clase cada vez que lo necesitan; en qué paso del recorrido aparece el fallo sigue lo dejado a la implementación en 5.2. Ninguna operación que quita, sobrescribe o libera un valor lee sus páginas: Guardar y Añadir sobre esa clave, Quitar, Quitar intervalo, vaciar el árbol (A2), borrarlo y sobrescribirlo con Renombrar (A3). Un daño en las páginas del valor no las hace fallar, así que la aplicación siempre puede quitar o reemplazar un valor dañado, y vaciar o borrar su árbol; pueden fallar por daño en las demás páginas del árbol, como dice A9. Las demás claves del árbol no se ven afectadas. |
| V8 | **Errores**: no hay clases nuevas. Los fallos de T11 valen en las operaciones y en la confirmación, también memoria agotada (P13) o disco lleno (G7) al confirmar una escritura con valores fuera de su hoja. Leer un valor no reserva memoria, así que no falla con memoria agotada por su tamaño. |
| V9 | **Formato `7`**: el cambio es incompatible (B12): un motor de formato `6` no conoce los valores fuera de su hoja, leería su referencia como si fuera el valor y, al liberar o reorganizar páginas, podría reutilizar las del valor. La versión de formato vigente pasa a ser `7`, y un almacén de formato `6` es de formato anterior aunque esté vacío; antes de la 1.0 no hay migración y los datos se recrean (D7). |
| V10 | **Valores muy grandes**: el máximo de V1 se admite y se prueba (AC-V1), pero no se optimiza. Se acepta para ellos que el valor se escriba dos veces, en el diario y en `krummi.data`, y la memoria que la confirmación necesita para escribirlo en el diario. La medición (V11) no incluye valores de más de 8 MiB. |
| V11 | **Medición** (B12, sin umbral por D5): el cambio incluye una medición reproducible, en Windows y en Linux, en un almacén en disco con confirmación durable, con claves aleatorias de 16 bytes y valores de bytes aleatorios, que no se comprimen, los dos con semilla fija. Para cada tamaño de valor de 1, 2, 3, 4, 6 y 8 KiB, 64 KiB, 1 MiB y 8 MiB, en un almacén nuevo con 256 MiB de valores en total: guardarlos al azar, en escrituras de 16 MiB de valores (16.384 entradas por escritura con 1 KiB, 2 con 8 MiB); tras cerrarlo y reabrirlo, sin vaciar la caché del sistema, leerlos todos al azar en una sola transacción de lectura, leyendo los bytes de cada valor devuelto; y el tamaño del almacén en bytes que da su información (P12) tras la última confirmación de la carga. Además, en otro almacén nuevo, una carga mixta de 1 millón de claves, el 90% con valores de 100 bytes y el 10% con valores de 2 a 8 KiB, con el tamaño elegido al azar con la misma probabilidad para cada número de bytes de ese intervalo y las entradas grandes repartidas al azar entre las demás, en escrituras de 1.000 entradas, en la que mide guardarlas, leerlas todas al azar, recorrer el árbol entero hacia delante y el tamaño del almacén. El informe incluye el sistema operativo, el procesador y el disco. Sus resultados van en el informe de cierre y alimentan la evaluación de V4. |

*Anotaciones del 2026-10-29 desde los árboles multivalor ([trees-multivalue.prd.md](trees-multivalue.prd.md)):*

- *V9, Salida y AC-V12: la versión de formato vigente pasa a ser `8`; un almacén de formato `7` es de formato anterior (decisión M14).*
- *Fuera de alcance: los árboles multivalor no tienen valores fuera de su hoja; cada valor es de 0 a 2.048 bytes (decisión M1).*

### Estado de partida

Verificado el 2026-10-27 en el código:

| Aspecto | Situación |
|---|---|
| API pública | Los árboles normales tienen las operaciones de 5.1 y los recorridos y Quitar intervalo de 5.2. Leer y los recorridos devuelven cada valor como una referencia sin copia a una sola secuencia de bytes, dentro de una página. |
| Valores | Viven siempre en la página hoja de su entrada; el máximo es 1.024 bytes (A11), elegido para que quepan al menos dos entradas del tamaño máximo en cada hoja. No hay valores fuera de su hoja. |
| Páginas | Páginas de 8.192 bytes. Cada página lleva su propia comprobación de integridad y su número en sus primeros 16 bytes, también cada página de un rango reservado de una vez, así que hoy los bytes de un rango de páginas no son contiguos. La integridad de una página se comprueba cada vez que se lee de `krummi.data`. Ningún PRD fija el algoritmo de la comprobación ni que cada página lleve la suya (Pager, *Dejado a la implementación*). |
| Memoria en disco | `krummi.data` se proyecta entero en memoria; al crecer, se proyecta otra vez completo y las proyecciones anteriores se conservan hasta cerrar, de modo que las referencias entregadas siguen valiendo. Un rango de páginas es contiguo en la proyección más reciente. |
| Memoria en modo temporal | Crece por trozos separados que nunca se mueven, cada uno del tamaño del crecimiento. Una página nunca queda partida entre dos trozos, pero un rango de páginas sí puede quedarlo. |
| Escrituras y versiones | Una escritura guarda en memoria su copia de cada página que modifica o reserva, página a página. Las versiones confirmadas que todavía no están en `krummi.data` se conservan página a página en `krummi.temp` (en disco) o en memoria (en modo temporal). Ninguna de las dos garantiza que las páginas de un rango queden contiguas. |
| Diario | Un registro por confirmación, con las páginas cambiadas, comprimido cuando ocupa menos; no tiene un tamaño máximo por confirmación. |
| Espacio libre | Un rango reservado de una vez tiene como mucho 65.407 páginas, unos 511 MiB, y nunca cruza de una región a otra (F5). |
| Errores | Dieciséis clases, las del estado de partida de 5.2. Tamaño no válido informa nombre, clave, valor o prefijo. |
| Formato | La versión de formato vigente es `6` (A13). Todos los números del motor en disco (números de página, longitudes, comprobaciones de integridad) se guardan en little endian explícito, no en el orden nativo del procesador; ningún PRD lo fija como regla. Hay un almacén de formato `6` creado con la versión de 5.1 y conservado como dato de prueba, que 5.2 abre y recorre (AC-R14). |

## Actores

- **Desarrollador integrador**: guarda, lee, recorre y quita entradas con valores de hasta 256 MiB, y trata los
  errores por su clase.
- **Mantenedor**: ejecuta la medición de V11 en Windows y en Linux y revisa la evaluación de V4.

## Flujo

1. La aplicación abre una transacción y un árbol normal (A1).
2. En una escritura guarda o añade entradas con valores de cualquier tamaño hasta el máximo (V1).
3. En cualquier transacción los lee o los recorre, y recibe cada valor como una sola secuencia de bytes sin copia
   (V2).
4. En una escritura los sobrescribe o los quita, y sus páginas vuelven al espacio libre (V5).
5. La confirmación aplica todos los cambios, o ninguno (G1).

## Validaciones y mensajes

Valen las de los ascendientes y las de 5.1 y 5.2, incluidos los fallos de T11 y T12, con la precedencia de A7 y R8.
Este PRD solo cambia el máximo de un valor (V1) y añade una situación de daño, solo de la biblioteca (la herramienta
no abre almacenes hasta el módulo 12):

| Situación | Regla | Clase de error |
|---|---|---|
| Un valor de más de 268.435.456 bytes, al guardar o añadir | V1, B6 | Tamaño no válido, como valor |
| Leer, o un recorrido, necesita una entrada cuyo valor fuera de su hoja tiene una página dañada | V7, G6 | Daño, con el número de una página del valor |

## Salida

El módulo no escribe nada en pantalla. La información del almacén (P12) conserva sus datos; la versión de formato
que informa pasa a ser `7` (V9). Leer y los recorridos devuelven los bytes guardados, sin cambios, en una sola
secuencia, sea cual sea su tamaño.

Ejemplo de los datos de un error de tamaño no válido, por un valor de 268.435.457 bytes en un almacén en disco:

| Dato | Valor |
|---|---|
| Carpeta | la del almacén |
| Qué incumple su tamaño | valor |
| Tamaño recibido | `268435457` |
| Mínimo | `0` |
| Máximo | `268435456` |

## Reutilización

- Las reglas comunes del módulo de árboles (B1–B12) y las de 5.1 y 5.2: aperturas, operaciones por clave,
  recorridos, Quitar intervalo, errores, precedencia, daño, identidad y cambios.
- La reserva de rangos de páginas contiguas del espacio libre y su liberación (F5, F8).
- Las transacciones, su instantánea, las versiones retenidas y las lecturas sin copia (T2, T14, T16, D11).
- El diario y la recuperación (G1, G5): los valores fuera de su hoja viajan como cualquier otro cambio.
- La comprobación de integridad y la clase de daño (G6).
- La simulación de caídas, cortes de corriente y fallos de las pruebas de los módulos 3 a 5.2.
- El almacén de formato `6` conservado como dato de prueba, que ahora debe rechazarse como formato anterior (V9).

## Fuera de alcance

- Guardar los valores muy grandes en archivos aparte del almacén: se evalúa con los streams (módulo 8).
- Optimizar los valores de más de unos pocos MiB (V10): evitar que pasen por el diario, o escribirlos sin que la
  confirmación los tenga enteros en memoria; y límites de latencia para confirmaciones muy grandes (D5).
- Reservar el sitio de un valor y escribirlo después en su lugar, sin pasar sus bytes a Guardar (lo que Voron llama
  `DirectAdd`), y guardar un valor a partir de varios trozos.
- Leer el tamaño de un valor sin leerlo, leer solo una parte de él, o modificar o ampliar una parte de un valor
  guardado.
- Saber si una entrada tiene su valor fuera de su hoja, y estadísticas de los valores de un árbol (informe de
  almacenamiento, módulo 12).
- Comprimir los valores (compresión de valores por diccionario, módulo 7; compresión de páginas hoja, 5.6) y cifrar
  sus páginas (módulo 9, que fija cómo se devuelven sin copia).
- Valores grandes en árboles multivalor y de tamaño fijo (5.4 y 5.5 fijan sus máximos).
- Juntar huecos libres para conseguir sitio contiguo, o mover valores para compactar `krummi.data` (compactación,
  módulo 11).
- Migrar un almacén de formato `6` (D7).
- Inspeccionar o verificar valores desde la herramienta de línea de comandos (módulo 12).

## Dejado a la implementación

- El umbral de la hoja, con la evaluación que exige V4.
- Cómo se representa un valor fuera de su hoja en su entrada y en sus páginas, y cómo se comprueba su integridad,
  siempre que cumpla G6 y V7: si lleva una comprobación por página o una para todo el valor, y si se comprueba en
  cada lectura o una vez por transacción (que otro programa modifique los archivos del almacén abierto queda fuera de
  alcance, P16).
- Qué página del valor se informa en un error de daño (V7), siempre que sea una de las suyas, también cuando varias
  están dañadas o al repetir la operación.
- Cómo se consigue que un valor fuera de su hoja sea contiguo en memoria en modo temporal, en la copia de la
  escritura y en las versiones que se conservan fuera de `krummi.data` (V2).
- Si sobrescribir un valor fuera de su hoja reutiliza parte de sus páginas o reserva otras, con el mismo número de
  páginas o con otro, dentro de V5; cerca del tamaño máximo, una forma puede caber y otra fallar con tamaño máximo,
  porque una escritura no reutiliza las páginas que ella misma libera (F6).
- Qué fallo de T11 se informa cuando una misma operación con un valor grande provocaría varios (memoria agotada,
  tamaño máximo o disco lleno), según lo que intente primero; en todos los casos sigue T11.
- En modo temporal y en las versiones que se conservan fuera de `krummi.data`, cuánta memoria reserva el motor de
  más para que un valor fuera de su hoja sea contiguo. Esa memoria cuenta para el tamaño máximo (P10, T14), así que
  cerca del límite un valor puede fallar con tamaño máximo en un modo y caber en el otro con el mismo máximo. En modo
  temporal, la que forma parte de la memoria del almacén cuenta en su información como páginas, y como libres
  mientras no las use nada (F10).
- La orden que ejecuta la medición de V11 y la forma de su informe.

## Criterios de aceptación

Valen las condiciones de prueba de los módulos anteriores y la cabecera de los criterios del padre: cada criterio va
en un almacén en disco y en modo temporal, salvo las partes que reabren el almacén o dañan sus archivos, que son
solo en disco. Este PRD comprueba el criterio del padre asignado a 5.3 (AC-B3 con 256 MiB, en AC-V1), y AC-B17 y
AC-B18 con valores fuera de su hoja (AC-V6 y AC-V8); repite AC10 y AC20 de la raíz con valores fuera de su hoja
(AC-V11 y AC-V3), y añade estos. Las pruebas que dependen de dónde se guarda un valor, de dónde está su referencia, de
páginas dañadas o de fallos en puntos concretos usan el conocimiento de la propia implementación.

1. **AC-V1** — Dada una escritura, cuando guarda con una clave de 2.048 bytes un valor vacío y otro de 268.435.456
   bytes, entonces se leen idénticos tras confirmar y, en disco, también tras reabrir, y un recorrido del árbol los
   devuelve; cuando el de 268.435.456 bytes se sobrescribe con uno de 1 byte y después se quita, entonces Leer
   devuelve el valor nuevo y después que la clave no está; y cuando intenta guardar o añadir uno de 268.435.457
   bytes, entonces falla con la clase de tamaño no válido sin guardar nada, como valor, con el tamaño recibido,
   mínimo `0` y máximo `268435456`, y la escritura sigue utilizable (V1, AC-B3).
2. **AC-V2** — Dado un árbol, cuando se guardan, con claves de 1 y de 2.048 bytes, valores de 1.024 y 1.025 bytes,
   de 1 MiB y 8 MiB, y de un byte menos, igual y uno más que cada uno de estos límites, que las pruebas toman de la
   implementación: el umbral de la hoja y el mayor valor que cabe en 1, en 2 y en 3 páginas fuera de la hoja, entonces Leer los devuelve idénticos en la misma escritura, en una lectura posterior y,
   en disco, tras reabrir; y cuando se sobrescriben con valores mayores, menores y vacíos, se añaden sobre claves
   existentes y se quitan, entonces Leer devuelve siempre el último valor guardado o que la clave no está, y el
   número de entradas coincide con las claves que Leer encuentra (V1, V2, A4, A5).
3. **AC-V3** — Dado un valor fuera de su hoja, cuando se lee, entonces la referencia está dentro de la memoria del
   motor que fija T16 y no en una copia, en cada uno de estos casos: en disco, una versión ya escrita en su sitio en
   `krummi.data`, dentro de la proyección de `krummi.data`; en disco, una versión confirmada que todavía no se ha
   escrito en `krummi.data`, y una versión anterior que una lectura sigue viendo después de que una escritura
   posterior sobrescribiera el valor, dentro del área de versiones de `krummi.temp` donde el motor la conserva (J7,
   T14, T16); en modo temporal, esos mismos casos dentro de la memoria del almacén o de la memoria donde el motor
   conserva las versiones; en modo temporal, un valor guardado cuando el almacén tiene que crecer y su rango, con
   páginas consecutivas, atravesaría el límite entre la memoria que el almacén ya tenía y la que añade al crecer; y
   en una escritura que lee el valor que acaba de guardar, dentro de su propia copia. En todos los casos el valor
   se devuelve como una sola secuencia contigua, con los bytes idénticos a los guardados, y usar la referencia
   después de terminar la transacción no compila (V2, AC20).
4. **AC-V4** — Dado un árbol con valores dentro y fuera de su hoja, cuando se recorre hacia delante, hacia atrás y
   con prefijo, entonces cada entrada sale con su valor completo e idéntico, como una sola secuencia sin copia; y
   cuando se quitan intervalos que contienen valores fuera de su hoja, entonces Quitar intervalo devuelve cuántas
   entradas quitó y un recorrido del árbol entero devuelve las demás con sus valores (V2, R1, R7).
5. **AC-V5** — Dado un almacén con un árbol ya creado, que se mantiene dentro de una sola región del espacio libre
   (con valores cuyo tamaño no obliga a saltar de región, F5), tomando las páginas en uso como en AC-A15: cuando se
   crea un árbol vacío, se confirma, se toma el valor, se le guardan valores dentro y fuera de su hoja, se
   sobrescriben con valores mayores y menores, se confirma y después se vacía, entonces el valor final es el tomado
   con el árbol vacío; lo mismo cuando se le quitan todas las entradas con Quitar intervalo; cuando el árbol se
   borra o se sobrescribe con Renombrar, el valor final es el de antes de crearlo cuando su entrada no hizo crecer
   el catálogo (las pruebas lo saben por la implementación), como en AC-A15; y cuando se guarda un valor fuera
   de su hoja de 1 MiB, se confirma, se sobrescribe con uno de 64 KiB que sigue fuera de su hoja y se confirma,
   entonces cada vez las páginas en uso crecen, respecto del árbol sin ese valor, como mucho en las páginas que
   llenarían sus bytes, redondeadas hacia arriba, más una, y las que crecen por la entrada de la hoja (V5, A2, A3,
   R7, F10).
6. **AC-V6** — Dada una transacción de lectura abierta con referencias a valores fuera de su hoja, y una escritura
   que sobrescribe y quita esos valores, y divide y fusiona hojas con valores dentro y fuera de ellas hasta cambiar
   la altura del árbol, cuando se confirma y otra escritura reutiliza las páginas liberadas (F6), entonces los bytes
   de las referencias de la lectura no cambian, la lectura sigue leyendo y recorriendo exactamente los valores de su
   instantánea, y una lectura nueva ve los de la escritura (V2, G3, T14, T16, AC-B17).
7. **AC-V7** — Dado un valor fuera de su hoja con un byte alterado en su primera, en una intermedia y en su última
   página, en pruebas separadas, cuando se lee o un recorrido necesita su entrada, entonces falla con la clase de
   daño y el número de una página del valor, y vuelve a fallar igual al repetirlo; las entradas que el recorrido
   devolvió antes son correctas y las demás claves se leen sin error; cuando una escritura guarda sobre esa clave, la
   quita o quita un intervalo que la contiene, entonces no falla, se confirma y Leer devuelve el valor nuevo o que la
   clave no está; cuando añade sobre esa clave, entonces devuelve que no la añadió sin fallar, y Leer sigue fallando
   con la clase de daño; y cuando una escritura vacía el árbol, lo
   borra o lo sobrescribe con Renombrar, entonces no falla y se confirma (V7, A9, G6, R8).
8. **AC-V8** — Dada una escritura que guarda, sobrescribe y quita valores fuera de su hoja, cuando se provoca una
   caída del proceso y simulada de corriente en cada punto en que su confirmación escribe o lleva datos al disco,
   entonces al reabrir cada clave tiene su valor de antes o el de después, todas a la vez, y el número de entradas
   coincide; cuando se simula un fallo de T11 distinto de entrada y salida al escribir en cada punto en que una
   operación reserva, lee o libera páginas, también al reservar el rango de un valor, entonces confirmar falla con la
   clase del primer fallo y no aplica nada, y tras abortar la escritura el árbol y su número de entradas están como
   antes y una escritura nueva funciona sin error; y cuando se simula disco lleno o memoria agotada durante la propia
   confirmación, entonces falla con esa clase, termina la transacción sin aplicar nada y una escritura nueva funciona
   sin error (V3, V8, G1, G5, T11, AC-B18).
9. **AC-V9** — Dado un almacén con páginas libres sueltas suficientes para un valor pero sin un hueco contiguo de su
   tamaño, y cuyo final está en la frontera del espacio libre (la prueba lo prepara sabiendo la implementación),
   cuando se guarda ese valor y se confirma, entonces se guarda, el número de páginas del almacén (P12) crece al
   menos en las que ocupa el valor y las páginas libres sueltas siguen libres; y con un tamaño máximo que no deja crecer lo suficiente, entonces la
   operación, o a más tardar la confirmación, falla con la clase de tamaño máximo, no aplica nada y una escritura
   nueva funciona sin error (V6, F5, T11).
10. **AC-V10** — Dada una escritura en disco que solo hace Añadir con un valor de 8 MiB sobre una clave existente,
    Quitar sobre claves que no están y operaciones que fallan por tamaño no válido con un valor de 268.435.457 bytes,
    cuando se confirma, entonces termina sin error y los archivos del almacén quedan idénticos byte a byte; Añadir
    con un valor de 268.435.456 bytes sobre una clave existente, con un tamaño máximo que no le deja sitio, devuelve
    que no la añadió sin fallar; y un
    valor de 268.435.457 bytes a través de una apertura de un árbol borrado, o en una escritura que ya tuvo un fallo
    de T11, falla con tamaño no válido (V1, A7, A10, B7, T10).
11. **AC-V11** — Dado un almacén pequeño con un valor fuera de su hoja, cuando se altera cada uno de sus bytes por
    separado y se abre y se lee el valor, entonces cada alteración se detecta como daño o no tiene efecto, con las
    reglas de AC10 de la raíz y sus anotaciones; nunca se devuelve el valor con un byte distinto (V7, G6, AC10).
12. **AC-V12** — Dado el almacén de formato `6` conservado como dato de prueba, cuando se abre, entonces falla con la
    clase de formato distinto como formato anterior y no se modifica; un almacén nuevo informa la versión de
    formato `7`; y leyendo en disco los bytes de la entrada de un valor fuera de su hoja, con el conocimiento de la
    implementación, su página y su longitud están en little endian (V3, V9, AC-B12).
13. **AC-V13** — Dado el informe de cierre, cuando se revisa, entonces contiene la medición de V11 con sus tiempos y
    tamaños del almacén por tamaño de valor y de la carga mixta, y los datos del equipo, en Windows y en Linux (V11).
14. **AC-V14** — Dado el informe de cierre, cuando se revisa, entonces contiene la evaluación de V4 con al menos los
    dos umbrales que nombra, los datos de la medición de cada uno, la complejidad que añade cada uno, el elegido y por
    qué (V4).

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Padre raíz, D11; módulo de árboles, B10: todo valor se devuelve como una sola secuencia contigua sin copia,
  también en modo temporal (V2).
- Transacciones, T16; Diario, J7: una versión confirmada de un valor fuera de su hoja que se conserva fuera de
  `krummi.data` se conserva contigua, y la referencia apunta a ella sin copia (V2).
- Módulo de árboles, B6: el máximo de un valor pasa a ser 256 MiB (V1).
- Módulo de árboles, *Dejado a la implementación*: el umbral de la hoja sigue dejado a la implementación, con la
  evaluación de V4.
- Árboles y operaciones por clave, A11 y AC-A17: el máximo provisional queda sustituido por el de V1.
- Árboles y operaciones por clave, A9; Recorrido en orden, R8: un daño en las páginas de un valor fuera de su hoja
  hace fallar a Leer y a los recorridos que necesitan su entrada, pero nunca a Guardar ni Añadir sobre su clave,
  Quitar, Quitar intervalo, vaciar, borrar ni sobrescribir con Renombrar su árbol (V7).
- Recorrido en orden, *Fuera de alcance*: los valores de más de 1.024 bytes en recorridos y en Quitar intervalo
  llegan con este PRD (V1, V2).
- Padre raíz, D7; Pager, P5; Diario, J3; escritura agrupada, E15; Espacio libre, F11; Transacciones, todo el
  documento; Árboles y operaciones por clave, A13; Recorrido en orden, R10, *Salida* y AC-R14: la versión de formato
  vigente pasa a ser `7`, y un almacén de formato `6`, también el creado con 5.1 y conservado como dato de prueba,
  es de formato anterior (V9).

## Referencias

- Padre: [trees.prd.md](trees.prd.md) v1.0; raíz: [krummi.prd.md](krummi.prd.md) v1.1
- Hermanos: [trees-keys.prd.md](trees-keys.prd.md) v1.0, [trees-scan.prd.md](trees-scan.prd.md) v1.0
- Relacionados: [free-space.prd.md](free-space.prd.md), [transactions.prd.md](transactions.prd.md),
  [pager.prd.md](pager.prd.md)
- Voron, páginas de desbordamiento: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron/Data/BTrees>
- LMDB, páginas de desbordamiento: <https://github.com/LMDB/lmdb/blob/mdb.master/libraries/liblmdb/mdb.c>
- SQLite, celdas de desbordamiento: <https://www.sqlite.org/fileformat2.html#b_tree_pages>

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 1927218b | 2026-10-27 06:38 UTC+00:00 | 2 h 4 min | 46 min | 1 h 18 min | 30 min | 48 min | 14 |

<!-- prd-time: [{"session": "1927218b-4e42-490b-bf4b-16e755467a49", "start": "2026-10-27T06:38:47.350000+00:00", "wall": 7442, "discarded": 2733, "agent": 1812, "human": 2895, "interventions": 14, "idle_minutes": 10}] -->
