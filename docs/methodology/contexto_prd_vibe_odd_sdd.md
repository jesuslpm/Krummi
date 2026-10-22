# Contexto de la conversación: PRD + Vibe Coding, PRD + ODD y PRD + SDD

**Fecha del resumen:** 14 de octubre de 2026  
**Destino:** documento de contexto para Claude, que está elaborando una comparación de metodologías de desarrollo asistido por agentes de IA.  
**Alcance:** recoge las ideas, preferencias, decisiones y evidencias compartidas en esta conversación. No es una evaluación científica de las metodologías ni una auditoría exhaustiva del código.

## 1. Tesis principal

La conclusión central de la conversación es:

> **El PRD es la fuente permanente de verdad funcional del producto. Las especificaciones técnicas, el diseño, las tareas y el código son artefactos derivados.**

Con un matiz: diseño y código contienen también decisiones de ingeniería propias, que no pertenecen al PRD.

El PRD **no es un prompt desechable** que se escribe para iniciar una sesión: es documentación duradera, legible por negocio, usuarios, analistas y desarrolladores, incluso por personas que nunca leerán código o especificaciones técnicas.

Cuando cambia una regla de negocio, se actualiza primero el PRD y después se trasladan sus consecuencias a las especificaciones, tareas, pruebas y código correspondientes. Las decisiones de negocio se acuerdan con una persona; las decisiones de implementación pueden delegarse a los agentes dentro de las restricciones aprobadas.

## 2. Contexto del autor del experimento

- Ingeniero de software con mucha experiencia en **C#, .NET, SQL Server, arquitectura, optimización de consultas y estructuras de datos**.
- Conoce los conceptos y algoritmos de ordenación externa/merge sort, SSTables e índices primarios y secundarios.
- Empezó el experimento **sin conocimientos previos prácticos de Rust**. Quería aprenderlo por diversión, aunque el ejercicio evolucionó hacia un caso de estudio sobre desarrollo agéntico.
- Su objetivo principal actual no es integrar IA en todos los productos de los clientes: muchos clientes no quieren asumir el coste de APIs LLM. Quiere **aprovechar la IA como herramienta para construir aplicaciones convencionales**, con autonomía y calidad. Si un cliente necesitara funcionalidad de IA, podría abordarla también, preferentemente con **C#/.NET** cuando sea apropiado.
- Ya trabaja en paralelo con **Git worktrees**, dejando a un agente implementar un cambio mientras él avanza en otro.

## 3. El problema detectado en la formación y las herramientas

El usuario observa que numerosos libros y cursos enseñan:

- Vibe coding como diálogo informal con un LLM.
- SDD como secuencia manual de documentos y pasos (`proposal → specs → design → tasks → implementation`).
- Poca utilización práctica de herramientas de orquestación como **OpenSpec, Spec Kit y Gentle AI**.

Su crítica no es que aprender manualmente las fases sea inútil, sino que **enseñar solo la ejecución manual puede dejar una visión incompleta de los flujos de trabajo reales con agentes**.

También considera que `proposal`, `design` y `tasks` son artefactos principalmente orientados a ingeniería, mientras que las `specs`, aunque expresan comportamiento, pueden resultar densas y fragmentadas para lectores de negocio. **Un PRD presenta el comportamiento del producto en lenguaje de negocio, con un flujo coherente y reglas legibles**.

Se aclaró que OpenSpec sí contempla especificaciones de comportamiento; la diferencia fundamental propuesta no es la inexistencia de requisitos en otras herramientas, sino **el PRD como documento funcional independiente, permanente y autorizado antes de iniciar la implementación**.

## 4. El skill PRD propio

El usuario desarrolló un skill `prd` y una plantilla `prd-template.md`, compartidos en la conversación y publicados como referencia en un Gist:

https://gist.github.com/jesuslpm/fc36d3503116171eee409b313365cde7

### Principios del skill

1. **WHAT y WHY, no HOW**: lenguaje de negocio; sin nombres de clases, endpoints, rutas de código ni números de línea.
2. **El PRD se acuerda conversando con el usuario**: preguntar una decisión de producto cada vez cuando realmente haga falta; nunca inventar reglas de negocio.
3. **Investigar el estado inicial** en código y datos; no suponer qué existe.
4. **PRDs jerárquicos**: PRD general de dominio y PRDs de cambios que lo referencian mediante `Parent document`.
5. **Fuera de alcance explícito y obligatorio**, para delimitar el cambio y el inicio del siguiente.
6. **Criterios de aceptación verificables**, normalmente *Given / When / Then*.
7. **Elaborar un PRD no autoriza implementar**: se necesita autorización posterior.
8. Bugs, refactorizaciones y tooling sin cambio de comportamiento del producto **no requieren necesariamente PRD** y pueden ir directamente a ODD.
9. El PRD es la autoridad en cuestiones funcionales; el documento de ejecución ODD se deriva de él.

### Estructura de la plantilla

- Objective and context
- Starting state
- Actors
- Flow
- Validations and messages
- Reuse
- Out of scope
- Acceptance criteria
- References

El PRD generado en el experimento añadió también **Decisiones**, con motivaciones y consecuencias explícitas, que resultaron útiles para cerrar cuestiones de producto antes de programar.

## 5. Comparación conceptual de las tres combinaciones

**Este apartado es una propuesta de encuadre para el documento de Claude, no el resultado de una comparación empírica controlada.**

| Aspecto | PRD + Vibe Coding | PRD + ODD | PRD + SDD |
|---|---|---|---|
| Fuente funcional | PRD aprobado | PRD aprobado | PRD aprobado |
| Después del PRD | Conversación directa e iterativa con la IA | Flujo orgánico con exploración, tareas, delegación y verificación proporcional | Fases formales de especificación, diseño, tareas, implementación y verificación |
| Formalidad técnica | Variable; depende de instrucciones y disciplina | Adaptativa | Más estructurada |
| Carga de coordinación humana | Potencialmente mayor en cambios largos, si no se añaden prácticas explícitas | El objetivo es delegar gran parte de la ejecución y conservar progreso/evidencias | El objetivo es una ejecución sistemática y trazable por fases |
| Uso interesante | Prototipos, tareas delimitadas, exploración, cambios sencillos | Funcionalidades reales con autonomía y bajo coste de ceremonia | Cambios de alto riesgo o complejidad que justifican artefactos técnicos formales |
| Riesgo a evaluar | Deriva de requisitos y continuidad del contexto | Pérdida o reinterpretación de requisitos en los handoffs | Sobrecarga documental y coordinación |

**Importante:** Vibe coding no queda automáticamente validado por anteponer un PRD; sigue siendo necesario fijar límites, tests y comprobaciones. Tampoco debe afirmarse que PRD + ODD sea objetivamente superior a PRD + SDD: falta una comparación controlada.

### Separación de responsabilidades común a los tres enfoques

**Colaboración humano + IA → PRD → aprobación humana → método de ejecución elegido → verificaciones contra PRD.**

El agente puede diseñar y escribir el software, pero **no debe alterar silenciosamente requisitos funcionales**. Si encuentra una decisión de negocio abierta, vuelve al usuario.

## 6. Experiencia previa con PRD + SDD

Antes de la actualización de Gentle AI, el usuario trabajaba habitualmente con **PRD + SDD**:

- Invertía el mayor esfuerzo en elaborar y revisar el PRD, utilizando un modelo potente (**Opus 5.5 Extra High**, según lo indicó).
- Una vez cerradas las preguntas de negocio, entregaba los requisitos a SDD.
- Según su experiencia, **rara vez necesitaba corregir el SDD** cuando el PRD estaba muy bien definido.
- Dejaba la implementación durante horas y aprovechaba worktrees para trabajar paralelamente.

Es una **observación personal**, no una medición comparativa de tasas de errores o intervenciones.

### Cambio en Gentle AI

En la conversación se identificó que la versión 4 de Gentle AI retiró los skills y el flujo SDD/OpenSpec y orientó la herramienta a ODD. El usuario presentó una issue solicitando que SDD pudiera coexistir de forma opcional con ODD, manteniendo ODD como opción predeterminada:

https://github.com/Gentleman-Programming/gentle-ai/issues/5475

Posteriormente presentó otra solicitud proponiendo un **skill PRD opcional previo a ODD**, y añadió en un comentario el Gist con su implementación. No es imprescindible que la integren: su skill propio ya le resulta suficiente. La motivación de compartirlo es contribuir a la metodología de desarrollo con agentes.

## 7. Experimento real: Chicago Crime, Rust, PRD + ODD

Repositorio público:

https://github.com/jesuslpm/chicago-crime

El proyecto parte de un CSV público de delitos de Chicago. Su primera entrega es un **generador en Rust** que transforma el CSV en un conjunto inmutable, consultable posteriormente mediante SSTables con claves/valores codificados como tuplas de FoundationDB. El **consultor aún no estaba implementado** al momento de las evidencias compartidas.

### Requisitos funcionales y restricciones relevantes

- CSV de referencia de **2.406.401.982 bytes** (Windows muestra **2,24 GiB**).
- **8.656.904 registros** y 22 columnas.
- SSTable primaria ordenada por ID y **cinco índices secundarios**: fecha, tipo principal, distrito, área comunitaria y número de caso.
- Tratamiento explícito de duplicados, nulos y registros inválidos.
- Fechas como **hora civil local de Chicago**, evitando inventar conversiones UTC en horas ambiguas/inexistentes por horario de verano.
- Presupuesto de **512 MiB de pico de memoria privada comprometida del proceso**.
- Publicación de datasets completos por versión, evitando exponer una generación parcial.
- El PRD contiene reglas de negocio, mensajes, límites de alcance, decisiones justificadas y criterios de aceptación mensurables.

### Traducción a ODD

ODD creó un documento técnico persistente:

`odd/tasks/crime-generator.md`

En el encabezado del documento se afirma expresamente: **“The PRDs win on any product question.”**

El plan técnico se dividió en siete tareas principales:

1. Workspace y contrato compartido de dataset.
2. Ingesta y validación del CSV.
3. Ordenación externa, duplicados y SSTable primaria.
4. SSTables de índices secundarios.
5. Publicación atómica, manifiesto y sobrescritura.
6. Cancelación, medición de memoria, resúmenes y códigos de salida.
7. Aceptación sobre el CSV de referencia.

El flujo recogía **test-first RED → GREEN**, `cargo fmt`, `clippy`, `cargo test`, commits por unidad de trabajo y revisión automática RDD. El README indica que algunas revisiones crearon tareas correctivas, como `T2a` y `T5a`.

La implementación se divide en:

- `crates/crime-dataset`: contrato y codificación compartidos.
- `crates/crime-generator`: aplicación generadora.
- `crates/crime-acceptance`: verificador independiente.

La verificación independiente intenta evitar reutilizar la propia lógica del generador al comprobar datos y SSTables.

### Resultados reales compartidos

Ejecución del generador en Windows:

| Métrica | Resultado |
|---|---:|
| Registros válidos | **8.656.904** |
| Registros rechazados | **0** |
| Tiempo de generación | **90,9 segundos** |
| Velocidad efectiva global | **≈95.236 registros/s** |
| Pico de memoria privada comprometida | **93,6 MiB** |
| Límite establecido | **512 MiB** |
| Segmentos temporales para registros | **35** |
| Segmentos temporales para índices | **44** |
| Entradas de la tabla primaria | **8.656.904** |
| Entradas combinadas de índices secundarios | **43.284.520** |
| Entradas totales en las seis tablas | **51.941.424** |
| Tamaño de SSTable primaria | **907.053.690 bytes = 865,1 MiB** |
| Tamaño de los cinco índices | **194.752.524 bytes ≈ 185,7 MiB** |
| Tamaño total de SSTables | **1.101.806.214 bytes ≈ 1.050,8 MiB** |
| Reducción respecto del CSV (en bytes) | **≈54,2 %** |

La cifra de velocidad incluye lectura/parsing del CSV, validación, ordenación externa, serialización, escritura/compression de la SSTable primaria, producción y ordenación de índices secundarios y escritura de las cinco SSTables secundarias. **No es solo velocidad del parser CSV.**

El dataset utiliza compresión Snappy. Cada índice contiene una entrada por cada registro válido.

SHA-256 del CSV de referencia: `5cb5fa41c930ece2e91f683d894c815d46333d2a5372b8df5ae40b6e8a6ae35d`.

### Tests y revisión

Se adjuntó la salida de `cargo test --workspace`, con **230 tests superados, cero fallidos**. Incluye, entre otros:

- Codificación/decodificación y orden de claves.
- Fechas y cambios de horario.
- Filas inválidas, errores de lectura y duplicados entre segmentos.
- Ordenación externa, segmentos corruptos y limpieza de temporales.
- Índices correctos y lectura independiente de SSTables.
- Determinismo binario en fixtures.
- Fallos de escritura y disco lleno.
- Cancelaciones en distintas etapas y conservación del dataset previo.
- Publicación por versiones.
- Pruebas del verificador de aceptación independiente.

El repositorio fue inspeccionado parcialmente en esta conversación. Se observaron decisiones técnicas concretas:

- Ordenación externa mediante `BinaryHeap` y fusión k-way, con memoria acotada y limpieza RAII.
- Orden estable por `(id, línea_csv)` para conservar la primera aparición de un duplicado.
- Validación estricta de orden de claves antes de escribir SSTables y manejo de escrituras parciales/errores de I/O.
- Publicación por versiones con un archivo `CURRENT` como referencia activa.

**Precaución:** aunque los resultados y tests compartidos son sólidos, no equivalen a una auditoría completa ni a una comparación de metodologías. En particular, se señaló que merecía verificar exhaustivamente en Windows las garantías del reemplazo de `CURRENT` mediante `std::fs::rename`, especialmente con lectores concurrentes. Los tests automatizados compartidos no sustituyen por sí solos al informe completo de aceptación sobre los 8,66 millones de registros.

### Estado del proyecto al cerrar la conversación

El **generador existe y se ha ejecutado correctamente**. El consultor todavía está pendiente. Se propuso aprender Rust posteriormente haciendo que Claude explique el código **desde la perspectiva de alguien que domina C#**, centrado en ownership, borrowing, lifetimes, iteradores, traits, `Result`, RAII, I/O y equivalencias/diferencias idiomáticas, no volviendo a enseñar los algoritmos conocidos.

## 8. Qué se ha aprendido y qué todavía no

### Observaciones apoyadas por el experimento

1. Un PRD cuidadosamente trabajado puede proporcionar un contrato funcional suficientemente preciso para que ODD genere un plan de implementación detallado.
2. El documento ODD diferencia decisiones de producto (autoridad del PRD) y decisiones técnicas (su propio ámbito).
3. El agente produjo y ejecutó una implementación Rust no trivial, aunque el usuario no conocía Rust antes del experimento.
4. Se han obtenido cifras reales de rendimiento, consumo de memoria y tamaño del dataset, junto con una batería amplia de pruebas.
5. La presencia de artefactos persistentes (PRD, ODD, tests, README) hace que el experimento sea más auditable y compartible que una simple demostración conversacional.

### Hipótesis pendientes de medir

- Que **PRD + ODD** requiere menos intervención humana que **PRD + SDD**.
- Que **PRD + ODD** cuesta menos tiempo o tokens que **PRD + SDD** con calidad equiparable.
- Que **PRD + Vibe Coding** produce resultados comparables en cambios equivalentes bajo pruebas justas.
- Que la calidad del PRD por sí sola sea suficiente para una autonomía alta: también importan arquitectura, agentes, herramientas y verificaciones.

### Métricas recomendadas para el futuro artículo o curso

- Horas invertidas por el humano en el PRD y su revisión.
- Modelo/esfuerzo utilizado para el PRD y para la implementación.
- Tiempo de ejecución del agente y coste/tokens, cuando estén disponibles.
- Número y tipo de intervenciones humanas durante implementación.
- Cambios de requisitos frente a correcciones puramente técnicas.
- Tests, criterios de aceptación superados, defectos encontrados por revisores.
- Rendimiento y recursos utilizados por el software resultante.
- Repetición de un cambio comparable con cada metodología, si se busca una comparación empírica.

## 9. Enfoque propuesto para el documento comparativo

Evitar equiparar **PRD + Vibe Coding**, **PRD + ODD** y **PRD + SDD** como si fueran tres frameworks completos y equivalentes. En los tres casos el PRD es **una fase previa y común**; lo que cambia es el mecanismo posterior de implementación, coordinación, verificación y persistencia.

Una explicación útil podría distinguir:

1. **Requisitos funcionales (permanentes):** PRD, escrito para personas y fuente de verdad del producto.
2. **Ejecución (variable):** vibe coding conversacional, ODD orgánico o SDD estructurado.
3. **Evidencia (imprescindible):** validación de criterios de aceptación, pruebas y revisión.

El mensaje central del usuario no es “la IA sustituye a los ingenieros”, ni “ya no hace falta aprender lenguajes”; es que **un ingeniero con conocimiento del dominio, algoritmos, arquitectura e invariantes puede dirigir a agentes para implementar en tecnologías que no domina todavía, conservando la responsabilidad de especificar y evaluar el resultado**.

También conviene enfatizar que el PRD no debe forzar ceremonias para cada bug o refactorización; su uso es proporcional al cambio funcional.

## 10. Fuentes y artefactos compartidos

- Repositorio del experimento: https://github.com/jesuslpm/chicago-crime
- Skill PRD y plantilla: https://gist.github.com/jesuslpm/fc36d3503116171eee409b313365cde7
- Documento ODD del generador: https://github.com/jesuslpm/chicago-crime/blob/main/odd/tasks/crime-generator.md
- PRD padre: https://github.com/jesuslpm/chicago-crime/blob/main/docs/prd/chicago-crime/chicago-crime.prd.md
- PRD generador: https://github.com/jesuslpm/chicago-crime/blob/main/docs/prd/chicago-crime/generador.prd.md
- README y metodología: https://github.com/jesuslpm/chicago-crime/blob/main/README.md
- Issue para soporte opcional de SDD: https://github.com/Gentleman-Programming/gentle-ai/issues/5475
- Dataset público original: https://data.cityofchicago.org/Public-Safety/Crimes-2001-to-Present/ijzp-q8t2

---

**Instrucción sugerida para Claude:** Usa este documento como contexto de la experiencia y la posición metodológica del usuario. Distingue los hechos observados de las opiniones e hipótesis. No presentes PRD + ODD como ganador de una comparación que aún no se ha realizado. Conserva como tesis central que el PRD es documentación funcional permanente y que los artefactos técnicos se derivan de él.
