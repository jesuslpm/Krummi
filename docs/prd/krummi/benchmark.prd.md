# Banco comparativo de Krummi — PRD

**Versión**: 1.0
**Fecha**: 2026-11-03
**Documento padre**: [Krummi, motor de almacenamiento embebido inspirado en Voron — PRD](krummi.prd.md) (v1.1)

## Objetivo y contexto

Construir el PRD hijo **6** del [reparto del producto](krummi.prd.md#reparto-en-prd-hijos): un banco comparativo
reproducible que mide Krummi frente a otros motores embebidos con las mismas cargas en el mismo equipo (D5), y que
fija los umbrales de rendimiento de los módulos 1 a 5.

Al cerrar este cambio, el mantenedor puede ejecutar una sola orden en Windows y en Linux y obtener un informe que
compara Krummi con **Voron**, **LMDB**, **redb**, **canopydb** y **fjall** en cada carga que cada motor ejecuta (K6), y
que dice, carga por carga, si Krummi cumple su umbral. El objetivo 3 del padre ("rendimiento competitivo y medido")
pasa a tener una definición comprobable: **en cada carga, Krummi rinde al menos el 80% de lo que rinde Voron**; en
las cargas de trabajo fijo, eso es no tardar más de 1,25 veces lo que tarda Voron (K7).

**Por qué Voron como referencia**: Krummi reproduce las capacidades de Voron (objetivo 2 del padre), así que la
pregunta que responde el banco es si las reproduce con un rendimiento comparable. Los demás motores no fijan
umbrales: dan contexto sobre dónde está Krummi entre los motores embebidos que una aplicación de Rust podría elegir.

**Por qué ahora**: los módulos 1 a 5 están cerrados y sus PRD dejaron los umbrales para este módulo. Hasta ahora
cada uno incluyó una medición propia de Krummi, sin comparar con nadie (F13, E16, V11, M15, X13).

Este PRD hereda todo lo del [PRD raíz](krummi.prd.md). Aquí solo se añade lo propio del módulo 6. Lo que cambia o
precisa reglas del padre está declarado en sus decisiones y anotado en el padre (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

En este documento, un **motor** es cada uno de los seis que compara el banco; una **carga** es una de las
operaciones medidas de la [tabla de cargas](#cargas); una **repetición** es una ejecución de una carga para un
motor; el **almacén base** es el que resulta de la carga W2 (K4).

### Decisiones de producto

| ID | Decisión |
|---|---|
| K1 | **Motores y versiones**: Krummi, con el código de la copia de trabajo desde la que se ejecuta el banco; **Voron** de RavenDB `7.2.7`; **LMDB** a través de heed `0.23.1`; **redb** `4.4.0`; **canopydb** `0.2.5`; y **fjall** `3.1.13`. Cambiar la versión de cualquiera es un cambio de este PRD y se anota aquí; cambiar la de Voron cambia además la referencia de los umbrales (K7). |
| K2 | **Voron, excepción a D5**: Voron no es un motor de Rust. El banco lo mide con un programa propio para .NET que usa Voron compilado desde el código fuente de RavenDB `7.2.7`, con el SDK de .NET 10 que ese código requiere. El código de RavenDB lo aporta el mantenedor (K10); el repositorio de Krummi no lo incluye ni lo distribuye (su licencia sin clave es AGPLv3). Voron no tiene paquete propio publicado y solo se puede obtener así. Krummi no reutiliza ni traduce código de Voron (D1): el banco solo lo usa para medirlo. Compilarlo puede dejar archivos intermedios dentro de la carpeta de RavenDB, pero no cambia ninguno de sus archivos de código. |
| K3 | **Mismas condiciones**: en cada ejecución, todos los motores se miden en el mismo equipo, sobre el mismo disco y en la misma carpeta de trabajo (K10), en este orden: Krummi, Voron, LMDB, redb, canopydb y fjall; y, dentro de cada motor, las cargas en el orden de la tabla. Nunca miden dos motores a la vez. Cada carga de cada motor se ejecuta en un proceso propio, que hace su repetición previa y sus tres medidas (K8) y termina; W12 y W13 son una sola ejecución con dos métricas y comparten proceso. Toda confirmación es **durable** en todos los motores: devuelve solo cuando sus cambios están llevados al medio físico (G2). Un motor cuya confirmación no lo garantiza por sí sola lo consigue con su propia operación de llevar al disco justo después de cada confirmación, y ese tiempo forma parte de la medición. Los motores con caché propia (redb, canopydb y la caché de bloques de fjall) la tienen de 1 GiB; los demás usan la caché del sistema. En canopydb y fjall, el banco usa sus transacciones de un solo escritor, no las de varios escritores optimistas. El resto de opciones de cada motor queda en su valor predeterminado, salvo lo que necesita para cumplir esta decisión o para ofrecer las estructuras de K6. Krummi y los demás motores de Rust se compilan con el perfil de publicación del repositorio; Voron, en configuración de publicación, con la recolección de memoria y la compilación por niveles predeterminadas de .NET. |
| K4 | **Datos**: las claves tienen 16 bytes y los valores 100 bytes, salvo donde la tabla de cargas dice otra cosa; los dos son bytes aleatorios, que no se comprimen. Una semilla fija genera los mismos bytes, en el mismo orden, para todos los motores y todas las repeticiones. **Al azar** significa una permutación sin repeticiones: cada clave aparece una sola vez; las **entradas nuevas** tienen claves distintas entre sí y de las del almacén base. **En orden de clave** son las mismas claves aleatorias ordenadas por sus bytes. El **almacén base** de un motor es el que deja W2 con 1.000.000 de entradas (10.000 en la ejecución reducida, K11), cerrado sin error. En las cargas que solo leen (W7–W11, W18, W20 y W22), el proceso de la carga abre su almacén de partida una sola vez: la repetición previa llena las cachés, la del sistema y la propia del motor, y las tres medidas leen con todas calientes (en la ejecución reducida, sin previa, la única medida lee con la caché propia del motor fría, K11). W12 modifica el almacén, así que sus lectores empiezan cada repetición con la caché propia del motor fría. En las que modifican, cada repetición empieza con su almacén de partida recién preparado, con exactamente el contenido de partida, recién abierto y sin vaciar la caché del sistema, y lo cierra al terminar. W16 empieza cada repetición con el almacén base cerrado, porque mide abrirlo. |
| K5 | **Cargas comunes**: W1 a W18 de la [tabla de cargas](#cargas). Las ejecutan los seis motores. |
| K6 | **Cargas de estructuras propias** (W19 a W22): árboles multivalor y árboles de tamaño fijo. Las ejecutan solo los motores que ofrecen esa estructura de forma nativa: Krummi, Voron, LMDB (claves duplicadas ordenadas y claves enteras) y redb (tablas multivalor y claves enteras). canopydb y fjall no las ejecutan y sus celdas dicen `no aplica`, también en la tabla de tamaños; el banco no las emula. |
| K7 | **Umbrales** (los de los módulos 1 a 5, D5): en cada carga, la métrica es el número de operaciones por segundo de K8. Krummi **cumple** el umbral de una carga cuando su métrica es al menos el 80% de la de Voron (razón Krummi / Voron ≥ 0,80, con los valores sin redondear). Hay un umbral por cada una de las 22 cargas, y todos valen en Windows y en Linux, cada informe con los suyos. Los demás motores no tienen umbral, ni el tamaño del almacén (K9). |
| K8 | **Medición**: cada carga se ejecuta, para cada motor, una vez sin medir y después tres veces medidas; su resultado es la **mediana** de las tres. El tiempo de una repetición va desde su primera operación hasta que devuelve su última confirmación o su última lectura; no incluye generar los datos, preparar el almacén, abrirlo ni cerrarlo, salvo en W16, ni el trabajo que el motor haga en segundo plano después. En las cargas con varios hilos, los hilos se crean antes y empiezan a la vez. La métrica de una repetición es su número de operaciones (columna *Operaciones*) dividido por su tiempo en segundos, y la de la carga es la mediana de las métricas de sus tres repeticiones medidas, calculada por separado para W12 y para W13. El trabajo que un motor deja en segundo plano tras su última confirmación (compactar, trasladar lo confirmado) no cuenta en ningún motor; lo paga el cierre, que no se mide. **Leer un valor** es recorrer todos sus bytes y acumularlos en una suma de comprobación; cuando la carga lee también la clave (W10, W11 y, una vez por clave, W20), sus bytes entran en la misma suma. Si la interfaz del motor solo devuelve una copia, la copia forma parte del tiempo. Una lectura que no encuentra su clave, o una suma que no coincide con la de los datos escritos, es un fallo del motor (*Validaciones y mensajes*). |
| K9 | **Tamaño del almacén**: tras la última repetición medida de W1, W2, W3, W14, W15, W17, W19 y W21, con el almacén cerrado sin error, el banco mide la suma de las longitudes de todos los archivos de la carpeta del almacén, incluidas sus subcarpetas, igual para todos los motores; si un motor deja archivos preasignados más grandes que sus datos, la cifra lo refleja sin corregirlo. Es informativo y no tiene umbral. |
| K10 | **La orden** (la de AC19 del padre): `cargo run --release -p krummi-bench -- --ravendb <carpeta> [--carpeta <carpeta>] [--informe <archivo>] [--reducido]`. Las opciones van en cualquier orden, cada una una sola vez y con su valor como argumento siguiente. `--ravendb` es la carpeta con el código de RavenDB `7.2.7`; `--carpeta`, una carpeta existente donde el banco crea los almacenes (la carpeta temporal del sistema si no se indica); `--informe`, el archivo del informe (`banco-<fecha>-<hora>.md` en la carpeta actual si no se indica, con la fecha y la hora locales de inicio como `AAAAMMDD` y `HHMMSS`). El banco crea sus almacenes dentro de una subcarpeta nueva y propia de la carpeta de trabajo y la borra al terminar, también si falla o se interrumpe. |
| K11 | **Ejecución reducida**: con `--reducido`, las cantidades son las de la columna *Reducida* de la tabla de cargas, con una sola repetición medida y sin la previa. Los tamaños de las claves, los valores y las escrituras y el número de hilos no cambian; como en la ejecución completa, la última escritura de una carga lleva las entradas que quedan cuando no llenan una escritura entera (en W17 reducida, una sola escritura de 64 valores). Sirve para comprobar que el banco funciona; los umbrales no se evalúan y el informe lo dice. |
| K12 | **Integración continua**: en cada ejecución de la integración continua, en Windows y en Linux, se ejecuta el banco con `--reducido` y debe terminar con código de salida `0`. Para ello obtiene el SDK de .NET 10 y el código de RavenDB `7.2.7`; si no puede descargarlos, la ejecución falla por esa causa externa. Los umbrales solo se evalúan en ejecuciones completas en el equipo del mantenedor. |
| K13 | **Incumplir un umbral** (precisa los códigos de salida del padre): el informe marca cada umbral como cumplido o incumplido, y la orden termina con código `1` si alguno se incumple, aunque la ejecución haya terminado y escrito su informe, como la verificación que encuentra daños. Este módulo cierra con el banco y su primer informe aunque Krummi incumpla umbrales: cada incumplimiento se convierte en un cambio aparte del módulo afectado, con su propio PRD. AC19 del padre se exige antes de cada versión del producto, no para cerrar este módulo (anotación a AC19).<br><br>*Anotación del 2026-11-04: el cambio necesita su propio PRD solo si cambia algo observable del producto: la salida, los mensajes, los datos guardados o su formato en disco, la interfaz pública, las garantías o las opciones. Una optimización que no cambia nada de eso no necesita PRD: se ejecuta directamente y su criterio de aceptación es que las cargas que ataca cumplen su umbral en las ejecuciones completas de Windows y de Linux, sin que empeoren las demás cargas (decisión del usuario).*<br><br>*Anotación del 2026-11-05: un cambio con su propio PRD fija su aceptación; la integridad recordada de las páginas ([pager-integrity-once.prd.md](pager-integrity-once.prd.md), decisión I9) exige que W7 y W22 mejoren y que ninguna carga empeore, pero no que W7 y W22 cumplan su umbral.* |
| K14 | **El banco no es parte del producto** (precisa D2 y D12): es una herramienta del mantenedor, no de la superficie pública del producto, y sus dependencias (los demás motores, el SDK de .NET) no llegan a la biblioteca de Krummi. Puede usar código no seguro de Rust solo donde la interfaz pública de otro motor lo exige. La comprobación de AC21 sigue valiendo para el motor. |

### Cargas

*Operaciones* es lo que cuenta la métrica (K8) en una ejecución completa; *Reducida* da las cantidades de la ejecución
reducida (K11). Los nombres son los que usa el informe.

| ID | Nombre | Empieza en | Qué hace | Operaciones | Reducida |
|---|---|---|---|---|---|
| W1 | `carga-en-orden` | Almacén vacío | Guarda 1.000.000 de entradas en orden de clave, en escrituras de 1.000. | 1.000.000 | 10.000 entradas |
| W2 | `carga-al-azar` | Almacén vacío | Guarda 1.000.000 de entradas al azar, en escrituras de 1.000. | 1.000.000 | 10.000 entradas |
| W3 | `carga-masiva` | Almacén vacío | Guarda 1.000.000 de entradas al azar en una sola escritura. | 1.000.000 | 10.000 entradas |
| W4 | `escrituras-sueltas` | Almacén base | Un hilo guarda 10.000 entradas nuevas al azar, una por escritura. | 10.000 | 160 entradas (múltiplo de 16, como W5 y W6) |
| W5 | `escrituras-concurrentes-4` | Almacén base | 4 hilos guardan a la vez 10.000 entradas nuevas en total, 2.500 cada uno, una por escritura. | 10.000 | 160 entradas, 40 por hilo |
| W6 | `escrituras-concurrentes-16` | Almacén base | 16 hilos guardan a la vez 10.000 entradas nuevas en total, 625 cada uno, una por escritura. | 10.000 | 160 entradas, 10 por hilo |
| W7 | `lecturas-al-azar` | Almacén base | Un hilo lee todas las claves al azar en una sola transacción de lectura. | 1.000.000 | 10.000 lecturas |
| W8 | `lecturas-al-azar-4` | Almacén base | 4 hilos, cada uno en su transacción de lectura, leen cada uno todas las claves en su propio orden al azar. | 4.000.000 | 40.000 lecturas |
| W9 | `lecturas-al-azar-16` | Almacén base | Como W8, con 16 hilos. | 16.000.000 | 160.000 lecturas |
| W10 | `recorrido` | Almacén base | Recorre todas las entradas hacia delante en una transacción de lectura, leyendo la clave y el valor de cada una. | 1.000.000 | 10.000 entradas |
| W11 | `intervalos` | Almacén base | En una transacción de lectura, 100.000 recorridos hacia delante, cada uno desde una clave existente elegida al azar, que leen hasta 100 entradas (menos si llegan al final). Por la semilla fija, el número de entradas leídas es el mismo en todos los motores. | Entradas leídas | 1.000 recorridos |
| W12 | `lecturas-con-escritor` | Almacén base | 4 hilos leen claves al azar, en transacciones de lectura de 1.000 lecturas cada una, mientras otro hilo hace 1.000 escrituras de 100 entradas nuevas. Cada lector recorre su propio orden al azar de las claves del almacén base y vuelve a empezarlo si lo agota. El tiempo es el del escritor, de su primera operación a su última confirmación; cuenta cada lectura individual que termina en ese tiempo. | Lecturas terminadas | 10 escrituras de 100 |
| W13 | `escritor-con-lectores` | La misma repetición que W12 | El escritor de W12; cuenta las entradas guardadas, no las confirmaciones. | 100.000 | 1.000 entradas |
| W14 | `borrado` | Almacén base | Quita todas las entradas al azar, en escrituras de 1.000. | 1.000.000 | 10.000 entradas |
| W15 | `reutilizacion` | Almacén base | Quita la mitad de las entradas al azar y después guarda otras tantas nuevas al azar, en escrituras de 1.000. | 1.000.000 | 5.000 y 5.000 entradas |
| W16 | `apertura` | Almacén base cerrado | 100 veces: abre el almacén, lee una clave al azar y lo cierra. El tiempo incluye abrir y cerrar. | 100 | 10 aperturas |
| W17 | `valores-grandes-carga` | Almacén vacío | Guarda 4.096 valores de 64 KiB (256 MiB) con claves al azar, en escrituras de 256. | 4.096 | 64 valores |
| W18 | `valores-grandes-lectura` | El almacén de W17 | Lee todos los valores al azar en una transacción de lectura. | 4.096 | 64 lecturas |
| W19 | `multivalor-carga` | Almacén vacío | Añade 1.000.000 de valores de 16 bytes a 100.000 claves, 10 por clave, en un orden al azar que intercala las claves, en escrituras de 1.000. | 1.000.000 | 10.000 valores en 1.000 claves |
| W20 | `multivalor-lectura` | El almacén de W19 | En una transacción de lectura, recorre los valores de cada clave, con las claves al azar. | 1.000.000 | 10.000 valores |
| W21 | `tamano-fijo-carga` | Almacén vacío | Guarda 1.000.000 de claves enteras de 64 bits distintas, al azar, con valores de 8 bytes, en escrituras de 1.000. | 1.000.000 | 10.000 entradas |
| W22 | `tamano-fijo-lectura` | El almacén de W21 | Lee todas las claves al azar en una transacción de lectura. | 1.000.000 | 10.000 lecturas |

Cada carga cubre al menos uno de los módulos 1 a 5: el Pager (W1–W3 y W16, y los tamaños de K9), las transacciones
(W8, W9, W12 y W13), el diario y la escritura agrupada (W4–W6 y W13), el espacio libre (W14 y W15, y sus tamaños) y
los árboles (todas; W10 y W11 para el recorrido, W17 y W18 para los valores grandes, W19 y W20 para los multivalor,
W21 y W22 para los de tamaño fijo). En W5 y W6, los motores con un solo escritor ponen en cola las escrituras de los
hilos (G4); Krummi además las lleva al disco agrupadas (E4).

### Estado de partida

Verificado el 2026-11-03 en el repositorio y en las fuentes públicas de cada motor:

| Aspecto | Situación |
|---|---|
| Banco | No existe. No hay ningún paquete del banco ni comparación con otros motores. |
| Mediciones de Krummi | Hay programas de ejemplo que miden solo a Krummi, sin umbral: los de F13, E16, V11, M15 y X13, y los de las cargas de árbol, el recorrido y la [comprobación de organización recordada](trees-node-check-cache.prd.md). Comparten un generador de claves aleatorias con semilla fija y una salida en tabla Markdown. |
| Integración continua | Ejecuta formato, clippy, pruebas y las barridas de aceptación en Windows y en Linux. No ejecuta ninguna medición ni tiene .NET. |
| Voron | No tiene paquete publicado en NuGet; solo se obtiene compilando el código de RavenDB. La última versión estable es `7.2.7`, cuyo Voron requiere .NET 10. Licencia sin clave: AGPLv3. |
| LMDB (heed) | heed `0.23.1`, publicado el 2026-10-26; compila LMDB desde C. Abrir un entorno es una función no segura de su interfaz. |
| redb | `4.4.0`, publicado el 2026-10-13. Árbol B+ en Rust, un escritor, con tablas multivalor y claves enteras. |
| canopydb | `0.2.5`, publicado el 2025-11-22. Árbol B+ en Rust; su transacción de escritura predeterminada es de un solo escritor y ofrece aparte varios escritores optimistas; la confirmación durable necesita su diario activado. Su integración continua prueba en Windows y en Linux. Su autor lo declara en fase temprana. |
| fjall | `3.1.13`, publicado el 2026-10-25. Árbol LSM en Rust, con transacciones de un solo escritor y optimistas. |

## Actores

- **Mantenedor**: ejecuta el banco completo en Windows y en Linux antes de cada versión, lee el informe y, ante un
  umbral incumplido, abre el cambio del módulo afectado (K13).
- **Integración continua**: ejecuta el banco reducido en cada cambio (K12).

## Flujo

1. El mantenedor tiene el SDK de .NET 10 y una carpeta con el código de RavenDB `7.2.7`.
2. Ejecuta la orden de K10. El banco comprueba sus opciones y requisitos en el orden de
   [Validaciones y mensajes](#validaciones-y-mensajes) antes de medir nada, y compila el programa de Voron.
3. Para cada motor, por turno (K3), prepara los almacenes que necesita y ejecuta cada carga con su repetición previa y
   sus tres repeticiones medidas (K8), y mide el tamaño de los almacenes (K9). Mientras tanto escribe su progreso en
   la salida de errores.
4. Al terminar, escribe el informe, borra su subcarpeta de trabajo y escribe el resumen en la salida estándar.
5. Termina con código `0` si Krummi cumple todos los umbrales o la ejecución es reducida, y con `1` si incumple
   alguno (K13).

## Validaciones y mensajes

Se comprueban en este orden; gana la primera que se cumple. Cada mensaje va en una línea propia en la salida de
errores. `<carpeta>`, `<archivo>` y `<destino>` se muestran tal como los escribió el mantenedor, o como los forma el
banco cuando son los predeterminados. Los argumentos se revisan de izquierda a derecha y gana el primero no válido;
`<opción>` es ese argumento tal como se recibió, la segunda aparición de una opción repetida, o el nombre de la
opción a la que le falta su valor (con `--carpeta --informe`, `--carpeta`). Los mensajes de carpeta y de destino son
los del padre; cada uno se usa donde el padre lo usa, por eso una carpeta que no existe da mensajes distintos según
sea la de trabajo (origen de los almacenes), la del informe (destino) o la de `--ravendb` (requisito).

| Situación | Regla | Mensaje | Código |
|---|---|---|---|
| Un argumento que no es una de las cuatro opciones (también `--opción=valor` o un argumento suelto), una opción repetida, o una con valor a la que le falta, o cuyo valor empieza por `--` | La orden no hace nada. | `Opción no válida: <opción>.` | `2` |
| Falta `--ravendb` | La orden no hace nada. | `Falta la opción --ravendb con la carpeta del código de RavenDB 7.2.7.` | `2` |
| El archivo del informe ya existe, también como carpeta | La orden no hace nada. | `El destino <destino> ya existe.` | `1` |
| La carpeta donde iría el informe no existe o no se puede usar | La orden no hace nada. | `No se puede usar la carpeta <carpeta>: <causa>.` | `1` |
| La carpeta de `--ravendb` no existe o no contiene el código de RavenDB `7.2.7` | La orden no hace nada. | `La carpeta <carpeta> no contiene el código de RavenDB 7.2.7.` | `1` |
| No está un SDK de .NET de la versión 10 | La orden no hace nada. | `No se encuentra el SDK de .NET 10, necesario para medir Voron.` | `1` |
| La carpeta de trabajo no existe | La orden no hace nada. | `La carpeta <carpeta> no existe.` | `1` |
| La carpeta de trabajo no se puede usar | La orden no hace nada. | `No se puede usar la carpeta <carpeta>: <causa>.` | `1` |
| Falla la compilación del programa de Voron | La ejecución se detiene antes de medir y borra su subcarpeta de trabajo. | `No se pudo compilar Voron: <causa>.` | `1` |
| Un motor falla al preparar o al ejecutar una carga (incluidos el disco lleno, una lectura que no encuentra su clave, una suma que no coincide y un proceso que termina sin terminar la carga) | La ejecución se detiene, borra su subcarpeta de trabajo y no escribe informe. | `El motor <motor> falló en la carga <carga>: <causa>.` | `1` |
| No se puede escribir el informe al terminar, también porque ya existe | Borra su subcarpeta de trabajo y no deja un informe a medias. | `No se pudo escribir el informe <archivo>: <causa>.` | `1` |
| La ejecución se interrumpe con Ctrl+C | Igual que un fallo: termina los procesos de los motores, borra su subcarpeta y no escribe informe. | `Banco interrumpido.` | `1` |

`<motor>` es el nombre del motor tal como aparece en el informe; `<carga>`, el nombre de la carga de la tabla cuyo
proceso estaba en marcha, también si el fallo es al abrir, cerrar o medir el tamaño de su almacén, o, si el fallo es
al preparar un almacén de partida, el de la primera carga que lo iba a usar (`lecturas-con-escritor` para W12 y W13);
`<causa>`, el texto del error que dio el motor, la compilación o el sistema.

## Salida

**Salida estándar**, al terminar, una sola línea, con `<archivo>` como en *Validaciones y mensajes*:

```text
Krummi cumple 21 de 22 umbrales. Informe: banco-20261103-101500.md
```

Con `--reducido`: `Ejecución reducida, sin umbrales. Informe: <archivo>`.

**El informe** es un archivo Markdown en español con esta estructura, la misma en Windows y en Linux. En el
ejemplo, las cifras son ilustrativas y las tablas muestran solo algunas filas:

```markdown
# Banco comparativo de Krummi

- Fecha: 2026-11-03 10:15 (UTC+01:00)
- Ejecución: completa
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: <modelo>, <núcleos lógicos> núcleos lógicos
- Memoria: <GiB> GiB
- Disco: <modelo>, <sistema de archivos>
- Krummi: <commit> (con cambios sin confirmar)
- Motores: Voron 7.2.7 (.NET <versión>), LMDB <versión> (heed 0.23.1), redb 4.4.0, canopydb 0.2.5, fjall 3.1.13
- Rust: <versión>
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 21 de 22 umbrales. Incumple: `escrituras-concurrentes-16`.

## Operaciones por segundo

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall | Krummi / Voron | Umbral |
|---|---|---|---|---|---|---|---|---|
| carga-en-orden | 812345 | 790122 | 1012345 | 501234 | 700123 | 1500234 | 1,02 | cumple |
| tamano-fijo-carga | 950000 | 910000 | 1200000 | 600000 | no aplica | no aplica | 1,04 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall |
|---|---|---|---|---|---|---|
| carga-en-orden | 150994944 | 160432128 | 146800640 | 170000000 | 155000000 | 120000000 |

## Opciones cambiadas

- LMDB: tamaño del mapa de 64 GiB; claves duplicadas ordenadas en W19 y W20; claves enteras en W21 y W22.
- redb: caché de 1 GiB.
- canopydb: caché de 1 GiB; diario activado; confirmación que espera al disco.
- fjall: caché de bloques de 1 GiB; llevar al disco tras cada confirmación.
```

Reglas de la forma:

- *Operaciones por segundo* tiene una fila por carga, las 22 en el orden de la tabla de cargas, y *Tamaño del
  almacén* una por cada carga de K9, en el mismo orden. Las columnas de motores son siempre las seis, en el orden de
  K3.
- Las operaciones por segundo y los tamaños son enteros, redondeados al más cercano, sin separador de miles. La razón
  Krummi / Voron lleva dos decimales con coma, **truncada**, no redondeada, de modo que una razón mostrada de `0,80` o
  más siempre cumple. Cuando la métrica de Voron es 0, por ejemplo en W12 si ningún lector de Voron termina una
  lectura mientras escribe el escritor, la razón se muestra `∞` si la de Krummi es mayor que 0 y `0,00` si también es
  0, y el umbral se cumple, porque la comparación de K7 es la métrica de Krummi frente a 0,80 veces la de Voron.
- La columna *Umbral* dice `cumple` o `incumple`. En una ejecución reducida, *Ejecución* dice `reducida`,
  *Repeticiones* dice `1 medida`, *Resultado* dice `Ejecución reducida, sin umbrales.` y la columna *Umbral* dice
  `sin evaluar`; las demás columnas y la tabla de tamaños se muestran igual.
- Con todos los umbrales cumplidos, *Resultado* dice `Krummi cumple 22 de 22 umbrales.` sin la frase *Incumple*; con
  varios incumplidos, la frase los lista entre comillas invertidas, separados por comas, en el orden de la tabla.
- *Fecha* es la del inicio de la ejecución, la misma del nombre predeterminado del informe. *Sistema* es el nombre y
  la versión del sistema operativo; en Linux, la distribución con su versión y la versión del núcleo. La versión de
  LMDB es la de la biblioteca C que compila heed.
- *Krummi* da los 12 primeros caracteres del identificador del último commit de la copia de trabajo y añade
  `(con cambios sin confirmar)` solo cuando tiene cambios en archivos con seguimiento; los archivos sin seguimiento no
  cuentan. Si no hay repositorio o commit, dice `desconocido`. *Disco* es el de la carpeta de trabajo.
- Un dato del equipo que el banco no puede obtener se muestra como `desconocido`.
- *Opciones cambiadas* nombra también cómo lleva cada motor sus confirmaciones al disco cuando el banco lo cambia
  (K3). Tiene una línea por motor con alguna opción cambiada, en el orden de K3, con sus opciones
  separadas por punto y coma; si ningún motor tiene opciones cambiadas, la sección dice `Ninguna.`

## Reutilización

- El generador de claves aleatorias con semilla fija y la salida en tabla de las mediciones existentes de Krummi.
- Los mensajes de carpeta que no existe, carpeta no utilizable y destino que ya existe del padre (*Validaciones y
  mensajes*).
- Las convenciones de salida del padre (*Salida*): resultados en la salida estándar, errores en la de errores, y los
  códigos `0`, `1` y `2`, con la precisión de K13.

## Fuera de alcance

- Optimizar Krummi para cumplir un umbral: cada incumplimiento es un cambio aparte (K13).
- Umbrales de los módulos posteriores al 5: cada módulo con impacto en el rendimiento añade sus cargas y su umbral
  sobre este banco (D5).
- Umbrales de tamaño del almacén (K9).
- Otros motores (sled, RocksDB, SQLite y otros) y otras formas de usar los motores elegidos: Voron a través del
  servidor de RavenDB, LMDB con otra biblioteca, varios escritores optimistas en canopydb o fjall (K3).
- Ajustes por motor más allá de K3: compilación anticipada de .NET, otro asignador de memoria, opciones de rendimiento.
- Modos no durables (el modo diferido de J10 y equivalentes) y el modo temporal (G9).
- Datos que no caben en memoria, caché fría (vaciar la caché del sistema) y discos de red.
- Medir el uso de memoria o de CPU, la recuperación tras una caída y el tamaño del diario por separado.
- Medir la dispersión entre repeticiones o marcar resultados inestables.
- Otras plataformas (D3), y máquinas virtuales, contenedores o WSL como entorno de las ejecuciones completas.

  *Anotación del 2026-11-03: excepción: WSL 2 con Ubuntu, sobre un sistema de archivos ext4 de su propio disco
  virtual, vale como Linux para las ejecuciones completas, como en las mediciones de los módulos anteriores; tiene el
  mismo equipo que la ejecución de Windows (decisión del usuario al empezar la ejecución del módulo).*
- Preparar el equipo (antivirus, modo de energía, procesos en segundo plano) y comprobar de antemano que hay disco y
  memoria suficientes: es responsabilidad del mantenedor.
- Elegir desde la orden qué motores o qué cargas ejecutar, y reanudar una ejecución interrumpida.
- Un límite de tiempo por carga: un motor que se cuelga se interrumpe con Ctrl+C, y en la integración continua lo
  corta el límite del propio trabajo.
- Comprobar automáticamente la durabilidad ante un corte de corriente de cada motor: se revisa su configuración
  (AC-K8).
- Gráficos, historial de ejecuciones, comparación entre informes y salida en otros formatos que Markdown.
- Publicar los resultados.

## Dejado a la implementación

- Cómo el banco controla el programa de Voron y cómo lo compila desde la carpeta de `--ravendb`.
- Cómo se comprueba que la carpeta de `--ravendb` es de la versión `7.2.7`.
- Las opciones que cada motor necesita para cumplir K3 y para ofrecer las estructuras de K6, como el tamaño del mapa
  de LMDB o el orden de bytes de las claves enteras; todas se listan en el informe.
- Cómo prepara el banco un almacén de partida (cargándolo o copiando uno cerrado), siempre que tenga exactamente el
  contenido de K4.
- Cómo se generan los datos con la semilla fija, siempre que todos los motores reciban los mismos bytes en el mismo
  orden (K4).
- El formato del progreso en la salida de errores y el nombre de la subcarpeta de trabajo.

## Criterios de aceptación

Comprueban este módulo. AC19 del padre lo comprueba el mantenedor antes de cada versión (K13).

1. **AC-K1** — Dada una ejecución completa en Windows y otra en Linux, cuando terminan, entonces cada una escribe un
   informe con la estructura de [Salida](#salida), las 22 cargas, los seis motores en sus versiones de K1 y
   `no aplica` exactamente en las celdas de canopydb y fjall de W19 a W22 en la tabla de operaciones y de W19 y
   W21 en la de tamaños, que no tiene filas para W20 ni W22 (K9).
2. **AC-K2** — Dados unos resultados por carga y motor y unos datos del equipo inyectados para la prueba, cuando el
   banco escribe el informe y la línea de la salida estándar, entonces son idénticos byte a byte a los esperados para
   cada caso: todos los umbrales cumplidos, uno incumplido, varios incumplidos, una razón de `0,7996` (que se muestra
   `0,79` e incumple), una razón exacta de `0,80` (por ejemplo 800 frente a 1000, que se muestra `0,80` y cumple), una métrica de Voron de 0 con la de Krummi mayor que 0 (que se muestra `∞` y cumple), las dos de 0 (que se muestra
   `0,00` y cumple), ejecución reducida, sin opciones cambiadas y datos del equipo desconocidos.
3. **AC-K3** — Dados unos resultados inyectados, cuando el banco evalúa cada umbral, entonces dice `cumple` si y solo
   si la métrica de Krummi es al menos 0,80 veces la de Voron sin redondear (K7), también cuando la de Voron es 0, y el código de salida es `0` si
   todas dicen `cumple` y `1` si alguna dice `incumple`.
4. **AC-K4** — Dados unos tiempos y unos números de operaciones por repetición inyectados, también distintos entre
   repeticiones como en W11 y W12, cuando el banco calcula la métrica de una carga, entonces usa la mediana de las
   métricas de las tres repeticiones medidas, por separado para W12 y W13, y descarta la previa; y en una ejecución
   reducida, la métrica de la única repetición (K8, K11).
5. **AC-K5** — Dada una ejecución con `--reducido`, cuando termina, entonces cada motor ejecutó las cantidades de la
   columna *Reducida*, la salida estándar es `Ejecución reducida, sin umbrales. Informe: <archivo>` y el código de
   salida es `0` (K11).
6. **AC-K6** — Dada la integración continua, cuando se ejecuta en Windows y en Linux, entonces ejecuta el banco con
   `--reducido` y falla si no termina con código `0` (K12).
7. **AC-K7** — Dada la semilla fija, cuando se generan los datos de cada carga para cada motor y en dos ejecuciones,
   entonces todos reciben los mismos bytes de claves y valores en el mismo orden (K4).
8. **AC-K8** — Dado cada motor, cuando se revisa cómo lo usa el banco, entonces cada confirmación usa la opción u
   operación que la documentación del motor da para llevar los cambios al medio físico (K3), la revisión queda escrita
   con la cita de esa documentación en el informe de cierre del módulo, y el informe del banco nombra esa opción en
   *Opciones cambiadas* cuando no es la predeterminada.
9. **AC-K9** — Dada cada situación de [Validaciones y mensajes](#validaciones-y-mensajes), cuando se ejecuta la orden,
   entonces muestra su mensaje exacto en la salida de errores, termina con su código, no escribe informe y la carpeta
   de trabajo queda como estaba.
10. **AC-K10** — Dado un motor que falla en una carga, que lee una clave que no encuentra o cuya suma no coincide,
    simulado para la prueba, cuando el banco lo detecta, entonces se detiene con el mensaje de la tabla y la carpeta
    de trabajo queda como estaba.
11. **AC-K11** — Dada una ejecución, cuando se observan los procesos, entonces nunca miden dos motores a la vez, cada
    carga de cada motor corre en su propio proceso (W12 y W13 en uno solo) y el orden de motores y cargas es el de K3.
12. **AC-K12** — Dado un almacén cerrado de contenido conocido, con subcarpetas, cuando el banco mide su tamaño,
    entonces da la suma de las longitudes de todos sus archivos (K9).
13. **AC-K13** — Dada una ejecución sin `--informe`, cuando termina, entonces el informe está en la carpeta actual con
    el nombre de K10 y la subcarpeta de trabajo ya no existe.
14. **AC-K14** — Dada la biblioteca de Krummi, cuando se calcula su árbol de dependencias, entonces no contiene ningún
    paquete del banco ni de los demás motores, y la comprobación de AC21 del padre sigue pasando (K14).

## Anotaciones a otros PRD

Añadidas el 2026-11-03 al [PRD raíz](krummi.prd.md) y a su traducción:

- **PRD raíz, objetivo 3 y D5**: "competitivo" significa los umbrales de K7, frente a Voron; excepción a D5: Voron,
  que no es un motor de Rust, entra en el banco y es la referencia de los umbrales (K2, K7).
- **PRD raíz, D2 y D12**: el banco es una herramienta del mantenedor, no de la superficie pública; puede usar código no
  seguro solo donde la interfaz de otro motor lo exige (K14).
- **PRD raíz, *Salida***: la orden del banco termina con `1` cuando Krummi incumple un umbral (K13).
- **PRD raíz, AC19**: los umbrales vigentes son los de K7; AC19 se exige antes de cada versión, no para cerrar el
  módulo 6 (K13).

## Referencias

- Padre: [Krummi — PRD raíz](krummi.prd.md) (objetivo 3, D2, D5, D12, *Salida*, AC19, AC21).
- Mediciones previas de Krummi: [espacio libre](free-space.prd.md) (F13),
  [escritura agrupada del diario](journal-group-commit.prd.md) (E16), [valores grandes](trees-large-values.prd.md)
  (V11), [árboles multivalor](trees-multivalue.prd.md) (M15), [árboles de tamaño fijo](trees-fixed-size.prd.md) (X13).
- Voron, en el repositorio de RavenDB: <https://github.com/ravendb/ravendb/tree/7.2.7/src/Voron>
- LMDB por heed: <https://github.com/meilisearch/heed>
- redb: <https://github.com/cberner/redb>
- canopydb: <https://github.com/arthurprs/canopydb>
- fjall: <https://github.com/fjall-rs/fjall>

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| e5a00060 | 2026-11-03 07:01 UTC+00:00 | 2 h 18 min | 1 h 29 min | 49 min | 24 min | 25 min | 7 |

<!-- prd-time: [{"session": "e5a00060-eace-41af-8b65-01e51beb6679", "start": "2026-11-03T07:01:57.102000+00:00", "wall": 8284, "discarded": 5367, "agent": 1422, "human": 1493, "interventions": 7, "idle_minutes": 10}] -->
