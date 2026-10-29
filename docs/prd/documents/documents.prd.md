# Base de datos documental embebida sobre Krummi — PRD (borrador)

**Versión**: 0.2 (borrador)
**Fecha**: 2026-10-29
**Documento padre**: ninguno (PRD raíz de un producto aparte)
**Estado**: borrador aparcado con preguntas abiertas. Se retoma cuando esté cerrado el producto Krummi
([krummi.prd.md](../krummi/krummi.prd.md)). No ha pasado la revisión del PRD ni la revisión cruzada con Codex,
no tiene traducción y no autoriza ninguna implementación.

## Objetivo y contexto

Construir una **base de datos documental embebida**, sin servidor, que una aplicación incorpora en su propio
proceso y que guarda sus datos en un almacén de Krummi. Las aplicaciones guardan documentos con identificador,
los agrupan en colecciones y los consultan por sus campos mediante índices secundarios, desde Rust y desde otros
lenguajes.

**Por qué es un producto aparte**: el PRD raíz de Krummi deja fuera de alcance los documentos, los lenguajes de
consulta y los bindings para otros lenguajes. Este producto los añade encima de Krummi sin cambiar su alcance.

### Decisiones de producto

Acordadas con el usuario en la conversación del 2026-10-29:

| ID | Decisión |
|---|---|
| DD1 | **Formato de almacenamiento binario con acceso directo a cada campo.** Un documento se guarda en un formato binario autodescriptivo que permite leer un campo sin interpretar el resto del documento y sin copiarlo, sobre la memoria que entregan las lecturas sin copia de Krummi (D11 de Krummi). No se guarda JSON en texto ni BSON. |
| DD2 | **JSON en el borde.** Los documentos entran y salen de la base de datos como JSON; el formato binario es interno. |
| DD3 | **Índices síncronos.** Los índices secundarios se actualizan en la misma transacción de escritura que el documento: una consulta nunca ve un índice desfasado respecto a los documentos. |
| DD4 | **Consultas como árbol de consulta, sin lenguaje de texto.** Una consulta es un árbol (filtros, orden, límite, proyección) que la aplicación construye con una API fluida en su propio lenguaje. No hay lenguaje de consulta en texto (ni RQL, ni SQL, ni el dialecto de MongoDB) que analizar. |
| DD5 | **Rust valida cada consulta.** El motor comprueba la versión, los campos, los tipos y los operadores de cada árbol de consulta que recibe, venga de donde venga. |
| DD6 | **Parámetros separados del árbol de consulta.** Los valores de una consulta viajan aparte de su forma, para que dos consultas con la misma forma se reconozcan como la misma. |
| DD7 | **Interfaz para otros lenguajes**: una interfaz binaria en C, estable y pequeña. Cada lenguaje (por ejemplo .NET) ofrece su propia API fluida que construye el árbol de consulta, lo serializa y lo envía en una sola llamada. Por esa interfaz, los documentos se entregan como copias en JSON, nunca como referencias a memoria del motor. |
| DD8 | **Resultados por cursor.** Una consulta devuelve sus resultados de uno en uno, sin cargarlos todos en memoria. |
| DD9 | **Volcado legible del árbol de consulta**, solo de salida, para logs y depuración. No se puede volver a leer como consulta. |
| DD10 | **Plan elegido por costo, con conteos exactos de los índices.** El planificador estima cuántos documentos devuelve cada plan posible y elige el más barato, también cuando recorrer toda la colección cuesta menos que un índice poco selectivo que no cubre la consulta. Las estimaciones no salen de estadísticas aparte, sino de los propios índices, que están siempre al día (DD3): una igualdad cuenta los documentos de su valor, y un rango suma los de cada valor distinto del rango, sin leer ningún documento. Contar un rango se detiene al llegar al número de coincidencias a partir del cual conviene recorrer la colección, así que contar nunca cuesta más que ese umbral. La colección lleva su número de documentos al día en cada transacción. |

### Estado de partida

Verificado el 2026-10-29 sobre el repositorio:

| Aspecto | Situación |
|---|---|
| Producto documental | No hay código ni PRD previos. |
| Krummi, cerrados | Pager, transacciones, diario y recuperación, espacio libre, y árboles B+ con claves, recorrido y valores grandes (5.1 a 5.3). |
| Krummi, en curso | Árboles multivalor (5.4), con su PRD en elaboración. |
| Krummi, pendientes | Árboles de tamaño fijo y compresión de páginas hoja (resto del módulo 5), banco comparativo, tablas con índices secundarios, streams, cifrado, backup, compactación, herramienta de operación y estructuras de Corax. |
| Referencia | RavenDB guarda sus documentos en Voron con un formato binario de acceso directo y los consulta con RQL; este producto copia lo primero y descarta lo segundo (DD1, DD4). |

## Actores

- **Desarrollador Rust**: incorpora la biblioteca, guarda y consulta documentos con la API fluida de Rust, y
  además puede leer documentos sin copia.
- **Desarrollador de otro lenguaje**: usa la base de datos a través del binding de su lenguaje (por ejemplo
  .NET), que envuelve la interfaz en C.
- **Mantenedor**: mantiene la interfaz en C y los bindings, y valida que todos produzcan árboles de consulta
  equivalentes.

## Flujo

1. El desarrollador abre una base de datos sobre un almacén de Krummi.
2. Define una colección y sus índices secundarios.
3. Guarda, lee y borra documentos por identificador, en JSON.
4. Construye una consulta con la API fluida de su lenguaje y le pasa los parámetros.
5. Recibe un cursor y lee los resultados de uno en uno.

## Validaciones y mensajes

Pendiente. Depende de las preguntas abiertas. Mínimo a cubrir: JSON mal formado, documento inexistente,
identificador duplicado al crear, árbol de consulta de versión desconocida, campo o tipo inválido en una consulta,
operador desconocido, parámetro ausente o de tipo distinto, e índice inexistente.

## Salida

Pendiente: la forma del JSON que se devuelve (incluidos los metadatos del documento, si los hay) y la forma del
volcado legible (DD9).

## Reutilización

- Las transacciones, la durabilidad y la recuperación de Krummi, sin capa transaccional propia.
- Las lecturas sin copia de Krummi (D11 de Krummi) para leer campos sin copiar el documento.
- Los valores grandes de Krummi (5.3) para los documentos que no caben en una página.
- Los árboles multivalor de Krummi (5.4) para los índices secundarios (valor de un campo → documentos que lo
  tienen), o las tablas con índices secundarios (módulo 7), según la pregunta Q2.
- El número de valores de cada clave que guardan los árboles multivalor (5.4), y su recorrido de claves, que
  devuelve cada clave con ese número sin leer sus valores: son los conteos con los que el planificador estima el
  costo de cada plan (DD10).

## Preguntas abiertas

| ID | Pregunta |
|---|---|
| Q1 | **Nombre del producto.** "Base de datos documental sobre Krummi" es provisional. |
| Q2 | **Base sobre la que se construye**: directamente sobre árboles y árboles multivalor (disponibles antes), o sobre las tablas con clave primaria e índices secundarios de Krummi (módulo 7), como hace RavenDB con Voron. Decide cuándo puede empezar este producto. La base elegida tiene que dar el número de documentos de cada valor de un índice sin recorrerlos (DD10); los árboles multivalor lo dan. |
| Q3 | **Formato de serialización del árbol de consulta** en la interfaz en C: uno simple y sin esquema, o uno con esquema y generación de código para cada lenguaje. |
| Q4 | **Primer binding**: ¿.NET? ¿Hay más en la primera versión? |
| Q5 | **Operadores de la primera versión**: igualdad, rangos, prefijo, pertenencia a una lista, `y`/`o`/`no`, orden, límite, proyección. ¿Cuáles entran? |
| Q6 | **Índices**: ¿solo los que define el desarrollador, o también índices automáticos? ¿Índices compuestos de varios campos? ¿Campos dentro de listas? |
| Q7 | **Consultas sin índice**: ¿se permite recorrer toda la colección, se rechaza o se avisa? |
| Q8 | **Identificadores**: ¿los pone siempre la aplicación o la base de datos puede generarlos? ¿De qué tipo son? |
| Q9 | **Metadatos del documento**: ¿versión del documento para control de concurrencia optimista, fecha de modificación? |
| Q10 | **Esquema**: ¿colecciones sin esquema, o con validación opcional de la forma de los documentos? |
| Q11 | **Actualizaciones parciales**: ¿se reemplaza siempre el documento entero, o hay operaciones sobre campos? |
| Q12 | **Lectura del formato binario desde los bindings**: ¿se descarta o queda como mejora posterior a DD7? |
| Q13 | **Herramienta de línea de comandos** para inspeccionar documentos y consultas: ¿entra en este producto? |
| Q14 | **Índices de cobertura**: ¿un índice puede guardar además otros campos del documento, para responder una consulta que solo pide esos campos sin leer el documento? |
| Q15 | **Combinar índices**: ¿el planificador de la primera versión puede usar varios índices en una misma consulta (intersección para `y`, unión para `o`), o uno solo como máximo? |
| Q16 | **Explicar el plan**: ¿el desarrollador puede pedir el plan elegido y sus conteos, como parte del volcado legible (DD9)? |
| Q17 | **Selectividad media de un índice** (valores totales entre claves totales), para planificar sin conocer el valor del parámetro, como en un plan reutilizado entre consultas de la misma forma (DD6), y volver a planificar cuando el conteo exacto del valor real se aleja de la media. Los árboles multivalor ya dan el número total de claves, pero no el total de valores, que el 5.4 deja fuera de alcance. Anotado el 2026-10-29: un PRD posterior de Krummi lo añadiría como un contador del árbol que se mantiene en cada escritura; no entra en el 5.4. |

## Fuera de alcance

- Un lenguaje de consulta en texto: RQL, SQL o el dialecto de MongoDB (DD4).
- Compatibilidad con RavenDB o con MongoDB, en formato o en API.
- Índices asíncronos (DD3).
- Estadísticas guardadas aparte de los índices, como histogramas o muestreos: el costo sale de los conteos exactos
  de los índices (DD10).
- Servidor de red, replicación y clústeres.
- Índices de texto completo.
- Todo lo que Krummi ya deja fuera de alcance y este producto no añade, como el acceso desde varios procesos.

## Dejado a la implementación

Pendiente.

## Criterios de aceptación

Pendientes hasta resolver las preguntas abiertas.

## Referencias

- [krummi.prd.md](../krummi/krummi.prd.md): producto sobre el que se construye.
- [trees-large-values.prd.md](../krummi/trees-large-values.prd.md) y
  [trees-multivalue.prd.md](../krummi/trees-multivalue.prd.md): estructuras que reutiliza.

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 42d35e66 | 2026-10-29 09:51 UTC+00:00 | 8 h 52 min | 7 h 40 min | 1 h 12 min | 7 min | 1 h 4 min | 13 |

<!-- prd-time: [{"session": "42d35e66-27b3-40a4-a47e-68ef0d63ff8b", "start": "2026-10-29T09:51:10.546000+00:00", "wall": 31902, "discarded": 27601, "agent": 434, "human": 3866, "interventions": 13, "idle_minutes": 10}] -->
