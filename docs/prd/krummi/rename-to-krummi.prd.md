# Cambio de nombre del producto a Krummi — PRD

**Versión**: 1.0
**Fecha**: 2026-10-23
**Documento padre**: [Krummi, motor de almacenamiento embebido inspirado en Voron — PRD](krummi.prd.md) (v1.1)

## Objetivo y contexto

Cambiar el nombre del producto de **MuninnDB** a **Krummi** en todo lo que ve quien lo usa o lo mantiene: los
paquetes de la biblioteca, los archivos que forman un almacén, la marca de identificación, los mensajes de error, la
documentación y los PRD, las herramientas de las copias de trabajo, el repositorio remoto, la carpeta de la copia
principal y su proyecto de memoria. El comportamiento del motor no cambia en nada más.

**Por qué**: el nombre MuninnDB ya está en uso por otro producto de la misma categoría. Según la comprobación del
usuario del 2026-10-23, <https://muninndb.com/> es "MuninnDB" de Scrypster, una base de datos de memoria para IA,
publicada en PyPI como `muninndb` con licencia source-available. Hay riesgo de confusión y de marca. El usuario
eligió **Krummi**, sin sufijo: es el apodo islandés del cuervo y mantiene el guiño a Voron ("cuervo" en ruso).

**Por qué ahora**: el producto todavía no se ha publicado y solo tiene tres módulos (Pager, transacciones y diario).
Cada módulo nuevo añade nombres, archivos y textos que habría que cambiar después.

Este PRD hereda todo lo del padre, lo del [Pager](pager.prd.md) (P1–P17), lo de las
[Transacciones](transactions.prd.md) (T1–T18) y lo del [Diario](journal.prd.md) (J1–J17); aquí solo se cambian
nombres, la marca y el número de formato. Lo que cambia de reglas anteriores está declarado en sus decisiones y
anotado en los documentos afectados (ver [Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

### Decisiones de producto

| ID | Decisión |
|---|---|
| R1 | **Nombre**: el producto se llama **Krummi**, con la inicial en mayúscula y sin sufijo, en todos los textos, también en los mensajes de la herramienta de línea de comandos y en los errores de la biblioteca. Los nombres de paquetes y de archivos van en minúsculas (R2, R3). |
| R2 | **Paquetes de la biblioteca**: el paquete que incorpora el desarrollador integrador se llama `krummi`, y los dos internos, `krummi-pager` y `krummi-tx`. Los nombres `muninndb`, `muninndb-pager` y `muninndb-tx` dejan de existir, sin paquetes de compatibilidad con el nombre anterior. Los tipos y funciones públicos conservan sus nombres actuales. |
| R3 | **Archivos de un almacén**: el archivo de datos se llama `krummi.data`; el de la creación atómica, `krummi.data.new`; los del diario, `krummi.journal.` seguido del mismo número de 19 cifras; y la subcarpeta del área temporal, `krummi.temp`. Sustituyen a los nombres `muninn.*` en todas las reglas y criterios del Pager, las Transacciones y el Diario, con las mismas reglas. |
| R4 | **Marca de identificación**: la marca de P4 se sustituye por una nueva. Un `krummi.data` que empieza por la marca de MuninnDB no empieza por la marca de Krummi. |
| R5 | **Formato `3`**: el cambio de nombres y de marca es incompatible (D7 del padre), así que la versión de formato vigente pasa a ser `3`. Un almacén de Krummi íntegro con un número menor, incluido `0`, es de formato anterior; esos números solo aparecen en las pruebas. Precisa P5 y J3. |
| R6 | **Almacenes de MuninnDB**: una carpeta con los archivos que dejaba MuninnDB (`muninn.data` y, si se cerró por una caída, archivos `muninn.journal.*` y `muninn.temp`) no contiene un almacén de Krummi: no tiene `krummi.data` y sí otras entradas (P7), así que la apertura falla con la clase de error de que no es un almacén, sin llegar a la comprobación del formato, y no toca nada, tampoco el diario pendiente. Antes de la 1.0 no hay migración y los datos se recrean (D7). Cambia el trato de los restos de MuninnDB: un `muninn.data.new` solo, que antes era una creación interrumpida y se borraba (P8), ahora es una entrada ajena y la carpeta no es un almacén. |
| R7 | **Documentación**: el README, la documentación de la biblioteca, las instrucciones del proyecto, la metodología, la guía de las copias de trabajo, las skills y los hooks del proyecto, la integración continua y los PRD vigentes (este, el padre y los de los módulos 1 a 3, con sus traducciones) pasan a decir Krummi y a usar los nombres de R2 y R3. El README explica el nombre nuevo (Krummi, el apodo islandés del cuervo, y el guiño a Voron) en lugar de la explicación de Muninn, sin nombrar el anterior. En los PRD vigentes cambian los títulos, las reglas, los flujos, los mensajes, las salidas y los criterios, y cada uno lleva una anotación que registra el cambio, sin cambiar su versión. Sus secciones de estado de partida, sus anotaciones anteriores y su tiempo de elaboración son historia: su texto no cambia y los nombres y números de formato de las anotaciones anteriores se leen según R3 y R5; solo se actualizan sus enlaces (R11). Los documentos de ejecución y los informes de cierre de los cambios anteriores también son historia y conservan el nombre de su época. |
| R8 | **Copias de trabajo del repositorio**: las copias de un cambio se llaman `krummi-<n>` (su carpeta y su proyecto de memoria), en lugar de `muninndb-<n>`. Hoy no hay ninguna copia abierta, así que no hay copias con el nombre anterior que conservar. |
| R9 | **Repositorio remoto**: el repositorio de GitHub pasa a llamarse `Krummi` (<https://github.com/jesuslpm/Krummi>). La ejecución de este cambio se hace en la copia principal, sin copia de trabajo, y renombra el repositorio al cerrar, con la autorización explícita del usuario dada al arrancarla, que nombra el repositorio, la operación y la sesión de GitHub del usuario con la que se hace. Orden del cierre: informe de cierre y fusión en la rama principal, como en cualquier cambio; después, renombre en GitHub, cambio de la dirección del remoto en la copia principal y traslado de la memoria (R13); y por último, una confirmación en la rama principal que añade al informe de cierre el resultado de esos tres pasos. Si el renombre falla, la ejecución no cambia la dirección del remoto ni traslada la memoria y el informe los declara pendientes. No se publica (push) nada. Fuera de los restos de R10, ningún archivo del repositorio enlaza la dirección anterior; GitHub la redirige a la nueva. |
| R10 | **Restos permitidos del nombre anterior**: después del cambio, el contenido de los archivos con seguimiento en el repositorio solo nombra MuninnDB o `muninn` en: (a) los documentos de ejecución y los informes de cierre de los cambios anteriores a este; (b) el documento de ejecución, el registro de fases y el informe de cierre de este cambio; (c) este PRD y su traducción; (d) en los PRD vigentes, el texto de sus secciones de estado de partida, sus anotaciones y su tiempo de elaboración (R7); (e) las pruebas que comprueban R4 y R6. Ninguna ruta de carpeta o archivo con seguimiento contiene `muninn`. |
| R11 | **PRD**: la carpeta de los PRD pasa a llamarse `krummi` y el PRD raíz, `krummi.prd.md`, con su traducción `krummi.prd.en.md`; los demás PRD conservan su nombre de archivo. Todos los enlaces de los PRD vigentes, incluidos los de sus secciones históricas y los de este PRD, y los de la documentación vigente apuntan a las rutas nuevas. Los enlaces de los documentos de ejecución y los informes de cierre anteriores a las rutas anteriores quedan rotos: son historia y no se reescriben. |
| R12 | **Carpeta de la copia principal**: la copia principal pasa a estar en `C:\projects\Krummi`, y las instrucciones del proyecto, la guía de las copias de trabajo y la metodología nombran esa ruta. La carpeta la mueve el usuario después de cerrar la ejecución, con el editor cerrado; hasta entonces la documentación ya nombra la ruta nueva. |
| R13 | **Proyecto de memoria**: la copia principal detecta el proyecto de memoria por la dirección del repositorio remoto, así que tras R9 pasa a usar el proyecto `krummi`. Con la autorización del usuario dada al arrancar la ejecución para leer `muninndb` y escribir en `krummi`, la ejecución guarda en `krummi` una copia de cada memoria que tenga `muninndb` en el momento de copiar, incluidas las guardadas durante esta ejecución, con el mismo tipo, título, contenido, ámbito y clave de tema; las copias tienen identificador y fecha nuevos, y las sesiones no se copian, aunque sí sus resúmenes, que son memorias. Repetir la copia tras un fallo a medias solo copia las que faltan. El informe de cierre da el número de memorias copiadas. El proyecto `muninndb` queda como estaba, como archivo. |

*Anotación del 2026-10-23 desde la escritura agrupada y directa del diario
([journal-group-commit.prd.md](journal-group-commit.prd.md)): R5, la versión de formato vigente pasa a ser `4`; un
almacén de formato `3` es de formato anterior (decisión E15).*

### Estado de partida

Verificado el 2026-10-23 sobre el repositorio:

| Aspecto | Situación |
|---|---|
| Producto | Módulos 1 (Pager), 2 (transacciones) y 3 (diario y recuperación) implementados en la rama principal local; según la referencia local, el repositorio remoto (<https://github.com/jesuslpm/MuninnDB>) va 10 confirmaciones por detrás. Nada se ha publicado: los paquetes están marcados como no publicables y sin licencia. |
| Paquetes | `muninndb` (el del integrador), `muninndb-pager` y `muninndb-tx`, en carpetas con esos nombres. |
| Almacén en disco | Formato `2`. Archivos `muninn.data`, `muninn.data.new`, `muninn.journal.<19 cifras>` y `muninn.temp`. La marca de identificación y el identificador interno de los registros del diario contienen el nombre anterior. El motor reconoce un almacén solo por la marca al principio de `muninn.data` (D10, P4). |
| Mensajes | Los errores de la biblioteca, en inglés, dicen "MuninnDB store". La herramienta de línea de comandos no existe todavía (módulo 12): sus mensajes en español solo están en los PRD. |
| Apariciones | `muninn` aparece, sin distinguir mayúsculas, 619 veces en 72 archivos con seguimiento: 267 en los PRD; 91 en 12 documentos de ejecución e informes de cierre; 173 en el código y las pruebas; y 88 en el README, los manifiestos del espacio de trabajo, la guía de las copias de trabajo, la metodología, la integración continua, las herramientas, las skills y los hooks de las copias de trabajo, sus pruebas y las instrucciones del proyecto. Un documento de ejecución y su informe de cierre ya citan el nombre Krummi. |
| Copias de trabajo | Las herramientas, que solo funcionan en Windows, crean copias `muninndb-<n>` con un proyecto de memoria `muninndb-<n>`. No hay ninguna copia abierta. Las herramientas no fijan la ruta de la copia principal; la nombran las instrucciones del proyecto, la guía de las copias de trabajo y la metodología. |
| PRD | Están en la carpeta `muninndb`; el raíz se llama `muninndb.prd.md`. Los documentos de ejecución y los informes de cierre los enlazan 15 veces por esa ruta, y las secciones de estado de partida de los tres PRD de módulo enlazan el raíz. |
| Memoria | La copia principal está en `C:\projects\MuninnDB`. La herramienta de memoria informa que detecta su proyecto, `muninndb`, por la dirección del repositorio remoto. El proyecto tiene 21 memorias, 7 sesiones y ningún mensaje guardado. La herramienta no tiene forma de renombrar un proyecto ni de fusionar dos con nombres distintos. |
| Nombre nuevo | Según la comprobación del usuario del 2026-10-23, `krummi` estaba libre en crates.io y una búsqueda rápida en la web no encontró bases de datos con ese nombre. No se ha hecho una comprobación de marca. |

## Actores

- **Desarrollador integrador**: incorpora el paquete `krummi` en lugar de `muninndb` y ve el nombre nuevo en los
  errores.
- **Operador**: encuentra los archivos `krummi.*` en la carpeta del almacén y, cuando exista la herramienta, sus
  mensajes con el nombre nuevo.
- **Mantenedor**: trabaja con los paquetes, la documentación, las copias de trabajo, el repositorio remoto y la
  memoria con el nombre nuevo; al arrancar la ejecución autoriza el renombre del repositorio y la copia de la
  memoria, y al terminar mueve la carpeta de la copia principal.

## Flujo

1. El desarrollador integrador declara la dependencia `krummi` y usa la biblioteca como hasta ahora.
2. Los almacenes nuevos y los que se abren después se rigen por las reglas de los módulos 1 a 3 con los archivos de
   R3, la marca de R4 y el formato de R5.
3. Al abrir una carpeta con un almacén de MuninnDB, la apertura falla porque no contiene un almacén de Krummi, sin
   tocar nada (R6).
4. Al cerrar la ejecución de este cambio, el repositorio remoto y la memoria pasan al nombre nuevo (R9, R13); después,
   el mantenedor mueve la carpeta de la copia principal (R12).

## Validaciones y mensajes

Las situaciones, reglas y clases de error son las del padre y de los módulos 1 a 3. Solo cambian los nombres de los
archivos (R3) y, en los mensajes, MuninnDB por Krummi; por ejemplo, el de la fila de carpeta que no es un almacén
pasa a ser:

```text
La carpeta <carpeta> no contiene un almacén de Krummi.
```

Estas filas precisan cómo se aplican las reglas existentes a los restos del nombre anterior:

| Situación | Regla | Clase de error |
|---|---|---|
| La carpeta no tiene `krummi.data` y tiene cualquier entrada `muninn.*` (`muninn.data`, `muninn.data.new`, `muninn.journal.*`, `muninn.temp`), sola o con otras | No es un almacén; no se toca nada ni se recupera el diario pendiente (R6, P7). | No es un almacén |
| `krummi.data` empieza por la marca de MuninnDB | No empieza por la marca (R4). | No es un almacén |

Dentro de la carpeta de un almacén de Krummi, las entradas `muninn.*` son archivos ajenos (P7). Los errores de la
biblioteca, en inglés, que hoy nombran el producto pasan a nombrar Krummi; su texto exacto sigue dejado a la
implementación (D8 del padre).

## Salida

La salida no cambia de forma; solo cambian los nombres. En disco, la carpeta de un almacén abierto con
confirmaciones sin trasladar contiene, por ejemplo:

```text
krummi.data
krummi.journal.0000000000000000001
krummi.temp\
```

## Reutilización

- Todas las reglas del padre y de los módulos 1 a 3: este cambio solo sustituye nombres, la marca y el número de
  formato.
- La comprobación de formato anterior y posterior de D7, P5 y J3, que pasa a comparar con `3`.

## Fuera de alcance

- La comprobación de marca de Krummi, la reserva del nombre en crates.io y la publicación de los paquetes: se deciden
  antes de la primera publicación, en el PRD de licencia y publicación que ya prevé el padre.
- Abrir, convertir o migrar almacenes de MuninnDB (D7: antes de la 1.0 no hay migración), y un mensaje propio que
  los distinga de una carpeta cualquiera que no es un almacén.
- Un paquete `muninndb` de compatibilidad que reexporte `krummi`.
- El nombre del ejecutable de la herramienta de línea de comandos y de sus órdenes, que fija el módulo 12.
- Cualquier otro cambio del formato en disco (tamaño de página, encabezado, diario) o del comportamiento del motor.
- Reescribir la historia del repositorio: confirmaciones, ramas y etiquetas existentes, documentos de ejecución e
  informes de cierre de los cambios anteriores, incluidos sus enlaces rotos (R11).
- La descripción, los temas y demás datos del repositorio de GitHub distintos de su nombre.
- Cambiar el texto de las memorias, borrar el proyecto de memoria `muninndb` y copiar sus sesiones (R13).
- Los clones del repositorio en otros equipos.
- Lo que el editor guarda ligado a la ruta `C:\projects\MuninnDB` (el historial de sesiones de Claude Code y su
  memoria automática del proyecto), que no se traslada a la ruta nueva (R12).
- Los archivos sin seguimiento que se regeneran (el registro de skills y la carpeta de compilación).
- El nombre de archivo de los PRD que no son el raíz.
- Que las herramientas de las copias de trabajo funcionen en Linux.

## Dejado a la implementación

- Los bytes de la marca nueva y su longitud (ya dejados por el Pager), incluido si contienen el texto `Krummi`, y el
  identificador interno de los registros del diario (organización interna del diario, ya dejada por el módulo 3).
- El texto en inglés de los errores de la biblioteca (D8), siempre que nombre Krummi y no MuninnDB.
- Cómo las pruebas construyen los archivos de MuninnDB para comprobar R4 y R6.
- Cómo se resuelven los avisos de conflicto que la herramienta de memoria dé al guardar las copias (R13), siempre
  que no se pierda ni se cambie ninguna memoria, y si se conserva su marca de fijada.

## Criterios de aceptación

Este cambio no añade criterios del padre ni de los módulos 1 a 3: sus pruebas y los barridos completos de AC10
siguen pasando con los nombres, la marca y la versión de formato nuevos. AC9 se adapta a R5: formato anterior con
`0` y `2`, posterior con `4`.

Y además:

1. **AC-R1** — Dada una carpeta que no existe, cuando se abre, entonces contiene solo `krummi.data`, que empieza por
   la marca nueva, y la información del almacén dice formato `3`; y con confirmaciones sin trasladar, la carpeta
   contiene además archivos `krummi.journal.` seguido de 19 cifras y `krummi.temp`, que desaparecen al cerrar sin
   error (R3, R5).
2. **AC-R2** — Dadas carpetas con los archivos que dejaba MuninnDB (una con un `muninn.data` con la marca de
   MuninnDB y formato `2`; otra con ese `muninn.data` y archivos `muninn.journal.*` con confirmaciones; y otras tres
   cuyo único contenido es `muninn.data.new`, un archivo `muninn.journal.0000000000000000001` o una carpeta
   `muninn.temp`), cuando se abren, entonces fallan con la clase de error de que no es un almacén y el contenido de
   las carpetas queda idéntico byte a byte (R6).
3. **AC-R3** — Dado un `krummi.data` que empieza por la marca de MuninnDB y por lo demás es un almacén válido, cuando
   se abre, entonces falla con la clase de error de que no es un almacén y queda idéntico byte a byte (R4).
4. **AC-R4** — Dado un almacén de Krummi cuya carpeta contiene además `muninn.data`, un archivo
   `muninn.journal.0000000000000000001` y una carpeta `muninn.temp` con un archivo, cuando se abre, se escribe, se
   confirma y se cierra, y también cuando se cierra por una caída con confirmaciones sin trasladar y se reabre, que
   lo recupera, entonces esas tres entradas quedan idénticas byte a byte (R3, P7, J8, J14).
5. **AC-R5** — Dado un proyecto Rust nuevo que declara la dependencia `krummi`, cuando crea un almacén con la
   biblioteca, confirma una escritura y lo reabre, entonces compila y lee lo escrito; los ejemplos de la
   documentación de la biblioteca compilan y pasan; y el espacio de trabajo tiene solo los paquetes `krummi`,
   `krummi-pager` y `krummi-tx` (R2).
6. **AC-R6** — Dados los errores de la biblioteca que hoy nombran el producto (no es un almacén y formato distinto),
   cuando se muestran como texto, entonces contienen `Krummi` y no contienen `muninn` sin distinguir mayúsculas
   (R1).
7. **AC-R7** — Dado el repositorio después del cambio, cuando se busca `muninn` sin distinguir mayúsculas en el
   contenido y en las rutas de los archivos con seguimiento, entonces no aparece en ninguna ruta, y en el contenido
   solo aparece en los archivos de R10 (a), (b), (c) y (e) y en los demás PRD vigentes; en estos, cada línea que
   lo contiene está en una sección *Estado de partida* o *Tiempo de elaboración*, o en una anotación (párrafo o
   punto de lista en cursiva, o sección *Anotaciones a otros PRD*) (R7, R10).
8. **AC-R8** — Dado el README, las instrucciones del proyecto, la guía de las copias de trabajo y la metodología,
   cuando se leen, entonces el README explica el nombre Krummi, y los demás nombran la ruta `C:\projects\Krummi` y
   las copias `C:\wt\krummi-<n>` (R7, R8, R12).
9. **AC-R9** — Dadas las herramientas de las copias de trabajo, en Windows, cuando se crea una copia del cambio `x`,
   entonces su carpeta y su proyecto de memoria se llaman `krummi-x`, y sus pruebas pasan (R8).
10. **AC-R10** — Dada la copia principal al cerrar la ejecución, cuando se consulta la dirección de su repositorio
    remoto, entonces es <https://github.com/jesuslpm/Krummi>, y GitHub informa que el repositorio
    `jesuslpm/Krummi` existe; o, si el renombre falló, la dirección no cambió y el informe de cierre lo declara
    pendiente (R9).
11. **AC-R11** — Dados los PRD vigentes, el README, las instrucciones del proyecto, la metodología y la guía de las
    copias de trabajo, cuando se siguen sus enlaces relativos y sus anclas, entonces ninguno está roto, y el PRD raíz
    es `krummi.prd.md` en la carpeta `krummi` (R11).
12. **AC-R12** — Dada la copia principal al cerrar la ejecución, cuando se consulta el proyecto de memoria que
    detecta, entonces es `krummi` y contiene una memoria con el mismo tipo, título, contenido, ámbito y clave de tema
    por cada una de las que tenía `muninndb` al copiar, el informe da ese número, y `muninndb` conserva las suyas;
    o, si el renombre falló, el informe declara pendiente la copia (R13).
13. **AC-R13** — Las pruebas de todos los paquetes y los barridos completos de AC10 pasan en Windows x64 y en Linux
    x86-64 con glibc (AC18); las pruebas en Python de los hooks y las skills pasan en los dos, y las de las
    herramientas de las copias de trabajo, en Windows.

## Anotaciones a otros PRD

El padre y los PRD de los módulos 1 a 3, con sus traducciones, llevan una anotación del cambio de nombre y se
actualizan según R7 y R11. La anotación dice además que los nombres de archivo y los números de formato de las
anotaciones anteriores se leen según R3 y R5. En particular:

- Padre: el producto se llama Krummi (R1); un almacén de MuninnDB no es un almacén de Krummi (R6).
- Pager, Transacciones y Diario: todas las apariciones de los nombres de archivo pasan a ser las de R3, y la marca de
  P4 se sustituye (R4).
- Pager, P5, y Diario, J3: la versión de formato vigente pasa a ser `3` (R5).
- Pager, P8: un `muninn.data.new` solo ya no se borra como creación interrumpida; es una entrada ajena (R6).

## Referencias

- Padre y raíz: [krummi.prd.md](krummi.prd.md) v1.1
- Módulos: [pager.prd.md](pager.prd.md), [transactions.prd.md](transactions.prd.md), [journal.prd.md](journal.prd.md)
- Producto con el nombre anterior: <https://muninndb.com/>

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 5def7028 | 2026-10-23 08:13 UTC+01:00 | 24 min | 0 min | 24 min | 21 min | 3 min | 4 |

<!-- prd-time: [{"session": "5def7028-6d5a-4538-82c1-c736b443b166", "start": "2026-10-23T07:13:58.479000+00:00", "wall": 1426, "discarded": 0, "agent": 1265, "human": 161, "interventions": 4, "idle_minutes": 10}] -->
