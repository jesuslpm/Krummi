# Krummi, motor de almacenamiento embebido inspirado en Voron — PRD

**Versión**: 1.1
**Fecha**: 2026-10-22
**Documento padre**: ninguno (PRD raíz)

## Objetivo y contexto

Construir **Krummi**, un motor de almacenamiento embebido y transaccional, escrito en Rust, **para uso en
producción**, que reproduce las capacidades de **Voron**, el motor de almacenamiento de RavenDB. Se entrega como
una biblioteca que otras aplicaciones Rust incorporan en su propio proceso, más una herramienta de línea de
comandos para operarlo.

Este documento es el paraguas del producto: fija sus objetivos, las garantías y reglas que valen para todos los
módulos, y reparte el producto en PRD hijos, cada uno con su propia ejecución y su propio cierre (ver
[Reparto en PRD hijos](#reparto-en-prd-hijos)). El primero es el **Pager**.

**Objetivos**:

1. **Un motor fiable**: transacciones ACID, durabilidad ante caídas del proceso, del sistema y cortes de
   corriente, y detección de cualquier daño en lo que el motor guarda.
2. **Todas las capacidades de Voron**: el producto está completo cuando los 16 PRD hijos del
   [reparto](#reparto-en-prd-hijos) están cerrados (decisión D4).
3. **Rendimiento competitivo y medido**: "competitivo" significa cumplir los umbrales que fija el PRD hijo del
   banco comparativo (decisión D5).

**Por qué**: Voron es un diseño probado durante años en producción (un escritor con lectores concurrentes que no
se bloquean, diario de escritura anticipada, recuperación automática), pero solo existe para .NET. Krummi
lleva ese diseño a Rust como producto propio, sin atarse a su formato en disco.

### Decisiones de producto

| ID | Decisión |
|---|---|
| D1 | **Producto para producción, con formato propio.** Krummi no lee ni escribe archivos de Voron ni de RavenDB. Reproduce sus capacidades, no su formato. |
| D2 | **Superficie pública**: una biblioteca Rust embebible (el producto principal) y una herramienta de línea de comandos de operación, con mensajes en español. |
| D3 | **Plataformas**: Windows x64 y Linux x86-64 con glibc, sobre discos locales. Las dos se prueban en cada PRD hijo. |
| D4 | **Alcance**: todas las capacidades de Voron, incluidas las estructuras añadidas para Corax (containers, posting lists, compact trees y lookups), que se construyen después del núcleo. |
| D5 | **Rendimiento**: se mide con un banco comparativo reproducible frente a otros motores embebidos de Rust, con las mismas cargas en el mismo equipo. El PRD hijo del banco (6) fija los motores, las cargas y los umbrales de los módulos 1 a 5; los módulos posteriores con impacto en el rendimiento fijan su propio umbral sobre ese banco. Antes del banco, los módulos 1 a 5 no tienen umbral. |
| D6 | **Un solo proceso por almacén.** El proceso que abre un almacén lo bloquea en exclusiva; la concurrencia (un escritor y muchos lectores) es entre hilos de ese proceso. Abrir otra vez un almacén ya abierto, desde otro proceso o desde el mismo, falla. La herramienta de línea de comandos trabaja solo sobre almacenes que nadie tiene abiertos. |
| D7 | **Evolución del formato**: cada almacén guarda la versión de su formato en disco, un número entero positivo que cambia con cada cambio incompatible del formato, también antes de la 1.0. Una versión del motor abre solo el formato que conoce; ante otro, anterior o posterior, se niega sin modificar nada. Desde la versión 1.0 del producto, la herramienta de línea de comandos migra un formato anterior al actual sin tocar el original; antes de la 1.0 no hay migración y los datos se recrean. |
| D8 | **Errores de la biblioteca**: los errores se agrupan en clases que la aplicación distingue sin leer texto; cada clase cubre las situaciones que le asigna la [tabla de validaciones](#validaciones-y-mensajes) o el PRD hijo dueño. Cada error lleva como datos aparte lo que su mensaje necesita (carpeta, versiones, número de página, causa). El texto de esos errores está en inglés; la herramienta de línea de comandos muestra en español el mensaje de cada situación. |
| D9 | **Almacenes por recuperar en la herramienta**: si un almacén se cerró por una caída, las órdenes de la herramienta lo recuperan al abrirlo (G5), como haría la biblioteca, y después hacen su trabajo. Las únicas que no lo modifican son la verificación y el rescate de datos (copiar lo que se pueda leer de un almacén dañado), que trabajan sobre el almacén tal como está. |
| D10 | **Reconocimiento de un almacén**: una carpeta contiene un almacén de Krummi cuando tiene su marca de identificación intacta. Todo lo demás que queda legible sin clave (G8), incluida la versión de formato, lleva su propia comprobación de integridad: si se altera, el almacén está dañado, no es de otro formato ni deja de ser un almacén. El PRD hijo 1 fija la marca y dónde está. |
| D11 | **Lecturas sin copia** (v1.1): las lecturas devuelven a la aplicación referencias a memoria del motor, sin copiar los datos a un buffer de quien llama. Las referencias valen mientras la transacción de lectura que las obtuvo esté abierta, y el compilador impide usarlas después. Si una página se guarda cifrada (módulo 9) o comprimida (módulo 5), se descifra o descomprime una sola vez en memoria de la transacción y la referencia apunta ahí. Las escrituras trabajan sobre una copia de la página, que el aislamiento por instantánea (G3) exige. |
| D12 | **Código no seguro** (v1.1): el Pager (módulo 1), con la variante cifrada que le añade el módulo 9, es la única parte del motor que puede usar código no seguro de Rust (`unsafe`). Lo encapsula detrás de una interfaz segura; el resto del motor lo tiene prohibido y una comprobación automática lo impide. |

*Anotación del 2026-10-22 a D12: ninguna operación pública provoca comportamiento indefinido, salvo que otro
programa modifique los archivos del almacén mientras está abierto, lo que queda fuera de alcance (decisión P16 de
[pager.prd.md](pager.prd.md)).*

*Anotación del 2026-10-22 a D7: un almacén íntegro cuyo número de formato es menor que el vigente es de formato
anterior, también con el número `0`, que ninguna versión usa y solo aparece en las pruebas (decisión P5 de
[pager.prd.md](pager.prd.md)).*

*Anotación del 2026-10-22 a D11: la referencia de una lectura también puede apuntar a la memoria donde el motor
conserva una versión confirmada de la página que no está en el archivo de datos, nunca a una copia hecha para esa
lectura (decisión T16 de [transactions.prd.md](transactions.prd.md)).*

*Anotación del 2026-10-23: el producto se llama Krummi; sus paquetes y los archivos de un almacén toman los nombres
de R2 y R3, y este documento se actualizó con los nombres nuevos (decisiones R1–R13 de
[rename-to-krummi.prd.md](rename-to-krummi.prd.md)). Los nombres de archivo y los números de formato de las
anotaciones anteriores se leen según R3 y R5, y el formato vigente es `3`. Una carpeta con un almacén de MuninnDB
no contiene un almacén de Krummi (D10, decisión R6).*

### Estado de partida

Verificado el 2026-10-22 sobre el repositorio y sobre el código público de Voron en el repositorio de RavenDB:

| Aspecto | Situación |
|---|---|
| MuninnDB | No hay código del producto ni PRD previos. El repositorio contiene solo la metodología de trabajo, las skills del proyecto, el guardián de worktrees y sus scripts. |
| Voron, núcleo (referencia) | Archivo de datos dividido en páginas de 8 KiB y mapeado en memoria, con variantes cifrada y para 32 bits; diario de escritura anticipada con recuperación; área temporal para las páginas que modifica la transacción en curso; gestión de espacio libre; claves de hasta 1.024 bytes. |
| Voron, estructuras (referencia) | Árboles B+ (con árboles multivalor, compresión de páginas hoja y streams), árboles de tamaño fijo, secciones de datos en bruto, tablas con esquema e índices (con compresión de valores por diccionario), containers, posting lists, compact trees y lookups. |
| Voron, operación (referencia) | Backup completo e incremental, compactación, actualización del esquema del almacén entre versiones, estadísticas e informe de almacenamiento, y una herramienta de recuperación aparte. |

## Actores

- **Desarrollador integrador**: incorpora la biblioteca en su aplicación Rust, abre almacenes, ejecuta
  transacciones, hace backups con el almacén en uso y trata los errores por su clase.
- **Operador**: usa la herramienta de línea de comandos sobre un almacén que nadie tiene abierto para hacer
  backup, restaurar, compactar, verificar, inspeccionar, migrar o rescatar datos.
- **Mantenedor**: ejecuta el banco comparativo y las pruebas en Windows y Linux antes de cada versión.

## Garantías del producto

Valen para todos los módulos. Un PRD hijo puede ampliarlas; si necesita relajar alguna, lo declara como excepción
y anota este documento.

| ID | Garantía |
|---|---|
| G1 | **Atomicidad**: una transacción de escritura confirmada aplica todos sus cambios; una abortada, o interrumpida antes de empezar a confirmarse, no aplica ninguno; una interrumpida durante la confirmación aplica todos o ninguno, nunca una parte. |
| G2 | **Durabilidad**: en el modo durable (el predeterminado), cuando la confirmación de una transacción termina sin error, sus cambios sobreviven a una caída del proceso, del sistema operativo o a un corte de corriente. Se supone un disco que cumple las órdenes de llevar los datos al medio físico; un disco que las ignora queda fuera de la garantía. |
| G3 | **Aislamiento por instantánea**: cada transacción de lectura ve el almacén tal como estaba cuando empezó, aunque después se confirmen escrituras. Los lectores no bloquean al escritor ni el escritor a los lectores. |
| G4 | **Un solo escritor**: en cada momento hay como máximo una transacción de escritura abierta por almacén; las demás esperan su turno. El PRD hijo de transacciones (2) fija el orden de espera, los límites de espera y qué pasa si un hilo pide una segunda escritura mientras tiene una abierta. |
| G5 | **Recuperación**: al abrir un almacén después de una caída, el motor lo devuelve, sin intervención del usuario, a un estado que incluye al menos todas las transacciones cuya durabilidad estaba garantizada, y ninguna parte de una transacción no confirmada. Lo que quedó escrito a medias al final del diario es una confirmación que no terminó y se descarta; un daño en lo ya confirmado es daño (G6). Una recuperación interrumpida o fallida se repite completa en la apertura siguiente. |
| G6 | **Integridad**: el motor nunca devuelve como válidos datos dañados de ninguna parte del almacén; al detectar un daño, la operación falla con la clase de error de daño. Lo que una lectura ya devolvió antes de llegar al daño era válido. Lo mismo vale para el backup, la compactación, la migración y la verificación, que comprueban todo lo que leen. |
| G7 | **Disco lleno**: si una escritura no cabe en disco, la operación falla con la clase de error de disco lleno, la transacción no aplica ninguno de sus cambios y el almacén sigue abierto y legible. Cuando vuelve a haber espacio, las escrituras siguientes funcionan sin reabrir el almacén. |
| G8 | **Confidencialidad**: con el cifrado activado, ningún dato del usuario llega al disco sin cifrar: ni el archivo de datos, ni el diario, ni los archivos temporales, ni los backups, que quedan cifrados. Solo lo necesario para reconocer el almacén y su versión de formato queda legible sin la clave. Una clave incorrecta nunca se informa como daño del almacén. |
| G9 | **Modo temporal**: un almacén puede abrirse solo en memoria, para pruebas, sin carpeta. Cumple G1, G3, G4 y G6; no crea ningún archivo ni carpeta y no conserva nada al cerrarse. Pedir cifrado en este modo es un error de opciones no válidas. |
| G10 | **Copias completas o nada**: las operaciones que producen otro almacén o archivo (backup, restauración, compactación, migración, rescate de datos) escriben solo en el destino que indica quien las llama, que no puede existir ya ni estar dentro del origen. Si fallan o se interrumpen, el origen queda como estaba, salvo la recuperación tras una caída que algunas hacen al abrirlo (D9), y no queda un resultado a medias con el nombre del destino. |
| G11 | **Modos no durables**: el PRD hijo del diario (3) puede ofrecer modos de confirmación sin durabilidad inmediata. En ellos siguen valiendo G1, G3, G4 y G6; solo se relajan G2 y el punto exacto al que vuelve G5, y el PRD hijo fija hasta dónde. |
| G12 | **Fallos del sistema al leer o escribir**: si el sistema falla al escribir o al llevar los datos al disco por una causa que no es falta de espacio, la operación falla con la clase de error de entrada y salida, y el almacén deja de aceptar escrituras hasta que se cierra y se vuelve a abrir, que lo recupera (G5); las lecturas siguen funcionando. Si el sistema falla al leer, la operación falla con la misma clase y el almacén sigue utilizable. Un fallo del sistema nunca se informa como daño, ni un daño como fallo del sistema. |

*Anotación del 2026-10-22: excepción a G12. Como en Voron, un fallo físico del disco al leer a través de la
proyección del archivo en memoria termina el proceso en lugar de devolver un error; G12 sigue valiendo para las
escrituras y para lo que no se lee por la proyección (decisión P17 de [pager.prd.md](pager.prd.md)).*

*Anotación del 2026-10-22: G6 se precisa para el encabezado del almacén. Si solo una de sus dos copias está
dañada, el almacén abre sin error con la otra, porque no se devuelve ningún dato dañado (decisión P6 de
[pager.prd.md](pager.prd.md)).*

*Anotación del 2026-10-22: excepción temporal a G1 (solo ante caídas), G2, G5 y G12 (en lo que la reapertura
recupera) hasta el módulo 3. Sin diario, una caída, un fallo de entrada y salida que el cierre no resuelve o dejar
de usar el almacén sin cerrarlo cuando falla la escritura pueden dejar el archivo de datos sin parte de lo confirmado desde la última
apertura; tras un cierre sin error, lo confirmado está, y G6 sigue valiendo (decisión T9 de
[transactions.prd.md](transactions.prd.md)).*

*Anotación del 2026-10-22: la excepción temporal anterior termina con el módulo 3; lo confirmado de forma durable
ya no puede faltar al reabrir (decisiones J4 y J13 de [journal.prd.md](journal.prd.md)).*

*Anotación del 2026-10-22: G5, G6 y D10 se precisan para el diario. Una confirmación del diario dañada, incompleta
o que falta, que no consta como durable en el propio diario, se descarta sin error como una confirmación que no
terminó; y si se daña la última confirmación durable sin ninguna posterior, no se distingue de ella y también se
descarta (decisión J9 de [journal.prd.md](journal.prd.md)).*

## Flujo

1. El desarrollador integrador abre un almacén indicando su carpeta, o en modo temporal. Si la carpeta no existe,
   la biblioteca la crea, con las carpetas intermedias, y crea el almacén vacío. La creación es atómica: o queda
   un almacén completo, o la carpeta queda vacía, sin restos del almacén.

   *Anotación del 2026-10-22: tras una creación interrumpida puede quedar un archivo temporal hasta la apertura
   siguiente, que lo borra y crea el almacén (decisión P8 de [pager.prd.md](pager.prd.md)).*
2. La apertura hace las comprobaciones en el [orden fijado](#orden-de-las-comprobaciones-al-abrir) y, si el
   almacén se cerró por una caída, lo recupera (G5).
3. La aplicación abre transacciones de lectura desde cualquier hilo y una de escritura cada vez (G3, G4); lee y
   modifica las estructuras de datos y confirma o aborta.
4. La aplicación cierra el almacén y se libera el bloqueo exclusivo.
5. Con el almacén cerrado, el operador ejecuta las operaciones de la herramienta de línea de comandos (D9).

## Reparto en PRD hijos

Cada módulo es un PRD hijo de este, con su propia ejecución. El orden es el de construcción: cada uno se apoya en
los anteriores. La columna *Fija* nombra lo que ese PRD hijo debe decidir, además de su contenido y de sus propios
errores y mensajes.

| # | PRD hijo | Contenido | Fija |
|---|---|---|---|
| 1 | **Pager** | Archivo de datos dividido en páginas; versión del formato; bloqueo exclusivo; crecimiento del archivo; detección de páginas dañadas; creación atómica; modo temporal. | El tamaño de página; los archivos que componen un almacén y qué hacer con archivos ajenos en su carpeta; cómo se bloquea el almacén sin dejar restos que contradigan la creación atómica; el tamaño máximo del almacén; el error de memoria agotada en modo temporal. |
| 2 | **Transacciones** | Transacciones de lectura y escritura con aislamiento por instantánea, un escritor y lectores concurrentes. | Lo que G4 le encarga; si una transacción puede pasar de un hilo a otro; el límite de lectores; qué pasa al cerrar un almacén con transacciones abiertas o al usar uno cerrado. |
| 3 | **Diario y recuperación** | Diario de escritura anticipada que hace durables las confirmaciones (G2), con compresión; traslado de lo confirmado al archivo de datos; recuperación (G5). | Los modos no durables (G11); el disco lleno durante la recuperación. |
| 4 | **Espacio libre** | Reutilización de las páginas que las transacciones liberan. | Qué pasa con el crecimiento del almacén cuando un lector de larga duración impide reutilizar páginas. |
| 5 | **Árboles B+** | Árboles con claves y valores de tamaño variable, valores grandes, recorrido en orden, búsqueda por prefijo, árboles multivalor, árboles de tamaño fijo y compresión de páginas hoja. | Los tamaños máximos de clave, de valor y de nombre de árbol, y sus errores. |
| 6 | **Banco comparativo** | Banco reproducible frente a otros motores embebidos de Rust (D5). | Los motores de referencia, las cargas, la orden que lo ejecuta, la forma del informe y los umbrales de los módulos 1 a 5. |
| 7 | **Tablas** | Secciones de datos en bruto; tablas con esquema, clave primaria e índices secundarios; compresión de valores por diccionario. | Sus errores (clave duplicada, esquema distinto). |
| 8 | **Streams** | Valores de tamaño arbitrario guardados y leídos por partes. | — |
| 9 | **Cifrado** | Cifrado autenticado en reposo de todo lo que el motor escribe en disco (G8). | Los errores y mensajes de clave incorrecta, ausente o sobrante. |
| 10 | **Backup y restauración** | Backup completo e incremental desde la biblioteca con el almacén en uso; restauración. | El disco lleno en el destino; cómo se encadenan los incrementales; el error de restaurar un backup dañado o incompleto. |
| 11 | **Compactación** | Copia del almacén sin el espacio libre, para recuperar disco. | El disco lleno en el destino. |
| 12 | **Herramienta de operación** | Órdenes de backup, restauración y compactación de almacenes cerrados (módulos 10 y 11); inspección, estadísticas e informe de almacenamiento; verificación de integridad; rescate de datos de un almacén dañado; migración de formato desde la 1.0 (D7). | El nombre del ejecutable, de sus órdenes y opciones; la ayuda y la versión; los mensajes de uso incorrecto; la codificación y los saltos de línea de la salida; cómo se muestran las rutas; la interrupción con Ctrl+C; la salida exacta de cada orden, incluido cómo informa la verificación de cada daño; qué hace la migración con un almacén de formato anterior que necesita recuperación. |
| 13 | **Containers** | Almacenamiento de valores pequeños direccionados por identificador. | — |
| 14 | **Posting lists** | Listas ordenadas y comprimidas de identificadores. | — |
| 15 | **Compact trees** | Árboles con claves comprimidas. | — |
| 16 | **Lookups** | Diccionarios de clave fija. | — |

## Validaciones y mensajes

Situaciones comunes a todo el producto. La biblioteca devuelve la clase de error de cada fila (D8); la columna de
mensaje es el texto que muestra la herramienta de línea de comandos, donde `<carpeta>` es la carpeta del almacén,
`<destino>` el destino de una copia, `<versión>` un número de formato y `<número>` el número de página, en decimal
y contando desde 0. El PRD hijo 12 fija cómo se muestran `<carpeta>` y `<destino>` y el texto de `<causa>`.

| Situación | Regla | Clase de error | Mensaje en la herramienta |
|---|---|---|---|
| La carpeta no se puede usar: es un archivo, falta permiso, el sistema de archivos es de solo lectura o la ruta no es válida | La apertura falla sin tocar nada. | Acceso | `No se puede usar la carpeta <carpeta>: <causa>.` |
| La carpeta de origen no existe (solo en la herramienta) | La orden falla: la herramienta nunca crea un almacén en una carpeta de origen. | Acceso | `La carpeta <carpeta> no existe.` |
| El almacén ya está abierto, por otro proceso o por el mismo | La apertura falla sin tocar el almacén (D6). | Almacén en uso | `El almacén <carpeta> ya está abierto.` |
| La carpeta no es un almacén de Krummi: tiene alguna entrada, incluidas las ocultas, que no lo forma, o está vacía y la abre la herramienta | La apertura falla sin tocar nada. | No es un almacén | `La carpeta <carpeta> no contiene un almacén de Krummi.` |
| El almacén se reconoce como de Krummi, pero lo necesario para abrirlo está dañado | La apertura falla sin tocar el almacén (G6). Gana a la fila de página dañada cuando el daño se encuentra al abrir. | Daño | `El almacén <carpeta> está dañado y no se puede abrir.` |
| El almacén tiene un formato anterior | La apertura falla sin tocar el almacén (D7). | Formato distinto | `El almacén <carpeta> tiene el formato <versión>, anterior al formato <versión> de esta versión de Krummi.` |
| El almacén tiene un formato posterior | La apertura falla sin tocar el almacén (D7). | Formato distinto | `El almacén <carpeta> tiene el formato <versión>, posterior al formato <versión> de esta versión de Krummi.` |
| Una página está dañada, después de abrir | La operación falla y no devuelve datos de esa página (G6). | Daño | `Página <número> dañada en el almacén <carpeta>.` |
| El disco está lleno | La operación falla y la transacción no aplica nada (G7). | Disco lleno | `No hay espacio en disco para completar la operación en <carpeta>.` |
| Otro fallo del sistema al escribir o al llevar datos al disco | La operación falla y el almacén deja de aceptar escrituras hasta reabrirlo (G12). | Entrada y salida | `Error de escritura en el almacén <carpeta>: <causa>.` |
| Fallo del sistema al leer | La operación falla y el almacén sigue utilizable (G12). | Entrada y salida | `Error de lectura en el almacén <carpeta>: <causa>.` |
| El destino de una copia ya existe | La operación falla sin escribir nada (G10). | Destino no válido | `El destino <destino> ya existe.` |
| El destino de una copia está dentro del origen | La operación falla sin escribir nada (G10). | Destino no válido | `El destino <destino> está dentro del almacén <carpeta>.` |
| Se pide cifrado en modo temporal (solo en la biblioteca) | La apertura falla (G9). | Opciones no válidas | — |

La herramienta escribe cada mensaje en una línea propia en la salida de errores.

### Orden de las comprobaciones al abrir

Cuando se cumplen varias situaciones, gana la primera de esta lista:

*Anotación del 2026-10-22: antes del paso 1 se comprueban las opciones de apertura; unas opciones no válidas ganan a
todo lo demás y la apertura falla sin tocar nada (flujo de [pager.prd.md](pager.prd.md)).*

1. La carpeta se puede usar.
2. El almacén no está abierto ya.
3. La carpeta contiene un almacén de Krummi, es decir, su marca de identificación está intacta (D10), o está
   vacía, y entonces la biblioteca crea uno.

   *Anotación del 2026-10-22: para este paso y para la fila de carpeta que no es un almacén, el archivo temporal
   que deja una creación interrumpida no cuenta como entrada cuando es el único contenido de la carpeta (decisión
   P8 de [pager.prd.md](pager.prd.md)).*
4. Lo que queda legible sin clave (G8), incluida la versión de formato, supera su comprobación de integridad
   (D10).
5. Su formato es el de esta versión. Un almacén de otro formato que además necesita recuperación se rechaza sin
   recuperar (D7 prevalece sobre G5).
6. Con cifrado, la clave es correcta (PRD hijo 9).
7. El resto de lo necesario para abrirlo no está dañado.
8. Si se cerró por una caída, la recuperación termina (G5). Un daño en lo ya confirmado se informa como almacén
   dañado.

## Salida

Fuera de la carpeta del almacén, la biblioteca solo escribe en los destinos que le indica quien la llama (G10), y
no escribe nada en pantalla.

Convenciones comunes de la herramienta de línea de comandos; el PRD hijo 12 fija la salida exacta de cada orden:

- Los resultados van a la salida estándar y los errores a la salida de errores.
- Códigos de salida: `0` éxito; `1` la operación falló, incluida una verificación que encuentra daños; `2` uso
  incorrecto (orden u opciones no válidas).
- Ejemplo de error, con el almacén abierto por una aplicación:

  ```text
  El almacén D:\datos\pedidos ya está abierto.
  ```

  Código de salida: `1`.

## Reutilización

Ninguna: es el primer PRD del producto. Voron es la referencia de comportamiento; su código no se reutiliza ni se
traduce, y su formato no se reutiliza (D1).

## Fuera de alcance

- Compatibilidad con archivos de Voron o de RavenDB (D1).
- Servidor de red, protocolo remoto, lenguaje de consulta o SQL.
- Replicación, distribución y clústeres.
- Acceso al mismo almacén desde varios procesos, incluida la lectura (D6).
- Abrir un almacén en modo de solo lectura, incluidos los almacenes en medios de solo lectura.
- macOS, sistemas de 32 bits, ARM64 y Linux con musl (D3).
- Sistemas de archivos de red o sincronizados (NFS, SMB, carpetas de OneDrive o similares).
- Discos que ignoran las órdenes de llevar los datos al medio físico (G2).
- Bindings para otros lenguajes (API C, .NET, Python).
- Una API asíncrona de Rust.
- Funciones propias de RavenDB por encima de Voron: documentos, índices de texto, fusión de transacciones de
  varios clientes.
- Transacciones que abarcan varios almacenes.
- Gestión y custodia de claves de cifrado, y cambio o rotación de la clave de un almacén existente.
- Borrado seguro de los datos eliminados.
- Integración con sistemas externos de logs, métricas o trazas.
- Copiar los archivos de un almacén abierto como forma de backup: el backup se hace con la biblioteca o la
  herramienta.
- Migración entre formatos antes de la versión 1.0, y migración a un formato anterior en cualquier versión (D7).
- Licencia y publicación del crate: se deciden antes de la primera publicación, en otro PRD.

## Dejado a la implementación

- Los algoritmos de compresión y de cifrado y la organización interna de las páginas, siempre que cumplan las
  garantías y los umbrales del banco comparativo.
- El texto en inglés de los errores de la biblioteca (D8).

## Criterios de aceptación

Valen para el producto completo. Cada PRD hijo comprueba a nivel de biblioteca los que tocan su módulo, desde el
módulo 1, y añade los suyos. El PRD hijo 12 comprueba además a nivel de herramienta AC8, AC9, AC10, AC15 y AC17.

1. **AC1** — Dado un almacén en modo durable, cuando la confirmación de una transacción termina sin error y
   después se mata el proceso, entonces al reabrir el almacén los cambios están presentes (G2, G5).
2. **AC2** — Dado un almacén en modo durable, cuando la confirmación de una transacción termina sin error y
   después se simula un corte de corriente (se pierde todo lo que no se llevó al disco, y la última escritura de
   cada archivo puede quedar a medias), entonces al reabrir los cambios están presentes (G2).
3. **AC3** — Dado un almacén en modo durable, cuando el proceso se mata con una transacción de escritura abierta
   que no ha empezado a confirmarse, entonces al reabrir no hay ninguno de sus cambios y sí todos los de las
   transacciones confirmadas antes (G1, G5).
4. **AC4** — Dado un almacén en modo durable, cuando se provoca una caída, del proceso y simulada de corriente,
   en cada punto en que la confirmación de una transacción escribe o lleva datos al disco, entonces al reabrir
   hay todos sus cambios o ninguno, nunca una parte, y todos los de las transacciones confirmadas antes (G1, G5).
5. **AC5** — Dada una recuperación que se interrumpe en cada uno de sus puntos de escritura, cuando se vuelve a
   abrir el almacén, entonces el resultado es el mismo que el de una recuperación sin interrupción (G5).
6. **AC6** — Dada una transacción de lectura abierta, cuando otra de escritura se confirma, entonces la de
   lectura sigue viendo los datos anteriores y una lectura nueva ve los nuevos; y mientras haya lectores
   abiertos, la escritura empieza y se confirma sin esperarlos (G3).
7. **AC7** — Dados varios hilos que piden a la vez una transacción de escritura, cuando se observan, entonces
   nunca hay dos abiertas a la vez y, dentro de los límites de espera que fije el PRD hijo 2, todas terminan
   confirmadas (G4).
8. **AC8** — Dado un almacén abierto, cuando otro proceso, o el mismo, lo intenta abrir, entonces la apertura
   falla con la clase de error de almacén en uso; en la herramienta, con `El almacén <carpeta> ya está abierto.`
   y código de salida `1` (D6).
9. **AC9** — Dado un almacén de formato anterior y otro de formato posterior, creados para la prueba con su número
   de formato cambiado y por lo demás válidos, cuando se abren, entonces la apertura falla con la clase de error
   de formato distinto, la herramienta muestra el mensaje de la variante que corresponde con código de salida
   `1`, y los archivos del almacén quedan idénticos byte a byte (D7).
10. **AC10** — Dado un almacén pequeño de prueba, cuando se altera cada uno de sus bytes por separado y se abre y
    se lee todo el almacén, entonces cada alteración produce un error de la clase de daño o ninguna diferencia
    observable en los datos leídos, salvo las de la marca de identificación, que producen el error de que no es un
    almacén; nunca datos distintos de los escritos (G6, D10). Cada PRD hijo lo repite con las
    estructuras que añade.

    *Anotación del 2026-10-22 a AC10: una alteración en la última confirmación del diario también puede producir su
    descarte sin error (decisión J9 de [journal.prd.md](journal.prd.md)).*
11. **AC11** — Dado un disco sin espacio, cuando una transacción de escritura no cabe, entonces falla con la clase
    de error de disco lleno, no aplica ningún cambio, el almacén sigue permitiendo transacciones de lectura y,
    al liberar espacio, una escritura siguiente se confirma sin reabrir (G7).
12. **AC12** — Dado un fallo simulado al llevar datos al disco, cuando falla la confirmación, entonces el error es
    de la clase de entrada y salida, las escrituras siguientes fallan, las lecturas funcionan y, al reabrir, el
    almacén está recuperado (G12).
13. **AC13** — Dado el cifrado activado, cuando se escriben valores conocidos y se cierra el almacén, entonces
    ninguno de esos valores aparece en claro en ningún archivo de la carpeta del almacén, de sus backups ni de la
    carpeta temporal, que la prueba sustituye por una carpeta propia; y al
    abrirlo con una clave incorrecta, el error no es de la clase de daño (G8).
14. **AC14** — Dado un almacén en modo temporal, cuando se usa y se cierra, entonces el contenido de la carpeta
    temporal, que la prueba sustituye por una carpeta propia, y de la carpeta de trabajo es el mismo que antes de
    abrirlo, y al abrir otro en modo
    temporal está vacío (G9).
15. **AC15** — Dada una operación que produce una copia (backup, restauración, compactación, migración o
    rescate de datos) sobre un origen que no necesita recuperación, cuando falla o se interrumpe, entonces el
    origen queda idéntico byte a byte y no queda un resultado con el nombre del destino; y cuando el destino ya existe o
    está dentro del origen, falla con la clase de error de destino no válido sin escribir nada (G10).
16. **AC16** — Dada una creación de almacén interrumpida, cuando se vuelve a abrir la carpeta, entonces se abre un
    almacén completo o se crea uno nuevo como en una carpeta vacía (flujo, paso 1).
17. **AC17** — Dado un almacén cerrado por una caída, cuando se lanza sobre él una orden de backup de la
    herramienta, entonces el almacén queda recuperado y el backup contiene las transacciones confirmadas; y
    cuando se lanza la verificación o el rescate de datos, el almacén queda idéntico byte a byte (D9).
18. **AC18** — Las pruebas de cada PRD hijo pasan en Windows x64 y en Linux x86-64 con glibc (D3).
19. **AC19** — El banco comparativo se ejecuta con la orden que fija su PRD hijo y produce en Windows y en Linux
    un informe con la misma estructura, los mismos motores, las mismas cargas y los mismos campos, en el que
    Krummi cumple los umbrales vigentes (D5).
20. **AC20** — Dada una lectura de cualquier estructura de datos, cuando devuelve un valor, entonces la referencia
    apunta a memoria del motor (la proyección del archivo, o la memoria de la transacción si la página está
    cifrada o comprimida) y no a una copia hecha para esa lectura; y usar la referencia después de cerrar la
    transacción no compila (D11). Cada PRD hijo con lecturas lo comprueba.
21. **AC21** — Dado el código del motor, cuando se compila, entonces una comprobación automática falla si hay
    código no seguro fuera del Pager y de su variante cifrada (D12).

## Referencias

- Voron, en el repositorio de RavenDB: <https://github.com/ravendb/ravendb/tree/v6.2/src/Voron>
- Metodología del repositorio: `docs/methodology/`

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| c37552ac | 2026-10-22 10:08 UTC+01:00 | 1 h 9 min | 0 min | 1 h 9 min | 35 min | 34 min | 22 |

<!-- prd-time: [{"session": "c37552ac-2871-4449-abde-3b084495ba67", "start": "2026-10-22T09:08:44.181000+00:00", "wall": 4160, "discarded": 0, "agent": 2113, "human": 2047, "interventions": 22, "idle_minutes": 10}] -->
