# Transacciones de MuninnDB — PRD

**Versión**: 1.1
**Fecha**: 2026-10-22
**Documento padre**: [MuninnDB, motor de almacenamiento embebido inspirado en Voron — PRD](muninndb.prd.md) (v1.1)

## Objetivo y contexto

Construir las **transacciones**, el módulo 2 del [reparto del padre](muninndb.prd.md#reparto-en-prd-hijos):
transacciones de lectura y de escritura sobre un almacén abierto, con aislamiento por instantánea (G3), un solo
escritor (G4) y lectores concurrentes que no se bloquean con él.

Al cerrar este módulo, el desarrollador integrador puede abrir transacciones de lectura desde varios hilos y una
de escritura cada vez, confirmarlas y abortarlas. Todavía no puede guardar datos en ellas: las estructuras de datos
llegan con los árboles B+ (módulo 5). Este módulo fija el marco que todas ellas usarán: quién espera a quién, qué
ve cada transacción y cuánto vive lo que lee.

**Por qué ahora**: todos los módulos siguientes leen y escriben dentro de una transacción, y el padre deja aquí lo
que G4 no decide (orden y límites de espera, segunda escritura de un mismo hilo), si una transacción puede pasar de
un hilo a otro, el límite de lectores y qué pasa al cerrar un almacén con transacciones abiertas.

Este PRD hereda todo lo del padre y lo del [Pager](pager.prd.md) (P1–P17); aquí solo se añade lo propio del
módulo. La excepción temporal a garantías del padre y la precisión de D11 están declaradas en T9 y T16 y anotadas
en él (ver [Anotaciones al padre](#anotaciones-al-padre)).

### Decisiones de producto

| ID | Decisión |
|---|---|
| T1 | **Superficie pública del módulo**: abrir una transacción de lectura, abrir una de escritura (con un límite de espera opcional, T4), confirmar una de escritura y abortarla. Terminar una transacción de lectura es dejar de usarla. Leer, reservar y escribir páginas dentro de una transacción es interno de la biblioteca, como en P1 del Pager: lo usan las pruebas y los módulos siguientes, no las aplicaciones. |
| T2 | **Qué ve cada transacción**: una de lectura ve lo confirmado hasta el momento en que se abre (G3). Una de escritura ve lo confirmado hasta el momento en que obtiene su turno, no cuando lo pidió, más sus propios cambios. Una confirmación es visible para toda transacción que se abra después de que la confirmación devuelva, incluidas las del mismo hilo; una lectura que se abre mientras se confirma ve todos los cambios de esa confirmación o ninguno. Un hilo con una lectura abierta puede pedir una escritura sobre el mismo almacén; su lectura conserva su instantánea. |
| T3 | **Un escritor y orden estricto de llegada** (G4): mientras hay una transacción de escritura abierta o alguna petición esperando, las peticiones nuevas se ponen al final de la cola y se atienden en el orden en que llegaron; ninguna se adelanta, ni siquiera en el instante en que termina una escritura. Una petición que deja de esperar (por su límite, T4, o por T12) sale de la cola sin cambiar el orden de las demás. Ninguna lectura espera a la escritura ni la escritura a las lecturas. |
| T4 | **Límite de espera**: por defecto, una petición de escritura espera lo que haga falta. Quien la pide puede indicar en esa llamada un límite de tiempo; si vence antes de obtener su turno, la petición falla con la clase nueva de error de tiempo de espera agotado, sale de la cola y no tiene ningún efecto. Un límite de cero no espera: obtiene la escritura si no hay ninguna abierta ni peticiones en cola y, si no, falla con esa clase sin llegar a encolarse. Un límite tan grande que no se puede contar desde el momento actual falla con la clase de uso incorrecto (T5), sin encolarse; cualquier límite de hasta 24 horas se admite siempre, y el máximo que admite la API siempre falla. |
| T5 | **Segunda escritura del mismo hilo**: si un hilo pide una transacción de escritura sobre un almacén en el que ya tiene una abierta, la petición falla sin esperar, con la clase nueva de error de **uso incorrecto**: una clase de la biblioteca para llamadas hechas en un orden o con valores que la biblioteca no admite, distinta de las opciones no válidas al abrir y del uso incorrecto de la herramienta. La transacción abierta no se ve afectada y sigue pudiendo confirmarse o abortarse. Una petición de escritura sobre otro almacén no cuenta. |
| T6 | **Hilos**: una transacción de lectura puede pasarse a otro hilo y compartirse entre varios a la vez, que ven la misma instantánea, siempre dentro de la vida del almacén (T8), es decir, con hilos que terminan antes de cerrarlo. Una transacción de escritura se queda en el hilo que la abrió: el compilador impide pasarla o compartirla. El almacén se puede usar a la vez desde varios hilos. |
| T7 | **Sin límite de lectores**: no hay un número máximo de transacciones de lectura abiertas a la vez; el único límite es la memoria (P13). |
| T8 | **Vida de las transacciones**: una transacción depende del almacén abierto del que salió. Cerrar el almacén mientras queda alguna abierta, o usar una después de cerrarlo, no compila. Confirmar y abortar terminan la transacción: confirmarla o abortarla otra vez, o usarla después, no compila. Las referencias que devuelve una lectura valen mientras su transacción esté abierta, y usarlas después no compila (D11 del padre); dentro de una escritura, las referencias a sus páginas valen hasta el siguiente cambio de esa misma transacción, y usarlas después tampoco compila. |
| T9 | **Confirmar sin diario** (excepción temporal hasta el módulo 3): confirmar hace visibles los cambios (T2) y los lleva a `muninn.data` a más tardar al cerrar el almacén o al dejar de usarlo sin cerrarlo; tras cerrar sin error y reabrir, todos los cambios confirmados están. Mientras no exista el diario, tras una caída del proceso, del sistema o un corte de corriente con el almacén abierto, tras un fallo de entrada y salida que el cierre no consigue resolver (T12), o tras dejar de usar el almacén sin cerrarlo cuando la escritura en `muninn.data` falla, puede faltar en `muninn.data` cualquier confirmación posterior a la última apertura, o una parte de ella. G6 sigue valiendo: al reabrir, una página a medias se informa como daño, nunca como dato válido. Dentro del proceso, G1 vale completa. Es una excepción a G1 (solo ante caídas), G2 y G5 del padre, y a G12 en lo que la reapertura recupera, anotada en él, que termina con el módulo 3. |
| T10 | **Abortar**: una transacción de escritura abortada, o que se deja de usar sin confirmarla (incluido el pánico del hilo que la tiene, también en mitad de la confirmación), no aplica ninguno de sus cambios, y la siguiente petición de la cola obtiene su turno. El almacén sigue utilizable sin ninguna acción de la aplicación. Si la transacción hizo crecer `muninn.data`, el archivo conserva el tamaño nuevo (P11). Confirmar una escritura sin cambios ni operaciones fallidas termina sin error y no cambia nada: no escribe en `muninn.data`, así que nunca es la operación que encuentra un fallo de entrada y salida (T12). |
| T11 | **Fallo dentro de una escritura**: si una operación de la transacción de escritura falla (disco lleno G7, tamaño máximo P10, memoria agotada P13, daño G6 o entrada y salida G12), al leer o al escribir, la transacción ya no puede confirmarse. Sus operaciones siguientes se pueden intentar, pero confirmarla falla con la clase del primer fallo, no aplica nada, termina la transacción y la siguiente petición de la cola obtiene su turno. Salvo con un fallo de entrada y salida al escribir (T12), el almacén sigue aceptando escrituras. Una confirmación que falla por sí misma sigue la misma regla: no aplica nada y termina la transacción. |
| T12 | **Almacén que no acepta escrituras** (G12): el primer fallo de entrada y salida al escribir lo informa la operación que lo encuentra (una operación de la escritura, su confirmación, la confirmación de una escritura posterior o el cierre). Desde ese momento, todas las peticiones de escritura que esperan fallan al instante con la clase de entrada y salida, y también las nuevas, aunque la escritura en curso siga abierta. Las transacciones de lectura siguen funcionando, y una lectura nueva ve lo confirmado hasta el fallo. Al cerrar, el almacén vuelve a intentar llevar lo confirmado a `muninn.data`: si lo consigue, el cierre termina sin error y, al reabrir, todo lo confirmado está; si no, el cierre devuelve la clase de entrada y salida y libera el bloqueo (P14). |
| T13 | **Precedencia de los fallos al pedir una escritura**: cuando se cumplen a la vez varias condiciones de fallo sin espera, gana la primera: uso incorrecto (T4, T5), entrada y salida (T12), tiempo de espera agotado con límite cero (T4). |
| T14 | **Versiones retenidas y crecimiento** (v1.1): hasta el módulo 4, las páginas que una escritura deja de usar no se reutilizan. Cada página conserva su número: la versión que confirma una escritura vive en memoria (T16) mientras alguna lectura abierta necesite la anterior, y después se escribe en su sitio en `muninn.data`. `muninn.data` solo crece cuando una escritura reserva páginas nuevas. Todo el espacio del almacén cuenta para el tamaño máximo (P10): en disco, el de `muninn.data` más el de las versiones que están solo en memoria, incluidas las de la escritura abierta; en modo temporal, toda la memoria del almacén. Una lectura larga sobre un almacén que se sigue modificando puede llevarlo al máximo, y entonces una escritura que necesita más espacio falla con la clase de tamaño máximo (T11). Ese espacio se recupera cuando terminan las lecturas que lo retienen, sin reabrir: las escrituras siguientes vuelven a caber. La información del almacén (P12) describe `muninn.data` tal como está, incluido lo que haya crecido por transacciones abiertas o abortadas. |
| T15 | **Modo temporal**: las transacciones funcionan igual que en disco (G9); lo confirmado vive en memoria hasta cerrar el almacén. |
| T16 | **Lecturas sin copia en transacciones** (D11 del padre): una lectura devuelve una referencia a la proyección de `muninn.data`, a la memoria del almacén temporal o, para una versión confirmada que todavía no está en `muninn.data` o que ya no está en él, a la memoria del motor donde se conserva; nunca a una copia hecha para esa lectura. Es una precisión de D11 del padre, anotada en él. |
| T17 | **Encabezado**: en este módulo las transacciones no reescriben el encabezado; el diario (módulo 3) fija cuándo se reescribe (P3 del Pager). |
| T18 | **Datos de los errores nuevos** (D8 del padre): los dos llevan la carpeta del almacén, o que es temporal; el de tiempo de espera agotado lleva además el límite pedido. |

*Anotación del 2026-10-22 (v1.1), antes de implementar: T14 y AC-T16 decían que el espacio retenido por una lectura
larga no se recuperaba en esa apertura. Se corrigen para seguir el diseño de Voron, en el que cada página conserva su
número y la versión nueva se escribe en su sitio cuando ninguna lectura necesita la anterior, de modo que ese
espacio se recupera al terminar las lecturas.*

### Estado de partida

Verificado el 2026-10-22:

| Aspecto | Situación |
|---|---|
| MuninnDB | El Pager (módulo 1) está implementado. La API pública solo abre, consulta y cierra almacenes; no hay transacciones. Internamente, las páginas se leen sin copia mientras el almacén está abierto y se escriben directamente en `muninn.data` con acceso exclusivo al almacén, sin instantáneas ni aislamiento. El encabezado solo se escribe al crear el almacén. Las clases de error existentes son las del padre más tamaño máximo y memoria agotada; no hay ninguna de tiempo de espera ni de uso incorrecto. |
| Pager | El informe de cierre del Pager deja pendiente liberar la memoria que el almacén retiene al crecer cuando ninguna transacción la use. |
| PRD padre | [muninndb.prd.md](muninndb.prd.md) v1.1 asigna a este módulo lo que G4 no decide, el paso de transacciones entre hilos, el límite de lectores y el cierre con transacciones abiertas o el uso de un almacén cerrado. P3 del Pager deja a los módulos 2 y 3 cuándo se reescribe el encabezado. |
| Voron (referencia) | Un escritor con un bloqueo de escritura que admite un límite de espera y falla al vencer; error si el mismo hilo abre una segunda escritura; lectores sin límite fijo; las páginas que cambia la escritura se copian a un área temporal, y lo confirmado pasa por el diario antes de llegar al archivo de datos cuando ningún lector necesita la versión anterior. |

## Actores

- **Desarrollador integrador**: abre transacciones de lectura y escritura desde los hilos de su aplicación,
  confirma o aborta las de escritura y trata los errores por su clase.

El operador y la herramienta de línea de comandos no intervienen en este módulo.

## Flujo

1. La aplicación abre un almacén (flujo del [Pager](pager.prd.md#flujo)).
2. Desde cualquier hilo, abre transacciones de lectura; cada una ve su instantánea (T2) hasta que deja de usarla.
3. Desde un hilo, pide una transacción de escritura, con un límite de espera opcional (T4). La petición se
   comprueba en el orden de T13 y, si hay otra escritura abierta o peticiones en cola, espera su turno (T3).
4. Dentro de la escritura, la biblioteca lee y cambia páginas (interno, T1).
5. La aplicación confirma la escritura (T9) o la aborta (T10). La siguiente petición de la cola obtiene su turno.
6. Cuando ya no queda ninguna transacción abierta, la aplicación cierra el almacén (T8, P14); lo confirmado queda
   en `muninn.data` (T9).

## Validaciones y mensajes

Valen las del padre y las del Pager. Este módulo añade estas situaciones, solo de la biblioteca (la herramienta no
abre transacciones):

| Situación | Regla | Clase de error |
|---|---|---|
| Un hilo pide una escritura sobre un almacén en el que ya tiene una abierta | La petición falla sin esperar; la abierta sigue utilizable (T5). | Uso incorrecto (nueva) |
| El límite de espera no se puede contar desde el momento actual | La petición falla sin encolarse (T4). | Uso incorrecto (nueva) |
| Se pide una escritura, o hay una esperando, y el almacén no acepta escrituras | La petición falla al instante (T12). | Entrada y salida |
| Vence el límite de espera de una petición de escritura | La petición falla sin ningún efecto y sale de la cola (T4). | Tiempo de espera agotado (nueva) |
| Límite cero con una escritura abierta o peticiones en cola | La petición falla sin encolarse (T4). | Tiempo de espera agotado (nueva) |
| Falta memoria al abrir una transacción o durante ella | La operación falla; el almacén sigue utilizable (T7, P13). | Memoria agotada |
| Falla una operación de una escritura | La transacción ya no se puede confirmar (T11). | La del fallo |
| Se confirma una escritura en la que falló una operación | La confirmación falla con la clase del primer fallo, no aplica nada y termina la transacción (T11). | La del primer fallo |
| Falla una escritura del sistema al confirmar o al llevar lo confirmado a `muninn.data` | La operación que lo encuentra falla; el almacén deja de aceptar escrituras (T12). | Entrada y salida |
| Se cierra un almacén que dejó de aceptar escrituras | El cierre reintenta llevar lo confirmado a `muninn.data`: sin error si lo consigue; si no, falla, libera el bloqueo y lo confirmado puede faltar al reabrir (T12, T9). | Entrada y salida, si falla |
| Confirmar o abortar dos veces una transacción, o usarla después de terminarla | No compila (T8). | — |
| Cerrar el almacén con transacciones abiertas, usar una transacción después de cerrarlo, o usar una referencia después de que termine su transacción o tras un cambio de la misma escritura | No compila (T8). | — |
| Pasar a otro hilo o compartir una transacción de escritura | No compila (T6). | — |

Cuando una petición de escritura cumple varias filas a la vez, gana la que va antes en T13.

## Salida

El módulo no escribe nada en pantalla ni añade datos a la información del almacén (P12), que mantiene su forma.
Su salida pública son los resultados de abrir, confirmar y abortar transacciones y las dos clases nuevas de error,
con los datos de T18.

En disco, la carpeta del almacén sigue conteniendo solo `muninn.data`, además de los archivos ajenos (P7).

## Reutilización

- El Pager: las páginas, su comprobación de integridad, el crecimiento, el tamaño máximo, la memoria agotada y las
  lecturas sin copia (P10, P11, P13, P15).
- Las clases de error del padre y del Pager; este módulo solo añade dos.

## Fuera de alcance

- Guardar datos desde la aplicación: estructuras de datos y su API (módulo 5 y siguientes).
- Una API pública para leer, reservar o escribir páginas (T1, P1 del Pager).
- Diario, durabilidad de las confirmaciones, atomicidad ante caídas y recuperación (módulo 3, T9), incluidos los
  criterios AC1 a AC5 del padre.
- Reescribir el encabezado (T17, módulo 3).
- Guardar el área temporal donde viven las versiones de T14 y T16 en archivos temporales proyectados en memoria,
  como Voron: en este módulo está en la memoria del proceso, y los archivos llegan con el diario (módulo 3).
- Modos de confirmación no durables (G11, módulo 3).
- Reutilizar el espacio que las escrituras dejan de usar (módulo 4, T14).
- Varias transacciones de escritura a la vez, aunque cambien partes distintas del almacén (G4).
- Transacciones anidadas, puntos de guardado dentro de una transacción y convertir una lectura en escritura.
- Prioridades entre peticiones de escritura, o un orden distinto del de llegada (T3).
- Un límite de espera por defecto para todo el almacén (T4).
- Cancelar desde otro hilo una petición que espera.
- Detectar interbloqueos entre almacenes: dos hilos que esperan sin límite, cada uno con una escritura abierta en
  el almacén que el otro pide, esperan para siempre.
- Reintentar una confirmación que falló (T11).
- Una transacción de escritura que la aplicación olvida sin terminarla (sin abortarla, confirmarla ni dejar de
  usarla, algo posible en Rust seguro): la cola queda bloqueada hasta que el proceso termina.
- Un límite de lectores, de tiempo de vida de una lectura, o expulsar o abortar lecturas largas (T7).
- Exponer el número de una instantánea o estadísticas de transacciones y de la cola.
- Pasar o compartir una transacción de escritura entre hilos, y una API asíncrona (padre).
- Transacciones que abarcan varios almacenes (padre).
- Umbrales de rendimiento: los fija el banco comparativo (módulo 6).

## Dejado a la implementación

- Cuándo, entre la confirmación y el cierre, se escriben en `muninn.data` los cambios confirmados y cuándo se
  llevan al disco (T9).
- Cuánto crece el almacén por cada confirmación (T14).
- Cuánta memoria usa una transacción y cuándo se libera la que retienen las lecturas que terminan.
- La precisión con que vence un límite de espera: nunca antes del límite, sin cota de cuánto después.
- A partir de qué límite de espera, por encima de 24 horas, la petición falla con la clase de uso incorrecto (T4).
- En qué momento de las operaciones de una escritura o de su confirmación se reserva el espacio y se detectan el
  disco lleno y el tamaño máximo, siempre que la confirmación informe el fallo (T11).
- Los nombres de los tipos, funciones, clases nuevas de error y campos de la API pública, que siguen las
  convenciones de Rust y están en inglés, y cómo se indica el límite de espera.
- Cómo las pruebas simulan fallos, falta de memoria y el orden de llegada de los hilos, incluidos los puntos de
  inyección en la confirmación y en el traspaso del turno que necesitan AC-T5, AC-T18 y AC-T19.

## Criterios de aceptación

Los criterios que necesitan leer o escribir páginas son pruebas internas de la biblioteca sobre su API interna
de transacciones, visible para las pruebas; las de no compilación usan la API pública cuando el criterio solo
toca transacciones y la interna cuando toca referencias a páginas. Cuando un criterio necesita un orden de
llegada conocido, la prueba espera a que cada petición esté en la cola antes de lanzar la siguiente. "Sin
esperar" significa que la petición devuelve su error mientras la escritura que la bloquearía sigue abierta.

Este módulo comprueba a nivel de biblioteca estos criterios del padre, adaptados a lo que existe en él:

- **AC6**: tal cual, con páginas como datos.
- **AC7**: con varios hilos que piden la escritura a la vez sin límite, todos la obtienen y confirman, y nunca hay
  dos abiertas a la vez; y con la escritura retenida por la prueba y peticiones encoladas en un orden conocido,
  unas sin límite y otras con un límite corto, las que tienen límite fallan con la clase de tiempo de espera
  agotado y las demás obtienen la escritura y confirman en su orden de llegada (T3).
- **AC10**: alterando cada byte de un almacén pequeño en el que se han confirmado varias transacciones de
  escritura y que se ha cerrado, y leyendo después todas las páginas en una transacción de lectura.
- **AC11**: con una escritura que necesita crecer; la confirmación falla con la clase de disco lleno (T11), no
  aplica nada, las lecturas siguen funcionando y, al liberar espacio, una escritura siguiente se confirma sin
  reabrir.
- **AC12**: con un fallo simulado en la primera escritura en `muninn.data` de lo confirmado por una transacción;
  la operación que lo encuentra (la confirmación, la confirmación de una escritura posterior o el cierre) falla con la
  clase de entrada y salida, las peticiones de escritura siguientes fallan con esa clase (T12), las lecturas
  funcionan y ven todo lo confirmado, y al reabrir el almacén abre (la recuperación llega con el módulo 3).
- **AC14**: el modo temporal con transacciones no crea archivos ni carpetas.
- **AC18** y **AC21**: tal cual.
- **AC20**: una lectura dentro de una transacción de lectura devuelve una referencia según T16, nunca a una copia
  hecha para esa lectura: dos lecturas de la misma página, en la misma transacción y en otra abierta después sin
  confirmaciones intermedias, devuelven la misma dirección, también para una versión confirmada que todavía no
  está en `muninn.data`; usarla después de terminar la transacción no compila.

Y además:

1. **AC-T1** — Dado un almacén, cuando una transacción de escritura cambia páginas y se confirma, y después se
   cierra y se reabre el almacén, entonces una transacción de lectura ve todos los cambios; lo mismo cuando el
   almacén se deja de usar sin cerrarlo y se reabre; y en modo temporal, una lectura abierta después de confirmar
   los ve.
2. **AC-T2** — Dada una transacción de escritura que cambia páginas y hace crecer el almacén, cuando se aborta, y
   en otras pruebas cuando se deja de usar sin confirmar y cuando el hilo que la tiene entra en pánico, entonces
   ninguna lectura posterior ve sus cambios, tampoco tras cerrar y reabrir, el tamaño que da la información del
   almacén no baja, y la siguiente petición de escritura la obtiene sin ninguna otra acción.
3. **AC-T3** — Dada una transacción de escritura abierta con cambios sin confirmar, cuando otro hilo, y el mismo
   hilo, abren una lectura, entonces la lectura no ve esos cambios y la escritura sí ve los suyos; y cuando la
   escritura se confirma, una lectura abierta después por el mismo hilo los ve (T2).
4. **AC-T4** — Dada una lectura abierta por un hilo antes de que ese mismo hilo pida una escritura que espera su
   turno detrás de otra, cuando
   la primera escritura confirma un cambio y la segunda obtiene su turno, entonces la segunda ve ese cambio y la
   lectura no (T2).
5. **AC-T5** — Dada una confirmación de una escritura que cambia varias páginas, detenida por la prueba en cada
   uno de sus puntos internos, cuando otro hilo abre una lectura y lee esas páginas en cada punto, entonces ve
   todos los cambios o ninguno, y una lectura abierta después de que la confirmación devuelve los ve todos (T2).
6. **AC-T6** — Dada una escritura abierta por otro hilo, cuando se pide una escritura con un límite de 100 ms,
   entonces falla con la clase de tiempo de espera agotado, no antes de 100 ms y sin efectos, y una petición
   posterior sin límite obtiene la escritura cuando la abierta termina; con límite cero, falla sin esperar; con
   límite cero y ninguna escritura abierta ni peticiones en cola, la obtiene; y con límite cero, ninguna escritura
   abierta y una petición en cola, falla con la clase de tiempo de espera agotado.
7. **AC-T7** — Dada una petición de escritura con límite que vence en la cola entre otras dos, cuando termina la
   escritura abierta, entonces las otras dos la obtienen en su orden de llegada (T3).
8. **AC-T8** — Dado un hilo con una escritura abierta, cuando pide otra sobre el mismo almacén, sin límite, con
   límite y con límite cero, entonces falla sin esperar con la clase de uso incorrecto, y la abierta se confirma
   después sin error; y cuando pide una sobre otro almacén, la obtiene.
9. **AC-T9** — Dada una petición de escritura con el mayor límite que admite la API, cuando se pide, entonces
   falla con la clase de uso incorrecto sin encolarse; y con un límite de 24 horas y la escritura libre, la
   obtiene (T4).
10. **AC-T10** — Dada una transacción de lectura, cuando se pasa a otro hilo y se comparte entre varios mientras
    se confirman escrituras, entonces todos ven la misma instantánea; y pasar o compartir una transacción de
    escritura entre hilos no compila (T6).
11. **AC-T11** — Dado un almacén, cuando se cierra con una transacción de lectura o de escritura abierta, o se usa
    una transacción después de cerrarlo, entonces no compila; cuando una escritura se confirma o se aborta dos
    veces, o se usa después de terminar, tampoco compila; y dentro de una escritura, usar una referencia a una
    página después de otro cambio de la misma transacción no compila (T8).
12. **AC-T12** — Dado un almacén, cuando se abren a la vez 10.000 transacciones de lectura desde varios hilos
    (10.000 transacciones abiertas, no 10.000 hilos), entonces todas se abren y leen sin error; y con un fallo
    simulado de memoria al abrir una lectura, falla con la clase de memoria agotada y el almacén sigue
    utilizable (T7).
13. **AC-T13** — Dada una escritura en la que falla una operación, en pruebas separadas por tamaño máximo, por
    memoria agotada y por una página dañada, cuando se intenta otra operación y se confirma, entonces la
    confirmación falla con la clase del primer fallo, no aplica nada, la siguiente petición de escritura la obtiene
    y una escritura siguiente que no falla se confirma (T11).
14. **AC-T14** — Dado un almacén que deja de aceptar escrituras con la escritura que falló todavía abierta y otra
    petición esperando, cuando ocurre el fallo, entonces la petición que espera falla con la clase de entrada y
    salida sin que la escritura abierta termine; una petición nueva también, sin esperar, incluida una con límite
    cero; una petición nueva del hilo que tiene la escritura abierta, y una de otro hilo con el mayor límite que
    admite la API, fallan con la clase de uso incorrecto (T13); y una lectura nueva ve lo confirmado hasta el
    fallo (T12).
15. **AC-T15** — Dada una lectura abierta, cuando se confirman escrituras que cambian las páginas que ve, entonces
    sus referencias siguen apuntando a la misma dirección con el mismo contenido mientras está abierta (T8, T16).
16. **AC-T16** — Dado un almacén con tamaño máximo y una lectura abierta, cuando se confirman escrituras que
    cambian siempre las mismas páginas, entonces llega un momento en que una escritura falla con la clase de
    tamaño máximo, en disco y en modo temporal, sin aplicar nada; y después de terminar la lectura, una escritura
    que cambia esas mismas páginas se confirma sin error, sin reabrir (T14).
17. **AC-T17** — Dada una escritura sin cambios, cuando se confirma, entonces termina sin error y la información
    del almacén no cambia; y con un fallo simulado en toda escritura en `muninn.data`, la confirmación de una
    escritura sin cambios termina igualmente sin error (T10).
18. **AC-T18** — Dada una escritura que termina mientras hay una petición en cola, cuando la prueba detiene el
    traspaso del turno y otro hilo pide una escritura en ese instante, sin límite y con límite cero, entonces la
    de la cola la obtiene primero, la nueva sin límite la obtiene después y la de límite cero falla con la clase
    de tiempo de espera agotado (T3, T4).
19. **AC-T19** — Dada una confirmación que la prueba interrumpe con un pánico en cada uno de sus puntos internos,
    cuando una lectura nueva lee el almacén, entonces no ve ninguno de sus cambios, y la siguiente petición de
    escritura la obtiene y confirma sin error (T10).
20. **AC-T20** — Dado un almacén que dejó de aceptar escrituras por un fallo simulado al llevar lo confirmado a
    `muninn.data`, cuando el fallo deja de producirse y se cierra, entonces el cierre termina sin error y, al
    reabrir, todo lo confirmado está; y cuando el fallo persiste, el cierre falla con la clase de entrada y
    salida y el almacén se puede volver a abrir (T12).
21. **AC-T21** — Dado un almacén con confirmaciones, cuando se cierra, entonces las dos copias del encabezado son
    idénticas byte a byte a las de antes de abrirlo (T17).
22. **AC-T22** — Dada una petición de escritura que vence su límite, y otra que falla por uso incorrecto, en un
    almacén en disco y en uno temporal, cuando se examina el error, entonces lleva la carpeta del almacén o que es
    temporal, y el de tiempo de espera agotado lleva además el límite pedido (T18).

## Anotaciones al padre

Este PRD declara una excepción temporal y una precisión en el padre, que lleva las anotaciones correspondientes:

- G1 (solo ante caídas), G2, G5 y G12 en lo que la reapertura recupera: hasta el módulo 3, una caída, un fallo de
  entrada y salida que el cierre no resuelve o dejar de usar el almacén sin cerrarlo cuando falla la escritura
  pueden dejar `muninn.data` sin parte de lo confirmado; G6 sigue valiendo (T9, T12).
- D11: la referencia de una lectura también puede apuntar a la memoria donde el motor conserva una versión
  confirmada que no está en `muninn.data` (T16).

## Referencias

- Padre y raíz: [muninndb.prd.md](muninndb.prd.md) v1.1
- Módulo anterior: [pager.prd.md](pager.prd.md)
- Voron, en el repositorio de RavenDB: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron>

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 103bdb2b | 2026-10-22 18:17 UTC+01:00 | 1 h 1 min | 0 min | 1 h 1 min | 26 min | 35 min | 26 |

<!-- prd-time: [{"session": "103bdb2b-a5ca-4e77-bfc3-9c07014dd34e", "start": "2026-10-22T17:17:11.788000+00:00", "wall": 3649, "discarded": 0, "agent": 1573, "human": 2075, "interventions": 26, "idle_minutes": 10}] -->
