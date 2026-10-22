# Cómo se generó el PRD: un ejemplo práctico

Este documento reconstruye, paso a paso, cómo se escribieron con un asistente de IA los PRD de este
repositorio: el [PRD general](../prd/chicago-crime/chicago-crime.prd.md), el
[PRD del generador](../prd/chicago-crime/generador.prd.md) y el
[PRD del consultor](../prd/chicago-crime/consultor.prd.md). Está pensado como material para una sesión
práctica sobre desarrollo asistido por IA.

Se lee en orden:

1. el contexto previo, una conversación con ChatGPT;
2. el pedido inicial;
3. cómo trabajó el asistente;
4. cada decisión de producto, con la pregunta, la respuesta del usuario y lo que se verificó;
5. la revisión final;
6. el PRD hijo del generador;
7. el resultado;
8. un segundo ejemplo: el PRD hijo del consultor, escrito un día después en otra conversación;
9. las lecciones.

Las intervenciones del usuario se citan **literalmente**, con sus erratas. Las del asistente están
resumidas.

---

## 1. Punto de partida: la conversación previa con ChatGPT

Antes de abrir el PRD, el usuario había explorado la solución técnica con ChatGPT y guardó un resumen en
[`conversacion_rust_csv_foundationdb_sstable.md`](../../conversacion_rust_csv_foundationdb_sstable.md).
Esta es la parte relevante, sin la guía de Rust para programadores de C# ni los fragmentos de código.

**Objetivo.** Procesar un CSV de delitos de Chicago que no cabe en RAM, ordenarlo por `ID` numérico
ascendente y generar una SSTable con:

- **Clave:** una tupla FoundationDB con el `ID`.
- **Valor:** una tupla FoundationDB con los campos del CSV convertidos a tipos adecuados.
- **Restricción:** un objetivo de memoria de **512 MiB**, sin construir un ordenador externo complejo.

**El dataset.** Es *Crimes — 2001 to Present*, del portal de datos de Chicago, con 22 columnas:

```csv
"ID","Case Number","Date","Block","IUCR","Primary Type","Description","Location Description","Arrest","Domestic","Beat","District","Ward","Community Area","FBI Code","X Coordinate","Y Coordinate","Year","Updated On","Latitude","Longitude","Location"
```

**Decisiones de diseño propuestas.**

| Tema | Propuesta |
|---|---|
| Orden | Comparar el `ID` como entero (`2 < 10 < 100`), no como texto. La codificación de tuplas de FoundationDB conserva ese orden al comparar bytes. |
| Ordenación externa | Usar un crate de ordenación externa (`extsort`) con segmentos de 250.000 registros y disco temporal. |
| Representación | Codificar la tupla del valor al leer y ordenar una estructura mínima `{ id, value }`, sin 22 cadenas por fila. |
| Tipos | Los códigos (`IUCR`, `Beat`, `District`) se guardan como texto, para conservar los ceros iniciales. Hay booleanos, enteros y decimales opcionales, y fechas como timestamp. |
| SSTable | Claves en orden estrictamente creciente; un ID duplicado se rechaza. |
| Memoria | Unos 512 bytes por registro como margen, que con 250.000 registros por segmento dan unos 122 MiB. Esta estimación **no estaba medida**. |

**Lo que quedó pendiente.** Estos cinco puntos son justamente lo que el PRD tenía que resolver:

- el formato exacto de la SSTable (LevelDB, RocksDB u otro);
- si el valor omite el `ID`, que ya está en la clave;
- si se conserva `Location`, que duplica las coordenadas;
- la política de fechas, de nulos y de registros erróneos;
- verificar las API reales de los crates.

> **Para la sesión:** la conversación con ChatGPT es diseño técnico, el *cómo*. El PRD se ocupa del
> *qué* y el *por qué*. El primer valor de este ejercicio es separar esas dos cosas.

---

## 2. El pedido inicial

> Haz un PRD general para hacer un par de aplicaciones Rust. Una que genere los SSTable uno principal por
> el ID y otros SSTables que sean índices secungarios. La segunda es para hacer consultas sobre los datos.
> Los datos están en chicago-crime-data.csv y mayor contexto tengo una conversación con ChatGPT
> resumidaen conversation_rust_csv_foundationdb_sstable.md. Se admiten sugerencias y preguntas.

Es un pedido corto y abierto. Lo que lo vuelve trabajable es la invitación final: *se admiten
sugerencias y preguntas*.

---

## 3. Cómo trabajó el asistente

### 3.1 Cargó una skill de PRD con reglas explícitas

El repositorio tiene una skill de proyecto, [`.claude/skills/prd`](../../.claude/skills/prd/SKILL.md), con
una plantilla y estas reglas:

- **Qué y por qué, en lenguaje de negocio.** Sin clases, rutas ni detalles de implementación.
- **Estado de partida verificado.** Se comprueba en el código y en los datos, nunca se supone.
- **Fuera de alcance obligatorio.** Es la línea desde la que arranca el siguiente cambio.
- **Una pregunta por vez.** Solo cuando la decisión de producto es del usuario; nunca se inventa.
- **Escribir el PRD no autoriza a implementar.**

### 3.2 Primera pregunta: el idioma

No había ningún PRD previo que fijara la convención, así que la skill obliga a preguntar.

> **Asistente:** ¿En qué idioma querés el PRD: español (neutro y profesional) o inglés?
>
> **Usuario:** español

### 3.3 Medir antes de escribir

Antes de redactar el "Estado de partida", el asistente recorrió el CSV completo con un script, en lugar
de suponer nada:

| Medición | Resultado |
|---|---|
| Registros | 8.656.904, con 22 columnas en todas las filas |
| IDs | Todos enteros (de 634 a 14.353.993), **sin duplicados** |
| Orden | **No** viene ordenado por ID: 2,5 millones de saltos hacia atrás |
| Número de caso | **No es único**: 632 repeticiones |
| Vacíos | Ward: 614.809 · Área comunitaria: 613.720 · Coordenadas: 100.851 |
| Cardinalidades | Tipo principal: 34 · Distrito: 26 · Área comunitaria: 78 |

Estos números cambiaron el PRD. El número de caso no es único, así que su índice tiene que devolver
varios registros. Y como hay campos vacíos, se decidió indexarlos como "sin valor" para que también se
puedan consultar.

### 3.4 Borrador con decisiones abiertas

El asistente escribió un borrador v0.1 completo y dejó seis **decisiones abiertas**, D1 a D6, cada una
con su recomendación. A partir de ahí la conversación fue cerrar una decisión por vez.

---

## 4. Las decisiones, una por una

### D1: formato de las SSTables

> **Asistente:** ¿Las SSTables las va a leer solo nuestra aplicación de consulta, o también tienen que
> poder cargarse en RocksDB u otro motor? *(Para ser compatible con RocksDB, en Windows hay que compilar
> C++ con LLVM.)*
>
> **Usuario:** Las SSTables de LevelDb creo que el paquete sstable es compatible con LevelDB

**Verificación.** El asistente no aceptó la afirmación sin comprobarla. El README del crate no decía
nada sobre compatibilidad, así que leyó su código fuente. El número mágico del pie
(`0xdb4775248b80fb57`), el CRC32C enmascarado de cada bloque y la compresión Snappy son los de LevelDB.
El usuario tenía razón, con un matiz: un archivo de *tabla* LevelDB no es una *base de datos* LevelDB.
Ese matiz pasó a "Fuera de alcance".

**Resultado:** formato de tabla de LevelDB, generado con Rust puro.

### D2: índices secundarios

> **Asistente:** ¿Te sirven como índices iniciales fecha, tipo principal, distrito, área comunitaria y
> número de caso? *(Arrest y Domestic solo valen sí o no, y filtran poco.)*
>
> **Usuario:** Sí me sirven los índices

### D3: política de fechas

**Verificación previa.** El asistente midió cuántas fechas caen en las horas conflictivas del cambio de
horario. **253** registros tienen una hora que no existe en Chicago, por el salto de primavera, y
**1.663** caen en la hora que se repite en otoño.

> **Asistente:** ¿Guardamos las fechas como hora local de Chicago, tal como vienen, sin convertir a UTC?
>
> **Usuario:** Sí hora local de chicago. Ahí tenían que haber puesto la hora UTC y ahí lo pagan: horas
> que no existen y horas ambiguas.

**Resultado:** se guarda la hora civil local, y el PRD registra la consecuencia aceptada: las fechas no
son instantes absolutos.

### D4: qué campos guarda el valor

El asistente proponía omitir del valor el `ID`, que ya está en la clave, y `Location`, que se
reconstruye desde las coordenadas.

> **Usuario:** El ID ocupa poco y luego no sería más fácil generar un json o un csv o incluso ahorrar
> memoria ram. A lo mejor compensa por memoria simplicidad y CPU.

**Verificación.** El asistente midió el tamaño codificado sobre las 8,66 M filas:

| Campo | Coste en el valor |
|---|---|
| ID | 33 MiB (2,9 %) |
| Location | 251 MiB (21,7 %) |

Le dio la razón en el tamaño y en la simplicidad: con el ID dentro, el valor es autocontenido. Pero
**corrigió el argumento de la RAM**: guardar el ID agrega 4 bytes por registro en lugar de ahorrarlos, y
el efecto en la CPU es despreciable.

> **Usuario** (mientras el asistente verificaba): Lo del location no lo compruebes, es una redundancia en
> los datos de entrada.

**Resultado:** el ID se queda en el valor y Location se omite.

### D5: registros inválidos

> **Asistente:** ¿Política estricta (abortar al primer error) o tolerante (omitir y anotar en el log)?
> *(Hoy el CSV tiene cero errores.)*
>
> **Usuario:** Un registro no válido se escribe en el log y se ignora, siempre hay datos malos de entrada
> en la vida real si vamos a abortar el proceso por eso no vamos a poderle sacar partido a los datos
> buenos, la vida no es ideal es imperfecta

El asistente aplicó ese criterio, pero separó dos clases de error. Lo que afecta a **todo el archivo**,
como una cabecera distinta o un disco lleno, sigue abortando. Después propuso un límite de rechazos
para protegerse de un archivo roto.

> **Usuario:** Sin límite de rechazos. Por cierto detectar un duplicado que estén muy lejos unos de otros
> es difícil en este enfoque, creo yo. Quizá podríamos detectar duplicados en el segmento y tomar el
> primero.

**Verificación.** La premisa no era correcta. Con ordenación externa, la fusión final produce un único
flujo ordenado por ID, así que dos duplicados quedan **contiguos** por lejos que estuvieran en el CSV.
Detectarlos solo dentro de cada segmento, en cambio, dejaría pasar los duplicados entre segmentos.

> **Usuario:** Bien entonces sí que podemos detectar duplicados despues de la ordenación merge, tomamos el
> primero e ignoramos el segundo y lo escribimos en el log.

**Resultado:** se omite y se anota en el log, sin límite de rechazos. Ante un duplicado se conserva la
primera aparición en el archivo. El PRD registra también la consecuencia aceptada: un archivo roto
podría generar un conjunto casi vacío.

### D6: cómo se usa el consultor

> **Usuario:** Qué es D6?

El asistente explicó las dos opciones con ejemplos: comandos de una sola ejecución o un modo interactivo
tipo consola.

> **Usuario:** En principio sí, comandos de una sola ejecución.

Con D6 cerrada, el PRD general pasó a la **versión 1.0**.

---

## 5. Revisión final del usuario

Con el documento cerrado, el usuario pidió ajustes precisos para eliminar ambigüedades. El texto que
pegó, resumido:

- **Publicación atómica:** una generación fallida, *incluida una interrupción inesperada del proceso*,
  no debe dejar visible un conjunto incompleto ni invalidar el anterior. *No es necesario garantizar
  durabilidad frente a cortes de alimentación.*
- **Límite de memoria:** los 512 MiB son el *pico de memoria privada comprometida del proceso*, sin la
  caché de archivos del sistema, y tiene que poder *medirse objetivamente en Windows*.
- **Dataset de referencia:** las cifras exactas de AC1, AC4 y AC5 corresponden al CSV analizado. Con
  otro CSV, los resultados deben corresponder a los registros válidos procesados.
- *No introduzcas decisiones de implementación… Haz los cambios mínimos necesarios.*

Y luego:

> **Usuario:** No quiero que el límite de memoria cuando se sobrepasa mate el proceso, simplemente una
> escritura en el log.

El asistente verificó primero que el PRD no decía que el proceso se matara. Pero el pedido destapaba una
ambigüedad real, así que la cerró: superar el límite solo anota un aviso. El PRD quedó en la **v1.2**.

---

## 6. El PRD hijo del generador

> **Usuario:** Ahora quisiera que escribieras el PRD para el generador tomando como padre el
> chicago-crime.prd.md. La idea es que cuando empezamos una aplicación o un módulo grande creamos el PRD
> padre y luego en cada cambio un PRD hijo, no sé si en este caso el PRD padres está tan bien
> especificado que un PRD hijo para el generador no tenga mucho sentido, pero ese el el flujo a seguir.

El asistente dio su opinión: el hijo tenía sentido, pero tenía que ser **delgado**. Debía remitir al
padre en lugar de repetirlo y aportar solo lo que el padre dejaba abierto:

- el límite de la entrega: solo el generador;
- cómo verificar sin consultor;
- detalles operativos: log, códigos de salida, Ctrl+C y restos de ejecuciones interrumpidas.

Propuso cinco decisiones con recomendación: G1 log, G2 compresión, G3 códigos de salida, G4 huella del
CSV y G5 salida determinista.

> **Usuario:** Sí, todolo que propones, sugieres, recomiendas me parece bien.

---

## 7. Resultado

| Documento | Versión al terminar esta conversación |
|---|---|
| [PRD general](../prd/chicago-crime/chicago-crime.prd.md) ([inglés](../prd/chicago-crime/chicago-crime.prd.en.md)) | 1.2: decisiones D1–D6, criterios AC1–AC13 |
| [PRD del generador](../prd/chicago-crime/generador.prd.md) ([inglés](../prd/chicago-crime/generador.prd.en.md)) | 1.0: decisiones G1–G5, criterios AC-G1–AC-G10 |

A partir de esos PRD se implementó el generador con ODD. Su aceptación sobre el CSV real dio **14/14
PASS**, y los números de los criterios coincidieron con las mediciones hechas durante el PRD.

Hubo una decisión que **no** entró al PRD: dividir el código en un workspace de Cargo con una biblioteca
compartida. Es el *cómo*, y quedó en el documento de implementación.

---

## 8. Segundo ejemplo: el PRD del consultor

Un día después, en otra conversación, se escribió el tercer PRD: el hijo para el consultor. Ya existían el
padre y el generador implementado, así que el pedido fue mínimo.

> **Usuario:** vamos a hacer un nuevo cambio, empezando por el PRD. Se trata de implementar el consultor,
> el crime query, ya tenemos el PRD de producto, el PRD padre chicago-crime.prd.md

Esta vez el asistente siguió una preferencia que el usuario había dejado guardada: hacer **todas** las
preguntas de producto durante el PRD, para que la implementación posterior no se detuviera nunca.

### 8.1 Primera ronda: decisiones con opciones

El asistente leyó el padre, el PRD del generador y el contrato del conjunto, escribió un borrador y fue
preguntando decisión por decisión, cada vez con su recomendación y una vista previa.

| Decisión | Pregunta | Respuesta del usuario |
|---|---|---|
| C4 | ¿Qué muestra la salida en tabla? | **"Todos en columnas"**, en contra de la recomendación (resumen más ficha) |
| C1 | ¿Cómo se comparan los valores? | La recomendada: sin distinguir mayúsculas y sin completar ceros (`theft` = `THEFT`, pero `8` ≠ `008`) |
| C3 | ¿Qué campos sin índice admiten condiciones? | La recomendada: todos salvo Block y Description |
| C7 | ¿Qué exigencia de memoria tiene? | La recomendada: acotada sin cifra; no crece con el tamaño del resultado |
| C2, C5, C6 | Fechas, canales de salida y códigos de salida | Las tres confirmadas juntas, porque seguían convenciones ya establecidas |

Con eso el PRD quedó en la v1.0. Pero el usuario no lo dio por cerrado.

### 8.2 El usuario abre funcionalidad nueva

> **Usuario:** No quisiera que se pudiera elegir el formato de salida json o csv y guardalo en un archivo
> de salida, en caso de json, que contenga la consulta que se ha hecho.

El mensaje era ambiguo. El asistente explicitó cómo lo interpretaba: "no traduzcas todavía; primero
quiero esto". Pidió que lo corrigiera si no era así, y con eso salieron dos decisiones nuevas:

- **C8, archivo de salida:** si el archivo existe, el usuario eligió **"Sobrescribir siempre"**, otra vez
  en contra de la recomendación.
- **C9, consulta dentro del JSON:** "Interpretada", es decir, con los rangos de fecha ya resueltos.

> **Usuario:** No, sigamos discutiendo la funcionalidad del consultor. No creo que sea buena idea poner una
> condición de un campo no indexado sin poner un and con otro campo indexado, porque la candidad de
> registros puede ser enorme. A propósito, me gustaría poder poner condiciones con and y hacer una
> intersección del resultado de los índices, en caso de no tener índice por el campo un filtro de los
> resultados. No sé si me explico.

Así nació **C10**:

- cada condición con índice aporta su lista de ID;
- con varias condiciones, el resultado son los ID comunes a todas;
- las condiciones sin índice solo filtran ese resultado;
- una consulta sin ninguna condición con índice se rechaza.

Esto contradecía al padre, que permitía recorrer la tabla entera con un aviso. El asistente no reescribió
el padre: **lo anotó** con fecha, como manda la skill. Y avisó de una consecuencia que quedó aceptada: una
condición con índice acota el recorrido, pero no siempre lo hace chico.

### 8.3 El orden de los resultados: el asistente se equivoca

> **Usuario:** El PRD dice esto: "Los resultados salen en orden ascendente de ID. Con límite, son los N
> primeros ID que cumplen la consulta." eso obligaría a ordenar y no lo considero necesario porque los ID
> en realidad no tienen ningún significado para el analista

El asistente defendió mantener el orden por ID. Su argumento fue que cada índice ya entrega los ID
ordenados, así que no costaba nada. El usuario respondió con texto libre, sin elegir ninguna opción:

> **Usuario:** Quiero discutir esto, porque no es verda que quedaran ordenados por ID, si tienes varios
> valores no queda ordenados por ID sino por el orden del índice, no digamos si tienes tienes condiciones
> AND

**Verificación.** El asistente lo comprobó contra el contrato y le dio la razón. La clave de un índice es
`(valor, ID)`, así que con varios valores los resultados salen agrupados por valor, no por ID. **Reconoció
el error y cambió su recomendación.**

> **Usuario:** En realidad si quieres resultados estables debes indicar el orden en el que quieres que
> salgan cuando pones un límite, si no pones límite da igual aunque los resultados no estén ordenados igual
> el conjunto es el mismo. Es cualdo pones SELECT TOP (x) es obligatorio ORDER BY.

El asistente aceptó el principio, con un matiz técnico: en SQL Server, `TOP` sin `ORDER BY` es válido y
devuelve filas arbitrarias; el que exige `ORDER BY` es `OFFSET … FETCH`. Después preguntó por qué campos se
podía ordenar, y la respuesta volvió a ser texto libre:

> **Usuario:** Puede ordenar por cualquier campo, y para que sea estable se añade el ID. Pero si no se
> indica ningún orden ni límite se admite inestabilidad.

Ordenar 1,8 millones de resultados sin límite chocaba con C7, que pedía memoria acotada. Para resolverlo
había que ordenar en disco:

> **Usuario:** Usar disco temporal cuando se sobrepasa un límite de memoria sería lo ideal, aunque no sé que
> complejidad podría tener

El asistente revisó primero qué existía y encontró que el generador ya tenía una ordenación externa
probada que se podía reutilizar. Con eso estimó la complejidad como baja.

### 8.4 Mayúsculas y minúsculas: un supuesto que nadie había medido

> **Usuario:** Lo de no distinguir mayúsculas de minúsculas no lo veo posible si el generador no ha hecho
> la conversión a minúsculas o mayúsculas.

**Verificación.** El usuario tenía razón: las claves de los índices se comparan byte a byte. Además, el
asistente reconoció que su C1 original se apoyaba en un supuesto que nunca había comprobado: que todos los
textos venían en mayúsculas. Recorrió el CSV entero:

- los campos consultables, efectivamente, venían siempre en mayúsculas;
- Block tenía 13.886 filas en minúsculas, aunque Block no admite condiciones;
- con un extracto futuro que trajera minúsculas, la búsqueda habría dado conteos incompletos sin ningún
  error.

> **Usuario:** Yo preferiría que los índices secundarios de texto el generado convirtiera a minúsculas,
> pero el primario debería estar tal cual en el CSV, de manera que el filtro puede comparar convirtiendo a
> minúsculas tanto el texto buscado como el campo.

Esa decisión tuvo un alcance mayor que el propio consultor:

- obligó a cambiar el **generador**, el verificador independiente y el formato publicado, que pasó al
  **esquema 2**;
- quedaron anotados el padre y el PRD del generador;
- el asistente la marcó como de **alto riesgo** para la implementación, porque cambia datos ya publicados.

Un PRD hijo, entonces, puede obligar a modificar a un hermano.

### 8.5 Una duda del usuario, verificada con datos

> **Usuario:** He leído en la conversación que THEFT aparece en todos los registro? en qué campo? es esto
> verdad?

El asistente lo verificó contra el CSV antes de contestar:

- `THEFT` aparece solo en Primary Type, en 1.838.502 registros de 8.656.904 (21,2 %);
- la frase que confundió era "`theft`, `Theft` y `THEFT` dan los mismos 1.838.502", que habla del conteo,
  no de que esté en todos los registros.

### 8.6 Recortar el alcance: la versión 1.1

En la sesión siguiente, antes de implementar, el usuario recortó el alcance. Primero descartó un
planificador de consultas por costes:

> **Usuario:** Sí, ya iremos añadiendo cosas más adelante, de momento prefiero algo que no requiera
> demasiado.

Después, cuando el asistente señaló que lo más costoso del PRD era ordenar por cualquier campo con
ordenación externa en disco:

> **Usuario:** Dejemos entonces C11 y C7 fuera de alcance de momento, podemos hacerlo en otro cambio, vamos
> más poco a poco, y dejamos de garantizar la estabilidad de momento.

El PRD pasó a la **v1.1**:

- **C11:** el orden no está especificado y el analista no puede pedirlo; un límite N devuelve N registros
  cualesquiera;
- **C7:** sin ordenación externa; solo rige que la memoria no crece con el tamaño del resultado;
- todo lo demás pasó a "Fuera de alcance", como base del siguiente cambio.

### 8.7 Resultado

El [PRD del consultor](../prd/chicago-crime/consultor.prd.md) v1.1 tiene once decisiones (C1–C11) y 18
criterios de aceptación (AC-C1 a AC-C18). Se implementó con ODD y su aceptación sobre el CSV real dio
**58/58 PASS**. Algunas cifras: THEFT, 1.838.502 en cualquier combinación de mayúsculas; consulta por ID,
entre 39 y 51 ms.

---

## 9. Lecciones para la sesión

1. **Contexto antes que pedido.** La conversación previa con ChatGPT aportó el diseño técnico; el PRD
   decidió el producto.
2. **Reglas explícitas para la IA.** Una skill con plantilla y reglas evita que el PRD derive en detalles
   de implementación.
3. **Medir antes de escribir.** Los datos reales, como los duplicados, los vacíos o las horas
   inexistentes, cambiaron decisiones que en la conversación con ChatGPT eran supuestos.
4. **Una pregunta por vez, siempre con recomendación.** El usuario decide rápido cuando cada opción trae
   su porqué.
5. **La IA verifica antes de dar la razón.** Pasó tres veces:
   - D1: el usuario tenía razón;
   - D4: tenía razón a medias;
   - D5: la premisa era incorrecta.

   En los tres casos la evidencia fueron el código fuente y números medidos, no opiniones.
6. **El humano decide el producto.** La política tolerante de errores contradecía la recomendación del
   asistente y fue la que se aplicó.
7. **Cada decisión deja su motivo y su consecuencia aceptada.** Así nadie la relitiga después sin saber
   por qué se tomó.
8. **Fuera de alcance y criterios con números reales.** Lo primero marca dónde empieza el próximo cambio.
   Lo segundo convierte "funciona" en algo comprobable.
9. **PRD padre e hijos.** El padre describe el producto y cada hijo describe un cambio, sin repetir al
   padre. Si un hijo contradice al padre, el padre se **anota** con fecha y no se reescribe; así hizo el
   consultor con C10, C11 y C1.
10. **La IA también se equivoca, y tiene que admitirlo con pruebas.** En el consultor afirmó que el orden
    por ID salía gratis y que los textos venían en mayúsculas sin haberlo medido. El usuario lo cuestionó, el
    asistente verificó y corrigió.
11. **Las respuestas libres valen más que las opciones.** Las mejores decisiones del consultor, como la
    intersección de índices, el orden con límite y los índices en minúsculas, no estaban entre las opciones
    propuestas.
12. **Recomendar no es decidir.** El usuario rechazó varias recomendaciones, como la tabla con todos los
    campos o sobrescribir siempre. El PRD recoge lo que decidió el usuario.
13. **Recortar alcance también es una decisión de producto.** Pasar a la v1.1 y dejar el orden para otro
    cambio ("vamos más poco a poco") hizo la entrega más chica y verificable.
14. **Un PRD hijo puede obligar a cambiar a un hermano.** Los índices en minúsculas del consultor obligaron
    a modificar el generador y a subir el esquema. El PRD lo hace explícito antes de escribir código.
