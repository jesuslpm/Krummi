# Diario y recuperación de MuninnDB — PRD

**Versión**: 1.0
**Fecha**: 2026-10-22
**Documento padre**: [MuninnDB, motor de almacenamiento embebido inspirado en Voron — PRD](muninndb.prd.md) (v1.1)

## Objetivo y contexto

Construir el **diario y la recuperación**, el módulo 3 del [reparto del padre](muninndb.prd.md#reparto-en-prd-hijos):
un diario de escritura anticipada que hace durables las confirmaciones (G2), con compresión; el traslado de lo
confirmado al archivo de datos; y la recuperación automática al abrir un almacén que se cerró por una caída (G5).

Al cerrar este módulo, una confirmación durable que termina sin error sobrevive a una caída del proceso, del sistema
o a un corte de corriente, y la apertura siguiente devuelve el almacén, sin intervención de la aplicación, a un
estado con todo lo confirmado y sin partes de confirmaciones a medias. Termina así la excepción temporal que el
módulo 2 declaró a G1, G2, G5 y G12 (T9 de [transactions.prd.md](transactions.prd.md)).

**Por qué ahora**: es la garantía central del producto (objetivo 1 del padre) y la base de los módulos que copian el
almacén en uso (backup, módulo 10). Además, el módulo 2 dejó aquí cuándo se reescribe el encabezado (T17) y llevar a
archivos las versiones de páginas que todavía no están en `muninn.data` (T14, T16).

Este PRD hereda todo lo del padre, lo del [Pager](pager.prd.md) (P1–P17) y lo de las
[Transacciones](transactions.prd.md) (T1–T18); aquí solo se añade lo propio del módulo. Lo que cambia de reglas
anteriores está declarado en sus decisiones y anotado en los documentos afectados (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

### Decisiones de producto

| ID | Decisión |
|---|---|
| J1 | **Superficie pública del módulo**: al confirmar una transacción de escritura se puede elegir el modo diferido (J10); el durable es el predeterminado. El almacén ofrece una operación para hacer durable todo lo confirmado (J11). El diario, el traslado y la recuperación no tienen API pública: la recuperación ocurre al abrir. |
| J2 | **Archivos del almacén**: además de `muninn.data`, la carpeta del almacén puede contener los archivos del diario, llamados `muninn.journal.` seguido de un número de 19 cifras decimales con ceros a la izquierda (por ejemplo `muninn.journal.0000000000000000001`), y la subcarpeta `muninn.temp` del área temporal (J7). Esos nombres quedan reservados (P3): en la carpeta de un almacén, las entradas con esos nombres son del almacén, que las crea y las borra cuando ya no las necesita; los demás nombres siguen la regla de los ajenos (P7). Una apertura que termina sin error deja en la carpeta solo `muninn.data`, además de los ajenos; los archivos del diario y `muninn.temp` aparecen después, con las confirmaciones. Una carpeta sin `muninn.data`, o cuyo `muninn.data` no empieza por la marca, sigue la regla del padre: si tiene alguna entrada, incluidos archivos del diario o `muninn.temp` y también junto a `muninn.data.new` (P8), no es un almacén y no se toca nada. Precisa P3 y P7. |
| J3 | **Formato `2`**: un motor sin diario no puede abrir con seguridad un almacén con confirmaciones pendientes en el diario, así que la versión de formato vigente pasa a ser `2` (D7 del padre). Un almacén de formato `1` es de formato anterior aunque se cerrara sin error. Precisa P5. |
| J4 | **Confirmación durable** (G2): confirmar en modo durable devuelve sin error solo cuando la confirmación es durable: sobrevive a una caída del proceso, del sistema o a un corte de corriente aunque todavía no esté en `muninn.data`. Hasta entonces sus cambios no son visibles para ninguna otra transacción y la siguiente petición de escritura no obtiene su turno (T2, T3). |
| J5 | **Traslado a `muninn.data`**: lo confirmado pasa a `muninn.data` sin que la aplicación lo pida y sin esperar al cierre, en cuanto ninguna lectura abierta necesita la versión anterior de esas páginas (T14), sin bloquear a los lectores ni al escritor. Solo se traslada lo que ya es durable: si hay que trasladar una confirmación diferida, antes se hace durable, con todas las anteriores (J10). Así, sin lecturas que lo impidan, el diario no crece sin límite con el almacén abierto; mientras una lectura impide trasladar, crece sin más límite que el disco (G7). Trasladar nunca hace crecer `muninn.data`: crece al reservar páginas dentro de la escritura (T14), así que la falta de espacio y el tamaño máximo se detectan en la escritura o en su confirmación (G7, P10, P11), nunca después de que la confirmación devuelve. |
| J6 | **Encabezado**: se reescribe cuando avanza lo trasladado y llevado al disco. Sustituye a T17; AC-T21 deja de valer. |
| J7 | **Área temporal en archivos**: las versiones confirmadas de páginas que todavía no están en `muninn.data` (T14, T16) se guardan en archivos dentro de `muninn.temp`, no en la memoria del proceso. Cuentan para el tamaño máximo como en T14, pero una escritura nunca falla por tamaño máximo a causa de versiones que ninguna lectura necesita: antes se trasladan. Las referencias de las lecturas (D11, T16) pueden apuntar a ellos y siguen valiendo, con la misma dirección y el mismo contenido, mientras su transacción esté abierta, aunque el área crezca. Un fallo al crearlos o escribirlos hace fallar la operación o la confirmación que los necesita con su clase (acceso, disco lleno, memoria agotada o entrada y salida), sin aplicar nada y sin que aparezca al reabrir, y sigue T11 y T12. Como las lecturas no copian, un fallo físico del disco al leerlos termina el proceso, como en P17. `muninn.temp` se borra al cerrar; el que deja una caída lo borra la apertura siguiente en su paso 8, después de comprobar el diario, haya o no algo que recuperar. Nunca se lee para recuperar. Precisa T14, T16 y P17. |
| J8 | **Recuperación** (G5, paso 8 del orden de apertura del padre): si el diario tiene confirmaciones que no constan como trasladadas, la apertura las aplica a `muninn.data` en el orden en que se confirmaron; en todo caso, termina sin archivos del diario ni `muninn.temp`, que solo se borran después de recuperar: una apertura que falla no borra ninguno de los dos. Un diario dañado (J9) se informa sin tocar el almacén. Si un archivo del diario no se puede leer por permisos, o un archivo del diario o `muninn.temp` no se puede borrar, la apertura falla con la clase de acceso. Si la recuperación falla por falta de espacio o de memoria, o por un fallo de entrada y salida, la apertura falla con su clase y la siguiente la repite (G5). Cuando se dan varias, gana la primera de este orden: daño, acceso, y después la que se encuentre de las demás. Con una de las dos copias del encabezado dañada (P6), la recuperación trae igualmente todo lo confirmado. La recuperación hace crecer `muninn.data` lo que necesitan las páginas de las confirmaciones recuperadas, aunque supere el tamaño máximo pedido al abrir, y nunca lo encoge; el almacén abre entonces como uno mayor que el máximo (P10). El crecimiento que hizo una escritura que no llegó a confirmarse puede no sobrevivir a una caída (P11). Precisa P10 y P11. |
| J9 | **Diario dañado**: una confirmación del diario es **durable según el diario** si alguna confirmación íntegra posterior se escribió cuando ella ya era durable. Una confirmación incompleta, que no supera su comprobación de integridad, o que falta porque falta o está vacío el archivo del diario donde iba, es una confirmación que no terminó si no es durable según el diario: se descarta junto con todas las posteriores (G5, J10). Si es durable según el diario, el diario está dañado: la apertura falla con la clase de daño sin tocar el almacén (G6), con el mismo error que un almacén que no se puede abrir por daño. También es daño que una entrada con nombre reservado no sea del tipo esperado (un archivo donde va una carpeta o al revés). Lo que ya consta como trasladado no se lee y su daño no se informa. Consecuencia: si se daña la última confirmación durable sin ninguna confirmación posterior, no se distingue de una que no terminó y se descarta sin error. Precisa G5, G6 y D10 del padre. |
| J10 | **Modo diferido** (G11): una confirmación diferida devuelve sin esperar a ser durable; desde ese momento es visible (T2) y sobrevive a una caída del proceso, pero no necesariamente a una caída del sistema o a un corte de corriente. Tras una de esas, la recuperación devuelve el almacén a un prefijo de las confirmaciones: todas las durables y, de las diferidas posteriores a la última durable, las primeras en orden hasta algún punto, nunca una parte de una ni una sin todas las anteriores. Una diferida pasa a ser durable, con todas las anteriores, cuando termina una confirmación durable con cambios posterior, la operación de J11, su traslado a `muninn.data` (J5) o un cierre sin error, incluido dejar de usar el almacén sin cerrarlo (J14). Una confirmación durable sin cambios no escribe nada (T10) y no hace durables las anteriores. La falta de espacio y los fallos al escribir el diario se informan en la confirmación diferida igual que en la durable (J12). El modo no cambia la cola (T3), los límites de espera (T4) ni la precedencia de T13. |
| J11 | **Hacer durable lo confirmado**: la operación devuelve cuando todas las confirmaciones que habían devuelto antes de llamarla son durables; las que están en curso mientras tanto no se garantizan. No entra en la cola de escritores (T3), no espera a la escritura abierta y no tiene límite de espera. En un almacén que no acepta escrituras (T12) falla al instante con la clase de entrada y salida, aunque no haya nada pendiente. Si no, y no hay nada pendiente, devuelve sin error sin escribir. Si falla al llevar el diario al disco, falla con esa clase y el almacén deja de aceptar escrituras (G12); las diferidas pendientes siguen la regla de prefijo de J10. |
| J12 | **Fallos al escribir el diario**: si falta espacio, la confirmación falla con la clase de disco lleno, no aplica nada y nunca aparece al reabrir (G7). Si falta memoria, igual con la clase de memoria agotada, y el almacén sigue aceptando escrituras (P13). Si falla llevar el diario al disco, crear sus archivos o escribirlo por otra causa, la confirmación falla con la clase de entrada y salida, no es visible, el almacén deja de aceptar escrituras (T12) y al reabrir la confirmación está entera o no está, como una confirmación interrumpida (G1). Lo que una confirmación que falla, o que se interrumpe con un pánico (T10), deja a medias en el diario nunca impide recuperar las confirmaciones siguientes, tampoco tras un corte de corriente, y solo aparece al reabrir en el caso de entrada y salida. |
| J13 | **Fallos fuera de una operación**: un fallo de entrada y salida al trasladar o al llevar `muninn.data` al disco, cuando no ocurre dentro de una operación de la aplicación, deja el almacén sin aceptar escrituras (T12) desde ese momento: lo informan con esa clase las peticiones de escritura que esperan y las nuevas, la confirmación con cambios de la escritura abierta, la operación de J11 y el cierre; las lecturas siguen funcionando y una confirmación sin cambios sigue terminando sin error (T10). Al reabrir, la recuperación trae todo lo confirmado. Precisa T12 (quién informa el fallo) y sustituye la parte de T9 y T12 según la cual lo confirmado podía faltar al reabrir. |
| J14 | **Cierre**: cerrar sin error deja todo lo confirmado en `muninn.data` llevado al disco, el encabezado al día y la carpeta sin archivos del diario ni `muninn.temp`, así que la apertura siguiente no recupera nada. Dejar de usar el almacén sin cerrarlo hace lo mismo (T9, P9). Si el cierre falla (P14, T12), los archivos del diario y `muninn.temp` pueden quedar, el bloqueo se libera y la apertura siguiente recupera todo lo confirmado (J8). |
| J15 | **Tamaño máximo** (P10): el diario no cuenta; las versiones de `muninn.temp` cuentan como en J7. La información del almacén (P12) sigue describiendo solo `muninn.data`. |
| J16 | **Compresión**: lo que se guarda en el diario ocupa menos que las páginas que cambia cuando sus datos se comprimen bien; los datos que no se comprimen se guardan igualmente, sin error. La compresión no cambia nada de lo que leen las transacciones ni de lo que contiene `muninn.data`. |
| J17 | **Modo temporal**: no tiene diario, `muninn.temp` ni recuperación (G9). El modo diferido y la operación de J11 se aceptan sin error y no hacen nada más. |

### Estado de partida

Verificado el 2026-10-22:

| Aspecto | Situación |
|---|---|
| MuninnDB | El Pager (módulo 1) y las transacciones (módulo 2) están implementados. No hay diario: lo confirmado se escribe en su sitio en `muninn.data` a más tardar al cerrar o al dejar de usar el almacén (T9). Las versiones confirmadas que todavía no están en `muninn.data` viven en la memoria del proceso. El encabezado guarda la versión de formato (`1`), el tamaño de página y una revisión que solo se escribe al crear el almacén (T17). Las clases de error son las del padre más tamaño máximo, memoria agotada, tiempo de espera agotado y uso incorrecto. No hay modos de confirmación ni operación para llevar lo confirmado al disco. |
| Transacciones | El informe de cierre del módulo 2 deja pendiente la memoria que retiene una lectura larga sin tamaño máximo, y que los archivos del área temporal llegan con este módulo. Antes de fallar con tamaño máximo, una escritura ya lleva a su sitio las versiones que ninguna lectura necesita. |
| PRD padre | [muninndb.prd.md](muninndb.prd.md) v1.1 asigna a este módulo los modos no durables (G11) y el disco lleno durante la recuperación, y le anota la excepción temporal de T9, que termina aquí. |
| Voron (referencia) | Diario en archivos numerados con escritura directa al medio físico y comprimido; cada transacción con su comprobación de integridad; traslado al archivo de datos en segundo plano cuando ningún lector necesita la versión anterior; el encabezado registra el último diario y la última transacción trasladados; la recuperación aplica el diario desde ahí y descarta una cola a medias; modos de confirmación seguro, diferido y sin diario; área temporal en archivos de una carpeta propia que se borran al abrir. |

## Actores

- **Desarrollador integrador**: confirma transacciones en modo durable o diferido, pide hacer durable lo confirmado,
  abre almacenes que se cerraron por una caída y trata los errores por su clase.

El operador y la herramienta de línea de comandos no intervienen en este módulo (D9 del padre llega con el 12).

## Flujo

1. La aplicación abre un almacén (flujo del [Pager](pager.prd.md#flujo)). En el paso 8 del orden del padre, la
   apertura comprueba el diario, recupera las confirmaciones que no constan como trasladadas y borra el diario y el
   `muninn.temp` que haya dejado una caída (J7, J8).
2. La aplicación abre transacciones (flujo de [Transacciones](transactions.prd.md#flujo)) y confirma las de escritura
   en modo durable o diferido (J4, J10).
3. Cuando lo necesita, pide hacer durable todo lo confirmado (J11).
4. Sin intervención de la aplicación, lo confirmado pasa a `muninn.data` en cuanto ninguna lectura necesita la
   versión anterior (J5), y el encabezado registra el avance (J6).
5. La aplicación cierra el almacén: todo lo confirmado queda en `muninn.data` y la carpeta sin diario ni
   `muninn.temp` (J14).

## Validaciones y mensajes

Valen las del padre, las del Pager y las de Transacciones. Este módulo no añade clases de error ni mensajes de la
herramienta. Resumen de las situaciones nuevas, solo de la biblioteca; la regla completa está en la decisión citada:

| Situación | Regla | Clase de error |
|---|---|---|
| Almacén de formato `1` | La apertura falla sin tocarlo (J3). | Formato distinto |
| Carpeta sin `muninn.data`, o con `muninn.data` sin la marca, con archivos del diario o `muninn.temp` | No es un almacén; no se toca nada (J2). | No es un almacén |
| Confirmación incompleta, dañada o que falta, durable según el diario; o entrada reservada de otro tipo | La apertura falla sin tocar el almacén (J9). | Daño |
| Confirmación incompleta, dañada o que falta, no durable según el diario | Se descarta con las posteriores; la apertura sigue (J9). | — |
| Falta espacio o memoria, o falla el sistema, durante la recuperación | La apertura falla sin borrar el diario; la siguiente repite (J8). Daño y acceso ganan. | Disco lleno, memoria agotada o entrada y salida |
| Un archivo del diario no se puede leer, o el diario o `muninn.temp` no se pueden borrar, al abrir | La apertura falla (J8). | Acceso |
| Falta espacio o memoria al escribir el diario al confirmar | La confirmación falla y nunca aparece al reabrir (J12). | Disco lleno o memoria agotada |
| Otro fallo al crear, escribir o llevar al disco el diario al confirmar | La confirmación falla; el almacén deja de aceptar escrituras; al reabrir está entera o no está (J12). | Entrada y salida |
| Fallo al crear o escribir `muninn.temp` | La operación o confirmación falla sin aplicar nada (J7). | Acceso, disco lleno, memoria agotada o entrada y salida |
| Fallo del traslado o al llevar `muninn.data` al disco | El almacén deja de aceptar escrituras; lo informan las operaciones de J13. | Entrada y salida |
| Hacer durable lo confirmado en un almacén que no acepta escrituras, o fallo al hacerlo | Falla; en el segundo caso el almacén deja de aceptar escrituras (J11). | Entrada y salida |

## Salida

El módulo no escribe nada en pantalla y no cambia la forma de la información del almacén (P12); la versión de
formato que informa pasa a ser `2`.

En disco, la carpeta de un almacén recién abierto, o cerrado sin error, contiene solo `muninn.data` y los ajenos
(J2, J14). Mientras está abierto puede contener además archivos del diario y `muninn.temp`. Ejemplo, con un
almacén abierto en el que una lectura larga impide trasladar lo confirmado:

```text
muninn.data
muninn.journal.0000000000000000001
muninn.temp\
```

## Reutilización

- El Pager: páginas, comprobación de integridad, encabezado doble, crecimiento, tamaño máximo y memoria agotada
  (P6, P10, P11, P13).
- Las transacciones: la cola de escritores, las instantáneas, el área temporal, la regla de cuándo una versión se
  escribe en su sitio y la de llevar a su sitio lo que ninguna lectura necesita antes de fallar por tamaño máximo
  (T3, T14, T16), y las reglas de fallos dentro de una escritura y de almacén que no acepta escrituras (T11, T12).
- Las clases de error existentes; este módulo no añade ninguna.

## Fuera de alcance

- Cifrado del diario y de `muninn.temp` (módulo 9).
- Conservar el diario para backups incrementales (módulo 10).
- Recuperar desde la herramienta de línea de comandos (módulo 12, D9), y abrir un almacén que necesita recuperación
  sin modificarlo.
- Un modo sin diario como el *Danger* de Voron, en el que una caída puede dejar el almacén a medias.
- Un modo de confirmación predeterminado para todo el almacén u opción de apertura para elegirlo (J1, J10).
- Informar a la aplicación de que hubo recuperación, de cuánto recuperó, o de si una confirmación diferida ya es
  durable.
- Reparar un diario dañado o abrir un almacén descartando la parte dañada (rescate de datos, módulo 12).
- Un límite de tamaño del diario mientras una lectura larga impide trasladar, o del tiempo que tarda la
  recuperación (J5).
- Un límite de espera o la cancelación de la operación de J11.
- Una API para forzar el traslado o la reescritura del encabezado.
- Elegir otra carpeta para el diario o para `muninn.temp`.
- Borrar, recortar o sustituir archivos del diario con el almacén cerrado, o traer los de otro almacén: solo se
  detecta lo que J9 llama daño; borrar los últimos archivos del diario, o copiar `muninn.data` sin su diario, no se
  detecta.
- Enlaces simbólicos u otras entradas especiales con nombres reservados.
- Agrupar confirmaciones de varios hilos en una sola escritura del diario.
- Migrar almacenes de formato `1` (D7).

## Dejado a la implementación

- El algoritmo de compresión y la organización interna del diario y de `muninn.temp` (padre), incluido cómo se
  reconoce que una confirmación es durable según el diario (J9).
- Cuántos archivos del diario hay, su tamaño, el número del primero y cuándo se crea uno nuevo, siempre que los
  números crezcan de uno en uno; y cuándo se borra o se reutiliza la parte ya trasladada, siempre que se cumpla J8
  con una copia del encabezado dañada.
- Cuánto tarda el traslado tras una confirmación y cuándo se reescribe el encabezado, siempre que se cumpla J5.
- En modo diferido, cuánto tiempo puede pasar hasta que una confirmación es durable sin una durable con cambios,
  la operación de J11 ni un cierre.
- Dónde viven los cambios de la escritura abierta antes de confirmarla, y los nombres de los archivos dentro de
  `muninn.temp`.
- Los nombres de los tipos, funciones y campos de la API pública, que siguen las convenciones de Rust y están en
  inglés, y cómo se indica el modo al confirmar.
- Cómo las pruebas simulan caídas, cortes de corriente, fallos de escritura y de llevar al disco, falta de espacio
  y de memoria, y cómo detienen el traslado.

## Criterios de aceptación

Los criterios que necesitan leer o escribir páginas son pruebas internas de la biblioteca sobre su API interna de
transacciones, como en el módulo 2; las que necesitan conocer cómo está organizado el diario (dónde empieza una
confirmación, en qué archivo está) usan el conocimiento de la propia implementación, como las del Pager con el
encabezado. Para que el diario tenga confirmaciones sin trasladar, la prueba mantiene abierta una lectura o detiene
el traslado. Una caída del proceso se simula matando un proceso hijo. Un corte de corriente se simula descartando
todo lo que no se llevó al medio físico, incluidas las creaciones, borrados y cambios de tamaño de archivos, y
dejando a medias la última escritura de cada archivo.

Este módulo comprueba a nivel de biblioteca estos criterios del padre, adaptados a lo que existe en él:

- **AC1, AC2, AC3 y AC4**: tal cual, con páginas como datos y con el diario sin trasladar.
- **AC5**: tal cual, interrumpiendo la recuperación con una caída del proceso y con un corte de corriente simulado.
- **AC6, AC7 y AC8**: tal cual.
- **AC10**: alterando cada byte de un almacén cerrado por una caída con confirmaciones durables sin trasladar, sus
  archivos del diario incluidos, y abriéndolo y leyendo todas las páginas: cada alteración produce un error de la
  clase de daño, ninguna diferencia, o, solo si la alteración está en la última confirmación del diario, el descarte
  de esa confirmación (J9); nunca datos que no se escribieron.
- **AC11**: con una confirmación que no cabe en el diario por falta de espacio (J12).
- **AC12**: con un fallo simulado al llevar el diario al disco en una confirmación durable; al reabrir están todas
  las confirmaciones durables anteriores y la que falló está entera o no está (J12).
- **AC14**: el modo temporal no crea diario ni `muninn.temp`.
- **AC18**, **AC20** y **AC21**: tal cual; AC20 también con versiones en `muninn.temp`.

Los criterios de Transacciones siguen valiendo, con estos cambios: AC-T21 deja de valer (J6); la adaptación de AC12
y AC-T20, que inyectan el fallo en la escritura de lo confirmado en `muninn.data`, se sustituyen por AC12 y AC-J9; y
AC-T15 y AC-T16 se repiten con las versiones en `muninn.temp`.

Y además:

1. **AC-J1** — Dada una carpeta que no existe, cuando se abre, entonces contiene solo `muninn.data`; con el traslado
   detenido por la prueba, tras una confirmación con cambios contiene además archivos del diario; y tras cerrar sin
   error, solo `muninn.data`, y al reabrir se leen todos los cambios (J2, J14).
2. **AC-J2** — Dado un almacén con una lectura abierta que impide trasladar, cuando se confirman escrituras y se
   mata el proceso, entonces al reabrir se leen todas y la carpeta contiene solo `muninn.data` (J8).
3. **AC-J3** — Dado un diario cuya última confirmación está recortada, y en otra prueba alterada en un byte, cuando
   se abre, entonces abre sin error sin ninguno de sus cambios y con todos los de las anteriores; y dado un diario
   con una confirmación durable alterada en un byte seguida de otra durable íntegra, la apertura falla con la clase
   de daño y los archivos del almacén, `muninn.temp` incluido, quedan idénticos byte a byte (J9).
4. **AC-J4** — Dado un almacén cerrado por una caída cuyo diario ocupa varios archivos con confirmaciones durables,
   cuando se borra uno que no es el último y se abre, entonces falla con la clase de daño y el resto queda idéntico
   byte a byte; y cuando se sustituye `muninn.temp` por un archivo, o un archivo del diario por una carpeta, también
   (J9).
5. **AC-J5** — Dado un almacén cuyas confirmaciones hicieron crecer `muninn.data`, cerrado por un corte de corriente
   simulado que pierde ese crecimiento, y un disco sin espacio para que vuelva a crecer, cuando se abre, entonces
   falla con la clase de disco lleno y los archivos del diario siguen presentes; cuando se libera espacio y se vuelve
   a abrir, abre con todo lo confirmado; y si además el diario tiene una confirmación durable dañada, falla con la
   clase de daño (J8).
6. **AC-J6** — Dado un almacén válido cuyo encabezado lleva el formato `1`, cuando se abre, entonces falla con la
   clase de formato distinto como formato anterior y queda idéntico byte a byte (J3).
7. **AC-J7** — Dada la secuencia de confirmaciones durable, diferida, diferida, durable, diferida, diferida, cuando
   se simula un corte de corriente en cada punto en que se escribe o se lleva al disco el diario, entonces al
   reabrir abre sin error y el conjunto de confirmaciones presentes es un prefijo de la secuencia que incluye todas
   las durables que habían devuelto; cuando se mata el proceso después de que la última diferida devuelve, están
   todas; y cuando antes del corte se llamó a J11 o se cerró sin error, están todas (J9, J10, J11).
8. **AC-J8** — Dada la operación de J11, cuando se llama sin nada pendiente, devuelve sin error; cuando se llama
   desde otro hilo con una escritura abierta y retenida por la prueba, devuelve sin esperarla; con un fallo simulado
   al llevar el diario al disco, falla con la clase de entrada y salida y las peticiones de escritura siguientes
   fallan con esa clase; en un almacén que ya no acepta escrituras y sin nada pendiente, falla al instante con esa
   clase; y en modo
   temporal, devuelve sin error, igual que una confirmación diferida (J11, J17).
9. **AC-J9** — Dado un fallo simulado de entrada y salida en el traslado, cuando ocurre, entonces la siguiente
   petición de escritura falla con esa clase, una confirmación sin cambios termina sin error y las lecturas
   funcionan; el cierre con el fallo persistente falla con esa clase, y al reabrir sin el fallo están todas las
   confirmaciones (J13, J14).
10. **AC-J10** — Dado un almacén cerrado por una caída que necesita recuperación, cuando se abre con un tamaño
    máximo menor que el de su `muninn.data`, entonces abre con todo lo confirmado y el primer crecimiento posterior
    falla con la clase de tamaño máximo (J8, P10).
11. **AC-J11** — Dada una lectura larga mientras se confirman escrituras, cuando se observa la carpeta, entonces
    contiene `muninn.temp` con archivos; al terminar las lecturas y cerrar, desaparece; y tras una caída con él
    presente, la apertura siguiente lo borra, también cuando no hay nada que recuperar (J7).
12. **AC-J12** — Dado un almacén cerrado por una caída con confirmaciones sin trasladar después de que el
    encabezado se haya reescrito varias veces, cuando se daña una copia del encabezado, en una prueba la que usa la
    apertura y en otra la otra, entonces abre con todo lo confirmado (J8, P6).
13. **AC-J13** — Dada una confirmación durable que la prueba detiene después de escribir el diario y antes de que
    sea durable, cuando otro hilo abre una lectura, entonces no ve sus cambios; y cuando se simula un corte de
    corriente en ese punto, al reabrir la confirmación está entera o no está (J4).
14. **AC-J14** — Dado un disco sin espacio para el diario, cuando se confirma, entonces falla con la clase de disco
    lleno; y cuando se libera espacio, se confirma de forma durable otra escritura y se mata el proceso, o en otra
    prueba se simula un corte de corriente, al reabrir está la segunda y no la primera; lo mismo cuando la primera
    confirmación se interrumpe con un pánico después de empezar a escribir el diario (J12).
15. **AC-J15** — Dada una confirmación que cambia 100 páginas llenas de un mismo byte, cuando se mide lo que ocupa
    en el diario según la propia implementación, entonces es menos que esas páginas; y dada una con bytes
    aleatorios, se confirma sin error y se recupera tras una caída (J16).
16. **AC-J16** — Dadas una carpeta con solo un archivo del diario, otra con un archivo del diario y
    `muninn.data.new`, y otra con solo `muninn.temp`, cuando se abren, entonces fallan con la clase de error de que
    no es un almacén y quedan idénticas byte a byte (J2).
17. **AC-J17** — Dado un traslado que la prueba detiene, cuando otro hilo confirma una escritura y abre y lee una
    lectura, entonces las dos terminan sin esperar al traslado (J5).
18. **AC-J18** — Dado un almacén con un tamaño máximo que solo deja sitio para las páginas que cambia una
    escritura, cuando se confirman sin lecturas abiertas 2.000 escrituras que cambian esas mismas páginas, entonces
    todas se confirman sin error; y, esperando en cada medida a que termine el traslado, el tamaño en disco de los
    archivos del diario tras las 2.000 no es mayor que tras las 1.000 primeras (J5, J7, J15).

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Padre, G5, G6 y D10, y AC10: una confirmación dañada, incompleta o que falta que no es durable según el diario se
  descarta sin error como una que no terminó, y la última confirmación durable dañada sin ninguna posterior no se
  distingue de ella (J9).
- Padre, excepción temporal a G1, G2, G5 y G12 (T9): termina con este módulo (J4, J13).
- Pager, P3 y P7: los nombres de J2 son del almacén, que los crea y los borra (J2).
- Pager, P5: la versión de formato vigente pasa a ser `2` (J3).
- Pager, P10 y P11: la recuperación hace crecer `muninn.data` lo que necesitan las confirmaciones recuperadas
  aunque supere el máximo pedido, y el crecimiento de una escritura que no llegó a confirmarse puede no sobrevivir a
  una caída (J8).
- Pager, P17: vale también para los archivos de `muninn.temp` (J7).
- Transacciones, T9 y T12: lo confirmado ya no puede faltar al reabrir, y un fallo del traslado lo informan las
  operaciones de J13.
- Transacciones, T14, T16 y el punto de *Fuera de alcance* sobre el área temporal: las versiones confirmadas que no
  están en `muninn.data` viven en `muninn.temp` (J7).
- Transacciones, T17 y AC-T21: el encabezado se reescribe cuando avanza el traslado (J6).

## Referencias

- Padre y raíz: [muninndb.prd.md](muninndb.prd.md) v1.1
- Módulos anteriores: [pager.prd.md](pager.prd.md), [transactions.prd.md](transactions.prd.md)
- Voron, en el repositorio de RavenDB: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron>

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| cdcd1dd6 | 2026-10-22 20:45 UTC+01:00 | 31 min | 0 min | 31 min | 25 min | 6 min | 5 |

<!-- prd-time: [{"session": "cdcd1dd6-3127-46eb-9d09-5e2bd3a6b0a9", "start": "2026-10-22T19:45:44.292000+00:00", "wall": 1853, "discarded": 0, "agent": 1515, "human": 338, "interventions": 5, "idle_minutes": 10}] -->
