# Espacio libre de Krummi — PRD

**Versión**: 1.0
**Fecha**: 2026-10-24
**Documento padre**: [Krummi, motor de almacenamiento embebido inspirado en Voron — PRD](krummi.prd.md) (v1.1)

## Objetivo y contexto

Construir el **espacio libre**, el módulo 4 del [reparto del padre](krummi.prd.md#reparto-en-prd-hijos): la
reutilización de las páginas que las transacciones liberan, de una en una o en rangos de páginas contiguas, sin
romper el aislamiento por instantánea (G3) ni la atomicidad y la recuperación (G1, G5).

Al cerrar este módulo, las escrituras pueden liberar páginas y reservar una página o un rango de N páginas
contiguas, y una reserva usa las páginas liberadas antes de hacer crecer el almacén. La información del almacén
dice cuántas páginas libres tiene. Las aplicaciones todavía no liberan nada por sí mismas: lo harán las estructuras
de datos a partir de los árboles B+ (módulo 5), que reservan y liberan a través de este módulo.

**Por qué ahora**: hasta hoy las páginas que una escritura deja de usar no se reutilizan (T14 de
[transactions.prd.md](transactions.prd.md)), y los árboles B+ del módulo 5 liberan y reservan páginas
continuamente. El gestor de espacio libre tiene que existir antes y no depender de ellos: un gestor guardado en un
árbol B+ necesita reservar páginas para sus propios nodos a través de sí mismo, y esa dependencia circular obliga a
reservas especiales y excepciones que este diseño evita.

Este PRD hereda todo lo del padre, lo del [Pager](pager.prd.md) (P1–P17), lo de las
[Transacciones](transactions.prd.md) (T1–T18), lo del [Diario](journal.prd.md) (J1–J17) y lo de la
[escritura agrupada y directa del diario](journal-group-commit.prd.md) (E1–E16); aquí solo se añade lo propio del
módulo. Lo que cambia de reglas anteriores está declarado en sus decisiones y anotado en los documentos afectados
(ver [Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

En este documento, el **mapa** es el registro persistente de qué páginas están libres; una **región** es un tramo
fijo de páginas consecutivas de `krummi.data` con su propia página del mapa; la **frontera** es el número de
página a partir del cual ninguna página se ha usado nunca; y las **páginas propias** del almacén son las del
encabezado, la de metadatos y las del mapa.

### Decisiones de producto

| ID | Decisión |
|---|---|
| F1 | **Superficie pública**: la información del almacén (P12) añade el número de páginas libres (F10). Reservar una página o un rango de páginas contiguas y liberar páginas dentro de una transacción de escritura es interno de la biblioteca, como en P1 y T1: lo usan los módulos siguientes y las pruebas, no las aplicaciones. |
| F2 | **Mapa por regiones**: `krummi.data` se divide en regiones de tamaño fijo, cada una con una página del mapa que registra qué páginas de la región están libres. La página del mapa es la primera de su región, salvo en la primera región, donde va justo después del encabezado y de la página de metadatos. Las páginas del mapa son páginas del almacén como las demás: una transacción las modifica con copia, viajan en su confirmación por el diario y se recuperan con ella (G1, G5, J8). Su posición fija hace que nunca se reserven a través del propio gestor, así que el mapa no depende de ninguna estructura del módulo 5 ni necesita reservas especiales. El formato del mapa es propio (D1). |
| F3 | **Frontera**: las páginas a partir de la frontera nunca se han usado y son libres sin estar en el mapa; el mapa registra las páginas libres por debajo de ella. Una reserva que no encuentra sitio por debajo de la frontera la avanza, y el archivo crece si hace falta (P11). La frontera entra en una región nueva cuando una reserva usa la primera página de esa región después de su página del mapa: la transacción que la avanza escribe entonces esa página del mapa, que queda ocupada; antes, la posición del mapa de esa región no está ocupada ni libre. Como todo cambio, el avance de la frontera solo se aplica si la transacción se confirma. La frontera nunca retrocede, y al reabrir, también tras una recuperación, `krummi.data` llega como mínimo hasta ella, salvo que el archivo se haya recortado con el almacén cerrado: entonces una página del mapa que falta es una página dañada (G6, fila de páginas que faltan del Pager) y una reserva de páginas que faltan hace crecer el archivo. |
| F4 | **Reserva de una página**: si hay alguna página libre por debajo de la frontera, en cualquier región, la reserva usa una de ellas; solo si no hay ninguna avanza la frontera. |
| F5 | **Reserva de un rango**: una transacción puede reservar exactamente N páginas contiguas, sin redondear a un tamaño fijo, desde 1 hasta el **tope**: las páginas de una región menos su página del mapa (unos 511 MiB). En la primera región, que también tiene el encabezado y la página de metadatos, el rango más largo posible es algo menor, así que un rango del tamaño del tope siempre va en otra región. Un rango nunca incluye páginas propias y nunca cruza de una región a otra. Las reglas, en orden: (1) si hay por debajo de la frontera un rango libre de al menos N páginas seguidas, la reserva lo usa; (2) si no, aunque haya N páginas libres sueltas, el rango se toma en la frontera: empieza en ella si las N páginas caben en lo que queda de su región; si no caben, empieza en la primera página después de la página del mapa de la región siguiente, y las páginas que se saltan quedan libres y disponibles para otras reservas; (3) el crecimiento necesario, incluidas las páginas saltadas, sigue P10, P11 y G7: si no cabe en el tamaño máximo, la reserva o, a más tardar, la confirmación falla con la clase de tamaño máximo, y si no cabe en el disco, con la de disco lleno, y en los dos casos sigue T11. El gestor no trocea un valor en varios rangos, no mueve páginas para juntar huecos y no compacta: eso corresponde a las estructuras que lo usan (módulos 5 y 8) y a la compactación (módulo 11). No hay un error de espacio fragmentado. |
| F6 | **Cuándo se reutiliza una página liberada**: una página que libera una transacción de escritura está disponible para las reservas de toda escritura que obtenga el turno después del punto sin retorno de la confirmación que la libera (E2), sin esperar a que terminen las lecturas abiertas. Las lecturas que ya podían ver la página siguen viendo su contenido anterior (T14, J5, J7). Dentro de la transacción que la libera, la página no está disponible para sus propias reservas. Si la transacción se aborta o su confirmación falla antes del punto sin retorno (T10, T11), la liberación no se aplica, como el resto de sus cambios; si falla después, la escritura que reutilizó la página sigue E8 como cualquier otra que vio esa confirmación. La recuperación nunca trae una reutilización sin la liberación que la permitió (J10, E9). Vale igual en los dos modos de confirmación. No hay una cola de liberaciones que espere al lector más antiguo. |
| F7 | **Lecturas largas y crecimiento** (lo que el padre deja a este módulo): una lectura de larga duración no impide reutilizar páginas liberadas (F6) y `krummi.data` no crece por su causa. Lo que retiene son las versiones anteriores de las páginas que se modifican o se reutilizan mientras está abierta, como dicen T14 y J7. |
| F8 | **Reservar y liberar dentro de una escritura**: una página reservada empieza con todos sus bytes a cero para la transacción y forma parte de su confirmación aunque la transacción no la modifique. Una transacción puede liberar páginas en uso según lo que ve (T2, E3), sueltas o en rangos, incluidas las que ella misma reservó; una página reservada como parte de un rango se puede liberar sola o con otras del rango, y las demás siguen en uso. Liberar una página descarta los cambios que la transacción le hubiera hecho y no cambia su contenido en `krummi.data`. Una vez liberada, la transacción ya no la puede leer ni modificar. Reservar y liberar son cambios: una transacción que lo hace no es una escritura sin cambios (T10, E11), y las referencias a sus páginas dejan de valer como tras cualquier otro cambio (T8). |
| F9 | **Lectura del mapa bajo demanda**: la apertura no lee las páginas del mapa; cada región se lee la primera vez que una reserva o una liberación la necesita, y su página del mapa pasa la comprobación de integridad de toda página (G6). Si está dañada, la operación falla con la clase de daño y el número de esa página, y sigue T11. Las lecturas, las escrituras que no necesitan esa región y la información del almacén siguen funcionando. El mapa dañado nunca se usa como válido, tampoco para saltarse la región: toda reserva o liberación que la lea vuelve a fallar igual. |
| F10 | **Páginas libres en la información** (amplía P12): la información del almacén da la suma de dos partes. La primera son las páginas libres del mapa según lo confirmado y durable, como lo vería una lectura que se abre en ese momento (E3). La segunda son las páginas que hay entre esa frontera durable y el final de `krummi.data` tal como está (P12), o de la memoria del almacén en modo temporal, sin contar las posiciones del mapa de las regiones a las que la frontera todavía no ha llegado (F3), y es cero si el archivo termina antes de la frontera. Así, lo que liberan o reservan transacciones abiertas o confirmaciones que todavía no son durables no cambia el valor, y las páginas que una escritura abierta, abortada o todavía no durable hizo crecer el archivo cuentan como libres. Como ese crecimiento puede no sobrevivir a una caída (P11, J8), el valor puede ser menor al reabrir. La primera parte es un total que se guarda en cada confirmación junto con los cambios del mapa y viaja con ella por el diario; nunca se calcula a partir del estado en memoria de las regiones leídas desde la apertura (F9), así que no depende de qué regiones se hayan leído. La consulta funciona sin leer el mapa, así que sigue funcionando siempre (P12), también con una página del mapa dañada (F9). |
| F11 | **Formato `5`**: el mapa cambia lo que un motor anterior puede leer y escribir con seguridad, así que la versión de formato vigente pasa a ser `5` (D7 del padre). Un almacén de formato `4` es de formato anterior aunque se cerrara sin error, y antes de la 1.0 no se migra. Precisa P5 y E15. |
| F12 | **Modo temporal**: el espacio libre funciona igual (G9): las reservas reutilizan las páginas liberadas antes de hacer crecer la memoria del almacén, y lo confirmado cuenta para F10 en cuanto la confirmación devuelve (E14). |
| F13 | **Medición**: el cambio incluye una medición reproducible, en Windows y en Linux, del tiempo de reservar una página y un rango de 16, 256 y 4.096 páginas en un almacén de al menos 1 millón de páginas con la mitad de ellas liberadas en huecos sueltos, y del tiempo de abrir ese almacén. No fija ningún umbral (D5 del padre); sus resultados van en el informe de cierre. |
| F14 | **Datos de los errores** (D8 del padre, T18): los errores de este módulo llevan los mismos datos que los de T18, la carpeta del almacén o que es temporal; el de daño lleva además el número de la página del mapa dañada. |

*Anotación del 2026-10-25 desde los árboles y operaciones por clave ([trees-keys.prd.md](trees-keys.prd.md)): F11, la versión de formato vigente pasa a ser `6`; un almacén de formato `5` es de formato anterior (decisión A13).*

*Anotación del 2026-10-27 desde los valores grandes ([trees-large-values.prd.md](trees-large-values.prd.md)): F11, la versión de formato vigente pasa a ser `7`; un almacén de formato `6` es de formato anterior (decisión V9).*

*Anotación del 2026-10-29 desde los árboles multivalor ([trees-multivalue.prd.md](trees-multivalue.prd.md)): F11, la versión de formato vigente pasa a ser `8`; un almacén de formato `7` es de formato anterior (decisión M14).*

### Estado de partida

Verificado el 2026-10-24 en el código y en el código público de Voron v6.0:

| Aspecto | Situación |
|---|---|
| Reserva de páginas | Una escritura solo puede reservar una página cada vez, que empieza a cero. Las páginas salen de un contador que solo sube, guardado en una página de metadatos que va justo después de las dos copias del encabezado y que viaja por el diario en las confirmaciones que reservan. No hay rangos de páginas contiguas. |
| Páginas liberadas | No existe ninguna forma de liberar una página ni ningún registro de páginas libres. Ninguna prueba ni documento técnico trata el espacio libre. |
| Versiones | Una página conserva su número cuando se modifica: la versión nueva se guarda en `krummi.temp` y se escribe en su sitio de `krummi.data` cuando ninguna lectura abierta necesita la anterior y es durable. Modificar una página nunca libera otra. |
| Diario | Cada confirmación guarda enteras las páginas que cambia, comprimidas cuando ocupan menos, y la recuperación las aplica en orden. |
| Crecimiento | Un almacén nuevo ocupa 64 KiB; el archivo dobla su tamaño en cada salto hasta saltos de 1 GiB. Cada crecimiento proyecta el archivo entero otra vez en memoria, así que un rango contiguo en `krummi.data` también lo es en la proyección. En modo temporal la memoria crece por trozos separados. |
| Encabezado | No tiene ningún campo libre para el gestor de espacio; el formato vigente es `4`. |
| Voron (referencia) | Guarda el espacio libre en un árbol de tamaño fijo con un mapa de bits por cada sección de 2.048 páginas, y evita la recursión con una guarda que, mientras modifica ese árbol, reserva sus páginas al final del archivo. Una página liberada se puede reutilizar en la transacción siguiente: la espera a los lectores está en el traslado al archivo de datos, no en el gestor. |
| Otros motores (referencia) | LMDB guarda las páginas libres en un árbol B+ indexado por transacción, las reutiliza cuando ningún lector puede verlas y busca rangos contiguos recorriendo una lista ordenada. SQL Server usa páginas de mapa en posiciones fijas (GAM, SGAM, PFS) por bloques de 8 páginas. redb y ext4 usan mapas de bits por región con un índice auxiliar en memoria. |

## Actores

- **Desarrollador integrador**: consulta en la información del almacén cuántas páginas libres tiene. No reserva ni
  libera páginas por sí mismo.
- **Mantenedor**: ejecuta la medición de F13 en Windows y en Linux.

Los módulos siguientes (árboles B+, tablas, streams y demás) son quienes reservan y liberan páginas a través de este
módulo.

## Flujo

1. Una transacción de escritura libera páginas que ya no usa (F8). Para sus propias reservas siguen ocupadas (F6).
2. La transacción reserva una página o un rango: el gestor busca primero entre las páginas libres por debajo de la
   frontera, leyendo bajo demanda las regiones que necesita (F4, F5, F9), y si no encuentra sitio avanza la
   frontera, que hace crecer el archivo si hace falta (F3).
3. La confirmación lleva por el diario, con el resto de sus páginas, los cambios del mapa y de la frontera (F2, F3).
4. Desde su punto sin retorno, las páginas que liberó están disponibles para las escrituras siguientes; las lecturas
   abiertas siguen viendo su contenido anterior (F6).
5. Cuando la confirmación es durable, la información del almacén refleja el nuevo número de páginas libres (F10).

## Validaciones y mensajes

Valen las de los ascendientes, incluidos los fallos de T11 y T12 en cualquier reserva o liberación. Este módulo
añade estas situaciones, solo de la biblioteca (la herramienta no abre almacenes hasta el módulo 12). Todas siguen
T11 sin excepciones, también el uso incorrecto: la operación no tiene efecto, la transacción ya no puede
confirmarse, confirmarla falla con la clase del primer fallo sin aplicar nada, y el almacén sigue aceptando
escrituras.

| Situación | Regla | Clase de error |
|---|---|---|
| Una reserva o una liberación lee una región cuya página del mapa está dañada | La operación falla con el número de esa página (F9). | Daño |
| El crecimiento de una reserva no cabe en el tamaño máximo o en el disco | Falla la reserva o, a más tardar, la confirmación (F5). | Tamaño máximo o disco lleno |
| Se pide un rango de 0 páginas o de más páginas que el tope | La operación falla sin efecto (F5). | Uso incorrecto |
| Se libera una página propia, una a partir de la frontera (que nunca estuvo en uso), una que no existe en el almacén o una ya liberada en la misma transacción | La operación falla sin efecto (F8). | Uso incorrecto |
| Se libera una página que el mapa tiene como libre | La operación falla sin efecto (F8). Solo se reconoce leyendo su región, así que con la página del mapa dañada gana el daño. | Uso incorrecto |
| Se lee o se modifica una página que la misma transacción liberó, o se modifica una página propia | La operación falla sin efecto (F8). | Uso incorrecto |

Cuando una operación cumple varias situaciones a la vez, gana el uso incorrecto que se reconoce sin leer el mapa
(todos los de la tabla salvo liberar una página que el mapa tiene como libre); después, el primer fallo que la
operación encuentra, al leer el mapa (daño, memoria agotada, entrada y salida) o al crecer (tamaño máximo, disco
lleno).

## Salida

El módulo no escribe nada en pantalla. La información del almacén (P12) añade un dato y conserva los demás sin
cambios; la versión de formato que informa pasa a ser `5` (F11). Ejemplo, para un almacén recién creado en disco
sin tamaño máximo:

| Dato | Valor |
|---|---|
| Versión de formato | `5` |
| Tamaño de página | `8192` |
| Tamaño del almacén en bytes | múltiplo de `8192`, como mucho `1048576` |
| Número de páginas | el tamaño entre `8192` |
| Tamaño máximo | ninguno |
| Temporal | no |
| Páginas libres | el número de páginas menos las páginas propias (encabezado, metadatos y mapa de la primera región) |

En disco, la carpeta del almacén tiene los mismos nombres que antes (J2); el mapa está dentro de `krummi.data`.

## Reutilización

- La copia de las páginas modificadas y su conservación en `krummi.temp` mientras alguna lectura necesita la versión
  anterior (T14, J5, J7): es lo que hace segura la reutilización inmediata (F6), sin duplicar esa lógica en el
  gestor.
- El diario, el punto sin retorno, la regla de prefijo y la recuperación (E2, E8, J8, J10): el mapa y la frontera
  viajan en las confirmaciones como cualquier otra página.
- La comprobación de integridad de las páginas y la clase de daño (G6).
- El crecimiento del archivo, el tamaño máximo y la memoria agotada (P10, P11, P13).
- El contador actual de páginas reservadas, que pasa a ser la frontera (F3).
- La simulación de caídas, cortes de corriente y fallos de las pruebas del Diario y de la escritura agrupada.

## Fuera de alcance

- Reservar o liberar páginas desde la API pública (F1).
- Reducir el tamaño de `krummi.data`, también cuando las páginas libres están al final (P11; compactación, módulo 11),
  y devolver al sistema la memoria de un almacén temporal cuyas páginas están libres.
- Mover páginas para juntar huecos, desfragmentar o trocear un valor en varios rangos (F5).
- Rangos mayores que el tope, que crucen de una región a otra o alineados a un múltiplo dado (F5); el tamaño máximo
  de un valor lo fija el módulo 5.
- Leer un rango como un bloque continuo en memoria, también en modo temporal, donde la memoria crece por trozos
  separados: lo fija el módulo 5 con sus valores grandes.
- Esperar al lector más antiguo para reutilizar una página, y reutilizarla dentro de la transacción que la libera
  (F6).
- Detectar que una transacción lee o modifica una página libre que no ha reservado ni liberado ella misma: es un
  error de la estructura que lo hace, que la verificación (módulo 12) puede encontrar.
- Datos de fragmentación en la información del almacén: el rango libre más largo, porcentajes o el estado de cada
  región (informe de almacenamiento, módulo 12).
- Comprobar que el total de páginas libres coincide con el mapa, o que el mapa coincide con las páginas que usan
  las estructuras: lo hace la verificación (módulo 12).
- Reparar o reconstruir una página del mapa dañada (rescate de datos, módulo 12).
- Borrar el contenido de las páginas liberadas (borrado seguro, fuera de alcance del padre).
- Elegir dónde se reserva una página para que quede cerca de otras de la misma estructura.
- Cifrar el mapa: lo cubre el cifrado de todo lo que el motor escribe (módulo 9).
- Umbrales de rendimiento (módulo 6).

## Dejado a la implementación

- El tamaño exacto de una región y, con él, el valor exacto del tope, siempre que el tope sea de al menos 65.000
  páginas; la disposición del mapa dentro de su página.
- Qué rango o página libre se elige cuando hay varios que sirven (el primero, el más ajustado u otro), y en qué
  orden se leen las regiones, siempre que se cumplan F4 y F5. Por eso, con una página del mapa
  dañada y páginas libres en otras regiones, una reserva puede encontrar sitio sin leer la región dañada o fallar
  con daño al leerla (F9).
- Cómo se guardan la frontera y el total de páginas libres, y cómo se resume en memoria el estado de las regiones ya
  leídas, siempre que ese resumen distinga las regiones leídas de las desconocidas, nunca trate una región
  desconocida como llena y se pueda descartar y reconstruir a partir del mapa.
- Los nombres de las operaciones internas y del nuevo dato de la información, que siguen las convenciones de Rust y
  están en inglés.
- La orden que ejecuta la medición de F13 y la forma de su informe.
- Cómo las pruebas dañan páginas del mapa, saben qué regiones se leyeron y simulan la falta de espacio solo al
  hacer crecer `krummi.data`.

## Criterios de aceptación

Valen las condiciones de prueba del Diario y de la escritura agrupada: los criterios que reservan, liberan, leen o
escriben páginas son pruebas internas de la biblioteca; las que necesitan saber qué regiones se leyeron, retener
confirmaciones en la cola del diario o simular fallos usan el conocimiento de la propia implementación. Las pruebas
comprueban qué conjunto de páginas recibe una reserva solo cuando F4 y F5 lo determinan. Los criterios que
necesitan varias regiones pueden usar un tamaño de región reducido solo para las pruebas; la última parte de AC-F9
y la medición de F13 usan el tamaño del producto.

Los criterios del padre raíz, de Transacciones, del Diario y de la escritura agrupada siguen valiendo, y AC10 del
padre raíz se repite con un almacén pequeño que tiene páginas liberadas y una página del mapa. Además:

1. **AC-F1** — Dado un almacén cuyas únicas páginas libres por debajo de la frontera son 10 páginas liberadas por
   una confirmación durable, cuando una escritura posterior reserva 10 páginas de una en una, entonces recibe
   exactamente esas 10, `krummi.data` no cambia de tamaño, la frontera no avanza y, tras confirmar, las páginas
   libres de la información bajan en 10; y la reserva siguiente avanza la frontera (F3, F4, F10).
2. **AC-F2** — Dado un almacén sin páginas libres por debajo de la frontera, cuando una escritura libera una página
   y después reserva otra, entonces la reservada no es la liberada; y tras confirmar, una escritura posterior que
   reserva una página recibe la liberada (F4, F6).
3. **AC-F3** — Dada una lectura abierta que ve el contenido de una página, cuando una escritura la libera y se
   confirma, de modo que es la única libre por debajo de la frontera, y otra escritura reserva una página, escribe
   un contenido nuevo y se confirma, entonces la reservada es ese mismo número, la lectura abierta sigue viendo el
   contenido anterior, una lectura nueva ve el nuevo, y tras terminar la lectura
   antigua, cerrar y reabrir, está el nuevo; y el mismo resultado se obtiene con las dos confirmaciones en modo
   diferido, con la lectura nueva abierta después de que son durables. Y repitiendo tres veces sobre el mismo número
   de página la liberación y la reutilización con un contenido distinto, con una lectura abierta antes de cada
   liberación y todas abiertas hasta el final, cada lectura ve el contenido de su generación (F6, T14, E3).
4. **AC-F4** — Dada una escritura que libera páginas y reserva otras entre las libres por debajo de la frontera,
   cuando se aborta, o se deja de usar sin
   confirmar, o su confirmación falla por disco lleno, entonces ninguna de sus liberaciones ni reservas se aplica:
   las páginas que liberó siguen en uso, las que reservó siguen libres, la frontera no avanza y las páginas libres
   de la información no cambian (F6, T10, T11).
5. **AC-F5** — Dada una confirmación en modo durable que libera una página, retenida en la cola del diario, y una
   escritura que reutiliza esa página y se confirma en modo durable después, cuando falla la escritura del grupo de
   la primera, entonces las dos fallan con la clase de entrada y salida; y al reabrir sin el fallo, el almacén tiene
   un prefijo de las dos y nunca la segunda sin la primera (F6, E8).
6. **AC-F6** — Dada una secuencia de confirmaciones, en los dos modos, que liberan páginas y las reutilizan con
   contenido nuevo, en la que una misma página se libera y se reutiliza al menos tres veces, cuando se simula un corte de corriente en cada punto en que se escribe o se lleva al disco el
   diario o `krummi.data`, y se mata el proceso en cada punto de la recuperación, entonces al reabrir el almacén
   tiene un prefijo de la secuencia, cada página reutilizada tiene el contenido de la última confirmación presente
   que la escribió, ninguna página está a la vez libre en el mapa y en uso según las confirmaciones presentes,
   `krummi.data` llega al menos hasta la frontera, y la primera parte de las páginas libres de la información
   coincide con las libres del mapa (F2, F3, F6, F10, G5).
7. **AC-F7** — Dado un almacén con un rango libre de 20 páginas seguidas por debajo de la frontera, cuando una
   escritura reserva 16 páginas contiguas, entonces las recibe dentro de ese rango sin que la frontera avance; y dado
   un almacén con 100 páginas libres sueltas, ningún rango libre de 16 y una frontera a la que le quedan más de 16
   páginas en su región, la reserva de 16 contiguas recibe las 16 páginas que empiezan en la frontera, que avanza
   16 (F5).
8. **AC-F8** — Dada una frontera a la que le quedan 10 páginas en su región y ninguna página libre por debajo de
   ella, cuando una escritura reserva 16 páginas contiguas, entonces el rango empieza en la primera página después
   de la página del mapa de la región siguiente, las páginas libres de la información, tras confirmar de forma
   durable, incluyen las 10 saltadas, y la reserva siguiente de una página recibe una de ellas (F3, F5, F10).
9. **AC-F9** — Dada una escritura, cuando pide un rango de 0 páginas o de una página más que el tope, libera dos
   veces la misma página, libera una página que el mapa tiene como libre, una del encabezado, de metadatos o del
   mapa, o una más allá del final del almacén, lee o modifica una página que ella misma liberó, o modifica una
   página del mapa, entonces cada operación falla con la clase de uso incorrecto sin efecto; y en cada caso, en una
   escritura que antes de la operación incorrecta modificó una página, reservó otra y liberó una tercera,
   confirmarla falla con la clase de uso incorrecto, ninguno de esos cambios aparece en una lectura nueva ni al
   reabrir, y la escritura siguiente se confirma sin error. En un almacén sin tamaño máximo, un rango del tamaño del
   tope se reserva sin error en una región que no es la primera (F5, F8, T11).
10. **AC-F10** — Dada una escritura que modifica una página, reserva un rango de 8 páginas, libera 3 de ellas y la
    página modificada, y libera una página que ella misma reservó suelta, cuando se confirma y se reabre, entonces
    las 5 páginas restantes del rango siguen en uso y empiezan a cero, las 3 del rango, la suelta y la modificada
    están libres, y la página modificada conserva en `krummi.data` su contenido anterior; y si antes de esa escritura
    no había páginas libres por debajo de la frontera, una escritura posterior que reserva 5 páginas de una en una
    recibe exactamente esas 5 (F4, F8).
11. **AC-F11** — Dado un almacén con un tamaño máximo que deja sitio para las versiones de una escritura en
    `krummi.temp` pero no para que `krummi.data` crezca, y con páginas libres por debajo de la frontera y ninguna
    entre la frontera y el final del archivo, cuando una escritura reserva de una en una tantas páginas como hay
    libres en el mapa, entonces se confirma sin error; cuando reserva una página más, falla con la clase de tamaño
    máximo, no aplica nada y el almacén sigue aceptando escrituras; y con una falta de espacio simulada solo al
    hacer crecer `krummi.data`, una reserva que necesita crecer falla con la clase de disco lleno y una que reutiliza
    páginas libres se confirma sin error (F5, P10, G7).
12. **AC-F12** — Dada una lectura abierta durante 1.000 confirmaciones que liberan y reservan el mismo número de
    páginas, cuando se observa `krummi.data`, entonces su tamaño no cambia desde la segunda (F7).
13. **AC-F13** — Dado un almacén cerrado sin error cuyas únicas páginas libres están en una región alejada de la
    frontera, cuando se abre, entonces la apertura no lee ninguna página del mapa; y cuando una escritura reserva
    una página, recibe una de esas páginas libres y la frontera no avanza (F4, F9).
14. **AC-F14** — Dado un almacén cerrado cuyas únicas páginas libres por debajo de la frontera están en una región
    cuya página del mapa tiene un byte alterado, cuando se abre, entonces abre sin error, la información da las
    mismas páginas libres que antes de alterarla, todas las páginas en uso salvo esa página del mapa se leen sin
    error, una escritura que solo
    modifica páginas en uso se confirma sin error, y una que reserva una página por debajo de la frontera falla con
    la clase de daño y el número de esa página del mapa, no se puede confirmar, y una reserva posterior vuelve a
    fallar igual (F9, F10).
15. **AC-F15** — Dado un almacén nuevo, cuando se consulta la información, entonces las páginas libres son el
    número de páginas menos las páginas propias; y dado un almacén con páginas en uso que reservó una confirmación
    durable anterior, cuando una escritura abierta las libera, las páginas libres no cambian; cuando una
    confirmación diferida las libera, no cambian hasta que es durable, y entonces suben en ese número; y tras cerrar
    y reabrir dan el mismo valor (F10, E3).
16. **AC-F16** — Dada una secuencia aleatoria y reproducible por semilla de al menos 100.000 operaciones sobre un
    almacén de hasta 4 regiones, con reservas de páginas sueltas y de rangos de hasta 64 páginas, liberaciones,
    confirmaciones en los dos modos y abortos, con lecturas abiertas y cerradas en puntos aleatorios y una caída
    simulada y reapertura en otros, cuando se compara con un modelo de referencia que conoce las páginas en uso y el
    tamaño real de `krummi.data`, entonces nunca se reserva una página en uso, ningún rango incluye una página
    propia, cada lectura ve el contenido de su instantánea, y las páginas libres de la información y del mapa
    coinciden con las del modelo (F2, F4, F5, F6, F10).
17. **AC-F17** — Dado un almacén válido cuyo encabezado lleva el formato `4`, cuando se abre, entonces falla con la
    clase de formato distinto como formato anterior y queda idéntico byte a byte; y la información de un almacén
    nuevo da el formato `5` (F11).
18. **AC-F18** — Dado un almacén en modo temporal, cuando se repiten AC-F1, AC-F2, AC-F3 sin la parte diferida ni
    la de reabrir, AC-F4 sin la parte de disco lleno, AC-F7, AC-F8, AC-F9 sin su última parte, AC-F10 sin reabrir,
    AC-F11 sin la parte de disco lleno, AC-F12 y AC-F15 sin la parte diferida ni la de reabrir, entonces dan el
    mismo resultado, con la memoria del almacén en lugar de `krummi.data` y de `krummi.temp` (F12).
19. **AC-F19** — Dado un almacén cerrado sin error, recortado con el almacén cerrado de modo que el archivo termina
    antes de la frontera y falta la página del mapa de una región con páginas libres, cuando se abre, entonces la
    segunda parte de las páginas libres de la información es cero, y una reserva que necesita esa región falla con
    la clase de daño y el número de esa página del mapa (F3, F10).
20. **AC-F20** — Dado el informe de cierre, cuando se revisa, entonces contiene los tiempos de la medición de F13
    en Windows y en Linux (F13).

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Padre raíz, reparto en PRD hijos, módulo 4: una lectura de larga duración no impide reutilizar páginas; lo que
  retiene son versiones en `krummi.temp` (F7).
- Padre raíz, D7 (anotación de formato): la versión de formato vigente pasa a ser `5` (F11).
- Pager, P5; escritura agrupada, E15: la versión de formato vigente pasa a ser `5`; un almacén de formato `4` es de
  formato anterior (F11).
- Pager, P12; Transacciones, T14 (última frase); Diario, J15: la información del almacén añade el número de páginas
  libres, que combina lo durable del mapa con el tamaño de `krummi.data` tal como está (F10); el resto de la
  información sigue describiendo solo `krummi.data`.
- Pager, fila de páginas que faltan porque el archivo se recortó con el almacén cerrado: vale también para una página
  del mapa (F3).
- Transacciones, T18: los errores de este módulo llevan sus datos (F14).
- Transacciones, T14: termina el "hasta el módulo 4": las páginas que una escritura libera se reutilizan según F6 y
  F7.
- Transacciones, T8 y T10: reservar y liberar son cambios (F8).
- Diario, J8: al terminar la recuperación, `krummi.data` llega como mínimo hasta la frontera (F3).

## Referencias

- Padre: [krummi.prd.md](krummi.prd.md) v1.1 (raíz)
- Relacionados: [pager.prd.md](pager.prd.md), [transactions.prd.md](transactions.prd.md),
  [journal.prd.md](journal.prd.md), [journal-group-commit.prd.md](journal-group-commit.prd.md)
- Voron, gestión del espacio libre: <https://github.com/ravendb/ravendb/blob/v6.0/src/Voron/Impl/FreeSpace/FreeSpaceHandling.cs>
- Voron, reserva de páginas: <https://github.com/ravendb/ravendb/blob/v6.0/src/Voron/Impl/LowLevelTransaction.cs>
- LMDB: <https://github.com/LMDB/lmdb/blob/mdb.master/libraries/liblmdb/mdb.c>
- SQL Server, páginas y extents: <https://learn.microsoft.com/en-us/sql/relational-databases/pages-and-extents-architecture-guide>
- redb, diseño: <https://github.com/cberner/redb/blob/master/docs/design.md>

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 73d9cfa6 | 2026-10-24 07:32 UTC+01:00 | 46 min | 0 min | 46 min | 24 min | 23 min | 8 |

<!-- prd-time: [{"session": "73d9cfa6-d168-4b93-b59e-ff2b19ef0a0c", "start": "2026-10-24T06:32:42.026000+00:00", "wall": 2782, "discarded": 0, "agent": 1431, "human": 1350, "interventions": 8, "idle_minutes": 10}] -->
