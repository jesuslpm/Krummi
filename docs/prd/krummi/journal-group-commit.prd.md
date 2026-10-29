# Diario: escritura agrupada y directa — PRD

**Versión**: 1.0
**Fecha**: 2026-10-23
**Documento padre**: [Diario y recuperación de Krummi — PRD](journal.prd.md) (v1.0)

## Objetivo y contexto

Cambiar cómo el diario hace durables las confirmaciones, para que varias escrituras confirmadas desde hilos
distintos compartan una sola escritura del diario (*group commit*, como InnoDB y PostgreSQL), y para que el diario
se escriba directamente al medio físico, sin pasar por la caché del sistema.

Hoy cada confirmación en modo durable escribe su registro, espera a que el disco lo guarde y solo después cede el
turno de escritura: mientras el disco trabaja, ninguna otra escritura avanza, y las confirmaciones durables por
segundo quedan limitadas por la latencia del disco. Con este cambio, la confirmación cede el turno antes de esperar
al disco, la escritura siguiente trabaja mientras tanto, y las confirmaciones que coinciden en la espera se escriben
juntas. Una confirmación diferida pasa a ser durable en el mismo momento que una en modo durable; la diferencia es
solo que devuelve sin esperarlo.

G4 no cambia: hay como máximo una transacción de escritura abierta. Una confirmación que ya cedió el turno y espera
a ser durable no es una transacción abierta.

**Por qué ahora**: el informe de cierre de los follow-ups del diario dejó pendiente que una confirmación diferida que
alcanza el umbral de confirmaciones pendientes espera al disco, contra la letra de J10; resolverlo necesita que otro
hilo escriba el diario, que es lo que trae este cambio. Además, es la base de rendimiento que medirá el banco
comparativo (módulo 6).

Este PRD hereda todo lo del [Diario](journal.prd.md) (J1–J17) y, a través de él, lo del padre raíz, del
[Pager](pager.prd.md) y de las [Transacciones](transactions.prd.md). Lo que cambia de reglas anteriores está
declarado en sus decisiones y anotado en los documentos afectados (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

En este documento, **cola del diario** es la de las confirmaciones que cedieron el turno y esperan a escribirse, y
**grupo** es el conjunto de confirmaciones que se escriben juntas; la cola de peticiones de escritura sigue siendo
la de T3.

### Decisiones de producto

| ID | Decisión |
|---|---|
| E1 | **Superficie pública**: no cambia. No hay opciones nuevas de apertura ni de confirmación. |
| E2 | **Punto sin retorno**: al confirmar una escritura con cambios, la confirmación hace primero todo lo que puede fallar: los fallos de T11, guardar sus páginas en `krummi.temp` (J7), prepararla para el diario y reservar su sitio en él (E7). Si algo falla, no aplica nada y sigue T11, J7 y J12: con la clase de entrada y salida el almacén deja de aceptar escrituras (T12); con las demás sigue aceptándolas. Si la cola del diario está llena, o si la confirmación necesita un archivo nuevo del diario (E7), espera, sin ceder el turno, a que haya sitio o a que el archivo esté reservado, también en modo diferido; si mientras tanto el almacén deja de aceptar escrituras, falla con la clase de entrada y salida sin aplicar nada. Después, la confirmación entra en la cola del diario y cede el turno: la siguiente petición de escritura lo obtiene sin esperar a que la anterior sea durable. Ese momento es el **punto sin retorno**: desde él la confirmación ya no se puede deshacer, tampoco por un pánico del hilo que la pidió, y solo puede fallar como dice E8. Un fallo de una confirmación antes de su punto sin retorno no afecta a las que ya estaban en la cola del diario: se escriben y se hacen visibles como siempre, también cuando ese fallo deja el almacén sin aceptar escrituras. Sustituye la parte de J4 según la cual la siguiente petición de escritura no obtiene su turno hasta que la confirmación es durable. |
| E3 | **Qué ve cada transacción** (almacén en disco; el temporal sigue E14): una escritura ve, además de lo de T2, todas las confirmaciones que pasaron su punto sin retorno antes de que ella obtuviera el turno, sean ya durables o no. Una lectura solo ve confirmaciones durables, en los dos modos: un grupo se hace visible entero para las lecturas nuevas en cuanto es durable, y una confirmación en modo durable lo hace antes de devolver, de modo que toda transacción que se abra después de que devuelva la ve (J4, T2). Una diferida no es visible para las lecturas hasta que es durable, aunque ya haya devuelto, tampoco para las del mismo hilo (E5). Las confirmaciones se hacen visibles en el orden en que pasaron su punto sin retorno: una lectura que ve una confirmación ve todas las anteriores. Sustituye la parte de J10 y de T2 según la cual una diferida es visible para toda transacción que se abra después de que devuelva. |
| E4 | **Confirmación agrupada**: las confirmaciones que están a la vez en la cola del diario se escriben juntas, en el orden en que pasaron su punto sin retorno, y pasan a ser durables a la vez. Una en modo durable devuelve cuando es durable (J4); esa espera no tiene límite y no se puede cancelar. |
| E5 | **Confirmación diferida**: pasa a ser durable en el mismo momento que lo sería una en modo durable que hubiera pasado el punto sin retorno en su lugar; el diario no trata distinto los dos modos. Devuelve en su punto sin retorno, sin esperar a ser durable ni a que lo sean las anteriores; a partir de ahí la ven las escrituras siguientes, y las lecturas cuando es durable (E3). Hasta que su grupo es durable, una caída del proceso, del sistema o un corte de corriente pueden perderla, siempre según la regla de prefijo de J10. Sustituye la parte de J10 según la cual una diferida sobrevive a una caída del proceso desde que devuelve. |
| E6 | **Escritura directa**: el diario se escribe directamente al medio físico, sin pasar por la caché del sistema, de modo que terminar de escribir un grupo ya es hacerlo durable. Si el sistema de archivos o el disco no la admiten, el diario se escribe a través de la caché y se lleva al disco en cada grupo, con las mismas garantías. Comprobar si se admite no crea archivos distintos de los de J2 ni hace fallar por sí solo la apertura ni una confirmación, y el almacén no informa de cuál de los dos modos usa. Un fallo al escribir con escritura directa es un fallo de E8, nunca un cambio a la alternativa. `krummi.data` y `krummi.temp` se escriben como hasta ahora. |
| E7 | **Reserva del diario**: cada archivo del diario se crea cuando una confirmación lo necesita (J2), con su tamaño final, y su creación y su tamaño son durables antes del punto sin retorno de esa confirmación, así que escribir un grupo nunca cambia el tamaño de un archivo. La falta de espacio para el diario se detecta al reservar, antes del punto sin retorno (E2). Una confirmación mayor que el tamaño habitual de un archivo se admite. La parte reservada y sin usar de un archivo, y un archivo del diario sin ninguna confirmación íntegra, no son daño: el segundo cuenta como vacío para J9. |
| E8 | **Fallos después del punto sin retorno**: si falla escribir o llevar al disco un grupo, fallan con la clase de entrada y salida todas las confirmaciones de ese grupo y de la cola del diario que todavía no habían devuelto, y el almacén deja de aceptar escrituras (T12). Desde ese fallo no se escribe nada más en el diario hasta el cierre, que lo reintenta (E12). Si ninguna espera, lo informan las operaciones de J13. La escritura abierta en ese momento, que puede haber visto esas confirmaciones (E3), falla al confirmarse con cambios con esa clase. Las lecturas nunca ven las confirmaciones de ese grupo ni de la cola del diario, tampoco las diferidas que ya habían devuelto. Al reabrir están todas las confirmaciones en modo durable que habían devuelto sin error y, de las demás, las primeras en orden hasta algún punto (J10). |
| E9 | **Daño y caídas con grupos**: para J9, las confirmaciones de un grupo cuentan juntas: un grupo es durable según el diario si algún grupo íntegro posterior se escribió cuando él ya era durable. Así, la ventana de J9 en la que un daño no se distingue de una confirmación que no terminó pasa de la última confirmación al último grupo: un daño en él descarta sin error la confirmación dañada y las posteriores, aunque hubieran devuelto sin error. Una caída del proceso, del sistema o un corte de corriente mientras se escribe un grupo nunca daña ni hace desaparecer grupos anteriores. |
| E10 | **Hacer durable lo confirmado** (J11): devuelve cuando son durables todas las confirmaciones que pasaron su punto sin retorno antes de llamarla, incluidas las que todavía no han devuelto. Lo demás de J11 no cambia. |
| E11 | **Confirmación sin cambios** (T10): en modo durable, si la escritura vio confirmaciones que todavía no son durables (E3), espera a que lo sean y, si fallan (E8), falla con la clase de entrada y salida; si no, termina al instante sin error. En modo diferido termina al instante sin error. En ningún caso escribe en el diario. Abortar una escritura (T10) nunca espera. |
| E12 | **Cierre**: cerrar, o dejar de usar el almacén sin cerrarlo, espera a que se escriba y sea durable todo lo que hay en la cola del diario, y después sigue J14. En un almacén que no acepta escrituras, el cierre vuelve a intentar escribir la cola y llevarla al disco, como T12: si lo consigue, termina sin error, y al reabrir pueden estar también confirmaciones que fallaron con la clase de entrada y salida (E8 lo admite); si no, falla con esa clase. En los dos casos, cuando el cierre devuelve no queda ningún hilo del almacén en marcha. |
| E13 | **Fallos del propio mecanismo**: lo que escribe el diario se pone en marcha al abrir, antes de recuperar (paso 8 del orden del padre raíz). Si no se puede, la apertura falla con la clase de memoria agotada sin tocar el almacén (P13). Si deja de funcionar con el almacén abierto, es un fallo de E8. |
| E14 | **Modo temporal**: no tiene diario (J17); confirmar funciona como hasta ahora, en los dos modos: la confirmación es visible para toda transacción que se abra después de que devuelva (T2). |
| E15 | **Formato `4`**: el diario cambia lo que un motor anterior puede leer con seguridad, así que la versión de formato vigente pasa a ser `4` (D7 del padre raíz). Un almacén de formato `3` es de formato anterior aunque se cerrara sin error, y antes de la 1.0 no se migra. Precisa P5, J3 y R5. |
| E16 | **Medición**: el cambio incluye una medición reproducible de confirmaciones durables por segundo con 1, 4 y 16 hilos que confirman a la vez, con escritura directa y con la alternativa de E6, en Windows y en Linux. No fija ningún umbral (D5 del padre raíz); sus resultados van en el informe de cierre. |

*Anotación del 2026-10-24 desde el espacio libre ([free-space.prd.md](free-space.prd.md)): E15, la versión de
formato vigente pasa a ser `5`; un almacén de formato `4` es de formato anterior (decisión F11).*

*Anotación del 2026-10-25 desde los árboles y operaciones por clave ([trees-keys.prd.md](trees-keys.prd.md)): E15, la versión de formato vigente pasa a ser `6`; un almacén de formato `5` es de formato anterior (decisión A13).*

*Anotación del 2026-10-27 desde los valores grandes ([trees-large-values.prd.md](trees-large-values.prd.md)): E15, la versión de formato vigente pasa a ser `7`; un almacén de formato `6` es de formato anterior (decisión V9).*

*Anotación del 2026-10-29 desde los árboles multivalor ([trees-multivalue.prd.md](trees-multivalue.prd.md)): E15, la versión de formato vigente pasa a ser `8`; un almacén de formato `7` es de formato anterior (decisión M14).*

### Estado de partida

Verificado el 2026-10-23 en el código:

| Aspecto | Situación |
|---|---|
| Confirmación | Toda la confirmación corre en el hilo que confirma, en este orden: guarda las páginas en el área temporal, escribe el registro del diario, espera al disco (solo en modo durable), hace visibles los cambios, cede el turno y después intenta trasladar a `krummi.data`. Mientras espera al disco tiene el diario en exclusiva, así que ninguna otra confirmación avanza. Cada confirmación es un registro; no hay grupos. |
| Modo diferido | Escribe el registro sin esperar al disco. El traslado obliga a llevar el diario al disco cuando hay 256 confirmaciones o 1 MiB pendientes, y la confirmación diferida que lo provoca espera a esa escritura antes de devolver. |
| Hacer durable (J11) | Lleva al disco el archivo actual del diario; no entra en la cola de peticiones de escritura y se ordena con las confirmaciones solo a través del acceso exclusivo al diario. |
| Archivos del diario | Se abren como archivos normales, con caché del sistema; no hay escritura directa en ningún sitio. Crecen al añadir registros, no se reservan de antemano, y se pasa al siguiente al llegar a 16 MiB. Cada registro lleva su comprobación de integridad y hasta qué confirmación era durable el diario al escribirlo, va comprimido cuando ocupa menos y se rellena con ceros hasta un múltiplo de 4 KiB. |
| Hilos | El almacén no crea ningún hilo; el traslado a `krummi.data` corre en el hilo que confirma, después de ceder el turno. |
| Fallos | Un fallo al llevar el diario al disco deja el almacén sin aceptar escrituras; las peticiones que esperan y las nuevas fallan con la clase de entrada y salida. La falta de espacio y de memoria no lo dejan así. |
| Pruebas | Hay simulación de cortes de corriente y de fallos de escritura y de llevar al disco, pero solo actúa en el hilo que la activa. |
| Medición | No hay ninguna medición de rendimiento en el repositorio. |
| Referencias | PostgreSQL e InnoDB agrupan en una sola escritura del registro las confirmaciones que esperan a la vez. Voron escribe el diario con escritura directa y comprueba al abrir si el sistema la admite; CanopyDB la ofrece como opción desactivada por defecto, con comprobación y alternativa. |

## Actores

- **Desarrollador integrador**: confirma desde varios hilos, en modo durable o diferido, y trata los errores por su
  clase. No hace nada distinto de antes.
- **Mantenedor**: ejecuta la medición de E16 en Windows y en Linux.

## Flujo

1. Un hilo confirma una escritura. La confirmación hace lo que puede fallar, incluida la reserva en el diario
   (E2, E7).
2. La confirmación pasa su punto sin retorno: entra en la cola del diario y cede el turno. La siguiente escritura
   empieza y la ve (E2, E3). Una diferida devuelve aquí (E5).
3. Las confirmaciones de la cola del diario se escriben juntas, de forma directa (E4, E6).
4. Cuando el grupo es durable, todas sus confirmaciones se hacen visibles para las lecturas, y las de modo durable
   devuelven sin error (E3, E4).
5. Al cerrar, se escribe lo que queda en la cola del diario y el almacén se detiene (E12).

## Validaciones y mensajes

Valen las del Diario y de sus ascendientes. Este cambio no añade clases de error ni mensajes de la herramienta; solo
cambia en qué momento se informan algunas situaciones (E2, E7, E8, E11, E13). La única situación nueva es abrir un
almacén sin poder poner en marcha lo que escribe el diario, que falla con la clase de memoria agotada (E13).

## Salida

El cambio no escribe nada en pantalla y no cambia la forma de la información del almacén (P12); la versión de
formato que informa pasa a ser `4` (E15).

En disco, la carpeta tiene los mismos nombres que en el Diario (J2). Cambia que cada archivo del diario tiene su
tamaño final desde que aparece (E7): mientras está abierto, un almacén con una sola confirmación pequeña ocupa en
disco, además de `krummi.data`, un archivo del diario completo, y la falta de espacio puede aparecer antes que hoy,
al reservar ese archivo.

## Reutilización

- El diario, su organización en archivos numerados, la comprobación de integridad y la compresión de cada
  confirmación, la regla de durable según el diario y la recuperación (J2, J8, J9, J16).
- La cola de peticiones de escritura y su orden estricto de llegada (T3).
- La regla de almacén que no acepta escrituras y quién informa sus fallos (T12, J13).
- La simulación de caídas, cortes de corriente y fallos de las pruebas del Diario, extendida a lo que escribe el
  diario.

## Fuera de alcance

- Fusionar varias transacciones en una, como la fusión de transacciones de RavenDB (fuera de alcance del padre
  raíz): cada confirmación sigue siendo una transacción con su propio turno.
- Varias transacciones de escritura abiertas a la vez (G4).
- Que las lecturas vean confirmaciones que todavía no son durables, en cualquiera de los dos modos.
- Opciones públicas para elegir la escritura directa o la alternativa, o para ajustar el agrupamiento.
- Informar a la aplicación de si se usa la escritura directa, de cuántas confirmaciones se agruparon o de si una
  diferida ya es durable.
- Un límite de espera para que una confirmación sea durable, cancelar esa espera, o detectar un disco que no
  responde.
- Reducir la ventana de J9 dentro del último grupo (E9).
- Escritura directa en `krummi.data` o en `krummi.temp`.
- Guardar en el diario solo las diferencias de cada página, como Voron.
- Detectar discos que ignoran la escritura al medio físico (G2).
- Umbrales de rendimiento (módulo 6).
- Una API asíncrona (padre raíz).

## Dejado a la implementación

- Cuándo empieza a escribirse un grupo y si se espera un poco a que lleguen más confirmaciones, siempre que ninguna
  confirmación de la cola del diario espere sin que se esté escribiendo, y sin tratar distinto los dos modos (E5).
- El tamaño de la cola del diario a partir del cual una confirmación espera (E2).
- El tamaño habitual de los archivos del diario (E7).
- Cómo se comprueba si el sistema admite la escritura directa y qué órdenes del sistema la piden (en la propuesta,
  `FILE_FLAG_NO_BUFFERING` con `FILE_FLAG_WRITE_THROUGH` en Windows, y `O_DIRECT` con `O_DSYNC` en Linux), y cómo
  se lleva al disco en la alternativa.
- Qué hilo escribe el diario y cuál traslada a `krummi.data`, siempre que se cumplan J5 y J13.
- La orden que ejecuta la medición de E16, sus cargas y la forma de su informe.
- Cómo las pruebas detienen la escritura del diario y simulan en ella caídas, cortes de corriente, pánicos y
  fallos, y qué puntos posteriores al punto sin retorno tiene una confirmación.

## Criterios de aceptación

Valen las condiciones de prueba del Diario: los criterios que leen o escriben páginas son pruebas internas de la
biblioteca, y los que necesitan saber cuántas escrituras hizo el diario o el tamaño de sus archivos usan el
conocimiento de la propia implementación. Para retener confirmaciones entre su punto sin retorno y su durabilidad,
o para formar un grupo conocido, la prueba detiene la escritura del diario, espera a que las confirmaciones estén en
la cola del diario y la reanuda; las pruebas saben también cuándo un grupo es durable y fuerzan la alternativa de E6.
"Sin esperar" y "al instante" significan, como en Transacciones, que la llamada devuelve mientras lo que la
bloquearía sigue retenido.

Los criterios del Diario, de Transacciones y del padre raíz siguen valiendo y los del Diario se repiten con
escritura directa y con la alternativa, con estos cambios:

- AC-J7: matar el proceso después de que la última diferida devuelve deja un prefijo (E5).
- AC-T19 y el pánico de AC-J14: valen solo para los puntos anteriores al punto sin retorno; en AC-J14, "después de
  empezar a escribir el diario" se lee "antes de su punto sin retorno". Para los puntos posteriores vale AC-E6: la
  confirmación interrumpida está al reabrir.
- AC6 del padre raíz, AC-T3 y AC-T5: "una lectura abierta después de que la confirmación devuelve" ve sus cambios
  en modo durable; en modo diferido, después de que su grupo es durable (E3).
- AC-J3 (tercera parte) y AC10 del padre raíz: la confirmación íntegra que sigue a la dañada está en un grupo
  posterior; el descarte sin error vale para un daño en el último grupo (E9).
- AC-J9: la confirmación sin cambios termina sin error si la escritura no vio confirmaciones que fallaron (E11).

Y además:

1. **AC-E1** — Dada una confirmación en modo durable retenida después de su punto sin retorno, cuando otro hilo pide
   una escritura, entonces la obtiene sin esperar y ve sus cambios; una lectura abierta en ese momento no los ve; y
   cuando la retenida devuelve sin error, una lectura abierta después por el mismo hilo los ve (E2, E3).
2. **AC-E2** — Dadas 8 confirmaciones en modo durable de 8 hilos retenidas en la cola del diario, que caben en el
   archivo actual del diario, cuando se reanuda, entonces todas devuelven sin error, el diario las escribió en una
   sola escritura, una lectura abierta antes de reanudar no ve ninguna y una abierta después de que la primera
   devuelve las ve todas; y dados 16 hilos que confirman
   a la vez 1.000 escrituras en modo durable cada uno, todas devuelven sin error, nunca hubo dos escrituras abiertas
   a la vez y, tras matar el proceso y reabrir, están todas (E4, G4).
3. **AC-E3** — Dada una secuencia conocida de confirmaciones en los dos modos, que la prueba agrupa en grupos de una,
   dos y tres, cuando se simula un corte de corriente en cada punto en que se escribe o se lleva al disco el diario,
   entonces al reabrir abre sin error y el conjunto de confirmaciones presentes es un prefijo de la secuencia que
   incluye todas las de modo durable que habían devuelto sin error y todos los grupos anteriores al que se escribía
   (E8, E9).
4. **AC-E4** — Dada una confirmación que necesita un archivo nuevo del diario, sin espacio en disco para reservarlo y
   con otra petición de escritura esperando, cuando se confirma, entonces falla con la clase de disco lleno, la
   escritura que obtiene el turno no ve sus cambios, y al liberar espacio una confirmación siguiente termina sin
   error sin reabrir; y si se mata el proceso tras el fallo, la apertura siguiente abre sin error de daño (E2, E7).
5. **AC-E5** — Dadas una diferida que ya devolvió y tres confirmaciones en modo durable retenidas en la cola del
   diario, y una escritura abierta que ve sus cambios, cuando falla la escritura de ese grupo, entonces las tres
   fallan con la clase de entrada y salida, la escritura abierta falla al confirmarse con cambios con esa clase, las
   peticiones de escritura siguientes también, y una lectura nueva no ve ninguna de las cuatro; y al reabrir sin el
   fallo, las presentes son un prefijo de las cuatro y todas las anteriores están (E8).
6. **AC-E6** — Dada una confirmación en modo durable que la prueba interrumpe con un pánico en cada punto posterior
   a su punto sin retorno, cuando su grupo es durable, entonces una lectura nueva ve sus cambios y, al reabrir,
   están (E2).
7. **AC-E7** — Dada la escritura del diario detenida, con una confirmación en modo durable retenida en la cola del
   diario, cuando otro hilo obtiene el turno y confirma una diferida, entonces devuelve sin esperar; la escritura
   siguiente ve las dos; una lectura que ese hilo abre después de que la diferida devuelve no ve ninguna; y cuando
   se reanuda y el grupo es durable, una lectura nueva ve las dos (E3, E5).
8. **AC-E8** — Dadas una diferida y después una en modo durable retenidas en la cola del diario, cuando se reanuda,
   entonces el diario las escribe en una sola escritura, y un corte de corriente simulado después de que la durable
   devuelve conserva las dos; y dada una diferida sola, un corte de corriente simulado después de que su grupo es
   durable, sin otra operación, la conserva (E4, E5).
9. **AC-E9** — Dado un almacén con una confirmación con cambios, cuando se confirman otras que caben en el mismo
   archivo del diario, entonces el tamaño del archivo no cambia; y una confirmación mayor que el tamaño habitual de
   un archivo termina sin error y se recupera tras una caída (E7).
10. **AC-E10** — Dadas diferidas retenidas en la cola del diario, cuando se cierra el almacén y la prueba reanuda la
    escritura durante el cierre, y en otra prueba cuando se deja de usar sin cerrarlo, entonces el cierre no devuelve
    hasta que se escriben, la carpeta queda solo con `krummi.data` y al reabrir están todas; y con un fallo simulado
    en la escritura del grupo que deja de producirse antes del cierre, el cierre termina sin error y al reabrir están
    todas las diferidas; y con el fallo persistente, el cierre falla con la clase de entrada y salida y la apertura
    siguiente sin el fallo abre con un prefijo de ellas (E12).
11. **AC-E11** — Dada una escritura sin cambios que vio una confirmación en modo durable retenida en la cola del
    diario, cuando se confirma en modo durable, entonces no devuelve hasta que la retenida es durable; si su grupo
    falla, falla con la clase de entrada y salida; y en modo diferido, o sin confirmaciones pendientes, termina al
    instante sin error (E11).
12. **AC-E12** — Dada una confirmación en modo durable retenida en la cola del diario, cuando otro hilo llama a la
    operación de J11, entonces no devuelve hasta que se reanuda la escritura y la retenida es durable (E10).
13. **AC-E13** — Dado un fallo simulado al poner en marcha lo que escribe el diario, cuando se abre el almacén,
    entonces falla con la clase de memoria agotada y el almacén queda idéntico byte a byte; y dado un fallo simulado
    de ese mecanismo con confirmaciones esperando, entonces fallan con la clase de entrada y salida, las peticiones
    siguientes también, y al reabrir están todas las que habían devuelto sin error (E13).
14. **AC-E14** — Dado un almacén válido cuyo encabezado lleva el formato `3`, cuando se abre, entonces falla con la
    clase de formato distinto como formato anterior y queda idéntico byte a byte; y la información de un almacén
    nuevo da el formato `4` (E15).
15. **AC-E15** — Dado el informe de cierre, cuando se revisa, entonces contiene las confirmaciones por segundo de la
    medición de E16 con 1, 4 y 16 hilos, con escritura directa y con la alternativa, en Windows y en Linux (E16).
16. **AC-E16** — Dados dos grupos de tres confirmaciones en modo durable que devolvieron sin error, en un almacén
    cerrado por una caída sin trasladar, cuando se altera un byte de la segunda confirmación del último grupo,
    entonces abre sin error con el primer grupo y la primera confirmación del último, sin las otras dos; y cuando se
    altera un byte de una confirmación del primer grupo, la apertura falla con la clase de daño y los archivos del
    almacén quedan idénticos byte a byte (E9).
17. **AC-E17** — Dada una confirmación que espera sitio en la cola del diario llena, con confirmaciones retenidas en
    ella, cuando otro hilo pide una escritura, entonces no la obtiene mientras la primera espera; y cuando un fallo
    simulado al llevar `krummi.data` al disco (J13) deja el almacén sin aceptar escrituras durante esa espera, la confirmación falla con la clase de entrada
    y salida sin aplicar nada, y al reanudar las retenidas se escriben y una lectura nueva las ve (E2).
18. **AC-E18** — Dado un almacén en modo temporal, cuando se confirma una diferida, entonces una lectura abierta
    después de que devuelve la ve (E14).

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Diario, J2: los archivos del diario tienen su tamaño final desde que aparecen (E7).
- Diario, J4: la confirmación cede el turno en su punto sin retorno, antes de ser durable, y desde ahí la ven las
  escrituras siguientes, no las lecturas (E2, E3).
- Diario, J9 y AC10 del padre raíz (y su anotación a G5, G6 y D10): las confirmaciones de un grupo cuentan juntas, y
  la ventana en la que un daño se descarta sin error pasa de la última confirmación al último grupo; un archivo del
  diario sin confirmaciones íntegras cuenta como vacío (E7, E9).
- Diario, J10 y AC-J7: una diferida pasa a ser durable cuando su grupo lo es, puede perderse ante una caída del
  proceso hasta entonces, y las lecturas no la ven hasta que es durable (E3, E5); su lista de cuándo una diferida
  pasa a ser durable, y el punto de *Dejado a la implementación* sobre cuánto tarda, quedan sustituidos por E5.
  "Devuelve sin esperar a ser durable" admite la espera de sitio en la cola del diario o de la reserva de un archivo
  nuevo (E2), y los fallos al escribir el diario se informan en la diferida solo antes de su punto sin retorno (E8).
- Diario, J11: espera también a las confirmaciones que pasaron su punto sin retorno y no han devuelto (E10).
- Diario, J12 y AC-J14: la falta de espacio y de memoria se detectan antes del punto sin retorno, y un pánico
  posterior no deshace la confirmación (E2); una confirmación que falló con la clase de entrada y salida puede estar
  al reabrir si el cierre consigue escribirla (E12), dentro de lo que J12 ya admite ("entera o no está").
- Diario, J13 y AC-J9, y Transacciones, T12: un fallo al escribir un grupo lo informan sus confirmaciones que
  esperan, la confirmación sin cambios en modo durable de una escritura que lo vio (E11) y, si no hay ninguna, las
  operaciones de J13; esa confirmación sin cambios ya no termina siempre sin error (E8, E11).
- Diario, *Fuera de alcance*: agrupar confirmaciones de varios hilos en una sola escritura del diario entra con este
  cambio (E4).
- Transacciones, T2: una escritura ve también las confirmaciones que pasaron su punto sin retorno antes que ella
  aunque no sean durables, y una lectura solo ve confirmaciones durables, también después de que una diferida
  devuelve (E3).
- Transacciones, T10 y AC-T19: un pánico después del punto sin retorno no deshace la confirmación (E2), y una
  confirmación sin cambios en modo durable puede esperar y puede ser la operación que encuentra un fallo de entrada
  y salida (E11).
- Padre raíz, AC6, y Transacciones, AC-T3 y AC-T5: en modo diferido, la lectura ve los cambios cuando su grupo es
  durable, no al devolver (E3).
- Pager, P13: no poder poner en marcha lo que escribe el diario al abrir es memoria agotada (E13).
- Pager, P5; Diario, J3; y cambio de nombre, R5: la versión de formato vigente pasa a ser `4` (E15).

## Referencias

- Padre: [journal.prd.md](journal.prd.md) v1.0
- Ascendencia: [krummi.prd.md](krummi.prd.md) v1.1 (raíz)
- Relacionados: [pager.prd.md](pager.prd.md), [transactions.prd.md](transactions.prd.md),
  [rename-to-krummi.prd.md](rename-to-krummi.prd.md)
- Propuesta de origen: informe de cierre de los follow-ups del diario, `odd/tasks/journal-followups-report.md`
- PostgreSQL, *WAL configuration* (`commit_delay`, `synchronous_commit`): <https://www.postgresql.org/docs/current/wal-configuration.html>
- Voron, en el repositorio de RavenDB: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron>

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| b7c9add8 | 2026-10-23 10:17 UTC+01:00 | 42 min | 0 min | 42 min | 23 min | 19 min | 7 |

<!-- prd-time: [{"session": "b7c9add8-aa47-43e9-8507-11b8b6891d58", "start": "2026-10-23T09:17:28.630000+00:00", "wall": 2549, "discarded": 0, "agent": 1403, "human": 1145, "interventions": 7, "idle_minutes": 10}] -->
