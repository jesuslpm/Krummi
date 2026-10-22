# Pager de MuninnDB — PRD

**Versión**: 1.0
**Fecha**: 2026-10-22
**Documento padre**: [MuninnDB, motor de almacenamiento embebido inspirado en Voron — PRD](muninndb.prd.md) (v1.1)

## Objetivo y contexto

Construir el **Pager**, el módulo 1 del [reparto del padre](muninndb.prd.md#reparto-en-prd-hijos): la base sobre
la que se apoyan todos los demás. Da a MuninnDB su archivo de datos dividido en páginas, el reconocimiento y la
versión de formato de un almacén, su creación atómica, el bloqueo exclusivo, el crecimiento del archivo, la
detección de páginas dañadas y el modo temporal en memoria.

Al cerrar este módulo, el desarrollador integrador puede crear, abrir, consultar y cerrar almacenes, con las
comprobaciones de apertura del padre que ya existen en este módulo (ver [Flujo](#flujo)). Todavía no puede guardar
datos: eso llega con las transacciones (módulo 2) y las estructuras que vienen después.

**Por qué ahora**: todos los módulos leen y escriben páginas a través del Pager, y el padre deja aquí decisiones
que fijan el formato en disco (tamaño de página, archivos del almacén, marca de identificación).

Este PRD hereda todas las decisiones (D1–D12), garantías (G1–G12), validaciones y criterios de aceptación del
padre; aquí solo se añade lo propio del módulo. Las precisiones y excepciones a reglas del padre están declaradas
en sus decisiones y anotadas en él (ver [Anotaciones al padre](#anotaciones-al-padre)).

### Decisiones de producto

| ID | Decisión |
|---|---|
| P1 | **Superficie pública del módulo**: abrir o crear un almacén (en una carpeta o en modo temporal), consultar su información y cerrarlo. Reservar, leer y escribir páginas es interno de la biblioteca: no forma parte del contrato público. |
| P2 | **Tamaño de página: 8 KiB** (8.192 bytes), fijo para todos los almacenes y no configurable. |
| P3 | **Archivos de un almacén**: un único archivo, `muninn.data`, en la carpeta del almacén, que contiene también sus dos copias del encabezado (versión de formato y lo necesario para abrirlo). En este módulo el encabezado solo se escribe al crear el almacén, así que sus dos copias son iguales; los módulos 2 y 3 fijan cuándo se reescribe, manteniendo P6. Los módulos posteriores, como el diario (3), añaden sus propios archivos. `muninn.data` y `muninn.data.new` (P8) son nombres reservados. |
| P4 | **Marca de identificación** (D10 del padre): una secuencia fija de bytes al principio de `muninn.data`, en un único lugar y fuera de las copias del encabezado. Una carpeta contiene un almacén de MuninnDB cuando tiene un archivo `muninn.data` que empieza por la marca intacta. |
| P5 | **Versión de formato**: la vigente al escribir este PRD es `1`. Un almacén íntegro con un número menor que el vigente es de formato anterior, también con `0`, que ninguna versión usa y solo aparece en las pruebas; con uno mayor, de formato posterior. Es una precisión de D7 del padre, anotada en él. |
| P6 | **Encabezado doble**: cada copia del encabezado lleva su comprobación de integridad. Al abrir se usa la copia íntegra más reciente. Si solo una copia está dañada, el almacén abre sin error con la otra, porque no se devuelve ningún dato dañado; la copia dañada no se repara al abrir y se sobrescribe la próxima vez que se escribe el encabezado. Si las dos están dañadas, el almacén está dañado y no se puede abrir. Es una precisión de G6 del padre, anotada en él. |
| P7 | **Archivos ajenos**: en la carpeta de un almacén (con `muninn.data` reconocido), los archivos y carpetas con otros nombres se ignoran y nunca se modifican ni se borran. Una carpeta sin `muninn.data` sigue la regla del padre: si tiene alguna entrada, incluidas las ocultas, no es un almacén. |
| P8 | **Creación atómica**: la biblioteca prepara el almacén en `muninn.data.new`, lo lleva al disco y solo entonces lo renombra a `muninn.data`; cuando la apertura termina sin error, el almacén nuevo sobrevive a un corte de corriente. Si la creación falla, la biblioteca borra `muninn.data.new`; si se interrumpe, puede quedar. Una carpeta cuyo único contenido es el archivo `muninn.data.new` se trata como vacía: la apertura siguiente lo borra y crea el almacén. En cualquier otro caso `muninn.data.new` no se toca. Las carpetas que la apertura crea porque no existían (paso 1 del orden de apertura) se conservan aunque la creación falle. Es una precisión del flujo y de la validación de carpetas del padre, anotada en él. |
| P9 | **Bloqueo**: el bloqueo exclusivo (D6) es del almacén y dura desde el paso 2 del orden de apertura hasta el cierre, también mientras se crea: de varios que abren a la vez la misma carpeta vacía o inexistente, uno crea el almacén y los demás fallan con la clase de almacén en uso. Lo libera el sistema operativo cuando el proceso termina, también si se cae; una caída nunca deja un almacén bloqueado. Bloquear no deja en la carpeta más archivos que los de P3 y P8. Abrir el mismo almacén por otra ruta (relativa, con otra capitalización en Windows, por un enlace) también falla con la clase de almacén en uso. Cerrar el almacén, o dejar de usarlo sin cerrarlo explícitamente, libera el bloqueo. |
| P10 | **Tamaño máximo opcional**: al abrir, la aplicación puede fijar un tamaño máximo del almacén en bytes, que vale mientras esté abierto y no se guarda en él. Debe ser de al menos 1 MiB (1.048.576 bytes), en los dos modos; si no es múltiplo de 8.192, se redondea hacia abajo. Sin él, el límite es el disco y el sistema de archivos. Con él, ningún crecimiento lleva `muninn.data` más allá del máximo: un salto de crecimiento se recorta al máximo, y solo falla, con la clase de error de tamaño máximo, cuando las páginas pedidas no caben. El almacén sigue utilizable. Un almacén existente que ya es mayor que el máximo abre tal cual y se lee con normalidad, pero no puede crecer. En modo temporal limita la memoria que usa el almacén. |
| P11 | **Crecimiento**: un almacén nuevo ocupa como mucho 1 MiB. El tamaño de `muninn.data` es siempre múltiplo de 8.192 y crece en saltos que nunca son menores que el anterior y que aumentan a medida que crece; nunca encoge por sí solo (recuperar espacio es la compactación, módulo 11). Al crecer, el espacio se reserva de verdad en disco, de modo que la falta de espacio se detecta al crecer (G7) y no después. Un crecimiento que falla deja el archivo con el tamaño que tenía. Si un crecimiento interrumpido deja el archivo con una página final incompleta, la apertura la ignora. |
| P12 | **Información de un almacén**: mientras está abierto, la consulta siempre funciona, también si el almacén ya no acepta escrituras (G12), y devuelve: la versión de formato; el tamaño de página; el tamaño del almacén en bytes (el de `muninn.data` sin la página final incompleta, o en modo temporal el que ocupan sus páginas); el número de páginas, incluidas las del encabezado; el tamaño máximo efectivo tras redondear, o que no hay ninguno; y si el almacén es temporal. |
| P13 | **Memoria agotada**: si el sistema no da la memoria que el almacén necesita para abrirse o crecer, en cualquiera de los dos modos, la operación falla con la clase de error de memoria agotada; si el almacén ya estaba abierto, sigue utilizable. |
| P14 | **Cerrar**: cerrar libera el bloqueo siempre. Si al cerrar falla llevar datos al disco, el cierre devuelve la clase de error de entrada y salida, y el bloqueo queda liberado igualmente. |
| P15 | **Lecturas sin copia en el Pager** (D11 del padre): leer una página devuelve una referencia a la proyección de `muninn.data` en memoria (o a la memoria del almacén temporal), sin copiarla. En este módulo la referencia vale mientras el almacén siga abierto, y el compilador impide usarla después de cerrarlo; el módulo 2 la ata a la transacción de lectura (D11). Que `muninn.data` crezca mientras tanto no invalida las referencias ya entregadas. Comprobar la integridad de la página (G6) no exige copiarla. |
| P16 | **Código no seguro** (D12 del padre): el código no seguro de Rust que necesita el Pager (proyectar el archivo en memoria y leer sobre esa proyección) queda dentro del Pager, detrás de una interfaz segura, y cada uso lleva escrita la condición que lo hace correcto. Ninguna operación pública del motor puede provocar un comportamiento indefinido, salvo que otro programa modifique `muninn.data` con el almacén abierto (ver Fuera de alcance). |
| P17 | **Fallo físico de lectura**: como en Voron, un fallo del disco al leer una página a través de la proyección (un sector ilegible, un disco que se desconecta) termina el proceso en lugar de devolver un error: el sistema lo comunica con una señal o una excepción del procesador que no se puede convertir en error con seguridad. Al reabrir, si el fallo persiste, aparece al abrir o al leer esa página. Es una excepción a G12 del padre, anotada en él; G12 sigue valiendo para las escrituras y para todo lo que no se lee a través de la proyección. |

### Estado de partida

Verificado el 2026-10-22:

| Aspecto | Situación |
|---|---|
| MuninnDB | No hay código: ni archivos Rust ni espacio de trabajo de Cargo. El repositorio solo tiene la metodología, el tooling y los PRD. |
| PRD padre | [muninndb.prd.md](muninndb.prd.md) v1.1, con las anotaciones de este PRD: asigna a este módulo el tamaño de página, los archivos del almacén y los ajenos, el bloqueo sin restos, la marca de identificación, el tamaño máximo y el error de memoria agotada en modo temporal. |
| Voron (referencia) | Páginas de 8 KiB; encabezado con marca, versión, revisión y hash, en dos copias que se alternan (en Voron son dos archivos aparte); archivo de datos y diario en una subcarpeta; tamaño máximo opcional; tamaño inicial configurable. |

## Actores

- **Desarrollador integrador**: crea, abre, consulta y cierra almacenes desde su aplicación, y trata los errores
  por su clase.

El operador y la herramienta de línea de comandos no intervienen en este módulo (llegan con el 12).

## Flujo

1. La aplicación abre un almacén indicando una carpeta, con un tamaño máximo opcional, o en modo temporal.
2. La biblioteca comprueba primero las opciones: un tamaño máximo menor que 1 MiB falla con la clase de opciones no
   válidas sin tocar nada, antes de cualquier otra comprobación.
3. Después sigue el [orden del padre](muninndb.prd.md#orden-de-las-comprobaciones-al-abrir), con estas precisiones:
   - paso 1: si la carpeta no existe, la crea con sus carpetas intermedias;
   - paso 2: el bloqueo no crea archivos (P9);
   - paso 3: la carpeta contiene un almacén según P4; está vacía si no tiene ninguna entrada o solo el archivo
     `muninn.data.new` (P8), y entonces se crea el almacén;
   - paso 4: al menos una copia del encabezado está íntegra (P6);
   - paso 5: la versión de formato es la vigente (P5);
   - paso 7: el archivo contiene completas las páginas del encabezado.

   Los pasos 6 (clave) y 8 (recuperación) del padre aplican desde los módulos 9 y 3.
4. La aplicación consulta la información del almacén (P12).
5. La aplicación cierra el almacén (P14).

En modo temporal solo aplican el paso 2 de este flujo y la creación del almacén en memoria.

## Validaciones y mensajes

Valen las del padre. Este módulo añade estas situaciones, solo de la biblioteca (la herramienta no abre almacenes
hasta el módulo 12, que fijará sus mensajes si las muestra):

| Situación | Regla | Clase de error |
|---|---|---|
| Tamaño máximo menor que 1 MiB, en cualquier modo | La apertura falla sin tocar nada (flujo, paso 2). | Opciones no válidas |
| Un crecimiento necesita más páginas de las que caben en el tamaño máximo | La operación falla y el almacén sigue utilizable (P10). Se comprueba antes de escribir, así que gana a disco lleno. | Tamaño máximo |
| El sistema no da memoria para abrir o crecer | La operación falla; un almacén abierto sigue utilizable (P13). | Memoria agotada |
| `muninn.data` es una carpeta, o no empieza por la marca (incluido un archivo vacío o más corto que la marca) | Es una carpeta que no contiene un almacén (fila del padre). | No es un almacén |
| `muninn.data` no se puede leer por permisos | La apertura falla sin tocar nada. | Acceso |
| `muninn.data` tiene la marca, pero no contiene completas las páginas del encabezado | La apertura falla sin tocar nada (flujo, paso 3, precisión del paso 7). | Daño |
| Se lee una página escrita que ya no está en `muninn.data` porque el archivo se recortó con el almacén cerrado | La lectura falla (G6). | Daño |
| No se puede borrar un `muninn.data.new` que hay que borrar (P8) | La apertura falla sin tocar nada más. | Acceso |
| Falla una escritura del sistema al crear el almacén | La apertura falla; se aplica la limpieza de P8. | Entrada y salida |
| Falla llevar datos al disco al cerrar | El cierre informa el error y libera el bloqueo (P14). | Entrada y salida |

## Salida

El módulo no escribe nada en pantalla. Su salida pública es la información de un almacén (P12). Ejemplo, para un
almacén recién creado en disco sin tamaño máximo:

| Dato | Valor |
|---|---|
| Versión de formato | la vigente (hoy `1`) |
| Tamaño de página | `8192` |
| Tamaño del almacén en bytes | múltiplo de `8192`, como mucho `1048576` |
| Número de páginas | el tamaño entre `8192` |
| Tamaño máximo | ninguno |
| Temporal | no |

En disco, la carpeta del almacén contiene solo `muninn.data`, además de los archivos ajenos que ya tuviera.

## Reutilización

Ninguna: es el primer módulo con código. Usa las clases de error comunes y el orden de apertura del padre.

## Fuera de alcance

- Transacciones, aislamiento y un único escritor (módulo 2).
- Diario, durabilidad de las confirmaciones y recuperación tras una caída (módulo 3).
- Reutilización de páginas liberadas (módulo 4).
- Cifrado (módulo 9).
- Cualquier orden de la herramienta de línea de comandos, incluida la verificación que informa de una copia del
  encabezado dañada (módulo 12).
- Una API pública para reservar, leer o escribir páginas (P1).
- Tamaño de página configurable (P2).
- Reducir el tamaño de `muninn.data` (módulo 11).
- Tamaño inicial o saltos de crecimiento configurables (P11), y cambiar el tamaño máximo con el almacén abierto
  (P10).
- Reparar al abrir una copia dañada del encabezado (P6).
- Comprobar la integridad de páginas que no se leen, en segundo plano o al abrir.
- Borrar, renombrar, mover, recortar o modificar la carpeta o `muninn.data` desde otro programa con el almacén
  abierto: el resultado no está definido. En Linux el bloqueo exclusivo no impide que otro programa lo haga.
- Umbrales de rendimiento: los fija el banco comparativo (módulo 6, D5 del padre).

## Dejado a la implementación

- Los bytes de la marca de identificación y su longitud, la disposición y el tamaño del encabezado, cómo se decide
  qué copia es la más reciente y el algoritmo de la comprobación de integridad.
- El tamaño exacto de un almacén nuevo (hasta 1 MiB) y de cada salto de crecimiento (P11).
- Cómo se toma el bloqueo, siempre que se cumpla P9.
- Los nombres de los tipos, funciones y campos de la API pública, que siguen las convenciones de Rust y están en
  inglés.
- Cómo las pruebas simulan fallos de escritura, falta de espacio y de memoria, e interrupciones.

## Criterios de aceptación

Los criterios que necesitan reservar, escribir o leer páginas (AC-P10, la última parte de AC-P11, AC-P12,
AC-P13, AC-P14, AC-P15, AC-P17 y los del padre adaptados) son pruebas internas de la biblioteca sobre su API
interna de páginas, visible para las pruebas; las que alteran el encabezado usan las constantes de la propia
implementación.

Este módulo comprueba a nivel de biblioteca estos criterios del padre, adaptados a lo que existe en él:

- **AC8**: tal cual.
- **AC9**: con almacenes de prueba válidos cuyo número de formato es `0` (anterior) y el vigente más uno
  (posterior).
- **AC10**: alterando cada byte de un almacén pequeño en el que la prueba ha escrito páginas, y leyendo después
  todas las páginas escritas; las alteraciones de una sola copia del encabezado no producen ninguna diferencia
  (P6).
- **AC11**: con un crecimiento en lugar de una transacción; las páginas ya escritas se siguen leyendo y, al
  liberar espacio, el crecimiento siguiente funciona.
- **AC12**: con un fallo al escribir páginas; las escrituras siguientes fallan con la clase de entrada y salida,
  las lecturas de páginas y la consulta de información funcionan, y al reabrir el almacén abre y las páginas
  escritas antes del fallo se leen (la recuperación llega con el módulo 3).
- **AC14**: el modo temporal no crea archivos ni carpetas y no conserva nada al cerrarse.
- **AC16** y **AC18**: tal cual.
- **AC20**: leer una página devuelve una referencia dentro de la proyección de `muninn.data` (o de la memoria del
  almacén temporal), y usarla después de cerrar el almacén no compila (P15), con una prueba de no compilación
  sobre la API interna.
- **AC21**: dado código no seguro de prueba fuera del Pager, la comprobación automática falla; sobre el código del
  motor, pasa (P16).

Y además:

1. **AC-P1** — Dada una carpeta que no existe, con carpetas intermedias que tampoco, cuando se abre, entonces se
   crean, la carpeta contiene solo `muninn.data`, y la información dice el formato vigente, página `8192`, un
   tamaño múltiplo de `8192` de como mucho `1048576` bytes, número de páginas igual al tamaño entre `8192`, sin
   tamaño máximo y no temporal.
2. **AC-P2** — Dada una carpeta vacía, y otra cuyo único contenido es el archivo `muninn.data.new`, cuando se
   abren, entonces en las dos queda solo `muninn.data` con un almacén nuevo.
3. **AC-P3** — Dada una carpeta sin `muninn.data` con solo un archivo ajeno (por ejemplo `.gitkeep`), y otra con
   `muninn.data.new` y un archivo ajeno, cuando se abren, entonces fallan con la clase de error de que no es un
   almacén y las carpetas quedan idénticas byte a byte.
4. **AC-P4** — Dado un almacén creado y cerrado, cuando se vuelve a abrir, entonces abre sin error y su
   información es la misma.
5. **AC-P5** — Dado un almacén abierto por un proceso, cuando ese proceso se mata, entonces otro proceso lo abre
   sin error; y dado un almacén que el proceso deja de usar sin cerrarlo explícitamente, cuando lo vuelve a
   abrir, también.
6. **AC-P6** — Dados dos procesos, y dos hilos de un mismo proceso, que abren a la vez una misma carpeta que no
   existe, cuando terminan, entonces uno abre el almacén, el otro falla con la clase de almacén en uso, y la
   carpeta contiene solo `muninn.data`; y dado un almacén abierto, cuando se abre por otra ruta que llega al
   mismo archivo, entonces falla con la clase de almacén en uso.
7. **AC-P7** — Dado un almacén con archivos y carpetas ajenos en su carpeta, cuando se abre, se usa y se cierra,
   entonces abre sin error y los ajenos quedan idénticos byte a byte.
8. **AC-P8** — Dado un almacén, cuando se daña una de las dos copias del encabezado sin tocar la marca, entonces
   abre sin error y su información es la misma; y cuando se dañan las dos, la apertura falla con la clase de
   error de daño y el almacén queda idéntico byte a byte.
9. **AC-P9** — Dada una creación interrumpida en cada uno de sus puntos de escritura, del proceso y simulada de
   corriente, cuando se vuelve a abrir la carpeta, entonces queda solo `muninn.data` con un almacén completo; y
   dada una creación que termina sin error seguida de un corte de corriente simulado, cuando se reabre, entonces
   el almacén está completo.
10. **AC-P10** — Dado un almacén con tamaño máximo de 1,5 MiB, cuando se escriben páginas hasta llenarlo, entonces
    `muninn.data` llega exactamente a 1,5 MiB y la página siguiente falla con la clase de tamaño máximo, el
    archivo conserva su tamaño y las páginas ya escritas se leen; y después de cerrar y reabrir sin tamaño máximo,
    el crecimiento funciona.
11. **AC-P11** — Dado un tamaño máximo menor que 1 MiB, en disco y en modo temporal, cuando se abre, entonces
    falla con la clase de opciones no válidas sin crear nada; dado un tamaño máximo no múltiplo de `8192`, la
    información muestra el redondeado hacia abajo; y dado un almacén existente mayor que el tamaño máximo pedido,
    cuando se abre, entonces abre y lee con normalidad, y el primer crecimiento falla con la clase de tamaño
    máximo.
12. **AC-P12** — Dado un almacén en el que se escriben páginas hasta que crece al menos tres veces, cuando se
    observa su tamaño, entonces siempre es múltiplo de `8192`, el tercer salto es mayor que el primero, y después
    de cerrar y reabrir el tamaño no ha bajado.
13. **AC-P13** — Dado un `muninn.data` al que se le añade una página final incompleta, cuando se abre, entonces abre
    sin error y su información ignora esa página; dado un `muninn.data` recortado dentro de las páginas del
    encabezado, la apertura falla con la clase de daño; y dado uno recortado justo después del encabezado, abre
    y leer una página escrita que se perdió falla con la clase de daño.
14. **AC-P14** — Dado un almacén temporal, cuando se abre, entonces su información dice temporal, formato vigente y
    página `8192`; con tamaño máximo, un crecimiento que no cabe falla con la clase de tamaño máximo; con un fallo
    simulado de memoria, falla con la clase de memoria agotada; y en los dos casos sigue utilizable. Un fallo
    simulado de memoria al crecer un almacén en disco también falla con la clase de memoria agotada y lo deja
    utilizable.
15. **AC-P15** — Dado un disco sin espacio, cuando se crea un almacén, entonces falla con la clase de disco lleno y
    no queda ni `muninn.data` ni `muninn.data.new`; y dado un almacén abierto y un disco sin espacio, cuando
    necesita crecer, entonces falla con la clase de disco lleno y el archivo conserva su tamaño.
16. **AC-P16** — Dado un fallo simulado al llevar datos al disco durante el cierre, cuando se cierra, entonces
    devuelve la clase de entrada y salida y el almacén se puede volver a abrir.
17. **AC-P17** — Dada una referencia a una página escrita, en disco y en modo temporal, cuando el almacén crece
    varias veces mientras se conserva, entonces la referencia apunta a la misma dirección y su contenido es el
    mismo (P15).

## Anotaciones al padre

Este PRD precisa seis puntos del padre, que lleva las anotaciones correspondientes:

- G12: un fallo físico de lectura sobre la proyección termina el proceso, como en Voron (P17).
- D12: la excepción de comportamiento indefinido cuando otro programa modifica `muninn.data` con el almacén
  abierto (P16).
- G6: una sola copia dañada del encabezado no impide abrir (P6).
- D7: un número de formato menor que el vigente, incluido `0`, es de formato anterior (P5).
- Orden de apertura: las opciones se comprueban antes del paso 1 y ganan a todo lo demás (flujo, paso 2).
- Paso 1 del flujo, paso 3 del orden de apertura y fila de carpeta que no es un almacén: el archivo
  `muninn.data.new` que deja una creación interrumpida no cuenta como entrada ajena cuando es el único contenido de
  la carpeta, y la apertura siguiente lo borra (P8).

## Referencias

- Padre y raíz: [muninndb.prd.md](muninndb.prd.md) v1.1
- Voron, en el repositorio de RavenDB: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron>

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| c37552ac | 2026-10-22 10:08 UTC+01:00 | 1 h 9 min | 0 min | 1 h 9 min | 35 min | 34 min | 22 |

<!-- prd-time: [{"session": "c37552ac-2871-4449-abde-3b084495ba67", "start": "2026-10-22T09:08:44.181000+00:00", "wall": 4160, "discarded": 0, "agent": 2113, "human": 2047, "interventions": 22, "idle_minutes": 10}] -->
